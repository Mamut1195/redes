"""Bake one commercial pipe segment (bell and spigot, with its joint) to .glb, Blender headless.

Usage:
    blender -b --factory-startup --python-exit-code 1 --python tools/blender/build_pipe.py -- params.json

params.json is one parameter set or {"jobs": [...]}; the names are listed in
build_pipe.receta.json (docs/familias/F17-04-tuberia.md).

A link is never baked whole: networks reach 20 000 links of any length. One segment is baked per
DN, wall, material and system, and the viewer lays it along the link (full segments plus a cut
one at the end, made by stretching the barrel, which is prismatic along the axis).

Frame of the piece: X along the pipe axis, Z up, metres. The origin is the spigot end on the
axis; the next segment's spigot end sits at x = laying length, inside this segment's socket.
The empties "entrada" (x = 0) and "salida" (x = laying length) mark both ends of the axis.

Parts (each a closed solid of revolution with explicit vertices and faces):
- "cuerpo": barrel from the spigot end (with its chamfer) to where the bell starts. The viewer
  stretches only this part to draw the cut segment at the end of a link.
- "campana": bell (taper and socket, with the gasket groove) or, for butt-fused pipes, the weld bead.
- "junta": rubber ring in the socket groove (only in bell and spigot joints).
"""

import math
import os
import sys
import unicodedata

import bpy

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from redes_bake import run  # noqa: E402

PIECE = "tuberia"
START_ANCHOR = "entrada"
END_ANCHOR = "salida"
SYSTEMS = {"agua", "saneamiento"}
JOINT_TYPES = {"campana_espiga", "termofusion"}
OTHER_MATERIAL = "otro"

# Display colours (forma): the viewer reads baseColorFactor. Plastic pipes keep the colour the
# viewer gives each system (blue water, orange sewer), so the network still reads at a glance.
COLOURS = {
    ("agua", "pvc"): (0.34, 0.55, 0.82),
    ("saneamiento", "pvc"): (0.86, 0.47, 0.13),
    ("agua", "pead"): (0.16, 0.20, 0.30),
    ("saneamiento", "pead"): (0.14, 0.14, 0.15),
    ("agua", OTHER_MATERIAL): (0.34, 0.55, 0.82),
    ("saneamiento", OTHER_MATERIAL): (0.78, 0.62, 0.44),
}
MATERIAL_COLOURS = {
    "hormigon": (0.66, 0.63, 0.57),
    "hormigon_armado": (0.62, 0.60, 0.55),
    "gres": (0.62, 0.36, 0.22),
    "hierro_ductil": (0.24, 0.25, 0.28),
    "fundicion": (0.24, 0.25, 0.28),
    "acero": (0.55, 0.57, 0.60),
    "asbesto_cemento": (0.72, 0.72, 0.70),
    "caucho": (0.10, 0.10, 0.11),
}
NEUTRAL_GREY = (0.6, 0.6, 0.6)

POSITIVE = [
    "inner_diameter_m",
    "wall_thickness_m",
    "socket_depth_ratio",
    "socket_depth_min_m",
    "socket_clearance_m",
    "bell_wall_ratio",
    "bell_taper_ratio",
    "spigot_chamfer_ratio",
    "bead_width_ratio",
    "bead_height_ratio",
]


def material_key(name):
    """Lower case, no accents, words joined by "_": "Hierro dúctil" -> "hierro_ductil"."""
    plain = unicodedata.normalize("NFKD", name).encode("ascii", "ignore").decode()
    return "_".join(plain.lower().replace("-", " ").split())


def by_material(table, material):
    key = material_key(material)
    return table[key] if key in table else table[OTHER_MATERIAL]


