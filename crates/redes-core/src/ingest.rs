//! Ingesta: convertir lo que exportan otras aplicaciones a un proyecto de Redes.
//!
//! Es la mitad "entrada" del producto. Aquí **no se calcula nada**: se lee, se mapea y se
//! valida estructura. Tres caminos:
//!
//! 1. **Plan JSON** (`redes.plan/1`): el contrato canónico. La *skill* de ingesta convierte
//!    cualquier fuente (Civil 3D, SWMM, EPANET, Revit, Excel) a este JSON.
//! 2. **DXF**: lo que el delineante exporta del CAD. Se leen puntos, líneas y polilíneas 3D y se
//!    mapean por capa.
//! 3. **GeoJSON**: puntos y líneas con propiedades.
//!
//! Todos devuelven un [`ImportReport`] con lo leído y lo descartado, con motivo.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::families::Value;
use crate::geom::{P2, P3};
use crate::model::{Dato, ElementId, EstadoObjeto, Fuente, Link, Node, Project};
use crate::terrain::{SurfacePoint, Terrain};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ImportReport {
    pub formato: String,
    pub nodos: usize,
    pub arcos: usize,
    pub puntos_terreno: usize,
    pub descartados: Vec<String>,
}

// ---------------------------------------------------------------------------------------------
// Plan JSON (contrato canónico para la skill de ingesta)

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Plan {
    #[serde(default = "schema_plan")]
    pub schema: String,
    pub proyecto: PlanProyecto,
    #[serde(default)]
    pub terreno: Vec<[f64; 3]>,
    #[serde(default)]
    pub objetos: Vec<PlanObjeto>,
}

fn schema_plan() -> String {
    "redes.plan/1".into()
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PlanProyecto {
    pub nombre: String,
    #[serde(default = "crs_default")]
    pub crs: String,
    #[serde(default)]
    pub fuente_app: String,
    #[serde(default)]
    pub fuente_archivo: String,
}

fn crs_default() -> String {
    "EPSG:32717".into()
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "tipo", rename_all = "snake_case")]
pub enum PlanObjeto {
    Nodo {
        #[serde(default)]
        codigo: String,
        familia: String,
        #[serde(default)]
        subtipo: String,
        e: f64,
        n: f64,
        #[serde(default)]
        cota: Option<f64>,
        #[serde(default)]
        cota_terreno: Option<f64>,
        #[serde(default)]
        diametro: Option<f64>,
        #[serde(default)]
        attrs: BTreeMap<String, Value>,
        #[serde(default)]
        datos: BTreeMap<String, PlanDato>,
    },
    Arco {
        #[serde(default)]
        codigo: String,
        familia: String,
        #[serde(default)]
        subtipo: String,
        desde: String,
        hasta: String,
        #[serde(default)]
        diametro_mm: Option<u32>,
        #[serde(default)]
        vertices: Vec<[f64; 3]>,
        #[serde(default)]
        attrs: BTreeMap<String, Value>,
        #[serde(default)]
        datos: BTreeMap<String, PlanDato>,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlanDato {
    pub valor: f64,
    #[serde(default)]
    pub unidad: String,
    #[serde(default)]
    pub fuente: String,
}

/// Importa un plan JSON y lo aplica al proyecto.
pub fn import_plan(p: &mut Project, path: &Path) -> Result<ImportReport, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("E_IO_NOT_FOUND: {e}"))?;
    let plan: Plan = serde_json::from_str(&text).map_err(|e| format!("E_FMT_PARSE: {e}"))?;
    if !plan.schema.starts_with("redes.plan/") {
        return Err(format!("E_FMT_UNSUPPORTED: esquema {}", plan.schema));
    }
    let mut rep = ImportReport {
        formato: "plan".into(),
        ..Default::default()
    };

    if !plan.terreno.is_empty() {
        let pts = plan
            .terreno
            .iter()
            .map(|v| SurfacePoint::new(v[0], v[1], v[2]))
            .collect::<Vec<_>>();
        rep.puntos_terreno = pts.len();
        p.terrain = Some(Terrain::build(&plan.proyecto.nombre, 32717, pts));
    }

