"""Bake a rapid gravity filter (agua.filtro / rapido_gravedad) to .glb.

Usage:
    blender -b --factory-startup --python-exit-code 1 --python tools/blender/build_plant_filter.py -- params.json

Data (from the project): interior length, width and total height, number of cells, bed depth and
bed type (its material), wall thickness and material. Shape (fixed by the contract, for approval):
floor slab, underdrain plenum and false floor, gravel support layer, wash-water troughs. The
layers are separate solids with their own materials, so a vertical cut shows them. Frame and
anchors: see plant_units.py.
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import plant_units as pu  # noqa: E402
from redes_bake import run  # noqa: E402

FAMILY = "agua.filtro"
SUBTYPE = "rapido_gravedad"


def validate(p):
    pu.require_positive(p, [
        "length_m", "width_m", "height_m", "bed_depth_m", "wall_thickness_m",
        "floor_thickness_m", "plenum_height_m", "false_floor_thickness_m", "gravel_depth_m",
        "trough_width_m", "trough_depth_m", "trough_crest_above_bed_m", "trough_plate_m",
    ])
    cells = pu.require_count(p, "cell_count")
    pu.require_count(p, "troughs_per_cell")
    stack = (p["plenum_height_m"] + p["false_floor_thickness_m"] + p["gravel_depth_m"]
             + p["bed_depth_m"] + p["trough_crest_above_bed_m"])
    if stack >= p["height_m"]:
        sys.exit("height_m is too small for the plenum, the layers and the troughs")
    cell = (p["length_m"] - (cells - 1) * p["wall_thickness_m"]) / cells
    if cell <= p["troughs_per_cell"] * p["trough_width_m"]:
        sys.exit("the cells are too short for their troughs")
    pu.require_connections(p)


def layer(cells, hy, z0, z1):
    return [pu.box(x0, x1, -hy, hy, z0, z1) for x0, x1 in cells]


def troughs(p, cells, hy, crest):
    w = p["trough_width_m"]
    shells = []
    for x0, x1 in cells:
        for xc in pu.even_centres(x0, x1, p["troughs_per_cell"]):
            shells += pu.channel_across(xc - w / 2, xc + w / 2, -hy, hy,
                                        crest - p["trough_depth_m"], crest, p["trough_plate_m"])
    return shells


def build(p):
    p = dict(p, cell_count=pu.require_count(p, "cell_count"),
             troughs_per_cell=pu.require_count(p, "troughs_per_cell"))
    hx, hy = p["length_m"] / 2, p["width_m"] / 2
    t, ft = p["wall_thickness_m"], p["floor_thickness_m"]
    top = ft + p["height_m"]
    cells = pu.bays(-hx, hx, p["cell_count"], t)
    dividers = pu.divider_centres(-hx, hx, p["cell_count"], t)

    body = [pu.box(-hx - t, hx + t, -hy - t, hy + t, 0.0, ft)]
    body += pu.rect_walls(p["length_m"], p["width_m"], t, ft, top)
    body += pu.cross_walls(dividers, t, p["width_m"], ft, top)

    false_floor = ft + p["plenum_height_m"]
    gravel = false_floor + p["false_floor_thickness_m"]
    bed = gravel + p["gravel_depth_m"]
    bed_top = bed + p["bed_depth_m"]

    unit = pu.Unit(FAMILY, SUBTYPE, ft, {
        "redes_largo_m": p["length_m"],
        "redes_ancho_m": p["width_m"],
        "redes_altura_m": p["height_m"],
        "redes_espesor_m": t,
    })
    unit.part("cuerpo", body, p["body_material"])
    unit.part("falso_fondo", layer(cells, hy, false_floor, gravel), p["false_floor_material"])
    unit.part("grava", layer(cells, hy, gravel, bed), p["gravel_material"])
    unit.part("lecho", layer(cells, hy, bed, bed_top), p["bed_material"])
    unit.part("canaletas", troughs(p, cells, hy, bed_top + p["trough_crest_above_bed_m"]),
              p["trough_material"])
    unit.connections(p["connections"], lambda a: pu.rect_exit(a, hx + t, hy + t))
    unit.link(
        "Baked by tools/blender/build_plant_filter.py (docs/familias/F17-07-planta.md).\n"
        "Every part is explicit geometry: edit the parameters and bake again.\n"
    )


run(__file__, validate, build)
