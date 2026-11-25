//! Geometría paramétrica de las piezas de la red: pozos, tuberías, válvulas, hidrantes, tanques,
//! descargas, cámaras y sumideros.
//!
//! Todo se genera como **triángulos con su normal y su color** en coordenadas locales; la vista
//! los coloca, los ordena por profundidad y los dibuja. No hay mallas externas ni texturas: la
//! forma sale de los parámetros (diámetro, profundidad, tipo), que son los mismos que el
//! inspector muestra.
//!
//! Reglas:
//! - El origen local de cada pieza es el **centro de su base** (z = 0), salvo los tubos, que se
//!   generan directamente en coordenadas de mundo.
//! - El color se calcula aquí: normal · luz + ambiente, para que una tapa metálica y un muro de
//!   hormigón se distingan sin texturas.

use serde::{Deserialize, Serialize};

use crate::geom::P3;

/// Luz direccional normalizada (desde el noroeste, 45°).
const LUZ: [f32; 3] = [0.45, 0.52, 0.72];

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct Material {
    pub color: [f32; 3],
    /// Metal: refleja más y tiene brillo especular aproximado.
    pub metal: bool,
}

impl Material {
    pub const fn new(r: f32, g: f32, b: f32) -> Material {
        Material {
            color: [r, g, b],
            metal: false,
        }
    }
    pub const fn metal(r: f32, g: f32, b: f32) -> Material {
        Material {
            color: [r, g, b],
            metal: true,
        }
    }
}

pub const HORMIGON: Material = Material::new(0.66, 0.63, 0.57);
pub const HORMIGON_OSCURO: Material = Material::new(0.42, 0.41, 0.39);
pub const HIERRO: Material = Material::metal(0.42, 0.45, 0.52);
pub const PVC: Material = Material::new(0.80, 0.77, 0.64);
pub const AGUA: Material = Material::new(0.25, 0.48, 0.78);
pub const PLASTICO: Material = Material::new(0.30, 0.36, 0.42);
pub const LATON: Material = Material::metal(0.68, 0.58, 0.34);

/// Un triángulo listo para dibujar: tres vértices y su color ya sombreado.
#[derive(Clone, Copy, Debug)]
pub struct Tri {
    pub p: [[f32; 3]; 3],
    pub color: [f32; 4],
}

/// Conjunto de triángulos de una pieza, en coordenadas locales.
#[derive(Clone, Debug, Default)]
pub struct Mesh {
    pub tris: Vec<Tri>,
}

impl Mesh {
    pub fn is_empty(&self) -> bool {
        self.tris.is_empty()
    }

    pub fn len(&self) -> usize {
        self.tris.len()
    }

    fn tri(&mut self, a: [f32; 3], b: [f32; 3], c: [f32; 3], m: Material) {
        let n = normal(a, b, c);
        let lambert = (n[0] * LUZ[0] + n[1] * LUZ[1] + n[2] * LUZ[2]).max(0.0);
        let ambiente = 0.50;
        let mut k = ambiente + 0.62 * lambert;
        if m.metal {
            // Brillo especular barato: la cara que mira a la luz se aclara más.
            k += 0.22 * lambert.powi(8);
        }
        let k = k.min(1.35);
        self.tris.push(Tri {
            p: [a, b, c],
            color: [m.color[0] * k, m.color[1] * k, m.color[2] * k, 1.0],
        });
    }

    pub fn extend(&mut self, other: Mesh) {
        self.tris.extend(other.tris);
    }

    /// Traslada la pieza.
    pub fn mover(mut self, d: [f32; 3]) -> Mesh {
        for t in &mut self.tris {
            for p in &mut t.p {
                p[0] += d[0];
                p[1] += d[1];
                p[2] += d[2];
            }
        }
        self
    }

    /// Gira la pieza alrededor de Z (planta), en radianes.
    pub fn girar(mut self, ang: f32) -> Mesh {
        let (s, c) = ang.sin_cos();
        for t in &mut self.tris {
            for p in &mut t.p {
                let (x, y) = (p[0], p[1]);
                p[0] = x * c - y * s;
                p[1] = x * s + y * c;
            }
        }
        self
    }

    /// Caja envolvente local `(min, max)`.
    pub fn bbox(&self) -> ([f32; 3], [f32; 3]) {
        let mut lo = [f32::INFINITY; 3];
        let mut hi = [f32::NEG_INFINITY; 3];
        for t in &self.tris {
            for p in &t.p {
                for i in 0..3 {
                    lo[i] = lo[i].min(p[i]);
                    hi[i] = hi[i].max(p[i]);
                }
            }
        }
        (lo, hi)
    }
}

fn normal(a: [f32; 3], b: [f32; 3], c: [f32; 3]) -> [f32; 3] {
    let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
    let n = [
        u[1] * v[2] - u[2] * v[1],
        u[2] * v[0] - u[0] * v[2],
        u[0] * v[1] - u[1] * v[0],
    ];
    let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-6);
    [n[0] / l, n[1] / l, n[2] / l]
}

// ---------------------------------------------------------------------------------------------
// Primitivas

/// Caja apoyada en z = 0, centrada en planta.
pub fn caja(ancho: f32, fondo: f32, alto: f32, m: Material) -> Mesh {
    let (x, y, z) = (ancho * 0.5, fondo * 0.5, alto);
    let v = |sx: f32, sy: f32, sz: f32| [sx * x, sy * y, sz * z];
    let mut mesh = Mesh::default();
    // Caras laterales (dos triángulos cada una).
    let caras = [
        (
            v(-1.0, -1.0, 0.0),
            v(1.0, -1.0, 0.0),
            v(1.0, -1.0, 1.0),
            v(-1.0, -1.0, 1.0),
        ),
        (
            v(1.0, -1.0, 0.0),
            v(1.0, 1.0, 0.0),
            v(1.0, 1.0, 1.0),
            v(1.0, -1.0, 1.0),
        ),
        (
            v(1.0, 1.0, 0.0),
            v(-1.0, 1.0, 0.0),
            v(-1.0, 1.0, 1.0),
            v(1.0, 1.0, 1.0),
        ),
        (
            v(-1.0, 1.0, 0.0),
            v(-1.0, -1.0, 0.0),
            v(-1.0, -1.0, 1.0),
            v(-1.0, 1.0, 1.0),
        ),
    ];
    for (a, b, c, d) in caras {
        mesh.tri(a, b, c, m);
        mesh.tri(a, c, d, m);
    }
    // Tapa.
    let (a, b, c, d) = (
        v(-1.0, -1.0, 1.0),
        v(1.0, -1.0, 1.0),
        v(1.0, 1.0, 1.0),
        v(-1.0, 1.0, 1.0),
    );
    mesh.tri(a, b, c, m);
    mesh.tri(a, c, d, m);
    mesh
}

/// Cilindro apoyado en z = 0.
pub fn cilindro(radio: f32, alto: f32, caras: usize, m: Material) -> Mesh {
    tronco(radio, radio, alto, caras, m)
}

