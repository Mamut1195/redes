#!/usr/bin/env python3
"""Convierte una tabla (CSV/TSV) de municipio a `redes.plan/1`.

Muchas veces el dato no viene en un DXF ni en un SWMM, sino en una hoja de cálculo: un listado de
pozos con E, N y cota, o de tramos con desde/hasta y diámetro. Este script lo pasa al contrato de
Redes sin recalcular nada.

Uso:
    python plan_desde_tabla.py pozos.csv --tipo pozos  --salida pozos.json
    python plan_desde_tabla.py tramos.csv --tipo tramos --salida tramos.json
    python plan_desde_tabla.py todo.csv  --tipo mixto  --salida todo.json

La cabecera se reconoce por nombre, sin importar el orden ni las mayúsculas:
    codigo, familia, subtipo, e, n, cota (= solera), cota_terreno, diametro, desde, hasta,
    diametro_mm, diametro_interior, cota_solera_inicio, cota_solera_fin, cota_clave_inicio,
    cota_clave_fin, material, estado, nota

Solera y clave NO son lo mismo (F12-01): la solera es el fondo interior del tubo o del pozo y la
clave es el punto más alto del interior del tubo (clave = solera + DI). Una columna de clave nunca
se copia como solera. Si un tramo trae solo la clave y declara su diámetro interior, la solera se
obtiene restando el DI (conversión de referencia, no cálculo de diseño) y se avisa; sin DI no se
deriva nada y la solera queda sin dato.

Lo que no se reconoce se copia tal cual a los atributos del objeto.
"""

from __future__ import annotations

import argparse
import csv
import json
import sys
from pathlib import Path

ALIAS = {
    "codigo": "codigo", "código": "codigo", "code": "codigo", "id": "codigo",
    "familia": "familia", "family": "familia",
    "subtipo": "subtipo", "tipo": "subtipo", "type": "subtipo",
    "e": "e", "este": "e", "x": "e", "coord_x": "e",
    "n": "n", "norte": "n", "y": "n", "coord_y": "n",
    "cota": "cota", "cota_solera": "cota", "solera": "cota", "invert": "cota", "z": "cota",
    "cota_clave": "cota_clave", "clave": "cota_clave",
    "cota_solera_inicio": "cota_solera_inicio", "solera_inicio": "cota_solera_inicio",
    "invert_inicio": "cota_solera_inicio",
    "cota_solera_fin": "cota_solera_fin", "solera_fin": "cota_solera_fin",
    "invert_fin": "cota_solera_fin",
    "cota_clave_inicio": "cota_clave_inicio", "clave_inicio": "cota_clave_inicio",
    "cota_clave_fin": "cota_clave_fin", "clave_fin": "cota_clave_fin",
    "cota_terreno": "cota_terreno", "terreno": "cota_terreno",
    "diametro": "diametro", "dn": "diametro", "diametro_mm": "diametro",
    "diametro_interior": "diametro_interior", "di": "diametro_interior",
    "di_mm": "diametro_interior", "diametro_interior_mm": "diametro_interior",
    "desde": "desde", "from": "desde", "inicio": "desde", "nodo_inicio": "desde",
    "hasta": "hasta", "to": "hasta", "fin": "hasta", "nodo_fin": "hasta",
    "material": "material", "estado": "estado", "nota": "nota", "observacion": "nota",
}


def num(t: str, defecto: float = 0.0) -> float:
    try:
        return float(str(t).replace(",", ".").strip())
    except (TypeError, ValueError):
        return defecto


def opcional(t) -> float | None:
    """Número de la celda o `None` si está vacía o no es un número (nunca 0 por defecto)."""
    if t is None or not str(t).strip():
        return None
    try:
        return float(str(t).replace(",", ".").strip())
    except ValueError:
        return None


def normaliza_cabecera(campo: str) -> str:
    c = campo.strip().lower().replace(" ", "_")
    return ALIAS.get(c, c)


def leer(ruta: Path) -> tuple[list[str], list[dict]]:
    texto = ruta.read_text(encoding="utf-8-sig", errors="replace")
    # Separador: el que más aparezca en la primera línea útil.
    primera = next((l for l in texto.splitlines() if l.strip()), "")
    sep = ";" if primera.count(";") > primera.count(",") else ","
    if "\t" in primera and primera.count("\t") > primera.count(sep):
        sep = "\t"
    filas = list(csv.DictReader(texto.splitlines(), delimiter=sep))
    cabecera = [normaliza_cabecera(k) for k in (filas[0].keys() if filas else [])]
    return cabecera, filas


