//! Validación **geométrica** de la red importada (F13-01): las comprobaciones que el revisor
//! haría a mano sobre cotas, diámetros y trazados. No es hidráulica ni normativa:
//!
//! - Los umbrales son del usuario ([`ReglasValidacion`]). Los valores por defecto son una
//!   referencia típica, no una norma: cada proyecto los ajusta a la suya.
//! - Solo se mide lo que trajo el origen. Si falta un dato para evaluar una regla en un objeto,
//!   el resultado es «no evaluable (falta X)», nunca un valor supuesto (D4). La única excepción
//!   es la **cota segura**: si, aun sin contar lo que falta, la regla ya se incumple (p. ej. el
//!   recubrimiento sobre la solera ya es menor que el mínimo), el incumplimiento es cierto y se
//!   informa diciendo qué no se contó.
//! - Núcleo puro: sin E/S. La CLI (`redes validar`) y la app lo llaman igual.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use serde::{Deserialize, Serialize};

use crate::families::{Category, caps, family};
use crate::geom::{P2, closest_on_segment};
use crate::model::{COTA_SOLERA, COTA_VERTICES, Link, Node, Project};
use crate::topology::{Topology, es_a_presion};

/// Rango admitido `[min, max]`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Rango {
    pub min: f64,
    pub max: f64,
}

/// Umbrales de la validación. Los valores por defecto son **referencia típica**: ajústelos a su
/// norma. Se guardan en el proyecto (`Project::reglas_validacion`) o se pasan con `--reglas`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ReglasValidacion {
    /// Pendiente admitida de los colectores sanitarios (m/m).
    pub pendiente_sanitario: Rango,
    /// Pendiente admitida de los colectores pluviales y canales (m/m).
    pub pendiente_pluvial: Rango,
    /// Pendiente admitida de los colectores combinados (m/m).
    pub pendiente_combinado: Rango,
    /// Salto de solera en un pozo por encima del cual se espera un subtipo de caída (m).
    pub salto_max_sin_caida_m: f64,
    /// Recubrimiento mínimo de la tubería de alcantarillado (m).
    pub recubrimiento_min_alcantarillado_m: f64,
    /// Recubrimiento mínimo de la tubería de agua potable (m).
    pub recubrimiento_min_agua_m: f64,
    /// Profundidad máxima de un pozo, tapa − solera (m).
    pub profundidad_max_pozo_m: f64,
    /// Separación horizontal mínima entre agua potable y alcantarillado (m).
    pub separacion_horizontal_min_m: f64,
    /// Separación vertical mínima, con el agua por encima, cuando no se cumple la horizontal (m).
    pub separacion_vertical_min_m: f64,
    /// Tolerancia absoluta entre longitud declarada y medida (m).
    pub longitud_tolerancia_m: f64,
    /// Tolerancia relativa entre longitud declarada y medida (tanto por uno).
    pub longitud_tolerancia_rel: f64,
}

impl Default for ReglasValidacion {
    fn default() -> Self {
        ReglasValidacion {
            pendiente_sanitario: Rango {
                min: 0.005,
                max: 0.15,
            },
            pendiente_pluvial: Rango {
                min: 0.003,
                max: 0.15,
            },
            pendiente_combinado: Rango {
                min: 0.003,
                max: 0.15,
            },
            salto_max_sin_caida_m: 0.75,
            recubrimiento_min_alcantarillado_m: 1.0,
            recubrimiento_min_agua_m: 0.8,
            profundidad_max_pozo_m: 5.0,
            separacion_horizontal_min_m: 3.0,
            separacion_vertical_min_m: 0.30,
            longitud_tolerancia_m: 0.5,
            longitud_tolerancia_rel: 0.01,
        }
    }
}

impl ReglasValidacion {
    /// Rechaza umbrales sin sentido (negativos, no finitos, `min > max`).
    pub fn comprobar(&self) -> Result<(), String> {
        let escalares = [
            ("salto_max_sin_caida_m", self.salto_max_sin_caida_m),
            (
                "recubrimiento_min_alcantarillado_m",
                self.recubrimiento_min_alcantarillado_m,
            ),
            ("recubrimiento_min_agua_m", self.recubrimiento_min_agua_m),
            ("profundidad_max_pozo_m", self.profundidad_max_pozo_m),
            (
                "separacion_horizontal_min_m",
                self.separacion_horizontal_min_m,
            ),
            ("separacion_vertical_min_m", self.separacion_vertical_min_m),
            ("longitud_tolerancia_m", self.longitud_tolerancia_m),
            ("longitud_tolerancia_rel", self.longitud_tolerancia_rel),
        ];
        for (k, v) in escalares {
            if !v.is_finite() || v < 0.0 {
                return Err(format!("{k} debe ser un número ≥ 0 (vale {v})"));
            }
        }
        for (k, r) in [
            ("pendiente_sanitario", self.pendiente_sanitario),
            ("pendiente_pluvial", self.pendiente_pluvial),
            ("pendiente_combinado", self.pendiente_combinado),
        ] {
            if !r.min.is_finite() || !r.max.is_finite() || r.min < 0.0 || r.min > r.max {
                return Err(format!(
                    "{k}: rango inválido [{}, {}] (0 ≤ min ≤ max)",
                    r.min, r.max
                ));
            }
        }
        Ok(())
    }

