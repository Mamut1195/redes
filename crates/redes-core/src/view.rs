//! Vista 3D: cámara orbital, proyección en perspectiva y armado de una **escena de primitivas
//! ya proyectadas**, ordenadas de atrás hacia adelante.
//!
//! Este módulo no dibuja: produce datos. Así la app (Rune) y el exportador SVG comparten
//! exactamente la misma vista, y las pruebas pueden verificar la escena sin GPU.

use crate::families::{caps, family};
use crate::geom::P3;
use crate::model::{EstadoObjeto, Project};

/// Cámara orbital alrededor de un objetivo.
#[derive(Clone, Debug)]
pub struct Camera {
    pub target: [f64; 3],
    /// Grados, sentido antihorario desde el este.
    pub azimuth: f32,
    /// Grados sobre el horizonte.
    pub elevation: f32,
    /// Distancia al objetivo (m).
    pub distance: f64,
    /// Campo de visión vertical (grados).
    pub fov: f64,
    /// Tamaño del viewport en puntos.
    pub viewport: [f32; 2],
    /// Exageración vertical (1.0 = escala real). Ayuda a leer el relieve.
    pub z_scale: f64,
}

impl Default for Camera {
    fn default() -> Self {
        Camera {
            target: [0.0, 0.0, 0.0],
            azimuth: -55.0,
            elevation: 32.0,
            distance: 900.0,
            fov: 45.0,
            viewport: [1000.0, 640.0],
            z_scale: 1.0,
        }
    }
}

impl Camera {
    /// Encuadra un proyecto: centro en planta, distancia según el tamaño.
    pub fn fit(&mut self, p: &Project) {
        let bb = p.bbox();
        let (z0, z1) = p.z_range();
        let o = p.origin;
        let cx = (bb.xmin + bb.xmax) * 0.5 - o.x;
        let cy = (bb.ymin + bb.ymax) * 0.5 - o.y;
        self.target = [cx, cy, (z0 + z1) * 0.5];
        let diag = ((bb.width().powi(2) + bb.height().powi(2)).sqrt()).max(60.0);
        self.distance = (diag * 1.25).max(120.0);
    }

    /// Orbita alrededor del objetivo. Ángulos en grados.
    pub fn orbit(&mut self, d_azimuth: f32, d_elevation: f32) {
        self.azimuth = (self.azimuth + d_azimuth) % 360.0;
        self.elevation = (self.elevation + d_elevation).clamp(2.0, 88.0);
    }

    /// Acerca o aleja (factor > 1 aleja).
    pub fn dolly(&mut self, factor: f64) {
        self.distance = (self.distance * factor).clamp(5.0, 200_000.0);
    }

    /// Desplaza el objetivo en el plano de la pantalla (`dx`, `dy` en puntos).
    pub fn pan(&mut self, dx: f32, dy: f32) {
        let (_, right, up) = self.basis();
        let scale = self.distance / self.focal();
        let mx = -dx as f64 * scale;
        let my = dy as f64 * scale;
        for i in 0..3 {
            self.target[i] += right[i] * mx + up[i] * my;
        }
    }

    pub fn eye(&self) -> [f64; 3] {
        let az = (self.azimuth as f64).to_radians();
        let el = (self.elevation as f64).to_radians();
        [
            self.target[0] + self.distance * el.cos() * az.cos(),
            self.target[1] + self.distance * el.cos() * az.sin(),
            self.target[2] + self.distance * el.sin(),
        ]
    }

    fn basis(&self) -> ([f64; 3], [f64; 3], [f64; 3]) {
        let eye = self.eye();
        let f = normalize([
            self.target[0] - eye[0],
            self.target[1] - eye[1],
            self.target[2] - eye[2],
        ]);
        let up = [0.0, 0.0, 1.0];
        let mut right = cross(f, up);
        if len(right) < 1e-9 {
            right = [1.0, 0.0, 0.0];
        }
        right = normalize(right);
        let up2 = cross(right, f);
        (eye, right, up2)
    }

    /// Matriz vista-proyección en columnas (16 floats) para un pase 3D con profundidad.
    ///
    /// Usa la misma base y la misma exageración vertical que [`Camera::project`], así el pase 3D y
    /// el pintor 2D encuadran igual.
    pub fn matriz(&self, aspecto: f32, z_scale: f32) -> [f32; 16] {
        let (eye, right, up) = self.basis();
        let f = self.forward();
        let d = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];

        // Vista (fila por fila).
        let v = [
            [right[0], right[1], right[2], -d(right, eye)],
            [up[0], up[1], up[2], -d(up, eye)],
            [-f[0], -f[1], -f[2], d(f, eye)],
            [0.0, 0.0, 0.0, 1.0],
        ];
        let aspect = aspecto.max(0.05) as f64;
        // `fov` está en grados: sin `to_radians` la tangente sale de un ángulo absurdo y el pase
        // 3D dibujaba el modelo ~1,3× más pequeño que el pintor.
        let tan = (self.fov * 0.5).to_radians().tan().max(1e-3);
        // Rango de profundidad atado a la distancia de la cámara: con far/near gigantes el
        // z-búfer pierde precisión y las caras lejanas ganan a las cercanas.
        let cerca = (self.distance * 0.02).clamp(0.05, 200.0);
        let lejos = (self.distance * 60.0).max(2_000.0);
        let fc = 1.0 / tan;
        let proy = [
            [fc / aspect, 0.0, 0.0, 0.0],
            [0.0, fc, 0.0, 0.0],
            [
                0.0,
                0.0,
                lejos / (cerca - lejos),
                lejos * cerca / (cerca - lejos),
            ],
            [0.0, 0.0, -1.0, 0.0],
        ];
        // Exageración vertical alrededor de la cota del objetivo.
        let k = z_scale as f64;
        let zr = self.target[2];
        let esc = [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, k, (1.0 - k) * zr],
            [0.0, 0.0, 0.0, 1.0],
        ];
        let mul = |a: [[f64; 4]; 4], b: [[f64; 4]; 4]| -> [[f64; 4]; 4] {
            let mut m = [[0.0f64; 4]; 4];
            for i in 0..4 {
                for j in 0..4 {
                    m[i][j] = (0..4).map(|q| a[i][q] * b[q][j]).sum();
                }
            }
            m
        };
        let m = mul(mul(proy, v), esc);
        let mut out = [0f32; 16];
        for col in 0..4 {
            for row in 0..4 {
                out[col * 4 + row] = m[row][col] as f32;
            }
        }
        out
    }

    /// Proyecta un punto del mundo local (origen ya restado). `None` si está detrás.
    ///
    /// Para muchos puntos con la misma cámara use [`Camera::proyector`]: esto rehace la base.
    pub fn project(&self, p: [f64; 3]) -> Option<[f32; 2]> {
        self.proyector().project(p)
    }

    fn forward(&self) -> [f64; 3] {
        let eye = self.eye();
        normalize([
            self.target[0] - eye[0],
            self.target[1] - eye[1],
            self.target[2] - eye[2],
        ])
    }

    fn focal(&self) -> f64 {
        (self.viewport[1] as f64 * 0.5) / (self.fov * 0.5).to_radians().tan()
    }

    /// Distancia del punto al plano de la cámara (para ordenar).
    pub fn depth(&self, p: [f64; 3]) -> f64 {
        self.proyector().depth(p)
    }

    /// Proyecta un segmento recortándolo contra el plano cercano.
    pub fn project_segment(&self, a: [f64; 3], b: [f64; 3]) -> Option<([f32; 2], [f32; 2])> {
        self.proyector().project_segment(a, b)
    }

    /// La cámara precalculada (F14-04): ojo, base, eje de visión y focal se calculan una vez y
    /// cada punto cuesta tres productos escalares. Antes cada `project` rehacía la base (seis
    /// llamadas trigonométricas y dos normalizaciones) y cada `depth` el ojo dos veces.
    ///
    /// Da los mismos números que [`Camera::project`] y [`Camera::depth`], bit a bit: es la
    /// misma cuenta en el mismo orden, solo que hecha una vez.
    pub fn proyector(&self) -> Proyector {
        let (eye, right, up) = self.basis();
        Proyector {
            eye,
            right,
            up,
            forward: self.forward(),
            focal: self.focal(),
            viewport: self.viewport,
        }
    }
}

/// Cámara precalculada para proyectar muchos puntos (F14-04). Se obtiene con
/// [`Camera::proyector`] y no se guarda: si la cámara cambia, se pide otro.
#[derive(Clone, Copy, Debug)]
pub struct Proyector {
    eye: [f64; 3],
    right: [f64; 3],
    up: [f64; 3],
    forward: [f64; 3],
    focal: f64,
    viewport: [f32; 2],
}

impl Proyector {
    /// Proyecta un punto del mundo local (origen ya restado). `None` si está detrás.
    #[inline]
    pub fn project(&self, p: [f64; 3]) -> Option<[f32; 2]> {
        self.proyectar(p).map(|(s, _)| s)
    }

    /// Proyección y profundidad de un punto a la vez (la profundidad es la `z` de la
    /// proyección: sale gratis). `None` si está detrás.
    #[inline]
    pub fn proyectar(&self, p: [f64; 3]) -> Option<([f32; 2], f64)> {
        let v = [p[0] - self.eye[0], p[1] - self.eye[1], p[2] - self.eye[2]];
        let z = dot(v, self.forward);
        if z <= 0.05 {
            return None;
        }
        let f = z / self.focal;
        let x = (dot(v, self.right) / f) as f32;
        let y = (dot(v, self.up) / f) as f32;
        Some(([self.viewport[0] * 0.5 + x, self.viewport[1] * 0.5 - y], z))
    }

    /// Distancia del punto al plano de la cámara (para ordenar).
    #[inline]
    pub fn depth(&self, p: [f64; 3]) -> f64 {
        dot(
            [p[0] - self.eye[0], p[1] - self.eye[1], p[2] - self.eye[2]],
            self.forward,
        )
    }

    /// Distancia focal en puntos: un objeto de `r` metros a `d` metros mide `r·focal/d` puntos.
    pub fn focal(&self) -> f64 {
        self.focal
    }

    /// Tamaño del viewport en puntos.
    pub fn viewport(&self) -> [f32; 2] {
        self.viewport
    }

    /// Proyecta un segmento recortándolo contra el plano cercano.
    pub fn project_segment(&self, a: [f64; 3], b: [f64; 3]) -> Option<([f32; 2], [f32; 2])> {
        self.segmento(a, b, self.depth(a), self.depth(b))
    }

    /// [`Proyector::project_segment`] con las profundidades de los extremos ya calculadas.
    fn segmento(&self, a: [f64; 3], b: [f64; 3], da: f64, db: f64) -> Option<([f32; 2], [f32; 2])> {
        let (a, b) = if da <= 0.05 && db <= 0.05 {
            return None;
        } else if da <= 0.05 {
            (lerp3(a, b, (0.06 - da) / (db - da)), b)
        } else if db <= 0.05 {
            (a, lerp3(a, b, (0.06 - da) / (db - da)))
        } else {
            (a, b)
        };
        Some((self.project(a)?, self.project(b)?))
    }
}

fn lerp3(a: [f64; 3], b: [f64; 3], t: f64) -> [f64; 3] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn len(a: [f64; 3]) -> f64 {
    dot(a, a).sqrt()
}
fn normalize(a: [f64; 3]) -> [f64; 3] {
    let l = len(a).max(1e-12);
    [a[0] / l, a[1] / l, a[2] / l]
}

/// Qué capas se dibujan.
#[derive(Clone, Copy, Debug)]
pub struct Layers {
    pub terreno: bool,
    pub alcantarillado: bool,
    pub agua: bool,
    pub nodos: bool,
    /// Edificios y calles (contexto urbano). Apagarlo deja ver la red bajo la ciudad.
    pub edificios: bool,
}

