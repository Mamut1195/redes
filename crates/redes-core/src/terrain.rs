//! Superficie topográfica: nube de puntos → TIN (Delaunay) → consulta de cota,
//! pendiente y curvas de nivel por *marching squares*.
//!
//! Decisiones del dominio que se respetan aquí:
//! - La superficie es **versionada** (`Terrain::version`) y se identifica por `name`.
//! - La cota se interpola **solo dentro del casco convexo**; fuera se devuelve `None`
//!   (el motor nunca extrapola en silencio).
//! - Los puntos se guardan en coordenadas proyectadas (E, N) y cota `z` en metros.

use crate::geom::{Bbox, P2};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct SurfacePoint {
    pub e: f64,
    pub n: f64,
    pub z: f64,
}

impl SurfacePoint {
    pub fn new(e: f64, n: f64, z: f64) -> Self {
        SurfacePoint { e, n, z }
    }
    fn p2(&self) -> P2 {
        P2::new(self.e, self.n)
    }
}

/// Malla triangular irregular. `tris[i]` son índices a `points`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Terrain {
    pub name: String,
    pub epsg: u32,
    /// Sube con cada edición de la superficie; los resultados que dependen de ella lo citan.
    pub version: u32,
    pub points: Vec<SurfacePoint>,
    pub tris: Vec<[u32; 3]>,
    pub bbox: Bbox,
    #[serde(skip)]
    index: TriIndex,
}

#[derive(Clone, Debug, Default)]
struct TriIndex {
    cell: f64,
    cells: std::collections::HashMap<(i32, i32), Vec<u32>>,
}

impl TriIndex {
    fn build(terrain_points: &[SurfacePoint], tris: &[[u32; 3]], bbox: Bbox) -> TriIndex {
        let cell = ((bbox.width().max(bbox.height())) / 48.0).max(1.0);
        let mut idx = TriIndex {
            cell,
            cells: Default::default(),
        };
        for (t, tri) in tris.iter().enumerate() {
            let mut bb = Bbox::default();
            for &i in tri {
                bb.add(terrain_points[i as usize].p2());
            }
            let x0 = (bb.xmin / cell).floor() as i32;
            let x1 = (bb.xmax / cell).floor() as i32;
            let y0 = (bb.ymin / cell).floor() as i32;
            let y1 = (bb.ymax / cell).floor() as i32;
            for cx in x0..=x1 {
                for cy in y0..=y1 {
                    idx.cells.entry((cx, cy)).or_default().push(t as u32);
                }
            }
        }
        idx
    }

    fn candidates(&self, p: P2) -> Vec<u32> {
        let k = (
            (p.x / self.cell).floor() as i32,
            (p.y / self.cell).floor() as i32,
        );
        self.cells.get(&k).cloned().unwrap_or_default()
    }
}