    /// Claves que admite el archivo de reglas (para rechazar un campo mal escrito).
    pub fn claves() -> Vec<String> {
        match serde_json::to_value(ReglasValidacion::default()) {
            Ok(serde_json::Value::Object(m)) => m.keys().cloned().collect(),
            _ => Vec::new(),
        }
    }
}

/// Reglas, en el orden en que se informan: `(clave, descripción)`.
pub const REGLAS: &[(&str, &str)] = &[
    (
        "contrapendiente",
        "tramo de gravedad cuya solera sube en el sentido declarado",
    ),
    (
        "pendiente",
        "pendiente medida (soleras y longitud en planta) fuera del rango del sistema",
    ),
    (
        "salto_pozo",
        "salto de solera a la llegada de un pozo sin subtipo de caída",
    ),
    (
        "recubrimiento",
        "rasante − (solera + DI + espesor) menor que el mínimo",
    ),
    (
        "profundidad_pozo",
        "tapa (o terreno) − solera del pozo mayor que el máximo",
    ),
    (
        "separacion",
        "agua potable demasiado cerca del alcantarillado (horizontal y vertical, agua arriba)",
    ),
    (
        "dn_aguas_abajo",
        "DN de salida menor que el de una llegada en gravedad",
    ),
    ("tramo_huerfano", "tramo con un extremo que no existe"),
    ("nodo_aislado", "nodo de red sin ningún tramo ni enlace"),
    (
        "longitud",
        "longitud declarada distinta de la medida (en planta y 3D)",
    ),
];

/// Comprueba una lista de reglas (`--solo`): devuelve la primera desconocida.
pub fn regla_desconocida<'a>(solo: &[&'a str]) -> Option<&'a str> {
    solo.iter()
        .copied()
        .find(|s| !REGLAS.iter().any(|(k, _)| k == s))
}

/// Gravedad del hallazgo. `NoEvaluable` no es un incumplimiento: falta un dato.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severidad {
    Grave,
    Aviso,
    NoEvaluable,
}

impl Severidad {
    pub fn key(self) -> &'static str {
        match self {
            Severidad::Grave => "grave",
            Severidad::Aviso => "aviso",
            Severidad::NoEvaluable => "no_evaluable",
        }
    }
}

/// Un resultado de una regla sobre uno o varios objetos.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Hallazgo {
    pub regla: String,
    pub severidad: Severidad,
    /// Códigos de los objetos implicados; el primero es el principal (al que se va con un clic).
    pub objetos: Vec<String>,
    /// Valor medido (en `unidad`); `None` si no se pudo medir.
    pub medido: Option<f64>,
    /// Umbral con el que se comparó.
    pub umbral: Option<f64>,
    pub unidad: String,
    pub mensaje: String,
    /// Dato que faltó (solo en `no_evaluable` o cuando el valor es una cota segura).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub falta: Option<String>,
    /// Ubicación en planta `[E, N]` (coordenadas del proyecto).
    pub ubicacion: Option<[f64; 2]>,
}

/// Recuento por regla.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ResumenRegla {
    pub regla: String,
    pub descripcion: String,
    /// Objetos (o pares) en que se pudo evaluar la regla.
    pub evaluados: usize,
    pub incumplen: usize,
    pub no_evaluables: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Informe {
    pub hallazgos: Vec<Hallazgo>,
    pub resumen: Vec<ResumenRegla>,
}

impl Informe {
    pub fn incumplen(&self) -> usize {
        self.resumen.iter().map(|r| r.incumplen).sum()
    }
    pub fn no_evaluables(&self) -> usize {
        self.resumen.iter().map(|r| r.no_evaluables).sum()
    }
}

