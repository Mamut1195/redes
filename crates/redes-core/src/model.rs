//! Modelo de datos de **Redes**: un documento para *ver* y *aprobar* redes ya diseñadas en
//! otras herramientas. Aquí no se calcula nada: los números que se muestran vienen del archivo
//! de origen y se guardan como datos con su procedencia.
//!
//! Invariantes:
//! - La identidad es [`ElementId`] (slot + generación); nunca un puntero ni un orden implícito.
//! - Los objetos **no se referencian entre sí** salvo por `ElementId`.
//! - La geometría es 3D: planta (E, N) en el CRS del proyecto y cotas en metros.
//! - Todo valor numérico de diseño se guarda en `resultados` con la app de la que vino; el
//!   visor lo muestra y **no lo recalcula**.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::families::{family, Value};
use crate::geom::{Bbox, P2, P3};
use crate::terrain::Terrain;

/// Identidad de un elemento. 8 bytes; `slot` nunca se reutiliza con la misma `generation`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ElementId {
    pub slot: u32,
    pub generation: u32,
}

impl ElementId {
    pub const fn new(slot: u32, generation: u32) -> Self {
        ElementId { slot, generation }
    }
    pub fn short(self) -> String {
        format!("{:04x}.{:04x}", self.slot, self.generation)
    }
    pub fn parse(s: &str) -> Option<ElementId> {
        let (a, b) = s.split_once('.')?;
        Some(ElementId::new(u32::from_str_radix(a, 16).ok()?, u32::from_str_radix(b, 16).ok()?))
    }
}

impl std::fmt::Display for ElementId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.short())
    }
}

/// Estado de revisión de un objeto (el flujo de aprobación es la razón de ser del visor).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EstadoObjeto {
    Pendiente,
    Aprobado,
    Observado,
}

impl EstadoObjeto {
    pub fn key(self) -> &'static str {
        match self {
            EstadoObjeto::Pendiente => "pendiente",
            EstadoObjeto::Aprobado => "aprobado",
            EstadoObjeto::Observado => "observado",
        }
    }
}

/// Estado global del proyecto.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EstadoProyecto {
    Importado,
    EnRevision,
    Aprobado,
}

impl EstadoProyecto {
    pub fn key(self) -> &'static str {
        match self {
            EstadoProyecto::Importado => "importado",
            EstadoProyecto::EnRevision => "en_revision",
            EstadoProyecto::Aprobado => "aprobado",
        }
    }
}

/// Un número que vino de otra aplicación. Se muestra tal cual, con su unidad y su fuente.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Dato {
    pub valor: f64,
    pub unidad: String,
    /// Aplicación o archivo del que procede (`SWMM`, `Civil 3D`, `EPANET`…).
    pub fuente: String,
}

/// De dónde salió el proyecto.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Fuente {
    pub app: String,
    pub archivo: String,
    pub formato: String,
    pub importado_en: String,
}

/// Un nodo del grafo (pozo, cámara, tanque, descarga, captación, válvula, hidrante).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Node {
    pub id: ElementId,
    /// Clave de familia, `dominio.familia`.
    pub family: String,
    /// Subtipo de catálogo.
    pub ty: String,
    /// Código legible único en el proyecto (`PZ-001`).
    pub code: String,
    /// Planta en coordenadas del proyecto (m).
    pub pos: P2,
    /// Cota de terreno natural (m).
    pub z_terrain: f64,
    /// Cota de tapa / rasante (m).
    pub z_rim: f64,
    /// Cota de solera (fondo interior) (m).
    pub z_invert: f64,
    /// Atributos de la familia (geometría y descripción, no resultados).
    pub attrs: BTreeMap<String, Value>,
    /// Números calculados por la app de origen (caudal, volumen, presión…).
    #[serde(default)]
    pub datos: BTreeMap<String, Dato>,
    #[serde(default = "estado_pendiente")]
    pub estado: EstadoObjeto,
    /// Observacion de la revision (que hay que corregir en la app de origen).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nota: Option<String>,
    #[serde(default)]
    pub deleted: bool,
}

fn estado_pendiente() -> EstadoObjeto {
    EstadoObjeto::Pendiente
}

