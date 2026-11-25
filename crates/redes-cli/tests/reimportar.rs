//! Contrato de `redes importar reimportar` con el binario real (F13-06, D9).

use std::path::{Path, PathBuf};
use std::process::Command;

fn corre(args: &[&str], dir: &Path) -> (i32, serde_json::Value) {
    let out = Command::new(env!("CARGO_BIN_EXE_redes"))
        .args(args)
        .current_dir(dir)
        .output()
        .expect("ejecuta el binario");
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let v: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("no es un sobre ({e}): {stdout:?}"));
    let codigo = out.status.code().unwrap_or(-1);
    assert_eq!(
        v["ok"].as_bool() == Some(false),
        codigo != 0,
        "ok:false ⇔ código ≠ 0: {v}"
    );
    (codigo, v)
}

fn tempdir(nombre: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("redes-cli-{nombre}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("carpeta temporal");
    d
}

const PZ1: &str = r#"{ "tipo": "nodo", "codigo": "PZ-1", "familia": "saneamiento.pozo", "subtipo": "inspeccion",
  "e": 758010.0, "n": 9797010.0, "cota": 2498.4, "cota_terreno": 2500.1 }"#;
const PZ2: &str = r#"{ "tipo": "nodo", "codigo": "PZ-2", "familia": "saneamiento.pozo", "subtipo": "inspeccion",
  "e": 758040.0, "n": 9797012.0, "cota": 2497.9, "cota_terreno": 2499.8 }"#;

fn cl1(cota: f64) -> String {
    format!(
        r#"{{ "tipo": "arco", "codigo": "CL-1", "familia": "saneamiento.tuberia",
  "subtipo": "colector_sanitario", "desde": "PZ-1", "hasta": "PZ-2", "diametro_mm": 200,
  "cota_solera_inicio": {cota}, "cota_solera_fin": 2497.9 }}"#
    )
}

fn plan(objetos: &[&str]) -> String {
    format!(
        r#"{{ "schema": "redes.plan/1", "proyecto": {{ "nombre": "R", "crs": "EPSG:32717" }},
  "objetos": [{}] }}"#,
        objetos.join(",")
    )
}

fn estado(dir: &Path, codigo: &str) -> String {
    let (_, v) = corre(
        &[
            "-p",
            "p.redes",
            "objeto",
            "observaciones",
            "--codigo",
            codigo,
        ],
        dir,
    );
    v["data"]["estado"].as_str().unwrap_or_default().to_string()
}