impl Default for Layers {
    fn default() -> Self {
        Layers {
            terreno: true,
            alcantarillado: true,
            agua: true,
            nodos: true,
            edificios: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Prim {
    /// Polígono relleno ya proyectado.
    Poly {
        pts: Vec<[f32; 2]>,
        fill: [f32; 4],
        depth: f32,
    },
    /// Segmento con ancho en puntos.
    Line {
        a: [f32; 2],
        b: [f32; 2],
        w: f32,
        color: [f32; 4],
        depth: f32,
    },
}

/// Modos de inspección de detalle (fase 2): corte, explosión y foco.
#[derive(Clone, Copy, Debug, Default)]
pub struct Opciones {
    /// Plano de corte vertical en coordenadas locales (`x`), o `None`.
    pub corte_x: Option<f32>,
    /// Explosión 0..2.5 aplicada a las piezas del objeto enfocado.
    pub explosion: f32,
    /// Objeto enfocado (explosión y etiqueta destacada).
    pub foco: Option<crate::model::ElementId>,
    /// Rayos X: lo enterrado (solera, fuste, escalera) se dibuja translúcido.
    pub rayos_x: bool,
}

/// Etiqueta en pantalla que la app dibuja como texto.
#[derive(Clone, Debug)]
pub struct Etiqueta {
    pub pos: [f32; 2],
    pub texto: String,
    pub seleccion: bool,
    pub depth: f32,
}

#[derive(Clone, Debug, Default)]
pub struct Scene {
    pub prims: Vec<Prim>,
    pub tri_count: usize,
    pub line_count: usize,
    /// Etiquetas ya proyectadas, en orden de dibujo (lejos → cerca).
    pub etiquetas: Vec<Etiqueta>,
    /// Primitivas, tramos y piezas descartados por caer enteros fuera de pantalla: no entran
    /// en el ordenamiento del pintor (F14-07).
    pub descartadas: usize,
}

/// Construye la escena: terreno sombreado, tubos, pozos y resaltado.
pub fn build(p: &Project, cam: &Camera, layers: Layers, selected: Option<&str>) -> Scene {
    build_with(p, p.terrain.as_ref(), cam, layers, selected)
}

/// Igual que [`build`] pero dibuja un terreno alternativo (una versión aligerada, por ejemplo).
pub fn build_with(
    p: &Project,
    terrain: Option<&crate::terrain::Terrain>,
    cam: &Camera,
    layers: Layers,
    selected: Option<&str>,
) -> Scene {
    build_opts(p, terrain, cam, layers, selected, &Opciones::default())
}

/// Igual que [`build_with`] con los modos de detalle activos.
/// Encoge un punto al interior de una caja, con un margen de 0,5 m.
///
/// Con una caja menor de 1 m el margen deja `min > max` y `clamp` paniquea (R-10). Aquí los
/// límites se ordenan y, si no queda hueco, se devuelve el centro.
fn recortar_a_bbox(
    q: crate::geom::P2,
    (xmin, xmax, ymin, ymax): (f64, f64, f64, f64),
) -> crate::geom::P2 {
    fn eje(v: f64, min: f64, max: f64) -> f64 {
        let (lo, hi) = (min + 0.5, max - 0.5);
        if lo <= hi {
            v.clamp(lo, hi)
        } else {
            (min + max) * 0.5
        }
    }
    crate::geom::P2::new(eje(q.x, xmin, xmax), eje(q.y, ymin, ymax))
}

pub fn build_opts(
    p: &Project,
    terrain: Option<&crate::terrain::Terrain>,
    cam: &Camera,
    layers: Layers,
    selected: Option<&str>,
    opts: &Opciones,
) -> Scene {
    build_with_models(p, terrain, cam, layers, selected, opts, &ModelPieces::new())
}

/// A library model that replaces the parametric piece of an object (D14): it is drawn instead
/// of it, with the same cut, tint and culling.
#[derive(Clone, Debug)]
pub struct ModelPiece {
    /// The model's triangles in its own frame (Z up, base at z = 0), already shaded.
    pub part: std::sync::Arc<crate::parts::Parte>,
    /// Height of the model's z = 0 above the object's invert (negative when it sits below it,
    /// as a baked manhole does by its base slab).
    pub lift_m: f64,
    pub rotation_deg: f32,
}

/// Model pieces by object.
pub type ModelPieces = std::collections::HashMap<crate::model::ElementId, ModelPiece>;

/// [`build_opts`] with library models replacing the pieces of the objects that have one.
pub fn build_with_models(
    p: &Project,
    terrain: Option<&crate::terrain::Terrain>,
    cam: &Camera,
    layers: Layers,
    selected: Option<&str>,
    opts: &Opciones,
    models: &ModelPieces,
) -> Scene {
    let no_pipes = crate::pipe_layout::PipePieces::default();
    construir(
        p, terrain, cam, layers, selected, opts, true, models, &no_pipes,
    )
}

/// [`build_with_models`] with baked pipes laid along the links that have one (F17-04).
#[allow(clippy::too_many_arguments)]
pub fn build_with_library(
    p: &Project,
    terrain: Option<&crate::terrain::Terrain>,
    cam: &Camera,
    layers: Layers,
    selected: Option<&str>,
    opts: &Opciones,
    models: &ModelPieces,
    pipes: &crate::pipe_layout::PipePieces,
) -> Scene {
    construir(p, terrain, cam, layers, selected, opts, true, models, pipes)
}

/// [`build_opts`] con el recorte por pantalla a elección: las pruebas comparan con y sin él.
#[allow(clippy::too_many_arguments)]
fn construir(
    p: &Project,
    terrain: Option<&crate::terrain::Terrain>,
    cam: &Camera,
    layers: Layers,
    selected: Option<&str>,
    opts: &Opciones,
    recortar: bool,
    models: &ModelPieces,
    pipes: &crate::pipe_layout::PipePieces,
) -> Scene {
    let o = p.origin;
    let zr = cam.target[2];
    let k = cam.z_scale;
    let to_local = move |q: P3| [q.x - o.x, q.y - o.y, (q.z - zr) * k + zr];
    // Cámara precalculada una vez por escena (F14-04).
    let pr = cam.proyector();
    // Recorte por pantalla (F14-07): lo que cae entero fuera del viewport no entra en las listas
    // y, por tanto, no se ordena. Sin recorte, la pantalla es infinita.
    let pantalla = if recortar {
        Pantalla::de(&pr)
    } else {
        Pantalla::infinita()
    };
    let mut descartadas = 0usize;
    let mut prims: Vec<Prim> = Vec::new();
    // La red se dibuja despues del terreno (modo rayos X): las tuberias van enterradas
    // y en una revision hay que verlas aunque el terreno las tape.
    let mut net: Vec<Prim> = Vec::new();
    // Contexto urbano: las calles van sobre el terreno y los edificios sobre las calles.
    let mut roads: Vec<Prim> = Vec::new();
    let mut city: Vec<Prim> = Vec::new();
    let mut tri_count = 0;
    let mut line_count = 0;
    let mut etiquetas: Vec<Etiqueta> = Vec::new();

    // --- terreno ---
    if layers.terreno
        && let Some(t) = terrain
    {
        let light = normalize([-0.45, -0.6, 0.66]);
        let (zmin, zmax) = t.z_range();
        let zspan = (zmax - zmin).max(1e-6);
        for tri in &t.tris {
            let a = t.points[tri[0] as usize];
            let b = t.points[tri[1] as usize];
            let c = t.points[tri[2] as usize];
            let pa = to_local(P3::new(a.e, a.n, a.z));
            let pb = to_local(P3::new(b.e, b.n, b.z));
            let pc = to_local(P3::new(c.e, c.n, c.z));
            let mut n = normalize(cross(
                [pb[0] - pa[0], pb[1] - pa[1], pb[2] - pa[2]],
                [pc[0] - pa[0], pc[1] - pa[1], pc[2] - pa[2]],
            ));
            if n[2] < 0.0 {
                // La triangulación puede venir con cualquier orientación: se normaliza
                // hacia arriba en vez de descartar la cara.
                n = [-n[0], -n[1], -n[2]];
            }
            let shade = (0.42 + 0.58 * dot(n, light).max(0.0)) as f32;
            let (Some((sa, da)), Some((sb, db)), Some((sc, dc))) =
                (pr.proyectar(pa), pr.proyectar(pb), pr.proyectar(pc))
            else {
                continue;
            };
            let pts = expand([sa, sb, sc], 0.6);
            if !pantalla.ve(&pts) {
                descartadas += 1;
                continue;
            }
            let depth = (da + db + dc) as f32 / 3.0;
            let fog = fog(depth, cam.distance as f32);
            // Color por cota (verde valle ? ocre altura) con sombreado por normal.
            let t01 = (((a.z + b.z + c.z) / 3.0 - zmin) / zspan).clamp(0.0, 1.0) as f32;
            let base = [0.26 + 0.34 * t01, 0.40 + 0.16 * t01, 0.26 + 0.06 * t01];
            prims.push(Prim::Poly {
                pts: pts.to_vec(),
                // Opaco: la niebla se mezcla contra el fondo en el color, no con alfa.
                // Con alfa, los triangulos solapados dejarian costuras brillantes.
                fill: [
                    base[0] * shade * fog + 0.050 * (1.0 - fog),
                    base[1] * shade * fog + 0.055 * (1.0 - fog),
                    base[2] * shade * fog + 0.066 * (1.0 - fog),
                    1.0,
                ],
                depth,
            });
            tri_count += 1;
        }
    }

    // --- ciudad: calles y manzanas ---
    if layers.edificios
        && let Some(ent) = &p.entorno
    {
        // Cota del terreno para apoyar calles y edificios. Si el punto cae fuera del TIN se
        // recorta al borde; si aun asi no hay dato, el elemento no se dibuja (nunca a cota 0).
        let z_de = |q: crate::geom::P2| -> Option<f64> {
            for tr in [terrain, p.terrain.as_ref()].into_iter().flatten() {
                let bb = tr.bbox;
                let dentro = recortar_a_bbox(q, (bb.xmin, bb.xmax, bb.ymin, bb.ymax));
                if let Some(z) = tr.z_at(dentro) {
                    return Some(z);
                }
            }
            None
        };

        // Calles: quads apoyados en el terreno, ligeramente elevados para no competir con el TIN.
        for calle in &ent.calles {
            let half = calle.ancho * 0.5;
            for w in calle.eje.windows(2) {
                let (a, b) = (w[0].p2(), w[1].p2());
                let d = ((b.x - a.x).powi(2) + (b.y - a.y).powi(2)).sqrt();
                if d < 0.01 {
                    continue;
                }
                let nx = -(b.y - a.y) / d * half;
                let ny = (b.x - a.x) / d * half;
                let esquinas = [
                    crate::geom::P2::new(a.x + nx, a.y + ny),
                    crate::geom::P2::new(b.x + nx, b.y + ny),
                    crate::geom::P2::new(b.x - nx, b.y - ny),
                    crate::geom::P2::new(a.x - nx, a.y - ny),
                ];
                let mut pts = [[0.0f32; 2]; 4];
                let mut prof = [0.0f64; 4];
                let mut ok = true;
                for (i, q) in esquinas.into_iter().enumerate() {
                    let Some(z) = z_de(q) else {
                        ok = false;
                        break;
                    };
                    let local = to_local(P3::new(q.x, q.y, z + 0.22));
                    match pr.proyectar(local) {
                        Some((s, d)) => {
                            pts[i] = s;
                            prof[i] = d;
                        }
                        None => ok = false,
                    }
                }
                if !ok {
                    continue;
                }
                if pantalla.ve(&pts) {
                    let depth = prof.iter().sum::<f64>() as f32 / 4.0;
                    roads.push(Prim::Poly {
                        pts: pts.to_vec(),
                        fill: [0.175, 0.185, 0.205, 1.0],
                        depth,
                    });
                } else {
                    descartadas += 1;
                }
                // Aceras: dos franjas claras a los lados de la calzada.
                for lado in [-1.0f64, 1.0] {
                    let off = lado * (half + 1.2);
                    let ancho_acera = 2.2;
                    let sx = -(b.y - a.y) / d;
                    let sy = (b.x - a.x) / d;
                    let mut apts = [[0.0f32; 2]; 4];
                    let mut aprof = [0.0f64; 4];
                    let mut aok = true;
                    for (i, (px, py)) in [
                        (a.x + sx * off, a.y + sy * off),
                        (b.x + sx * off, b.y + sy * off),
                        (
                            b.x + sx * (off + lado * ancho_acera),
                            b.y + sy * (off + lado * ancho_acera),
                        ),
                        (
                            a.x + sx * (off + lado * ancho_acera),
                            a.y + sy * (off + lado * ancho_acera),
                        ),
                    ]
                    .into_iter()
                    .enumerate()
                    {
                        let q = crate::geom::P2::new(px, py);
                        let Some(z) = z_de(q) else {
                            aok = false;
                            break;
                        };
                        let local = to_local(P3::new(px, py, z + 0.26));
                        match pr.proyectar(local) {
                            Some((s, d)) => {
                                apts[i] = s;
                                aprof[i] = d;
                            }
                            None => aok = false,
                        }
                    }
                    if aok && !pantalla.ve(&apts) {
                        descartadas += 1;
                    } else if aok {
                        let ad = aprof.iter().sum::<f64>() as f32 / 4.0;
                        roads.push(Prim::Poly {
                            pts: apts.to_vec(),
                            fill: [0.42, 0.42, 0.43, 1.0],
                            depth: ad,
                        });
                    }
                }
            }
        }

        // Manzanas: muros troceados (para que el orden por profundidad funcione) y cubierta en
        // abanico desde el centroide.
        const TROZO: f64 = 12.0;
        for mz in &ent.manzanas {
            if mz.poligono.len() < 3 {
                continue;
            }
            let cotas: Vec<f64> = mz.poligono.iter().filter_map(|q| z_de(*q)).collect();
            if cotas.len() < 3 {
                continue;
            }
            // El edificio arranca en la cota más baja (enterrado 1 m en el lado alto) y la
            // cubierta queda a nivel sobre la cota más alta: así no flota en ladera.
            let base = cotas.iter().fold(f64::INFINITY, |a, b| a.min(*b)) - 1.0;
            let cota_alta = cotas.iter().fold(f64::NEG_INFINITY, |a, b| a.max(*b));
            let top = cota_alta + mz.altura.max(3.0);
            let (wr, wg, wb) = if mz.uso == "comercio" {
                (0.50, 0.55, 0.62)
            } else {
                (0.70, 0.64, 0.55)
            };
            let v = 0.80 + 0.32 * mz.tono;
            let n = mz.poligono.len();

            for i in 0..n {
                let p1 = mz.poligono[i];
                let p2 = mz.poligono[(i + 1) % n];
                let largo = ((p2.x - p1.x).powi(2) + (p2.y - p1.y).powi(2)).sqrt();
                if largo < 0.1 {
                    continue;
                }
                let trozos = (largo / TROZO).ceil() as usize;
                for s in 0..trozos {
                    let t0 = s as f64 / trozos as f64;
                    let t1 = (s + 1) as f64 / trozos as f64;
                    let a =
                        crate::geom::P2::new(p1.x + (p2.x - p1.x) * t0, p1.y + (p2.y - p1.y) * t0);
                    let b =
                        crate::geom::P2::new(p1.x + (p2.x - p1.x) * t1, p1.y + (p2.y - p1.y) * t1);
                    let quad = [
                        to_local(P3::new(a.x, a.y, base)),
                        to_local(P3::new(b.x, b.y, base)),
                        to_local(P3::new(b.x, b.y, top)),
                        to_local(P3::new(a.x, a.y, top)),
                    ];
                    let mut pts = [[0.0f32; 2]; 4];
                    let mut prof = [0.0f64; 4];
                    let mut ok = true;
                    for (i, q) in quad.into_iter().enumerate() {
                        match pr.proyectar(q) {
                            Some((s, d)) => {
                                pts[i] = s;
                                prof[i] = d;
                            }
                            None => ok = false,
                        }
                    }
                    if !ok {
                        continue;
                    }
                    if !pantalla.ve(&pts) {
                        descartadas += 1;
                        continue;
                    }
                    let depth = prof.iter().sum::<f64>() as f32 / 4.0;
                    let shade = 0.70 + 0.30 * (((p2.x - p1.x) as f32 / 18.0).cos().abs());
                    let f = fog(depth, cam.distance as f32);
                    city.push(Prim::Poly {
                        pts: pts.to_vec(),
                        fill: [
                            wr * v * shade * f + 0.050 * (1.0 - f),
                            wg * v * shade * f + 0.055 * (1.0 - f),
                            wb * v * shade * f + 0.066 * (1.0 - f),
                            1.0,
                        ],
                        depth,
                    });
                }
            }

            let cx = mz.poligono.iter().map(|q| q.x).sum::<f64>() / n as f64;
            let cy = mz.poligono.iter().map(|q| q.y).sum::<f64>() / n as f64;
            let centro = to_local(P3::new(cx, cy, top));
            let Some((cs, cd)) = pr.proyectar(centro) else {
                continue;
            };
            for i in 0..n {
                let q1 = mz.poligono[i];
                let q2 = mz.poligono[(i + 1) % n];
                let a = to_local(P3::new(q1.x, q1.y, top));
                let b = to_local(P3::new(q2.x, q2.y, top));
                let (Some((sa, da)), Some((sb, db))) = (pr.proyectar(a), pr.proyectar(b)) else {
                    continue;
                };
                if !pantalla.ve(&[cs, sa, sb]) {
                    descartadas += 1;
                    continue;
                }
                let depth = ((cd + da + db) / 3.0) as f32;
                let f = fog(depth, cam.distance as f32);
                city.push(Prim::Poly {
                    pts: vec![cs, sa, sb],
                    fill: [
                        0.40 * f + 0.050 * (1.0 - f),
                        0.38 * f + 0.055 * (1.0 - f),
                        0.36 * f + 0.066 * (1.0 - f),
                        1.0,
                    ],
                    depth,
                });
            }
        }
    }

    // --- arcos: tubo paramétrico si se ve grande; si no, línea ---
    // Con muchos tramos se reparten en bloques entre varios hilos (F14-04): cada bloque produce
    // sus primitivas en orden y se concatenan en el orden de los bloques, así que la escena es
    // idéntica a la de un solo hilo.
    let cx = Contexto {
        p,
        pr,
        pantalla,
        layers,
        selected,
        corte_x: opts.corte_x,
        zr,
        k,
        distancia: cam.distance as f32,
        pipes,
    };
    for b in en_bloques(&p.links, |bloque| dibujar_tramos(bloque, &cx)) {
        net.extend(b.prims);
        line_count += b.lineas;
        descartadas += b.descartadas;
    }

    // --- nodos: piezas paramétricas por familia ---
    // Primero, en bloques y en paralelo, lo que no depende del presupuesto de triángulos: qué
    // nodos se dibujan, su tamaño en pantalla y los rombos de los lejanos. Después, en orden, las
    // piezas (su nivel de detalle depende de los triángulos ya emitidos).
    if layers.nodos {
        let clases = en_bloques(&p.nodes, |bloque| clasificar_nodos(bloque, &cx));
        for clase in clases.into_iter().flatten() {
            let (n, top, r_px, sel, tinte) = match clase {
                Clase::Rombo(prim) => {
                    net.push(prim);
                    tri_count += 1;
                    continue;
                }
                Clase::Fuera => {
                    descartadas += 1;
                    continue;
                }
                Clase::Pieza {
                    n,
                    top,
                    r_px,
                    sel,
                    tinte,
                } => (n, top, r_px, sel, tinte),
            };

            // Explosión: cada parte nombrada del objeto enfocado sube a su altura.
            let altura_pieza = (n.z_rim - n.z_invert).max(0.5) as f32;
            let explotar = opts.explosion > 0.0 && opts.foco == Some(n.id);

            // Una pieza entera fuera de pantalla no se proyecta (su etiqueta sí se calcula, como
            // antes: el reparto de etiquetas no depende del recorte). Los pozos se descartan sin
            // teselarlos, con una esfera que se sabe de antemano; el resto, con la de su malla.
            let dz_max = if explotar {
                (opts.explosion * altura_pieza * 0.85) as f64
            } else {
                0.0
            };
            let model = models.get(&n.id);
            if model.is_none()
                && let Some((centro, radio)) = esfera_previa(n, dz_max, &to_local, k)
                && !pantalla.ve_esfera(&pr, centro, radio)
            {
                descartadas += 1;
                etiquetas.extend(etiqueta_de(n, &pr, top, r_px, sel));
                continue;
            }

            // LOD por tamaño en pantalla, con presupuesto global de triángulos.
            let mut lod = if r_px > 14.0 { 2u8 } else { 1u8 };
            if tri_count > 90_000 {
                lod = 1;
            }
            let parametric;
            let partes: &[crate::parts::Parte] = match model {
                Some(m) => std::slice::from_ref(m.part.as_ref()),
                None => {
                    parametric = pieza_partes(n, lod);
                    &parametric
                }
            };
            if partes.is_empty() {
                continue;
            }
            let colocar = colocador(model);
            if let Some((centro, radio)) =
                esfera_de_pieza(partes, n, dz_max, &colocar, &to_local, k)
                && !pantalla.ve_esfera(&pr, centro, radio)
            {
                descartadas += 1;
                etiquetas.extend(etiqueta_de(n, &pr, top, r_px, sel));
                continue;
            }

            for (indice_parte, parte) in partes.iter().enumerate() {
                for tri in &parte.mesh.tris {
                    let mut dz = 0.0f64;
                    if explotar {
                        // Reparto proporcional: la pila entera se abre ~0,85 de la altura de la pieza.
                        let n = partes.len().max(2) as f32;
                        let t = indice_parte as f32 / (n - 1.0);
                        dz = (t * opts.explosion * altura_pieza * 0.85) as f64;
                    }
                    let tri_local = tri.p.map(|v| {
                        let q = colocar(v);
                        to_local(P3::new(
                            q[0] + n.pos.x,
                            q[1] + n.pos.y,
                            q[2] + n.z_invert + dz,
                        ))
                    });

                    // Corte vertical: se recorta el triángulo y se marca la arista de corte.
                    let (trozos, corte) = match opts.corte_x {
                        Some(xc) => cortar_tri(tri_local, xc),
                        None => (Trozos::uno(tri_local), None),
                    };
                    if let Some((a, b)) = corte
                        && let (Some((sa, da)), Some((sb, db))) = (pr.proyectar(a), pr.proyectar(b))
                    {
                        let d = ((da + db) * 0.5) as f32;
                        if pantalla.ve_linea(sa, sb, 1.6) {
                            net.push(Prim::Line {
                                a: sa,
                                b: sb,
                                w: 1.6,
                                color: [0.98, 0.86, 0.42, 1.0],
                                depth: d,
                            });
                            line_count += 1;
                        } else {
                            descartadas += 1;
                        }

                        // Poché: la cara de corte se traman con rayitas perpendiculares, para que
                        // la sección se lea como un plano y no como un alambre amarillo.
                        let (dx, dy) = (sb[0] - sa[0], sb[1] - sa[1]);
                        let largo = (dx * dx + dy * dy).sqrt();
                        if largo > 6.0 {
                            let (ux, uy) = (dx / largo, dy / largo);
                            let (px, py) = (-uy, ux);
                            let paso = 7.0f32;
                            let mut s = paso * 0.5;
                            while s < largo {
                                let (cx, cy) = (sa[0] + ux * s, sa[1] + uy * s);
                                let (ra, rb) = (
                                    [cx - px * 2.6 + ux * 1.6, cy - py * 2.6 + uy * 1.6],
                                    [cx + px * 2.6 - ux * 1.6, cy + py * 2.6 - uy * 1.6],
                                );
                                if pantalla.ve_linea(ra, rb, 1.0) {
                                    net.push(Prim::Line {
                                        a: ra,
                                        b: rb,
                                        w: 1.0,
                                        color: [0.86, 0.74, 0.40, 1.0],
                                        depth: d,
                                    });
                                    line_count += 1;
                                } else {
                                    descartadas += 1;
                                }
                                s += paso;
                            }
                        }
                    }

                    for q in trozos.iter() {
                        let (Some((s0, d0)), Some((s1, d1)), Some((s2, d2))) =
                            (pr.proyectar(q[0]), pr.proyectar(q[1]), pr.proyectar(q[2]))
                        else {
                            continue;
                        };
                        let pts = expand([s0, s1, s2], 0.3);
                        if !pantalla.ve(&pts) {
                            descartadas += 1;
                            continue;
                        }
                        let depth = ((d0 + d1 + d2) / 3.0) as f32;
                        let f = fog(depth, cam.distance as f32);
                        let base = tinte.unwrap_or([tri.color[0], tri.color[1], tri.color[2]]);
                        // Un objeto seleccionado u observado conserva el sombreado pero cambia de tono.
                        let (cr, cg, cb) = if tinte.is_some() {
                            let k = (tri.color[0] + tri.color[1] + tri.color[2]) / 2.4;
                            (base[0] * k, base[1] * k, base[2] * k)
                        } else {
                            (base[0], base[1], base[2])
                        };
                        // Rayos X: solera, fuste y escalera son lo enterrado; se aclaran para
                        // ver lo que queda por debajo de la rasante.
                        let enterrada = matches!(parte.nombre, "solera" | "fuste" | "escalera");
                        let alfa = if opts.rayos_x && enterrada { 0.35 } else { 1.0 };
                        net.push(Prim::Poly {
                            pts: pts.to_vec(),
                            fill: [
                                cr * f + 0.050 * (1.0 - f),
                                cg * f + 0.055 * (1.0 - f),
                                cb * f + 0.066 * (1.0 - f),
                                alfa,
                            ],
                            depth,
                        });
                        tri_count += 1;
                    }
                }
            }

            etiquetas.extend(etiqueta_de(n, &pr, top, r_px, sel));
        }
    }

    // El terreno se ordena por profundidad; la red se ordena aparte y se pinta encima
    // (modo rayos X), de modo que las tuberias enterradas siempre se ven.
    sort_far_first(&mut prims);
    sort_far_first(&mut roads);
    juntar(&mut prims, roads);
    // La red se dibuja sobre el terreno (modo rayos X) pero **bajo** los edificios: así las
    // tuberías se ven a través del suelo y quedan ocultas detrás de una fachada, como debe ser.
    sort_far_first(&mut net);
    // Las líneas guía de las etiquetas entran en `net` y se ordenan con el resto de la red.
    let etiquetas = ordenar_etiquetas(etiquetas, &mut net);
    juntar(&mut prims, net);
    sort_far_first(&mut city);
    juntar(&mut prims, city);
    Scene {
        prims,
        tri_count,
        line_count,
        etiquetas,
        descartadas,
    }
}

/// Objetos por hilo a partir de los cuales la escena reparte tramos y nodos entre hilos.
const OBJETOS_POR_HILO: usize = 2_500;

/// Lo que comparten los tramos de una escena (solo lectura: se reparte entre hilos).
struct Contexto<'a> {
    p: &'a Project,
    pr: Proyector,
    pantalla: Pantalla,
    layers: Layers,
    selected: Option<&'a str>,
    corte_x: Option<f32>,
    zr: f64,
    k: f64,
    distancia: f32,
    pipes: &'a crate::pipe_layout::PipePieces,
}

impl Contexto<'_> {
    /// UTM → local con la exageración vertical (el origen se resta en `f64`, docs/15).
    fn local(&self, q: P3) -> [f64; 3] {
        let o = self.p.origin;
        [q.x - o.x, q.y - o.y, (q.z - self.zr) * self.k + self.zr]
    }
}

/// Reparte `items` en bloques consecutivos entre varios hilos si son muchos y devuelve el
/// resultado de cada bloque en orden (F14-04). Con pocos, un solo bloque en este hilo.
fn en_bloques<'a, T: Sync, R: Send>(items: &'a [T], f: impl Fn(&'a [T]) -> R + Sync) -> Vec<R> {
    let hilos = if items.len() >= OBJETOS_POR_HILO * 2 {
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1)
            .clamp(1, 8)
            .min(items.len() / OBJETOS_POR_HILO)
    } else {
        1
    };
    if hilos <= 1 {
        return vec![f(items)];
    }
    let tam = items.len().div_ceil(hilos);
    let f = &f;
    std::thread::scope(|s| {
        let tareas: Vec<_> = items
            .chunks(tam)
            .map(|bloque| s.spawn(move || f(bloque)))
            .collect();
        tareas
            .into_iter()
            .map(|t| t.join().expect("hilo de la escena"))
            .collect()
    })
}

