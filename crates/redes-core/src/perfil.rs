//! Perfil longitudinal de un colector (F13-02).
//!
//! Dado un recorrido (tramos consecutivos en el sentido declarado) produce las abscisas en planta
//! y, por pozo y por tramo, las cotas **que trae el archivo**. No calcula hidráulica: la pendiente
//! sale de las soleras declaradas y la clave es solera + diámetro interior. Lo que falta queda en
//! `None` (hueco); nunca se interpola como si fuera dato. La única cota que no es del objeto es la
//! rasante muestreada del TIN entre pozos, marcada con su origen.

use std::collections::{BTreeSet, VecDeque};

use serde::{Deserialize, Serialize};

use crate::geom::P2;
use crate::model::{COTA_SOLERA, ElementId, Link, Project};

/// Error de un recorrido inválido, con código del contrato `redes.envelope/1`.
#[derive(Clone, Debug, PartialEq)]
pub struct PerfilError {
    pub code: &'static str,
    pub message: String,
}

impl PerfilError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        PerfilError {
            code,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for PerfilError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

/// Un pozo (nodo) del recorrido.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PozoPerfil {
    pub codigo: String,
    pub abscisa: f64,
    pub terreno: Option<f64>,
    pub tapa: Option<f64>,
    pub solera: Option<f64>,
    /// (tapa o, si falta, terreno) − solera; `None` si falta alguno.
    pub profundidad: Option<f64>,
}

/// Un tramo del recorrido.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TramoPerfil {
    pub codigo: String,
    pub desde: String,
    pub hasta: String,
    pub abscisa_ini: f64,
    pub abscisa_fin: f64,
    /// Longitud en planta (m).
    pub longitud: f64,
    pub dn_mm: Option<u32>,
    /// Diámetro interior (m), declarado o el nominal; `None` si no hay ninguno.
    pub di_m: Option<f64>,
    pub solera_ini: Option<f64>,
    pub solera_fin: Option<f64>,
    pub clave_ini: Option<f64>,
    pub clave_fin: Option<f64>,
    /// Pendiente de solera en el sentido del flujo (m/m, positiva si baja). `None` si falta una
    /// solera o la longitud es nula.
    pub pendiente: Option<f64>,
}

/// Punto de la rasante a lo largo del recorrido.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PuntoRasante {
    pub abscisa: f64,
    pub z: Option<f64>,
    /// `"nodo"` (cota de terreno del pozo) o `"tin"` (muestreo del terreno).
    pub origen: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Perfil {
    pub tramos: Vec<TramoPerfil>,
    pub pozos: Vec<PozoPerfil>,
    pub rasante: Vec<PuntoRasante>,
    pub longitud_total: f64,
    /// Paso de muestreo del TIN (m); 0 = sin muestreo intermedio.
    pub paso_m: f64,
}

fn longitud_planta(l: &Link, a: P2, b: P2) -> f64 {
    let s: f64 = l.path.windows(2).map(|w| w[0].p2().dist(w[1].p2())).sum();
    if s > 0.0 { s } else { a.dist(b) }
}

/// Punto a `s` metros del inicio del trazado en planta.
fn punto_a(l: &Link, a: P2, b: P2, s: f64) -> P2 {
    let mut resto = s;
    for w in l.path.windows(2) {
        let (p, q) = (w[0].p2(), w[1].p2());
        let d = p.dist(q);
        if resto <= d && d > 0.0 {
            let t = resto / d;
            return P2::new(p.x + (q.x - p.x) * t, p.y + (q.y - p.y) * t);
        }
        resto -= d;
    }
    let _ = a;
    b
}

fn solera_tramo(l: &Link) -> (Option<f64>, Option<f64>) {
    if l.falta(COTA_SOLERA) && l.cota_solera_inicio.is_none() && l.cota_solera_fin.is_none() {
        return (None, None);
    }
    let ini = l.cota_solera_inicio.or_else(|| {
        (!l.falta(COTA_SOLERA))
            .then(|| l.path.first().map(|p| p.z))
            .flatten()
    });
    let fin = l.cota_solera_fin.or_else(|| {
        (!l.falta(COTA_SOLERA))
            .then(|| l.path.last().map(|p| p.z))
            .flatten()
    });
    (ini, fin)
}

fn di_tramo(l: &Link) -> Option<f64> {
    l.num("diametro_interior")
        .filter(|d| *d > 0.0)
        .map(|d| d / 1000.0)
        .or_else(|| l.dn().map(|d| d as f64 / 1000.0))
}

