//! Demostración de la **cadena completa**: captación en quebrada → conducción → planta de
//! tratamiento → reservorio → distribución, y el alcantarillado hasta su planta y su descarga.
//!
//! Es la escena que responde a «¿dónde está el agua desde que se toma hasta que se devuelve?».
//! Las cotas son coherentes: todo baja por gravedad desde la captación (2530 m) hasta la descarga
//! (2490 m), y la planta se dispone en el sentido del flujo.

use std::collections::BTreeMap;

use crate::families::Value;
use crate::geom::{P2, P3};
use crate::model::{ElementId, EstadoObjeto, EstadoProyecto, Fuente, Link, Node, Project};
use crate::terrain::{SurfacePoint, Terrain};

const ORIGEN_E: f64 = 760_000.0;
const ORIGEN_N: f64 = 9_795_000.0;

/// Cota del terreno: ladera que baja de oeste a este, con la quebrada al norte.
fn cota(x: f64, y: f64) -> f64 {
    2532.0 - 0.085 * x + 0.02 * (y - 120.0).abs() - 1.2 * (-((y - 200.0) / 18.0).powi(2)).exp()
}

#[allow(clippy::too_many_arguments)]
fn nodo(
    p: &mut Project,
    familia: &str,
    ty: &str,
    code: &str,
    x: f64,
    y: f64,
    z: f64,
    attrs: &[(&str, f64)],
) -> ElementId {
    let mut a = BTreeMap::new();
    for (k, v) in attrs {
        a.insert((*k).to_string(), Value::Num(*v));
    }
    p.add_node(Node {
        id: ElementId::new(0, 0),
        family: familia.into(),
        ty: ty.into(),
        code: code.into(),
        pos: P2::new(ORIGEN_E + x, ORIGEN_N + y),
        z_terrain: cota(x, y),
        z_rim: z,
        z_invert: z - 1.8,
        attrs: a,
        datos: BTreeMap::new(),
        estado: EstadoObjeto::Pendiente,
        observaciones: Vec::new(),
        modelo: None,
        deleted: false,
        sin_dato: Vec::new(),
    })
    .unwrap_or_else(|e| panic!("{code}: {e}"))
}

#[allow(clippy::too_many_arguments)]
fn tuberia(
    p: &mut Project,
    familia: &str,
    ty: &str,
    code: &str,
    desde: (ElementId, [f64; 3]),
    hasta: (ElementId, [f64; 3]),
    dn: u32,
    material: &str,
) -> ElementId {
    let medio = [
        (desde.1[0] + hasta.1[0]) * 0.5,
        (desde.1[1] + hasta.1[1]) * 0.5,
        (desde.1[2] + hasta.1[2]) * 0.5,
    ];
    p.add_link(Link {
        id: ElementId::new(0, 0),
        family: familia.into(),
        ty: ty.into(),
        code: code.into(),
        from: desde.0,
        to: hasta.0,
        path: vec![
            P3::new(desde.1[0], desde.1[1], desde.1[2]),
            P3::new(medio[0], medio[1], medio[2]),
            P3::new(hasta.1[0], hasta.1[1], hasta.1[2]),
        ],
        d_mm: dn,
        attrs: BTreeMap::from([
            ("diametro_nominal".to_string(), Value::Num(dn as f64)),
            ("material".to_string(), Value::Text(material.to_string())),
        ]),
        datos: BTreeMap::new(),
        estado: EstadoObjeto::Pendiente,
        observaciones: Vec::new(),
        cota_solera_inicio: None,
        cota_solera_fin: None,
        deleted: false,
        sin_dato: Vec::new(),
    })
    .unwrap_or_else(|e| panic!("{code}: {e}"))
}

/// Source of the demo numbers that a real project would bring from its design (F17-03).
pub const DEMO_SOURCE: &str = "demo: dato de ejemplo, no de un diseño";

