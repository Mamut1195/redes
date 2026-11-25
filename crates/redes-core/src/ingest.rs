//! Ingesta: lo que entra a un proyecto de Redes (D5).
//!
//! Es la mitad "entrada" del producto. Aquí **no se calcula nada ni se adivina nada**: se lee,
//! se valida estructura y lo que falta queda ausente. Tres caminos y ninguno más:
//!
//! 1. **Plan JSON** (`redes.plan/1`): el contrato canónico. Los formatos ajenos (Civil 3D, SWMM,
//!    EPANET, DXF o GeoJSON de terceros, Excel) los adapta un agente con la skill
//!    `skills/redes-ingesta`, que escribe este JSON.
//! 2. **DXF de Redes**: la vuelta del DXF que exporta Redes (capa `REDES_INFO`, F8).
//! 3. **Terreno**: puntos `E,N,Z`.
//!
//! Todos devuelven un [`ImportReport`] con lo leído, lo descartado y los avisos, con motivo.

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
    /// Objetos descartados de verdad (no se pudieron importar).
    pub descartados: Vec<String>,
    /// Objetos que entraron con otra clave porque la suya estaba retirada (docs/20 §4.2).
    #[serde(default)]
    pub renombrados: Vec<String>,
    /// Objetos que entraron con algún dato ausente (F12-03): se muestran con `—`, no se rellenan.
    #[serde(default)]
    pub avisos: Vec<String>,
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
        /// Cota de solera.
        #[serde(default)]
        cota: Option<f64>,
        #[serde(default)]
        cota_terreno: Option<f64>,
        /// Cota de tapa. Si falta, queda ausente: no se copia el terreno (F12-03).
        #[serde(default)]
        cota_tapa: Option<f64>,
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
        /// Solera (fondo interior) del tramo en su inicio y su fin, si el origen la declara.
        #[serde(default)]
        cota_solera_inicio: Option<f64>,
        #[serde(default)]
        cota_solera_fin: Option<f64>,
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
            cota_tapa,
            diametro,
            attrs,
            datos,
        } = o
        {
            // F12-03: nada se rellena. La cota de terreno puede salir del TIN que trae el mismo
            // plan (es un dato del origen, y se avisa); la solera y la tapa, solo del plan.
            let del_tin = cota_terreno
                .is_none()
                .then(|| p.terrain.as_ref().and_then(|t| t.z_at(P2::new(*e, *n))))
                .flatten();
            let zt = cota_terreno.or(del_tin);
            let (zs, zr) = (*cota, *cota_tapa);
            let Some(dibujo) = zr.or(zt).or(zs) else {
                rep.descartados.push(format!(
                    "nodo {codigo}: sin ninguna cota (solera, tapa ni terreno, y sin TIN en su planta): no se puede ubicar en altura"
                ));
                continue;
            };
            let mut sin_dato = Vec::new();
            for (falta, clave) in [
                (zt.is_none(), crate::model::COTA_TERRENO),
                (zr.is_none(), crate::model::COTA_TAPA),
                (zs.is_none(), crate::model::COTA_SOLERA),
            ] {
                if falta {
                    sin_dato.push(clave.to_string());
                }
            }
            if del_tin.is_some() {
                rep.avisos.push(format!(
                    "nodo {codigo}: cota_terreno tomada del TIN del plan"
                ));
            }
            if !sin_dato.is_empty() {
                let plano = zs.is_none() || (zt.is_none() && zr.is_none());
                rep.avisos.push(format!(
                    "nodo {codigo}: sin {} (se muestra «—»{})",
                    sin_dato.join(", "),
                    if plano {
                        "; se dibuja plano, sin altura"
                    } else {
                        ""
                    }
                ));
            }
            let mut a = attrs.clone();
            if let Some(d) = diametro {
                a.insert("diametro_interior".to_string(), Value::Num(*d));
            }
            let code = if codigo.is_empty() {
                p.next_code(familia)
            } else {
                codigo.clone()
            };
            let mut node = Node {
                id: ElementId::new(0, 0),
                family: familia.clone(),
                ty: if subtipo.is_empty() {
                    default_subtype(familia)
                } else {
                    subtipo.clone()
                },
                code: code.clone(),
                pos: P2::new(*e, *n),
                // Lo ausente guarda solo un valor de dibujo (la cota de arriba que sí hay, o la
                // solera): la pieza queda plana y `sin_dato` impide mostrarlo como dato.
                z_terrain: zt.unwrap_or(dibujo),
                z_rim: zr.unwrap_or(dibujo),
                z_invert: zs.unwrap_or(dibujo),
                attrs: a,
                datos: to_datos(datos),
                estado: EstadoObjeto::Pendiente,
                nota: None,
                modelo: None,
                deleted: false,
                sin_dato,
            };
            // Clave retirada: se traduce a su destino y se anota (nunca en silencio).
            if let Some((nf, nt)) = crate::families_extra::alias(&node.family, &node.ty) {
                rep.renombrados
                    .push(format!("{}:{} -> {nf}:{nt}", node.family, node.ty));
                node.family = nf.to_string();
                node.ty = nt.to_string();
            }
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
            cota_solera_inicio,
            cota_solera_fin,
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
            let mut sin_dato = Vec::new();
            let path = if vertices.len() >= 2 {
                vertices.iter().map(|v| P3::new(v[0], v[1], v[2])).collect()
            } else {
                // La solera declarada del tramo manda sobre la del pozo (F12-02). Si no hay
                // ninguna de las dos, el trazado va a la cota de dibujo del pozo y se marca.
                let z0 = cota_solera_inicio.or(a.cota_solera());
                let z1 = cota_solera_fin.or(b.cota_solera());
                if z0.is_none() || z1.is_none() {
                    sin_dato.push(crate::model::COTA_SOLERA.to_string());
                }
                vec![
                    P3::new(a.pos.x, a.pos.y, z0.unwrap_or(a.z_invert)),
                    P3::new(b.pos.x, b.pos.y, z1.unwrap_or(b.z_invert)),
                ]
            };
            let dn = diametro_mm.filter(|d| *d > 0);
            if dn.is_none() {
                sin_dato.push(crate::model::DIAMETRO_NOMINAL.to_string());
            }
            if !sin_dato.is_empty() {
                rep.avisos.push(format!(
                    "arco {codigo}: sin {} (se muestra «—» y se dibuja como línea sin dato)",
                    sin_dato.join(", ")
                ));
            }
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
                // Sin DN declarado queda 0 («sin dato»), no 200 (F12-03).
                d_mm: dn.unwrap_or(0),
                attrs: attrs.clone(),
                datos: to_datos(datos),
                estado: EstadoObjeto::Pendiente,
                nota: None,
                cota_solera_inicio: *cota_solera_inicio,
                cota_solera_fin: *cota_solera_fin,
                deleted: false,
                sin_dato,
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
// DXF de ida y vuelta (F8)

/// Importa un DXF **exportado por Redes**: lo reconstruye desde su capa `REDES_INFO`.
///
/// Es la vuelta de [`crate::export::export_dxf`]. Un DXF de otra aplicación (Civil 3D, un
/// levantamiento…) no trae esa capa y se rechaza con `E_FMT_NOT_REDES`: lo adapta un agente con
/// la skill `skills/redes-ingesta`, que escribe un `redes.plan/1` (D5). Antes se adivinaba la
/// familia por el nombre de la capa; esa heurística se retiró (F12-09).
///
/// `_tolerance` ya no se usa (era la del enganche de extremos de la heurística); se conserva para
/// no romper a quien llama.
pub fn import_dxf(p: &mut Project, path: &Path, _tolerance: f64) -> Result<ImportReport, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("E_IO_NOT_FOUND: {e}"))?;
    let pairs = dxf_pairs(&text);
    if pairs.is_empty() || !pairs.iter().any(|(c, v)| c == "0" && v == "EOF") {
        return Err("E_FMT_PARSE: el archivo no parece un DXF ASCII completo (falta EOF)".into());
    }
    let textos = textos_redes_info(&pairs);
    if textos.is_empty() {
        return Err(
            "E_FMT_NOT_REDES: el DXF no trae la capa REDES_INFO, así que no lo exportó Redes"
                .into(),
        );
    }
    let mut rep = ImportReport {
        formato: "dxf".into(),
        ..Default::default()
    };
    importar_de_capa_info(p, &textos, &mut rep)?;
    p.fuentes.push(Fuente {
        app: "Redes (DXF)".into(),
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

/// Contenido de los `TEXT` de la capa `REDES_INFO`.
fn textos_redes_info(pairs: &[(String, String)]) -> Vec<String> {
    // Un TEXT es: código 8 (capa), 10/20/30 (posición) y 1 (contenido).
    let mut textos: Vec<String> = Vec::new();
    let mut i = 0;
    while i + 1 < pairs.len() {
        if pairs[i].0 == "0" && pairs[i].1 == "TEXT" {
            let mut j = i + 1;
            let mut capa = String::new();
            let mut contenido = String::new();
            while j + 1 < pairs.len() && pairs[j].0 != "0" {
                match pairs[j].0.as_str() {
                    "8" => capa = pairs[j].1.clone(),
                    "1" => contenido = pairs[j].1.clone(),
                    _ => {}
                }
                j += 1;
            }
            if capa == "REDES_INFO" && !contenido.is_empty() {
                textos.push(contenido);
            }
            i = j;
            continue;
        }
        i += 1;
    }
    textos
}

/// Reconstruye el proyecto desde los textos de `REDES_INFO`.
///
/// Si la capa no trae ni un objeto legible devuelve `E_FMT_NOT_REDES` sin tocar el proyecto.
fn importar_de_capa_info(
    p: &mut Project,
    textos: &[String],
    rep: &mut ImportReport,
) -> Result<(), String> {
    let mut attrs: std::collections::BTreeMap<String, std::collections::BTreeMap<String, Value>> =
        Default::default();
    let mut datos: std::collections::BTreeMap<String, std::collections::BTreeMap<String, Dato>> =
        Default::default();
    for t in textos {
        if let Some(resto) = t.strip_prefix("ATTRS|") {
            let mut it = resto.splitn(2, '|');
            let (Some(code), Some(cuerpo)) = (it.next(), it.next()) else {
                continue;
            };
            let mut m = std::collections::BTreeMap::new();
            for par in cuerpo.split(';').filter(|s| !s.is_empty()) {
                if let Some((k, v)) = par.split_once('=') {
                    m.insert(k.to_string(), parse_valor(v));
                }
            }
            attrs.insert(code.to_string(), m);
        } else if let Some(resto) = t.strip_prefix("DATOS|") {
            let mut it = resto.splitn(2, '|');
            let (Some(code), Some(cuerpo)) = (it.next(), it.next()) else {
                continue;
            };
            let mut m = std::collections::BTreeMap::new();
            for trozo in cuerpo.split(';').filter(|s| !s.is_empty()) {
                let partes: Vec<&str> = trozo.split('=').collect();
                if partes.len() < 4 {
                    continue;
                }
                let valor = partes[1].parse::<f64>().unwrap_or(f64::NAN);
                if !valor.is_finite() {
                    continue;
                }
                m.insert(
                    partes[0].to_string(),
                    Dato {
                        valor,
                        unidad: partes[2].to_string(),
                        fuente: partes[3..].join("="),
                    },
                );
            }
            datos.insert(code.to_string(), m);
        }
    }

    let mut ids: std::collections::BTreeMap<String, ElementId> = Default::default();
    let mut nodos_creados = 0usize;
    for t in textos {
        let Some(resto) = t.strip_prefix("REDES|1|") else {
            continue;
        };
        let campos: Vec<&str> = resto.split('|').collect();
        if campos.first() != Some(&"N") || campos.len() < 10 {
            continue;
        }
        let (familia, subtipo) = (campos[1], campos[2]);
        let code = campos[3].to_string();
        let x = campos[5].parse::<f64>().ok();
        let y = campos[6].parse::<f64>().ok();
        let (Some(x), Some(y)) = (x, y) else { continue };
        // Una cota vacía o ilegible queda ausente (F12-03): ni 0 ni «tapa − 1».
        let cota = |s: &str| s.trim().parse::<f64>().ok().filter(|v| v.is_finite());
        let (zr, zs, zt) = (cota(campos[7]), cota(campos[8]), cota(campos[9]));
        let Some(dibujo) = zr.or(zt).or(zs) else {
            rep.descartados
                .push(format!("nodo {code}: sin ninguna cota legible"));
            continue;
        };
        let sin_dato: Vec<String> = [
            (zt.is_none(), crate::model::COTA_TERRENO),
            (zr.is_none(), crate::model::COTA_TAPA),
            (zs.is_none(), crate::model::COTA_SOLERA),
        ]
        .iter()
        .filter(|(falta, _)| *falta)
        .map(|(_, k)| k.to_string())
        .collect();
        if !sin_dato.is_empty() {
            rep.avisos
                .push(format!("nodo {code}: sin {}", sin_dato.join(", ")));
        }
        let node = Node {
            id: ElementId::new(0, 0),
            family: familia.to_string(),
            ty: subtipo.to_string(),
            code: code.clone(),
            pos: P2::new(x, y),
            z_terrain: zt.unwrap_or(dibujo),
            z_rim: zr.unwrap_or(dibujo),
            z_invert: zs.unwrap_or(dibujo),
            attrs: attrs.get(&code).cloned().unwrap_or_default(),
            datos: datos.get(&code).cloned().unwrap_or_default(),
            estado: EstadoObjeto::Pendiente,
            nota: None,
            modelo: None,
            deleted: false,
            sin_dato,
        };
        match p.add_node(node) {
            Ok(id) => {
                ids.insert(code, id);
                nodos_creados += 1;
            }
            Err(e) => rep.descartados.push(format!("nodo {code}: {e}")),
        }
    }

    let mut arcos_creados = 0usize;
    for t in textos {
        let Some(resto) = t.strip_prefix("REDES|1|") else {
            continue;
        };
        let campos: Vec<&str> = resto.split('|').collect();
        if campos.first() != Some(&"L") || campos.len() < 7 {
            continue;
        }
        let (familia, subtipo) = (campos[1], campos[2]);
        let code = campos[3].to_string();
        // DN vacío, ilegible o 0: ausente, no 200 (F12-03).
        let dn = campos[5].trim().parse::<u32>().ok().filter(|d| *d > 0);
        let trazado: Vec<P3> = campos[6]
            .split(';')
            .filter_map(|v| {
                let c: Vec<f64> = v.split(',').filter_map(|n| n.parse().ok()).collect();
                (c.len() == 3).then(|| P3::new(c[0], c[1], c[2]))
            })
            .collect();
        if trazado.len() < 2 {
            continue;
        }
        // Extremos: el nodo más cercano a cada punta del trazado.
        let mas_cercano = |q: P3| -> Option<ElementId> {
            p.nodes
                .iter()
                .filter(|n| !n.deleted)
                .map(|n| (n.id, n.pos.dist(q.p2())))
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(id, _)| id)
        };
        let (Some(desde), Some(hasta)) = (
            mas_cercano(trazado[0]),
            mas_cercano(*trazado.last().unwrap()),
        ) else {
            continue;
        };
        let link = Link {
            id: ElementId::new(0, 0),
            family: familia.to_string(),
            ty: subtipo.to_string(),
            code: code.clone(),
            from: desde,
            to: hasta,
            path: trazado,
            d_mm: dn.unwrap_or(0),
            attrs: attrs.get(&code).cloned().unwrap_or_default(),
            datos: datos.get(&code).cloned().unwrap_or_default(),
            estado: EstadoObjeto::Pendiente,
            nota: None,
            cota_solera_inicio: None,
            cota_solera_fin: None,
            deleted: false,
            sin_dato: if dn.is_none() {
                rep.avisos.push(format!(
                    "arco {code}: sin {}",
                    crate::model::DIAMETRO_NOMINAL
                ));
                vec![crate::model::DIAMETRO_NOMINAL.to_string()]
            } else {
                Vec::new()
            },
        };
        match p.add_link(link) {
            Ok(_) => arcos_creados += 1,
            Err(e) => rep.descartados.push(format!("arco {code}: {e}")),
        }
    }

    if nodos_creados == 0 && arcos_creados == 0 {
        return Err(
            "E_FMT_NOT_REDES: la capa REDES_INFO no trae ningún objeto legible (REDES|1|N/L)"
                .into(),
        );
    }
    rep.nodos = nodos_creados;
    rep.arcos = arcos_creados;
    Ok(())
}

/// Interpreta el texto de un atributo exportado: número, entero, booleano o texto.
fn parse_valor(v: &str) -> Value {
    if let Ok(n) = v.parse::<f64>() {
        return Value::Num(n);
    }
    match v {
        "true" => Value::Bool(true),
        "false" => Value::Bool(false),
        otro => Value::Text(otro.to_string()),
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

    /// En la vuelta del DXF, una cota o un DN vacíos quedan ausentes: ni 0, ni «tapa − 1», ni 200.
    #[test]
    fn la_vuelta_del_dxf_no_rellena_cotas_ni_dn() {
        let texto = |s: &str| format!("0\nTEXT\n8\nREDES_INFO\n10\n0\n20\n0\n30\n0\n1\n{s}\n");
        let dxf = format!(
            "0\nSECTION\n2\nENTITIES\n{}{}{}{}0\nENDSEC\n0\nEOF\n",
            texto("REDES|1|N|saneamiento.pozo|inspeccion|PZ-001|pendiente|0|0||98.0|"),
            texto("REDES|1|N|saneamiento.pozo|inspeccion|PZ-002|pendiente|10|0|||100.0"),
            texto("REDES|1|N|saneamiento.pozo|inspeccion|PZ-003|pendiente|50|50|||"),
            texto(
                "REDES|1|L|saneamiento.tuberia|colector_sanitario|CL-001|pendiente||0,0,98;10,0,97.9"
            ),
        );
        let ruta = std::env::temp_dir().join(format!("redes-vuelta-{}.dxf", std::process::id()));
        std::fs::write(&ruta, dxf).unwrap();
        let mut p = Project::new("x", "EPSG:32717");
        let rep = import_dxf(&mut p, &ruta, 0.0).expect("importa");
        let _ = std::fs::remove_file(&ruta);
        let pz = p.node_by_code("PZ-001").unwrap();
        assert_eq!(pz.cota_solera(), Some(98.0));
        assert_eq!(pz.cota_tapa(), None);
        assert_eq!(pz.cota_terreno(), None);
        assert_eq!(
            p.node_by_code("PZ-002").unwrap().cota_terreno(),
            Some(100.0)
        );
        assert!(p.node_by_code("PZ-003").is_none(), "sin ninguna cota");
        assert!(rep.descartados.iter().any(|d| d.contains("PZ-003")));
        assert_eq!(p.link_by_code("CL-001").unwrap().dn(), None);
        assert!(!rep.avisos.is_empty());
    }

    #[test]
    fn la_fecha_tiene_forma_iso() {
        let d = today();
        assert_eq!(d.len(), 10);
        assert_eq!(&d[4..5], "-");
    }
}
