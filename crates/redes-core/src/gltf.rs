//! Exportador glTF 2.0 (`.glb`) para abrir el modelo en Blender, three.js o cualquier visor
//! estándar.
//!
//! La geometría fija va en una primitiva por familia, y cada pieza en un nodo hijo que comparte su
//! malla con las piezas iguales. Posiciones, normales y color por vértice; material `unlit` para
//! que el visor externo muestre el mismo color que la app (que ya sombrea por triángulo). El nodo
//! raíz lleva la rotación de Z-arriba (modelo) a Y-arriba (glTF).

use std::collections::BTreeMap;

use crate::geom::{P2, P3};
use crate::model::Project;
use crate::parts;

/// Opciones de exportación.
#[derive(Clone, Copy, Debug)]
pub struct OpcionesGltf {
    /// Incluir el terreno como malla (por defecto sí).
    pub con_terreno: bool,
}

impl Default for OpcionesGltf {
    fn default() -> Self {
        OpcionesGltf { con_terreno: true }
    }
}

pub struct Vertice {
    pub pos: [f32; 3],
    pub normal: [f32; 3],
    pub color: [f32; 3],
}

/// Triángulo en coordenadas locales del proyecto (origen restado), listo para un motor 3D.
#[derive(Clone, Copy, Debug)]
pub struct Triangulo {
    pub pos: [[f32; 3]; 3],
    pub normal: [f32; 3],
    pub color: [f32; 3],
    /// Índice en `Malla::familias`.
    pub familia: u16,
}

/// Malla completa con la lista de familias que la componen.
#[derive(Clone, Debug, Default)]
pub struct Malla {
    pub familias: Vec<String>,
    pub tris: Vec<Triangulo>,
}

/// Extrae la geometría del proyecto en triángulos con normal y color.
///
/// Es la fuente única del `.glb` y del pase 3D con profundidad: así el visor externo y el interno
/// dibujan exactamente la misma malla.
pub fn triangulos_exportables(p: &Project, con_terreno: bool) -> Malla {
    triangulos_con_cache(p, con_terreno, &mut crate::mallas::CachePiezas::default())
}

/// Igual que [`triangulos_exportables`], pero toma las mallas de pieza de una caché que vive
/// entre reconstrucciones (F14-06): las piezas que no cambiaron no se vuelven a generar.
pub fn triangulos_con_cache(
    p: &Project,
    con_terreno: bool,
    cache: &mut crate::mallas::CachePiezas,
) -> Malla {
    let mut out = Malla::default();
    let mut families = FamilyTable::default();
    cache.iniciar_pasada();
    for n in p.nodes.iter().filter(|n| !n.deleted) {
        let malla = cache.pieza(n, 2);
        let family = families.id(&mut out, &n.family);
        for tri in &malla.tris {
            let pos = tri.p.map(|v| {
                to_local(
                    p,
                    P3::new(
                        v[0] as f64 + n.pos.x,
                        v[1] as f64 + n.pos.y,
                        v[2] as f64 + n.z_invert,
                    ),
                )
            });
            out.tris.push(Triangulo {
                pos,
                normal: normal_de(pos),
                color: [tri.color[0], tri.color[1], tri.color[2]],
                familia: family,
            });
        }
    }
    cache.podar();
    push_static_triangles(p, con_terreno, &mut out, &mut families);
    out
}

/// Everything but the node pieces: pipes, urban context and terrain. The GPU scene draws these
/// as culled batches and the pieces as instances (F14-05).
pub fn static_triangles(p: &Project, with_terrain: bool) -> Malla {
    let mut out = Malla::default();
    push_static_triangles(p, with_terrain, &mut out, &mut FamilyTable::default());
    out
}

#[derive(Default)]
struct FamilyTable {
    index: BTreeMap<String, u16>,
}

impl FamilyTable {
    fn id(&mut self, malla: &mut Malla, name: &str) -> u16 {
        if let Some(i) = self.index.get(name) {
            return *i;
        }
        let i = malla.familias.len() as u16;
        malla.familias.push(name.to_string());
        self.index.insert(name.to_string(), i);
        i
    }
}

