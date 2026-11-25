//! Índices de búsqueda de [`Project`] (F14-01): por id, por código y nodo → tramos.
//!
//! Antes, `node()`, `node_by_code()`, `add_node()`… recorrían todo el proyecto; dibujar o importar
//! una red de 50 000 tramos era cuadrático. El índice guarda **posiciones** en `nodes`/`links`:
//!
//! - **No se serializa**: se reconstruye al abrir ([`Project::reindexar`] en `project_io::load`),
//!   como el índice del TIN. Clonar el proyecto (deshacer) lo clona válido: las posiciones no
//!   cambian con el clon.
//! - **Incluye los objetos borrados**: una baja (`deleted = true`) o su vuelta atrás no lo tocan;
//!   cada búsqueda filtra `deleted` al leer. Por eso borrar no exige mantenerlo.
//! - **Se mantiene en las altas** ([`Project::add_node`], [`Project::add_link`]).
//! - **Se sabe desfasado**: guarda cuántos nodos y tramos había y el id del último. Si alguien
//!   cambia `nodes`/`links` a mano (los campos aún son públicos, F15-02), las búsquedas vuelven al
//!   recorrido lineal —lentas pero correctas— hasta el siguiente [`Project::reindexar`] o alta.
//!   Lo único que no detecta es reescribir un objeto en su sitio (cambiarle id, código o
//!   extremos): quien lo haga llama a `reindexar()`.

use std::collections::HashMap;

use crate::model::{ElementId, Link, Node, Project};

/// Clave de código: sin distinguir mayúsculas ASCII, como `eq_ignore_ascii_case`.
fn clave(codigo: &str) -> String {
    codigo.to_ascii_lowercase()
}

#[derive(Clone, Default)]
pub(crate) struct Indice {
    nodos: usize,
    tramos: usize,
    ultimo_nodo: Option<ElementId>,
    ultimo_tramo: Option<ElementId>,
    nodo_por_id: HashMap<ElementId, usize>,
    tramo_por_id: HashMap<ElementId, usize>,
    /// Posiciones en orden del vector (el mismo orden que el recorrido lineal).
    nodos_por_codigo: HashMap<String, Vec<usize>>,
    tramos_por_codigo: HashMap<String, Vec<usize>>,
    /// Tramos que tocan cada nodo (por `from` o por `to`), en orden del vector.
    tramos_de_nodo: HashMap<ElementId, Vec<usize>>,
    /// Objetos (nodos y tramos, borrados incluidos) por familia: base de `next_code`.
    por_familia: HashMap<String, usize>,
}

impl std::fmt::Debug for Indice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Indice {{ nodos: {}, tramos: {} }}",
            self.nodos, self.tramos
        )
    }
}

impl Indice {
    fn de(nodes: &[Node], links: &[Link]) -> Indice {
        let mut ix = Indice::default();
        for (i, n) in nodes.iter().enumerate() {
            ix.alta_nodo(i, n);
        }
        for (i, l) in links.iter().enumerate() {
            ix.alta_tramo(i, l);
        }
        ix
    }

    pub(crate) fn alta_nodo(&mut self, i: usize, n: &Node) {
        self.nodo_por_id.entry(n.id).or_insert(i);
        self.nodos_por_codigo
            .entry(clave(&n.code))
            .or_default()
            .push(i);
        *self.por_familia.entry(n.family.clone()).or_default() += 1;
        self.nodos = i + 1;
        self.ultimo_nodo = Some(n.id);
    }

    pub(crate) fn alta_tramo(&mut self, i: usize, l: &Link) {
        self.tramo_por_id.entry(l.id).or_insert(i);
        self.tramos_por_codigo
            .entry(clave(&l.code))
            .or_default()
            .push(i);
        self.tramos_de_nodo.entry(l.from).or_default().push(i);
        if l.to != l.from {
            self.tramos_de_nodo.entry(l.to).or_default().push(i);
        }
        *self.por_familia.entry(l.family.clone()).or_default() += 1;
        self.tramos = i + 1;
        self.ultimo_tramo = Some(l.id);
    }
}

/// Lo que el índice necesita leer de un nodo o un tramo.
trait Elemento {
    fn ident(&self) -> ElementId;
    fn codigo(&self) -> &str;
    fn borrado(&self) -> bool;
}

impl Elemento for Node {
    fn ident(&self) -> ElementId {
        self.id
    }
    fn codigo(&self) -> &str {
        &self.code
    }
    fn borrado(&self) -> bool {
        self.deleted
    }
}

impl Elemento for Link {
    fn ident(&self) -> ElementId {
        self.id
    }
    fn codigo(&self) -> &str {
        &self.code
    }
    fn borrado(&self) -> bool {
        self.deleted
    }
}

/// Posición del objeto con ese id (vivo, o también borrado si `vivos` es falso).
fn por_id<T: Elemento>(
    v: &[T],
    mapa: Option<&HashMap<ElementId, usize>>,
    id: ElementId,
    vivos: bool,
) -> Option<usize> {
    let vale = |e: &T| e.ident() == id && !(vivos && e.borrado());
    if let Some(mapa) = mapa {
        match mapa.get(&id) {
            None => return None,
            Some(&i) if v.get(i).is_some_and(|e| e.ident() == id) => {
                return vale(&v[i]).then_some(i);
            }
            // El objeto de esa posición ya no es el indexado: se busca recorriendo.
            Some(_) => {}
        }
    }
    v.iter().position(vale)
}

