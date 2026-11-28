"""Bake an in-line valve (agua.valvula: compuerta, mariposa, retencion) to .glb with Blender
running headless.

Usage:
    blender -b --factory-startup --python-exit-code 1 --python tools/blender/build_valve.py -- params.json

The names are listed in build_valve.receta.json (docs/familias/F17-05-accesorios.md).

Frame of the piece: Z up, metres, the bore along X with its axis at z = DN/2 over the origin (the
bore invert under the centre, empty "solera"). The reference port is at -X. A check valve opens
from its inlet (the pipe that ends at the valve) towards the other port; the arrow on its cover
says which way.
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from redes_bake import run  # noqa: E402
from redes_mallas import (  # noqa: E402
    Piece, box, check_ports, check_positive, cylinder, lathe, prism,
)

FAMILY = "agua.valvula"
SUBTYPES = {"compuerta", "mariposa", "retencion"}
X = (1.0, 0.0, 0.0)
Y = (0.0, 1.0, 0.0)
UP = (0.0, 0.0, 1.0)


def validate(p):
    check_positive(p, ["dn_mm", "flange_od_dn", "flange_od_extra_m", "flange_thickness_m",
                       "segments"])
    if p["subtype"] not in SUBTYPES:
        raise SystemExit(f"subtype must be one of {sorted(SUBTYPES)}")
    for key in ("gate", "butterfly", "check"):
        check_positive(p[key], [k for k, v in p[key].items() if isinstance(v, (int, float))])
    check_ports(p["ports"], 2)


def body_length(p, dn):
    shape = {"compuerta": p["gate"], "mariposa": p["butterfly"], "retencion": p["check"]}
    s = shape[p["subtype"]]
    return s["length_base_m"] + s["length_per_dn"] * dn


def flanges(p, dn, length, axis_z):
    radius = (p["flange_od_dn"] * dn + p["flange_od_extra_m"]) / 2
    t = p["flange_thickness_m"]
    return [cylinder((x, 0.0, axis_z), X, UP, t, radius, p["segments"])
            for x in (-length / 2, length / 2 - t)]


def handwheel(centre_z, radius, rim, spokes, segments):
    """Horizontal ring with spokes, centred on the vertical axis."""
    ring = lathe((0.0, 0.0, centre_z - rim / 2), UP, X,
                 [(radius - rim, 0.0), (radius, 0.0), (radius, rim), (radius - rim, rim)],
                 segments)
    bars = []
    for k in range(spokes // 2):
        half = rim / 2
        if k % 2 == 0:
            bars.append(box(-radius + rim / 2, radius - rim / 2, -half, half,
                            centre_z - half, centre_z + half))
        else:
            bars.append(box(-half, half, -radius + rim / 2, radius - rim / 2,
                            centre_z - half, centre_z + half))
    return [ring] + bars


def gate(p, dn, axis_z, length):
    s, seg = p["gate"], p["segments"]
    body_r = s["body_od_dn"] * dn / 2
    body = lathe((-length / 2, 0.0, axis_z), X, UP,
                 [(0.0, 0.0), (dn * 0.6, 0.0), (body_r, length * 0.25), (body_r, length * 0.75),
                  (dn * 0.6, length), (0.0, length)], seg)
    top = axis_z + s["bonnet_height_dn"] * dn
    bonnet = lathe((0.0, 0.0, axis_z), UP, X,
                   [(0.0, 0.0), (s["bonnet_width_dn"] * dn / 2, 0.0),
                    (s["bonnet_width_dn"] * dn / 4, top - axis_z), (0.0, top - axis_z)], seg)
    stem_top = top + s["stem_height_m"]
    stem = cylinder((0.0, 0.0, top), UP, X, s["stem_height_m"], s["stem_diameter_m"] / 2, 12)
    wheel = handwheel(stem_top, s["wheel_diameter_dn"] * dn / 2 + s["wheel_extra_m"],
                      s["wheel_rim_m"], 4, seg)
    return [body, bonnet], [stem] + wheel


def butterfly(p, dn, axis_z, length):
    s, seg = p["butterfly"], p["segments"]
    body_r = s["body_od_dn"] * dn / 2
    body = cylinder((-length / 2, 0.0, axis_z), X, UP, length, body_r, seg)
    neck_top = axis_z + body_r + s["neck_height_m"]
    neck = cylinder((0.0, 0.0, axis_z), UP, X, neck_top - axis_z, s["neck_diameter_m"] / 2, 16)
    g = s["gearbox_m"]
    gearbox = box(-g / 2, g / 2, -g / 2, g / 2, neck_top, neck_top + g)
    wheel = handwheel(neck_top + g + s["wheel_rim_m"],
                      s["wheel_diameter_dn"] * dn / 2 + s["wheel_extra_m"],
                      s["wheel_rim_m"], 4, seg)
    return [body, neck], [gearbox] + wheel


def check(p, dn, axis_z, length, flow_sign):
    s, seg = p["check"], p["segments"]
    body_r = s["body_od_dn"] * dn / 2
    body = lathe((-length / 2, 0.0, axis_z), X, UP,
                 [(0.0, 0.0), (dn * 0.6, 0.0), (body_r, length * 0.2), (body_r, length * 0.8),
                  (dn * 0.6, length), (0.0, length)], seg)
    # The cover sits over the hinge, towards the inlet.
    cover_x = -flow_sign * length * s["cover_offset_ratio"]
    cover_top = axis_z + body_r + s["cover_height_dn"] * dn
    cover = lathe((cover_x, 0.0, axis_z), UP, X,
                  [(0.0, 0.0), (s["cover_diameter_dn"] * dn / 2, 0.0),
                   (s["cover_diameter_dn"] * dn / 2, cover_top - axis_z - 0.02),
                   (0.0, cover_top - axis_z)], seg)
    hinge = cylinder((cover_x, -body_r * 1.05, axis_z + body_r * 0.6), Y, UP,
                     body_r * 2.1, s["hinge_diameter_m"] / 2, 12)
    a = s["arrow_m"]
    tip, back = flow_sign * a / 2, -flow_sign * a / 2
    triangle = [(back, -a / 3), (tip, 0.0), (back, a / 3)]
    if flow_sign < 0:
        triangle.reverse()
    arrow = prism([(x + cover_x, y) for x, y in triangle], cover_top - 0.02, cover_top + 0.01)
    return [body, cover], [hinge, arrow]


def flow_sign(ports):
    """+1 if the flow runs to +X (the reference port is the inlet), -1 the other way."""
    if ports[0]["name"].startswith("salida") and ports[1]["name"].startswith("entrada"):
        return -1.0
    return 1.0


def build(p):
    dn = p["dn_mm"] / 1000.0
    axis_z = dn / 2
    length = body_length(p, dn)
    if p["subtype"] == "compuerta":
        body, operator = gate(p, dn, axis_z, length)
    elif p["subtype"] == "mariposa":
        body, operator = butterfly(p, dn, axis_z, length)
    else:
        body, operator = check(p, dn, axis_z, length, flow_sign(p["ports"]))

    piece = Piece(FAMILY, p["subtype"], f"DN{p['dn_mm']:g}", {"redes_dn_mm": p["dn_mm"]})
    piece.part("cuerpo", body + flanges(p, dn, length, axis_z), p["material"])
    piece.part("accionamiento", operator, p["operator_material"])
    piece.invert_anchor()
    for i, (port, x) in enumerate(zip(p["ports"], (-length / 2, length / 2)), start=1):
        piece.port_anchor(port, (x, 0.0, axis_z), i)


run(__file__, validate, build)
