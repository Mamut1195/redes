//! Ingesta con archivos de verdad: lo que entra al producto (plan, DXF de Redes y terreno, D5).
//!
//! Las fixtures viven en `tests/fixtures` y son pequeñas a propósito: lo que se comprueba es el
//! contrato (qué entra, qué se descarta y qué se dice), no el volumen.

use std::path::PathBuf;

fn fixture(nombre: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(nombre)
}

use redes_core::model::Project;

// ---------------------------------------------------------------------------------------------
// DXF (F12-09, D5): solo entra el DXF que exportó Redes. El de otra aplicación se rechaza con un
// código claro y sin tocar el proyecto; lo adapta la skill de ingesta.

/// Importa un DXF escrito en el momento y devuelve el error, comprobando que el proyecto queda
/// intacto (ni objetos, ni fuentes, ni revisión).
fn dxf_rechazado(nombre: &str, contenido: &str) -> String {
    let ruta = std::env::temp_dir().join(format!("redes-{nombre}-{}.dxf", std::process::id()));
    std::fs::write(&ruta, contenido).unwrap();
    let mut p = Project::new("dxf", "EPSG:32717");
    let revision = p.revision;
    let err = redes_core::ingest::import_dxf(&mut p, &ruta, 0.5).expect_err("debe rechazarse");
    let _ = std::fs::remove_file(&ruta);
    assert_eq!(p.count(), 0, "{nombre}: no entra nada");
    assert!(p.fuentes.is_empty(), "{nombre}: no anota fuente");
    assert_eq!(p.revision, revision, "{nombre}: no toca la revisión");
    err
}

/// Un DXF de CAD con pozos y un colector en capas con nombre, pero sin `REDES_INFO`.
const DXF_AJENO: &str = "0\nSECTION\n2\nENTITIES\n\
0\nPOINT\n8\nPOZOS\n10\n758010.0\n20\n9797010.0\n30\n2500.0\n\
0\nPOINT\n8\nPOZOS\n10\n758040.0\n20\n9797012.0\n30\n2498.5\n\
0\nLINE\n8\nALCANTARILLADO\n10\n758010.0\n20\n9797010.0\n30\n2500.0\n11\n758040.0\n21\n9797012.0\n31\n2498.5\n\
0\nENDSEC\n0\nEOF\n";

#[test]
fn un_dxf_de_otra_aplicacion_se_rechaza_con_e_fmt_not_redes() {
    let err = dxf_rechazado("ajeno", DXF_AJENO);
    assert!(err.starts_with("E_FMT_NOT_REDES:"), "{err}");
    assert!(err.contains("REDES_INFO"), "dice qué falta: {err}");
}

#[test]
fn un_dxf_vacio_o_ilegible_se_rechaza_con_e_fmt_parse() {
    assert!(dxf_rechazado("vacio", "").starts_with("E_FMT_PARSE:"));
    // Truncado: sin EOF no se sabe si está completo.
    let truncado = DXF_AJENO.replace("0\nEOF\n", "");
    assert!(dxf_rechazado("truncado", &truncado).starts_with("E_FMT_PARSE:"));
}

#[test]
fn una_capa_redes_info_sin_objetos_legibles_se_rechaza() {
    let dxf = "0\nSECTION\n2\nENTITIES\n0\nTEXT\n8\nREDES_INFO\n10\n0\n20\n0\n30\n0\n1\nREDES|1|N|roto\n\
0\nENDSEC\n0\nEOF\n";
    assert!(dxf_rechazado("info-rota", dxf).starts_with("E_FMT_NOT_REDES:"));
}

#[test]
fn el_terreno_en_csv_construye_el_tin() {
    let mut p = Project::new("terreno", "EPSG:32717");
    let rep = redes_core::ingest::import_terreno(&mut p, &fixture("terreno.csv"), 32717)
        .expect("terreno");
    assert_eq!(rep.puntos_terreno, 9);
    let t = p.terrain.as_ref().expect("TIN");
    assert!(t.tris.len() >= 8, "triángulos: {}", t.tris.len());
    assert!(
        t.z_at(redes_core::geom::P2::new(758_020.0, 9_797_020.0))
            .is_some()
    );
}

#[test]
fn un_plan_de_la_skill_entra_completo() {
    let mut p = Project::new("plan", "EPSG:32717");
    let rep = redes_core::ingest::import_plan(&mut p, &fixture("plan.json")).expect("plan");
    assert_eq!(rep.nodos, 2);
    assert_eq!(rep.arcos, 1);
    assert!(rep.descartados.is_empty(), "{:?}", rep.descartados);
    assert!(p.nodes.iter().any(|n| n.code == "PZ-901"));
    let cl = tramo(&p, "CL-901");
    assert!((cl.path[0].z - 2498.4).abs() < 1e-9, "los vértices entran");
}

#[test]
fn un_terreno_con_una_fila_mala_avisa_y_sigue() {
    let mut p = Project::new("terreno-malo", "EPSG:32717");
    let ruta = std::env::temp_dir().join(format!("redes-terreno-{}.csv", std::process::id()));
    std::fs::write(
        &ruta,
        "E,N,Z\n758000,9797000,2500\n758020,texto,2501\n758040,9797040,2502\n758060,9797060,2503\n",
    )
    .unwrap();
    let rep = redes_core::ingest::import_terreno(&mut p, &ruta, 32717).expect("importa");
    assert_eq!(rep.puntos_terreno, 3, "descarta la fila no numérica");
    assert!(!rep.descartados.is_empty(), "y lo dice");
    let _ = std::fs::remove_file(&ruta);
}

