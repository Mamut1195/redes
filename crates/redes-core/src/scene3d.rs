//! GPU scene for the depth pass: indexed prototypes, instances, three levels of detail and
//! frustum culling (F14-05).
//!
//! The old pass uploaded one flat triangle list, so each manhole copied its whole mesh (~570
//! triangles) and 20 000 manholes asked for 1.2 GB, over the 512 MB budget, and got clipped. Now:
//!
//! - **Shared prototypes.** A piece mesh is split at its part breaks (base, shaft, rung…). Each
//!   part is moved to a canonical form (lowest corner at the origin and, if it has height, unit
//!   height) and looked up among the prototypes already registered. A 2 m shaft and a 3 m shaft
//!   are one prototype with a different Z scale. Parts that do not match exactly (after a 0.1 mm
//!   quantisation) become new prototypes: nothing is approximated beyond that.
//! - **Three levels of detail per object:** full (2), simple (1) and box (0, the bounding box in
//!   the average colour). [`plan`] picks one by on-screen size; only the instance list changes.
//! - **Culling.** Every object, and every cell of a 16 × 16 grid over the static geometry (pipes,
//!   streets, blocks, terrain), has a bounding box; what falls outside the view is not drawn.
//! - **Budget with a warning.** If the scene does not fit the GPU budget, what fits is drawn and
//!   [`Scene::warning`] says so; the app shows it in the status bar.
//!
//! Coordinates are local: the origin is subtracted in `f64` before going down to `f32`
//! (docs/15). Elevations are real; vertical exaggeration lives in the camera matrix.

use std::collections::HashMap;
use std::ops::Range;
use std::sync::Arc;

use crate::gltf::{Triangulo, Vertice};
use crate::mallas::CachePiezas;
use crate::model::Project;
use crate::parts::{Mesh, Tri};
use crate::view::{Camera, Layers};

/// Position, normal and colour (3 × 3 `f32`).
pub const VERTEX_BYTES: usize = 36;
/// One `u32`.
pub const INDEX_BYTES: usize = 4;
/// Offset (3), scale (3) and rotation cosine/sine (2), as `f32`.
pub const INSTANCE_BYTES: usize = 32;
/// GPU memory budget for the whole scene (vertices + indices + instances).
pub const GPU_BUDGET: u64 = 512 * 1024 * 1024;

/// On-screen radius (points) from which the full mesh is drawn: the same threshold the painter
/// (`view.rs`) uses to go from LOD 1 to LOD 2.
pub const FULL_DETAIL_PX: f32 = 14.0;
/// Below this on-screen radius (points) the box is enough.
pub const SIMPLE_DETAIL_PX: f32 = 3.0;

const GRID_SIDE: usize = 16;
/// Part signature quantisation: 0.1 mm in plan, 1e-4 of the part height in Z. Coarser than the
/// `f32` noise left by subtracting the part corner, so equal parts always match.
const QUANTUM_XY: f32 = 1e4;
const QUANTUM_Z: f32 = 1e4;
const QUANTUM_COLOUR: f32 = 4096.0;

/// A library model already placed in the project.
pub struct PlacedModel {
    /// Local position (origin subtracted) and elevation.
    pub position: [f32; 3],
    /// Z-up mesh, in metres.
    pub mesh: Arc<Vec<Vertice>>,
    /// Rotation about Z, in degrees.
    pub rotation_deg: f32,
}

/// A contiguous slice of the index buffer and the vertices it addresses.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MeshRange {
    pub first_index: u32,
    pub index_count: u32,
    pub base_vertex: i32,
    pub vertex_count: u32,
}

impl MeshRange {
    fn end_bytes(&self) -> (u64, u64) {
        let vertices = (self.base_vertex as u64 + self.vertex_count as u64) * VERTEX_BYTES as u64;
        let indices = (self.first_index as u64 + self.index_count as u64) * INDEX_BYTES as u64;
        (vertices, indices)
    }
}

/// Axis-aligned bounding box in local coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Aabb {
    pub min: [f32; 3],
    pub max: [f32; 3],
}

impl Aabb {
    pub const EMPTY: Aabb = Aabb {
        min: [f32::INFINITY; 3],
        max: [f32::NEG_INFINITY; 3],
    };

    pub fn include(&mut self, p: [f32; 3]) {
        self.min = [0, 1, 2].map(|k| self.min[k].min(p[k]));
        self.max = [0, 1, 2].map(|k| self.max[k].max(p[k]));
    }

    pub fn is_empty(&self) -> bool {
        self.min[0] > self.max[0]
    }

    fn size(&self) -> [f32; 3] {
        [
            self.max[0] - self.min[0],
            self.max[1] - self.min[1],
            self.max[2] - self.min[2],
        ]
    }

