//! `plan_desde_tabla.py`: una hoja de cálculo con cotas de clave no entra como solera (F12-01).
//!
//! La prueba ejecuta el script de la skill con el Python del sistema. Si no hay Python en la
//! máquina, se avisa y se salta: el contrato que se comprueba es el del script, no el del motor.

use std::path::{Path, PathBuf};
use std::process::Command;

fn raiz() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

fn python() -> Option<&'static str> {
    ["python", "python3", "py"].into_iter().find(|exe| {
        Command::new(exe)
            .arg("--version")
            .output()
            .is_ok_and(|o| o.status.success())
    })
}

/// Convierte `csv` con el script y devuelve el plan.
fn convertir(nombre: &str, csv: &str, tipo: &str) -> Option<(serde_json::Value, PathBuf)> {
    let exe = python()?;
    let dir = std::env::temp_dir().join(format!("redes-tabla-{}-{nombre}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let entrada = dir.join(format!("{nombre}.csv"));
    let salida = dir.join(format!("{nombre}.json"));
    std::fs::write(&entrada, csv).unwrap();
    let script = raiz()
        .join("skills")
        .join("redes-ingesta")
        .join("scripts")
        .join("plan_desde_tabla.py");
    let out = Command::new(exe)
        .arg("-I")
        .arg(&script)
        .arg(&entrada)
        .args(["--tipo", tipo, "--salida"])
        .arg(&salida)
        .output()
        .expect("ejecuta python");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let plan = serde_json::from_str(&std::fs::read_to_string(&salida).unwrap()).unwrap();
    Some((plan, salida))
}

fn objeto<'a>(plan: &'a serde_json::Value, codigo: &str) -> &'a serde_json::Value {
    plan["objetos"]
        .as_array()
        .unwrap()
        .iter()
        .find(|o| o["codigo"] == codigo)
        .unwrap_or_else(|| panic!("falta {codigo}"))
}

fn limpiar(salida: &Path) {
    if let Some(dir) = salida.parent() {
        let _ = std::fs::remove_dir_all(dir);
    }
}

#[test]
fn un_excel_con_cotas_de_clave_entra_con_solera_igual_a_clave_menos_di() {
    let csv = "codigo,desde,hasta,dn,di,clave_inicio,clave_fin\n\
               T1,P1,P2,315,296.6,2498.500,2498.200\n\
               T2,P1,P2,200,,2498.500,2498.200\n";
    let Some((plan, salida)) = convertir("tramos", csv, "tramos") else {
        eprintln!("sin Python: se salta la prueba de plan_desde_tabla.py");
        return;
    };
    let t1 = objeto(&plan, "T1");
    let solera = t1["cota_solera_inicio"].as_f64().expect("solera de inicio");
    assert!((solera - (2498.5 - 0.2966)).abs() < 1e-3, "solera {solera}");
    let solera_fin = t1["cota_solera_fin"].as_f64().expect("solera de fin");
    assert!((solera_fin - (2498.2 - 0.2966)).abs() < 1e-3);
    // La clave declarada se conserva aparte, con su nombre.
    assert_eq!(t1["attrs"]["cota_clave"].as_f64(), Some(2498.5));
    assert_eq!(t1["attrs"]["cota_clave_fin"].as_f64(), Some(2498.2));
    // Sin DI no se deriva nada: la clave no se hace pasar por solera.
    let t2 = objeto(&plan, "T2");
    assert!(t2.get("cota_solera_inicio").is_none(), "{t2}");
    assert_eq!(t2["attrs"]["cota_clave"].as_f64(), Some(2498.5));
    limpiar(&salida);
}

#[test]
fn la_clave_de_un_pozo_no_se_copia_como_su_solera() {
    let csv = "codigo;e;n;clave;solera\n\
               P1;760000;9796000;2498,5;\n\
               P2;760050;9796000;;2497,9\n";
    let Some((plan, salida)) = convertir("pozos", csv, "pozos") else {
        eprintln!("sin Python: se salta la prueba de plan_desde_tabla.py");
        return;
    };
    let p1 = objeto(&plan, "P1");
    assert!(p1.get("cota").is_none(), "la clave no es la solera: {p1}");
    assert_eq!(p1["attrs"]["cota_clave"].as_f64(), Some(2498.5));
    let p2 = objeto(&plan, "P2");
    assert_eq!(p2["cota"].as_f64(), Some(2497.9));
    limpiar(&salida);
}