// ---------------------------------------------------------------------------------------------
// SWMM y EPANET (F12-04, F12-05): los planes los escribió un agente con la skill
// `skills/redes-ingesta` (D5) a partir de los `.inp` de esta carpeta; `swmm.md` §6 y `epanet.md` §6
// explican cada línea. Son ejemplos y pruebas del contrato `redes.plan/1`.

fn importa_plan(nombre: &str) -> (Project, redes_core::ingest::ImportReport) {
    importa_plan_descartando(nombre, &[])
}

/// Importa un plan de fixture y exige que se descarten exactamente esos nodos, por no traer
/// ninguna cota (F12-03: antes entraban a cota 0).
fn importa_plan_descartando(
    nombre: &str,
    sin_cota: &[&str],
) -> (Project, redes_core::ingest::ImportReport) {
    let mut p = Project::new(nombre, "EPSG:32717");
    let rep = redes_core::ingest::import_plan(&mut p, &fixture(nombre)).expect("plan");
    assert_eq!(
        rep.descartados.len(),
        sin_cota.len(),
        "{nombre}: {:?}",
        rep.descartados
    );
    for c in sin_cota {
        assert!(
            rep.descartados
                .iter()
                .any(|d| d.starts_with(&format!("nodo {c}:")) && d.contains("sin ninguna cota")),
            "{nombre}: {c} {:?}",
            rep.descartados
        );
        assert!(p.nodes.iter().all(|n| n.code != *c));
    }
    assert!(
        rep.renombrados.is_empty(),
        "{nombre}: {:?}",
        rep.renombrados
    );
    (p, rep)
}

fn nodo<'a>(p: &'a Project, codigo: &str) -> &'a redes_core::model::Node {
    p.nodes.iter().find(|n| n.code == codigo).expect(codigo)
}

fn tramo<'a>(p: &'a Project, codigo: &str) -> &'a redes_core::model::Link {
    p.links.iter().find(|l| l.code == codigo).expect(codigo)
}

#[test]
fn un_swmm_en_cms_entra_con_saltos_y_secciones() {
    let (p, rep) = importa_plan("swmm_si_cms.plan.json");
    assert_eq!((rep.nodos, rep.arcos), (7, 4));
    // C1 llega 0,25 m por encima de la solera de J2: el salto en el pozo se conserva.
    let c1 = tramo(&p, "C1");
    assert_eq!(c1.ty, "colector_pluvial");
    assert_eq!(c1.d_mm, 600);
    assert!((c1.path.last().unwrap().z - 2510.05).abs() < 1e-6);
    assert!((nodo(&p, "J2").z_invert - 2509.8).abs() < 1e-6);
    assert_eq!(c1.path.len(), 3, "vértice intermedio de [VERTICES]");
    assert_eq!(tramo(&p, "C3").family, "saneamiento.canal");
    assert_eq!(tramo(&p, "C3").ty, "box_culvert");
    assert_eq!(nodo(&p, "ST1").family, "saneamiento.bombeo");
    assert_eq!(nodo(&p, "OF1").family, "saneamiento.descarga");
    assert_eq!(nodo(&p, "W1").ty, "alivio_lateral");
}

#[test]
fn un_swmm_en_cfs_entra_en_metros() {
    let (p, rep) = importa_plan("swmm_us_cfs.plan.json");
    assert_eq!((rep.nodos, rep.arcos), (8, 5));
    assert!((nodo(&p, "MH1").z_invert - 820.0 * 0.3048).abs() < 1e-6);
    let p1 = tramo(&p, "P-1");
    assert_eq!(p1.ty, "colector_sanitario");
    assert_eq!(p1.d_mm, 305, "1 ft");
    // OutOffset en modo ELEVATION: 816,25 ft, 0,75 ft sobre la solera de MH2.
    assert!((p1.path.last().unwrap().z - 816.25 * 0.3048).abs() < 1e-4);
    assert_eq!(tramo(&p, "FM-1").ty, "impulsion");
    let g5 = tramo(&p, "G-5");
    assert!(
        (g5.datos["altura_seccion"].valor - 0.6096).abs() < 1e-9,
        "el ovoide conserva su altura"
    );
    assert_eq!(
        g5.attrs.get("seccion").and_then(|v| v.as_str()),
        Some("ovoide")
    );
    assert_eq!(g5.dn(), None, "un ovoide sin DN no recibe uno");
    assert_eq!(p1.num("diametro_interior"), Some(304.8), "DI de SWMM");
    // La bomba (su cámara húmeda) une WW1 con la impulsión: la red sigue entera.
    let topo = redes_core::topology::Topology::build(&p);
    assert_eq!(topo.components.len(), 1, "{:?}", topo.components);
}

