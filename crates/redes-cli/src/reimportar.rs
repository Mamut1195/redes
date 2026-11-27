//! `redes importar reimportar`: carga una entrega nueva sin perder la revisión (ING-10, F13-06).
//!
//! La lógica vive en `redes_core::reimportar` (D9); aquí solo se traduce a argumentos y al sobre.

use std::path::PathBuf;

use clap::Args;
use redes_core::diff::Opciones;
use redes_core::envelope::{Diagnostic, Envelope, Severity};
use redes_core::model::Project;
use redes_core::reimportar::{Formato, Informe, preparar};
use serde_json::json;

pub const CMD: &str = "importar.reimportar";

#[derive(Args)]
pub struct ReimportarArgs {
    /// Entrega nueva: `redes.plan/1` (.json) o DXF exportado por Redes (.dxf).
    #[arg(long)]
    pub archivo: PathBuf,
    /// `plan` o `dxf`; por defecto, por la extensión del archivo.
    #[arg(long)]
    pub formato: Option<String>,
    /// Acepta dar de baja objetos que tenían revisión (aprobado, observado u observaciones).
    #[arg(long)]
    pub aceptar_bajas: bool,
    /// Movimiento en planta que no cuenta como cambio de diseño (m).
    #[arg(long, default_value_t = 0.01)]
    pub tolerancia_pos: f64,
}

/// Códigos de error propios, para `redes esquema`.
pub const CODIGOS: &[(&str, &str, i32)] = &[(
    "E_STATE_BAJAS_CON_REVISION",
    "reimportar: la entrega ya no trae objetos con revisión; se aceptan con --aceptar-bajas",
    3,
)];

fn falla(code: &str, msg: String, pista: &str) -> Envelope {
    Envelope::fail(
        CMD,
        vec![Diagnostic::new(code, Severity::Error, msg).hint(pista)],
    )
}

pub fn ejecutar(p: &mut Project, a: &ReimportarArgs, dry: bool) -> Envelope {
    let formato = match a.formato.as_deref() {
        None => Formato::por_extension(&a.archivo),
        Some("plan") => Formato::Plan,
        Some("dxf") => Formato::Dxf,
        Some(otro) => {
            return falla(
                "E_ARG_INVALID",
                format!("E_ARG_INVALID: --formato admite `plan` o `dxf`, no `{otro}`"),
                "Omítalo para deducirlo de la extensión.",
            );
        }
    };
    if !a.tolerancia_pos.is_finite() || a.tolerancia_pos < 0.0 {
        return falla(
            "E_ARG_INVALID",
            "E_ARG_INVALID: --tolerancia-pos debe ser un numero >= 0".into(),
            "Por defecto 0,01 m.",
        );
    }
    let op = Opciones {
        tolerancia_pos: a.tolerancia_pos,
        ..Opciones::default()
    };
    let prep = match preparar(p, &a.archivo, formato, &op) {
        Ok(x) => x,
        Err(e) => {
            let code = e.split(':').next().unwrap_or("E_FMT_PARSE").to_string();
            return falla(
                &code,
                e,
                "La entrega entra como `redes.plan/1` (skill `skills/redes-ingesta`) o como el DXF que exportó Redes; mismo CRS que el proyecto.",
            );
        }
    };
    if let Some(e) = prep.informe.bloqueo(a.aceptar_bajas) {
        return falla(
            "E_STATE_BAJAS_CON_REVISION",
            e,
            "Revise `data.bajas` (trae su estado y sus observaciones). Si la baja es correcta, repita con `--aceptar-bajas`; si no, pida la entrega corregida.",
        )
        .with_data(datos(&prep.informe, &a.archivo, dry, false, p));
    }
    let guardado = !dry;
    if dry {
        let data = datos(&prep.informe, &a.archivo, dry, guardado, &prep.proyecto);
        return Envelope::ok(CMD).with_data(data);
    }
    let informe = prep.aplicar(p);
    Envelope::ok(CMD).with_data(datos(&informe, &a.archivo, dry, guardado, p))
}

fn datos(
    i: &Informe,
    archivo: &std::path::Path,
    dry: bool,
    guardado: bool,
    resultado: &Project,
) -> serde_json::Value {
    let (pnd, ok, obs) = resultado.revision_counts();
    let r = &i.importacion;
    json!({
        "archivo": archivo.display().to_string(),
        "formato": i.formato,
        "resumen": {
            "altas": i.altas.len(),
            "bajas": i.bajas.len(),
            "bajas_con_revision": i.bajas_con_revision().count(),
            "cambiados": i.cambiados.len(),
            "sin_cambios": i.sin_cambios,
            "revision_conservada": i.revision_conservada,
            "revision_reiniciada": i.reiniciados.len(),
            "observaciones_conservadas": i.observaciones_conservadas,
            "cambia_el_diseno": i.cambia_el_diseno(),
        },
        "altas": i.altas,
        "bajas": i.bajas,
        "cambiados": i.cambiados,
        "reiniciados": i.reiniciados,
        "estado_proyecto": { "antes": i.estado_proyecto_antes, "despues": i.estado_proyecto_despues },
        "revision": { "pendientes": pnd, "aprobados": ok, "observados": obs },
        "importacion": {
            "nodos": r.nodos,
            "arcos": r.arcos,
            "puntos_terreno": r.puntos_terreno,
            "descartados": r.descartados,
            "avisos": r.avisos,
        },
        "dry_run": dry,
        "guardado": guardado,
        "siguiente": if dry {
            "sin --dry-run para reimportar"
        } else if i.reiniciados.is_empty() {
            "redes objeto lista --estado pendiente"
        } else {
            "revise los reiniciados: redes objeto lista --estado pendiente"
        },
    })
}