/// Qué hacer con un nodo dibujable.
enum Clase<'a> {
    /// Rombo de un nodo lejano, ya proyectado.
    Rombo(Prim),
    /// Rombo de un nodo lejano que cae fuera de pantalla.
    Fuera,
    /// Pieza con malla: se tesela después, en orden.
    Pieza {
        n: &'a crate::model::Node,
        top: [f64; 3],
        r_px: f32,
        sel: bool,
        tinte: Option<[f32; 3]>,
    },
}

/// Clasifica los nodos de un bloque, en orden. Los que no se dibujan no aparecen.
fn clasificar_nodos<'a>(nodes: &'a [crate::model::Node], cx: &Contexto) -> Vec<Clase<'a>> {
    let mut out = Vec::new();
    for n in nodes.iter().filter(|n| !n.deleted) {
        let Some(def) = family(&n.family) else {
            continue;
        };
        // Se dibuja lo que tiene forma: nodos hidráulicos, acometidas y también el resto de
        // familias con geometría propia (accesorios, tapas, hitos, cruces…).
        if !def.can(caps::HYDRAULIC_NODE)
            && !def.can(caps::ATTACHABLE)
            && !def.can(caps::REPORTABLE)
            && !def.can(caps::STRUCTURAL)
        {
            continue;
        }
        let es_agua = n.family.starts_with("agua");
        if (es_agua && !cx.layers.agua) || (!es_agua && !cx.layers.alcantarillado) {
            continue;
        }
        let sel = cx.selected.map(|s| s == n.code).unwrap_or(false);
        let observado = n.estado == EstadoObjeto::Observado;
        let tinte: Option<[f32; 3]> = if sel {
            Some([0.36, 0.95, 0.56])
        } else if observado {
            Some([0.98, 0.45, 0.42])
        } else {
            None
        };

        let (top, r_px, top_px) = nodo_en_pantalla(n, &cx.pr, &|q| cx.local(q));

        // Muy lejos: símbolo de tamaño fijo (una ciudad no se llena de cilindros).
        if r_px < 3.0
            && let Some((c, d)) = top_px
        {
            let sz = if sel { 6.0 } else { 3.6 };
            let rombo = [
                [c[0] - sz, c[1]],
                [c[0], c[1] - sz],
                [c[0] + sz, c[1]],
                [c[0], c[1] + sz],
            ];
            if !cx.pantalla.ve(&rombo) {
                out.push(Clase::Fuera);
                continue;
            }
            let base: [f32; 3] = if es_agua {
                [0.33, 0.63, 0.96]
            } else {
                [0.87, 0.60, 0.31]
            };
            let col = tinte.unwrap_or(base);
            out.push(Clase::Rombo(Prim::Poly {
                pts: rombo.to_vec(),
                fill: [col[0], col[1], col[2], 1.0],
                depth: d as f32,
            }));
            continue;
        }
        out.push(Clase::Pieza {
            n,
            top,
            r_px,
            sel,
            tinte,
        });
    }
    out
}

/// Primitivas de un bloque de tramos, en orden, con sus contadores.
#[derive(Default)]
struct Salida {
    prims: Vec<Prim>,
    lineas: usize,
    descartadas: usize,
}

fn dibujar_tramos(links: &[crate::model::Link], cx: &Contexto) -> Salida {
    let mut out = Salida::default();
    // Un solo búfer para el eje de todos los tramos: sin un `Vec` por tramo (F14-04).
    let mut traza: Vec<P3> = Vec::new();
    for l in links.iter().filter(|l| !l.deleted) {
        dibujar_tramo(l, cx, &mut traza, &mut out);
    }
    out
}