#[test]
fn un_epanet_en_lps_entra_sin_inventar_tanques() {
    // EPANET no da cota a la bomba ni a la captación: se descartan, no entran a cota 0.
    let (p, rep) = importa_plan_descartando("epanet_si_lps.plan.json", &["B1", "CAPT"]);
    assert_eq!((rep.nodos, rep.arcos), (9, 6));
    let tq = nodo(&p, "TQ1");
    assert_eq!(tq.num("cota_nivel_max"), Some(2533.2));
    assert_eq!(tq.num("volumen_util"), None, "EPANET no da volumen útil");
    assert_eq!(nodo(&p, "N2").family, "comun.accesorio");
    assert_eq!(nodo(&p, "PRV1").family, "agua.valvula");
    assert_eq!(nodo(&p, "PRV1").ty, "reduccion_presion");
    // EPANET no da soleras: los vértices van sin Z y el tramo queda sin cotas de dato.
    let t1 = tramo(&p, "T1");
    assert_eq!(t1.path.len(), 4);
    assert_eq!(t1.cotas_trazado(), None);
    // El nudo trae la cota del eje, no la solera (F12-15).
    let n2 = nodo(&p, "N2");
    assert_eq!(n2.cota_solera(), None);
    assert_eq!(n2.datos["cota_eje"].valor, 2512.5);
    // La válvula une N4 con N5 sin tramo inventado: la red no queda cortada.
    let topo = redes_core::topology::Topology::build(&p);
    let comp = |c: &str| {
        let id = nodo(&p, c).id;
        topo.components.iter().position(|k| k.contains(&id))
    };
    assert_eq!(comp("N4"), comp("N6"), "{:?}", topo.components);
    assert_eq!(comp("TQ1"), comp("N6"));
}

#[test]
fn un_epanet_en_gpm_entra_en_milimetros_y_metros() {
    let (p, rep) = importa_plan_descartando("epanet_us_gpm.plan.json", &["PU1"]);
    assert_eq!((rep.nodos, rep.arcos), (8, 5));
    assert_eq!(tramo(&p, "102").d_mm, 203, "8 in");
    assert!((nodo(&p, "J20").datos["cota_eje"].valor - 690.0 * 0.3048).abs() < 1e-6);
    assert_eq!(
        nodo(&p, "J20").cota_solera(),
        None,
        "el Elev de EPANET no es solera"
    );
    assert_eq!(
        nodo(&p, "TWR").num("diametro"),
        Some(12192.0),
        "40 ft en mm"
    );
    assert!(
        p.nodes.iter().all(|n| n.code != "LAKE"),
        "el RESERVOIR sin --reservorio se descarta en el script, no se convierte en terreno"
    );
}

// ---------------------------------------------------------------------------------------------
// No inventar datos (F12-03): lo que el plan no trae queda ausente, se avisa y se ve como `—`.

fn plan_en_memoria(
    nombre: &str,
    plan: serde_json::Value,
) -> (Project, redes_core::ingest::ImportReport) {
    let ruta = std::env::temp_dir().join(format!("redes-{nombre}-{}.json", std::process::id()));
    std::fs::write(&ruta, plan.to_string()).unwrap();
    let mut p = Project::new(nombre, "EPSG:32717");
    let rep = redes_core::ingest::import_plan(&mut p, &ruta).expect("importa");
    let _ = std::fs::remove_file(&ruta);
    (p, rep)
}

fn plan_con(objetos: serde_json::Value, terreno: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "schema": "redes.plan/1",
        "proyecto": { "nombre": "x", "crs": "EPSG:32717" },
        "terreno": terreno,
        "objetos": objetos
    })
}

#[test]
fn un_pozo_solo_con_solera_no_recibe_terreno_ni_tapa() {
    let (p, rep) = plan_en_memoria(
        "solo-solera",
        plan_con(
            serde_json::json!([
                { "tipo": "nodo", "codigo": "P1", "familia": "saneamiento.pozo", "e": 760000.0, "n": 9796000.0, "cota": 2498.5 }
            ]),
            serde_json::json!([]),
        ),
    );
    let n = nodo(&p, "P1");
    assert_eq!(n.cota_solera(), Some(2498.5));
    assert_eq!(n.cota_terreno(), None, "antes: terreno = solera");
    assert_eq!(n.cota_tapa(), None, "antes: tapa = terreno");
    assert_eq!(n.altura_dato(), None);
    assert!(n.sin_altura());
    assert!(
        rep.avisos
            .iter()
            .any(|a| a.contains("P1") && a.contains("cota_terreno")),
        "{:?}",
        rep.avisos
    );
    // Se dibuja un símbolo plano, no un pozo con profundidad.
    let malla = redes_core::view::pieza_para_exportar(n);
    let alto = malla
        .tris
        .iter()
        .flat_map(|t| t.p.iter().map(|v| v[2]))
        .fold(0.0f32, f32::max);
    assert!(alto <= 0.051, "símbolo plano: {alto}");
}

#[test]
fn un_pozo_solo_con_terreno_no_recibe_solera() {
    let (p, rep) = plan_en_memoria(
        "solo-terreno",
        plan_con(
            serde_json::json!([
                { "tipo": "nodo", "codigo": "P1", "familia": "saneamiento.pozo", "e": 760000.0, "n": 9796000.0, "cota_terreno": 2500.0 }
            ]),
            serde_json::json!([]),
        ),
    );
    let n = nodo(&p, "P1");
    assert_eq!(n.cota_terreno(), Some(2500.0));
    assert_eq!(n.cota_solera(), None, "antes: solera = terreno − 1,5");
    assert_eq!(n.cota_tapa(), None);
    assert!(n.sin_altura());
    assert!(!rep.avisos.is_empty());
}

