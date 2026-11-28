"""Bake a concentric reducer (comun.accesorio / reduccion) to .glb with Blender running headless.

Usage:
    blender -b --factory-startup --python-exit-code 1 --python tools/blender/build_reducer.py -- params.json

The names are listed in build_reducer.receta.json (docs/familias/F17-05-accesorios.md).

Frame of the piece: Z up, metres, the axis along X. The reference port (-X) takes dn_mm and the
other one outlet_dn_mm, unless the pipes' own DN say it is the other way round. The axis is at
the reference bore radius over the origin, so the reference pipe's invert is the origin (empty
"solera").
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from redes_bake import run  # noqa: E402
from redes_mallas import Piece, check_ports, check_positive, lathe  # noqa: E402

FAMILY = "comun.accesorio"
SUBTYPE = "reduccion"
X = (1.0, 0.0, 0.0)
UP = (0.0, 0.0, 1.0)


def validate(p):
    check_positive(p, ["dn_mm", "outlet_dn_mm", "body_od_dn", "socket_od_dn",
                       "socket_length_dn", "cone_length_per_step", "segments"])
    check_ports(p["ports"], 2)


def end_diameters(p):
    """DN (m) at -X and at +X: the declared inlet at the reference port, unless the pipes' DN
    match the declared pair the other way round."""
    inlet, outlet = p["dn_mm"], p["outlet_dn_mm"]
    reference, other = p["ports"][0].get("dn_mm"), p["ports"][1].get("dn_mm")
    if reference == outlet and other == inlet and inlet != outlet:
        inlet, outlet = outlet, inlet
    return inlet / 1000.0, outlet / 1000.0


def build(p):
    d0, d1 = end_diameters(p)
    seg = p["segments"]
    axis_z = d0 / 2
    s0, s1 = p["socket_length_dn"] * d0, p["socket_length_dn"] * d1
    cone = p["cone_length_per_step"] * abs(d0 - d1) + 0.5 * min(d0, d1)
    total = s0 + cone + s1
    x0 = -total / 2
    origin = (x0, 0.0, axis_z)
    r0, r1 = p["body_od_dn"] * d0 / 2, p["body_od_dn"] * d1 / 2
    q0, q1 = p["socket_od_dn"] * d0 / 2, p["socket_od_dn"] * d1 / 2
    body = lathe(origin, X, UP, [(0.0, 0.0), (q0, 0.0), (q0, s0), (r0, s0),
                                 (r1, s0 + cone), (q1, s0 + cone), (q1, total), (0.0, total)], seg)

    piece = Piece(FAMILY, SUBTYPE, f"DN{d0 * 1000:g}x{d1 * 1000:g}", {
        "redes_dn_entrada_mm": round(d0 * 1000, 3),
        "redes_dn_salida_mm": round(d1 * 1000, 3),
    })
    piece.part("cuerpo", [body], p["material"])
    piece.invert_anchor()
    for i, (port, x) in enumerate(zip(p["ports"], (x0, -x0)), start=1):
        piece.port_anchor(port, (x, 0.0, axis_z), i)


run(__file__, validate, build)