/// Tronco de cono apoyado en z = 0.
pub fn tronco(r1: f32, r2: f32, alto: f32, caras: usize, m: Material) -> Mesh {
    let n = caras.max(6);
    let mut mesh = Mesh::default();
    for k in 0..n {
        let a0 = k as f32 / n as f32 * std::f32::consts::TAU;
        let a1 = (k + 1) as f32 / n as f32 * std::f32::consts::TAU;
        let (c0, s0) = a0.sin_cos();
        let (c1, s1) = a1.sin_cos();
        let b0 = [r1 * c0, r1 * s0, 0.0];
        let b1 = [r1 * c1, r1 * s1, 0.0];
        let t0 = [r2 * c0, r2 * s0, alto];
        let t1 = [r2 * c1, r2 * s1, alto];
        mesh.tri(b0, b1, t1, m);
        mesh.tri(b0, t1, t0, m);
        // Tapa superior (si no es un cono cerrado en punta).
        if r2 > 1e-4 {
            mesh.tri([0.0, 0.0, alto], t0, t1, m);
        }
    }
    mesh
}

/// Disco horizontal en z = 0.
pub fn disco(radio: f32, caras: usize, m: Material) -> Mesh {
    let n = caras.max(6);
    let mut mesh = Mesh::default();
    for k in 0..n {
        let a0 = k as f32 / n as f32 * std::f32::consts::TAU;
        let a1 = (k + 1) as f32 / n as f32 * std::f32::consts::TAU;
        mesh.tri(
            [0.0, 0.0, 0.0],
            [radio * a0.cos(), radio * a0.sin(), 0.0],
            [radio * a1.cos(), radio * a1.sin(), 0.0],
            m,
        );
    }
    mesh
}

/// Anillo horizontal (marco de tapa, brida) en z = 0.
pub fn anillo(r_ext: f32, r_int: f32, caras: usize, m: Material) -> Mesh {
    let n = caras.max(6);
    let mut mesh = Mesh::default();
    for k in 0..n {
        let a0 = k as f32 / n as f32 * std::f32::consts::TAU;
        let a1 = (k + 1) as f32 / n as f32 * std::f32::consts::TAU;
        let (c0, s0) = a0.sin_cos();
        let (c1, s1) = a1.sin_cos();
        let e0 = [r_ext * c0, r_ext * s0, 0.0];
        let e1 = [r_ext * c1, r_ext * s1, 0.0];
        let i0 = [r_int * c0, r_int * s0, 0.0];
        let i1 = [r_int * c1, r_int * s1, 0.0];
        mesh.tri(e0, e1, i1, m);
        mesh.tri(e0, i1, i0, m);
    }
    mesh
}

/// Tubo vertical (pared interior + exterior) apoyado en z = 0.
pub fn tubo(r_ext: f32, r_int: f32, alto: f32, caras: usize, m: Material) -> Mesh {
    let n = caras.max(6);
    let mut mesh = Mesh::default();
    for k in 0..n {
        let a0 = k as f32 / n as f32 * std::f32::consts::TAU;
        let a1 = (k + 1) as f32 / n as f32 * std::f32::consts::TAU;
        let (c0, s0) = a0.sin_cos();
        let (c1, s1) = a1.sin_cos();
        let eb0 = [r_ext * c0, r_ext * s0, 0.0];
        let eb1 = [r_ext * c1, r_ext * s1, 0.0];
        let et0 = [r_ext * c0, r_ext * s0, alto];
        let et1 = [r_ext * c1, r_ext * s1, alto];
        let ib0 = [r_int * c0, r_int * s0, 0.0];
        let ib1 = [r_int * c1, r_int * s1, 0.0];
        let it0 = [r_int * c0, r_int * s0, alto];
        let it1 = [r_int * c1, r_int * s1, alto];
        // Exterior.
        mesh.tri(eb0, eb1, et1, m);
        mesh.tri(eb0, et1, et0, m);
        // Interior (normal hacia dentro: se invierte el orden).
        mesh.tri(ib0, it1, ib1, m);
        mesh.tri(ib0, it0, it1, m);
        // Boca superior.
        mesh.tri(et0, et1, it1, m);
        mesh.tri(et0, it1, it0, m);
    }
    mesh
}

/// Cono de reducción excéntrico: base de radio `r_base` centrada en el origen y boca de radio
/// `r_boca` desplazada `dx` en X. Con `dx = -(r_base - r_boca)` la generatriz del lado −X queda
/// vertical, alineada con la pared del fuste: es la cara por la que baja la escalera.
///
/// Con `espesor` se dibuja hueco (pared exterior, interior y canto de la boca), para que el corte y
/// la vista por la boca muestren el interior; sin él es la superficie exterior con la boca tapada.
pub fn tronco_excentrico(
    r_base: f32,
    r_boca: f32,
    alto: f32,
    dx: f32,
    espesor: Option<f32>,
    caras: usize,
    m: Material,
) -> Mesh {
    let n = caras.max(6);
    let mut mesh = Mesh::default();
    let e = espesor.unwrap_or(0.0).clamp(0.0, r_boca.min(r_base) - 0.01);
    for k in 0..n {
        let a0 = k as f32 / n as f32 * std::f32::consts::TAU;
        let a1 = (k + 1) as f32 / n as f32 * std::f32::consts::TAU;
        let (c0, s0) = a0.sin_cos();
        let (c1, s1) = a1.sin_cos();
        let base = |r: f32, c: f32, s: f32| [r * c, r * s, 0.0];
        let boca = |r: f32, c: f32, s: f32| [dx + r * c, r * s, alto];
        let (eb0, eb1) = (base(r_base, c0, s0), base(r_base, c1, s1));
        let (et0, et1) = (boca(r_boca, c0, s0), boca(r_boca, c1, s1));
        mesh.tri(eb0, eb1, et1, m);
        mesh.tri(eb0, et1, et0, m);
        if espesor.is_some() {
            let (ib0, ib1) = (base(r_base - e, c0, s0), base(r_base - e, c1, s1));
            let (it0, it1) = (boca(r_boca - e, c0, s0), boca(r_boca - e, c1, s1));
            mesh.tri(ib0, it1, ib1, m);
            mesh.tri(ib0, it0, it1, m);
            mesh.tri(et0, et1, it1, m);
            mesh.tri(et0, it1, it0, m);
        } else {
            mesh.tri([dx, 0.0, alto], et0, et1, m);
        }
    }
    mesh
}

