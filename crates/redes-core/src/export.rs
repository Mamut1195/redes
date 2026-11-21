//! Salida: DXF para el detallado en CAD y GeoJSON para SIG.
//!
//! El visor no dibuja planos: aprueba el modelo y lo entrega. El DXF sale con una capa por
//! familia, polilíneas 3D para los arcos y puntos con cota para los nodos, de modo que el
//! delineante pueda montar el detalle en AutoCAD/Civil 3D sin rehacer la geometría.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

use crate::families::family;
use crate::model::{EstadoObjeto, Project};

/// Exporta el modelo a DXF ASCII (R12 cómodo para cualquier CAD).
pub fn export_dxf(p: &Project, path: &Path) -> Result<usize, String> {
    export_dxf_filtered(p, path, false)
}

/// Igual que [export_dxf], pero puede entregar solo lo aprobado (--solo-aprobados).
pub fn export_dxf_filtered(
    p: &Project,
    path: &Path,
    solo_aprobados: bool,
) -> Result<usize, String> {
    let mut s = String::with_capacity(64 * 1024);
    // Cabecera mínima.
    s.push_str("0\nSECTION\n2\nHEADER\n0\nENDSEC\n");
    s.push_str("0\nSECTION\n2\nTABLES\n0\nTABLE\n2\nLAYER\n70\n0\n0\nENDTAB\n0\nENDSEC\n");
    // Bloques: cada familia con su bloque, para que en CAD haya objetos y no geometría suelta.
    let familias: std::collections::BTreeSet<String> = p
        .nodes
        .iter()
        .filter(|n| !n.deleted)
        .map(|n| layer_name(&n.family))
        .collect();
    s.push_str("0\nSECTION\n2\nBLOCKS\n");
    for nombre in &familias {
        write!(
            s,
            "0\nBLOCK\n8\n{nombre}\n2\n{nombre}\n70\n0\n10\n0.0\n20\n0.0\n30\n0.0\n3\n{nombre}\n"
        )
        .ok();
        // Unidad: círculo de radio 1 y eje vertical de altura 1 (la escala la pone el INSERT).
        write!(
            s,
            "0\nCIRCLE\n8\n{nombre}\n10\n0.0\n20\n0.0\n30\n0.0\n40\n1.0\n\
0\nLINE\n8\n{nombre}\n10\n0.0\n20\n0.0\n30\n0.0\n11\n0.0\n21\n0.0\n31\n1.0\n"
        )
        .ok();
        for (etiqueta, alto) in [("CODIGO", 0.0), ("SUBTIPO", 0.0)] {
            write!(
                s,
                "0\nATTDEF\n8\n{nombre}\n3\n{etiqueta}\n2\n{etiqueta}\n70\n0\n10\n1.2\n20\n{alto}\n30\n0.0\n40\n0.25\n1\n\n"
            )
            .ok();
        }
        s.push_str("0\nENDBLK\n8\n0\n");
    }
    s.push_str("0\nENDSEC\n");

    s.push_str("0\nSECTION\n2\nENTITIES\n");

    let mut layers: BTreeMap<String, String> = BTreeMap::new();
    let mut n = 0usize;

    for node in p.nodes.iter().filter(|x| {
        !x.deleted && (!solo_aprobados || x.estado == crate::model::EstadoObjeto::Aprobado)
    }) {
        let layer = layer_name(&node.family);
        layers.insert(layer.clone(), layer.clone());
        // Bloque con atributos: el objeto del CAD, con código y subtipo legibles.
        write!(
            s,
            "0\nINSERT\n8\n{}\n2\n{}\n10\n{:.4}\n20\n{:.4}\n30\n{:.4}\n41\n{:.3}\n42\n{:.3}\n43\n{:.3}\n66\n1\n",
            layer,
            layer,
            node.pos.x,
            node.pos.y,
            node.z_rim,
            node.diametro(),
            node.diametro(),
            node.altura().max(0.1)
        )
        .ok();
        for (etiqueta, valor) in [("CODIGO", &node.code), ("SUBTIPO", &node.ty)] {
            write!(
                s,
                "0\nATTRIB\n8\n{}\n10\n{:.4}\n20\n{:.4}\n30\n{:.4}\n40\n0.25\n1\n{}\n2\n{}\n70\n0\n",
                layer,
                node.pos.x + 1.2,
                node.pos.y,
                node.z_rim,
                valor,
                etiqueta
            )
            .ok();
        }
        write!(s, "0\nSEQEND\n8\n{}\n", layer).ok();

        // Punto con cota.
        write!(
            s,
            "0\nPOINT\n8\n{}\n10\n{:.4}\n20\n{:.4}\n30\n{:.4}\n",
            layer, node.pos.x, node.pos.y, node.z_invert
        )
        .ok();
        // Círculo del pozo (diámetro interior) para que se vea en planta.
        let r = node.diametro() / 2.0;
        write!(
            s,
            "0\nCIRCLE\n8\n{}\n10\n{:.4}\n20\n{:.4}\n30\n{:.4}\n40\n{:.4}\n",
            layer, node.pos.x, node.pos.y, node.z_rim, r
        )
        .ok();
        // Altura del pozo como línea vertical hasta la tapa.
        write!(
            s,
            "0\nLINE\n8\n{}_TAPA\n10\n{:.4}\n20\n{:.4}\n30\n{:.4}\n11\n{:.4}\n21\n{:.4}\n31\n{:.4}\n",
            layer, node.pos.x, node.pos.y, node.z_invert, node.pos.x, node.pos.y, node.z_rim
        )
        .ok();
        n += 1;
    }

    for link in p.links.iter().filter(|x| {
        !x.deleted && (!solo_aprobados || x.estado == crate::model::EstadoObjeto::Aprobado)
    }) {
        let layer = layer_name(&link.family);
        layers.insert(layer.clone(), layer.clone());
        if link.path.len() < 2 {
            continue;
        }
        // POLYLINE 3D con flag 8.
        write!(s, "0\nPOLYLINE\n8\n{}\n66\n1\n70\n8\n", layer).ok();
        for v in &link.path {
            write!(
                s,
                "0\nVERTEX\n8\n{}\n10\n{:.4}\n20\n{:.4}\n30\n{:.4}\n",
                layer, v.x, v.y, v.z
            )
            .ok();
        }
        s.push_str("0\nSEQEND\n8\n");
        s.push_str(&layer);
        s.push('\n');
        n += 1;
    }

    // Capa de datos: un TEXT por objeto con la información que el CAD no guarda en la geometría
    // (familia, subtipo, código, atributos y datos). Es lo que permite que la ida y vuelta no
    // pierda nada; el usuario puede apagar esta capa y seguir viendo el dibujo.
    layers.insert("REDES_INFO".to_string(), "REDES_INFO".to_string());
    for node in p.nodes.iter().filter(|x| {
        !x.deleted && (!solo_aprobados || x.estado == crate::model::EstadoObjeto::Aprobado)
    }) {
        let attrs: Vec<String> = node
            .attrs
            .iter()
            .map(|(k, v)| format!("{k}={}", valor_a_texto(v)))
            .collect();
        let datos: Vec<String> = node
            .datos
            .iter()
            .map(|(k, d)| format!("{k}={};{};{}", d.valor, d.unidad, d.fuente))
            .collect();
        let carga = format!(
            "REDES|1|N|{}|{}|{}|{}|{}|{}|{}|{}|{}",
            node.family,
            node.ty,
            node.code,
            node.estado.key(),
            node.pos.x,
            node.pos.y,
            node.z_rim,
            node.z_invert,
            node.z_terrain
        );
        write!(
            s,
            "0\nTEXT\n8\nREDES_INFO\n10\n{:.4}\n20\n{:.4}\n30\n{:.4}\n1\n{}\n40\n0.0\n",
            node.pos.x, node.pos.y, node.z_rim, carga
        )
        .ok();
        if !attrs.is_empty() {
            write!(
                s,
                "0\nTEXT\n8\nREDES_INFO\n10\n{:.4}\n20\n{:.4}\n30\n{:.4}\n1\nATTRS|{}|{}\n40\n0.0\n",
                node.pos.x, node.pos.y, node.z_rim, node.code, attrs.join(";")
            )
            .ok();
        }
        if !datos.is_empty() {
            write!(
                s,
                "0\nTEXT\n8\nREDES_INFO\n10\n{:.4}\n20\n{:.4}\n30\n{:.4}\n1\nDATOS|{}|{}\n40\n0.0\n",
                node.pos.x, node.pos.y, node.z_rim, node.code, datos.join(";")
            )
            .ok();
        }
    }
    for link in p.links.iter().filter(|x| {
        !x.deleted && (!solo_aprobados || x.estado == crate::model::EstadoObjeto::Aprobado)
    }) {
        if link.path.len() < 2 {
            continue;
        }
        let medio = &link.path[link.path.len() / 2];
        let trazado: Vec<String> = link
            .path
            .iter()
            .map(|v| format!("{:.4},{:.4},{:.4}", v.x, v.y, v.z))
            .collect();
        let carga = format!(
            "REDES|1|L|{}|{}|{}|{}|{}|{}",
            link.family,
            link.ty,
            link.code,
            link.estado.key(),
            link.d_mm,
            trazado.join(";")
        );
        write!(
            s,
            "0\nTEXT\n8\nREDES_INFO\n10\n{:.4}\n20\n{:.4}\n30\n{:.4}\n1\n{}\n40\n0.0\n",
            medio.x, medio.y, medio.z, carga
        )
        .ok();
        let attrs: Vec<String> = link
            .attrs
            .iter()
            .map(|(k, v)| format!("{k}={}", valor_a_texto(v)))
            .collect();
        if !attrs.is_empty() {
            write!(
                s,
                "0\nTEXT\n8\nREDES_INFO\n10\n{:.4}\n20\n{:.4}\n30\n{:.4}\n1\nATTRS|{}|{}\n40\n0.0\n",
                medio.x, medio.y, medio.z, link.code, attrs.join(";")
            )
            .ok();
        }
    }

    s.push_str("0\nENDSEC\n0\nEOF\n");
    if let Some(dir) = path.parent()
        && !dir.as_os_str().is_empty()
    {
        std::fs::create_dir_all(dir).map_err(|e| format!("E_IO_PERMISSION: {e}"))?;
    }
    std::fs::write(path, s).map_err(|e| format!("E_IO_PERMISSION: {e}"))?;
    Ok(n)
}

