//! Topographic surface: points → constrained Delaunay TIN → height, slope and contour lines.
//!
//! - The surface is versioned (`Terrain::version`) and named (`name`).
//! - Height is interpolated only on the triangles. Beyond the boundary, inside a hole or past the
//!   convex hull `z_at` returns `None`: the engine never extrapolates.
//! - Breaklines and the boundary and hole edges are constraint edges of the triangulation, so no
//!   triangle crosses them (D18, F17-11).
//! - Points are projected (E, N) with height `z` in metres, in `vertical_datum`.

use std::collections::{HashMap, HashSet};

use crate::geom::{Bbox, P2};
use serde::{Deserialize, Serialize};
use spade::{ConstrainedDelaunayTriangulation, HasPosition, Point2, Triangulation};

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
    fn is_finite(&self) -> bool {
        self.e.is_finite() && self.n.is_finite() && self.z.is_finite()
    }
}

impl HasPosition for SurfacePoint {
    type Scalar = f64;
    fn position(&self) -> Point2<f64> {
        Point2::new(self.e, self.n)
    }
}

/// Reference surface of the heights (F17-12). Redes never converts between them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum VerticalDatum {
    #[serde(rename = "egm2008")]
    Egm2008,
    #[serde(rename = "egm96")]
    Egm96,
    #[serde(rename = "elipsoidal")]
    Ellipsoidal,
    #[default]
    #[serde(rename = "no_declarado")]
    Undeclared,
}

impl VerticalDatum {
    pub fn key(self) -> &'static str {
        match self {
            VerticalDatum::Egm2008 => "egm2008",
            VerticalDatum::Egm96 => "egm96",
            VerticalDatum::Ellipsoidal => "elipsoidal",
            VerticalDatum::Undeclared => "no_declarado",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            VerticalDatum::Egm2008 => "ortométrico EGM2008",
            VerticalDatum::Egm96 => "ortométrico EGM96",
            VerticalDatum::Ellipsoidal => "elipsoidal WGS 84",
            VerticalDatum::Undeclared => "no declarado",
        }
    }
    pub fn is_declared(self) -> bool {
        self != VerticalDatum::Undeclared
    }
}

/// What a survey point is for. The class comes from the translation (D18): the app never
/// reclassifies a point.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PointClass {
    #[serde(rename = "terreno")]
    Ground,
    #[serde(rename = "quiebre")]
    Breakline,
    #[serde(rename = "contorno")]
    Boundary,
    #[serde(rename = "excluido")]
    Excluded,
}

impl PointClass {
    pub fn key(self) -> &'static str {
        match self {
            PointClass::Ground => "terreno",
            PointClass::Breakline => "quiebre",
            PointClass::Boundary => "contorno",
            PointClass::Excluded => "excluido",
        }
    }
    pub fn enters_tin(self) -> bool {
        self != PointClass::Excluded
    }
}

/// A surveyed point as delivered, with its original description.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SurveyPoint {
    #[serde(rename = "numero")]
    pub number: String,
    pub e: f64,
    pub n: f64,
    pub z: f64,
    #[serde(rename = "descripcion")]
    pub description: String,
    #[serde(rename = "clase")]
    pub class: PointClass,
}

/// An ordered chain of point numbers: a breakline, or a closed ring (boundary or hole).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SurveyChain {
    #[serde(rename = "descripcion", default)]
    pub description: String,
    #[serde(rename = "puntos")]
    pub points: Vec<String>,
}

/// How much of an elevation grid entered the TIN.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GridUse {
    #[serde(rename = "fuente")]
    pub source: String,
    #[serde(rename = "celda_m")]
    pub cell_m: f64,
    #[serde(rename = "filas")]
    pub rows: usize,
    #[serde(rename = "columnas")]
    pub cols: usize,
    #[serde(rename = "nodos_usados")]
    pub nodes_used: usize,
}