    // Índice de códigos para resolver `desde`/`hasta`.
    let mut codes: BTreeMap<String, ElementId> = BTreeMap::new();
    for o in &plan.objetos {
        if let PlanObjeto::Nodo {
            codigo,
            familia,
            subtipo,
            e,
            n,
            cota,
            cota_terreno,
            diametro,
            attrs,
            datos,
        } = o
        {
            let zt = cota_terreno
                .or(*cota)
                .or_else(|| p.terrain.as_ref().and_then(|t| t.z_at(P2::new(*e, *n))))
                .unwrap_or(0.0);
            let zc = cota.unwrap_or(zt - 1.5);
            let mut a = attrs.clone();
            if let Some(d) = diametro {
                a.insert("diametro_interior".to_string(), Value::Num(*d));
            }
            let code = if codigo.is_empty() {
                p.next_code(familia)
            } else {
                codigo.clone()
            };
            let node = Node {
                id: ElementId::new(0, 0),
                family: familia.clone(),
                ty: if subtipo.is_empty() {
                    default_subtype(familia)
                } else {
                    subtipo.clone()
                },
                code: code.clone(),
                pos: P2::new(*e, *n),
                z_terrain: zt,
                z_rim: zt,
                z_invert: zc,
                attrs: a,
                datos: to_datos(datos),
                estado: EstadoObjeto::Pendiente,
                nota: None,
                deleted: false,
            };
            match p.add_node(node) {
                Ok(id) => {
                    codes.insert(code, id);
                    rep.nodos += 1;
                }
                Err(e) => rep.descartados.push(format!("nodo {codigo}: {e}")),
            }
        }
    }

    for o in &plan.objetos {
        if let PlanObjeto::Arco {
            codigo,
            familia,
            subtipo,
            desde,
            hasta,
            diametro_mm,
            vertices,
            attrs,
            datos,
        } = o
        {
            let (Some(&from), Some(&to)) = (codes.get(desde), codes.get(hasta)) else {
                rep.descartados.push(format!(
                    "arco {codigo}: extremo inexistente ({desde} → {hasta})"
                ));
                continue;
            };
            let (a, b) = (p.node(from).unwrap(), p.node(to).unwrap());
            let path = if vertices.len() >= 2 {
                vertices.iter().map(|v| P3::new(v[0], v[1], v[2])).collect()
            } else {
                vec![
                    P3::new(a.pos.x, a.pos.y, a.z_invert),
                    P3::new(b.pos.x, b.pos.y, b.z_invert),
                ]
            };
            let code = if codigo.is_empty() {
                p.next_code(familia)
            } else {
                codigo.clone()
            };
            let link = Link {
                id: ElementId::new(0, 0),
                family: familia.clone(),
                ty: if subtipo.is_empty() {
                    default_subtype(familia)
                } else {
                    subtipo.clone()
                },
                code,
                from,
                to,
                path,
                d_mm: diametro_mm.unwrap_or(200),
                attrs: attrs.clone(),
                datos: to_datos(datos),
                estado: EstadoObjeto::Pendiente,
                nota: None,
                deleted: false,
            };
            match p.add_link(link) {
                Ok(_) => rep.arcos += 1,
                Err(e) => rep.descartados.push(format!("arco {codigo}: {e}")),
            }
        }
    }

    p.fuentes.push(Fuente {
        app: if plan.proyecto.fuente_app.is_empty() {
            "plan".into()
        } else {
            plan.proyecto.fuente_app.clone()
        },
        archivo: if plan.proyecto.fuente_archivo.is_empty() {
            path.display().to_string()
        } else {
            plan.proyecto.fuente_archivo.clone()
        },
        formato: "plan".into(),
        importado_en: today(),
    });
    if !plan.proyecto.nombre.is_empty() {
        p.name = plan.proyecto.nombre.clone();
    }
    if !plan.proyecto.crs.is_empty() {
        p.crs = plan.proyecto.crs.clone();
    }
    p.commit("importar plan");
    Ok(rep)
}

fn to_datos(m: &BTreeMap<String, PlanDato>) -> BTreeMap<String, Dato> {
    m.iter()
        .map(|(k, v)| {
            (
                k.clone(),
                Dato {
                    valor: v.valor,
                    unidad: v.unidad.clone(),
                    fuente: v.fuente.clone(),
                },
            )
        })
        .collect()
}