impl Node {
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.attrs.get(key)
    }
    pub fn num(&self, key: &str) -> Option<f64> {
        self.attrs.get(key).and_then(|v| v.as_f64())
    }
    /// Diferencia geométrica terreno − solera (para dibujar la altura del pozo).
    pub fn altura(&self) -> f64 {
        (self.z_terrain - self.z_invert).max(0.0)
    }
    pub fn diametro(&self) -> f64 {
        self.num("diametro_interior").unwrap_or(1.20)
    }
}

/// Un arco del grafo (tubería, canal, acometida, sifón).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Link {
    pub id: ElementId,
    pub family: String,
    pub ty: String,
    pub code: String,
    pub from: ElementId,
    pub to: ElementId,
    /// Trazado 3D (planta + cota de eje o clave en cada vértice).
    pub path: Vec<P3>,
    /// Diámetro nominal (mm).
    pub d_mm: u32,
    pub attrs: BTreeMap<String, Value>,
    #[serde(default)]
    pub datos: BTreeMap<String, Dato>,
    #[serde(default = "estado_pendiente")]
    pub estado: EstadoObjeto,
    /// Observacion de la revision (que hay que corregir en la app de origen).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nota: Option<String>,
    #[serde(default)]
    pub deleted: bool,
}

impl Link {
    pub fn d_m(&self) -> f64 {
        self.d_mm as f64 / 1000.0
    }
    pub fn length_2d(&self) -> f64 {
        self.path.windows(2).map(|w| w[0].p2().dist(w[1].p2())).sum()
    }
    pub fn length_3d(&self) -> f64 {
        crate::geom::polyline_len_3d(&self.path)
    }
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.attrs.get(key)
    }
    pub fn num(&self, key: &str) -> Option<f64> {
        self.attrs.get(key).and_then(|v| v.as_f64())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JournalEntry {
    pub revision: u64,
    pub command: String,
}

/// El documento: la única fuente de verdad de lo que se ve.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Project {
    pub schema: String,
    pub id: String,
    pub name: String,
    /// CRS del proyecto, p. ej. `EPSG:32717`.
    pub crs: String,
    /// Origen local para el render (resta en f64 antes de pasar a f32).
    pub origin: P2,
    pub revision: u64,
    pub estado: EstadoProyecto,
    /// Aplicaciones y archivos de los que se importó.
    #[serde(default)]
    pub fuentes: Vec<Fuente>,
    pub terrain: Option<Terrain>,
    /// Contexto urbano (calles y manzanas) para la vista 3D.
    #[serde(default)]
    pub entorno: Option<crate::city::Entorno>,
    pub nodes: Vec<Node>,
    pub links: Vec<Link>,
    pub(crate) next_slot: u32,
    #[serde(default)]
    pub journal: Vec<JournalEntry>,
    /// Campos que esta versión no conoce: se conservan al guardar en vez de borrarlos.
    ///
    /// Sin esto, abrir un archivo de una versión futura y guardarlo destruye en silencio los
    /// campos nuevos (revisión adversarial, D-04).
    #[serde(flatten, default)]
    pub extras: std::collections::BTreeMap<String, serde_json::Value>,
}

impl Default for Project {
    fn default() -> Self {
        Project::new("Proyecto sin título", "EPSG:32717")
    }
}

impl Project {
    pub fn new(name: &str, crs: &str) -> Project {
        Project {
            schema: "redes.project/2".into(),
            id: format!("prj_{:08x}", fnv1a(name.as_bytes())),
            name: name.into(),
            crs: crs.into(),
            origin: P2::new(0.0, 0.0),
            revision: 0,
            estado: EstadoProyecto::Importado,
            fuentes: Vec::new(),
            terrain: None,
            entorno: None,
            nodes: Vec::new(),
            links: Vec::new(),
            next_slot: 1,
            journal: Vec::new(),
            extras: Default::default(),
        }
    }

    fn alloc(&mut self) -> ElementId {
        let slot = self.next_slot;
        self.next_slot += 1;
        ElementId::new(slot, 1)
    }

    pub fn node(&self, id: ElementId) -> Option<&Node> {
        self.nodes.iter().find(|n| n.id == id && !n.deleted)
    }
    pub fn link(&self, id: ElementId) -> Option<&Link> {
        self.links.iter().find(|l| l.id == id && !l.deleted)
    }
    pub fn node_by_code(&self, code: &str) -> Option<&Node> {
        self.nodes.iter().find(|n| !n.deleted && n.code.eq_ignore_ascii_case(code))
    }
    pub fn link_by_code(&self, code: &str) -> Option<&Link> {
        self.links.iter().find(|l| !l.deleted && l.code.eq_ignore_ascii_case(code))
    }

