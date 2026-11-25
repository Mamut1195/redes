//! Comandos de revisión de un objeto con observaciones con historial (F13-03).
//!
//! `objeto aprobar | observar | cerrar-observacion | observaciones | reabrir`. La lógica vive en
//! `redes_core::revision`; aquí solo se traduce a argumentos y al sobre JSON.

use clap::Subcommand;
use redes_core::envelope::{Diagnostic, Envelope, Severity};
use redes_core::geom::P3;
use redes_core::model::Project;
use redes_core::revision::{ErrorRevision, Observacion};
use serde_json::json;

#[derive(Subcommand)]
pub enum RevisionCmd {
    /// Marca un objeto como aprobado. Con observaciones abiertas falla (`E_STATE_OBS_ABIERTAS`) salvo que
    /// se cierren todas en el mismo acto con `--cerrar-todas "texto"`.
    Aprobar {
        #[arg(long)]
        codigo: String,
        /// Texto de cierre para todas las observaciones abiertas del objeto.
        #[arg(long, value_name = "TEXTO")]
        cerrar_todas: Option<String>,
    },
    /// Añade una observación abierta (no sobrescribe las anteriores); el objeto queda observado.
    Observar {
        #[arg(long)]
        codigo: String,
        /// Qué hay que corregir en la app de origen.
        #[arg(long, alias = "nota", default_value = "")]
        texto: String,
        /// Punto al que se refiere, `E,N,Z` en coordenadas del proyecto.
        #[arg(long, value_name = "E,N,Z")]
        ubicacion: Option<String>,
    },
    /// Cierra una observación; si era la última abierta, el objeto vuelve a pendiente.
    CerrarObservacion {
        #[arg(long)]
        codigo: String,
        /// Número de la observación (`objeto observaciones`).
        #[arg(long)]
        id: u32,
        /// Qué se hizo para cerrarla.
        #[arg(long)]
        texto: Option<String>,
    },
    /// Historial de observaciones de un objeto (abiertas y cerradas).
    Observaciones {
        #[arg(long)]
        codigo: String,
    },
    /// Devuelve un objeto aprobado a pendiente (con observaciones abiertas falla).
    Reabrir {
        #[arg(long)]
        codigo: String,
    },
}

impl RevisionCmd {
    pub fn id(&self) -> &'static str {
        match self {
            RevisionCmd::Aprobar { .. } => "objeto.aprobar",
            RevisionCmd::Observar { .. } => "objeto.observar",
            RevisionCmd::CerrarObservacion { .. } => "objeto.cerrar_observacion",
            RevisionCmd::Observaciones { .. } => "objeto.observaciones",
            RevisionCmd::Reabrir { .. } => "objeto.reabrir",
        }
    }
}

/// ¿El comando escribe el proyecto?
pub fn muta(id: &str) -> bool {
    matches!(
        id,
        "objeto.aprobar" | "objeto.observar" | "objeto.cerrar_observacion" | "objeto.reabrir"
    )
}

/// Códigos de error propios, para `redes esquema`.
pub const CODIGOS: &[(&str, &str, i32)] = &[
    (
        "E_STATE_OBS_ABIERTAS",
        "el objeto tiene observaciones abiertas (al aprobar o reabrir)",
        3,
    ),
    ("E_STATE_OBS_CERRADA", "la observación ya estaba cerrada", 3),
];