/// Subtracts the origin in `f64` before going down to `f32` (docs/15).
fn to_local(p: &Project, q: P3) -> [f32; 3] {
    [
        (q.x - p.origin.x) as f32,
        (q.y - p.origin.y) as f32,
        q.z as f32,
    ]
}

fn push_static_triangles(
    p: &Project,
    con_terreno: bool,
    out: &mut Malla,
    families: &mut FamilyTable,
) {
    let a_local = |q: P3| to_local(p, q);
    for l in p.links.iter().filter(|l| !l.deleted) {
        if l.path.len() < 2 {
            continue;
        }
        let es_agua = l.family.starts_with("agua");
        let mat = if es_agua {
            parts::Material::new(0.34, 0.55, 0.82)
        } else {
            parts::Material::new(0.78, 0.62, 0.44)
        };
        // Eje = solera + DI/2; sección = diámetro exterior (F12-02).
        let d_ext = l.diametro_exterior_m() as f32;
        let material = l.get("material").and_then(|v| v.as_str()).unwrap_or("");
        let family = families.id(out, &l.family);
        for w in l.eje().windows(2) {
            // En local: construir tubos con coordendas UTM en f32 los colapsa.
            let wa = P3::new(w[0].x - p.origin.x, w[0].y - p.origin.y, w[0].z);
            let wb = P3::new(w[1].x - p.origin.x, w[1].y - p.origin.y, w[1].z);
            let malla = parts::tuberia_de_material(wa, wb, d_ext, 1, mat, material);
            for tri in &malla.tris {
                // `tuberia` ya devuelve coordenadas locales (x,y) y z absoluta: no hay que
                // volver a restar el origen.
                let pos = tri.p.map(|v| [v[0], v[1], v[2]]);
                out.tris.push(Triangulo {
                    pos,
                    normal: normal_de(pos),
                    color: [tri.color[0], tri.color[1], tri.color[2]],
                    familia: family,
                });
            }
        }
    }

    // Contexto urbano: calles y manzanas, con la cota del terreno.
    if let Some(ent) = &p.entorno {
        let cota = |q: P2| -> Option<f64> { p.terrain.as_ref().and_then(|t| t.z_at(q)) };
        let asfalto = [0.18f32, 0.19, 0.21];
        for calle in &ent.calles {
            let half = calle.ancho as f32 * 0.5;
            let family = families.id(out, &calle.nombre);
            for w in calle.eje.windows(2) {
                let (a, b) = (w[0].p2(), w[1].p2());
                let d = ((b.x - a.x).powi(2) + (b.y - a.y).powi(2)).sqrt();
                if d < 0.05 {
                    continue;
                }
                let nx = -(b.y - a.y) / d * half as f64;
                let ny = (b.x - a.x) / d * half as f64;
                let esquinas = [
                    P2::new(a.x + nx, a.y + ny),
                    P2::new(b.x + nx, b.y + ny),
                    P2::new(b.x - nx, b.y - ny),
                    P2::new(a.x - nx, a.y - ny),
                ];
                let mut pt = [[0f32; 3]; 4];
                let mut ok = true;
                for (i, q) in esquinas.iter().enumerate() {
                    match cota(*q) {
                        Some(z) => pt[i] = a_local(P3::new(q.x, q.y, z + 0.2)),
                        None => ok = false,
                    }
                }
                if !ok {
                    continue;
                }
                for tri in [[pt[0], pt[1], pt[2]], [pt[0], pt[2], pt[3]]] {
                    out.tris.push(Triangulo {
                        pos: tri,
                        normal: normal_de(tri),
                        color: asfalto,
                        familia: family,
                    });
                }
            }
        }
        for mz in &ent.manzanas {
            if mz.poligono.len() < 3 {
                continue;
            }
            let cotas: Vec<f64> = mz.poligono.iter().filter_map(|q| cota(*q)).collect();
            if cotas.len() < 3 {
                continue;
            }
            let base = cotas.iter().fold(f64::INFINITY, |a, b| a.min(*b)) - 1.0;
            let alta = cotas.iter().fold(f64::NEG_INFINITY, |a, b| a.max(*b));
            let top = alta + mz.altura.max(3.0);
            let pared = if mz.uso == "comercio" {
                [0.50f32, 0.55, 0.62]
            } else {
                [0.70f32, 0.64, 0.55]
            };
            let n = mz.poligono.len();
            let family = families.id(out, &mz.uso);
            let roof = families.id(out, "cubierta");
            for i in 0..n {
                let q1 = mz.poligono[i];
                let q2 = mz.poligono[(i + 1) % n];
                let (b1, b2) = (
                    a_local(P3::new(q1.x, q1.y, base)),
                    a_local(P3::new(q2.x, q2.y, base)),
                );
                let (t1, t2) = (
                    a_local(P3::new(q1.x, q1.y, top)),
                    a_local(P3::new(q2.x, q2.y, top)),
                );
                for tri in [[b1, b2, t2], [b1, t2, t1]] {
                    out.tris.push(Triangulo {
                        pos: tri,
                        normal: normal_de(tri),
                        color: pared,
                        familia: family,
                    });
                }
            }
            // Cubierta en abanico.
            let cx = mz.poligono.iter().map(|q| q.x).sum::<f64>() / n as f64;
            let cy = mz.poligono.iter().map(|q| q.y).sum::<f64>() / n as f64;
            let centro = a_local(P3::new(cx, cy, top));
            for i in 0..n {
                let q1 = mz.poligono[i];
                let q2 = mz.poligono[(i + 1) % n];
                let tri = [
                    centro,
                    a_local(P3::new(q1.x, q1.y, top)),
                    a_local(P3::new(q2.x, q2.y, top)),
                ];
                out.tris.push(Triangulo {
                    pos: tri,
                    normal: normal_de(tri),
                    color: [0.38, 0.36, 0.35],
                    familia: roof,
                });
            }
        }
    }

    if con_terreno && let Some(t) = &p.terrain {
        let (zmin, zmax) = t.z_range();
        let zspan = (zmax - zmin).max(1e-6);
        let family = families.id(out, "terreno");
        for tri in &t.tris {
            let pts = [
                &t.points[tri[0] as usize],
                &t.points[tri[1] as usize],
                &t.points[tri[2] as usize],
            ];
            let pos = pts.map(|q| a_local(P3::new(q.e, q.n, q.z)));
            let zc = (pts[0].z + pts[1].z + pts[2].z) / 3.0;
            let t01 = ((zc - zmin) / zspan).clamp(0.0, 1.0) as f32;
            out.tris.push(Triangulo {
                pos,
                normal: normal_de(pos),
                color: [0.26 + 0.34 * t01, 0.40 + 0.16 * t01, 0.26 + 0.06 * t01],
                familia: family,
            });
        }
    }
}

