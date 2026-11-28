"""Bake a load chamber (cÃƒÂ¡mara de carga) to .glb with Blender running headless.

Usage:
    blender -b --factory-startup --python-exit-code 1 --python tools/blender/build_intake_break_chamber.py -- params.json

params.json is one parameter set or {"jobs": [...]}; the names are listed in
build_intake_break_chamber.receta.json (docs/familias/F17-06-captacion.md).

Frame of the piece: Z up, metres, the water flows along +X, the chamber is centred on the
origin in plan. Every level (bottom, top, water) arrives relative to the object's invert;
z = 0 is the underside of the floor slab and the empty "solera" marks the invert, so the viewer
sits the piece on the invert elevation of the object.

Parts, all explicit vertex and face lists: floor, walls and roof with the access opening
(cuerpo), the cover (tapa), the rungs under the opening (escalones) and the water surface
(agua). The fall is where the inlet pipe arrives above the water, from the pipe data; a
stilling baffle would need its own design data.
"""

import math
import os
import sys

import bpy

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from redes_bake import run  # noqa: E402

FAMILY = "agua.camara"
SUBTYPE = "quiebre"
INVERT_ANCHOR = "solera"
WATER = "agua"

# Display colour of each named material: the viewer reads baseColorFactor, the names stay the
# data (docs/17, section 5). An unknown material keeps a neutral grey instead of failing.
MATERIAL_COLOURS = {
    "hormigon": (0.66, 0.63, 0.57),
    "hormigon_armado": (0.62, 0.60, 0.55),
    "fundicion": (0.24, 0.25, 0.28),
    "acero": (0.55, 0.57, 0.60),
    "polipropileno": (0.86, 0.47, 0.13),
    WATER: (0.30, 0.55, 0.80),
}
NEUTRAL_GREY = (0.6, 0.6, 0.6)

POSITIVE = [
    "length_m",
    "width_m",
    "wall_thickness_m",
    "slab_thickness_m",
    "opening_size_m",
    "cover_thickness_m",
    "cover_clearance_m",
    "water_sheet_m",
    "step_spacing_m",
    "step_width_m",
    "step_projection_m",
    "step_bar_m",
]


def validate(p):
    for name in POSITIVE:
        if not p[name] > 0:
            sys.exit(f"{name} must be positive")
    for name in ["opening_margin_m", "step_first_height_m", "step_top_clearance_m",
                 "step_embed_m"]:
        if p[name] < 0:
            sys.exit(f"{name} cannot be negative")
    ceiling = p["top_rise_m"] - p["slab_thickness_m"]
    if ceiling <= p["bottom_rise_m"] + 0.3:
        sys.exit("the chamber needs room between its floor and its roof slab")
    if not p["bottom_rise_m"] < p["water_rise_m"] <= ceiling:
        sys.exit("the water level must be between the floor and the roof")
    if p["opening_margin_m"] + p["opening_size_m"] >= p["length_m"] / 2:
        sys.exit("the opening must fit in the upstream half of the roof")
    if p["opening_size_m"] >= p["width_m"]:
        sys.exit("the opening must fit in the width of the roof")
    if p["cover_thickness_m"] >= p["slab_thickness_m"]:
        sys.exit("cover_thickness_m must be smaller than slab_thickness_m")
    for c in p["connections"]:
        for key in ("name", "azimuth_deg", "invert_rise_m"):
            if key not in c:
                sys.exit(f"connection without {key}: {c}")


def layout(p):
    """Plan half sides and elevations every part shares, derived from the parameters only."""
    lowest = p["bottom_rise_m"] - p["slab_thickness_m"]
    hx, hy = p["length_m"] / 2, p["width_m"] / 2
    # The opening and the rungs go by the upstream wall.
    x0 = -hx + p["opening_margin_m"]

    def z(rise_above_invert):
        return rise_above_invert - lowest

    return {
        "z": z,
        "t": p["wall_thickness_m"],
        "hx": hx,
        "hy": hy,
        "floor": z(p["bottom_rise_m"]),
        "ceiling": z(p["top_rise_m"] - p["slab_thickness_m"]),
        "top": z(p["top_rise_m"]),
        "opening": (x0, x0 + p["opening_size_m"], p["opening_size_m"] / 2),
    }


def box(x0, x1, y0, y1, z0, z1):
    """Closed axis-aligned box as vertices and outward quads; the corners may come in any order."""
    x0, x1 = sorted((x0, x1))
    y0, y1 = sorted((y0, y1))
    z0, z1 = sorted((z0, z1))
    verts = [(x, y, z) for z in (z0, z1) for y in (y0, y1) for x in (x0, x1)]
    faces = [
        [0, 2, 3, 1],
        [4, 5, 7, 6],
        [0, 1, 5, 4],
        [2, 6, 7, 3],
        [0, 4, 6, 2],
        [1, 3, 7, 5],
    ]
    return verts, faces


def merge(shells):
    verts, faces = [], []
    for v, f in shells:
        base = len(verts)
        verts += v
        faces += [[base + i for i in face] for face in f]
    return verts, faces


def body_shells(p, g):
    """Floor slab, four walls and the roof slab around the access opening."""
    t, hx, hy = g["t"], g["hx"], g["hy"]
    floor, ceiling, top = g["floor"], g["ceiling"], g["top"]
    ox0, ox1, oh = g["opening"]
    return [
        box(-hx - t, hx + t, -hy - t, hy + t, 0.0, floor),
        box(-hx - t, hx + t, hy, hy + t, floor, ceiling),
        box(-hx - t, hx + t, -hy - t, -hy, floor, ceiling),
        box(hx, hx + t, -hy, hy, floor, ceiling),
        box(-hx - t, -hx, -hy, hy, floor, ceiling),
        box(-hx - t, ox0, -hy - t, hy + t, ceiling, top),
        box(ox1, hx + t, -hy - t, hy + t, ceiling, top),
        box(ox0, ox1, -hy - t, -oh, ceiling, top),
        box(ox0, ox1, oh, hy + t, ceiling, top),
    ]


