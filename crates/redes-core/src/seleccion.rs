//! Selección por clic con rejilla en pantalla (F14-07).
//!
//! Antes la app recorría todos los nodos y tramos en cada clic. La rejilla proyecta la red una
//! vez por cámara y reparte cada objeto en las celdas de la pantalla que quedan a su alcance
//! (18 pt de un nodo, 14 pt de un tramo); un clic mira solo los objetos de su celda. Mientras la
//! cámara no cambie, elegir cuesta lo que haya en una celda: O(1) amortizado.
//!
//! Da exactamente lo mismo que el recorrido lineal (prueba
//! `la_rejilla_elige_lo_mismo_que_el_recorrido_lineal`): mismos candidatos, mismas distancias y,
//! a igual distancia, el mismo desempate (nodos antes que tramos, en orden del proyecto).

use crate::model::Project;
use crate::view::Camera;

/// Alcance de un nodo en puntos: más lejos del clic no se elige.
pub const ALCANCE_NODO: f32 = 18.0;
/// Alcance de un tramo en puntos (distancia al segmento entre sus extremos proyectados).
pub const ALCANCE_TRAMO: f32 = 14.0;
/// Lado de la celda en puntos.
const CELDA: f32 = 32.0;

#[derive(Clone, Debug)]
enum Forma {
    Punto([f32; 2]),
    Segmento([f32; 2], [f32; 2]),
}

#[derive(Clone, Debug)]
struct Candidato {
    codigo: String,
    forma: Forma,
}

impl Candidato {
    /// Distancia al clic si está a su alcance.
    fn distancia(&self, at: [f32; 2]) -> Option<f32> {
        match self.forma {
            Forma::Punto(s) => {
                let d = ((s[0] - at[0]).powi(2) + (s[1] - at[1]).powi(2)).sqrt();
                (d < ALCANCE_NODO).then_some(d)
            }
            Forma::Segmento(a, b) => {
                let d = distancia_a_segmento(at, a, b);
                (d < ALCANCE_TRAMO).then_some(d)
            }
        }
    }
}

/// Distancia de `p` al segmento `a`–`b`, en pantalla.
pub fn distancia_a_segmento(p: [f32; 2], a: [f32; 2], b: [f32; 2]) -> f32 {
    let ab = [b[0] - a[0], b[1] - a[1]];
    let len2 = ab[0] * ab[0] + ab[1] * ab[1];
    if len2 <= 1e-6 {
        return ((p[0] - a[0]).powi(2) + (p[1] - a[1]).powi(2)).sqrt();
    }
    let t = (((p[0] - a[0]) * ab[0] + (p[1] - a[1]) * ab[1]) / len2).clamp(0.0, 1.0);
    let q = [a[0] + ab[0] * t, a[1] + ab[1] * t];
    ((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2)).sqrt()
}

/// Candidatos en el orden del recorrido lineal: nodos vivos y después tramos vivos con al menos
/// dos vértices, los que se proyectan.
fn candidatos(p: &Project, cam: &Camera) -> Vec<Candidato> {
    let o = p.origin;
    let pr = cam.proyector();
    let mut out = Vec::new();
    for n in p.nodes.iter().filter(|n| !n.deleted) {
        if let Some(s) = pr.project([n.pos.x - o.x, n.pos.y - o.y, n.z_rim]) {
            out.push(Candidato {
                codigo: n.code.clone(),
                forma: Forma::Punto(s),
            });
        }
    }
    for l in p.links.iter().filter(|l| !l.deleted) {
        if l.path.len() < 2 {
            continue;
        }
        let a = l.path[0];
        let b = l.path[l.path.len() - 1];
        if let (Some(sa), Some(sb)) = (
            pr.project([a.x - o.x, a.y - o.y, a.z]),
            pr.project([b.x - o.x, b.y - o.y, b.z]),
        ) {
            out.push(Candidato {
                codigo: l.code.clone(),
                forma: Forma::Segmento(sa, sb),
            });
        }
    }
    out
}

/// El más cercano de `lista` (estrictamente: a igual distancia gana el primero).
fn mas_cercano<'a>(
    lista: impl Iterator<Item = &'a Candidato>,
    at: [f32; 2],
) -> Option<&'a Candidato> {
    let mut mejor: Option<(f32, &Candidato)> = None;
    for c in lista {
        if let Some(d) = c.distancia(at)
            && mejor.is_none_or(|(md, _)| d < md)
        {
            mejor = Some((d, c));
        }
    }
    mejor.map(|(_, c)| c)
}

/// Rejilla de selección de una cámara. Se construye una vez por cámara (y por proyecto) y se
/// consulta en cada clic.
#[derive(Clone, Debug)]
pub struct RejillaSeleccion {
    candidatos: Vec<Candidato>,
    /// Esquina superior izquierda de la rejilla en puntos (el viewport menos el alcance).
    x0: f32,
    y0: f32,
    cols: usize,
    filas: usize,
    /// Índices de candidato por celda, en orden creciente (el del recorrido lineal).
    celdas: Vec<Vec<u32>>,
}

