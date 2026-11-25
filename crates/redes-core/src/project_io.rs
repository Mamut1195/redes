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
    if let Some(dir) = path.parent()
        && !dir.as_os_str().is_empty()
    {
        std::fs::create_dir_all(dir).map_err(|e| format!("E_IO_PERMISSION: {e}"))?;
    }
    let json = serde_json::to_string_pretty(p).map_err(|e| format!("E_FMT_PARSE: {e}"))?;
    let tmp = path.with_extension("redes.tmp");
    {
        let mut f = std::fs::File::create(&tmp).map_err(|e| format!("E_IO_PERMISSION: {e}"))?;
        f.write_all(json.as_bytes())
            .map_err(|e| format!("E_IO_PERMISSION: {e}"))?;
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
        fin(n.z_invert, "cota de solera", &n.code)?;
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

/// Versión de esquema que entiende esta compilación.
pub const SCHEMA_VERSION: u32 = 2;

/// Carga un proyecto y lo valida antes de devolverlo.
///
/// Un archivo de disco no es de fiar: puede venir de una versión futura, de una edición manual o
/// de un proceso interrumpido. Aquí se rechaza lo irrecuperable con un código estable en vez de
/// reventar más adelante (revisión adversarial D-03, D-04, D-07, D-08).
pub fn load(path: &Path) -> Result<Project, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("E_IO_NOT_FOUND: {e}"))?;
    let mut p: Project = serde_json::from_str(&text).map_err(|e| format!("E_FMT_PARSE: {e}"))?;
    validar_esquema(&p.schema)?;
    validar_tin(&p)?;
    validar_identidad(&mut p)?;
    p.migrar_cotas();
    // El índice espacial del TIN no se serializa: se reconstruye al abrir.
    if let Some(t) = p.terrain.as_mut() {
        t.reindexar();
    }
    Ok(p)
}

/// Acepta versiones hasta [`SCHEMA_VERSION`]; una futura se rechaza en vez de perder datos.
pub fn validar_esquema(schema: &str) -> Result<(), String> {
    let version = schema
        .strip_prefix("redes.project/")
        .and_then(|v| v.parse::<u32>().ok())
        .ok_or_else(|| format!("E_PROJ_SCHEMA: esquema desconocido {schema}"))?;
    if version > SCHEMA_VERSION {
        return Err(format!(
            "E_PROJ_SCHEMA_TOO_NEW: el archivo usa la versión {version} y esta app entiende hasta la {SCHEMA_VERSION}; ábralo con una versión más nueva"
        ));
    }
    Ok(())
}

/// Comprueba que la triangulación del terreno no tenga índices fuera de rango.
fn validar_tin(p: &Project) -> Result<(), String> {
    let Some(t) = &p.terrain else { return Ok(()) };
    if t.points.is_empty() {
        return Err("E_TIN_INVALIDO: el terreno no tiene puntos".into());
    }
    if t.tris.len() > 4_000_000 {
        return Err(format!(
            "E_TIN_INVALIDO: {} triángulos, demasiados",
            t.tris.len()
        ));
    }
    let n = t.points.len();
    for (i, tri) in t.tris.iter().enumerate() {
        for k in tri {
            if (*k as usize) >= n {
                return Err(format!(
                    "E_TIN_INVALIDO: el triángulo {i} referencia el vértice {k} y solo hay {n}"
                ));
            }
        }
        if tri[0] == tri[1] || tri[1] == tri[2] || tri[0] == tri[2] {
            return Err(format!("E_TIN_INVALIDO: el triángulo {i} repite vértice"));
        }
    }
    Ok(())
}