/// The survey behind a TIN, kept so the numbers, descriptions and classes are not lost.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Survey {
    #[serde(rename = "fuente", default)]
    pub source: String,
    #[serde(rename = "puntos")]
    pub points: Vec<SurveyPoint>,
    #[serde(rename = "quiebres", default)]
    pub breaklines: Vec<SurveyChain>,
    #[serde(rename = "contorno", default)]
    pub boundary: Vec<String>,
    #[serde(rename = "huecos", default)]
    pub holes: Vec<SurveyChain>,
    #[serde(rename = "malla", default, skip_serializing_if = "Option::is_none")]
    pub grid: Option<GridUse>,
}

/// A clipping ring of the TIN in plan. `on_tin`: every vertex is a TIN vertex and every edge a
/// constraint, so the clip is exact; otherwise triangles that overlap the ring are dropped too.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ClipRing {
    pub vertices: Vec<P2>,
    pub on_tin: bool,
}

/// What to triangulate: vertices, constraint edges (indices into `points`) and clipping rings.
#[derive(Clone, Debug, Default)]
pub struct TinInput {
    pub points: Vec<SurfacePoint>,
    pub constraints: Vec<[u32; 2]>,
    pub boundary: Option<ClipRing>,
    pub holes: Vec<ClipRing>,
}

/// Constraint edges that cross another constraint, as indices into `TinInput::points`.
#[derive(Clone, Debug, PartialEq)]
pub struct ConstraintConflict {
    pub edges: Vec<[u32; 2]>,
}

/// Malla triangular irregular. `tris[i]` son índices a `points`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Terrain {
    pub name: String,
    pub epsg: u32,
    /// Sube con cada edición de la superficie; los resultados que dependen de ella lo citan.
    pub version: u32,
    #[serde(rename = "datum_vertical")]
    pub vertical_datum: VerticalDatum,
    pub points: Vec<SurfacePoint>,
    pub tris: Vec<[u32; 3]>,
    pub bbox: Bbox,
    /// Constraint edges between TIN vertices: kept so the display copy (`thinned`) honours them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub constraints: Vec<[u32; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub boundary: Option<ClipRing>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub holes: Vec<ClipRing>,
    #[serde(
        rename = "levantamiento",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub survey: Option<Survey>,
    #[serde(skip)]
    index: TriIndex,
}

#[derive(Clone, Debug, Default)]
struct TriIndex {
    cell: f64,
    cells: HashMap<(i32, i32), Vec<u32>>,
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
    /// Unconstrained Delaunay TIN over the plan of the points, clipped to their convex hull.
    pub fn build(name: &str, epsg: u32, points: Vec<SurfacePoint>) -> Terrain {
        let input = TinInput {
            points,
            ..Default::default()
        };
        Terrain::triangulate(name, epsg, VerticalDatum::Undeclared, input)
            .expect("without constraints there is nothing to conflict")
    }