fn layer_name(family_key: &str) -> String {
    family(family_key)
        .map(|f| {
            format!(
                "{}_{}",
                f.code_prefix,
                f.label.replace(' ', "_").to_uppercase()
            )
        })
        .unwrap_or_else(|| "REDES".into())
}

/// Valor de atributo como texto estable para el DXF (`numo`, `int`, `bool`, `texto`).
fn valor_a_texto(v: &crate::families::Value) -> String {
    match v {
        crate::families::Value::Num(n) => format!("{n}"),
        crate::families::Value::Int(i) => format!("{i}"),
        crate::families::Value::Bool(b) => format!("{b}"),
        crate::families::Value::Text(s) => s.replace(';', ",").replace('|', "/"),
    }
}

/// Exporta a GeoJSON (puntos y líneas con sus datos importados).
pub fn export_geojson(p: &Project, path: &Path) -> Result<usize, String> {
    let mut features = Vec::new();
    for n in p.nodes.iter().filter(|x| !x.deleted) {
        let mut props = serde_json::json!({
            "codigo": n.code,
            "familia": n.family,
            "subtipo": n.ty,
            "cota_terreno": n.z_terrain,
            "cota_clave": n.z_invert,
            "estado": n.estado.key(),
            "nota": n.nota,
        });
        for (k, v) in &n.attrs {
            props[k] = serde_json::json!(valor_a_texto(v));
        }
        for (k, d) in &n.datos {
            props[k] =
                serde_json::json!({ "valor": d.valor, "unidad": d.unidad, "fuente": d.fuente });
        }
        features.push(serde_json::json!({
            "type": "Feature",
            "geometry": { "type": "Point", "coordinates": [n.pos.x, n.pos.y, n.z_invert] },
            "properties": props,
        }));
    }
    for l in p.links.iter().filter(|x| !x.deleted) {
        if l.path.len() < 2 {
            continue;
        }
        let mut props = serde_json::json!({
            "codigo": l.code,
            "familia": l.family,
            "subtipo": l.ty,
            "diametro_mm": l.d_mm,
            "longitud_m": l.length_2d(),
            "estado": l.estado.key(),
            "nota": l.nota,
        });
        for (k, v) in &l.attrs {
            props[k] = serde_json::json!(valor_a_texto(v));
        }
        for (k, d) in &l.datos {
            props[k] =
                serde_json::json!({ "valor": d.valor, "unidad": d.unidad, "fuente": d.fuente });
        }
        features.push(serde_json::json!({
            "type": "Feature",
            "geometry": { "type": "LineString", "coordinates": l.path.iter().map(|q| [q.x, q.y, q.z]).collect::<Vec<_>>() },
            "properties": props,
        }));
    }
    let fc = serde_json::json!({
        "type": "FeatureCollection",
        "crs": { "type": "name", "properties": { "name": p.crs } },
        "features": features,
    });
    if let Some(dir) = path.parent()
        && !dir.as_os_str().is_empty()
    {
        std::fs::create_dir_all(dir).map_err(|e| format!("E_IO_PERMISSION: {e}"))?;
    }
    std::fs::write(path, serde_json::to_string_pretty(&fc).unwrap_or_default())
        .map_err(|e| format!("E_IO_PERMISSION: {e}"))?;
    Ok(features.len())
}