def a_plan(ruta: Path, tipo: str, fuente: str) -> tuple[dict, list[str]]:
    avisos: list[str] = []
    cabecera, filas = leer(ruta)
    if not filas:
        raise SystemExit(f"ERROR: {ruta} no tiene filas")

    objetos: list[dict] = []
    for i, fila in enumerate(filas, start=2):
        d = {normaliza_cabecera(k): v for k, v in fila.items() if k is not None}
        codigo = (d.get("codigo") or "").strip()
        if not codigo:
            avisos.append(f"línea {i}: sin código, se omite")
            continue
        familia = (d.get("familia") or "").strip()
        subtipo = (d.get("subtipo") or "").strip()
        estado = (d.get("estado") or "").strip()
        nota = (d.get("nota") or "").strip()

        es_arco = tipo == "tramos" or (tipo == "mixto" and (d.get("desde") or d.get("hasta")))
        if es_arco:
            desde = (d.get("desde") or "").strip()
            hasta = (d.get("hasta") or "").strip()
            if not desde or not hasta:
                avisos.append(f"línea {i}: tramo {codigo} sin desde/hasta, se omite")
                continue
            objeto = {
                "tipo": "arco",
                "codigo": codigo,
                "familia": familia or "saneamiento.tuberia",
                "subtipo": subtipo or "colector_sanitario",
                "desde": desde,
                "hasta": hasta,
            }
            dn = opcional(d.get("diametro"))
            if dn is not None:
                objeto["diametro_mm"] = int(round(dn))
            attrs: dict = {}
            if d.get("material"):
                attrs["material"] = d["material"]
            # Diámetro interior en mm; un valor menor que 10 solo puede venir en metros.
            di_mm = opcional(d.get("diametro_interior"))
            if di_mm is not None and di_mm < 10:
                avisos.append(f"línea {i}: tramo {codigo} con DI {di_mm} (se lee en m)")
                di_mm *= 1000.0
            if di_mm is not None:
                attrs["diametro_interior"] = round(di_mm, 1)
            for extremo, clave_attr in (("inicio", "cota_clave"), ("fin", "cota_clave_fin")):
                solera = opcional(d.get(f"cota_solera_{extremo}"))
                clave = opcional(d.get(f"cota_clave_{extremo}"))
                if clave is not None:
                    attrs[clave_attr] = clave
                if solera is None and clave is not None:
                    if di_mm is not None:
                        solera = round(clave - di_mm / 1000.0, 3)
                        avisos.append(
                            f"línea {i}: tramo {codigo} solera de {extremo} = clave - DI = {solera}"
                        )
                    else:
                        avisos.append(
                            f"línea {i}: tramo {codigo} trae clave de {extremo} sin DI: "
                            "la solera queda sin dato"
                        )
                if solera is not None:
                    objeto[f"cota_solera_{extremo}"] = solera
            if attrs:
                objeto["attrs"] = attrs
        else:
            # Un nodo sin planta no se puede dibujar: se avisa en vez de colocarlo en (0,0).
            if not (d.get("e") or "").strip() or not (d.get("n") or "").strip():
                avisos.append(f"línea {i}: nodo {codigo} sin E/N, se omite")
                continue
            objeto = {
                "tipo": "nodo",
                "codigo": codigo,
                "familia": familia or "saneamiento.pozo",
                "subtipo": subtipo or "inspeccion",
                "e": num(d.get("e")),
                "n": num(d.get("n")),
            }
            # La cota del nodo es su solera. Una clave no se copia como solera.
            solera = opcional(d.get("cota"))
            if solera is not None:
                objeto["cota"] = solera
            terreno = opcional(d.get("cota_terreno"))
            if terreno is not None:
                objeto["cota_terreno"] = terreno
            clave = opcional(d.get("cota_clave"))
            if clave is not None:
                objeto.setdefault("attrs", {})["cota_clave"] = clave
                if solera is None:
                    avisos.append(
                        f"línea {i}: nodo {codigo} trae cota de clave y no de solera: "
                        "la solera queda sin dato"
                    )
            if d.get("diametro"):
                objeto["diametro"] = num(d.get("diametro"))
        if estado:
            objeto["estado"] = estado
        if nota:
            objeto["nota"] = nota

        # Cualquier columna no reconocida se conserva como atributo.
        extra = {
            k: v
            for k, v in d.items()
            if k and k not in ALIAS.values() and k not in {"desde", "hasta"} and v not in (None, "")
        }
        if extra:
            objeto.setdefault("attrs", {}).update(extra)
        objetos.append(objeto)

    plan = {
        "schema": "redes.plan/1",
        "proyecto": {
            "nombre": ruta.stem.replace("_", " ").title(),
            "crs": "EPSG:32717",
            "fuente_app": fuente,
            "fuente_archivo": ruta.name,
        },
        "terreno": [],
        "objetos": objetos,
    }
    nodos = sum(1 for o in objetos if o["tipo"] == "nodo")
    arcos = len(objetos) - nodos
    avisos.insert(0, f"{nodos} nodos y {arcos} arcos desde {ruta.name}")
    return plan, avisos


def main() -> int:
    ap = argparse.ArgumentParser(description="Tabla CSV/TSV -> redes.plan/1")
    ap.add_argument("archivo", type=Path)
    ap.add_argument("--tipo", choices=["pozos", "tramos", "mixto"], default="mixto")
    ap.add_argument("--fuente", default="Hoja de cálculo")
    ap.add_argument("--salida", type=Path, required=True)
    args = ap.parse_args()

    if not args.archivo.exists():
        print(f"ERROR: no existe {args.archivo}", file=sys.stderr)
        return 2

    plan, avisos = a_plan(args.archivo, args.tipo, args.fuente)
    args.salida.parent.mkdir(parents=True, exist_ok=True)
    args.salida.write_text(json.dumps(plan, ensure_ascii=False, indent=2), encoding="utf-8")
    for a in avisos:
        print(f"aviso: {a}", file=sys.stderr)
    print(f"plan: {len(plan['objetos'])} objetos -> {args.salida}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
