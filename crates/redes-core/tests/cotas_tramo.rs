//! Cota explícita del tramo (F12-02): la solera la declara el tramo, no el pozo, y el tubo se
//! dibuja con el fondo interior en esa solera.

use redes_core::model::Project;

/// Importa un plan con dos pozos (solera 99,00) y un tramo que declara solera 100,00 en ambos
/// extremos con DI 300 mm (DN 315) y pared de 7,7 mm.
fn proyecto() -> Project {
    let plan = r#"{
        "schema": "redes.plan/1",
        "proyecto": { "nombre": "cotas", "crs": "EPSG:32717" },
        "objetos": [
            { "tipo": "nodo", "codigo": "P1", "familia": "saneamiento.pozo",
              "e": 1000.0, "n": 2000.0, "cota": 99.0, "cota_terreno": 102.0 },
            { "tipo": "nodo", "codigo": "P2", "familia": "saneamiento.pozo",
              "e": 1040.0, "n": 2000.0, "cota": 99.0, "cota_terreno": 102.0 },
            { "tipo": "arco", "codigo": "T1", "familia": "saneamiento.tuberia",
              "desde": "P1", "hasta": "P2", "diametro_mm": 315,
              "cota_solera_inicio": 100.0, "cota_solera_fin": 100.0,
              "attrs": { "diametro_interior": 300.0, "espesor_pared": 0.0077 } }
        ]
    }"#;
    let ruta = std::env::temp_dir().join(format!("redes-cotas-{}.json", std::process::id()));
    std::fs::write(&ruta, plan).unwrap();
    let mut p = Project::new("cotas", "EPSG:32717");
    let rep = redes_core::ingest::import_plan(&mut p, &ruta).expect("importa");
    let _ = std::fs::remove_file(&ruta);
    assert_eq!(rep.arcos, 1, "{rep:?}");
    p.origin = redes_core::geom::P2::new(1000.0, 2000.0);
    p
}

#[test]
fn el_tramo_guarda_la_solera_declarada_y_no_la_del_pozo() {
    let p = proyecto();
    let l = &p.links[0];
    assert_eq!(l.cota_solera_inicio, Some(100.0));
    assert_eq!(l.cota_solera_fin, Some(100.0));
    assert!(
        (l.path[0].z - 100.0).abs() < 1e-9,
        "trazado en la solera del tramo"
    );
    assert!((l.path[1].z - 100.0).abs() < 1e-9);
}

#[test]
fn un_tramo_con_solera_100_y_di_030_dibuja_el_eje_en_100_15() {
    let p = proyecto();
    let malla = redes_core::gltf::triangulos_exportables(&p, false);
    let tubo = malla
        .familias
        .iter()
        .position(|f| f == "saneamiento.tuberia")
        .expect("hay tubo") as u16;
    let (mut lo, mut hi) = (f32::MAX, f32::MIN);
    for t in malla.tris.iter().filter(|t| t.familia == tubo) {
        for v in t.pos {
            lo = lo.min(v[2]);
            hi = hi.max(v[2]);
        }
    }
    // Eje en solera + DI/2 = 100,15 (antes se centraba en la solera: 100,00).
    assert!(
        ((lo + hi) * 0.5 - 100.15).abs() < 2e-3,
        "eje {}",
        (lo + hi) * 0.5
    );
    // Radio exterior = DI/2 + pared (antes, DN/2). La sección es un polígono inscrito: su
    // semialtura está entre R·cos(30°) (6 caras) y R.
    let r_ext = 0.15 + 0.0077f32;
    let semialto = (hi - lo) * 0.5;
    assert!(
        semialto <= r_ext + 1e-3 && semialto >= r_ext * 0.866 - 1e-3,
        "semialtura {semialto} vs radio exterior {r_ext}"
    );
}

#[test]
fn el_geojson_exporta_las_soleras_del_tramo() {
    let p = proyecto();
    let ruta = std::env::temp_dir().join(format!("redes-cotas-{}.geojson", std::process::id()));
    redes_core::export::export_geojson(&p, &ruta).unwrap();
    let fc: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&ruta).unwrap()).unwrap();
    let _ = std::fs::remove_file(&ruta);
    let tramo = fc["features"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["properties"]["codigo"] == "T1")
        .unwrap();
    assert_eq!(
        tramo["properties"]["cota_solera_inicio"].as_f64(),
        Some(100.0)
    );
    assert_eq!(tramo["properties"]["cota_solera_fin"].as_f64(), Some(100.0));
}