    fn centre(&self) -> [f32; 3] {
        [
            (self.min[0] + self.max[0]) * 0.5,
            (self.min[1] + self.max[1]) * 0.5,
            (self.min[2] + self.max[2]) * 0.5,
        ]
    }

    fn corner(&self, i: usize) -> [f32; 3] {
        [
            if i & 1 == 0 { self.min[0] } else { self.max[0] },
            if i & 2 == 0 { self.min[1] } else { self.max[1] },
            if i & 4 == 0 { self.min[2] } else { self.max[2] },
        ]
    }
}

/// One placement of a prototype. The vertex shader applies the same transform as [`Self::apply`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Instance {
    pub prototype: u32,
    pub offset: [f32; 3],
    pub scale: [f32; 3],
    /// Cosine and sine of the rotation about Z.
    pub rotation: [f32; 2],
}

impl Instance {
    const IDENTITY: Instance = Instance {
        prototype: 0,
        offset: [0.0; 3],
        scale: [1.0; 3],
        rotation: [1.0, 0.0],
    };

    pub fn apply(&self, p: [f32; 3]) -> [f32; 3] {
        let [x, y, z] = [
            p[0] * self.scale[0],
            p[1] * self.scale[1],
            p[2] * self.scale[2],
        ];
        let [c, s] = self.rotation;
        [
            x * c - y * s + self.offset[0],
            x * s + y * c + self.offset[1],
            z + self.offset[2],
        ]
    }

    fn write_bytes(&self, out: &mut Vec<u8>) {
        let floats = self.offset.iter().chain(&self.scale).chain(&self.rotation);
        for v in floats {
            out.extend_from_slice(&v.to_le_bytes());
        }
    }
}

/// Level of detail: box, simple or full.
pub type Lod = usize;
pub const LOD_BOX: Lod = 0;
pub const LOD_SIMPLE: Lod = 1;
pub const LOD_FULL: Lod = 2;

/// An object (a piece or a placed model) with its instances per level of detail.
#[derive(Clone, Debug)]
pub struct Object {
    pub bounds: Aabb,
    /// Ranges in [`Scene::instances`], indexed by [`Lod`].
    pub lods: [Range<u32>; 3],
}

/// A cell of static geometry (no instances), culled by its box.
#[derive(Clone, Debug)]
pub struct Batch {
    pub bounds: Aabb,
    pub mesh: MeshRange,
}

/// What lives on the GPU while the project and the layers stay the same.
#[derive(Clone, Debug, Default)]
pub struct Scene {
    /// Packed vertices, already trimmed to the budget.
    pub vertices: Arc<[u8]>,
    /// Little-endian `u32` indices, already trimmed to the budget.
    pub indices: Arc<[u8]>,
    pub prototypes: Vec<MeshRange>,
    pub instances: Vec<Instance>,
    pub objects: Vec<Object>,
    pub batches: Vec<Batch>,
    /// Prototypes and batches that fit the budget (the first `n` of each).
    pub drawable_prototypes: usize,
    pub drawable_batches: usize,
    /// Instances a plan can ask for in the worst case (all visible at their largest LOD), plus
    /// the identity instance the batches use.
    pub max_instances: usize,
    /// GPU bytes the whole scene asks for, whether it fits or not.
    pub requested_bytes: u64,
    /// Status-bar text when the scene does not fit the budget.
    pub warning: Option<String>,
}

impl Scene {
    /// GPU bytes actually uploaded: vertices, indices and the instance buffer.
    pub fn gpu_bytes(&self) -> u64 {
        (self.vertices.len() + self.indices.len() + self.instance_buffer_bytes()) as u64
    }

    pub fn instance_buffer_bytes(&self) -> usize {
        self.max_instances.max(1) * INSTANCE_BYTES
    }

    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }

    /// Builds the scene of the project with the visible layers and the placed models.
    pub fn build(
        p: &Project,
        layers: Layers,
        models: &[PlacedModel],
        pieces: &mut CachePiezas,
        budget: u64,
    ) -> Scene {
        let mut b = Builder::default();
        b.add_pieces(p, layers, pieces);
        b.add_models(models);
        let fixed = crate::gltf::static_triangles(p, layers.terreno);
        let visible: Vec<bool> = fixed
            .familias
            .iter()
            .map(|f| family_visible(f, layers))
            .collect();
        let tris: Vec<&Triangulo> = fixed
            .tris
            .iter()
            .filter(|t| visible[t.familia as usize])
            .collect();
        b.add_batches(&tris);
        b.finish(budget)
    }

    /// Placed triangles with every object at `lod`. For tests and measurements: it shows that
    /// instancing reproduces the meshes.
    pub fn world_triangles(&self, lod: Lod) -> Vec<[[f32; 3]; 3]> {
        let mut out = Vec::new();
        for o in &self.objects {
            for inst in &self.instances[as_usize(&o.lods[lod.min(LOD_FULL)])] {
                let proto = self.prototypes[inst.prototype as usize];
                out.extend(
                    self.triangles_of(proto)
                        .into_iter()
                        .map(|t| t.map(|v| inst.apply(v))),
                );
            }
        }
        for batch in &self.batches {
            out.extend(self.triangles_of(batch.mesh));
        }
        out
    }

    fn triangles_of(&self, m: MeshRange) -> Vec<[[f32; 3]; 3]> {
        let index = |k: usize| read_u32(&self.indices, k * INDEX_BYTES);
        let position = |v: u32| -> [f32; 3] {
            let at = (v as i64 + m.base_vertex as i64) as usize * VERTEX_BYTES;
            [0, 1, 2].map(|k| read_f32(&self.vertices, at + k * 4))
        };
        (0..m.index_count as usize / 3)
            .map(|t| {
                let k = m.first_index as usize + t * 3;
                [0, 1, 2].map(|j| position(index(k + j)))
            })
            .collect()
    }
}

