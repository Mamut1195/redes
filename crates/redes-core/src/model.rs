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

use crate::families::{Value, family};
use crate::geom::{Bbox, P2, P3};
use crate::revision::Observacion;
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
        Some(ElementId::new(
            u32::from_str_radix(a, 16).ok()?,
            u32::from_str_radix(b, 16).ok()?,
        ))
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
    /// Pieza de la biblioteca que da la apariencia de este objeto (`biblioteca.json`).
    ///
    /// Si está vacío se dibuja la pieza paramétrica; si apunta a un `.glb` de la biblioteca, el
    /// visor usa ese modelo y sus medidas alimentan los datos del objeto.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modelo: Option<String>,
    #[serde(default = "estado_pendiente")]
    pub estado: EstadoObjeto,
    /// Observaciones de la revisión, con historial (F13-03). Sustituyen a la antigua `nota`
    /// única (sin conversión desde `redes.project/2`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub observaciones: Vec<Observacion>,
    #[serde(default)]
    pub deleted: bool,
    /// Datos que el origen **no trajo** (F12-03): claves [`COTA_TERRENO`], [`COTA_TAPA`],
    /// [`COTA_SOLERA`]. El campo numérico correspondiente guarda solo un valor de **dibujo** (la
    /// otra cota conocida, de modo que la pieza sale plana) y nunca se muestra como dato: se lee
    /// con [`Node::cota_terreno`], [`Node::cota_tapa`] y [`Node::cota_solera`], que dan `None`.
    /// Vacío en los `.redes` anteriores: entonces todo lo guardado es dato.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sin_dato: Vec<String>,
}

/// Clave de [`Node::sin_dato`]: falta la cota de terreno.
pub const COTA_TERRENO: &str = "cota_terreno";
/// Clave de [`Node::sin_dato`]: falta la cota de tapa.
pub const COTA_TAPA: &str = "cota_tapa";
/// Clave de [`Node::sin_dato`] y [`Link::sin_dato`]: falta la cota de solera (en un tramo, la de
/// su trazado).
pub const COTA_SOLERA: &str = "cota_solera";
/// Clave de [`Link::sin_dato`]: los vértices intermedios no traían cota y su Z se interpoló por
/// longitud entre las soleras declaradas del tramo (F12-15). Es geometría de dibujo, no dato; las
/// soleras de los extremos sí lo son.
pub const COTA_VERTICES: &str = "cota_vertices";
/// Clave de [`Link::sin_dato`]: falta el diámetro nominal.
pub const DIAMETRO_NOMINAL: &str = "diametro_nominal";

/// Texto con que se muestra un dato ausente.
pub const SIN_DATO: &str = "—";

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
    /// ¿Falta este dato en el origen? (`clave` de [`Node::sin_dato`]).
    pub fn falta(&self, clave: &str) -> bool {
        self.sin_dato.iter().any(|k| k == clave)
    }
    /// Cota de terreno, si el origen la trajo.
    pub fn cota_terreno(&self) -> Option<f64> {
        (!self.falta(COTA_TERRENO)).then_some(self.z_terrain)
    }
    /// Cota de tapa, si el origen la trajo.
    pub fn cota_tapa(&self) -> Option<f64> {
        (!self.falta(COTA_TAPA)).then_some(self.z_rim)
    }
    /// Cota de solera, si el origen la trajo.
    pub fn cota_solera(&self) -> Option<f64> {
        (!self.falta(COTA_SOLERA)).then_some(self.z_invert)
    }
    /// Sin altura dibujable: falta la solera, o faltan a la vez tapa y terreno. Se dibuja un
    /// símbolo plano «sin dato», no una pieza con una altura inventada.
    pub fn sin_altura(&self) -> bool {
        self.falta(COTA_SOLERA) || (self.falta(COTA_TAPA) && self.falta(COTA_TERRENO))
    }
    /// Altura con datos: (tapa o, si falta, terreno) − solera. `None` si falta alguna.
    pub fn altura_dato(&self) -> Option<f64> {
        let arriba = self.cota_tapa().or(self.cota_terreno())?;
        Some((arriba - self.cota_solera()?).max(0.0))
    }
    /// Diferencia geométrica terreno − solera (para dibujar la altura del pozo).
    pub fn altura(&self) -> f64 {
        (self.z_terrain - self.z_invert).max(0.0)
    }
    pub fn diametro(&self) -> f64 {
        self.num("diametro_interior").unwrap_or(1.20)
    }
    /// Texto de la última observación abierta (lo que antes mostraba la `nota`).
    pub fn nota_vigente(&self) -> Option<&str> {
        crate::revision::nota_vigente(&self.observaciones)
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
    /// Trazado 3D: planta y cota de **solera** (fondo interior del tubo) en cada vértice.
    ///
    /// El eje del tubo queda medio diámetro interior más arriba ([`Link::eje`]) y la clave, un
    /// diámetro interior más arriba (F12-02).
    pub path: Vec<P3>,
    /// Diámetro nominal (mm). Es una designación comercial: no es el interior ni el exterior.
    pub d_mm: u32,
    pub attrs: BTreeMap<String, Value>,
    #[serde(default)]
    pub datos: BTreeMap<String, Dato>,
    #[serde(default = "estado_pendiente")]
    pub estado: EstadoObjeto,
    /// Observaciones de la revisión, con historial (F13-03). Sustituyen a la antigua `nota`
    /// única (sin conversión desde `redes.project/2`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub observaciones: Vec<Observacion>,
    #[serde(default)]
    pub deleted: bool,
    /// Cota de solera declarada por el origen al inicio del tramo (m). `None` si no la trae: no
    /// se toma la del pozo, que puede estar más baja (caída) o en otra entrada.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cota_solera_inicio: Option<f64>,
    /// Cota de solera declarada por el origen al final del tramo (m).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cota_solera_fin: Option<f64>,
    /// Datos que el origen no trajo (F12-03): [`DIAMETRO_NOMINAL`] (entonces `d_mm` = 0) y
    /// [`COTA_SOLERA`] (las cotas del trazado son solo de dibujo). Ver [`Node::sin_dato`].
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sin_dato: Vec<String>,
}