/// Ejecuta el comando. En `--dry-run` la operación se hace igual en memoria (y valida), pero el
/// llamador no guarda.
pub fn ejecutar(p: &mut Project, cmd: RevisionCmd, dry: bool) -> Envelope {
    let id = cmd.id();
    let r = match cmd {
        RevisionCmd::Aprobar {
            codigo,
            cerrar_todas,
        } => p
            .aprobar_objeto(&codigo, cerrar_todas.as_deref())
            .map(|cerradas| json!({ "codigo": codigo, "cerradas": cerradas })),
        RevisionCmd::Observar {
            codigo,
            texto,
            ubicacion,
        } => {
            let ubicacion = match ubicacion.as_deref().map(punto).transpose() {
                Ok(u) => u,
                Err(e) => {
                    return Envelope::fail(
                        id,
                        vec![
                            Diagnostic::new("E_ARG_MISSING", Severity::Error, e)
                                .hint("Use `--ubicacion E,N,Z`, p. ej. `758010.5,9797010,2498.4`."),
                        ],
                    );
                }
            };
            p.observar(&codigo, &texto, ubicacion)
                .map(|n| json!({ "codigo": codigo, "id": n }))
        }
        RevisionCmd::CerrarObservacion {
            codigo,
            id: n,
            texto,
        } => p
            .cerrar_observacion(&codigo, n, texto.as_deref())
            .map(|_| json!({ "codigo": codigo, "id": n })),
        RevisionCmd::Observaciones { codigo } => {
            return match p.observaciones_de(&codigo) {
                Some(obs) => Envelope::ok(id).with_data(json!({
                    "codigo": codigo,
                    "estado": estado_de(p, &codigo),
                    "abiertas": obs.iter().filter(|o| o.abierta()).count(),
                    "observaciones": obs.iter().map(observacion_json).collect::<Vec<_>>(),
                })),
                None => fallo(id, &ErrorRevision::NoExiste { codigo }),
            };
        }
        RevisionCmd::Reabrir { codigo } => p
            .reabrir_objeto(&codigo)
            .map(|_| json!({ "codigo": codigo })),
    };
    match r {
        Ok(mut data) => {
            let codigo = data["codigo"].as_str().unwrap_or_default().to_string();
            let (pnd, ok, obs) = p.revision_counts();
            data["estado"] = json!(estado_de(p, &codigo));
            data["nota"] = json!(
                p.node_by_code(&codigo)
                    .and_then(|n| n.nota_vigente())
                    .or_else(|| p.link_by_code(&codigo).and_then(|l| l.nota_vigente()))
            );
            data["revision"] = json!({ "pendientes": pnd, "aprobados": ok, "observados": obs });
            if dry {
                data["dry_run"] = json!(true);
            }
            Envelope::ok(id).with_data(data)
        }
        Err(e) => fallo(id, &e),
    }
}

fn estado_de(p: &Project, codigo: &str) -> Option<&'static str> {
    p.node_by_code(codigo)
        .map(|n| n.estado.key())
        .or_else(|| p.link_by_code(codigo).map(|l| l.estado.key()))
}

/// Una observación en el sobre JSON (la misma forma que en el `.redes`).
pub fn observacion_json(o: &Observacion) -> serde_json::Value {
    let mut v = serde_json::to_value(o).unwrap_or_default();
    v["estado"] = json!(o.estado.key());
    v
}

fn punto(s: &str) -> Result<P3, String> {
    let v: Vec<f64> = s
        .split(',')
        .map(|t| t.trim().parse::<f64>())
        .collect::<Result<_, _>>()
        .map_err(|_| format!("Ubicación ilegible: {s}"))?;
    match v.as_slice() {
        [x, y, z] if v.iter().all(|c| c.is_finite()) => Ok(P3::new(*x, *y, *z)),
        _ => Err(format!("La ubicación necesita E,N,Z: {s}")),
    }
}

fn fallo(id: &str, e: &ErrorRevision) -> Envelope {
    let pista = match e {
        ErrorRevision::NoExiste { .. } => "Liste con `redes objeto lista`.".to_string(),
        ErrorRevision::TextoVacio => "Use `--texto \"qué hay que corregir\"`.".to_string(),
        ErrorRevision::ObservacionNoExiste { codigo, .. }
        | ErrorRevision::YaCerrada { codigo, .. } => {
            format!("Vea el historial con `redes objeto observaciones --codigo {codigo}`.")
        }
        ErrorRevision::Abiertas { codigo, .. } => format!(
            "Ciérrelas con `redes objeto cerrar-observacion --codigo {codigo} --id N`, o apruebe con `--cerrar-todas \"texto\"`."
        ),
    };
    Envelope::fail(
        id,
        vec![Diagnostic::new(e.codigo(), Severity::Error, e.to_string()).hint(pista)],
    )
}