/// Número de primitivas por familia que tendría el `.glb` del proyecto (familias con geometría).
pub fn contar_primitivas(p: &Project, con_terreno: bool) -> usize {
    let fixed = static_triangles(p, con_terreno);
    let mut families: std::collections::BTreeSet<&str> = fixed
        .tris
        .iter()
        .map(|t| fixed.familias[t.familia as usize].as_str())
        .collect();
    let mut cache = crate::mallas::CachePiezas::default();
    for n in p.nodes.iter().filter(|n| !n.deleted) {
        if !cache.pieza(n, 2).is_empty() {
            families.insert(n.family.as_str());
        }
    }
    families.len()
}

/// The `.glb` would not fit the format: GLB lengths are `u32`, so 4 GiB at most.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GlbTooLarge {
    pub bytes: u64,
}

impl std::fmt::Display for GlbTooLarge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "el .glb pesaría {} MB y el formato admite 4 096 MB como máximo; exporte sin terreno o por partes",
            self.bytes.div_ceil(1024 * 1024)
        )
    }
}

impl std::error::Error for GlbTooLarge {}

/// Exports the project to a binary glTF (`.glb`).
///
/// Pipes, urban context and terrain go in one mesh with a primitive per family. Each node piece is
/// a child node with its translation, and nodes with the same piece (same `ClavePieza`) share one
/// mesh, so 20k equal manholes write one manhole mesh instead of 20k copies (F14-05).
pub fn exportar_glb(p: &Project, opts: OpcionesGltf) -> Result<Vec<u8>, GlbTooLarge> {
    let fixed = static_triangles(p, opts.con_terreno);
    let mut cache = crate::mallas::CachePiezas::default();
    let pieces: Vec<(&crate::model::Node, std::sync::Arc<parts::Mesh>)> = p
        .nodes
        .iter()
        .filter(|n| !n.deleted)
        .map(|n| (n, cache.pieza(n, 2)))
        .filter(|(_, m)| !m.is_empty())
        .collect();
    // The base goes to z = 0: in glTF height becomes Y through the root rotation, and 2 500 m of
    // elevation would leave the model floating 2.5 km up in an external viewer.
    let z0 = lowest_elevation(&fixed, &pieces);

    let mut by_family: BTreeMap<&str, Vec<&Triangulo>> = BTreeMap::new();
    for t in &fixed.tris {
        by_family
            .entry(fixed.familias[t.familia as usize].as_str())
            .or_default()
            .push(t);
    }
    let mut glb = GlbDocument::default();
    let root_primitives: Vec<GlbPrimitive> = by_family
        .into_iter()
        .map(|(family, tris)| glb.primitive(family, Source::Triangles(tris, z0)))
        .collect();
    if !root_primitives.is_empty() {
        glb.root_mesh = Some(glb.add_mesh("redes", root_primitives));
    }

    let mut mesh_of: std::collections::HashMap<usize, usize> = Default::default();
    for (n, mesh) in &pieces {
        let key = std::sync::Arc::as_ptr(mesh) as usize;
        let index = match mesh_of.get(&key) {
            Some(i) => *i,
            None => {
                let primitive = glb.primitive(&n.family, Source::Piece(mesh));
                let i = glb.add_mesh(&n.family, vec![primitive]);
                mesh_of.insert(key, i);
                i
            }
        };
        glb.nodes.push(GlbNode {
            name: n.code.clone(),
            mesh: index,
            translation: [
                (n.pos.x - p.origin.x) as f32,
                (n.pos.y - p.origin.y) as f32,
                (n.z_invert - z0) as f32,
            ],
        });
    }
    glb.write()
}

