"""Bake a two-channel grit chamber (comun.desarenador / dos_naves) to .glb.

Usage:
    blender -b --factory-startup --python-exit-code 1 --python tools/blender/build_plant_grit_chamber.py -- params.json

Data (from the project): interior length, width and wall height, water depth (the outlet weir
crest), number of channels and of inlet gates (shared equally between the channels), purge DN,
wall thickness and material. Shape (fixed by the contract, for approval): floor slab, the benches
that slope to a grit gutter along each channel, the outlet weir position, the gates (drawn
raised, above the water) and their stems. Frame and anchors: see plant_units.py.
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import plant_units as pu  # noqa: E402
from redes_bake import run  # noqa: E402

FAMILY = "comun.desarenador"
SUBTYPE = "dos_naves"


def validate(p):
    pu.require_positive(p, [
        "length_m", "width_m", "height_m", "water_depth_m", "drain_dn_mm", "wall_thickness_m",
        "floor_thickness_m", "gutter_width_m", "bench_rise_m", "weir_offset_m", "weir_thickness_m",
        "gate_offset_m", "gate_plate_m", "stem_diameter_m", "stem_height_m",
    ])
    channels = pu.require_count(p, "channel_count")
    gates = pu.require_count(p, "gate_count")
    if gates % channels:
        sys.exit("gate_count must be a multiple of channel_count (equal gates per channel)")
    if p["water_depth_m"] >= p["height_m"]:
        sys.exit("water_depth_m must be below the wall top (height_m)")
    if p["bench_rise_m"] >= p["water_depth_m"]:
        sys.exit("bench_rise_m must be below the water")
    bay = (p["width_m"] - (channels - 1) * p["wall_thickness_m"]) / channels
    if p["gutter_width_m"] >= bay or p["drain_dn_mm"] / 1000.0 >= bay:
        sys.exit("the channels are too narrow for the gutter and the purge")
    if p["gate_offset_m"] + p["weir_offset_m"] >= p["length_m"]:
        sys.exit("length_m is too short for the gates and the outlet weir")
    pu.require_connections(p)


def benches(p, channels, x0, x1):
    ft, g, rise = p["floor_thickness_m"], p["gutter_width_m"] / 2, p["bench_rise_m"]
    shells = []
    for y0, y1 in channels:
        yc = (y0 + y1) / 2
        shells.append(pu.prism_yz([(y0, ft), (yc - g, ft), (y0, ft + rise)], x0, x1))
        shells.append(pu.prism_yz([(yc + g, ft), (y1, ft), (y1, ft + rise)], x0, x1))
    return shells


def gates(p, channels, hx, top):
    """Raised gates: the plate fills the dry height above the water, with its stem on top."""
    per_channel = pu.require_count(p, "gate_count") // len(channels)
    x = -hx + p["gate_offset_m"]
    th = p["gate_plate_m"] / 2
    z0 = p["floor_thickness_m"] + p["water_depth_m"]
    plates, stems = [], []
    for y0, y1 in channels:
        width = (y1 - y0) / per_channel
        for k in range(per_channel):
            a, b = y0 + k * width, y0 + (k + 1) * width
            plates.append(pu.box(x - th, x + th, a, b, z0, top))
            yc = (a + b) / 2
            stems.append(pu.cylinder((x, yc, top), (x, yc, top + p["stem_height_m"]),
                                     p["stem_diameter_m"] / 2, p["segments"]))
    return plates + stems


def build(p):
    hx, hy = p["length_m"] / 2, p["width_m"] / 2
    t, ft = p["wall_thickness_m"], p["floor_thickness_m"]
    top = ft + p["height_m"]
    count = pu.require_count(p, "channel_count")
    channels = pu.bays(-hy, hy, count, t)
    weir_x = hx - p["weir_offset_m"]
    th = p["weir_thickness_m"] / 2
    crest = ft + p["water_depth_m"]

    body = [pu.box(-hx - t, hx + t, -hy - t, hy + t, 0.0, ft)]
    body += pu.rect_walls(p["length_m"], p["width_m"], t, ft, top)
    body += [pu.box(-hx, hx, y - t / 2, y + t / 2, ft, top)
             for y in pu.divider_centres(-hy, hy, count, t)]
    body += [pu.box(weir_x - th, weir_x + th, y0, y1, ft, crest) for y0, y1 in channels]
    body += benches(p, channels, -hx, weir_x - th)
    radius = p["drain_dn_mm"] / 2000.0
    drains = [
        pu.cylinder((-hx + p["gate_offset_m"], (y0 + y1) / 2, ft + radius),
                    (-hx - t - pu.STUB_PAST_WALL_M, (y0 + y1) / 2, ft + radius), radius, p["segments"])
        for y0, y1 in channels
    ]
    unit = pu.Unit(FAMILY, SUBTYPE, ft, {
        "redes_largo_m": p["length_m"],
        "redes_ancho_m": p["width_m"],
        "redes_altura_m": p["height_m"],
        "redes_espesor_m": t,
    })
    unit.part("cuerpo", body, p["body_material"])
    unit.part("compuertas", gates(p, channels, hx, top), p["gate_material"])
    unit.part("purga", drains, p["drain_material"])
    unit.connections(p["connections"], lambda a: pu.rect_exit(a, hx + t, hy + t))
    unit.link(
        "Baked by tools/blender/build_plant_grit_chamber.py (docs/familias/F17-07-planta.md).\n"
        "Every part is explicit geometry: edit the parameters and bake again.\n"
    )


run(__file__, validate, build)