#[test]
fn con_solera_y_terreno_la_tapa_sigue_ausente_pero_el_pozo_tiene_altura() {
    let (p, _) = plan_en_memoria(
        "sin-tapa",
        plan_con(
            serde_json::json!([
                { "tipo": "nodo", "codigo": "P1", "familia": "saneamiento.pozo", "e": 760000.0, "n": 9796000.0, "cota": 2498.0, "cota_terreno": 2500.0 },
                { "tipo": "nodo", "codigo": "P2", "familia": "saneamiento.pozo", "e": 760010.0, "n": 9796000.0, "cota": 2497.0, "cota_terreno": 2500.0, "cota_tapa": 2500.2 }
            ]),
            serde_json::json!([]),
        ),
    );
    let p1 = nodo(&p, "P1");
    assert_eq!(p1.cota_tapa(), None, "la tapa no se copia del terreno");
    assert!(!p1.sin_altura());
    assert_eq!(p1.altura_dato(), Some(2.0));
    assert_eq!(nodo(&p, "P2").cota_tapa(), Some(2500.2));
    let h = nodo(&p, "P2").altura_dato().unwrap();
    assert!((h - 3.2).abs() < 1e-9, "tapa − solera: {h}");
}

#[test]
fn un_nodo_sin_ninguna_cota_se_descarta_con_motivo() {
    let mut p = Project::new("sin-cota", "EPSG:32717");
    let ruta = std::env::temp_dir().join(format!("redes-sin-cota-{}.json", std::process::id()));
    std::fs::write(
        &ruta,
        plan_con(
            serde_json::json!([
                { "tipo": "nodo", "codigo": "P1", "familia": "saneamiento.pozo", "e": 760000.0, "n": 9796000.0 }
            ]),
            serde_json::json!([]),
        )
        .to_string(),
    )
    .unwrap();
    let rep = redes_core::ingest::import_plan(&mut p, &ruta).expect("importa");
    let _ = std::fs::remove_file(&ruta);
    assert_eq!(rep.nodos, 0, "antes entraba a cota 0");
    assert!(
        rep.descartados
            .iter()
            .any(|d| d.contains("P1") && d.contains("sin ninguna cota")),
        "{:?}",
        rep.descartados
    );
}

#[test]
fn la_cota_de_terreno_del_tin_del_plan_entra_con_aviso() {
    let (p, rep) = plan_en_memoria(
        "tin",
        plan_con(
            serde_json::json!([
                { "tipo": "nodo", "codigo": "P1", "familia": "saneamiento.pozo", "e": 760010.0, "n": 9796010.0, "cota": 2498.0 }
            ]),
            serde_json::json!([
                [760000.0, 9796000.0, 2500.0],
                [760100.0, 9796000.0, 2500.0],
                [760000.0, 9796100.0, 2500.0]
            ]),
        ),
    );
    assert!((nodo(&p, "P1").cota_terreno().unwrap() - 2500.0).abs() < 1e-6);
    assert!(
        rep.avisos.iter().any(|a| a.contains("TIN")),
        "{:?}",
        rep.avisos
    );
}

#[test]
fn un_tramo_sin_diametro_queda_sin_dn_y_se_dibuja_sin_dato() {
    let (p, rep) = plan_en_memoria(
        "sin-dn",
        plan_con(
            serde_json::json!([
                { "tipo": "nodo", "codigo": "P1", "familia": "saneamiento.pozo", "e": 760000.0, "n": 9796000.0, "cota": 2498.5, "cota_terreno": 2500.0 },
                { "tipo": "nodo", "codigo": "P2", "familia": "saneamiento.pozo", "e": 760050.0, "n": 9796000.0, "cota": 2498.2, "cota_terreno": 2500.0 },
                { "tipo": "arco", "codigo": "C1", "familia": "saneamiento.tuberia", "desde": "P1", "hasta": "P2" },
                { "tipo": "arco", "codigo": "C2", "familia": "saneamiento.tuberia", "desde": "P2", "hasta": "P1", "diametro_mm": 315 }
            ]),
            serde_json::json!([]),
        ),
    );
    let c1 = tramo(&p, "C1");
    assert_eq!(c1.dn(), None, "antes: DN 200");
    assert_eq!(c1.d_mm, 0);
    assert!(c1.dibujo_sin_dato());
    assert!(
        rep.avisos.iter().any(|a| a.contains("C1")),
        "{:?}",
        rep.avisos
    );
    assert_eq!(tramo(&p, "C2").dn(), Some(315));
    assert!(!tramo(&p, "C2").dibujo_sin_dato());
    // Cantidades: el tramo sin DN va en su grupo «DN —», no en «DN 200».
    let filas = redes_core::cantidades::longitudes(&p);
    assert!(filas.iter().any(|f| f.clave.contains("DN —")), "{filas:?}");
    assert!(filas.iter().all(|f| !f.clave.contains("DN 200")));
}

