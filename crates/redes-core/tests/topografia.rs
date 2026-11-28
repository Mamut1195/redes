//! `redes.topografia/1` (F17-11, F17-12, D18): the survey an agent translates with the skill
//! enters strictly, is triangulated with its breaklines, boundary and holes, and keeps each
//! point's number, description and class.

use std::path::PathBuf;

use redes_core::geom::P2;
use redes_core::model::Project;
use redes_core::survey::import_survey;
use redes_core::terrain::{PointClass, VerticalDatum};
use serde_json::{Value, json};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

fn write_temp(name: &str, content: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("redes-topo-{name}-{}.json", std::process::id()));
    std::fs::write(&path, content).unwrap();
    path
}

fn import_json(p: &mut Project, name: &str, doc: &Value) -> Result<(), String> {
    let path = write_temp(name, &doc.to_string());
    let (revision, terrain) = (p.revision, p.terrain.as_ref().map(|t| t.points.len()));
    let r = import_survey(p, &path).map(|_| ());
    let _ = std::fs::remove_file(&path);
    if r.is_err() {
        assert_eq!(
            (p.revision, p.terrain.as_ref().map(|t| t.points.len())),
            (revision, terrain),
            "{name}: an error leaves the project untouched"
        );
    }
    r
}

fn rejected(name: &str, doc: &Value) -> String {
    let mut p = Project::new(name, "EPSG:32717");
    import_json(&mut p, name, doc).expect_err("must be rejected")
}

/// A 3×3 survey square at 17S plus one point per `extra`.
fn square(extra: Vec<Value>) -> Value {
    let mut puntos = Vec::new();
    for j in 0..3 {
        for i in 0..3 {
            let n = (j * 3 + i + 1).to_string();
            puntos.push(json!({
                "numero": n, "e": 760_000.0 + i as f64 * 10.0, "n": 9_796_000.0 + j as f64 * 10.0,
                "z": 2500.0 + i as f64, "descripcion": "TN", "clase": "terreno"
            }));
        }
    }
    puntos.extend(extra);
    json!({
        "schema": "redes.topografia/1",
        "nombre": "cuadro",
        "crs": "EPSG:32717",
        "datum_vertical": "no_declarado",
        "puntos": puntos
    })
}

fn point(numero: &str, e: f64, n: f64, z: f64, desc: &str, clase: &str) -> Value {
    json!({ "numero": numero, "e": e, "n": n, "z": z, "descripcion": desc, "clase": clase })
}

fn imported_ravine() -> Project {
    let mut p = Project::new("quebrada", "EPSG:32717");
    import_survey(&mut p, &fixture("topografia_quebrada.topografia.json")).expect("fixture");
    p
}

// ---------------------------------------------------------------------------------------------
// The fixture translated per the skill (references/topografia.md §7).

#[test]
fn the_translated_pnezd_survey_validates_and_keeps_every_point() {
    let mut p = Project::new("quebrada", "EPSG:32717");
    let rep = import_survey(&mut p, &fixture("topografia_quebrada.topografia.json")).unwrap();
    assert_eq!(rep.puntos, 137);
    assert_eq!(rep.quiebres, 5);
    assert!(rep.contorno);
    assert_eq!(rep.huecos, 1);
    assert_eq!(
        rep.por_clase["excluido"], 8,
        "4 building corners, tree, pole, BM, manhole"
    );
    let t = p.terrain.as_ref().unwrap();
    let survey = t.survey.as_ref().unwrap();
    let pz = survey
        .points
        .iter()
        .find(|q| q.description == "PZ EXIST")
        .unwrap();
    assert_eq!(pz.class, PointClass::Excluded);
    assert_eq!(pz.number, "137");
    assert_eq!(
        t.points.len(),
        137 - 8,
        "excluded points do not enter the TIN"
    );
    assert!(p.journal.iter().any(|j| j.command == "importar topografia"));
}