/// Subtipo por defecto de una familia: el primero del registro.
///
/// Antes era una tabla paralela que se quedó desfasada: 21 de 29 familias recibían un subtipo
/// inexistente y el objeto se perdía en la importación.
fn default_subtype(family: &str) -> String {
    crate::families::family(family)
        .and_then(|f| f.types.first())
        .map(|t| t.key.to_string())
        .unwrap_or_else(|| "inspeccion".to_string())
}

// ---------------------------------------------------------------------------------------------
// DXF

/// Importa un DXF ASCII: POINT, LINE y POLYLINE/LWPOLYLINE de 3D.
///
/// Mapeo por capa (heurístico y configurable):
/// `POZO|MH|PZ` → pozo, `AGUA|AP|DIST` → agua, `ALC|SAN|COLECTOR|CL` → alcantarillado.
pub fn import_dxf(p: &mut Project, path: &Path, tolerance: f64) -> Result<ImportReport, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("E_IO_NOT_FOUND: {e}"))?;
    let pairs = dxf_pairs(&text);
    if pairs.is_empty() {
        return Err("E_FMT_PARSE: el archivo no parece un DXF ASCII".into());
    }
    let mut rep = ImportReport {
        formato: "dxf".into(),
        ..Default::default()
    };
    let mut points: Vec<(P2, f64, String)> = Vec::new();
    let mut circles: Vec<(P2, f64)> = Vec::new();
    let mut lines: Vec<(Vec<P3>, String, String)> = Vec::new();
    let mut i = 0usize;

    while i + 1 < pairs.len() {
        let (code, val) = (&pairs[i].0, &pairs[i].1);
        if code == "0"
            && (val == "POINT"
                || val == "LINE"
                || val == "POLYLINE"
                || val == "LWPOLYLINE"
                || val == "CIRCLE")
        {
            let kind = val.clone();
            i += 1;
            let mut layer = String::from("0");
            let mut xs: Vec<f64> = Vec::new();
            let mut ys: Vec<f64> = Vec::new();
            let mut zs: Vec<f64> = Vec::new();
            let mut elev = 0.0;
            let mut is_3d = false;
            while i + 1 < pairs.len() && pairs[i].0 != "0" {
                let (c, v) = (&pairs[i].0, &pairs[i].1);
                match c.as_str() {
                    "8" => layer = v.clone(),
                    "70" => is_3d = v.parse::<i32>().map(|f| f & 8 != 0).unwrap_or(false),
                    "38" => elev = v.parse().unwrap_or(0.0),
                    "10" => xs.push(v.parse().unwrap_or(f64::NAN)),
                    "20" => ys.push(v.parse().unwrap_or(f64::NAN)),
                    "30" => zs.push(v.parse().unwrap_or(0.0)),
                    "11" => xs.push(v.parse().unwrap_or(f64::NAN)),
                    "21" => ys.push(v.parse().unwrap_or(f64::NAN)),
                    "31" => zs.push(v.parse().unwrap_or(0.0)),
                    _ => {}
                }
                i += 1;
            }
            match kind.as_str() {
                "CIRCLE" => {
                    if let (Some(x), Some(y)) = (xs.first(), ys.first()) {
                        circles.push((P2::new(*x, *y), zs.first().copied().unwrap_or(elev)));
                    }
                }
                "POINT" => {
                    if let (Some(x), Some(y)) = (xs.first(), ys.first()) {
                        points.push((P2::new(*x, *y), zs.first().copied().unwrap_or(elev), layer));
                    }
                }
                "LINE" => {
                    if xs.len() >= 2 && ys.len() >= 2 {
                        let a = P3::new(xs[0], ys[0], zs.first().copied().unwrap_or(elev));
                        let b = P3::new(xs[1], ys[1], zs.get(1).copied().unwrap_or(elev));
                        lines.push((vec![a, b], layer, "LINE".into()));
                    }
                }
                _ => {
                    // POLYLINE: los vértices vienen en entidades VERTEX posteriores.
                    if kind == "POLYLINE" {
                        let mut verts = Vec::new();
                        let mut j = i;
                        while j + 1 < pairs.len() {
                            if pairs[j].0 == "0" && pairs[j].1 == "VERTEX" {
                                let (mut vx, mut vy, mut vz) = (f64::NAN, f64::NAN, 0.0);
                                j += 1;
                                while j + 1 < pairs.len() && pairs[j].0 != "0" {
                                    match pairs[j].0.as_str() {
                                        "10" => vx = pairs[j].1.parse().unwrap_or(f64::NAN),
                                        "20" => vy = pairs[j].1.parse().unwrap_or(f64::NAN),
                                        "30" => vz = pairs[j].1.parse().unwrap_or(0.0),
                                        _ => {}
                                    }
                                    j += 1;
                                }
                                if !vx.is_nan() && !vy.is_nan() {
                                    verts.push(P3::new(vx, vy, vz));
                                }
                            } else if pairs[j].0 == "0" && pairs[j].1 == "SEQEND" {
                                j += 2;
                                break;
                            } else {
                                j += 1;
                            }
                        }
                        i = j;
                        if verts.len() >= 2 {
                            lines.push((
                                verts,
                                layer,
                                if is_3d { "POLYLINE3D" } else { "POLYLINE" }.into(),
                            ));
                        }
                    } else {
                        // LWPOLYLINE: x/y repetidos y una elevación.
                        let mut verts = Vec::new();
                        for k in 0..xs.len().min(ys.len()) {
                            verts.push(P3::new(xs[k], ys[k], elev));
                        }
                        if verts.len() >= 2 {
                            lines.push((verts, layer, "LWPOLYLINE".into()));
                        }
                    }
                }
            }
            continue;
        }
        i += 1;
    }

    // Geometría no finita (un `10` con texto acaba en NaN): se descarta entera antes de usarla.
    let finito2 = |c: &P2| c.x.is_finite() && c.y.is_finite();
    let finito3 = |q: &P3| q.x.is_finite() && q.y.is_finite() && q.z.is_finite();
    circles.retain(|(c, z)| finito2(c) && z.is_finite());
    points.retain(|(c, z, _)| finito2(c) && z.is_finite());
    lines.retain(|(v, _, _)| v.iter().all(finito3));

    // Nodos desde POINT.
    let mut node_ids: Vec<(ElementId, P2)> = Vec::new();
    for (pos, z, layer) in &points {
        let fam = family_for_layer(layer, true);
        // La cota de tapa viene del CIRCLE que el CAD dibuja sobre el punto (si existe).
        let rim = circles
            .iter()
            .filter(|(c, _)| c.dist(*pos) <= tolerance.max(0.5) * 4.0)
            .map(|(_, z)| *z)
            .fold(f64::NEG_INFINITY, f64::max);
        let zt = p
            .terrain
            .as_ref()
            .and_then(|t| t.z_at(*pos))
            .unwrap_or(if rim.is_finite() { rim } else { *z });
        let node = Node {
            id: ElementId::new(0, 0),
            family: fam.to_string(),
            ty: default_subtype(fam),
            code: String::new(),
            pos: *pos,
            z_terrain: zt,
            z_rim: if rim.is_finite() { rim } else { zt },
            z_invert: *z,
            attrs: BTreeMap::new(),
            datos: BTreeMap::new(),
            estado: EstadoObjeto::Pendiente,
            nota: None,
            deleted: false,
        };
        match p.add_node(node) {
            Ok(id) => {
                node_ids.push((id, *pos));
                rep.nodos += 1;
            }
            Err(e) => rep
                .descartados
                .push(format!("punto ({:.2},{:.2}): {e}", pos.x, pos.y)),
        }
    }

    // Arcos desde polilíneas, con snapping a nodos existentes.
    for (verts, layer, _kind) in &lines {
        let fam = family_for_layer(layer, false);
        let first = verts[0].p2();
        let last = verts[verts.len() - 1].p2();
        // Las líneas verticales de tapa (misma planta, distinta cota) no son arcos: se ignoran.
        if first.dist(last) < 1e-6 {
            continue;
        }
        let from = nearest_node(p, &mut node_ids, first, tolerance, &mut rep);
        let to = nearest_node(p, &mut node_ids, last, tolerance, &mut rep);
        let (Some(from), Some(to)) = (from, to) else {
            continue;
        };
        if from == to {
            rep.descartados
                .push(format!("polilínea en capa {layer}: extremos coincidentes"));
            continue;
        }
        let link = Link {
            id: ElementId::new(0, 0),
            family: fam.to_string(),
            ty: default_subtype(fam),
            code: String::new(),
            from,
            to,
            path: verts.clone(),
            d_mm: 200,
            attrs: BTreeMap::new(),
            datos: BTreeMap::new(),
            estado: EstadoObjeto::Pendiente,
            nota: None,
            deleted: false,
        };
        match p.add_link(link) {
            Ok(_) => rep.arcos += 1,
            Err(e) => rep
                .descartados
                .push(format!("polilínea en capa {layer}: {e}")),
        }
    }

    p.fuentes.push(Fuente {
        app: "CAD (DXF)".into(),
        archivo: path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default(),
        formato: "dxf".into(),
        importado_en: today(),
    });
    p.commit("importar dxf");
    Ok(rep)
}

