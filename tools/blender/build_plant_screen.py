"""Bake a mechanically raked coarse bar screen (comun.reja / gruesos_mecanica) to .glb.

Usage:
    blender -b --factory-startup --python-exit-code 1 --python tools/blender/build_plant_screen.py -- params.json

Data (from the project): screen width (the channel width), screen height, inclination, clear
opening between bars, bar thickness and the channel wall thickness. The bar count follows from
those by layout alone: as many bars as fit with the declared opening between them, centred.
Shape (fixed by the contract, for approval): channel length, floor slab, bar depth, rake housing
and materials. The channel is open at both ends; the bars lean downstream (+X). Frame and
anchors: see plant_units.py.
"""

import math
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import plant_units as pu  # noqa: E402
from redes_bake import run  # noqa: E402

FAMILY = "comun.reja"
SUBTYPE = "gruesos_mecanica"


def validate(p):
    pu.require_positive(p, [
        "width_m", "screen_height_m", "opening_mm", "bar_thickness_mm", "wall_thickness_m",
        "channel_length_m", "floor_thickness_m", "bar_depth_m", "housing_depth_m",
        "housing_height_m",
    ])
    if not 0 < p["inclination_deg"] <= 90:
        sys.exit("inclination_deg must be in (0, 90]")
    if bar_positions(p) == []:
        sys.exit("not one bar fits in width_m with that opening")
    if run_of(p) + p["bar_depth_m"] >= p["channel_length_m"]:
        sys.exit("channel_length_m is too short for the inclined screen")
    pu.require_connections(p)


def run_of(p):
    """Horizontal reach of an inclined bar."""
    return p["screen_height_m"] / math.tan(math.radians(p["inclination_deg"]))


def bar_positions(p):
    """Centres in Y of the bars that fit with the declared clear opening between them."""
    a, b, w = p["opening_mm"] / 1000.0, p["bar_thickness_mm"] / 1000.0, p["width_m"]
    count = int((w - a) // (a + b))
    if count < 1:
        return []
    start = -(count * b + (count - 1) * a) / 2 + b / 2
    return [start + k * (a + b) for k in range(count)]


def bars(p, ft):
    reach, d, b = run_of(p), p["bar_depth_m"] / 2, p["bar_thickness_mm"] / 2000.0
    foot = -reach / 2
    top = ft + p["screen_height_m"]
    outline = [(foot - d, ft), (foot + d, ft), (foot + reach + d, top), (foot + reach - d, top)]
    return [pu.prism_xz(outline, y - b, y + b) for y in bar_positions(p)]


def build(p):
    hx, hy = p["channel_length_m"] / 2, p["width_m"] / 2
    t, ft = p["wall_thickness_m"], p["floor_thickness_m"]
    top = ft + p["screen_height_m"]
    channel = [
        pu.box(-hx, hx, -hy - t, hy + t, 0.0, ft),
        pu.box(-hx, hx, -hy - t, -hy, ft, top),
        pu.box(-hx, hx, hy, hy + t, ft, top),
    ]
    head = run_of(p) / 2
    housing = pu.box(head - p["housing_depth_m"] / 2, head + p["housing_depth_m"] / 2,
                     -hy - t, hy + t, top, top + p["housing_height_m"])
    unit = pu.Unit(FAMILY, SUBTYPE, ft, {
        "redes_ancho_m": p["width_m"],
        "redes_altura_m": p["screen_height_m"],
        "redes_espesor_m": t,
    })
    unit.part("canal", channel, p["channel_material"])
    unit.part("barras", bars(p, ft), p["bar_material"])
    unit.part("rastrillo", [housing], p["bar_material"])
    unit.connections(p["connections"], lambda a: pu.rect_exit(a, hx, hy + t))
    unit.link(
        "Baked by tools/blender/build_plant_screen.py (docs/familias/F17-07-planta.md).\n"
        "Every part is explicit geometry: edit the parameters and bake again.\n"
    )


run(__file__, validate, build)
