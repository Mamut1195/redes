//! El sobre JSON único de la CLI (`redes.envelope/1`).
//!
//! Contrato (docs/council/03-cli-ai.md §6):
//! - Todas las claves están **siempre** presentes.
//! - `ok: false` ⟺ `meta.exit_code != 0` (invariante testeable).
//! - `command` es el id canónico de la acción (`pozo.crear`), no la línea de comandos.
//! - Los diagnósticos son la unidad atómica de información: código estable + mensaje + pista
//!   + comando sugerido.

use serde::{Deserialize, Serialize};

use crate::model::Project;

pub const ENVELOPE_SCHEMA: &str = "redes.envelope/1";
pub const ENGINE_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
    Info,
}

/// Unidad atómica de información: código estable + mensaje + pista ejecutable.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Diagnostic {
    pub code: String,
    pub severity: Severity,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub object: Option<crate::model::ElementId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub object_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

impl Diagnostic {
    pub fn new(code: &str, severity: Severity, message: impl Into<String>) -> Diagnostic {
        Diagnostic {
            code: code.into(),
            severity,
            message: message.into(),
            object: None,
            object_code: None,
            field: None,
            hint: None,
            details: None,
        }
    }
    pub fn at(mut self, id: crate::model::ElementId, code: &str) -> Self {
        self.object = Some(id);
        self.object_code = Some(code.to_string());
        self
    }
    pub fn field(mut self, f: &str) -> Self {
        self.field = Some(f.into());
        self
    }
    pub fn hint(mut self, h: impl Into<String>) -> Self {
        self.hint = Some(h.into());
        self
    }
    pub fn details(mut self, d: serde_json::Value) -> Self {
        self.details = Some(d);
        self
    }
    pub fn is_error(&self) -> bool {
        self.severity == Severity::Error
    }
}

/// Códigos de salida del contrato.
pub mod exit {
    pub const OK: i32 = 0;
    pub const ERROR: i32 = 1;
    pub const ARG: i32 = 2;
    pub const STATE: i32 = 3;
    pub const NOT_FOUND: i32 = 4;
    pub const CONFLICT: i32 = 5;
    pub const IO: i32 = 7;
}

