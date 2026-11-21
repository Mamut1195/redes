//! Geometría plana y de planta. Unidades internas: **metros** (SI), ángulos en radianes.
//!
//! Todo el modelo trabaja en coordenadas proyectadas (E, N) con origen local del proyecto;
//! la conversión a pantalla es responsabilidad de la UI, no del motor.

use serde::{Deserialize, Serialize};

/// Punto en planta (Este, Norte) en metros.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct P2 {
    pub x: f64,
    pub y: f64,
}

impl P2 {
    pub const fn new(x: f64, y: f64) -> P2 {
        P2 { x, y }
    }
    pub fn dist(self, o: P2) -> f64 {
        ((self.x - o.x).powi(2) + (self.y - o.y).powi(2)).sqrt()
    }
    /// Longitud del tramo entre dos puntos.
    pub fn len_to(self, o: P2) -> f64 {
        self.dist(o)
    }
    pub fn lerp(self, o: P2, t: f64) -> P2 {
        P2::new(self.x + (o.x - self.x) * t, self.y + (o.y - self.y) * t)
    }
    pub fn to_array(self) -> [f64; 2] {
        [self.x, self.y]
    }
}

/// Punto con cota (terreno, clave, rasante…). `z` en metros sobre el datum del proyecto.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct P3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl P3 {
    pub const fn new(x: f64, y: f64, z: f64) -> P3 {
        P3 { x, y, z }
    }
    pub fn p2(self) -> P2 {
        P2::new(self.x, self.y)
    }
}

/// Distancia punto–segmento en planta (para hit-testing y trazado).
pub fn dist_to_segment(p: P2, a: P2, b: P2) -> f64 {
    let abx = b.x - a.x;
    let aby = b.y - a.y;
    let len2 = abx * abx + aby * aby;
    if len2 <= f64::EPSILON {
        return p.dist(a);
    }
    let t = (((p.x - a.x) * abx + (p.y - a.y) * aby) / len2).clamp(0.0, 1.0);
    p.dist(P2::new(a.x + abx * t, a.y + aby * t))
}

/// Punto más cercano de un segmento, con su parámetro `t` en `0..=1`.
pub fn closest_on_segment(p: P2, a: P2, b: P2) -> (P2, f64) {
    let abx = b.x - a.x;
    let aby = b.y - a.y;
    let len2 = abx * abx + aby * aby;
    if len2 <= f64::EPSILON {
        return (a, 0.0);
    }
    let t = (((p.x - a.x) * abx + (p.y - a.y) * aby) / len2).clamp(0.0, 1.0);
    (P2::new(a.x + abx * t, a.y + aby * t), t)
}

/// Longitud 3D de una polilínea de puntos con cota.
pub fn polyline_len_3d(points: &[P3]) -> f64 {
    points
        .windows(2)
        .map(|w| {
            let dx = w[1].x - w[0].x;
            let dy = w[1].y - w[0].y;
            let dz = w[1].z - w[0].z;
            (dx * dx + dy * dy + dz * dz).sqrt()
        })
        .sum()
}

/// Longitud en planta.
pub fn polyline_len_2d(points: &[P2]) -> f64 {
    points.windows(2).map(|w| w[0].dist(w[1])).sum()
}

/// Área de un polígono simple por la fórmula del cordón (positiva si el anillo es antihorario).
pub fn polygon_area(points: &[P2]) -> f64 {
    let n = points.len();
    if n < 3 {
        return 0.0;
    }
    let mut s = 0.0;
    for i in 0..n {
        let a = points[i];
        let b = points[(i + 1) % n];
        s += a.x * b.y - b.x * a.y;
    }
    s * 0.5
}

/// Caja envolvente `[xmin, ymin, xmax, ymax]`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Bbox {
    pub xmin: f64,
    pub ymin: f64,
    pub xmax: f64,
    pub ymax: f64,
}

impl Default for Bbox {
    fn default() -> Self {
        Bbox { xmin: f64::INFINITY, ymin: f64::INFINITY, xmax: f64::NEG_INFINITY, ymax: f64::NEG_INFINITY }
    }
}

impl Bbox {
    pub fn add(&mut self, p: P2) {
        self.xmin = self.xmin.min(p.x);
        self.ymin = self.ymin.min(p.y);
        self.xmax = self.xmax.max(p.x);
        self.ymax = self.ymax.max(p.y);
    }
    pub fn is_empty(&self) -> bool {
        self.xmin > self.xmax
    }
    pub fn contains(&self, p: P2) -> bool {
        p.x >= self.xmin && p.x <= self.xmax && p.y >= self.ymin && p.y <= self.ymax
    }
    pub fn width(&self) -> f64 {
        if self.is_empty() { 0.0 } else { self.xmax - self.xmin }
    }
    pub fn height(&self) -> f64 {
        if self.is_empty() { 0.0 } else { self.ymax - self.ymin }
    }
    pub fn center(&self) -> P2 {
        P2::new((self.xmin + self.xmax) * 0.5, (self.ymin + self.ymax) * 0.5)
    }
    pub fn to_array(&self) -> [f64; 4] {
        [self.xmin, self.ymin, self.xmax, self.ymax]
    }
}

/// Índice espacial de celdas fijas para hit-testing de arcos (sin dependencias).
pub struct SegmentGrid {
    cell: f64,
    cells: std::collections::HashMap<(i32, i32), Vec<u32>>,
}

impl SegmentGrid {
    pub fn new(cell: f64) -> SegmentGrid {
        SegmentGrid { cell: cell.max(0.5), cells: Default::default() }
    }

    fn key(&self, p: P2) -> (i32, i32) {
        ((p.x / self.cell).floor() as i32, (p.y / self.cell).floor() as i32)
    }

    pub fn insert(&mut self, id: u32, a: P2, b: P2) {
        let (x0, y0) = self.key(P2::new(a.x.min(b.x), a.y.min(b.y)));
        let (x1, y1) = self.key(P2::new(a.x.max(b.x), a.y.max(b.y)));
        for cx in x0..=x1 {
            for cy in y0..=y1 {
                self.cells.entry((cx, cy)).or_default().push(id);
            }
        }
    }

    /// Ids de segmentos cuya celda cae a menos de `tol` del punto.
    pub fn near(&self, p: P2, tol: f64) -> Vec<u32> {
        let reach = (tol / self.cell).ceil() as i32 + 1;
        let (cx, cy) = self.key(p);
        let mut out = Vec::new();
        for gx in cx - reach..=cx + reach {
            for gy in cy - reach..=cy + reach {
                if let Some(ids) = self.cells.get(&(gx, gy)) {
                    out.extend_from_slice(ids);
                }
            }
        }
        out.sort_unstable();
        out.dedup();
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distancia_a_segmento() {
        let a = P2::new(0.0, 0.0);
        let b = P2::new(10.0, 0.0);
        assert!((dist_to_segment(P2::new(5.0, 3.0), a, b) - 3.0).abs() < 1e-9);
        assert!((dist_to_segment(P2::new(-4.0, 0.0), a, b) - 4.0).abs() < 1e-9);
    }

    #[test]
    fn area_de_un_cuadrado() {
        let sq = [P2::new(0.0, 0.0), P2::new(2.0, 0.0), P2::new(2.0, 2.0), P2::new(0.0, 2.0)];
        assert!((polygon_area(&sq).abs() - 4.0).abs() < 1e-12);
    }

    #[test]
    fn longitud_3d() {
        let p = [P3::new(0.0, 0.0, 0.0), P3::new(3.0, 0.0, 4.0)];
        assert!((polyline_len_3d(&p) - 5.0).abs() < 1e-12);
    }
}
