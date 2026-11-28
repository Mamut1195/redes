"""Bake a fixed-weir intake (bocatoma de barraje fijo) to .glb with Blender running headless.

Usage:
    blender -b --factory-startup --python-exit-code 1 --python tools/blender/build_intake_weir.py -- params.json

params.json is one parameter set or {"jobs": [...]}; the names are listed in
build_intake_weir.receta.json (docs/familias/F17-06-captacion.md).

Frame of the piece: Z up, metres, the river flows along +X and the weir crosses it along Y,
centred on the origin in plan. Every level (bed, sill, crest) arrives relative to the object's
invert; z = 0 is the lowest underside of the piece and the empty "solera" marks the invert, so
the viewer sits the piece on the invert elevation of the object.

Parts, all explicit vertex and face lists: the weir with its apron (azud), the training walls
(muros) with the intake window cut in the wall of the bank the outlet pipe leaves from, and the
intake box behind the window (toma).
"""

import math
import os
import sys

import bpy

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from redes_bake import run  # noqa: E402

FAMILY = "agua.bocatoma"
SUBTYPE = "barraje_fijo"
INVERT_ANCHOR = "solera"

# Display colour of each named material: the viewer reads baseColorFactor, the names stay the
# data (docs/17, section 5). An unknown material keeps a neutral grey instead of failing.
MATERIAL_COLOURS = {
    "hormigon": (0.66, 0.63, 0.57),
    "hormigon_armado": (0.62, 0.60, 0.55),
    "mamposteria": (0.58, 0.52, 0.45),
}
NEUTRAL_GREY = (0.6, 0.6, 0.6)

POSITIVE = [
    "weir_length_m",
    "window_width_m",
    "window_height_m",
    "wall_thickness_m",
    "crest_width_m",
    "downstream_slope_h_per_v",
    "foundation_depth_m",
    "apron_length_m",
    "apron_thickness_m",
    "wall_freeboard_m",
    "intake_box_length_m",
]


def validate(p):
    for name in POSITIVE:
        if not p[name] > 0:
            sys.exit(f"{name} must be positive")
    for name in ["window_offset_m", "wall_upstream_m"]:
        if p[name] < 0:
            sys.exit(f"{name} cannot be negative")
    if p["crest_rise_m"] <= p["bed_rise_m"]:
        sys.exit("the crest must be above the river bed")
    if p["sill_rise_m"] <= p["bed_rise_m"] - p["foundation_depth_m"]:
        sys.exit("the window sill must be above the foundation")
    if p["sill_rise_m"] + p["window_height_m"] >= wall_top(p):
        sys.exit("the window must fit under the top of the wall")
    for c in p["connections"]:
        for key in ("name", "azimuth_deg", "invert_rise_m"):
            if key not in c:
                sys.exit(f"connection without {key}: {c}")


def wall_top(p):
    return p["crest_rise_m"] + p["wall_freeboard_m"]


def intake_side(p):
    """+1 or -1: the bank of the first outlet pipe; -Y when no pipe leaves the intake."""
    outlets = [c for c in p["connections"] if c["name"].startswith("salida")]
    if not outlets:
        return -1.0
    return -1.0 if math.sin(math.radians(outlets[0]["azimuth_deg"])) < 0 else 1.0


