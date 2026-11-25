#!/usr/bin/env python3
"""Convierte un .inp de SWMM 5 o de EPANET 2.2 a `redes.plan/1`.

No calcula nada: lee, convierte unidades (US -> SI) y escribe el contrato de entrada de Redes.
Un dato que el archivo no trae queda ausente y se avisa; nunca se rellena con un valor «razonable».

Uso:
    python inp_to_plan.py --tipo swmm   red.inp --salida plan.json [--sistema pluvial|sanitario|combinado]
    python inp_to_plan.py --tipo epanet red.inp --salida plan.json [--reservorio agua.captacion:toma_directa]
    ... [--coords coords.csv] [--informe informe.json]

Columnas según los manuales oficiales (SWMM 5.2 User's Manual, apéndice D; EPANET 2.2, apéndice C):

SWMM
  [OPTIONS]    FLOW_UNITS (CFS|GPM|MGD = pies; CMS|LPS|MLD = metros; si falta, SWMM asume CFS),
               LINK_OFFSETS (DEPTH por defecto | ELEVATION)
  [JUNCTIONS]  Name Elev MaxDepth InitDepth SurDepth Aponded
  [OUTFALLS]   Name Elev Type [Stage|Tseries] [Gated] [RouteTo]
  [DIVIDERS]   Name Elev DivLink Type (params) MaxDepth ...
  [STORAGE]    Name Elev MaxDepth InitDepth Shape ...
  [CONDUITS]   Name From To Length Roughness InOffset OutOffset InitFlow MaxFlow
  [PUMPS]      Name From To Curve Status Startup Shutoff
  [ORIFICES]   Name From To Type Offset Qcoeff Gated CloseTime
  [WEIRS]      Name From To Type CrestHt Qcoeff Gated EndCon EndCoeff Surcharge ...
  [XSECTIONS]  Link Shape Geom1 Geom2 Geom3 Geom4 Barrels Culvert
  [VERTICES]   Link X Y

EPANET
  [OPTIONS]    Units (CFS|GPM|MGD|IMGD|AFD = US; LPS|LPM|MLD|CMH|CMD|CMS = SI; si falta, GPM),
               Headloss (H-W por defecto | D-W | C-M)
  [JUNCTIONS]  ID Elev Demand Pattern
  [RESERVOIRS] ID Head Pattern
  [TANKS]      ID Elev InitLvl MinLvl MaxLvl Diam MinVol VolCurve Overflow
  [PIPES]      ID Node1 Node2 Length Diam Roughness MinorLoss Status
  [PUMPS]      ID Node1 Node2 (HEAD curva | POWER valor | SPEED | PATTERN)
  [VALVES]     ID Node1 Node2 Diam Type Setting MinorLoss
  [DEMANDS]    Junction Demand Pattern ;Category
  [VERTICES]   Link X Y

Unidades US: longitudes y cotas en pies; diámetros de tubería y válvula de EPANET en pulgadas;
diámetro de tanque de EPANET y geometrías de sección de SWMM en pies. Las coordenadas de planta
no se convierten: son unidades de mapa y dependen del CRS del proyecto.

El mapeo de familias y sus motivos está en `references/mapeos.md`.
"""

from __future__ import annotations

import argparse
import json
import math
import re
import sys
from dataclasses import dataclass, field
from pathlib import Path

PLAN_SCHEMA = "redes.plan/1"

PIE = 0.3048  # m
PULGADA_MM = 25.4  # mm
# EPANET 2.2 (src/types.h): PSIperFT = 0.4333 y KWperHP = 0.7457.
PSI_A_MCA = PIE / 0.4333
HP_A_KW = 0.7457

SWMM_UNIDADES = {"CFS": "US", "GPM": "US", "MGD": "US", "CMS": "SI", "LPS": "SI", "MLD": "SI"}
EPANET_UNIDADES = {
    "CFS": "US", "GPM": "US", "MGD": "US", "IMGD": "US", "AFD": "US",
    "LPS": "SI", "LPM": "SI", "MLD": "SI", "CMH": "SI", "CMD": "SI", "CMS": "SI",
}
# Factor a L/s de cada unidad de caudal (conversión de unidades, no cálculo).
A_LPS = {
    "CFS": 28.316846592, "GPM": 0.0630901964, "MGD": 43.812636389, "IMGD": 52.616782,
    "AFD": 14.276410, "LPS": 1.0, "LPM": 1.0 / 60.0, "MLD": 1.0e6 / 86400.0,
    "CMH": 1.0 / 3.6, "CMD": 1.0 / 86.4, "CMS": 1000.0,
}

SISTEMAS = ("pluvial", "sanitario", "combinado")
SUBTIPO_COLECTOR = {
    "pluvial": "colector_pluvial",
    "sanitario": "colector_sanitario",
    "combinado": "colector_combinado",
}

# SWMM: secciones circulares (se dibujan como tubo con diámetro).
SWMM_CIRCULARES = {"CIRCULAR", "FORCE_MAIN", "FILLED_CIRCULAR"}
# SWMM: secciones cerradas no circulares. Son tuberías, pero Redes sólo dibuja tubo circular:
# entran sin diámetro y con aviso.
SWMM_CERRADAS = {
    "EGG", "HORSESHOE", "GOTHIC", "CATENARY", "SEMIELLIPTICAL", "BASKETHANDLE", "SEMICIRCULAR",
    "RECT_TRIANGULAR", "RECT_ROUND", "MODBASKETHANDLE", "HORIZ_ELLIPSE", "VERT_ELLIPSE", "ARCH",
    "CUSTOM",
}

