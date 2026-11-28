"""Bake a horizontal-flow baffled flocculator (agua.floculador / pantallas_horizontales) to .glb.

Usage:
    blender -b --factory-startup --python-exit-code 1 --python tools/blender/build_plant_flocculator.py -- params.json

Data (from the project): interior length, width and wall height, number of chambers, number of
baffles (for the whole unit, shared out between the chambers from the first one) and their
spacing, wall thickness and material. Shape (fixed by the contract, for approval): floor slab,
baffle thickness and material, and the end opening each baffle and chamber wall leaves, on
alternate sides so the water snakes through. Frame and anchors: see plant_units.py.
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import plant_units as pu  # noqa: E402
from redes_bake import run  # noqa: E402

FAMILY = "agua.floculador"
SUBTYPE = "pantallas_horizontales"


def validate(p):
    pu.require_positive(p, [
        "length_m", "width_m", "height_m", "baffle_spacing_m", "wall_thickness_m",
        "floor_thickness_m", "baffle_thickness_m", "baffle_gap_m",
    ])
    pu.require_count(p, "chamber_count")
    pu.require_count(p, "baffle_count", minimum=0)
    if p["baffle_gap_m"] >= p["width_m"]:
        sys.exit("baffle_gap_m must be smaller than width_m")
    for (x0, x1), count in chambers(p):
        if count and (count - 1) * p["baffle_spacing_m"] + p["baffle_thickness_m"] >= x1 - x0:
            sys.exit(f"{count} baffles {p['baffle_spacing_m']} m apart do not fit in a "
                     f"{x1 - x0:.2f} m chamber")
    pu.require_connections(p)


def chambers(p):
    """Each chamber's (start, end) and how many baffles it holds."""
    count = pu.require_count(p, "chamber_count")
    baffles = pu.require_count(p, "baffle_count", minimum=0)
    hx = p["length_m"] / 2
    spans = pu.bays(-hx, hx, count, p["wall_thickness_m"])
    share = [baffles // count + (1 if k < baffles % count else 0) for k in range(count)]
    return list(zip(spans, share))


def baffle_centres(p, span, count):
    mid = (span[0] + span[1]) / 2
    s = p["baffle_spacing_m"]
    return [mid + (k - (count - 1) / 2) * s for k in range(count)]


def build(p):
    hx, hy = p["length_m"] / 2, p["width_m"] / 2
    t, ft = p["wall_thickness_m"], p["floor_thickness_m"]
    top = ft + p["height_m"]
    layout = chambers(p)
    dividers = pu.divider_centres(-hx, hx, len(layout), t)
    baffles = [x for span, count in layout for x in baffle_centres(p, span, count)]
    # One sequence from inlet to outlet, so every wall opens on the other side from the last.
    walls = sorted([(x, "divider") for x in dividers] + [(x, "baffle") for x in baffles])
    concrete, plates = [], []
    for i, (x, kind) in enumerate(walls):
        thickness = t if kind == "divider" else p["baffle_thickness_m"]
        y0, y1 = (-hy, hy - p["baffle_gap_m"]) if i % 2 == 0 else (-hy + p["baffle_gap_m"], hy)
        shell = pu.box(x - thickness / 2, x + thickness / 2, y0, y1, ft, top)
        (concrete if kind == "divider" else plates).append(shell)

    body = [pu.box(-hx - t, hx + t, -hy - t, hy + t, 0.0, ft)]
    body += pu.rect_walls(p["length_m"], p["width_m"], t, ft, top) + concrete
    unit = pu.Unit(FAMILY, SUBTYPE, ft, {
        "redes_largo_m": p["length_m"],
        "redes_ancho_m": p["width_m"],
        "redes_altura_m": p["height_m"],
        "redes_espesor_m": t,
    })
    unit.part("cuerpo", body, p["body_material"])
    unit.part("pantallas", plates, p["baffle_material"])
    unit.connections(p["connections"], lambda a: pu.rect_exit(a, hx + t, hy + t))
    unit.link(
        "Baked by tools/blender/build_plant_flocculator.py (docs/familias/F17-07-planta.md).\n"
        "Every part is explicit geometry: edit the parameters and bake again.\n"
    )


run(__file__, validate, build)
