"""Bake a hydraulic-jump rapid mixer (agua.mezcla / resalto_hidraulico) to .glb.

Usage:
    blender -b --factory-startup --python-exit-code 1 --python tools/blender/build_plant_mixer.py -- params.json

Data (from the project): interior length, width and wall height, the drop height and the jump
length (the channel downstream of the drop), wall thickness and material. Shape (fixed by the
contract, for approval): floor slab and the run of the sloped drop face. Frame and anchors: see
plant_units.py.
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import plant_units as pu  # noqa: E402
from redes_bake import run  # noqa: E402

FAMILY = "agua.mezcla"
SUBTYPE = "resalto_hidraulico"


def validate(p):
    pu.require_positive(p, [
        "length_m", "width_m", "height_m", "drop_height_m", "jump_length_m", "wall_thickness_m",
        "floor_thickness_m", "drop_run_m",
    ])
    if p["drop_height_m"] >= p["height_m"]:
        sys.exit("drop_height_m must be below the wall top (height_m)")
    if p["length_m"] - p["jump_length_m"] <= p["drop_run_m"]:
        sys.exit("length_m minus jump_length_m leaves no room for the drop")
    pu.require_connections(p)


def build(p):
    hx, hy = p["length_m"] / 2, p["width_m"] / 2
    t, ft = p["wall_thickness_m"], p["floor_thickness_m"]
    top = ft + p["height_m"]
    toe = hx - p["jump_length_m"]
    crest = ft + p["drop_height_m"]
    drop = pu.prism_xz(
        [(-hx, ft), (toe, ft), (toe - p["drop_run_m"], crest), (-hx, crest)], -hy, hy
    )
    body = [pu.box(-hx - t, hx + t, -hy - t, hy + t, 0.0, ft)]
    body += pu.rect_walls(p["length_m"], p["width_m"], t, ft, top)

    unit = pu.Unit(FAMILY, SUBTYPE, ft, {
        "redes_largo_m": p["length_m"],
        "redes_ancho_m": p["width_m"],
        "redes_altura_m": p["height_m"],
        "redes_espesor_m": t,
    })
    unit.part("cuerpo", body, p["body_material"])
    unit.part("caida", [drop], p["body_material"])
    unit.connections(p["connections"], lambda a: pu.rect_exit(a, hx + t, hy + t))
    unit.link(
        "Baked by tools/blender/build_plant_mixer.py (docs/familias/F17-07-planta.md).\n"
        "Every part is explicit geometry: edit the parameters and bake again.\n"
    )


run(__file__, validate, build)
