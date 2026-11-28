"""Bake a sand and gravel sludge drying bed (comun.lecho_secado / arena_grava) to .glb.

Usage:
    blender -b --factory-startup --python-exit-code 1 --python tools/blender/build_plant_drying_bed.py -- params.json

Data (from the project): interior length and width, wall height, number of cells, sand and
gravel depths, drain DN, wall thickness and material. Shape (fixed by the contract, for
approval): floor slab (flat: the declared floor slope is not drawn), one drain per cell across
it, layer and drain materials. Frame and anchors: see plant_units.py.
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import plant_units as pu  # noqa: E402
from redes_bake import run  # noqa: E402

FAMILY = "comun.lecho_secado"
SUBTYPE = "arena_grava"


def validate(p):
    pu.require_positive(p, [
        "length_m", "width_m", "wall_height_m", "sand_depth_m", "gravel_depth_m", "drain_dn_mm",
        "wall_thickness_m", "floor_thickness_m",
    ])
    pu.require_count(p, "cell_count")
    if p["sand_depth_m"] + p["gravel_depth_m"] >= p["wall_height_m"]:
        sys.exit("the sand and gravel do not fit under the wall top")
    if p["drain_dn_mm"] / 1000.0 >= p["gravel_depth_m"]:
        sys.exit("the drain must fit in the gravel layer")
    pu.require_connections(p)


def build(p):
    cells = pu.require_count(p, "cell_count")
    hx, hy = p["length_m"] / 2, p["width_m"] / 2
    t, ft = p["wall_thickness_m"], p["floor_thickness_m"]
    top = ft + p["wall_height_m"]
    spans = pu.bays(-hx, hx, cells, t)
    body = [pu.box(-hx - t, hx + t, -hy - t, hy + t, 0.0, ft)]
    body += pu.rect_walls(p["length_m"], p["width_m"], t, ft, top)
    body += pu.cross_walls(pu.divider_centres(-hx, hx, cells, t), t, p["width_m"], ft, top)
    sand = ft + p["gravel_depth_m"]
    radius = p["drain_dn_mm"] / 2000.0
    drains = [
        pu.cylinder(((x0 + x1) / 2, -hy, ft + radius), ((x0 + x1) / 2, hy, ft + radius), radius,
                    p["segments"])
        for x0, x1 in spans
    ]
    unit = pu.Unit(FAMILY, SUBTYPE, ft, {
        "redes_largo_m": p["length_m"],
        "redes_ancho_m": p["width_m"],
        "redes_altura_m": p["wall_height_m"],
        "redes_espesor_m": t,
    })
    unit.part("cuerpo", body, p["body_material"])
    unit.part("grava", [pu.box(x0, x1, -hy, hy, ft, sand) for x0, x1 in spans],
              p["gravel_material"])
    unit.part("arena", [pu.box(x0, x1, -hy, hy, sand, sand + p["sand_depth_m"])
                        for x0, x1 in spans], p["sand_material"])
    unit.part("drenes", drains, p["drain_material"])
    unit.connections(p["connections"], lambda a: pu.rect_exit(a, hx + t, hy + t))
    unit.link(
        "Baked by tools/blender/build_plant_drying_bed.py (docs/familias/F17-07-planta.md).\n"
        "Every part is explicit geometry: edit the parameters and bake again.\n"
    )


run(__file__, validate, build)