fn read_u32(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}

fn read_f32(bytes: &[u8], at: usize) -> f32 {
    f32::from_bits(read_u32(bytes, at))
}

fn as_usize(r: &Range<u32>) -> Range<usize> {
    r.start as usize..r.end as usize
}

/// Same layer rules as the painter: water, sewer and terrain by their layer, everything else
/// (urban context and shared pieces) under «edificios».
pub fn family_visible(name: &str, layers: Layers) -> bool {
    if name.starts_with("agua.") {
        return layers.agua;
    }
    if name.starts_with("saneamiento.") {
        return layers.alcantarillado;
    }
    name == "terreno" || layers.edificios
}

/// Offset of a part: the node base in `f64` plus the part corner, and only then `f32`.
fn place(base: [f64; 3], corner: [f32; 3]) -> [f32; 3] {
    [0, 1, 2].map(|k| (base[k] + corner[k] as f64) as f32)
}

fn average_colour(m: &Mesh) -> [f32; 3] {
    if m.tris.is_empty() {
        return [0.6, 0.6, 0.6];
    }
    let mut sum = [0.0f32; 3];
    for t in &m.tris {
        for (k, v) in sum.iter_mut().enumerate() {
            *v += t.color[k];
        }
    }
    sum.map(|v| v / m.tris.len() as f32)
}

/// A registered part: its prototype and how to place it relative to the node base.
struct Part {
    prototype: u32,
    corner: [f32; 3],
    size_xy: [f32; 2],
    scale_z: f32,
}

#[derive(Default)]
struct Builder {
    vertices: Vec<u8>,
    vertex_count: u32,
    indices: Vec<u32>,
    prototypes: Vec<MeshRange>,
    /// Canonical signature per prototype (empty for boxes and models).
    signatures: Vec<Vec<i32>>,
    by_hash: HashMap<u64, Vec<u32>>,
    boxes: HashMap<[i32; 3], u32>,
    instances: Vec<Instance>,
    objects: Vec<Object>,
    batches: Vec<Batch>,
    signature: Vec<i32>,
}

impl Builder {
    fn add_pieces(&mut self, p: &Project, layers: Layers, pieces: &mut CachePiezas) {
        pieces.iniciar_pasada();
        let mut split_meshes: HashMap<usize, Arc<Vec<Part>>> = HashMap::new();
        for n in p.nodes.iter().filter(|n| !n.deleted) {
            if !family_visible(&n.family, layers) {
                continue;
            }
            let full = pieces.pieza(n, LOD_FULL as u8);
            if full.is_empty() {
                continue;
            }
            let simple = pieces.pieza(n, LOD_SIMPLE as u8);
            let base = [n.pos.x - p.origin.x, n.pos.y - p.origin.y, n.z_invert];
            let mut bounds = Aabb::EMPTY;
            let mut lods: [Range<u32>; 3] = [0..0, 0..0, 0..0];
            for (lod, mesh) in [(LOD_FULL, &full), (LOD_SIMPLE, &simple)] {
                // The cache hands out the same `Arc` for equal pieces: split each mesh once.
                let parts = split_meshes
                    .entry(Arc::as_ptr(mesh) as usize)
                    .or_insert_with(|| Arc::new(self.split(mesh)))
                    .clone();
                lods[lod] = self.place_parts(&parts, base, &mut bounds);
            }
            lods[LOD_BOX] = self.place_box(average_colour(&simple), bounds);
            self.objects.push(Object { bounds, lods });
        }
        pieces.podar();
    }

    fn place_parts(&mut self, parts: &[Part], base: [f64; 3], bounds: &mut Aabb) -> Range<u32> {
        let start = self.instances.len() as u32;
        for part in parts {
            let offset = place(base, part.corner);
            self.instances.push(Instance {
                prototype: part.prototype,
                offset,
                scale: [1.0, 1.0, part.scale_z],
                rotation: [1.0, 0.0],
            });
            bounds.include(offset);
            bounds.include([
                offset[0] + part.size_xy[0],
                offset[1] + part.size_xy[1],
                offset[2] + part.scale_z,
            ]);
        }
        start..self.instances.len() as u32
    }

