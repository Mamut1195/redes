"""Building blocks shared by the treatment-unit recipes `build_plant_*.py` (docs/familias/F17-07).

Every unit uses the same frame: Z up, metres, origin at the plan centre of the unit on the
underside of its floor slab. The interior floor (the declared `cota_fondo`) is at
z = floor_thickness_m and the empty "solera" marks it, so the viewer sits the piece on the invert
of the object (docs/17, section F17-03). Plan sizes (largo, ancho, diametro_interior) are
interior: the walls stand outside them. Length runs along +X, the flow direction of the demo.

Geometry is explicit vertex and face lists with outward faces, one closed shell per solid, so the
.glb is byte-identical between runs (docs/17, section F17-01). Each recipe lists this module in
its contract (`modulos`), which puts it in the cache key.
"""

import math
import sys

import bpy

FLOOR_ANCHOR = "solera"

# Display colour of each named material: the viewer reads baseColorFactor, the names stay the
# data (docs/17, section 5). Distinct colours let a vertical cut tell the layers apart.
MATERIAL_COLOURS = {
    "hormigon": (0.66, 0.63, 0.57),
    "hormigon_armado": (0.62, 0.60, 0.55),
    "acero": (0.42, 0.45, 0.52),
    "acero_inoxidable": (0.74, 0.76, 0.79),
    "polietileno": (0.16, 0.42, 0.74),
    "fibra_vidrio": (0.88, 0.74, 0.22),
    "pvc": (0.82, 0.84, 0.88),
    "arena": (0.89, 0.76, 0.50),
    "antracita": (0.13, 0.13, 0.15),
    "mixto": (0.52, 0.44, 0.33),
    "carbon": (0.08, 0.08, 0.09),
    "grava": (0.50, 0.43, 0.38),
}
NEUTRAL_GREY = (0.6, 0.6, 0.6)

# Drain stubs end this far outside the wall, so a view shows where they leave the unit.
STUB_PAST_WALL_M = 0.5


# --- validation -------------------------------------------------------------------------------


def require_positive(p, names):
    for name in names:
        if not p[name] > 0:
            sys.exit(f"{name} must be positive")


def require_count(p, name, minimum=1):
    """A count parameter, which may arrive as 3.0 from a project number."""
    value = p[name]
    if isinstance(value, bool) or float(value) != int(value) or int(value) < minimum:
        sys.exit(f"{name} must be a whole number >= {minimum}")
    return int(value)


def require_connections(p):
    for c in p["connections"]:
        for key in ("name", "azimuth_deg", "invert_rise_m"):
            if key not in c:
                sys.exit(f"connection without {key}: {c}")


# --- shells: (vertices, faces) with outward faces -------------------------------------------------


def box(x0, x1, y0, y1, z0, z1):
    """Closed axis-aligned box."""
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


def prism_xz(polygon, y0, y1):
    """Closed prism of a convex polygon in elevation (x right, z up, counter-clockwise),
    extruded along Y. Built as a plan prism with Y and Z swapped; the swap mirrors the solid,
    so every face is turned back outward."""
    verts, faces = prism(polygon, y0, y1)
    return [(x, y, z) for x, z, y in verts], [list(reversed(f)) for f in faces]


def prism_yz(polygon, x0, x1):
    """Closed prism of a convex polygon in cross-section (y right, z up, counter-clockwise),
    extruded along X. (y, z, x) is a cyclic turn of (x, y, z), so the faces stay outward."""
    verts, faces = prism(polygon, x0, x1)
    return [(x, y, z) for y, z, x in verts], faces


def loft(outline, segments, centre=(0.0, 0.0)):
    """Closed solid of revolution of a counter-clockwise (radius, z) outline around a vertical
    axis. Radius 0 is a single point on the axis. Hand-built (no bmesh.ops.spin), so the face
    order never changes between runs (docs/17, section F17-01)."""
    cx, cy = centre
    verts, rings = [], []
    for r, z in outline:
        if r == 0.0:
            rings.append([len(verts)] * segments)
            verts.append((cx, cy, z))
            continue
        rings.append(list(range(len(verts), len(verts) + segments)))
        for k in range(segments):
            angle = 2 * math.pi * k / segments
            verts.append((cx + r * math.cos(angle), cy + r * math.sin(angle), z))
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