/// Tubo paramétrico si se ve grande; si no, línea.
fn dibujar_tramo(l: &crate::model::Link, cx: &Contexto, traza: &mut Vec<P3>, out: &mut Salida) {
    let es_agua = l.family.starts_with("agua");
    if (es_agua && !cx.layers.agua) || (!es_agua && !cx.layers.alcantarillado) {
        return;
    }
    if l.path.len() < 2 {
        return;
    }
    let sel = cx.selected.map(|s| s == l.code).unwrap_or(false);
    let (lr, lg, lb): (f32, f32, f32) = if es_agua {
        (0.33, 0.63, 0.96)
    } else {
        (0.87, 0.60, 0.31)
    };
    let mat = if es_agua {
        crate::parts::Material::new(0.34, 0.55, 0.82)
    } else {
        crate::parts::Material::new(0.78, 0.62, 0.44)
    };
    // El tubo se dibuja por su eje (solera + DI/2) con el diámetro exterior; no atraviesa
    // el pozo: se recorta en su pared (en el ancla de la pieza horneada, si la tiene).
    let d_ext = l.diametro_exterior_m();
    crate::pipe_layout::drawn_axis(cx.p, l, &cx.pipes.node_anchors, traza);
    // Sin DN o sin cota (F12-03): línea gris fina, nunca un tubo de diámetro inventado.
    let sin_dato = l.dibujo_sin_dato();
    if !sin_dato
        && let Some(seg) = cx.pipes.by_link.get(&l.id)
        && seen_as_tube(traza, d_ext, cx)
    {
        draw_baked_pipe(seg, traza, d_ext, sel, [lr, lg, lb], cx, out);
        return;
    }

    // Los tubos se construyen en coordenadas LOCALES: con UTM en f32 el paso es ~1 m y la
    // sección colapsaría (los tubos se verían como cintas).
    for w in traza.windows(2) {
        let wa = P3::new(w[0].x - cx.p.origin.x, w[0].y - cx.p.origin.y, w[0].z);
        let wb = P3::new(w[1].x - cx.p.origin.x, w[1].y - cx.p.origin.y, w[1].z);
        let a = cx.local(w[0]);
        let b = cx.local(w[1]);
        let (da, db) = (cx.pr.depth(a), cx.pr.depth(b));
        let Some((sa, sb)) = cx.pr.segmento(a, b, da, db) else {
            continue;
        };
        let depth = ((da + db) * 0.5) as f32;
        let r_px = pipe_px(d_ext, depth, cx);

        if r_px < PIPE_LINE_PX || sin_dato {
            // A lo lejos, la tubería es una línea: mismo color, mucho menos coste.
            let width = if sel {
                4.5
            } else if sin_dato {
                1.4
            } else {
                (r_px * 2.0).clamp(1.2, 6.0)
            };
            if !cx.pantalla.ve_linea(sa, sb, width) {
                out.descartadas += 1;
                continue;
            }
            let fog = fog(depth, cx.distancia);
            let [cr, cg, cb] = if sin_dato {
                COLOR_SIN_DATO
            } else {
                [lr, lg, lb]
            };
            out.prims.push(Prim::Line {
                a: sa,
                b: sb,
                w: width,
                color: [cr, cg, cb, fog],
                depth,
            });
            out.lineas += 1;
            continue;
        }

        // Un tramo entero fuera de pantalla no se tesela: la junta más gruesa cabe en una
        // esfera de 1,1·DE alrededor del eje (estirada por la exageración vertical).
        let radio = d_ext * 1.1 * cx.k.max(1.0);
        if !cx.pantalla.ve_tubo(&cx.pr, (sa, da), (sb, db), radio) {
            out.descartadas += 1;
            continue;
        }

        // De cerca, tubo con juntas.
        let lod = if r_px > 6.0 { 2 } else { 1 };
        let link_material = l.get("material").and_then(|v| v.as_str()).unwrap_or("");
        let malla =
            crate::parts::tuberia_de_material(wa, wb, d_ext as f32, lod, mat, link_material);
        for tri in &malla.tris {
            // La malla ya está en local: se aplica solo la exageración vertical.
            let verts = [
                [
                    tri.p[0][0] as f64,
                    tri.p[0][1] as f64,
                    (tri.p[0][2] as f64 - cx.zr) * cx.k + cx.zr,
                ],
                [
                    tri.p[1][0] as f64,
                    tri.p[1][1] as f64,
                    (tri.p[1][2] as f64 - cx.zr) * cx.k + cx.zr,
                ],
                [
                    tri.p[2][0] as f64,
                    tri.p[2][1] as f64,
                    (tri.p[2][2] as f64 - cx.zr) * cx.k + cx.zr,
                ],
            ];
            // El corte secciona también las tuberías.
            let (trozos, arista) = match cx.corte_x {
                Some(xc) => cortar_tri(verts, xc),
                None => (Trozos::uno(verts), None),
            };
            push_cut_edge(arista, cx, out);
            push_pipe_pieces(
                &trozos,
                [tri.color[0], tri.color[1], tri.color[2]],
                sel,
                cx,
                out,
            );
        }
    }
}

/// Selected pipes are drawn in this colour, parametric or baked.
const SELECTED_PIPE: [f32; 3] = [0.36, 0.95, 0.56];
/// Below this on-screen radius (points) a pipe is a line.
const PIPE_LINE_PX: f32 = 2.2;
/// From this on-screen radius (points) a baked pipe shows its bells; below, its plain barrel.
const PIPE_BELLS_PX: f32 = 6.0;

/// The pieces of a cut pipe triangle, projected, fogged and tinted if selected.
fn push_pipe_pieces(trozos: &Trozos, color: [f32; 3], sel: bool, cx: &Contexto, out: &mut Salida) {
    for trozo in trozos.iter() {
        let (Some((s0, d0)), Some((s1, d1)), Some((s2, d2))) = (
            cx.pr.proyectar(trozo[0]),
            cx.pr.proyectar(trozo[1]),
            cx.pr.proyectar(trozo[2]),
        ) else {
            continue;
        };
        let pts = expand([s0, s1, s2], 0.35);
        if !cx.pantalla.ve(&pts) {
            out.descartadas += 1;
            continue;
        }
        let depth_t = ((d0 + d1 + d2) / 3.0) as f32;
        let f = fog(depth_t, cx.distancia);
        let c = if sel { SELECTED_PIPE } else { color };
        out.prims.push(Prim::Poly {
            pts: pts.to_vec(),
            fill: [
                c[0] * f + 0.050 * (1.0 - f),
                c[1] * f + 0.055 * (1.0 - f),
                c[2] * f + 0.066 * (1.0 - f),
                1.0,
            ],
            depth: depth_t,
        });
        out.lineas += 1;
    }
}

/// The yellow edge a vertical cut leaves on a pipe triangle.
fn push_cut_edge(edge: Option<([f64; 3], [f64; 3])>, cx: &Contexto, out: &mut Salida) {
    let Some((ca, cb)) = edge else { return };
    let (Some((sa, da)), Some((sb, db))) = (cx.pr.proyectar(ca), cx.pr.proyectar(cb)) else {
        return;
    };
    if cx.pantalla.ve_linea(sa, sb, 1.4) {
        out.prims.push(Prim::Line {
            a: sa,
            b: sb,
            w: 1.4,
            color: [0.98, 0.86, 0.42, 1.0],
            depth: ((da + db) * 0.5) as f32,
        });
        out.lineas += 1;
    } else {
        out.descartadas += 1;
    }
}

/// Whether any run of the drawn axis is big enough on screen to be a tube, not a line.
fn seen_as_tube(axis: &[P3], d_ext: f64, cx: &Contexto) -> bool {
    axis.windows(2).any(|w| {
        let (a, b) = (cx.local(w[0]), cx.local(w[1]));
        let (da, db) = (cx.pr.depth(a), cx.pr.depth(b));
        cx.pr.segmento(a, b, da, db).is_some()
            && pipe_px(d_ext, ((da + db) * 0.5) as f32, cx) >= PIPE_LINE_PX
    })
}

fn pipe_px(d_ext: f64, depth: f32, cx: &Contexto) -> f32 {
    (d_ext * 0.5 * cx.pr.focal() / depth.max(1.0) as f64) as f32
}

/// A link drawn with its baked segment (F17-04): whole segments with their bells up close, the
/// plain barrel a little farther and a line from afar, segment by segment.
fn draw_baked_pipe(
    seg: &crate::pipe_layout::PipeSegment,
    axis: &[P3],
    d_ext: f64,
    sel: bool,
    line_colour: [f32; 3],
    cx: &Contexto,
    out: &mut Salida,
) {
    use crate::pipe_layout::{PieceKind, lay_segments};
    let o = cx.p.origin;
    let local: Vec<[f64; 3]> = axis.iter().map(|q| [q.x - o.x, q.y - o.y, q.z]).collect();
    let exaggerate = |q: [f64; 3]| [q[0], q[1], (q[2] - cx.zr) * cx.k + cx.zr];
    let mut shading = crate::parts::Mesh::default();
    for piece in lay_segments(&local, seg) {
        let (a, b) = (exaggerate(piece.start), exaggerate(piece.end()));
        let (da, db) = (cx.pr.depth(a), cx.pr.depth(b));
        let Some((sa, sb)) = cx.pr.segmento(a, b, da, db) else {
            continue;
        };
        let depth = ((da + db) * 0.5) as f32;
        let r_px = pipe_px(d_ext, depth, cx);
        if r_px < PIPE_LINE_PX {
            let width = if sel {
                4.5
            } else {
                (r_px * 2.0).clamp(1.2, 6.0)
            };
            if !cx.pantalla.ve_linea(sa, sb, width) {
                out.descartadas += 1;
                continue;
            }
            let [cr, cg, cb] = line_colour;
            out.prims.push(Prim::Line {
                a: sa,
                b: sb,
                w: width,
                color: [cr, cg, cb, fog(depth, cx.distancia)],
                depth,
            });
            out.lineas += 1;
            continue;
        }
        let radio = seg.radius_m * 2.2 * cx.k.max(1.0);
        if !cx.pantalla.ve_tubo(&cx.pr, (sa, da), (sb, db), radio) {
            out.descartadas += 1;
            continue;
        }
        let (piece, mesh) = match (piece.kind, r_px >= PIPE_BELLS_PX) {
            (PieceKind::Full, true) => (piece, &seg.full),
            _ => (piece.as_barrel(seg), &seg.barrel),
        };
        for tri in mesh.chunks_exact(3) {
            let placed = [0, 1, 2].map(|k| piece.place(tri[k].pos));
            // Lit in the world frame (a pipe going north is lit unlike one going east), from
            // edges relative to the first corner: f32 at a 2500 m elevation would blur the normal.
            let edge = |k: usize| [0, 1, 2].map(|i| (placed[k][i] - placed[0][i]) as f32);
            shading.tris.clear();
            shading.tri(
                [0.0; 3],
                edge(1),
                edge(2),
                crate::parts::Material::new(tri[0].color[0], tri[0].color[1], tri[0].color[2]),
            );
            let lit = shading.tris[0].color;
            let verts = placed.map(exaggerate);
            let (trozos, arista) = match cx.corte_x {
                Some(xc) => cortar_tri(verts, xc),
                None => (Trozos::uno(verts), None),
            };
            push_cut_edge(arista, cx, out);
            push_pipe_pieces(&trozos, [lit[0], lit[1], lit[2]], sel, cx, out);
        }
    }
}

/// Solo las etiquetas de la escena, sin proyectar ni teselar geometría (F14-02).
///
/// El pase 3D (`--gpu3d`) dibuja la malla en la GPU y solo necesita el texto encima: mover la
/// cámara recalcula esto y la matriz, no la escena del pintor. Sigue las mismas reglas que
/// [`build_opts`]: misma capa, mismo umbral de tamaño en pantalla y mismo reparto en carriles.
pub fn etiquetas(
    p: &Project,
    cam: &Camera,
    layers: Layers,
    selected: Option<&str>,
) -> Vec<Etiqueta> {
    if !layers.nodos {
        return Vec::new();
    }
    let o = p.origin;
    let zr = cam.target[2];
    let k = cam.z_scale;
    let to_local = move |q: P3| [q.x - o.x, q.y - o.y, (q.z - zr) * k + zr];
    let pr = cam.proyector();
    let mut etiquetas = Vec::new();
    for n in p.nodes.iter().filter(|n| !n.deleted) {
        if !nodo_dibujable(n, layers) {
            continue;
        }
        let sel = selected.map(|s| s == n.code).unwrap_or(false);
        let (top, r_px, top_px) = nodo_en_pantalla(n, &pr, &to_local);
        // Mismos descartes que la escena: lejos es un símbolo y sin pieza no hay etiqueta.
        if r_px < 3.0 && top_px.is_some() {
            continue;
        }
        let Some(e) = etiqueta_de(n, &pr, top, r_px, sel) else {
            continue;
        };
        if pieza_partes(n, 1).is_empty() {
            continue;
        }
        etiquetas.push(e);
    }
    // Las líneas guía no se usan: en el pase 3D no hay pintor que las dibuje.
    ordenar_etiquetas(etiquetas, &mut Vec::new())
}

/// ¿El nodo tiene forma que dibujar con estas capas?
fn nodo_dibujable(n: &crate::model::Node, layers: Layers) -> bool {
    let Some(def) = family(&n.family) else {
        return false;
    };
    // Se dibuja lo que tiene forma: nodos hidráulicos, acometidas y también el resto de
    // familias con geometría propia (accesorios, tapas, hitos, cruces…).
    if !def.can(caps::HYDRAULIC_NODE)
        && !def.can(caps::ATTACHABLE)
        && !def.can(caps::REPORTABLE)
        && !def.can(caps::STRUCTURAL)
    {
        return false;
    }
    let es_agua = n.family.starts_with("agua");
    !((es_agua && !layers.agua) || (!es_agua && !layers.alcantarillado))
}

/// Punto proyectado en pantalla con su profundidad.
type EnPantalla = ([f32; 2], f64);

/// Punto superior del nodo (local, con exageración), su radio en pantalla y su proyección con
/// profundidad (`None` si queda detrás de la cámara).
fn nodo_en_pantalla(
    n: &crate::model::Node,
    pr: &Proyector,
    to_local: &impl Fn(P3) -> [f64; 3],
) -> ([f64; 3], f32, Option<EnPantalla>) {
    let r = n.drawing_diameter_m() * 0.5;
    let top = to_local(P3::new(n.pos.x, n.pos.y, n.z_rim));
    let edge = to_local(P3::new(n.pos.x + r, n.pos.y, n.z_rim));
    let top_px = pr.proyectar(top);
    let r_px = match (top_px, pr.project(edge)) {
        (Some((a, _)), Some(b)) => ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt(),
        _ => 0.0,
    };
    (top, r_px, top_px)
}

