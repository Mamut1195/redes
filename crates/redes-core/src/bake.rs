//! Parametric families baked by Blender (D14, D15, D16).
//!
//! A family recipe is a Blender script `X.py` plus its contract `X.receta.json`, which says where
//! each parameter comes from: a project attribute, the rim-to-invert height, or a fixed shape
//! value of the recipe. This module resolves the parameters of an object, refuses to fill in the
//! missing ones, and names the cached `.glb` after the recipe and the parameters, so the same
//! inputs never bake twice. Running Blender is the CLI's job (`redes biblioteca hornear`).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::Value as Json;

use crate::biblioteca::{self, Entrada};
use crate::families::Value;
use crate::model::{COTA_SOLERA, COTA_TAPA, Link, Node, Project};

pub const RECIPE_SCHEMA: &str = "redes.receta/1";
pub const RECIPE_SUFFIX: &str = ".receta.json";
/// Shared Blender driver every recipe imports; it is part of the cache key.
pub const DRIVER_FILE: &str = "redes_bake.py";
/// Cache folder, relative to the library folder (next to `biblioteca.json`).
pub const CACHE_DIR: &str = "biblioteca/horneado";
const KEY_DOMAIN: &[u8] = b"redes.horneado/1";
const KEY_HEX_LEN: usize = 32;

/// Prefix of the `extras` the manifest keeps for a baked piece.
pub const EXTRAS_PREFIX: &str = "horneado.";

/// Where a recipe parameter comes from.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(tag = "desde")]
pub enum ParamSource {
    /// An attribute of the object, as imported.
    #[serde(rename = "atributo")]
    Attribute {
        #[serde(rename = "clave")]
        key: String,
        #[serde(rename = "unidad", default)]
        unit: Option<String>,
    },
    /// Rim elevation minus invert elevation; both must come from the source.
    #[serde(rename = "tapa_menos_solera")]
    RimMinusInvert {
        #[serde(rename = "unidad", default)]
        unit: Option<String>,
    },
    /// An elevation attribute minus the object's invert, both from the source: structures draw
    /// their levels (crest, sill, bottom, water) in the frame the viewer places at the invert, and
    /// the same structure at another elevation is the same piece
    /// (docs/familias/F17-06-captacion.md).
    #[serde(rename = "menos_solera")]
    AttributeMinusInvert {
        #[serde(rename = "clave")]
        key: String,
        #[serde(rename = "unidad", default)]
        unit: Option<String>,
    },
    /// The pipes that reach the object: name, direction and invert of each one, from the links
    /// that carry those data (docs/17, section F17-03). A pipe without them has no anchor.
    #[serde(rename = "conexiones")]
    Connections,
    /// A drawing detail fixed by the recipe (cone height, segments…). Never becomes a `Dato`.
    #[serde(rename = "forma")]
    Shape {
        #[serde(rename = "valor")]
        value: Json,
        #[serde(rename = "unidad", default)]
        unit: Option<String>,
    },
}

#[derive(Clone, Debug, Deserialize)]
pub struct Recipe {
    pub schema: String,
    #[serde(rename = "familia")]
    pub family: String,
    #[serde(rename = "subtipo", default)]
    pub subtype: Option<String>,
    #[serde(rename = "parametros")]
    pub params: BTreeMap<String, ParamSource>,
}

/// One set of recipe parameters, by recipe name. Sorted, so its JSON is canonical.
pub type ParamSet = BTreeMap<String, Json>;

/// A recipe read from disk with the bytes that decide its geometry.
#[derive(Clone, Debug)]
pub struct LoadedRecipe {
    pub recipe: Recipe,
    pub name: String,
    pub script: PathBuf,
    script_bytes: Vec<u8>,
    driver_bytes: Vec<u8>,
}