#[test]
fn reimportar_conserva_la_revision_y_reinicia_lo_que_cambio() {
    let dir = tempdir("reimportar");
    let base = plan(&[PZ1, PZ2, &cl1(2498.4)]);
    std::fs::write(dir.join("v1.json"), &base).unwrap();
    std::fs::write(dir.join("v1b.json"), &base).unwrap();
    std::fs::write(dir.join("v2.json"), plan(&[PZ1, PZ2, &cl1(2498.1)])).unwrap();
    std::fs::write(dir.join("v3.json"), plan(&[PZ2])).unwrap();
    let p = ["-p", "p.redes"];
    let con = |extra: &[&str]| -> Vec<String> {
        p.iter()
            .chain(extra)
            .map(|s| s.to_string())
            .collect::<Vec<_>>()
    };
    let run = |extra: &[&str]| {
        let a = con(extra);
        let r: Vec<&str> = a.iter().map(String::as_str).collect();
        corre(&r, &dir)
    };

    assert_eq!(
        corre(
            &["proyecto", "nuevo", "--nombre", "R", "--salida", "p.redes"],
            &dir
        )
        .0,
        0
    );
    assert_eq!(run(&["importar", "plan", "--archivo", "v1.json"]).0, 0);
    assert_eq!(run(&["objeto", "aprobar", "--codigo", "CL-1"]).0, 0);
    assert_eq!(run(&["objeto", "aprobar", "--codigo", "PZ-2"]).0, 0);
    assert_eq!(
        run(&["objeto", "observar", "--codigo", "PZ-1", "--texto", "Tapa"]).0,
        0
    );

    // La misma entrega: nada cambia.
    let (c, v) = run(&["importar", "reimportar", "--archivo", "v1b.json"]);
    assert_eq!(c, 0, "{v}");
    assert_eq!(v["command"], "importar.reimportar");
    let r = &v["data"]["resumen"];
    assert_eq!(r["revision_conservada"], 3, "{v}");
    assert_eq!(r["revision_reiniciada"], 0);
    assert_eq!(r["cambiados"], 0);
    assert_eq!(r["observaciones_conservadas"], 1);
    assert_eq!(estado(&dir, "CL-1"), "aprobado");
    assert_eq!(estado(&dir, "PZ-1"), "observado");

    // En seco: el informe completo y el archivo intacto.
    let antes = std::fs::read(dir.join("p.redes")).unwrap();
    let (c, v) = run(&[
        "--dry-run",
        "importar",
        "reimportar",
        "--archivo",
        "v2.json",
    ]);
    assert_eq!(c, 0, "{v}");
    assert_eq!(v["data"]["reiniciados"], serde_json::json!(["CL-1"]));
    assert_eq!(v["data"]["guardado"], false);
    assert_eq!(v["data"]["revision"]["aprobados"], 1, "lo que quedaría");
    assert_eq!(std::fs::read(dir.join("p.redes")).unwrap(), antes);

    // De verdad: CL-1 vuelve a pendiente y la revisión del proyecto sube.
    let (c, v) = run(&["importar", "reimportar", "--archivo", "v2.json"]);
    assert_eq!(c, 0, "{v}");
    let cambiado = &v["data"]["cambiados"][0];
    assert_eq!(cambiado["codigo"], "CL-1");
    assert_eq!(cambiado["revision"], "reiniciada");
    assert_eq!(estado(&dir, "CL-1"), "pendiente");
    assert_eq!(estado(&dir, "PZ-2"), "aprobado");

    // Bajas con revisión: falla con E_STATE_BAJAS_CON_REVISION y no escribe.
    let antes = std::fs::read(dir.join("p.redes")).unwrap();
    let (c, v) = run(&["importar", "reimportar", "--archivo", "v3.json"]);
    assert_eq!(c, 3, "{v}");
    assert_eq!(v["error"]["code"], "E_STATE_BAJAS_CON_REVISION");
    let bajas = v["data"]["bajas"].as_array().unwrap();
    let pz1 = bajas.iter().find(|b| b["codigo"] == "PZ-1").unwrap();
    assert_eq!(pz1["estado"], "observado");
    assert_eq!(pz1["observaciones"][0]["texto"], "Tapa");
    assert_eq!(std::fs::read(dir.join("p.redes")).unwrap(), antes);

    let (c, v) = run(&[
        "importar",
        "reimportar",
        "--archivo",
        "v3.json",
        "--aceptar-bajas",
    ]);
    assert_eq!(c, 0, "{v}");
    assert_eq!(v["data"]["resumen"]["bajas"], 2);
    assert_eq!(estado(&dir, "PZ-2"), "aprobado");

    // Un formato desconocido es un error de argumento.
    let (c, v) = run(&[
        "importar",
        "reimportar",
        "--archivo",
        "v3.json",
        "--formato",
        "dwg",
    ]);
    assert_eq!(c, 2, "{v}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn el_esquema_lista_reimportar_y_su_codigo() {
    let dir = tempdir("reimportar-esquema");
    let (c, v) = corre(&["esquema"], &dir);
    assert_eq!(c, 0);
    let comandos = v["data"]["comandos"].as_array().unwrap();
    let cmd = comandos
        .iter()
        .find(|x| x["nombre"] == "importar reimportar")
        .expect("importar reimportar en el esquema");
    let args: Vec<&str> = cmd["argumentos"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|a| a["nombre"].as_str())
        .collect();
    for a in ["archivo", "formato", "aceptar-bajas", "tolerancia-pos"] {
        assert!(args.contains(&a), "{a} en {args:?}");
    }
    let codigos = v["data"]["codigos_error"].to_string();
    assert!(codigos.contains("E_STATE_BAJAS_CON_REVISION"));
    let _ = std::fs::remove_dir_all(&dir);
}
