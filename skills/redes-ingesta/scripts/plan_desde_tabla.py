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
    codigo, familia, subtipo, e, n, cota, cota_terreno, diametro, desde, hasta,
    diametro_mm, material, estado, nota

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
    "cota": "cota", "cota_clave": "cota", "invert": "cota", "z": "cota",
    "cota_terreno": "cota_terreno", "terreno": "cota_terreno",
    "diametro": "diametro", "dn": "diametro", "diametro_mm": "diametro",
    "desde": "desde", "from": "desde", "inicio": "desde", "nodo_inicio": "desde",
    "hasta": "hasta", "to": "hasta", "fin": "hasta", "nodo_fin": "hasta",
    "material": "material", "estado": "estado", "nota": "nota", "observacion": "nota",
}


def num(t: str, defecto: float = 0.0) -> float:
    try:
        return float(str(t).replace(",", ".").strip())
    except (TypeError, ValueError):
        return defecto


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
                "diametro_mm": int(num(d.get("diametro"), 200)),
                "trazado": [],
            }
            if d.get("material"):
                objeto["attrs"] = {"material": d["material"]}
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
                "cota": num(d.get("cota")),
                "cota_terreno": num(d.get("cota_terreno"), num(d.get("cota"))),
            }
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