/// Código de salida para un diagnóstico, según su área.
pub fn exit_for(d: &Diagnostic) -> i32 {
    let c = d.code.as_str();
    if c.starts_with("E_ARG") {
        exit::ARG
    } else if c.starts_with("E_IO") || c.starts_with("E_FMT") {
        exit::IO
    } else if c.starts_with("E_PROJ") || c.starts_with("E_STATE_NOT_FOUND") {
        exit::NOT_FOUND
    } else if c.starts_with("E_STATE")
        || c.starts_with("E_GEO")
        || c.starts_with("E_TOPO")
        || c.starts_with("E_HYD")
    {
        exit::STATE
    } else {
        exit::ERROR
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProjectRef {
    pub id: String,
    pub path: String,
    pub name: String,
    pub crs: String,
    pub units: String,
    pub revision: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UnitsCtx {
    pub system: String,
    pub length: String,
    pub area: String,
    pub volume: String,
    pub flow: String,
    pub velocity: String,
    pub pressure: String,
    pub slope: String,
    pub diameter: String,
}

impl Default for UnitsCtx {
    fn default() -> Self {
        UnitsCtx {
            system: "si".into(),
            length: "m".into(),
            area: "m2".into(),
            volume: "m3".into(),
            flow: "L/s".into(),
            velocity: "m/s".into(),
            pressure: "mca".into(),
            slope: "m/m".into(),
            diameter: "mm".into(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Meta {
    pub cli_version: String,
    pub engine_version: String,
    pub envelope_schema: String,
    pub exit_code: i32,
    pub deterministic: bool,
    pub duration_ms: u64,
    pub host: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Artifact {
    pub kind: String,
    pub path: String,
    pub bytes: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Pagination {
    pub limit: usize,
    pub returned: usize,
    pub total: usize,
    pub has_more: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Envelope {
    pub schema: String,
    pub ok: bool,
    pub command: String,
    pub request_id: String,
    pub project: Option<ProjectRef>,
    pub data: Option<serde_json::Value>,
    pub artifacts: Vec<Artifact>,
    pub diagnostics: Vec<Diagnostic>,
    pub warnings: Vec<Diagnostic>,
    pub explain: Option<serde_json::Value>,
    pub pagination: Option<Pagination>,
    pub units: UnitsCtx,
    pub meta: Meta,
    pub error: Option<Diagnostic>,
}

impl Envelope {
    pub fn ok(command: &str) -> Envelope {
        Envelope {
            schema: ENVELOPE_SCHEMA.into(),
            ok: true,
            command: command.into(),
            request_id: format!("req_{:08x}", fnv1a(command.as_bytes())),
            project: None,
            data: None,
            artifacts: Vec::new(),
            diagnostics: Vec::new(),
            warnings: Vec::new(),
            explain: None,
            pagination: None,
            units: UnitsCtx::default(),
            meta: Meta {
                cli_version: ENGINE_VERSION.into(),
                engine_version: ENGINE_VERSION.into(),
                envelope_schema: ENVELOPE_SCHEMA.into(),
                exit_code: 0,
                deterministic: false,
                duration_ms: 0,
                host: std::env::consts::OS.into(),
            },
            error: None,
        }
    }

    /// Falla con uno o más diagnósticos; el primero de severidad `error` encabeza `error`.
    pub fn fail(command: &str, diagnostics: Vec<Diagnostic>) -> Envelope {
        let mut env = Envelope::ok(command);
        env.ok = false;
        env.warnings = diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Warning)
            .cloned()
            .collect();
        env.error = diagnostics
            .iter()
            .find(|d| d.is_error())
            .cloned()
            .or_else(|| diagnostics.first().cloned());
        env.meta.exit_code = env.error.as_ref().map(exit_for).unwrap_or(exit::ERROR);
        env.diagnostics = diagnostics;
        env
    }

    pub fn with_project(mut self, p: &Project, path: &str) -> Envelope {
        self.project = Some(ProjectRef {
            id: p.id.clone(),
            path: path.into(),
            name: p.name.clone(),
            crs: p.crs.clone(),
            units: "si".into(),
            revision: p.revision,
        });
        self
    }

    pub fn with_data(mut self, v: serde_json::Value) -> Envelope {
        self.data = Some(v);
        self
    }

    /// Añade diagnósticos informativos (p. ej. el reporte de validación) sin fallar.
    pub fn with_diagnostics(mut self, d: Vec<Diagnostic>) -> Envelope {
        self.warnings = d
            .iter()
            .filter(|x| x.severity == Severity::Warning)
            .cloned()
            .collect();
        self.diagnostics = d;
        if self.diagnostics.iter().any(|x| x.is_error()) {
            self.ok = false;
            self.meta.exit_code = exit_for(self.diagnostics.iter().find(|x| x.is_error()).unwrap());
            self.error = self.diagnostics.iter().find(|x| x.is_error()).cloned();
        }
        self
    }

    pub fn with_explain(mut self, v: serde_json::Value) -> Envelope {
        self.explain = Some(v);
        self
    }

    pub fn with_artifact(mut self, kind: &str, path: &str) -> Envelope {
        let bytes = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
        self.artifacts.push(Artifact {
            kind: kind.into(),
            path: path.into(),
            bytes,
        });
        self
    }

    pub fn within(mut self, ms: u64) -> Envelope {
        self.meta.duration_ms = ms;
        self
    }

    pub fn exit_code(&self) -> i32 {
        self.meta.exit_code
    }

    /// Serializa respetando el orden canónico de claves (el de la estructura).
    pub fn to_json(&self, pretty: bool) -> String {
        if pretty {
            serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".into())
        } else {
            serde_json::to_string(self).unwrap_or_else(|_| "{}".into())
        }
    }
}

fn fnv1a(bytes: &[u8]) -> u32 {
    let mut h: u32 = 0x811c_9dc5;
    for b in bytes {
        h ^= *b as u32;
        h = h.wrapping_mul(0x0100_0193);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_contrato_de_ok_y_exit_code_se_cumple() {
        let ok = Envelope::ok("proyecto.info");
        assert!(ok.ok);
        assert_eq!(ok.exit_code(), 0);
        assert_eq!(
            serde_json::to_value(&ok).unwrap()["schema"],
            ENVELOPE_SCHEMA
        );
    }

    #[test]
    fn un_error_produce_exit_no_cero_y_carga_error() {
        let d = Diagnostic::new("E_ARG_MISSING", Severity::Error, "falta --name");
        let env = Envelope::fail("proyecto.crear", vec![d]);
        assert!(!env.ok);
        assert_eq!(env.exit_code(), exit::ARG);
        assert!(env.error.is_some());
    }

    #[test]
    fn todas_las_claves_estan_presentes() {
        let v = serde_json::to_value(Envelope::ok("x")).unwrap();
        for k in [
            "schema",
            "ok",
            "command",
            "request_id",
            "project",
            "data",
            "artifacts",
            "diagnostics",
            "warnings",
            "explain",
            "pagination",
            "units",
            "meta",
            "error",
        ] {
            assert!(v.get(k).is_some(), "falta la clave {k}");
        }
    }
}
