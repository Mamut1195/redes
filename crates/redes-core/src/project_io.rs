//! Lectura y escritura del archivo de proyecto.
//!
//! v0.1 usa **JSON canónico** en un único archivo `*.redes`:
//! - legible y diffeable (un proyecto es texto, se revisa en un PR),
//! - inspeccionable por un agente sin herramientas,
//! - sin dependencias C.
//!
//! SQLite entra cuando el tamaño lo exija (>50 MB o guardado incremental). El `schema` está
//! versionado; antes de v1 solo se abre la versión vigente (D10) y las migraciones empiezan al
//! congelar el formato.

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
///
/// Antes de v1 solo se abre esta versión exacta (D10): no hay archivos en producción que
/// conservar y un `.redes` se regenera reimportando la entrega. Las migraciones
/// (`migrar_vN→vN+1`) empiezan cuando el formato se congele en v1.
///
/// - 3: `observaciones` con historial (F13-03).
pub const SCHEMA_VERSION: u32 = 3;

/// Solo el campo `schema`: se mira antes de deserializar el resto, para que un archivo de otra
/// versión dé `E_PROJ_SCHEMA` y no un `E_FMT_PARSE` confuso sobre un campo que cambió.
#[derive(serde::Deserialize)]
struct Cabecera {
    #[serde(default)]
    schema: Option<serde_json::Value>,
}

/// Carga un proyecto y lo valida antes de devolverlo.
///
/// Un archivo de disco no es de fiar: puede venir de otra versión, de una edición manual o
/// de un proceso interrumpido. Aquí se rechaza lo irrecuperable con un código estable en vez de
/// reventar más adelante (revisión adversarial D-03, D-04, D-07, D-08).
pub fn load(path: &Path) -> Result<Project, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("E_IO_NOT_FOUND: {e}"))?;
    let cabecera: Cabecera =
        serde_json::from_str(&text).map_err(|e| format!("E_FMT_PARSE: {e}"))?;
    let schema = match cabecera.schema {
        Some(serde_json::Value::String(s)) => s,
        Some(otro) => otro.to_string(),
        None => String::new(),
    };
    validar_esquema(&schema)?;
    let mut p: Project = serde_json::from_str(&text).map_err(|e| format!("E_FMT_PARSE: {e}"))?;
    validar_tin(&p)?;
    validar_identidad(&mut p)?;
    // El índice espacial del TIN no se serializa: se reconstruye al abrir.
    if let Some(t) = p.terrain.as_mut() {
        t.reindexar();
    }
    // Tampoco los de búsqueda del proyecto (F14-01).
    p.reindexar();
    Ok(p)
}