impl Terrain {
    /// Construye el TIN por triangulación de Delaunay de la planta de los puntos.
    pub fn build(name: &str, epsg: u32, points: Vec<SurfacePoint>) -> Terrain {
        // `delaunator` indexa con `-1` cuando recibe NaN y el proceso aborta: se filtran los
        // puntos no finitos antes de triangular (un CSV/DXF hostil puede traerlos).
        let points: Vec<SurfacePoint> = points
            .into_iter()
            .filter(|p| p.e.is_finite() && p.n.is_finite() && p.z.is_finite())
            .collect();
        let mut bbox = Bbox::default();
        for p in &points {
            bbox.add(p.p2());
        }
        let mut coords: Vec<delaunator::Point> = Vec::with_capacity(points.len());
        for p in &points {
            coords.push(delaunator::Point { x: p.e, y: p.n });
        }
        let mut tris: Vec<[u32; 3]> = Vec::new();
        if points.len() >= 3 {
            let d = delaunator::triangulate(&coords);
            for t in d.triangles.chunks(3) {
                let tri = [t[0] as u32, t[1] as u32, t[2] as u32];
                // Descarta triángulos degenerados (colineales) para no dividir por cero.
                if tri_area(&points, tri) > 1e-9 {
                    tris.push(tri);
                }
            }
        }
        let index = TriIndex::build(&points, &tris, bbox);
        Terrain {
            name: name.to_string(),
            epsg,
            version: 1,
            points,
            tris,
            bbox,
            index,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.tris.is_empty()
    }

    /// Cota interpolada en planta. `None` fuera del TIN.
    /// Reconstruye el índice espacial (se pierde al serializar: `#[serde(skip)]`).
    ///
    /// Sin esto, un proyecto cargado de disco tiene el TIN pero `z_at` devuelve `None`
    /// para cualquier punto: los edificios y las calles no se dibujan.
    pub fn reindexar(&mut self) {
        self.index = TriIndex::build(&self.points, &self.tris, self.bbox);
    }

    pub fn z_at(&self, p: P2) -> Option<f64> {
        if !self.bbox.contains(p) {
            return None;
        }
        let mut best: Option<(f64, f64)> = None;
        for t in self.index.candidates(p) {
            let tri = self.tris[t as usize];
            let (a, b, c) = (
                self.points[tri[0] as usize],
                self.points[tri[1] as usize],
                self.points[tri[2] as usize],
            );
            if let Some((w0, w1, w2)) = barycentric(p, a.p2(), b.p2(), c.p2())
                && w0 >= -1e-9
                && w1 >= -1e-9
                && w2 >= -1e-9
            {
                let z = w0 * a.z + w1 * b.z + w2 * c.z;
                let depth = w0.min(w1).min(w2);
                if best.is_none_or(|(bd, _)| depth > bd) {
                    best = Some((depth, z));
                }
            }
        }
        best.map(|(_, z)| z)
    }

    /// Pendiente en tanto por uno (`dz/ds`, positiva cuesta arriba) y su dirección en radianes.
    pub fn slope_at(&self, p: P2, step: f64) -> Option<(f64, f64)> {
        let h = step.max(0.5);
        let zx =
            (self.z_at(P2::new(p.x + h, p.y))? - self.z_at(P2::new(p.x - h, p.y))?) / (2.0 * h);
        let zy =
            (self.z_at(P2::new(p.x, p.y + h))? - self.z_at(P2::new(p.x, p.y - h))?) / (2.0 * h);
        Some(((zx * zx + zy * zy).sqrt(), zy.atan2(zx)))
    }

    /// Rango de cotas de la superficie.
    pub fn z_range(&self) -> (f64, f64) {
        let mut lo = f64::INFINITY;
        let mut hi = f64::NEG_INFINITY;
        for p in &self.points {
            lo = lo.min(p.z);
            hi = hi.max(p.z);
        }
        if lo > hi { (0.0, 0.0) } else { (lo, hi) }
    }

    /// Curvas de nivel por *marching squares* sobre una rejilla muestreada del TIN.
    /// Devuelve segmentos agrupados por cota, listos para dibujar.
    pub fn contours(&self, interval: f64, cells: usize) -> Vec<Contour> {
        let (zmin, zmax) = self.z_range();
        // `is_nan` explícito: `!(a > b)` y `a <= b` no son equivalentes con NaN.
        if interval.is_nan() || interval <= 0.0 || self.is_empty() {
            return Vec::new();
        }
        let nx = cells.clamp(8, 400);
        let ny = nx;
        let (x0, y0, x1, y1) = (
            self.bbox.xmin,
            self.bbox.ymin,
            self.bbox.xmax,
            self.bbox.ymax,
        );
        let dx = (x1 - x0) / nx as f64;
        let dy = (y1 - y0) / ny as f64;
        let mut grid = vec![f64::NAN; (nx + 1) * (ny + 1)];
        for j in 0..=ny {
            for i in 0..=nx {
                let p = P2::new(x0 + i as f64 * dx, y0 + j as f64 * dy);
                if let Some(z) = self.z_at(p) {
                    grid[j * (nx + 1) + i] = z;
                }
            }
        }
        let first = (zmin / interval).ceil() * interval;
        let mut out = Vec::new();
        let mut level = first;
        while level < zmax {
            let mut segments = Vec::new();
            for j in 0..ny {
                for i in 0..nx {
                    let za = grid[j * (nx + 1) + i];
                    let zb = grid[j * (nx + 1) + i + 1];
                    let zc = grid[(j + 1) * (nx + 1) + i + 1];
                    let zd = grid[(j + 1) * (nx + 1) + i];
                    if za.is_nan() || zb.is_nan() || zc.is_nan() || zd.is_nan() {
                        continue;
                    }
                    let pa = P2::new(x0 + i as f64 * dx, y0 + j as f64 * dy);
                    let pb = P2::new(pa.x + dx, pa.y);
                    let pc = P2::new(pa.x + dx, pa.y + dy);
                    let pd = P2::new(pa.x, pa.y + dy);
                    let mut pts: Vec<P2> = Vec::with_capacity(4);
                    for (p, q, zp, zq) in [
                        (pa, pb, za, zb),
                        (pb, pc, zb, zc),
                        (pc, pd, zc, zd),
                        (pd, pa, zd, za),
                    ] {
                        // `!=` en vez de un producto: un vértice exactamente en la cota también
                        // cuenta como cruce (si no, un plano alineado con la rejilla no genera
                        // ninguna curva).
                        if (zp < level) != (zq < level) {
                            let t = (level - zp) / (zq - zp);
                            pts.push(p.lerp(q, t));
                        }
                    }
                    match pts.len() {
                        2 => segments.push([pts[0], pts[1]]),
                        4 => {
                            // Caso ambiguo: se decide por el promedio del centro.
                            let center = (za + zb + zc + zd) * 0.25;
                            if center > level {
                                segments.push([pts[0], pts[3]]);
                                segments.push([pts[1], pts[2]]);
                            } else {
                                segments.push([pts[0], pts[1]]);
                                segments.push([pts[2], pts[3]]);
                            }
                        }
                        _ => {}
                    }
                }
            }
            if !segments.is_empty() {
                out.push(Contour { z: level, segments });
            }
            level += interval;
        }
        out
    }

    /// Versión aligerada para dibujar: conserva un punto por celda de `cell` metros.
    /// El TIN completo se reserva para consultas; la malla de pantalla no necesita 2 400 puntos.
    pub fn thinned(&self, cell: f64) -> Terrain {
        let cell = cell.max(0.5);
        let mut seen = std::collections::HashSet::new();
        let mut pts = Vec::new();
        for p in &self.points {
            let key = ((p.e / cell).floor() as i64, (p.n / cell).floor() as i64);
            if seen.insert(key) {
                pts.push(*p);
            }
        }
        if pts.len() < 3 {
            return self.clone();
        }
        Terrain::build(&self.name, self.epsg, pts)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Contour {
    pub z: f64,
    pub segments: Vec<[P2; 2]>,
}

fn tri_area(points: &[SurfacePoint], tri: [u32; 3]) -> f64 {
    let a = points[tri[0] as usize];
    let b = points[tri[1] as usize];
    let c = points[tri[2] as usize];
    ((b.e - a.e) * (c.n - a.n) - (c.e - a.e) * (b.n - a.n)).abs() * 0.5
}

fn barycentric(p: P2, a: P2, b: P2, c: P2) -> Option<(f64, f64, f64)> {
    let den = (b.y - c.y) * (a.x - c.x) + (c.x - b.x) * (a.y - c.y);
    if den.abs() < 1e-12 {
        return None;
    }
    let w0 = ((b.y - c.y) * (p.x - c.x) + (c.x - b.x) * (p.y - c.y)) / den;
    let w1 = ((c.y - a.y) * (p.x - c.x) + (a.x - c.x) * (p.y - c.y)) / den;
    let w2 = 1.0 - w0 - w1;
    Some((w0, w1, w2))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plane() -> Terrain {
        // Plano inclinado z = 100 + x/10 en una rejilla 3×3.
        let mut pts = Vec::new();
        for j in 0..3 {
            for i in 0..3 {
                let x = i as f64 * 10.0;
                let y = j as f64 * 10.0;
                pts.push(SurfacePoint::new(x, y, 100.0 + x / 10.0));
            }
        }
        Terrain::build("plano", 32717, pts)
    }

    #[test]
    fn interpola_un_plano_exacto() {
        let t = plane();
        let z = t.z_at(P2::new(5.0, 4.0)).unwrap();
        assert!((z - 100.5).abs() < 1e-9, "z = {z}");
    }

    #[test]
    fn fuera_del_tin_devuelve_none() {
        let t = plane();
        assert!(t.z_at(P2::new(-5.0, 5.0)).is_none());
        assert!(t.z_at(P2::new(500.0, 5.0)).is_none());
    }

    #[test]
    fn pendiente_de_un_plano() {
        let t = plane();
        let (s, _) = t.slope_at(P2::new(5.0, 5.0), 1.0).unwrap();
        assert!((s - 0.1).abs() < 1e-6, "s = {s}");
    }

    #[test]
    fn genera_curvas() {
        let t = plane();
        let c = t.contours(1.0, 40);
        assert!(!c.is_empty());
        assert!(c.iter().all(|l| !l.segments.is_empty()));
    }
}