/// Barra cilíndrica entre dos puntos (escalera, vástago, pasamanos).
pub fn barra(a: [f32; 3], b: [f32; 3], radio: f32, caras: usize, m: Material) -> Mesh {
    let n = caras.max(4);
    let d = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let l = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt().max(1e-5);
    let u = [d[0] / l, d[1] / l, d[2] / l];
    // Base ortonormal alrededor de u.
    let aux = if u[2].abs() < 0.9 {
        [0.0, 0.0, 1.0]
    } else {
        [1.0, 0.0, 0.0]
    };
    let mut v = [
        u[1] * aux[2] - u[2] * aux[1],
        u[2] * aux[0] - u[0] * aux[2],
        u[0] * aux[1] - u[1] * aux[0],
    ];
    let vl = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-5);
    v = [v[0] / vl, v[1] / vl, v[2] / vl];
    let w = [
        u[1] * v[2] - u[2] * v[1],
        u[2] * v[0] - u[0] * v[2],
        u[0] * v[1] - u[1] * v[0],
    ];
    let mut mesh = Mesh::default();
    for k in 0..n {
        let a0 = k as f32 / n as f32 * std::f32::consts::TAU;
        let a1 = (k + 1) as f32 / n as f32 * std::f32::consts::TAU;
        let p0 = [
            a[0] + radio * (v[0] * a0.cos() + w[0] * a0.sin()),
            a[1] + radio * (v[1] * a0.cos() + w[1] * a0.sin()),
            a[2] + radio * (v[2] * a0.cos() + w[2] * a0.sin()),
        ];
        let p1 = [
            a[0] + radio * (v[0] * a1.cos() + w[0] * a1.sin()),
            a[1] + radio * (v[1] * a1.cos() + w[1] * a1.sin()),
            a[2] + radio * (v[2] * a1.cos() + w[2] * a1.sin()),
        ];
        let q0 = [p0[0] + d[0], p0[1] + d[1], p0[2] + d[2]];
        let q1 = [p1[0] + d[0], p1[1] + d[1], p1[2] + d[2]];
        mesh.tri(p0, p1, q1, m);
        mesh.tri(p0, q1, q0, m);
    }
    mesh
}

/// Barra cilíndrica **cerrada** entre dos puntos (cuerpo de válvula, brida, tubo corto).
pub fn barra_tapada(a: [f32; 3], b: [f32; 3], radio: f32, caras: usize, m: Material) -> Mesh {
    let mut mesh = barra(a, b, radio, caras, m);
    let n = caras.max(4);
    // Tapas: abanico en cada extremo, orientado por la normal del eje.
    let d = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let l = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt().max(1e-5);
    let u = [d[0] / l, d[1] / l, d[2] / l];
    let aux = if u[2].abs() < 0.9 {
        [0.0, 0.0, 1.0]
    } else {
        [1.0, 0.0, 0.0]
    };
    let mut v = [
        u[1] * aux[2] - u[2] * aux[1],
        u[2] * aux[0] - u[0] * aux[2],
        u[0] * aux[1] - u[1] * aux[0],
    ];
    let vl = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-5);
    v = [v[0] / vl, v[1] / vl, v[2] / vl];
    let w = [
        u[1] * v[2] - u[2] * v[1],
        u[2] * v[0] - u[0] * v[2],
        u[0] * v[1] - u[1] * v[0],
    ];
    for (centro, signo) in [(a, -1.0f32), (b, 1.0)] {
        for k in 0..n {
            let a0 = k as f32 / n as f32 * std::f32::consts::TAU;
            let a1 = (k + 1) as f32 / n as f32 * std::f32::consts::TAU;
            let p0 = [
                centro[0] + radio * (v[0] * a0.cos() + w[0] * a0.sin()),
                centro[1] + radio * (v[1] * a0.cos() + w[1] * a0.sin()),
                centro[2] + radio * (v[2] * a0.cos() + w[2] * a0.sin()),
            ];
            let p1 = [
                centro[0] + radio * (v[0] * a1.cos() + w[0] * a1.sin()),
                centro[1] + radio * (v[1] * a1.cos() + w[1] * a1.sin()),
                centro[2] + radio * (v[2] * a1.cos() + w[2] * a1.sin()),
            ];
            if signo > 0.0 {
                mesh.tri(centro, p0, p1, m);
            } else {
                mesh.tri(centro, p1, p0, m);
            }
        }
    }
    mesh
}
/// Placa inclinada entre dos cotas: el polígono se dibuja a `z0` y `z1` y se une con paredes.
///
/// Sirve para placas de sedimentador, taludes de laguna y transiciones de aforador: `Mesh::girar`
/// solo gira alrededor de Z, así que una superficie inclinada no se podía componer con lo que
/// había.
pub fn rampa(poly: &[[f32; 2]], z0: f32, z1: f32, m: Material) -> Mesh {
    let mut mesh = Mesh::default();
    if poly.len() < 3 {
        return mesh;
    }
    let n = poly.len();
    let (cx, cy) = (
        poly.iter().map(|q| q[0]).sum::<f32>() / n as f32,
        poly.iter().map(|q| q[1]).sum::<f32>() / n as f32,
    );
    // Cara superior.
    for k in 0..n {
        let a = poly[k];
        let b = poly[(k + 1) % n];
        mesh.tri([cx, cy, z1], [a[0], a[1], z1], [b[0], b[1], z1], m);
    }
    // Cara inferior (girada) y paredes laterales.
    for k in 0..n {
        let a = poly[k];
        let b = poly[(k + 1) % n];
        mesh.tri([cx, cy, z0], [b[0], b[1], z0], [a[0], a[1], z0], m);
        mesh.tri([a[0], a[1], z0], [b[0], b[1], z0], [b[0], b[1], z1], m);
        mesh.tri([a[0], a[1], z0], [b[0], b[1], z1], [a[0], a[1], z1], m);
    }
    mesh
}

/// Marco rectangular (hueco) para rejas y compuertas: cuatro barras de sección cuadrada.
pub fn marco_rectangular(ancho: f32, alto: f32, espesor: f32, m: Material) -> Mesh {
    let mut mesh = Mesh::default();
    let (w, h, e) = (ancho.max(0.2), alto.max(0.2), espesor.max(0.02));
    mesh.extend(caja(w, e, e, m).mover([0.0, 0.0, h * 0.5]));
    mesh.extend(caja(w, e, e, m).mover([0.0, 0.0, -h * 0.5]));
    mesh.extend(caja(e, e, h, m).mover([w * 0.5, 0.0, 0.0]));
    mesh.extend(caja(e, e, h, m).mover([-w * 0.5, 0.0, 0.0]));
    mesh
}

/// Prisma extruido desde un polígono en planta (muros de cabeza, alas de descarga).
pub fn prisma(poly: &[[f32; 2]], alto: f32, m: Material) -> Mesh {
    if poly.len() < 3 {
        return Mesh::default();
    }
    let mut mesh = Mesh::default();
    let n = poly.len();
    // Laterales.
    for k in 0..n {
        let a = poly[k];
        let b = poly[(k + 1) % n];
        let a0 = [a[0], a[1], 0.0];
        let b0 = [b[0], b[1], 0.0];
        let a1 = [a[0], a[1], alto];
        let b1 = [b[0], b[1], alto];
        mesh.tri(a0, b0, b1, m);
        mesh.tri(a0, b1, a1, m);
    }
    // Tapa (abanico).
    let cx = poly.iter().map(|p| p[0]).sum::<f32>() / n as f32;
    let cy = poly.iter().map(|p| p[1]).sum::<f32>() / n as f32;
    for k in 0..n {
        let a = poly[k];
        let b = poly[(k + 1) % n];
        mesh.tri([cx, cy, alto], [a[0], a[1], alto], [b[0], b[1], alto], m);
    }
    mesh
}

