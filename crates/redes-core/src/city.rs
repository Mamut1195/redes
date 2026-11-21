//! Entorno urbano para la vista 3D: calles y manzanas construidas.
//!
//! No es una familia de la red: es **contexto** para leer el modelo (una tapa de pozo solo tiene
//! sentido si se ve la calle bajo la que está). Se guarda en el proyecto para que la vista sea
//! reproducible sin depender de un SIG externo.

use serde::{Deserialize, Serialize};

use crate::geom::{P2, P3};

/// Una calle: eje en planta, cota del terreno en cada vértice y ancho de calzada.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Calle {
    pub nombre: String,
    pub eje: Vec<P3>,
    /// Ancho total de la calzada (m).
    pub ancho: f64,
}

impl Calle {
    /// Longitud en planta (m).
    pub fn longitud(&self) -> f64 {
        self.eje.windows(2).map(|w| w[0].p2().dist(w[1].p2())).sum()
    }
}

/// Una manzana construida: polígono en planta y altura de edificación.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Manzana {
    pub poligono: Vec<P2>,
    /// Altura sobre el terreno (m).
    pub altura: f64,
    /// `vivienda`, `comercio`, `equipamiento`.
    pub uso: String,
    /// Variación de color 0..1 para que no parezca una maqueta de plástico.
    pub tono: f32,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Entorno {
    pub calles: Vec<Calle>,
    pub manzanas: Vec<Manzana>,
}

impl Entorno {
    pub fn is_empty(&self) -> bool {
        self.calles.is_empty() && self.manzanas.is_empty()
    }

    /// Caja en planta de todo el entorno.
    pub fn bbox(&self) -> crate::geom::Bbox {
        let mut b = crate::geom::Bbox::default();
        for c in &self.calles {
            for p in &c.eje {
                b.add(p.p2());
            }
        }
        for m in &self.manzanas {
            for p in &m.poligono {
                b.add(*p);
            }
        }
        b
    }
}