/// Resumen de revisión para la barra de estado y los informes.
pub fn revision_summary(p: &Project) -> (usize, usize, usize) {
    let (mut pend, mut ok, mut obs) = (0, 0, 0);
    for e in p
        .nodes
        .iter()
        .filter(|n| !n.deleted)
        .map(|n| n.estado)
        .chain(p.links.iter().filter(|l| !l.deleted).map(|l| l.estado))
    {
        match e {
            EstadoObjeto::Pendiente => pend += 1,
            EstadoObjeto::Aprobado => ok += 1,
            EstadoObjeto::Observado => obs += 1,
        }
    }
    (pend, ok, obs)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// La ida y vuelta por DXF no pierde familia, subtipo, código ni atributos (F8-01).
    #[test]
    fn el_dxf_vuelve_con_los_mismos_objetos() {
        let original = crate::demo_catalog::catalogo_completo();
        let ruta = std::env::temp_dir().join(format!("redes-rt-{}.dxf", std::process::id()));
        let exportados = export_dxf(&original, &ruta).expect("exporta");
        assert!(exportados > 0);

        let mut destino = crate::model::Project::new("vuelta", "EPSG:32717");
        crate::ingest::import_dxf(&mut destino, &ruta, 0.5).expect("importa");

        assert_eq!(
            destino.nodes.len(),
            original.nodes.len(),
            "nodos: {} vs {}",
            destino.nodes.len(),
            original.nodes.len()
        );
        assert_eq!(destino.links.len(), original.links.len(), "arcos");
        for n in &original.nodes {
            let otro = destino
                .nodes
                .iter()
                .find(|x| x.code == n.code)
                .unwrap_or_else(|| panic!("falta {}", n.code));
            assert_eq!(otro.family, n.family, "familia de {}", n.code);
            assert_eq!(otro.ty, n.ty, "subtipo de {}", n.code);
            assert_eq!(
                otro.attrs.len(),
                n.attrs.len(),
                "atributos de {}: {:?} vs {:?}",
                n.code,
                otro.attrs.keys().collect::<Vec<_>>(),
                n.attrs.keys().collect::<Vec<_>>()
            );
            assert!((otro.pos.x - n.pos.x).abs() < 1e-3, "x de {}", n.code);
            assert!((otro.z_rim - n.z_rim).abs() < 1e-3, "cota de {}", n.code);
        }
        let _ = std::fs::remove_file(&ruta);
    }

    #[test]
    fn el_dxf_sale_con_entidades() {
        let p = crate::demo::project();
        let dir = std::env::temp_dir().join("redes_dxf_test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("p.dxf");
        let n = export_dxf(&p, &path).unwrap();
        assert!(n > 10);
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("POLYLINE"));
        assert!(text.ends_with("EOF\n"));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn el_geojson_tiene_features() {
        let p = crate::demo::project();
        let dir = std::env::temp_dir().join("redes_geo_test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("p.geojson");
        let n = export_geojson(&p, &path).unwrap();
        assert_eq!(n, p.count());
        let _ = std::fs::remove_file(&path);
    }
}
