//! Baked pipe segments laid along their links (F17-04, docs/familias/F17-04-tuberia.md).
//!
//! A link is never baked whole: one commercial segment (bell, spigot and joint) is baked per DN,
//! wall, material and system, and repeated here along the drawn axis of every link that uses it.
//! Every segment but the last is the whole piece; the last one is cut where the link ends, drawn
//! with the barrel alone stretched to its length (the barrel is prismatic along the axis, so the
//! stretch is exact). Nothing here changes the design path: only where the drawing starts and ends.
//!
//! The drawn axis stops at the wall of the nodes it joins: at the anchor a baked piece declares
//! for this link (`entrada__<code>`/`salida__<code>`, docs/17 section F17-03), else at the edge of
//! the fallback piece.

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use crate::geom::P3;
use crate::gltf::Vertice;
use crate::model::{ElementId, Link, Project};

/// The part of a pipe piece the viewer may stretch along the axis.
pub const BARREL_PART: &str = "cuerpo";
/// Empties at both ends of the segment axis: their distance is the laying length.
pub const SEGMENT_START_ANCHOR: &str = "entrada";
pub const SEGMENT_END_ANCHOR: &str = "salida";

/// Gap the parametric pieces leave around a node for its pipes (their wall).
const FALLBACK_WALL_GAP_M: f64 = 0.15;
/// A trimmed end never takes more than this share of the first or last run of the path: a pipe
/// between two very close nodes still shows a stub.
const MAX_TRIM_SHARE: f64 = 0.45;
/// Colour of a triangle whose piece declares no material.
const UNPAINTED: [f32; 3] = [0.62, 0.60, 0.56];
/// Shorter remainders are not drawn: they would be a sliver inside the node wall.
const MIN_PIECE_M: f64 = 1e-3;

/// A baked pipe segment, ready to be laid along links.
#[derive(Clone)]
pub struct PipeSegment {
    /// The whole segment: barrel, bell and joint.
    pub full: Arc<Vec<Vertice>>,
    /// The barrel alone, from the spigot end (x = 0) to where the bell starts.
    pub barrel: Arc<Vec<Vertice>>,
    /// Distance between the spigot ends of two laid segments.
    pub laying_length_m: f64,
    /// Where the barrel ends along the axis.
    pub barrel_end_m: f64,
    /// Largest distance from the axis to the mesh (the bell), for culling.
    pub radius_m: f64,
    /// Extent of the whole segment along the axis (the bell reaches past the laying length).
    pub span_m: (f64, f64),
}

impl PipeSegment {
    /// Reads a segment baked by `tools/blender/build_pipe.py`.
    pub fn from_glb(bytes: &[u8]) -> Result<PipeSegment, String> {
        let parts = crate::biblioteca::triangles_by_part(bytes)?;
        let anchors = crate::biblioteca::anclas_de_glb(bytes)?;
        let (Some(start), Some(end)) = (
            anchors.get(SEGMENT_START_ANCHOR),
            anchors.get(SEGMENT_END_ANCHOR),
        ) else {
            return Err(format!(
                "E_TUBO_SIN_ANCLAS: el tramo de tubo no trae las anclas {SEGMENT_START_ANCHOR} y {SEGMENT_END_ANCHOR}"
            ));
        };
        let laying_length_m = (end[0] - start[0]) as f64;
        let barrel = parts.get(BARREL_PART).ok_or_else(|| {
            format!("E_TUBO_SIN_CUERPO: el tramo de tubo no trae la parte {BARREL_PART}")
        })?;
        let barrel_end_m = barrel
            .iter()
            .flat_map(|t| t.pos)
            .map(|q| q[0])
            .fold(f64::NEG_INFINITY, f64::max);
        if !(laying_length_m > 0.0 && barrel_end_m > 0.0) {
            return Err("E_TUBO_SIN_LARGO: el tramo de tubo no tiene largo a lo largo de X".into());
        }
        let all: Vec<_> = parts.values().flatten().collect();
        let mut span = (f64::INFINITY, f64::NEG_INFINITY);
        let mut radius = 0.0f64;
        for q in all.iter().flat_map(|t| t.pos) {
            span = (span.0.min(q[0]), span.1.max(q[0]));
            radius = radius.max(q[1].hypot(q[2]));
        }
        Ok(PipeSegment {
            full: Arc::new(vertices(all.into_iter())),
            barrel: Arc::new(vertices(barrel.iter())),
            laying_length_m,
            barrel_end_m,
            radius_m: radius,
            span_m: span,
        })
    }
}

