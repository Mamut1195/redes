//! `redes.topografia/1`: the canonical survey contract (D18, F17-11).
//!
//! An agent following `skills/redes-ingesta/references/topografia.md` translates the field survey
//! (PNEZD text, DXF, LandXML) and classifies every point; this module only validates the file and
//! triangulates it. Nothing is reclassified, renumbered or filled in: a file that does not add up
//! is rejected with the reason.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::geom::P2;
use crate::ingest::{check_crs, check_vertical_datum, reject_lonlat};
use crate::model::{Fuente, Project};
use crate::terrain::{
    ClipRing, GridUse, PointClass, RingTest, SurfacePoint, Survey, SurveyChain, SurveyPoint,
    Terrain, TinInput, VerticalDatum, segments_cross,
};

pub const SURVEY_SCHEMA: &str = "redes.topografia/1";

/// Two TIN points closer than this in plan are the same place.
const SAME_PLACE_M: f64 = 0.001;
/// Two heights closer than this are the same height.
const SAME_HEIGHT_M: f64 = 0.001;
/// Grid nodes accepted at most: beyond this the TIN would pass the project's triangle cap.
const GRID_NODES_MAX: usize = 1_500_000;
/// Examples listed in an error message; the count says how many there are in all.
const EXAMPLES: usize = 8;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SurveyFile {
    pub schema: String,
    #[serde(rename = "nombre")]
    pub name: String,
    pub crs: String,
    #[serde(rename = "datum_vertical")]
    pub vertical_datum: VerticalDatum,
    #[serde(rename = "fuente", default)]
    pub source: String,
    #[serde(rename = "puntos", default)]
    pub points: Vec<SurveyPoint>,
    #[serde(rename = "quiebres", default)]
    pub breaklines: Vec<SurveyChain>,
    #[serde(rename = "contorno", default)]
    pub boundary: Vec<String>,
    #[serde(rename = "huecos", default)]
    pub holes: Vec<SurveyChain>,
    #[serde(rename = "malla", default)]
    pub grid: Option<ElevationGrid>,
}

/// A regular elevation grid (a DEM such as Copernicus GLO-30), already projected to the CRS.
/// `values` go row by row from the north-west cell: rows advance south, columns east.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ElevationGrid {
    #[serde(rename = "e_centro_noroeste")]
    pub northwest_center_e: f64,
    #[serde(rename = "n_centro_noroeste")]
    pub northwest_center_n: f64,
    #[serde(rename = "celda_m")]
    pub cell_m: f64,
    #[serde(rename = "filas")]
    pub rows: usize,
    #[serde(rename = "columnas")]
    pub cols: usize,
    #[serde(rename = "sin_dato", default)]
    pub nodata: Option<f64>,
    #[serde(rename = "valores")]
    pub values: Vec<f64>,
    #[serde(rename = "fuente", default)]
    pub source: String,
}

/// What the import read and built, for the CLI envelope.
#[derive(Clone, Debug, Default, Serialize)]
pub struct SurveyReport {
    pub nombre: String,
    pub crs: String,
    pub datum_vertical: &'static str,
    pub puntos: usize,
    pub por_clase: HashMap<&'static str, usize>,
    pub quiebres: usize,
    pub contorno: bool,
    pub huecos: usize,
    pub nodos_malla: usize,
    pub vertices_tin: usize,
    pub triangulos: usize,
    pub avisos: Vec<String>,
}

