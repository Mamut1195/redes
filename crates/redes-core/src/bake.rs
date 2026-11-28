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
use crate::model::{COTA_SOLERA, COTA_TAPA, DIAMETRO_NOMINAL, Dato, Link, Node, Project};

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
/// Recipe parameter that carries the DN of a pipe piece, recorded in its manifest entry.
pub const PIPE_DN_PARAM: &str = "dn_mm";

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
    /// The nominal diameter of a link (mm), when the source brought it.
    #[serde(rename = "dn")]
    NominalDiameter {
        #[serde(rename = "unidad", default)]
        unit: Option<String>,
    },
    /// The bore a link is drawn with (m): its declared inner diameter or, without one, its DN,
    /// the same rule as [`Link::diametro_interior_m`], so the baked pipe and its axis agree.
    #[serde(rename = "diametro_interior_o_dn")]
    DrawingBore {
        #[serde(rename = "unidad", default)]
        unit: Option<String>,
    },
    /// `agua` or `saneamiento`, from the object's family.
    #[serde(rename = "sistema")]
    System,
    /// The object's subtype, for a recipe that draws several (`subtipos`).
    #[serde(rename = "subtipo")]
    Subtype,
    /// The ports of a fitting in its own frame (docs/familias/F17-05): one per pipe that reaches
    /// it, with the direction relative to the reference port and the pipe's DN. The frame turns
    /// with the pipes, so equal fittings share one piece wherever they point.
    #[serde(rename = "puertos")]
    Ports {
        /// How many ports each subtype has; another count is missing data, not a guess.
        #[serde(rename = "por_subtipo")]
        count_by_subtype: BTreeMap<String, usize>,
        /// Angles are drawn to this step (degrees), so a bend of 44.97° shares the 45° piece.
        #[serde(rename = "paso_angulo_deg")]
        angle_step_deg: f64,
        /// Every pipe must declare its DN (the piece is drawn with it).
        #[serde(rename = "con_dn", default)]
        require_dn: bool,
        /// The piece is straight (a valve, a reducer): the far port is drawn on the run, so a
        /// pipe that leaves at an angle does not make another piece.
        #[serde(rename = "recto", default)]
        straight: bool,
    },
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
    /// More families the recipe draws: one pipe recipe serves water and sewer links.
    #[serde(rename = "otras_familias", default)]
    pub other_families: Vec<String>,
    /// Subtypes one recipe draws when they share their data (gate, butterfly and check valves).
    #[serde(rename = "subtipos", default)]
    pub subtypes: Vec<String>,
    #[serde(rename = "colocacion", default)]
    pub placement: Placement,
    #[serde(rename = "parametros")]
    pub params: BTreeMap<String, ParamSource>,
    /// Python modules next to the script that it imports (`plant_units.py`…). They shape the
    /// geometry as much as the script does, so they are part of the cache key.
    #[serde(rename = "modulos", default)]
    pub modules: Vec<String>,
}

/// How the viewer turns and lifts a baked piece onto its object.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
pub enum Placement {
    /// Turned by the object's `giro`, sat on its invert (the manhole, F17-03).
    #[default]
    #[serde(rename = "objeto")]
    Object,
    /// Turned like its ports' frame and sat on the reference pipe's invert: a fitting on a line.
    #[serde(rename = "puertos")]
    Ports,
    /// Turned like its ports' frame and sat on the object's invert: a hydrant on its branch.
    #[serde(rename = "puertos_en_objeto")]
    PortsOnObject,
}

impl Placement {
    fn extra_value(self) -> Option<&'static str> {
        match self {
            Placement::Object => None,
            Placement::Ports => Some("puertos"),
            Placement::PortsOnObject => Some("puertos_en_objeto"),
        }
    }
}