# EPANET: tipo de válvula -> (subtipo de agua.valvula, valor de `tipo_valvula` o None).
EPANET_VALVULAS = {
    "PRV": ("reduccion_presion", "reduccion_presion"),
    "PSV": ("sostenedora", None),
    "FCV": ("reguladora_caudal", "reguladora"),
}


class ErrorEntrada(Exception):
    """El archivo no permite decidir algo sin adivinar; se pide al usuario."""


@dataclass
class Contexto:
    fuente: str
    coords: dict[str, tuple[float, float]]
    vertices: dict[str, list[tuple[float, float]]]
    avisos: list[str] = field(default_factory=list)
    descartes: list[str] = field(default_factory=list)

    def dato(self, valor: float, unidad: str) -> dict:
        return {"valor": r4(valor), "unidad": unidad, "fuente": self.fuente}


@dataclass
class Resultado:
    plan: dict
    avisos: list[str]
    descartes: list[str]


# ---------------------------------------------------------------------------------------------
# Lectura


def read_sections(path: Path) -> dict[str, list[list[str]]]:
    """Devuelve {SECCION: [campos...]} conservando el orden de aparición."""
    sections: dict[str, list[list[str]]] = {}
    current: str | None = None
    for raw in path.read_text(encoding="utf-8-sig", errors="replace").splitlines():
        line = raw.split(";", 1)[0].strip()
        if not line:
            continue
        m = re.match(r"^\[(.+?)\]$", line)
        if m:
            current = m.group(1).strip().upper()
            sections.setdefault(current, [])
            continue
        if current is None:
            continue
        sections[current].append(line.split())
    return sections


def num(value: str | None) -> float | None:
    """Número o None. Nunca un valor por defecto: lo ausente queda ausente."""
    try:
        return float(value)  # type: ignore[arg-type]
    except (TypeError, ValueError):
        return None


def col(row: list[str], i: int) -> str | None:
    return row[i] if len(row) > i else None


def r4(v: float) -> float:
    return round(v, 4)


def opciones(sections: dict[str, list[list[str]]]) -> dict[str, list[str]]:
    return {row[0].upper(): row[1:] for row in sections.get("OPTIONS", []) if row}


def coordenadas_del_inp(sections: dict[str, list[list[str]]]) -> dict[str, tuple[float, float]]:
    """Lee `[COORDINATES]` (SWMM y EPANET usan el mismo formato: ID X Y)."""
    coords: dict[str, tuple[float, float]] = {}
    for fila in sections.get("COORDINATES", []):
        x, y = num(col(fila, 1)), num(col(fila, 2))
        if x is not None and y is not None:
            coords[fila[0]] = (x, y)
    return coords


def vertices_del_inp(sections: dict[str, list[list[str]]]) -> dict[str, list[tuple[float, float]]]:
    """Lee `[VERTICES]` (Link X Y, en orden de aparición)."""
    out: dict[str, list[tuple[float, float]]] = {}
    for fila in sections.get("VERTICES", []):
        x, y = num(col(fila, 1)), num(col(fila, 2))
        if x is not None and y is not None:
            out.setdefault(fila[0], []).append((x, y))
    return out


def coordenadas_csv(path: Path) -> dict[str, tuple[float, float]]:
    coords: dict[str, tuple[float, float]] = {}
    for line in path.read_text(encoding="utf-8-sig", errors="replace").splitlines():
        parts = [p.strip() for p in re.split(r"[,;]", line) if p.strip()]
        if len(parts) >= 3:
            x, y = num(parts[1]), num(parts[2])
            if x is not None and y is not None:
                coords[parts[0]] = (x, y)
    return coords


# ---------------------------------------------------------------------------------------------
# Piezas comunes


def nodo(
    ctx: Contexto,
    codigo: str,
    familia: str,
    subtipo: str,
    *,
    cota: float | None = None,
    cota_terreno: float | None = None,
    attrs: dict | None = None,
    datos: dict | None = None,
    en: str | None = None,
) -> dict | None:
    """Nodo con planta. `en` = código del nodo cuya planta se usa (dispositivos de enlace)."""
    xy = ctx.coords.get(en or codigo)
    if xy is None:
        donde = f"el nodo {en}" if en else "el nodo"
        ctx.descartes.append(
            f"nodo {codigo}: {donde} no tiene coordenadas ([COORDINATES] ni --coords); "
            "no se inventa la planta"
        )
        return None
    o: dict = {
        "tipo": "nodo",
        "codigo": codigo,
        "familia": familia,
        "subtipo": subtipo,
        "e": xy[0],
        "n": xy[1],
    }
    if cota is not None:
        o["cota"] = r4(cota)
    if cota_terreno is not None:
        o["cota_terreno"] = r4(cota_terreno)
    if attrs:
        o["attrs"] = attrs
    if datos:
        o["datos"] = datos
    return o