/// Reads, validates and triangulates a `redes.topografia/1` file into the project's terrain.
pub fn import_survey(p: &mut Project, path: &Path) -> Result<SurveyReport, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("E_IO_NOT_FOUND: {e}"))?;
    let file = parse(&text)?;
    let epsg = check_crs(p, &file.crs, "la topografía")?;
    let grid_origin = file
        .grid
        .as_ref()
        .map(|g| ("malla", g.northwest_center_e, g.northwest_center_n));
    reject_lonlat(
        file.points
            .iter()
            .map(|q| (q.number.as_str(), q.e, q.n))
            .chain(grid_origin),
        "la topografía",
    )?;
    let mut avisos = Vec::new();
    avisos.extend(check_vertical_datum(
        p.vertical_datum,
        None,
        file.vertical_datum,
        "la topografía",
    )?);
    let plan = plan_tin(&file, &mut avisos)?;
    if crate::ingest::epsg_de(&p.crs) != Some(epsg) {
        avisos.push(format!(
            "el proyecto estaba vacío y adopta el CRS de la topografía: EPSG:{epsg} (antes {})",
            p.crs
        ));
    }

    let name = if file.name.is_empty() {
        path.file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "topografia".into())
    } else {
        file.name.clone()
    };
    let mut terrain = Terrain::triangulate(&name, epsg, file.vertical_datum, plan.input.clone())
        .map_err(|c| crossing_error(&c.edges, &plan))?;
    if terrain.tris.is_empty() {
        return Err(
            "E_TOPO_VACIA: no queda ningún triángulo: los puntos son colineales o el contorno no encierra ninguno"
                .into(),
        );
    }
    terrain.survey = Some(Survey {
        source: file.source.clone(),
        points: file.points.clone(),
        breaklines: file.breaklines.clone(),
        boundary: file.boundary.clone(),
        holes: file.holes.clone(),
        grid: plan.grid_use.clone(),
    });

    let mut por_clase = HashMap::new();
    for q in &file.points {
        *por_clase.entry(q.class.key()).or_insert(0) += 1;
    }
    let report = SurveyReport {
        nombre: name,
        crs: format!("EPSG:{epsg}"),
        datum_vertical: file.vertical_datum.key(),
        puntos: file.points.len(),
        por_clase,
        quiebres: file.breaklines.len(),
        contorno: !file.boundary.is_empty(),
        huecos: file.holes.len(),
        nodos_malla: plan.grid_use.as_ref().map_or(0, |g| g.nodes_used),
        vertices_tin: terrain.points.len(),
        triangulos: terrain.tris.len(),
        avisos,
    };
    // `check_crs` let it through: same CRS as the project, or an empty project that adopts it.
    p.crs = format!("EPSG:{epsg}");
    p.fuentes.push(Fuente {
        app: if file.source.is_empty() {
            "topografía".into()
        } else {
            file.source.clone()
        },
        archivo: path.display().to_string(),
        formato: "topografia".into(),
        importado_en: crate::ingest::today(),
    });
    p.terrain = Some(terrain);
    p.commit("importar topografia");
    Ok(report)
}

/// Schema first (another version must not be read as this one), then the strict structure.
fn parse(text: &str) -> Result<SurveyFile, String> {
    let raw: serde_json::Value =
        serde_json::from_str(text).map_err(|e| format!("E_FMT_PARSE: {e}"))?;
    match raw.get("schema").and_then(serde_json::Value::as_str) {
        Some(SURVEY_SCHEMA) => {}
        Some(s) if s.starts_with("redes.topografia/") => {
            return Err(format!(
                "E_FMT_VERSION: la topografía es `{s}` y esta app solo importa `{SURVEY_SCHEMA}`"
            ));
        }
        Some(s) => {
            return Err(format!(
                "E_FMT_UNSUPPORTED: `{s}` no es una topografía; se espera `\"schema\": \"{SURVEY_SCHEMA}\"`"
            ));
        }
        None => {
            return Err(format!(
                "E_FMT_UNSUPPORTED: falta `\"schema\": \"{SURVEY_SCHEMA}\"`"
            ));
        }
    }
    serde_json::from_value(raw).map_err(|e| {
        let msg = e.to_string();
        if msg.contains("unknown field") {
            format!("E_FMT_CAMPO: campo que no es del contrato {SURVEY_SCHEMA}: {msg}")
        } else {
            format!("E_FMT_PARSE: {msg}")
        }
    })
}

/// The validated triangulation input, with what is needed to name a conflict.
struct TinPlan {
    input: TinInput,
    /// For each input vertex, the survey point number (`None` for grid nodes).
    vertex_number: Vec<Option<String>>,
    /// For each constraint edge, the chain it comes from.
    edge_origin: HashMap<[u32; 2], String>,
    grid_use: Option<GridUse>,
}