/// Valida el proyecto. `solo`: limita a esas reglas (claves de [`REGLAS`]); `None` = todas.
pub fn validar(p: &Project, reglas: &ReglasValidacion, solo: Option<&[&str]>) -> Informe {
    let activa = |k: &str| solo.is_none_or(|s| s.contains(&k));
    let topo = Topology::build(p);
    let mut cx = Cx::default();
    if activa("contrapendiente") {
        contrapendiente(p, &topo, &mut cx);
    }
    if activa("pendiente") {
        pendiente(p, reglas, &mut cx);
    }
    if activa("salto_pozo") {
        salto_pozo(p, reglas, &mut cx);
    }
    if activa("recubrimiento") {
        recubrimiento(p, reglas, &mut cx);
    }
    if activa("profundidad_pozo") {
        profundidad_pozo(p, reglas, &mut cx);
    }
    if activa("separacion") {
        separacion(p, reglas, &mut cx);
    }
    if activa("dn_aguas_abajo") {
        dn_aguas_abajo(p, &topo, &mut cx);
    }
    if activa("tramo_huerfano") {
        tramo_huerfano(p, &mut cx);
    }
    if activa("nodo_aislado") {
        nodo_aislado(p, &topo, &mut cx);
    }
    if activa("longitud") {
        longitud(p, reglas, &mut cx);
    }

    let orden = |r: &str| REGLAS.iter().position(|(k, _)| *k == r).unwrap_or(99);
    cx.hallazgos.sort_by(|a, b| {
        (a.severidad, orden(&a.regla), &a.objetos).cmp(&(b.severidad, orden(&b.regla), &b.objetos))
    });
    let resumen = REGLAS
        .iter()
        .filter(|(k, _)| activa(k))
        .map(|(k, d)| {
            let (evaluados, incumplen, no_evaluables) =
                cx.cuentas.get(k).copied().unwrap_or_default();
            ResumenRegla {
                regla: k.to_string(),
                descripcion: d.to_string(),
                evaluados,
                incumplen,
                no_evaluables,
            }
        })
        .collect();
    Informe {
        hallazgos: cx.hallazgos,
        resumen,
    }
}

// ---------------------------------------------------------------------------------------------
// Acumulador

#[derive(Default)]
struct Cx {
    hallazgos: Vec<Hallazgo>,
    cuentas: BTreeMap<&'static str, (usize, usize, usize)>,
}

impl Cx {
    fn cumple(&mut self, regla: &'static str) {
        self.cuentas.entry(regla).or_default().0 += 1;
    }
    fn incumple(&mut self, regla: &'static str, h: Hallazgo) {
        let c = self.cuentas.entry(regla).or_default();
        c.0 += 1;
        c.1 += 1;
        self.hallazgos.push(h);
    }
    fn no_evaluable(
        &mut self,
        regla: &'static str,
        objetos: Vec<String>,
        falta: &str,
        ubicacion: Option<P2>,
    ) {
        self.cuentas.entry(regla).or_default().2 += 1;
        self.hallazgos.push(Hallazgo {
            regla: regla.into(),
            severidad: Severidad::NoEvaluable,
            mensaje: format!("{}: no evaluable (falta {falta})", objetos.join(" / ")),
            objetos,
            medido: None,
            umbral: None,
            unidad: String::new(),
            falta: Some(falta.into()),
            ubicacion: ubicacion.map(|q| [q.x, q.y]),
        });
    }
}

#[allow(clippy::too_many_arguments)]
fn hallazgo(
    regla: &str,
    severidad: Severidad,
    objetos: Vec<String>,
    medido: f64,
    umbral: f64,
    unidad: &str,
    mensaje: String,
    ubicacion: Option<P2>,
) -> Hallazgo {
    Hallazgo {
        regla: regla.into(),
        severidad,
        objetos,
        medido: Some(medido),
        umbral: Some(umbral),
        unidad: unidad.into(),
        mensaje,
        falta: None,
        ubicacion: ubicacion.map(|q| [q.x, q.y]),
    }
}

// ---------------------------------------------------------------------------------------------
// Lectores sin suposiciones

fn categoria(family_key: &str) -> Option<Category> {
    family(family_key).map(|f| f.category)
}

/// Tramo de alcantarillado por gravedad (las reglas de pendiente aplican). El sifón invertido
/// baja y sube por diseño: queda fuera.
fn es_gravedad(l: &Link) -> bool {
    categoria(&l.family) == Some(Category::Saneamiento)
        && !es_a_presion(&l.family, &l.ty)
        && l.family != "saneamiento.sifon"
}

fn rango_de<'a>(l: &Link, r: &'a ReglasValidacion) -> (&'static str, &'a Rango) {
    if l.ty == "colector_pluvial" || l.family == "saneamiento.canal" {
        ("pluvial", &r.pendiente_pluvial)
    } else if l.ty == "colector_combinado" {
        ("combinado", &r.pendiente_combinado)
    } else {
        ("sanitario", &r.pendiente_sanitario)
    }
}

/// Solera en el vértice `i` del trazado, si es dato: en los extremos la declarada (o la del
/// trazado si es dato); en los intermedios, solo si sus cotas no se interpolaron (F12-15).
fn solera_vertice(l: &Link, i: usize) -> Option<f64> {
    let n = l.path.len();
    if n == 0 || i >= n {
        return None;
    }
    let trazado = (!l.falta(COTA_SOLERA)).then(|| l.path[i].z);
    if i == 0 {
        l.cota_solera_inicio.or(trazado)
    } else if i == n - 1 {
        l.cota_solera_fin.or(trazado)
    } else if l.falta(COTA_VERTICES) {
        None
    } else {
        trazado
    }
}

/// Diámetro interior declarado (m). El DN es una designación comercial: no se usa como DI.
fn di_m(l: &Link) -> Option<f64> {
    l.num("diametro_interior")
        .filter(|d| d.is_finite() && *d > 0.0)
        .map(|d| d / 1000.0)
}