def cylinder(start, end, radius, segments):
    """Closed solid cylinder between two points (a pipe stub, a stem)."""
    axis = [e - s for s, e in zip(start, end)]
    length = math.sqrt(sum(a * a for a in axis))
    w = [a / length for a in axis]
    helper = (0.0, 0.0, 1.0) if abs(w[2]) < 0.9 else (1.0, 0.0, 0.0)
    u = cross(helper, w)
    u = [a / math.sqrt(sum(b * b for b in u)) for a in u]
    v = cross(w, u)
    verts, faces = loft([(0.0, 0.0), (radius, 0.0), (radius, length), (0.0, length)], segments)
    placed = [
        tuple(start[i] + x * u[i] + y * v[i] + z * w[i] for i in range(3)) for x, y, z in verts
    ]
    return placed, faces


def cross(a, b):
    return (
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    )


def rect(x0, x1, y0, y1):
    """Counter-clockwise corners of a rectangle in plan."""
    return [(x0, y0), (x1, y0), (x1, y1), (x0, y1)]


def hopper_block(outer, cavity_top, cavity_bottom, z_top, z_cavity_bottom, z_bottom):
    """Closed block with an inverted frustum dug into its top face: a sludge or grit hopper.

    `outer`, `cavity_top` and `cavity_bottom` are counter-clockwise rectangles in plan; the cavity
    walls face into the hopper, so a vertical cut shows the sloped walls as solid concrete.
    """
    verts = [(x, y, z_top) for x, y in outer]
    verts += [(x, y, z_top) for x, y in cavity_top]
    verts += [(x, y, z_cavity_bottom) for x, y in cavity_bottom]
    verts += [(x, y, z_bottom) for x, y in outer]
    o, c, b, ob = 0, 4, 8, 12
    faces = []
    for k in range(4):
        n = (k + 1) % 4
        faces.append([o + k, o + n, c + n, c + k])
        faces.append([c + k, c + n, b + n, b + k])
        faces.append([ob + k, ob + n, o + n, o + k])
    faces.append([b, b + 1, b + 2, b + 3])
    faces.append([ob + 3, ob + 2, ob + 1, ob])
    return verts, faces


def slab_around(x0, x1, y0, y1, z0, z1, hole):
    """A slab with a rectangular opening, as up to four boxes that never overlap."""
    hx0, hx1, hy0, hy1 = hole
    shells = []
    for bx0, bx1, by0, by1 in [
        (x0, hx0, y0, y1),
        (hx1, x1, y0, y1),
        (hx0, hx1, y0, hy0),
        (hx0, hx1, hy1, y1),
    ]:
        if bx1 - bx0 > 1e-9 and by1 - by0 > 1e-9:
            shells.append(box(bx0, bx1, by0, by1, z0, z1))
    return shells


def rect_walls(length, width, wall, z0, z1):
    """Four walls around an interior of length x width centred on the origin, without overlaps:
    the long walls run the whole outer length and the end walls fit between them."""
    hx, hy = length / 2, width / 2
    return [
        box(-hx - wall, hx + wall, -hy - wall, -hy, z0, z1),
        box(-hx - wall, hx + wall, hy, hy + wall, z0, z1),
        box(-hx - wall, -hx, -hy, hy, z0, z1),
        box(hx, hx + wall, -hy, hy, z0, z1),
    ]


def cross_walls(positions, thickness, width, z0, z1, gaps=None):
    """Walls across the unit (constant X) at the given centres. With `gaps`, each wall leaves an
    opening of that width at one end, alternating sides, so the flow snakes between them."""
    hy = width / 2
    shells = []
    for i, x in enumerate(positions):
        y0, y1 = -hy, hy
        if gaps is not None:
            if i % 2 == 0:
                y1 = hy - gaps
            else:
                y0 = -hy + gaps
        shells.append(box(x - thickness / 2, x + thickness / 2, y0, y1, z0, z1))
    return shells


def even_centres(start, end, count):
    """Centres of `count` equal bays between start and end."""
    step = (end - start) / count
    return [start + step * (k + 0.5) for k in range(count)]


def divider_centres(start, end, count, thickness):
    """Centres of the count - 1 dividers that split [start, end] into `count` equal bays."""
    bay = (end - start - (count - 1) * thickness) / count
    return [start + (k + 1) * bay + k * thickness + thickness / 2 for k in range(count - 1)]


def bays(start, end, count, thickness):
    """(start, end) of the `count` equal bays left between dividers of that thickness."""
    bay = (end - start - (count - 1) * thickness) / count
    return [(start + k * (bay + thickness), start + k * (bay + thickness) + bay) for k in range(count)]


def channel(x0, x1, y0, y1, z_bottom, z_top, plate):
    """Open U launder along X (walls at y0 and y1 rise to the crest z_top)."""
    return [
        box(x0, x1, y0, y1, z_bottom, z_bottom + plate),
        box(x0, x1, y0, y0 + plate, z_bottom + plate, z_top),
        box(x0, x1, y1 - plate, y1, z_bottom + plate, z_top),
    ]