impl RejillaSeleccion {
    /// Proyecta la red con `cam` y reparte los objetos en celdas. Lo que no alcanza el viewport
    /// no entra: un clic dentro del lienzo nunca lo elegiría.
    pub fn nueva(p: &Project, cam: &Camera) -> RejillaSeleccion {
        let candidatos = candidatos(p, cam);
        let m = ALCANCE_NODO.max(ALCANCE_TRAMO);
        let (x0, y0) = (-m, -m);
        let cols = (((cam.viewport[0] + 2.0 * m) / CELDA).ceil() as usize).max(1);
        let filas = (((cam.viewport[1] + 2.0 * m) / CELDA).ceil() as usize).max(1);
        let mut celdas = vec![Vec::new(); cols * filas];
        // Rango de celdas que toca un intervalo, recortado a la rejilla.
        let rango = |lo: f32, hi: f32, o: f32, n: usize| -> Option<(usize, usize)> {
            if !(lo.is_finite() && hi.is_finite()) {
                return None;
            }
            let a = ((lo - o) / CELDA).floor().max(0.0);
            let b = ((hi - o) / CELDA).floor().min(n as f32 - 1.0);
            (a <= b).then_some((a as usize, b as usize))
        };
        for (i, c) in candidatos.iter().enumerate() {
            let (min, max, r) = match c.forma {
                Forma::Punto(s) => (s, s, ALCANCE_NODO),
                Forma::Segmento(a, b) => (
                    [a[0].min(b[0]), a[1].min(b[1])],
                    [a[0].max(b[0]), a[1].max(b[1])],
                    ALCANCE_TRAMO,
                ),
            };
            let (Some((c0, c1)), Some((f0, f1))) = (
                rango(min[0] - r, max[0] + r, x0, cols),
                rango(min[1] - r, max[1] + r, y0, filas),
            ) else {
                continue;
            };
            // Media diagonal de la celda: un punto de la celda está a lo sumo a eso del centro.
            let media = CELDA * std::f32::consts::FRAC_1_SQRT_2;
            for f in f0..=f1 {
                for col in c0..=c1 {
                    // Un tramo largo en diagonal no llena su caja: solo las celdas que roza.
                    if let Forma::Segmento(a, b) = c.forma {
                        let centro = [
                            x0 + (col as f32 + 0.5) * CELDA,
                            y0 + (f as f32 + 0.5) * CELDA,
                        ];
                        if distancia_a_segmento(centro, a, b) > r + media {
                            continue;
                        }
                    }
                    celdas[f * cols + col].push(i as u32);
                }
            }
        }
        RejillaSeleccion {
            candidatos,
            x0,
            y0,
            cols,
            filas,
            celdas,
        }
    }

    /// Índices de los candidatos de la celda de `at` (vacío fuera de la rejilla).
    fn celda(&self, at: [f32; 2]) -> &[u32] {
        let i = ((at[0] - self.x0) / CELDA).floor();
        let j = ((at[1] - self.y0) / CELDA).floor();
        if !(i.is_finite() && j.is_finite()) || i < 0.0 || j < 0.0 {
            return &[];
        }
        let (i, j) = (i as usize, j as usize);
        if i >= self.cols || j >= self.filas {
            return &[];
        }
        &self.celdas[j * self.cols + i]
    }

    /// Cuántos objetos mira un clic en `at` (para las pruebas y el banco).
    pub fn examinados(&self, at: [f32; 2]) -> usize {
        self.celda(at).len()
    }

    /// Objetos de la rejilla (los que se proyectan).
    pub fn len(&self) -> usize {
        self.candidatos.len()
    }

    pub fn is_empty(&self) -> bool {
        self.candidatos.is_empty()
    }

    /// Código del objeto más cercano al clic, si alguno está a su alcance.
    pub fn elegir(&self, at: [f32; 2]) -> Option<&str> {
        mas_cercano(
            self.celda(at).iter().map(|&i| &self.candidatos[i as usize]),
            at,
        )
        .map(|c| c.codigo.as_str())
    }
}

/// Selección recorriendo toda la red (la de antes de F14-07). Referencia de las pruebas.
pub fn elegir_lineal(p: &Project, cam: &Camera, at: [f32; 2]) -> Option<String> {
    let lista = candidatos(p, cam);
    mas_cercano(lista.iter(), at).map(|c| c.codigo.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn camara(p: &Project, distancia: Option<f64>) -> Camera {
        let mut c = Camera {
            viewport: [900.0, 600.0],
            ..Camera::default()
        };
        c.fit(p);
        if let Some(d) = distancia {
            c.distance = d;
        }
        c
    }

    /// Mismo resultado que el recorrido lineal en una malla de clics que cubre el lienzo, con la
    /// red encuadrada y de cerca.
    #[test]
    fn la_rejilla_elige_lo_mismo_que_el_recorrido_lineal() {
        for p in [crate::demo::project(), crate::demo_city::ciudad()] {
            for distancia in [None, Some(80.0)] {
                let cam = camara(&p, distancia);
                let rejilla = RejillaSeleccion::nueva(&p, &cam);
                assert!(!rejilla.is_empty());
                let mut aciertos = 0;
                let mut y = -10.0;
                while y < 610.0 {
                    let mut x = -10.0;
                    while x < 910.0 {
                        let lineal = elegir_lineal(&p, &cam, [x, y]);
                        assert_eq!(
                            rejilla.elegir([x, y]).map(str::to_string),
                            lineal,
                            "clic en ({x}, {y})"
                        );
                        aciertos += lineal.is_some() as usize;
                        x += 7.0;
                    }
                    y += 7.0;
                }
                assert!(aciertos > 50, "pocos clics sobre objetos: {aciertos}");
            }
        }
    }

    #[test]
    fn fuera_del_lienzo_no_elige_nada() {
        let p = crate::demo::project();
        let cam = camara(&p, None);
        let rejilla = RejillaSeleccion::nueva(&p, &cam);
        assert_eq!(rejilla.elegir([-500.0, 300.0]), None);
        assert_eq!(rejilla.elegir([f32::NAN, 0.0]), None);
    }
}