/// Flat-shaded vertices of glTF triangles (one normal per face).
fn vertices<'a>(tris: impl Iterator<Item = &'a crate::biblioteca::TrianguloLeido>) -> Vec<Vertice> {
    let mut out = Vec::new();
    for t in tris {
        let pos = t.pos.map(|q| [q[0] as f32, q[1] as f32, q[2] as f32]);
        let normal = face_normal(pos);
        for q in pos {
            out.push(Vertice {
                pos: q,
                normal,
                color: t.color.unwrap_or(UNPAINTED),
            });
        }
    }
    out
}

fn face_normal(p: [[f32; 3]; 3]) -> [f32; 3] {
    let u = [0, 1, 2].map(|k| p[1][k] - p[0][k]);
    let v = [0, 1, 2].map(|k| p[2][k] - p[0][k]);
    let n = [
        u[1] * v[2] - u[2] * v[1],
        u[2] * v[0] - u[0] * v[2],
        u[0] * v[1] - u[1] * v[0],
    ];
    let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    if len > 0.0 {
        n.map(|c| c / len)
    } else {
        [0.0, 0.0, 1.0]
    }
}

/// Anchors of the nodes drawn by a library piece, by node.
pub type NodeAnchors = HashMap<ElementId, BTreeMap<String, [f32; 3]>>;

/// What the viewer needs to draw baked pipes: the segment of each link that has one and the
/// anchors of the nodes, where the pipes start and end.
#[derive(Clone, Default)]
pub struct PipePieces {
    pub by_link: HashMap<ElementId, Arc<PipeSegment>>,
    pub node_anchors: NodeAnchors,
}

/// How a laid piece is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PieceKind {
    /// The whole segment, with its bell.
    Full,
    /// The barrel alone, stretched: the cut segment at the end of a link, or a run of segments
    /// drawn plain from afar.
    Barrel,
}

/// One segment laid on a link, in local coordinates (origin subtracted, real elevations).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LaidPiece {
    pub start: [f64; 3],
    /// Unit vector along the axis.
    pub direction: [f64; 3],
    pub length_m: f64,
    /// Stretch along the axis: 1 for a whole segment on a straight run.
    pub scale_x: f64,
    pub kind: PieceKind,
}

impl LaidPiece {
    pub fn end(&self) -> [f64; 3] {
        [0, 1, 2].map(|k| self.start[k] + self.direction[k] * self.length_m)
    }

    /// Cosine and sine of the turn about Z that takes +X to the axis in plan.
    pub fn yaw(&self) -> [f64; 2] {
        let [dx, dy, _] = self.direction;
        let h = dx.hypot(dy);
        if h < 1e-12 {
            [1.0, 0.0]
        } else {
            [dx / h, dy / h]
        }
    }

    /// Cosine and sine of the tilt that lifts +X to the axis (positive uphill).
    pub fn pitch(&self) -> [f64; 2] {
        let [dx, dy, dz] = self.direction;
        [dx.hypot(dy), dz]
    }

    /// A vertex of the segment mesh in local coordinates: stretched along X, tilted, then
    /// turned, in `f64` (docs/15).
    pub fn place(&self, v: [f32; 3]) -> [f64; 3] {
        let (x, y, z) = (v[0] as f64 * self.scale_x, v[1] as f64, v[2] as f64);
        let [cp, sp] = self.pitch();
        let [cy, sy] = self.yaw();
        let (x1, z1) = (x * cp - z * sp, x * sp + z * cp);
        [
            self.start[0] + x1 * cy - y * sy,
            self.start[1] + x1 * sy + y * cy,
            self.start[2] + z1,
        ]
    }

    /// The same span drawn as the barrel alone.
    pub fn as_barrel(&self, seg: &PipeSegment) -> LaidPiece {
        LaidPiece {
            scale_x: self.length_m / seg.barrel_end_m,
            kind: PieceKind::Barrel,
            ..*self
        }
    }
}

