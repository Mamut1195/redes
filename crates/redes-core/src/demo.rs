//! Proyecto de demostración "La Esperanza": una red de alcantarillado y una de agua potable
//! sobre un levantamiento, con datos **importados** de otras aplicaciones (no calculados aquí).
//!
//! Coordenadas UTM 17S reales (E ≈ 760 000, N ≈ 9 796 000) para ejercitar el origen local:
//! el modelo guarda f64 absoluto y la cámara resta el origen antes de pasar a f32.

use std::collections::BTreeMap;

use crate::families::Value;
use crate::geom::{P2, P3};
use crate::model::{Dato, ElementId, EstadoObjeto, EstadoProyecto, Fuente, Link, Node, Project};
use crate::terrain::{SurfacePoint, Terrain};

pub const ORIGEN_E: f64 = 760_000.0;
pub const ORIGEN_N: f64 = 9_796_000.0;

fn cota(p: &Project, e: f64, n: f64, fallback: f64) -> f64 {
    p.terrain
        .as_ref()
        .and_then(|t| t.z_at(P2::new(e, n)))
        .unwrap_or(fallback)
}

/// Construye el proyecto de demostración.
pub fn project() -> Project {
    let mut p = Project::new("Redes La Esperanza", "EPSG:32717");
    p.origin = P2::new(ORIGEN_E, ORIGEN_N);
    p.estado = EstadoProyecto::EnRevision;
    p.fuentes = vec![
        Fuente {
            app: "Civil 3D".into(),
            archivo: "levantamiento-la-esperanza.dxf".into(),
            formato: "dxf".into(),
            importado_en: "2026-11-18".into(),
        },
        Fuente {
            app: "SWMM".into(),
            archivo: "red-alcantarillado.inp".into(),
            formato: "swmm".into(),
            importado_en: "2026-11-18".into(),
        },
    ];

    // --- topografía: rejilla 60×40 sobre 600 × 400 m, con pendiente hacia el este ---
    let mut pts = Vec::new();
    for j in 0..40 {
        for i in 0..60 {
            let e = ORIGEN_E + i as f64 * 10.0;
            let n = ORIGEN_N + j as f64 * 10.0;
            let z = 2514.0 - i as f64 * 10.0 / 600.0 * 8.0
                + (i as f64 * 0.035).sin() * 1.2
                + (n - ORIGEN_N) / 400.0 * 3.0
                + (i as f64 * 0.11).cos() * 0.5;
            pts.push(SurfacePoint::new(e, n, z));
        }
    }
    p.terrain = Some(Terrain::build("levantamiento-la-esperanza", 32717, pts));

    // --- alcantarillado: troncal de 8 pozos ---
    let mut trunk = Vec::new();
    for i in 0..8 {
        let e = ORIGEN_E + 60.0 + i as f64 * 65.0;
        let n = ORIGEN_N + 90.0 + (i as f64 * 0.9).sin() * 12.0;
        let z_t = cota(&p, e, n, 2512.0);
        let depth = 1.6 + i as f64 * 0.22;
        let mut datos = BTreeMap::new();
        datos.insert(
            "caudal_diseno".to_string(),
            Dato {
                valor: 12.0 + i as f64 * 3.4,
                unidad: "L/s".into(),
                fuente: "SWMM".into(),
            },
        );
        let id = p
            .add_node(Node {
                id: ElementId::new(0, 0),
                family: "saneamiento.pozo".into(),
                ty: if i == 2 {
                    "caida_exterior".into()
                } else {
                    "inspeccion".into()
                },
                code: String::new(),
                pos: P2::new(e, n),
                z_terrain: z_t,
                z_rim: z_t,
                z_invert: z_t - depth,
                attrs: BTreeMap::from([
                    ("diametro_interior".to_string(), Value::Num(1.20)),
                    ("cota_tapa".to_string(), Value::Num(z_t)),
                    ("cota_solera".to_string(), Value::Num(z_t - depth)),
                    (
                        "altura_caida".to_string(),
                        Value::Num(if i == 2 { 0.6 } else { 0.0 }),
                    ),
                    ("material".to_string(), Value::Text("hormigon".into())),
                ]),
                datos,
                estado: if i < 3 {
                    EstadoObjeto::Aprobado
                } else {
                    EstadoObjeto::Pendiente
                },
                nota: None,
                modelo: None,
                deleted: false,
            })
            .unwrap();
        trunk.push(id);
    }

    for w in trunk.windows(2) {
        let (a, b) = (p.node(w[0]).unwrap(), p.node(w[1]).unwrap());
        let (a_pos, a_z, b_pos, b_z) = (a.pos, a.z_invert, b.pos, b.z_invert);
        let (ac, bc) = (a.code.clone(), b.code.clone());
        let mut datos = BTreeMap::new();
        datos.insert(
            "volumen_excavacion".to_string(),
            Dato {
                valor: 118.4,
                unidad: "m³".into(),
                fuente: "Civil 3D".into(),
            },
        );
        datos.insert(
            "tirante".to_string(),
            Dato {
                valor: 0.42,
                unidad: "m".into(),
                fuente: "SWMM".into(),
            },
        );
        p.add_link(Link {
            id: ElementId::new(0, 0),
            family: "saneamiento.tuberia".into(),
            ty: "colector_sanitario".into(),
            code: String::new(),
            from: w[0],
            to: w[1],
            path: vec![
                P3::new(a_pos.x, a_pos.y, a_z),
                P3::new(b_pos.x, b_pos.y, b_z),
            ],
            d_mm: 250,
            attrs: BTreeMap::from([
                ("material".to_string(), Value::Text("PVC".into())),
                ("diametro_nominal".to_string(), Value::Int(250)),
                (
                    "pendiente".to_string(),
                    Value::Num((a_z - b_z) / a_pos.dist(b_pos)),
                ),
            ]),
            datos,
            estado: if a_z > b_z + 0.2 {
                EstadoObjeto::Aprobado
            } else {
                EstadoObjeto::Pendiente
            },
            nota: None,
            cota_solera_inicio: None,
            cota_solera_fin: None,
            deleted: false,
        })
        .ok();
        let _ = (ac, bc);
    }

    // --- laterales ---
    for (k, i) in [2usize, 3, 4, 5, 6, 7].into_iter().enumerate() {
        let (bx, by, bz_t, bz_c) = {
            let b = p.node(trunk[i]).unwrap();
            (b.pos.x, b.pos.y, b.z_terrain, b.z_invert)
        };
        let (e, n) = (bx - 18.0, by + 55.0 + k as f64 * 3.0);
        let z_t = cota(&p, e, n, bz_t);
        let z_c = bz_c + 0.55;
        let id = p
            .add_node(Node {
                id: ElementId::new(0, 0),
                family: "saneamiento.pozo".into(),
                ty: "inspeccion".into(),
                code: String::new(),
                pos: P2::new(e, n),
                z_terrain: z_t,
                z_rim: z_t,
                z_invert: z_c,
                attrs: BTreeMap::from([
                    ("diametro_interior".to_string(), Value::Num(0.90)),
                    ("cota_solera".to_string(), Value::Num(z_c)),
                ]),
                datos: BTreeMap::new(),
                estado: EstadoObjeto::Pendiente,
                nota: None,
                modelo: None,
                deleted: false,
            })
            .unwrap();
        p.add_link(Link {
            id: ElementId::new(0, 0),
            family: "saneamiento.tuberia".into(),
            ty: "colector_sanitario".into(),
            code: String::new(),
            from: id,
            to: trunk[i],
            path: vec![P3::new(e, n, z_c), P3::new(bx, by, bz_c)],
            d_mm: 200,
            attrs: BTreeMap::from([("material".to_string(), Value::Text("PVC".into()))]),
            datos: BTreeMap::new(),
            estado: EstadoObjeto::Pendiente,
            nota: None,
            cota_solera_inicio: None,
            cota_solera_fin: None,
            deleted: false,
        })
        .ok();
    }

    // --- emisario ---
    let (lx, ly, lz) = {
        let last = p.node(*trunk.last().unwrap()).unwrap();
        (last.pos.x, last.pos.y, last.z_invert)
    };
    let (e, n) = (lx + 70.0, ly - 30.0);
    let z_t = cota(&p, e, n, lz + 1.5);
    let descarga = p
        .add_node(Node {
            id: ElementId::new(0, 0),
            family: "saneamiento.descarga".into(),
            ty: "emisario_muro".into(),
            code: String::new(),
            pos: P2::new(e, n),
            z_terrain: z_t,
            z_rim: z_t,
            z_invert: lz - 0.80,
            attrs: BTreeMap::from([
                ("tipo_descarga".to_string(), Value::Text("emisario".into())),
                ("cota_entrega".to_string(), Value::Num(lz - 0.80)),
                (
                    "cuerpo_receptor".to_string(),
                    Value::Text("quebrada La Esperanza".into()),
                ),
            ]),
            datos: BTreeMap::new(),
            estado: EstadoObjeto::Pendiente,
            nota: None,
            modelo: None,
            deleted: false,
        })
        .unwrap();
    p.add_link(Link {
        id: ElementId::new(0, 0),
        family: "saneamiento.tuberia".into(),
        ty: "emisario".into(),
        code: String::new(),
        from: *trunk.last().unwrap(),
        to: descarga,
        path: vec![P3::new(lx, ly, lz), P3::new(e, n, lz - 0.80)],
        d_mm: 315,
        attrs: BTreeMap::new(),
        datos: BTreeMap::new(),
        estado: EstadoObjeto::Pendiente,
        nota: None,
        cota_solera_inicio: None,
        cota_solera_fin: None,
        deleted: false,
    })
    .ok();

    // --- agua potable: tanque + red de distribución ---
    let tanque_pos = P2::new(ORIGEN_E + 40.0, ORIGEN_N + 330.0);
    let tanque_z = cota(&p, tanque_pos.x, tanque_pos.y, 2516.0);
    let tanque = p
        .add_node(Node {
            id: ElementId::new(0, 0),
            family: "agua.tanque".into(),
            ty: "apoyado_cilindrico".into(),
            code: String::new(),
            pos: tanque_pos,
            z_terrain: tanque_z,
            z_rim: tanque_z + 6.0,
            z_invert: tanque_z,
            attrs: BTreeMap::from([
                (
                    "tipo_tanque".to_string(),
                    Value::Text("apoyado_cilindrico".into()),
                ),
                ("volumen_util".to_string(), Value::Num(600.0)),
                ("cota_piso".to_string(), Value::Num(tanque_z)),
                ("cota_nivel_max".to_string(), Value::Num(tanque_z + 5.0)),
            ]),
            datos: BTreeMap::new(),
            estado: EstadoObjeto::Aprobado,
            nota: None,
            modelo: None,
            deleted: false,
        })
        .unwrap();

    let mut prev = tanque;
    for i in 0..6 {
        let e = ORIGEN_E + 110.0 + i as f64 * 85.0;
        let n = ORIGEN_N + 330.0 - i as f64 * 6.0;
        let z_t = cota(&p, e, n, 2516.0);
        let id = p
            .add_node(Node {
                id: ElementId::new(0, 0),
                family: "agua.camara".into(),
                ty: "valvulas".into(),
                code: String::new(),
                pos: P2::new(e, n),
                z_terrain: z_t,
                z_rim: z_t,
                z_invert: z_t - 1.2,
                attrs: BTreeMap::from([
                    ("tipo_camara".to_string(), Value::Text("valvulas".into())),
                    ("diametro_interior".to_string(), Value::Num(1.50)),
                    ("cota_solera".to_string(), Value::Num(z_t - 1.2)),
                ]),
                datos: BTreeMap::new(),
                estado: EstadoObjeto::Pendiente,
                nota: None,
                modelo: None,
                deleted: false,
            })
            .unwrap();
        let (ax, ay, az) = {
            let a = p.node(prev).unwrap();
            (a.pos.x, a.pos.y, a.z_terrain - 1.2)
        };
        let (bx, by, bz) = {
            let b = p.node(id).unwrap();
            (b.pos.x, b.pos.y, b.z_terrain - 1.2)
        };
        p.add_link(Link {
            id: ElementId::new(0, 0),
            family: "agua.tuberia".into(),
            ty: "distribucion_primaria".into(),
            code: String::new(),
            from: prev,
            to: id,
            path: vec![P3::new(ax, ay, az), P3::new(bx, by, bz)],
            d_mm: 160,
            attrs: BTreeMap::from([
                ("material".to_string(), Value::Text("PVC".into())),
                ("coef_hazen".to_string(), Value::Num(140.0)),
            ]),
            datos: BTreeMap::new(),
            estado: EstadoObjeto::Pendiente,
            nota: None,
            cota_solera_inicio: None,
            cota_solera_fin: None,
            deleted: false,
        })
        .ok();
        prev = id;
    }

    // Válvula e hidrante (dispositivos sobre la red).
    p.add_node(Node {
        id: ElementId::new(0, 0),
        family: "agua.valvula".into(),
        ty: "compuerta".into(),
        code: String::new(),
        pos: P2::new(ORIGEN_E + 195.0, ORIGEN_N + 324.0),
        z_terrain: 2517.0,
        z_rim: 2517.0,
        z_invert: 2515.8,
        attrs: BTreeMap::from([
            ("tipo_valvula".to_string(), Value::Text("compuerta".into())),
            ("diametro_nominal".to_string(), Value::Int(160)),
            (
                "estado_operacion".to_string(),
                Value::Text("abierta".into()),
            ),
        ]),
        datos: BTreeMap::new(),
        estado: EstadoObjeto::Pendiente,
        nota: None,
        modelo: None,
        deleted: false,
    })
    .ok();

    p.add_node(Node {
        id: ElementId::new(0, 0),
        family: "agua.llave".into(),
        ty: "columna".into(),
        code: String::new(),
        pos: P2::new(ORIGEN_E + 450.0, ORIGEN_N + 300.0),
        z_terrain: 2515.0,
        z_rim: 2515.0,
        z_invert: 2513.8,
        attrs: BTreeMap::from([
            ("tipo_llave".to_string(), Value::Text("columna".into())),
            ("diametro_nominal".to_string(), Value::Int(100)),
        ]),
        datos: BTreeMap::new(),
        estado: EstadoObjeto::Pendiente,
        nota: None,
        modelo: None,
        deleted: false,
    })
    .ok();

    p.commit("demo");
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_demo_tiene_red_y_terreno() {
        let p = project();
        assert!(p.terrain.is_some());
        assert!(p.nodes.len() > 10);
        assert!(p.links.len() > 8);
        assert_eq!(p.count(), p.nodes.len() + p.links.len());
        assert!(!p.fuentes.is_empty());
    }

    #[test]
    fn el_demo_trae_datos_importados_y_no_los_calcula() {
        let p = project();
        let con_datos = p.links.iter().filter(|l| !l.datos.is_empty()).count();
        assert!(con_datos > 0);
        let d = p
            .links
            .iter()
            .find_map(|l| l.datos.get("volumen_excavacion"))
            .unwrap();
        assert_eq!(d.unidad, "m³");
        assert_eq!(d.fuente, "Civil 3D");
    }
}