impl LoadedRecipe {
    /// Reads `X.receta.json`, its script `X.py` and the shared driver next to them.
    pub fn load(contract: &Path) -> Result<LoadedRecipe, String> {
        let file_name = contract
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or_default();
        let Some(name) = file_name.strip_suffix(RECIPE_SUFFIX) else {
            return Err(format!(
                "E_RECETA_INVALIDA: {} no termina en {RECIPE_SUFFIX}",
                contract.display()
            ));
        };
        let text = std::fs::read_to_string(contract)
            .map_err(|e| format!("E_IO_NOT_FOUND: {}: {e}", contract.display()))?;
        let recipe: Recipe = serde_json::from_str(text.trim_start_matches('\u{feff}'))
            .map_err(|e| format!("E_RECETA_INVALIDA: {e}"))?;
        if recipe.schema != RECIPE_SCHEMA {
            return Err(format!(
                "E_RECETA_INVALIDA: esquema {} (se espera {RECIPE_SCHEMA})",
                recipe.schema
            ));
        }
        let dir = contract.parent().unwrap_or(Path::new("."));
        let script = dir.join(format!("{name}.py"));
        let read = |path: &Path| {
            std::fs::read(path).map_err(|e| format!("E_IO_NOT_FOUND: {}: {e}", path.display()))
        };
        Ok(LoadedRecipe {
            script_bytes: read(&script)?,
            driver_bytes: read(&dir.join(DRIVER_FILE))?,
            script,
            name: name.to_string(),
            recipe,
        })
    }

    pub fn key(&self, params: &ParamSet) -> String {
        bake_key(&self.driver_bytes, &self.script_bytes, params)
    }

    /// Manifest id of a baked piece: readable family prefix plus the start of the key.
    pub fn piece_id(&self, key: &str) -> String {
        format!("{}-{}", self.id_prefix(), &key[..12])
    }

    /// Prefix shared by every piece this recipe bakes, to find the stale ones.
    pub fn id_prefix(&self) -> String {
        match &self.recipe.subtype {
            Some(s) => format!("{}.{s}", self.recipe.family),
            None => self.recipe.family.clone(),
        }
    }
}

impl Recipe {
    /// Whether the recipe draws this object.
    pub fn applies_to(&self, node: &Node) -> bool {
        !node.deleted
            && node.family == self.family
            && self.subtype.as_deref().is_none_or(|s| node.ty == s)
    }

    /// Parameters of an object, or the project data it lacks (D16: nothing is filled in).
    pub fn params_for_node(&self, p: &Project, node: &Node) -> Result<ParamSet, Vec<String>> {
        self.resolve(|source| match source {
            ParamSource::Attribute { key, unit } => {
                attribute(node, key, unit.as_deref()).ok_or_else(|| key.clone())
            }
            ParamSource::RimMinusInvert { .. } => rim_minus_invert(node),
            ParamSource::AttributeMinusInvert { key, unit } => {
                attribute_minus_invert(node, key, unit.as_deref())
            }
            ParamSource::Connections => Ok(node_connections(p, node).to_json()),
            ParamSource::Shape { .. } => unreachable!("shape values come from the recipe"),
        })
    }

    /// Whether the recipe draws anchors for the pipes, so a missing one is worth reporting.
    pub fn uses_connections(&self) -> bool {
        self.params
            .values()
            .any(|s| matches!(s, ParamSource::Connections))
    }

    /// Parameters given by hand, by recipe name; shape values still come from the recipe.
    pub fn params_from_given(&self, given: &ParamSet) -> Result<ParamSet, Vec<String>> {
        let mut unknown: Vec<String> = given
            .keys()
            .filter(|k| {
                !matches!(
                    self.params.get(*k),
                    Some(
                        ParamSource::Attribute { .. }
                            | ParamSource::RimMinusInvert { .. }
                            | ParamSource::AttributeMinusInvert { .. }
                            | ParamSource::Connections
                    )
                )
            })
            .map(|k| format!("{k} (no es un parámetro de dato de la receta)"))
            .collect();
        let resolved =
            self.resolve_named(|name, _| given.get(name).cloned().ok_or_else(|| name.to_string()));
        match resolved {
            Ok(set) if unknown.is_empty() => Ok(set),
            Ok(_) => Err(unknown),
            Err(missing) => {
                unknown.extend(missing);
                Err(unknown)
            }
        }
    }