// ---------------------------------------------------------------------------------------------
// Piezas

/// Parámetros que definen el dibujo de un pozo.
#[derive(Clone, Copy, Debug)]
pub struct PozoParams {
    pub diametro: f32,
    pub rim: f32,
    pub invert: f32,
    /// 0 = inspección, 1 = caída, 2 = resalto, 3 = ciego, 4 = bombeo, 5 = limpieza.
    pub tipo: u8,
    /// Detalle: 0 símbolo, 1 simple, 2 completo.
    pub lod: u8,
}

/// Altura del cono de reducción de un pozo (m): 0,6–0,9 m es lo habitual en obra.
pub const ALTO_CONO_MIN: f32 = 0.6;
pub const ALTO_CONO_MAX: f32 = 0.9;
/// Radio libre de la boca de acceso (m): 600 mm de paso, el de una tapa normalizada.
pub const RADIO_BOCA_LIBRE: f32 = 0.30;
/// Espesor de la pared del fuste y del cono (m).
const PARED_POZO: f32 = 0.12;
/// Separación entre la pared y el eje de los largueros de la escalera (m).
const SEPARACION_ESCALERA: f32 = 0.15;

/// Una parte del ensamblaje, con nombre, para poder separarla en la explosión.
#[derive(Clone, Debug)]
pub struct Parte {
    pub nombre: &'static str,
    pub mesh: Mesh,
}

/// Pozo de inspección con cono, marco, tapa y escalera. Origen en el centro de la solera.
pub fn pozo(p: PozoParams) -> Mesh {
    let mut mesh = Mesh::default();
    for parte in pozo_partes(p) {
        mesh.extend(parte.mesh);
    }
    mesh
}

/// El mismo pozo, despiezado: solera, fuste, cono, marco, tapa, escalera y accesorio del tipo.
pub fn pozo_partes(p: PozoParams) -> Vec<Parte> {
    let r = (p.diametro * 0.5).max(0.3);
    let h = (p.rim - p.invert).max(0.8);
    let mut partes: Vec<Parte> = Vec::new();

    // Solera.
    partes.push(Parte {
        nombre: "solera",
        mesh: cilindro(
            r + 0.12,
            0.16,
            if p.lod >= 2 { 16 } else { 8 },
            HORMIGON_OSCURO,
        ),
    });
    let arranque = 0.16;
    let caras = if p.lod >= 2 { 16 } else { 8 };
    // Cono excéntrico de 0,6–0,9 m (F12-11): la boca queda alineada con la pared del lado −X, que
    // sigue vertical hasta la tapa, y la escalera baja pegada a esa pared justo bajo la boca. En
    // pozos poco profundos el cono cede altura para que quede fuste. Es lo que hace reconocible un
    // pozo: se dibuja en todos los niveles de detalle, con la misma altura.
    let alto_cono = (h * 0.25)
        .clamp(ALTO_CONO_MIN, ALTO_CONO_MAX)
        .min(h - arranque - 0.2)
        .max(0.0);
    let cuerpo = (h - arranque - alto_cono).max(0.2);
    let r_int = (r - PARED_POZO).max(0.15);
    let r_boca = (RADIO_BOCA_LIBRE + PARED_POZO).min(r);
    // Centro de la boca (y de marco y tapa): desplazado hacia la escalera.
    let x_boca = -(r - r_boca);

    // Fuste hueco: con pared, la sección muestra espesor y el pozo no parece un tambor macizo.
    partes.push(Parte {
        nombre: "fuste",
        mesh: tubo(r, r_int, cuerpo, caras, HORMIGON).mover([0.0, 0.0, arranque]),
    });

    // Cono de reducción hasta el marco: hueco en el detalle completo, para ver la escalera por la
    // boca y en el corte.
    if alto_cono > 0.05 {
        partes.push(Parte {
            nombre: "cono",
            mesh: tronco_excentrico(
                r,
                r_boca,
                alto_cono,
                x_boca,
                (p.lod >= 2).then_some(r - r_int),
                caras,
                HORMIGON,
            )
            .mover([0.0, 0.0, arranque + cuerpo]),
        });
    }

    // Marco y tapa: el marco tiene canto (se ve en el corte) y la tapa lleva bisagra, que es lo
    // que hace que un pozo se reconozca a 40 px.
    let z_marco = arranque + cuerpo + alto_cono;
    let r_tapa = if p.lod >= 2 { 0.34f32 } else { 0.30 };
    partes.push(Parte {
        nombre: "marco",
        mesh: tubo(r_tapa + 0.10, r_tapa + 0.02, 0.07, 16, HIERRO).mover([
            x_boca,
            0.0,
            z_marco - 0.07,
        ]),
    });
    // La cara superior de la tapa queda a ras de la cota de tapa (la calle), no por encima.
    let mut tapa = cilindro(r_tapa, 0.055, 16, HIERRO).mover([x_boca, 0.0, z_marco - 0.0275]);
    if p.lod >= 2 {
        // Dibujo grabado (rebajado) y bisagra del lado +X, ambos dentro del espesor de la tapa.
        tapa.extend(cilindro(r_tapa * 0.55, 0.012, 12, HORMIGON_OSCURO).mover([
            x_boca,
            0.0,
            z_marco - 0.05,
        ]));
        tapa.extend(caja(0.10, 0.05, 0.035, HIERRO).mover([
            x_boca + r_tapa * 0.85,
            0.0,
            z_marco - 0.05,
        ]));
    }
    partes.push(Parte {
        nombre: "tapa",
        mesh: tapa,
    });

    // Escalera de acceso (dos largueros, peldaños y anclajes a la pared) en el lado −X, separada
    // 15 cm de la pared y bajo la boca: la pared de ese lado es vertical de la solera a la tapa,
    // así que sube hasta 30 cm bajo el marco sin cortar el cono.
    if p.lod >= 2 {
        let mut esc = Mesh::default();
        let x_pared = -r_int;
        let x = x_pared + SEPARACION_ESCALERA;
        // Media anchura que cabe en la boca a esa distancia del centro, con holgura.
        let libre = r_boca - PARED_POZO;
        let dx = x - x_boca;
        let semi = (libre * libre - dx * dx).max(0.0).sqrt();
        let lado = 0.16f32.min(semi * 0.8);
        let suelo_escalera = arranque + 0.3;
        let techo_escalera = (z_marco - 0.30).max(suelo_escalera + 0.1);
        for y in [-lado, lado] {
            esc.extend(barra(
                [x, y, suelo_escalera],
                [x, y, techo_escalera],
                0.022,
                5,
                HIERRO,
            ));
            for z in [suelo_escalera + 0.05, techo_escalera - 0.05] {
                esc.extend(barra([x_pared, y, z], [x, y, z], 0.015, 4, HIERRO));
            }
        }
        let mut z = arranque + 0.45;
        while z < techo_escalera - 0.05 {
            esc.extend(barra([x, -lado, z], [x, lado, z], 0.018, 5, HIERRO));
            z += 0.3;
        }
        partes.push(Parte {
            nombre: "escalera",
            mesh: esc,
        });
    }

    let mut extra = Mesh::default();
    match p.tipo {
        // Pozo de caída: tubo vertical de caída adosado al fuste.
        1 => {
            let r_caida = 0.10f32.max(p.diametro * 0.10);
            let h_caida = (h * 0.55).min(2.5);
            extra.extend(tubo(r_caida + 0.02, r_caida, h_caida, 10, PVC).mover([
                r * 0.55,
                0.0,
                arranque + 0.2,
            ]));
        }
        // Pozo de resalto: resalto en la solera.
        2 => {
            extra.extend(caja(0.5, 0.35, 0.20, HORMIGON_OSCURO).mover([0.0, 0.0, arranque]));
        }
        // Pozo ciego: sin salida, con rejilla de infiltración en la solera.
        3 => {
            for k in 0..5 {
                let y = -0.3 + k as f32 * 0.15;
                extra.extend(barra([-0.35, y, 0.17], [0.35, y, 0.17], 0.012, 4, HIERRO));
            }
        }
        // Cámara de bombeo: dos bombas sumergidas.
        4 => {
            for lado in [-0.28f32, 0.28] {
                extra.extend(cilindro(0.13, 0.55, 10, PLASTICO).mover([lado, 0.0, arranque]));
                extra.extend(barra(
                    [lado, 0.0, arranque + 0.55],
                    [lado, 0.0, arranque + 0.9],
                    0.04,
                    6,
                    HIERRO,
                ));
            }
        }
        // Pozo con arenero: cubeta de sedimentación bajo la solera.
        6 => {
            extra.extend(cilindro(r * 0.75, 0.35, 10, HORMIGON_OSCURO).mover([0.0, 0.0, -0.30]));
        }
        // Pozo disipador: dado deflector en la solera.
        7 => {
            extra.extend(
                prisma(
                    &[[-0.35, -0.25], [0.35, -0.25], [0.35, 0.25], [-0.35, 0.25]],
                    0.35,
                    HORMIGON_OSCURO,
                )
                .mover([0.0, 0.0, arranque]),
            );
        }
        // Caja de revisión: tapa cuadrada.
        5 => {
            extra.extend(caja(0.7, 0.7, 0.06, HIERRO).mover([0.0, 0.0, z_marco + 0.02]));
        }
        _ => {}
    }
    if !extra.is_empty() {
        partes.push(Parte {
            nombre: "accesorio",
            mesh: extra,
        });
    }
    partes
}

