//! Catálogo de piezas: un proyecto con una muestra de cada familia y subtipo, puesto en fila
//! para poder mirarlo de cerca. Es la escena de promoción y la prueba visual del modelado
//! paramétrico.

use std::collections::BTreeMap;

use crate::families::Value;
use crate::geom::{P2, P3};
use crate::model::{Dato, ElementId, EstadoObjeto, EstadoProyecto, Fuente, Link, Node, Project};
use crate::terrain::{SurfacePoint, Terrain};

pub const ORIGEN_E: f64 = 758_000.0;
pub const ORIGEN_N: f64 = 9_797_000.0;

fn cota(x: f64) -> f64 {
    2500.0 + 0.035 * x
}

/// Proyecto de catálogo: 14 piezas en dos hileras sobre una plataforma.
pub fn catalogo() -> Project {
    let mut p = Project::new("Catálogo de piezas", "EPSG:32717");
    p.origin = P2::new(ORIGEN_E, ORIGEN_N);
    p.estado = EstadoProyecto::EnRevision;
    p.fuentes = vec![Fuente {
        app: "Catálogo Redes".into(),
        archivo: "catalogo.json".into(),
        formato: "plan".into(),
        importado_en: "2026-11-18".into(),
    }];

    // Terreno: plataforma de 120 × 50 m con una pendiente suave.
    let mut pts = Vec::new();
    let mut y = 0.0;
    while y <= 50.0 {
        let mut x = 0.0;
        while x <= 120.0 {
            pts.push(SurfacePoint::new(ORIGEN_E + x, ORIGEN_N + y, cota(x)));
            x += 4.0;
        }
        y += 4.0;
    }
    p.terrain = Some(Terrain::build("plataforma-catalogo", 32717, pts));

    // Hilera de pozos con sus tuberías (fila de arriba, y = 36).
    let mut pozos = Vec::new();
    let tipos = [
        "inspeccion",
        "caida_exterior",
        "resalto",
        "ciego",
        "disipador",
        "con_arenero",
    ];
    for (i, tipo) in tipos.iter().enumerate() {
        let x = 10.0 + i as f64 * 11.0;
        let e = ORIGEN_E + x;
        let n = ORIGEN_N + 36.0;
        let z = cota(x);
        let id = p
            .add_node(Node {
                id: ElementId::new(0, 0),
                family: "saneamiento.pozo".into(),
                ty: (*tipo).into(),
                code: String::new(),
                pos: P2::new(e, n),
                z_terrain: z,
                z_rim: z,
                z_invert: z - 2.2,
                attrs: BTreeMap::from([
                    (
                        "diametro_interior".to_string(),
                        Value::Num(if *tipo == "inspeccion" { 1.2 } else { 1.5 }),
                    ),
                    ("cota_tapa".to_string(), Value::Num(z)),
                    ("cota_solera".to_string(), Value::Num(z - 2.2)),
                    (
                        "altura_caida".to_string(),
                        Value::Num(if tipo.starts_with("caida") { 0.8 } else { 0.0 }),
                    ),
                ]),
                datos: BTreeMap::new(),
                estado: EstadoObjeto::Pendiente,
                nota: None,
                modelo: None,
                deleted: false,
            })
            .unwrap();
        pozos.push(id);
    }
    for w in pozos.windows(2) {
        let (a, b) = {
            let a = p.node(w[0]).unwrap();
            let b = p.node(w[1]).unwrap();
            (a.pos, b.pos)
        };
        p.add_link(Link {
            id: ElementId::new(0, 0),
            family: "saneamiento.tuberia".into(),
            ty: "colector_sanitario".into(),
            code: String::new(),
            from: w[0],
            to: w[1],
            path: vec![
                P3::new(a.x, a.y, cota(a.x - ORIGEN_E) - 2.2),
                P3::new(b.x, b.y, cota(b.x - ORIGEN_E) - 2.3),
            ],
            d_mm: 250,
            attrs: BTreeMap::from([("material".to_string(), Value::Text("PVC".into()))]),
            datos: BTreeMap::new(),
            estado: EstadoObjeto::Pendiente,
            nota: None,
            deleted: false,
        })
        .ok();
    }

    // Hilera de accesorios y equipos (fila de abajo, y = 14).
    let mut fila = Vec::new();
    let equipos: [(&str, &str, u32, BTreeMap<String, Value>); 8] = [
        (
            "agua.valvula",
            "compuerta",
            200,
            BTreeMap::from([("diametro_nominal".to_string(), Value::Int(200))]),
        ),
        (
            "agua.valvula",
            "mariposa",
            250,
            BTreeMap::from([("diametro_nominal".to_string(), Value::Int(250))]),
        ),
        (
            "agua.valvula",
            "retencion",
            200,
            BTreeMap::from([("diametro_nominal".to_string(), Value::Int(200))]),
        ),
        (
            "agua.valvula",
            "reduccion_presion",
            150,
            BTreeMap::from([("diametro_nominal".to_string(), Value::Int(150))]),
        ),
        (
            "agua.hidrante",
            "columna",
            100,
            BTreeMap::from([("diametro_nominal".to_string(), Value::Int(100))]),
        ),
        (
            "agua.medidor",
            "chorro_multiple",
            25,
            BTreeMap::from([("diametro_nominal".to_string(), Value::Int(25))]),
        ),
        (
            "agua.camara",
            "valvulas",
            150,
            BTreeMap::from([("diametro_interior".to_string(), Value::Num(1.5))]),
        ),
        ("saneamiento.sumidero", "calzada", 200, BTreeMap::new()),
    ];
    for (i, (familia, tipo, _dn, attrs)) in equipos.iter().enumerate() {
        let x = 8.0 + i as f64 * 8.5;
        let e = ORIGEN_E + x;
        let n = ORIGEN_N + 14.0;
        let z = cota(x);
        let id = p
            .add_node(Node {
                id: ElementId::new(0, 0),
                family: (*familia).into(),
                ty: (*tipo).into(),
                code: String::new(),
                pos: P2::new(e, n),
                z_terrain: z,
                z_rim: z,
                // En la vitrina los equipos se apoyan en el terreno: se quieren ver.
                z_invert: z - 0.02,
                attrs: attrs.clone(),
                datos: BTreeMap::new(),
                estado: EstadoObjeto::Pendiente,
                nota: None,
                modelo: None,
                deleted: false,
            })
            .unwrap();
        fila.push(id);
    }

    // Segunda hilera: accesorios y elementos complementarios (y = 4).
    let complementos: [(&str, &str, u32); 10] = [
        ("comun.accesorio", "tee", 200),
        ("comun.tapa", "ciega_circular", 600),
        ("comun.escalera", "peldano", 300),
        ("saneamiento.caja", "revision", 200),
        ("saneamiento.arenero", "patio", 200),
        ("saneamiento.trampa_grasa", "prefabricada_hdpe", 150),
        ("agua.dado_anclaje", "codo", 200),
        ("saneamiento.bombeo", "camara_humeda", 200),
        ("agua.llave", "banqueta", 25),
        ("agua.arqueta", "medidor", 25),
    ];
    for (i, (familia, tipo, _dn)) in complementos.iter().enumerate() {
        let x = 8.0 + i as f64 * 9.0;
        let e = ORIGEN_E + x;
        let n = ORIGEN_N + 4.0;
        let z = cota(x);
        let mut attrs = BTreeMap::new();
        if *familia == "comun.tapa" {
            attrs.insert("diametro_tapa".to_string(), Value::Int(600));
            attrs.insert("clase_carga".to_string(), Value::Text("D400".into()));
        } else if *familia == "comun.escalera" {
            attrs.insert("altura".to_string(), Value::Num(3.0));
        } else {
            attrs.insert("diametro_nominal".to_string(), Value::Int(200));
        }
        p.add_node(Node {
            id: ElementId::new(0, 0),
            family: (*familia).into(),
            ty: (*tipo).into(),
            code: String::new(),
            pos: P2::new(e, n),
            z_terrain: z,
            z_rim: z,
            z_invert: z - 0.02,
            attrs,
            datos: BTreeMap::new(),
            estado: EstadoObjeto::Pendiente,
            nota: None,
            modelo: None,
            deleted: false,
        })
        .ok();
    }

    // Tercera hilera: familias de contexto y obras (y = 25).
    let obras: [(&str, &str); 5] = [
        ("agua.captacion", "manantial_ladera"),
        ("agua.planta", "ptap_convencional"),
        ("comun.cruce", "via"),
        ("comun.hito", "bm"),
        ("comun.cuerpo_receptor", "quebrada"),
    ];
    for (i, (familia, tipo)) in obras.iter().enumerate() {
        let x = 10.0 + i as f64 * 13.0;
        let e = ORIGEN_E + x;
        let n = ORIGEN_N + 25.0;
        let z = cota(x);
        let mut attrs = BTreeMap::new();
        attrs.insert("cota".to_string(), Value::Num(z));
        attrs.insert("caudal_concesionado".to_string(), Value::Num(12.0));
        attrs.insert("caudal_diseno".to_string(), Value::Num(18.0));
        attrs.insert("tipo_cruce".to_string(), Value::Text("via".into()));
        attrs.insert("tipo_hito".to_string(), Value::Text("bm".into()));
        attrs.insert("tipo_receptor".to_string(), Value::Text("quebrada".into()));
        attrs.insert(
            "tipo_captacion".to_string(),
            Value::Text("manantial".into()),
        );
        attrs.insert("tipo_planta".to_string(), Value::Text("ptap".into()));
        p.add_node(Node {
            id: ElementId::new(0, 0),
            family: (*familia).into(),
            ty: (*tipo).into(),
            code: String::new(),
            pos: P2::new(e, n),
            z_terrain: z,
            z_rim: z,
            z_invert: z - 0.02,
            attrs,
            datos: BTreeMap::new(),
            estado: EstadoObjeto::Pendiente,
            nota: None,
            modelo: None,
            deleted: false,
        })
        .ok();
    }

    // Cuarta hilera: una tira de conducciones, una de cada familia de arco (y = 45).
    let mut cadena = Vec::new();
    for i in 0..7 {
        let x = 8.0 + i as f64 * 9.0;
        let e = ORIGEN_E + x;
        let n = ORIGEN_N + 45.0;
        let z = cota(x);
        let id = p
            .add_node(Node {
                id: ElementId::new(0, 0),
                family: "saneamiento.camara".into(),
                ty: "reunion".into(),
                code: String::new(),
                pos: P2::new(e, n),
                z_terrain: z,
                z_rim: z,
                z_invert: z - 1.2,
                attrs: BTreeMap::from([
                    ("diametro_interior".to_string(), Value::Num(1.2)),
                    ("cota_solera".to_string(), Value::Num(z - 1.2)),
                ]),
                datos: BTreeMap::new(),
                estado: EstadoObjeto::Pendiente,
                nota: None,
                modelo: None,
                deleted: false,
            })
            .unwrap();
        cadena.push(id);
    }
    let familias_arco = [
        ("saneamiento.tuberia", "colector_sanitario", 250),
        ("agua.tuberia", "distribucion_primaria", 160),
        ("saneamiento.canal", "rectangular", 300),
        ("saneamiento.sifon", "invertido", 200),
        ("agua.acometida", "domiciliaria", 25),
        ("saneamiento.acometida", "domiciliaria", 150),
    ];
    for (i, (familia, tipo, dn)) in familias_arco.iter().enumerate() {
        let (a, b) = {
            let a = p.node(cadena[i]).unwrap();
            let b = p.node(cadena[i + 1]).unwrap();
            (a.pos, b.pos)
        };
        let mut attrs = BTreeMap::new();
        attrs.insert("diametro_nominal".to_string(), Value::Int(*dn as i64));
        attrs.insert("ancho_plantilla".to_string(), Value::Num(0.6));
        p.add_link(Link {
            id: ElementId::new(0, 0),
            family: (*familia).into(),
            ty: (*tipo).into(),
            code: String::new(),
            from: cadena[i],
            to: cadena[i + 1],
            path: vec![
                P3::new(a.x, a.y, cota(a.x - ORIGEN_E) - 1.2),
                P3::new(b.x, b.y, cota(b.x - ORIGEN_E) - 1.25),
            ],
            d_mm: *dn,
            attrs,
            datos: BTreeMap::new(),
            estado: EstadoObjeto::Pendiente,
            nota: None,
            deleted: false,
        })
        .ok();
    }

    // Tanque y descarga en los extremos.
    let z_tanque = cota(108.0);
    p.add_node(Node {
        id: ElementId::new(0, 0),
        family: "agua.tanque".into(),
        ty: "apoyado_cilindrico".into(),
        code: String::new(),
        pos: P2::new(ORIGEN_E + 108.0, ORIGEN_N + 25.0),
        z_terrain: z_tanque,
        z_rim: z_tanque + 4.0,
        z_invert: z_tanque,
        attrs: BTreeMap::from([
            ("volumen_util".to_string(), Value::Num(300.0)),
            ("cota_piso".to_string(), Value::Num(z_tanque)),
            ("cota_nivel_max".to_string(), Value::Num(z_tanque + 4.0)),
        ]),
        datos: BTreeMap::new(),
        estado: EstadoObjeto::Pendiente,
        nota: None,
        modelo: None,
        deleted: false,
    })
    .ok();

    let z_desc = cota(4.0);
    p.add_node(Node {
        id: ElementId::new(0, 0),
        family: "saneamiento.descarga".into(),
        ty: "emisario_muro".into(),
        code: String::new(),
        pos: P2::new(ORIGEN_E + 4.0, ORIGEN_N + 25.0),
        z_terrain: z_desc,
        z_rim: z_desc,
        z_invert: z_desc - 1.4,
        attrs: BTreeMap::from([("cota_entrega".to_string(), Value::Num(z_desc - 1.4))]),
        datos: BTreeMap::from([(
            "caudal_max".to_string(),
            Dato {
                valor: 42.0,
                unidad: "L/s".into(),
                fuente: "SWMM".into(),
            },
        )]),
        estado: EstadoObjeto::Pendiente,
        nota: None,
        modelo: None,
        deleted: false,
    })
    .ok();

    p.commit("demo catalogo");
    p
}