/// Wall thickness and material of a demo manhole, so the manhole recipe can bake it (D17: they
/// are design data). The thickness is a `Dato` that says it is demo data.
fn declare_demo_manhole_wall(p: &mut Project, code: &str) {
    let n = p
        .node_by_code_mut(code)
        .unwrap_or_else(|| panic!("{code}: demo manhole"));
    n.datos.insert(
        "espesor_pared".into(),
        crate::model::Dato {
            valor: 0.15,
            unidad: "m".into(),
            fuente: DEMO_SOURCE.into(),
        },
    );
    n.attrs
        .insert("material".into(), Value::Text("hormigon".into()));
}

/// The intake numbers the recipes take as design data (F17-06) and the demo plan lacks: window
/// height, wall and slab thickness and material. Each one says it is demo data.
const DEMO_INTAKE_DATA: &[(&str, &[(&str, f64)])] = &[
    ("BC-001", &[("altura_ventana", 0.5), ("espesor_muro", 0.3)]),
    ("DZ-001", &[("espesor_muro", 0.25)]),
    ("CA-001", &[("espesor_muro", 0.2), ("espesor_losa", 0.2)]),
    ("CA-002", &[("espesor_muro", 0.2), ("espesor_losa", 0.2)]),
];

fn declare_demo_intake_data(p: &mut Project) {
    for (code, data) in DEMO_INTAKE_DATA {
        let n = p
            .node_by_code_mut(code)
            .unwrap_or_else(|| panic!("{code}: demo intake structure"));
        for (key, value) in *data {
            n.datos.insert(
                (*key).into(),
                crate::model::Dato {
                    valor: *value,
                    unidad: "m".into(),
                    fuente: DEMO_SOURCE.into(),
                },
            );
        }
        n.attrs
            .insert("material".into(), Value::Text("hormigon".into()));
    }
}

