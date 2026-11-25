//! `redes acta`: acta de revisión en CSV o HTML imprimible (F13-04).
//!
//! El contenido lo arma `redes_core::acta`; aquí se elige el formato, se escribe el archivo y se
//! resume en el sobre. No escribe el proyecto.

use std::path::PathBuf;

use redes_core::acta::Acta;
use redes_core::envelope::{Diagnostic, Envelope, Severity};
use redes_core::model::Project;
use serde_json::json;

#[derive(clap::Args)]
pub struct ActaArgs {
    /// `csv` (Excel, con BOM y `;`) o `html` (imprimible). Sin él se deduce de la extensión de --salida.
    #[arg(long)]
    pub formato: Option<String>,
    /// Archivo de salida. Sin él solo se devuelve el resumen en el sobre JSON.
    #[arg(long)]
    pub salida: Option<PathBuf>,
}

fn falla(code: &str, msg: String, pista: &str) -> Envelope {
    Envelope::fail(
        "acta",
        vec![Diagnostic::new(code, Severity::Error, msg).hint(pista)],
    )
}

pub fn ejecutar(p: &Project, a: &ActaArgs, dry: bool) -> Envelope {
    let formato = a.formato.clone().or_else(|| {
        a.salida
            .as_ref()
            .and_then(|s| s.extension())
            .map(|e| e.to_string_lossy().to_lowercase())
            .map(|e| if e == "htm" { "html".into() } else { e })
    });
    let formato = match formato.as_deref() {
        None if a.salida.is_none() => None,
        Some(f @ ("csv" | "html")) => Some(f),
        otro => {
            return falla(
                "E_ARG_INVALID",
                format!(
                    "formato de acta no admitido: {}",
                    otro.unwrap_or("(sin extensión)")
                ),
                "Use `--formato csv` o `--formato html`, o una salida `.csv` / `.html`.",
            );
        }
    };

    let acta = Acta::de(p, redes_core::acta::ahora());
    let mut env = Envelope::ok("acta");
    let mut escrito = None;
    if let (Some(salida), Some(f)) = (&a.salida, formato) {
        if dry {
            escrito = Some(false);
        } else {
            let texto = if f == "csv" {
                acta.a_csv()
            } else {
                acta.a_html()
            };
            if let Some(dir) = salida.parent()
                && !dir.as_os_str().is_empty()
            {
                let _ = std::fs::create_dir_all(dir);
            }
            if let Err(e) = std::fs::write(salida, texto) {
                return falla(
                    "E_IO_PERMISSION",
                    format!("no se pudo escribir {}: {e}", salida.display()),
                    "Compruebe que la carpeta existe y que el archivo no está abierto en Excel.",
                );
            }
            env = env.with_artifact(f, &salida.display().to_string());
            escrito = Some(true);
        }
    }
    let r = acta.resumen;
    env.with_data(json!({
        "formato": formato,
        "salida": a.salida.as_ref().map(|s| s.display().to_string()),
        "escrito": escrito,
        "emitida_utc": acta.emitida_utc,
        "revision": acta.revision,
        "estado_proyecto": acta.estado_proyecto,
        "con_autor": acta.con_autor(),
        "resumen": {
            "objetos": r.objetos,
            "pendientes": r.pendientes,
            "aprobados": r.aprobados,
            "observados": r.observados,
            "observaciones": r.observaciones,
            "abiertas": r.abiertas,
            "cerradas": r.cerradas,
        },
    }))
}