    fn place_box(&mut self, colour: [f32; 3], bounds: Aabb) -> Range<u32> {
        let prototype = self.box_prototype(colour);
        let start = self.instances.len() as u32;
        self.instances.push(Instance {
            prototype,
            offset: bounds.min,
            scale: bounds.size().map(|s| s.max(1e-3)),
            rotation: [1.0, 0.0],
        });
        start..start + 1
    }

    /// One prototype per library mesh, one instance per placement, the same at every LOD.
    fn add_models(&mut self, models: &[PlacedModel]) {
        let mut by_mesh: HashMap<usize, u32> = HashMap::new();
        for m in models {
            let prototype = *by_mesh
                .entry(Arc::as_ptr(&m.mesh) as usize)
                .or_insert_with(|| self.model_prototype(&m.mesh));
            let (sin, cos) = m.rotation_deg.to_radians().sin_cos();
            let instance = Instance {
                prototype,
                offset: m.position,
                scale: [1.0; 3],
                rotation: [cos, sin],
            };
            let mut bounds = Aabb::EMPTY;
            for v in m.mesh.iter() {
                bounds.include(instance.apply(v.pos));
            }
            if bounds.is_empty() {
                continue;
            }
            let i = self.instances.len() as u32;
            self.instances.push(instance);
            self.objects.push(Object {
                bounds,
                lods: [i..i + 1, i..i + 1, i..i + 1],
            });
        }
    }

    fn push_vertex(&mut self, pos: [f32; 3], normal: [f32; 3], colour: [f32; 3]) {
        for v in pos.iter().chain(&normal).chain(&colour) {
            self.vertices.extend_from_slice(&v.to_le_bytes());
        }
        self.vertex_count += 1;
    }

    /// Appends flat-shaded triangles (position, normal, colour), sharing equal vertices.
    fn add_indexed(
        &mut self,
        tris: impl Iterator<Item = ([[f32; 3]; 3], [f32; 3], [f32; 3])>,
    ) -> MeshRange {
        let first_index = self.indices.len() as u32;
        let base = self.vertex_count;
        let mut seen: HashMap<[u32; 9], u32> = HashMap::new();
        for (pos, normal, colour) in tris {
            for p in pos {
                let mut key = [0u32; 9];
                for (k, v) in p.iter().chain(&normal).chain(&colour).enumerate() {
                    key[k] = v.to_bits();
                }
                let next = self.vertex_count - base;
                let index = *seen.entry(key).or_insert(next);
                if index == next {
                    self.push_vertex(p, normal, colour);
                }
                self.indices.push(index);
            }
        }
        MeshRange {
            first_index,
            index_count: self.indices.len() as u32 - first_index,
            base_vertex: base as i32,
            vertex_count: self.vertex_count - base,
        }
    }

    fn new_prototype(&mut self, mesh: MeshRange, signature: Vec<i32>) -> u32 {
        self.prototypes.push(mesh);
        self.signatures.push(signature);
        (self.prototypes.len() - 1) as u32
    }

    /// Splits a piece mesh at its part breaks ([`Mesh::part_breaks`]) and registers each part.
    fn split(&mut self, mesh: &Mesh) -> Vec<Part> {
        let total = mesh.tris.len();
        let mut breaks: Vec<usize> = mesh
            .part_breaks
            .iter()
            .map(|b| *b as usize)
            .filter(|b| *b > 0 && *b < total)
            .collect();
        breaks.sort_unstable();
        breaks.dedup();
        let mut parts = Vec::with_capacity(breaks.len() + 1);
        let mut start = 0;
        for end in breaks.into_iter().chain(std::iter::once(total)) {
            parts.push(self.register(&mesh.tris[start..end]));
            start = end;
        }
        parts
    }

    /// Moves a part to its canonical form and reuses the prototype if one already matches it.
    fn register(&mut self, tris: &[Tri]) -> Part {
        let mut bounds = Aabb::EMPTY;
        for t in tris {
            for p in t.p {
                bounds.include(p);
            }
        }
        let height = bounds.max[2] - bounds.min[2];
        let scale_z = if height > 1e-4 { height } else { 1.0 };
        let canonical = |p: [f32; 3]| -> [f32; 3] {
            [
                p[0] - bounds.min[0],
                p[1] - bounds.min[1],
                (p[2] - bounds.min[2]) / scale_z,
            ]
        };
        self.fill_signature(tris, canonical);
        let hash = fx_hash(&self.signature);
        let existing = self.by_hash.get(&hash).and_then(|candidates| {
            candidates
                .iter()
                .copied()
                .find(|c| self.signatures[*c as usize] == self.signature)
        });
        let prototype = match existing {
            Some(proto) => proto,
            None => {
                let mesh = self.add_indexed(tris.iter().map(|t| {
                    let pos = t.p.map(canonical);
                    (pos, normal_of(pos), [t.color[0], t.color[1], t.color[2]])
                }));
                let proto = self.new_prototype(mesh, self.signature.clone());
                self.by_hash.entry(hash).or_default().push(proto);
                proto
            }
        };
        Part {
            prototype,
            corner: bounds.min,
            size_xy: [bounds.max[0] - bounds.min[0], bounds.max[1] - bounds.min[1]],
            scale_z,
        }
    }