/// Válvula con cuerpo horizontal, bridas y accionamiento. Origen en el centro del cuerpo.
pub fn valvula(dn_mm: u32, tipo: &str, lod: u8) -> Mesh {
    let mut mesh = Mesh::default();
    for parte in valvula_partes(dn_mm, tipo, lod) {
        mesh.extend(parte.mesh);
    }
    mesh
}

/// La misma válvula despiezada: cuerpo, bridas y accionamiento.
pub fn valvula_partes(dn_mm: u32, tipo: &str, lod: u8) -> Vec<Parte> {
    let d = (dn_mm as f32 / 1000.0).max(0.05);
    let r = d * 0.62;
    let largo = (d * 1.9).max(0.22);
    let caras = if lod >= 2 { 14 } else { 8 };
    let mut partes = Vec::new();

    // Cuerpo: eje horizontal a lo largo de Y (el tubo llega por los extremos).
    partes.push(Parte {
        nombre: "cuerpo",
        mesh: barra_tapada(
            [0.0, -largo * 0.5, 0.0],
            [0.0, largo * 0.5, 0.0],
            r,
            caras,
            HIERRO,
        ),
    });
    // Bridas: discos cortos en los extremos.
    let mut bridas = Mesh::default();
    for lado in [-1.0f32, 1.0] {
        let y = lado * largo * 0.5;
        bridas.extend(barra_tapada(
            [0.0, y - 0.02, 0.0],
            [0.0, y + 0.02, 0.0],
            r * 1.45,
            caras,
            HIERRO,
        ));
    }
    partes.push(Parte {
        nombre: "bridas",
        mesh: bridas,
    });

    let mut acc = Mesh::default();
    match tipo {
        "mariposa" => {
            acc.extend(barra(
                [0.0, 0.0, r * 0.9],
                [0.0, 0.0, r * 0.9 + 0.06],
                0.05,
                8,
                HIERRO,
            ));
            acc.extend(barra(
                [0.0, 0.0, r * 0.9 + 0.05],
                [r * 1.9, 0.0, r * 0.9 + 0.22],
                0.018,
                6,
                HIERRO,
            ));
        }
        "retencion" => {
            for lado in [-1.0f32, 1.0] {
                acc.extend(barra(
                    [lado * r * 1.1, 0.0, 0.0],
                    [lado * r * 1.5, 0.0, 0.0],
                    0.03,
                    6,
                    HIERRO,
                ));
            }
        }
        "aire" | "purga" => {
            acc.extend(barra(
                [0.0, 0.0, 0.0],
                [0.0, 0.0, 0.18],
                r * 0.8,
                caras,
                HIERRO,
            ));
            acc.extend(tronco(r * 0.8, 0.04, 0.12, caras, HIERRO).mover([0.0, 0.0, 0.18]));
            acc.extend(cilindro(0.03, 0.05, 8, LATON).mover([0.0, 0.0, 0.30]));
        }
        "reduccion_presion" | "reguladora" | "reguladora_caudal" | "sostenedora" => {
            acc.extend(cilindro(r * 1.15, 0.14, caras, HIERRO).mover([0.0, 0.0, r * 0.75]));
            acc.extend(tronco(r * 1.15, r * 0.55, 0.10, caras, HIERRO).mover([
                0.0,
                0.0,
                r * 0.75 + 0.14,
            ]));
            acc.extend(barra(
                [0.0, 0.0, r * 0.75 + 0.24],
                [0.0, 0.0, r * 0.75 + 0.36],
                0.02,
                6,
                LATON,
            ));
        }
        _ => {
            // Compuerta u otras: vástago, volante y radios.
            let alto = 0.34 + d * 0.8;
            acc.extend(barra(
                [0.0, 0.0, r * 0.55],
                [0.0, 0.0, r * 0.55 + alto],
                0.025,
                6,
                HIERRO,
            ));
            let z = r * 0.55 + alto;
            acc.extend(anillo(0.12, 0.09, 12, HIERRO).mover([0.0, 0.0, z]));
            for k in 0..3 {
                let a = k as f32 / 3.0 * std::f32::consts::TAU;
                acc.extend(barra(
                    [0.0, 0.0, z],
                    [0.12 * a.cos(), 0.12 * a.sin(), z],
                    0.008,
                    4,
                    HIERRO,
                ));
            }
        }
    }
    partes.push(Parte {
        nombre: "accionamiento",
        mesh: acc,
    });
    partes
}

