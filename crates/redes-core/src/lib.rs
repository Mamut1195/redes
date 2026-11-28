//! # redes-core
//!
//! Motor de datos de **Redes**: un visor 3D de redes de agua potable y alcantarillado.
//!
//! Esta capa **no calcula ingeniería**. Lee lo que producen otras aplicaciones (CAD, SWMM,
//! EPANET, Excel) con [`ingest`], lo guarda como documento con procedencia y estado de revisión
//! ([`model`]), lo dibuja ([`view`]) y lo devuelve al CAD ([`export`]).
//!
//! ```
//! use redes_core::{demo, export::revision_summary};
//!
//! let proyecto = demo::project();
//! let (pendientes, aprobados, observados) = revision_summary(&proyecto);
//! assert!(pendientes + aprobados + observados == proyecto.count());
//! ```

pub mod acta;
pub mod biblioteca;
pub mod cantidades;
pub mod city;
pub mod demo;
pub mod demo_cadena;
pub mod demo_catalog;
pub mod demo_city;
pub mod diff;
pub mod envelope;
pub mod export;
pub mod families;
pub mod families_extra;
pub mod geom;
pub mod gltf;
pub mod indice;
pub mod ingest;
pub mod mallas;
pub mod model;
pub mod parts;
pub mod parts_cadena;
pub mod perfil;
pub mod project_io;
pub mod reimportar;
pub mod revision;
pub mod scene3d;
pub mod seleccion;
pub mod terrain;
pub mod topology;
pub mod undo;
pub mod validar;
pub mod view;

/// Versión del motor (la misma del crate).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub mod prelude {
    pub use crate::envelope::Envelope;
    pub use crate::families::{FAMILIES, FamilyDef, Value, family};
    pub use crate::geom::{Bbox, P2, P3};
    pub use crate::model::{Dato, ElementId, EstadoObjeto, EstadoProyecto, Link, Node, Project};
    pub use crate::view::{Camera, Scene};
}