#[test]
fn un_tramo_entre_pozos_sin_solera_no_tiene_cotas_de_dato() {
    let (p, _) = plan_en_memoria(
        "tramo-sin-cota",
        plan_con(
            serde_json::json!([
                { "tipo": "nodo", "codigo": "P1", "familia": "saneamiento.pozo", "e": 760000.0, "n": 9796000.0, "cota_terreno": 2500.0 },
                { "tipo": "nodo", "codigo": "P2", "familia": "saneamiento.pozo", "e": 760050.0, "n": 9796000.0, "cota_terreno": 2499.0 },
                { "tipo": "arco", "codigo": "C1", "familia": "saneamiento.tuberia", "desde": "P1", "hasta": "P2", "diametro_mm": 200 }
            ]),
            serde_json::json!([]),
        ),
    );
    let c1 = tramo(&p, "C1");
    assert_eq!(c1.cotas_trazado(), None);
    assert!(c1.dibujo_sin_dato());
    let filas = redes_core::cantidades::longitudes(&p);
    assert!(filas.iter().all(|f| f.cantidad_3d.is_none()), "{filas:?}");
}

// ---------------------------------------------------------------------------------------------
// Sistema de coordenadas (F12-13): el plan declara su CRS, se compara con el proyecto y nunca se
// reproyecta en silencio.

fn plan_crs(crs: Option<&str>, e: f64, n: f64) -> serde_json::Value {
    let mut proyecto = serde_json::json!({ "nombre": "crs" });
    if let Some(c) = crs {
        proyecto["crs"] = serde_json::json!(c);
    }
    serde_json::json!({
        "schema": "redes.plan/1",
        "proyecto": proyecto,
        "terreno": [[e, n, 2500.0], [e + 100.0, n, 2500.0], [e, n + 100.0, 2500.0]],
        "objetos": [
            { "tipo": "nodo", "codigo": "P1", "familia": "saneamiento.pozo", "e": e + 1.0, "n": n + 1.0, "cota": 2498.0 }
        ]
    })
}

/// Importa el plan en `p`; si falla, comprueba que no tocó nada.
fn importa_en(p: &mut Project, nombre: &str, plan: serde_json::Value) -> Result<(), String> {
    let ruta = std::env::temp_dir().join(format!("redes-crs-{nombre}-{}.json", std::process::id()));
    std::fs::write(&ruta, plan.to_string()).unwrap();
    let (antes, revision, crs) = (p.count(), p.revision, p.crs.clone());
    let r = redes_core::ingest::import_plan(p, &ruta).map(|_| ());
    let _ = std::fs::remove_file(&ruta);
    if r.is_err() {
        assert_eq!(
            (p.count(), p.revision, p.crs.clone()),
            (antes, revision, crs),
            "{nombre}: un error no toca el proyecto"
        );
    }
    r
}

fn proyecto_17s_con_objetos() -> Project {
    let mut p = Project::new("17s", "EPSG:32717");
    importa_en(
        &mut p,
        "base17",
        plan_crs(Some("EPSG:32717"), 760_000.0, 9_796_000.0),
    )
    .expect("base");
    p
}

#[test]
fn un_plan_en_18s_entra_en_un_proyecto_vacio_y_lo_fija() {
    let mut p = Project::new("vacio", "EPSG:32717");
    importa_en(
        &mut p,
        "18s",
        plan_crs(Some("EPSG:32718"), 280_000.0, 8_660_000.0),
    )
    .expect("18S");
    assert_eq!(p.crs, "EPSG:32718");
    assert_eq!(p.terrain.as_ref().unwrap().epsg, 32718, "antes: 32717 fijo");
}

#[test]
fn un_plan_en_otro_crs_que_el_proyecto_da_e_crs() {
    let mut p = proyecto_17s_con_objetos();
    let err = importa_en(
        &mut p,
        "mezcla",
        plan_crs(Some("EPSG:32718"), 280_000.0, 8_660_000.0),
    )
    .unwrap_err();
    assert!(err.starts_with("E_CRS:"), "{err}");
    assert!(err.contains("no reproyecta"), "{err}");
}

#[test]
fn un_plan_sin_crs_o_con_crs_no_utm_sur_da_e_crs() {
    for (nombre, crs) in [
        ("sin", None),
        ("vacio", Some("")),
        ("norte", Some("EPSG:32617")),
        ("wgs84", Some("EPSG:4326")),
        ("basura", Some("UTM17")),
    ] {
        let mut p = Project::new(nombre, "EPSG:32717");
        let err =
            importa_en(&mut p, nombre, plan_crs(crs, 760_000.0, 9_796_000.0)).expect_err(nombre);
        assert!(err.starts_with("E_CRS:"), "{nombre}: {err}");
    }
}

#[test]
fn un_plan_en_grados_da_e_crs_legible() {
    let mut p = Project::new("grados", "EPSG:32717");
    let err = importa_en(&mut p, "grados", plan_crs(Some("EPSG:32717"), -78.5, -0.2)).unwrap_err();
    assert!(err.starts_with("E_CRS:"), "{err}");
    assert!(err.contains("lon/lat"), "dice por qué: {err}");
}

fn terreno_csv(nombre: &str, e: f64, n: f64) -> std::path::PathBuf {
    let ruta = std::env::temp_dir().join(format!("redes-crs-{nombre}-{}.csv", std::process::id()));
    std::fs::write(
        &ruta,
        format!(
            "E,N,Z\n{e},{n},2500\n{},{n},2501\n{e},{},2502\n",
            e + 0.001,
            n + 0.001
        ),
    )
    .unwrap();
    ruta
}

