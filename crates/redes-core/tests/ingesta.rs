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
    assert!(p.links.iter().any(|l| l.code == "CL-901"));
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
// SWMM y EPANET (F12-04, F12-05): los planes los genera `skills/redes-ingesta/scripts/inp_to_plan.py`
// a partir de los `.inp` de esta carpeta; `test_inp_to_plan.py` comprueba que siguen al día.

fn importa_plan(nombre: &str) -> (Project, redes_core::ingest::ImportReport) {
    let mut p = Project::new(nombre, "EPSG:32717");
    let rep = redes_core::ingest::import_plan(&mut p, &fixture(nombre)).expect("plan");
    assert!(
        rep.descartados.is_empty(),
        "{nombre}: {:?}",
        rep.descartados
    );
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
    assert!(
        tramo(&p, "G-5").datos.contains_key("altura_seccion"),
        "el ovoide conserva su altura"
    );
}

#[test]
fn un_epanet_en_lps_entra_sin_inventar_tanques() {
    let (p, rep) = importa_plan("epanet_si_lps.plan.json");
    assert_eq!((rep.nodos, rep.arcos), (11, 6));
    let tq = nodo(&p, "TQ1");
    assert_eq!(tq.num("cota_nivel_max"), Some(2533.2));
    assert_eq!(tq.num("volumen_util"), None, "EPANET no da volumen útil");
    assert_eq!(nodo(&p, "N2").family, "comun.accesorio");
    assert_eq!(nodo(&p, "PRV1").family, "agua.valvula");
    assert_eq!(nodo(&p, "PRV1").ty, "reduccion_presion");
    assert_eq!(nodo(&p, "B1").family, "agua.bombeo");
    assert_eq!(nodo(&p, "CAPT").family, "agua.captacion");
    assert_eq!(tramo(&p, "T1").path.len(), 4);
}

#[test]
fn un_epanet_en_gpm_entra_en_milimetros_y_metros() {
    let (p, rep) = importa_plan("epanet_us_gpm.plan.json");
    assert_eq!((rep.nodos, rep.arcos), (9, 5));
    assert_eq!(tramo(&p, "102").d_mm, 203, "8 in");
    assert!((nodo(&p, "J20").z_invert - 690.0 * 0.3048).abs() < 1e-6);
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