fn nearest_node(
    p: &mut Project,
    ids: &mut Vec<(ElementId, P2)>,
    pos: P2,
    tol: f64,
    rep: &mut ImportReport,
) -> Option<ElementId> {
    if let Some((id, _)) = ids.iter().find(|(_, q)| q.dist(pos) <= tol) {
        return Some(*id);
    }
    let zt = p.terrain.as_ref().and_then(|t| t.z_at(pos)).unwrap_or(0.0);
    let node = Node {
        id: ElementId::new(0, 0),
        family: "saneamiento.pozo".into(),
        ty: "inspeccion".into(),
        code: String::new(),
        pos,
        z_terrain: zt,
        z_rim: zt,
        z_invert: zt - 1.5,
        attrs: BTreeMap::new(),
        datos: BTreeMap::new(),
        estado: EstadoObjeto::Pendiente,
        nota: None,
        deleted: false,
    };
    match p.add_node(node) {
        Ok(id) => {
            ids.push((id, pos));
            rep.nodos += 1;
            Some(id)
        }
        Err(e) => {
            rep.descartados.push(format!("nodo implícito: {e}"));
            None
        }
    }
}

fn family_for_layer(layer: &str, point: bool) -> &'static str {
    let l = layer.to_ascii_uppercase();
    if l.contains("POZO") || l.contains("MH") || l.contains("PZ") || l.contains("CAMARA") {
        return "saneamiento.pozo";
    }
    if l.contains("AGUA") || l.contains("AP") || l.contains("DIST") || l.contains("HIDR") {
        return "agua.tuberia";
    }
    if l.contains("VERT") || l.contains("DESC") || l.contains("EMIS") {
        return "saneamiento.descarga";
    }
    if point {
        "saneamiento.pozo"
    } else {
        "saneamiento.tuberia"
    }
}