def trazado(
    ctx: Contexto, link: str, a: str, b: str, z_a: float | None, z_b: float | None
) -> list[list[float]] | None:
    """Trazado 3D desde la planta de los extremos y los `[VERTICES]` del enlace.

    La cota de los vértices intermedios se reparte en proporción a la longitud en planta entre
    las cotas de los extremos: es la definición del propio modelo (SWMM y EPANET tratan el tramo
    como una recta entre sus extremos), no un cálculo de diseño.
    """
    interior = ctx.vertices.get(link, [])
    pa, pb = ctx.coords.get(a), ctx.coords.get(b)
    if pa is None or pb is None:
        return None
    if z_a is None or z_b is None:
        if interior:
            ctx.avisos.append(f"{link}: sin cota en un extremo; sus vértices de planta no se usan")
        return None
    pts = [pa, *interior, pb]
    acum = [0.0]
    for p, q in zip(pts, pts[1:]):
        acum.append(acum[-1] + math.hypot(q[0] - p[0], q[1] - p[1]))
    total = acum[-1]
    out = []
    for p, s in zip(pts, acum):
        t = s / total if total > 0 else 0.0
        out.append([p[0], p[1], r4(z_a + (z_b - z_a) * t)])
    return out


def arco(
    ctx: Contexto,
    emitidos: set[str],
    codigo: str,
    familia: str,
    subtipo: str,
    desde: str,
    hasta: str,
    *,
    diametro_mm: int | None = None,
    vertices: list[list[float]] | None = None,
    attrs: dict | None = None,
    datos: dict | None = None,
) -> dict | None:
    faltan = [x for x in (desde, hasta) if x not in emitidos]
    if faltan:
        ctx.descartes.append(
            f"arco {codigo}: extremo {', '.join(faltan)} no importado ({desde} -> {hasta})"
        )
        return None
    o: dict = {
        "tipo": "arco",
        "codigo": codigo,
        "familia": familia,
        "subtipo": subtipo,
        "desde": desde,
        "hasta": hasta,
    }
    if diametro_mm is not None:
        o["diametro_mm"] = diametro_mm
    if vertices:
        o["vertices"] = vertices
    if attrs:
        o["attrs"] = attrs
    if datos:
        o["datos"] = datos
    return o


def agregar(lista: list[dict], o: dict | None, emitidos: set[str] | None = None) -> None:
    if o is not None:
        lista.append(o)
        if emitidos is not None:
            emitidos.add(o["codigo"])


# ---------------------------------------------------------------------------------------------
# SWMM


def detectar_sistema(sections: dict[str, list[list[str]]]) -> tuple[str | None, str]:
    """Pluvial, sanitario o combinado según lo que declara el archivo. Devuelve (sistema, motivo)."""
    ops = opciones(sections)
    ignora_lluvia = (ops.get("IGNORE_RAINFALL") or ["NO"])[0].upper() == "YES"
    lluvia = bool(sections.get("SUBCATCHMENTS")) and not ignora_lluvia
    dwf = bool(sections.get("DWF"))
    if lluvia and dwf:
        return "combinado", "tiene [SUBCATCHMENTS] y [DWF]"
    if lluvia:
        return "pluvial", "tiene [SUBCATCHMENTS] y no [DWF]"
    if dwf:
        return "sanitario", "tiene [DWF] y no subcuencas con lluvia"
    titulo = " ".join(" ".join(r) for r in sections.get("TITLE", [])).lower()
    pl = any(k in titulo for k in ("pluvial", "storm", "lluvia", "drenaje"))
    sa = any(k in titulo for k in ("sanitari", "sanitary", "residual", "servidas", "wastewater"))
    if pl and not sa:
        return "pluvial", "el [TITLE] lo dice"
    if sa and not pl:
        return "sanitario", "el [TITLE] lo dice"
    if pl and sa:
        return "combinado", "el [TITLE] nombra ambos"
    return None, "sin [SUBCATCHMENTS], sin [DWF] y el [TITLE] no lo dice"


