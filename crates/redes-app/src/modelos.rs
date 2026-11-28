//! Modelos de la biblioteca en el visor: caché de mallas y colocación en el proyecto.
//!
//! Un objeto con `modelo` asignado se dibuja con ese `.glb` en lugar de la pieza paramétrica, en el
//! pase 3D y en el pintor (D14). La malla se carga **una vez** por pieza y se reutiliza en todas
//! las instancias.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use redes_core::bake::model_placement;
use redes_core::biblioteca::{Biblioteca, Entrada};
use redes_core::gltf::Vertice;
use redes_core::model::{ElementId, Node, Project};
use redes_core::parts::{Material, Mesh, Parte};
use redes_core::pipe_layout::{PipePieces, PipeSegment};
use redes_core::view::{ModelPiece, ModelPieces};

/// Malla de una pieza, ya en el espacio del proyecto (Z arriba, metros).
pub type Malla = Arc<Vec<Vertice>>;

/// Caché de mallas por identificador de pieza.
#[derive(Default)]
pub struct Cache {
    mallas: HashMap<String, Malla>,
    fallos: HashSet<String>,
    cargas: usize,
    /// Proxies por pieza y paso de rejilla (bits del `f32`): se calculan una vez (F14-06).
    proxies: HashMap<(String, u32), Malla>,
    proxies_calculados: usize,
    /// Shaded pieces for the painter, by piece id (and proxy step, 0 for the full mesh).
    partes: HashMap<(String, u32), Arc<Parte>>,
    /// Baked pipe segments by piece id; `None` if the `.glb` is missing or is not a segment.
    segments: HashMap<String, Option<Arc<PipeSegment>>>,
}

impl Cache {
    /// The pipe segment of a piece, read once (F17-04).
    pub fn segment(&mut self, carpeta: &Path, pieza: &Entrada) -> Option<Arc<PipeSegment>> {
        self.segments
            .entry(pieza.id.clone())
            .or_insert_with(|| {
                let bytes = std::fs::read(carpeta.join(&pieza.archivo)).ok()?;
                PipeSegment::from_glb(&bytes).ok().map(Arc::new)
            })
            .clone()
    }

    /// Proxy de la malla de una pieza (ver [`proxy`]), calculado una sola vez por pieza y paso.
    pub fn proxy(&mut self, carpeta: &Path, pieza: &Entrada, paso: f32) -> Option<Malla> {
        let clave = (pieza.id.clone(), paso.to_bits());
        if let Some(m) = self.proxies.get(&clave) {
            return Some(Arc::clone(m));
        }
        let malla = self.malla(carpeta, pieza)?;
        let dec = Arc::new(proxy(&malla, paso));
        self.proxies_calculados += 1;
        self.proxies.insert(clave, Arc::clone(&dec));
        Some(dec)
    }

    /// The full mesh of a piece as a shaded painter piece, computed once per piece.
    pub fn parte(&mut self, pieza: &Entrada, malla: &[Vertice]) -> Option<Arc<Parte>> {
        let clave = (pieza.id.clone(), 0);
        Some(Arc::clone(
            self.partes
                .entry(clave)
                .or_insert_with(|| Arc::new(parte_de(malla))),
        ))
    }

    /// The proxy of a piece as a shaded painter piece, computed once per piece and step.
    pub fn parte_proxy(
        &mut self,
        carpeta: &Path,
        pieza: &Entrada,
        paso: f32,
    ) -> Option<Arc<Parte>> {
        let clave = (pieza.id.clone(), paso.to_bits());
        if let Some(p) = self.partes.get(&clave) {
            return Some(Arc::clone(p));
        }
        let proxy = self.proxy(carpeta, pieza, paso)?;
        let parte = Arc::new(parte_de(&proxy));
        self.partes.insert(clave, Arc::clone(&parte));
        Some(parte)
    }

    /// Cuántos proxies se han calculado (para las pruebas).
    #[allow(dead_code)]
    pub fn proxies_calculados(&self) -> usize {
        self.proxies_calculados
    }