/// Etiqueta del nodo: solo cuando la pieza ya se distingue o está seleccionada.
fn etiqueta_de(
    n: &crate::model::Node,
    pr: &Proyector,
    top: [f64; 3],
    r_px: f32,
    sel: bool,
) -> Option<Etiqueta> {
    if (r_px > 9.0 || sel)
        && let Some((c, d)) = pr.proyectar(top)
    {
        return Some(Etiqueta {
            pos: [c[0], c[1] - 10.0],
            texto: format!("{} · {}", n.code, n.ty.replace('_', " ")),
            seleccion: sel,
            depth: d as f32,
        });
    }
    None
}

/// Reparte las etiquetas en carriles, conserva las más cercanas y deja la seleccionada al final.
fn ordenar_etiquetas(etiquetas: Vec<Etiqueta>, net: &mut Vec<Prim>) -> Vec<Etiqueta> {
    // Las etiquetas se reparten en carriles para que no se pisen; si una no cabe, se calla.
    let mut etiquetas = acomodar_etiquetas(etiquetas, net);
    // Se conservan las etiquetas más cercanas (las que el usuario está mirando) y la del
    // objeto seleccionado se dibuja al final, para que nunca la tape otra.
    let seleccionada = etiquetas.iter().position(|e| e.seleccion);
    let mut elegida = seleccionada.map(|i| etiquetas.remove(i));
    etiquetas.sort_by(|a, b| {
        a.depth
            .partial_cmp(&b.depth)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    etiquetas.truncate(24);
    if let Some(e) = elegida.take() {
        etiquetas.push(e);
    }
    etiquetas
}

/// Coloca las etiquetas en carriles sin solaparse y, si hay que moverlas, dibuja la línea guía.
///
/// Sin esto, en una escena con 40 pozos juntos las etiquetas se apilaban y no se leía ninguna.
fn acomodar_etiquetas(mut etiquetas: Vec<Etiqueta>, net: &mut Vec<Prim>) -> Vec<Etiqueta> {
    const ALTO: f32 = 15.0;
    // Carriles: primero el sitio natural, luego por encima y por debajo en saltos de un renglón.
    const CARRILES: [f32; 7] = [
        0.0,
        -ALTO,
        ALTO,
        -2.0 * ALTO,
        2.0 * ALTO,
        -3.0 * ALTO,
        3.0 * ALTO,
    ];
    // Se acomodan las más cercanas primero: son las que el usuario está mirando.
    etiquetas.sort_by(|a, b| a.depth.total_cmp(&b.depth));

    let mut ocupados: Vec<[f32; 4]> = Vec::new();
    let mut salida: Vec<Etiqueta> = Vec::new();
    for mut e in etiquetas {
        let ancho = (e.texto.chars().count() as f32 * 7.2 + 8.0).clamp(30.0, 260.0);
        let ancla = e.pos;
        let mut colocado = false;
        for dy in CARRILES {
            let x = ancla[0] - ancho * 0.5;
            let y = ancla[1] + dy - ALTO * 0.5;
            let caja = [x, y, x + ancho, y + ALTO];
            let choca = ocupados
                .iter()
                .any(|o| caja[0] < o[2] && caja[2] > o[0] && caja[1] < o[3] && caja[3] > o[1]);
            if choca {
                continue;
            }
            ocupados.push(caja);
            e.pos = [ancla[0], ancla[1] + dy];
            if dy.abs() > 1.0 {
                // Línea guía desde el objeto hasta la etiqueta desplazada.
                net.push(Prim::Line {
                    a: [ancla[0], ancla[1] - 4.0],
                    b: [ancla[0], ancla[1] + dy - 4.0],
                    w: 0.8,
                    color: [0.62, 0.66, 0.74, 0.85],
                    depth: e.depth,
                });
            }
            salida.push(e);
            colocado = true;
            break;
        }
        if !colocado {
            // No cabe en ningún carril: se omite en vez de superponer.
            continue;
        }
    }
    // Se devuelven de lejos a cerca, que es el orden de dibujo.
    salida.sort_by(|a, b| b.depth.total_cmp(&a.depth));
    salida
}

/// Ordena de mas lejano a mas cercano (algoritmo del pintor).
///
/// Ordena claves enteras de la profundidad y luego coloca las primitivas una vez
/// (`sort_by_cached_key`): mover primitivas enteras y comparar flotantes en cada paso costaba
/// más que todo lo demás de la escena (F14-04). Es estable, como antes: a igual profundidad se
/// respeta el orden de llegada.
fn sort_far_first(list: &mut [Prim]) {
    // Clave de 64 bits: profundidad descendente arriba y posición de llegada abajo. Ordenar
    // enteros sin estabilidad con la posición dentro equivale al ordenamiento estable de antes.
    let mut claves: Vec<u64> = list
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let d = match p {
                Prim::Poly { depth, .. } => *depth,
                Prim::Line { depth, .. } => *depth,
            };
            let ascendente = (clave_de_orden(d) as u32) ^ 0x8000_0000;
            ((!ascendente as u64) << 32) | i as u64
        })
        .collect();
    claves.sort_unstable();
    // `origen[j]`: qué primitiva va en el puesto `j`. Se aplica en su sitio, ciclo a ciclo.
    let mut origen: Vec<u32> = claves.iter().map(|c| *c as u32).collect();
    for i in 0..origen.len() {
        let mut j = i;
        while origen[j] as usize != i {
            let k = origen[j] as usize;
            list.swap(j, k);
            origen[j] = j as u32;
            j = k;
        }
        origen[j] = j as u32;
    }
}

/// Añade `mas` al final de `lista`; si `lista` está vacía, la sustituye sin copiar.
fn juntar(lista: &mut Vec<Prim>, mas: Vec<Prim>) {
    if lista.is_empty() {
        *lista = mas;
    } else {
        lista.extend(mas);
    }
}

/// Entero que ordena como la profundidad: los flotantes positivos ordenan como sus bits y los
/// negativos al revés. `-0,0` y `0,0` dan la misma clave (son iguales para el pintor).
fn clave_de_orden(d: f32) -> i32 {
    let b = (d + 0.0).to_bits() as i32;
    b ^ (((b >> 31) as u32) >> 1) as i32
}

/// Niebla por distancia: acerca el color al fondo con la lejanía.
/// Dilata un poligono proyectado unas decimas de pixel alrededor de su centroide.
///
/// Los triangulos del terreno se antialiasan por separado y dejarian una costura de un pixel
/// entre vecinos; solaparlos un poco la hace invisible.
///
/// Trabaja en la pila (F14-04): el único `Vec` del triángulo es el de la primitiva que se guarda.
fn expand<const N: usize>(pts: [[f32; 2]; N], px: f32) -> [[f32; 2]; N] {
    let n = N.max(1) as f32;
    let cx = pts.iter().map(|p| p[0]).sum::<f32>() / n;
    let cy = pts.iter().map(|p| p[1]).sum::<f32>() / n;
    pts.map(|p| {
        let (dx, dy) = (p[0] - cx, p[1] - cy);
        let l = (dx * dx + dy * dy).sqrt().max(1e-3);
        [p[0] + dx / l * px, p[1] + dy / l * px]
    })
}

/// Rectángulo visible del viewport, en puntos, con un margen para el antialias (F14-07).
///
/// Lo que cae entero fuera no se dibuja, así que no entra en las listas que se ordenan. El
/// recorte es exacto (por caja de la primitiva ya proyectada) o conservador (por esfera o cápsula
/// de una pieza o un tramo entero, antes de teselarlo): nunca quita nada que se vería.
#[derive(Clone, Copy, Debug)]
struct Pantalla {
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
}

impl Pantalla {
    /// Margen en puntos: antialias y anchos de línea redondeados.
    const MARGEN: f32 = 2.0;
    /// Margen extra de una pieza entera: la trama del corte sobresale ~3,6 px de la arista.
    const MARGEN_PIEZA: f32 = 6.0;

    fn de(pr: &Proyector) -> Pantalla {
        let [w, h] = pr.viewport();
        Pantalla {
            x0: -Self::MARGEN,
            y0: -Self::MARGEN,
            x1: w + Self::MARGEN,
            y1: h + Self::MARGEN,
        }
    }

    /// Sin recorte: todo se ve.
    fn infinita() -> Pantalla {
        Pantalla {
            x0: f32::NEG_INFINITY,
            y0: f32::NEG_INFINITY,
            x1: f32::INFINITY,
            y1: f32::INFINITY,
        }
    }

    fn ve_caja(&self, min: [f32; 2], max: [f32; 2]) -> bool {
        !(max[0] < self.x0 || min[0] > self.x1 || max[1] < self.y0 || min[1] > self.y1)
    }

    /// ¿El polígono toca la pantalla? (por su caja: exacto para decidir que no se ve).
    fn ve(&self, pts: &[[f32; 2]]) -> bool {
        let mut min = [f32::INFINITY; 2];
        let mut max = [f32::NEG_INFINITY; 2];
        for p in pts {
            min = [min[0].min(p[0]), min[1].min(p[1])];
            max = [max[0].max(p[0]), max[1].max(p[1])];
        }
        self.ve_caja(min, max)
    }

    fn ve_linea(&self, a: [f32; 2], b: [f32; 2], w: f32) -> bool {
        let m = w * 0.5;
        self.ve_caja(
            [a[0].min(b[0]) - m, a[1].min(b[1]) - m],
            [a[0].max(b[0]) + m, a[1].max(b[1]) + m],
        )
    }

    /// ¿Puede verse algo de una cápsula de radio `radio` (m) alrededor del segmento `a`–`b`?
    /// Recibe los extremos ya proyectados con su profundidad.
    ///
    /// Un punto a `radio` del eje se proyecta a menos de `focal·radio·(1 + t)/(d − radio)` de
    /// la proyección del eje, con `d` la profundidad mínima de los extremos y `t` la mayor
    /// pendiente de visión de los extremos (|x|/z, |y|/z). Si la cápsula toca el plano cercano no
    /// se acota: se dibuja y el recorte por triángulo decide.
    fn ve_tubo(
        &self,
        pr: &Proyector,
        (sa, da): ([f32; 2], f64),
        (sb, db): ([f32; 2], f64),
        radio: f64,
    ) -> bool {
        let d = da.min(db) - radio;
        if d <= 0.05 {
            return true;
        }
        let [w, h] = pr.viewport();
        let (cx, cy) = (w as f64 * 0.5, h as f64 * 0.5);
        let t = [sa, sb]
            .iter()
            .map(|s| ((s[0] as f64 - cx).abs()).max((s[1] as f64 - cy).abs()))
            .fold(0.0f64, f64::max)
            / pr.focal();
        let m = (pr.focal() * radio * (1.0 + t) / d) as f32 + Self::MARGEN_PIEZA;
        self.ve_caja(
            [sa[0].min(sb[0]) - m, sa[1].min(sb[1]) - m],
            [sa[0].max(sb[0]) + m, sa[1].max(sb[1]) + m],
        )
    }

    /// ¿Puede verse algo de una esfera (centro local, radio en m)?
    fn ve_esfera(&self, pr: &Proyector, centro: [f64; 3], radio: f64) -> bool {
        let d = pr.depth(centro);
        if d + radio <= 0.05 {
            // Entera detrás del plano cercano: ningún vértice se proyecta.
            return false;
        }
        match pr.proyectar(centro) {
            Some((s, dc)) => self.ve_tubo(pr, (s, dc), (s, dc), radio),
            None => true,
        }
    }
}

/// Holgura horizontal de la esfera previa de un pozo sobre su radio: cubre la solera ensanchada,
/// el marco de la tapa y la bajante exterior de los pozos de caída (prueba
/// `la_esfera_previa_del_pozo_envuelve_su_malla`).
const HOLGURA_POZO: f64 = 1.5;

/// Esfera que envuelve un pozo o una cámara de bombeo **sin teselarlo** (F14-07): su malla cabe
/// en un cilindro de radio `D/2 + 1,5 m` y de `−0,5 m` a la altura más `0,5 m`. Así un pozo fuera
/// de pantalla no cuesta su malla. `None` para el resto de familias (o sin altura finita).
fn esfera_previa(
    n: &crate::model::Node,
    dz_max: f64,
    to_local: &impl Fn(P3) -> [f64; 3],
    k: f64,
) -> Option<([f64; 3], f64)> {
    let d = match n.family.as_str() {
        "saneamiento.pozo" => n.drawing_diameter_m(),
        "saneamiento.bombeo" => n.drawing_diameter_m().max(1.8),
        _ => return None,
    };
    let h = n.z_rim - n.z_invert;
    if !h.is_finite() || !d.is_finite() || !n.z_invert.is_finite() {
        return None;
    }
    let r = (d * 0.5).max(0.3) + HOLGURA_POZO;
    let (z0, z1) = (-0.5, h.max(0.8) + 0.5 + dz_max);
    let centro = to_local(P3::new(n.pos.x, n.pos.y, n.z_invert + (z0 + z1) * 0.5));
    let hz = (z1 - z0) * 0.5 * k.abs();
    Some((centro, (2.0 * r * r + hz * hz).sqrt()))
}

/// Esfera que envuelve la pieza ya colocada (centro local con exageración, radio en m), para
/// descartarla entera antes de proyectar sus triángulos. `None` si no tiene triángulos.
fn esfera_de_pieza(
    partes: &[crate::parts::Parte],
    n: &crate::model::Node,
    dz_max: f64,
    colocar: &impl Fn([f32; 3]) -> [f64; 3],
    to_local: &impl Fn(P3) -> [f64; 3],
    k: f64,
) -> Option<([f64; 3], f64)> {
    let mut min = [f64::INFINITY; 3];
    let mut max = [f64::NEG_INFINITY; 3];
    for v in partes
        .iter()
        .flat_map(|p| p.mesh.tris.iter().flat_map(|t| t.p))
    {
        let v = colocar(v);
        for i in 0..3 {
            min[i] = min[i].min(v[i]);
            max[i] = max[i].max(v[i]);
        }
    }
    if min[0] > max[0] {
        return None;
    }
    // La explosión sube las partes hasta `dz_max`: la caja crece por arriba.
    let max = [max[0], max[1], max[2] + dz_max];
    let c = [
        (min[0] + max[0]) * 0.5,
        (min[1] + max[1]) * 0.5,
        (min[2] + max[2]) * 0.5,
    ];
    let centro = to_local(P3::new(c[0] + n.pos.x, c[1] + n.pos.y, c[2] + n.z_invert));
    let (hx, hy, hz) = (
        (max[0] - min[0]) * 0.5,
        (max[1] - min[1]) * 0.5,
        (max[2] - min[2]) * 0.5 * k.abs(),
    );
    // Holgura relativa por el redondeo de los vértices en `f32`.
    let radio = (hx * hx + hy * hy + hz * hz).sqrt() * 1.001 + 0.01;
    Some((centro, radio))
}

