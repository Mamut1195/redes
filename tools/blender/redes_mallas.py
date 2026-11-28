"""Explicit mesh helpers shared by the fitting, valve and hydrant recipes (F17-05).

Every solid is a list of vertices and outward faces built in a fixed order: no bmesh operator
that merges or reorders, so a bake is byte-identical (D15). A recipe lists this module under
"modulos" in its contract, which puts it in the cache key.

Solids are swept rings. A ring is (centre, axis, side, radius): the circle of that radius
around the axis through the centre, starting on the side vector. Radius 0 is a single point.
Consecutive rings are joined; a list that starts and ends on a point closes like a lathe
outline. Walked counter-clockwise in the (radius, axis) plane, the faces point outward.
"""

import math

import bpy

# Display colour of each named material; the name stays the data (docs/17, section 5). An
# unknown material keeps a neutral grey instead of failing.
MATERIAL_COLOURS = {
    "pvc": (0.84, 0.84, 0.80),
    "pead": (0.12, 0.12, 0.13),
    "hdpe": (0.12, 0.12, 0.13),
    "hierro_ductil": (0.30, 0.33, 0.40),
    "hd": (0.30, 0.33, 0.40),
    "fundicion": (0.24, 0.25, 0.28),
    "hierro": (0.42, 0.45, 0.52),
    "acero": (0.55, 0.57, 0.60),
    "laton": (0.78, 0.62, 0.28),
    "bronce": (0.66, 0.46, 0.22),
    "pintura_roja": (0.72, 0.10, 0.08),
    "hormigon": (0.66, 0.63, 0.57),
}
NEUTRAL_GREY = (0.6, 0.6, 0.6)


def add(a, b):
    return (a[0] + b[0], a[1] + b[1], a[2] + b[2])


def scale(a, k):
    return (a[0] * k, a[1] * k, a[2] * k)


def cross(a, b):
    return (a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0])


def plan_direction(azimuth_deg):
    a = math.radians(azimuth_deg)
    return (math.cos(a), math.sin(a), 0.0)


def rotate(v, axis, angle):
    """Rodrigues rotation of v about a unit axis."""
    c, s = math.cos(angle), math.sin(angle)
    k_dot = axis[0] * v[0] + axis[1] * v[1] + axis[2] * v[2]
    kxv = cross(axis, v)
    return tuple(v[i] * c + kxv[i] * s + axis[i] * k_dot * (1 - c) for i in range(3))


def ring_points(centre, axis, side, radius, segments):
    other = cross(axis, side)
    return [
        add(centre, add(scale(side, radius * math.cos(2 * math.pi * k / segments)),
                        scale(other, radius * math.sin(2 * math.pi * k / segments))))
        for k in range(segments)
    ]


def sweep(rings, segments):
    """Vertices and outward faces of a closed chain of rings (see the module docstring)."""
    verts, indices = [], []
    for centre, axis, side, radius in rings:
        if radius == 0.0:
            indices.append([len(verts)] * segments)
            verts.append(centre)
            continue
        indices.append(list(range(len(verts), len(verts) + segments)))
        verts += ring_points(centre, axis, side, radius, segments)
    faces = []
    for i in range(len(rings)):
        a, b = indices[i], indices[(i + 1) % len(rings)]
        for k in range(segments):
            n = (k + 1) % segments
            quad = [a[k], a[n], b[n], b[k]]
            corners = [v for j, v in enumerate(quad) if v != quad[j - 1]]
            if len(corners) >= 3:
                faces.append(corners)
    return verts, faces


def lathe(origin, axis, side, outline, segments):
    """Solid of revolution: outline of (radius, distance along the axis) from the origin."""
    return sweep([(add(origin, scale(axis, t)), axis, side, r) for r, t in outline], segments)


def cylinder(start, axis, side, length, radius, segments):
    """Closed cylinder from start along a unit axis."""
    return lathe(start, axis, side, [(0.0, 0.0), (radius, 0.0), (radius, length),
                                     (0.0, length)], segments)