    /// Carga (o devuelve de la caché) la malla de una pieza del manifiesto.
    pub fn malla(&mut self, carpeta: &Path, pieza: &Entrada) -> Option<Malla> {
        if let Some(m) = self.mallas.get(&pieza.id) {
            return Some(m.clone());
        }
        if self.fallos.contains(&pieza.id) {
            return None;
        }
        let ruta = carpeta.join(&pieza.archivo);
        let bytes = match std::fs::read(&ruta) {
            Ok(b) => b,
            Err(_) => {
                self.fallos.insert(pieza.id.clone());
                return None;
            }
        };
        let malla = match malla_de_glb(&bytes) {
            Some(m) => m,
            None => {
                self.fallos.insert(pieza.id.clone());
                return None;
            }
        };
        self.cargas += 1;
        let malla = Arc::new(malla);
        self.mallas.insert(pieza.id.clone(), malla.clone());
        Some(malla)
    }

    /// Cuántas veces se ha leído un `.glb` (para las pruebas).
    #[allow(dead_code)]
    pub fn cargas(&self) -> usize {
        self.cargas
    }
}

/// Vertices of a `.glb` ready for the 3D pass: Z up (as the core reads it), metres, one normal per
/// face so the model is lit like the parametric pieces.
///
/// The core reader already turns glTF's Y up into Z up; swapping the axes again here laid every
/// Blender piece on its side (F17-03).
pub fn malla_de_glb(bytes: &[u8]) -> Option<Vec<Vertice>> {
    let tris = redes_core::biblioteca::triangulos_de_glb(bytes).ok()?;
    let mut out = Vec::with_capacity(tris.len() * 3);
    for t in &tris {
        let pos = t.pos.map(|q| [q[0] as f32, q[1] as f32, q[2] as f32]);
        let normal = face_normal(pos);
        for q in pos {
            out.push(Vertice {
                pos: q,
                normal,
                color: t.color.unwrap_or(COLOR_SIN_MATERIAL),
            });
        }
    }
    Some(out)
}

/// Colour of a model triangle that has neither vertex colour nor material.
const COLOR_SIN_MATERIAL: [f32; 3] = [0.62, 0.60, 0.56];

fn face_normal(p: [[f32; 3]; 3]) -> [f32; 3] {
    let u = [p[1][0] - p[0][0], p[1][1] - p[0][1], p[1][2] - p[0][2]];
    let v = [p[2][0] - p[0][0], p[2][1] - p[0][1], p[2][2] - p[0][2]];
    let n = [
        u[1] * v[2] - u[2] * v[1],
        u[2] * v[0] - u[0] * v[2],
        u[0] * v[1] - u[1] * v[0],
    ];
    let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    if l > 0.0 {
        [n[0] / l, n[1] / l, n[2] / l]
    } else {
        [0.0, 0.0, 1.0]
    }
}

/// Una instancia colocada en el proyecto.
pub struct Instancia {
    pub pos: [f32; 3],
    pub malla: Malla,
    /// Giro alrededor de Z, en grados (0 si el objeto no lo declara).
    pub giro: f32,
    /// The object it draws: its parametric piece is not drawn as well.
    pub nodo: ElementId,
}

/// Manifiesto del proyecto, si lo tiene declarado.
pub fn biblioteca_del_proyecto(p: &Project, dir: &Path) -> Option<(PathBuf, Arc<Biblioteca>)> {
    let ruta = p.biblioteca.as_ref()?;
    let base = dir.join(ruta);
    let carpeta = base
        .parent()
        .map(|d| d.to_path_buf())
        .unwrap_or_else(|| dir.to_path_buf());
    Biblioteca::cargar(&base)
        .ok()
        .map(|b| (carpeta, Arc::new(b)))
}

/// An object with a model it can draw: the manifest entry and the loaded mesh.
struct Modelado<'a> {
    nodo: &'a Node,
    pieza: &'a Entrada,
    malla: Malla,
}