#[test]
fn un_terreno_en_otra_zona_que_el_proyecto_da_e_crs() {
    let mut p = proyecto_17s_con_objetos();
    let revision = p.revision;
    let ruta = terreno_csv("zona", 760_000.0, 9_796_000.0);
    let err = redes_core::ingest::import_terreno(&mut p, &ruta, 32718).unwrap_err();
    let _ = std::fs::remove_file(&ruta);
    assert!(err.starts_with("E_CRS:"), "{err}");
    assert_eq!(p.revision, revision);
    assert_eq!(
        p.terrain.as_ref().unwrap().epsg,
        32717,
        "el terreno no cambia"
    );
}

#[test]
fn un_terreno_en_grados_da_e_crs() {
    let mut p = Project::new("grados", "EPSG:32717");
    let ruta = terreno_csv("grados", -78.5, -0.2);
    let err = redes_core::ingest::import_terreno(&mut p, &ruta, 32717).unwrap_err();
    let _ = std::fs::remove_file(&ruta);
    assert!(
        err.starts_with("E_CRS:") && err.contains("lon/lat"),
        "{err}"
    );
    assert!(p.terrain.is_none());
}

// ---------------------------------------------------------------------------------------------
// Contrato del plan sólido (F12-15).

/// Importa un plan que debe fallar y comprueba que el proyecto queda intacto.
fn plan_rechazado(nombre: &str, plan: serde_json::Value) -> String {
    let ruta = std::env::temp_dir().join(format!("redes-{nombre}-{}.json", std::process::id()));
    std::fs::write(&ruta, plan.to_string()).unwrap();
    let mut p = Project::new(nombre, "EPSG:32717");
    let err = redes_core::ingest::import_plan(&mut p, &ruta).expect_err("debe rechazarse");
    let _ = std::fs::remove_file(&ruta);
    assert_eq!(p.count(), 0, "{nombre}: no entra nada");
    assert!(p.fuentes.is_empty());
    err
}

fn dos_pozos() -> Vec<serde_json::Value> {
    vec![
        serde_json::json!({ "tipo": "nodo", "codigo": "P1", "familia": "saneamiento.pozo", "e": 760000.0, "n": 9796000.0, "cota": 2500.0, "cota_terreno": 2502.0 }),
        serde_json::json!({ "tipo": "nodo", "codigo": "P2", "familia": "saneamiento.pozo", "e": 760100.0, "n": 9796000.0, "cota": 2499.0, "cota_terreno": 2501.0 }),
    ]
}

fn con_arco(arco: serde_json::Value) -> serde_json::Value {
    let mut o = dos_pozos();
    o.push(arco);
    plan_con(serde_json::Value::Array(o), serde_json::json!([]))
}

#[test]
fn un_campo_desconocido_se_rechaza_con_su_nombre_y_el_valido() {
    let err = plan_rechazado(
        "campo-desconocido",
        con_arco(serde_json::json!({
            "tipo": "arco", "codigo": "C1", "familia": "saneamiento.tuberia",
            "desde": "P1", "hasta": "P2", "diametro_mm": 300,
            "trazado": [[760000.0, 9796000.0, 2500.0], [760100.0, 9796000.0, 2499.0]],
            "cota_solera_inicial": 2500.0
        })),
    );
    assert!(err.starts_with("E_FMT_CAMPO:"), "{err}");
    assert!(err.contains("arco C1"), "dice el objeto: {err}");
    assert!(
        err.contains("«trazado» (¿quiso decir «vertices»?)"),
        "{err}"
    );
    assert!(
        err.contains("«cota_solera_inicial» (¿quiso decir «cota_solera_inicio»?)"),
        "{err}"
    );
    // También en `proyecto` y dentro de un dato.
    let mut plan = plan_con(serde_json::Value::Array(dos_pozos()), serde_json::json!([]));
    plan["proyecto"]["unidades"] = serde_json::json!("si");
    plan["objetos"][0]["datos"] =
        serde_json::json!({ "caudal": { "valor": 1.0, "unidades": "L/s" } });
    let err = plan_rechazado("campo-proyecto", plan);
    assert!(err.contains("proyecto: «unidades»"), "{err}");
    assert!(
        err.contains("datos.caudal: «unidades» (¿quiso decir «unidad»?)"),
        "{err}"
    );
}

#[test]
fn un_diametro_en_otra_unidad_se_avisa_y_no_se_corrige() {
    let mut o = dos_pozos();
    o[0]["diametro"] = serde_json::json!(1200.0);
    o.push(serde_json::json!({
        "tipo": "arco", "codigo": "C1", "familia": "saneamiento.tuberia",
        "desde": "P1", "hasta": "P2", "diametro_mm": 6, "diametro_interior_mm": 0.3
    }));
    let (p, rep) = plan_en_memoria(
        "unidades",
        plan_con(serde_json::Value::Array(o), serde_json::json!([])),
    );
    let aviso = |que: &str| rep.avisos.iter().any(|a| a.contains(que));
    assert!(aviso("nodo P1: diametro = 1200 m"), "{:?}", rep.avisos);
    assert!(aviso("arco C1: diametro_mm = 6 mm"), "{:?}", rep.avisos);
    assert!(
        aviso("arco C1: diametro_interior_mm = 0.3 mm"),
        "{:?}",
        rep.avisos
    );
    assert_eq!(
        nodo(&p, "P1").num("diametro_interior"),
        Some(1200.0),
        "tal cual"
    );
    assert_eq!(tramo(&p, "C1").dn(), Some(6), "tal cual");
}

