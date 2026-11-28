"""Bake a parametric inspection manhole to .glb with Blender running headless.

Usage:
    blender -b --factory-startup --python tools/blender/build_manhole.py -- params.json

Prototype for F17-01 (docs/17, section F17-01). The body and cone are one
revolved solid built from explicit vertex and face lists; the cover is a
Geometry Nodes modifier driven by group inputs, so the same bake exercises both
ways of building a parametric family.
"""

import json
import math
import sys

import bpy

DEFAULTS = {
    "inner_diameter_m": 1.2,
    "depth_m": 2.5,
    "wall_thickness_m": 0.15,
    "cone_height_m": 0.6,
    "opening_diameter_m": 0.6,
    "cover_thickness_m": 0.05,
    "body_material": "hormigon",
    "cover_material": "hierro",
    "segments": 48,
}


def read_params(argv):
    if "--" not in argv:
        sys.exit("expected: -- <params.json>")
    with open(argv[argv.index("--") + 1], encoding="utf-8") as f:
        given = json.load(f)
    unknown = set(given) - set(DEFAULTS) - {"output"}
    if unknown:
        sys.exit(f"unknown params: {sorted(unknown)}")
    if "output" not in given:
        sys.exit("missing param: output")
    return {**DEFAULTS, **given}


def validate(p):
    ri = p["inner_diameter_m"] / 2
    rt = p["opening_diameter_m"] / 2
    t = p["wall_thickness_m"]
    if min(ri, rt, t, p["cone_height_m"], p["cover_thickness_m"]) <= 0:
        sys.exit("all dimensions must be positive")
    if rt > ri:
        sys.exit("opening_diameter_m cannot exceed inner_diameter_m")
    if p["depth_m"] <= p["cone_height_m"] + t:
        sys.exit("depth_m must exceed cone_height_m + wall_thickness_m")


def body_profile(p):
    """Closed (r, z) outline of base slab, barrel and concentric cone."""
    ri = p["inner_diameter_m"] / 2
    rt = p["opening_diameter_m"] / 2
    t = p["wall_thickness_m"]
    h = p["depth_m"]
    cone_start = h - p["cone_height_m"]
    return [
        (0.0, 0.0),
        (ri + t, 0.0),
        (ri + t, cone_start),
        (rt + t, h),
        (rt, h),
        (ri, cone_start),
        (ri, t),
        (0.0, t),
    ]


def revolve(profile, segments):
    """Vertices and outward faces of a closed (r, z) profile spun around Z.

    Built by hand instead of with bmesh.ops.spin + remove_doubles: those emit
    faces in an order that changes between runs, so the .glb was not
    byte-identical (docs/17, section F17-01).
    """
    verts, rings = [], []
    for r, z in profile:
        if r == 0.0:
            rings.append([len(verts)] * segments)
            verts.append((0.0, 0.0, z))
            continue
        rings.append(list(range(len(verts), len(verts) + segments)))
        for k in range(segments):
            angle = 2 * math.pi * k / segments
            verts.append((r * math.cos(angle), r * math.sin(angle), z))
    faces = []
    for i in range(len(profile)):
        a, b = rings[i], rings[(i + 1) % len(profile)]
        for k in range(segments):
            n = (k + 1) % segments
            quad = [a[k], a[n], b[n], b[k]]
            corners = [v for j, v in enumerate(quad) if v != quad[j - 1]]
            if len(corners) >= 3:
                faces.append(corners)
    return verts, faces


def build_body(p, material):
    verts, faces = revolve(body_profile(p), p["segments"])
    mesh = bpy.data.meshes.new("manhole_body")
    mesh.from_pydata(verts, [], faces)
    mesh.validate()
    mesh.materials.append(material)
    return bpy.data.objects.new("saneamiento.pozo__inspeccion__cuerpo", mesh)


def cover_node_tree(material):
    tree = bpy.data.node_groups.new("manhole_cover", "GeometryNodeTree")
    for name, kind in [
        ("Radius", "NodeSocketFloat"),
        ("Thickness", "NodeSocketFloat"),
        ("Elevation", "NodeSocketFloat"),
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
    lift.operation = "ADD"
    offset = nodes.new("ShaderNodeCombineXYZ")
    move = nodes.new("GeometryNodeTransform")
    triangulate = nodes.new("GeometryNodeTriangulate")

    links.new(group_in.outputs["Radius"], cylinder.inputs["Radius"])
    links.new(group_in.outputs["Thickness"], cylinder.inputs["Depth"])
    links.new(group_in.outputs["Segments"], cylinder.inputs["Vertices"])
    # Mesh Cylinder is centred on its origin; lift it so it rests on the cone.
    links.new(group_in.outputs["Thickness"], half.inputs[0])
    links.new(group_in.outputs["Elevation"], lift.inputs[0])
    links.new(half.outputs[0], lift.inputs[1])
    links.new(lift.outputs[0], offset.inputs["Z"])
    links.new(cylinder.outputs["Mesh"], move.inputs["Geometry"])
    links.new(offset.outputs[0], move.inputs["Translation"])
    links.new(move.outputs["Geometry"], triangulate.inputs["Mesh"])
    # Generated faces carry no material unless the tree sets one.
    set_material = nodes.new("GeometryNodeSetMaterial")
    set_material.inputs["Material"].default_value = material
    links.new(triangulate.outputs["Mesh"], set_material.inputs["Geometry"])
    links.new(set_material.outputs["Geometry"], group_out.inputs["Geometry"])
    return tree


def build_cover(p, material):
    mesh = bpy.data.meshes.new("manhole_cover")
    obj = bpy.data.objects.new("saneamiento.pozo__inspeccion__tapa", mesh)
    modifier = obj.modifiers.new("cover", "NODES")
    tree = cover_node_tree(material)
    modifier.node_group = tree
    inputs = {
        "Radius": p["opening_diameter_m"] / 2 + p["wall_thickness_m"],
        "Thickness": p["cover_thickness_m"],
        "Elevation": p["depth_m"],
        "Segments": p["segments"],
    }
    for item in tree.interface.items_tree:
        if item.item_type == "SOCKET" and item.in_out == "INPUT":
            modifier[item.identifier] = inputs[item.name]
    return obj


def tag(obj, p, part):
    obj["redes_familia"] = "saneamiento.pozo"
    obj["redes_subtipo"] = "inspeccion"
    obj["redes_parte"] = part
    obj["redes_diametro_interior_m"] = p["inner_diameter_m"]
    obj["redes_altura_m"] = p["depth_m"]
    obj["redes_espesor_m"] = p["wall_thickness_m"]
    obj["redes_material"] = p["body_material"] if part == "cuerpo" else p["cover_material"]


def main():
    p = read_params(sys.argv)
    validate(p)
    bpy.ops.wm.read_factory_settings(use_empty=True)
    body_material = bpy.data.materials.new(p["body_material"])
    cover_material = bpy.data.materials.new(p["cover_material"])
    root = bpy.data.objects.new(
        f"saneamiento.pozo__inspeccion__DI{round(p['inner_diameter_m'] * 1000)}", None
    )
    scene = bpy.context.scene
    scene.collection.objects.link(root)
    for obj, part in [(build_body(p, body_material), "cuerpo"), (build_cover(p, cover_material), "tapa")]:
        tag(obj, p, part)
        obj.parent = root
        scene.collection.objects.link(obj)
    bpy.ops.export_scene.gltf(
        filepath=p["output"],
        export_format="GLB",
        export_apply=True,
        export_extras=True,
        export_yup=True,
        export_materials="EXPORT",
    )


main()
