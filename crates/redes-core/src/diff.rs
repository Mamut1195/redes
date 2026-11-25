//! Comparación de dos entregas de un proyecto (REV-08, F13-05).
//!
//! Los objetos se emparejan por **código** (`PZ-001`), no por [`crate::model::ElementId`], que
//! cambia de una entrega a otra. Se reportan altas, bajas y cambios por objeto con el detalle por
//! campo. Los campos de revisión (estado, observaciones) van aparte de los de diseño.
//!
//! Solo lee el modelo; no lo modifica. El orden del resultado es determinista: nodos antes que
//! tramos, y dentro de cada grupo por código.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use crate::families::Value;
use crate::geom::P2;
use crate::model::{
    COTA_SOLERA, COTA_TAPA, COTA_TERRENO, DIAMETRO_NOMINAL, Dato, Link, Node, Project, SIN_DATO,
};
use crate::revision::Observacion;

/// Parámetros de la comparación.
#[derive(Clone, Copy, Debug)]
pub struct Opciones {
    /// Movimiento en planta por debajo del cual no se reporta (m).
    pub tolerancia_pos: f64,
    /// Diferencia de cota por debajo de la cual no se reporta (m).
    pub tolerancia_cota: f64,
    /// Limita el resultado a un tipo de cambio.
    pub solo: Option<Clase>,
}

impl Default for Opciones {
    fn default() -> Self {
        Opciones {
            tolerancia_pos: 0.01,
            tolerancia_cota: 0.001,
            solo: None,
        }
    }
}

/// Naturaleza de un cambio.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Clase {
    Diseno,
    Revision,
}

impl Clase {
    pub fn key(self) -> &'static str {
        match self {
            Clase::Diseno => "diseno",
            Clase::Revision => "revision",
        }
    }
}

/// Cambio de un campo de un objeto.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Cambio {
    pub clase: Clase,
    pub campo: String,
    pub antes: String,
    pub despues: String,
}

/// Objeto presente en una sola entrega.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Presencia {
    pub tipo: &'static str,
    pub codigo: String,
    pub familia: String,
    pub subtipo: String,
}

/// Objeto presente en ambas entregas con al menos un cambio.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Modificado {
    pub tipo: &'static str,
    pub codigo: String,
    /// Familia en la entrega B.
    pub familia: String,
    pub cambios: Vec<Cambio>,
}

/// Cuentas por familia.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct PorFamilia {
    pub altas: usize,
    pub bajas: usize,
    pub modificados: usize,
}

/// Resultado de comparar A (entrega anterior) con B (entrega nueva).
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Diff {
    pub altas: Vec<Presencia>,
    pub bajas: Vec<Presencia>,
    pub modificados: Vec<Modificado>,
}

impl Diff {
    pub fn vacio(&self) -> bool {
        self.altas.is_empty() && self.bajas.is_empty() && self.modificados.is_empty()
    }
    pub fn cambios_de_diseno(&self) -> usize {
        self.contar(Clase::Diseno)
    }
    pub fn cambios_de_revision(&self) -> usize {
        self.contar(Clase::Revision)
    }
    fn contar(&self, c: Clase) -> usize {
        self.modificados
            .iter()
            .flat_map(|m| &m.cambios)
            .filter(|x| x.clase == c)
            .count()
    }
    /// Altas, bajas y objetos modificados por familia, en orden de clave.
    pub fn por_familia(&self) -> BTreeMap<String, PorFamilia> {
        let mut m: BTreeMap<String, PorFamilia> = BTreeMap::new();
        for a in &self.altas {
            m.entry(a.familia.clone()).or_default().altas += 1;
        }
        for b in &self.bajas {
            m.entry(b.familia.clone()).or_default().bajas += 1;
        }
        for x in &self.modificados {
            m.entry(x.familia.clone()).or_default().modificados += 1;
        }
        m
    }
}