#[test]
fn el_di_y_la_seccion_entran_y_sin_dn_no_se_inventa_uno() {
    let mut o = dos_pozos();
    o.push(serde_json::json!({
        "tipo": "arco", "codigo": "C1", "familia": "saneamiento.tuberia",
        "desde": "P1", "hasta": "P2", "diametro_mm": 300, "diametro_interior_mm": 285.0
    }));
    o.push(serde_json::json!({
        "tipo": "arco", "codigo": "C2", "familia": "saneamiento.tuberia",
        "desde": "P1", "hasta": "P2", "seccion": "ovoide", "alto_m": 1.2, "ancho_m": 0.8
    }));
    o.push(serde_json::json!({
        "tipo": "arco", "codigo": "C3", "familia": "saneamiento.tuberia",
        "desde": "P1", "hasta": "P2", "seccion": "cuadrada"
    }));
    o.push(serde_json::json!({
        "tipo": "arco", "codigo": "C4", "familia": "agua.tuberia",
        "desde": "P1", "hasta": "P2", "seccion": "circular"
    }));
    let (p, rep) = plan_en_memoria(
        "seccion",
        plan_con(serde_json::Value::Array(o), serde_json::json!([])),
    );
    let c1 = tramo(&p, "C1");
    assert_eq!(c1.dn(), Some(300));
    assert_eq!(c1.num("diametro_interior"), Some(285.0));
    let c2 = tramo(&p, "C2");
    assert_eq!(c2.dn(), None, "un ovoide sin DN no recibe uno");
    assert_eq!(
        c2.attrs.get("seccion").and_then(|v| v.as_str()),
        Some("ovoide")
    );
    assert_eq!(c2.datos["altura_seccion"].valor, 1.2);
    assert_eq!(c2.datos["altura_seccion"].unidad, "m");
    assert_eq!(c2.datos["ancho_seccion"].valor, 0.8);
    let motivo = |c: &str| {
        rep.descartados
            .iter()
            .find(|d| d.starts_with(&format!("arco {c}:")))
            .cloned()
            .unwrap_or_default()
    };
    assert!(
        motivo("C3").contains("ovoide"),
        "lista los válidos: {:?}",
        rep.descartados
    );
    assert!(motivo("C4").contains("seccion"), "{:?}", rep.descartados);
}

#[test]
fn los_vertices_sin_z_se_interpolan_entre_soleras_y_se_marcan() {
    let mut o = dos_pozos();
    o.push(serde_json::json!({
        "tipo": "arco", "codigo": "C1", "familia": "saneamiento.tuberia",
        "desde": "P1", "hasta": "P2", "diametro_mm": 300,
        "cota_solera_inicio": 2500.0, "cota_solera_fin": 2499.0,
        "vertices": [[760000.0, 9796000.0], [760075.0, 9796000.0], [760100.0, 9796000.0]]
    }));
    o.push(serde_json::json!({
        "tipo": "arco", "codigo": "C2", "familia": "saneamiento.tuberia",
        "desde": "P1", "hasta": "P2", "diametro_mm": 300,
        "vertices": [[760000.0, 9796000.0], [760050.0, 9796010.0], [760100.0, 9796000.0]]
    }));
    o[0].as_object_mut().unwrap().remove("cota");
    o.push(serde_json::json!({
        "tipo": "arco", "codigo": "C3", "familia": "saneamiento.tuberia",
        "desde": "P1", "hasta": "P2",
        "vertices": [[760000.0, 9796000.0, 1.0], [760100.0, 9796000.0]]
    }));
    let (p, rep) = plan_en_memoria(
        "sin-z",
        plan_con(serde_json::Value::Array(o), serde_json::json!([])),
    );
    let c1 = tramo(&p, "C1");
    assert!(
        (c1.path[1].z - 2499.25).abs() < 1e-9,
        "por longitud: {:?}",
        c1.path
    );
    assert_eq!(
        c1.cotas_trazado(),
        Some((2500.0, 2499.0)),
        "los extremos son dato"
    );
    assert!(c1.falta(redes_core::model::COTA_VERTICES));
    assert!(!c1.dibujo_sin_dato());
    assert!(
        rep.avisos
            .iter()
            .any(|a| a.contains("C1") && a.contains("interpolada"))
    );
    // Sin solera en un extremo (P1 no la trae): cotas ausentes, línea gris.
    let c2 = tramo(&p, "C2");
    assert_eq!(c2.path.len(), 3);
    assert_eq!(c2.cotas_trazado(), None);
    assert!(c2.dibujo_sin_dato());
    // Mezclar [E,N,Z] y [E,N] es un error del objeto.
    assert!(
        rep.descartados.iter().any(|d| d.starts_with("arco C3:")),
        "{:?}",
        rep.descartados
    );
}