fn plan_tin(file: &SurveyFile, avisos: &mut Vec<String>) -> Result<TinPlan, String> {
    let by_number = index_numbers(&file.points)?;
    let TinVertices {
        of_point: vertex_of_point,
        mut points,
        numbers: vertex_number,
    } = tin_vertices(&file.points, avisos)?;

    let mut constraints: Vec<[u32; 2]> = Vec::new();
    let mut edge_origin = HashMap::new();
    let mut in_breakline: HashSet<usize> = HashSet::new();
    for (k, chain) in file.breaklines.iter().enumerate() {
        let who = chain_name("quiebre", k, chain);
        let idx = resolve_chain(&who, chain, &by_number, &file.points, BREAKLINE_CLASSES)?;
        if idx.len() < 2 {
            return Err(format!(
                "E_TOPO_GEOMETRIA: {who} tiene {} punto(s); una línea de quiebre necesita al menos 2",
                idx.len()
            ));
        }
        for pair in idx.windows(2) {
            let (a, b) = (vertex_of_point[pair[0]], vertex_of_point[pair[1]]);
            let (Some(a), Some(b)) = (a, b) else {
                unreachable!("breakline classes enter the TIN")
            };
            if a == b {
                return Err(format!(
                    "E_TOPO_GEOMETRIA: {who} repite la planta entre los puntos {} y {}",
                    file.points[pair[0]].number, file.points[pair[1]].number
                ));
            }
            constraints.push([a, b]);
            edge_origin.insert(edge_key(a, b), who.clone());
        }
        in_breakline.extend(idx);
    }

    let mut in_ring: HashSet<usize> = HashSet::new();
    let boundary = if file.boundary.is_empty() {
        avisos.push(
            "sin contorno: el TIN cubre el casco convexo de los puntos; en las concavidades no hay medida y la cota es una interpolación larga"
                .into(),
        );
        None
    } else {
        let chain = SurveyChain {
            description: "contorno".into(),
            points: file.boundary.clone(),
        };
        let ring = resolve_ring("el contorno", &chain, &by_number, &file.points)?;
        in_ring.extend(ring.iter().copied());
        Some(clip_ring(
            "el contorno",
            &ring,
            &file.points,
            &vertex_of_point,
            &mut constraints,
            &mut edge_origin,
        ))
    };
    let mut holes = Vec::new();
    let boundary_test = boundary.as_ref().map(RingTest::new);
    for (k, chain) in file.holes.iter().enumerate() {
        let who = chain_name("hueco", k, chain);
        let ring = resolve_ring(&who, chain, &by_number, &file.points)?;
        if let Some(b) = &boundary_test
            && let Some(&out) = ring
                .iter()
                .find(|&&i| !b.contains(plan_of(&file.points[i])))
        {
            return Err(format!(
                "E_TOPO_GEOMETRIA: {who}: el punto {} queda fuera del contorno",
                file.points[out].number
            ));
        }
        in_ring.extend(ring.iter().copied());
        holes.push(clip_ring(
            &who,
            &ring,
            &file.points,
            &vertex_of_point,
            &mut constraints,
            &mut edge_origin,
        ));
    }
    warn_unused_classes(file, &in_breakline, &in_ring, avisos);

    let mut vertex_number = vertex_number;
    let grid_use = match &file.grid {
        Some(grid) => {
            let nodes = grid_nodes(grid, &points, &constraints, boundary.as_ref(), &holes)?;
            let used = nodes.len();
            vertex_number.extend(std::iter::repeat_n(None, used));
            points.extend(nodes);
            Some(GridUse {
                source: grid.source.clone(),
                cell_m: grid.cell_m,
                rows: grid.rows,
                cols: grid.cols,
                nodes_used: used,
            })
        }
        None => None,
    };
    if points.len() < 3 {
        return Err(format!(
            "E_TOPO_VACIA: hay {} punto(s) que entran al TIN (terreno, quiebre, contorno o malla); se necesitan al menos 3",
            points.len()
        ));
    }
    Ok(TinPlan {
        input: TinInput {
            points,
            constraints,
            boundary,
            holes,
        },
        vertex_number,
        edge_origin,
        grid_use,
    })
}

const BREAKLINE_CLASSES: &[PointClass] = &[PointClass::Breakline, PointClass::Boundary];

fn chain_name(kind: &str, k: usize, chain: &SurveyChain) -> String {
    if chain.description.is_empty() {
        format!("{kind} {}", k + 1)
    } else {
        format!("{kind} {} «{}»", k + 1, chain.description)
    }
}

fn edge_key(a: u32, b: u32) -> [u32; 2] {
    [a.min(b), a.max(b)]
}

fn plan_of(q: &SurveyPoint) -> P2 {
    P2::new(q.e, q.n)
}