/// Construye el perfil de una lista de códigos de tramo consecutivos (sentido declarado).
///
/// `paso_m` es la separación del muestreo del terreno entre pozos (0 = solo los pozos).
pub fn desde_tramos(p: &Project, codigos: &[String], paso_m: f64) -> Result<Perfil, PerfilError> {
    if codigos.is_empty() {
        return Err(PerfilError::new(
            "E_ARG_MISSING",
            "el recorrido no tiene tramos",
        ));
    }
    let mut links: Vec<&Link> = Vec::new();
    for c in codigos {
        let l = p.link_by_code(c).ok_or_else(|| {
            PerfilError::new("E_STATE_NOT_FOUND", format!("el tramo '{c}' no existe"))
        })?;
        links.push(l);
    }
    for w in links.windows(2) {
        if w[0].to != w[1].from {
            return Err(PerfilError::new(
                "E_TOPO_NO_CONSECUTIVO",
                format!(
                    "el tramo {} no continúa en {}: termina en {} y {} empieza en {} (se sigue el sentido declarado)",
                    w[0].code,
                    w[1].code,
                    nodo_cod(p, w[0].to),
                    w[1].code,
                    nodo_cod(p, w[1].from)
                ),
            ));
        }
    }
    construir(p, &links, paso_m)
}

fn nodo_cod(p: &Project, id: ElementId) -> String {
    p.node(id).map_or("?".to_string(), |n| n.code.clone())
}

/// Perfil entre dos pozos siguiendo el sentido declarado; si hay varios caminos, el de menos
/// tramos (y, a igualdad, el primero en el orden del proyecto).
pub fn desde_pozos(
    p: &Project,
    desde: &str,
    hasta: &str,
    paso_m: f64,
) -> Result<Perfil, PerfilError> {
    let a = p.node_by_code(desde).ok_or_else(|| {
        PerfilError::new("E_STATE_NOT_FOUND", format!("el pozo '{desde}' no existe"))
    })?;
    let b = p.node_by_code(hasta).ok_or_else(|| {
        PerfilError::new("E_STATE_NOT_FOUND", format!("el pozo '{hasta}' no existe"))
    })?;
    if a.id == b.id {
        return Err(PerfilError::new(
            "E_TOPO_SIN_RUTA",
            "el pozo de inicio y el de fin son el mismo",
        ));
    }
    let mut previo: std::collections::BTreeMap<ElementId, &Link> = Default::default();
    let mut visto: BTreeSet<ElementId> = BTreeSet::from([a.id]);
    let mut cola = VecDeque::from([a.id]);
    while let Some(n) = cola.pop_front() {
        if n == b.id {
            break;
        }
        for l in p.tramos_de(n).filter(|l| l.from == n) {
            if visto.insert(l.to) {
                previo.insert(l.to, l);
                cola.push_back(l.to);
            }
        }
    }
    if !previo.contains_key(&b.id) {
        return Err(PerfilError::new(
            "E_TOPO_SIN_RUTA",
            format!(
                "no hay recorrido de {} a {} siguiendo el sentido declarado de los tramos",
                a.code, b.code
            ),
        ));
    }
    let mut links = Vec::new();
    let mut cur = b.id;
    while cur != a.id {
        let l = previo[&cur];
        links.push(l);
        cur = l.from;
    }
    links.reverse();
    construir(p, &links, paso_m)
}

/// Colector que contiene un tramo: aguas arriba hasta la cabecera y aguas abajo hasta la descarga,
/// de la misma familia. En una bifurcación toma el tramo de mayor DN (a igualdad, el de menor
/// código). Devuelve los códigos en orden.
pub fn colector_de(p: &Project, codigo: &str) -> Result<Vec<String>, PerfilError> {
    let semilla = p.link_by_code(codigo).ok_or_else(|| {
        PerfilError::new(
            "E_STATE_NOT_FOUND",
            format!("el tramo '{codigo}' no existe"),
        )
    })?;
    let mejor = |mut v: Vec<&Link>| -> Option<ElementId> {
        v.sort_by(|a, b| b.d_mm.cmp(&a.d_mm).then_with(|| a.code.cmp(&b.code)));
        v.first().map(|l| l.id)
    };
    let mut visto: BTreeSet<ElementId> = BTreeSet::from([semilla.id]);
    let mut arriba: Vec<&Link> = Vec::new();
    let mut cur = semilla;
    loop {
        let cand: Vec<&Link> = p
            .links
            .iter()
            .filter(|l| {
                !l.deleted
                    && l.family == semilla.family
                    && l.to == cur.from
                    && !visto.contains(&l.id)
            })
            .collect();
        let Some(id) = mejor(cand) else { break };
        let l = p.link(id).expect("existe");
        visto.insert(id);
        arriba.push(l);
        cur = l;
    }
    arriba.reverse();
    let mut todos = arriba;
    todos.push(semilla);
    cur = semilla;
    loop {
        let cand: Vec<&Link> = p
            .links
            .iter()
            .filter(|l| {
                !l.deleted
                    && l.family == semilla.family
                    && l.from == cur.to
                    && !visto.contains(&l.id)
            })
            .collect();
        let Some(id) = mejor(cand) else { break };
        let l = p.link(id).expect("existe");
        visto.insert(id);
        todos.push(l);
        cur = l;
    }
    Ok(todos.iter().map(|l| l.code.clone()).collect())
}