/// Espesor de pared declarado (m), o derivado de DE y DI declarados.
fn espesor_m(l: &Link) -> Option<f64> {
    l.num("espesor_pared")
        .filter(|e| e.is_finite() && *e > 0.0)
        .or_else(|| {
            let de = l.num("diametro_exterior").filter(|d| *d > 0.0)?;
            let e = (de - di_m(l)?) * 0.5;
            (e > 0.0).then_some(e)
        })
}

fn punto_medio(l: &Link) -> Option<P2> {
    let total = l.length_2d();
    let mut acum = 0.0;
    for w in l.path.windows(2) {
        let s = w[0].p2().dist(w[1].p2());
        if acum + s >= total * 0.5 && s > 0.0 {
            return Some(w[0].p2().lerp(w[1].p2(), (total * 0.5 - acum) / s));
        }
        acum += s;
    }
    l.path.first().map(|q| q.p2())
}

fn faltas_texto(f: &BTreeSet<&str>) -> String {
    f.iter().copied().collect::<Vec<_>>().join(", ")
}

// ---------------------------------------------------------------------------------------------
// Reglas

fn contrapendiente(p: &Project, topo: &Topology, cx: &mut Cx) {
    const R: &str = "contrapendiente";
    for l in p.links.iter().filter(|l| !l.deleted && es_gravedad(l)) {
        let (Some(a), Some(b)) = (solera_vertice(l, 0), solera_vertice(l, l.path.len() - 1)) else {
            cx.no_evaluable(R, vec![l.code.clone()], "cota_solera", punto_medio(l));
            continue;
        };
        // La topología marca la contrapendiente sobre el trazado; aquí, además, con la solera
        // declarada de los extremos.
        if topo.contrapendiente.contains(&l.id) || b > a + 1e-9 {
            cx.incumple(
                R,
                hallazgo(
                    R,
                    Severidad::Grave,
                    vec![l.code.clone()],
                    b - a,
                    0.0,
                    "m",
                    format!(
                        "{}: la solera sube {:.3} m en el sentido declarado ({:.3} → {:.3})",
                        l.code,
                        b - a,
                        a,
                        b
                    ),
                    punto_medio(l),
                ),
            );
        } else {
            cx.cumple(R);
        }
    }
}

fn pendiente(p: &Project, r: &ReglasValidacion, cx: &mut Cx) {
    const R: &str = "pendiente";
    for l in p.links.iter().filter(|l| !l.deleted && es_gravedad(l)) {
        let largo = l.length_2d();
        let (Some(a), Some(b)) = (solera_vertice(l, 0), solera_vertice(l, l.path.len() - 1)) else {
            cx.no_evaluable(R, vec![l.code.clone()], "cota_solera", punto_medio(l));
            continue;
        };
        if largo <= 1e-6 {
            cx.no_evaluable(R, vec![l.code.clone()], "trazado en planta", punto_medio(l));
            continue;
        }
        let s = (a - b) / largo;
        if s < 0.0 {
            // Contrapendiente: ya la informa su regla.
            cx.cumple(R);
            continue;
        }
        let (sistema, rango) = rango_de(l, r);
        let umbral = if s < rango.min {
            Some((rango.min, "mínimo"))
        } else if s > rango.max {
            Some((rango.max, "máximo"))
        } else {
            None
        };
        match umbral {
            Some((u, cual)) => cx.incumple(
                R,
                hallazgo(
                    R,
                    Severidad::Aviso,
                    vec![l.code.clone()],
                    s,
                    u,
                    "m/m",
                    format!(
                        "{}: pendiente {:.2} % fuera del rango {sistema} ({cual} {:.2} %)",
                        l.code,
                        s * 100.0,
                        u * 100.0
                    ),
                    punto_medio(l),
                ),
            ),
            None => cx.cumple(R),
        }
    }
}

/// Subtipo de pozo con caída: el registro le exige `altura_caida`.
fn es_caida(n: &Node) -> bool {
    family(&n.family)
        .and_then(|f| f.ty(&n.ty))
        .is_some_and(|t| t.required.contains(&"altura_caida"))
}