def swmm_objetos(
    sections: dict[str, list[list[str]]], ctx: Contexto, sistema_arg: str | None
) -> list[dict]:
    ops = opciones(sections)
    unidades = (ops.get("FLOW_UNITS") or [None])[0]
    if unidades is None:
        unidades = "CFS"
        ctx.avisos.append("[OPTIONS] sin FLOW_UNITS: SWMM asume CFS, así que cotas y longitudes en pies")
    unidades = unidades.upper()
    if unidades not in SWMM_UNIDADES:
        raise ErrorEntrada(f"FLOW_UNITS desconocido: {unidades}")
    f = PIE if SWMM_UNIDADES[unidades] == "US" else 1.0
    modo = ((ops.get("LINK_OFFSETS") or ["DEPTH"])[0]).upper()

    detectado, motivo = detectar_sistema(sections)
    if sistema_arg:
        sistema = sistema_arg
        if detectado and detectado != sistema_arg:
            ctx.avisos.append(
                f"--sistema {sistema_arg} manda, aunque el archivo parece {detectado} ({motivo})"
            )
    elif detectado:
        sistema = detectado
        ctx.avisos.append(f"sistema {sistema}: {motivo}")
    else:
        raise ErrorEntrada(
            f"no se puede saber si la red es pluvial, sanitaria o combinada ({motivo}); "
            "indíquelo con --sistema pluvial|sanitario|combinado"
        )

    xsec = {row[0]: row for row in sections.get("XSECTIONS", []) if len(row) >= 2}
    cotas: dict[str, float] = {}  # solera de cada nodo, en m
    nodos: list[dict] = []
    emitidos: set[str] = set()

    # Las estaciones de bombeo: un almacenamiento del que aspira una bomba es su cámara húmeda.
    bombas = [r for r in sections.get("PUMPS", []) if len(r) >= 3]
    aspiran_de = {r[1] for r in bombas}

    def profundidad(codigo: str, valor: float | None, que: str) -> float | None:
        if valor is None or valor <= 0:
            ctx.avisos.append(f"{codigo}: {que} = 0 o ausente; cota de terreno ausente (no se supone)")
            return None
        return valor * f

    for row in sections.get("JUNCTIONS", []):
        elev = num(col(row, 1))
        if elev is None:
            ctx.descartes.append(f"nodo {row[0]}: sin cota de solera")
            continue
        name, z = row[0], elev * f
        cotas[name] = z
        prof = profundidad(name, num(col(row, 2)), "MaxDepth")
        agregar(
            nodos,
            nodo(
                ctx, name, "saneamiento.pozo", "inspeccion",
                cota=z,
                cota_terreno=z + prof if prof is not None else None,
                attrs={"profundidad": r4(prof)} if prof is not None else None,
                datos={"profundidad_max": ctx.dato(prof, "m")} if prof is not None else None,
            ),
            emitidos,
        )

    n_params = {"OVERFLOW": 0, "CUTOFF": 1, "TABULAR": 1, "WEIR": 3}
    for row in sections.get("DIVIDERS", []):
        elev = num(col(row, 1))
        tipo = (col(row, 3) or "").upper()
        if elev is None or tipo not in n_params:
            ctx.descartes.append(f"nodo {row[0]}: divisor de caudal ilegible")
            continue
        name, z = row[0], elev * f
        cotas[name] = z
        prof = profundidad(name, num(col(row, 4 + n_params[tipo])), "MaxDepth")
        agregar(
            nodos,
            nodo(
                ctx, name, "saneamiento.pozo", "inspeccion",
                cota=z,
                cota_terreno=z + prof if prof is not None else None,
                attrs={"profundidad": r4(prof)} if prof is not None else None,
            ),
            emitidos,
        )

    for row in sections.get("OUTFALLS", []):
        elev = num(col(row, 1))
        if elev is None:
            ctx.descartes.append(f"nodo {row[0]}: sin cota de solera")
            continue
        name, z = row[0], elev * f
        cotas[name] = z
        o = nodo(
            ctx, name, "saneamiento.descarga", "emisario_muro",
            cota=z,
            attrs={"tipo_descarga": "emisario", "cota_entrega": r4(z)},
        )
        if o:
            ctx.avisos.append(
                f"{name}: subtipo emisario_muro genérico; SWMM no describe la estructura de descarga"
            )
        agregar(nodos, o, emitidos)

    for row in sections.get("STORAGE", []):
        elev = num(col(row, 1))
        if elev is None:
            ctx.descartes.append(f"nodo {row[0]}: sin cota de solera")
            continue
        name, z = row[0], elev * f
        cotas[name] = z
        ymax = num(col(row, 2))
        prof = ymax * f if ymax and ymax > 0 else None
        datos = {"profundidad_max": ctx.dato(prof, "m")} if prof is not None else None
        if name in aspiran_de:
            o = nodo(
                ctx, name, "saneamiento.bombeo", "camara_humeda", cota=z,
                attrs={"profundidad": r4(prof)} if prof is not None else None, datos=datos,
            )
        else:
            o = nodo(
                ctx, name, "saneamiento.camara", "compensacion", cota=z,
                attrs={"altura_util": r4(prof)} if prof is not None else None, datos=datos,
            )
        agregar(nodos, o, emitidos)

    def cota_extremo(nodo_id: str, raw: str | None) -> float | None:
        base = cotas.get(nodo_id)
        if base is None:
            return None
        if raw is None or raw == "*":
            return base
        v = num(raw)
        if v is None:
            return base
        return base + v * f if modo == "DEPTH" else v * f

    subtipo_colector = SUBTIPO_COLECTOR[sistema]
    arcos: list[dict] = []
    for row in sections.get("CONDUITS", []):
        if len(row) < 5:
            ctx.descartes.append(f"arco {row[0]}: fila de [CONDUITS] incompleta")
            continue
        name, frm, to = row[0], row[1], row[2]
        largo, rough = num(row[3]), num(row[4])
        z_in, z_out = cota_extremo(frm, col(row, 5)), cota_extremo(to, col(row, 6))
        datos: dict = {}
        if largo is not None:
            datos["longitud_origen"] = ctx.dato(largo * f, "m")
        if z_in is not None:
            datos["cota_solera_inicio"] = ctx.dato(z_in, "m")
            if abs(z_in - cotas[frm]) > 1e-9:
                datos["desfase_entrada"] = ctx.dato(z_in - cotas[frm], "m")
        if z_out is not None:
            datos["cota_solera_fin"] = ctx.dato(z_out, "m")
            if abs(z_out - cotas[to]) > 1e-9:
                datos["desfase_salida"] = ctx.dato(z_out - cotas[to], "m")
        attrs: dict = {}
        if rough is not None:
            attrs["rugosidad_n"] = rough

        xs = xsec.get(name)
        forma = xs[1].upper() if xs else None
        g = [num(col(xs, i)) if xs else None for i in range(2, 6)]
        barriles = num(col(xs, 6)) if xs else None
        familia, subtipo, diam = "saneamiento.tuberia", subtipo_colector, None
        if forma is None:
            ctx.avisos.append(f"{name}: sin [XSECTIONS]; diámetro ausente")
        elif forma in SWMM_CIRCULARES:
            if g[0]:
                diam = int(round(g[0] * f * 1000.0))
            if forma == "FORCE_MAIN":
                subtipo = "impulsion"
                if g[1]:
                    ecuacion = ((ops.get("FORCE_MAIN_EQUATION") or ["H-W"])[0]).upper()
                    if ecuacion == "H-W":
                        attrs["coef_hazen"] = g[1]
                    else:
                        attrs["rugosidad_abs"] = r4(g[1] * f * 1000.0)
            if forma == "FILLED_CIRCULAR" and g[1]:
                datos["altura_sedimento"] = ctx.dato(g[1] * f, "m")
        elif forma == "RECT_CLOSED":
            familia, subtipo = "saneamiento.canal", "box_culvert"
            attrs["seccion"] = "rectangular"
            attrs["numero_celdas"] = int(barriles) if barriles else 1
            if g[1] is not None:
                attrs["ancho_celda"] = r4(g[1] * f)
            if g[0] is not None:
                attrs["altura_celda"] = r4(g[0] * f)
        elif forma == "RECT_OPEN":
            familia, subtipo = "saneamiento.canal", "rectangular"
            attrs["seccion"] = "rectangular"
            if g[1] is not None:
                attrs["base"] = attrs["ancho_plantilla"] = r4(g[1] * f)
            if g[0] is not None:
                attrs["altura_muro"] = r4(g[0] * f)
        elif forma == "TRAPEZOIDAL":
            familia, subtipo = "saneamiento.canal", "trapezoidal"
            attrs["seccion"] = "trapezoidal"
            if g[1] is not None:
                attrs["base"] = attrs["ancho_plantilla"] = r4(g[1] * f)
            if g[0] is not None:
                attrs["altura_muro"] = r4(g[0] * f)
            if g[2] is not None and g[2] == g[3]:
                attrs["talud"] = g[2]
            elif g[2] is not None:
                ctx.avisos.append(f"{name}: taludes distintos ({g[2]} y {g[3]}); `talud` ausente")
                datos["talud_izquierdo"] = ctx.dato(g[2], "H:V")
                datos["talud_derecho"] = ctx.dato(g[3] or 0.0, "H:V")
        elif forma == "TRIANGULAR":
            familia, subtipo = "saneamiento.canal", "triangular"
            attrs["seccion"] = "triangular"
            if g[0] is not None:
                attrs["profundidad"] = r4(g[0] * f)
            if g[1] is not None:
                attrs["ancho"] = r4(g[1] * f)
        elif forma in SWMM_CERRADAS:
            ctx.avisos.append(
                f"{name}: sección {forma} (no circular); Redes sólo dibuja tubo circular, "
                "el diámetro queda ausente"
            )
            if g[0] is not None:
                datos["altura_seccion"] = ctx.dato(g[0] * f, "m")
            if g[1]:
                datos["ancho_seccion"] = ctx.dato(g[1] * f, "m")
        else:
            ctx.descartes.append(f"arco {name}: sección {forma} sin familia en Redes")
            continue
        if barriles and barriles > 1 and familia == "saneamiento.tuberia":
            ctx.avisos.append(f"{name}: {int(barriles)} conductos en paralelo; Redes dibuja uno")
        vert = None
        if ctx.vertices.get(name) or z_in != cotas.get(frm) or z_out != cotas.get(to):
            vert = trazado(ctx, name, frm, to, z_in, z_out)
        agregar(
            arcos,
            arco(
                ctx, emitidos, name, familia, subtipo, frm, to,
                diametro_mm=diam, vertices=vert, attrs=attrs or None, datos=datos or None,
            ),
        )

    # Bombas: la estación es el almacenamiento del que aspiran; la bomba en línea no tiene familia.
    estaciones = {o["codigo"] for o in nodos if o["familia"] == "saneamiento.bombeo"}
    for row in bombas:
        name, frm, to = row[0], row[1], row[2]
        if frm in estaciones:
            ctx.avisos.append(
                f"bomba {name} ({frm} -> {to}): representada por la estación {frm}; "
                "Redes no tiene arco de bomba"
            )
        else:
            ctx.descartes.append(
                f"bomba {name} ({frm} -> {to}): no aspira de un [STORAGE]; "
                "saneamiento no tiene familia de bomba en línea"
            )

    # Vertederos: dispositivo en el nodo de entrada.
    for row in sections.get("WEIRS", []):
        if len(row) < 5:
            ctx.descartes.append(f"vertedero {row[0]}: fila incompleta")
            continue
        name, frm, to, tipo = row[0], row[1], row[2], row[3].upper()
        subtipo = {"SIDEFLOW": "alivio_lateral", "TRANSVERSE": "vertedero_cresta"}.get(tipo)
        if subtipo is None:
            ctx.descartes.append(f"vertedero {name}: tipo {tipo} sin subtipo en saneamiento.descarga")
            continue
        if name in emitidos:
            ctx.descartes.append(f"vertedero {name}: su código ya lo usa un nodo")
            continue
        base = cotas.get(frm)
        cresta = cota_extremo(frm, row[4])
        attrs = {"tipo_descarga": "alivio"}
        if cresta is not None:
            attrs["cota_cresta"] = r4(cresta)
            if subtipo == "vertedero_cresta" and base is not None:
                attrs["altura_cresta"] = r4(cresta - base)
        xs = xsec.get(name)
        if xs and num(col(xs, 3)):
            attrs["longitud_vertedero"] = r4(num(col(xs, 3)) * f)  # type: ignore[operator]
        o = nodo(ctx, name, "saneamiento.descarga", subtipo, cota=base, attrs=attrs, en=frm)
        if o:
            ctx.avisos.append(
                f"vertedero {name} ({frm} -> {to}): se ubica en {frm} con su cota; "
                "Redes no une dispositivos con arcos"
            )
        agregar(nodos, o)

    for seccion, que in (("ORIFICES", "orificio"), ("OUTLETS", "salida regulada")):
        for row in sections.get(seccion, []):
            ctx.descartes.append(
                f"{que} {row[0]} ({col(row, 1)} -> {col(row, 2)}): sin familia en Redes"
            )

    return nodos + arcos