fn modelados<'a>(
    p: &'a Project,
    carpeta: &Path,
    bib: &'a Biblioteca,
    cache: &mut Cache,
) -> Vec<Modelado<'a>> {
    let mut out = Vec::new();
    for n in p.nodes.iter().filter(|n| !n.deleted) {
        let Some(id) = n.modelo.as_ref() else {
            continue;
        };
        let Some(pieza) = bib.piezas.iter().find(|x| &x.id == id) else {
            continue;
        };
        // A missing or broken `.glb` keeps the parametric piece (docs/17 section 11.4).
        let Some(malla) = cache.malla(carpeta, pieza) else {
            continue;
        };
        out.push(Modelado {
            nodo: n,
            pieza,
            malla,
        });
    }
    out
}

/// Resolves which mesh each object draws and places it: in plan at the object, turned and in
/// height as [`model_placement`] says (a fitting turns with its pipes, docs/familias/F17-05).
pub fn instancias(
    p: &Project,
    carpeta: &Path,
    bib: &Biblioteca,
    cache: &mut Cache,
) -> Vec<Instancia> {
    modelados(p, carpeta, bib, cache)
        .into_iter()
        .map(|m| {
            let at = model_placement(p, m.nodo, m.pieza);
            Instancia {
                // The origin is subtracted in f64 and only then lowered to f32 (docs/15).
                pos: [
                    (m.nodo.pos.x - p.origin.x) as f32,
                    (m.nodo.pos.y - p.origin.y) as f32,
                    at.base_m as f32,
                ],
                malla: m.malla,
                giro: at.rotation_deg as f32,
                nodo: m.nodo.id,
            }
        })
        .collect()
}

/// Models over this many triangles are drawn by their proxy in the painter (no `--gpu3d`):
/// the painter sorts every triangle in every frame.
pub const TRIANGULOS_PINTOR: usize = 20_000;

/// The models the painter draws in place of the parametric pieces, and how many of them had to
/// be simplified to their proxy.
pub fn piezas_pintor(
    p: &Project,
    carpeta: &Path,
    bib: &Biblioteca,
    cache: &mut Cache,
    paso_proxy: f32,
) -> (ModelPieces, usize) {
    let mut out = ModelPieces::new();
    let mut simplificados = 0;
    for m in modelados(p, carpeta, bib, cache) {
        let pesado = m.malla.len() / 3 > TRIANGULOS_PINTOR;
        let parte = if pesado {
            simplificados += 1;
            cache.parte_proxy(carpeta, m.pieza, paso_proxy)
        } else {
            cache.parte(m.pieza, &m.malla)
        };
        let Some(parte) = parte else { continue };
        let at = model_placement(p, m.nodo, m.pieza);
        out.insert(
            m.nodo.id,
            ModelPiece {
                part: parte,
                lift_m: at.base_m - m.nodo.z_invert,
                rotation_deg: at.rotation_deg as f32,
            },
        );
    }
    (out, simplificados)
}

/// The links drawn with a baked segment and the anchors of the nodes drawn with a model, where
/// every pipe starts and ends (F17-04). A missing or broken `.glb` keeps the parametric pipe.
pub fn tuberias(p: &Project, carpeta: &Path, bib: &Biblioteca, cache: &mut Cache) -> PipePieces {
    let mut out = PipePieces::default();
    for m in modelados(p, carpeta, bib, cache) {
        if !m.pieza.anclas.is_empty() {
            out.node_anchors.insert(m.nodo.id, m.pieza.anclas.clone());
        }
    }
    for l in p.links.iter().filter(|l| !l.deleted) {
        let Some(pieza) = l
            .modelo
            .as_ref()
            .and_then(|id| bib.piezas.iter().find(|x| &x.id == id))
        else {
            continue;
        };
        if let Some(segment) = cache.segment(carpeta, pieza) {
            out.by_link.insert(l.id, segment);
        }
    }
    out
}

