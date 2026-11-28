"""Bake an air valve on its line (agua.valvula / aire) to .glb with Blender running headless.

Usage:
    blender -b --factory-startup --python-exit-code 1 --python tools/blender/build_air_valve.py -- params.json

The names are listed in build_air_valve.receta.json (docs/familias/F17-05-accesorios.md).

Frame of the piece: Z up, metres. The line runs along X with its axis at z = main DN/2 over the
origin (the main's invert, empty "solera"); the main's DN is the reference pipe's. A tapping
saddle clasps the main and the valve stands on its crown, with the valve's own DN.
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from redes_bake import run  # noqa: E402
from redes_mallas import Piece, check_ports, check_positive, cylinder, lathe  # noqa: E402

FAMILY = "agua.valvula"
SUBTYPE = "aire"
X = (1.0, 0.0, 0.0)
UP = (0.0, 0.0, 1.0)


def validate(p):
    check_positive(p, ["dn_mm", "saddle_od_main", "saddle_length_dn", "riser_m",
                       "flange_od_dn", "flange_thickness_m", "body_od_dn", "body_height_dn",
                       "segments"])
    check_ports(p["ports"], 2)
    if not p["ports"][0].get("dn_mm"):
        raise SystemExit("the reference port has no dn_mm")


def build(p):
    dn = p["dn_mm"] / 1000.0
    main = p["ports"][0]["dn_mm"] / 1000.0
    seg = p["segments"]
    axis_z = main / 2
    saddle_r = p["saddle_od_main"] * main / 2
    saddle_len = p["saddle_length_dn"] * dn + 0.06
    saddle = cylinder((-saddle_len / 2, 0.0, axis_z), X, UP, saddle_len, saddle_r, seg)
    crown = axis_z + saddle_r
    riser = cylinder((0.0, 0.0, axis_z), UP, X, crown - axis_z + p["riser_m"], dn * 0.6, seg)
    flange_z = crown + p["riser_m"]
    flange_r = p["flange_od_dn"] * dn / 2 + 0.03
    flange = cylinder((0.0, 0.0, flange_z), UP, X, p["flange_thickness_m"], flange_r, seg)
    body_z = flange_z + p["flange_thickness_m"]
    body_r = p["body_od_dn"] * dn / 2 + 0.02
    height = p["body_height_dn"] * dn + 0.1
    body = lathe((0.0, 0.0, body_z), UP, X,
                 [(0.0, 0.0), (body_r, 0.0), (body_r, height * 0.8),
                  (body_r * 0.7, height * 0.95), (body_r * 0.25, height), (0.0, height)], seg)
    cowl = cylinder((0.0, 0.0, body_z + height), UP, X, 0.04, body_r * 0.35, seg)

    piece = Piece(FAMILY, SUBTYPE, f"DN{p['dn_mm']:g}_en_DN{p['ports'][0]['dn_mm']:g}", {
        "redes_dn_mm": p["dn_mm"],
        "redes_dn_tuberia_mm": p["ports"][0]["dn_mm"],
    })
    piece.part("collarin", [saddle], p["saddle_material"])
    piece.part("cuerpo", [riser, flange, body, cowl], p["material"])
    piece.invert_anchor()
    for i, (port, x) in enumerate(zip(p["ports"], (-saddle_len / 2, saddle_len / 2)), start=1):
        piece.port_anchor(port, (x, 0.0, axis_z), i)


run(__file__, validate, build)