/// Catálogo **completo**: una muestra de cada familia del registro, en rejilla.
///
/// Es la garantía de cobertura: si una familia nueva no se puede dibujar, esta escena (y la prueba
/// que la acompaña) lo delata. Las familias lineales se colocan como arco entre dos nodos.
pub fn catalogo_completo() -> Project {
    use crate::families::{GeomKind, families};

    let familias = families();
    let columnas = 6usize;
    let paso = 9.0;
    let filas = familias.len().div_ceil(columnas);
    let ancho = columnas as f64 * paso + 4.0;
    let alto = filas as f64 * paso + 4.0;

    let mut p = Project::new("Catálogo completo", "EPSG:32717");
    p.origin = P2::new(ORIGEN_E, ORIGEN_N);
    p.estado = EstadoProyecto::EnRevision;
    p.fuentes = vec![Fuente {
        app: "Catálogo Redes".into(),
        archivo: "catalogo-completo.json".into(),
        formato: "plan".into(),
        importado_en: "2026-11-21".into(),
    }];

    // Plataforma plana del tamaño de la rejilla.
    let mut pts = Vec::new();
    for ix in 0..=6 {
        for iy in 0..=6 {
            let x = ancho * ix as f64 / 6.0;
            let y = alto * iy as f64 / 6.0;
            pts.push(SurfacePoint::new(
                ORIGEN_E + x,
                ORIGEN_N + y,
                2500.0 + 0.01 * x,
            ));
        }
    }
    p.terrain = Some(Terrain::build("catalogo-completo", 32717, pts));

    let mut previo: Option<(ElementId, P3)> = None;
    for (i, f) in familias.iter().enumerate() {
        let col = i % columnas;
        let fila = i / columnas;
        let x = 2.0 + col as f64 * paso;
        let y = 2.0 + fila as f64 * paso;
        let z = 2500.0 + 0.01 * x;
        let ty = f
            .types
            .first()
            .map(|t| t.key.to_string())
            .unwrap_or_default();
        if ty.is_empty() {
            continue;
        }

        let nodo = Node {
            id: ElementId::new(0, 0),
            family: f.key.to_string(),
            ty: ty.clone(),
            // Código explícito: dos familias pueden compartir prefijo y `next_code` colisionaría.
            code: format!("{}-{:03}", f.code_prefix, i + 1),
            pos: P2::new(ORIGEN_E + x, ORIGEN_N + y),
            z_terrain: z,
            z_rim: z,
            z_invert: z - 1.5,
            attrs: BTreeMap::new(),
            datos: BTreeMap::new(),
            estado: EstadoObjeto::Pendiente,
            nota: None,
            modelo: None,
            deleted: false,
        };
        let id = match p.add_node(nodo) {
            Ok(id) => id,
            Err(_) => continue,
        };

        // Familias lineales: un arco corto que sale del nodo, para que se dibuje el trazado.
        if f.geom == GeomKind::Polyline
            && let Some((prev_id, prev_pos)) = previo
        {
            let arco = Link {
                id: ElementId::new(0, 0),
                family: f.key.to_string(),
                ty,
                code: String::new(),
                from: prev_id,
                to: id,
                path: vec![
                    prev_pos,
                    P3::new(
                        (prev_pos.x + ORIGEN_E + x) * 0.5,
                        (prev_pos.y + ORIGEN_N + y) * 0.5,
                        z - 1.5,
                    ),
                    P3::new(ORIGEN_E + x, ORIGEN_N + y, z - 1.5),
                ],
                d_mm: 200,
                attrs: BTreeMap::new(),
                datos: BTreeMap::new(),
                estado: EstadoObjeto::Pendiente,
                nota: None,
                deleted: false,
            };
            let _ = p.add_link(arco);
        }
        previo = Some((id, P3::new(ORIGEN_E + x, ORIGEN_N + y, z - 1.5)));
    }
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hay_una_pieza_de_cada_familia_principal() {
        let p = catalogo();
        let mut vistos: Vec<&str> = p.nodes.iter().map(|n| n.family.as_str()).collect();
        vistos.sort_unstable();
        vistos.dedup();
        for f in [
            "saneamiento.pozo",
            "saneamiento.camara",
            "saneamiento.sumidero",
            "saneamiento.descarga",
            "agua.valvula",
            "agua.hidrante",
            "agua.medidor",
            "agua.tanque",
        ] {
            assert!(vistos.contains(&f), "falta la familia {f}");
        }
        assert!(p.count() > 20);
    }
}