fn lowest_elevation(
    fixed: &Malla,
    pieces: &[(&crate::model::Node, std::sync::Arc<parts::Mesh>)],
) -> f64 {
    let fixed_min = fixed
        .tris
        .iter()
        .flat_map(|t| t.pos.iter().map(|v| v[2] as f64))
        .fold(f64::INFINITY, f64::min);
    let pieces_min = pieces
        .iter()
        .map(|(n, m)| n.z_invert + m.bbox().0[2] as f64)
        .fold(f64::INFINITY, f64::min);
    let z = fixed_min.min(pieces_min);
    if z.is_finite() { z } else { 0.0 }
}

fn normal_de(tri: [[f32; 3]; 3]) -> [f32; 3] {
    let u = [
        tri[1][0] - tri[0][0],
        tri[1][1] - tri[0][1],
        tri[1][2] - tri[0][2],
    ];
    let v = [
        tri[2][0] - tri[0][0],
        tri[2][1] - tri[0][1],
        tri[2][2] - tri[0][2],
    ];
    let n = [
        u[1] * v[2] - u[2] * v[1],
        u[2] * v[0] - u[0] * v[2],
        u[0] * v[1] - u[1] * v[0],
    ];
    let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-9);
    [n[0] / l, n[1] / l, n[2] / l]
}

