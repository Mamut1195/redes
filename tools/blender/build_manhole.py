"""Bake a parametric inspection manhole to .glb with Blender running headless.

Usage:
    blender -b --factory-startup --python-exit-code 1 --python tools/blender/build_manhole.py -- params.json

params.json is one parameter set or {"jobs": [...]}; the names are listed in
build_manhole.receta.json (docs/17, sections F17-02 and F17-03).

Frame of the piece: Z up, metres, origin at the centre of the underside of the base slab. The
invert (top of the slab, bottom of the channel) is at z = base_thickness_m and the rim at
z = base_thickness_m + depth_m. The empty "solera" marks the invert, so the viewer can sit the
piece on the invert elevation of the object (docs/17, section F17-03).

Body, cone, frame, rungs and bench are explicit vertex and face lists; the cover is a Geometry
Nodes modifier driven by group inputs, editable in the review .blend.
"""

import math
import os
import sys

import bpy

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from redes_bake import run  # noqa: E402

FAMILY = "saneamiento.pozo"
SUBTYPE = "inspeccion"
INVERT_ANCHOR = "solera"
CONE_TYPES = {"excentrico", "concentrico"}

# Display colour of each named material: the viewer reads baseColorFactor, the names stay the
# data (docs/17, section 5). An unknown material keeps a neutral grey instead of failing.
MATERIAL_COLOURS = {
    "hormigon": (0.66, 0.63, 0.57),
    "hormigon_armado": (0.62, 0.60, 0.55),
    "fundicion": (0.24, 0.25, 0.28),
    "hierro": (0.42, 0.45, 0.52),
    "polipropileno": (0.86, 0.47, 0.13),
    "acero": (0.55, 0.57, 0.60),
}
NEUTRAL_GREY = (0.6, 0.6, 0.6)

POSITIVE = [
    "inner_diameter_m",
    "depth_m",
    "wall_thickness_m",
    "base_thickness_m",
    "cone_height_m",
    "opening_diameter_m",
    "frame_height_m",
    "frame_width_m",
    "frame_seat_m",
    "cover_thickness_m",
    "cover_clearance_m",
    "step_spacing_m",
    "step_width_m",
    "step_projection_m",
    "step_bar_m",
]


def validate(p):
    for name in POSITIVE:
        if not p[name] > 0:
            sys.exit(f"{name} must be positive")
    for name in ["base_overhang_m", "min_barrel_height_m", "step_first_height_m",
                 "step_top_clearance_m", "step_embed_m"]:
        if p[name] < 0:
            sys.exit(f"{name} cannot be negative")
    if p["cone_type"] not in CONE_TYPES:
        sys.exit(f"cone_type must be one of {sorted(CONE_TYPES)}")
    if p["opening_diameter_m"] >= p["inner_diameter_m"]:
        sys.exit("opening_diameter_m must be smaller than inner_diameter_m")
    if p["frame_seat_m"] >= p["opening_diameter_m"] / 2:
        sys.exit("frame_seat_m must be smaller than the opening radius")
    if p["cover_thickness_m"] >= p["frame_height_m"]:
        sys.exit("cover_thickness_m must be smaller than frame_height_m")
    if p["depth_m"] <= p["frame_height_m"] + p["wall_thickness_m"] + 0.1:
        sys.exit("depth_m is too small for the frame and a top slab")
    for c in p["connections"]:
        for key in ("name", "azimuth_deg", "invert_rise_m"):
            if key not in c:
                sys.exit(f"connection without {key}: {c}")


def layout(p):
    """Elevations and radii every part shares, derived from the parameters only."""
    ri = p["inner_diameter_m"] / 2
    rt = p["opening_diameter_m"] / 2
    t = p["wall_thickness_m"]
    invert = p["base_thickness_m"]
    rim = invert + p["depth_m"]
    cone_top = rim - p["frame_height_m"]
    room = cone_top - invert - p["min_barrel_height_m"]
    cone_height = min(p["cone_height_m"], room)
    # A shallow manhole has no room for a cone: a flat top slab as thick as the wall closes it.
    flat_top = cone_height < t
    eccentric = p["cone_type"] == "excentrico"
    if flat_top:
        cone_height = t
        # Kept off the barrel wall so the slab underside never pinches to a zero-width edge.
        offset = max(0.0, ri - rt - t) if eccentric else 0.0
    else:
        offset = ri - rt if eccentric else 0.0
    return {
        "ri": ri,
        "rt": rt,
        "t": t,
        "invert": invert,
        "rim": rim,
        "cone_top": cone_top,
        "cone_start": cone_top - cone_height,
        "flat_top": flat_top,
        # The opening sits towards -X, where the wall stays vertical and the rungs go.
        "opening_x": -offset,
    }


