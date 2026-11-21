//! Lectura y escritura del archivo de proyecto.
//!
//! v0.1 usa **JSON canónico** en un único archivo `*.redes`:
//! - legible y diffeable (un proyecto es texto, se revisa en un PR),
//! - inspeccionable por un agente sin herramientas,
//! - sin dependencias C.
//!
//! SQLite entra cuando el tamaño lo exija (>50 MB o guardado incremental); el contrato de
//! `schema` ya está versionado para migrar sin romper archivos.

use std::io::Write;
use std::path::{Path, PathBuf};

use crate::model::Project;

pub const EXT: &str = "redes";

/// Guarda de forma atómica: escribe un temporal y lo renombra.
///
/// Nota: el motor no llama a esto desde el bucle de frames de la UI; el autoguardado corre en
/// un trabajador (regla "cero E/S en el frame loop").
pub fn save(p: &Project, path: &Path) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        if !dir.as_os_str().is_empty() {
            std::fs::create_dir_all(dir).map_err(|e| format!("E_IO_PERMISSION: {e}"))?;
        }
    }
    let json = serde_json::to_string_pretty(p).map_err(|e| format!("E_FMT_PARSE: {e}"))?;
    let tmp = path.with_extension("redes.tmp");
    {
        let mut f = std::fs::File::create(&tmp).map_err(|e| format!("E_IO_PERMISSION: {e}"))?;
        f.write_all(json.as_bytes()).map_err(|e| format!("E_IO_PERMISSION: {e}"))?;
        f.sync_all().map_err(|e| format!("E_IO_PERMISSION: {e}"))?;
    }
    std::fs::rename(&tmp, path).map_err(|e| format!("E_IO_PERMISSION: {e}"))?;
    Ok(())
}

/// Carga un proyecto y valida el esquema.
pub fn load(path: &Path) -> Result<Project, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("E_IO_NOT_FOUND: {e}"))?;
    let p: Project = serde_json::from_str(&text).map_err(|e| format!("E_FMT_PARSE: {e}"))?;
    if !p.schema.starts_with("redes.project/") {
        return Err(format!("E_PROJ_SCHEMA_TOO_NEW: esquema desconocido {}", p.schema));
    }
    // El índice espacial del TIN no se serializa: se reconstruye al abrir.
    let mut p = p;
    if let Some(t) = p.terrain.as_mut() {
        t.reindexar();
    }
    Ok(p)
}

/// Busca un `*.redes` subiendo por el árbol de directorios.
pub fn find_upwards(start: &Path) -> Option<PathBuf> {
    let mut dir = Some(start);
    while let Some(d) = dir {
        if let Ok(rd) = std::fs::read_dir(d) {
            let mut hits: Vec<PathBuf> = rd
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.extension().map(|x| x == EXT).unwrap_or(false))
                .collect();
            hits.sort();
            if let Some(first) = hits.into_iter().next() {
                return Some(first);
            }
        }
        dir = d.parent();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guardar_y_abrir_conserva_el_documento() {
        let dir = std::env::temp_dir().join("redes_io_test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("p.redes");
        let mut p = Project::new("La Esperanza", "EPSG:32717");
        p.name = "La Esperanza 2".into();
        p.commit("prueba");
        save(&p, &path).unwrap();
        let q = load(&path).unwrap();
        assert_eq!(q.name, p.name);
        assert_eq!(q.name, "La Esperanza 2");
        assert_eq!(q.revision, 1);
        assert_eq!(q.schema, "redes.project/2");
        let _ = std::fs::remove_file(&path);
    }
}