/// Segments along an axis (local coordinates): whole ones every laying length and the last one
/// cut at the end. A segment over a bend of the path follows its chord.
pub fn lay_segments(axis: &[[f64; 3]], seg: &PipeSegment) -> Vec<LaidPiece> {
    let walk = Walk::new(axis);
    let total = walk.length();
    let step = seg.laying_length_m;
    if total < MIN_PIECE_M || step <= 0.0 {
        return Vec::new();
    }
    let count = ((total / step) - 1e-9).ceil().max(1.0) as usize;
    let mut out = Vec::with_capacity(count);
    for k in 0..count {
        let s0 = k as f64 * step;
        let s1 = (s0 + step).min(total);
        let (a, b) = (walk.at(s0), walk.at(s1));
        let chord = [0, 1, 2].map(|i| b[i] - a[i]);
        let length_m = (chord[0] * chord[0] + chord[1] * chord[1] + chord[2] * chord[2]).sqrt();
        if length_m < MIN_PIECE_M {
            continue;
        }
        let piece = LaidPiece {
            start: a,
            direction: chord.map(|c| c / length_m),
            length_m,
            scale_x: length_m / step,
            kind: PieceKind::Full,
        };
        // The link ends at a node wall with a plain cut end, never with a bell.
        out.push(if k + 1 == count {
            piece.as_barrel(seg)
        } else {
            piece
        });
    }
    out
}

/// Consecutive pieces on one straight run, drawn as one stretched barrel each: the plain pipe a
/// distant link needs.
pub fn plain_runs(pieces: &[LaidPiece], seg: &PipeSegment) -> Vec<LaidPiece> {
    let mut out: Vec<LaidPiece> = Vec::new();
    for piece in pieces {
        if let Some(run) = out.last_mut() {
            let along = (0..3)
                .map(|k| run.direction[k] * piece.direction[k])
                .sum::<f64>();
            let joined = (0..3).all(|k| (run.end()[k] - piece.start[k]).abs() < 1e-6);
            if joined && along > 1.0 - 1e-9 {
                run.length_m += piece.length_m;
                *run = run.as_barrel(seg);
                continue;
            }
        }
        out.push(piece.as_barrel(seg));
    }
    out
}

/// Arc-length walk along a polyline.
struct Walk<'a> {
    points: &'a [[f64; 3]],
    cumulative: Vec<f64>,
}