/// Primera posición (en orden del vector) cuyo código cumple `igual` y que pasa `filtro`.
fn por_codigo<T: Elemento>(
    v: &[T],
    mapa: Option<&HashMap<String, Vec<usize>>>,
    codigo: &str,
    igual: impl Fn(&str) -> bool,
    filtro: impl Fn(&T) -> bool,
) -> Option<usize> {
    let vale = |e: &T| igual(e.codigo()) && filtro(e);
    match mapa {
        Some(mapa) => mapa
            .get(&clave(codigo))?
            .iter()
            .copied()
            .find(|&i| v.get(i).is_some_and(vale)),
        None => v.iter().position(vale),
    }
}

impl Project {
    /// El índice, si corresponde a los vectores actuales (ver el módulo [`crate::indice`]).
    fn indice(&self) -> Option<&Indice> {
        let ix = &self.indice;
        (ix.nodos == self.nodes.len()
            && ix.tramos == self.links.len()
            && ix.ultimo_nodo == self.nodes.last().map(|n| n.id)
            && ix.ultimo_tramo == self.links.last().map(|l| l.id))
        .then_some(ix)
    }

    /// Reconstruye los índices. Lo hace la carga; quien cambie `nodes`/`links` a mano, también.
    pub fn reindexar(&mut self) {
        self.indice = Indice::de(&self.nodes, &self.links);
    }

    /// ¿Están los índices al día? Si no, las búsquedas recorren todo (correctas, pero lentas).
    #[doc(hidden)]
    pub fn indice_al_dia(&self) -> bool {
        self.indice().is_some()
    }

    /// Reindexa solo si hace falta (antes de un alta, que mantiene el índice incremental).
    pub(crate) fn asegurar_indice(&mut self) {
        if self.indice().is_none() {
            self.reindexar();
        }
    }

    pub(crate) fn pos_nodo(&self, id: ElementId, vivos: bool) -> Option<usize> {
        por_id(
            &self.nodes,
            self.indice().map(|ix| &ix.nodo_por_id),
            id,
            vivos,
        )
    }

    pub(crate) fn pos_tramo(&self, id: ElementId, vivos: bool) -> Option<usize> {
        por_id(
            &self.links,
            self.indice().map(|ix| &ix.tramo_por_id),
            id,
            vivos,
        )
    }

    /// Nodo vivo con ese código, sin distinguir mayúsculas.
    pub(crate) fn pos_nodo_codigo(&self, codigo: &str) -> Option<usize> {
        por_codigo(
            &self.nodes,
            self.indice().map(|ix| &ix.nodos_por_codigo),
            codigo,
            |c| c.eq_ignore_ascii_case(codigo),
            |n| !n.deleted,
        )
    }

    /// Tramo vivo con ese código, sin distinguir mayúsculas.
    pub(crate) fn pos_tramo_codigo(&self, codigo: &str) -> Option<usize> {
        por_codigo(
            &self.links,
            self.indice().map(|ix| &ix.tramos_por_codigo),
            codigo,
            |c| c.eq_ignore_ascii_case(codigo),
            |l| !l.deleted,
        )
    }

    /// Nodo vivo con ese código **exacto** (la reimportación empareja así).
    pub(crate) fn nodo_codigo_exacto(&self, codigo: &str) -> Option<&Node> {
        por_codigo(
            &self.nodes,
            self.indice().map(|ix| &ix.nodos_por_codigo),
            codigo,
            |c| c == codigo,
            |n| !n.deleted,
        )
        .map(|i| &self.nodes[i])
    }

    /// Tramo vivo con ese código **exacto**.
    pub(crate) fn tramo_codigo_exacto(&self, codigo: &str) -> Option<&Link> {
        por_codigo(
            &self.links,
            self.indice().map(|ix| &ix.tramos_por_codigo),
            codigo,
            |c| c == codigo,
            |l| !l.deleted,
        )
        .map(|i| &self.links[i])
    }

    /// ¿Lo usa ya algún nodo o tramo, vivo o borrado, con ese código exacto? (`next_code`).
    pub(crate) fn codigo_usado(&self, codigo: &str) -> bool {
        let ix = self.indice();
        por_codigo(
            &self.nodes,
            ix.map(|ix| &ix.nodos_por_codigo),
            codigo,
            |c| c == codigo,
            |_| true,
        )
        .is_some()
            || por_codigo(
                &self.links,
                ix.map(|ix| &ix.tramos_por_codigo),
                codigo,
                |c| c == codigo,
                |_| true,
            )
            .is_some()
    }

    /// Nodos y tramos de una familia, borrados incluidos.
    pub(crate) fn cuantos_de_familia(&self, familia: &str) -> usize {
        match self.indice() {
            Some(ix) => ix.por_familia.get(familia).copied().unwrap_or(0),
            None => {
                self.nodes.iter().filter(|x| x.family == familia).count()
                    + self.links.iter().filter(|x| x.family == familia).count()
            }
        }
    }

    /// Tramos vivos que empiezan o terminan en `nodo`, en el orden del proyecto. O(grado).
    pub fn tramos_de(&self, nodo: ElementId) -> impl Iterator<Item = &Link> + '_ {
        let candidatos: Box<dyn Iterator<Item = &Link> + '_> = match self.indice() {
            Some(ix) => Box::new(
                ix.tramos_de_nodo
                    .get(&nodo)
                    .into_iter()
                    .flatten()
                    .filter_map(|&i| self.links.get(i)),
            ),
            None => Box::new(self.links.iter()),
        };
        candidatos.filter(move |l| !l.deleted && (l.from == nodo || l.to == nodo))
    }
}