fn dxf_pairs(text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut lines = text.lines();
    while let (Some(c), Some(v)) = (lines.next(), lines.next()) {
        out.push((c.trim().to_string(), v.trim().to_string()));
    }
    out
}

// ---------------------------------------------------------------------------------------------
// GeoJSON

/// Importa puntos y líneas de un GeoJSON. Las propiedades `familia`/`family` y `codigo` mandan.
pub fn import_geojson(p: &mut Project, path: &Path) -> Result<ImportReport, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("E_IO_NOT_FOUND: {e}"))?;
    let v: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("E_FMT_PARSE: {e}"))?;
    let mut rep = ImportReport {
        formato: "geojson".into(),
        ..Default::default()
    };
    let features = v
        .get("features")
        .and_then(|f| f.as_array())
        .cloned()
        .unwrap_or_default();
    let mut codes: BTreeMap<String, ElementId> = BTreeMap::new();

    for f in &features {
        let Some(g) = f.get("geometry") else { continue };
        let ty = g.get("type").and_then(|t| t.as_str()).unwrap_or("");
        let props = f
            .get("properties")
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        let fam = props
            .get("familia")
            .or_else(|| props.get("family"))
            .and_then(|x| x.as_str())
            .unwrap_or(if ty == "Point" {
                "saneamiento.pozo"
            } else {
                "saneamiento.tuberia"
            })
            .to_string();
        let codigo = props
            .get("codigo")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        if ty == "Point" {
            let c = g
                .get("coordinates")
                .and_then(|c| c.as_array())
                .cloned()
                .unwrap_or_default();
            let (Some(e), Some(n)) = (
                c.first().and_then(|x| x.as_f64()),
                c.get(1).and_then(|x| x.as_f64()),
            ) else {
                continue;
            };
            let z = c.get(2).and_then(|x| x.as_f64()).unwrap_or(0.0);
            let zt = p
                .terrain
                .as_ref()
                .and_then(|t| t.z_at(P2::new(e, n)))
                .unwrap_or(z);
            let node = Node {
                id: ElementId::new(0, 0),
                family: fam.clone(),
                ty: default_subtype(&fam),
                code: codigo.clone(),
                pos: P2::new(e, n),
                z_terrain: zt,
                z_rim: zt,
                z_invert: z,
                attrs: BTreeMap::new(),
                datos: BTreeMap::new(),
                estado: EstadoObjeto::Pendiente,
                nota: None,
                deleted: false,
            };
            match p.add_node(node) {
                Ok(id) => {
                    codes.insert(
                        if codigo.is_empty() {
                            format!("{id}")
                        } else {
                            codigo
                        },
                        id,
                    );
                    rep.nodos += 1;
                }
                Err(e) => rep.descartados.push(e),
            }
        } else if ty == "LineString" {
            let coords = g
                .get("coordinates")
                .and_then(|c| c.as_array())
                .cloned()
                .unwrap_or_default();
            let path: Vec<P3> = coords
                .iter()
                .filter_map(|c| {
                    let a = c.as_array()?;
                    Some(P3::new(
                        a.first()?.as_f64()?,
                        a.get(1)?.as_f64()?,
                        a.get(2).and_then(|z| z.as_f64()).unwrap_or(0.0),
                    ))
                })
                .collect();
            if path.len() < 2 {
                continue;
            }
            let from_code = props.get("desde").and_then(|x| x.as_str()).unwrap_or("");
            let to_code = props.get("hasta").and_then(|x| x.as_str()).unwrap_or("");
            let from = codes.get(from_code).copied().or_else(|| {
                let pos = path[0].p2();
                p.nodes
                    .iter()
                    .find(|n| !n.deleted && n.pos.dist(pos) < 1.0)
                    .map(|n| n.id)
            });
            let to = codes.get(to_code).copied().or_else(|| {
                let pos = path[path.len() - 1].p2();
                p.nodes
                    .iter()
                    .find(|n| !n.deleted && n.pos.dist(pos) < 1.0)
                    .map(|n| n.id)
            });
            let (Some(from), Some(to)) = (from, to) else {
                rep.descartados
                    .push("línea sin nodos en sus extremos".into());
                continue;
            };
            let link = Link {
                id: ElementId::new(0, 0),
                family: fam.clone(),
                ty: default_subtype(&fam),
                code: codigo,
                from,
                to,
                path,
                d_mm: props
                    .get("diametro_mm")
                    .and_then(|x| x.as_u64())
                    .unwrap_or(200) as u32,
                attrs: BTreeMap::new(),
                datos: BTreeMap::new(),
                estado: EstadoObjeto::Pendiente,
                nota: None,
                deleted: false,
            };
            match p.add_link(link) {
                Ok(_) => rep.arcos += 1,
                Err(e) => rep.descartados.push(e),
            }
        }
    }
    p.commit("importar geojson");
    Ok(rep)
}

