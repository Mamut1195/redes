//! `redes importar topografia` with the real binary (F17-11, F17-12): the PNEZD survey translated
//! per the skill validates, the old `importar terreno` is gone, and a datum mismatch fails with a
//! non-zero exit code.

use std::path::{Path, PathBuf};
use std::process::Command;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../redes-core/tests/fixtures")
        .join(name)
}

fn tempdir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("redes-cli-topo-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn run(args: &[&str], dir: &Path) -> (i32, serde_json::Value) {
    let out = Command::new(env!("CARGO_BIN_EXE_redes"))
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    let code = out.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let envelope = serde_json::from_str(&stdout).unwrap_or(serde_json::Value::Null);
    (code, envelope)
}

fn new_project(dir: &Path, crs: &str) -> String {
    let path = dir.join("p.redes");
    let p = path.to_str().unwrap().to_string();
    let (code, _) = run(
        &[
            "proyecto", "nuevo", "--nombre", "t", "--crs", crs, "--salida", &p,
        ],
        dir,
    );
    assert_eq!(code, 0);
    p
}

#[test]
fn the_translated_survey_validates_dry_and_then_imports() {
    let d = tempdir("ok");
    let p = new_project(&d, "EPSG:32717");
    let survey = fixture("topografia_quebrada.topografia.json");
    let survey = survey.to_str().unwrap();
    let before = std::fs::read_to_string(&p).unwrap();

    let (code, j) = run(
        &[
            "-p",
            &p,
            "--dry-run",
            "importar",
            "topografia",
            "--archivo",
            survey,
        ],
        &d,
    );
    assert_eq!((code, &j["ok"]), (0, &serde_json::json!(true)), "{j}");
    assert_eq!(j["data"]["topografia"]["puntos"], 137);
    assert_eq!(
        std::fs::read_to_string(&p).unwrap(),
        before,
        "dry run writes nothing"
    );

    let (code, j) = run(
        &["-p", &p, "importar", "topografia", "--archivo", survey],
        &d,
    );
    assert_eq!(code, 0, "{j}");
    let (_, info) = run(&["-p", &p, "proyecto", "info"], &d);
    assert_eq!(info["data"]["terreno"]["puntos_levantamiento"], 137);
    assert_eq!(info["data"]["terreno"]["datum_vertical"], "no_declarado");
    assert_eq!(info["data"]["terreno"]["contorno"], true);
}

/// The old command read any text as E, N, Z and misplaced a PNEZD survey without a word.
#[test]
fn the_old_terrain_import_no_longer_exists() {
    let d = tempdir("viejo");
    let p = new_project(&d, "EPSG:32717");
    let pnezd = fixture("topografia_quebrada.pnezd.txt");
    let before = std::fs::read_to_string(&p).unwrap();
    let (code, _) = run(
        &[
            "-p",
            &p,
            "importar",
            "terreno",
            "--archivo",
            pnezd.to_str().unwrap(),
        ],
        &d,
    );
    assert_ne!(code, 0);
    let (code, j) = run(
        &[
            "-p",
            &p,
            "importar",
            "topografia",
            "--archivo",
            pnezd.to_str().unwrap(),
        ],
        &d,
    );
    assert_ne!(code, 0);
    assert_eq!(j["ok"], false);
    assert_eq!(j["error"]["code"], "E_FMT_PARSE");
    assert_eq!(std::fs::read_to_string(&p).unwrap(), before);
}

#[test]
fn a_datum_mismatch_fails_with_e_datum_and_exit_code_1() {
    let d = tempdir("datum");
    let p = new_project(&d, "EPSG:32717");
    let plan = d.join("plan.json");
    std::fs::write(
        &plan,
        serde_json::json!({
            "schema": "redes.plan/1",
            "proyecto": { "nombre": "red", "crs": "EPSG:32717", "datum_vertical": "elipsoidal" },
            "objetos": [{ "tipo": "nodo", "codigo": "P1", "familia": "saneamiento.pozo",
                          "e": 760010.0, "n": 9796010.0, "cota": 2498.0 }]
        })
        .to_string(),
    )
    .unwrap();
    let (code, j) = run(
        &[
            "-p",
            &p,
            "importar",
            "plan",
            "--archivo",
            plan.to_str().unwrap(),
        ],
        &d,
    );
    assert_eq!(code, 0, "{j}");
    let survey = fixture("topografia_civil3d.topografia.json");
    let (code, j) = run(
        &[
            "-p",
            &p,
            "importar",
            "topografia",
            "--archivo",
            survey.to_str().unwrap(),
        ],
        &d,
    );
    assert_eq!(code, 1, "{j}");
    assert_eq!(j["error"]["code"], "E_DATUM");
    assert!(j["error"]["hint"].as_str().unwrap().contains("datum"));
}

#[test]
fn a_northern_hemisphere_project_takes_its_survey() {
    let d = tempdir("norte");
    let p = new_project(&d, "EPSG:32618");
    let survey = d.join("norte.json");
    let points: Vec<serde_json::Value> = (0..9)
        .map(|k| {
            serde_json::json!({
                "numero": (k + 1).to_string(), "e": 600_000.0 + (k % 3) as f64 * 10.0,
                "n": 480_000.0 + (k / 3) as f64 * 10.0, "z": 300.0 + k as f64,
                "descripcion": "TN", "clase": "terreno"
            })
        })
        .collect();
    std::fs::write(
        &survey,
        serde_json::json!({
            "schema": "redes.topografia/1", "nombre": "norte", "crs": "EPSG:32618",
            "datum_vertical": "egm2008", "puntos": points
        })
        .to_string(),
    )
    .unwrap();
    let (code, j) = run(
        &[
            "-p",
            &p,
            "importar",
            "topografia",
            "--archivo",
            survey.to_str().unwrap(),
        ],
        &d,
    );
    assert_eq!(code, 0, "{j}");
    let (_, info) = run(&["-p", &p, "proyecto", "info"], &d);
    assert_eq!(info["data"]["crs"], "EPSG:32618");
    assert_eq!(info["data"]["terreno"]["datum_vertical"], "egm2008");
}

#[test]
fn the_schema_lists_the_survey_error_codes() {
    let d = tempdir("esquema");
    let (_, j) = run(&["esquema"], &d);
    let codes = j.to_string();
    for code in [
        "E_DATUM",
        "E_TOPO_PUNTO",
        "E_TOPO_COINCIDENTES",
        "E_TOPO_REFERENCIA",
        "E_TOPO_GEOMETRIA",
        "E_TOPO_MALLA",
        "E_TOPO_VACIA",
    ] {
        assert!(codes.contains(code), "missing {code}");
    }
}
