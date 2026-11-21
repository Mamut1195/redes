//! Ingesta con archivos de verdad: los cuatro formatos que entran al producto.
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

#[test]
fn un_dxf_de_otra_aplicacion_entra_por_sus_capas() {
    let mut p = Project::new("dxf", "EPSG:32717");
    let rep = redes_core::ingest::import_dxf(&mut p, &fixture("red_ajena.dxf"), 0.5).expect("dxf");
    assert_eq!(rep.nodos, 2, "dos POINT");
    assert_eq!(rep.arcos, 1, "una LINE");
    assert!(p.nodes.iter().all(|n| n.family == "saneamiento.pozo"));
    assert!(p.links.iter().all(|l| l.family == "saneamiento.tuberia"));
    // El archivo no trae la capa de datos: los códigos son correlativos del registro.
    assert!(p.nodes.iter().all(|n| !n.code.is_empty()));
}

#[test]
fn un_geojson_municipal_conserva_sus_atributos() {
    let mut p = Project::new("geojson", "EPSG:32717");
    let rep =
        redes_core::ingest::import_geojson(&mut p, &fixture("municipal.geojson")).expect("geojson");
    assert!(rep.nodos >= 1 && rep.arcos >= 1, "{rep:?}");
    let pozo = p
        .nodes
        .iter()
        .find(|n| n.code == "PZ-900")
        .expect("el punto conserva su código");
    assert_eq!(pozo.family, "saneamiento.pozo");
    assert!(
        pozo.num("diametro_interior").is_some(),
        "atributo de familia"
    );
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