    fn resolve(
        &self,
        mut data: impl FnMut(&ParamSource) -> Result<Json, String>,
    ) -> Result<ParamSet, Vec<String>> {
        self.resolve_named(|_, source| data(source))
    }

    fn resolve_named(
        &self,
        mut data: impl FnMut(&str, &ParamSource) -> Result<Json, String>,
    ) -> Result<ParamSet, Vec<String>> {
        let mut set = ParamSet::new();
        let mut missing = Vec::new();
        for (name, source) in &self.params {
            let value = match source {
                ParamSource::Shape { value, .. } => Ok(value.clone()),
                other => data(name, other),
            };
            match value {
                Ok(v) => {
                    set.insert(name.clone(), v);
                }
                Err(lacking) => missing.push(lacking),
            }
        }
        missing.sort();
        missing.dedup();
        if missing.is_empty() {
            Ok(set)
        } else {
            Err(missing)
        }
    }

    /// Recipe units that disagree with the family registry (`unidad` vs the attribute's unit).
    pub fn unit_mismatches(&self) -> Vec<String> {
        let Some(def) = crate::families::family(&self.family) else {
            return vec![format!("familia desconocida: {}", self.family)];
        };
        let mut out = Vec::new();
        for (name, source) in &self.params {
            let (declared, expected) = match source {
                ParamSource::Attribute { key, unit }
                | ParamSource::AttributeMinusInvert { key, unit } => match def.attr(key) {
                    Some(a) => (unit.as_deref().unwrap_or(""), a.unit.symbol()),
                    None => {
                        out.push(format!("{name}: la familia no tiene el atributo {key}"));
                        continue;
                    }
                },
                ParamSource::RimMinusInvert { unit } => (unit.as_deref().unwrap_or(""), "m"),
                ParamSource::Connections | ParamSource::Shape { .. } => continue,
            };
            if declared != expected {
                out.push(format!(
                    "{name}: unidad «{declared}», la familia usa «{expected}»"
                ));
            }
        }
        out
    }
}

/// An attribute of the object or, if it has none, a `Dato` with that key in the recipe's unit:
/// a number that arrives with its source is as good as an imported attribute.
fn attribute(node: &Node, key: &str, unit: Option<&str>) -> Option<Json> {
    if node.falta(key) {
        return None;
    }
    let Some(value) = node.get(key) else {
        return node
            .datos
            .get(key)
            .filter(|d| d.valor.is_finite() && unit.is_none_or(|u| d.unidad == u))
            .map(|d| Json::from(d.valor));
    };
    match value {
        Value::Num(v) if v.is_finite() => Some(Json::from(*v)),
        Value::Num(_) => None,
        Value::Int(v) => Some(Json::from(*v)),
        Value::Bool(v) => Some(Json::from(*v)),
        Value::Text(v) if v.trim().is_empty() => None,
        Value::Text(v) => Some(Json::from(v.clone())),
    }
}

fn rim_minus_invert(node: &Node) -> Result<Json, String> {
    match (node.cota_tapa(), node.cota_solera()) {
        // Rounded to the micrometre: 2491.63 - 2490.0 is 1.6300000000001091 in f64, and that
        // noise would give the same manhole another key and other bytes than a typed 1.63.
        (Some(rim), Some(invert)) => Ok(Json::from(round_to(rim - invert, 1e6))),
        (None, _) => Err(COTA_TAPA.to_string()),
        (Some(_), None) => Err(COTA_SOLERA.to_string()),
    }
}

fn attribute_minus_invert(node: &Node, key: &str, unit: Option<&str>) -> Result<Json, String> {
    let level = attribute(node, key, unit)
        .and_then(|v| v.as_f64())
        .ok_or_else(|| key.to_string())?;
    let invert = node.cota_solera().ok_or_else(|| COTA_SOLERA.to_string())?;
    // Rounded like the derived depth, so float noise never gives the same structure another key.
    Ok(Json::from(round_to(level - invert, 1e6)))
}

/// The pipes of an object as recipe data: the ones with the data for an anchor and the others,
/// with why they have none.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Connections {
    pub anchored: Vec<Connection>,
    pub unanchored: Vec<Unanchored>,
}