/// A mesh as one shaded piece for the painter.
fn parte_de(malla: &[Vertice]) -> Parte {
    let mut mesh = Mesh::default();
    for t in malla.chunks_exact(3) {
        let material = Material {
            color: t[0].color,
            metal: false,
        };
        mesh.tri(t[0].pos, t[1].pos, t[2].pos, material);
    }
    Parte {
        nombre: "modelo",
        mesh,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn glb_de_un_cubo() -> Vec<u8> {
        // 2 m cube with its base at z = 0, Z up.
        let v = [
            [-1.0f32, 0.0, -1.0],
            [1.0, 0.0, -1.0],
            [1.0, 0.0, 1.0],
            [-1.0, 0.0, 1.0],
            [-1.0, 2.0, -1.0],
            [1.0, 2.0, -1.0],
            [1.0, 2.0, 1.0],
            [-1.0, 2.0, 1.0],
        ];
        let caras = [
            (0, 1, 2),
            (0, 2, 3),
            (4, 6, 5),
            (4, 7, 6),
            (0, 4, 5),
            (0, 5, 1),
            (1, 5, 6),
            (1, 6, 2),
            (2, 6, 7),
            (2, 7, 3),
            (3, 7, 4),
            (3, 4, 0),
        ];
        let mut verts = Vec::new();
        for (a, b, c) in caras {
            for i in [a, b, c] {
                verts.push(Vertice {
                    pos: [v[i][0], v[i][2], v[i][1]],
                    normal: [0.0, 0.0, 1.0],
                    color: [0.7, 0.6, 0.5],
                });
            }
        }
        redes_core::gltf::glb_desde_vertices(&std::collections::BTreeMap::from([(
            "modelo".to_string(),
            verts,
        )]))
        .expect("fits in a .glb")
    }

    fn proyecto_con_dos_valvulas() -> Project {
        let mut p = Project::new("modelos", "EPSG:32717");
        for (i, code) in ["VL-1", "VL-2"].iter().enumerate() {
            let mut n = redes_core::model::Node {
                id: redes_core::model::ElementId::new(0, 0),
                family: "agua.valvula".into(),
                ty: "compuerta".into(),
                code: (*code).into(),
                pos: redes_core::geom::P2::new(10.0 * i as f64, 0.0),
                z_terrain: 2500.0,
                z_rim: 2501.0,
                z_invert: 2500.0,
                attrs: Default::default(),
                datos: Default::default(),
                modelo: Some("cubo".into()),
                estado: redes_core::model::EstadoObjeto::Pendiente,
                observaciones: Vec::new(),
                deleted: false,
                sin_dato: Vec::new(),
            };
            n.modelo = Some("cubo".into());
            p.add_node(n).expect("nodo");
        }
        p
    }

    fn biblioteca_temporal(dir: &Path) -> Biblioteca {
        let pieza = dir.join("biblioteca").join("cubo.glb");
        std::fs::create_dir_all(pieza.parent().unwrap()).unwrap();
        std::fs::write(&pieza, glb_de_un_cubo()).unwrap();
        Biblioteca {
            schema: "redes.biblioteca/1".into(),
            piezas: vec![Entrada {
                id: "cubo".into(),
                familia: "agua.valvula".into(),
                subtipo: Some("compuerta".into()),
                dn_mm: None,
                archivo: "biblioteca/cubo.glb".into(),
                escala: 1.0,
                triangulos: 12,
                dimensiones_m: [2.0, 2.0, 2.0],
                bytes: 512,
                volumen_m3: Some(8.0),
                area_m2: Some(24.0),
                cerrada: Some(true),
                anclas: Default::default(),
                extras: Default::default(),
                licencia: None,
                autor: None,
            }],
        }
    }

    #[test]
    fn una_pieza_se_carga_una_sola_vez_para_varios_objetos() {
        let dir = std::env::temp_dir().join(format!("redes-modelos-{}", std::process::id()));
        let bib = biblioteca_temporal(&dir);
        let p = proyecto_con_dos_valvulas();
        let mut cache = Cache::default();
        let inst = instancias(&p, &dir, &bib, &mut cache);
        assert_eq!(inst.len(), 2, "dos objetos con el mismo modelo");
        assert_eq!(cache.cargas(), 1, "el .glb se lee una vez");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// El proxy se calcula una vez por pieza: dos objetos y dos reconstrucciones no lo repiten.
    #[test]
    fn el_proxy_se_calcula_una_vez_por_pieza() {
        let dir = std::env::temp_dir().join(format!("redes-modelos4-{}", std::process::id()));
        let bib = biblioteca_temporal(&dir);
        let mut cache = Cache::default();
        let a = cache.proxy(&dir, &bib.piezas[0], 0.05).expect("proxy");
        for _ in 0..3 {
            let b = cache.proxy(&dir, &bib.piezas[0], 0.05).expect("proxy");
            assert!(Arc::ptr_eq(&a, &b), "el mismo proxy, sin recalcular");
        }
        assert_eq!(cache.proxies_calculados(), 1);
        assert_eq!(cache.cargas(), 1, "el .glb se leyó una vez");
        cache.proxy(&dir, &bib.piezas[0], 0.25).expect("otro paso");
        assert_eq!(cache.proxies_calculados(), 2, "otro paso es otro proxy");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn la_instancia_usa_coordenadas_locales() {
        let dir = std::env::temp_dir().join(format!("redes-modelos3-{}", std::process::id()));
        let bib = biblioteca_temporal(&dir);
        let mut p = proyecto_con_dos_valvulas();
        // Los nodos se colocan en coordenadas absolutas, como en un proyecto real.
        for n in p.nodes.iter_mut() {
            n.pos.x += 758_000.0;
            n.pos.y += 9_797_000.0;
        }
        p.origin = redes_core::geom::P2::new(758_000.0, 9_797_000.0);
        let mut cache = Cache::default();
        let inst = instancias(&p, &dir, &bib, &mut cache);
        assert!(
            (inst[0].pos[0] - 0.0).abs() < 1e-3,
            "x local = {}",
            inst[0].pos[0]
        );
        assert!(
            (inst[1].pos[0] - 10.0).abs() < 1e-3,
            "x local = {}",
            inst[1].pos[0]
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn el_modelo_se_coloca_en_la_cota_del_objeto() {
        let dir = std::env::temp_dir().join(format!("redes-modelos2-{}", std::process::id()));
        let bib = biblioteca_temporal(&dir);
        let p = proyecto_con_dos_valvulas();
        let mut cache = Cache::default();
        let inst = instancias(&p, &dir, &bib, &mut cache);
        let primero = &inst[0];
        assert!((primero.pos[2] - 2501.0).abs() < 1e-3, "cota de tapa");
        // La malla tiene la base en 0 y 2 m de alto: en Z del mundo debe llegar a 2.
        let alto = primero
            .malla
            .iter()
            .map(|v| v.pos[2])
            .fold(f32::MIN, f32::max);
        assert!((alto - 2.0).abs() < 1e-3, "alto del modelo = {alto}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_piece_with_an_invert_anchor_sits_on_the_invert_minus_its_slab() {
        let dir = std::env::temp_dir().join(format!("redes-modelos5-{}", std::process::id()));
        let mut bib = biblioteca_temporal(&dir);
        bib.piezas[0].anclas = std::collections::BTreeMap::from([(
            redes_core::biblioteca::ANCLA_SOLERA.to_string(),
            [0.0f32, 0.0, 0.2],
        )]);
        let p = proyecto_con_dos_valvulas();
        let mut cache = Cache::default();
        let inst = instancias(&p, &dir, &bib, &mut cache);
        assert!(
            (inst[0].pos[2] - 2499.8).abs() < 1e-3,
            "invert 2500 - slab 0.2"
        );
        assert_eq!(inst[0].nodo, p.nodes[0].id, "it replaces that object");
        let (painter, simplified) = piezas_pintor(&p, &dir, &bib, &mut cache, 0.05);
        assert_eq!(simplified, 0);
        let piece = &painter[&p.nodes[0].id];
        assert!((piece.lift_m + 0.2).abs() < 1e-6, "lift {}", piece.lift_m);
        assert_eq!(
            piece.part.mesh.tris.len(),
            12,
            "the whole cube, not a proxy"
        );
        let again = piezas_pintor(&p, &dir, &bib, &mut cache, 0.05).0;
        assert!(
            Arc::ptr_eq(&piece.part, &again[&p.nodes[0].id].part),
            "shaded once"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_z_up_mesh_comes_back_z_up() {
        // Written Z up (the writer turns it to glTF's Y up), read back Z up: no second swap.
        let glb = redes_core::gltf::glb_desde_vertices(&std::collections::BTreeMap::from([(
            "modelo".to_string(),
            vec![
                Vertice {
                    pos: [0.0, 0.0, 0.0],
                    normal: [0.0, 0.0, 1.0],
                    color: [1.0, 0.0, 0.0],
                },
                Vertice {
                    pos: [1.0, 0.0, 0.0],
                    normal: [0.0, 0.0, 1.0],
                    color: [1.0, 0.0, 0.0],
                },
                Vertice {
                    pos: [0.0, 0.0, 2.0],
                    normal: [0.0, 0.0, 1.0],
                    color: [1.0, 0.0, 0.0],
                },
            ],
        )]))
        .expect("fits in a .glb");
        let malla = malla_de_glb(&glb).expect("malla");
        assert_eq!(malla.len(), 3);
        let alto = malla.iter().map(|v| v.pos[2]).fold(f32::MIN, f32::max);
        assert!((alto - 2.0).abs() < 1e-3, "alto = {alto}");
    }
}

/// Proxy de una malla: la misma silueta con menos triángulos (F6-05).
///
/// Cuantiza los vértices a una rejilla de `paso` metros y descarta los triángulos que colapsan a
/// un punto. Sirve para ver la forma de un modelo pesado cuando no hay pase 3D; no sustituye a la
/// malla real para medir.
pub fn proxy(malla: &[Vertice], paso: f32) -> Vec<Vertice> {
    if paso <= 0.0 {
        return malla
            .iter()
            .map(|v| Vertice {
                pos: v.pos,
                normal: v.normal,
                color: v.color,
            })
            .collect();
    }
    let mut out = Vec::with_capacity(malla.len());
    for tri in malla.chunks_exact(3) {
        let q: Vec<[f32; 3]> = tri
            .iter()
            .map(|v| {
                [
                    (v.pos[0] / paso).round() * paso,
                    (v.pos[1] / paso).round() * paso,
                    (v.pos[2] / paso).round() * paso,
                ]
            })
            .collect();
        if q[0] == q[1] || q[1] == q[2] || q[0] == q[2] {
            continue;
        }
        for (i, v) in tri.iter().enumerate() {
            out.push(Vertice {
                pos: q[i],
                normal: v.normal,
                color: v.color,
            });
        }
    }
    out
}

#[cfg(test)]
mod tests_proxy {
    use super::*;

    fn triangulo(x: f32) -> Vertice {
        Vertice {
            pos: [x, 0.0, 0.0],
            normal: [0.0, 0.0, 1.0],
            color: [0.5, 0.5, 0.5],
        }
    }

    #[test]
    fn el_proxy_quita_triangulos_degenerados() {
        // Uno de ~90 cm de lado (sobrevive con paso 0,25) y otro de 2 cm (colapsa a un punto).
        let mut malla = vec![triangulo(0.0), triangulo(0.4), triangulo(0.9)];
        malla.extend([triangulo(1.0), triangulo(1.02), triangulo(1.01)]);
        let reducida = proxy(&malla, 0.25);
        assert_eq!(reducida.len(), 3, "solo sobrevive el triángulo grande");
        assert_eq!(proxy(&malla, 0.0).len(), malla.len(), "paso 0 no toca nada");
    }
}
