"""Bake a circular secondary clarifier (saneamiento.sedimentador / secundario) to .glb.

Usage:
    blender -b --factory-startup --python-exit-code 1 --python tools/blender/build_plant_clarifier.py -- params.json

Data (from the project): interior diameter, wall height and water depth (the peripheral launder
crest), both over the central sump floor (the declared floor), purge DN, wall thickness and
material. Shape (fixed by the contract, for approval): floor slab, the cone the floor rises to
the wall, central sump and feed well, centre column, peripheral launder and the radial bridge.
Frame and anchors: see plant_units.py.
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import plant_units as pu  # noqa: E402
from redes_bake import run  # noqa: E402

FAMILY = "saneamiento.sedimentador"
SUBTYPE = "secundario"


def validate(p):
    pu.require_positive(p, [
        "inner_diameter_m", "height_m", "water_depth_m", "drain_dn_mm", "wall_thickness_m",
        "floor_thickness_m", "floor_rise_m", "sump_diameter_m", "feed_well_diameter_m",
        "feed_well_depth_m", "feed_well_plate_m", "column_diameter_m", "launder_width_m",
        "launder_depth_m", "launder_plate_m", "bridge_width_m", "bridge_depth_m",
    ])
    if p["water_depth_m"] >= p["height_m"]:
        sys.exit("water_depth_m must be below the wall top (height_m)")
    if p["floor_rise_m"] >= p["water_depth_m"] - p["launder_depth_m"]:
        sys.exit("the floor cone reaches the launder")
    ri = p["inner_diameter_m"] / 2
    if p["feed_well_diameter_m"] / 2 + p["feed_well_plate_m"] >= ri - p["launder_width_m"]:
        sys.exit("the feed well does not fit inside the launder")
    if not p["column_diameter_m"] < p["sump_diameter_m"] < p["feed_well_diameter_m"]:
        sys.exit("expected column < sump < feed well diameters")
    pu.require_connections(p)


def build(p):
    ri = p["inner_diameter_m"] / 2
    t, ft = p["wall_thickness_m"], p["floor_thickness_m"]
    ro = ri + t
    top = ft + p["height_m"]
    water = ft + p["water_depth_m"]
    seg = p["segments"]
    body = pu.loft([
        (0.0, 0.0), (ro, 0.0), (ro, top), (ri, top), (ri, ft + p["floor_rise_m"]),
        (p["sump_diameter_m"] / 2, ft), (0.0, ft),
    ], seg)
    lw, lt = p["launder_width_m"], p["launder_plate_m"]
    inner_lip = ri - lw
    launder = pu.loft([
        (inner_lip - lt, water - p["launder_depth_m"]), (ri, water - p["launder_depth_m"]),
        (ri, water - p["launder_depth_m"] + lt), (inner_lip, water - p["launder_depth_m"] + lt),
        (inner_lip, water), (inner_lip - lt, water),
    ], seg)
    rf = p["feed_well_diameter_m"] / 2
    feed_well = pu.loft([
        (rf, top - p["feed_well_depth_m"]), (rf + p["feed_well_plate_m"], top - p["feed_well_depth_m"]),
        (rf + p["feed_well_plate_m"], top), (rf, top),
    ], seg)
    bridge_top = top + p["bridge_depth_m"]
    column = pu.cylinder((0.0, 0.0, ft), (0.0, 0.0, bridge_top), p["column_diameter_m"] / 2, seg)
    half = p["bridge_width_m"] / 2
    bridge = pu.box(0.0, ro, -half, half, top, bridge_top)
    radius = p["drain_dn_mm"] / 2000.0
    drain = pu.cylinder((0.0, 0.0, ft + radius), (-(ro + pu.STUB_PAST_WALL_M), 0.0, ft + radius), radius, seg // 2)

    unit = pu.Unit(FAMILY, SUBTYPE, ft, {
        "redes_diametro_interior_m": p["inner_diameter_m"],
        "redes_altura_m": p["height_m"],
        "redes_espesor_m": t,
    })
    unit.part("cuerpo", [body, column], p["body_material"])
    unit.part("canaleta", [launder], p["launder_material"])
    unit.part("campana", [feed_well], p["metal_material"])
    unit.part("puente", [bridge], p["metal_material"])
    unit.part("purga", [drain], p["drain_material"])
    unit.connections(p["connections"], lambda a: pu.circle_exit(a, ro))
    unit.link(
        "Baked by tools/blender/build_plant_clarifier.py (docs/familias/F17-07-planta.md).\n"
        "Every part is explicit geometry: edit the parameters and bake again.\n"
    )


run(__file__, validate, build)
