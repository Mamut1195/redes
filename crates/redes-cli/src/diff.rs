//! `redes proyecto diff`: compara dos entregas de un proyecto (REV-08, F13-05).

use std::path::Path;

use redes_core::diff::{Clase, Opciones, a_csv, comparar};
use redes_core::envelope::{Diagnostic, Envelope, Severity};
use serde_json::json;

const CMD: &str = "proyecto.diff";

fn falla(msg: String) -> Envelope {
    let code = msg
        .split(':')
        .next()
        .unwrap_or("E_IO_NOT_FOUND")
        .to_string();
    Envelope::fail(
        CMD,
        vec![Diagnostic::new(&code, Severity::Error, msg).hint(
            "Compare dos `.redes` con el mismo CRS: `redes proyecto diff --a viejo.redes --b nuevo.redes`.",
        )],
    )
}

pub fn ejecutar(
    a: &Path,
    b: &Path,
    tolerancia_pos: f64,
    solo: Option<&str>,
    salida: Option<&Path>,
    dry: bool,
) -> Envelope {
    let solo = match solo {
        None => None,
        Some("diseno") => Some(Clase::Diseno),
        Some("revision") => Some(Clase::Revision),
        Some(otro) => {
            return falla(format!(
                "E_ARG_INVALID: --solo admite `diseno` o `revision`, no `{otro}`"
            ));
        }
    };
    if !tolerancia_pos.is_finite() || tolerancia_pos < 0.0 {
        return falla("E_ARG_INVALID: --tolerancia-pos debe ser un numero >= 0".into());
    }
    let pa = match redes_core::project_io::load(a) {
        Ok(p) => p,
        Err(e) => return falla(e),
    };
    let pb = match redes_core::project_io::load(b) {
        Ok(p) => p,
        Err(e) => return falla(e),
    };
    let op = Opciones {
        tolerancia_pos,
        solo,
        ..Opciones::default()
    };
    let d = match comparar(&pa, &pb, &op) {
        Ok(d) => d,
        Err(e) => return falla(e),
    };
    let por_familia: serde_json::Map<String, serde_json::Value> = d
        .por_familia()
        .into_iter()
        .map(|(k, v)| (k, json!(v)))
        .collect();
    let mut env = Envelope::ok(CMD)
        .with_project(&pb, &b.display().to_string())
        .with_data(json!({
            "a": { "archivo": a.display().to_string(), "nombre": pa.name, "revision": pa.revision },
            "b": { "archivo": b.display().to_string(), "nombre": pb.name, "revision": pb.revision },
            "tolerancias": {
                "posicion_m": op.tolerancia_pos,
                "cota_m": op.tolerancia_cota,
            },
            "solo": solo.map(Clase::key),
            "resumen": {
                "altas": d.altas.len(),
                "bajas": d.bajas.len(),
                "modificados": d.modificados.len(),
                "cambios_diseno": d.cambios_de_diseno(),
                "cambios_revision": d.cambios_de_revision(),
                "sin_cambios": d.vacio(),
                "por_familia": por_familia,
            },
            "altas": d.altas,
            "bajas": d.bajas,
            "modificados": d.modificados,
        }));
    if let Some(destino) = salida {
        if !dry && let Err(e) = std::fs::write(destino, a_csv(&d)) {
            return falla(format!("E_IO_WRITE: {e}"));
        }
        env = env.with_artifact("csv", &destino.display().to_string());
    }
    env
}
