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
    /// CRS declarado (`EPSG:327NN`, UTM sur). Obligatorio en la práctica: vacío → `E_CRS`
    /// (F12-13). Antes se suponía EPSG:32717 en silencio.
    #[serde(default)]
    pub crs: String,
    #[serde(default)]
    pub fuente_app: String,
    #[serde(default)]
    pub fuente_archivo: String,
}

// ---------------------------------------------------------------------------------------------
// Sistema de coordenadas (F12-13): se declara, se comprueba y no se reproyecta nunca.

/// Código EPSG de un CRS escrito como `EPSG:32717` (o solo `32717`).
pub fn epsg_de(crs: &str) -> Option<u32> {
    let t = crs.trim();
    let num = t
        .get(..5)
        .filter(|p| p.eq_ignore_ascii_case("EPSG:"))
        .map(|_| &t[5..])
        .unwrap_or(t);
    num.trim().parse().ok()
}

/// ¿Es un UTM sur WGS 84 (`EPSG:32701`…`32760`)? Redes trabaja en metros UTM; 17S (32717) y 18S
/// (32718) son los de Ecuador y Perú.
pub fn es_utm_sur(epsg: u32) -> bool {
    (32701..=32760).contains(&epsg)
}

/// ¿Parecen grados (lon/lat) y no metros UTM? Un UTM tiene E de cientos de miles y N de millones.
fn parece_lonlat(e: f64, n: f64) -> bool {
    e.abs() <= 180.0 && n.abs() <= 90.0
}

/// Comprueba el CRS de lo que entra contra el del proyecto. Devuelve el EPSG a usar.
///
/// - CRS ausente, ilegible o que no es UTM sur → `E_CRS`.
/// - Distinto del proyecto → `E_CRS` (no se reproyecta), salvo que el proyecto esté vacío: entonces
///   adopta el del plan, porque no hay nada que desalinear.
/// - Un terreno ya cargado con otro EPSG → `E_CRS`.
fn comprobar_crs(p: &Project, crs: &str, origen: &str) -> Result<u32, String> {
    if crs.trim().is_empty() {
        return Err(format!(
            "E_CRS: {origen} no declara su sistema de coordenadas (p. ej. \"EPSG:32717\")"
        ));
    }
    let Some(epsg) = epsg_de(crs) else {
        return Err(format!("E_CRS: {origen}: CRS ilegible «{crs}»"));
    };
    if !es_utm_sur(epsg) {
        return Err(format!(
            "E_CRS: {origen} está en EPSG:{epsg}; Redes solo admite UTM sur WGS 84 (EPSG:327NN, p. ej. 32717 = 17S, 32718 = 18S). Si es 326NN es el hemisferio norte"
        ));
    }
    let vacio = p.count() == 0 && p.terrain.is_none();
    if !vacio && epsg_de(&p.crs) != Some(epsg) {
        return Err(format!(
            "E_CRS: {origen} está en EPSG:{epsg} y el proyecto en {}; Redes no reproyecta",
            p.crs
        ));
    }
    if let Some(t) = &p.terrain
        && t.epsg != epsg
    {
        return Err(format!(
            "E_CRS: el terreno del proyecto está en EPSG:{} y {origen} en EPSG:{epsg}",
            t.epsg
        ));
    }
    Ok(epsg)
}

/// Rechaza coordenadas en grados: un GeoJSON o un Excel en lon/lat que llega como si fuera UTM
/// quedaría a miles de kilómetros (o todo en un punto).
fn rechazar_lonlat<'a>(
    puntos: impl Iterator<Item = (&'a str, f64, f64)>,
    origen: &str,
) -> Result<(), String> {
    let malos: Vec<String> = puntos
        .filter(|(_, e, n)| parece_lonlat(*e, *n))
        .map(|(que, e, n)| format!("{que} ({e}, {n})"))
        .take(3)
        .collect();
    if malos.is_empty() {
        return Ok(());
    }
    Err(format!(
        "E_CRS: {origen} trae coordenadas que parecen grados lon/lat, no metros UTM: {}. Proyéctelas a UTM en origen; Redes no reproyecta",
        malos.join(", ")
    ))
}