/// Accesorio de tubería (codo, tee, reducción, unión, tapón). Origen en el centro.
pub fn accesorio(tipo: &str, dn_mm: u32, angulo_grados: f32, lod: u8) -> Mesh {
    let d = (dn_mm as f32 / 1000.0).max(0.02);
    let r = d * 0.55;
    let largo = (d * 2.2).max(0.12);
    let caras = if lod >= 2 { 12 } else { 8 };
    let mut mesh = Mesh::default();
    let ang = angulo_grados.to_radians();

    match tipo {
        "codo" => {
            // Dos tramos que se encuentran con el ángulo pedido.
            mesh.extend(barra_tapada(
                [0.0, 0.0, 0.0],
                [largo, 0.0, 0.0],
                r,
                caras,
                PVC,
            ));
            mesh.extend(barra_tapada(
                [0.0, 0.0, 0.0],
                [largo * ang.cos(), largo * ang.sin(), 0.0],
                r,
                caras,
                PVC,
            ));
            mesh.extend(caja(d * 1.25, d * 1.25, d * 1.25, PVC).mover([
                -d * 0.62,
                -d * 0.62,
                -d * 0.62,
            ]));
        }
        "tee" => {
            mesh.extend(barra_tapada(
                [-largo, 0.0, 0.0],
                [largo, 0.0, 0.0],
                r,
                caras,
                PVC,
            ));
            mesh.extend(barra_tapada(
                [0.0, 0.0, 0.0],
                [0.0, largo, 0.0],
                r,
                caras,
                PVC,
            ));
        }
        "cruz" => {
            mesh.extend(barra_tapada(
                [-largo, 0.0, 0.0],
                [largo, 0.0, 0.0],
                r,
                caras,
                PVC,
            ));
            mesh.extend(barra_tapada(
                [0.0, -largo, 0.0],
                [0.0, largo, 0.0],
                r,
                caras,
                PVC,
            ));
        }
        "reduccion" => {
            mesh.extend(barra_tapada(
                [-largo, 0.0, 0.0],
                [0.0, 0.0, 0.0],
                r * 1.35,
                caras,
                PVC,
            ));
            mesh.extend(barra_tapada(
                [0.0, 0.0, 0.0],
                [largo, 0.0, 0.0],
                r * 0.8,
                caras,
                PVC,
            ));
        }
        "union" => {
            mesh.extend(barra_tapada(
                [-largo, 0.0, 0.0],
                [largo, 0.0, 0.0],
                r,
                caras,
                PVC,
            ));
            mesh.extend(barra_tapada(
                [-0.03, 0.0, 0.0],
                [0.03, 0.0, 0.0],
                r * 1.3,
                caras,
                HIERRO,
            ));
        }
        _ => {
            // Tapón: cuerpo corto y tapa.
            mesh.extend(barra_tapada(
                [0.0, 0.0, 0.0],
                [largo * 0.6, 0.0, 0.0],
                r,
                caras,
                PVC,
            ));
            mesh.extend(barra_tapada(
                [largo * 0.6, 0.0, 0.0],
                [largo * 0.68, 0.0, 0.0],
                r * 1.2,
                caras,
                HIERRO,
            ));
        }
    }
    mesh
}

/// Hidrante: cuerpo, brida, bonete y bocas.
pub fn hidrante(dn_mm: u32) -> Mesh {
    let d = (dn_mm as f32 / 1000.0).max(0.08);
    let r = (d * 0.55).max(0.06);
    let alto = 0.75;
    let mut mesh = Mesh::default();
    mesh.extend(cilindro(r * 1.5, 0.08, 12, HIERRO));
    mesh.extend(cilindro(r, alto, 12, HIERRO).mover([0.0, 0.0, 0.08]));
    mesh.extend(tronco(r, r * 0.55, 0.12, 12, HIERRO).mover([0.0, 0.0, 0.08 + alto]));
    // Bocas laterales.
    for lado in [-1.0f32, 1.0] {
        mesh.extend(barra_tapada(
            [lado * r * 0.95, 0.0, 0.08 + alto * 0.62],
            [lado * (r * 0.95 + 0.09), 0.0, 0.08 + alto * 0.62],
            0.05,
            8,
            LATON,
        ));
    }
    // Boca frontal.
    mesh.extend(barra_tapada(
        [0.0, r * 0.95, 0.08 + alto * 0.62],
        [0.0, r * 0.95 + 0.09, 0.08 + alto * 0.62],
        0.055,
        8,
        LATON,
    ));
    mesh
}

/// Tanque apoyado con cubierta cónica.
pub fn tanque(diametro: f32, altura: f32) -> Mesh {
    let r = (diametro * 0.5).max(0.6);
    let mut mesh = Mesh::default();
    mesh.extend(cilindro(r + 0.15, 0.25, 16, HORMIGON_OSCURO));
    mesh.extend(cilindro(r, altura, 18, HORMIGON));
    mesh.extend(tronco(r + 0.10, 0.12, (r * 0.45).min(1.6), 18, HIERRO).mover([0.0, 0.0, altura]));
    // Escalera exterior.
    for lado in [-0.14f32, 0.14] {
        mesh.extend(barra(
            [r + 0.1, lado, 0.2],
            [r + 0.1, lado, altura],
            0.02,
            4,
            HIERRO,
        ));
    }
    mesh
}

/// Descarga / vertedero: muro de cabeza, alas y losa de fondo.
pub fn descarga(dn_mm: u32) -> Mesh {
    let d = (dn_mm as f32 / 1000.0).max(0.15);
    let ancho = d * 3.0;
    let mut mesh = Mesh::default();
    mesh.extend(caja(ancho, 0.45, 1.5, HORMIGON).mover([0.0, 0.0, -0.6]));
    // Alas inclinadas.
    for lado in [-1.0f32, 1.0] {
        let poly = [
            [lado * ancho * 0.5, -0.2],
            [lado * ancho * 0.95, -0.9],
            [lado * ancho * 0.95, -1.15],
            [lado * ancho * 0.5, -0.45],
        ];
        mesh.extend(prisma(&poly, 1.1, HORMIGON).mover([0.0, 0.0, -0.5]));
    }
    // Losa de fondo (disipador).
    mesh.extend(caja(ancho * 1.6, 1.6, 0.25, HORMIGON_OSCURO).mover([0.0, -0.8, -0.85]));
    mesh
}

/// Cámara de válvulas: caja enterrada con tapa.
pub fn camara(diametro: f32, alto: f32) -> Mesh {
    let r = (diametro * 0.5).max(0.5);
    let mut mesh = Mesh::default();
    mesh.extend(cilindro(r + 0.12, 0.12, 14, HORMIGON_OSCURO));
    mesh.extend(tubo(r, r - 0.12, alto, 14, HORMIGON));
    mesh.extend(anillo(r + 0.12, r - 0.2, 14, HIERRO).mover([0.0, 0.0, alto]));
    mesh
}