/// Proyecto de la cadena completa (≈ 60 objetos).
pub fn cadena() -> Project {
    let mut p = Project::new("Cadena completa · San Bartolo", "EPSG:32717");
    p.origin = P2::new(ORIGEN_E, ORIGEN_N);
    p.estado = EstadoProyecto::EnRevision;
    p.fuentes = vec![Fuente {
        app: "Redes".into(),
        archivo: "cadena-demo.json".into(),
        formato: "plan".into(),
        importado_en: "2026-11-21".into(),
    }];

    // --- Terreno: ladera con la quebrada al norte ---
    let mut pts = Vec::new();
    let mut y = 0.0;
    while y <= 240.0 {
        let mut x = 0.0;
        while x <= 460.0 {
            pts.push(SurfacePoint::new(ORIGEN_E + x, ORIGEN_N + y, cota(x, y)));
            x += 10.0;
        }
        y += 10.0;
    }
    p.terrain = Some(Terrain::build("ladera", 32717, pts));

    // --- 1. Captación en la quebrada ---
    let bocatoma = nodo(
        &mut p,
        "agua.bocatoma",
        "barraje_fijo",
        "BC-001",
        20.0,
        200.0,
        2530.4,
        &[
            ("largo_barraje", 8.0),
            ("altura_barraje", 1.4),
            ("cota_cresta", 2530.4),
            ("cota_umbral", 2529.6),
            ("cota_fondo", 2528.6),
            ("ancho_ventana", 1.2),
        ],
    );
    let _reja = nodo(
        &mut p,
        "comun.reja",
        "gruesos_manual",
        "RJ-001",
        26.0,
        198.0,
        2530.0,
        &[
            ("ancho", 1.2),
            ("altura", 1.0),
            ("inclinacion", 70.0),
            ("abertura", 40.0),
            ("cota_umbral", 2529.6),
        ],
    );
    let desarenador = nodo(
        &mut p,
        "comun.desarenador",
        "longitudinal",
        "DZ-001",
        48.0,
        192.0,
        2528.8,
        &[
            ("largo", 9.0),
            ("ancho", 3.0),
            ("altura_util", 1.6),
            ("tirante", 1.0),
            ("cota_fondo", 2527.2),
            ("cota_agua", 2528.2),
            ("diametro_purga", 200.0),
            ("tipo_limpieza", 0.0),
        ],
    );
    let camara_carga = nodo(
        &mut p,
        "agua.camara",
        "carga",
        "CA-001",
        72.0,
        186.0,
        2527.6,
        &[
            ("largo", 3.0),
            ("ancho", 2.6),
            ("altura_util", 2.0),
            ("cota_fondo", 2525.6),
            ("cota_tapa", 2527.6),
            ("cota_agua", 2527.2),
            ("longitud_vertedero", 1.6),
            ("cota_cresta", 2527.2),
        ],
    );
    let camara_quiebre = nodo(
        &mut p,
        "agua.camara",
        "quiebre",
        "CA-002",
        150.0,
        168.0,
        2519.4,
        &[
            ("largo", 3.0),
            ("ancho", 2.6),
            ("altura_util", 1.8),
            ("cota_fondo", 2517.6),
            ("cota_tapa", 2519.4),
            ("cota_agua", 2519.0),
            ("altura_caida", 1.2),
        ],
    );
    let valvula_aire = nodo(
        &mut p,
        "agua.valvula",
        "aire",
        "VL-001",
        110.0,
        178.0,
        2523.4,
        &[("diametro_nominal", 100.0)],
    );
    let valvula_purga = nodo(
        &mut p,
        "agua.valvula",
        "purga",
        "VL-002",
        130.0,
        172.0,
        2521.6,
        &[("diametro_nominal", 100.0)],
    );

    let _ = tuberia(
        &mut p,
        "agua.tuberia",
        "agua_cruda",
        "TB-001",
        (bocatoma, [ORIGEN_E + 26.0, ORIGEN_N + 198.0, 2529.6]),
        (desarenador, [ORIGEN_E + 48.0, ORIGEN_N + 192.0, 2528.2]),
        200,
        "PVC",
    );
    let _ = tuberia(
        &mut p,
        "agua.tuberia",
        "agua_cruda",
        "TB-002",
        (desarenador, [ORIGEN_E + 52.0, ORIGEN_N + 192.0, 2528.0]),
        (camara_carga, [ORIGEN_E + 72.0, ORIGEN_N + 186.0, 2527.2]),
        200,
        "PVC",
    );
    let _ = tuberia(
        &mut p,
        "agua.tuberia",
        "conduccion",
        "TB-003",
        (camara_carga, [ORIGEN_E + 74.0, ORIGEN_N + 185.0, 2527.0]),
        (valvula_aire, [ORIGEN_E + 110.0, ORIGEN_N + 178.0, 2523.4]),
        250,
        "PVC",
    );
    let _ = tuberia(
        &mut p,
        "agua.tuberia",
        "conduccion",
        "TB-004",
        (valvula_aire, [ORIGEN_E + 111.0, ORIGEN_N + 177.0, 2523.2]),
        (valvula_purga, [ORIGEN_E + 130.0, ORIGEN_N + 172.0, 2521.6]),
        250,
        "PVC",
    );
    let _ = tuberia(
        &mut p,
        "agua.tuberia",
        "conduccion",
        "TB-005",
        (valvula_purga, [ORIGEN_E + 131.0, ORIGEN_N + 171.0, 2521.4]),
        (camara_quiebre, [ORIGEN_E + 150.0, ORIGEN_N + 168.0, 2519.0]),
        250,
        "PVC",
    );
    declare_demo_intake_data(&mut p);

    // --- 2. Planta de tratamiento (PTAP) ---
    let mezcla = nodo(
        &mut p,
        "agua.mezcla",
        "resalto_hidraulico",
        "MZ-001",
        178.0,
        160.0,
        2517.4,
        &[
            ("largo", 3.0),
            ("ancho", 2.4),
            ("altura_util", 1.6),
            ("cota_fondo", 2515.6),
            ("altura_caida", 0.5),
            ("longitud_resalto", 2.0),
        ],
    );
    let floculador = nodo(
        &mut p,
        "agua.floculador",
        "pantallas_horizontales",
        "FL-001",
        196.0,
        160.0,
        2517.0,
        &[
            ("largo", 9.0),
            ("ancho", 5.0),
            ("altura_util", 2.8),
            ("cota_fondo", 2514.2),
            ("numero_camaras", 3.0),
            ("numero_pantallas", 6.0),
            ("separacion_pantallas", 0.8),
        ],
    );
    let sedimentador = nodo(
        &mut p,
        "agua.sedimentador",
        "convencional",
        "SD-001",
        216.0,
        160.0,
        2516.6,
        &[
            ("largo", 11.0),
            ("ancho", 6.0),
            ("altura_util", 3.0),
            ("tirante", 2.4),
            ("cota_fondo", 2513.6),
            ("cota_agua", 2516.0),
            ("diametro_purga", 200.0),
        ],
    );
    let filtro = nodo(
        &mut p,
        "agua.filtro",
        "rapido_gravedad",
        "FI-001",
        238.0,
        160.0,
        2516.2,
        &[
            ("largo", 8.0),
            ("ancho", 6.0),
            ("altura_total", 3.2),
            ("altura_lecho", 0.9),
            ("altura_borde_libre", 0.5),
            ("cota_fondo", 2513.0),
            ("numero_celdas", 3.0),
        ],
    );
    let desinfeccion = nodo(
        &mut p,
        "agua.desinfeccion",
        "hipoclorito_sodio",
        "DI-001",
        254.0,
        160.0,
        2516.0,
        &[
            ("diametro_conexion", 200.0),
            ("capacidad_tanque", 1.2),
            ("numero_bombas", 2.0),
        ],
    );
    let contacto = nodo(
        &mut p,
        "agua.tanque",
        "contacto",
        "TQ-001",
        268.0,
        160.0,
        2515.8,
        &[("diametro_interior", 4.0), ("volumen_util", 50.0)],
    );
    let caseta_dosif = nodo(
        &mut p,
        "comun.caseta",
        "dosificacion",
        "CS-001",
        196.0,
        174.0,
        2517.2,
        &[
            ("largo", 5.0),
            ("ancho", 4.0),
            ("altura", 3.0),
            ("cota_piso", 2517.2),
        ],
    );
    let laboratorio = nodo(
        &mut p,
        "comun.caseta",
        "laboratorio",
        "CS-002",
        226.0,
        176.0,
        2516.8,
        &[
            ("largo", 6.0),
            ("ancho", 4.5),
            ("altura", 3.0),
            ("cota_piso", 2516.8),
        ],
    );
    let lecho = nodo(
        &mut p,
        "comun.lecho_secado",
        "arena_grava",
        "LS-001",
        232.0,
        138.0,
        2515.2,
        &[
            ("largo", 12.0),
            ("ancho", 8.0),
            ("altura_muro", 1.0),
            ("numero_celdas", 3.0),
            ("pendiente_fondo", 0.01),
            ("cota_fondo", 2515.2),
            ("espesor_arena", 0.3),
            ("espesor_grava", 0.25),
        ],
    );
    let aforador = nodo(
        &mut p,
        "comun.aforador",
        "parshall",
        "AF-001",
        168.0,
        160.0,
        2517.6,
        &[
            ("ancho_canal", 1.6),
            ("cota_umbral", 2517.2),
            ("ancho_garganta", 0.3),
            ("longitud_garganta", 0.6),
        ],
    );

    let _ = tuberia(
        &mut p,
        "agua.tuberia",
        "aduccion",
        "TB-006",
        (camara_quiebre, [ORIGEN_E + 151.0, ORIGEN_N + 167.0, 2518.8]),
        (aforador, [ORIGEN_E + 168.0, ORIGEN_N + 160.0, 2517.4]),
        250,
        "PVC",
    );
    let _ = tuberia(
        &mut p,
        "agua.tuberia",
        "aduccion",
        "TB-007",
        (aforador, [ORIGEN_E + 170.0, ORIGEN_N + 160.0, 2517.4]),
        (mezcla, [ORIGEN_E + 178.0, ORIGEN_N + 160.0, 2517.0]),
        250,
        "PVC",
    );
    let _ = tuberia(
        &mut p,
        "agua.tuberia",
        "aduccion",
        "TB-008",
        (mezcla, [ORIGEN_E + 180.0, ORIGEN_N + 160.0, 2516.6]),
        (floculador, [ORIGEN_E + 196.0, ORIGEN_N + 160.0, 2516.4]),
        250,
        "PVC",
    );
    let _ = tuberia(
        &mut p,
        "agua.tuberia",
        "aduccion",
        "TB-009",
        (floculador, [ORIGEN_E + 201.0, ORIGEN_N + 160.0, 2516.2]),
        (sedimentador, [ORIGEN_E + 216.0, ORIGEN_N + 160.0, 2516.0]),
        250,
        "PVC",
    );
    let _ = tuberia(
        &mut p,
        "agua.tuberia",
        "aduccion",
        "TB-010",
        (sedimentador, [ORIGEN_E + 222.0, ORIGEN_N + 160.0, 2515.8]),
        (filtro, [ORIGEN_E + 238.0, ORIGEN_N + 160.0, 2515.6]),
        250,
        "PVC",
    );
    let _ = tuberia(
        &mut p,
        "agua.tuberia",
        "conduccion",
        "TB-011",
        (filtro, [ORIGEN_E + 242.0, ORIGEN_N + 160.0, 2515.5]),
        (desinfeccion, [ORIGEN_E + 254.0, ORIGEN_N + 160.0, 2515.4]),
        250,
        "PVC",
    );
    let _ = tuberia(
        &mut p,
        "agua.tuberia",
        "conduccion",
        "TB-012",
        (desinfeccion, [ORIGEN_E + 256.0, ORIGEN_N + 160.0, 2515.3]),
        (contacto, [ORIGEN_E + 268.0, ORIGEN_N + 160.0, 2515.2]),
        250,
        "PVC",
    );

    // --- 3. Reservorio y distribución ---
    let reservorio = nodo(
        &mut p,
        "agua.tanque",
        "apoyado_cilindrico",
        "TQ-002",
        292.0,
        160.0,
        2515.0,
        &[
            ("diametro_interior", 10.0),
            ("volumen_util", 400.0),
            ("cota_rebose", 2514.6),
        ],
    );
    let _ = tuberia(
        &mut p,
        "agua.tuberia",
        "conduccion",
        "TB-013",
        (contacto, [ORIGEN_E + 272.0, ORIGEN_N + 160.0, 2515.2]),
        (reservorio, [ORIGEN_E + 292.0, ORIGEN_N + 160.0, 2514.8]),
        250,
        "PVC",
    );

    // Malla de distribución: seis calles de este a oeste con válvulas e hidrantes.
    let mut prev = (reservorio, [ORIGEN_E + 297.0, ORIGEN_N + 160.0, 2514.4]);
    let mut n = 0;
    for fila in 0..3 {
        let y = 120.0 + fila as f64 * 30.0;
        for col in 0..4 {
            n += 1;
            let x = 310.0 + col as f64 * 30.0;
            let z = cota(x, y) + 0.6;
            let ty = if col == 0 {
                "distribucion_primaria"
            } else {
                "distribucion_secundaria"
            };
            let code = format!("PZ-2{:02}", n);
            let pozo = nodo(
                &mut p,
                "saneamiento.pozo",
                "inspeccion",
                &code,
                x,
                y,
                cota(x, y) + 0.05,
                &[("diametro_interior", 1.2)],
            );
            declare_demo_manhole_wall(&mut p, &code);
            let _ = pozo;
            let val = nodo(
                &mut p,
                "agua.valvula",
                "compuerta",
                &format!("VL-1{:02}", n),
                x + 8.0,
                y,
                z,
                &[("diametro_nominal", 150.0)],
            );
            let _ = tuberia(
                &mut p,
                "agua.tuberia",
                ty,
                &format!("TB-1{:02}", n),
                prev,
                (val, [ORIGEN_E + x + 8.0, ORIGEN_N + y, z]),
                150,
                "PVC",
            );
            prev = (val, [ORIGEN_E + x + 8.0, ORIGEN_N + y, z]);
        }
    }

    // --- 4. Alcantarillado hasta la PTAR ---
    let mut prev_alc: Option<(ElementId, [f64; 3])> = None;
    for k in 0..6 {
        let x = 310.0 + k as f64 * 22.0;
        let y = 100.0 - k as f64 * 4.0;
        let z = cota(x, y);
        let code = format!("PZ-3{:02}", k + 1);
        let pozo = nodo(
            &mut p,
            "saneamiento.pozo",
            "inspeccion",
            &code,
            x,
            y,
            z + 0.05,
            &[("diametro_interior", 1.2)],
        );
        declare_demo_manhole_wall(&mut p, &code);
        if let Some(prev) = prev_alc {
            let _ = tuberia(
                &mut p,
                "saneamiento.tuberia",
                "colector_sanitario",
                &format!("CL-3{:02}", k + 1),
                prev,
                (pozo, [ORIGEN_E + x, ORIGEN_N + y, z - 1.5]),
                200,
                "hormigon",
            );
        }
        prev_alc = Some((pozo, [ORIGEN_E + x, ORIGEN_N + y, z - 1.5]));
    }
    let reja_ptar = nodo(
        &mut p,
        "comun.reja",
        "gruesos_mecanica",
        "RJ-002",
        452.0,
        74.0,
        2501.2,
        &[
            ("ancho", 1.6),
            ("altura", 1.2),
            ("abertura", 25.0),
            ("inclinacion", 75.0),
            ("cota_umbral", 2500.4),
        ],
    );
    let desarenador_ptar = nodo(
        &mut p,
        "comun.desarenador",
        "dos_naves",
        "DZ-002",
        464.0,
        74.0,
        2501.0,
        &[
            ("largo", 10.0),
            ("ancho", 5.0),
            ("altura_util", 1.8),
            ("tirante", 1.2),
            ("cota_fondo", 2499.2),
            ("cota_agua", 2500.4),
            ("diametro_purga", 200.0),
            ("numero_naves", 2.0),
            ("numero_compuertas", 2.0),
        ],
    );
    let reactor = nodo(
        &mut p,
        "saneamiento.reactor",
        "uasb",
        "RX-001",
        486.0,
        74.0,
        2500.8,
        &[
            ("largo", 12.0),
            ("ancho", 8.0),
            ("altura_util", 4.5),
            ("tirante", 4.0),
            ("cota_fondo", 2496.2),
            ("cota_agua", 2500.2),
            ("numero_camaras", 2.0),
            ("numero_difusores", 6.0),
        ],
    );
    let sedimentador_ptar = nodo(
        &mut p,
        "saneamiento.sedimentador",
        "secundario",
        "SE-001",
        510.0,
        74.0,
        2500.4,
        &[
            ("diametro_interior", 9.0),
            ("tirante", 2.6),
            ("cota_cresta", 2500.0),
            ("cota_agua", 2500.0),
            ("cota_fondo", 2497.4),
            ("diametro_purga", 200.0),
        ],
    );
    let laguna = nodo(
        &mut p,
        "saneamiento.laguna",
        "maduracion",
        "LG-001",
        540.0,
        74.0,
        2500.0,
        &[
            ("largo", 40.0),
            ("ancho", 24.0),
            ("profundidad", 1.6),
            ("cota_fondo", 2498.2),
            ("cota_agua", 2499.6),
            ("talud", 2.5),
            ("ancho_corona", 3.0),
            ("numero_celdas", 2.0),
        ],
    );
    let descarga = nodo(
        &mut p,
        "comun.cuerpo_receptor",
        "rio",
        "CR-001",
        570.0,
        74.0,
        2498.0,
        &[],
    );

    let _ = tuberia(
        &mut p,
        "saneamiento.tuberia",
        "colector_sanitario",
        "CL-307",
        (
            prev_alc.map(|x| x.0).unwrap_or(reja_ptar),
            [ORIGEN_E + 420.0, ORIGEN_N + 76.0, 2500.2],
        ),
        (reja_ptar, [ORIGEN_E + 452.0, ORIGEN_N + 74.0, 2500.4]),
        250,
        "hormigon",
    );
    let _ = tuberia(
        &mut p,
        "saneamiento.tuberia",
        "colector_sanitario",
        "CL-308",
        (reja_ptar, [ORIGEN_E + 454.0, ORIGEN_N + 74.0, 2500.4]),
        (
            desarenador_ptar,
            [ORIGEN_E + 464.0, ORIGEN_N + 74.0, 2500.2],
        ),
        250,
        "hormigon",
    );
    let _ = tuberia(
        &mut p,
        "saneamiento.tuberia",
        "colector_sanitario",
        "CL-309",
        (
            desarenador_ptar,
            [ORIGEN_E + 470.0, ORIGEN_N + 74.0, 2500.0],
        ),
        (reactor, [ORIGEN_E + 486.0, ORIGEN_N + 74.0, 2500.0]),
        250,
        "hormigon",
    );
    let _ = tuberia(
        &mut p,
        "saneamiento.tuberia",
        "colector_sanitario",
        "CL-310",
        (reactor, [ORIGEN_E + 494.0, ORIGEN_N + 74.0, 2499.8]),
        (
            sedimentador_ptar,
            [ORIGEN_E + 510.0, ORIGEN_N + 74.0, 2499.8],
        ),
        250,
        "hormigon",
    );
    let _ = tuberia(
        &mut p,
        "saneamiento.tuberia",
        "emisario",
        "CL-311",
        (
            sedimentador_ptar,
            [ORIGEN_E + 516.0, ORIGEN_N + 74.0, 2499.6],
        ),
        (laguna, [ORIGEN_E + 540.0, ORIGEN_N + 74.0, 2499.4]),
        300,
        "hormigon",
    );
    let _ = tuberia(
        &mut p,
        "saneamiento.tuberia",
        "emisario",
        "CL-312",
        (laguna, [ORIGEN_E + 560.0, ORIGEN_N + 74.0, 2498.6]),
        (descarga, [ORIGEN_E + 570.0, ORIGEN_N + 74.0, 2498.0]),
        300,
        "hormigon",
    );

    let _ = aforador;
    let _ = caseta_dosif;
    let _ = laboratorio;
    let _ = lecho;
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_cadena_cubre_las_etapas() {
        let p = cadena();
        let mut familias: Vec<&str> = p.nodes.iter().map(|n| n.family.as_str()).collect();
        familias.sort();
        familias.dedup();
        for esperada in [
            "agua.bocatoma",
            "comun.desarenador",
            "agua.camara",
            "agua.mezcla",
            "agua.floculador",
            "agua.sedimentador",
            "agua.filtro",
            "agua.desinfeccion",
            "agua.tanque",
            "saneamiento.pozo",
            "saneamiento.reactor",
            "saneamiento.laguna",
        ] {
            assert!(
                familias.contains(&esperada),
                "falta {esperada} en la cadena"
            );
        }
        assert!(p.count() > 45, "la cadena debe tener una red completa");
    }

    #[test]
    fn el_agua_baja_de_la_captacion_a_la_descarga() {
        let p = cadena();
        let alta = p.nodes.iter().map(|n| n.z_terrain).fold(f64::MIN, f64::max);
        let baja = p.nodes.iter().map(|n| n.z_terrain).fold(f64::MAX, f64::min);
        assert!(
            alta - baja > 25.0,
            "la cadena debe salvar desnivel: {alta} -> {baja}"
        );
        let captacion = p
            .nodes
            .iter()
            .find(|n| n.family == "agua.bocatoma")
            .unwrap();
        let descarga = p
            .nodes
            .iter()
            .find(|n| n.family == "comun.cuerpo_receptor")
            .unwrap();
        assert!(
            captacion.z_terrain > descarga.z_terrain,
            "captación aguas arriba de la descarga"
        );
    }
}