/// Un objeto del plan. Los nombres de campo son el contrato: uno desconocido se rechaza con
/// `E_FMT_CAMPO` antes de leer nada (F12-15), porque antes se perdía en silencio.
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
        /// Cota de solera (m).
        #[serde(default)]
        cota: Option<f64>,
        #[serde(default)]
        cota_terreno: Option<f64>,
        /// Cota de tapa. Si falta, queda ausente: no se copia el terreno (F12-03).
        #[serde(default)]
        cota_tapa: Option<f64>,
        /// Cota del eje de la tubería en el nudo (m), la de una red a presión (`Elev` de un
        /// nudo de EPANET). No es solera: va a `datos.cota_eje` y solo sirve para ubicar el
        /// nodo en altura (F12-15).
        #[serde(default)]
        cota_eje: Option<f64>,
        /// Diámetro interior del pozo o de la estructura, en **metros**.
        #[serde(default)]
        diametro: Option<f64>,
        /// Dispositivo de enlace (bomba, válvula, vertedero): nodos del origen que une sin tramo
        /// propio (F12-15). Ver [`crate::model::Enlace`].
        #[serde(default)]
        desde: Option<String>,
        #[serde(default)]
        hasta: Option<String>,
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
        /// Diámetro **nominal** (DN), mm.
        #[serde(default)]
        diametro_mm: Option<u32>,
        /// Diámetro **interior** (mm), el que dan SWMM o EPANET. Va a `attrs.diametro_interior`.
        #[serde(default)]
        diametro_interior_mm: Option<f64>,
        /// Forma de la sección (lista `seccion` de la familia). Va a `attrs.seccion`.
        #[serde(default)]
        seccion: Option<String>,
        /// Alto y ancho interiores de una sección no circular (m). Van a `datos.altura_seccion`
        /// y `datos.ancho_seccion`; nunca se convierten en un DN.
        #[serde(default)]
        alto_m: Option<f64>,
        #[serde(default)]
        ancho_m: Option<f64>,
        /// Solera (fondo interior) del tramo en su inicio y su fin, si el origen la declara.
        #[serde(default)]
        cota_solera_inicio: Option<f64>,
        #[serde(default)]
        cota_solera_fin: Option<f64>,
        /// Vértices `[E,N,Z]` (Z = solera) o `[E,N]` (sin Z: se interpola, ver F12-15).
        #[serde(default)]
        vertices: Vec<Vec<f64>>,
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

// ---------------------------------------------------------------------------------------------
// Campos del contrato (F12-15). Un campo que no está aquí se rechaza: antes se perdía en silencio.

const CAMPOS_RAIZ: &[&str] = &["schema", "proyecto", "terreno", "objetos"];
const CAMPOS_PROYECTO: &[&str] = &["nombre", "crs", "fuente_app", "fuente_archivo"];
const CAMPOS_NODO: &[&str] = &[
    "tipo",
    "codigo",
    "familia",
    "subtipo",
    "e",
    "n",
    "cota",
    "cota_terreno",
    "cota_tapa",
    "cota_eje",
    "diametro",
    "desde",
    "hasta",
    "attrs",
    "datos",
];
const CAMPOS_ARCO: &[&str] = &[
    "tipo",
    "codigo",
    "familia",
    "subtipo",
    "desde",
    "hasta",
    "diametro_mm",
    "diametro_interior_mm",
    "seccion",
    "alto_m",
    "ancho_m",
    "cota_solera_inicio",
    "cota_solera_fin",
    "vertices",
    "attrs",
    "datos",
];
const CAMPOS_DATO: &[&str] = &["valor", "unidad", "fuente"];

