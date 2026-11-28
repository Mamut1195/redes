"""Bake a longitudinal desander (desarenador de flujo horizontal) to .glb with Blender headless.

Usage:
    blender -b --factory-startup --python-exit-code 1 --python tools/blender/build_intake_desander.py -- params.json

params.json is one parameter set or {"jobs": [...]}; the names are listed in
build_intake_desander.receta.json (docs/familias/F17-06-captacion.md).

Frame of the piece: Z up, metres, the water flows along +X, the basin is centred on the origin
in plan. The bottom arrives relative to the object's invert; z = 0 is the underside of the
floor slab and the empty "solera" marks the invert, so the viewer sits the piece on the invert
elevation of the object.

Parts, all explicit vertex and face lists: the open basin (cuerpo), the purge pipe through the
downstream wall at the bottom (purga), whose end is the "desague" anchor, and the water surface
at the declared depth (agua). The floor is flat: a hopper or an inlet transition would need
their own design data.
"""

import math
import os
import sys

import bpy

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from redes_bake import run  # noqa: E402

FAMILY = "comun.desarenador"
SUBTYPE = "longitudinal"
INVERT_ANCHOR = "solera"
WATER = "agua"

# Display colour of each named material: the viewer reads baseColorFactor, the names stay the
# data (docs/17, section 5). An unknown material keeps a neutral grey instead of failing.
MATERIAL_COLOURS = {
    "hormigon": (0.66, 0.63, 0.57),
    "hormigon_armado": (0.62, 0.60, 0.55),
    "pvc": (0.80, 0.82, 0.86),
    "acero": (0.55, 0.57, 0.60),
    WATER: (0.30, 0.55, 0.80),
}
NEUTRAL_GREY = (0.6, 0.6, 0.6)

POSITIVE = [
    "length_m",
    "width_m",
    "wall_height_m",
    "water_depth_m",
    "purge_dn_mm",
    "wall_thickness_m",
    "slab_thickness_m",
    "water_sheet_m",
    "purge_stub_m",
]


def validate(p):
    for name in POSITIVE:
        if not p[name] > 0:
            sys.exit(f"{name} must be positive")
    if p["water_depth_m"] > p["wall_height_m"]:
        sys.exit("the water depth must fit inside the walls")
    if p["purge_dn_mm"] / 1000.0 >= min(p["wall_height_m"], p["width_m"]):
        sys.exit("the purge pipe must fit in the downstream wall")
    if not p["segments"] >= 8:
        sys.exit("segments must be at least 8")
    for c in p["connections"]:
        for key in ("name", "azimuth_deg", "invert_rise_m"):
            if key not in c:
                sys.exit(f"connection without {key}: {c}")


def layout(p):
    bottom = p["bottom_rise_m"]
    lowest = bottom - p["slab_thickness_m"]
    t = p["wall_thickness_m"]
    return {
        # Rise above the invert -> z of the piece.
        "z": lambda rise: rise - lowest,
        "t": t,
        "hx": p["length_m"] / 2,
        "hy": p["width_m"] / 2,
        "bottom": bottom,
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


def cylinder_x(radius, x0, x1, y, z, segments):
    """Closed cylinder along +X from x0 to x1, axis at (y, z)."""
    ring = [
        (y + radius * math.cos(2 * math.pi * k / segments),
         z + radius * math.sin(2 * math.pi * k / segments))
        for k in range(segments)
    ]
    verts = [(x0, v, w) for v, w in ring] + [(x1, v, w) for v, w in ring]
    n = segments
    # The ring runs counter-clockwise seen from +X: the x1 cap keeps it, the x0 cap reverses it.
    faces = [list(reversed(range(n))), list(range(n, 2 * n))]
    for k in range(n):
        j = (k + 1) % n
        faces.append([k, j, n + j, n + k])
    return verts, faces


def merge(shells):
    verts, faces = [], []
    for v, f in shells:
        base = len(verts)
        verts += v
        faces += [[base + i for i in face] for face in f]
    return verts, faces


def basin_shells(p, g):
    """Floor slab under the whole footprint and four walls standing on it."""
    z, t, hx, hy = g["z"], g["t"], g["hx"], g["hy"]
    floor, top = z(g["bottom"]), z(g["bottom"] + p["wall_height_m"])
    return [
        box(-hx - t, hx + t, -hy - t, hy + t, 0.0, floor),
        box(-hx - t, hx + t, hy, hy + t, floor, top),
        box(-hx - t, hx + t, -hy - t, -hy, floor, top),
        box(hx, hx + t, -hy, hy, floor, top),
        box(-hx - t, -hx, -hy, hy, floor, top),
    ]


def purge_shell(p, g):
    """Purge pipe at the bottom of the downstream wall, sticking out of it."""
    r = p["purge_dn_mm"] / 2000.0
    floor = g["z"](g["bottom"])
    start = g["hx"] - r
    end = g["hx"] + g["t"] + p["purge_stub_m"]
    return cylinder_x(r, start, end, 0.0, floor + r, p["segments"])


def water_shell(p, g):
    level = g["z"](g["bottom"] + p["water_depth_m"])
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

    The height is the pipe invert at the desander, taken from the project (invert_rise_m above
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
    obj["redes_altura_util_m"] = p["wall_height_m"]
    obj["redes_tirante_m"] = p["water_depth_m"]


def build(p):
    g = layout(p)
    parts = [
        ("cuerpo", basin_shells(p, g), p["body_material"]),
        ("purga", [purge_shell(p, g)], p["purge_material"]),
        ("agua", [water_shell(p, g)], WATER),
    ]
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
    purge_end = g["hx"] + g["t"] + p["purge_stub_m"]
    purge = anchor(
        "desague",
        (purge_end, 0.0, g["z"](g["bottom"])),
        {"redes_parte": "purga", "redes_dn_mm": p["purge_dn_mm"]},
    )
    for empty in [invert, purge] + connection_anchors(p, g):
        empty.parent = root
        scene.collection.objects.link(empty)


run(__file__, validate, build)
