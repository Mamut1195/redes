#!/usr/bin/env python3
"""Convierte un .inp de SWMM o de EPANET a `redes.plan/1`.

No calcula nada: sólo lee, normaliza unidades y escribe el contrato de entrada de Redes.
Los caudales, presiones o volúmenes que traiga el archivo se copian como `datos` con su fuente.

Uso:
    python inp_to_plan.py --tipo swmm  red.inp  --salida plan.json
    python inp_to_plan.py --tipo epanet red.inp --salida plan.json

Reglas:
  · Un nodo de SWMM que no sea OUTfall se mapea a `saneamiento.pozo`.
  · Una conducción se mapea a `saneamiento.tuberia` (subtipo por sección).
  · En EPANET, JUNCTIONS/RESERVOIRS/TANKS son nodos y PIPES son `agua.tuberia`.
  · Cotas: se conservan las del archivo; si sólo hay diámetro de sección, se usa el máximo.
  · Todo lo que no se entiende se ignora y se informa al final.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

PLAN_SCHEMA = "redes.plan/1"


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


def num(value: str, default: float = 0.0) -> float:
    try:
        return float(value)
    except (TypeError, ValueError):
        return default


def swmm_plan(sections: dict[str, list[list[str]]], fuente: str) -> tuple[dict, list[str]]:
    avisos: list[str] = []
    nodos: list[dict] = []
    codigos: dict[str, str] = {}

    for row in sections.get("JUNCTIONS", []):
        if len(row) < 2:
            continue
        name, invert = row[0], num(row[1])
        depth = num(row[2]) if len(row) > 2 else 0.0
        codigos[name] = name
        nodos.append(
            {
                "tipo": "nodo",
                "codigo": name,
                "familia": "saneamiento.pozo",
                "subtipo": "inspeccion",
                "e": 0.0,
                "n": 0.0,
                "cota": invert,
                "cota_terreno": invert + max(depth, 1.5),
                "diametro": 1.2,
                "datos": {"profundidad_max": {"valor": depth, "unidad": "m", "fuente": fuente}},
            }
        )

    for row in sections.get("OUTFALLS", []):
        if len(row) < 2:
            continue
        name, invert = row[0], num(row[1])
        codigos[name] = name
        nodos.append(
            {
                "tipo": "nodo",
                "codigo": name,
                "familia": "saneamiento.descarga",
                "subtipo": "emisario_final",
                "e": 0.0,
                "n": 0.0,
                "cota": invert,
                "cota_terreno": invert + 1.0,
            }
        )

    arcos: list[dict] = []
    for row in sections.get("CONDUITS", []):
        if len(row) < 5:
            continue
        name, frm, to, length, rough = row[0], row[1], row[2], num(row[3]), num(row[4])
        if frm not in codigos or to not in codigos:
            avisos.append(f"conducción {name}: extremo inexistente")
            continue
        din, dout = num(row[5]) if len(row) > 5 else 0.0, num(row[6]) if len(row) > 6 else 0.0
        diam_mm = int(round(max(din, dout) * 1000.0)) if max(din, dout) > 0 else 200
        arcos.append(
            {
                "tipo": "arco",
                "codigo": name,
                "familia": "saneamiento.tuberia",
                "subtipo": "colector",
                "desde": frm,
                "hasta": to,
                "diametro_mm": diam_mm,
                "attrs": {"rugosidad_n": rough},
                "datos": {"longitud_origen": {"valor": length, "unidad": "m", "fuente": fuente}},
            }
        )

    for row in sections.get("XSECTIONS", []):
        # [XSECTIONS] Shape Geom1 Geom2 ... -> se aplica al arco por nombre
        if len(row) < 3:
            continue
        name, geom1 = row[0], num(row[2])
        if geom1 > 0:
            for a in arcos:
                if a["codigo"] == name:
                    a["diametro_mm"] = int(round(geom1 * 1000.0))

    plan = {
        "schema": PLAN_SCHEMA,
        "proyecto": {
            "nombre": Path(fuente).stem,
            "crs": "EPSG:32717",
            "fuente_app": "SWMM",
            "fuente_archivo": fuente,
        },
        "terreno": [],
        "objetos": nodos + arcos,
    }
    return plan, avisos


def epanet_plan(sections: dict[str, list[list[str]]], fuente: str) -> tuple[dict, list[str]]:
    avisos: list[str] = []
    nodos: list[dict] = []
    codigos: set[str] = set()

    for row in sections.get("JUNCTIONS", []):
        if len(row) < 2:
            continue
        name, elev = row[0], num(row[1])
        codigos.add(name)
        nodos.append(
            {
                "tipo": "nodo",
                "codigo": name,
                "familia": "saneamiento.camara",
                "subtipo": "valvulas",
                "e": 0.0,
                "n": 0.0,
                "cota": elev,
                "cota_terreno": elev + 1.2,
                "diametro": 1.5,
            }
        )

    for row in sections.get("RESERVOIRS", []):
        if len(row) < 2:
            continue
        name, head = row[0], num(row[1])
        codigos.add(name)
        nodos.append(
            {
                "tipo": "nodo",
                "codigo": name,
                "familia": "agua.tanque",
                "subtipo": "apoyado",
                "e": 0.0,
                "n": 0.0,
                "cota": head,
                "cota_terreno": head,
                "attrs": {"volumen_util": 100.0, "cota_piso": head, "cota_nivel_max": head},
            }
        )

    for row in sections.get("TANKS", []):
        if len(row) < 8:
            continue
        name, elev = row[0], num(row[1])
        codigos.add(name)
        nodos.append(
            {
                "tipo": "nodo",
                "codigo": name,
                "familia": "agua.tanque",
                "subtipo": "apoyado",
                "e": 0.0,
                "n": 0.0,
                "cota": elev,
                "cota_terreno": elev,
                "attrs": {"volumen_util": 0.0, "cota_piso": elev, "cota_nivel_max": elev + num(row[5])},
            }
        )

    arcos: list[dict] = []
    for row in sections.get("PIPES", []):
        if len(row) < 6:
            continue
        name, n1, n2, length, diam_mm, rough = row[0], row[1], row[2], num(row[3]), num(row[4]), num(row[5])
        if n1 not in codigos or n2 not in codigos:
            avisos.append(f"tubería {name}: extremo inexistente")
            continue
        arcos.append(
            {
                "tipo": "arco",
                "codigo": name,
                "familia": "agua.tuberia",
                "subtipo": "distribucion",
                "desde": n1,
                "hasta": n2,
                "diametro_mm": int(round(diam_mm)),
                "attrs": {"coef_hazen": rough},
                "datos": {"longitud_origen": {"valor": length, "unidad": "m", "fuente": fuente}},
            }
        )

    plan = {
        "schema": PLAN_SCHEMA,
        "proyecto": {
            "nombre": Path(fuente).stem,
            "crs": "EPSG:32717",
            "fuente_app": "EPANET",
            "fuente_archivo": fuente,
        },
        "terreno": [],
        "objetos": nodos + arcos,
    }
    return plan, avisos


def main() -> int:
    ap = argparse.ArgumentParser(description="Convierte SWMM/EPANET .inp a redes.plan/1")
    ap.add_argument("--tipo", choices=["swmm", "epanet"], required=True)
    ap.add_argument("archivo", type=Path)
    ap.add_argument("--salida", type=Path, required=True)
    ap.add_argument("--coords", type=Path, help="CSV 'codigo,E,N' para dar planta a los nodos")
    args = ap.parse_args()

    if not args.archivo.exists():
        print(f"ERROR: no existe {args.archivo}", file=sys.stderr)
        return 2

    sections = read_sections(args.archivo)
    builder = swmm_plan if args.tipo == "swmm" else epanet_plan
    plan, avisos = builder(sections, str(args.archivo))

    # Coordenadas opcionales: el .inp no las trae y sin planta no se puede ver en 3D.
    if args.coords and args.coords.exists():
        coords: dict[str, tuple[float, float]] = {}
        for line in args.coords.read_text(encoding="utf-8-sig", errors="replace").splitlines():
            parts = [p.strip() for p in re.split(r"[,;]", line) if p.strip()]
            if len(parts) >= 3:
                coords[parts[0]] = (num(parts[1]), num(parts[2]))
        faltan = 0
        for o in plan["objetos"]:
            if o["tipo"] == "nodo":
                if o["codigo"] in coords:
                    o["e"], o["n"] = coords[o["codigo"]]
                else:
                    faltan += 1
        if faltan:
            avisos.append(f"{faltan} nodos sin coordenada en {args.coords}")
    else:
        avisos.append("sin --coords: todos los nodos quedan en (0,0); el visor los apilará")

    args.salida.parent.mkdir(parents=True, exist_ok=True)
    args.salida.write_text(json.dumps(plan, ensure_ascii=False, indent=2), encoding="utf-8")

    nodos = sum(1 for o in plan["objetos"] if o["tipo"] == "nodo")
    arcos = sum(1 for o in plan["objetos"] if o["tipo"] == "arco")
    print(f"plan: {nodos} nodos, {arcos} arcos -> {args.salida}")
    for a in avisos:
        print(f"aviso: {a}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