/// Proyecto **plantilla**: lo mínimo para empezar a dibujar sin partir de cero.
///
/// Terreno plano de 80 × 60 m, tres pozos de inspección en línea y dos colectores con diámetro y
/// material declarados. No trae datos importados: es un lienzo, no un ejemplo resuelto.
pub fn plantilla() -> Project {
    let mut p = Project::new("Proyecto nuevo", "EPSG:32717");
    p.origin = P2::new(758_000.0, 9_797_000.0);
    p.estado = EstadoProyecto::EnRevision;

    // Terreno plano con una pendiente suave hacia el este.
    let mut pts = Vec::new();
    let mut y = 0.0;
    while y <= 60.0 {
        let mut x = 0.0;
        while x <= 80.0 {
            pts.push(SurfacePoint::new(
                p.origin.x + x,
                p.origin.y + y,
                2500.0 - 0.02 * x,
            ));
            x += 5.0;
        }
        y += 5.0;
    }
    p.terrain = Some(crate::terrain::Terrain::build("plataforma", 32717, pts));

    let mut ids = Vec::new();
    for (i, (x, y)) in [(20.0, 30.0), (45.0, 30.0), (70.0, 30.0)]
        .iter()
        .enumerate()
    {
        let z = 2500.0 - 0.02 * x;
        let id = p
            .add_node(Node {
                id: ElementId::new(0, 0),
                family: "saneamiento.pozo".into(),
                ty: "inspeccion".into(),
                code: String::new(),
                pos: P2::new(p.origin.x + x, p.origin.y + y),
                z_terrain: z,
                z_rim: z,
                z_invert: z - 2.0 - 0.15 * i as f64,
                attrs: BTreeMap::from([
                    ("diametro_interior".to_string(), Value::Num(1.2)),
                    ("cota_tapa".to_string(), Value::Num(z)),
                    (
                        "cota_solera".to_string(),
                        Value::Num(z - 2.0 - 0.15 * i as f64),
                    ),
                ]),
                datos: BTreeMap::new(),
                estado: EstadoObjeto::Pendiente,
                nota: None,
                modelo: None,
                deleted: false,
            })
            .expect("pozo de plantilla");
        ids.push((
            id,
            [p.origin.x + x, p.origin.y + y, z - 2.0 - 0.15 * i as f64],
        ));
    }

    for (i, par) in ids.windows(2).enumerate() {
        let (a, b) = (&par[0], &par[1]);
        p.add_link(Link {
            id: ElementId::new(0, 0),
            family: "saneamiento.tuberia".into(),
            ty: "colector_sanitario".into(),
            code: String::new(),
            from: a.0,
            to: b.0,
            path: vec![
                P3::new(a.1[0], a.1[1], a.1[2]),
                P3::new(
                    (a.1[0] + b.1[0]) * 0.5,
                    (a.1[1] + b.1[1]) * 0.5,
                    (a.1[2] + b.1[2]) * 0.5,
                ),
                P3::new(b.1[0], b.1[1], b.1[2]),
            ],
            d_mm: if i == 0 { 200 } else { 250 },
            attrs: BTreeMap::from([
                (
                    "diametro_nominal".to_string(),
                    Value::Int(if i == 0 { 200 } else { 250 }),
                ),
                ("material".to_string(), Value::Text("PVC".into())),
            ]),
            datos: BTreeMap::new(),
            estado: EstadoObjeto::Pendiente,
            nota: None,
            cota_solera_inicio: None,
            cota_solera_fin: None,
            deleted: false,
        })
        .expect("colector de plantilla");
    }
    p.commit("proyecto.plantilla");
    p
}

#[cfg(test)]
mod tests_plantilla {
    #[test]
    fn la_plantilla_trae_terreno_y_red() {
        let p = super::plantilla();
        assert!(p.terrain.is_some(), "terreno");
        assert_eq!(p.nodes.len(), 3, "tres pozos");
        assert_eq!(p.links.len(), 2, "dos colectores");
        assert!(p.nodes.iter().all(|n| n.code.starts_with("PZ-")));
        // El código del arco lo asigna el registro con el prefijo de su familia.
        assert!(p.links.iter().all(|l| !l.code.is_empty()));
    }
}