/// Repara `next_slot` y rechaza identificadores o códigos duplicados.
///
/// Un `ElementId` repetido hace que se apruebe el objeto equivocado; un código repetido deja un
/// objeto inalcanzable y bloquea `proyecto aprobar` para siempre.
fn validar_identidad(p: &mut Project) -> Result<(), String> {
    use std::collections::HashSet;

    let mut ids: HashSet<(u32, u32)> = HashSet::new();
    let mut codigos: HashSet<String> = HashSet::new();
    let mut max_slot = 0u32;

    for (que, n) in p
        .nodes
        .iter()
        .map(|n| ("nodo", (n.id, n.code.clone())))
        .chain(p.links.iter().map(|l| ("arco", (l.id, l.code.clone()))))
    {
        let (id, code) = n;
        max_slot = max_slot.max(id.slot);
        if !ids.insert((id.slot, id.generation)) {
            return Err(format!(
                "E_STATE_DUPLICATE_ID: el {que} {code} repite el identificador {}/{}",
                id.slot, id.generation
            ));
        }
        if !codigos.insert(code.clone()) {
            return Err(format!("E_STATE_DUPLICATE_NAME: código duplicado {code}"));
        }
    }

    // `next_slot` no es de fiar: se recalcula para que los objetos nuevos nunca colisionen.
    let esperado = max_slot.saturating_add(1);
    if p.next_slot <= max_slot {
        p.next_slot = esperado;
    }
    Ok(())
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

    fn escribir_temporal(nombre: &str, valor: &serde_json::Value) -> std::path::PathBuf {
        let ruta =
            std::env::temp_dir().join(format!("redes-{nombre}-{}.redes", std::process::id()));
        std::fs::write(&ruta, serde_json::to_string(valor).unwrap()).unwrap();
        ruta
    }

    /// El modelo asignado y la ruta de la biblioteca sobreviven a guardar y abrir.
    #[test]
    fn conserva_el_modelo_y_la_biblioteca() {
        let mut p = crate::demo_catalog::catalogo();
        p.biblioteca = Some("biblioteca.json".into());
        p.nodes[0].modelo = Some("pozo-tipo".into());
        p.nodes[0].datos.insert(
            "volumen_m3".to_string(),
            crate::model::Dato {
                valor: 2.5,
                unidad: "m3".into(),
                fuente: "modelo:pozo-tipo".into(),
            },
        );
        let ruta = std::env::temp_dir().join(format!("redes-modelo-{}.redes", std::process::id()));
        crate::project_io::save(&p, &ruta).expect("guarda");
        let q = crate::project_io::load(&ruta).expect("carga");
        assert_eq!(q.biblioteca.as_deref(), Some("biblioteca.json"));
        assert_eq!(q.nodes[0].modelo.as_deref(), Some("pozo-tipo"));
        let d = q.nodes[0].datos.get("volumen_m3").expect("dato");
        assert!((d.valor - 2.5).abs() < 1e-9);
        assert_eq!(d.fuente, "modelo:pozo-tipo");
        let _ = std::fs::remove_file(&ruta);
    }

    #[test]
    fn rechaza_un_esquema_futuro() {
        let mut v = serde_json::to_value(crate::demo_catalog::catalogo()).unwrap();
        v["schema"] = serde_json::json!("redes.project/99");
        let ruta = escribir_temporal("futuro", &v);
        let err = crate::project_io::load(&ruta).unwrap_err();
        assert!(err.starts_with("E_PROJ_SCHEMA_TOO_NEW"), "{err}");
        let _ = std::fs::remove_file(&ruta);
    }

    #[test]
    fn conserva_los_campos_desconocidos() {
        let mut v = serde_json::to_value(crate::demo_catalog::catalogo()).unwrap();
        v["cota_rasante_municipal"] = serde_json::json!(2500.75);
        let ruta = escribir_temporal("extra", &v);
        let p = crate::project_io::load(&ruta).expect("carga");
        crate::project_io::save(&p, &ruta).expect("guarda");
        let texto = std::fs::read_to_string(&ruta).unwrap();
        assert!(
            texto.contains("cota_rasante_municipal"),
            "el campo desconocido se perdió al guardar"
        );
        let _ = std::fs::remove_file(&ruta);
    }

    #[test]
    fn rechaza_un_tin_con_indice_fuera_de_rango() {
        let mut v = serde_json::to_value(crate::demo_city::ciudad()).unwrap();
        v["terrain"]["tris"] = serde_json::json!([[0, 1, 999999]]);
        let ruta = escribir_temporal("tin", &v);
        let err = crate::project_io::load(&ruta).unwrap_err();
        assert!(err.starts_with("E_TIN_INVALIDO"), "{err}");
        let _ = std::fs::remove_file(&ruta);
    }

    #[test]
    fn rechaza_codigos_duplicados() {
        let mut v = serde_json::to_value(crate::demo_catalog::catalogo()).unwrap();
        let primero = v["nodes"][0]["code"].clone();
        v["nodes"][1]["code"] = primero;
        let ruta = escribir_temporal("codigos", &v);
        let err = crate::project_io::load(&ruta).unwrap_err();
        assert!(err.starts_with("E_STATE_DUPLICATE_NAME"), "{err}");
        let _ = std::fs::remove_file(&ruta);
    }

    #[test]
    fn repara_next_slot_al_cargar() {
        let mut v = serde_json::to_value(crate::demo_catalog::catalogo()).unwrap();
        v["next_slot"] = serde_json::json!(0);
        let ruta = escribir_temporal("slot", &v);
        let mut p = crate::project_io::load(&ruta).expect("carga");
        // Todos los objetos tienen id; el siguiente debe ser distinto de cualquiera.
        let n = crate::model::Node {
            id: crate::model::ElementId::new(0, 0),
            family: "saneamiento.pozo".into(),
            ty: "inspeccion".into(),
            code: String::new(),
            pos: crate::geom::P2::new(0.0, 0.0),
            z_terrain: 0.0,
            z_rim: 0.0,
            z_invert: -1.0,
            attrs: Default::default(),
            datos: Default::default(),
            estado: crate::model::EstadoObjeto::Pendiente,
            nota: None,
            modelo: None,
            deleted: false,
        };
        let id = p.add_node(n).expect("añade");
        assert!(
            !p.nodes
                .iter()
                .filter(|x| x.id != id)
                .any(|x| x.id.slot == id.slot),
            "el id nuevo colisiona"
        );
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