    fn fill_signature(&mut self, tris: &[Tri], canonical: impl Fn([f32; 3]) -> [f32; 3]) {
        self.signature.clear();
        for t in tris {
            for p in t.p {
                let q = canonical(p);
                self.signature.push((q[0] * QUANTUM_XY).round() as i32);
                self.signature.push((q[1] * QUANTUM_XY).round() as i32);
                self.signature.push((q[2] * QUANTUM_Z).round() as i32);
            }
            for c in &t.color[..3] {
                self.signature.push((c * QUANTUM_COLOUR).round() as i32);
            }
        }
    }

    /// Unit cube `[0,1]³` in one colour: the «box» level of detail.
    fn box_prototype(&mut self, colour: [f32; 3]) -> u32 {
        let key = colour.map(|v| (v * 32.0).round() as i32);
        if let Some(proto) = self.boxes.get(&key) {
            return *proto;
        }
        let colour = key.map(|v| v as f32 / 32.0);
        let corner = |i: usize| {
            Aabb {
                min: [0.0; 3],
                max: [1.0; 3],
            }
            .corner(i)
        };
        const FACES: [[usize; 4]; 6] = [
            [0, 2, 3, 1],
            [4, 5, 7, 6],
            [0, 1, 5, 4],
            [1, 3, 7, 5],
            [3, 2, 6, 7],
            [2, 0, 4, 6],
        ];
        let tris: Vec<_> = FACES
            .iter()
            .flat_map(|[a, b, c, d]| [[*a, *b, *c], [*a, *c, *d]])
            .map(|t| {
                let pos = t.map(corner);
                (pos, normal_of(pos), colour)
            })
            .collect();
        let mesh = self.add_indexed(tris.into_iter());
        let proto = self.new_prototype(mesh, Vec::new());
        self.boxes.insert(key, proto);
        proto
    }

    fn model_prototype(&mut self, mesh: &[Vertice]) -> u32 {
        let range = self.add_indexed(
            mesh.chunks_exact(3)
                .map(|t| ([t[0].pos, t[1].pos, t[2].pos], t[0].normal, t[0].color)),
        );
        self.new_prototype(range, Vec::new())
    }

    /// Spreads the static geometry over a 16 × 16 grid by triangle centroid.
    fn add_batches(&mut self, tris: &[&Triangulo]) {
        if tris.is_empty() {
            return;
        }
        let centroid = |t: &Triangulo| -> [f32; 2] {
            [0, 1].map(|k| (t.pos[0][k] + t.pos[1][k] + t.pos[2][k]) / 3.0)
        };
        let mut extent = Aabb::EMPTY;
        for t in tris {
            let [x, y] = centroid(t);
            extent.include([x, y, 0.0]);
        }
        let size = extent.size().map(|s| s.max(1e-3));
        let cell = |[x, y]: [f32; 2]| -> usize {
            let axis = |v: f32, k: usize| {
                (((v - extent.min[k]) / size[k] * GRID_SIDE as f32) as usize).min(GRID_SIDE - 1)
            };
            axis(y, 1) * GRID_SIDE + axis(x, 0)
        };
        let mut cells: Vec<Vec<&Triangulo>> = vec![Vec::new(); GRID_SIDE * GRID_SIDE];
        for t in tris {
            cells[cell(centroid(t))].push(t);
        }
        for group in cells.into_iter().filter(|g| !g.is_empty()) {
            let mut bounds = Aabb::EMPTY;
            for p in group.iter().flat_map(|t| t.pos) {
                bounds.include(p);
            }
            let mesh = self.add_indexed(group.iter().map(|t| (t.pos, t.normal, t.color)));
            self.batches.push(Batch { bounds, mesh });
        }
    }