    /// Siguiente código libre de una familia (`PZ-001`, `PZ-002`…).
    pub fn next_code(&self, family_key: &str) -> String {
        let prefix = family(family_key).map(|f| f.code_prefix).unwrap_or("EL");
        let mut n = self
            .nodes
            .iter()
            .filter(|x| x.family == family_key)
            .count()
            + self.links.iter().filter(|x| x.family == family_key).count()
            + 1;
        loop {
            let code = format!("{prefix}-{n:03}");
            let taken = self.nodes.iter().any(|x| x.code == code) || self.links.iter().any(|x| x.code == code);
            if !taken {
                return code;
            }
            n += 1;
        }
    }

    /// Añade un nodo. Solo valida estructura: familia, subtipo y código único.
    pub fn add_node(&mut self, mut node: Node) -> Result<ElementId, String> {
        let def = family(&node.family).ok_or_else(|| format!("familia desconocida: {}", node.family))?;
        if def.ty(&node.ty).is_none() {
            return Err(format!("subtipo '{}' no existe en {}", node.ty, node.family));
        }
        if node.code.is_empty() {
            node.code = self.next_code(&node.family);
        }
        if self.nodes.iter().any(|n| !n.deleted && n.code.eq_ignore_ascii_case(&node.code)) {
            return Err(format!("E_STATE_DUPLICATE_NAME: código duplicado {}", node.code));
        }
        node.id = self.alloc();
        let id = node.id;
        self.nodes.push(node);
        Ok(id)
    }

    /// Añade un arco entre dos nodos existentes.
    pub fn add_link(&mut self, mut link: Link) -> Result<ElementId, String> {
        let def = family(&link.family).ok_or_else(|| format!("familia desconocida: {}", link.family))?;
        if def.ty(&link.ty).is_none() {
            return Err(format!("subtipo '{}' no existe en {}", link.ty, link.family));
        }
        if link.from == link.to {
            return Err("E_TOPO_SELF_LOOP: el arco empieza y termina en el mismo nodo".into());
        }
        if self.node(link.from).is_none() || self.node(link.to).is_none() {
            return Err("E_TOPO_ORPHAN_LINK: el arco apunta a un nodo inexistente".into());
        }
        if link.code.is_empty() {
            link.code = self.next_code(&link.family);
        }
        if self.links.iter().any(|l| !l.deleted && l.code.eq_ignore_ascii_case(&link.code)) {
            return Err(format!("E_STATE_DUPLICATE_NAME: código duplicado {}", link.code));
        }
        link.id = self.alloc();
        let id = link.id;
        self.links.push(link);
        Ok(id)
    }

    pub fn commit(&mut self, command: &str) {
        self.revision += 1;
        self.journal.push(JournalEntry { revision: self.revision, command: command.to_string() });
    }

    pub fn remove(&mut self, id: ElementId) -> Result<(), String> {
        if let Some(n) = self.nodes.iter_mut().find(|n| n.id == id) {
            if self.links.iter().any(|l| !l.deleted && (l.from == id || l.to == id)) {
                return Err("E_STATE_REFERENCED: el nodo tiene arcos conectados".into());
            }
            n.deleted = true;
            return Ok(());
        }
        if let Some(l) = self.links.iter_mut().find(|l| l.id == id) {
            l.deleted = true;
            return Ok(());
        }
        Err("E_STATE_NOT_FOUND".into())
    }

    pub fn bbox(&self) -> Bbox {
        let mut b = Bbox::default();
        for n in self.nodes.iter().filter(|n| !n.deleted) {
            b.add(n.pos);
        }
        for l in self.links.iter().filter(|l| !l.deleted) {
            for p in &l.path {
                b.add(p.p2());
            }
        }
        if let Some(t) = &self.terrain {
            b.add(P2::new(t.bbox.xmin, t.bbox.ymin));
            b.add(P2::new(t.bbox.xmax, t.bbox.ymax));
        }
        if let Some(e) = &self.entorno {
            let eb = e.bbox();
            if !eb.is_empty() {
                b.add(P2::new(eb.xmin, eb.ymin));
                b.add(P2::new(eb.xmax, eb.ymax));
            }
        }
        b
    }

