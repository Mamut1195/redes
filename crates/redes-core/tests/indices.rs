//! Índices del proyecto (F14-01): búsqueda por id y por código y adyacencia nodo → tramos sin
//! recorrer todo el proyecto.
//!
//! Coherencia: tras secuencias aleatorias de altas, bajas, clonados (deshacer), guardado y carga,
//! cada búsqueda indexada da lo mismo que el recorrido lineal. La escala (20k/50k tramos) está en
//! `tests/banco.rs` (F14-10).

mod comun;

use std::collections::BTreeMap;

use comun::{plan_sintetico, temporal};
use redes_core::geom::{P2, P3};
use redes_core::model::{ElementId, EstadoObjeto, Link, Node, Project};

// ---------------------------------------------------------------------------------------------
// Coherencia: índice == recorrido lineal

/// Generador congruencial: secuencias reproducibles sin dependencias.
struct Azar(u64);

impl Azar {
    fn sig(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 33
    }
    fn hasta(&mut self, n: usize) -> usize {
        (self.sig() % n.max(1) as u64) as usize
    }
}

fn pozo(codigo: &str, x: f64) -> Node {
    Node {
        id: ElementId::new(0, 0),
        family: "saneamiento.pozo".into(),
        ty: "inspeccion".into(),
        code: codigo.into(),
        pos: P2::new(758_000.0 + x, 9_797_000.0),
        z_terrain: 2500.0,
        z_rim: 2500.0,
        z_invert: 2498.0,
        attrs: BTreeMap::new(),
        datos: BTreeMap::new(),
        modelo: None,
        estado: EstadoObjeto::Pendiente,
        observaciones: Vec::new(),
        deleted: false,
        sin_dato: Vec::new(),
    }
}

fn colector(codigo: &str, from: ElementId, to: ElementId) -> Link {
    Link {
        id: ElementId::new(0, 0),
        family: "saneamiento.tuberia".into(),
        ty: "colector_sanitario".into(),
        code: codigo.into(),
        from,
        to,
        path: vec![
            P3::new(758_000.0, 9_797_000.0, 2498.0),
            P3::new(758_010.0, 9_797_000.0, 2497.9),
        ],
        d_mm: 200,
        attrs: BTreeMap::new(),
        datos: BTreeMap::new(),
        estado: EstadoObjeto::Pendiente,
        observaciones: Vec::new(),
        cota_solera_inicio: None,
        cota_solera_fin: None,
        deleted: false,
        sin_dato: Vec::new(),
        modelo: None,
    }
}

/// Cada búsqueda del proyecto contra su definición lineal (la de antes de F14-01).
fn comprueba(p: &Project, paso: &str) {
    let mut ids: Vec<ElementId> = p.nodes.iter().map(|n| n.id).collect();
    ids.extend(p.links.iter().map(|l| l.id));
    ids.push(ElementId::new(999_999, 1));
    let mut codigos: Vec<String> = p.nodes.iter().map(|n| n.code.clone()).collect();
    codigos.extend(p.links.iter().map(|l| l.code.clone()));
    let variantes: Vec<String> = codigos
        .iter()
        .flat_map(|c| [c.to_ascii_lowercase(), c.to_ascii_uppercase()])
        .collect();
    codigos.extend(variantes);
    codigos.push("NO-EXISTE".into());

    for &id in &ids {
        let nodo = p.nodes.iter().find(|n| n.id == id && !n.deleted);
        assert_eq!(
            p.node(id).map(|n| n.id),
            nodo.map(|n| n.id),
            "{paso}: node({id:?})"
        );
        let tramo = p.links.iter().find(|l| l.id == id && !l.deleted);
        assert_eq!(
            p.link(id).map(|l| l.id),
            tramo.map(|l| l.id),
            "{paso}: link({id:?})"
        );
        let lineal: Vec<ElementId> = p
            .links
            .iter()
            .filter(|l| !l.deleted && (l.from == id || l.to == id))
            .map(|l| l.id)
            .collect();
        let indexado: Vec<ElementId> = p.tramos_de(id).map(|l| l.id).collect();
        assert_eq!(indexado, lineal, "{paso}: tramos_de({id:?})");
    }
    for c in &codigos {
        let nodo = p
            .nodes
            .iter()
            .find(|n| !n.deleted && n.code.eq_ignore_ascii_case(c));
        assert_eq!(
            p.node_by_code(c).map(|n| n.id),
            nodo.map(|n| n.id),
            "{paso}: node_by_code({c})"
        );
        let tramo = p
            .links
            .iter()
            .find(|l| !l.deleted && l.code.eq_ignore_ascii_case(c));
        assert_eq!(
            p.link_by_code(c).map(|l| l.id),
            tramo.map(|l| l.id),
            "{paso}: link_by_code({c})"
        );
    }
    for (familia, prefijo) in [("saneamiento.pozo", "PZ"), ("saneamiento.tuberia", "CL")] {
        let mut n = p.nodes.iter().filter(|x| x.family == familia).count()
            + p.links.iter().filter(|x| x.family == familia).count()
            + 1;
        let lineal = loop {
            let c = format!("{prefijo}-{n:03}");
            if !p.nodes.iter().any(|x| x.code == c) && !p.links.iter().any(|x| x.code == c) {
                break c;
            }
            n += 1;
        };
        assert_eq!(p.next_code(familia), lineal, "{paso}: next_code({familia})");
    }
}