/// Acepta **solo** `redes.project/{SCHEMA_VERSION}` (D10). Una versión anterior, futura o
/// desconocida se rechaza con `E_PROJ_SCHEMA`: antes de v1 no hay migraciones.
pub fn validar_esquema(schema: &str) -> Result<(), String> {
    let vigente = format!("redes.project/{SCHEMA_VERSION}");
    if schema == vigente {
        return Ok(());
    }
    let trae = if schema.is_empty() {
        "no declara versión (falta `schema`)".to_string()
    } else {
        format!("es `{schema}`")
    };
    Err(format!(
        "E_PROJ_SCHEMA: el archivo {trae} y esta app solo abre `{vigente}`. Antes de v1 no se convierten versiones: regenere el .redes reimportando la entrega (`redes importar plan --archivo plan.json` o `redes importar dxf --archivo entrega.dxf` sobre un proyecto nuevo de `redes proyecto nuevo`)"
    ))
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
/// objeto inalcanzable y bloquea `proyecto aprobar` para siempre. Los ids son únicos entre todos
/// los objetos, borrados incluidos; los códigos, solo entre los vivos: un código dado de baja se
/// puede volver a usar (las búsquedas por código ya ignoran los borrados).
fn validar_identidad(p: &mut Project) -> Result<(), String> {
    use std::collections::HashSet;

    let mut ids: HashSet<(u32, u32)> = HashSet::new();
    let mut codigos: HashSet<&str> = HashSet::new();
    let mut max_slot = 0u32;

    for (que, id, code, borrado) in p
        .nodes
        .iter()
        .map(|n| ("nodo", n.id, n.code.as_str(), n.deleted))
        .chain(
            p.links
                .iter()
                .map(|l| ("arco", l.id, l.code.as_str(), l.deleted)),
        )
    {
        max_slot = max_slot.max(id.slot);
        if !ids.insert((id.slot, id.generation)) {
            return Err(format!(
                "E_STATE_DUPLICATE_ID: el {que} {code} repite el identificador {}/{}",
                id.slot, id.generation
            ));
        }
        if !borrado && !codigos.insert(code) {
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

    /// Carga el catálogo con otro `schema` y devuelve el error.
    fn error_con_esquema(nombre: &str, schema: Option<serde_json::Value>) -> String {
        let mut v = serde_json::to_value(crate::demo_catalog::catalogo()).unwrap();
        match schema {
            Some(s) => v["schema"] = s,
            None => {
                v.as_object_mut().unwrap().remove("schema");
            }
        }
        let ruta = escribir_temporal(nombre, &v);
        let err = crate::project_io::load(&ruta).unwrap_err();
        let _ = std::fs::remove_file(&ruta);
        err
    }

    #[test]
    fn rechaza_un_esquema_futuro() {
        let err = error_con_esquema("futuro", Some(serde_json::json!("redes.project/99")));
        assert!(err.starts_with("E_PROJ_SCHEMA:"), "{err}");
        assert!(
            err.contains("redes.project/99"),
            "dice qué versión trae: {err}"
        );
        assert!(err.contains("redes.project/3"), "dice cuál entiende: {err}");
    }

    /// D10: antes de v1 no hay migraciones; un `/2` no abre y el mensaje dice cómo regenerarlo.
    #[test]
    fn rechaza_un_esquema_anterior_sin_migrar() {
        for (nombre, schema) in [
            ("v2", serde_json::json!("redes.project/2")),
            ("v1", serde_json::json!("redes.project/1")),
            ("ajeno", serde_json::json!("otra.cosa/3")),
            ("numero", serde_json::json!(3)),
        ] {
            let err = error_con_esquema(nombre, Some(schema));
            assert!(err.starts_with("E_PROJ_SCHEMA:"), "{nombre}: {err}");
            assert!(err.contains("redes importar"), "sugiere reimportar: {err}");
        }
        let err = error_con_esquema("sin", None);
        assert!(err.starts_with("E_PROJ_SCHEMA:"), "sin schema: {err}");
    }

    /// Un `/2` cuyo contenido ya no deserializa como `Project` da `E_PROJ_SCHEMA`, no
    /// `E_FMT_PARSE`: la versión se mira antes que los campos.
    #[test]
    fn la_version_se_comprueba_antes_que_los_campos() {
        let ruta = escribir_temporal(
            "v2-roto",
            &serde_json::json!({"schema": "redes.project/2", "nodes": "no es una lista"}),
        );
        let err = crate::project_io::load(&ruta).unwrap_err();
        let _ = std::fs::remove_file(&ruta);
        assert!(err.starts_with("E_PROJ_SCHEMA:"), "{err}");
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

    /// Borrar `PZ-001`, dar de alta otro `PZ-001` y guardar: el archivo vuelve a abrir y la
    /// búsqueda por código devuelve el vivo.
    #[test]
    fn un_codigo_dado_de_baja_se_puede_reutilizar() {
        let mut p = crate::demo_catalog::catalogo();
        let viejo = p
            .nodes
            .iter()
            .find(|n| !n.deleted && p.tramos_de(n.id).next().is_none())
            .cloned()
            .expect("un nodo sin tramos");
        p.remove(viejo.id).expect("baja");
        let mut nuevo = viejo.clone();
        nuevo.pos.x += 5.0;
        let id_nuevo = p.add_node(nuevo).expect("re-alta con el mismo código");
        assert_ne!(id_nuevo, viejo.id);
        let ruta = std::env::temp_dir().join(format!("redes-realta-{}.redes", std::process::id()));
        crate::project_io::save(&p, &ruta).expect("guarda");
        let q = crate::project_io::load(&ruta).expect("abre con el código reutilizado");
        let _ = std::fs::remove_file(&ruta);
        assert_eq!(q.node_by_code(&viejo.code).map(|n| n.id), Some(id_nuevo));
        assert!(q.node(viejo.id).is_none(), "el borrado sigue borrado");
    }

    /// Los ids sí son únicos entre todos los objetos, también los borrados.
    #[test]
    fn rechaza_un_id_repetido_aunque_este_borrado() {
        let mut v = serde_json::to_value(crate::demo_catalog::catalogo()).unwrap();
        let id = v["nodes"][0]["id"].clone();
        v["nodes"][1]["id"] = id;
        v["nodes"][1]["deleted"] = serde_json::json!(true);
        let ruta = escribir_temporal("id-borrado", &v);
        let err = crate::project_io::load(&ruta).unwrap_err();
        assert!(err.starts_with("E_STATE_DUPLICATE_ID"), "{err}");
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
            observaciones: Vec::new(),
            modelo: None,
            deleted: false,
            sin_dato: Vec::new(),
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
        assert_eq!(q.schema, format!("redes.project/{SCHEMA_VERSION}"));
        let _ = std::fs::remove_file(&path);
    }
}