    /// Constrained Delaunay TIN: no triangle crosses a constraint edge, and the triangles outside
    /// the boundary or inside a hole are removed.
    pub fn triangulate(
        name: &str,
        epsg: u32,
        vertical_datum: VerticalDatum,
        input: TinInput,
    ) -> Result<Terrain, ConstraintConflict> {
        let Triangulated {
            points,
            tris,
            constraints,
        } = constrained_delaunay(input.points, input.constraints)?;
        let tris = clip_triangles(&points, tris, input.boundary.as_ref(), &input.holes);
        let mut bbox = Bbox::default();
        for p in &points {
            bbox.add(p.p2());
        }
        let index = TriIndex::build(&points, &tris, bbox);
        Ok(Terrain {
            name: name.to_string(),
            epsg,
            version: 1,
            vertical_datum,
            points,
            tris,
            bbox,
            constraints,
            boundary: input.boundary,
            holes: input.holes,
            survey: None,
            index,
        })
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

    /// Display copy: one point per `cell` metres plus every constraint vertex, re-triangulated
    /// with the same constraints and clipping. Thinning without them would bridge the ravine and
    /// fill the boundary concavities again.
    pub fn thinned(&self, cell: f64) -> Terrain {
        let cell = cell.max(0.5);
        let pinned: HashSet<u32> = self.constraints.iter().flatten().copied().collect();
        let mut seen = HashSet::new();
        let mut remap: Vec<Option<u32>> = vec![None; self.points.len()];
        let mut pts = Vec::new();
        for (i, p) in self.points.iter().enumerate() {
            let key = ((p.e / cell).floor() as i64, (p.n / cell).floor() as i64);
            let first_in_cell = seen.insert(key);
            if first_in_cell || pinned.contains(&(i as u32)) {
                remap[i] = Some(pts.len() as u32);
                pts.push(*p);
            }
        }
        if pts.len() < 3 {
            return self.clone();
        }
        let constraints = self
            .constraints
            .iter()
            .filter_map(|[a, b]| Some([remap[*a as usize]?, remap[*b as usize]?]))
            .collect();
        let input = TinInput {
            points: pts,
            constraints,
            boundary: self.boundary.clone(),
            holes: self.holes.clone(),
        };
        Terrain::triangulate(&self.name, self.epsg, self.vertical_datum, input)
            .unwrap_or_else(|_| self.clone())
    }
}

struct Triangulated {
    points: Vec<SurfacePoint>,
    tris: Vec<[u32; 3]>,
    constraints: Vec<[u32; 2]>,
}

/// Triangulates with spade's CDT. Returns the vertices actually used (non-finite points and
/// exact duplicates dropped), the non-degenerate triangles and the constraint edges, all with
/// indices into the returned vertices.
fn constrained_delaunay(
    points: Vec<SurfacePoint>,
    constraints: Vec<[u32; 2]>,
) -> Result<Triangulated, ConstraintConflict> {
    // A NaN aborted the old triangulator and spade refuses it: a hostile file must not reach it.
    let mut kept_index: Vec<Option<usize>> = Vec::with_capacity(points.len());
    let mut finite = Vec::with_capacity(points.len());
    for p in points {
        if p.is_finite() {
            kept_index.push(Some(finite.len()));
            finite.push(p);
        } else {
            kept_index.push(None);
        }
    }
    let edges: Vec<[usize; 2]> = constraints
        .iter()
        .filter_map(|[a, b]| Some([kept_index[*a as usize]?, kept_index[*b as usize]?]))
        .filter(|[a, b]| a != b)
        .collect();
    if finite.len() < 3 {
        return Ok(Triangulated {
            points: finite,
            tris: Vec::new(),
            constraints: Vec::new(),
        });
    }
    let mut conflicts = Vec::new();
    let cdt =
        ConstrainedDelaunayTriangulation::<SurfacePoint>::try_bulk_load_cdt(finite, edges, |e| {
            conflicts.push([e[0] as u32, e[1] as u32])
        })
        .map_err(|_| ConstraintConflict { edges: Vec::new() })?;
    if !conflicts.is_empty() {
        return Err(ConstraintConflict { edges: conflicts });
    }
    let points: Vec<SurfacePoint> = cdt.vertices().map(|v| *v.data()).collect();
    let tris: Vec<[u32; 3]> = cdt
        .inner_faces()
        .map(|f| f.vertices().map(|v| v.fix().index() as u32))
        .filter(|tri| tri_area(&points, *tri) > 1e-9)
        .collect();
    let constraints = cdt
        .undirected_edges()
        .filter(|e| e.is_constraint_edge())
        .map(|e| e.vertices().map(|v| v.fix().index() as u32))
        .collect();
    Ok(Triangulated {
        points,
        tris,
        constraints,
    })
}

/// Drops the triangles outside the boundary and inside the holes.
fn clip_triangles(
    points: &[SurfacePoint],
    tris: Vec<[u32; 3]>,
    boundary: Option<&ClipRing>,
    holes: &[ClipRing],
) -> Vec<[u32; 3]> {
    if boundary.is_none() && holes.is_empty() {
        return tris;
    }
    let boundary = boundary.map(RingTest::new);
    let holes: Vec<RingTest> = holes.iter().map(RingTest::new).collect();
    tris.into_iter()
        .filter(|tri| {
            let corners = tri.map(|i| points[i as usize].p2());
            let centroid = P2::new(
                (corners[0].x + corners[1].x + corners[2].x) / 3.0,
                (corners[0].y + corners[1].y + corners[2].y) / 3.0,
            );
            let outside = boundary
                .as_ref()
                .is_some_and(|b| !b.contains(centroid) || b.straddles(corners));
            let in_hole = holes
                .iter()
                .any(|h| h.contains(centroid) || h.straddles(corners));
            !outside && !in_hole
        })
        .collect()
}

/// Point-in-ring and overlap tests for one ring, with the edges bucketed in horizontal bands so
/// a 100k-point survey with a long boundary does not test every edge for every triangle.
pub(crate) struct RingTest<'a> {
    ring: &'a ClipRing,
    bbox: Bbox,
    band_h: f64,
    bands: Vec<Vec<u32>>,
}