/// Compara dos proyectos. Error si no comparten CRS.
pub fn comparar(a: &Project, b: &Project, op: &Opciones) -> Result<Diff, String> {
    if a.crs != b.crs {
        return Err(format!(
            "E_STATE_CRS_DISTINTO: las entregas usan CRS distinto ({} y {}); no se pueden comparar posiciones",
            a.crs, b.crs
        ));
    }
    let mut d = Diff::default();
    let na = nodos(a);
    let nb = nodos(b);
    let la = tramos(a);
    let lb = tramos(b);

    for (code, n) in &nb {
        if !na.contains_key(code) {
            d.altas.push(Presencia {
                tipo: "nodo",
                codigo: (*code).to_string(),
                familia: n.family.clone(),
                subtipo: n.ty.clone(),
            });
        }
    }
    for (code, l) in &lb {
        if !la.contains_key(code) {
            d.altas.push(Presencia {
                tipo: "tramo",
                codigo: (*code).to_string(),
                familia: l.family.clone(),
                subtipo: l.ty.clone(),
            });
        }
    }
    for (code, n) in &na {
        if !nb.contains_key(code) {
            d.bajas.push(Presencia {
                tipo: "nodo",
                codigo: (*code).to_string(),
                familia: n.family.clone(),
                subtipo: n.ty.clone(),
            });
        }
    }
    for (code, l) in &la {
        if !lb.contains_key(code) {
            d.bajas.push(Presencia {
                tipo: "tramo",
                codigo: (*code).to_string(),
                familia: l.family.clone(),
                subtipo: l.ty.clone(),
            });
        }
    }
    for (code, x) in &na {
        if let Some(y) = nb.get(code) {
            let c = cambios_nodo(x, y, op);
            if !c.is_empty() {
                d.modificados.push(Modificado {
                    tipo: "nodo",
                    codigo: (*code).to_string(),
                    familia: y.family.clone(),
                    cambios: c,
                });
            }
        }
    }
    for (code, x) in &la {
        if let Some(y) = lb.get(code) {
            let c = cambios_tramo(a, x, b, y, op);
            if !c.is_empty() {
                d.modificados.push(Modificado {
                    tipo: "tramo",
                    codigo: (*code).to_string(),
                    familia: y.family.clone(),
                    cambios: c,
                });
            }
        }
    }

    if let Some(solo) = op.solo {
        if solo == Clase::Revision {
            d.altas.clear();
            d.bajas.clear();
        }
        for m in &mut d.modificados {
            m.cambios.retain(|c| c.clase == solo);
        }
        d.modificados.retain(|m| !m.cambios.is_empty());
    }
    Ok(d)
}

fn nodos(p: &Project) -> BTreeMap<&str, &Node> {
    let mut m = BTreeMap::new();
    for n in p.nodes.iter().filter(|n| !n.deleted) {
        m.entry(n.code.as_str()).or_insert(n);
    }
    m
}

fn tramos(p: &Project) -> BTreeMap<&str, &Link> {
    let mut m = BTreeMap::new();
    for l in p.links.iter().filter(|l| !l.deleted) {
        m.entry(l.code.as_str()).or_insert(l);
    }
    m
}

fn cambio(clase: Clase, campo: &str, antes: String, despues: String) -> Cambio {
    Cambio {
        clase,
        campo: campo.to_string(),
        antes,
        despues,
    }
}

fn num(v: f64) -> String {
    let s = format!("{v:.4}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() || s == "-0" {
        "0".into()
    } else {
        s.to_string()
    }
}

fn opt_num(v: Option<f64>) -> String {
    v.map(num).unwrap_or_else(|| SIN_DATO.to_string())
}

fn valor(v: &Value) -> String {
    match v {
        Value::Num(n) => num(*n),
        Value::Int(i) => i.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Text(s) => s.clone(),
    }
}

fn texto_opt(v: Option<&str>) -> String {
    v.map(str::to_string)
        .unwrap_or_else(|| SIN_DATO.to_string())
}

/// Compara una cota que puede faltar: aparecer o desaparecer es un cambio.
fn cota(out: &mut Vec<Cambio>, campo: &str, a: Option<f64>, b: Option<f64>, tol: f64) {
    let distinto = match (a, b) {
        (None, None) => false,
        (Some(x), Some(y)) => (x - y).abs() > tol,
        _ => true,
    };
    if distinto {
        out.push(cambio(Clase::Diseno, campo, opt_num(a), opt_num(b)));
    }
}