impl<'a> Walk<'a> {
    fn new(points: &'a [[f64; 3]]) -> Walk<'a> {
        let mut cumulative = vec![0.0];
        for w in points.windows(2) {
            let d = (0..3)
                .map(|k| (w[1][k] - w[0][k]).powi(2))
                .sum::<f64>()
                .sqrt();
            cumulative.push(cumulative.last().copied().unwrap_or(0.0) + d);
        }
        Walk { points, cumulative }
    }

    fn length(&self) -> f64 {
        if self.points.len() < 2 {
            0.0
        } else {
            *self.cumulative.last().unwrap_or(&0.0)
        }
    }

    fn at(&self, s: f64) -> [f64; 3] {
        let i = self
            .cumulative
            .partition_point(|c| *c <= s)
            .clamp(1, self.points.len() - 1);
        let (s0, s1) = (self.cumulative[i - 1], self.cumulative[i]);
        let t = if s1 > s0 { (s - s0) / (s1 - s0) } else { 0.0 };
        let (a, b) = (self.points[i - 1], self.points[i]);
        [0, 1, 2].map(|k| a[k] + (b[k] - a[k]) * t.clamp(0.0, 1.0))
    }
}

/// Plan distance from the node to where the link is drawn from: its anchor on a baked piece
/// (the outer face of the wall), else the edge of the fallback piece.
pub fn wall_offset(p: &Project, link: &Link, node: ElementId, anchors: &NodeAnchors) -> f64 {
    let anchored = anchors.get(&node).and_then(|by_name| {
        by_name.iter().find_map(|(name, a)| {
            name.rsplit_once("__")
                .is_some_and(|(_, code)| code == link.code)
                .then(|| (a[0] as f64).hypot(a[1] as f64))
        })
    });
    anchored.unwrap_or_else(|| {
        p.node(node)
            .map(|n| n.drawing_diameter_m() * 0.5 + FALLBACK_WALL_GAP_M)
            .unwrap_or(0.4)
    })
}

/// The axis of a link as drawn (UTM, real elevations): the path raised half the bore and trimmed
/// at the wall of both nodes. Written into `out` so a caller can reuse one buffer for every link.
pub fn drawn_axis(p: &Project, link: &Link, anchors: &NodeAnchors, out: &mut Vec<P3>) {
    let half_bore = link.diametro_interior_m() * 0.5;
    out.clear();
    out.extend(link.path.iter().map(|q| P3::new(q.x, q.y, q.z + half_bore)));
    trim_ends(
        out,
        wall_offset(p, link, link.from, anchors),
        wall_offset(p, link, link.to, anchors),
    );
}

/// [`drawn_axis`] in local coordinates: the origin subtracted in `f64` (docs/15).
pub fn drawn_axis_local(p: &Project, link: &Link, anchors: &NodeAnchors) -> Vec<[f64; 3]> {
    let mut axis = Vec::with_capacity(link.path.len());
    drawn_axis(p, link, anchors, &mut axis);
    axis.iter()
        .map(|q| [q.x - p.origin.x, q.y - p.origin.y, q.z])
        .collect()
}

/// Moves the first and last vertex of a polyline inwards by a plan distance, so the pipe is born
/// and dies at the node wall instead of crossing the node.
pub fn trim_ends(pts: &mut [P3], r0: f64, r1: f64) {
    if pts.len() < 2 {
        return;
    }
    let n = pts.len();
    let towards = |from: P3, to: P3, r: f64| {
        let d = from.p2().dist(to.p2()).max(1e-6);
        let t = (r / d).min(MAX_TRIM_SHARE);
        P3::new(
            from.x + (to.x - from.x) * t,
            from.y + (to.y - from.y) * t,
            from.z + (to.z - from.z) * t,
        )
    };
    pts[0] = towards(pts[0], pts[1], r0);
    pts[n - 1] = towards(pts[n - 1], pts[n - 2], r1);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn segment(laying: f64, barrel_end: f64) -> PipeSegment {
        PipeSegment {
            full: Arc::new(Vec::new()),
            barrel: Arc::new(Vec::new()),
            laying_length_m: laying,
            barrel_end_m: barrel_end,
            radius_m: 0.12,
            span_m: (0.0, laying + 0.1),
        }
    }

    #[test]
    fn whole_segments_then_one_cut_barrel() {
        let seg = segment(6.0, 5.8);
        let pieces = lay_segments(&[[0.0, 0.0, 0.0], [20.0, 0.0, 0.0]], &seg);
        assert_eq!(pieces.len(), 4, "6 + 6 + 6 + 2");
        assert!(pieces[..3].iter().all(|p| p.kind == PieceKind::Full));
        assert!(pieces[..3].iter().all(|p| (p.scale_x - 1.0).abs() < 1e-12));
        let last = pieces[3];
        assert_eq!(last.kind, PieceKind::Barrel);
        assert!((last.length_m - 2.0).abs() < 1e-9);
        assert!(
            (last.scale_x - 2.0 / 5.8).abs() < 1e-12,
            "the barrel reaches the end"
        );
        assert!((last.end()[0] - 20.0).abs() < 1e-9);
        assert!((pieces[1].start[0] - 6.0).abs() < 1e-12);
    }

    #[test]
    fn an_exact_multiple_still_ends_plain() {
        let seg = segment(6.0, 5.8);
        let pieces = lay_segments(&[[0.0, 0.0, 0.0], [12.0, 0.0, 0.0]], &seg);
        assert_eq!(pieces.len(), 2);
        assert_eq!(pieces[0].kind, PieceKind::Full);
        assert_eq!(
            pieces[1].kind,
            PieceKind::Barrel,
            "no bell at the node wall"
        );
        let short = lay_segments(&[[0.0, 0.0, 0.0], [1.0, 0.0, 0.0]], &seg);
        assert_eq!(short.len(), 1);
        assert_eq!(short[0].kind, PieceKind::Barrel);
    }

    #[test]
    fn a_sloped_piece_is_tilted_and_turned_onto_its_axis() {
        let seg = segment(6.0, 5.8);
        let axis = [[10.0, 20.0, 2500.0], [10.0, 40.0, 2499.0]];
        let pieces = lay_segments(&axis, &seg);
        let p = pieces[0];
        let end = p.place([6.0, 0.0, 0.0]);
        let want = [
            10.0,
            20.0 + 6.0 * p.pitch()[0],
            2500.0 - 6.0 * (1.0 / 20.0f64.hypot(1.0)),
        ];
        for k in 0..3 {
            assert!((end[k] - want[k]).abs() < 1e-9, "{end:?} vs {want:?}");
        }
        // The section stays square to the axis: +Z of the piece leans back as the pipe goes down.
        let up = p.place([0.0, 0.0, 1.0]);
        let along = (0..3)
            .map(|k| (up[k] - p.start[k]) * p.direction[k])
            .sum::<f64>();
        assert!(along.abs() < 1e-12);
        // +Y of the piece is horizontal, to the left of the axis (west for a pipe going north).
        let left = p.place([0.0, 1.0, 0.0]);
        assert!((left[0] - 9.0).abs() < 1e-12 && (left[2] - 2500.0).abs() < 1e-12);
    }

    #[test]
    fn straight_runs_merge_and_a_bend_splits_them() {
        let seg = segment(6.0, 5.8);
        let axis = [[0.0, 0.0, 0.0], [12.0, 0.0, 0.0], [12.0, 12.0, 0.0]];
        let pieces = lay_segments(&axis, &seg);
        assert_eq!(pieces.len(), 4);
        let runs = plain_runs(&pieces, &seg);
        assert_eq!(runs.len(), 2, "{runs:?}");
        assert!((runs[0].length_m - 12.0).abs() < 1e-9);
        assert!(runs.iter().all(|r| r.kind == PieceKind::Barrel));
        assert!((runs[0].scale_x - 12.0 / 5.8).abs() < 1e-12);
    }

    #[test]
    fn the_axis_stops_at_the_anchor_of_a_baked_node() {
        let p = crate::demo_cadena::cadena();
        let l = p.link_by_code("CL-303").expect("sewer pipe");
        let mut anchors = NodeAnchors::new();
        let mut plain = Vec::new();
        drawn_axis(&p, l, &anchors, &mut plain);
        anchors.insert(
            l.from,
            BTreeMap::from([(format!("salida__{}", l.code), [0.9f32, 0.0, 0.25])]),
        );
        let mut trimmed = Vec::new();
        drawn_axis(&p, l, &anchors, &mut trimmed);
        // Trimmed from the path end, which the source puts at the node.
        let from = l.path[0].p2();
        // UTM magnitudes leave ~1e-10 m of rounding.
        assert!((trimmed[0].p2().dist(from) - 0.9).abs() < 1e-6);
        assert!(
            (plain[0].p2().dist(from) - wall_offset(&p, l, l.from, &NodeAnchors::new())).abs()
                < 1e-6
        );
        assert_eq!(trimmed.last(), plain.last(), "the other end keeps its rule");
        let share = 0.9 / l.path[0].p2().dist(l.path[1].p2());
        let on_axis =
            l.path[0].z + (l.path[1].z - l.path[0].z) * share + l.diametro_interior_m() / 2.0;
        assert!(
            (on_axis - trimmed[0].z).abs() < 1e-6,
            "on the axis, not on the invert"
        );
    }

    #[test]
    fn another_links_anchor_does_not_trim() {
        let p = crate::demo_cadena::cadena();
        let l = p.link_by_code("CL-303").unwrap();
        let anchors = NodeAnchors::from([(
            l.from,
            BTreeMap::from([("salida__CL-999".to_string(), [3.0f32, 0.0, 0.0])]),
        )]);
        assert_eq!(
            wall_offset(&p, l, l.from, &anchors),
            wall_offset(&p, l, l.from, &NodeAnchors::new())
        );
    }
}