def bent_tube(start, axis, bend_normal, radius, legs, segments, steps_per_quarter):
    """Closed tube along straight legs and bends, starting at start heading along axis.

    legs is a list of ("straight", length) and ("bend", angle_rad, bend_radius); a bend turns
    the heading about bend_normal (right hand). The rings keep bend_normal as their side, so a
    planar bend never twists.
    """
    centre, heading = start, axis
    stations = [(centre, heading)]
    for leg in legs:
        if leg[0] == "straight":
            centre = add(centre, scale(heading, leg[1]))
            stations.append((centre, heading))
            continue
        _, angle, bend_radius = leg
        pivot = add(centre, scale(cross(bend_normal, heading), bend_radius))
        arm = add(centre, scale(pivot, -1.0))
        steps = max(2, math.ceil(abs(angle) / (math.pi / 2) * steps_per_quarter))
        for k in range(1, steps + 1):
            t = angle * k / steps
            stations.append((add(pivot, rotate(arm, bend_normal, t)), rotate(heading, bend_normal, t)))
        centre, heading = stations[-1]
    rings = [(stations[0][0], stations[0][1], bend_normal, 0.0)]
    rings += [(c, h, bend_normal, radius) for c, h in stations]
    rings.append((stations[-1][0], stations[-1][1], bend_normal, 0.0))
    return sweep(rings, segments), heading, centre


def box(x0, x1, y0, y1, z0, z1):
    """Closed axis-aligned box as vertices and outward quads."""
    verts = [(x, y, z) for z in (z0, z1) for y in (y0, y1) for x in (x0, x1)]
    faces = [[0, 2, 3, 1], [4, 5, 7, 6], [0, 1, 5, 4], [2, 6, 7, 3], [0, 4, 6, 2], [1, 3, 7, 5]]
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


def turned(shell, azimuth_deg):
    """A shell turned about the vertical axis through the origin."""
    c, s = math.cos(math.radians(azimuth_deg)), math.sin(math.radians(azimuth_deg))
    verts, faces = shell
    return [(x * c - y * s, x * s + y * c, z) for x, y, z in verts], faces


def merge(shells):
    verts, faces = [], []
    for v, f in shells:
        base = len(verts)
        verts += v
        faces += [[base + i for i in face] for face in f]
    return verts, faces


def material(name):
    mat = bpy.data.materials.get(name) or bpy.data.materials.new(name)
    colour = MATERIAL_COLOURS.get(name.lower(), NEUTRAL_GREY)
    mat.diffuse_color = (*colour, 1.0)
    principled = mat.node_tree.nodes.get("Principled BSDF") if mat.node_tree else None
    if principled is not None:
        principled.inputs["Base Color"].default_value = (*colour, 1.0)
    return mat


class Piece:
    """The objects of one baked piece under a root empty, tagged for the manifest."""

    def __init__(self, family, subtype, root_suffix, tags):
        self.prefix = f"{family.replace('.', '_')}__{subtype}"
        self.family, self.subtype, self.tags = family, subtype, tags
        self.scene = bpy.context.scene
        self.root = bpy.data.objects.new(f"{self.prefix}__{root_suffix}", None)
        self.scene.collection.objects.link(self.root)

    def part(self, name, shells, material_name):
        if not shells:
            return
        verts, faces = merge(shells)
        mesh = bpy.data.meshes.new(name)
        mesh.from_pydata(verts, [], faces)
        mesh.validate()
        mesh.materials.append(material(material_name))
        obj = bpy.data.objects.new(f"{self.prefix}__{name}", mesh)
        obj["redes_familia"] = self.family
        obj["redes_subtipo"] = self.subtype
        obj["redes_parte"] = name
        obj["redes_material"] = material_name
        for key, value in self.tags.items():
            obj[key] = value
        self.link(obj)

    def anchor(self, name, location, props):
        empty = bpy.data.objects.new(name, None)
        empty.location = location
        for key, value in props.items():
            empty[key] = value
        self.link(empty)

    def invert_anchor(self):
        """The bore invert under the centre: the viewer sits it on the pipe's invert."""
        self.anchor("solera", (0.0, 0.0, 0.0), {"redes_parte": "solera"})

    def port_anchor(self, port, location, index):
        direction = plan_direction(port["azimuth_deg"])
        props = {
            "redes_puerto": index,
            "redes_azimut_deg": port["azimuth_deg"],
            "redes_eje": [round(direction[0], 6), round(direction[1], 6), 0.0],
        }
        if port.get("dn_mm"):
            props["redes_dn_mm"] = port["dn_mm"]
        self.anchor(port["name"], location, props)

    def link(self, obj):
        obj.parent = self.root
        self.scene.collection.objects.link(obj)


def check_ports(ports, count):
    if len(ports) != count:
        raise SystemExit(f"expected {count} ports, got {len(ports)}")
    for q in ports:
        for key in ("name", "azimuth_deg"):
            if key not in q:
                raise SystemExit(f"port without {key}: {q}")


def check_positive(p, names):
    for name in names:
        if not p[name] > 0:
            raise SystemExit(f"{name} must be positive")