    /// Rango de cotas del modelo (para encuadrar la cámara).
    pub fn z_range(&self) -> (f64, f64) {
        let mut lo = f64::INFINITY;
        let mut hi = f64::NEG_INFINITY;
        for n in self.nodes.iter().filter(|n| !n.deleted) {
            lo = lo.min(n.z_invert);
            hi = hi.max(n.z_rim.max(n.z_terrain));
        }
        for l in self.links.iter().filter(|l| !l.deleted) {
            for p in &l.path {
                lo = lo.min(p.z);
                hi = hi.max(p.z);
            }
        }
        if let Some(t) = &self.terrain {
            let (a, b) = t.z_range();
            lo = lo.min(a);
            hi = hi.max(b);
        }
        if lo > hi { (0.0, 1.0) } else { (lo, hi) }
    }

    pub fn count(&self) -> usize {
        self.nodes.iter().filter(|n| !n.deleted).count() + self.links.iter().filter(|l| !l.deleted).count()
    }

    /// Cuántos objetos hay en cada estado de revisión.
    pub fn revision_counts(&self) -> (usize, usize, usize) {
        let mut p = 0;
        let mut a = 0;
        let mut o = 0;
        for e in self.nodes.iter().filter(|n| !n.deleted).map(|n| n.estado) {
            match e {
                EstadoObjeto::Pendiente => p += 1,
                EstadoObjeto::Aprobado => a += 1,
                EstadoObjeto::Observado => o += 1,
            }
        }
        for e in self.links.iter().filter(|l| !l.deleted).map(|l| l.estado) {
            match e {
                EstadoObjeto::Pendiente => p += 1,
                EstadoObjeto::Aprobado => a += 1,
                EstadoObjeto::Observado => o += 1,
            }
        }
        (p, a, o)
    }
}

fn fnv1a(bytes: &[u8]) -> u32 {
    let mut h: u32 = 0x811c_9dc5;
    for b in bytes {
        h ^= *b as u32;
        h = h.wrapping_mul(0x0100_0193);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pozo(p: &mut Project, x: f64, y: f64, z: f64) -> ElementId {
        p.add_node(Node {
            id: ElementId::new(0, 0),
            family: "saneamiento.pozo".into(),
            ty: "inspeccion".into(),
            code: String::new(),
            pos: P2::new(x, y),
            z_terrain: z + 2.0,
            z_rim: z + 2.0,
            z_invert: z,
            attrs: BTreeMap::from([("diametro_interior".to_string(), Value::Num(1.2))]),
            datos: BTreeMap::new(),
            estado: EstadoObjeto::Pendiente,
            nota: None,
            deleted: false,
        })
        .unwrap()
    }

    #[test]
    fn codigos_correlativos_y_unicos() {
        let mut p = Project::new("t", "EPSG:32717");
        let a = pozo(&mut p, 0.0, 0.0, 100.0);
        let b = pozo(&mut p, 10.0, 0.0, 99.0);
        assert_eq!(p.node(a).unwrap().code, "PZ-001");
        assert_eq!(p.node(b).unwrap().code, "PZ-002");
    }

    #[test]
    fn no_se_crea_un_arco_colgante() {
        let mut p = Project::new("t", "EPSG:32717");
        let a = pozo(&mut p, 0.0, 0.0, 100.0);
        let r = p.add_link(Link {
            id: ElementId::new(0, 0),
            family: "saneamiento.tuberia".into(),
            ty: "colector_sanitario".into(),
            code: String::new(),
            from: a,
            to: ElementId::new(999, 1),
            path: vec![],
            d_mm: 200,
            attrs: BTreeMap::new(),
            datos: BTreeMap::new(),
            estado: EstadoObjeto::Pendiente,
            nota: None,
            deleted: false,
        });
        assert!(r.is_err());
    }

    #[test]
    fn cuenta_estados_de_revision() {
        let mut p = Project::new("t", "EPSG:32717");
        pozo(&mut p, 0.0, 0.0, 100.0);
        pozo(&mut p, 10.0, 0.0, 99.0);
        assert_eq!(p.revision_counts(), (2, 0, 0));
    }
}