/// Tras cientos de altas, bajas, altas repetidas, deshacer (clon), guardar y abrir, y ediciones a
/// mano de los vectores, cada búsqueda indexada coincide con el recorrido lineal.
#[test]
fn el_indice_coincide_con_el_recorrido_lineal_tras_cualquier_secuencia() {
    for semilla in 1..=6u64 {
        let mut az = Azar(semilla);
        let mut p = Project::new("coherencia", "EPSG:32717");
        let mut historial: Vec<Project> = Vec::new();
        let archivo = temporal(&format!("coherencia-{semilla}.redes"));
        for paso in 0..220 {
            let vivos: Vec<ElementId> = p
                .nodes
                .iter()
                .filter(|n| !n.deleted)
                .map(|n| n.id)
                .collect();
            let todos: Vec<ElementId> = p
                .nodes
                .iter()
                .map(|n| n.id)
                .chain(p.links.iter().map(|l| l.id))
                .collect();
            let que = match az.hasta(100) {
                // Alta de pozo: código automático, propio o repetido con otra caja.
                0..=29 => {
                    let codigo = match az.hasta(4) {
                        0 => String::new(),
                        1 if !p.nodes.is_empty() => {
                            p.nodes[az.hasta(p.nodes.len())].code.to_ascii_lowercase()
                        }
                        _ => format!("Pz-{}", az.hasta(60)),
                    };
                    let repetido = !codigo.is_empty() && p.node_by_code(&codigo).is_some();
                    let r = p.add_node(pozo(&codigo, paso as f64));
                    assert_eq!(r.is_err(), repetido, "alta de {codigo:?}");
                    "alta_nodo"
                }
                // Alta de tramo entre dos pozos vivos.
                30..=54 if vivos.len() >= 2 => {
                    let a = vivos[az.hasta(vivos.len())];
                    let b = vivos[az.hasta(vivos.len())];
                    let codigo = match az.hasta(3) {
                        0 => String::new(),
                        _ => format!("cl-{}", az.hasta(60)),
                    };
                    let _ = p.add_link(colector(&codigo, a, b));
                    "alta_tramo"
                }
                // Baja de cualquier objeto, vivo o ya borrado.
                55..=74 if !todos.is_empty() => {
                    let id = todos[az.hasta(todos.len())];
                    let con_tramos = p.nodes.iter().any(|n| n.id == id)
                        && p.links
                            .iter()
                            .any(|l| !l.deleted && (l.from == id || l.to == id));
                    let r = p.remove(id);
                    assert_eq!(r.is_err(), con_tramos, "baja de {id:?}");
                    "baja"
                }
                // Deshacer: la app guarda clones del proyecto y vuelve a uno.
                75..=84 => {
                    if az.hasta(2) == 0 || historial.is_empty() {
                        historial.push(p.clone());
                    } else {
                        p = historial.pop().unwrap();
                    }
                    "deshacer"
                }
                // Guardar y abrir.
                85..=89 => {
                    redes_core::project_io::save(&p, &archivo).expect("guarda");
                    // Un código dado de baja y vuelto a dar de alta no impide abrir.
                    p = redes_core::project_io::load(&archivo).expect("abre lo que guardó");
                    assert!(p.indice_al_dia(), "al abrir se reconstruye el índice");
                    "guardar_abrir"
                }
                // Edición a mano de los vectores (campos públicos): el índice debe saberse
                // desfasado y las búsquedas seguir siendo correctas.
                90..=94 if !p.links.is_empty() => {
                    let mut l = p.links.pop().unwrap();
                    if az.hasta(2) == 0 {
                        l.id = ElementId::new(500_000 + paso as u32, 1);
                        l.code = format!("MANO-{paso}");
                        p.links.push(l);
                    }
                    "a_mano"
                }
                _ => {
                    p.reindexar();
                    "reindexar"
                }
            };
            comprueba(&p, &format!("semilla {semilla}, paso {paso} ({que})"));
            if matches!(que, "alta_nodo" | "alta_tramo" | "baja" | "reindexar") {
                assert!(p.indice_al_dia(), "{que} mantiene el índice");
            }
        }
        let _ = std::fs::remove_file(&archivo);
    }
}

/// Reimportar sustituye los vectores del proyecto: el índice se rehace y empareja por código.
#[test]
fn reimportar_deja_el_indice_al_dia() {
    let ruta = temporal("reimportar.json");
    std::fs::write(&ruta, plan_sintetico(300)).unwrap();
    let mut p = Project::new("Sintetica", "EPSG:32717");
    redes_core::ingest::import_plan(&mut p, &ruta).expect("importa");
    assert!(p.indice_al_dia());
    redes_core::reimportar::reimportar(
        &mut p,
        &ruta,
        redes_core::reimportar::Formato::Plan,
        &redes_core::diff::Opciones::default(),
        false,
    )
    .expect("reimporta");
    let _ = std::fs::remove_file(&ruta);
    assert!(
        p.indice_al_dia(),
        "tras reimportar el índice corresponde a la entrega"
    );
    comprueba(&p, "reimportar");
}

/// La app edita en su sitio el nodo seleccionado por código (F14-01): `node_by_code_mut` da el
/// vivo, nunca uno borrado con el mismo código, y la edición queda en el proyecto.
#[test]
fn node_by_code_mut_edita_el_nodo_vivo() {
    let mut p = Project::new("editar", "EPSG:32717");
    let viejo = p.add_node(pozo("PZ-1", 0.0)).unwrap();
    p.nodes.iter_mut().find(|n| n.id == viejo).unwrap().deleted = true;
    let vivo = p.add_node(pozo("PZ-1", 10.0)).unwrap();
    assert!(p.node_by_code_mut("NO-EXISTE").is_none());
    let n = p.node_by_code_mut("pz-1").expect("encuentra el vivo");
    assert_eq!(n.id, vivo);
    n.z_invert = 2490.0;
    assert_eq!(p.node(vivo).unwrap().z_invert, 2490.0);
    assert!(p.indice_al_dia());
    comprueba(&p, "node_by_code_mut");
}
