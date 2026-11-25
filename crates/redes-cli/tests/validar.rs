//! Contrato de `redes validar` (F13-01) con el binario real: es un informe (`ok:true` aunque haya
//! hallazgos), con resumen por regla; solo falla por argumentos o por un archivo de reglas malo.

use std::path::{Path, PathBuf};
use std::process::Command;

fn corre(args: &[&str], dir: &Path) -> (i32, serde_json::Value) {
    let out = Command::new(env!("CARGO_BIN_EXE_redes"))
        .args(args)
        .current_dir(dir)
        .output()
        .expect("ejecuta el binario");
    let texto = String::from_utf8_lossy(&out.stdout).to_string();
    let j = serde_json::from_str(&texto).unwrap_or_else(|e| {
        panic!(
            "stdout no es un sobre ({e}): {texto:?} / {:?}",
            String::from_utf8_lossy(&out.stderr)
        )
    });
    (out.status.code().unwrap_or(-1), j)
}

fn tempdir(nombre: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("redes-validar-{nombre}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("carpeta temporal");
    d
}

/// Proyecto con el fixture de validación importado.
fn proyecto(d: &Path) -> String {
    let ruta = d.join("v.redes");
    let r = ruta.to_str().unwrap().to_string();
    let plan = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../redes-core/tests/fixtures/validar.plan.json");
    let (c, _) = corre(&["proyecto", "nuevo", "--nombre", "v", "--salida", &r], d);
    assert_eq!(c, 0);
    let (c, j) = corre(
        &[
            "-p",
            &r,
            "importar",
            "plan",
            "--archivo",
            plan.to_str().unwrap(),
        ],
        d,
    );
    assert_eq!(c, 0, "{j}");
    r
}

fn regla<'a>(j: &'a serde_json::Value, r: &str) -> &'a serde_json::Value {
    j["data"]["resumen"]
        .as_array()
        .unwrap()
        .iter()
        .find(|x| x["regla"] == r)
        .unwrap_or_else(|| panic!("sin regla {r}"))
}

#[test]
fn validar_es_un_informe_con_resumen_y_hallazgos() {
    let d = tempdir("informe");
    let r = proyecto(&d);
    let (c, j) = corre(&["-p", &r, "validar"], &d);
    assert_eq!(c, 0, "{j}");
    assert_eq!(j["schema"], "redes.envelope/1");
    assert_eq!(j["ok"], true, "es un informe aunque haya hallazgos");
    assert_eq!(j["command"], "validar");
    assert_eq!(j["data"]["fuente_umbrales"], "referencia");
    assert_eq!(regla(&j, "contrapendiente")["incumplen"], 1);
    assert_eq!(regla(&j, "longitud")["no_evaluables"], 1);
    let hallazgos = j["data"]["hallazgos"].as_array().unwrap();
    let h = hallazgos
        .iter()
        .find(|h| h["regla"] == "contrapendiente")
        .unwrap();
    assert_eq!(h["severidad"], "grave");
    assert_eq!(h["objetos"][0], "CL-2");
    assert!(h["ubicacion"][0].as_f64().unwrap() > 758_000.0);
    assert!(h["mensaje"].as_str().unwrap().contains("CL-2"));
    assert!(j["data"]["total"]["incumplen"].as_u64().unwrap() >= 8);

    // --solo limita el informe.
    let (c, j) = corre(&["-p", &r, "validar", "--solo", "pendiente,separacion"], &d);
    assert_eq!(c, 0);
    assert_eq!(j["data"]["resumen"].as_array().unwrap().len(), 2);
    assert!(
        j["data"]["hallazgos"]
            .as_array()
            .unwrap()
            .iter()
            .all(|h| h["regla"] == "pendiente" || h["regla"] == "separacion")
    );
}

#[test]
fn una_regla_desconocida_es_error_de_argumento() {
    let d = tempdir("solo");
    let r = proyecto(&d);
    let (c, j) = corre(&["-p", &r, "validar", "--solo", "pendiente,hidraulica"], &d);
    assert_eq!(c, 2);
    assert_eq!(j["ok"], false);
    assert_eq!(j["error"]["code"], "E_ARG_INVALID");
    assert!(
        j["error"]["hint"]
            .as_str()
            .unwrap()
            .contains("recubrimiento")
    );
}

#[test]
fn los_umbrales_salen_del_archivo_y_se_pueden_guardar() {
    let d = tempdir("reglas");
    let r = proyecto(&d);
    let archivo = d.join("reglas.json");
    std::fs::write(&archivo, r#"{ "profundidad_max_pozo_m": 8.0 }"#).unwrap();
    let a = archivo.to_str().unwrap();
    let (c, j) = corre(
        &[
            "-p",
            &r,
            "validar",
            "--reglas",
            a,
            "--solo",
            "profundidad_pozo",
        ],
        &d,
    );
    assert_eq!(c, 0, "{j}");
    assert_eq!(j["data"]["fuente_umbrales"], "archivo");
    assert_eq!(regla(&j, "profundidad_pozo")["incumplen"], 0);
    assert_eq!(j["data"]["umbrales"]["separacion_horizontal_min_m"], 3.0);

    // --dry-run no guarda.
    corre(
        &["-p", &r, "--dry-run", "validar", "--reglas", a, "--guardar"],
        &d,
    );
    let (_, j) = corre(&["-p", &r, "validar"], &d);
    assert_eq!(j["data"]["fuente_umbrales"], "referencia");
    // --guardar los deja en el proyecto.
    let (c, j) = corre(&["-p", &r, "validar", "--reglas", a, "--guardar"], &d);
    assert_eq!(c, 0);
    assert_eq!(j["data"]["umbrales_guardados"], true);
    let (_, j) = corre(&["-p", &r, "validar", "--solo", "profundidad_pozo"], &d);
    assert_eq!(j["data"]["fuente_umbrales"], "proyecto");
    assert_eq!(regla(&j, "profundidad_pozo")["incumplen"], 0);

    // Un campo mal escrito no se ignora.
    std::fs::write(&archivo, r#"{ "profundidad_maxima": 8.0 }"#).unwrap();
    let (c, j) = corre(&["-p", &r, "validar", "--reglas", a], &d);
    assert_eq!(c, 7);
    assert_eq!(j["error"]["code"], "E_FMT_CAMPO");
    // Ni un umbral sin sentido.
    std::fs::write(
        &archivo,
        r#"{ "pendiente_sanitario": { "min": 0.2, "max": 0.1 } }"#,
    )
    .unwrap();
    let (c, j) = corre(&["-p", &r, "validar", "--reglas", a], &d);
    assert_eq!(c, 7);
    assert_eq!(j["error"]["code"], "E_FMT_PARSE");
}

#[test]
fn validar_aparece_en_el_esquema() {
    let d = tempdir("esquema");
    let (_, j) = corre(&["esquema"], &d);
    let comandos = j["data"]["comandos"].as_array().unwrap();
    assert!(comandos.iter().any(|c| c["nombre"] == "validar"));
    let codigos = j["data"]["codigos_error"].to_string();
    assert!(codigos.contains("E_ARG_INVALID"));
}