fn salto_pozo(p: &Project, r: &ReglasValidacion, cx: &mut Cx) {
    const R: &str = "salto_pozo";
    for n in p
        .nodes
        .iter()
        .filter(|n| !n.deleted && n.family == "saneamiento.pozo")
    {
        if es_caida(n) {
            continue;
        }
        for l in p.tramos_de(n.id).filter(|l| l.to == n.id && es_gravedad(l)) {
            let objetos = vec![n.code.clone(), l.code.clone()];
            let llegada = solera_vertice(l, l.path.len().saturating_sub(1));
            let (llegada, fondo) = match (llegada, n.cota_solera()) {
                (Some(a), Some(b)) => (a, b),
                (None, Some(_)) => {
                    cx.no_evaluable(
                        R,
                        objetos,
                        &format!("cota_solera de llegada de {}", l.code),
                        Some(n.pos),
                    );
                    continue;
                }
                (_, None) => {
                    cx.no_evaluable(
                        R,
                        objetos,
                        &format!("cota_solera del pozo {}", n.code),
                        Some(n.pos),
                    );
                    continue;
                }
            };
            let salto = llegada - fondo;
            if salto > r.salto_max_sin_caida_m {
                cx.incumple(
                    R,
                    hallazgo(
                        R,
                        Severidad::Aviso,
                        objetos,
                        salto,
                        r.salto_max_sin_caida_m,
                        "m",
                        format!(
                            "{}: {} llega {:.2} m sobre la solera y el pozo no es de caída (subtipo {}; umbral {:.2} m)",
                            n.code, l.code, salto, n.ty, r.salto_max_sin_caida_m
                        ),
                        Some(n.pos),
                    ),
                );
            } else {
                cx.cumple(R);
            }
        }
    }
}

fn recubrimiento(p: &Project, r: &ReglasValidacion, cx: &mut Cx) {
    const R: &str = "recubrimiento";
    for l in p.links.iter().filter(|l| !l.deleted && l.path.len() >= 2) {
        let umbral = match categoria(&l.family) {
            Some(Category::Agua) => r.recubrimiento_min_agua_m,
            Some(Category::Saneamiento) => r.recubrimiento_min_alcantarillado_m,
            _ => continue,
        };
        let (di, e) = (di_m(l), espesor_m(l));
        let mut faltas: BTreeSet<&str> = BTreeSet::new();
        if di.is_none() {
            faltas.insert("diametro_interior");
        }
        if e.is_none() {
            faltas.insert("espesor_pared");
        }
        let ultimo = l.path.len() - 1;
        // (recubrimiento, es exacto, punto)
        let mut peor: Option<(f64, P2)> = None;
        let mut todos_exactos = true;
        for (i, q) in l.path.iter().enumerate() {
            let nodo = if i == 0 {
                p.node(l.from)
            } else if i == ultimo {
                p.node(l.to)
            } else {
                None
            };
            let rasante = nodo.and_then(|n| n.cota_terreno()).or_else(|| {
                p.terrain
                    .as_ref()
                    .and_then(|t| t.z_at(q.p2()))
                    .filter(|z| z.is_finite())
            });
            let Some(rasante) = rasante else {
                faltas.insert("rasante (terreno del nodo o TIN)");
                todos_exactos = false;
                continue;
            };
            let Some(solera) = solera_vertice(l, i) else {
                faltas.insert("cota_solera");
                todos_exactos = false;
                continue;
            };
            // Sin DI o sin espesor el valor es una cota superior del recubrimiento real.
            let c = rasante - (solera + di.unwrap_or(0.0) + e.unwrap_or(0.0));
            if peor.is_none_or(|(m, _)| c < m) {
                peor = Some((c, q.p2()));
            }
        }
        let exacto = todos_exactos && di.is_some() && e.is_some();
        match peor {
            Some((c, q)) if c < umbral - 1e-9 => {
                let mensaje = if di.is_some() && e.is_some() {
                    format!(
                        "{}: recubrimiento {:.2} m < mínimo {:.2} m",
                        l.code, c, umbral
                    )
                } else {
                    format!(
                        "{}: recubrimiento ≤ {:.2} m aun sin contar {} (mínimo {:.2} m)",
                        l.code,
                        c,
                        faltas_texto(&faltas),
                        umbral
                    )
                };
                let mut h = hallazgo(
                    R,
                    Severidad::Aviso,
                    vec![l.code.clone()],
                    c,
                    umbral,
                    "m",
                    mensaje,
                    Some(q),
                );
                if !faltas.is_empty() {
                    h.falta = Some(faltas_texto(&faltas));
                }
                cx.incumple(R, h);
            }
            _ if exacto => cx.cumple(R),
            Some((_, q)) => {
                cx.no_evaluable(R, vec![l.code.clone()], &faltas_texto(&faltas), Some(q))
            }
            None => cx.no_evaluable(
                R,
                vec![l.code.clone()],
                &faltas_texto(&faltas),
                punto_medio(l),
            ),
        }
    }
}

fn profundidad_pozo(p: &Project, r: &ReglasValidacion, cx: &mut Cx) {
    const R: &str = "profundidad_pozo";
    for n in p
        .nodes
        .iter()
        .filter(|n| !n.deleted && n.family == "saneamiento.pozo")
    {
        let Some(solera) = n.cota_solera() else {
            cx.no_evaluable(R, vec![n.code.clone()], "cota_solera", Some(n.pos));
            continue;
        };
        let (arriba, desde) = match (n.cota_tapa(), n.cota_terreno()) {
            (Some(t), _) => (t, "tapa"),
            (None, Some(t)) => (t, "terreno"),
            (None, None) => {
                cx.no_evaluable(
                    R,
                    vec![n.code.clone()],
                    "cota_tapa y cota_terreno",
                    Some(n.pos),
                );
                continue;
            }
        };
        let prof = arriba - solera;
        if prof > r.profundidad_max_pozo_m + 1e-9 {
            cx.incumple(
                R,
                hallazgo(
                    R,
                    Severidad::Aviso,
                    vec![n.code.clone()],
                    prof,
                    r.profundidad_max_pozo_m,
                    "m",
                    format!(
                        "{}: profundidad {:.2} m ({desde} − solera) > máximo {:.2} m",
                        n.code, prof, r.profundidad_max_pozo_m
                    ),
                    Some(n.pos),
                ),
            );
        } else {
            cx.cumple(R);
        }
    }
}