    /// Closes the scene and trims it to the budget: prototypes first, then batches, keeping the
    /// prefix that fits.
    fn finish(self, budget: u64) -> Scene {
        let max_instances = 1 + self
            .objects
            .iter()
            .map(|o| o.lods.iter().map(|r| r.len()).max().unwrap_or(0))
            .sum::<usize>();
        let instance_bytes = (max_instances * INSTANCE_BYTES) as u64;
        let requested_bytes =
            instance_bytes + self.vertices.len() as u64 + (self.indices.len() * INDEX_BYTES) as u64;

        let ranges = self
            .prototypes
            .iter()
            .copied()
            .chain(self.batches.iter().map(|b| b.mesh));
        let (mut vertex_end, mut index_end, mut fitting) = (0u64, 0u64, 0usize);
        for range in ranges {
            let (v, i) = range.end_bytes();
            if instance_bytes + v + i > budget {
                break;
            }
            (vertex_end, index_end, fitting) = (v, i, fitting + 1);
        }
        let drawable_prototypes = fitting.min(self.prototypes.len());
        let drawable_batches = fitting - drawable_prototypes;
        let total = self.prototypes.len() + self.batches.len();
        let warning = (fitting < total).then(|| {
            format!(
                "La escena 3D pide {} MB de GPU y el tope es {} MB: no se dibujan {} de {total} mallas",
                requested_bytes.div_ceil(MIB),
                budget / MIB,
                total - fitting,
            )
        });
        let index_count = index_end as usize / INDEX_BYTES;
        let indices: Vec<u8> = self.indices[..index_count]
            .iter()
            .flat_map(|i| i.to_le_bytes())
            .collect();
        Scene {
            vertices: Arc::from(&self.vertices[..vertex_end as usize]),
            indices: Arc::from(indices),
            prototypes: self.prototypes,
            instances: self.instances,
            objects: self.objects,
            batches: self.batches,
            drawable_prototypes,
            drawable_batches,
            max_instances,
            requested_bytes,
            warning,
        }
    }
}

const MIB: u64 = 1024 * 1024;

/// Fast non-cryptographic hash; equality is then confirmed on the whole signature.
fn fx_hash(values: &[i32]) -> u64 {
    values.iter().fold(values.len() as u64, |h, v| {
        (h.rotate_left(5) ^ (*v as u32 as u64)).wrapping_mul(0x517c_c1b7_2722_0a95)
    })
}

fn normal_of(tri: [[f32; 3]; 3]) -> [f32; 3] {
    let u = [0, 1, 2].map(|k| tri[1][k] - tri[0][k]);
    let v = [0, 1, 2].map(|k| tri[2][k] - tri[0][k]);
    let n = [
        u[1] * v[2] - u[2] * v[1],
        u[2] * v[0] - u[0] * v[2],
        u[0] * v[1] - u[1] * v[0],
    ];
    let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-9);
    n.map(|c| c / len)
}

// ---------------------------------------------------------------------------------------------
// Per-camera plan

/// One indexed, instanced draw call.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DrawCall {
    pub mesh: MeshRange,
    pub first_instance: u32,
    pub instance_count: u32,
}

/// What one camera draws: visible instances at their level of detail, grouped by prototype.
#[derive(Clone, Debug, Default)]
pub struct DrawPlan {
    /// Instance buffer (32 bytes each); instance 0 is the identity, for the batches.
    pub instances: Arc<[u8]>,
    pub draws: Vec<DrawCall>,
    /// Triangles the GPU draws with this plan.
    pub triangles: u64,
    /// Visible objects per level of detail.
    pub objects_per_lod: [usize; 3],
    /// Objects and batches left out by culling.
    pub culled: usize,
}

/// Does the box fall entirely outside the view frustum? Tests the 8 corners in clip space
/// (wgpu depth 0..1). `view_proj` is column-major.
pub fn outside_frustum(view_proj: &[f32; 16], b: &Aabb) -> bool {
    let mut outside = [true; 6];
    for i in 0..8 {
        let [x, y, z, w] = to_clip(view_proj, b.corner(i));
        outside[0] &= x < -w;
        outside[1] &= x > w;
        outside[2] &= y < -w;
        outside[3] &= y > w;
        outside[4] &= z < 0.0;
        outside[5] &= z > w;
    }
    outside.iter().any(|o| *o)
}

fn to_clip(m: &[f32; 16], p: [f32; 3]) -> [f32; 4] {
    [0, 1, 2, 3].map(|r| m[r] * p[0] + m[4 + r] * p[1] + m[8 + r] * p[2] + m[12 + r])
}

/// On-screen radius in points of one metre seen from one metre away.
pub fn px_per_metre(cam: &Camera) -> f32 {
    let tan = (cam.fov * 0.5).to_radians().tan().max(1e-3);
    (cam.viewport[1] as f64 * 0.5 / tan) as f32
}

/// Level of detail from the on-screen radius of the box.
pub fn level_of_detail(view_proj: &[f32; 16], px_per_metre: f32, b: &Aabb) -> Lod {
    let [sx, sy, sz] = b.size();
    let radius = 0.5 * (sx * sx + sy * sy + sz * sz).sqrt();
    let w = to_clip(view_proj, b.centre())[3];
    if w <= radius.max(1e-3) {
        return LOD_FULL;
    }
    let radius_px = radius * px_per_metre / w;
    if radius_px >= FULL_DETAIL_PX {
        LOD_FULL
    } else if radius_px >= SIMPLE_DETAIL_PX {
        LOD_SIMPLE
    } else {
        LOD_BOX
    }
}