fn atributos(
    out: &mut Vec<Cambio>,
    a: &BTreeMap<String, Value>,
    b: &BTreeMap<String, Value>,
    excluir: &[&str],
) {
    let claves: BTreeSet<&String> = a.keys().chain(b.keys()).collect();
    for k in claves {
        if excluir.contains(&k.as_str()) {
            continue;
        }
        let (x, y) = (a.get(k), b.get(k));
        let igual = match (x, y) {
            (Some(p), Some(q)) => match (p.as_f64(), q.as_f64()) {
                (Some(m), Some(n)) => (m - n).abs() <= 1e-9,
                _ => p == q,
            },
            (None, None) => true,
            _ => false,
        };
        if !igual {
            out.push(cambio(
                Clase::Diseno,
                &format!("attrs.{k}"),
                x.map(valor).unwrap_or_else(|| SIN_DATO.into()),
                y.map(valor).unwrap_or_else(|| SIN_DATO.into()),
            ));
        }
    }
}

fn datos(out: &mut Vec<Cambio>, a: &BTreeMap<String, Dato>, b: &BTreeMap<String, Dato>) {
    let claves: BTreeSet<&String> = a.keys().chain(b.keys()).collect();
    for k in claves {
        match (a.get(k), b.get(k)) {
            (Some(x), Some(y)) => {
                if (x.valor - y.valor).abs() > 1e-9 * x.valor.abs().max(y.valor.abs()).max(1.0) {
                    out.push(cambio(
                        Clase::Diseno,
                        &format!("datos.{k}.valor"),
                        num(x.valor),
                        num(y.valor),
                    ));
                }
                if x.unidad != y.unidad {
                    out.push(cambio(
                        Clase::Diseno,
                        &format!("datos.{k}.unidad"),
                        x.unidad.clone(),
                        y.unidad.clone(),
                    ));
                }
                if x.fuente != y.fuente {
                    out.push(cambio(
                        Clase::Diseno,
                        &format!("datos.{k}.fuente"),
                        x.fuente.clone(),
                        y.fuente.clone(),
                    ));
                }
            }
            (Some(x), None) => out.push(cambio(
                Clase::Diseno,
                &format!("datos.{k}"),
                format!("{} {}", num(x.valor), x.unidad),
                SIN_DATO.into(),
            )),
            (None, Some(y)) => out.push(cambio(
                Clase::Diseno,
                &format!("datos.{k}"),
                SIN_DATO.into(),
                format!("{} {}", num(y.valor), y.unidad),
            )),
            (None, None) => {}
        }
    }
}

/// `sin_dato` que no tienen un campo propio (las cotas y el DN ya se comparan como `—`).
fn sin_dato(out: &mut Vec<Cambio>, a: &[String], b: &[String], propios: &[&str]) {
    let sa: BTreeSet<&str> = a.iter().map(String::as_str).collect();
    let sb: BTreeSet<&str> = b.iter().map(String::as_str).collect();
    for k in sa.symmetric_difference(&sb) {
        if propios.contains(k) {
            continue;
        }
        let (antes, despues) = if sa.contains(k) {
            ("sin dato", "con dato")
        } else {
            ("con dato", "sin dato")
        };
        out.push(cambio(
            Clase::Diseno,
            &format!("sin_dato.{k}"),
            antes.into(),
            despues.into(),
        ));
    }
}

/// Campos de revisión: el estado y cada observación por su id (F13-03). Una observación que
/// aparece, desaparece o cambia (se cierra, cambia su texto) es un cambio `observaciones.<id>`
/// con `estado: texto` antes y después.
fn revision(out: &mut Vec<Cambio>, estado: (&str, &str), obs: (&[Observacion], &[Observacion])) {
    if estado.0 != estado.1 {
        out.push(cambio(
            Clase::Revision,
            "estado",
            estado.0.into(),
            estado.1.into(),
        ));
    }
    let ids: BTreeSet<u32> = obs.0.iter().chain(obs.1).map(|o| o.id).collect();
    for id in ids {
        let buscar = |v: &[Observacion]| v.iter().find(|o| o.id == id).cloned();
        let (oa, ob) = (buscar(obs.0), buscar(obs.1));
        if oa == ob {
            continue;
        }
        let t = |o: &Option<Observacion>| {
            o.as_ref()
                .map(|o| format!("{}: {}", o.estado.key(), o.texto))
                .unwrap_or_else(|| SIN_DATO.into())
        };
        out.push(cambio(
            Clase::Revision,
            &format!("observaciones.{id}"),
            t(&oa),
            t(&ob),
        ));
    }
}