/// Construye un `.glb` a partir de vértices ya agrupados por material/familia.
///
/// Lo usa el importador de la biblioteca para reescribir un modelo ya normalizado (Y-arriba,
/// origen en la base, escala) sin volver a pasar por el proyecto.
pub fn glb_desde_vertices(
    por_familia: &BTreeMap<String, Vec<Vertice>>,
) -> Result<Vec<u8>, GlbTooLarge> {
    let mut glb = GlbDocument::default();
    let primitives = por_familia
        .iter()
        .filter(|(_, verts)| !verts.is_empty())
        .map(|(family, verts)| glb.primitive(family, Source::Vertices(verts)))
        .collect();
    glb.root_mesh = Some(glb.add_mesh("redes", primitives));
    glb.write()
}

/// Where a primitive takes its vertices from. Writing reads the source again instead of keeping a
/// copy: the `.glb` is written once, with no intermediate buffers.
enum Source<'a> {
    Vertices(&'a [Vertice]),
    /// Static triangles, lowered by `z0`.
    Triangles(Vec<&'a Triangulo>, f64),
    /// A piece mesh in its local frame; its node carries the translation.
    Piece(&'a parts::Mesh),
}

impl Source<'_> {
    fn vertex_count(&self) -> usize {
        match self {
            Source::Vertices(v) => v.len(),
            Source::Triangles(t, _) => t.len() * 3,
            Source::Piece(m) => m.tris.len() * 3,
        }
    }

    fn for_each_vertex(&self, mut f: impl FnMut(&Vertice)) {
        match self {
            Source::Vertices(vertices) => vertices.iter().for_each(f),
            Source::Triangles(tris, z0) => {
                for t in tris {
                    for pos in t.pos {
                        let lowered = [pos[0], pos[1], (pos[2] as f64 - z0) as f32];
                        f(&Vertice {
                            pos: lowered,
                            normal: t.normal,
                            color: t.color,
                        });
                    }
                }
            }
            Source::Piece(mesh) => {
                for t in &mesh.tris {
                    let normal = normal_de(t.p);
                    let color = [t.color[0], t.color[1], t.color[2]];
                    for pos in t.p {
                        f(&Vertice { pos, normal, color });
                    }
                }
            }
        }
    }
}

struct GlbPrimitive<'a> {
    material: usize,
    source: Source<'a>,
}

struct GlbMesh<'a> {
    name: String,
    primitives: Vec<GlbPrimitive<'a>>,
}

struct GlbNode {
    name: String,
    mesh: usize,
    translation: [f32; 3],
}

struct GlbMaterial {
    name: String,
    colour_sum: [f64; 3],
    vertices: usize,
}

/// A `.glb` under construction: the root node (Z-up to Y-up, with the optional root mesh) and its
/// child nodes.
#[derive(Default)]
struct GlbDocument<'a> {
    materials: Vec<GlbMaterial>,
    material_of: BTreeMap<String, usize>,
    meshes: Vec<GlbMesh<'a>>,
    root_mesh: Option<usize>,
    nodes: Vec<GlbNode>,
}

const GLB_HEADER_BYTES: u64 = 12 + 8 + 8;

/// GLB lengths are `u32`: a larger file cannot be written.
fn check_glb_size(total: u64) -> Result<u32, GlbTooLarge> {
    u32::try_from(total).map_err(|_| GlbTooLarge { bytes: total })
}