fn index_numbers(points: &[SurveyPoint]) -> Result<HashMap<&str, usize>, String> {
    let mut by_number: HashMap<&str, usize> = HashMap::with_capacity(points.len());
    let mut repeated = Vec::new();
    for (i, q) in points.iter().enumerate() {
        if q.number.trim().is_empty() {
            return Err(format!(
                "E_TOPO_PUNTO: el punto en ({}, {}) no tiene número; cada punto del levantamiento lleva el suyo",
                q.e, q.n
            ));
        }
        if by_number.insert(q.number.as_str(), i).is_some() {
            repeated.push(q.number.clone());
        }
    }
    if repeated.is_empty() {
        Ok(by_number)
    } else {
        Err(format!(
            "E_TOPO_PUNTO: {} número(s) de punto repetidos: {}. Cada número es único; no se elige uno",
            repeated.len(),
            examples(&repeated)
        ))
    }
}

/// Which survey points become TIN vertices. Same plan and same height: the first one stands for
/// both (with a warning). Same plan and different heights: the surface would need two heights in
/// one place, so the file is rejected.
struct TinVertices {
    /// TIN vertex of each survey point (`None` for excluded points).
    of_point: Vec<Option<u32>>,
    points: Vec<SurfacePoint>,
    numbers: Vec<Option<String>>,
}

fn tin_vertices(points: &[SurveyPoint], avisos: &mut Vec<String>) -> Result<TinVertices, String> {
    let mut vertex_of_point = vec![None; points.len()];
    let mut tin = Vec::new();
    let mut numbers = Vec::new();
    let mut at_place: HashMap<(i64, i64), usize> = HashMap::new();
    let mut clashes = Vec::new();
    let mut repeats = Vec::new();
    for (i, q) in points.iter().enumerate() {
        if !q.class.enters_tin() {
            continue;
        }
        let place = (
            (q.e / SAME_PLACE_M).round() as i64,
            (q.n / SAME_PLACE_M).round() as i64,
        );
        match at_place.get(&place) {
            Some(&first) => {
                let f = &points[first];
                if (f.z - q.z).abs() > SAME_HEIGHT_M {
                    clashes.push(format!(
                        "{} (z {}) y {} (z {})",
                        f.number, f.z, q.number, q.z
                    ));
                } else {
                    repeats.push(format!("{} = {}", q.number, f.number));
                    vertex_of_point[i] = vertex_of_point[first];
                }
            }
            None => {
                at_place.insert(place, i);
                vertex_of_point[i] = Some(tin.len() as u32);
                tin.push(SurfacePoint::new(q.e, q.n, q.z));
                numbers.push(Some(q.number.clone()));
            }
        }
    }
    if !clashes.is_empty() {
        return Err(format!(
            "E_TOPO_COINCIDENTES: {} par(es) de puntos del TIN en la misma planta con distinta cota: {}. La superficie no puede tener dos cotas en un sitio: marque uno como excluido o corríjalo en el origen",
            clashes.len(),
            examples(&clashes)
        ));
    }
    if !repeats.is_empty() {
        avisos.push(format!(
            "{} punto(s) repiten planta y cota de otro y entran una sola vez: {}",
            repeats.len(),
            examples(&repeats)
        ));
    }
    Ok(TinVertices {
        of_point: vertex_of_point,
        points: tin,
        numbers,
    })
}

fn resolve_chain(
    who: &str,
    chain: &SurveyChain,
    by_number: &HashMap<&str, usize>,
    points: &[SurveyPoint],
    allowed: &[PointClass],
) -> Result<Vec<usize>, String> {
    let mut idx = Vec::with_capacity(chain.points.len());
    for number in &chain.points {
        let Some(&i) = by_number.get(number.as_str()) else {
            return Err(format!(
                "E_TOPO_REFERENCIA: {who} cita el punto «{number}», que no está en `puntos`"
            ));
        };
        if !allowed.contains(&points[i].class) {
            let allowed: Vec<&str> = allowed.iter().map(|c| c.key()).collect();
            return Err(format!(
                "E_TOPO_REFERENCIA: {who} cita el punto «{number}», de clase {}; aquí solo valen {}",
                points[i].class.key(),
                allowed.join(" o ")
            ));
        }
        if idx.last() == Some(&i) {
            return Err(format!(
                "E_TOPO_GEOMETRIA: {who} repite seguido el punto «{number}»"
            ));
        }
        idx.push(i);
    }
    Ok(idx)
}

