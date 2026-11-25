//! `redes validar` (F13-01): informe de comprobaciones geométricas sobre los datos importados.
//!
//! Es un informe: sale `ok:true` aunque haya hallazgos. Solo falla por argumentos (regla
//! desconocida en `--solo`) o por un archivo de reglas ilegible o con campos que no existen.

use std::path::{Path, PathBuf};

use clap::Args;
use redes_core::envelope::{Diagnostic, Envelope, Severity};
use redes_core::model::Project;
use redes_core::validar::{self, REGLAS, ReglasValidacion};
use serde_json::json;

#[derive(Args)]
pub struct ValidarArgs {
    /// Archivo JSON de umbrales; los que no traiga se toman del proyecto o de la referencia.
    #[arg(long, value_name = "ARCHIVO")]
    pub reglas: Option<PathBuf>,
    /// Solo estas reglas, separadas por comas (p. ej. `pendiente,recubrimiento`).
    #[arg(long, value_delimiter = ',', value_name = "REGLAS")]
    pub solo: Vec<String>,
    /// Guarda en el proyecto los umbrales usados, para las próximas validaciones.
    #[arg(long)]
    pub guardar: bool,
}

pub fn ejecutar(p: &mut Project, path: &Path, args: &ValidarArgs, dry: bool) -> Envelope {
    let solo: Vec<&str> = args
        .solo
        .iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();
    if let Some(mala) = validar::regla_desconocida(&solo) {
        let validas: Vec<&str> = REGLAS.iter().map(|(k, _)| *k).collect();
        return Envelope::fail(
            "validar",
            vec![
                Diagnostic::new(
                    "E_ARG_INVALID",
                    Severity::Error,
                    format!("regla desconocida en --solo: {mala}"),
                )
                .field("solo")
                .hint(format!("Reglas: {}", validas.join(", "))),
            ],
        );
    }

    let (reglas, fuente) = match resolver_reglas(p, args.reglas.as_deref()) {
        Ok(v) => v,
        Err(d) => return Envelope::fail("validar", vec![d]),
    };

    let informe = validar::validar(p, &reglas, (!solo.is_empty()).then_some(solo.as_slice()));
    let graves = informe
        .hallazgos
        .iter()
        .filter(|h| h.severidad == validar::Severidad::Grave)
        .count();

    let mut guardado = false;
    if args.guardar && !dry {
        p.reglas_validacion = Some(reglas.clone());
        p.commit("validar.guardar_reglas");
        if let Err(e) = redes_core::project_io::save(p, path) {
            return Envelope::fail(
                "validar",
                vec![Diagnostic::new(
                    e.split(':').next().unwrap_or("E_IO_PERMISSION"),
                    Severity::Error,
                    e.clone(),
                )],
            );
        }
        guardado = true;
    }

    Envelope::ok("validar").with_data(json!({
        "alcance": "comprobaciones geométricas sobre los datos importados; no es cálculo hidráulico ni verificación normativa",
        "umbrales": reglas,
        "fuente_umbrales": fuente,
        "nota_umbrales": "los valores de referencia son típicos, no una norma: ajústelos a la suya con --reglas",
        "umbrales_guardados": guardado,
        "total": {
            "hallazgos": informe.hallazgos.len(),
            "incumplen": informe.incumplen(),
            "graves": graves,
            "no_evaluables": informe.no_evaluables(),
        },
        "resumen": informe.resumen,
        "hallazgos": informe.hallazgos,
    }))
}

/// Umbrales: referencia ← proyecto ← archivo (cada capa pisa solo las claves que trae).
#[allow(clippy::result_large_err)]
fn resolver_reglas(
    p: &Project,
    archivo: Option<&Path>,
) -> Result<(ReglasValidacion, &'static str), Diagnostic> {
    let base = p.reglas_validacion.clone().unwrap_or_default();
    let fuente_base = if p.reglas_validacion.is_some() {
        "proyecto"
    } else {
        "referencia"
    };
    let Some(ruta) = archivo else {
        return Ok((base, fuente_base));
    };
    let texto = std::fs::read_to_string(ruta).map_err(|e| {
        Diagnostic::new(
            "E_IO_NOT_FOUND",
            Severity::Error,
            format!("{}: {e}", ruta.display()),
        )
        .field("reglas")
    })?;
    let crudo: serde_json::Value = serde_json::from_str(&texto).map_err(|e| {
        Diagnostic::new(
            "E_FMT_PARSE",
            Severity::Error,
            format!("archivo de reglas ilegible: {e}"),
        )
        .field("reglas")
    })?;
    let Some(obj) = crudo.as_object() else {
        return Err(Diagnostic::new(
            "E_FMT_PARSE",
            Severity::Error,
            "el archivo de reglas debe ser un objeto JSON",
        )
        .field("reglas"));
    };
    let claves = ReglasValidacion::claves();
    if let Some(mala) = obj.keys().find(|k| !claves.contains(k)) {
        return Err(Diagnostic::new(
            "E_FMT_CAMPO",
            Severity::Error,
            format!("campo desconocido en el archivo de reglas: {mala}"),
        )
        .field(mala)
        .hint(format!("Campos admitidos: {}", claves.join(", "))));
    }
    let mut v = serde_json::to_value(&base).unwrap_or_default();
    if let Some(m) = v.as_object_mut() {
        for (k, x) in obj {
            m.insert(k.clone(), x.clone());
        }
    }
    let reglas: ReglasValidacion = serde_json::from_value(v).map_err(|e| {
        Diagnostic::new(
            "E_FMT_PARSE",
            Severity::Error,
            format!("archivo de reglas: {e}"),
        )
        .field("reglas")
    })?;
    reglas.comprobar().map_err(|e| {
        Diagnostic::new(
            "E_FMT_PARSE",
            Severity::Error,
            format!("umbral inválido: {e}"),
        )
        .field("reglas")
    })?;
    Ok((reglas, "archivo"))
}