/// Nombres que un agente escribe a menudo en lugar del del contrato. La distancia de edición no
/// los encuentra (`trazado` no se parece a `vertices`).
const SINONIMOS: &[(&str, &str)] = &[
    ("trazado", "vertices"),
    ("geometria", "vertices"),
    ("coordenadas", "vertices"),
    ("puntos", "vertices"),
    ("polilinea", "vertices"),
    ("dn", "diametro_mm"),
    ("diametro_nominal", "diametro_mm"),
    ("di", "diametro_interior_mm"),
    ("diametro_interior", "diametro_interior_mm"),
    ("solera", "cota"),
    ("cota_solera", "cota"),
    ("cota_fondo", "cota"),
    ("rasante", "cota_terreno"),
    ("cota_rasante", "cota_terreno"),
    ("terreno", "cota_terreno"),
    ("tapa", "cota_tapa"),
    ("eje", "cota_eje"),
    ("x", "e"),
    ("y", "n"),
    ("este", "e"),
    ("norte", "n"),
    ("inicio", "desde"),
    ("fin", "hasta"),
    ("nodo_inicio", "desde"),
    ("nodo_fin", "hasta"),
    ("from", "desde"),
    ("to", "hasta"),
    ("id", "codigo"),
    ("nombre", "codigo"),
    ("forma", "seccion"),
    ("alto", "alto_m"),
    ("altura", "alto_m"),
    ("ancho", "ancho_m"),
    ("atributos", "attrs"),
    ("unidades", "unidad"),
];

/// Distancia de edición (Levenshtein) entre dos textos cortos.
fn distancia(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut fila: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut diag = fila[0];
        fila[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let arriba = fila[j + 1];
            fila[j + 1] = (diag + usize::from(ca != *cb))
                .min(arriba + 1)
                .min(fila[j] + 1);
            diag = arriba;
        }
    }
    fila[b.len()]
}

/// El campo válido más parecido a `campo`, si lo hay.
fn sugerencia(campo: &str, validos: &[&'static str]) -> Option<&'static str> {
    let c = campo.to_lowercase();
    if let Some((_, s)) = SINONIMOS.iter().find(|(k, _)| *k == c) {
        // Solo si el sinónimo es válido aquí (`nombre` es válido en `proyecto`, no en un objeto).
        return validos.iter().find(|v| *v == s).copied();
    }
    let tope = (c.chars().count() / 3).max(2);
    validos
        .iter()
        .map(|v| (distancia(&c, v), *v))
        .filter(|(d, _)| *d <= tope)
        .min_by_key(|(d, _)| *d)
        .map(|(_, v)| v)
}

/// Comprueba que el plan solo usa campos del contrato. Devuelve `E_FMT_CAMPO` con **todos** los
/// campos desconocidos, el objeto donde están y el nombre válido más parecido.
fn revisar_campos(v: &serde_json::Value) -> Result<(), String> {
    use serde_json::Value as J;
    let mut malos: Vec<String> = Vec::new();
    let mut revisa = |donde: &str, obj: &serde_json::Map<String, J>, validos: &[&'static str]| {
        for k in obj.keys().filter(|k| !validos.contains(&k.as_str())) {
            let pista = match sugerencia(k, validos) {
                Some(s) => format!(" (¿quiso decir «{s}»?)"),
                None => format!(" (válidos: {})", validos.join(", ")),
            };
            malos.push(format!("{donde}: «{k}»{pista}"));
        }
    };
    let Some(raiz) = v.as_object() else {
        return Ok(()); // serde dirá qué falla
    };
    revisa("raíz", raiz, CAMPOS_RAIZ);
    if let Some(pr) = raiz.get("proyecto").and_then(J::as_object) {
        revisa("proyecto", pr, CAMPOS_PROYECTO);
    }
    for (i, o) in raiz
        .get("objetos")
        .and_then(J::as_array)
        .into_iter()
        .flatten()
        .enumerate()
    {
        let Some(o) = o.as_object() else { continue };
        let tipo = o.get("tipo").and_then(J::as_str).unwrap_or("");
        let codigo = o.get("codigo").and_then(J::as_str).unwrap_or("");
        let donde = format!("objetos[{i}] ({tipo} {codigo})");
        let validos = match tipo {
            "nodo" => CAMPOS_NODO,
            "arco" => CAMPOS_ARCO,
            _ => continue, // tipo desconocido: lo rechaza serde con su nombre
        };
        revisa(&donde, o, validos);
        for (k, d) in o.get("datos").and_then(J::as_object).into_iter().flatten() {
            if let Some(d) = d.as_object() {
                revisa(&format!("{donde}.datos.{k}"), d, CAMPOS_DATO);
            }
        }
    }
    if malos.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "E_FMT_CAMPO: el plan usa campos que no son del contrato redes.plan/1 y se perderían: {}",
            malos.join("; ")
        ))
    }
}