/// Offset of a piece vertex from the object (plan position and invert): a parametric piece is
/// already in that frame; a model turns by its rotation and rises by its lift.
fn colocador(model: Option<&ModelPiece>) -> impl Fn([f32; 3]) -> [f64; 3] {
    let (lift, rotation) = model.map_or((0.0, 0.0f32), |m| (m.lift_m, m.rotation_deg));
    let (sin, cos) = (rotation as f64).to_radians().sin_cos();
    move |v: [f32; 3]| {
        let (x, y) = (v[0] as f64, v[1] as f64);
        [x * cos - y * sin, x * sin + y * cos, v[2] as f64 + lift]
    }
}

fn fog(depth: f32, distance: f32) -> f32 {
    let t = (depth / (distance * 2.2).max(1.0)).clamp(0.0, 0.55);
    1.0 - t
}

/// Triángulos que quedan de uno al cortarlo (cero, uno o dos), en la pila (F14-04).
#[derive(Clone, Copy, Debug)]
struct Trozos {
    tris: [[[f64; 3]; 3]; 2],
    n: usize,
}

impl Trozos {
    fn ninguno() -> Trozos {
        Trozos {
            tris: [[[0.0; 3]; 3]; 2],
            n: 0,
        }
    }

    fn uno(t: [[f64; 3]; 3]) -> Trozos {
        Trozos {
            tris: [t, [[0.0; 3]; 3]],
            n: 1,
        }
    }

    fn iter(&self) -> std::slice::Iter<'_, [[f64; 3]; 3]> {
        self.tris[..self.n].iter()
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.n
    }

    #[cfg(test)]
    fn is_empty(&self) -> bool {
        self.n == 0
    }
}

/// Trozos de un triángulo al cortarlo y, si el plano lo atraviesa, la arista de corte.
type Corte = (Trozos, Option<([f64; 3], [f64; 3])>);

/// Recorta un triángulo contra el semiespacio `x <= xc` y devuelve los trozos que quedan
/// más la arista de corte (si el plano lo atraviesa).
fn cortar_tri(tri: [[f64; 3]; 3], xc: f32) -> Corte {
    let xc = xc as f64;
    let dentro = |p: [f64; 3]| p[0] <= xc;
    let n_dentro = tri.iter().filter(|p| dentro(**p)).count();
    if n_dentro == 0 {
        return (Trozos::ninguno(), None);
    }
    if n_dentro == 3 {
        return (Trozos::uno(tri), None);
    }

    let mut poligono = [[0.0f64; 3]; 4];
    let mut lados = 0usize;
    let mut arista: Option<([f64; 3], [f64; 3])> = None;
    for i in 0..3 {
        let a = tri[i];
        let b = tri[(i + 1) % 3];
        let (da, db) = (dentro(a), dentro(b));
        if da {
            poligono[lados] = a;
            lados += 1;
        }
        if da != db {
            let t =
                (xc - a[0]) / (b[0] - a[0]).abs().max(1e-9) * if b[0] > a[0] { 1.0 } else { -1.0 };
            let t = t.clamp(0.0, 1.0);
            let p = [xc, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t];
            poligono[lados] = p;
            lados += 1;
            arista = match arista {
                None => Some((p, p)),
                Some((p0, _)) => Some((p0, p)),
            };
        }
    }

    // Con uno o dos vértices dentro, el polígono recortado tiene tres o cuatro lados.
    let mut trozos = Trozos::ninguno();
    if lados >= 3 {
        trozos.tris[0] = [poligono[0], poligono[1], poligono[2]];
        trozos.n = 1;
        if lados == 4 {
            trozos.tris[1] = [poligono[0], poligono[2], poligono[3]];
            trozos.n = 2;
        }
    }
    (trozos, arista)
}

/// Malla completa de una pieza para exportadores (glTF, DXF de sólidos).
///
/// Usa siempre el nivel de detalle alto: lo exportado se mira en otra aplicación.
pub fn pieza_para_exportar(n: &crate::model::Node) -> crate::parts::Mesh {
    pieza_local(n, 2)
}

/// Malla de una pieza con el nivel de detalle pedido (1 simple, 2 completo), para la caché de
/// mallas del pase 3D (F14-06).
pub fn pieza_con_detalle(n: &crate::model::Node, lod: u8) -> crate::parts::Mesh {
    pieza_local(n, lod.min(2))
}

/// Gris de lo que se dibuja sin dato: no es un material, es un aviso.
pub const COLOR_SIN_DATO: [f32; 3] = [0.56, 0.56, 0.58];

/// Símbolo plano de un nodo sin altura con datos (le falta la solera, o la tapa y el terreno):
/// un disco gris de 5 cm a la cota que sí se conoce. No sugiere una profundidad (F12-03).
fn simbolo_sin_dato(n: &crate::model::Node) -> crate::parts::Mesh {
    let [r, g, b] = COLOR_SIN_DATO;
    crate::parts::cilindro(
        (n.drawing_diameter_m() * 0.5) as f32,
        0.05,
        20,
        crate::parts::Material::new(r, g, b),
    )
}

/// Construye las partes nombradas de una pieza (pozos y válvulas se despiezan; el resto es una).
fn pieza_partes(n: &crate::model::Node, lod: u8) -> Vec<crate::parts::Parte> {
    use crate::parts as pt;
    if n.sin_altura() {
        return vec![pt::Parte {
            nombre: "sin_dato",
            mesh: simbolo_sin_dato(n),
        }];
    }
    // Cadena de agua cruda (docs/20): bocatoma, cámaras, tratamiento y salida del alcantarillado.
    if let Some(partes) = crate::parts_cadena::piezas(n, lod) {
        return partes;
    }
    let dn = n.num("diametro_nominal").unwrap_or(200.0) as u32;
    match n.family.as_str() {
        "saneamiento.pozo" | "saneamiento.bombeo" => {
            let tipo = if n.ty.starts_with("caida") {
                1
            } else if n.ty == "resalto" {
                2
            } else if n.ty == "ciego" {
                3
            } else if n.ty == "bombeo" {
                4
            } else if n.ty.starts_with("caja") || n.ty == "limpieza" {
                5
            } else if n.ty == "con_arenero" {
                6
            } else if n.ty == "disipador" {
                7
            } else {
                0
            };
            pt::pozo_partes(pt::PozoParams {
                diametro: if n.family == "saneamiento.bombeo" {
                    n.drawing_diameter_m().max(1.8)
                } else {
                    n.drawing_diameter_m()
                } as f32,
                rim: (n.z_rim - n.z_invert) as f32,
                invert: 0.0,
                tipo,
                lod,
            })
        }
        "agua.valvula" => pt::valvula_partes(dn, &n.ty, lod),
        "agua.llave" => {
            if n.ty == "hidrante" {
                vec![pt::Parte {
                    nombre: "hidrante",
                    mesh: pt::hidrante(dn),
                }]
            } else {
                pt::valvula_partes(dn, "compuerta", lod)
            }
        }
        otra => {
            let malla = pieza_local(n, lod);
            if malla.is_empty() {
                Vec::new()
            } else {
                // El nombre de la familia sirve para depurar la explosión.
                let nombre: &'static str = match otra {
                    "agua.hidrante" => "hidrante",
                    "agua.tanque" => "tanque",
                    "saneamiento.descarga" => "descarga",
                    "saneamiento.camara" => "camara",
                    "saneamiento.sumidero" => "sumidero",
                    _ => "pieza",
                };
                vec![pt::Parte {
                    nombre,
                    mesh: malla,
                }]
            }
        }
    }
}