def validate(p):
    for name in POSITIVE:
        if not p[name] > 0:
            sys.exit(f"{name} must be positive")
    if not isinstance(p["dn_mm"], int) or p["dn_mm"] <= 0:
        sys.exit("dn_mm must be a positive integer")
    if p["system"] not in SYSTEMS:
        sys.exit(f"system must be one of {sorted(SYSTEMS)}")
    for table in ("segment_length_m", "joint_type"):
        if OTHER_MATERIAL not in p[table]:
            sys.exit(f"{table} needs an entry for {OTHER_MATERIAL!r}")
    if by_material(p["joint_type"], p["pipe_material"]) not in JOINT_TYPES:
        sys.exit(f"joint_type must be one of {sorted(JOINT_TYPES)}")
    start, end = p["gasket_span"]
    if not 0 < start < end < 1:
        sys.exit("gasket_span must be two fractions of the socket depth, 0 < start < end < 1")
    if p["segments"] < 8:
        sys.exit("segments must be at least 8")
    g = layout(p)
    if g["barrel_end"] <= g["chamfer"]:
        sys.exit("segment_length_m is too short for the bell of this pipe")


def layout(p):
    """Radii and stations along X every part shares, derived from the parameters only."""
    ri = p["inner_diameter_m"] / 2
    t = p["wall_thickness_m"]
    ro = ri + t
    length = by_material(p["segment_length_m"], p["pipe_material"])
    joint = by_material(p["joint_type"], p["pipe_material"])
    socket = max(p["socket_depth_min_m"], p["socket_depth_ratio"] * 2 * ro)
    rs = ro + p["socket_clearance_m"]
    rb = rs + p["bell_wall_ratio"] * t
    taper = p["bell_taper_ratio"] * (rb - ro)
    bead = p["bead_width_ratio"] * t
    fused = joint == "termofusion"
    return {
        "ri": ri,
        "t": t,
        "ro": ro,
        "length": length,
        "joint": joint,
        "fused": fused,
        "socket": socket,
        "rs": rs,
        "rb": rb,
        # The groove that holds the ring is cut halfway into the bell wall.
        "rg": rs + 0.5 * p["bell_wall_ratio"] * t,
        "taper": taper,
        "bead": bead,
        "bead_height": p["bead_height_ratio"] * t,
        # A butt-fused pipe is plain to its end; a socketed one stops where the taper starts.
        "barrel_end": length - bead if fused else length - taper,
        # Fused ends are planed square; a spigot is chamfered to enter the ring.
        "chamfer": 0.0 if fused else p["spigot_chamfer_ratio"] * t,
    }


def barrel_outline(g):
    """(radius, x) outline of the barrel, counter-clockwise: outward faces once revolved."""
    if g["chamfer"] == 0.0:
        return [(g["ri"], 0.0), (g["ro"], 0.0), (g["ro"], g["barrel_end"]),
                (g["ri"], g["barrel_end"])]
    return [
        (g["ri"], 0.0),
        (g["ro"] - 0.5 * g["t"], 0.0),
        (g["ro"], g["chamfer"]),
        (g["ro"], g["barrel_end"]),
        (g["ri"], g["barrel_end"]),
    ]


def bell_outline(p, g):
    """Taper, socket and gasket groove; the socket floor is at the laying length."""
    x0, floor = g["barrel_end"], g["length"]
    mouth = floor + g["socket"]
    start, end = p["gasket_span"]
    groove0, groove1 = floor + start * g["socket"], floor + end * g["socket"]
    return [
        (g["ri"], x0),
        (g["ro"], x0),
        (g["rb"], floor),
        (g["rb"], mouth),
        (g["rs"], mouth),
        (g["rs"], groove1),
        (g["rg"], groove1),
        (g["rg"], groove0),
        (g["rs"], groove0),
        (g["rs"], floor),
        (g["ri"], floor),
    ]


def bead_outline(g):
    """Weld bead of a butt-fused joint, half on each segment around the laying length."""
    x0, x1 = g["barrel_end"], g["length"] + g["bead"]
    return [(g["ri"], x0), (g["ro"] + g["bead_height"], x0),
            (g["ro"] + g["bead_height"], x1), (g["ri"], x1)]


