//! Contrato de `redes perfil` con el binario real (F13-02).

use std::path::{Path, PathBuf};
use std::process::Command;

fn corre(args: &[&str], dir: &Path) -> (i32, serde_json::Value) {
    let out = Command::new(env!("CARGO_BIN_EXE_redes"))
        .args(args)
        .current_dir(dir)
        .output()
        .expect("ejecuta el binario");
    let txt = String::from_utf8_lossy(&out.stdout).to_string();
    let j = serde_json::from_str(&txt).unwrap_or_else(|e| panic!("sin sobre ({e}): {txt}"));
    (out.status.code().unwrap_or(-1), j)
}

fn demo(nombre: &str) -> (PathBuf, PathBuf) {
    let d = std::env::temp_dir().join(format!("redes-perfil-{nombre}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    let ruta = d.join("demo.redes");
    let (c, _) = corre(
        &["proyecto", "demo", "--salida", ruta.to_str().unwrap()],
        &d,
    );
    assert_eq!(c, 0);
    (d, ruta)
}

#[test]
fn perfil_desde_hasta_escribe_svg_y_json() {
    let (d, ruta) = demo("ok");
    let svg = d.join("p.svg");
    let (c, j) = corre(
        &[
            "-p",
            ruta.to_str().unwrap(),
            "perfil",
            "--desde",
            "PZ-001",
            "--hasta",
            "PZ-008",
            "--salida",
            svg.to_str().unwrap(),
            "--exageracion",
            "5",
        ],
        &d,
    );
    assert_eq!(c, 0, "{j}");
    assert_eq!(j["schema"], "redes.envelope/1");
    assert_eq!(j["command"], "perfil");
    assert_eq!(j["data"]["perfil"]["tramos"].as_array().unwrap().len(), 7);
    assert_eq!(j["data"]["perfil"]["pozos"].as_array().unwrap().len(), 8);
    assert_eq!(j["data"]["exageracion_efectiva"], 5.0);
    assert_eq!(j["data"]["perfil"]["tramos"][0]["dn_mm"], 250);
    let s = std::fs::read_to_string(&svg).unwrap();
    assert!(s.starts_with("<svg") && s.contains("PZ-008") && s.contains("CL-007"));
    assert!(s.contains("×5,0"));
}

#[test]
fn perfil_por_tramos_y_errores_claros() {
    let (d, ruta) = demo("err");
    let p = ruta.to_str().unwrap();
    let (c, j) = corre(&["-p", p, "perfil", "--tramos", "CL-001,CL-002"], &d);
    assert_eq!(c, 0, "{j}");
    assert_eq!(j["data"]["perfil"]["tramos"].as_array().unwrap().len(), 2);
    let (c, j) = corre(&["-p", p, "perfil", "--tramos", "CL-001,CL-003"], &d);
    assert_eq!(
        (c, j["error"]["code"].as_str()),
        (3, Some("E_TOPO_NO_CONSECUTIVO"))
    );
    let (c, j) = corre(
        &["-p", p, "perfil", "--desde", "PZ-008", "--hasta", "PZ-001"],
        &d,
    );
    assert_eq!(
        (c, j["error"]["code"].as_str()),
        (3, Some("E_TOPO_SIN_RUTA"))
    );
    let (c, j) = corre(&["-p", p, "perfil", "--tramos", "XX-1"], &d);
    assert_eq!(
        (c, j["error"]["code"].as_str()),
        (4, Some("E_STATE_NOT_FOUND"))
    );
    let (c, j) = corre(&["-p", p, "perfil"], &d);
    assert_eq!((c, j["error"]["code"].as_str()), (2, Some("E_ARG_MISSING")));
}

#[test]
fn el_esquema_registra_perfil() {
    let d = std::env::temp_dir();
    let (_, j) = corre(&["esquema"], &d);
    let cmds = j["data"]["comandos"].as_array().unwrap();
    assert!(cmds.iter().any(|c| c["nombre"] == "perfil"));
}