/// Sumidero de calzada con rejilla.
pub fn sumidero() -> Mesh {
    let mut mesh = Mesh::default();
    mesh.extend(caja(0.75, 0.55, 0.9, HORMIGON_OSCURO));
    mesh.extend(caja(0.8, 0.6, 0.05, HIERRO).mover([0.0, 0.0, 0.9]));
    for k in 0..4 {
        let x = -0.28 + k as f32 * 0.19;
        mesh.extend(barra([x, -0.22, 0.93], [x, 0.22, 0.93], 0.015, 4, HIERRO));
    }
    mesh
}

/// Medidor domiciliario con carátula.
pub fn medidor(dn_mm: u32) -> Mesh {
    let d = (dn_mm as f32 / 1000.0).max(0.02);
    let r = d * 0.9;
    let mut mesh = Mesh::default();
    mesh.extend(cilindro(r, 0.16, 12, LATON));
    mesh.extend(disco(r * 0.8, 12, PLASTICO).mover([0.0, 0.0, 0.16]));
    mesh
}

/// Planta de tratamiento: edificio, dos tanques y patio.
pub fn planta(diametro: f32) -> Mesh {
    let r = (diametro * 0.5).clamp(3.0, 12.0);
    let mut m = Mesh::default();
    m.extend(caja(r * 1.1, r * 0.7, 4.0, HORMIGON));
    m.extend(cilindro(r * 0.45, 2.4, 16, HORMIGON_OSCURO).mover([r * 1.2, r * 0.2, 0.0]));
    m.extend(cilindro(r * 0.38, 2.0, 16, HORMIGON_OSCURO).mover([r * 1.2, -r * 0.9, 0.0]));
    m.extend(caja(r * 2.4, r * 1.8, 0.12, HORMIGON_OSCURO).mover([-r * 0.6, -r * 0.9, 0.0]));
    m
}

/// Captación: caja de toma, rejilla y tubo de salida.
pub fn captacion() -> Mesh {
    let mut m = Mesh::default();
    m.extend(caja(2.2, 1.6, 1.4, HORMIGON));
    for k in 0..6 {
        let x = -0.9 + k as f32 * 0.36;
        m.extend(barra([x, -0.85, 0.2], [x, -0.85, 1.2], 0.03, 5, HIERRO));
    }
    m.extend(barra_tapada(
        [0.0, 0.8, 0.5],
        [0.0, 1.8, 0.4],
        0.15,
        10,
        PVC,
    ));
    m
}

/// Hito / BM: fuste y placa.
pub fn hito(altura: f32) -> Mesh {
    let mut m = Mesh::default();
    m.extend(cilindro(0.06, altura.max(0.3), 8, HORMIGON));
    m.extend(caja(0.3, 0.04, 0.2, LATON).mover([-0.15, -0.02, altura * 0.72]));
    m
}

/// Cuerpo receptor: lámina de agua entre dos orillas.
pub fn cuerpo_receptor(ancho: f32, largo: f32) -> Mesh {
    let mut m = Mesh::default();
    let w = ancho.max(3.0);
    let l = largo.max(6.0);
    m.extend(caja(l, w, 0.05, AGUA).mover([-l * 0.5, -w * 0.5, 0.0]));
    // Orillas en talud.
    m.extend(prisma(
        &[
            [-l * 0.5, w * 0.5],
            [l * 0.5, w * 0.5],
            [l * 0.5, w * 0.9],
            [-l * 0.5, w * 0.9],
        ],
        0.8,
        HORMIGON_OSCURO,
    ));
    m.extend(prisma(
        &[
            [-l * 0.5, -w * 0.5],
            [l * 0.5, -w * 0.5],
            [l * 0.5, -w * 0.9],
            [-l * 0.5, -w * 0.9],
        ],
        0.8,
        HORMIGON_OSCURO,
    ));
    m
}

/// Cruce: dos apoyos y un tramo elevado (paso aéreo sobre vía o cauce).
pub fn cruce(luz: f32, dn_mm: u32) -> Mesh {
    let l = luz.clamp(4.0, 20.0);
    let d = (dn_mm as f32 / 1000.0).max(0.1);
    let mut m = Mesh::default();
    for lado in [-0.5f32, 0.5] {
        m.extend(caja(0.4, 0.4, 2.5, HORMIGON).mover([lado * l * 0.5, -0.2, 0.0]));
    }
    m.extend(barra_tapada(
        [-l * 0.5, 0.0, 2.5],
        [l * 0.5, 0.0, 2.5],
        d * 0.6,
        10,
        HIERRO,
    ));
    m
}

/// Tanque elevado: fuste, cuatro patas y cuba con cubierta.
pub fn tanque_elevado(diametro: f32, altura: f32) -> Mesh {
    let r = (diametro * 0.5).clamp(2.0, 9.0);
    let h = altura.clamp(3.0, 20.0);
    let mut m = Mesh::default();
    for k in 0..4 {
        let a = k as f32 / 4.0 * std::f32::consts::TAU + 0.78;
        m.extend(barra(
            [r * 0.75 * a.cos(), r * 0.75 * a.sin(), 0.0],
            [r * 0.75 * a.cos(), r * 0.75 * a.sin(), h],
            0.16,
            8,
            HORMIGON,
        ));
    }
    m.extend(cilindro(r, h * 0.28, 18, HORMIGON).mover([0.0, 0.0, h]));
    m.extend(tronco(r + 0.1, 0.1, r * 0.4, 18, HIERRO).mover([0.0, 0.0, h + h * 0.28]));
    m
}

/// Canal abierto a lo largo de un tramo: plantilla y dos muros.
pub fn canal(a: P3, b: P3, ancho: f32, alto: f32) -> Mesh {
    let (ax, ay, az) = (a.x as f32, a.y as f32, a.z as f32);
    let (bx, by, bz) = (b.x as f32, b.y as f32, b.z as f32);
    let dx = bx - ax;
    let dy = by - ay;
    let l = (dx * dx + dy * dy).sqrt().max(0.1);
    let (ux, uy) = (dx / l, dy / l);
    let (nx, ny) = (-uy, ux);
    let hw = ancho.max(0.3) * 0.5;
    let mut m = Mesh::default();
    // Solera.
    m.extend(prisma(
        &[
            [ax + nx * hw, ay + ny * hw],
            [bx + nx * hw, by + ny * hw],
            [bx - nx * hw, by - ny * hw],
            [ax - nx * hw, ay - ny * hw],
        ],
        0.2,
        HORMIGON_OSCURO,
    ));
    // Muros.
    for lado in [-1.0f32, 1.0] {
        m.extend(prisma(
            &[
                [ax + nx * hw * lado, ay + ny * hw * lado],
                [bx + nx * hw * lado, by + ny * hw * lado],
                [bx + nx * (hw + 0.25) * lado, by + ny * (hw + 0.25) * lado],
                [ax + nx * (hw + 0.25) * lado, ay + ny * (hw + 0.25) * lado],
            ],
            alto.max(0.4),
            HORMIGON,
        ));
    }
    let _ = az;
    let _ = bz;
    m
}

