"""Bake a pipe elbow (comun.accesorio / codo) to .glb with Blender running headless.

Usage:
    blender -b --factory-startup --python-exit-code 1 --python tools/blender/build_elbow.py -- params.json

The names are listed in build_elbow.receta.json (docs/familias/F17-05-accesorios.md).

Frame of the piece: Z up, metres, the pipe axes meet over the origin at z = DN/2, so the bore
invert under the meeting point is the origin (empty "solera"). The reference port points to -X;
the other one points where its pipe goes, so the bend angle is the one the pipes are drawn with.
"""

import math
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from redes_bake import run  # noqa: E402
from redes_mallas import (  # noqa: E402
    Piece, bent_tube, check_ports, check_positive, cylinder, plan_direction, scale,
)

FAMILY = "comun.accesorio"
SUBTYPE = "codo"
UP = (0.0, 0.0, 1.0)
STRAIGHT_BELOW_DEG = 0.5


def validate(p):
    check_positive(p, ["dn_mm", "body_od_dn", "socket_od_dn", "socket_length_dn",
                       "bend_radius_dn", "max_arm_dn", "segments", "steps_per_quarter"])
    check_ports(p["ports"], 2)


def layout(p):
    """Bend angle, bend radius and arm length, from the ports and the shape values."""
    dn = p["dn_mm"] / 1000.0
    turn = math.radians(p["ports"][1]["azimuth_deg"])
    socket = p["socket_length_dn"] * dn
    radius = p["bend_radius_dn"] * dn
    half = abs(turn) / 2
    tangent = radius * math.tan(half)
    # Near a U-turn the tangent runs to infinity: the bend tightens so an arm never passes
    # max_arm_dn diameters (a shape rule, docs/familias/F17-05-accesorios.md).
    if tangent > p["max_arm_dn"] * dn:
        tangent = p["max_arm_dn"] * dn
        radius = tangent / math.tan(half)
    return {"dn": dn, "turn": turn, "radius": radius, "tangent": tangent, "socket": socket}


def build(p):
    g = layout(p)
    dn, seg = g["dn"], p["segments"]
    axis_z = dn / 2
    body_r = p["body_od_dn"] * dn / 2
    socket_r = p["socket_od_dn"] * dn / 2
    reach = g["tangent"] + g["socket"]
    start = (-reach, 0.0, axis_z)
    if abs(math.degrees(g["turn"])) >= STRAIGHT_BELOW_DEG:
        legs = [("straight", g["socket"]), ("bend", g["turn"], g["radius"]),
                ("straight", g["socket"])]
    else:
        legs = [("straight", 2 * reach)]
    (body, _, _) = bent_tube(start, (1.0, 0.0, 0.0), UP, body_r, legs, seg,
                             p["steps_per_quarter"])

    outlet = plan_direction(p["ports"][1]["azimuth_deg"])
    inlet = (-1.0, 0.0, 0.0)
    ends = [(inlet, (1.0, 0.0, 0.0)), (outlet, scale(outlet, -1.0))]
    sockets = []
    for direction, inward in ends:
        mouth = (direction[0] * reach, direction[1] * reach, axis_z)
        sockets.append(cylinder(mouth, inward, UP, g["socket"], socket_r, seg))

    piece = Piece(FAMILY, SUBTYPE, f"DN{p['dn_mm']:g}", {
        "redes_dn_mm": p["dn_mm"],
        "redes_angulo_deg": round(math.degrees(g["turn"]), 6),
    })
    piece.part("cuerpo", [body], p["material"])
    piece.part("embocaduras", sockets, p["material"])
    piece.invert_anchor()
    for i, (port, (direction, _)) in enumerate(zip(p["ports"], ends), start=1):
        location = (direction[0] * reach, direction[1] * reach, axis_z)
        piece.port_anchor(port, location, i)


run(__file__, validate, build)