def gasket_outline(p, g):
    start, end = p["gasket_span"]
    floor = g["length"]
    x0, x1 = floor + start * g["socket"], floor + end * g["socket"]
    return [(g["ro"], x0), (g["rg"], x0), (g["rg"], x1), (g["ro"], x1)]


def revolve(outline, segments):
    """Vertices and faces of a closed (radius, x) outline revolved about the X axis.

    Built by hand instead of with bmesh.ops.spin: its face order changes between runs and the
    .glb would not be byte-identical (docs/17, section F17-01).
    """
    verts, rings = [], []
    for r, x in outline:
        rings.append(list(range(len(verts), len(verts) + segments)))
        for k in range(segments):
            angle = 2 * math.pi * k / segments
            verts.append((x, r * math.cos(angle), r * math.sin(angle)))
    faces = []
    for i in range(len(outline)):
        a, b = rings[i], rings[(i + 1) % len(outline)]
        for k in range(segments):
            n = (k + 1) % segments
            faces.append([a[k], a[n], b[n], b[k]])
    return verts, faces


def colour(system, material):
    key = material_key(material)
    if (system, key) in COLOURS:
        return COLOURS[(system, key)]
    if key in MATERIAL_COLOURS:
        return MATERIAL_COLOURS[key]
    return COLOURS[(system, OTHER_MATERIAL)]


def material(name, rgb):
    mat = bpy.data.materials.new(name)
    mat.diffuse_color = (*rgb, 1.0)
    principled = mat.node_tree.nodes.get("Principled BSDF") if mat.node_tree else None
    if principled is not None:
        principled.inputs["Base Color"].default_value = (*rgb, 1.0)
    return mat


def mesh_object(part, outline, segments, mat):
    verts, faces = revolve(outline, segments)
    mesh = bpy.data.meshes.new(part)
    mesh.from_pydata(verts, [], faces)
    mesh.validate()
    mesh.materials.append(mat)
    return bpy.data.objects.new(f"{PIECE}__{part}", mesh)


def anchor(name, x, props):
    empty = bpy.data.objects.new(name, None)
    empty.location = (x, 0.0, 0.0)
    for key, value in props.items():
        empty[key] = value
    return empty


def tag(obj, p, g, part, mat_name):
    obj["redes_pieza"] = PIECE
    obj["redes_parte"] = part
    obj["redes_material"] = mat_name
    obj["redes_dn_mm"] = p["dn_mm"]
    obj["redes_diametro_interior_m"] = p["inner_diameter_m"]
    obj["redes_espesor_m"] = p["wall_thickness_m"]
    obj["redes_largo_tendido_m"] = g["length"]
    obj["redes_junta"] = g["joint"]


def build(p):
    g = layout(p)
    seg = p["segments"]
    pipe_mat = material(material_key(p["pipe_material"]),
                        colour(p["system"], p["pipe_material"]))
    parts = [("cuerpo", barrel_outline(g), pipe_mat, p["pipe_material"])]
    if g["fused"]:
        parts.append(("campana", bead_outline(g), pipe_mat, p["pipe_material"]))
    else:
        parts.append(("campana", bell_outline(p, g), pipe_mat, p["pipe_material"]))
        ring = material(p["gasket_material"],
                        MATERIAL_COLOURS.get(p["gasket_material"], NEUTRAL_GREY))
        parts.append(("junta", gasket_outline(p, g), ring, p["gasket_material"]))

    root = bpy.data.objects.new(f"{PIECE}__DN{p['dn_mm']}", None)
    scene = bpy.context.scene
    scene.collection.objects.link(root)
    for part, outline, mat, mat_name in parts:
        obj = mesh_object(part, outline, seg, mat)
        tag(obj, p, g, part, mat_name)
        obj.parent = root
        scene.collection.objects.link(obj)
    for name, x in [(START_ANCHOR, 0.0), (END_ANCHOR, g["length"])]:
        empty = anchor(name, x, {"redes_parte": name, "redes_largo_tendido_m": g["length"]})
        empty.parent = root
        scene.collection.objects.link(empty)


run(__file__, validate, build)