fn cambios_nodo(a: &Node, b: &Node, op: &Opciones) -> Vec<Cambio> {
    let mut c = Vec::new();
    if a.family != b.family {
        c.push(cambio(
            Clase::Diseno,
            "familia",
            a.family.clone(),
            b.family.clone(),
        ));
    }
    if a.ty != b.ty {
        c.push(cambio(Clase::Diseno, "subtipo", a.ty.clone(), b.ty.clone()));
    }
    if a.pos.dist(b.pos) > op.tolerancia_pos {
        c.push(cambio(
            Clase::Diseno,
            "posicion",
            format!("{} ; {}", num(a.pos.x), num(a.pos.y)),
            format!("{} ; {}", num(b.pos.x), num(b.pos.y)),
        ));
    }
    let t = op.tolerancia_cota;
    cota(
        &mut c,
        "cota_terreno",
        a.cota_terreno(),
        b.cota_terreno(),
        t,
    );
    cota(&mut c, "cota_tapa", a.cota_tapa(), b.cota_tapa(), t);
    cota(&mut c, "cota_solera", a.cota_solera(), b.cota_solera(), t);
    atributos(&mut c, &a.attrs, &b.attrs, &[]);
    datos(&mut c, &a.datos, &b.datos);
    sin_dato(
        &mut c,
        &a.sin_dato,
        &b.sin_dato,
        &[COTA_TERRENO, COTA_TAPA, COTA_SOLERA],
    );
    if a.modelo != b.modelo {
        c.push(cambio(
            Clase::Diseno,
            "modelo",
            texto_opt(a.modelo.as_deref()),
            texto_opt(b.modelo.as_deref()),
        ));
    }
    revision(
        &mut c,
        (a.estado.key(), b.estado.key()),
        (&a.observaciones, &b.observaciones),
    );
    c
}

fn codigo_nodo(p: &Project, id: crate::model::ElementId) -> String {
    p.node(id)
        .map(|n| n.code.clone())
        .unwrap_or_else(|| SIN_DATO.into())
}

fn cambios_tramo(pa: &Project, a: &Link, pb: &Project, b: &Link, op: &Opciones) -> Vec<Cambio> {
    let mut c = Vec::new();
    if a.family != b.family {
        c.push(cambio(
            Clase::Diseno,
            "familia",
            a.family.clone(),
            b.family.clone(),
        ));
    }
    if a.ty != b.ty {
        c.push(cambio(Clase::Diseno, "subtipo", a.ty.clone(), b.ty.clone()));
    }
    let (da, db) = (codigo_nodo(pa, a.from), codigo_nodo(pb, b.from));
    if da != db {
        c.push(cambio(Clase::Diseno, "desde", da, db));
    }
    let (ha, hb) = (codigo_nodo(pa, a.to), codigo_nodo(pb, b.to));
    if ha != hb {
        c.push(cambio(Clase::Diseno, "hasta", ha, hb));
    }
    if a.dn() != b.dn() {
        let f = |v: Option<u32>| v.map(|d| d.to_string()).unwrap_or_else(|| SIN_DATO.into());
        c.push(cambio(Clase::Diseno, "dn", f(a.dn()), f(b.dn())));
    }
    let t = op.tolerancia_cota;
    cota(
        &mut c,
        "cota_solera_inicio",
        a.cota_solera_inicio,
        b.cota_solera_inicio,
        t,
    );
    cota(
        &mut c,
        "cota_solera_fin",
        a.cota_solera_fin,
        b.cota_solera_fin,
        t,
    );
    // Trazado: planta y, si la cota es dato en ambas entregas, la cota de cada vértice.
    if a.path.len() != b.path.len() {
        c.push(cambio(
            Clase::Diseno,
            "trazado.vertices",
            a.path.len().to_string(),
            b.path.len().to_string(),
        ));
    } else {
        let con_cota = !a.falta(COTA_SOLERA) && !b.falta(COTA_SOLERA);
        let mut movidos = Vec::new();
        let mut cota_movida = Vec::new();
        for (i, (p, q)) in a.path.iter().zip(&b.path).enumerate() {
            if P2::new(p.x, p.y).dist(P2::new(q.x, q.y)) > op.tolerancia_pos {
                movidos.push((i + 1).to_string());
            }
            if con_cota && (p.z - q.z).abs() > t {
                cota_movida.push((i + 1).to_string());
            }
        }
        if !movidos.is_empty() {
            c.push(cambio(
                Clase::Diseno,
                "trazado.planta",
                "vertices originales".into(),
                format!("movidos: {}", movidos.join(",")),
            ));
        }
        if !cota_movida.is_empty() {
            c.push(cambio(
                Clase::Diseno,
                "trazado.cota",
                "cotas originales".into(),
                format!("vertices con otra cota: {}", cota_movida.join(",")),
            ));
        }
    }
    if a.falta(COTA_SOLERA) != b.falta(COTA_SOLERA) {
        c.push(cambio(
            Clase::Diseno,
            "cota_solera",
            if a.falta(COTA_SOLERA) {
                SIN_DATO
            } else {
                "con dato"
            }
            .into(),
            if b.falta(COTA_SOLERA) {
                SIN_DATO
            } else {
                "con dato"
            }
            .into(),
        ));
    }
    atributos(&mut c, &a.attrs, &b.attrs, &[]);
    datos(&mut c, &a.datos, &b.datos);
    sin_dato(
        &mut c,
        &a.sin_dato,
        &b.sin_dato,
        &[COTA_SOLERA, DIAMETRO_NOMINAL],
    );
    revision(
        &mut c,
        (a.estado.key(), b.estado.key()),
        (&a.observaciones, &b.observaciones),
    );
    c
}