def channel_across(x0, x1, y0, y1, z_bottom, z_top, plate):
    """Open U launder along Y (walls at x0 and x1 rise to the crest z_top)."""
    return [
        box(x0, x1, y0, y1, z_bottom, z_bottom + plate),
        box(x0, x0 + plate, y0, y1, z_bottom + plate, z_top),
        box(x1 - plate, x1, y0, y1, z_bottom + plate, z_top),
    ]


def merge(shells):
    verts, faces = [], []
    for v, f in shells:
        base = len(verts)
        verts += v
        faces += [[base + i for i in face] for face in f]
    return verts, faces


# --- anchors ------------------------------------------------------------------------------------


def rect_exit(azimuth_deg, half_x, half_y):
    """Where a ray from the centre at that azimuth leaves the outer rectangle."""
    a = math.radians(azimuth_deg)
    dx, dy = math.cos(a), math.sin(a)
    reach = min(half_x / abs(dx) if abs(dx) > 1e-12 else math.inf,
                half_y / abs(dy) if abs(dy) > 1e-12 else math.inf)
    return dx * reach, dy * reach


def circle_exit(azimuth_deg, radius):
    a = math.radians(azimuth_deg)
    return radius * math.cos(a), radius * math.sin(a)


# --- Blender objects ----------------------------------------------------------------------------


class Unit:
    """The parts and anchors of one baked unit, linked under one root empty."""

    def __init__(self, family, subtype, floor_z, tags):
        self.prefix = f"{family.replace('.', '_')}__{subtype}"
        self.family, self.subtype = family, subtype
        self.floor_z = floor_z
        self.tags = tags
        self.materials = {}
        self.parts = []
        self.anchors = []

    def material(self, name):
        if name not in self.materials:
            mat = bpy.data.materials.new(name)
            colour = MATERIAL_COLOURS.get(name, NEUTRAL_GREY)
            mat.diffuse_color = (*colour, 1.0)
            principled = mat.node_tree.nodes.get("Principled BSDF") if mat.node_tree else None
            if principled is not None:
                principled.inputs["Base Color"].default_value = (*colour, 1.0)
            self.materials[name] = mat
        return self.materials[name]

    def part(self, name, shells, material_name):
        """One closed mesh per part, with its own material and extras."""
        if not shells:
            return
        verts, faces = merge(shells)
        mesh = bpy.data.meshes.new(name)
        mesh.from_pydata(verts, [], faces)
        mesh.validate()
        mesh.materials.append(self.material(material_name))
        obj = bpy.data.objects.new(f"{self.prefix}__{name}", mesh)
        obj["redes_familia"] = self.family
        obj["redes_subtipo"] = self.subtype
        obj["redes_parte"] = name
        obj["redes_material"] = material_name
        for key, value in self.tags.items():
            obj[key] = value
        self.parts.append(obj)

    def connections(self, connections, exit_at):
        """One empty per pipe on the outer face, at the pipe invert taken from the project
        (invert_rise_m above the unit floor); a pipe without it has no entry (the CLI warns)."""
        for c in sorted(connections, key=lambda c: c["name"]):
            a = math.radians(c["azimuth_deg"])
            x, y = exit_at(c["azimuth_deg"])
            props = {
                "redes_tramo": c["name"].split("__", 1)[-1],
                "redes_azimut_deg": c["azimuth_deg"],
                "redes_sobre_solera_m": c["invert_rise_m"],
                "redes_eje": [round(math.cos(a), 6), round(math.sin(a), 6), 0.0],
            }
            if c.get("dn_mm"):
                props["redes_dn_mm"] = c["dn_mm"]
            self.anchor(c["name"], (x, y, self.floor_z + c["invert_rise_m"]), props)

    def anchor(self, name, location, props):
        empty = bpy.data.objects.new(name, None)
        empty.location = location
        for key, value in props.items():
            empty[key] = value
        self.anchors.append(empty)

    def link(self, note):
        scene = bpy.context.scene
        root = bpy.data.objects.new(self.prefix, None)
        scene.collection.objects.link(root)
        self.anchor(FLOOR_ANCHOR, (0.0, 0.0, self.floor_z), {"redes_parte": "solera"})
        for obj in self.parts + self.anchors:
            obj.parent = root
            scene.collection.objects.link(obj)
        # A text block reaches the review .blend only; glTF does not export it.
        bpy.data.texts.new("redes_unidad.txt").write(note)