def body_rings(p, g):
    """Closed (radius, z, centre x) outline of base slab, barrel and cone (or flat top slab)."""
    ri, rt, t = g["ri"], g["rt"], g["t"]
    rs = ri + t + p["base_overhang_m"]
    ox = g["opening_x"]
    outline = [(0.0, 0.0, 0.0), (rs, 0.0, 0.0), (rs, g["invert"], 0.0)]
    if rs > ri + t:
        outline.append((ri + t, g["invert"], 0.0))
    if g["flat_top"]:
        outline += [
            (ri + t, g["cone_top"], 0.0),
            (rt, g["cone_top"], ox),
            (rt, g["cone_start"], ox),
            (ri, g["cone_start"], 0.0),
        ]
    else:
        outline += [
            (ri + t, g["cone_start"], 0.0),
            (rt + t, g["cone_top"], ox),
            (rt, g["cone_top"], ox),
            (ri, g["cone_start"], 0.0),
        ]
    return outline + [(ri, g["invert"], 0.0), (0.0, g["invert"], 0.0)]


def frame_rings(p, g):
    """L-shaped ring of the frame: outer collar plus the seat the cover rests on."""
    rt, ox = g["rt"], g["opening_x"]
    seat = rt - p["frame_seat_m"]
    outer = rt + p["frame_width_m"]
    seat_top = g["rim"] - p["cover_thickness_m"]
    return [
        (seat, g["cone_top"], ox),
        (outer, g["cone_top"], ox),
        (outer, g["rim"], ox),
        (rt, g["rim"], ox),
        (rt, seat_top, ox),
        (seat, seat_top, ox),
    ]


def loft(outline, segments):
    """Vertices and outward faces of a closed outline of circles stacked along Z.

    Each ring is (radius, z, centre x); radius 0 is a single point on the axis. Built by hand
    instead of with bmesh.ops.spin + remove_doubles: those emit faces in an order that changes
    between runs, so the .glb was not byte-identical (docs/17, section F17-01).
    """
    verts, rings = [], []
    for r, z, cx in outline:
        if r == 0.0:
            rings.append([len(verts)] * segments)
            verts.append((cx, 0.0, z))
            continue
        rings.append(list(range(len(verts), len(verts) + segments)))
        for k in range(segments):
            angle = 2 * math.pi * k / segments
            verts.append((cx + r * math.cos(angle), r * math.sin(angle), z))
    faces = []
    for i in range(len(outline)):
        a, b = rings[i], rings[(i + 1) % len(outline)]
        for k in range(segments):
            n = (k + 1) % segments
            quad = [a[k], a[n], b[n], b[k]]
            corners = [v for j, v in enumerate(quad) if v != quad[j - 1]]
            if len(corners) >= 3:
                faces.append(corners)
    return verts, faces


def box(x0, x1, y0, y1, z0, z1):
    """Closed axis-aligned box as vertices and outward quads."""
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


def prism(polygon, z0, z1):
    """Closed prism of a convex counter-clockwise polygon in plan."""
    n = len(polygon)
    verts = [(x, y, z0) for x, y in polygon] + [(x, y, z1) for x, y in polygon]
    faces = [list(reversed(range(n))), list(range(n, 2 * n))]
    for i in range(n):
        j = (i + 1) % n
        faces.append([i, j, n + j, n + i])
    return verts, faces


def merge(shells):
    verts, faces = [], []
    for v, f in shells:
        base = len(verts)
        verts += v
        faces += [[base + i for i in face] for face in f]
    return verts, faces


def inner_wall_x(g, z):
    """X of the inner wall on the -X side at height z: where the rungs are anchored."""
    if g["flat_top"] or z <= g["cone_start"]:
        return -g["ri"]
    s = (z - g["cone_start"]) / (g["cone_top"] - g["cone_start"])
    radius = g["ri"] + (g["rt"] - g["ri"]) * s
    return g["opening_x"] * s - radius