# ---------------------------------------------------------------------------------------------
# EPANET


def epanet_objetos(
    sections: dict[str, list[list[str]]],
    ctx: Contexto,
    reservorio: str | None,
    subtipo_tuberia: str | None,
    subtipo_tanque: str | None,
) -> list[dict]:
    ops = opciones(sections)
    unidades = (ops.get("UNITS") or [None])[0]
    if unidades is None:
        unidades = "GPM"
        ctx.avisos.append("[OPTIONS] sin Units: EPANET asume GPM, así que cotas en pies y diámetros en pulgadas")
    unidades = unidades.upper()
    if unidades not in EPANET_UNIDADES:
        raise ErrorEntrada(f"Units desconocido: {unidades}")
    us = EPANET_UNIDADES[unidades] == "US"
    f = PIE if us else 1.0
    f_diam = PULGADA_MM if us else 1.0
    q = A_LPS[unidades]
    perdida = ((ops.get("HEADLOSS") or ["H-W"])[0]).upper()

    pipes = [r for r in sections.get("PIPES", []) if len(r) >= 3]
    pumps = [r for r in sections.get("PUMPS", []) if len(r) >= 3]
    valves = [r for r in sections.get("VALVES", []) if len(r) >= 3]

    # Grado y diámetros de cada nodo, para el tipo de accesorio del nudo.
    grado: dict[str, int] = {}
    diametros: dict[str, set[float]] = {}
    for r, i_diam in [(r, 4) for r in pipes] + [(r, None) for r in pumps] + [(r, 3) for r in valves]:
        for extremo in (r[1], r[2]):
            grado[extremo] = grado.get(extremo, 0) + 1
            d = num(col(r, i_diam)) if i_diam is not None else None
            if d is not None:
                diametros.setdefault(extremo, set()).add(d)

    demandas: dict[str, list[float]] = {}
    for row in sections.get("DEMANDS", []):
        d = num(col(row, 1))
        if d is not None:
            demandas.setdefault(row[0], []).append(d)

    cotas: dict[str, float] = {}
    nodos: list[dict] = []
    emitidos: set[str] = set()

    for row in sections.get("JUNCTIONS", []):
        name, elev = row[0], num(col(row, 1))
        g = grado.get(name, 0)
        if g == 0:
            ctx.descartes.append(f"nodo {name}: nudo sin tuberías")
            continue
        if g == 1:
            sub = "tapon"
        elif g == 2:
            sub = "reduccion" if len(diametros.get(name, ())) > 1 else "union"
        elif g == 3:
            sub = "tee"
        elif g == 4:
            sub = "cruz"
        else:
            ctx.descartes.append(f"nodo {name}: nudo con {g} enlaces; no hay accesorio de ese tipo")
            continue
        datos: dict = {}
        lista = demandas.get(name)
        if lista is None and num(col(row, 2)) is not None:
            lista = [num(col(row, 2))]  # type: ignore[list-item]
        for i, d in enumerate(lista or []):
            clave = "demanda_base" if len(lista) == 1 else f"demanda_base_{i + 1}"  # type: ignore[arg-type]
            datos[clave] = ctx.dato(d * q, "L/s")
        z = elev * f if elev is not None else None
        if z is not None:
            cotas[name] = z
        agregar(
            nodos,
            nodo(
                ctx, name, "comun.accesorio", sub, cota=z,
                attrs={"tipo_accesorio": sub}, datos=datos or None,
            ),
            emitidos,
        )

    for row in sections.get("RESERVOIRS", []):
        name, head = row[0], num(col(row, 1))
        if not reservorio:
            ctx.descartes.append(
                f"nodo {name}: un RESERVOIR de EPANET es una carga fija (río, lago, acuífero o "
                "tanque grande) y no tiene familia propia; indique cuál es con "
                "--reservorio familia:subtipo"
            )
            continue
        fam, _, sub = reservorio.partition(":")
        datos = {"carga_hidraulica": ctx.dato(head * f, "m")} if head is not None else None
        agregar(nodos, nodo(ctx, name, fam, sub, datos=datos), emitidos)

    sub_tanque = subtipo_tanque or "apoyado_cilindrico"
    for row in sections.get("TANKS", []):
        name = row[0]
        elev, ini, vmin, vmax, diam = (num(col(row, i)) for i in range(1, 6))
        curva = col(row, 7)
        if elev is None:
            ctx.descartes.append(f"nodo {name}: tanque sin cota de fondo")
            continue
        z = elev * f
        cotas[name] = z
        attrs: dict = {"cota_piso": r4(z)}
        if vmax is not None:
            attrs["cota_nivel_max"] = r4(z + vmax * f)
        if vmin is not None:
            attrs["cota_nivel_min"] = r4(z + vmin * f)
        if curva and curva != "*":
            ctx.avisos.append(f"{name}: volumen por curva {curva}; el diámetro no describe el tanque")
        elif diam is not None:
            attrs["diametro"] = r4(diam * f * 1000.0)
        datos = {"nivel_inicial": ctx.dato(ini * f, "m")} if ini is not None else None
        o = nodo(ctx, name, "agua.tanque", sub_tanque, cota=z, attrs=attrs, datos=datos)
        if o and not subtipo_tanque:
            ctx.avisos.append(
                f"{name}: EPANET no distingue tanque apoyado, elevado o enterrado; "
                "subtipo apoyado_cilindrico (cámbielo con --subtipo-tanque)"
            )
        agregar(nodos, o, emitidos)

    sub_tub = subtipo_tuberia or "distribucion_primaria"
    if pipes and not subtipo_tuberia:
        ctx.avisos.append(
            "EPANET no distingue conducción de distribución: tuberías como "
            "distribucion_primaria (cámbielo con --subtipo-tuberia)"
        )
    arcos: list[dict] = []
    for row in pipes:
        name, n1, n2 = row[0], row[1], row[2]
        largo, diam, rough = num(col(row, 3)), num(col(row, 4)), num(col(row, 5))
        estado = (col(row, 7) or "").upper()
        attrs: dict = {}
        datos: dict = {}
        if largo is not None:
            datos["longitud_origen"] = ctx.dato(largo * f, "m")
        if rough is not None:
            if perdida == "H-W":
                attrs["coef_hazen"] = rough
            elif perdida == "D-W":
                # SI: mm; US: milipies.
                attrs["rugosidad_abs"] = r4(rough * PIE if us else rough)
            else:
                datos["rugosidad_n"] = ctx.dato(rough, "")
        if estado == "CV":
            attrs["tipo_valvula"] = "retencion"
            ctx.avisos.append(f"{name}: tubería con válvula de retención (CV) en el propio tramo")
        elif estado == "CLOSED":
            ctx.avisos.append(f"{name}: estado inicial cerrado en EPANET")
        vert = None
        if ctx.vertices.get(name):
            vert = trazado(ctx, name, n1, n2, cotas.get(n1), cotas.get(n2))
        agregar(
            arcos,
            arco(
                ctx, emitidos, name, "agua.tuberia", sub_tub, n1, n2,
                diametro_mm=int(round(diam * f_diam)) if diam else None,
                vertices=vert, attrs=attrs or None, datos=datos or None,
            ),
        )

    def dispositivo(name: str, n1: str, n2: str, familia: str, subtipo: str, attrs: dict) -> None:
        if name in emitidos:
            ctx.descartes.append(f"{name}: su código ya lo usa un nodo")
            return
        o = nodo(ctx, name, familia, subtipo, cota=cotas.get(n1), attrs=attrs or None, en=n1)
        if o:
            ctx.avisos.append(
                f"{name} ({n1} -> {n2}): se ubica en {n1}; Redes no une dispositivos con arcos"
            )
        agregar(nodos, o)

    for row in pumps:
        name, n1, n2 = row[0], row[1], row[2]
        attrs = {}
        claves = [c.upper() for c in row[3:]]
        if "POWER" in claves:
            p = num(col(row, 3 + claves.index("POWER") + 1))
            if p is not None:
                attrs["potencia_unitaria"] = r4(p * HP_A_KW if us else p)
        dispositivo(name, n1, n2, "agua.bombeo", "booster_en_linea", attrs)

    for row in valves:
        name, n1, n2 = row[0], row[1], row[2]
        diam, tipo, consigna = num(col(row, 3)), (col(row, 4) or "").upper(), num(col(row, 5))
        mapeo = EPANET_VALVULAS.get(tipo)
        if mapeo is None:
            ctx.descartes.append(f"válvula {name} ({n1} -> {n2}): tipo {tipo} sin subtipo en agua.valvula")
            continue
        sub, tipo_valvula = mapeo
        attrs = {}
        if tipo_valvula:
            attrs["tipo_valvula"] = tipo_valvula
        if diam:
            attrs["diametro_nominal"] = int(round(diam * f_diam))
        if consigna is not None:
            if tipo in ("PRV", "PSV"):
                attrs["consigna_presion"] = r4(consigna * PSI_A_MCA if us else consigna)
            else:
                attrs["consigna_caudal"] = r4(consigna * q)
        dispositivo(name, n1, n2, "agua.valvula", sub, attrs)

    return nodos + arcos