/// One pipe end at the object, in the object's frame.
#[derive(Clone, Debug, PartialEq)]
pub struct Connection {
    /// `salida__<code>` for a pipe that starts at the object, `entrada__<code>` for one that ends
    /// there. It names the empty in the `.glb`.
    pub name: String,
    /// Direction from the object to the pipe, counter-clockwise from +X (east), in degrees, minus
    /// the object's `giro` if it declares one.
    pub azimuth_deg: f64,
    /// Pipe invert at the object minus the object's invert (m).
    pub invert_rise_m: f64,
    pub dn_mm: Option<u32>,
}

/// A pipe that reaches the object without the data to place its anchor.
#[derive(Clone, Debug, PartialEq)]
pub struct Unanchored {
    pub link: Option<String>,
    pub reason: String,
}

impl Connections {
    /// Canonical JSON: sorted by name, rounded like the derived depth so float noise never
    /// changes the key.
    pub fn to_json(&self) -> Json {
        let mut sorted = self.anchored.clone();
        sorted.sort_by(|a, b| a.name.cmp(&b.name));
        Json::Array(
            sorted
                .iter()
                .map(|c| {
                    serde_json::json!({
                        "name": c.name,
                        "azimuth_deg": round_to(c.azimuth_deg, 1e6),
                        "invert_rise_m": round_to(c.invert_rise_m, 1e6),
                        "dn_mm": c.dn_mm,
                    })
                })
                .collect(),
        )
    }
}

/// Pipes that reach a node and, for each one, its anchor data or why it has none. Nothing is
/// guessed: the invert must be the link's declared one (or its datum path), the direction comes
/// from the drawn path, and the node's own invert must be a datum.
pub fn node_connections(p: &Project, node: &Node) -> Connections {
    let mut out = Connections::default();
    let links: Vec<&Link> = p.tramos_de(node.id).collect();
    if links.is_empty() {
        out.unanchored.push(Unanchored {
            link: None,
            reason: "sin tramos conectados".into(),
        });
        return out;
    }
    let Some(invert) = node.cota_solera() else {
        for l in links {
            out.unanchored.push(Unanchored {
                link: Some(l.code.clone()),
                reason: "el pozo no trae cota de solera".into(),
            });
        }
        return out;
    };
    let giro = node.num("giro").filter(|g| g.is_finite()).unwrap_or(0.0);
    for l in links {
        match connection_of(l, node, invert, giro) {
            Ok(c) => out.anchored.push(c),
            Err(reason) => out.unanchored.push(Unanchored {
                link: Some(l.code.clone()),
                reason,
            }),
        }
    }
    out
}

fn connection_of(l: &Link, node: &Node, invert: f64, giro: f64) -> Result<Connection, String> {
    let starts_here = l.from == node.id;
    let declared = if starts_here {
        l.cota_solera_inicio
    } else {
        l.cota_solera_fin
    };
    let pipe_invert = declared
        .or_else(|| {
            l.cotas_trazado()
                .map(|(a, b)| if starts_here { a } else { b })
        })
        .filter(|z| z.is_finite())
        .ok_or_else(|| "el tramo no trae cota de solera en este extremo".to_string())?;
    let neighbour = if starts_here {
        l.path.iter().skip(1).find(|q| q.p2().dist(node.pos) > 1e-6)
    } else {
        l.path
            .iter()
            .rev()
            .skip(1)
            .find(|q| q.p2().dist(node.pos) > 1e-6)
    }
    .ok_or_else(|| "el trazado del tramo no sale del pozo".to_string())?;
    let azimuth = (neighbour.y - node.pos.y)
        .atan2(neighbour.x - node.pos.x)
        .to_degrees()
        - giro;
    let kind = if starts_here { "salida" } else { "entrada" };
    Ok(Connection {
        name: format!("{kind}__{}", l.code),
        azimuth_deg: normalise_degrees(azimuth),
        invert_rise_m: pipe_invert - invert,
        dn_mm: l.dn(),
    })
}