#[test]
fn the_ravine_keeps_its_thalweg_height() {
    let p = imported_ravine();
    let t = p.terrain.as_ref().unwrap();
    // Halfway between two thalweg shots (n = 2 and 26) the ground is the thalweg's, 3 m below
    // the banks; a TIN bridging the ravine would give the banks' height here.
    let (e, n) = (760_050.75, 9_796_014.0);
    let z = t.z_at(P2::new(e, n)).unwrap();
    let base = 2500.0 + 0.02 * 14.0;
    assert!((z - (base - 3.0)).abs() < 0.01, "z = {z}");
}

#[test]
fn no_height_outside_the_boundary_notch_or_in_the_building() {
    let p = imported_ravine();
    let t = p.terrain.as_ref().unwrap();
    let at = |e: f64, n: f64| t.z_at(P2::new(760_000.0 + e, 9_796_000.0 + n));
    assert!(at(110.0, 50.0).is_none(), "the notch of the boundary");
    assert!(at(22.0, 77.0).is_none(), "inside the building");
    assert!(at(-1.0, 50.0).is_none(), "beyond the boundary");
    assert!(at(65.0, 20.0).is_some());
}

#[test]
fn the_terrain_survives_save_and_load() {
    let p = imported_ravine();
    let path = std::env::temp_dir().join(format!("redes-topo-io-{}.redes", std::process::id()));
    redes_core::project_io::save(&p, &path).unwrap();
    let q = redes_core::project_io::load(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    let (a, b) = (p.terrain.as_ref().unwrap(), q.terrain.as_ref().unwrap());
    assert_eq!(a.tris, b.tris);
    assert_eq!(a.survey, b.survey);
    let probe = P2::new(760_050.75, 9_796_014.0);
    assert_eq!(a.z_at(probe), b.z_at(probe));
}

/// The old `importar terreno` read the three first columns as E, N, Z: a PNEZD row entered with
/// the point number as E and the easting as Z. A text file is not the contract any more.
#[test]
fn a_raw_pnezd_file_cannot_be_misread_as_terrain() {
    let mut p = Project::new("pnezd", "EPSG:32717");
    let err = import_survey(&mut p, &fixture("topografia_quebrada.pnezd.txt")).unwrap_err();
    assert!(err.starts_with("E_FMT_PARSE:"), "{err}");
    assert!(p.terrain.is_none());
}

#[test]
fn a_small_civil3d_landxml_translation_validates() {
    let mut p = Project::new("landxml", "EPSG:32717");
    let rep = import_survey(&mut p, &fixture("topografia_civil3d.topografia.json")).unwrap();
    assert_eq!(rep.datum_vertical, "egm2008");
    assert_eq!(rep.quiebres, 1);
    assert!(rep.contorno);
}

// ---------------------------------------------------------------------------------------------
// Strict contract: nothing is guessed.

#[test]
fn another_schema_version_or_a_missing_one_is_refused() {
    for (schema, code) in [
        (json!("redes.topografia/2"), "E_FMT_VERSION:"),
        (json!("redes.plan/1"), "E_FMT_UNSUPPORTED:"),
        (Value::Null, "E_FMT_UNSUPPORTED:"),
    ] {
        let mut doc = square(vec![]);
        if schema.is_null() {
            doc.as_object_mut().unwrap().remove("schema");
        } else {
            doc["schema"] = schema;
        }
        let err = rejected("schema", &doc);
        assert!(err.starts_with(code), "{err}");
    }
}

#[test]
fn an_unknown_field_or_class_is_refused() {
    let mut doc = square(vec![]);
    doc["puntos"][0]["codigo"] = json!("TN");
    assert!(rejected("campo", &doc).starts_with("E_FMT_CAMPO:"));
    let mut doc = square(vec![]);
    doc["puntos"][0]["clase"] = json!("arbol");
    assert!(rejected("clase", &doc).starts_with("E_FMT_PARSE:"));
    let mut doc = square(vec![]);
    doc.as_object_mut().unwrap().remove("datum_vertical");
    let err = rejected("sin-datum", &doc);
    assert!(
        err.contains("datum_vertical"),
        "the datum is never assumed: {err}"
    );
}

#[test]
fn a_non_finite_number_cannot_enter() {
    let text = square(vec![]).to_string().replacen("2500.0", "1e999", 1);
    let path = write_temp("infinito", &text);
    let mut p = Project::new("inf", "EPSG:32717");
    let err = import_survey(&mut p, &path).unwrap_err();
    let _ = std::fs::remove_file(&path);
    assert!(err.starts_with("E_FMT_PARSE:"), "{err}");
}

#[test]
fn a_repeated_point_number_is_an_error() {
    let doc = square(vec![point(
        "5",
        760_100.0,
        9_796_100.0,
        2501.0,
        "TN",
        "terreno",
    )]);
    let err = rejected("repetido", &doc);
    assert!(
        err.starts_with("E_TOPO_PUNTO:") && err.contains('5'),
        "{err}"
    );
}

#[test]
fn same_place_with_another_height_is_an_error_and_same_height_a_warning() {
    let clash = square(vec![point(
        "X",
        760_010.0,
        9_796_010.0,
        2490.0,
        "TN",
        "terreno",
    )]);
    let err = rejected("coincidentes", &clash);
    assert!(
        err.starts_with("E_TOPO_COINCIDENTES:") && err.contains("X"),
        "{err}"
    );

    let twin = square(vec![point(
        "X",
        760_010.0,
        9_796_010.0,
        2501.0,
        "TN",
        "terreno",
    )]);
    let path = write_temp("gemelo", &twin.to_string());
    let mut p = Project::new("gemelo", "EPSG:32717");
    let rep = import_survey(&mut p, &path).unwrap();
    let _ = std::fs::remove_file(&path);
    assert!(
        rep.avisos.iter().any(|a| a.contains("X = 5")),
        "{:?}",
        rep.avisos
    );
    assert_eq!(rep.vertices_tin, 9);

    // An excluded point (a pole) may stand where the ground was shot.
    let pole = square(vec![point(
        "P",
        760_010.0,
        9_796_010.0,
        2507.0,
        "POSTE",
        "excluido",
    )]);
    let mut p = Project::new("poste", "EPSG:32717");
    import_json(&mut p, "poste", &pole).expect("excluded points do not clash");
}

#[test]
fn a_breakline_must_cite_existing_breakline_points() {
    let mut doc = square(vec![]);
    doc["puntos"][0]["clase"] = json!("quiebre");
    doc["quiebres"] = json!([{ "descripcion": "BV", "puntos": ["1", "99"] }]);
    let err = rejected("desconocido", &doc);
    assert!(
        err.starts_with("E_TOPO_REFERENCIA:") && err.contains("99"),
        "{err}"
    );

    let mut doc = square(vec![point(
        "T",
        760_005.0,
        9_796_005.0,
        2509.0,
        "ARB",
        "excluido",
    )]);
    doc["puntos"][0]["clase"] = json!("quiebre");
    doc["quiebres"] = json!([{ "descripcion": "BV", "puntos": ["1", "T"] }]);
    let err = rejected("excluido", &doc);
    assert!(
        err.starts_with("E_TOPO_REFERENCIA:") && err.contains("excluido"),
        "{err}"
    );
}

#[test]
fn crossing_breaklines_are_an_error() {
    let mut doc = square(vec![]);
    for k in [0, 1, 3, 5] {
        doc["puntos"][k]["clase"] = json!("quiebre");
    }
    // 1–6 and 2–4 cut each other away from any point: two heights for one place.
    doc["quiebres"] = json!([
        { "descripcion": "BV1", "puntos": ["1", "6"] },
        { "descripcion": "BV2", "puntos": ["2", "4"] }
    ]);
    let err = rejected("cruzados", &doc);
    assert!(
        err.starts_with("E_TOPO_GEOMETRIA:") && err.contains("BV"),
        "{err}"
    );
}

#[test]
fn a_self_intersecting_boundary_is_an_error() {
    let mut doc = square(vec![]);
    doc["contorno"] = json!(["1", "3", "7", "9"]);
    let err = rejected("lazo", &doc);
    assert!(
        err.starts_with("E_TOPO_GEOMETRIA:") && err.contains("se corta"),
        "{err}"
    );
}

#[test]
fn a_hole_outside_the_boundary_is_an_error() {
    let extra = vec![
        point("H1", 760_100.0, 9_796_100.0, 2500.0, "EDI", "excluido"),
        point("H2", 760_105.0, 9_796_100.0, 2500.0, "EDI", "excluido"),
        point("H3", 760_105.0, 9_796_105.0, 2500.0, "EDI", "excluido"),
    ];
    let mut doc = square(extra);
    doc["contorno"] = json!(["1", "3", "9", "7"]);
    doc["huecos"] = json!([{ "descripcion": "EDI", "puntos": ["H1", "H2", "H3"] }]);
    let err = rejected("hueco-fuera", &doc);
    assert!(
        err.starts_with("E_TOPO_GEOMETRIA:") && err.contains("fuera"),
        "{err}"
    );
}

#[test]
fn without_boundary_the_report_says_the_hull_is_interpolated() {
    let path = write_temp("casco", &square(vec![]).to_string());
    let mut p = Project::new("casco", "EPSG:32717");
    let rep = import_survey(&mut p, &path).unwrap();
    let _ = std::fs::remove_file(&path);
    assert!(rep.avisos.iter().any(|a| a.contains("casco convexo")));
}

// ---------------------------------------------------------------------------------------------
// Elevation grid (a DEM): it fills only where there is no survey.

fn grid(rows: usize, cols: usize, values: Vec<f64>) -> Value {
    json!({
        "e_centro_noroeste": 760_000.0, "n_centro_noroeste": 9_796_090.0, "celda_m": 30.0,
        "filas": rows, "columnas": cols, "sin_dato": -32767.0, "valores": values,
        "fuente": "Copernicus GLO-30"
    })
}

#[test]
fn a_grid_alone_becomes_a_tin_and_skips_nodata() {
    let mut doc = square(vec![]);
    doc["puntos"] = json!([]);
    doc["datum_vertical"] = json!("egm2008");
    doc["malla"] = grid(4, 4, {
        let mut v: Vec<f64> = (0..16).map(|k| 2500.0 + k as f64).collect();
        v[5] = -32767.0;
        v
    });
    let mut p = Project::new("dem", "EPSG:32717");
    import_json(&mut p, "dem", &doc).unwrap();
    let t = p.terrain.as_ref().unwrap();
    assert_eq!(t.points.len(), 15);
    assert_eq!(t.vertical_datum, VerticalDatum::Egm2008);
    assert_eq!(
        t.survey.as_ref().unwrap().grid.as_ref().unwrap().nodes_used,
        15
    );
}

#[test]
fn survey_points_win_over_the_grid_nearby() {
    let mut doc = square(vec![]);
    doc["malla"] = grid(4, 4, vec![2400.0; 16]);
    let mut p = Project::new("mezcla", "EPSG:32717");
    import_json(&mut p, "mezcla", &doc).unwrap();
    let t = p.terrain.as_ref().unwrap();
    let used = t.survey.as_ref().unwrap().grid.as_ref().unwrap().nodes_used;
    assert!(
        used < 16,
        "grid nodes next to the survey are left out ({used})"
    );
    let z = t.z_at(P2::new(760_005.0, 9_796_005.0)).unwrap();
    assert!((z - 2500.5).abs() < 1e-9, "the survey decides here: {z}");
}

#[test]
fn an_inconsistent_grid_is_an_error() {
    let mut doc = square(vec![]);
    doc["malla"] = grid(4, 4, vec![2400.0; 15]);
    assert!(rejected("malla", &doc).starts_with("E_TOPO_MALLA:"));
}

// ---------------------------------------------------------------------------------------------
// CRS and vertical datum (F17-12).

#[test]
fn a_survey_in_the_northern_hemisphere_enters() {
    let mut doc = square(vec![]);
    doc["crs"] = json!("EPSG:32618");
    let mut p = Project::new("norte", "EPSG:32717");
    import_json(&mut p, "norte", &doc).expect("326NN is UTM WGS 84");
    assert_eq!(p.crs, "EPSG:32618");
    assert_eq!(p.terrain.as_ref().unwrap().epsg, 32618);
}

#[test]
fn a_survey_in_another_zone_or_in_degrees_is_e_crs() {
    let mut p = Project::new("zona", "EPSG:32717");
    import_json(&mut p, "base", &square(vec![])).unwrap();
    p.commit("base");
    let mut doc = square(vec![]);
    doc["crs"] = json!("EPSG:32718");
    let err = import_json(&mut p, "zona", &doc).unwrap_err();
    assert!(err.starts_with("E_CRS:"), "{err}");

    let doc = json!({
        "schema": "redes.topografia/1", "nombre": "g", "crs": "EPSG:32717",
        "datum_vertical": "no_declarado",
        "puntos": [
            point("1", -78.5, -0.2, 2500.0, "TN", "terreno"),
            point("2", -78.4, -0.2, 2500.0, "TN", "terreno"),
            point("3", -78.5, -0.1, 2500.0, "TN", "terreno")
        ]
    });
    let err = rejected("grados", &doc);
    assert!(
        err.starts_with("E_CRS:") && err.contains("lon/lat"),
        "{err}"
    );
}

fn plan_with_datum(datum: &str) -> PathBuf {
    let plan = json!({
        "schema": "redes.plan/1",
        "proyecto": { "nombre": "red", "crs": "EPSG:32717", "datum_vertical": datum },
        "objetos": [
            { "tipo": "nodo", "codigo": "P1", "familia": "saneamiento.pozo",
              "e": 760_005.0, "n": 9_796_005.0, "cota": 2498.0 }
        ]
    });
    write_temp(&format!("plan-{datum}"), &plan.to_string())
}

#[test]
fn a_survey_in_another_datum_than_the_network_is_e_datum() {
    let mut p = Project::new("datum", "EPSG:32717");
    redes_core::ingest::import_plan(&mut p, &plan_with_datum("egm2008")).unwrap();
    assert_eq!(p.vertical_datum, VerticalDatum::Egm2008);
    let mut doc = square(vec![]);
    doc["datum_vertical"] = json!("elipsoidal");
    let err = import_json(&mut p, "elipsoidal", &doc).unwrap_err();
    assert!(err.starts_with("E_DATUM:"), "{err}");
    assert!(
        err.contains("EGM2008") && err.contains("elipsoidal"),
        "{err}"
    );

    doc["datum_vertical"] = json!("egm2008");
    import_json(&mut p, "mismo", &doc).expect("same datum");
}

#[test]
fn a_plan_in_another_datum_than_the_terrain_is_e_datum() {
    let mut p = Project::new("datum", "EPSG:32717");
    let mut doc = square(vec![]);
    doc["datum_vertical"] = json!("egm96");
    import_json(&mut p, "egm96", &doc).unwrap();
    let err = redes_core::ingest::import_plan(&mut p, &plan_with_datum("egm2008")).unwrap_err();
    assert!(err.starts_with("E_DATUM:"), "{err}");
    assert_eq!(p.count(), 0);
}

#[test]
fn an_undeclared_datum_against_a_declared_one_is_a_warning() {
    let mut p = Project::new("datum", "EPSG:32717");
    redes_core::ingest::import_plan(&mut p, &plan_with_datum("egm2008")).unwrap();
    let path = write_temp("sin-declarar", &square(vec![]).to_string());
    let rep = import_survey(&mut p, &path).unwrap();
    let _ = std::fs::remove_file(&path);
    assert!(
        rep.avisos.iter().any(|a| a.contains("no declara datum")),
        "{:?}",
        rep.avisos
    );
}

// ---------------------------------------------------------------------------------------------
// Size: a field survey of 100k points with breaklines, a long boundary and building holes.

/// Deterministic synthetic survey: `n` ground points, 20 meandering breaklines of 200 shots, a
/// 400-vertex boundary and 50 buildings (excluded corners) as holes.
fn big_survey(n: usize) -> Value {
    let mut seed: u64 = 0x2545_F491_4F6C_DD1D;
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed >> 11) as f64 / (1u64 << 53) as f64
    };
    let (e0, n0, side) = (760_000.0, 9_796_000.0, 1_000.0);
    let z = |x: f64, y: f64| 2500.0 + 0.02 * y + 3.0 * (x / 90.0).sin();
    let mut puntos = Vec::with_capacity(n + 5000);
    for k in 0..n {
        let (x, y) = (next() * side, next() * side);
        puntos.push(point(
            &format!("T{k}"),
            e0 + x,
            n0 + y,
            z(x, y),
            "TN",
            "terreno",
        ));
    }
    let mut quiebres = Vec::new();
    for line in 0..20 {
        // Inside the boundary: its inner radius is 473 m around (500, 500).
        let y0 = 150.0 + line as f64 * 35.0;
        let half_chord = (473.0f64.powi(2) - (y0 - 500.0).powi(2)).sqrt() - 20.0;
        let mut numbers = Vec::new();
        for s in 0..200 {
            let x = 500.0 - half_chord + s as f64 * (2.0 * half_chord / 199.0);
            let y = y0 + 6.0 * (x / 37.0).sin();
            let id = format!("Q{line}-{s}");
            puntos.push(point(&id, e0 + x, n0 + y, z(x, y) - 1.0, "BV", "quiebre"));
            numbers.push(id);
        }
        quiebres.push(json!({ "descripcion": format!("BV{line}"), "puntos": numbers }));
    }
    let mut contorno = Vec::new();
    for k in 0..400 {
        let a = k as f64 / 400.0 * std::f64::consts::TAU;
        let r = 498.0 - 25.0 * (5.0 * a).sin().abs();
        let (x, y) = (500.0 + r * a.cos(), 500.0 + r * a.sin());
        let id = format!("L{k}");
        puntos.push(point(&id, e0 + x, n0 + y, z(x, y), "LIM", "contorno"));
        contorno.push(id);
    }
    let mut huecos = Vec::new();
    for b in 0..50 {
        let (cx, cy) = (
            300.0 + (b % 10) as f64 * 40.0,
            350.0 + (b / 10) as f64 * 47.0,
        );
        let corners = [(-6.0, -4.0), (6.0, -4.0), (6.0, 4.0), (-6.0, 4.0)];
        let mut numbers = Vec::new();
        for (c, (dx, dy)) in corners.iter().enumerate() {
            let id = format!("E{b}-{c}");
            puntos.push(point(
                &id,
                e0 + cx + dx,
                n0 + cy + dy,
                z(cx, cy),
                "EDI",
                "excluido",
            ));
            numbers.push(id);
        }
        huecos.push(json!({ "descripcion": format!("EDI {b}"), "puntos": numbers }));
    }
    json!({
        "schema": "redes.topografia/1", "nombre": "grande", "crs": "EPSG:32717",
        "datum_vertical": "egm2008", "puntos": puntos, "quiebres": quiebres,
        "contorno": contorno, "huecos": huecos
    })
}

fn time_import(n: usize) -> (f64, usize) {
    let path = write_temp(&format!("grande-{n}"), &big_survey(n).to_string());
    let mut p = Project::new("grande", "EPSG:32717");
    let t0 = std::time::Instant::now();
    let rep = import_survey(&mut p, &path).expect("synthetic survey imports");
    let ms = t0.elapsed().as_secs_f64() * 1000.0;
    let _ = std::fs::remove_file(&path);
    (ms, rep.triangulos)
}

/// Quadratic validation or clipping shows as growth well above ×4 from 10k to 40k points.
#[test]
fn survey_import_grows_linearly() {
    let (small, _) = time_import(10_000);
    let (large, tris) = time_import(40_000);
    eprintln!("topografía 10k: {small:.0} ms; 40k: {large:.0} ms ({tris} triángulos)");
    assert!(large / small < 10.0, "×{:.1}", large / small);
}

/// Figures: `cargo test --release -p redes-core --test topografia -- --ignored --nocapture`.
#[test]
#[ignore]
fn survey_tin_of_100k_points() {
    let (ms, tris) = time_import(100_000);
    eprintln!("topografía 100k puntos: {ms:.0} ms, {tris} triángulos");
}
