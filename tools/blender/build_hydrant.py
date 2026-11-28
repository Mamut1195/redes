"""Bake a pillar fire hydrant (agua.hidrante / columna) to .glb with Blender running headless.

Usage:
    blender -b --factory-startup --python-exit-code 1 --python tools/blender/build_hydrant.py -- params.json

The names are listed in build_hydrant.receta.json (docs/familias/F17-05-accesorios.md).

Frame of the piece: Z up, metres, origin at the invert of the inlet under the column (empty
"solera"): the viewer sits it on the hydrant's invert. The branch pipe arrives from -X, a shoe
bend turns it up, the buried riser reaches the ground at z = bury_depth_m and the column stands
height_m over it with its outlets, the first one facing +X (away from the branch).
"""

import math
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from redes_bake import run  # noqa: E402
from redes_mallas import (  # noqa: E402
    Piece, bent_tube, box, check_ports, check_positive, cylinder, lathe, plan_direction,
)

FAMILY = "agua.hidrante"
SUBTYPE = "columna"
X = (1.0, 0.0, 0.0)
UP = (0.0, 0.0, 1.0)
# The shoe turns +X into +Z: a right-hand turn about -Y.
SHOE_NORMAL = (0.0, -1.0, 0.0)


def validate(p):
    check_positive(p, ["dn_mm", "outlets", "height_m", "bury_depth_m", "inlet_length_dn",
                       "shoe_radius_dn", "pipe_od_dn", "column_od_dn", "flange_thickness_m",
                       "outlet_dn_mm", "outlet_length_m", "outlet_height_ratio", "segments"])
    check_ports(p["ports"], 1)


def outlets(p, column_r, ground):
    """Horizontal outlets with their caps, evenly round the column from +X."""
    seg = p["segments"]
    z = ground + p["outlet_height_ratio"] * p["height_m"]
    r = p["outlet_dn_mm"] / 1000.0 * 0.65
    stubs, caps = [], []
    count = int(p["outlets"])
    for k in range(count):
        d = plan_direction(360.0 * k / count)
        start = (d[0] * column_r * 0.9, d[1] * column_r * 0.9, z)
        stubs.append(cylinder(start, d, UP, p["outlet_length_m"], r, seg))
        reach = column_r * 0.9 + p["outlet_length_m"]
        caps.append(cylinder((d[0] * reach, d[1] * reach, z), d, UP, 0.03, r * 1.2, seg))
    return stubs, caps


def build(p):
    dn = p["dn_mm"] / 1000.0
    seg = p["segments"]
    axis_z = dn / 2
    pipe_r = p["pipe_od_dn"] * dn / 2
    shoe_r = p["shoe_radius_dn"] * dn
    inlet = p["inlet_length_dn"] * dn
    ground = p["bury_depth_m"]
    # The riser rises from the shoe to the ground; a hydrant shallower than its shoe has none.
    riser = max(0.0, ground - (axis_z + shoe_r))
    start = (-(shoe_r + inlet), 0.0, axis_z)
    legs = [("straight", inlet), ("bend", math.pi / 2, shoe_r)]
    if riser > 0.0:
        legs.append(("straight", riser))
    (buried, _, _) = bent_tube(start, X, SHOE_NORMAL, pipe_r, legs, seg, 8)
    column_r = p["column_od_dn"] * dn / 2
    flange = cylinder((0.0, 0.0, ground - p["flange_thickness_m"]), UP, X,
                      2 * p["flange_thickness_m"], column_r * 1.45, seg)
    top = ground + p["height_m"]
    column = lathe((0.0, 0.0, ground), UP, X,
                   [(0.0, 0.0), (column_r, 0.0), (column_r, p["height_m"] * 0.9),
                    (column_r * 1.15, p["height_m"] * 0.92), (column_r * 0.6, p["height_m"]),
                    (0.0, p["height_m"])], seg)
    nut = box(-0.025, 0.025, -0.025, 0.025, top, top + 0.05)
    stubs, caps = outlets(p, column_r, ground)

    piece = Piece(FAMILY, SUBTYPE, f"DN{p['dn_mm']:g}", {
        "redes_dn_mm": p["dn_mm"],
        "redes_bocas": p["outlets"],
        "redes_altura_sobre_rasante_m": p["height_m"],
        "redes_profundidad_m": p["bury_depth_m"],
    })
    piece.part("zapata_y_tubo", [buried], p["buried_material"])
    piece.part("columna", [flange, column] + stubs, p["paint_material"])
    piece.part("tapas_y_cuadradillo", caps + [nut], p["fittings_material"])
    piece.invert_anchor()
    piece.port_anchor(p["ports"][0], start, 1)


run(__file__, validate, build)
