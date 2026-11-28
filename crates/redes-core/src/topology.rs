//! Capa de topología: el grafo que se **deriva** del modelo.
//!
//! Los objetos no se conocen entre sí. Un arco declara `from`/`to`; esta capa responde:
//! ¿qué arcos llegan a este nodo?, ¿qué componentes conexas hay?, ¿en qué sentido fluye?,
//! ¿el conjunto alcanza una fuente?
//!
//! Se reconstruye entera cuando cambia el documento (v0.1: barato para miles de objetos).
//! La reconstrucción incremental y el `epoch` son la evolución natural (ADR-T1).

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde::{Deserialize, Serialize};

use crate::model::{ElementId, Project};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Topology {
    /// Arcos incidentes por nodo (ids de arcos, en orden de inserción).
    pub incident: BTreeMap<ElementId, Vec<ElementId>>,
    /// Grado de cada nodo (número de arcos incidentes).
    pub degree: BTreeMap<ElementId, usize>,
    /// Componentes conexas (ids de nodos), en orden determinista.
    pub components: Vec<Vec<ElementId>>,
    /// Sentido de flujo por arco: `true` si va de `from` a `to`. Es el sentido **declarado**
    /// (from→to); nunca se invierte por las cotas.
    pub flow_forward: BTreeMap<ElementId, bool>,
    /// Arcos por gravedad cuya cota sube en el sentido declarado (contrapendiente). Es un aviso:
    /// no cambia el sentido ni ningún dato. Las redes a presión no entran aquí.
    #[serde(default)]
    pub contrapendiente: Vec<ElementId>,
    /// Nodos que no alcanzan ninguna fuente.
    pub stranded: Vec<ElementId>,
}

impl Topology {
    /// Deriva el grafo del documento.
    pub fn build(p: &Project) -> Topology {
        let mut incident: BTreeMap<ElementId, Vec<ElementId>> = BTreeMap::new();
        for n in p.nodes.iter().filter(|n| !n.deleted) {
            incident.entry(n.id).or_default();
        }
        let mut flow_forward = BTreeMap::new();
        let mut contrapendiente = Vec::new();
        for l in p.links.iter().filter(|l| !l.deleted) {
            // El sentido es el declarado (from→to). Las cotas solo sirven para avisar.
            flow_forward.insert(l.id, true);
            if !es_a_presion(&l.family, &l.ty)
                && let (Some(a), Some(b)) = (l.path.first(), l.path.last())
                && a.z < b.z - 1e-9
            {
                contrapendiente.push(l.id);
            }
            incident.entry(l.from).or_default().push(l.id);
            incident.entry(l.to).or_default().push(l.id);
        }
        // Dispositivos de enlace (F12-15): una bomba o una válvula que en el origen unía dos nodos
        // sin tramo propio. No son arcos (no hay tubo que dibujar), pero la red no está cortada.
        let vivo = |id: ElementId| p.node(id).is_some_and(|n| !n.deleted);
        let mut unidos: BTreeMap<ElementId, Vec<ElementId>> = BTreeMap::new();
        for e in &p.enlaces {
            if !vivo(e.dispositivo) {
                continue;
            }
            for otro in [e.desde, e.hasta].into_iter().flatten() {
                if otro != e.dispositivo && vivo(otro) {
                    unidos.entry(e.dispositivo).or_default().push(otro);
                    unidos.entry(otro).or_default().push(e.dispositivo);
                }
            }
        }
        let degree = incident
            .iter()
            .map(|(k, v)| (*k, v.len() + unidos.get(k).map_or(0, |u| u.len())))
            .collect();

        // Componentes conexas por BFS sobre nodos vivos.
        let mut seen: BTreeSet<ElementId> = BTreeSet::new();
        let mut components = Vec::new();
        for n in p.nodes.iter().filter(|n| !n.deleted) {
            if !seen.insert(n.id) {
                continue;
            }
            let mut comp = Vec::new();
            let mut q = VecDeque::new();
            q.push_back(n.id);
            while let Some(cur) = q.pop_front() {
                comp.push(cur);
                for &lid in incident.get(&cur).into_iter().flatten() {
                    if let Some(l) = p.link(lid) {
                        let other = if l.from == cur { l.to } else { l.from };
                        if seen.insert(other) {
                            q.push_back(other);
                        }
                    }
                }
                for &other in unidos.get(&cur).into_iter().flatten() {
                    if seen.insert(other) {
                        q.push_back(other);
                    }
                }
            }
            comp.sort();
            components.push(comp);
        }
        components.sort();

        // Nodos que no alcanzan una fuente (pozo de bombeo, tanque, captación, planta).
        let sources: BTreeSet<ElementId> = p
            .nodes
            .iter()
            .filter(|n| !n.deleted && (is_source(&n.family) || is_sink(&n.family)))
            .map(|n| n.id)
            .collect();
        let mut stranded = Vec::new();
        if !sources.is_empty() {
            for comp in &components {
                let has_source = comp.iter().any(|id| sources.contains(id));
                if !has_source {
                    stranded.extend(comp.iter().copied());
                }
            }
        }

        Topology {
            incident,
            degree,
            components,
            flow_forward,
            contrapendiente,
            stranded,
        }
    }