// --- separación agua / alcantarillado ---------------------------------------------------------

/// Rejilla de segmentos: cada segmento se apunta en las celdas de sus muestras y sus vecinas,
/// así que una consulta solo mira la celda de cada muestra del otro segmento. Con celda
/// ≥ 2 × separación, dos puntos a menos de la separación quedan siempre en celdas vecinas.
struct Rejilla {
    celda: f64,
    celdas: HashMap<(i64, i64), Vec<(usize, usize)>>,
}

impl Rejilla {
    fn new(separacion: f64) -> Rejilla {
        Rejilla {
            celda: (separacion * 2.0).max(10.0),
            celdas: HashMap::new(),
        }
    }
    fn clave(&self, q: P2) -> (i64, i64) {
        (
            (q.x / self.celda).floor() as i64,
            (q.y / self.celda).floor() as i64,
        )
    }
    /// Muestras de un segmento cada cuarto de celda (incluye los extremos).
    fn muestras(&self, a: P2, b: P2) -> impl Iterator<Item = P2> {
        let n = ((a.dist(b) / (self.celda * 0.25)).ceil() as usize).clamp(1, 100_000);
        (0..=n).map(move |k| a.lerp(b, k as f64 / n as f64))
    }
    fn insertar(&mut self, id: (usize, usize), a: P2, b: P2) {
        let mut vistas = BTreeSet::new();
        for q in self.muestras(a, b) {
            let (cx, cy) = self.clave(q);
            for dx in -1..=1 {
                for dy in -1..=1 {
                    vistas.insert((cx + dx, cy + dy));
                }
            }
        }
        for k in vistas {
            self.celdas.entry(k).or_default().push(id);
        }
    }
    fn candidatos(&self, a: P2, b: P2) -> BTreeSet<(usize, usize)> {
        let mut out = BTreeSet::new();
        for q in self.muestras(a, b) {
            if let Some(v) = self.celdas.get(&self.clave(q)) {
                out.extend(v.iter().copied());
            }
        }
        out
    }
}

/// Distancia en planta entre dos segmentos y parámetros `(t, u)` de sus puntos más cercanos.
fn dist_segmentos(a0: P2, a1: P2, b0: P2, b1: P2) -> (f64, f64, f64) {
    let (dax, day) = (a1.x - a0.x, a1.y - a0.y);
    let (dbx, dby) = (b1.x - b0.x, b1.y - b0.y);
    let den = dax * dby - day * dbx;
    if den.abs() > 1e-12 {
        let t = ((b0.x - a0.x) * dby - (b0.y - a0.y) * dbx) / den;
        let u = ((b0.x - a0.x) * day - (b0.y - a0.y) * dax) / den;
        if (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u) {
            return (0.0, t, u);
        }
    }
    let mut mejor = (f64::INFINITY, 0.0, 0.0);
    for (punto, t_fijo, en_a) in [
        (a0, 0.0, true),
        (a1, 1.0, true),
        (b0, 0.0, false),
        (b1, 1.0, false),
    ] {
        let (q, s) = if en_a {
            closest_on_segment(punto, b0, b1)
        } else {
            closest_on_segment(punto, a0, a1)
        };
        let d = punto.dist(q);
        if d < mejor.0 {
            mejor = if en_a { (d, t_fijo, s) } else { (d, s, t_fijo) };
        }
    }
    mejor
}

/// Solera en el parámetro `t` del segmento `i` (interpolada entre dos vértices con dato).
fn solera_en(l: &Link, i: usize, t: f64) -> Option<f64> {
    let a = solera_vertice(l, i)?;
    let b = solera_vertice(l, i + 1)?;
    Some(a + (b - a) * t)
}