impl<'a> GlbDocument<'a> {
    fn primitive(&mut self, family: &str, source: Source<'a>) -> GlbPrimitive<'a> {
        let material = match self.material_of.get(family) {
            Some(m) => *m,
            None => {
                self.materials.push(GlbMaterial {
                    name: family.to_string(),
                    colour_sum: [0.0; 3],
                    vertices: 0,
                });
                self.material_of
                    .insert(family.to_string(), self.materials.len() - 1);
                self.materials.len() - 1
            }
        };
        GlbPrimitive { material, source }
    }

    fn add_mesh(&mut self, name: &str, primitives: Vec<GlbPrimitive<'a>>) -> usize {
        self.meshes.push(GlbMesh {
            name: name.to_string(),
            primitives,
        });
        self.meshes.len() - 1
    }

    /// Lays out the binary chunk, writes the JSON and then streams every vertex straight into the
    /// output: one copy of the geometry instead of three.
    fn write(mut self) -> Result<Vec<u8>, GlbTooLarge> {
        let mut views = Vec::new();
        let mut accessors = Vec::new();
        let mut meshes = Vec::new();
        let mut bin_len = 0u64;
        for mesh in &self.meshes {
            let mut primitives = Vec::new();
            for prim in &mesh.primitives {
                let count = prim.source.vertex_count();
                let (min, max) =
                    bounds_and_colour(&prim.source, &mut self.materials[prim.material]);
                let base = accessors.len();
                for (k, extra) in [Some((min, max)), None, None].into_iter().enumerate() {
                    let bytes = count as u64 * 12;
                    views.push(format!(
                        r#"{{"buffer":0,"byteOffset":{bin_len},"byteLength":{bytes},"target":34962}}"#
                    ));
                    bin_len += bytes;
                    let range = extra
                        .map(|(mn, mx)| {
                            format!(
                                r#","min":[{},{},{}],"max":[{},{},{}]"#,
                                mn[0], mn[1], mn[2], mx[0], mx[1], mx[2]
                            )
                        })
                        .unwrap_or_default();
                    accessors.push(format!(
                        r#"{{"bufferView":{},"componentType":5126,"count":{count},"type":"VEC3"{range}}}"#,
                        base + k
                    ));
                }
                primitives.push(format!(
                    r#"{{"attributes":{{"POSITION":{base},"NORMAL":{},"COLOR_0":{}}},"material":{},"mode":4}}"#,
                    base + 1,
                    base + 2,
                    prim.material
                ));
            }
            meshes.push(format!(
                r#"{{"name":{},"primitives":[{}]}}"#,
                json_string(&mesh.name),
                primitives.join(",")
            ));
        }
        let mut json = self.json(&meshes, &accessors, &views, bin_len).into_bytes();
        while !json.len().is_multiple_of(4) {
            json.push(b' ');
        }
        let total = check_glb_size(GLB_HEADER_BYTES + json.len() as u64 + bin_len)?;

        let mut out = Vec::with_capacity(total as usize);
        out.extend_from_slice(&0x4654_6C67u32.to_le_bytes()); // "glTF"
        out.extend_from_slice(&2u32.to_le_bytes());
        out.extend_from_slice(&total.to_le_bytes());
        out.extend_from_slice(&(json.len() as u32).to_le_bytes());
        out.extend_from_slice(&0x4E4F_534Au32.to_le_bytes()); // "JSON"
        out.extend_from_slice(&json);
        out.extend_from_slice(&(bin_len as u32).to_le_bytes());
        out.extend_from_slice(&0x004E_4942u32.to_le_bytes()); // "BIN\0"
        for prim in self.meshes.iter().flat_map(|m| &m.primitives) {
            let attributes: [fn(&Vertice) -> [f32; 3]; 3] = [|v| v.pos, |v| v.normal, |v| v.color];
            for attribute in attributes {
                prim.source.for_each_vertex(|v| {
                    for c in attribute(v) {
                        out.extend_from_slice(&c.to_le_bytes());
                    }
                });
            }
        }
        debug_assert_eq!(out.len(), total as usize);
        Ok(out)
    }

    fn json(
        &self,
        meshes: &[String],
        accessors: &[String],
        views: &[String],
        bin_len: u64,
    ) -> String {
        let mut root = r#"{"name":"Redes""#.to_string();
        if let Some(mesh) = self.root_mesh {
            root.push_str(&format!(r#","mesh":{mesh}"#));
        }
        root.push_str(r#","rotation":[-0.7071068,0.0,0.0,0.7071068]"#);
        if !self.nodes.is_empty() {
            let children: Vec<String> = (1..=self.nodes.len()).map(|i| i.to_string()).collect();
            root.push_str(&format!(r#","children":[{}]"#, children.join(",")));
        }
        root.push('}');
        let nodes = std::iter::once(root).chain(self.nodes.iter().map(|n| {
            let [x, y, z] = n.translation;
            format!(
                r#"{{"name":{},"mesh":{},"translation":[{x},{y},{z}]}}"#,
                json_string(&n.name),
                n.mesh
            )
        }));
        let materials = self.materials.iter().map(|m| {
            let c = m.colour_sum.map(|s| (s / m.vertices.max(1) as f64) as f32);
            format!(
                r#"{{"name":{},"pbrMetallicRoughness":{{"baseColorFactor":[{},{},{},1.0],"metallicFactor":0.0,"roughnessFactor":0.9}},"doubleSided":true,"extensions":{{"KHR_materials_unlit":{{}}}}}}"#,
                json_string(&m.name),
                c[0],
                c[1],
                c[2]
            )
        });
        format!(
            r#"{{"asset":{{"version":"2.0","generator":"Redes"}},"extensionsUsed":["KHR_materials_unlit"],"scene":0,"scenes":[{{"nodes":[0]}}],"nodes":[{}],"meshes":[{}],"materials":[{}],"accessors":[{}],"bufferViews":[{}],"buffers":[{{"byteLength":{bin_len}}}]}}"#,
            nodes.collect::<Vec<_>>().join(","),
            meshes.join(","),
            materials.collect::<Vec<_>>().join(","),
            accessors.join(","),
            views.join(","),
        )
    }
}

fn bounds_and_colour(source: &Source<'_>, material: &mut GlbMaterial) -> ([f32; 3], [f32; 3]) {
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    source.for_each_vertex(|v| {
        for k in 0..3 {
            min[k] = min[k].min(v.pos[k]);
            max[k] = max[k].max(v.pos[k]);
            material.colour_sum[k] += v.color[k] as f64;
        }
        material.vertices += 1;
    });
    (min, max)
}

fn json_string(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| "\"\"".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leer_json(glb: &[u8]) -> String {
        let largo_json = u32::from_le_bytes([glb[12], glb[13], glb[14], glb[15]]) as usize;
        String::from_utf8(glb[20..20 + largo_json].to_vec()).unwrap()
    }

    #[test]
    fn el_proyecto_guardado_conserva_el_entorno() {
        let p = crate::demo_city::ciudad();
        let ruta = std::env::temp_dir().join(format!("redes-entorno-{}.redes", std::process::id()));
        crate::project_io::save(&p, &ruta).expect("guardar");
        let q = crate::project_io::load(&ruta).expect("cargar");
        let n = q.entorno.as_ref().map(|e| e.calles.len()).unwrap_or(0);
        assert!(n >= 10, "el archivo perdió las calles: {n}");
        let m = triangulos_exportables(&q, false);
        assert!(
            m.familias
                .iter()
                .any(|f| f.starts_with("Av.") || f.starts_with("Calle")),
            "sin calles en la malla: {:?}",
            m.familias
        );
        let _ = std::fs::remove_file(&ruta);
    }

    #[test]
    fn la_ciudad_exporta_calles_y_edificios() {
        let p = crate::demo_city::ciudad();
        let malla = triangulos_exportables(&p, false);
        let hay_calle = malla
            .familias
            .iter()
            .any(|f| f.starts_with("Av.") || f.starts_with("Calle"));
        let hay_cubierta = malla.familias.iter().any(|f| f == "cubierta");
        assert!(hay_calle, "faltan calles: {:?}", malla.familias);
        assert!(hay_cubierta, "faltan cubiertas: {:?}", malla.familias);
    }

    #[test]
    fn el_glb_tiene_cabecera_y_json_validos() {
        let p = crate::demo_catalog::catalogo();
        let glb = exportar_glb(&p, OpcionesGltf::default()).expect("fits");
        assert_eq!(&glb[0..4], b"glTF", "magic");
        assert_eq!(
            u32::from_le_bytes([glb[4], glb[5], glb[6], glb[7]]),
            2,
            "versión"
        );
        let total = u32::from_le_bytes([glb[8], glb[9], glb[10], glb[11]]) as usize;
        assert_eq!(total, glb.len(), "el largo declarado debe coincidir");

        let json = leer_json(&glb);
        assert!(json.contains(r#""version":"2.0""#));
        assert!(json.contains(r#""primitives""#));
        // Al menos una primitiva por familia del catálogo.
        let primitivas = json.matches(r#""mode":4"#).count();
        assert!(primitivas >= 10, "primitivas: {primitivas}");
        assert!(json.contains("KHR_materials_unlit"));
    }

    #[test]
    fn las_coordenadas_estan_en_metros_y_dentro_del_modelo() {
        let p = crate::demo_city::ciudad();
        let glb = exportar_glb(&p, OpcionesGltf { con_terreno: false }).expect("fits");
        let json = leer_json(&glb);
        // El primer accessor POSITION lleva min/max: deben ser coordenadas locales en metros.
        let inicio = json.find(r#""min":["#).expect("min del POSITION");
        let trozo = &json[inicio..(inicio + 160).min(json.len())];
        let nums: Vec<f32> = trozo
            .split(|c: char| !(c.is_ascii_digit() || c == '-' || c == '.'))
            .filter(|s| !s.is_empty() && *s != "-" && *s != ".")
            .filter_map(|s| s.parse().ok())
            .collect();
        for v in nums.iter().take(6) {
            assert!(v.abs() < 20_000.0, "coordenada fuera de rango: {v}");
        }
    }

    #[test]
    fn el_documento_serializado_pesa_lo_razonable() {
        let p = crate::demo_catalog::catalogo();
        let glb = exportar_glb(&p, OpcionesGltf::default()).expect("fits");
        assert!(
            glb.len() > 10_000,
            "un catálogo no puede pesar {} bytes",
            glb.len()
        );
        assert!(glb.len() < 40_000_000, "demasiado grande: {}", glb.len());
    }

    fn json_of(glb: &[u8]) -> serde_json::Value {
        serde_json::from_str(&leer_json(glb)).expect("valid JSON")
    }

    /// Nodes with the same piece point at one shared mesh, and the file still holds every
    /// triangle of the project once the node translations are applied (F14-05).
    #[test]
    fn equal_pieces_share_one_mesh_and_keep_every_triangle() {
        let mut p = crate::demo_city::ciudad();
        let manhole = p
            .nodes
            .iter()
            .find(|n| n.family == "saneamiento.pozo")
            .expect("manhole")
            .clone();
        let copies = 6;
        for k in 1..=copies {
            let mut copy = manhole.clone();
            copy.code = format!("{}-{k}", manhole.code);
            copy.pos.x += 3.0 * k as f64;
            p.nodes.push(copy);
        }
        let glb = exportar_glb(&p, OpcionesGltf::default()).expect("fits");
        let doc = json_of(&glb);
        let meshes = doc["meshes"].as_array().expect("meshes").len();
        let piece_nodes = doc["nodes"].as_array().expect("nodes").len() - 1;
        assert_eq!(piece_nodes, p.nodes.iter().filter(|n| !n.deleted).count());
        assert!(
            meshes + copies <= piece_nodes + 1,
            "{meshes} meshes for {piece_nodes} pieces"
        );

        let read = crate::biblioteca::leer_glb(&glb).expect("readable");
        let exported = triangulos_exportables(&p, true).tris.len();
        assert_eq!(read.triangulos, exported);
    }

    #[test]
    fn a_glb_over_4_gib_is_an_error() {
        assert_eq!(check_glb_size(1_000), Ok(1_000));
        assert_eq!(check_glb_size(u32::MAX as u64), Ok(u32::MAX));
        let too_large = check_glb_size(u32::MAX as u64 + 1).expect_err("over 4 GiB");
        assert!(too_large.to_string().contains("4 096 MB"), "{too_large}");
    }
}
