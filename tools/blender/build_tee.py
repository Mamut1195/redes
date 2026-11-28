"""Bake a pipe tee (comun.accesorio / tee) to .glb with Blender running headless.

Usage:
    blender -b --factory-startup --python-exit-code 1 --python tools/blender/build_tee.py -- params.json

The names are listed in build_tee.receta.json (docs/familias/F17-05-accesorios.md).

Frame of the piece: Z up, metres, the run along X with its axis at z = DN/2 over the origin (the
bore invert under the centre, empty "solera"). The reference port is one end of the run (-X); of
the other two, the one closest to +X is the other end and the third is the branch, drawn at the
angle its pipe has. All three axes are at one height.
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from redes_bake import run  # noqa: E402
from redes_mallas import (  # noqa: E402
    Piece, check_ports, check_positive, cylinder, plan_direction, scale,
)

FAMILY = "comun.accesorio"
SUBTYPE = "tee"
UP = (0.0, 0.0, 1.0)


def validate(p):
    check_positive(p, ["dn_mm", "branch_dn_mm", "body_od_dn", "socket_od_dn",
                       "socket_length_dn", "segments"])
    check_ports(p["ports"], 3)


def run_and_branch(ports):
    """The far end of the run (closest to +X) and the branch, of the two non-reference ports."""
    a, b = ports[1], ports[2]
    if abs(b["azimuth_deg"]) < abs(a["azimuth_deg"]):
        a, b = b, a
    return a, b


def build(p):
    run_dn, branch_dn = p["dn_mm"] / 1000.0, p["branch_dn_mm"] / 1000.0
    seg = p["segments"]
    axis_z = run_dn / 2
    run_r, branch_r = p["body_od_dn"] * run_dn / 2, p["body_od_dn"] * branch_dn / 2
    run_socket = p["socket_length_dn"] * run_dn
    branch_socket = p["socket_length_dn"] * branch_dn
    # Each outlet clears the other pipe's body before its socket starts.
    run_reach = branch_r + run_socket
    branch_reach = run_r + branch_socket
    far, branch = run_and_branch(p["ports"])
    branch_dir = plan_direction(branch["azimuth_deg"])

    body = [
        cylinder((-run_reach, 0.0, axis_z), (1.0, 0.0, 0.0), UP, 2 * run_reach, run_r, seg),
        cylinder((0.0, 0.0, axis_z), branch_dir, UP, branch_reach, branch_r, seg),
    ]
    ends = {
        p["ports"][0]["name"]: ((-1.0, 0.0, 0.0), run_reach, run_socket, p["socket_od_dn"] * run_dn / 2),
        far["name"]: ((1.0, 0.0, 0.0), run_reach, run_socket, p["socket_od_dn"] * run_dn / 2),
        branch["name"]: (branch_dir, branch_reach, branch_socket,
                         p["socket_od_dn"] * branch_dn / 2),
    }
    sockets = []
    mouths = {}
    for name, (direction, reach, length, radius) in ends.items():
        mouth = (direction[0] * reach, direction[1] * reach, axis_z)
        mouths[name] = mouth
        sockets.append(cylinder(mouth, scale(direction, -1.0), UP, length, radius, seg))

    piece = Piece(FAMILY, SUBTYPE, f"DN{p['dn_mm']:g}x{p['branch_dn_mm']:g}", {
        "redes_dn_mm": p["dn_mm"],
        "redes_dn_ramal_mm": p["branch_dn_mm"],
    })
    piece.part("cuerpo", body, p["material"])
    piece.part("embocaduras", sockets, p["material"])
    piece.invert_anchor()
    for i, port in enumerate(p["ports"], start=1):
        piece.port_anchor(port, mouths[port["name"]], i)


run(__file__, validate, build)