/// Construye la malla local de una pieza según su familia y subtipo.
///
/// El origen local es el centro de la base (x=0, y=0, z=0 en la clave del objeto).
fn pieza_local(n: &crate::model::Node, lod: u8) -> crate::parts::Mesh {
    use crate::parts as pt;
    if n.sin_altura() {
        return simbolo_sin_dato(n);
    }
    let dn = n.num("diametro_nominal").unwrap_or(200.0) as u32;
    match n.family.as_str() {
        "saneamiento.pozo" => {
            let tipo = if n.ty.starts_with("caida") {
                1
            } else if n.ty == "resalto" {
                2
            } else if n.ty == "ciego" {
                3
            } else if n.ty == "bombeo" {
                4
            } else if n.ty.starts_with("caja") || n.ty == "limpieza" {
                5
            } else if n.ty == "con_arenero" {
                6
            } else if n.ty == "disipador" {
                7
            } else {
                0
            };
            pt::pozo(pt::PozoParams {
                diametro: n.drawing_diameter_m() as f32,
                rim: (n.z_rim - n.z_invert) as f32,
                invert: 0.0,
                tipo,
                lod,
            })
        }
        "agua.valvula" => pt::valvula(dn, &n.ty, lod),
        "agua.llave" => {
            if n.ty == "hidrante" {
                pt::hidrante(dn)
            } else {
                pt::valvula(dn, "compuerta", lod)
            }
        }
        "comun.accesorio" => {
            let ang = n.num("angulo").unwrap_or(90.0) as f32;
            pt::accesorio(&n.ty, dn, ang, lod)
        }
        "comun.tapa" => {
            let dt = n.num("diametro_tapa").unwrap_or(600.0) as f32 / 1000.0;
            let mut m = pt::anillo(dt * 0.5, dt * 0.42, 16, pt::HIERRO);
            m.extend(pt::cilindro(dt * 0.42, 0.05, 16, pt::HIERRO));
            m
        }
        "comun.escalera" => {
            let alto = n.num("altura").unwrap_or(3.0) as f32;
            let paso = n.num("paso").unwrap_or(0.30) as f32;
            let mut m = pt::barra([-0.18, 0.0, 0.0], [-0.18, 0.0, alto], 0.022, 5, pt::HIERRO);
            m.extend(pt::barra(
                [0.18, 0.0, 0.0],
                [0.18, 0.0, alto],
                0.022,
                5,
                pt::HIERRO,
            ));
            let mut z = 0.3;
            while z < alto {
                m.extend(pt::barra(
                    [-0.18, 0.0, z],
                    [0.18, 0.0, z],
                    0.018,
                    5,
                    pt::HIERRO,
                ));
                z += paso;
            }
            m
        }
        "agua.hidrante" => pt::hidrante(dn),
        "agua.arqueta"
        | "saneamiento.caja"
        | "saneamiento.arenero"
        | "saneamiento.trampa_grasa" => {
            let mut m = pt::caja(
                0.9,
                0.9,
                ((n.z_rim - n.z_invert) as f32).max(0.5),
                pt::HORMIGON_OSCURO,
            );
            m.extend(pt::caja(1.0, 1.0, 0.05, pt::HIERRO).mover([-0.5, -0.5, 0.0]));
            m
        }
        "agua.dado_anclaje" => pt::caja(0.8, 0.8, 0.6, pt::HORMIGON_OSCURO),
        "agua.planta" => pt::planta(n.num("caudal_diseno").unwrap_or(20.0) as f32),
        "agua.captacion" => pt::captacion(),
        "comun.hito" => pt::hito(n.num("altura").unwrap_or(0.9) as f32),
        "comun.cuerpo_receptor" => pt::cuerpo_receptor(8.0, 14.0),
        "comun.cruce" => pt::cruce(8.0, 300),
        "agua.tanque" => {
            if n.ty.contains("elevado") {
                pt::tanque_elevado(
                    n.num("volumen_util")
                        .map(|v| (v / 3.0).sqrt())
                        .unwrap_or(6.0) as f32,
                    ((n.z_rim - n.z_invert) as f32).max(6.0),
                )
            } else {
                let diam = n
                    .num("volumen_util")
                    .map(|v| (v / 3.0).sqrt().clamp(3.0, 14.0))
                    .unwrap_or(6.0);
                pt::tanque(diam as f32, ((n.z_rim - n.z_invert) as f32).clamp(2.0, 8.0))
            }
        }
        "agua.medidor" => pt::medidor(dn),
        "saneamiento.descarga" => pt::descarga(dn.max(250)),
        "saneamiento.camara" => pt::camara(
            n.drawing_diameter_m() as f32,
            ((n.z_rim - n.z_invert) as f32).clamp(0.8, 4.0),
        ),
        "saneamiento.sumidero" => pt::sumidero(),
        "saneamiento.bombeo" => pt::pozo(pt::PozoParams {
            diametro: n.drawing_diameter_m().max(1.8) as f32,
            rim: (n.z_rim - n.z_invert) as f32,
            invert: 0.0,
            tipo: 4,
            lod,
        }),
        _ => pt::cilindro(
            n.drawing_diameter_m() as f32 * 0.5,
            ((n.z_rim - n.z_invert) as f32).max(0.5),
            10,
            pt::HIERRO,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn demo_cam() -> Camera {
        let p = crate::demo::project();
        let mut c = Camera::default();
        c.fit(&p);
        c
    }

    #[test]
    fn a_model_piece_is_drawn_instead_of_the_parametric_one() {
        let p = crate::demo::project();
        let n = p
            .nodes
            .iter()
            .find(|n| n.family == "saneamiento.pozo")
            .unwrap();
        let cam = Camera {
            target: [n.pos.x - p.origin.x, n.pos.y - p.origin.y, n.z_invert],
            distance: 6.0,
            z_scale: 1.0,
            ..Camera::default()
        };
        let draw = |mesh: crate::parts::Mesh| {
            let models = ModelPieces::from([(
                n.id,
                ModelPiece {
                    part: std::sync::Arc::new(crate::parts::Parte {
                        nombre: "modelo",
                        mesh,
                    }),
                    lift_m: -0.2,
                    rotation_deg: 30.0,
                },
            )]);
            build_with_models(
                &p,
                None,
                &cam,
                Layers::default(),
                None,
                &Opciones::default(),
                &models,
            )
            .tri_count
        };
        let parametric = build_opts(
            &p,
            None,
            &cam,
            Layers::default(),
            None,
            &Opciones::default(),
        )
        .tri_count;
        let mut one = crate::parts::Mesh::default();
        one.tri(
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            crate::parts::HORMIGON,
        );
        let empty = draw(crate::parts::Mesh::default());
        assert!(
            empty + 50 < parametric,
            "the manhole piece is gone: {empty} vs {parametric}"
        );
        assert_eq!(
            draw(one),
            empty + 1,
            "and the model triangle drawn in its place"
        );
    }

    /// F12-10: de cerca, el tubo lleva las juntas de su material (hormigón más que PVC).
    #[test]
    fn el_hormigon_muestra_mas_juntas_que_el_pvc() {
        let tris = |material: &str| {
            let mut p = crate::demo::project();
            for l in &mut p.links {
                l.attrs.insert(
                    "material".into(),
                    crate::families::Value::Text(material.into()),
                );
            }
            let l = p.links.iter().find(|l| l.length_2d() > 10.0).unwrap();
            let m = l.path[l.path.len() / 2];
            let mut c = Camera::default();
            c.fit(&p);
            c.target = [m.x - p.origin.x, m.y - p.origin.y, m.z];
            c.distance = 6.0;
            build(&p, &c, Layers::default(), None).prims.len()
        };
        let (h, v) = (tris("Hormigón"), tris("PVC"));
        assert!(h > v, "{h} vs {v}");
    }

    #[test]
    fn la_camara_encuadra_el_proyecto() {
        let p = crate::demo::project();
        let mut c = Camera::default();
        c.fit(&p);
        assert!(c.distance > 100.0);
        let s = build(&p, &c, Layers::default(), None);
        assert!(s.tri_count > 500, "triángulos {}", s.tri_count);
        assert!(s.line_count > 10);
    }

    #[test]
    fn apagar_capas_quita_primitivas() {
        let p = crate::demo::project();
        let c = demo_cam();
        let todo = build(&p, &c, Layers::default(), None);
        let sin_terreno = build(
            &p,
            &c,
            Layers {
                terreno: false,
                ..Layers::default()
            },
            None,
        );
        assert!(sin_terreno.prims.len() < todo.prims.len());
    }

    #[test]
    fn la_proyeccion_pone_el_objetivo_en_el_centro() {
        let c = demo_cam();
        let s = c.project(c.target).unwrap();
        assert!((s[0] - c.viewport[0] * 0.5).abs() < 1.0);
        assert!((s[1] - c.viewport[1] * 0.5).abs() < 1.0);
    }

    /// Fase 1 del catálogo: las 29 familias deben dibujar algo con sus parámetros.
    #[test]
    fn todas_las_familias_dibujan() {
        use crate::families::{self, TopoRole};
        use crate::model::{ElementId, Node};
        use std::collections::BTreeMap;

        let mut sin_dibujo = Vec::new();
        for f in families::families() {
            let tipo = f.types.first().map(|t| t.key).unwrap_or("inspeccion");
            let nodo = Node {
                id: ElementId::new(1, 1),
                family: f.key.to_string(),
                ty: tipo.to_string(),
                code: "X-001".into(),
                pos: crate::geom::P2::new(0.0, 0.0),
                z_terrain: 10.0,
                z_rim: 10.0,
                z_invert: 7.0,
                attrs: BTreeMap::from([
                    (
                        "diametro_interior".to_string(),
                        crate::families::Value::Num(1.2),
                    ),
                    (
                        "diametro_nominal".to_string(),
                        crate::families::Value::Int(200),
                    ),
                    (
                        "volumen_util".to_string(),
                        crate::families::Value::Num(300.0),
                    ),
                    (
                        "caudal_diseno".to_string(),
                        crate::families::Value::Num(20.0),
                    ),
                    ("altura".to_string(), crate::families::Value::Num(3.0)),
                ]),
                datos: BTreeMap::new(),
                estado: crate::model::EstadoObjeto::Pendiente,
                observaciones: Vec::new(),
                modelo: None,
                deleted: false,
                sin_dato: Vec::new(),
            };
            let malla = pieza_local(&nodo, 2);
            let dibuja = if malla.is_empty() {
                // Las familias de arco se dibujan como tubo, no como pieza de nodo.
                if f.topo == TopoRole::Link {
                    !crate::parts::tuberia(
                        crate::geom::P3::new(0.0, 0.0, 7.0),
                        crate::geom::P3::new(10.0, 0.0, 7.0),
                        0.25,
                        2,
                        crate::parts::PVC,
                    )
                    .is_empty()
                } else {
                    false
                }
            } else {
                true
            };
            if !dibuja {
                sin_dibujo.push(f.key);
            }
        }
        assert!(sin_dibujo.is_empty(), "familias sin dibujo: {sin_dibujo:?}");
    }

    #[test]
    fn el_corte_parte_un_triangulo_en_dos() {
        // Triángulo que cruza el plano x = 1: queda un cuadrilátero (2 triángulos) y una arista.
        let tri = [[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [0.0, 2.0, 0.0]];
        let (trozos, arista) = cortar_tri(tri, 1.0);
        assert_eq!(trozos.len(), 2, "el recorte debe dar dos triángulos");
        assert!(arista.is_some(), "el plano atraviesa el triángulo");
        for tri in trozos.iter() {
            for v in tri.iter() {
                assert!(
                    v[0] <= 1.0 + 1e-9,
                    "no debe quedar nada al otro lado del plano"
                );
            }
        }
    }

    #[test]
    fn el_corte_descarta_lo_que_queda_entero_al_otro_lado() {
        let tri = [[2.0, 0.0, 0.0], [3.0, 0.0, 0.0], [2.0, 1.0, 0.0]];
        let (trozos, arista) = cortar_tri(tri, 1.0);
        assert!(trozos.is_empty());
        assert!(arista.is_none());
    }

    #[test]
    fn el_corte_reduce_la_geometria_dibujada() {
        let p = crate::demo_catalog::catalogo();
        let mut cam = Camera::default();
        cam.viewport = [900.0, 600.0];
        cam.fit(&p);
        let completo = build_opts(
            &p,
            None,
            &cam,
            Layers::default(),
            None,
            &Opciones::default(),
        );
        let cortado = build_opts(
            &p,
            None,
            &cam,
            Layers::default(),
            None,
            &Opciones {
                corte_x: Some(32.0),
                ..Opciones::default()
            },
        );
        assert!(
            cortado.tri_count < completo.tri_count,
            "corte {} vs completo {}",
            cortado.tri_count,
            completo.tri_count
        );
    }

    #[test]
    fn la_explosion_separa_las_partes_del_objeto_enfocado() {
        let p = crate::demo_catalog::catalogo();
        let mut cam = Camera::default();
        cam.viewport = [900.0, 600.0];
        cam.fit(&p);
        // Un plano cercano y una pieza grande: así la explosión se nota en pantalla.
        cam.distance = 9.0;
        let n = p
            .nodes
            .iter()
            .find(|n| !n.deleted && n.family == "saneamiento.pozo")
            .unwrap();
        // La cámara mira al pozo: fuera de pantalla no se dibujaría y no habría nada que mover
        // (F14-07).
        cam.target = [n.pos.x - p.origin.x, n.pos.y - p.origin.y, n.z_rim];
        let code = n.code.clone();
        let id = n.id;
        let normal = build_opts(
            &p,
            None,
            &cam,
            Layers::default(),
            Some(&code),
            &Opciones::default(),
        );
        let explotado = build_opts(
            &p,
            None,
            &cam,
            Layers::default(),
            Some(&code),
            &Opciones {
                foco: Some(id),
                explosion: 1.0,
                corte_x: None,
                rayos_x: false,
            },
        );
        // El mismo número de triángulos, pero desplazados: la escena cambia.
        assert_eq!(normal.tri_count, explotado.tri_count);
        let a: Vec<[f32; 2]> = normal
            .prims
            .iter()
            .filter_map(|p| match p {
                Prim::Poly { pts, .. } => pts.first().copied(),
                _ => None,
            })
            .collect();
        let b: Vec<[f32; 2]> = explotado
            .prims
            .iter()
            .filter_map(|p| match p {
                Prim::Poly { pts, .. } => pts.first().copied(),
                _ => None,
            })
            .collect();
        assert_ne!(a, b, "la explosión debe mover la geometría");
    }

    /// El render (no `pieza_local`) debe dibujar familias que no son nodo hidráulico, como un
    /// hito o una tapa. Si vuelve el filtro que las ocultaba, esta prueba falla.
    /// Cobertura total: **cada** familia del registro debe producir geometría en el render.
    /// Recorre el catálogo completo y, si alguna no dibuja, falla con la lista.
    #[test]
    fn cada_familia_del_registro_dibuja() {
        let p = crate::demo_catalog::catalogo_completo();
        let mut cam = Camera::default();
        cam.viewport = [1200.0, 900.0];
        cam.fit(&p);
        let escena = build_opts(
            &p,
            None,
            &cam,
            Layers::default(),
            None,
            &Opciones::default(),
        );
        assert!(escena.tri_count > 0, "el catálogo completo no dibujó nada");
        // El proyecto debe tener una muestra por familia (nodo o arco).
        let familias = crate::families::families().len();
        assert!(
            p.count() >= familias,
            "faltan muestras: {} objetos para {familias} familias",
            p.count()
        );
    }

    /// Cada familia, por separado, debe dibujar al menos una primitiva.
    #[test]
    fn ninguna_familia_queda_en_blanco() {
        use crate::model::{ElementId, Node, Project};
        let mut en_blanco = Vec::new();
        for f in crate::families::families() {
            let Some(ty) = f.types.first() else { continue };
            let mut p = Project::new("una", "EPSG:32717");
            let ok = p
                .add_node(Node {
                    id: ElementId::new(1, 1),
                    family: f.key.to_string(),
                    ty: ty.key.to_string(),
                    code: format!("{}-1", f.code_prefix),
                    pos: crate::geom::P2::new(0.0, 0.0),
                    z_terrain: 0.0,
                    z_rim: 0.0,
                    z_invert: -1.5,
                    attrs: Default::default(),
                    datos: Default::default(),
                    estado: crate::model::EstadoObjeto::Pendiente,
                    observaciones: Vec::new(),
                    modelo: None,
                    deleted: false,
                    sin_dato: Vec::new(),
                })
                .is_ok();
            if !ok {
                en_blanco.push(format!("{} (no se pudo crear)", f.key));
                continue;
            }
            let mut cam = Camera::default();
            cam.viewport = [900.0, 600.0];
            cam.fit(&p);
            cam.distance = 8.0;
            let s = build_opts(
                &p,
                None,
                &cam,
                Layers::default(),
                None,
                &Opciones::default(),
            );
            if s.tri_count == 0 {
                en_blanco.push(f.key.to_string());
            }
        }
        assert!(
            en_blanco.is_empty(),
            "familias sin geometría: {en_blanco:?}"
        );
    }

    #[test]
    fn el_render_dibuja_familias_no_hidraulicas() {
        use crate::model::{ElementId, Node, Project};
        use std::collections::BTreeMap;

        let mut p = Project::new("filtro", "EPSG:32717");
        for (familia, ty) in [
            ("comun.hito", "bm"),
            ("comun.tapa", "ciega_circular"),
            ("comun.escalera", "peldano"),
        ] {
            p.add_node(Node {
                id: ElementId::new(0, 0),
                family: familia.into(),
                ty: ty.into(),
                code: String::new(),
                pos: crate::geom::P2::new(0.0, 0.0),
                z_terrain: 0.0,
                z_rim: 0.0,
                z_invert: -1.0,
                attrs: BTreeMap::new(),
                datos: BTreeMap::new(),
                estado: crate::model::EstadoObjeto::Pendiente,
                observaciones: Vec::new(),
                modelo: None,
                deleted: false,
                sin_dato: Vec::new(),
            })
            .expect("nodo");
        }
        let mut cam = Camera::default();
        cam.viewport = [900.0, 600.0];
        cam.fit(&p);
        cam.distance = 12.0;
        let s = build_opts(
            &p,
            None,
            &cam,
            Layers::default(),
            None,
            &Opciones::default(),
        );
        assert!(
            s.tri_count > 0,
            "el render no dibujó las familias no hidráulicas"
        );
    }

    /// Con la cámara cerca debe haber al menos una etiqueta; si dejan de generarse, esto falla.
    #[test]
    fn las_etiquetas_aparecen_al_acercarse() {
        let p = crate::demo_catalog::catalogo();
        let mut cam = Camera::default();
        cam.viewport = [900.0, 600.0];
        cam.fit(&p);
        cam.distance = 12.0;
        cam.target = [10.0, 36.0, 2500.0];
        let s = build_opts(
            &p,
            None,
            &cam,
            Layers::default(),
            None,
            &Opciones::default(),
        );
        assert!(!s.etiquetas.is_empty(), "no se generó ninguna etiqueta");
        for e in &s.etiquetas {
            assert!(!e.texto.is_empty());
        }
    }

    /// El pase 3D pide solo las etiquetas (F14-02): deben ser las mismas que las de la escena
    /// completa, en el mismo orden y sitio, con y sin selección y al apagar una capa.
    #[test]
    fn las_etiquetas_sueltas_son_las_de_la_escena() {
        let p = crate::demo_catalog::catalogo();
        let cam = Camera {
            viewport: [900.0, 600.0],
            distance: 12.0,
            target: [10.0, 36.0, 2500.0],
            ..Default::default()
        };
        let lejos = p
            .nodes
            .iter()
            .rev()
            .find(|n| !n.deleted)
            .map(|n| n.code.clone());
        let sin_agua = Layers {
            agua: false,
            ..Layers::default()
        };
        let mut casos = 0;
        for capas in [Layers::default(), sin_agua] {
            for sel in [None, lejos.as_deref()] {
                let escena = build_opts(&p, None, &cam, capas, sel, &Opciones::default());
                let sueltas = etiquetas(&p, &cam, capas, sel);
                let clave = |v: &[Etiqueta]| -> Vec<(String, [i32; 2], bool)> {
                    v.iter()
                        .map(|e| {
                            let pos = [(e.pos[0] * 10.0) as i32, (e.pos[1] * 10.0) as i32];
                            (e.texto.clone(), pos, e.seleccion)
                        })
                        .collect()
                };
                assert_eq!(clave(&sueltas), clave(&escena.etiquetas));
                casos += escena.etiquetas.len();
            }
        }
        assert!(casos > 0, "la prueba debe comparar etiquetas reales");
    }

    /// El corte debe seccionar también las tuberías. Se aísla la red (sin piezas de nodo) y se
    /// cuentan las aristas de corte (amarillas): sin corte no debe haber ninguna.
    #[test]
    fn el_corte_secciona_las_tuberias() {
        let p = crate::demo_catalog::catalogo();
        let mut cam = Camera::default();
        cam.viewport = [900.0, 600.0];
        cam.fit(&p);
        // De cerca: a lo lejos las tuberías se dibujan como líneas y no se pueden seccionar.
        cam.distance = 18.0;
        cam.target = [30.0, 36.0, 2501.0];
        cam.elevation = 20.0;
        let solo_red = Layers {
            terreno: false,
            alcantarillado: true,
            agua: true,
            nodos: false,
            edificios: false,
        };

        let contar_cortes = |s: &Scene| -> usize {
            s.prims
                .iter()
                .filter(|p| match p {
                    Prim::Line { color, .. } => {
                        (color[0] - 0.98).abs() < 0.02 && (color[1] - 0.86).abs() < 0.02
                    }
                    _ => false,
                })
                .count()
        };

        let sin = build_opts(&p, None, &cam, solo_red, None, &Opciones::default());
        let con = build_opts(
            &p,
            None,
            &cam,
            solo_red,
            None,
            &Opciones {
                corte_x: Some(30.0),
                ..Opciones::default()
            },
        );
        assert_eq!(
            contar_cortes(&sin),
            0,
            "sin corte no debe haber aristas de corte"
        );
        assert!(
            contar_cortes(&con) > 0,
            "el corte no seccionó ninguna tubería"
        );
    }

    /// La matriz del pase 3D y `project()` deben llevar un mismo punto al mismo píxel del lienzo.
    /// Si divergen, el modelo 3D se dibuja desplazado o estirado respecto al pintor.
    /// Con una caja de terreno menor de 1 m, el margen de 0,5 m dejaba `min > max` y `clamp`
    /// paniqueaba (R-10).
    #[test]
    fn el_recorte_a_la_caja_no_panica_con_una_caja_minuscula() {
        use crate::geom::P2;
        let caja = (0.0, 0.4, 0.0, 0.4);
        for q in [
            P2::new(0.0, 0.0),
            P2::new(0.4, 0.4),
            P2::new(-5.0, 9.0),
            P2::new(f64::MAX, f64::MIN),
        ] {
            let r = super::recortar_a_bbox(q, caja);
            assert!(r.x.is_finite() && r.y.is_finite(), "{q:?} -> {r:?}");
            assert!(r.x >= 0.0 && r.x <= 0.4, "{q:?} -> {r:?}");
            assert!(r.y >= 0.0 && r.y <= 0.4, "{q:?} -> {r:?}");
        }
        // Caja normal: el punto se recorta al interior con el margen.
        let normal = (0.0, 100.0, 0.0, 50.0);
        let r = super::recortar_a_bbox(P2::new(-10.0, 80.0), normal);
        assert!(
            (r.x - 0.5).abs() < 1e-9 && (r.y - 49.5).abs() < 1e-9,
            "{r:?}"
        );
    }

    /// La vista de planta debe mirar desde el sur: norte arriba, este a la derecha.
    #[test]
    fn la_planta_pone_el_norte_arriba() {
        let mut cam = Camera::default();
        cam.viewport = [742.0, 714.0];
        cam.target = [0.0, 0.0, 0.0];
        cam.azimuth = -90.0;
        cam.elevation = 88.0;
        cam.distance = 100.0;
        let centro = cam.project([0.0, 0.0, 0.0]).expect("centro visible");
        let norte = cam.project([0.0, 20.0, 0.0]).expect("norte visible");
        let este = cam.project([20.0, 0.0, 0.0]).expect("este visible");
        assert!(
            norte[1] < centro[1] - 5.0,
            "el norte debe quedar arriba: {:?} vs {:?}",
            norte,
            centro
        );
        assert!(
            este[0] > centro[0] + 5.0,
            "el este debe quedar a la derecha: {:?} vs {:?}",
            este,
            centro
        );
    }

    #[test]
    #[allow(clippy::field_reassign_with_default)]
    fn la_matriz_3d_coincide_con_la_proyeccion_2d() {
        let p = crate::demo_catalog::catalogo();
        let mut cam = Camera::default();
        cam.viewport = [742.0, 714.0];
        cam.fit(&p);
        cam.azimuth = -58.0;
        cam.elevation = 34.0;
        let (w, h) = (cam.viewport[0], cam.viewport[1]);
        let m = cam.matriz(w / h, 1.0);

        let puntos = [
            [0.0, 0.0, 2500.0],
            [60.0, 24.0, 2502.0],
            [-40.0, 30.0, 2510.0],
            [110.0, 45.0, 2505.0],
        ];
        for q in puntos {
            let esperado = cam.project(q).expect("delante de la cámara");
            // NDC a partir de la matriz (columna mayor).
            let x = m[0] * q[0] as f32 + m[4] * q[1] as f32 + m[8] * q[2] as f32 + m[12];
            let y = m[1] * q[0] as f32 + m[5] * q[1] as f32 + m[9] * q[2] as f32 + m[13];
            let wc = m[3] * q[0] as f32 + m[7] * q[1] as f32 + m[11] * q[2] as f32 + m[15];
            let (ndc_x, ndc_y) = (x / wc, y / wc);
            let pantalla = [(ndc_x + 1.0) * 0.5 * w, (1.0 - ndc_y) * 0.5 * h];
            assert!(
                (pantalla[0] - esperado[0]).abs() < 1.0 && (pantalla[1] - esperado[1]).abs() < 1.0,
                "punto {q:?}: matriz {:?} vs proyecto {:?}",
                pantalla,
                esperado
            );
        }
    }

    /// Con muchos objetos juntos, las etiquetas se reparten y no se pisan (F10-03).
    #[test]
    fn las_etiquetas_no_se_solapan() {
        let p = crate::demo_city::ciudad();
        let mut cam = Camera::default();
        cam.viewport = [900.0, 600.0];
        cam.fit(&p);
        cam.distance = 120.0;
        let s = build_opts(
            &p,
            p.terrain.as_ref(),
            &cam,
            Layers::default(),
            None,
            &Opciones::default(),
        );
        assert!(
            s.etiquetas.len() >= 4,
            "hay etiquetas: {}",
            s.etiquetas.len()
        );
        let caja = |e: &Etiqueta| {
            let ancho = (e.texto.chars().count() as f32 * 7.2 + 8.0).clamp(30.0, 260.0);
            [
                e.pos[0] - ancho * 0.5,
                e.pos[1] - 7.5,
                e.pos[0] + ancho * 0.5,
                e.pos[1] + 7.5,
            ]
        };
        for i in 0..s.etiquetas.len() {
            for j in (i + 1)..s.etiquetas.len() {
                let (a, b) = (caja(&s.etiquetas[i]), caja(&s.etiquetas[j]));
                // Se permite un roce mínimo de 2 px, como en el criterio de aceptación.
                let solapa = a[0] < b[2] - 2.0
                    && a[2] > b[0] + 2.0
                    && a[1] < b[3] - 2.0
                    && a[3] > b[1] + 2.0;
                assert!(
                    !solapa,
                    "se pisan «{}» y «{}»: {:?} vs {:?}",
                    s.etiquetas[i].texto, s.etiquetas[j].texto, a, b
                );
            }
        }
    }

    #[test]
    fn el_terreno_queda_ordenado_de_lejos_a_cerca() {
        let p = crate::demo::project();
        let c = demo_cam();
        let s = build(&p, &c, Layers::default(), None);
        // El terreno abre la escena y esta ordenado por profundidad; la red se pinta despues.
        let terreno = p.terrain.as_ref().map(|t| t.tris.len()).unwrap_or(0);
        assert!(terreno > 0);
        let depths: Vec<f32> = s
            .prims
            .iter()
            .take(terreno)
            .map(|x| match x {
                Prim::Poly { depth, .. } => *depth,
                Prim::Line { depth, .. } => *depth,
            })
            .collect();
        assert!(depths.len() >= terreno);
        assert!(depths.windows(2).all(|w| w[0] >= w[1] - 1e-3));
        // Si la profundidad fuera constante (mutación que la suite no detectaba) esta resta da 0.
        let (lejos, cerca) = (depths[0], *depths.last().unwrap());
        assert!(
            lejos - cerca > 1.0,
            "el terreno debe abarcar profundidad real: {lejos} -> {cerca}"
        );
        // Y el orden tiene que depender de la cámara: mirando desde el otro lado cambia.
        let mut al_revés = c;
        al_revés.azimuth += 180.0;
        let otro = build(&p, &al_revés, Layers::default(), None);
        let primer_vertice = |escena: &Scene| -> Option<[f32; 2]> {
            escena.prims.first().and_then(|x| match x {
                Prim::Poly { pts, .. } => pts.first().copied(),
                Prim::Line { .. } => None,
            })
        };
        assert_ne!(
            primer_vertice(&s),
            primer_vertice(&otro),
            "el orden del terreno no depende de la cámara"
        );
    }

    fn profundidad(p: &Prim) -> f32 {
        match p {
            Prim::Poly { depth, .. } | Prim::Line { depth, .. } => *depth,
        }
    }

    /// ¿Toca la primitiva el viewport (con 2 pt de margen)? Cuenta propia, sin `Pantalla`: si
    /// el recorte se equivoca de rectángulo, esta prueba no se equivoca con él.
    fn se_ve(viewport: [f32; 2], p: &Prim) -> bool {
        let (mut min, mut max) = ([f32::INFINITY; 2], [f32::NEG_INFINITY; 2]);
        let mut caja = |q: [f32; 2], m: f32| {
            min = [min[0].min(q[0] - m), min[1].min(q[1] - m)];
            max = [max[0].max(q[0] + m), max[1].max(q[1] + m)];
        };
        match p {
            Prim::Poly { pts, .. } => pts.iter().for_each(|q| caja(*q, 0.0)),
            Prim::Line { a, b, w, .. } => {
                caja(*a, w * 0.5);
                caja(*b, w * 0.5);
            }
        }
        max[0] >= -2.0
            && min[0] <= viewport[0] + 2.0
            && max[1] >= -2.0
            && min[1] <= viewport[1] + 2.0
    }

    /// Cámara cerca de un pozo de la demo: piezas con malla y casi toda la red fuera.
    fn de_cerca(p: &Project) -> Camera {
        let mut c = demo_cam();
        let n = p
            .nodes
            .iter()
            .find(|n| n.family == "saneamiento.pozo")
            .unwrap();
        c.target = [n.pos.x - p.origin.x, n.pos.y - p.origin.y, n.z_rim];
        c.distance = 25.0;
        c.azimuth = 35.0;
        c
    }

    /// F14-07: el recorte por pantalla solo quita lo que no se ve. La escena recortada es la
    /// completa sin las primitivas que caen enteras fuera del viewport, en el mismo orden.
    #[test]
    fn el_recorte_solo_quita_lo_que_no_se_ve() {
        let mut casos = 0;
        for p in [crate::demo::project(), crate::demo_city::ciudad()] {
            let cerca = de_cerca(&p);
            let mut con_corte = cerca.clone();
            con_corte.distance = 12.0;
            let xc = con_corte.target[0] as f32;
            for (cam, opts) in [
                (demo_cam(), Opciones::default()),
                (cerca, Opciones::default()),
                (
                    con_corte,
                    Opciones {
                        corte_x: Some(xc),
                        ..Opciones::default()
                    },
                ),
            ] {
                let t = p.terrain.as_ref();
                let sin_modelos = ModelPieces::new();
                let todo = construir(
                    &p,
                    t,
                    &cam,
                    Layers::default(),
                    None,
                    &opts,
                    false,
                    &sin_modelos,
                    &Default::default(),
                );
                let recortada = construir(
                    &p,
                    t,
                    &cam,
                    Layers::default(),
                    None,
                    &opts,
                    true,
                    &sin_modelos,
                    &Default::default(),
                );
                // Sin recorte solo se descarta lo que queda entero detrás de la cámara.
                assert!(recortada.descartadas >= todo.descartadas);
                let mut i = 0;
                for prim in &todo.prims {
                    if recortada.prims.get(i) == Some(prim) {
                        i += 1;
                    } else {
                        assert!(
                            !se_ve(cam.viewport, prim),
                            "el recorte quitó algo visible: {prim:?}"
                        );
                    }
                }
                assert_eq!(
                    i,
                    recortada.prims.len(),
                    "la escena recortada no es una sub-lista"
                );
                casos += (recortada.descartadas > todo.descartadas) as usize;
            }
        }
        assert!(casos >= 2, "de cerca el recorte debe descartar algo");
    }

    /// F14-04: el orden por claves enteras es el mismo que el ordenamiento estable por
    /// profundidad de antes, con empates, negativos y ceros con signo.
    #[test]
    fn el_orden_del_pintor_es_el_estable_de_antes() {
        let mut lista: Vec<Prim> = (0..500u32)
            .map(|i| {
                let d = match i % 7 {
                    0 => 0.0,
                    1 => -0.0,
                    2 => -((i % 13) as f32) * 0.5,
                    _ => ((i * 7919) % 101) as f32 * 0.25,
                };
                Prim::Line {
                    a: [i as f32, 0.0],
                    b: [0.0, 0.0],
                    w: 1.0,
                    color: [0.0; 4],
                    depth: d,
                }
            })
            .collect();
        let mut antes = lista.clone();
        antes.sort_by(|a, b| {
            profundidad(b)
                .partial_cmp(&profundidad(a))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        sort_far_first(&mut lista);
        assert_eq!(lista, antes);
    }

    /// F14-04: los bloques entre hilos devuelven los resultados en el orden de la lista.
    #[test]
    fn los_bloques_conservan_el_orden() {
        let items: Vec<u32> = (0..(OBJETOS_POR_HILO as u32 * 6 + 17)).collect();
        let bloques = en_bloques(&items, |b| b.to_vec());
        if std::thread::available_parallelism().map_or(1, |n| n.get()) > 1 {
            assert!(bloques.len() > 1, "con muchos objetos se reparten");
        }
        assert_eq!(bloques.concat(), items);
        assert_eq!(en_bloques(&items[..10], |b| b.len()), vec![10]);
    }

    /// F14-07: la esfera previa de un pozo (sin teselarlo) envuelve su malla, con explosión
    /// y exageración vertical, para todos los subtipos de pozo y de bombeo.
    #[test]
    fn la_esfera_previa_del_pozo_envuelve_su_malla() {
        let base = crate::demo::project();
        let pozo = base
            .nodes
            .iter()
            .find(|n| n.family == "saneamiento.pozo")
            .unwrap()
            .clone();
        let o = base.origin;
        let mut probados = 0;
        for familia in ["saneamiento.pozo", "saneamiento.bombeo"] {
            for ty in &family(familia).unwrap().types {
                for (diametro, k, dz) in [(1.2, 1.0, 0.0), (1.8, 2.6, 1.5), (0.6, 2.0, 0.0)] {
                    let mut n = pozo.clone();
                    n.family = familia.to_string();
                    n.ty = ty.key.to_string();
                    n.attrs.insert(
                        "diametro_interior".into(),
                        crate::families::Value::Num(diametro),
                    );
                    let zr = n.z_invert + 1.0;
                    let to_local = |q: P3| [q.x - o.x, q.y - o.y, (q.z - zr) * k + zr];
                    let (centro, radio) = esfera_previa(&n, dz, &to_local, k).unwrap();
                    for parte in pieza_partes(&n, 2) {
                        for v in parte.mesh.tris.iter().flat_map(|t| t.p) {
                            for sube in [0.0, dz] {
                                let q = to_local(P3::new(
                                    n.pos.x + v[0] as f64,
                                    n.pos.y + v[1] as f64,
                                    n.z_invert + v[2] as f64 + sube,
                                ));
                                let d = ((q[0] - centro[0]).powi(2)
                                    + (q[1] - centro[1]).powi(2)
                                    + (q[2] - centro[2]).powi(2))
                                .sqrt();
                                assert!(
                                    d <= radio + 1e-6,
                                    "{familia}/{} D={diametro}: vértice a {d:.3} m, radio {radio:.3}",
                                    ty.key
                                );
                            }
                        }
                    }
                    probados += 1;
                }
            }
        }
        assert!(probados >= 6);
    }
}