fn normalise_degrees(a: f64) -> f64 {
    let wrapped = a.rem_euclid(360.0);
    if wrapped > 180.0 {
        wrapped - 360.0
    } else {
        wrapped
    }
}

/// Cache key of a bake: the driver, the recipe script and the canonical parameters.
///
/// Line endings are normalised so a checkout with CRLF names the same `.glb` as one with LF.
/// The Blender version is left out on purpose: the cache must be readable without Blender, and
/// the recipes are pinned to one Blender release (docs/17, section F17-02).
pub fn bake_key(driver: &[u8], script: &[u8], params: &ParamSet) -> String {
    let canonical = serde_json::to_vec(params).expect("a map of JSON values serialises");
    let mut hasher = blake3::Hasher::new();
    hasher.update(KEY_DOMAIN);
    for part in [lf_only(driver), lf_only(script), canonical] {
        hasher.update(&(part.len() as u64).to_le_bytes());
        hasher.update(&part);
    }
    hasher.finalize().to_hex()[..KEY_HEX_LEN].to_string()
}

fn lf_only(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len());
    for (i, b) in bytes.iter().enumerate() {
        if !(*b == b'\r' && bytes.get(i + 1) == Some(&b'\n')) {
            out.push(*b);
        }
    }
    out
}

/// Cached `.glb` of a key inside a library folder.
pub fn cached_glb(library_dir: &Path, key: &str) -> PathBuf {
    library_dir.join(CACHE_DIR).join(format!("{key}.glb"))
}

/// Review scene saved next to a cached piece. Not deterministic (Blender stores timestamps),
/// so it never takes part in the key or in byte comparisons.
pub fn cached_blend(library_dir: &Path, key: &str) -> PathBuf {
    library_dir.join(CACHE_DIR).join(format!("{key}.blend"))
}

/// Path of a cached piece relative to the manifest, with `/` on every platform.
pub fn cached_glb_relative(key: &str) -> String {
    format!("{CACHE_DIR}/{key}.glb")
}

/// What the manifest records about a bake: recipe, key and every parameter used.
pub fn bake_extras(recipe_name: &str, key: &str, params: &ParamSet) -> BTreeMap<String, String> {
    let mut out = BTreeMap::from([
        (format!("{EXTRAS_PREFIX}receta"), recipe_name.to_string()),
        (format!("{EXTRAS_PREFIX}clave"), key.to_string()),
    ]);
    for (name, value) in params {
        let text = match value {
            Json::String(s) => s.clone(),
            other => other.to_string(),
        };
        out.insert(format!("{EXTRAS_PREFIX}{name}"), text);
    }
    out
}

/// Manifest entry of a baked piece, measured from its `.glb` like an imported one.
pub fn baked_entry(
    loaded: &LoadedRecipe,
    key: &str,
    params: &ParamSet,
    glb: &[u8],
) -> Result<Entrada, String> {
    let model = biblioteca::leer_glb(glb).map_err(|e| format!("E_GLB_INVALIDO: {e}"))?;
    let measured = biblioteca::cantidades_glb(glb).ok();
    let mut extras = model.extras;
    extras.extend(bake_extras(&loaded.name, key, params));
    Ok(Entrada {
        id: loaded.piece_id(key),
        familia: loaded.recipe.family.clone(),
        subtipo: loaded.recipe.subtype.clone(),
        dn_mm: None,
        archivo: cached_glb_relative(key),
        escala: 1.0,
        triangulos: model.triangulos,
        dimensiones_m: model.bbox.dimensiones().map(|d| round_to(d, 1e3)),
        bytes: glb.len(),
        volumen_m3: measured.as_ref().map(|c| round_to(c.volumen_m3.abs(), 1e4)),
        area_m2: measured.as_ref().map(|c| round_to(c.area_m2, 1e3)),
        cerrada: measured.as_ref().map(|c| c.cerrada()),
        anclas: biblioteca::anclas_de_glb(glb).unwrap_or_default(),
        extras,
        // The recipes live in this MIT repository.
        licencia: Some("MIT".into()),
        autor: Some(format!("receta:{}", loaded.name)),
    })
}

