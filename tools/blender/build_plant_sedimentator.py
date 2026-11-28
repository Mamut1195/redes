"""Bake a horizontal-flow sedimentation tank (agua.sedimentador / convencional) to .glb.

Usage:
    blender -b --factory-startup --python-exit-code 1 --python tools/blender/build_plant_sedimentator.py -- params.json

Data (from the project): interior length, width and wall height, water depth (the outlet launder
crest), purge DN, wall thickness and material. Shape (fixed by the contract, for approval): floor
slab, sludge hopper at the inlet end, slatted diffuser wall, outlet launders. Frame and anchors:
see plant_units.py.
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import plant_units as pu  # noqa: E402
from redes_bake import run  # noqa: E402

FAMILY = "agua.sedimentador"
SUBTYPE = "convencional"


def validate(p):
    pu.require_positive(p, [
        "length_m", "width_m", "height_m", "water_depth_m", "drain_dn_mm", "wall_thickness_m",
        "floor_thickness_m", "hopper_length_m", "hopper_depth_m", "hopper_bottom_m",
        "diffuser_thickness_m", "diffuser_slat_m", "diffuser_slot_m", "launder_width_m",
        "launder_depth_m", "launder_plate_m", "launder_spacing_m",
    ])
    pu.require_count(p, "launder_count")
    if p["water_depth_m"] >= p["height_m"]:
        sys.exit("water_depth_m must be below the wall top (height_m)")
    if p["hopper_length_m"] + p["launder_count"] * p["launder_spacing_m"] >= p["length_m"]:
        sys.exit("length_m is too short for the hopper and the outlet launders")
    if p["hopper_bottom_m"] >= min(p["hopper_length_m"], p["width_m"]):
        sys.exit("hopper_bottom_m must be smaller than the hopper top")
    if p["diffuser_offset_m"] >= p["length_m"] / 2:
        sys.exit("diffuser_offset_m must stay in the inlet half")
    if p["launder_depth_m"] >= p["water_depth_m"]:
        sys.exit("launder_depth_m must be smaller than water_depth_m")
    pu.require_connections(p)


def hopper(p, hx, hy):
    """Hopper block under the inlet end: its top is the floor, its cavity holds the sludge."""
    t, ft = p["wall_thickness_m"], p["floor_thickness_m"]
    x0, x1 = -hx, -hx + p["hopper_length_m"]
    cx, half = (x0 + x1) / 2, p["hopper_bottom_m"] / 2
    outer = (x0 - t, x1, -hy - t, hy + t)
    block = pu.hopper_block(
        pu.rect(*outer),
        pu.rect(x0, x1, -hy, hy),
        pu.rect(cx - half, cx + half, -half, half),
        ft,
        ft - p["hopper_depth_m"],
        -p["hopper_depth_m"],
    )
    return block, outer, cx


def diffuser(p, hx, hy):
    """Slatted diffuser wall across the inlet: slats with slots between them, open at the bottom
    so the sludge reaches the hopper."""
    x = -hx + p["diffuser_offset_m"]
    th = p["diffuser_thickness_m"] / 2
    z0 = p["floor_thickness_m"] + p["diffuser_bottom_gap_m"]
    z1 = p["floor_thickness_m"] + p["height_m"]
    pitch = p["diffuser_slat_m"] + p["diffuser_slot_m"]
    count = int((2 * hy + p["diffuser_slot_m"]) // pitch)
    start = -(count * pitch - p["diffuser_slot_m"]) / 2
    return [
        pu.box(x - th, x + th, start + k * pitch, start + k * pitch + p["diffuser_slat_m"], z0, z1)
        for k in range(count)
    ]


def launders(p, hx, hy):
    crest = p["floor_thickness_m"] + p["water_depth_m"]
    w = p["launder_width_m"]
    shells = []
    for k in range(p["launder_count"]):
        xc = hx - p["launder_spacing_m"] * (k + 0.5)
        shells += pu.channel_across(xc - w / 2, xc + w / 2, -hy, hy, crest - p["launder_depth_m"],
                                    crest, p["launder_plate_m"])
    return shells


def build(p):
    p = dict(p, launder_count=pu.require_count(p, "launder_count"))
    hx, hy = p["length_m"] / 2, p["width_m"] / 2
    t, ft = p["wall_thickness_m"], p["floor_thickness_m"]
    top = ft + p["height_m"]
    block, outer, hopper_x = hopper(p, hx, hy)
    body = pu.slab_around(-hx - t, hx + t, -hy - t, hy + t, 0.0, ft, outer)
    body += [block] + pu.rect_walls(p["length_m"], p["width_m"], t, ft, top)
    radius = p["drain_dn_mm"] / 2000.0
    drain_z = ft - p["hopper_depth_m"] + radius
    drain = pu.cylinder((hopper_x, 0.0, drain_z), (-hx - t - pu.STUB_PAST_WALL_M, 0.0, drain_z), radius,
                        p["segments"])

    unit = pu.Unit(FAMILY, SUBTYPE, ft, {
        "redes_largo_m": p["length_m"],
        "redes_ancho_m": p["width_m"],
        "redes_altura_m": p["height_m"],
        "redes_espesor_m": t,
    })
    unit.part("cuerpo", body, p["body_material"])
    unit.part("pantalla_difusora", diffuser(p, hx, hy), p["diffuser_material"])
    unit.part("canaletas", launders(p, hx, hy), p["launder_material"])
    unit.part("purga", [drain], p["drain_material"])
    unit.connections(p["connections"], lambda a: pu.rect_exit(a, hx + t, hy + t))
    unit.link(
        "Baked by tools/blender/build_plant_sedimentator.py (docs/familias/F17-07-planta.md).\n"
        "Every part is explicit geometry: edit the parameters and bake again.\n"
    )


run(__file__, validate, build)
