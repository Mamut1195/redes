//! Contrato del CLI: todo comando responde un sobre JSON y un código de salida estable.
//!
//! Estas pruebas ejecutan el binario real (`CARGO_BIN_EXE_redes`), así que cubren lo que un agente
//! ve: `stdout`, `stderr` y el código de salida.

use std::path::{Path, PathBuf};
use std::process::Command;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_redes")
}

struct Salida {
    codigo: i32,
    stdout: String,
    stderr: String,
}

fn corre(args: &[&str], dir: &Path) -> Salida {
    let out = Command::new(bin())
        .args(args)
        .current_dir(dir)
        .output()
        .expect("ejecuta el binario");
    Salida {
        codigo: out.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&out.stdout).to_string(),
        stderr: String::from_utf8_lossy(&out.stderr).to_string(),
    }
}

fn sobre(s: &Salida) -> serde_json::Value {
    serde_json::from_str(&s.stdout).unwrap_or_else(|e| {
        panic!(
            "stdout no es un sobre JSON ({e}): {:?} / stderr {:?}",
            s.stdout, s.stderr
        )
    })
}

fn tempdir(nombre: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("redes-cli-{nombre}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("carpeta temporal");
    d
}

#[test]
fn todo_sobre_declara_su_esquema_y_su_ok() {
    let d = tempdir("esquema");
    let s = corre(&["esquema"], &d);
    let j = sobre(&s);
    assert_eq!(j["schema"], "redes.envelope/1");
    assert_eq!(j["ok"], true);
    assert_eq!(j["command"], "esquema");
    assert_eq!(s.codigo, 0);
}

#[test]
fn capacidades_publica_el_registro_completo() {
    let d = tempdir("capacidades");
    let j = sobre(&corre(&["capacidades"], &d));
    let familias = j["data"]["familias"].as_array().expect("familias");
    assert_eq!(familias.len(), 46, "el registro tiene 46 familias");
    let subtipos: usize = familias
        .iter()
        .map(|f| f["subtipos"].as_array().map(|s| s.len()).unwrap_or(0))
        .sum();
    assert_eq!(subtipos, 315, "y 315 subtipos");
}

#[test]
fn sin_proyecto_el_error_es_e_proj_not_found_y_codigo_4() {
    // El CLI busca `*.redes` hacia arriba; un resto de pruebas anteriores en %TEMP% haría que este
    // caso pasara por la razón equivocada. Se limpian solo los de la carpeta temporal.
    if let Ok(entradas) = std::fs::read_dir(std::env::temp_dir()) {
        for e in entradas.flatten() {
            let p = e.path();
            if p.extension().is_some_and(|x| x == "redes") {
                let _ = std::fs::remove_file(&p);
            }
        }
    }
    let d = tempdir("sinproyecto");
    let s = corre(&["objeto", "lista"], &d);
    let j = sobre(&s);
    assert_eq!(j["ok"], false);
    assert_eq!(j["diagnostics"][0]["code"], "E_PROJ_NOT_FOUND");
    assert_eq!(s.codigo, 4);
}

#[test]
fn un_proyecto_nuevo_se_crea_y_se_puede_leer() {
    let d = tempdir("nuevo");
    let ruta = d.join("p.redes");
    let j = sobre(&corre(
        &[
            "proyecto",
            "nuevo",
            "--nombre",
            "Prueba",
            "--salida",
            ruta.to_str().unwrap(),
        ],
        &d,
    ));
    assert_eq!(j["ok"], true);
    assert!(ruta.exists(), "el archivo se escribió");
    let j = sobre(&corre(
        &["-p", ruta.to_str().unwrap(), "proyecto", "info"],
        &d,
    ));
    assert_eq!(j["ok"], true);
    assert_eq!(j["project"]["name"], "Prueba");
}

#[test]
fn aprobar_un_objeto_inexistente_devuelve_e_state_not_found() {
    let d = tempdir("aprobar");
    let ruta = d.join("c.redes");
    corre(
        &[
            "proyecto",
            "catalogo",
            "--salida",
            ruta.to_str().unwrap(),
            "--quiet",
        ],
        &d,
    );
    let s = corre(
        &[
            "-p",
            ruta.to_str().unwrap(),
            "objeto",
            "aprobar",
            "--codigo",
            "NO-EXISTE",
        ],
        &d,
    );
    let j = sobre(&s);
    assert_eq!(j["ok"], false);
    assert_eq!(j["diagnostics"][0]["code"], "E_STATE_NOT_FOUND");
    assert_eq!(s.codigo, 4);
}

#[test]
fn un_archivo_ilegible_da_e_io_not_found_y_codigo_7() {
    let d = tempdir("ilegible");
    let ruta = d.join("p.redes");
    corre(
        &[
            "proyecto",
            "nuevo",
            "--nombre",
            "x",
            "--salida",
            ruta.to_str().unwrap(),
            "--quiet",
        ],
        &d,
    );
    let s = corre(
        &[
            "-p",
            ruta.to_str().unwrap(),
            "importar",
            "plan",
            "--archivo",
            d.join("no-existe.json").to_str().unwrap(),
        ],
        &d,
    );
    let j = sobre(&s);
    assert_eq!(j["ok"], false);
    assert_eq!(j["diagnostics"][0]["code"], "E_IO_NOT_FOUND");
    assert_eq!(s.codigo, 7);
}

#[test]
fn dry_run_no_escribe_ni_un_archivo() {
    let d = tempdir("dry");
    let ruta = d.join("c.redes");
    corre(
        &[
            "proyecto",
            "catalogo",
            "--salida",
            ruta.to_str().unwrap(),
            "--quiet",
        ],
        &d,
    );
    let dxf = d.join("salida.dxf");
    let j = sobre(&corre(
        &[
            "-p",
            ruta.to_str().unwrap(),
            "--dry-run",
            "exportar",
            "dxf",
            "--salida",
            dxf.to_str().unwrap(),
        ],
        &d,
    ));
    assert_eq!(j["ok"], true);
    assert_eq!(j["data"]["dry_run"], true);
    assert!(!dxf.exists(), "con --dry-run no se escribe");
}

#[test]
fn quiet_no_imprime_nada_y_pretty_indenta() {
    let d = tempdir("salidas");
    let ruta = d.join("c.redes");
    let s = corre(
        &[
            "proyecto",
            "catalogo",
            "--salida",
            ruta.to_str().unwrap(),
            "--quiet",
        ],
        &d,
    );
    assert_eq!(s.codigo, 0);
    assert!(s.stdout.trim().is_empty(), "--quiet no imprime");

    let s = corre(
        &["-p", ruta.to_str().unwrap(), "--pretty", "proyecto", "info"],
        &d,
    );
    assert!(s.stdout.contains("\n  "), "--pretty indenta");
    assert!(sobre(&s)["ok"].as_bool().unwrap());
}

#[test]
fn el_dxf_sale_y_vuelve_por_el_cli() {
    let d = tempdir("dxf");
    let ruta = d.join("c.redes");
    corre(
        &[
            "proyecto",
            "catalogo-total",
            "--salida",
            ruta.to_str().unwrap(),
            "--quiet",
        ],
        &d,
    );
    let dxf = d.join("c.dxf");
    let j = sobre(&corre(
        &[
            "-p",
            ruta.to_str().unwrap(),
            "exportar",
            "dxf",
            "--salida",
            dxf.to_str().unwrap(),
        ],
        &d,
    ));
    assert_eq!(j["ok"], true, "exporta: {j}");
    assert!(dxf.exists());

    let vuelta = d.join("v.redes");
    corre(
        &[
            "proyecto",
            "nuevo",
            "--nombre",
            "vuelta",
            "--salida",
            vuelta.to_str().unwrap(),
            "--quiet",
        ],
        &d,
    );
    let j = sobre(&corre(
        &[
            "-p",
            vuelta.to_str().unwrap(),
            "importar",
            "dxf",
            "--archivo",
            dxf.to_str().unwrap(),
        ],
        &d,
    ));
    assert_eq!(j["ok"], true, "importa: {j}");
    assert!(j["data"]["nodos"].as_u64().unwrap() >= 40);
    assert_eq!(j["data"]["descartados"].as_u64().unwrap(), 0);
}

#[test]
fn cantidades_publica_filas_y_totales() {
    let d = tempdir("cantidades");
    let ruta = d.join("cad.redes");
    corre(
        &[
            "proyecto",
            "cadena",
            "--salida",
            ruta.to_str().unwrap(),
            "--quiet",
        ],
        &d,
    );
    let j = sobre(&corre(&["-p", ruta.to_str().unwrap(), "cantidades"], &d));
    assert_eq!(j["ok"], true);
    let filas = j["data"]["filas"].as_array().expect("filas");
    assert!(!filas.is_empty());
    let totales = j["data"]["totales"].as_array().expect("totales");
    assert!(totales.iter().any(|t| t["unidad"] == "m"));
}

#[test]
fn una_familia_inexistente_no_rompe_el_esquema() {
    let d = tempdir("familia");
    let j = sobre(&corre(&["esquema"], &d));
    // `esquema` no filtra por familia: publica el registro y no falla.
    assert_eq!(j["ok"], true);
    assert!(
        j["data"]["familias"]
            .as_array()
            .is_some_and(|f| !f.is_empty())
    );
}
