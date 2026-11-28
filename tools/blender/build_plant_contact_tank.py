"""Bake a baffled chlorine contact tank (agua.tanque / contacto) to .glb.

Usage:
    blender -b --factory-startup --python-exit-code 1 --python tools/blender/build_plant_contact_tank.py -- params.json

Data (from the project): interior length, width and wall height, number of baffles, wall
thickness and material. Shape (fixed by the contract, for approval): floor slab, baffle thickness
and the end opening each baffle leaves, on alternate ends so the water runs a serpentine.
The baffles run along X and split the width into equal channels. Frame and anchors: see
plant_units.py.
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import plant_units as pu  # noqa: E402
from redes_bake import run  # noqa: E402

FAMILY = "agua.tanque"
SUBTYPE = "contacto"
# Narrowest channel between baffles the drawing accepts; below it the baffles would touch.
MIN_CHANNEL_M = 0.1


def validate(p):
    pu.require_positive(p, [
        "length_m", "width_m", "height_m", "wall_thickness_m", "floor_thickness_m",
        "baffle_thickness_m", "baffle_gap_m",
    ])
    baffles = pu.require_count(p, "baffle_count", minimum=0)
    if p["baffle_gap_m"] >= p["length_m"]:
        sys.exit("baffle_gap_m must be smaller than length_m")
    if (baffles + 1) * MIN_CHANNEL_M + baffles * p["baffle_thickness_m"] >= p["width_m"]:
        sys.exit("width_m is too narrow for that many baffles")
    pu.require_connections(p)


def baffles(p, hx, hy, z0, z1):
    count = pu.require_count(p, "baffle_count", minimum=0)
    th = p["baffle_thickness_m"]
    shells = []
    for i, y in enumerate(pu.divider_centres(-hy, hy, count + 1, th)):
        x0, x1 = (-hx, hx - p["baffle_gap_m"]) if i % 2 == 0 else (-hx + p["baffle_gap_m"], hx)
        shells.append(pu.box(x0, x1, y - th / 2, y + th / 2, z0, z1))
    return shells


def build(p):
    hx, hy = p["length_m"] / 2, p["width_m"] / 2
    t, ft = p["wall_thickness_m"], p["floor_thickness_m"]
    top = ft + p["height_m"]
    body = [pu.box(-hx - t, hx + t, -hy - t, hy + t, 0.0, ft)]
    body += pu.rect_walls(p["length_m"], p["width_m"], t, ft, top)
    unit = pu.Unit(FAMILY, SUBTYPE, ft, {
        "redes_largo_m": p["length_m"],
        "redes_ancho_m": p["width_m"],
        "redes_altura_m": p["height_m"],
        "redes_espesor_m": t,
    })
    unit.part("cuerpo", body, p["body_material"])
    unit.part("bafles", baffles(p, hx, hy, ft, top), p["body_material"])
    unit.connections(p["connections"], lambda a: pu.rect_exit(a, hx + t, hy + t))
    unit.link(
        "Baked by tools/blender/build_plant_contact_tank.py (docs/familias/F17-07-planta.md).\n"
        "Every part is explicit geometry: edit the parameters and bake again.\n"
    )


run(__file__, validate, build)