impl Link {
    /// ¿Falta este dato en el origen? (`clave` de [`Link::sin_dato`]).
    pub fn falta(&self, clave: &str) -> bool {
        self.sin_dato.iter().any(|k| k == clave)
    }
    /// Diámetro nominal (mm), si el origen lo trajo.
    pub fn dn(&self) -> Option<u32> {
        (!self.falta(DIAMETRO_NOMINAL) && self.d_mm > 0).then_some(self.d_mm)
    }
    /// Cotas del trazado al inicio y al fin, si son dato (no de dibujo).
    pub fn cotas_trazado(&self) -> Option<(f64, f64)> {
        if self.falta(COTA_SOLERA) {
            return None;
        }
        Some((self.path.first()?.z, self.path.last()?.z))
    }
    /// Se dibuja como «sin dato» (línea gris): le falta el diámetro o la cota.
    pub fn dibujo_sin_dato(&self) -> bool {
        self.falta(COTA_SOLERA)
            || (self.dn().is_none() && self.num("diametro_interior").is_none_or(|d| d <= 0.0))
    }
    pub fn d_m(&self) -> f64 {
        self.d_mm as f64 / 1000.0
    }
    /// Diámetro interior para dibujar (m): el declarado (`attrs.diametro_interior`, mm) o, si
    /// falta, el nominal. El DN es la mejor aproximación geométrica que hay, no un dato nuevo.
    pub fn diametro_interior_m(&self) -> f64 {
        self.num("diametro_interior")
            .filter(|d| *d > 0.0)
            .map(|d| d / 1000.0)
            .unwrap_or_else(|| self.d_m())
    }
    /// Espesor de pared para dibujar (m): el declarado (`attrs.espesor_pared`) o uno típico.
    pub fn espesor_pared_m(&self) -> f64 {
        self.num("espesor_pared")
            .filter(|e| *e > 0.0)
            .unwrap_or(0.008)
    }
    /// Diámetro exterior para dibujar (m): DI + 2 · espesor. El DN no es el diámetro exterior.
    pub fn diametro_exterior_m(&self) -> f64 {
        self.diametro_interior_m() + 2.0 * self.espesor_pared_m()
    }
    /// Eje del tubo: el trazado (solera) subido medio diámetro interior.
    pub fn eje(&self) -> Vec<P3> {
        let r = self.diametro_interior_m() * 0.5;
        self.path
            .iter()
            .map(|q| P3::new(q.x, q.y, q.z + r))
            .collect()
    }
    pub fn length_2d(&self) -> f64 {
        self.path
            .windows(2)
            .map(|w| w[0].p2().dist(w[1].p2()))
            .sum()
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
    /// Texto de la última observación abierta (lo que antes mostraba la `nota`).
    pub fn nota_vigente(&self) -> Option<&str> {
        crate::revision::nota_vigente(&self.observaciones)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JournalEntry {
    pub revision: u64,
    pub command: String,
    /// Usuario del sistema que hizo el cambio, si se pudo leer.
    ///
    /// No es una firma criptográfica: identifica la sesión, no prueba quién estuvo delante de la
    /// pantalla. Sirve para saber a quién preguntar y para que una aprobación no quede anónima.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usuario: Option<String>,
    /// Fecha del cambio, en UTC (`YYYY-MM-DDTHH:MM:SSZ`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cuando: Option<String>,
}

/// Un dispositivo puntual que une nodos sin tramo (F12-15): una bomba o una válvula de EPANET, un
/// vertedero de SWMM. `desde`/`hasta` son los nodos del origen; uno puede faltar cuando el propio
/// dispositivo es un extremo (la cámara húmeda que representa a la bomba).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Enlace {
    pub dispositivo: ElementId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub desde: Option<ElementId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hasta: Option<ElementId>,
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
    /// Ruta del manifiesto de la biblioteca de modelos (`biblioteca.json`), relativa al proyecto.
    ///
    /// Se guarda la ruta y no el manifiesto: el archivo de piezas puede crecer y no tiene por qué
    /// duplicarse dentro de cada `.redes`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub biblioteca: Option<String>,
    pub nodes: Vec<Node>,
    pub links: Vec<Link>,
    /// Dispositivos de enlace (bomba, válvula, vertedero) que en el origen unían nodos sin un tramo
    /// propio (F12-15). En Redes son nodos; esto le dice a la topología a qué nodos están unidos,
    /// sin inventar tramos. Vacío en los `.redes` anteriores.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub enlaces: Vec<Enlace>,
    /// Umbrales propios de `redes validar` (F13-01). `None`: los de referencia
    /// ([`crate::validar::ReglasValidacion::default`]). Ausente en los `.redes` anteriores.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reglas_validacion: Option<crate::validar::ReglasValidacion>,
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
            schema: format!("redes.project/{}", crate::project_io::SCHEMA_VERSION),
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
            enlaces: Vec::new(),
            reglas_validacion: None,
            next_slot: 1,
            journal: Vec::new(),
            biblioteca: None,
            extras: Default::default(),
        }
    }