def cover_shell(p, g):
    ox0, ox1, oh = g["opening"]
    c = p["cover_clearance_m"]
    return box(ox0 + c, ox1 - c, -oh + c, oh - c, g["top"] - p["cover_thickness_m"], g["top"])


def rung_heights(p, g):
    z = g["floor"] + p["step_first_height_m"]
    top = g["ceiling"] - p["step_top_clearance_m"]
    heights = []
    while z <= top + 1e-9:
        heights.append(round(z, 6))
        z += p["step_spacing_m"]
    return heights


def rung_shells(p, g):
    """U-shaped rungs on the upstream wall, under the opening."""
    s = p["step_bar_m"] / 2
    w = p["step_width_m"] / 2
    wall = -g["hx"]
    tread = wall + p["step_projection_m"]
    shells = []
    for z in rung_heights(p, g):
        shells.append(box(tread - s, tread + s, -w, w, z - s, z + s))
        for y in (-w, w):
            shells.append(box(wall - p["step_embed_m"], tread - s, y - s, y + s, z - s, z + s))
    return shells


def water_shell(p, g):
    level = g["z"](p["water_rise_m"])
    return box(-g["hx"], g["hx"], -g["hy"], g["hy"], level - p["water_sheet_m"], level)


def material(name):
    mat = bpy.data.materials.new(name)
    colour = MATERIAL_COLOURS.get(name, NEUTRAL_GREY)
    mat.diffuse_color = (*colour, 1.0)
    principled = mat.node_tree.nodes.get("Principled BSDF") if mat.node_tree else None
    if principled is not None:
        principled.inputs["Base Color"].default_value = (*colour, 1.0)
    return mat


def object_name(part):
    return f"{FAMILY.replace('.', '_')}__{SUBTYPE}__{part}"


def mesh_object(name, shells, mat):
    verts, faces = merge(shells)
    mesh = bpy.data.meshes.new(name)
    mesh.from_pydata(verts, [], faces)
    mesh.validate()
    mesh.materials.append(mat)
    return bpy.data.objects.new(object_name(name), mesh)


def anchor(name, location, props):
    empty = bpy.data.objects.new(name, None)
    empty.location = location
    for key, value in props.items():
        empty[key] = value
    return empty


def wall_exit(hx, hy, azimuth_deg):
    """Where a ray from the centre at this azimuth leaves a rectangle of half sides hx, hy."""
    a = math.radians(azimuth_deg)
    dx, dy = math.cos(a), math.sin(a)
    reach = min(hx / abs(dx) if abs(dx) > 1e-9 else math.inf,
                hy / abs(dy) if abs(dy) > 1e-9 else math.inf)
    return reach * dx, reach * dy


def connection_anchors(p, g):
    """One empty per pipe on the outer face of the wall it crosses, at the pipe invert.

    The height is the pipe invert at the chamber, taken from the project (invert_rise_m above
    the invert); a pipe without that datum has no entry here (the CLI reports it).
    """
    out = []
    hx, hy = g["hx"] + g["t"], g["hy"] + g["t"]
    for c in sorted(p["connections"], key=lambda c: c["name"]):
        a = math.radians(c["azimuth_deg"])
        props = {
            "redes_tramo": c["name"].split("__", 1)[-1],
            "redes_azimut_deg": c["azimuth_deg"],
            "redes_sobre_solera_m": c["invert_rise_m"],
            "redes_eje": [round(math.cos(a), 6), round(math.sin(a), 6), 0.0],
        }
        if c.get("dn_mm"):
            props["redes_dn_mm"] = c["dn_mm"]
        x, y = wall_exit(hx, hy, c["azimuth_deg"])
        out.append(anchor(c["name"], (x, y, g["z"](c["invert_rise_m"])), props))
    return out


def tag(obj, p, part, mat_name):
    obj["redes_familia"] = FAMILY
    obj["redes_subtipo"] = SUBTYPE
    obj["redes_parte"] = part
    obj["redes_material"] = mat_name
    obj["redes_largo_m"] = p["length_m"]
    obj["redes_ancho_m"] = p["width_m"]
    obj["redes_espesor_m"] = p["wall_thickness_m"]


def build(p):
    g = layout(p)
    parts = [
        ("cuerpo", body_shells(p, g), p["body_material"]),
        ("tapa", [cover_shell(p, g)], p["cover_material"]),
        ("agua", [water_shell(p, g)], WATER),
    ]
    rungs = rung_shells(p, g)
    if rungs:
        parts.append(("escalones", rungs, p["step_material"]))
    root = bpy.data.objects.new(
        object_name(f"L{round(p['length_m'] * 1000)}xB{round(p['width_m'] * 1000)}"), None
    )
    scene = bpy.context.scene
    scene.collection.objects.link(root)
    materials = {}
    for part, shells, mat_name in parts:
        if mat_name not in materials:
            materials[mat_name] = material(mat_name)
        obj = mesh_object(part, shells, materials[mat_name])
        tag(obj, p, part, mat_name)
        obj.parent = root
        scene.collection.objects.link(obj)
    invert = anchor(INVERT_ANCHOR, (0.0, 0.0, g["z"](0.0)), {"redes_parte": "solera"})
    for empty in [invert] + connection_anchors(p, g):
        empty.parent = root
        scene.collection.objects.link(empty)


run(__file__, validate, build)
