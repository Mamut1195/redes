//! Contrato de `redes acta` con el binario real (F13-04).

use std::path::{Path, PathBuf};
use std::process::Command;

fn corre(args: &[&str], dir: &Path) -> (i32, serde_json::Value) {
    let out = Command::new(env!("CARGO_BIN_EXE_redes"))
        .args(args)
        .current_dir(dir)
        .env_remove("REDES_JOURNAL_USUARIO")
        .output()
        .expect("ejecuta el binario");
    let txt = String::from_utf8_lossy(&out.stdout).to_string();
    let j = serde_json::from_str(&txt).unwrap_or_else(|e| panic!("sin sobre ({e}): {txt}"));
    (out.status.code().unwrap_or(-1), j)
}

fn demo(nombre: &str) -> (PathBuf, PathBuf) {
    let d = std::env::temp_dir().join(format!("redes-acta-{nombre}-{}", std::process::id()));
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
fn acta_csv_y_html_con_las_observaciones() {
    let (d, ruta) = demo("ok");
    let p = ruta.to_str().unwrap();
    for (cod, txt) in [
        ("PZ-001", "Cota de tapa no coincide con el levantamiento"),
        ("CL-002", "Revisar dirección del flujo"),
    ] {
        let (c, j) = corre(
            &[
                "-p", p, "objeto", "observar", "--codigo", cod, "--texto", txt,
            ],
            &d,
        );
        assert_eq!(c, 0, "{j}");
    }

    let csv = d.join("acta.csv");
    let (c, j) = corre(&["-p", p, "acta", "--salida", csv.to_str().unwrap()], &d);
    assert_eq!(c, 0, "{j}");
    assert_eq!(j["ok"], true);
    assert_eq!(j["command"], "acta");
    assert_eq!(j["data"]["formato"], "csv");
    assert_eq!(j["data"]["escrito"], true);
    assert_eq!(j["data"]["resumen"]["observaciones"], 2);
    assert_eq!(j["data"]["resumen"]["abiertas"], 2);
    assert_eq!(j["data"]["resumen"]["observados"], 2);
    assert_eq!(j["data"]["con_autor"], false);
    let bytes = std::fs::read(&csv).unwrap();
    assert_eq!(&bytes[..3], b"\xEF\xBB\xBF", "BOM UTF-8");
    let s = String::from_utf8(bytes).unwrap();
    assert!(s.contains("Revisar dirección del flujo"));
    assert!(
        s.lines()
            .any(|l| l.starts_with("PZ-001;") && l.contains(";#1;abierta;"))
    );

    let html = d.join("acta.html");
    let (c, j) = corre(
        &[
            "-p",
            p,
            "acta",
            "--formato",
            "html",
            "--salida",
            html.to_str().unwrap(),
        ],
        &d,
    );
    assert_eq!(c, 0, "{j}");
    let h = std::fs::read_to_string(&html).unwrap();
    assert!(h.contains("Cota de tapa no coincide") && h.contains("@media print"));

    // `--dry-run` no escribe; sin `--salida` solo devuelve el resumen.
    let seco = d.join("seco.html");
    let (c, j) = corre(
        &[
            "-p",
            p,
            "--dry-run",
            "acta",
            "--salida",
            seco.to_str().unwrap(),
        ],
        &d,
    );
    assert_eq!((c, j["data"]["escrito"].as_bool()), (0, Some(false)));
    assert!(!seco.exists());
    let (c, j) = corre(&["-p", p, "acta"], &d);
    assert_eq!(c, 0, "{j}");
    assert!(j["data"]["formato"].is_null());
}

#[test]
fn un_formato_desconocido_falla_con_salida_distinta_de_cero() {
    let (d, ruta) = demo("err");
    let p = ruta.to_str().unwrap();
    let (c, j) = corre(&["-p", p, "acta", "--salida", "acta.pdf"], &d);
    assert_ne!(c, 0);
    assert_eq!(j["ok"], false);
    assert_eq!(j["error"]["code"], "E_ARG_INVALID");
    let (c, j) = corre(&["-p", p, "acta", "--formato", "xlsx"], &d);
    assert_ne!(c, 0);
    assert_eq!(j["error"]["code"], "E_ARG_INVALID");
    assert!(!d.join("acta.pdf").exists());
}