def rung_heights(p, g):
    z = g["invert"] + p["step_first_height_m"]
    top = g["cone_top"] - p["step_top_clearance_m"]
    heights = []
    while z <= top + 1e-9:
        heights.append(round(z, 6))
        z += p["step_spacing_m"]
    return heights


def rung_shells(p, g):
    """U-shaped rungs: a tread parallel to the wall and two legs set into it."""
    s = p["step_bar_m"] / 2
    w = p["step_width_m"] / 2
    shells = []
    for z in rung_heights(p, g):
        wall = inner_wall_x(g, z)
        tread = wall + p["step_projection_m"]
        shells.append(box(tread - s, tread + s, -w, w, z - s, z + s))
        for y in (-w, w):
            shells.append(box(wall - p["step_embed_m"], tread - s, y - s, y + s, z - s, z + s))
    return shells


def outlet(p):
    """First outlet with a known DN: it sets the direction and width of the channel."""
    outlets = [c for c in p["connections"] if c["name"].startswith("salida") and c.get("dn_mm")]
    return outlets[0] if outlets else None


def bench_shells(p, g):
    """Bench on the slab, split by a straight channel along the outlet.

    The channel is as wide as the outlet DN and the bench rises half of it (a half-pipe
    height); without an outlet DN there is no channel to draw, so there is no bench.
    """
    out = outlet(p)
    if out is None:
        return []
    width = out["dn_mm"] / 1000.0
    ri = g["ri"]
    if width >= 2 * ri:
        return []
    half = width / 2
    a = math.radians(out["azimuth_deg"])
    along, across = (math.cos(a), math.sin(a)), (-math.sin(a), math.cos(a))
    start = math.asin(half / ri)
    count = max(4, p["segments"] // 2)
    shells = []
    for side in (1.0, -1.0):
        polygon = []
        for k in range(count + 1):
            theta = start + (math.pi - 2 * start) * k / count
            u, v = ri * math.cos(theta), side * ri * math.sin(theta)
            polygon.append((u * along[0] + v * across[0], u * along[1] + v * across[1]))
        if side < 0:
            polygon.reverse()
        shells.append(prism(polygon, g["invert"], g["invert"] + half))
    return shells


def material(name):
    mat = bpy.data.materials.new(name)
    colour = MATERIAL_COLOURS.get(name, NEUTRAL_GREY)
    mat.diffuse_color = (*colour, 1.0)
    principled = mat.node_tree.nodes.get("Principled BSDF") if mat.node_tree else None
    if principled is not None:
        principled.inputs["Base Color"].default_value = (*colour, 1.0)
    return mat


def mesh_object(name, shells, mat):
    verts, faces = merge(shells)
    mesh = bpy.data.meshes.new(name)
    mesh.from_pydata(verts, [], faces)
    mesh.validate()
    mesh.materials.append(mat)
    return bpy.data.objects.new(f"{FAMILY.replace('.', '_')}__{SUBTYPE}__{name}", mesh)


def cover_node_tree(mat):
    tree = bpy.data.node_groups.new("manhole_cover", "GeometryNodeTree")
    for name, kind in [
        ("Radius", "NodeSocketFloat"),
        ("Thickness", "NodeSocketFloat"),
        ("Elevation", "NodeSocketFloat"),
        ("Offset X", "NodeSocketFloat"),
        ("Segments", "NodeSocketInt"),
    ]:
        tree.interface.new_socket(name, in_out="INPUT", socket_type=kind)
    tree.interface.new_socket("Geometry", in_out="OUTPUT", socket_type="NodeSocketGeometry")

    nodes, links = tree.nodes, tree.links
    group_in = nodes.new("NodeGroupInput")
    group_out = nodes.new("NodeGroupOutput")
    cylinder = nodes.new("GeometryNodeMeshCylinder")
    half = nodes.new("ShaderNodeMath")
    half.operation = "MULTIPLY"
    half.inputs[1].default_value = 0.5
    lift = nodes.new("ShaderNodeMath")
    lift.operation = "SUBTRACT"
    offset = nodes.new("ShaderNodeCombineXYZ")
    move = nodes.new("GeometryNodeTransform")
    triangulate = nodes.new("GeometryNodeTriangulate")

    links.new(group_in.outputs["Radius"], cylinder.inputs["Radius"])
    links.new(group_in.outputs["Thickness"], cylinder.inputs["Depth"])
    links.new(group_in.outputs["Segments"], cylinder.inputs["Vertices"])
    # Mesh Cylinder is centred on its origin; "Elevation" is the top face, flush with the rim.
    links.new(group_in.outputs["Thickness"], half.inputs[0])
    links.new(group_in.outputs["Elevation"], lift.inputs[0])
    links.new(half.outputs[0], lift.inputs[1])
    links.new(lift.outputs[0], offset.inputs["Z"])
    links.new(group_in.outputs["Offset X"], offset.inputs["X"])
    links.new(cylinder.outputs["Mesh"], move.inputs["Geometry"])
    links.new(offset.outputs[0], move.inputs["Translation"])
    links.new(move.outputs["Geometry"], triangulate.inputs["Mesh"])
    # Generated faces carry no material unless the tree sets one.
    set_material = nodes.new("GeometryNodeSetMaterial")
    set_material.inputs["Material"].default_value = mat
    links.new(triangulate.outputs["Mesh"], set_material.inputs["Geometry"])
    links.new(set_material.outputs["Geometry"], group_out.inputs["Geometry"])
    return tree


def cover_object(p, g, mat):
    mesh = bpy.data.meshes.new("tapa")
    obj = bpy.data.objects.new(f"{FAMILY.replace('.', '_')}__{SUBTYPE}__tapa", mesh)
    modifier = obj.modifiers.new("cover", "NODES")
    tree = cover_node_tree(mat)
    modifier.node_group = tree
    inputs = {
        "Radius": g["rt"] - p["cover_clearance_m"],
        "Thickness": p["cover_thickness_m"],
        "Elevation": g["rim"],
        "Offset X": g["opening_x"],
        "Segments": p["segments"],
    }
    for item in tree.interface.items_tree:
        if item.item_type == "SOCKET" and item.in_out == "INPUT":
            modifier[item.identifier] = inputs[item.name]
    return obj


def anchor(name, location, props):
    empty = bpy.data.objects.new(name, None)
    empty.location = location
    for key, value in props.items():
        empty[key] = value
    return empty


def connection_anchors(p, g):
    """One empty per pipe that reaches the manhole, on the outer face of the barrel.

    Its height is the pipe invert at the manhole, taken from the project (invert_rise_m above
    the manhole invert); a pipe without that datum has no entry here (the CLI reports it).
    """
    r = g["ri"] + g["t"]
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
        location = (r * math.cos(a), r * math.sin(a), g["invert"] + c["invert_rise_m"])
        out.append(anchor(c["name"], location, props))
    return out


def tag(obj, p, part, mat_name):
    obj["redes_familia"] = FAMILY
    obj["redes_subtipo"] = SUBTYPE
    obj["redes_parte"] = part
    obj["redes_material"] = mat_name
    obj["redes_diametro_interior_m"] = p["inner_diameter_m"]
    obj["redes_altura_m"] = p["depth_m"]
    obj["redes_espesor_m"] = p["wall_thickness_m"]


def build(p):
    g = layout(p)
    body_mat = material(p["body_material"])
    cover_mat = material(p["cover_material"])
    step_mat = material(p["step_material"])
    seg = p["segments"]
    parts = [
        ("cuerpo", mesh_object("cuerpo", [loft(body_rings(p, g), seg)], body_mat),
         p["body_material"]),
        ("marco", mesh_object("marco", [loft(frame_rings(p, g), seg)], cover_mat),
         p["cover_material"]),
        ("tapa", cover_object(p, g, cover_mat), p["cover_material"]),
    ]
    rungs = rung_shells(p, g)
    if rungs:
        parts.append(("escalones", mesh_object("escalones", rungs, step_mat), p["step_material"]))
    bench = bench_shells(p, g)
    if bench:
        parts.append(("banqueta", mesh_object("banqueta", bench, body_mat), p["body_material"]))

    root = bpy.data.objects.new(
        f"{FAMILY.replace('.', '_')}__{SUBTYPE}__DI{round(p['inner_diameter_m'] * 1000)}", None
    )
    scene = bpy.context.scene
    scene.collection.objects.link(root)
    for part, obj, mat_name in parts:
        tag(obj, p, part, mat_name)
        obj.parent = root
        scene.collection.objects.link(obj)
    invert = anchor(INVERT_ANCHOR, (0.0, 0.0, g["invert"]), {"redes_parte": "solera"})
    for empty in [invert] + connection_anchors(p, g):
        empty.parent = root
        scene.collection.objects.link(empty)


run(__file__, validate, build)