fn separacion(p: &Project, r: &ReglasValidacion, cx: &mut Cx) {
    const R: &str = "separacion";
    let sep_h = r.separacion_horizontal_min_m;
    let sep_v = r.separacion_vertical_min_m;
    let vivos: Vec<&Link> = p
        .links
        .iter()
        .filter(|l| !l.deleted && l.path.len() >= 2)
        .collect();
    let mut rejilla = Rejilla::new(sep_h);
    for (k, l) in vivos.iter().enumerate() {
        if categoria(&l.family) == Some(Category::Saneamiento) {
            for (s, w) in l.path.windows(2).enumerate() {
                rejilla.insertar((k, s), w[0].p2(), w[1].p2());
            }
        }
    }
    for agua in vivos
        .iter()
        .filter(|l| categoria(&l.family) == Some(Category::Agua))
    {
        // Por cada colector cercano: (distancia mínima, punto), peor holgura vertical en los
        // tramos a menos de la separación y lo que faltó.
        struct Par {
            d_min: f64,
            en: P2,
            holgura: Option<(f64, P2)>,
            exacta: bool,
            faltas: BTreeSet<&'static str>,
        }
        let mut pares: BTreeMap<usize, Par> = BTreeMap::new();
        for (i, w) in agua.path.windows(2).enumerate() {
            let (a0, a1) = (w[0].p2(), w[1].p2());
            for (k, s) in rejilla.candidatos(a0, a1) {
                let alc = vivos[k];
                let (b0, b1) = (alc.path[s].p2(), alc.path[s + 1].p2());
                let (d, t, u) = dist_segmentos(a0, a1, b0, b1);
                let en = a0.lerp(a1, t);
                let par = pares.entry(k).or_insert(Par {
                    d_min: f64::INFINITY,
                    en,
                    holgura: None,
                    exacta: true,
                    faltas: BTreeSet::new(),
                });
                if d < par.d_min {
                    par.d_min = d;
                    par.en = en;
                }
                if d >= sep_h {
                    continue;
                }
                let (Some(zw), Some(zs)) = (solera_en(agua, i, t), solera_en(alc, s, u)) else {
                    par.exacta = false;
                    par.faltas.insert("cota_solera");
                    continue;
                };
                // Fondo exterior del agua y clave exterior del colector. Lo que falta se toma
                // como 0: la holgura resultante es una cota superior de la real.
                let (ew, di_s, es) = (espesor_m(agua), di_m(alc), espesor_m(alc));
                for (falta, v) in [
                    ("espesor_pared del agua", ew.is_none()),
                    ("diametro_interior del colector", di_s.is_none()),
                    ("espesor_pared del colector", es.is_none()),
                ] {
                    if v {
                        par.exacta = false;
                        par.faltas.insert(falta);
                    }
                }
                let holgura =
                    (zw - ew.unwrap_or(0.0)) - (zs + di_s.unwrap_or(0.0) + es.unwrap_or(0.0));
                if par.holgura.is_none_or(|(h, _)| holgura < h) {
                    par.holgura = Some((holgura, en));
                }
            }
        }
        for (k, par) in pares {
            let alc = vivos[k];
            let objetos = vec![agua.code.clone(), alc.code.clone()];
            if par.d_min >= sep_h {
                cx.cumple(R);
                continue;
            }
            match par.holgura {
                Some((h, en)) if h < sep_v - 1e-9 => {
                    let posicion = if h < 0.0 {
                        "el agua no queda por encima".to_string()
                    } else {
                        format!("el agua queda {h:.2} m por encima")
                    };
                    let mut msg = format!(
                        "{} a {:.2} m en planta de {} (mín. {:.2} m) y {posicion} (mín. {:.2} m)",
                        agua.code, par.d_min, alc.code, sep_h, sep_v
                    );
                    if !par.exacta {
                        msg.push_str(&format!(
                            "; cota segura, sin contar {}",
                            faltas_texto(&par.faltas)
                        ));
                    }
                    let mut hz = hallazgo(
                        R,
                        if h < 0.0 {
                            Severidad::Grave
                        } else {
                            Severidad::Aviso
                        },
                        objetos,
                        h,
                        sep_v,
                        "m",
                        msg,
                        Some(en),
                    );
                    if !par.exacta {
                        hz.falta = Some(faltas_texto(&par.faltas));
                    }
                    cx.incumple(R, hz);
                }
                Some(_) if par.exacta => cx.cumple(R),
                _ => cx.no_evaluable(
                    R,
                    objetos,
                    &format!(
                        "{} para la separación vertical (en planta {:.2} m < {:.2} m)",
                        faltas_texto(&par.faltas),
                        par.d_min,
                        sep_h
                    ),
                    Some(par.en),
                ),
            }
        }
    }
}