// ---------------------------------------------------------------------------------------------
// Unidades (F12-15): se avisa de lo que parece un error de unidad; nunca se corrige.

/// Diámetro de un pozo o estructura (m) mayor que esto: casi seguro está en mm.
const DIAMETRO_NODO_MAX_M: f64 = 20.0;
/// DN o DI de un tubo (mm) menor que esto: casi seguro está en m o en pulgadas.
const DIAMETRO_TUBO_MIN_MM: f64 = 10.0;
/// Alto o ancho de una sección (m) mayor que esto: casi seguro está en mm.
const SECCION_MAX_M: f64 = 20.0;

fn aviso_diametro_nodo(rep: &mut ImportReport, codigo: &str, campo: &str, d: f64) {
    if d > DIAMETRO_NODO_MAX_M {
        rep.avisos.push(format!(
            "nodo {codigo}: {campo} = {d} m es mayor que {DIAMETRO_NODO_MAX_M} m; ¿está en mm? El diámetro del nodo va en metros (se importa tal cual)"
        ));
    }
}

fn aviso_diametro_tubo(rep: &mut ImportReport, codigo: &str, campo: &str, d: f64) {
    if d > 0.0 && d < DIAMETRO_TUBO_MIN_MM {
        rep.avisos.push(format!(
            "arco {codigo}: {campo} = {d} mm es menor que {DIAMETRO_TUBO_MIN_MM} mm; ¿está en m o en pulgadas? El diámetro del tubo va en mm (se importa tal cual)"
        ));
    }
}

/// Opciones de un atributo de lista de una familia, si lo es.
fn opciones(familia: &str, attr: &str) -> Option<&'static [&'static str]> {
    match crate::families::family(familia)?.attr(attr)?.ty {
        crate::families::AttrType::Enum(v) => Some(v),
        _ => None,
    }
}

/// Vértices del plan: todos `[E,N,Z]`, todos `[E,N]` o error.
enum Vertices {
    Con(Vec<P3>),
    SinZ(Vec<P2>),
}

fn leer_vertices(v: &[Vec<f64>]) -> Result<Vertices, String> {
    if v.iter().flatten().any(|c| !c.is_finite()) {
        return Err("vértice con una coordenada no finita".into());
    }
    if v.iter().all(|c| c.len() == 3) {
        Ok(Vertices::Con(
            v.iter().map(|c| P3::new(c[0], c[1], c[2])).collect(),
        ))
    } else if v.iter().all(|c| c.len() == 2) {
        Ok(Vertices::SinZ(
            v.iter().map(|c| P2::new(c[0], c[1])).collect(),
        ))
    } else {
        Err("los vértices deben ser todos [E,N,Z] o todos [E,N] (sin Z)".into())
    }
}

/// Z por longitud en planta entre `z0` y `z1` (geometría de dibujo, no dato).
fn interpolar_z(planta: &[P2], z0: f64, z1: f64) -> Vec<P3> {
    let mut acum = vec![0.0];
    for w in planta.windows(2) {
        acum.push(acum.last().unwrap() + w[0].dist(w[1]));
    }
    let total = *acum.last().unwrap();
    planta
        .iter()
        .zip(&acum)
        .map(|(q, s)| {
            let t = if total > 0.0 { s / total } else { 0.0 };
            P3::new(q.x, q.y, z0 + (z1 - z0) * t)
        })
        .collect()
}