/// A closed ring: at least three distinct points and no edge crossing another. The closing point
/// may be repeated at the end.
fn resolve_ring(
    who: &str,
    chain: &SurveyChain,
    by_number: &HashMap<&str, usize>,
    points: &[SurveyPoint],
) -> Result<Vec<usize>, String> {
    const ANY: &[PointClass] = &[
        PointClass::Ground,
        PointClass::Breakline,
        PointClass::Boundary,
        PointClass::Excluded,
    ];
    let mut ring = resolve_chain(who, chain, by_number, points, ANY)?;
    if ring.len() > 1 && ring.first() == ring.last() {
        ring.pop();
    }
    let distinct: HashSet<usize> = ring.iter().copied().collect();
    if distinct.len() < 3 || distinct.len() != ring.len() {
        return Err(format!(
            "E_TOPO_GEOMETRIA: {who} necesita al menos 3 puntos distintos y no repetir ninguno ({} citados, {} distintos)",
            ring.len(),
            distinct.len()
        ));
    }
    let plan: Vec<P2> = ring.iter().map(|&i| plan_of(&points[i])).collect();
    let n = plan.len();
    for i in 0..n {
        for j in i + 1..n {
            let adjacent = j == i + 1 || (i == 0 && j == n - 1);
            if !adjacent && segments_cross(plan[i], plan[(i + 1) % n], plan[j], plan[(j + 1) % n]) {
                return Err(format!(
                    "E_TOPO_GEOMETRIA: {who} se corta a sí mismo: el lado {}–{} cruza el lado {}–{}",
                    points[ring[i]].number,
                    points[ring[(i + 1) % n]].number,
                    points[ring[j]].number,
                    points[ring[(j + 1) % n]].number
                ));
            }
        }
    }
    Ok(ring)
}

/// The ring in plan. When every vertex is a TIN vertex its edges become constraints and the clip
/// is exact; an excluded vertex (a building corner) has no height for the surface, so that ring
/// clips by overlap instead.
fn clip_ring(
    who: &str,
    ring: &[usize],
    points: &[SurveyPoint],
    vertex_of_point: &[Option<u32>],
    constraints: &mut Vec<[u32; 2]>,
    edge_origin: &mut HashMap<[u32; 2], String>,
) -> ClipRing {
    let vertices: Vec<Option<u32>> = ring.iter().map(|&i| vertex_of_point[i]).collect();
    let on_tin = vertices.iter().all(Option::is_some);
    if on_tin {
        let n = vertices.len();
        for k in 0..n {
            let (a, b) = (vertices[k].unwrap(), vertices[(k + 1) % n].unwrap());
            if a != b {
                constraints.push([a, b]);
                edge_origin.insert(edge_key(a, b), who.to_string());
            }
        }
    }
    ClipRing {
        vertices: ring.iter().map(|&i| plan_of(&points[i])).collect(),
        on_tin,
    }
}

fn warn_unused_classes(
    file: &SurveyFile,
    in_breakline: &HashSet<usize>,
    in_ring: &HashSet<usize>,
    avisos: &mut Vec<String>,
) {
    let loose_breaks: Vec<String> = file
        .points
        .iter()
        .enumerate()
        .filter(|(i, q)| q.class == PointClass::Breakline && !in_breakline.contains(i))
        .map(|(_, q)| q.number.clone())
        .collect();
    if !loose_breaks.is_empty() {
        avisos.push(format!(
            "{} punto(s) de clase quiebre no están en ninguna línea de quiebre (entran como terreno): {}",
            loose_breaks.len(),
            examples(&loose_breaks)
        ));
    }
    let loose_bounds: Vec<String> = file
        .points
        .iter()
        .enumerate()
        .filter(|(i, q)| q.class == PointClass::Boundary && !in_ring.contains(i))
        .map(|(_, q)| q.number.clone())
        .collect();
    if !loose_bounds.is_empty() {
        avisos.push(format!(
            "{} punto(s) de clase contorno no están en el contorno ni en un hueco (entran como terreno): {}",
            loose_bounds.len(),
            examples(&loose_bounds)
        ));
    }
}