/// Tubería entre dos puntos con juntas cada `paso` metros. Se genera en coordenadas de mundo.
pub fn tuberia(a: P3, b: P3, d_ext: f32, lod: u8, m: Material) -> Mesh {
    let caras = if lod >= 2 { 12 } else { 6 };
    let direccion = [b.x - a.x, b.y - a.y, b.z - a.z];
    let largo =
        (direccion[0] * direccion[0] + direccion[1] * direccion[1] + direccion[2] * direccion[2])
            .sqrt();
    if largo < 0.05 {
        return Mesh::default();
    }
    let p0 = [a.x as f32, a.y as f32, a.z as f32];
    let p1 = [b.x as f32, b.y as f32, b.z as f32];
    let mut mesh = Mesh::default();
    mesh.extend(barra(p0, p1, d_ext * 0.5, caras, m));
    if lod >= 2 {
        // Campanas cada 6 m: anillos cortos y algo más gruesos.
        let n = (largo / 6.0).floor() as usize;
        for k in 1..=n {
            let t = k as f32 * 6.0 / largo as f32;
            if t >= 1.0 {
                break;
            }
            let centro = [
                p0[0] + (p1[0] - p0[0]) * t,
                p0[1] + (p1[1] - p0[1]) * t,
                p0[2] + (p1[2] - p0[2]) * t,
            ];
            let caja_j = caja(d_ext * 1.12, d_ext * 1.12, d_ext * 1.25, m).mover([
                centro[0] - d_ext * 0.56,
                centro[1] - d_ext * 0.56,
                centro[2] - d_ext * 0.62,
            ]);
            mesh.extend(caja_j);
        }
    }
    mesh
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn las_piezas_generan_triangulos() {
        let p = pozo(PozoParams {
            diametro: 1.2,
            rim: 2512.0,
            invert: 2510.0,
            tipo: 0,
            lod: 2,
        });
        assert!(p.len() > 200, "pozo completo: {} triángulos", p.len());
        let (lo, hi) = p.bbox();
        assert!((lo[2] - 0.0).abs() < 0.2, "arranca en la solera: {lo:?}");
        assert!((hi[2] - 2.0).abs() < 0.1, "la tapa llega al rim: {hi:?}");
    }

    fn parte<'a>(partes: &'a [Parte], nombre: &str) -> &'a Mesh {
        &partes
            .iter()
            .find(|p| p.nombre == nombre)
            .unwrap_or_else(|| panic!("falta la parte {nombre}"))
            .mesh
    }

    /// F12-11: la escalera cae dentro de la boca (se puede bajar) y el cono mide 0,6–0,9 m.
    #[test]
    fn la_escalera_cae_dentro_de_la_boca() {
        for (diametro, alto) in [(1.2f32, 3.0f32), (1.0, 2.0), (1.5, 5.0), (1.2, 1.6)] {
            let partes = pozo_partes(PozoParams {
                diametro,
                rim: alto,
                invert: 0.0,
                tipo: 0,
                lod: 2,
            });
            // Centro de la boca = centro de la tapa.
            let (lo, hi) = parte(&partes, "tapa").bbox();
            let centro = [(lo[0] + hi[0]) * 0.5, (lo[1] + hi[1]) * 0.5];
            let (_, alto_fuste) = parte(&partes, "fuste").bbox();
            for t in &parte(&partes, "escalera").tris {
                for v in &t.p {
                    let d = ((v[0] - centro[0]).powi(2) + (v[1] - centro[1]).powi(2)).sqrt();
                    // Los anclajes van a la pared; los largueros y peldaños, bajo la boca.
                    let en_pared = v[0] < -(diametro * 0.5 - PARED_POZO) + 0.05;
                    assert!(
                        en_pared || d < RADIO_BOCA_LIBRE,
                        "DI {diametro}: un punto de la escalera queda a {d:.2} m del eje de la \
                         boca (radio libre {RADIO_BOCA_LIBRE})"
                    );
                }
            }
            // La escalera sube por el cono hasta cerca de la tapa, no se queda en el fuste.
            let (_, hi_esc) = parte(&partes, "escalera").bbox();
            assert!(
                hi_esc[2] > alto_fuste[2] + 0.2,
                "DI {diametro}: la escalera acaba en {:.2}, fuste hasta {:.2}",
                hi_esc[2],
                alto_fuste[2]
            );
            assert!(hi_esc[2] < alto - 0.2, "la escalera no asoma por la tapa");

            let (lo_c, hi_c) = parte(&partes, "cono").bbox();
            let alto_cono = hi_c[2] - lo_c[2];
            if alto >= 2.0 {
                assert!(
                    (ALTO_CONO_MIN - 1e-3..=ALTO_CONO_MAX + 1e-3).contains(&alto_cono),
                    "cono de {alto_cono:.2} m"
                );
            }
            // Excéntrico: la boca toca la pared del lado de la escalera.
            assert!(
                (lo_c[0] + diametro * 0.5).abs() < 0.02,
                "la generatriz −X debe seguir la pared: {lo_c:?}"
            );
            assert!(centro[0] < -0.05, "la tapa se desplaza hacia la escalera");
        }
    }

    /// El cono excéntrico se dibuja en todos los niveles de detalle (también en símbolo y simple).
    #[test]
    fn el_cono_excentrico_respeta_el_lod() {
        for lod in 0..=2u8 {
            let partes = pozo_partes(PozoParams {
                diametro: 1.2,
                rim: 3.0,
                invert: 0.0,
                tipo: 0,
                lod,
            });
            let (lo, hi) = parte(&partes, "cono").bbox();
            assert!(hi[2] - lo[2] >= ALTO_CONO_MIN - 1e-3, "LOD {lod}");
            assert_eq!(
                partes.iter().any(|p| p.nombre == "escalera"),
                lod >= 2,
                "la escalera es solo del detalle completo"
            );
        }
    }

    #[test]
    fn las_valvulas_tienen_forma_distinta() {
        let compuerta = valvula(200, "compuerta", 2);
        let retencion = valvula(200, "retencion", 2);
        assert!(compuerta.len() > retencion.len());
        let (_, hi_c) = compuerta.bbox();
        let (_, hi_r) = retencion.bbox();
        assert!(hi_c[2] > hi_r[2] + 0.10, "la compuerta tiene volante alto");
    }

    #[test]
    fn el_lod_reduce_triangulos() {
        let base = PozoParams {
            diametro: 1.2,
            rim: 2512.0,
            invert: 2508.0,
            tipo: 0,
            lod: 2,
        };
        let mut simple = base;
        simple.lod = 1;
        assert!(pozo(simple).len() < pozo(base).len());
    }
}
