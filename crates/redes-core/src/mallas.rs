//! Caché de mallas paramétricas por pieza (F14-06).
//!
//! La malla de un nodo depende solo de lo que la define: familia, subtipo, atributos (DN,
//! diámetros, ángulos…), altura tapa − solera, si le falta altura y el nivel de detalle. Dos nodos
//! con la misma [`ClavePieza`] comparten la malla, y reconstruir la escena sin cambios no vuelve a
//! generar ninguna: el contador [`CachePiezas::generadas`] lo comprueba en las pruebas.
//!
//! La posición del nodo no entra en la clave: la malla está en coordenadas locales de la pieza
//! (origen en el centro de la solera) y se coloca después.

use std::collections::HashMap;
use std::sync::Arc;

use crate::model::Node;
use crate::parts::Mesh;

/// Lo que define la malla paramétrica de un nodo.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ClavePieza {
    pub familia: String,
    pub tipo: String,
    /// Atributos serializados en orden (el mapa es ordenado): incluye DN y diámetros.
    pub atributos: String,
    /// Bits de `z_rim − z_invert` en f64: la altura exacta, sin redondear.
    pub altura: u64,
    pub sin_altura: bool,
    pub lod: u8,
}

impl ClavePieza {
    pub fn de(n: &Node, lod: u8) -> ClavePieza {
        ClavePieza {
            familia: n.family.clone(),
            tipo: n.ty.clone(),
            atributos: serde_json::to_string(&n.attrs).unwrap_or_default(),
            altura: (n.z_rim - n.z_invert).to_bits(),
            sin_altura: n.sin_altura(),
            lod,
        }
    }
}

struct Entrada {
    malla: Arc<Mesh>,
    /// Última pasada en que se usó: lo que no se usa en una pasada completa se poda.
    pasada: u64,
}

/// Caché de mallas de pieza con contador de generaciones.
#[derive(Default)]
pub struct CachePiezas {
    mapa: HashMap<ClavePieza, Entrada>,
    generadas: usize,
    pasada: u64,
}

impl CachePiezas {
    /// Malla de la pieza del nodo con ese nivel de detalle; se genera solo si no estaba.
    pub fn pieza(&mut self, n: &Node, lod: u8) -> Arc<Mesh> {
        let clave = ClavePieza::de(n, lod);
        let pasada = self.pasada;
        if let Some(e) = self.mapa.get_mut(&clave) {
            e.pasada = pasada;
            return Arc::clone(&e.malla);
        }
        let malla = Arc::new(crate::view::pieza_con_detalle(n, lod));
        self.generadas += 1;
        self.mapa.insert(
            clave,
            Entrada {
                malla: Arc::clone(&malla),
                pasada,
            },
        );
        malla
    }

    /// Cuántas mallas de pieza se han generado desde que existe la caché.
    pub fn generadas(&self) -> usize {
        self.generadas
    }

    /// Mallas distintas guardadas.
    pub fn len(&self) -> usize {
        self.mapa.len()
    }

    pub fn is_empty(&self) -> bool {
        self.mapa.is_empty()
    }

    /// Empieza una pasada completa por el proyecto (antes de reconstruir la escena).
    pub fn iniciar_pasada(&mut self) {
        self.pasada = self.pasada.wrapping_add(1);
    }

    /// Olvida las mallas que la última pasada no usó (piezas editadas o borradas), para que la
    /// caché no crezca con cada edición.
    pub fn podar(&mut self) {
        let pasada = self.pasada;
        self.mapa.retain(|_, e| e.pasada == pasada);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dos_nodos_iguales_comparten_la_malla_y_la_segunda_pasada_no_genera() {
        let p = crate::demo_city::ciudad();
        let mut cache = CachePiezas::default();
        cache.iniciar_pasada();
        for n in p.nodes.iter().filter(|n| !n.deleted) {
            cache.pieza(n, 2);
        }
        cache.podar();
        let primera = cache.generadas();
        assert!(primera > 0);
        assert!(
            primera <= p.nodes.len(),
            "nunca más mallas que nodos ({primera})"
        );
        cache.iniciar_pasada();
        for n in p.nodes.iter().filter(|n| !n.deleted) {
            cache.pieza(n, 2);
        }
        cache.podar();
        assert_eq!(cache.generadas(), primera, "la segunda pasada no genera");
    }

    #[test]
    fn la_clave_distingue_altura_atributos_y_detalle() {
        let p = crate::demo::project();
        let n = p
            .nodes
            .iter()
            .find(|n| n.family == "saneamiento.pozo")
            .expect("pozo")
            .clone();
        let mut otro = n.clone();
        otro.z_rim += 0.37;
        assert_ne!(ClavePieza::de(&n, 2), ClavePieza::de(&otro, 2));
        let mut otro = n.clone();
        otro.attrs
            .insert("diametro_interior".into(), crate::families::Value::Num(1.5));
        assert_ne!(ClavePieza::de(&n, 2), ClavePieza::de(&otro, 2));
        assert_ne!(ClavePieza::de(&n, 2), ClavePieza::de(&n, 1));
        // La posición no cuenta: la malla es local.
        let mut movido = n.clone();
        movido.pos.x += 100.0;
        movido.z_rim += 3.0;
        movido.z_invert += 3.0;
        assert_eq!(ClavePieza::de(&n, 2), ClavePieza::de(&movido, 2));
    }

    #[test]
    fn podar_olvida_lo_que_no_se_uso() {
        let p = crate::demo::project();
        let mut cache = CachePiezas::default();
        cache.iniciar_pasada();
        for n in &p.nodes {
            cache.pieza(n, 2);
        }
        cache.podar();
        assert!(!cache.is_empty());
        cache.iniciar_pasada();
        cache.podar();
        assert!(cache.is_empty(), "una pasada vacía deja la caché vacía");
    }
}