fn round_to(v: f64, scale: f64) -> f64 {
    (v * scale).round() / scale
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::families::Value;

    fn repo_recipe() -> LoadedRecipe {
        let contract = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tools/blender/build_manhole.receta.json");
        LoadedRecipe::load(&contract).expect("the manhole recipe loads")
    }

    fn demo() -> Project {
        crate::demo::project()
    }

    fn manhole() -> Node {
        let mut n = demo()
            .nodes
            .iter()
            .find(|n| n.family == "saneamiento.pozo" && n.ty == "inspeccion")
            .expect("demo manhole")
            .clone();
        n.attrs.insert("diametro_interior".into(), Value::Num(1.2));
        n.attrs.insert("espesor_pared".into(), Value::Num(0.15));
        n.attrs
            .insert("material".into(), Value::Text("hormigon".into()));
        n.sin_dato.clear();
        n
    }

    #[test]
    fn manhole_params_come_from_the_object_and_the_recipe() {
        let r = repo_recipe();
        let n = manhole();
        let set = r
            .recipe
            .params_for_node(&demo(), &n)
            .expect("complete manhole");
        assert_eq!(set["inner_diameter_m"], Json::from(1.2));
        assert_eq!(
            set["depth_m"],
            Json::from(round_to(n.z_rim - n.z_invert, 1e6))
        );
        assert_eq!(set["body_material"], Json::from("hormigon"));
        assert_eq!(set["segments"], Json::from(48), "shape value of the recipe");
        assert_eq!(set.len(), r.recipe.params.len());
    }

    #[test]
    fn a_derived_depth_carries_no_float_noise() {
        let r = repo_recipe();
        let mut n = manhole();
        n.z_rim = 2491.63;
        n.z_invert = 2490.0;
        let set = r.recipe.params_for_node(&demo(), &n).unwrap();
        assert_eq!(set["depth_m"], Json::from(1.63));
    }

    #[test]
    fn a_missing_datum_is_reported_not_filled_in() {
        let r = repo_recipe();
        let mut n = manhole();
        n.attrs.remove("espesor_pared");
        n.sin_dato.push(COTA_TAPA.into());
        let missing = r
            .recipe
            .params_for_node(&demo(), &n)
            .expect_err("incomplete manhole");
        assert_eq!(
            missing,
            vec!["cota_tapa".to_string(), "espesor_pared".to_string()]
        );
    }

    #[test]
    fn the_recipe_units_agree_with_the_family_registry() {
        assert_eq!(repo_recipe().recipe.unit_mismatches(), Vec::<String>::new());
    }

    #[test]
    fn the_key_follows_params_and_recipe_but_not_key_order_or_line_endings() {
        let r = repo_recipe();
        let set = r.recipe.params_for_node(&demo(), &manhole()).unwrap();
        let key = r.key(&set);
        assert_eq!(key.len(), KEY_HEX_LEN);
        assert_eq!(key, r.key(&set.clone()), "deterministic");

        let mut deeper = set.clone();
        deeper.insert("depth_m".into(), Json::from(9.0));
        assert_ne!(key, r.key(&deeper), "another depth is another piece");

        let script = std::fs::read(&r.script).unwrap();
        let mut edited = script.clone();
        edited.extend_from_slice(b"\n# edited\n");
        assert_ne!(
            bake_key(&r.driver_bytes, &script, &set),
            bake_key(&r.driver_bytes, &edited, &set),
            "a recipe edit invalidates the cache"
        );
        let crlf: Vec<u8> = String::from_utf8(script.clone())
            .unwrap()
            .replace('\n', "\r\n")
            .into_bytes();
        assert_eq!(
            bake_key(&r.driver_bytes, &script, &set),
            bake_key(&r.driver_bytes, &crlf, &set)
        );
    }

    #[test]
    fn given_params_must_be_data_params_and_complete() {
        let r = repo_recipe();
        let full = r.recipe.params_for_node(&demo(), &manhole()).unwrap();
        let data: ParamSet = full
            .iter()
            .filter(|(k, _)| !matches!(r.recipe.params[*k], ParamSource::Shape { .. }))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        assert_eq!(r.recipe.params_from_given(&data).unwrap(), full);

        let mut with_shape = data.clone();
        with_shape.insert("segments".into(), Json::from(8));
        assert!(r.recipe.params_from_given(&with_shape).is_err());

        let mut short = data;
        short.remove("depth_m");
        assert_eq!(
            r.recipe.params_from_given(&short).unwrap_err(),
            vec!["depth_m".to_string()]
        );
    }

    #[test]
    fn every_manhole_of_the_chain_demo_has_the_data_to_bake() {
        let r = repo_recipe();
        let p = crate::demo_cadena::cadena();
        let manholes: Vec<&Node> = p.nodes.iter().filter(|n| r.recipe.applies_to(n)).collect();
        assert!(manholes.len() >= 10, "{}", manholes.len());
        for n in manholes {
            let set = r.recipe.params_for_node(&p, n).unwrap_or_else(|m| {
                panic!("{}: missing {m:?}", n.code);
            });
            assert_eq!(set["wall_thickness_m"], Json::from(0.15), "{}", n.code);
            assert_eq!(
                n.datos["espesor_pared"].fuente,
                crate::demo_cadena::DEMO_SOURCE,
                "the thickness says it is demo data"
            );
        }
    }

    #[test]
    fn a_datum_in_another_unit_is_not_taken_as_the_attribute() {
        let r = repo_recipe();
        let mut n = manhole();
        n.attrs.remove("espesor_pared");
        n.datos.insert(
            "espesor_pared".into(),
            crate::model::Dato {
                valor: 150.0,
                unidad: "mm".into(),
                fuente: "Civil 3D".into(),
            },
        );
        assert_eq!(
            r.recipe.params_for_node(&demo(), &n).unwrap_err(),
            vec!["espesor_pared".to_string()]
        );
    }

    #[test]
    fn pipes_become_anchors_with_their_invert_and_direction() {
        let p = crate::demo_cadena::cadena();
        let middle = p.node_by_code("PZ-302").expect("sewer manhole");
        let c = node_connections(&p, middle);
        assert!(c.unanchored.is_empty(), "{:?}", c.unanchored);
        let names: Vec<&str> = c.anchored.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(names, vec!["entrada__CL-302", "salida__CL-303"]);
        for a in &c.anchored {
            // The demo pipes reach the manholes 1.5 m under the ground; the inverts are 1.75 m.
            assert!((a.invert_rise_m - 0.25).abs() < 1e-6, "{a:?}");
            assert_eq!(a.dn_mm, Some(200));
        }
        let outlet = &c.anchored[1];
        // The next manhole is 22 m east and 4 m south.
        let expected = (-4.0f64).atan2(22.0).to_degrees();
        assert!((outlet.azimuth_deg - expected).abs() < 1e-6, "{outlet:?}");
        let inlet = &c.anchored[0];
        assert!(
            (inlet.azimuth_deg - (expected + 180.0)).abs() < 1e-6,
            "{inlet:?}"
        );
    }

    #[test]
    fn a_pipe_without_invert_has_no_anchor_and_says_why() {
        let mut p = crate::demo_cadena::cadena();
        let id = p.node_by_code("PZ-302").unwrap().id;
        for l in p.links.iter_mut().filter(|l| l.to == id) {
            l.sin_dato.push(COTA_SOLERA.into());
        }
        let c = node_connections(&p, p.node_by_code("PZ-302").unwrap());
        assert_eq!(c.anchored.len(), 1);
        assert_eq!(c.unanchored[0].link.as_deref(), Some("CL-302"));
        let lonely = node_connections(&p, p.node_by_code("PZ-201").unwrap());
        assert!(lonely.anchored.is_empty());
        assert_eq!(lonely.unanchored[0].reason, "sin tramos conectados");
    }

    #[test]
    fn a_contract_must_be_named_after_its_script() {
        let err = LoadedRecipe::load(Path::new("tools/blender/build_manhole.json")).unwrap_err();
        assert!(err.starts_with("E_RECETA_INVALIDA"), "{err}");
    }
}