    /// Corrige al abrir los `.redes` anteriores a F12-01: los nodos guardaban su **solera** en el
    /// atributo `cota_clave`. Solo se renombra cuando el valor coincide con `z_invert` (es la
    /// solera mal rotulada); una clave de verdad, distinta de la solera, se deja como está.
    pub fn migrar_cotas(&mut self) {
        for n in &mut self.nodes {
            let es_solera = n
                .num("cota_clave")
                .is_some_and(|v| (v - n.z_invert).abs() < 1e-3);
            if es_solera
                && !n.attrs.contains_key("cota_solera")
                && let Some(v) = n.attrs.remove("cota_clave")
            {
                n.attrs.insert("cota_solera".to_string(), v);
            }
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
        self.nodes
            .iter()
            .find(|n| !n.deleted && n.code.eq_ignore_ascii_case(code))
    }
    pub fn link_by_code(&self, code: &str) -> Option<&Link> {
        self.links
            .iter()
            .find(|l| !l.deleted && l.code.eq_ignore_ascii_case(code))
    }

    /// Siguiente código libre de una familia (`PZ-001`, `PZ-002`…).
    pub fn next_code(&self, family_key: &str) -> String {
        let prefix = family(family_key).map(|f| f.code_prefix).unwrap_or("EL");
        let mut n = self.nodes.iter().filter(|x| x.family == family_key).count()
            + self.links.iter().filter(|x| x.family == family_key).count()
            + 1;
        loop {
            let code = format!("{prefix}-{n:03}");
            let taken = self.nodes.iter().any(|x| x.code == code)
                || self.links.iter().any(|x| x.code == code);
            if !taken {
                return code;
            }
            n += 1;
        }
    }

    /// Añade un nodo. Solo valida estructura: familia, subtipo y código único.
    pub fn add_node(&mut self, mut node: Node) -> Result<ElementId, String> {
        let def =
            family(&node.family).ok_or_else(|| format!("familia desconocida: {}", node.family))?;
        if def.ty(&node.ty).is_none() {
            return Err(format!(
                "subtipo '{}' no existe en {}",
                node.ty, node.family
            ));
        }
        if node.code.is_empty() {
            node.code = self.next_code(&node.family);
        }
        if self
            .nodes
            .iter()
            .any(|n| !n.deleted && n.code.eq_ignore_ascii_case(&node.code))
        {
            return Err(format!(
                "E_STATE_DUPLICATE_NAME: código duplicado {}",
                node.code
            ));
        }
        node.id = self.alloc();
        let id = node.id;
        self.nodes.push(node);
        Ok(id)
    }

    /// Añade un arco entre dos nodos existentes.
    pub fn add_link(&mut self, mut link: Link) -> Result<ElementId, String> {
        let def =
            family(&link.family).ok_or_else(|| format!("familia desconocida: {}", link.family))?;
        if def.ty(&link.ty).is_none() {
            return Err(format!(
                "subtipo '{}' no existe en {}",
                link.ty, link.family
            ));
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
        if self
            .links
            .iter()
            .any(|l| !l.deleted && l.code.eq_ignore_ascii_case(&link.code))
        {
            return Err(format!(
                "E_STATE_DUPLICATE_NAME: código duplicado {}",
                link.code
            ));
        }
        link.id = self.alloc();
        let id = link.id;
        self.links.push(link);
        Ok(id)
    }

    pub fn commit(&mut self, command: &str) {
        self.revision += 1;
        self.journal.push(JournalEntry {
            revision: self.revision,
            command: command.to_string(),
            usuario: usuario_del_sistema(),
            cuando: ahora_utc(),
        });
    }

    pub fn remove(&mut self, id: ElementId) -> Result<(), String> {
        if let Some(n) = self.nodes.iter_mut().find(|n| n.id == id) {
            if self
                .links
                .iter()
                .any(|l| !l.deleted && (l.from == id || l.to == id))
            {
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
        self.nodes.iter().filter(|n| !n.deleted).count()
            + self.links.iter().filter(|l| !l.deleted).count()
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

/// Usuario del sistema, **solo si se pide** con `REDES_JOURNAL_USUARIO=1`.
///
/// Decisión del 2026-11-21: la revisión no identifica a nadie por defecto. El journal guarda la
/// revisión, el comando y la fecha; el usuario es opcional y explícito.
pub(crate) fn usuario_del_sistema() -> Option<String> {
    let activo = std::env::var("REDES_JOURNAL_USUARIO")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    if !activo {
        return None;
    }
    std::env::var("USERNAME")
        .or_else(|_| std::env::var("USER"))
        .ok()
        .filter(|s| !s.trim().is_empty())
}

/// Fecha y hora en UTC, sin dependencias: `YYYY-MM-DDTHH:MM:SSZ`.
pub(crate) fn ahora_utc() -> Option<String> {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs();
    let (dias, resto) = (secs / 86_400, secs % 86_400);
    let (h, m, s) = (resto / 3600, (resto % 3600) / 60, resto % 60);
    let mut year = 1970i64;
    let mut dias = dias as i64;
    loop {
        let bisiesto = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
        let largo = if bisiesto { 366 } else { 365 };
        if dias < largo {
            break;
        }
        dias -= largo;
        year += 1;
    }
    let bisiesto = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    let meses = [
        31,
        if bisiesto { 29 } else { 28 },
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
    let mut mes = 0usize;
    while mes < 12 && dias >= meses[mes] {
        dias -= meses[mes];
        mes += 1;
    }
    Some(format!(
        "{year:04}-{:02}-{:02}T{h:02}:{m:02}:{s:02}Z",
        mes + 1,
        dias + 1
    ))
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
            observaciones: Vec::new(),
            modelo: None,
            deleted: false,
            sin_dato: Vec::new(),
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

    /// Las aprobaciones no quedan anónimas: el journal anota usuario y fecha (F11-02 parcial).
    #[test]
    fn el_journal_registra_usuario_y_fecha() {
        let mut p = Project::new("journal", "EPSG:32717");
        p.commit("objeto.aprobar");
        let e = p.journal.last().expect("entrada");
        assert_eq!(e.command, "objeto.aprobar");
        // Sin `REDES_JOURNAL_USUARIO=1` no se identifica a nadie (decisión del producto).
        assert!(e.usuario.is_none(), "no debe identificar por defecto");
        let cuando = e.cuando.clone().expect("fecha");
        assert_eq!(cuando.len(), 20, "formato UTC: {cuando}");
        assert!(cuando.ends_with('Z') && cuando.contains('T'), "{cuando}");
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
            observaciones: Vec::new(),
            cota_solera_inicio: None,
            cota_solera_fin: None,
            deleted: false,
            sin_dato: Vec::new(),
        });
        assert!(r.is_err());
    }

    /// Un `.redes` viejo guardaba la solera del pozo como `cota_clave`: al abrirlo pasa a
    /// `cota_solera`; una clave que no es la solera no se toca (F12-01).
    #[test]
    fn la_solera_rotulada_como_clave_se_migra() {
        let mut p = Project::new("viejo", "EPSG:32717");
        let a = pozo(&mut p, 0.0, 0.0, 100.0);
        let b = pozo(&mut p, 10.0, 0.0, 99.0);
        let n = p.nodes.iter_mut().find(|n| n.id == a).unwrap();
        n.attrs.insert("cota_clave".into(), Value::Num(100.0));
        let n = p.nodes.iter_mut().find(|n| n.id == b).unwrap();
        n.attrs.insert("cota_clave".into(), Value::Num(99.4));
        p.migrar_cotas();
        let a = p.node(a).unwrap();
        assert_eq!(a.num("cota_solera"), Some(100.0));
        assert!(a.get("cota_clave").is_none());
        let b = p.node(b).unwrap();
        assert_eq!(b.num("cota_clave"), Some(99.4), "una clave real no se toca");
        assert!(b.get("cota_solera").is_none());
    }

    /// Un tramo guardado antes de F12-02 (sin cotas de solera declaradas) sigue abriendo.
    #[test]
    fn un_tramo_viejo_sin_cotas_declaradas_abre() {
        let viejo = r#"{"id":{"slot":3,"generation":1},"family":"saneamiento.tuberia",
            "ty":"colector_sanitario","code":"CL-001","from":{"slot":1,"generation":1},
            "to":{"slot":2,"generation":1},"path":[{"x":0,"y":0,"z":100},{"x":10,"y":0,"z":99.9}],"d_mm":300,
            "attrs":{}}"#;
        let l: Link = serde_json::from_str(viejo).expect("abre");
        assert_eq!(l.cota_solera_inicio, None);
        assert_eq!(l.cota_solera_fin, None);
        // Y al guardar no aparecen campos vacíos.
        let texto = serde_json::to_string(&l).unwrap();
        assert!(!texto.contains("cota_solera"), "{texto}");
        // Sin `sin_dato` todo lo guardado es dato (F12-03).
        assert_eq!(l.dn(), Some(300));
        assert_eq!(l.cotas_trazado(), Some((100.0, 99.9)));
        assert!(!texto.contains("sin_dato"));
    }

    /// Un nodo de un `.redes` anterior a F12-03 abre con todas sus cotas como dato.
    #[test]
    fn un_nodo_viejo_conserva_sus_cotas_como_dato() {
        let viejo = r#"{"id":{"slot":1,"generation":1},"family":"saneamiento.pozo",
            "ty":"inspeccion","code":"PZ-001","pos":{"x":0,"y":0},
            "z_terrain":100.0,"z_rim":100.1,"z_invert":98.0,"attrs":{}}"#;
        let n: Node = serde_json::from_str(viejo).expect("abre");
        assert_eq!(n.cota_terreno(), Some(100.0));
        assert_eq!(n.cota_tapa(), Some(100.1));
        assert_eq!(n.cota_solera(), Some(98.0));
        assert!(!n.sin_altura());
        assert!(!serde_json::to_string(&n).unwrap().contains("sin_dato"));
    }

    /// El trazado es la solera: el eje queda DI/2 más arriba y el exterior suma la pared (F12-02).
    #[test]
    fn el_eje_del_tubo_queda_medio_diametro_sobre_la_solera() {
        let l: Link = serde_json::from_str(
            r#"{"id":{"slot":3,"generation":1},"family":"saneamiento.tuberia",
            "ty":"colector_sanitario","code":"CL-001","from":{"slot":1,"generation":1},
            "to":{"slot":2,"generation":1},"path":[{"x":0,"y":0,"z":100},{"x":10,"y":0,"z":100}],"d_mm":315,
            "attrs":{"diametro_interior":300.0,"espesor_pared":0.0077}}"#,
        )
        .unwrap();
        assert!(
            (l.diametro_interior_m() - 0.30).abs() < 1e-9,
            "DI declarado, no DN"
        );
        assert!((l.eje()[0].z - 100.15).abs() < 1e-9);
        assert!((l.diametro_exterior_m() - (0.30 + 2.0 * 0.0077)).abs() < 1e-9);
    }

    #[test]
    fn cuenta_estados_de_revision() {
        let mut p = Project::new("t", "EPSG:32717");
        pozo(&mut p, 0.0, 0.0, 100.0);
        pozo(&mut p, 10.0, 0.0, 99.0);
        assert_eq!(p.revision_counts(), (2, 0, 0));
    }
}