#[test]
fn un_dispositivo_de_enlace_une_sus_nodos_sin_tramo() {
    let o = serde_json::json!([
        { "tipo": "nodo", "codigo": "N1", "familia": "comun.accesorio", "subtipo": "union", "e": 760000.0, "n": 9796000.0, "cota_eje": 2500.0 },
        { "tipo": "nodo", "codigo": "N2", "familia": "comun.accesorio", "subtipo": "union", "e": 760005.0, "n": 9796000.0, "cota_eje": 2499.0 },
        { "tipo": "nodo", "codigo": "V1", "familia": "agua.valvula", "subtipo": "reduccion_presion", "e": 760000.0, "n": 9796000.0, "cota_eje": 2500.0, "desde": "N1", "hasta": "N2" },
        { "tipo": "nodo", "codigo": "V2", "familia": "agua.valvula", "subtipo": "reduccion_presion", "e": 760000.0, "n": 9796000.0, "cota_eje": 2500.0, "desde": "NADA", "hasta": "V2" }
    ]);
    let (p, rep) = plan_en_memoria("enlace", plan_con(o, serde_json::json!([])));
    assert_eq!(p.links.len(), 0, "no se inventan tramos");
    assert_eq!(p.enlaces.len(), 1, "V2 no une nada");
    let topo = redes_core::topology::Topology::build(&p);
    let (n1, n2) = (nodo(&p, "N1").id, nodo(&p, "N2").id);
    assert!(
        topo.components
            .iter()
            .any(|c| c.contains(&n1) && c.contains(&n2)),
        "{:?}",
        topo.components
    );
    assert!(
        rep.avisos
            .iter()
            .any(|a| a.contains("V2") && a.contains("NADA"))
    );
    assert!(
        rep.avisos
            .iter()
            .any(|a| a.contains("V2") && a.contains("propio nodo"))
    );
}

/// Un tramo sin soleras sale por el DXF y vuelve sin soleras: las Z de dibujo del trazado no se
/// convierten en dato a la vuelta. Uno con soleras vuelve con ellas.
#[test]
fn un_tramo_sin_soleras_vuelve_del_dxf_sin_soleras() {
    let (p, _) = plan_en_memoria(
        "vuelta-soleras",
        plan_con(
            serde_json::json!([
                { "tipo": "nodo", "codigo": "P1", "familia": "saneamiento.pozo", "e": 760000.0, "n": 9796000.0, "cota_terreno": 2500.0 },
                { "tipo": "nodo", "codigo": "P2", "familia": "saneamiento.pozo", "e": 760050.0, "n": 9796000.0, "cota_terreno": 2499.0 },
                { "tipo": "nodo", "codigo": "P3", "familia": "saneamiento.pozo", "e": 760000.0, "n": 9796050.0, "cota": 2497.0, "cota_terreno": 2500.0 },
                { "tipo": "nodo", "codigo": "P4", "familia": "saneamiento.pozo", "e": 760050.0, "n": 9796050.0, "cota": 2496.5, "cota_terreno": 2499.0 },
                { "tipo": "arco", "codigo": "C1", "familia": "saneamiento.tuberia", "desde": "P1", "hasta": "P2", "diametro_mm": 200 },
                { "tipo": "arco", "codigo": "C2", "familia": "saneamiento.tuberia", "desde": "P3", "hasta": "P4", "diametro_mm": 200 }
            ]),
            serde_json::json!([]),
        ),
    );
    assert_eq!(tramo(&p, "C1").cotas_trazado(), None);
    let ruta = std::env::temp_dir().join(format!("redes-vuelta-sol-{}.dxf", std::process::id()));
    redes_core::export::export_dxf(&p, &ruta).unwrap();
    let mut d = Project::new("vuelta", "EPSG:32717");
    redes_core::ingest::import_dxf(&mut d, &ruta, 0.5).unwrap();
    let _ = std::fs::remove_file(&ruta);
    let c1 = tramo(&d, "C1");
    assert_eq!(c1.cotas_trazado(), None, "antes volvía con las Z de dibujo");
    assert!(c1.dibujo_sin_dato());
    assert_eq!(tramo(&d, "C2").cotas_trazado(), Some((2497.0, 2496.5)));
}

#[test]
fn la_cota_del_eje_ubica_el_nudo_sin_ser_solera() {
    let o = serde_json::json!([
        { "tipo": "nodo", "codigo": "J1", "familia": "comun.accesorio", "subtipo": "tee", "e": 760000.0, "n": 9796000.0, "cota_eje": 2512.5 }
    ]);
    let mut plan = plan_con(o, serde_json::json!([]));
    plan["proyecto"]["fuente_app"] = serde_json::json!("EPANET 2.2");
    plan["proyecto"]["fuente_archivo"] = serde_json::json!("red.inp");
    let (p, rep) = plan_en_memoria("eje", plan);
    let j1 = nodo(&p, "J1");
    assert_eq!(j1.cota_solera(), None);
    assert_eq!(j1.cota_terreno(), None);
    assert!(j1.sin_altura());
    assert_eq!(j1.z_invert, 2512.5, "solo de dibujo");
    let eje = &j1.datos["cota_eje"];
    assert_eq!((eje.valor, eje.unidad.as_str()), (2512.5, "m"));
    assert_eq!(eje.fuente, "EPANET 2.2 (red.inp)");
    assert!(rep.descartados.is_empty());
}