/// Draw plan for one camera: box culling, LOD by on-screen size and instances grouped by
/// prototype. Vertices and indices are not touched.
pub fn plan(scene: &Scene, view_proj: &[f32; 16], px_per_metre: f32) -> DrawPlan {
    let mut plan = DrawPlan::default();
    for batch in &scene.batches[..scene.drawable_batches] {
        if outside_frustum(view_proj, &batch.bounds) {
            plan.culled += 1;
            continue;
        }
        plan.draws.push(DrawCall {
            mesh: batch.mesh,
            first_instance: 0,
            instance_count: 1,
        });
        plan.triangles += (batch.mesh.index_count / 3) as u64;
    }
    let chosen = visible_instances(scene, view_proj, px_per_metre, &mut plan);
    let grouped = group_by_prototype(scene, &chosen);
    let mut bytes = Vec::with_capacity((1 + grouped.order.len()) * INSTANCE_BYTES);
    Instance::IDENTITY.write_bytes(&mut bytes);
    for i in &grouped.order {
        scene.instances[*i as usize].write_bytes(&mut bytes);
    }
    for (proto, (first, count)) in grouped.spans.iter().enumerate() {
        if *count == 0 {
            continue;
        }
        let mesh = scene.prototypes[proto];
        plan.draws.push(DrawCall {
            mesh,
            first_instance: *first,
            instance_count: *count,
        });
        plan.triangles += (mesh.index_count / 3) as u64 * *count as u64;
    }
    plan.instances = Arc::from(bytes);
    plan
}

fn visible_instances(
    scene: &Scene,
    view_proj: &[f32; 16],
    px_per_metre: f32,
    plan: &mut DrawPlan,
) -> Vec<u32> {
    let mut chosen = Vec::new();
    for o in &scene.objects {
        if outside_frustum(view_proj, &o.bounds) {
            plan.culled += 1;
            continue;
        }
        let lod = level_of_detail(view_proj, px_per_metre, &o.bounds);
        plan.objects_per_lod[lod] += 1;
        chosen.extend(o.lods[lod].clone());
    }
    chosen
}

struct Grouped {
    /// Instance indices, sorted by prototype.
    order: Vec<u32>,
    /// `(first instance in the plan buffer, count)` per prototype.
    spans: Vec<(u32, u32)>,
}