/// CSV de los cambios: BOM y `;`, una fila por cambio (y por alta o baja).
pub fn a_csv(d: &Diff) -> String {
    let lim = |s: &str| s.replace([';', '\n', '\r'], " ");
    let mut s = String::from("\u{feff}tipo;codigo;familia;cambio;clase;campo;antes;despues\n");
    for x in &d.altas {
        s.push_str(&format!(
            "{};{};{};alta;diseno;;;\n",
            x.tipo,
            lim(&x.codigo),
            x.familia
        ));
    }
    for x in &d.bajas {
        s.push_str(&format!(
            "{};{};{};baja;diseno;;;\n",
            x.tipo,
            lim(&x.codigo),
            x.familia
        ));
    }
    for m in &d.modificados {
        for c in &m.cambios {
            s.push_str(&format!(
                "{};{};{};modificado;{};{};{};{}\n",
                m.tipo,
                lim(&m.codigo),
                m.familia,
                c.clase.key(),
                c.campo,
                lim(&c.antes),
                lim(&c.despues)
            ));
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::EstadoObjeto;

    fn base() -> Project {
        crate::demo::project()
    }

    #[test]
    fn iguales_no_dan_nada() {
        let d = comparar(&base(), &base(), &Opciones::default()).unwrap();
        assert!(d.vacio());
    }

    #[test]
    fn detecta_alta_baja_cota_dn_dato_y_respeta_la_tolerancia() {
        let a = base();
        let mut b = base();
        // baja: el ultimo tramo; alta: un nodo nuevo
        let baja = b.links.pop().unwrap();
        let mut nuevo = b.nodes[0].clone();
        nuevo.code = "PZ-NUEVO".into();
        nuevo.id = crate::model::ElementId::new(9000, 1);
        b.nodes.push(nuevo);
        // cota
        b.nodes[1].z_invert -= 0.25;
        // DN
        let li = b.links.iter().position(|l| l.dn().is_some()).unwrap();
        let dn_antes = b.links[li].d_mm;
        b.links[li].d_mm += 100;
        // dato que desaparece
        let ni = b.nodes.iter().position(|n| !n.datos.is_empty()).unwrap();
        let clave = b.nodes[ni].datos.keys().next().unwrap().clone();
        b.nodes[ni].datos.remove(&clave);
        // movimiento por debajo de la tolerancia
        b.nodes[2].pos.x += 0.004;
        // y de revision
        b.nodes[3].estado = EstadoObjeto::Aprobado;

        let d = comparar(&a, &b, &Opciones::default()).unwrap();
        assert!(d.bajas.iter().any(|x| x.codigo == baja.code));
        assert!(d.altas.iter().any(|x| x.codigo == "PZ-NUEVO"));
        let campos = |cod: &str| -> Vec<String> {
            d.modificados
                .iter()
                .filter(|m| m.codigo == cod)
                .flat_map(|m| m.cambios.iter().map(|c| c.campo.clone()))
                .collect()
        };
        assert_eq!(campos(&b.nodes[1].code), vec!["cota_solera"]);
        assert_eq!(campos(&b.links[li].code), vec!["dn"]);
        let ch = d
            .modificados
            .iter()
            .find(|m| m.codigo == b.links[li].code)
            .unwrap();
        assert_eq!(ch.cambios[0].antes, dn_antes.to_string());
        assert_eq!(campos(&b.nodes[ni].code), vec![format!("datos.{clave}")]);
        assert!(campos(&b.nodes[2].code).is_empty(), "0,004 m < 0,01 m");
        assert_eq!(campos(&b.nodes[3].code), vec!["estado"]);

        let solo_diseno = comparar(
            &a,
            &b,
            &Opciones {
                solo: Some(Clase::Diseno),
                ..Default::default()
            },
        )
        .unwrap();
        assert!(
            solo_diseno
                .modificados
                .iter()
                .all(|m| m.codigo != b.nodes[3].code)
        );
        let solo_rev = comparar(
            &a,
            &b,
            &Opciones {
                solo: Some(Clase::Revision),
                ..Default::default()
            },
        )
        .unwrap();
        assert!(solo_rev.altas.is_empty() && solo_rev.bajas.is_empty());
        assert_eq!(solo_rev.modificados.len(), 1);

        // con tolerancia menor se ve el movimiento
        let fino = comparar(
            &a,
            &b,
            &Opciones {
                tolerancia_pos: 0.001,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(fino.modificados.iter().any(|m| m.codigo == b.nodes[2].code));
    }

    #[test]
    fn un_dato_que_aparece_es_cambio_y_el_crs_distinto_es_error() {
        let a = base();
        let mut b = base();
        b.nodes[0].sin_dato.push(COTA_TAPA.into());
        let d = comparar(&a, &b, &Opciones::default()).unwrap();
        let m = &d.modificados[0];
        assert_eq!(m.cambios[0].campo, "cota_tapa");
        assert_eq!(m.cambios[0].despues, SIN_DATO);
        b.crs = "EPSG:32718".into();
        let e = comparar(&a, &b, &Opciones::default()).unwrap_err();
        assert!(e.starts_with("E_STATE_CRS_DISTINTO"));
    }

    /// F13-03: cada observación se compara por su id y se informa con `estado: texto`.
    #[test]
    fn las_observaciones_se_comparan_por_id() {
        let a = base();
        let mut b = base();
        let c = b.links[0].code.clone();
        b.observar(&c, "Revisar pendiente", None).unwrap();
        b.observar(&c, "Falta DN", None).unwrap();
        b.cerrar_observacion(&c, 1, None).unwrap();
        let d = comparar(&a, &b, &Opciones::default()).unwrap();
        let m = d.modificados.iter().find(|m| m.codigo == c).unwrap();
        let campos: Vec<(&str, &str, &str)> = m
            .cambios
            .iter()
            .map(|x| (x.campo.as_str(), x.antes.as_str(), x.despues.as_str()))
            .collect();
        assert_eq!(
            campos,
            vec![
                ("estado", a.links[0].estado.key(), "observado"),
                ("observaciones.1", SIN_DATO, "cerrada: Revisar pendiente"),
                ("observaciones.2", SIN_DATO, "abierta: Falta DN"),
            ]
        );
        assert!(m.cambios.iter().all(|x| x.clase == Clase::Revision));
    }

    #[test]
    fn el_trazado_y_el_orden_son_deterministas() {
        let a = base();
        let mut b = base();
        b.links[0].path[0].x += 1.0;
        let d1 = comparar(&a, &b, &Opciones::default()).unwrap();
        let d2 = comparar(&a, &b, &Opciones::default()).unwrap();
        assert_eq!(d1, d2);
        assert_eq!(d1.modificados[0].cambios[0].campo, "trazado.planta");
        assert!(a_csv(&d1).starts_with('\u{feff}'));
    }
}