fn dn_aguas_abajo(p: &Project, topo: &Topology, cx: &mut Cx) {
    const R: &str = "dn_aguas_abajo";
    for n in p.nodes.iter().filter(|n| !n.deleted) {
        let tramos: Vec<&Link> = topo
            .links_at(n.id)
            .iter()
            .filter_map(|id| p.link(*id))
            .filter(|l| es_gravedad(l))
            .collect();
        let llegadas: Vec<&Link> = tramos.iter().copied().filter(|l| l.to == n.id).collect();
        if llegadas.is_empty() {
            continue;
        }
        for sale in tramos.iter().filter(|l| l.from == n.id) {
            let mut objetos = vec![sale.code.clone()];
            let Some(dn_sale) = sale.dn() else {
                cx.no_evaluable(
                    R,
                    objetos,
                    &format!("diametro_nominal de {}", sale.code),
                    Some(n.pos),
                );
                continue;
            };
            let mayor = llegadas
                .iter()
                .filter_map(|l| l.dn().map(|d| (d, l)))
                .max_by_key(|(d, _)| *d);
            let sin_dn: Vec<&str> = llegadas
                .iter()
                .filter(|l| l.dn().is_none())
                .map(|l| l.code.as_str())
                .collect();
            match mayor {
                Some((d, l)) if d > dn_sale => {
                    objetos.push(l.code.clone());
                    objetos.push(n.code.clone());
                    cx.incumple(
                        R,
                        hallazgo(
                            R,
                            Severidad::Aviso,
                            objetos,
                            dn_sale as f64,
                            d as f64,
                            "mm",
                            format!(
                                "{}: sale de {} con DN {} menor que la llegada {} (DN {})",
                                sale.code, n.code, dn_sale, l.code, d
                            ),
                            Some(n.pos),
                        ),
                    );
                }
                _ if !sin_dn.is_empty() => {
                    objetos.extend(sin_dn.iter().map(|s| s.to_string()));
                    cx.no_evaluable(
                        R,
                        objetos,
                        &format!("diametro_nominal de {}", sin_dn.join(", ")),
                        Some(n.pos),
                    );
                }
                _ => cx.cumple(R),
            }
        }
    }
}

fn tramo_huerfano(p: &Project, cx: &mut Cx) {
    const R: &str = "tramo_huerfano";
    for l in p.links.iter().filter(|l| !l.deleted) {
        let faltan: Vec<&str> = [(l.from, "inicio"), (l.to, "fin")]
            .into_iter()
            .filter(|(id, _)| p.node(*id).is_none())
            .map(|(_, k)| k)
            .collect();
        if faltan.is_empty() {
            cx.cumple(R);
        } else {
            cx.incumple(
                R,
                Hallazgo {
                    regla: R.into(),
                    severidad: Severidad::Grave,
                    objetos: vec![l.code.clone()],
                    medido: None,
                    umbral: None,
                    unidad: String::new(),
                    mensaje: format!(
                        "{}: el nodo de {} no existe o está borrado",
                        l.code,
                        faltan.join(" y del ")
                    ),
                    falta: None,
                    ubicacion: punto_medio(l).map(|q| [q.x, q.y]),
                },
            );
        }
    }
}

fn nodo_aislado(p: &Project, topo: &Topology, cx: &mut Cx) {
    const R: &str = "nodo_aislado";
    let es_de_red = |n: &Node| family(&n.family).is_some_and(|f| f.can(caps::HYDRAULIC_NODE));
    let aislados: BTreeSet<_> = topo.isolated(p).collect();
    for n in p.nodes.iter().filter(|n| !n.deleted && es_de_red(n)) {
        if aislados.contains(&n.id) {
            cx.incumple(
                R,
                Hallazgo {
                    regla: R.into(),
                    severidad: Severidad::Aviso,
                    objetos: vec![n.code.clone()],
                    medido: None,
                    umbral: None,
                    unidad: String::new(),
                    mensaje: format!("{}: no tiene ningún tramo ni enlace", n.code),
                    falta: None,
                    ubicacion: Some([n.pos.x, n.pos.y]),
                },
            );
        } else {
            cx.cumple(R);
        }
    }
}

fn longitud(p: &Project, r: &ReglasValidacion, cx: &mut Cx) {
    const R: &str = "longitud";
    for l in p.links.iter().filter(|l| !l.deleted) {
        let declarada = l
            .num("longitud")
            .or_else(|| l.datos.get("longitud").map(|d| d.valor))
            .filter(|v| v.is_finite() && *v > 0.0);
        let Some(declarada) = declarada else {
            cx.no_evaluable(
                R,
                vec![l.code.clone()],
                "longitud declarada",
                punto_medio(l),
            );
            continue;
        };
        if l.path.len() < 2 {
            cx.no_evaluable(R, vec![l.code.clone()], "trazado", None);
            continue;
        }
        let (l2, l3) = (l.length_2d(), l.length_3d());
        // La declarada puede ser en planta o inclinada: se compara con la más próxima.
        let (medida, cual) = if (declarada - l2).abs() <= (declarada - l3).abs() {
            (l2, "en planta")
        } else {
            (l3, "3D")
        };
        let tol = r
            .longitud_tolerancia_m
            .max(r.longitud_tolerancia_rel * medida);
        let desvio = declarada - medida;
        if desvio.abs() > tol {
            cx.incumple(
                R,
                hallazgo(
                    R,
                    Severidad::Aviso,
                    vec![l.code.clone()],
                    medida,
                    tol,
                    "m",
                    format!(
                        "{}: longitud declarada {:.2} m, medida {:.2} m {cual} (desvío {:+.2} m > tolerancia {:.2} m)",
                        l.code, declarada, medida, desvio, tol
                    ),
                    punto_medio(l),
                ),
            );
        } else {
            cx.cumple(R);
        }
    }
}