# ---------------------------------------------------------------------------------------------
# Plan


ALIAS = {
    ("agua.captacion", "toma_rio"): ("agua.bocatoma", "barraje_fijo"),
    ("agua.captacion", "toma_quebrada"): ("agua.bocatoma", "bocatoma_lateral"),
    ("agua.captacion", "rejilla_fondo"): ("agua.bocatoma", "toma_tirolesa"),
    ("saneamiento.camara", "carga"): ("agua.camara", "carga"),
    ("saneamiento.camara", "quiebre"): ("agua.camara", "quiebre"),
    ("saneamiento.camara", "valvulas"): ("agua.camara", "valvulas"),
    ("saneamiento.camara", "medicion"): ("agua.camara", "medicion"),
    ("saneamiento.camara", "reparto"): ("agua.camara", "reparto"),
    ("saneamiento.camara", "alivio"): ("agua.camara", "alivio"),
    ("agua.planta", "cloracion"): ("comun.caseta", "cloracion"),
    ("agua.planta", "lecho_secado"): ("comun.lecho_secado", "arena_grava"),
    ("agua.planta", "espesador"): ("saneamiento.sedimentador", "espesador"),
    ("agua.planta", "laguna_estabilizacion"): ("saneamiento.laguna", "facultativa"),
}