/// Manifest extra that tells the viewer how to place a piece (absent: [`Placement::Object`]).
pub const PLACEMENT_EXTRA: &str = "horneado.colocacion";

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
    module_bytes: Vec<Vec<u8>>,
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
        let module_bytes = recipe
            .modules
            .iter()
            .map(|m| read(&dir.join(m)))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(LoadedRecipe {
            script_bytes: read(&script)?,
            driver_bytes: read(&dir.join(DRIVER_FILE))?,
            module_bytes,
            script,
            name: name.to_string(),
            recipe,
        })
    }

    pub fn key(&self, params: &ParamSet) -> String {
        let mut sources = vec![self.script_bytes.as_slice()];
        sources.extend(self.module_bytes.iter().map(Vec::as_slice));
        key_of_sources(&self.driver_bytes, &sources, params)
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
        !node.deleted && self.draws(&node.family, &node.ty)
    }

    /// Whether the recipe draws this link.
    pub fn applies_to_link(&self, link: &Link) -> bool {
        !link.deleted && self.draws(&link.family, &link.ty)
    }

    fn draws(&self, family: &str, ty: &str) -> bool {
        (family == self.family || self.other_families.iter().any(|f| f == family))
            && self.subtype.as_deref().is_none_or(|s| ty == s)
            && (self.subtypes.is_empty() || self.subtypes.iter().any(|s| s == ty))
    }

    /// Parameters of an object, or the project data it lacks (D16: nothing is filled in).
    pub fn params_for_node(&self, p: &Project, node: &Node) -> Result<ParamSet, Vec<String>> {
        self.resolve(|source| match source {
            ParamSource::Attribute { key, unit } => attribute(
                node.falta(key),
                node.get(key),
                node.datos.get(key),
                unit.as_deref(),
            )
            .ok_or_else(|| key.clone()),
            ParamSource::RimMinusInvert { .. } => rim_minus_invert(node),
            ParamSource::AttributeMinusInvert { key, unit } => {
                attribute_minus_invert(node, key, unit.as_deref())
            }
            ParamSource::Connections => Ok(node_connections(p, node).to_json()),
            ParamSource::Subtype => Ok(Json::from(node.ty.clone())),
            ParamSource::Ports {
                count_by_subtype,
                angle_step_deg,
                require_dn,
                straight,
            } => {
                let wanted = count_by_subtype.get(&node.ty).copied();
                let step = (!*straight).then_some(*angle_step_deg);
                ports_param(p, node, wanted, step, *require_dn)
                    .map_err(|why| format!("{PORTS_MISSING} ({why})"))
            }
            ParamSource::NominalDiameter { .. } | ParamSource::DrawingBore { .. } => {
                Err(DIAMETRO_NOMINAL.to_string())
            }
            ParamSource::System => Ok(Json::from(system_of(&node.family))),
            ParamSource::Shape { .. } => unreachable!("shape values come from the recipe"),
        })
    }

    /// Parameters of a link, or the data it lacks (D16: nothing is filled in).
    pub fn params_for_link(&self, link: &Link) -> Result<ParamSet, Vec<String>> {
        self.resolve(|source| match source {
            ParamSource::Attribute { key, unit } => attribute(
                link.falta(key),
                link.get(key),
                link.datos.get(key),
                unit.as_deref(),
            )
            .ok_or_else(|| key.clone()),
            ParamSource::NominalDiameter { .. } => link
                .dn()
                .map(Json::from)
                .ok_or_else(|| DIAMETRO_NOMINAL.to_string()),
            ParamSource::DrawingBore { .. } => drawing_bore(link),
            ParamSource::System => Ok(Json::from(system_of(&link.family))),
            ParamSource::RimMinusInvert { .. } => Err(COTA_TAPA.to_string()),
            ParamSource::AttributeMinusInvert { .. } => Err(COTA_SOLERA.to_string()),
            ParamSource::Connections => Ok(Connections::default().to_json()),
            ParamSource::Subtype => Ok(Json::from(link.ty.clone())),
            ParamSource::Ports { .. } => Err(PORTS_MISSING.to_string()),
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
                self.params
                    .get(*k)
                    .is_none_or(|s| matches!(s, ParamSource::Shape { .. }))
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
        let mut out = Vec::new();
        for family in std::iter::once(&self.family).chain(&self.other_families) {
            let Some(def) = crate::families::family(family) else {
                out.push(format!("familia desconocida: {family}"));
                continue;
            };
            for (name, source) in &self.params {
                let (declared, expected) = match source {
                    ParamSource::Attribute { key, unit }
                    | ParamSource::AttributeMinusInvert { key, unit } => match def.attr(key) {
                        Some(a) => (unit.as_deref().unwrap_or(""), a.unit.symbol()),
                        None => {
                            out.push(format!(
                                "{name}: la familia {family} no tiene el atributo {key}"
                            ));
                            continue;
                        }
                    },
                    ParamSource::RimMinusInvert { unit } | ParamSource::DrawingBore { unit } => {
                        (unit.as_deref().unwrap_or(""), "m")
                    }
                    ParamSource::NominalDiameter { unit } => (unit.as_deref().unwrap_or(""), "mm"),
                    ParamSource::Connections
                    | ParamSource::System
                    | ParamSource::Subtype
                    | ParamSource::Ports { .. }
                    | ParamSource::Shape { .. } => {
                        continue;
                    }
                };
                if declared != expected {
                    out.push(format!(
                        "{name}: unidad «{declared}», la familia {family} usa «{expected}»"
                    ));
                }
            }
        }
        out
    }
}

/// An attribute of the object or, if it has none, a `Dato` with that key in the recipe's unit:
/// a number that arrives with its source is as good as an imported attribute.
fn attribute(
    missing: bool,
    value: Option<&Value>,
    datum: Option<&Dato>,
    unit: Option<&str>,
) -> Option<Json> {
    if missing {
        return None;
    }
    let Some(value) = value else {
        return datum
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

fn drawing_bore(link: &Link) -> Result<Json, String> {
    let declared = link
        .num("diametro_interior")
        .filter(|d| d.is_finite() && *d > 0.0 && !link.falta("diametro_interior"));
    match (declared, link.dn()) {
        // Declared in mm (family registry); rounded like the depth so the key carries no noise.
        (Some(mm), _) => Ok(Json::from(round_to(mm / 1000.0, 1e6))),
        (None, Some(dn)) => Ok(Json::from(round_to(dn as f64 / 1000.0, 1e6))),
        (None, None) => Err(DIAMETRO_NOMINAL.to_string()),
    }
}

/// `agua` or `saneamiento`: the first part of the family key.
fn system_of(family: &str) -> &str {
    family.split('.').next().unwrap_or(family)
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
    let level = attribute(node.falta(key), node.get(key), node.datos.get(key), unit)
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

/// Name of the missing datum when a fitting's pipes do not give its ports.
pub const PORTS_MISSING: &str = "conexiones";

/// A fitting's ports in its own frame: the reference port points to -X (azimuth 180°), so the
/// run goes along +X. Turning the piece by `rotation_deg` puts every port on its pipe.
#[derive(Clone, Debug, PartialEq)]
pub struct PortFrame {
    /// World azimuth the frame is turned by (degrees, counter-clockwise from +X).
    pub rotation_deg: f64,
    /// Invert of the reference pipe at the object (m).
    pub reference_invert_m: f64,
    /// The reference port first, then the others by azimuth.
    pub ports: Vec<Port>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Port {
    /// `entrada__<n>` for a pipe that ends at the fitting, `salida__<n>` for one that starts there;
    /// it names the anchor. The number is the port's place in the frame, not the pipe's code, so
    /// the piece is shared.
    pub name: String,
    pub link: String,
    /// Direction to the pipe in the frame (degrees), unrounded.
    pub azimuth_deg: f64,
    pub dn_mm: Option<u32>,
}

struct PipeEnd<'a> {
    link: &'a Link,
    incoming: bool,
    azimuth_deg: f64,
    invert_m: f64,
}

/// The ports of a fitting from the pipes that reach it, or why they cannot be placed. Nothing
/// is guessed: each pipe needs its drawn direction and its invert at the fitting.
pub fn node_port_frame(p: &Project, node: &Node) -> Result<PortFrame, String> {
    let mut links: Vec<&Link> = p.tramos_de(node.id).collect();
    if links.is_empty() {
        return Err("sin tramos conectados".into());
    }
    links.sort_by(|a, b| a.code.cmp(&b.code));
    let ends = links
        .into_iter()
        .map(|l| pipe_end(l, node).map_err(|why| format!("{}: {why}", l.code)))
        .collect::<Result<Vec<_>, _>>()?;
    let reference = &ends[reference_end(&ends)];
    let rotation = normalise_degrees(reference.azimuth_deg - 180.0);
    let mut others: Vec<(f64, &PipeEnd)> = ends
        .iter()
        .filter(|e| !std::ptr::eq(*e, reference))
        .map(|e| (normalise_degrees(e.azimuth_deg - rotation), e))
        .collect();
    others.sort_by(|a, b| a.0.total_cmp(&b.0));
    let ordered = std::iter::once((180.0, reference)).chain(others);
    let ports = ordered
        .enumerate()
        .map(|(i, (azimuth_deg, e))| Port {
            name: format!(
                "{}__{}",
                if e.incoming { "entrada" } else { "salida" },
                i + 1
            ),
            link: e.link.code.clone(),
            azimuth_deg,
            dn_mm: e.link.dn(),
        })
        .collect();
    Ok(PortFrame {
        rotation_deg: rotation,
        reference_invert_m: reference.invert_m,
        ports,
    })
}

/// The port the frame starts from: one of the straightest pair of pipes (the run of a tee), the
/// incoming one if only one of the pair comes in, else the first by code.
fn reference_end(ends: &[PipeEnd]) -> usize {
    let mut best: Option<(f64, usize, usize)> = None;
    for i in 0..ends.len() {
        for j in i + 1..ends.len() {
            let spread = normalise_degrees(ends[i].azimuth_deg - ends[j].azimuth_deg).abs();
            if best.is_none_or(|(s, _, _)| spread > s + 1e-9) {
                best = Some((spread, i, j));
            }
        }
    }
    match best {
        Some((_, i, j)) if !ends[i].incoming && ends[j].incoming => j,
        Some((_, i, _)) => i,
        None => 0,
    }
}

fn pipe_end<'a>(l: &'a Link, node: &Node) -> Result<PipeEnd<'a>, String> {
    let starts_here = l.from == node.id;
    let declared = if starts_here {
        l.cota_solera_inicio
    } else {
        l.cota_solera_fin
    };
    let invert = declared
        .or_else(|| {
            l.cotas_trazado()
                .map(|(a, b)| if starts_here { a } else { b })
        })
        .filter(|z| z.is_finite())
        .ok_or("el tramo no trae cota de solera en este extremo")?;
    let mut path = l.path.iter();
    let neighbour = if starts_here {
        path.find(|q| q.p2().dist(node.pos) > 1e-6)
    } else {
        path.rev().find(|q| q.p2().dist(node.pos) > 1e-6)
    }
    .ok_or("el trazado del tramo no sale del objeto")?;
    Ok(PipeEnd {
        link: l,
        incoming: !starts_here,
        azimuth_deg: (neighbour.y - node.pos.y)
            .atan2(neighbour.x - node.pos.x)
            .to_degrees(),
        invert_m: invert,
    })
}

/// The `puertos` parameter: the ports of the frame with their angles drawn to the recipe's step.
fn ports_param(
    p: &Project,
    node: &Node,
    wanted: Option<usize>,
    step_deg: Option<f64>,
    require_dn: bool,
) -> Result<Json, String> {
    let Some(wanted) = wanted else {
        return Err(format!(
            "la receta no dice cuántas bocas tiene «{}»",
            node.ty
        ));
    };
    let frame = node_port_frame(p, node)?;
    if frame.ports.len() != wanted {
        return Err(format!(
            "{} tramos conectados; la pieza tiene {wanted} bocas",
            frame.ports.len()
        ));
    }
    if require_dn && let Some(port) = frame.ports.iter().find(|q| q.dn_mm.is_none()) {
        return Err(format!("{}: el tramo no trae DN", port.link));
    }
    Ok(Json::Array(
        frame
            .ports
            .iter()
            .enumerate()
            .map(|(i, q)| {
                let azimuth = match step_deg {
                    Some(step) => drawn_angle(q.azimuth_deg, step),
                    None if i == 0 => 180.0,
                    None => 0.0,
                };
                serde_json::json!({
                    "name": q.name,
                    "azimuth_deg": azimuth,
                    "dn_mm": q.dn_mm,
                })
            })
            .collect(),
    ))
}

fn drawn_angle(a: f64, step_deg: f64) -> f64 {
    let stepped = if step_deg > 0.0 {
        (a / step_deg).round() * step_deg
    } else {
        a
    };
    round_to(normalise_degrees(stepped), 1e6)
}

/// Where a library piece goes on an object: its turn about Z and the elevation of its z = 0.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ModelPlacement {
    pub rotation_deg: f64,
    pub base_m: f64,
}

/// Placement of a piece on an object. A piece baked in its ports' frame turns with the pipes;
/// if the pipes no longer give a frame, it is placed like any other piece.
pub fn model_placement(p: &Project, node: &Node, piece: &Entrada) -> ModelPlacement {
    let by_object = ModelPlacement {
        rotation_deg: node.num("giro").filter(|g| g.is_finite()).unwrap_or(0.0),
        base_m: biblioteca::cota_base_modelo(node, &piece.anclas),
    };
    let mode = piece.extras.get(PLACEMENT_EXTRA).map(String::as_str);
    let on_pipe = mode == Placement::Ports.extra_value();
    if !on_pipe && mode != Placement::PortsOnObject.extra_value() {
        return by_object;
    }
    let Ok(frame) = node_port_frame(p, node) else {
        return by_object;
    };
    let invert_anchor = piece
        .anclas
        .get(biblioteca::ANCLA_SOLERA)
        .map_or(0.0, |a| a[2] as f64);
    ModelPlacement {
        rotation_deg: frame.rotation_deg,
        base_m: if on_pipe {
            frame.reference_invert_m - invert_anchor
        } else {
            by_object.base_m
        },
    }
}

/// Cache key of a bake: the driver, the recipe script and the canonical parameters.
///
/// Line endings are normalised so a checkout with CRLF names the same `.glb` as one with LF.
/// The Blender version is left out on purpose: the cache must be readable without Blender, and
/// the recipes are pinned to one Blender release (docs/17, section F17-02).
pub fn bake_key(driver: &[u8], script: &[u8], params: &ParamSet) -> String {
    key_of_sources(driver, &[script], params)
}

/// [`bake_key`] of a script and the modules it imports, in the recipe's order. A recipe without
/// modules keeps the key it had before modules existed.
fn key_of_sources(driver: &[u8], sources: &[&[u8]], params: &ParamSet) -> String {
    let canonical = serde_json::to_vec(params).expect("a map of JSON values serialises");
    let mut hasher = blake3::Hasher::new();
    hasher.update(KEY_DOMAIN);
    let mut parts = vec![lf_only(driver)];
    parts.extend(sources.iter().map(|s| lf_only(s)));
    parts.push(canonical);
    for part in parts {
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
    if let Some(mode) = loaded.recipe.placement.extra_value() {
        extras.insert(PLACEMENT_EXTRA.to_string(), mode.to_string());
    }
    let baked_subtype = loaded
        .recipe
        .params
        .iter()
        .find(|(_, s)| matches!(s, ParamSource::Subtype))
        .and_then(|(name, _)| params.get(name))
        .and_then(|v| v.as_str())
        .map(str::to_string);
    Ok(Entrada {
        id: loaded.piece_id(key),
        familia: loaded.recipe.family.clone(),
        subtipo: loaded.recipe.subtype.clone().or(baked_subtype),
        // A pipe piece is one DN; a manhole has none (each pipe's DN goes in its anchor).
        dn_mm: params
            .get(PIPE_DN_PARAM)
            .and_then(Json::as_u64)
            .map(|d| d as u32)
            .or_else(|| baked_dn_mm(&loaded.recipe, params)),
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

/// The piece's DN when the recipe draws it from the object's `diametro_nominal` (a manhole has
/// none: its pipes carry theirs).
fn baked_dn_mm(recipe: &Recipe, params: &ParamSet) -> Option<u32> {
    recipe
        .params
        .iter()
        .find(|(_, s)| matches!(s, ParamSource::Attribute { key, .. } if key == crate::model::DIAMETRO_NOMINAL))
        .and_then(|(name, _)| params.get(name))
        .and_then(Json::as_f64)
        .filter(|d| d.is_finite() && *d > 0.0)
        .map(|d| d.round() as u32)
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

    fn pipe_recipe() -> LoadedRecipe {
        let contract = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tools/blender/build_pipe.receta.json");
        LoadedRecipe::load(&contract).expect("the pipe recipe loads")
    }

    #[test]
    fn pipe_params_come_from_the_link_and_serve_water_and_sewer() {
        let r = pipe_recipe();
        assert_eq!(r.recipe.unit_mismatches(), Vec::<String>::new());
        let p = crate::demo_cadena::cadena();
        let sewer = p.link_by_code("CL-303").unwrap();
        assert!(r.recipe.applies_to_link(sewer));
        let set = r.recipe.params_for_link(sewer).expect("complete demo pipe");
        assert_eq!(set["dn_mm"], Json::from(200));
        assert_eq!(set["inner_diameter_m"], Json::from(0.2));
        assert_eq!(set["pipe_material"], Json::from("hormigon"));
        assert_eq!(set["system"], Json::from("saneamiento"));
        assert_eq!(
            set["wall_thickness_m"],
            Json::from(sewer.datos["espesor_pared"].valor)
        );
        let water = p.link_by_code("TB-003").unwrap();
        assert!(r.recipe.applies_to_link(water), "otras_familias");
        let water_set = r.recipe.params_for_link(water).unwrap();
        assert_eq!(water_set["system"], Json::from("agua"));
        assert_ne!(r.key(&set), r.key(&water_set));
        assert!(!r.recipe.applies_to(p.node_by_code("PZ-302").unwrap()));
    }

    #[test]
    fn a_pipe_is_baked_with_its_declared_bore_and_never_without_its_data() {
        let r = pipe_recipe();
        let p = crate::demo_cadena::cadena();
        let mut l = p.link_by_code("CL-303").unwrap().clone();
        l.attrs
            .insert("diametro_interior".into(), Value::Num(196.4));
        let set = r.recipe.params_for_link(&l).unwrap();
        assert_eq!(
            set["inner_diameter_m"],
            Json::from(0.1964),
            "declared, in mm"
        );

        l.datos.remove("espesor_pared");
        l.sin_dato.push(DIAMETRO_NOMINAL.into());
        l.d_mm = 0;
        l.attrs.remove("diametro_interior");
        assert_eq!(
            r.recipe.params_for_link(&l).unwrap_err(),
            vec![DIAMETRO_NOMINAL.to_string(), "espesor_pared".to_string()]
        );
    }

    #[test]
    fn a_baked_pipe_piece_records_its_dn() {
        let r = pipe_recipe();
        let p = crate::demo_cadena::cadena();
        let set = r
            .recipe
            .params_for_link(p.link_by_code("CL-303").unwrap())
            .unwrap();
        let glb = crate::gltf::glb_desde_vertices(&BTreeMap::from([(
            "modelo".to_string(),
            [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]
                .map(|pos| crate::gltf::Vertice {
                    pos,
                    normal: [0.0, 0.0, 1.0],
                    color: [0.5, 0.5, 0.5],
                })
                .into_iter()
                .collect(),
        )]))
        .unwrap();
        let entry = baked_entry(&r, &r.key(&set), &set, &glb).unwrap();
        assert_eq!(entry.dn_mm, Some(200));
        assert_eq!(entry.familia, "saneamiento.tuberia");
    }

    #[test]
    fn a_module_the_recipe_imports_is_part_of_the_key() {
        let contract = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tools/blender/build_plant_sedimentator.receta.json");
        let r = LoadedRecipe::load(&contract).expect("the sedimentator recipe loads");
        assert_eq!(r.module_bytes.len(), 1, "plant_units.py");
        let set = ParamSet::from([("length_m".to_string(), Json::from(11.0))]);
        let mut edited = r.clone();
        edited.module_bytes[0].extend_from_slice(b"\n# edited\n");
        assert_ne!(
            r.key(&set),
            edited.key(&set),
            "a module edit invalidates the cache"
        );
        let mut without = r.clone();
        without.module_bytes.clear();
        assert_eq!(
            without.key(&set),
            bake_key(&r.driver_bytes, &r.script_bytes, &set),
            "a recipe without modules keeps its old key"
        );
    }

    fn recipe(name: &str) -> LoadedRecipe {
        let contract = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(format!("../../tools/blender/{name}.receta.json"));
        LoadedRecipe::load(&contract).unwrap_or_else(|e| panic!("{name}: {e}"))
    }

    const FITTING_RECIPES: [&str; 6] = [
        "build_elbow",
        "build_tee",
        "build_reducer",
        "build_valve",
        "build_air_valve",
        "build_hydrant",
    ];

    fn node_at(family: &str, ty: &str, code: &str, x: f64, y: f64) -> Node {
        Node {
            id: crate::model::ElementId::new(0, 0),
            family: family.into(),
            ty: ty.into(),
            code: code.into(),
            pos: crate::geom::P2::new(758_000.0 + x, 9_797_000.0 + y),
            z_terrain: 2500.0,
            z_rim: 2500.0,
            z_invert: 2498.8,
            attrs: BTreeMap::new(),
            datos: BTreeMap::new(),
            estado: crate::model::EstadoObjeto::Pendiente,
            observaciones: Vec::new(),
            modelo: None,
            deleted: false,
            sin_dato: Vec::new(),
        }
    }

    /// A fitting at the origin and one pipe per `(code, towards the fitting, azimuth, DN)`,
    /// 10 m long, with its invert 2498.8 at the fitting.
    fn fitting(family: &str, ty: &str, pipes: &[(&str, bool, f64, u32)]) -> (Project, Node) {
        let mut p = Project::new("F17-05", "EPSG:32717");
        let mut centre = node_at(family, ty, "AX-1", 0.0, 0.0);
        centre
            .attrs
            .insert("diametro_nominal".into(), Value::Int(200));
        centre
            .attrs
            .insert("material".into(), Value::Text("PVC".into()));
        let centre_id = p.add_node(centre).unwrap();
        for (i, (code, incoming, azimuth, dn)) in pipes.iter().enumerate() {
            let (dx, dy) = (
                10.0 * azimuth.to_radians().cos(),
                10.0 * azimuth.to_radians().sin(),
            );
            let far = p
                .add_node(node_at(
                    "agua.arqueta",
                    "medidor",
                    &format!("AQ-{i}"),
                    dx,
                    dy,
                ))
                .unwrap();
            let at = |x: f64, y: f64| crate::geom::P3::new(758_000.0 + x, 9_797_000.0 + y, 2498.8);
            let (from, to, path) = if *incoming {
                (far, centre_id, vec![at(dx, dy), at(0.0, 0.0)])
            } else {
                (centre_id, far, vec![at(0.0, 0.0), at(dx, dy)])
            };
            p.add_link(Link {
                id: crate::model::ElementId::new(0, 0),
                family: "agua.tuberia".into(),
                ty: "conduccion".into(),
                code: code.to_string(),
                from,
                to,
                path,
                d_mm: *dn,
                attrs: BTreeMap::new(),
                datos: BTreeMap::new(),
                estado: crate::model::EstadoObjeto::Pendiente,
                observaciones: Vec::new(),
                cota_solera_inicio: None,
                cota_solera_fin: None,
                modelo: None,
                deleted: false,
                sin_dato: Vec::new(),
            })
            .unwrap();
        }
        let n = p.node(centre_id).unwrap().clone();
        (p, n)
    }

    #[test]
    fn the_fitting_recipes_load_and_agree_with_the_registry() {
        for name in FITTING_RECIPES {
            let r = recipe(name);
            assert_eq!(r.recipe.unit_mismatches(), Vec::<String>::new(), "{name}");
            assert_eq!(r.module_bytes.len(), 1, "{name} imports redes_mallas.py");
            assert_ne!(r.recipe.placement, Placement::Object, "{name}");
        }
    }

    #[test]
    fn an_elbow_turns_with_its_pipes_and_equal_elbows_share_one_piece() {
        let r = recipe("build_elbow");
        // Heading east then north, and heading north then west: both a 90° left turn.
        let (p1, east) = fitting(
            "comun.accesorio",
            "codo",
            &[("TB-1", true, 180.0, 200), ("TB-2", false, 90.0, 200)],
        );
        let (p2, north) = fitting(
            "comun.accesorio",
            "codo",
            &[("TB-7", true, -90.0, 200), ("TB-8", false, 180.0, 200)],
        );
        let a = r.recipe.params_for_node(&p1, &east).unwrap();
        let b = r.recipe.params_for_node(&p2, &north).unwrap();
        assert_eq!(
            a["ports"], b["ports"],
            "codes and heading are not in the piece"
        );
        assert_eq!(r.key(&a), r.key(&b));
        assert_eq!(a["ports"][0]["name"], "entrada__1");
        assert_eq!(a["ports"][1]["azimuth_deg"], Json::from(90.0));

        let f1 = node_port_frame(&p1, &east).unwrap();
        let f2 = node_port_frame(&p2, &north).unwrap();
        assert!(f1.rotation_deg.abs() < 1e-9, "{f1:?}");
        assert!((f2.rotation_deg - 90.0).abs() < 1e-9, "{f2:?}");
        assert_eq!(f2.ports[1].link, "TB-8");
        assert!((f1.reference_invert_m - 2498.8).abs() < 1e-9);
    }

    #[test]
    fn a_bend_is_drawn_to_the_recipe_step() {
        let r = recipe("build_elbow");
        let (p, n) = fitting(
            "comun.accesorio",
            "codo",
            &[("TB-1", true, 180.0, 200), ("TB-2", false, 44.7, 200)],
        );
        let set = r.recipe.params_for_node(&p, &n).unwrap();
        assert_eq!(set["ports"][1]["azimuth_deg"], Json::from(45.0));
    }

    #[test]
    fn the_reference_of_a_tee_is_on_its_run_whatever_the_codes() {
        let r = recipe("build_tee");
        // The branch sorts first by code; the run is TB-5 (in) and TB-6 (out).
        let (p, mut n) = fitting(
            "comun.accesorio",
            "tee",
            &[
                ("TB-0", false, 90.0, 100),
                ("TB-5", true, 180.0, 200),
                ("TB-6", false, 0.0, 200),
            ],
        );
        n.attrs.insert("dn_ramal".into(), Value::Int(100));
        let frame = node_port_frame(&p, &n).unwrap();
        let links: Vec<&str> = frame.ports.iter().map(|q| q.link.as_str()).collect();
        assert_eq!(links, ["TB-5", "TB-6", "TB-0"]);
        let set = r.recipe.params_for_node(&p, &n).unwrap();
        let names: Vec<&str> = set["ports"]
            .as_array()
            .unwrap()
            .iter()
            .map(|q| q["name"].as_str().unwrap())
            .collect();
        assert_eq!(names, ["entrada__1", "salida__2", "salida__3"]);
        assert_eq!(set["ports"][2]["dn_mm"], Json::from(100));
    }

    #[test]
    fn a_fitting_without_its_pipes_keeps_the_fallback_and_says_why() {
        let r = recipe("build_valve");
        let (p, n) = fitting("agua.valvula", "compuerta", &[("TB-1", true, 180.0, 200)]);
        assert!(r.recipe.applies_to(&n));
        let missing = r.recipe.params_for_node(&p, &n).unwrap_err();
        assert_eq!(
            missing,
            vec!["conexiones (1 tramos conectados; la pieza tiene 2 bocas)".to_string()]
        );

        let (mut p, n) = fitting(
            "agua.valvula",
            "compuerta",
            &[("TB-1", true, 180.0, 200), ("TB-2", false, 0.0, 200)],
        );
        assert!(r.recipe.params_for_node(&p, &n).is_ok());
        for l in p.links.iter_mut() {
            l.sin_dato.push(COTA_SOLERA.into());
        }
        let missing = r.recipe.params_for_node(&p, &n).unwrap_err();
        assert!(
            missing[0].contains("TB-1: el tramo no trae cota"),
            "{missing:?}"
        );
    }

    #[test]
    fn one_valve_recipe_draws_the_subtypes_it_lists_and_names_them() {
        let r = recipe("build_valve");
        for (ty, drawn) in [
            ("compuerta", true),
            ("mariposa", true),
            ("retencion", true),
            ("aire", false),
            ("purga", false),
        ] {
            let n = node_at("agua.valvula", ty, "VL-1", 0.0, 0.0);
            assert_eq!(r.recipe.applies_to(&n), drawn, "{ty}");
        }
        let (p, mut n) = fitting(
            "agua.valvula",
            "mariposa",
            &[("TB-1", true, 180.0, 150), ("TB-2", false, 0.0, 150)],
        );
        n.attrs
            .insert("material".into(), Value::Text("hierro_ductil".into()));
        let set = r.recipe.params_for_node(&p, &n).unwrap();
        assert_eq!(set["subtype"], Json::from("mariposa"));
    }

    #[test]
    fn an_air_valve_needs_the_dn_of_the_main_it_sits_on() {
        let r = recipe("build_air_valve");
        let (mut p, n) = fitting(
            "agua.valvula",
            "aire",
            &[("TB-1", true, 180.0, 200), ("TB-2", false, 0.0, 200)],
        );
        assert!(r.recipe.params_for_node(&p, &n).is_ok());
        p.links[0]
            .sin_dato
            .push(crate::model::DIAMETRO_NOMINAL.into());
        let missing = r.recipe.params_for_node(&p, &n).unwrap_err();
        assert!(missing[0].contains("el tramo no trae DN"), "{missing:?}");
    }

    #[test]
    fn a_port_framed_piece_turns_with_its_pipes_and_sits_on_the_pipe_invert() {
        let (p, mut n) = fitting(
            "comun.accesorio",
            "codo",
            &[("TB-1", true, -90.0, 200), ("TB-2", false, 180.0, 200)],
        );
        // The object's own elevations disagree with its pipes: the piece follows the pipes.
        n.z_invert = 2497.0;
        n.attrs.insert("giro".into(), Value::Num(33.0));
        let mut piece = Entrada {
            id: "comun.accesorio.codo-x".into(),
            familia: "comun.accesorio".into(),
            subtipo: Some("codo".into()),
            dn_mm: Some(200),
            archivo: String::new(),
            escala: 1.0,
            triangulos: 0,
            dimensiones_m: [0.0; 3],
            bytes: 0,
            volumen_m3: None,
            area_m2: None,
            cerrada: None,
            anclas: BTreeMap::from([(biblioteca::ANCLA_SOLERA.to_string(), [0.0f32, 0.0, 0.0])]),
            extras: BTreeMap::new(),
            licencia: None,
            autor: None,
        };
        let by_object = model_placement(&p, &n, &piece);
        assert_eq!(by_object.rotation_deg, 33.0, "no frame extra: the giro");
        assert!((by_object.base_m - 2497.0).abs() < 1e-9);

        piece
            .extras
            .insert(PLACEMENT_EXTRA.into(), "puertos".into());
        let on_pipe = model_placement(&p, &n, &piece);
        assert!((on_pipe.rotation_deg - 90.0).abs() < 1e-9, "{on_pipe:?}");
        assert!((on_pipe.base_m - 2498.8).abs() < 1e-9, "{on_pipe:?}");

        piece
            .extras
            .insert(PLACEMENT_EXTRA.into(), "puertos_en_objeto".into());
        let on_object = model_placement(&p, &n, &piece);
        assert!((on_object.rotation_deg - 90.0).abs() < 1e-9);
        assert!((on_object.base_m - 2497.0).abs() < 1e-9);
    }

    /// The chain demo's in-line valves share one piece, even where the pipe turns at the valve:
    /// only the valve at the end of a line (one pipe) keeps its fallback piece.
    #[test]
    fn the_chain_demo_valves_bake_to_few_pieces() {
        let p = crate::demo_cadena::cadena();
        let gate = recipe("build_valve");
        let mut keys = std::collections::BTreeSet::new();
        let mut lacking = Vec::new();
        for n in p.nodes.iter().filter(|n| gate.recipe.applies_to(n)) {
            match gate.recipe.params_for_node(&p, n) {
                Ok(set) => {
                    keys.insert(gate.key(&set));
                }
                Err(m) => lacking.push((n.code.clone(), m)),
            }
        }
        assert_eq!(lacking.len(), 1, "{lacking:?}");
        assert!(lacking[0].1[0].starts_with(PORTS_MISSING), "{lacking:?}");
        assert_eq!(keys.len(), 1, "eleven valves, one piece");
        let air = recipe("build_air_valve");
        let vl001 = p.node_by_code("VL-001").unwrap();
        assert!(air.recipe.params_for_node(&p, vl001).is_ok());
    }

    #[test]
    fn editing_the_shared_mesh_module_invalidates_the_fitting_pieces() {
        let r = recipe("build_elbow");
        let set = ParamSet::from([("dn_mm".to_string(), Json::from(200))]);
        let mut edited = r.clone();
        edited.module_bytes[0].extend_from_slice(b"\n# edited\n");
        assert_ne!(
            r.key(&set),
            edited.key(&set),
            "redes_mallas.py is in the key"
        );
    }

    #[test]
    fn a_contract_must_be_named_after_its_script() {
        let err = LoadedRecipe::load(Path::new("tools/blender/build_manhole.json")).unwrap_err();
        assert!(err.starts_with("E_RECETA_INVALIDA"), "{err}");
    }
}
