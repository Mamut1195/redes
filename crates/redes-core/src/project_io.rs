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
    // Blindaje: `serde_json` escribe `null` para NaN/infinito y el archivo deja de poder abrirse.
    // Se rechaza antes de tocar el disco.
    validar_finito(p)?;
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

/// Comprueba que no haya NaN ni infinitos en cotas, posiciones ni números de atributos/datos.
///
/// `serde_json` serializa esos valores como `null`, y un proyecto con `null` en un `f64` ya no se
/// puede volver a abrir. Es preferible rechazar el guardado a corromper el archivo.
pub fn validar_finito(p: &Project) -> Result<(), String> {
    fn fin(v: f64, que: &str, code: &str) -> Result<(), String> {
        if v.is_finite() {
            Ok(())
        } else {
            Err(format!("E_VALOR_NO_FINITO: {que} en {code} ({v})"))
        }
    }
    let valor = |v: &crate::families::Value, que: &str, code: &str| -> Result<(), String> {
        match v {
            crate::families::Value::Num(x) => fin(*x, que, code),
            _ => Ok(()),
        }
    };

    for n in p.nodes.iter().filter(|n| !n.deleted) {
        fin(n.pos.x, "E", &n.code)?;
        fin(n.pos.y, "N", &n.code)?;
        fin(n.z_terrain, "cota de terreno", &n.code)?;
        fin(n.z_rim, "cota de tapa", &n.code)?;
        fin(n.z_invert, "cota de clave", &n.code)?;
        for (k, v) in &n.attrs {
            valor(v, k, &n.code)?;
        }
        for (k, d) in &n.datos {
            fin(d.valor, k, &n.code)?;
        }
    }
    for l in p.links.iter().filter(|l| !l.deleted) {
        for q in &l.path {
            fin(q.x, "E del trazado", &l.code)?;
            fin(q.y, "N del trazado", &l.code)?;
            fin(q.z, "cota del trazado", &l.code)?;
        }
        for (k, v) in &l.attrs {
            valor(v, k, &l.code)?;
        }
        for (k, d) in &l.datos {
            fin(d.valor, k, &l.code)?;
        }
    }
    if let Some(t) = &p.terrain {
        for q in t.points.iter().take(1_000_000) {
            fin(q.e, "E del terreno", &t.name)?;
            fin(q.n, "N del terreno", &t.name)?;
            fin(q.z, "cota del terreno", &t.name)?;
        }
    }
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
    /// Un NaN no puede llegar al disco: `serde_json` lo escribe como `null` y el proyecto deja de
    /// abrirse. `save` debe rechazarlo y no tocar el archivo.
    #[test]
    fn no_guarda_valores_no_finitos() {
        let mut p = crate::demo_catalog::catalogo();
        let ruta = std::env::temp_dir().join(format!("redes-nan-{}.redes", std::process::id()));
        let _ = std::fs::remove_file(&ruta);
        crate::project_io::save(&p, &ruta).expect("guardado inicial");
        let antes = std::fs::read(&ruta).expect("leer");
        // Se inyecta un NaN como el que dejaría un DXF/CSV hostil.
        p.nodes[0].z_rim = f64::NAN;
        let err = crate::project_io::save(&p, &ruta).expect_err("debe rechazar el NaN");
        assert!(err.starts_with("E_VALOR_NO_FINITO"), "{err}");
        let despues = std::fs::read(&ruta).expect("leer otra vez");
        assert_eq!(antes, despues, "el archivo no debe cambiar");
        let _ = std::fs::remove_file(&ruta);
    }

    /// Un CSV con NaN en la cota se descarta en la ingesta y no envenena el proyecto.
    #[test]
    fn el_terreno_descarta_lineas_no_finitas() {
        let mut p = crate::model::Project::new("nan", "EPSG:32717");
        let csv = "E,N,Z\n758000,9797000,2500\nNaN,9797001,2501\n758002,9797002,2502\n758010,9797010,2505\n";
        let ruta = std::env::temp_dir().join(format!("redes-nan-{}.csv", std::process::id()));
        std::fs::write(&ruta, csv).expect("csv");
        let rep = crate::ingest::import_terreno(&mut p, &ruta, 32717).expect("importa");
        assert_eq!(p.terrain.as_ref().map(|t| t.points.len()).unwrap_or(0), 3);
        assert!(!rep.descartados.is_empty(), "debe informar del descarte");
        assert!(crate::project_io::validar_finito(&p).is_ok());
        let _ = std::fs::remove_file(&ruta);
    }

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
