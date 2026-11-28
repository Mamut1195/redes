"""Bake an upflow anaerobic sludge blanket reactor (saneamiento.reactor / uasb) to .glb.

Usage:
    blender -b --factory-startup --python-exit-code 1 --python tools/blender/build_plant_uasb.py -- params.json

Data (from the project): interior length, width and wall height, water depth (the launder
crest), number of chambers, number of inlet points (shared out between the chambers from the
first one), wall thickness and material. Shape (fixed by the contract, for approval): floor
slab, the gas-liquid-solid separator hoods (inverted V, across the width), effluent launders,
the inlet drop pipes along the upstream wall of each chamber and their DN. Frame and anchors:
see plant_units.py.
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import plant_units as pu  # noqa: E402
from redes_bake import run  # noqa: E402

FAMILY = "saneamiento.reactor"
SUBTYPE = "uasb"
# The inlet drop pipes come from a header above the walls, which is not drawn.
INLET_ABOVE_WALL_M = 0.3


def validate(p):
    pu.require_positive(p, [
        "length_m", "width_m", "height_m", "water_depth_m", "wall_thickness_m",
        "floor_thickness_m", "hood_width_m", "hood_height_m", "hood_plate_m",
        "hood_top_below_water_m", "launder_width_m", "launder_depth_m", "launder_plate_m",
        "inlet_dn_mm", "inlet_clearance_m", "inlet_wall_offset_m",
    ])
    chambers = pu.require_count(p, "chamber_count")
    pu.require_count(p, "inlet_count")
    hoods = pu.require_count(p, "hoods_per_chamber", minimum=2)
    if p["water_depth_m"] >= p["height_m"]:
        sys.exit("water_depth_m must be below the wall top (height_m)")
    if p["hood_top_below_water_m"] + p["hood_height_m"] >= p["water_depth_m"]:
        sys.exit("the hoods do not fit under the water")
    bay = (p["length_m"] - (chambers - 1) * p["wall_thickness_m"]) / chambers
    if hoods * p["hood_width_m"] + p["inlet_wall_offset_m"] >= bay:
        sys.exit("the chambers are too short for their hoods")
    pu.require_connections(p)


def hood(xc, w, plate, zb, zt, hy):
    """Inverted V across the width, as two sloping plates that meet at the apex."""
    left = [(xc - w / 2, zb), (xc - w / 2 + plate, zb), (xc + plate / 2, zt), (xc - plate / 2, zt)]
    right = [(xc + w / 2 - plate, zb), (xc + w / 2, zb), (xc + plate / 2, zt), (xc - plate / 2, zt)]
    return [pu.prism_xz(left, -hy, hy), pu.prism_xz(right, -hy, hy)]


def build(p):
    hx, hy = p["length_m"] / 2, p["width_m"] / 2
    t, ft = p["wall_thickness_m"], p["floor_thickness_m"]
    top = ft + p["height_m"]
    water = ft + p["water_depth_m"]
    count = pu.require_count(p, "chamber_count")
    hoods_each = pu.require_count(p, "hoods_per_chamber", minimum=2)
    inlets = pu.require_count(p, "inlet_count")
    chambers = pu.bays(-hx, hx, count, t)

    body = [pu.box(-hx - t, hx + t, -hy - t, hy + t, 0.0, ft)]
    body += pu.rect_walls(p["length_m"], p["width_m"], t, ft, top)
    body += pu.cross_walls(pu.divider_centres(-hx, hx, count, t), t, p["width_m"], ft, top)

    zt = water - p["hood_top_below_water_m"]
    zb = zt - p["hood_height_m"]
    hoods, launders, pipes = [], [], []
    radius = p["inlet_dn_mm"] / 2000.0
    for k, (x0, x1) in enumerate(chambers):
        centres = pu.even_centres(x0 + p["inlet_wall_offset_m"], x1, hoods_each)
        for xc in centres:
            hoods += hood(xc, p["hood_width_m"], p["hood_plate_m"], zb, zt, hy)
        w = p["launder_width_m"]
        for a, b in zip(centres, centres[1:]):
            xm = (a + b) / 2
            launders += pu.channel_across(xm - w / 2, xm + w / 2, -hy, hy,
                                          water - p["launder_depth_m"], water,
                                          p["launder_plate_m"])
        here = inlets // count + (1 if k < inlets % count else 0)
        x = x0 + p["inlet_wall_offset_m"] / 2
        for y in pu.even_centres(-hy, hy, here) if here else []:
            pipes.append(pu.cylinder((x, y, top + INLET_ABOVE_WALL_M), (x, y, ft + p["inlet_clearance_m"]),
                                     radius, p["segments"]))

    unit = pu.Unit(FAMILY, SUBTYPE, ft, {
        "redes_largo_m": p["length_m"],
        "redes_ancho_m": p["width_m"],
        "redes_altura_m": p["height_m"],
        "redes_espesor_m": t,
    })
    unit.part("cuerpo", body, p["body_material"])
    unit.part("separadores", hoods, p["hood_material"])
    unit.part("canaletas", launders, p["launder_material"])
    unit.part("entradas", pipes, p["inlet_material"])
    unit.connections(p["connections"], lambda a: pu.rect_exit(a, hx + t, hy + t))
    unit.link(
        "Baked by tools/blender/build_plant_uasb.py (docs/familias/F17-07-planta.md).\n"
        "Every part is explicit geometry: edit the parameters and bake again.\n"
    )


run(__file__, validate, build)