/// Importa puntos de terreno `E,N,Z` desde CSV o texto.
pub fn import_terreno(p: &mut Project, path: &Path, epsg: u32) -> Result<ImportReport, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("E_IO_NOT_FOUND: {e}"))?;
    let mut pts = Vec::new();
    let mut rep = ImportReport {
        formato: "csv".into(),
        ..Default::default()
    };
    for (i, line) in text.lines().enumerate() {
        let l = line.trim();
        if l.is_empty() || l.starts_with('#') {
            continue;
        }
        let parts: Vec<&str> = l
            .split([',', ';', '\t', ' '])
            .filter(|s| !s.is_empty())
            .collect();
        if parts.len() < 3 {
            rep.descartados
                .push(format!("línea {}: menos de 3 columnas", i + 1));
            continue;
        }
        match (
            parts[0].parse::<f64>(),
            parts[1].parse::<f64>(),
            parts[2].parse::<f64>(),
        ) {
            // `"NaN".parse::<f64>()` funciona en Rust: sin este filtro entra un NaN que luego se
            // serializa como `null` y deja el proyecto imposible de abrir.
            (Ok(e), Ok(n), Ok(z)) if e.is_finite() && n.is_finite() && z.is_finite() => {
                pts.push(SurfacePoint::new(e, n, z))
            }
            _ => {
                if i == 0 {
                    continue;
                }
                rep.descartados
                    .push(format!("línea {}: no numérico o no finito", i + 1));
            }
        }
    }
    if pts.len() < 3 {
        return Err("E_FMT_PARSE: se necesitan al menos 3 puntos válidos E,N,Z".into());
    }
    rep.puntos_terreno = pts.len();
    let name = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "terreno".into());
    p.terrain = Some(Terrain::build(&name, epsg, pts));
    p.commit("importar terreno");
    Ok(rep)
}