/// Importa un plan JSON y lo aplica al proyecto.
///
/// Para validar sin escribir (`--dry-run`), se llama sobre una copia del proyecto: el informe es
/// el mismo que el de la importación real.
pub fn import_plan(p: &mut Project, path: &Path) -> Result<ImportReport, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("E_IO_NOT_FOUND: {e}"))?;
    let crudo: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("E_FMT_PARSE: {e}"))?;
    revisar_campos(&crudo)?;
    let plan: Plan = serde_json::from_str(&text).map_err(|e| format!("E_FMT_PARSE: {e}"))?;
    if !plan.schema.starts_with("redes.plan/") {
        return Err(format!("E_FMT_UNSUPPORTED: esquema {}", plan.schema));
    }
    // F12-13: el CRS se comprueba antes de tocar nada.
    let epsg = comprobar_crs(p, &plan.proyecto.crs, "el plan")?;
    let terreno = plan
        .terreno
        .iter()
        .map(|v| ("punto de terreno", v[0], v[1]));
    let objetos = plan.objetos.iter().flat_map(|o| match o {
        PlanObjeto::Nodo { codigo, e, n, .. } => vec![(codigo.as_str(), *e, *n)],
        PlanObjeto::Arco {
            codigo, vertices, ..
        } => vertices
            .iter()
            .filter(|v| v.len() >= 2)
            .map(|v| (codigo.as_str(), v[0], v[1]))
            .collect(),
    });
    rechazar_lonlat(terreno.chain(objetos), "el plan")?;
    let mut rep = ImportReport {
        formato: "plan".into(),
        ..Default::default()
    };
    if epsg_de(&p.crs) != Some(epsg) {
        rep.avisos.push(format!(
            "el proyecto estaba vacío y adopta el CRS del plan: EPSG:{epsg} (antes {})",
            p.crs
        ));
    }
    // Fuente de los `datos` que salen de un campo del plan (cota_eje, alto_m…).
    let fuente_plan = {
        let app = if plan.proyecto.fuente_app.is_empty() {
            "plan"
        } else {
            plan.proyecto.fuente_app.as_str()
        };
        if plan.proyecto.fuente_archivo.is_empty() {
            app.to_string()
        } else {
            format!("{app} ({})", plan.proyecto.fuente_archivo)
        }
    };
    let dato_m = |valor: f64| Dato {
        valor,
        unidad: "m".into(),
        fuente: fuente_plan.clone(),
    };

    if !plan.terreno.is_empty() {
        let pts = plan
            .terreno
            .iter()
            .map(|v| SurfacePoint::new(v[0], v[1], v[2]))
            .collect::<Vec<_>>();
        rep.puntos_terreno = pts.len();
        p.terrain = Some(Terrain::build(&plan.proyecto.nombre, epsg, pts));
    }

    // Índice de códigos para resolver `desde`/`hasta`.
    let mut codes: BTreeMap<String, ElementId> = BTreeMap::new();
    // Dispositivos de enlace por resolver cuando estén todos los nodos.
    let mut por_enlazar: Vec<(ElementId, String, Option<String>, Option<String>)> = Vec::new();
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
            cota_eje,
            diametro,
            desde,
            hasta,
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
            // La cota del eje (red a presión) solo ubica el nodo cuando no hay otra: no es
            // solera, ni terreno, ni tapa (F12-15).
            let Some(dibujo) = zr.or(zt).or(zs).or(*cota_eje) else {
                rep.descartados.push(format!(
                    "nodo {codigo}: sin ninguna cota (solera, tapa, terreno ni eje, y sin TIN en su planta): no se puede ubicar en altura"
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
                    match (plano, zr.or(zt).or(zs).is_none()) {
                        (true, true) => "; se dibuja plano, a la cota del eje",
                        (true, false) => "; se dibuja plano, sin altura",
                        _ => "",
                    }
                ));
            }
            let mut a = attrs.clone();
            if let Some(d) = diametro {
                aviso_diametro_nodo(&mut rep, codigo, "diametro", *d);
                a.insert("diametro_interior".to_string(), Value::Num(*d));
            } else if let Some(d) = a.get("diametro_interior").and_then(Value::as_f64) {
                aviso_diametro_nodo(&mut rep, codigo, "attrs.diametro_interior", d);
            }
            let mut ds = to_datos(datos);
            if let Some(z) = cota_eje {
                ds.insert("cota_eje".to_string(), dato_m(*z));
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
                datos: ds,
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
                    codes.insert(code.clone(), id);
                    rep.nodos += 1;
                    if desde.is_some() || hasta.is_some() {
                        por_enlazar.push((id, code, desde.clone(), hasta.clone()));
                    }
                }
                Err(e) => rep.descartados.push(format!("nodo {codigo}: {e}")),
            }
        }
    }

    // Dispositivos de enlace (F12-15): la bomba o la válvula une sus nodos sin tramo inventado.
    for (id, code, desde, hasta) in por_enlazar {
        let mut resuelve = |lado: &str, c: Option<String>| -> Option<ElementId> {
            let c = c?;
            let otro = codes
                .get(&c)
                .copied()
                .or_else(|| p.node_by_code(&c).map(|n| n.id));
            match otro {
                Some(o) if o == id => {
                    rep.avisos.push(format!(
                        "nodo {code}: {lado} «{c}» es el propio nodo; se ignora"
                    ));
                    None
                }
                Some(o) => Some(o),
                None => {
                    rep.avisos.push(format!(
                        "nodo {code}: {lado} «{c}» no existe; el dispositivo queda sin unir por ese lado"
                    ));
                    None
                }
            }
        };
        let (d, h) = (resuelve("desde", desde), resuelve("hasta", hasta));
        if d.is_some() || h.is_some() {
            p.enlaces.push(crate::model::Enlace {
                dispositivo: id,
                desde: d,
                hasta: h,
            });
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
            diametro_interior_mm,
            seccion,
            alto_m,
            ancho_m,
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
            let verts = match leer_vertices(vertices) {
                Ok(v) => v,
                Err(e) => {
                    rep.descartados.push(format!("arco {codigo}: {e}"));
                    continue;
                }
            };
            let mut a_attrs = attrs.clone();
            if let Some(s) = seccion {
                match opciones(familia, "seccion") {
                    Some(lista) if lista.contains(&s.as_str()) => {
                        a_attrs.insert("seccion".into(), Value::Text(s.clone()));
                    }
                    Some(lista) => {
                        rep.descartados.push(format!(
                            "arco {codigo}: seccion «{s}» no está en la lista de {familia}: {}",
                            lista.join(", ")
                        ));
                        continue;
                    }
                    None => {
                        rep.descartados.push(format!(
                            "arco {codigo}: la familia {familia} no tiene atributo «seccion»"
                        ));
                        continue;
                    }
                }
            }
            let (a, b) = (p.node(from).unwrap(), p.node(to).unwrap());
            let mut sin_dato = Vec::new();
            // La solera declarada del tramo manda sobre la del pozo (F12-02). Si no hay ninguna
            // de las dos, el trazado va a la cota de dibujo del pozo y se marca.
            let z0 = cota_solera_inicio.or(a.cota_solera());
            let z1 = cota_solera_fin.or(b.cota_solera());
            let (za, zb) = (z0.unwrap_or(a.z_invert), z1.unwrap_or(b.z_invert));
            let path = match verts {
                Vertices::Con(v) if v.len() >= 2 => v,
                Vertices::SinZ(planta) if planta.len() >= 2 => {
                    // F12-15: la Z de los vértices se interpola por longitud entre las soleras de
                    // los extremos. Es geometría de dibujo, no dato.
                    if z0.is_none() || z1.is_none() {
                        sin_dato.push(crate::model::COTA_SOLERA.to_string());
                    } else if planta.len() > 2 {
                        sin_dato.push(crate::model::COTA_VERTICES.to_string());
                        rep.avisos.push(format!(
                            "arco {codigo}: Z de {} vértices intermedios interpolada por longitud entre las soleras (dibujo, no dato)",
                            planta.len() - 2
                        ));
                    }
                    interpolar_z(&planta, za, zb)
                }
                _ => {
                    if z0.is_none() || z1.is_none() {
                        sin_dato.push(crate::model::COTA_SOLERA.to_string());
                    }
                    vec![P3::new(a.pos.x, a.pos.y, za), P3::new(b.pos.x, b.pos.y, zb)]
                }
            };
            let dn = diametro_mm.filter(|d| *d > 0);
            if dn.is_none() {
                sin_dato.push(crate::model::DIAMETRO_NOMINAL.to_string());
            }
            if let Some(d) = dn {
                aviso_diametro_tubo(&mut rep, codigo, "diametro_mm", d as f64);
            }
            if let Some(di) = diametro_interior_mm {
                aviso_diametro_tubo(&mut rep, codigo, "diametro_interior_mm", *di);
                if let Some(otro) = attrs.get("diametro_interior").and_then(Value::as_f64)
                    && (otro - di).abs() > 1e-9
                {
                    rep.avisos.push(format!(
                        "arco {codigo}: diametro_interior_mm ({di}) y attrs.diametro_interior ({otro}) no coinciden; se usa diametro_interior_mm"
                    ));
                }
                a_attrs.insert("diametro_interior".into(), Value::Num(*di));
            } else if let Some(di) = attrs.get("diametro_interior").and_then(Value::as_f64) {
                aviso_diametro_tubo(&mut rep, codigo, "attrs.diametro_interior", di);
            }
            let mut ds = to_datos(datos);
            for (campo, clave, v) in [
                ("alto_m", "altura_seccion", alto_m),
                ("ancho_m", "ancho_seccion", ancho_m),
            ] {
                if let Some(v) = v {
                    if *v > SECCION_MAX_M {
                        rep.avisos.push(format!(
                            "arco {codigo}: {campo} = {v} m es mayor que {SECCION_MAX_M} m; ¿está en mm? (se importa tal cual)"
                        ));
                    }
                    ds.insert(clave.to_string(), dato_m(*v));
                }
            }
            if !sin_dato.is_empty() {
                let de_dato: Vec<&str> = sin_dato
                    .iter()
                    .map(String::as_str)
                    .filter(|k| *k != crate::model::COTA_VERTICES)
                    .collect();
                if !de_dato.is_empty() {
                    rep.avisos.push(format!(
                        "arco {codigo}: sin {} (se muestra «—» y se dibuja como línea sin dato)",
                        de_dato.join(", ")
                    ));
                }
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
                attrs: a_attrs,
                datos: ds,
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
    p.crs = format!("EPSG:{epsg}");
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
        // Soleras del tramo: campos 7 y 8 si la línea los trae (vacío = ausente); si no, la regla
        // del plan: la del pozo del extremo. Sin ninguna de las dos, las Z del trazado son de
        // dibujo y el tramo vuelve sin cotas de dato, como salió (F12-15).
        let solera = |i: usize| {
            campos
                .get(i)
                .and_then(|s| s.trim().parse::<f64>().ok())
                .filter(|v| v.is_finite())
        };
        let (si, sf) = (solera(7), solera(8));
        let de_pozo = |id: ElementId| p.node(id).and_then(|n| n.cota_solera());
        let sin_solera = si.or(de_pozo(desde)).is_none() || sf.or(de_pozo(hasta)).is_none();
        let mut sin_dato = Vec::new();
        if sin_solera {
            sin_dato.push(crate::model::COTA_SOLERA.to_string());
        }
        if dn.is_none() {
            sin_dato.push(crate::model::DIAMETRO_NOMINAL.to_string());
        }
        if !sin_dato.is_empty() {
            rep.avisos
                .push(format!("arco {code}: sin {}", sin_dato.join(", ")));
        }
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
            cota_solera_inicio: si,
            cota_solera_fin: sf,
            deleted: false,
            sin_dato,
        };
        match p.add_link(link) {
            Ok(_) => arcos_creados += 1,
            Err(e) => rep.descartados.push(format!("arco {code}: {e}")),
        }
    }

    // Dispositivos de enlace (F12-15): `REDES|1|E|dispositivo|desde|hasta`. Un DXF anterior no
    // trae estas líneas y abre igual.
    for t in textos {
        let Some(resto) = t.strip_prefix("REDES|1|E|") else {
            continue;
        };
        let campos: Vec<&str> = resto.split('|').collect();
        let id = |i: usize| campos.get(i).and_then(|c| ids.get(c.trim())).copied();
        let Some(disp) = id(0) else {
            continue;
        };
        let (desde, hasta) = (id(1), id(2));
        if desde.is_some() || hasta.is_some() {
            p.enlaces.push(crate::model::Enlace {
                dispositivo: disp,
                desde,
                hasta,
            });
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
    // F12-13: EPSG del terreno contra el proyecto, y nada de grados; antes de tocar nada.
    let origen = format!("el terreno (EPSG:{epsg})");
    comprobar_crs(p, &format!("EPSG:{epsg}"), &origen)?;
    rechazar_lonlat(pts.iter().map(|q| ("punto", q.e, q.n)), &origen)?;
    if epsg_de(&p.crs) != Some(epsg) {
        rep.avisos.push(format!(
            "el proyecto estaba vacío y adopta el CRS del terreno: EPSG:{epsg} (antes {})",
            p.crs
        ));
        p.crs = format!("EPSG:{epsg}");
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