    pub fn links_at(&self, node: ElementId) -> &[ElementId] {
        self.incident
            .get(&node)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    pub fn degree_of(&self, node: ElementId) -> usize {
        self.degree.get(&node).copied().unwrap_or(0)
    }

    /// Nodos de grado 0 (aislados).
    pub fn isolated<'a>(&'a self, p: &'a Project) -> impl Iterator<Item = ElementId> + 'a {
        p.nodes
            .iter()
            .filter(|n| !n.deleted && self.degree_of(n.id) == 0)
            .map(|n| n.id)
    }
}

/// Tramo a presión (impulsión, conducción forzada, agua potable): la regla de pendiente no aplica.
/// Se deriva del registro: familia de agua, o subtipo que exige clase de presión.
pub(crate) fn es_a_presion(family: &str, ty: &str) -> bool {
    let Some(def) = crate::families::family(family) else {
        return false;
    };
    def.category == crate::families::Category::Agua
        || def
            .ty(ty)
            .is_some_and(|t| t.required.contains(&"clase_presion"))
}

/// Sumideros de una red de gravedad: descarga, cuerpo receptor y obras de tratamiento.
fn is_sink(family: &str) -> bool {
    matches!(
        family,
        "saneamiento.descarga"
            | "comun.cuerpo_receptor"
            | "saneamiento.reactor"
            | "saneamiento.laguna"
            | "saneamiento.sedimentador"
    )
}

fn is_source(family: &str) -> bool {
    matches!(
        family,
        "agua.tanque" | "agua.captacion" | "agua.planta" | "saneamiento.bombeo"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;
    use std::collections::BTreeMap;

    fn p_with_chain(n: usize) -> (Project, Vec<ElementId>) {
        let mut p = Project::new("t", "EPSG:32717");
        let mut ids = Vec::new();
        for i in 0..n {
            let id = p
                .add_node(Node {
                    id: ElementId::new(0, 0),
                    family: "saneamiento.pozo".into(),
                    ty: "inspeccion".into(),
                    code: String::new(),
                    pos: crate::geom::P2::new(i as f64 * 50.0, 0.0),
                    z_terrain: 102.0 - i as f64 * 0.3,
                    z_rim: 102.0 - i as f64 * 0.3,
                    z_invert: 100.0 - i as f64 * 0.3,
                    attrs: BTreeMap::new(),
                    datos: BTreeMap::new(),
                    estado: EstadoObjeto::Pendiente,
                    observaciones: Vec::new(),
                    modelo: None,
                    deleted: false,
                    sin_dato: Vec::new(),
                })
                .unwrap();
            ids.push(id);
        }
        for (i, w) in ids.windows(2).enumerate() {
            p.add_link(Link {
                id: ElementId::new(0, 0),
                family: "saneamiento.tuberia".into(),
                ty: "colector_sanitario".into(),
                code: String::new(),
                from: w[0],
                to: w[1],
                path: vec![
                    crate::geom::P3::new(i as f64 * 50.0, 0.0, 100.0 - i as f64 * 0.3),
                    crate::geom::P3::new((i + 1) as f64 * 50.0, 0.0, 100.0 - (i + 1) as f64 * 0.3),
                ],
                d_mm: 200,
                attrs: BTreeMap::new(),
                datos: BTreeMap::new(),
                estado: EstadoObjeto::Pendiente,
                observaciones: Vec::new(),
                cota_solera_inicio: None,
                cota_solera_fin: None,
                deleted: false,
                sin_dato: Vec::new(),
                modelo: None,
            })
            .unwrap();
        }
        (p, ids)
    }

    #[test]
    fn grados_de_una_cadena() {
        let (p, ids) = p_with_chain(3);
        let t = Topology::build(&p);
        assert_eq!(t.degree_of(ids[0]), 1);
        assert_eq!(t.degree_of(ids[1]), 2);
        assert_eq!(t.degree_of(ids[2]), 1);
        assert_eq!(t.components.len(), 1);
        assert_eq!(t.components[0].len(), 3);
    }

    #[test]
    fn detecta_nodos_aislados() {
        let (mut p, ids) = p_with_chain(2);
        let extra = p
            .add_node(Node {
                id: ElementId::new(0, 0),
                family: "saneamiento.pozo".into(),
                ty: "inspeccion".into(),
                code: String::new(),
                pos: crate::geom::P2::new(500.0, 0.0),
                z_terrain: 100.0,
                z_rim: 100.0,
                z_invert: 98.0,
                attrs: BTreeMap::new(),
                datos: BTreeMap::new(),
                estado: EstadoObjeto::Pendiente,
                observaciones: Vec::new(),
                modelo: None,
                deleted: false,
                sin_dato: Vec::new(),
            })
            .unwrap();
        p.commit("test");
        let t = Topology::build(&p);
        let iso: Vec<_> = t.isolated(&p).collect();
        assert_eq!(iso, vec![extra]);
        assert_eq!(t.components.len(), 2);
        assert_eq!(t.degree_of(ids[0]), 1);
    }

    /// Una válvula de EPANET une dos nudos sin tramo propio: la red no queda cortada (F12-15).
    #[test]
    fn un_dispositivo_de_enlace_une_dos_redes_sin_tramo() {
        let (mut p, a) = p_with_chain(2);
        let mut otra = p.nodes[0].clone();
        otra.id = ElementId::new(0, 0);
        otra.code = String::new();
        let b = p.add_node(otra).unwrap();
        let mut valvula = p.nodes[0].clone();
        valvula.id = ElementId::new(0, 0);
        valvula.code = String::new();
        let v = p.add_node(valvula).unwrap();
        assert_eq!(
            Topology::build(&p).components.len(),
            3,
            "sin enlace: cortada"
        );
        p.enlaces.push(Enlace {
            dispositivo: v,
            desde: Some(a[1]),
            hasta: Some(b),
        });
        let t = Topology::build(&p);
        assert_eq!(t.components.len(), 1, "{:?}", t.components);
        assert_eq!(t.degree_of(v), 2);
        assert_eq!(t.degree_of(a[1]), 2);
        assert_eq!(t.isolated(&p).count(), 0);
        // Un extremo borrado no conecta.
        p.remove(b).unwrap();
        assert_eq!(Topology::build(&p).degree_of(v), 1);
    }

    #[test]
    fn sentido_de_flujo_por_cotas() {
        let (p, _) = p_with_chain(2);
        let t = Topology::build(&p);
        let lid = p.links[0].id;
        assert_eq!(t.flow_forward.get(&lid), Some(&true));
        assert!(t.contrapendiente.is_empty());
    }

    #[test]
    fn la_contrapendiente_se_avisa_y_no_invierte_el_sentido() {
        let (mut p, _) = p_with_chain(2);
        let lid = p.links[0].id;
        // La cota sube en el sentido declarado.
        p.links[0].path[0].z = 90.0;
        let t = Topology::build(&p);
        assert_eq!(t.flow_forward.get(&lid), Some(&true));
        assert_eq!(t.contrapendiente, vec![lid]);
        // A presión la regla de pendiente no aplica.
        p.links[0].ty = "impulsion".into();
        assert!(Topology::build(&p).contrapendiente.is_empty());
        p.links[0].family = "agua.tuberia".into();
        p.links[0].ty = "distribucion_primaria".into();
        assert!(Topology::build(&p).contrapendiente.is_empty());
    }

    #[test]
    fn la_red_con_descarga_no_queda_aislada() {
        let (mut p, ids) = p_with_chain(2);
        // Sin ninguna fuente ni sumidero no se marca nada.
        assert!(Topology::build(&p).stranded.is_empty());
        // Con un bombeo en otra red, la cadena sin sumidero queda aislada...
        let mut n = p.nodes[0].clone();
        n.id = ElementId::new(0, 0);
        n.code = String::new();
        n.family = "saneamiento.bombeo".into();
        n.ty = crate::families::family("saneamiento.bombeo").unwrap().types[0]
            .key
            .into();
        n.pos = crate::geom::P2::new(900.0, 0.0);
        p.add_node(n).unwrap();
        assert_eq!(Topology::build(&p).stranded.len(), 2);
        // ...y con una descarga en la cadena deja de estarlo.
        p.nodes[1].family = "saneamiento.descarga".into();
        let t = Topology::build(&p);
        assert!(!t.stranded.contains(&ids[0]), "{:?}", t.stranded);
    }
}