def aplicar_alias(plan: dict, avisos: list[str]) -> None:
    """Traduce las claves retiradas a su destino. Nunca en silencio: queda el aviso."""
    for o in plan.get("objetos", []):
        clave = (o.get("familia"), o.get("subtipo"))
        destino = ALIAS.get(clave)
        if destino:
            avisos.append(f"{clave[0]}:{clave[1]} renombrado a {destino[0]}:{destino[1]}")
            o["familia"], o["subtipo"] = destino


def convertir(
    archivo: Path,
    tipo: str,
    *,
    coords_csv: Path | None = None,
    sistema: str | None = None,
    reservorio: str | None = None,
    subtipo_tuberia: str | None = None,
    subtipo_tanque: str | None = None,
) -> Resultado:
    sections = read_sections(archivo)
    coords = coordenadas_del_inp(sections)
    if coords_csv is not None:
        coords.update(coordenadas_csv(coords_csv))
    app = "SWMM" if tipo == "swmm" else "EPANET"
    ctx = Contexto(fuente=f"{app} ({archivo.name})", coords=coords, vertices=vertices_del_inp(sections))
    if tipo == "swmm":
        objetos = swmm_objetos(sections, ctx, sistema)
    else:
        if reservorio and ":" not in reservorio:
            raise ErrorEntrada("--reservorio va como familia:subtipo (p. ej. agua.captacion:toma_directa)")
        objetos = epanet_objetos(sections, ctx, reservorio, subtipo_tuberia, subtipo_tanque)
    plan = {
        "schema": PLAN_SCHEMA,
        "proyecto": {
            "nombre": archivo.stem,
            "crs": "EPSG:32717",
            "fuente_app": app,
            "fuente_archivo": archivo.name,
        },
        "terreno": [],
        "objetos": objetos,
    }
    aplicar_alias(plan, ctx.avisos)
    return Resultado(plan, ctx.avisos, ctx.descartes)


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description="Convierte SWMM/EPANET .inp a redes.plan/1")
    ap.add_argument("--tipo", choices=["swmm", "epanet"], required=True)
    ap.add_argument("archivo", type=Path)
    ap.add_argument("--salida", type=Path, required=True)
    ap.add_argument("--coords", type=Path, help="CSV 'codigo,E,N' que completa o sustituye [COORDINATES]")
    ap.add_argument("--sistema", choices=SISTEMAS, help="SWMM: obligatorio si el archivo no lo deja claro")
    ap.add_argument("--reservorio", help="EPANET: familia:subtipo para los RESERVOIRS (si no, se descartan)")
    ap.add_argument("--subtipo-tuberia", help="EPANET: subtipo de agua.tuberia (por defecto distribucion_primaria, con aviso)")
    ap.add_argument("--subtipo-tanque", help="EPANET: subtipo de agua.tanque (por defecto apoyado_cilindrico, con aviso)")
    ap.add_argument("--informe", type=Path, help="JSON con avisos y descartados")
    args = ap.parse_args(argv)

    if not args.archivo.exists():
        print(f"ERROR: no existe {args.archivo}", file=sys.stderr)
        return 2
    if args.coords and not args.coords.exists():
        print(f"ERROR: no existe {args.coords}", file=sys.stderr)
        return 2
    try:
        res = convertir(
            args.archivo, args.tipo,
            coords_csv=args.coords, sistema=args.sistema, reservorio=args.reservorio,
            subtipo_tuberia=args.subtipo_tuberia, subtipo_tanque=args.subtipo_tanque,
        )
    except ErrorEntrada as e:
        print(f"ERROR: {e}", file=sys.stderr)
        return 2

    args.salida.parent.mkdir(parents=True, exist_ok=True)
    args.salida.write_text(json.dumps(res.plan, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    if args.informe:
        args.informe.write_text(
            json.dumps({"avisos": res.avisos, "descartados": res.descartes}, ensure_ascii=False, indent=2) + "\n",
            encoding="utf-8",
        )

    nodos = sum(1 for o in res.plan["objetos"] if o["tipo"] == "nodo")
    arcos = sum(1 for o in res.plan["objetos"] if o["tipo"] == "arco")
    print(f"plan: {nodos} nodos, {arcos} arcos, {len(res.descartes)} descartados -> {args.salida}")
    for a in res.avisos:
        print(f"aviso: {a}", file=sys.stderr)
    for d in res.descartes:
        print(f"descartado: {d}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