/// Counting sort by prototype; prototypes over the budget are dropped.
fn group_by_prototype(scene: &Scene, chosen: &[u32]) -> Grouped {
    let drawable = |i: &u32| {
        let proto = scene.instances[*i as usize].prototype as usize;
        (proto < scene.drawable_prototypes).then_some(proto)
    };
    let mut spans = vec![(0u32, 0u32); scene.prototypes.len()];
    for proto in chosen.iter().filter_map(drawable) {
        spans[proto].1 += 1;
    }
    // Slot 0 of the plan buffer is the identity instance.
    let mut next = 1u32;
    for span in &mut spans {
        span.0 = next;
        next += span.1;
    }
    let mut cursor: Vec<u32> = spans.iter().map(|s| s.0).collect();
    let mut order = vec![0u32; (next - 1) as usize];
    for i in chosen {
        if let Some(proto) = drawable(i) {
            order[(cursor[proto] - 1) as usize] = *i;
            cursor[proto] += 1;
        }
    }
    Grouped { order, spans }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manhole() -> crate::model::Node {
        let p = crate::demo::project();
        p.nodes
            .iter()
            .find(|n| n.family == "saneamiento.pozo")
            .expect("demo manhole")
            .clone()
    }

    fn camera_on(target: [f64; 3], distance: f64) -> (Camera, [f32; 16]) {
        let cam = Camera {
            viewport: [900.0, 600.0],
            target,
            distance,
            ..Default::default()
        };
        let vp = cam.matriz(1.5, 1.0);
        (cam, vp)
    }

    fn build(p: &Project, budget: u64) -> Scene {
        Scene::build(
            p,
            Layers::default(),
            &[],
            &mut CachePiezas::default(),
            budget,
        )
    }

    /// A manhole splits into parts (shaft, cone, rungs…) and a deeper one reuses almost all of
    /// them: the taller shaft is the same prototype with another Z scale.
    #[test]
    fn manholes_of_different_depth_share_prototypes() {
        let n = manhole();
        let mut b = Builder::default();
        let parts = b.split(&crate::view::pieza_con_detalle(&n, 2));
        assert!(parts.len() > 5, "{} parts", parts.len());
        let first = b.prototypes.len();
        assert!(first < parts.len(), "the rungs share one prototype");
        let mut deeper = n.clone();
        deeper.z_invert -= 1.7;
        b.split(&crate::view::pieza_con_detalle(&deeper, 2));
        let new = b.prototypes.len() - first;
        assert!(
            new <= 3,
            "{new} new prototypes for the same manhole 1.7 m deeper"
        );
    }

    /// With every object at full detail, the instanced scene draws the same triangles as the
    /// exported mesh (to 1 mm), so sharing prototypes changes nothing on screen.
    #[test]
    fn instancing_reproduces_the_exported_mesh() {
        let p = crate::demo_city::ciudad();
        let scene = build(&p, GPU_BUDGET);
        let key = |t: &[[f32; 3]; 3]| -> [i64; 3] {
            [0, 1, 2].map(|k| ((t[0][k] + t[1][k] + t[2][k]) / 3.0 * 1000.0).round() as i64)
        };
        let drawn: Vec<[i64; 3]> = scene.world_triangles(LOD_FULL).iter().map(key).collect();
        let mesh = crate::gltf::triangulos_exportables(&p, true);
        assert_eq!(drawn.len(), mesh.tris.len());
        let exported: std::collections::HashSet<[i64; 3]> =
            mesh.tris.iter().map(|t| key(&t.pos)).collect();
        let within_1_mm = |c: &[i64; 3]| {
            (0..27).any(|i| {
                let d = [i % 3 - 1, i / 3 % 3 - 1, i / 9 - 1];
                exported.contains(&[c[0] + d[0], c[1] + d[1], c[2] + d[2]])
            })
        };
        let off = drawn.iter().filter(|c| !within_1_mm(c)).count();
        assert_eq!(off, 0, "{off} triangles moved by more than 1 mm");
        assert!(scene.prototypes.len() * 4 < scene.instances.len());
    }

    #[test]
    fn distant_objects_draw_as_boxes_and_near_ones_in_full() {
        let p = crate::demo::project();
        let n = manhole();
        let local = [n.pos.x - p.origin.x, n.pos.y - p.origin.y, n.z_rim];
        let scene = build(&p, GPU_BUDGET);

        let (cam, vp) = camera_on(local, 12.0);
        let near = plan(&scene, &vp, px_per_metre(&cam));
        assert!(
            near.objects_per_lod[LOD_FULL] >= 1,
            "{:?}",
            near.objects_per_lod
        );

        let (cam, vp) = camera_on(local, 20_000.0);
        let far = plan(&scene, &vp, px_per_metre(&cam));
        let visible: usize = far.objects_per_lod.iter().sum();
        assert!(visible > 1);
        assert_eq!(
            far.objects_per_lod[LOD_BOX], visible,
            "{:?}",
            far.objects_per_lod
        );
    }

    #[test]
    fn objects_outside_the_view_are_culled() {
        let p = crate::demo_city::ciudad();
        let scene = build(&p, GPU_BUDGET);
        let mut cam = Camera {
            viewport: [900.0, 600.0],
            ..Default::default()
        };
        cam.fit(&p);
        let whole = plan(&scene, &cam.matriz(1.5, 1.0), px_per_metre(&cam));
        assert_eq!(whole.culled, 0, "the fitted view sees everything");

        cam.distance = 10.0;
        let close = plan(&scene, &cam.matriz(1.5, 1.0), px_per_metre(&cam));
        assert!(
            close.culled > scene.objects.len() / 2,
            "culled {}",
            close.culled
        );
        assert!(close.draws.len() < whole.draws.len());

        let behind = Aabb {
            min: [-1e6, -1e6, -1e6],
            max: [-1e6 + 1.0, -1e6 + 1.0, -1e6 + 1.0],
        };
        assert!(outside_frustum(&cam.matriz(1.5, 1.0), &behind));
    }

    #[test]
    fn a_scene_over_budget_is_trimmed_and_warns() {
        let p = crate::demo_city::ciudad();
        let full = build(&p, GPU_BUDGET);
        assert!(full.warning.is_none(), "{:?}", full.warning);
        assert_eq!(full.gpu_bytes(), full.requested_bytes);

        let budget = full.requested_bytes / 2;
        let half = build(&p, budget);
        assert!(
            half.gpu_bytes() <= budget,
            "{} > {budget}",
            half.gpu_bytes()
        );
        let warning = half.warning.clone().expect("over budget: warning");
        assert!(warning.contains("no se dibujan"), "{warning}");
        let (cam, vp) = camera_on([0.0, 0.0, 2500.0], 5_000.0);
        let trimmed = plan(&half, &vp, px_per_metre(&cam));
        let last_index = trimmed
            .draws
            .iter()
            .map(|d| d.mesh.first_index + d.mesh.index_count)
            .max()
            .unwrap_or(0);
        assert!(last_index as usize * INDEX_BYTES <= half.indices.len());
    }
}
