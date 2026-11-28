//! Modelos de la biblioteca en el visor: caché de mallas y colocación en el proyecto.
//!
//! Un objeto con `modelo` asignado se dibuja con ese `.glb` en lugar de (o junto a) la pieza
//! paramétrica. La malla se carga **una vez** por pieza y se reutiliza en todas las instancias.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use redes_core::biblioteca::{Biblioteca, Entrada};
use redes_core::gltf::Vertice;
use redes_core::model::Project;

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
}

impl Cache {
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

/// Convierte un `.glb` en vértices listos para el pase 3D, pasando de Y-arriba a Z-arriba.
pub fn malla_de_glb(bytes: &[u8]) -> Option<Vec<Vertice>> {
    let tris = redes_core::biblioteca::triangulos_de_glb(bytes).ok()?;
    let mut out = Vec::with_capacity(tris.len() * 3);
    for t in &tris {
        for q in &t.pos {
            out.push(Vertice {
                pos: [q[0] as f32, -(q[2] as f32), q[1] as f32],
                normal: [0.0, 0.0, 1.0],
                color: t.color.unwrap_or([0.62, 0.60, 0.56]),
            });
        }
    }
    Some(out)
}

/// Una instancia colocada en el proyecto.
pub struct Instancia {
    pub pos: [f32; 3],
    pub malla: Malla,
    /// Giro alrededor de Z, en grados (0 si el objeto no lo declara).
    pub giro: f32,
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

/// Resuelve qué malla toca a cada objeto y la coloca en su cota.
pub fn instancias(
    p: &Project,
    carpeta: &Path,
    bib: &Biblioteca,
    cache: &mut Cache,
) -> Vec<Instancia> {
    let mut out = Vec::new();
    for n in p.nodes.iter().filter(|n| !n.deleted) {
        let Some(id) = n.modelo.as_ref() else {
            continue;
        };
        let Some(pieza) = bib.piezas.iter().find(|x| &x.id == id) else {
            continue;
        };
        let Some(malla) = cache.malla(carpeta, pieza) else {
            continue;
        };
        let giro = n.num("giro").map(|v| v as f32).unwrap_or(0.0);
        // La geometría paramétrica se dibuja en coordenadas locales (origen restado) para no
        // perder precisión en f32: el modelo tiene que entrar en el mismo espacio.
        out.push(Instancia {
            pos: [
                (n.pos.x - p.origin.x) as f32,
                (n.pos.y - p.origin.y) as f32,
                n.z_rim as f32,
            ],
            malla,
            giro,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn glb_de_un_cubo() -> Vec<u8> {
        // Cubo de 2 m con base en 0, en espacio glTF (Y arriba).
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
                    pos: v[i],
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
    fn convierte_de_y_arriba_a_z_arriba() {
        // Un triángulo con y arriba: el eje Y del glb pasa a ser Z del mundo.
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
                    pos: [0.0, 2.0, 0.0],
                    normal: [0.0, 0.0, 1.0],
                    color: [1.0, 0.0, 0.0],
                },
            ],
        )]))
        .expect("fits in a .glb");
        let malla = malla_de_glb(&glb).expect("malla");
        assert_eq!(malla.len(), 3);
        // La altura (2 en Y del glb) acaba en Z del mundo.
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