impl<'a> RingTest<'a> {
    pub(crate) fn new(ring: &'a ClipRing) -> RingTest<'a> {
        let mut bbox = Bbox::default();
        for p in &ring.vertices {
            bbox.add(*p);
        }
        let n_edges = ring.vertices.len();
        let n_bands = (n_edges / 4).clamp(1, 1024);
        let band_h = (bbox.height() / n_bands as f64).max(1e-9);
        let mut bands = vec![Vec::new(); n_bands];
        for i in 0..n_edges {
            let (a, b) = ring.edge(i);
            let lo = ((a.y.min(b.y) - bbox.ymin) / band_h).floor().max(0.0) as usize;
            let hi = ((a.y.max(b.y) - bbox.ymin) / band_h).floor().max(0.0) as usize;
            for band in bands.iter_mut().take(hi.min(n_bands - 1) + 1).skip(lo) {
                band.push(i as u32);
            }
        }
        RingTest {
            ring,
            bbox,
            band_h,
            bands,
        }
    }

    /// Even-odd rule.
    pub(crate) fn contains(&self, p: P2) -> bool {
        if !self.bbox.contains(p) {
            return false;
        }
        let band =
            (((p.y - self.bbox.ymin) / self.band_h).floor() as usize).min(self.bands.len() - 1);
        let mut inside = false;
        for &i in &self.bands[band] {
            let (a, b) = self.ring.edge(i as usize);
            if (a.y > p.y) != (b.y > p.y) {
                let x = a.x + (p.y - a.y) * (b.x - a.x) / (b.y - a.y);
                if p.x < x {
                    inside = !inside;
                }
            }
        }
        inside
    }

    /// Does the triangle reach across the ring? Only possible when the ring is not made of
    /// constraint edges (some vertex is not a TIN vertex).
    fn straddles(&self, tri: [P2; 3]) -> bool {
        if self.ring.on_tin {
            return false;
        }
        let mut tb = Bbox::default();
        for p in tri {
            tb.add(p);
        }
        if tb.xmax < self.bbox.xmin
            || tb.xmin > self.bbox.xmax
            || tb.ymax < self.bbox.ymin
            || tb.ymin > self.bbox.ymax
        {
            return false;
        }
        let vertex_inside = self
            .ring
            .vertices
            .iter()
            .any(|&q| strictly_inside_triangle(q, tri));
        vertex_inside
            || (0..self.ring.vertices.len()).any(|i| {
                let (a, b) = self.ring.edge(i);
                (0..3).any(|k| segments_cross(a, b, tri[k], tri[(k + 1) % 3]))
            })
    }
}

impl ClipRing {
    fn edge(&self, i: usize) -> (P2, P2) {
        let n = self.vertices.len();
        (self.vertices[i], self.vertices[(i + 1) % n])
    }
}

fn orient(a: P2, b: P2, c: P2) -> f64 {
    (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)
}

/// Proper crossing: the segments cut each other at a point interior to both.
pub(crate) fn segments_cross(a: P2, b: P2, c: P2, d: P2) -> bool {
    let d1 = orient(c, d, a);
    let d2 = orient(c, d, b);
    let d3 = orient(a, b, c);
    let d4 = orient(a, b, d);
    d1 * d2 < 0.0 && d3 * d4 < 0.0
}

fn strictly_inside_triangle(p: P2, t: [P2; 3]) -> bool {
    let s = [
        orient(t[0], t[1], p),
        orient(t[1], t[2], p),
        orient(t[2], t[0], p),
    ];
    s.iter().all(|v| *v > 0.0) || s.iter().all(|v| *v < 0.0)
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

    /// A ravine: banks every 12 m at ±6 m, thalweg 3 m deeper but only every 24 m. Without the
    /// thalweg as a constraint, Delaunay joins bank to bank over it and fills the ravine.
    fn ravine() -> (TinInput, Vec<(P2, P2)>) {
        let mut points = Vec::new();
        for k in 0..=4 {
            let n = k as f64 * 24.0;
            points.push(SurfacePoint::new(50.0, n, 97.0));
        }
        for k in 0..=8 {
            let n = k as f64 * 12.0;
            points.push(SurfacePoint::new(44.0, n, 100.0));
            points.push(SurfacePoint::new(56.0, n, 100.0));
        }
        for n in [0.0, 48.0, 96.0] {
            points.push(SurfacePoint::new(20.0, n, 101.0));
            points.push(SurfacePoint::new(80.0, n, 101.0));
        }
        let constraints: Vec<[u32; 2]> = (0..4).map(|k| [k, k + 1]).collect();
        let thalweg = constraints
            .iter()
            .map(|[a, b]| (points[*a as usize].p2(), points[*b as usize].p2()))
            .collect();
        let input = TinInput {
            points,
            constraints,
            ..Default::default()
        };
        (input, thalweg)
    }

    fn crossings(t: &Terrain, lines: &[(P2, P2)]) -> usize {
        t.tris
            .iter()
            .flat_map(|tri| (0..3).map(move |k| (tri[k], tri[(k + 1) % 3])))
            .filter(|(a, b)| {
                let (pa, pb) = (t.points[*a as usize].p2(), t.points[*b as usize].p2());
                lines.iter().any(|(c, d)| segments_cross(pa, pb, *c, *d))
            })
            .count()
    }

    #[test]
    fn no_triangle_crosses_the_ravine_breakline() {
        let (input, thalweg) = ravine();
        let unconstrained = Terrain::build("sin", 32717, input.points.clone());
        assert!(
            crossings(&unconstrained, &thalweg) > 0,
            "the fixture must be one that plain Delaunay gets wrong"
        );
        let t = Terrain::triangulate("quebrada", 32717, VerticalDatum::Egm2008, input).unwrap();
        assert_eq!(crossings(&t, &thalweg), 0);
        // Midway between two thalweg shots the height is the thalweg's, not the banks'.
        let z = t.z_at(P2::new(50.0, 12.0)).unwrap();
        assert!((z - 97.0).abs() < 1e-9, "z = {z}");
    }

    #[test]
    fn the_display_copy_keeps_the_ravine() {
        let (input, thalweg) = ravine();
        let t = Terrain::triangulate("quebrada", 32717, VerticalDatum::Egm2008, input).unwrap();
        let thin = t.thinned(30.0);
        assert!(thin.points.len() < t.points.len(), "it does thin");
        assert_eq!(crossings(&thin, &thalweg), 0);
    }

    #[test]
    fn crossing_constraints_are_reported_not_triangulated() {
        let points = vec![
            SurfacePoint::new(0.0, 0.0, 1.0),
            SurfacePoint::new(10.0, 10.0, 1.0),
            SurfacePoint::new(0.0, 10.0, 1.0),
            SurfacePoint::new(10.0, 0.0, 1.0),
        ];
        let input = TinInput {
            points,
            constraints: vec![[0, 1], [2, 3]],
            ..Default::default()
        };
        let err = Terrain::triangulate("x", 32717, VerticalDatum::Undeclared, input).unwrap_err();
        assert_eq!(err.edges.len(), 1);
    }

    /// An L-shaped boundary over a full square of points: the missing quadrant has no triangle.
    fn square_grid() -> Vec<SurfacePoint> {
        let mut pts = Vec::new();
        for j in 0..=10 {
            for i in 0..=10 {
                let (x, y) = (i as f64 * 10.0, j as f64 * 10.0);
                pts.push(SurfacePoint::new(x, y, 100.0 + x / 10.0));
            }
        }
        pts
    }

    fn grid_index(i: u32, j: u32) -> u32 {
        j * 11 + i
    }

    #[test]
    fn boundary_concavities_get_no_triangles() {
        let points = square_grid();
        let ring_idx = [
            grid_index(0, 0),
            grid_index(10, 0),
            grid_index(10, 5),
            grid_index(5, 5),
            grid_index(5, 10),
            grid_index(0, 10),
        ];
        let corners: Vec<u32> = ring_idx.to_vec();
        // Every grid point along the ring is a vertex on it: the constraint edges pass through them.
        let constraints = (0..corners.len())
            .map(|k| [corners[k], corners[(k + 1) % corners.len()]])
            .collect();
        let boundary = ClipRing {
            vertices: corners.iter().map(|&i| points[i as usize].p2()).collect(),
            on_tin: true,
        };
        let notch = P2::new(80.0, 80.0);
        let hull = Terrain::build("casco", 32717, points.clone());
        assert!(hull.z_at(notch).is_some(), "the hull covers the notch");
        let input = TinInput {
            points,
            constraints,
            boundary: Some(boundary),
            holes: Vec::new(),
        };
        let t = Terrain::triangulate("ele", 32717, VerticalDatum::Undeclared, input).unwrap();
        assert!(t.z_at(notch).is_none());
        assert!(t.z_at(P2::new(52.0, 80.0)).is_none());
        assert!((t.z_at(P2::new(25.0, 80.0)).unwrap() - 102.5).abs() < 1e-9);
        let in_notch = t.tris.iter().any(|tri| {
            let c = tri
                .iter()
                .map(|&i| t.points[i as usize].p2())
                .fold(P2::new(0.0, 0.0), |s, p| {
                    P2::new(s.x + p.x / 3.0, s.y + p.y / 3.0)
                });
            c.x > 50.0 && c.y > 50.0
        });
        assert!(!in_notch);
        // Three quarters of the 200 grid triangles stay.
        assert_eq!(t.tris.len(), 150);
    }

    #[test]
    fn a_hole_whose_corners_are_not_tin_vertices_still_has_no_height_inside() {
        let hole = ClipRing {
            vertices: vec![
                P2::new(32.0, 32.0),
                P2::new(58.0, 32.0),
                P2::new(58.0, 58.0),
                P2::new(32.0, 58.0),
            ],
            on_tin: false,
        };
        let input = TinInput {
            points: square_grid(),
            holes: vec![hole],
            ..Default::default()
        };
        let t = Terrain::triangulate("hueco", 32717, VerticalDatum::Undeclared, input).unwrap();
        for p in [
            P2::new(33.0, 33.0),
            P2::new(45.0, 45.0),
            P2::new(57.5, 40.0),
        ] {
            assert!(t.z_at(p).is_none(), "{p:?} is inside the hole");
        }
        assert!(t.z_at(P2::new(15.0, 15.0)).is_some());
    }

    #[test]
    fn vertical_datum_keys_round_trip() {
        for d in [
            VerticalDatum::Egm2008,
            VerticalDatum::Egm96,
            VerticalDatum::Ellipsoidal,
            VerticalDatum::Undeclared,
        ] {
            let json = serde_json::to_string(&d).unwrap();
            assert_eq!(json, format!("\"{}\"", d.key()));
            assert_eq!(serde_json::from_str::<VerticalDatum>(&json).unwrap(), d);
        }
    }
}