fn today() -> String {
    // Fecha de importación sin dependencias: días desde 1970 → YYYY-MM-DD (UTC).
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let (mut days, rem) = (secs / 86_400, secs % 86_400);
    let _ = rem;
    let mut year = 1970i64;
    loop {
        let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
        let ylen = if leap { 366 } else { 365 };
        if days < ylen {
            break;
        }
        days -= ylen;
        year += 1;
    }
    let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    let months = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut m = 0;
    while m < 12 && days >= months[m] {
        days -= months[m];
        m += 1;
    }
    format!("{year:04}-{:02}-{:02}", m + 1, days + 1)
}

#[cfg(test)]
mod tests {
    /// El subtipo por defecto debe existir en su familia (ADV-03).
    #[test]
    fn el_subtipo_por_defecto_existe_en_todas_las_familias() {
        for f in crate::families::families() {
            let ty = default_subtype(f.key);
            assert!(
                f.types.iter().any(|t| t.key == ty),
                "{}/{}: subtipo por defecto inválido",
                f.key,
                ty
            );
        }
    }

    use super::*;

    #[test]
    fn importa_un_plan_minimo() {
        let dir = std::env::temp_dir().join("redes_plan_test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("p.json");
        let plan = serde_json::json!({
            "schema": "redes.plan/1",
            "proyecto": { "nombre": "Prueba", "crs": "EPSG:32717", "fuente_app": "SWMM" },
            "terreno": [[760000.0, 9796000.0, 2500.0], [760100.0, 9796000.0, 2499.0], [760000.0, 9796100.0, 2501.0]],
            "objetos": [
                { "tipo": "nodo", "codigo": "P1", "familia": "saneamiento.pozo", "e": 760000.0, "n": 9796000.0, "cota": 2498.5 },
                { "tipo": "nodo", "codigo": "P2", "familia": "saneamiento.pozo", "e": 760050.0, "n": 9796000.0, "cota": 2498.2 },
                { "tipo": "arco", "codigo": "C1", "familia": "saneamiento.tuberia", "desde": "P1", "hasta": "P2", "diametro_mm": 200,
                  "datos": { "caudal": { "valor": 12.5, "unidad": "L/s", "fuente": "SWMM" } } }
            ]
        });
        std::fs::write(&path, serde_json::to_string(&plan).unwrap()).unwrap();
        let mut p = Project::new("x", "EPSG:32717");
        let rep = import_plan(&mut p, &path).unwrap();
        assert_eq!(rep.nodos, 2);
        assert_eq!(rep.arcos, 1);
        assert_eq!(rep.puntos_terreno, 3);
        assert!(p.links[0].datos.contains_key("caudal"));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn la_fecha_tiene_forma_iso() {
        let d = today();
        assert_eq!(d.len(), 10);
        assert_eq!(&d[4..5], "-");
    }
}