def layout(p):
    """Plan stations and elevations every part shares, derived from the parameters only."""
    bed, crest, sill = p["bed_rise_m"], p["crest_rise_m"], p["sill_rise_m"]
    t = p["wall_thickness_m"]
    lowest = min(bed - p["foundation_depth_m"], bed - p["apron_thickness_m"], sill - t)
    upstream_face = -p["crest_width_m"] / 2
    toe = p["crest_width_m"] / 2 + p["downstream_slope_h_per_v"] * (crest - bed)
    window_end = upstream_face - p["window_offset_m"]
    window_start = window_end - p["window_width_m"]
    return {
        # Rise above the invert -> z of the piece.
        "z": lambda rise: rise - lowest,
        "t": t,
        "half": p["weir_length_m"] / 2,
        "upstream_face": upstream_face,
        "toe": toe,
        "apron_end": toe + p["apron_length_m"],
        "window": (window_start, window_end),
        "wall_start": window_start - p["wall_upstream_m"],
        "side": intake_side(p),
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


def extrude_y(profile, y0, y1):
    """Closed prism of a convex counter-clockwise (x, z) profile, extruded from y0 to y1."""
    n = len(profile)
    verts = [(x, y0, z) for x, z in profile] + [(x, y1, z) for x, z in profile]
    # Seen from -Y (outside the y0 cap) the profile already runs counter-clockwise.
    faces = [list(range(n)), list(reversed(range(n, 2 * n)))]
    for i in range(n):
        j = (i + 1) % n
        faces.append([j, i, n + i, n + j])
    return verts, faces


def merge(shells):
    verts, faces = [], []
    for v, f in shells:
        base = len(verts)
        verts += v
        faces += [[base + i for i in face] for face in f]
    return verts, faces


def weir_shells(p, g):
    """Weir body (vertical upstream face, flat crest, sloping downstream face) and its apron."""
    z = g["z"]
    bed, crest = p["bed_rise_m"], p["crest_rise_m"]
    footing = bed - p["foundation_depth_m"]
    up, crest_end, toe = g["upstream_face"], p["crest_width_m"] / 2, g["toe"]
    profile = [
        (up, z(footing)),
        (toe, z(footing)),
        (toe, z(bed)),
        (crest_end, z(crest)),
        (up, z(crest)),
    ]
    half = g["half"]
    body = extrude_y(profile, -half, half)
    apron = box(toe, g["apron_end"], -half, half, z(bed - p["apron_thickness_m"]), z(bed))
    return [body, apron]


def wall_shells(p, g):
    """Training walls on both banks; the intake bank's wall is open at the window."""
    z, t, half, side = g["z"], g["t"], g["half"], g["side"]
    bottom = z(p["bed_rise_m"] - p["foundation_depth_m"])
    top = z(wall_top(p))
    start, end = g["wall_start"], g["apron_end"]
    w0, w1 = g["window"]
    sill = z(p["sill_rise_m"])
    lintel = z(p["sill_rise_m"] + p["window_height_m"])
    shells = [box(start, end, -side * half, -side * (half + t), bottom, top)]
    near, far = side * half, side * (half + t)
    shells += [
        box(start, w0, near, far, bottom, top),
        box(w1, end, near, far, bottom, top),
        box(w0, w1, near, far, bottom, sill),
        box(w0, w1, near, far, lintel, top),
    ]
    return shells


def intake_box_shells(p, g):
    """Open box behind the window, floored at the sill, that the outlet pipe leaves from."""
    z, t, side = g["z"], g["t"], g["side"]
    w0, w1 = g["window"]
    sill, top = z(p["sill_rise_m"]), z(wall_top(p))
    wall = side * (g["half"] + t)
    inner_end = wall + side * p["intake_box_length_m"]
    outer_end = inner_end + side * t
    return [
        box(w0 - t, w1 + t, wall, outer_end, sill - t, sill),
        box(w0 - t, w0, wall, inner_end, sill, top),
        box(w1, w1 + t, wall, inner_end, sill, top),
        box(w0 - t, w1 + t, inner_end, outer_end, sill, top),
    ]


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


def connection_anchors(p, g):
    """One empty per pipe: outlets on the outer face of the intake box, inlets upstream.

    Its height is the pipe invert at the intake, taken from the project (invert_rise_m above
    the invert); a pipe without that datum has no entry here (the CLI reports it).
    """
    w0, w1 = g["window"]
    box_face = g["side"] * (g["half"] + 2 * g["t"] + p["intake_box_length_m"])
    out = []
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
        z = g["z"](c["invert_rise_m"])
        if c["name"].startswith("salida"):
            location = ((w0 + w1) / 2, box_face, z)
        else:
            location = (g["wall_start"], 0.0, z)
        out.append(anchor(c["name"], location, props))
    return out


def tag(obj, p, part):
    obj["redes_familia"] = FAMILY
    obj["redes_subtipo"] = SUBTYPE
    obj["redes_parte"] = part
    obj["redes_material"] = p["body_material"]
    obj["redes_largo_barraje_m"] = p["weir_length_m"]
    obj["redes_ancho_ventana_m"] = p["window_width_m"]
    obj["redes_altura_ventana_m"] = p["window_height_m"]
    obj["redes_espesor_m"] = p["wall_thickness_m"]


def build(p):
    g = layout(p)
    mat = material(p["body_material"])
    parts = [
        ("azud", weir_shells(p, g)),
        ("muros", wall_shells(p, g)),
        ("toma", intake_box_shells(p, g)),
    ]
    root = bpy.data.objects.new(object_name(f"L{round(p['weir_length_m'] * 1000)}"), None)
    scene = bpy.context.scene
    scene.collection.objects.link(root)
    for part, shells in parts:
        obj = mesh_object(part, shells, mat)
        tag(obj, p, part)
        obj.parent = root
        scene.collection.objects.link(obj)
    invert = anchor(INVERT_ANCHOR, (0.0, 0.0, g["z"](0.0)), {"redes_parte": "solera"})
    for empty in [invert] + connection_anchors(p, g):
        empty.parent = root
        scene.collection.objects.link(empty)


run(__file__, validate, build)