fn construir(p: &Project, links: &[&Link], paso_m: f64) -> Result<Perfil, PerfilError> {
    let paso = if paso_m.is_finite() && paso_m > 0.0 {
        paso_m
    } else {
        0.0
    };
    let mut tramos = Vec::new();
    let mut pozos = Vec::new();
    let mut rasante: Vec<PuntoRasante> = Vec::new();
    let mut s0 = 0.0;

    let pozo = |p: &Project, id: ElementId, abscisa: f64, rasante: &mut Vec<PuntoRasante>| {
        let n = p.node(id);
        let (terreno, tapa, solera) = n.map_or((None, None, None), |n| {
            (n.cota_terreno(), n.cota_tapa(), n.cota_solera())
        });
        let profundidad = match (tapa.or(terreno), solera) {
            (Some(a), Some(b)) => Some((a - b).max(0.0)),
            _ => None,
        };
        let (z, origen) = match (terreno, n.and_then(|n| p.terrain.as_ref()?.z_at(n.pos))) {
            (Some(z), _) => (Some(z), "nodo"),
            (None, Some(z)) => (Some(z), "tin"),
            (None, None) => (None, "nodo"),
        };
        rasante.push(PuntoRasante {
            abscisa,
            z,
            origen: origen.into(),
        });
        PozoPerfil {
            codigo: n.map_or("?".into(), |n| n.code.clone()),
            abscisa,
            terreno,
            tapa,
            solera,
            profundidad,
        }
    };

    pozos.push(pozo(p, links[0].from, 0.0, &mut rasante));
    for l in links {
        let (na, nb) = (p.node(l.from), p.node(l.to));
        let (a, b) = (
            l.path.first().map(|v| v.p2()).or(na.map(|n| n.pos)),
            l.path.last().map(|v| v.p2()).or(nb.map(|n| n.pos)),
        );
        let (a, b) = (
            a.unwrap_or(P2::new(0.0, 0.0)),
            b.unwrap_or(P2::new(0.0, 0.0)),
        );
        let long = longitud_planta(l, a, b);
        if paso > 0.0
            && let Some(t) = &p.terrain
        {
            let mut s = paso;
            while s < long - paso * 0.25 {
                rasante.push(PuntoRasante {
                    abscisa: s0 + s,
                    z: t.z_at(punto_a(l, a, b, s)),
                    origen: "tin".into(),
                });
                s += paso;
            }
        }
        let (si, sf) = solera_tramo(l);
        let di = di_tramo(l);
        let clave = |s: Option<f64>| s.zip(di).map(|(s, d)| s + d);
        let pendiente = match (si, sf) {
            (Some(i), Some(f)) if long > 1e-9 => Some((i - f) / long),
            _ => None,
        };
        tramos.push(TramoPerfil {
            codigo: l.code.clone(),
            desde: nodo_cod(p, l.from),
            hasta: nodo_cod(p, l.to),
            abscisa_ini: s0,
            abscisa_fin: s0 + long,
            longitud: long,
            dn_mm: l.dn(),
            di_m: di,
            solera_ini: si,
            solera_fin: sf,
            clave_ini: clave(si),
            clave_fin: clave(sf),
            pendiente,
        });
        s0 += long;
        pozos.push(pozo(p, l.to, s0, &mut rasante));
    }
    Ok(Perfil {
        tramos,
        pozos,
        rasante,
        longitud_total: s0,
        paso_m: paso,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::demo;

    fn codigos_troncal(p: &Project) -> Vec<String> {
        let a = p
            .nodes
            .iter()
            .find(|n| n.family == "saneamiento.pozo")
            .unwrap();
        let _ = a;
        p.links
            .iter()
            .filter(|l| l.ty == "colector_sanitario")
            .take(7)
            .map(|l| l.code.clone())
            .collect()
    }

    #[test]
    fn el_perfil_de_la_demo_coincide_con_las_cotas_de_los_objetos() {
        let p = demo::project();
        let cods = codigos_troncal(&p);
        assert_eq!(cods.len(), 7);
        let pf = desde_tramos(&p, &cods, 10.0).unwrap();
        assert_eq!(pf.tramos.len(), 7);
        assert_eq!(pf.pozos.len(), 8);
        for (t, c) in pf.tramos.iter().zip(&cods) {
            let l = p.link_by_code(c).unwrap();
            assert_eq!(t.solera_ini, Some(l.path[0].z));
            assert_eq!(t.solera_fin, Some(l.path[1].z));
            assert_eq!(t.dn_mm, Some(250));
            let di = t.di_m.unwrap();
            assert!((t.clave_ini.unwrap() - (l.path[0].z + di)).abs() < 1e-9);
            assert!(t.pendiente.is_some());
        }
        for z in &pf.pozos {
            let n = p.node_by_code(&z.codigo).unwrap();
            assert_eq!(z.terreno, Some(n.z_terrain));
            assert_eq!(z.tapa, Some(n.z_rim));
            assert_eq!(z.solera, Some(n.z_invert));
            assert!((z.profundidad.unwrap() - (n.z_rim - n.z_invert)).abs() < 1e-9);
        }
        let ult = pf.tramos.last().unwrap();
        assert!((pf.longitud_total - ult.abscisa_fin).abs() < 1e-9);
        // Hay muestreo del terreno entre pozos.
        assert!(
            pf.rasante
                .iter()
                .any(|r| r.origen == "tin" && r.z.is_some())
        );
    }

    #[test]
    fn desde_hasta_sigue_el_sentido_declarado() {
        let p = demo::project();
        let cods = codigos_troncal(&p);
        let primero = p.link_by_code(&cods[0]).unwrap();
        let ultimo = p.link_by_code(&cods[6]).unwrap();
        let (a, b) = (nodo_cod(&p, primero.from), nodo_cod(&p, ultimo.to));
        let pf = desde_pozos(&p, &a, &b, 0.0).unwrap();
        let obtenidos: Vec<_> = pf.tramos.iter().map(|t| t.codigo.clone()).collect();
        assert_eq!(obtenidos, cods);
        // Al revés no hay ruta: el sentido es el declarado.
        let e = desde_pozos(&p, &b, &a, 0.0).unwrap_err();
        assert_eq!(e.code, "E_TOPO_SIN_RUTA");
        assert!(pf.rasante.iter().all(|r| r.origen == "nodo"));
    }

    #[test]
    fn falta_la_solera_hueco_sin_pendiente_ni_clave() {
        let mut p = demo::project();
        let cods = codigos_troncal(&p);
        let l = p.links.iter_mut().find(|l| l.code == cods[1]).unwrap();
        l.sin_dato.push(COTA_SOLERA.into());
        let pf = desde_tramos(&p, &cods, 0.0).unwrap();
        let t = &pf.tramos[1];
        assert!(t.solera_ini.is_none() && t.solera_fin.is_none());
        assert!(t.clave_ini.is_none() && t.pendiente.is_none());
        assert!(pf.tramos[0].pendiente.is_some());
        // Un pozo sin solera: sin profundidad.
        let id = p.link_by_code(&cods[0]).unwrap().from;
        p.nodes
            .iter_mut()
            .find(|n| n.id == id)
            .unwrap()
            .sin_dato
            .push(COTA_SOLERA.into());
        let pf = desde_tramos(&p, &cods, 0.0).unwrap();
        assert!(pf.pozos[0].solera.is_none() && pf.pozos[0].profundidad.is_none());
    }

    #[test]
    fn recorridos_invalidos_dan_error_claro() {
        let p = demo::project();
        let cods = codigos_troncal(&p);
        assert_eq!(
            desde_tramos(&p, &["NOEXISTE".into()], 0.0)
                .unwrap_err()
                .code,
            "E_STATE_NOT_FOUND"
        );
        assert_eq!(
            desde_tramos(&p, &[], 0.0).unwrap_err().code,
            "E_ARG_MISSING"
        );
        // Saltarse un tramo no es consecutivo.
        let e = desde_tramos(&p, &[cods[0].clone(), cods[2].clone()], 0.0).unwrap_err();
        assert_eq!(e.code, "E_TOPO_NO_CONSECUTIVO");
        assert!(e.message.contains(&cods[0]));
        assert_eq!(
            desde_pozos(&p, "XX", "YY", 0.0).unwrap_err().code,
            "E_STATE_NOT_FOUND"
        );
    }

    #[test]
    fn el_colector_va_de_la_cabecera_a_la_descarga() {
        let p = demo::project();
        let cods = codigos_troncal(&p);
        let c = colector_de(&p, &cods[3]).unwrap();
        assert_eq!(&c[..7], &cods[..]);
    }
}