/// Grid nodes that enter the TIN. Survey wins where it exists: a node closer than one cell to a
/// survey vertex or to a constraint edge is left out, so the DEM never fights the measured points
/// or bends a breakline. Nodes outside the boundary or inside a hole are left out too.
fn grid_nodes(
    grid: &ElevationGrid,
    survey: &[SurfacePoint],
    constraints: &[[u32; 2]],
    boundary: Option<&ClipRing>,
    holes: &[ClipRing],
) -> Result<Vec<SurfacePoint>, String> {
    if !(grid.cell_m.is_finite() && grid.cell_m > 0.0) {
        return Err(format!(
            "E_TOPO_MALLA: celda_m = {}; debe ser un tamaño de celda positivo en metros",
            grid.cell_m
        ));
    }
    if grid.rows == 0 || grid.cols == 0 || grid.rows.saturating_mul(grid.cols) != grid.values.len()
    {
        return Err(format!(
            "E_TOPO_MALLA: filas × columnas = {} × {} y hay {} valores; deben coincidir",
            grid.rows,
            grid.cols,
            grid.values.len()
        ));
    }
    let cell = grid.cell_m;
    let mut occupied: HashSet<(i64, i64)> = HashSet::new();
    let mut mark = |p: P2| {
        occupied.insert(((p.x / cell).floor() as i64, (p.y / cell).floor() as i64));
    };
    for q in survey {
        mark(P2::new(q.e, q.n));
    }
    for [a, b] in constraints {
        let (a, b) = (survey[*a as usize], survey[*b as usize]);
        let (pa, pb) = (P2::new(a.e, a.n), P2::new(b.e, b.n));
        let steps = (pa.dist(pb) / (cell * 0.5)).ceil().max(1.0) as usize;
        for s in 0..=steps {
            mark(pa.lerp(pb, s as f64 / steps as f64));
        }
    }
    let near_survey = |p: P2| {
        let (cx, cy) = ((p.x / cell).floor() as i64, (p.y / cell).floor() as i64);
        (-1..=1).any(|dx| (-1..=1).any(|dy| occupied.contains(&(cx + dx, cy + dy))))
    };
    let boundary = boundary.map(RingTest::new);
    let holes: Vec<RingTest> = holes.iter().map(RingTest::new).collect();
    let mut nodes = Vec::new();
    for row in 0..grid.rows {
        for col in 0..grid.cols {
            let z = grid.values[row * grid.cols + col];
            if grid.nodata.is_some_and(|nd| z == nd) || !z.is_finite() {
                continue;
            }
            let p = P2::new(
                grid.northwest_center_e + col as f64 * cell,
                grid.northwest_center_n - row as f64 * cell,
            );
            let outside = boundary.as_ref().is_some_and(|b| !b.contains(p));
            let in_hole = holes.iter().any(|h| h.contains(p));
            if outside || in_hole || near_survey(p) {
                continue;
            }
            nodes.push(SurfacePoint::new(p.x, p.y, z));
        }
    }
    if nodes.len() > GRID_NODES_MAX {
        return Err(format!(
            "E_TOPO_MALLA: {} nodos de malla entrarían al TIN (máximo {GRID_NODES_MAX}); recorte la malla a la zona del proyecto",
            nodes.len()
        ));
    }
    Ok(nodes)
}

fn crossing_error(edges: &[[u32; 2]], plan: &TinPlan) -> String {
    let name = |v: u32| {
        plan.vertex_number[v as usize]
            .clone()
            .unwrap_or_else(|| format!("nodo de malla {v}"))
    };
    let described: Vec<String> = edges
        .iter()
        .map(|&[a, b]| {
            let origin = plan
                .edge_origin
                .get(&edge_key(a, b))
                .cloned()
                .unwrap_or_else(|| "una restricción".into());
            format!("{origin} (tramo {}–{})", name(a), name(b))
        })
        .collect();
    format!(
        "E_TOPO_GEOMETRIA: {} tramo(s) de quiebre o de contorno cruzan otro: {}. Dos líneas de quiebre no pueden cortarse fuera de un punto común; corríjalo en la traducción",
        described.len(),
        examples(&described)
    )
}

fn examples(items: &[String]) -> String {
    let mut s = items
        .iter()
        .take(EXAMPLES)
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    if items.len() > EXAMPLES {
        s.push_str(&format!(" y {} más", items.len() - EXAMPLES));
    }
    s
}
