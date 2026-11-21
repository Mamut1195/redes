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
    /// Sentido de flujo por arco: `true` si va de `from` a `to`.
    pub flow_forward: BTreeMap<ElementId, bool>,
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
        for l in p.links.iter().filter(|l| !l.deleted) {
            // El sentido en gravedad lo imponen las cotas: aguas abajo = clave menor.
            let (z0, z1) = (l.path.first().map(|q| q.z).unwrap_or(0.0), l.path.last().map(|q| q.z).unwrap_or(0.0));
            flow_forward.insert(l.id, z0 >= z1);
            incident.entry(l.from).or_default().push(l.id);
            incident.entry(l.to).or_default().push(l.id);
        }
        let degree = incident.iter().map(|(k, v)| (*k, v.len())).collect();

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
            }
            comp.sort();
            components.push(comp);
        }
        components.sort();

        // Nodos que no alcanzan una fuente (pozo de bombeo, tanque, captación, planta).
        let sources: BTreeSet<ElementId> = p
            .nodes
            .iter()
            .filter(|n| !n.deleted && is_source(&n.family))
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

        Topology { incident, degree, components, flow_forward, stranded }
    }

    pub fn links_at(&self, node: ElementId) -> &[ElementId] {
        self.incident.get(&node).map(|v| v.as_slice()).unwrap_or(&[])
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

fn is_source(family: &str) -> bool {
    matches!(family, "agua.tanque" | "agua.captacion" | "agua.planta" | "saneamiento.bombeo")
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
                    nota: None,
                    deleted: false,
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
                nota: None,
                deleted: false,
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
                nota: None,
                deleted: false,
            })
            .unwrap();
        p.commit("test");
        let t = Topology::build(&p);
        let iso: Vec<_> = t.isolated(&p).collect();
        assert_eq!(iso, vec![extra]);
        assert_eq!(t.components.len(), 2);
        assert_eq!(t.degree_of(ids[0]), 1);
    }

    #[test]
    fn sentido_de_flujo_por_cotas() {
        let (p, _) = p_with_chain(2);
        let t = Topology::build(&p);
        let lid = p.links[0].id;
        assert_eq!(t.flow_forward.get(&lid), Some(&true));
    }
}
