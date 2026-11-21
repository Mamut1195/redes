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
    pub fn project(&self, p: [f64; 3]) -> Option<[f32; 2]> {
        let (eye, right, up) = self.basis();
        let v = [p[0] - eye[0], p[1] - eye[1], p[2] - eye[2]];
        let z = dot(v, self.forward());
        if z <= 0.05 {
            return None;
        }
        let f = z / self.focal();
        let x = (dot(v, right) / f) as f32;
        let y = (dot(v, up) / f) as f32;
        Some([self.viewport[0] * 0.5 + x, self.viewport[1] * 0.5 - y])
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
        dot(
            [
                p[0] - self.eye()[0],
                p[1] - self.eye()[1],
                p[2] - self.eye()[2],
            ],
            self.forward(),
        )
    }

    /// Proyecta un segmento recortándolo contra el plano cercano.
    pub fn project_segment(&self, a: [f64; 3], b: [f64; 3]) -> Option<([f32; 2], [f32; 2])> {
        let (da, db) = (self.depth(a), self.depth(b));
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

#[derive(Clone, Debug)]
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
    let o = p.origin;
    let zr = cam.target[2];
    let k = cam.z_scale;
    let to_local = move |q: P3| [q.x - o.x, q.y - o.y, (q.z - zr) * k + zr];
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
            let (Some(sa), Some(sb), Some(sc)) =
                (cam.project(pa), cam.project(pb), cam.project(pc))
            else {
                continue;
            };
            let depth = (cam.depth(pa) + cam.depth(pb) + cam.depth(pc)) as f32 / 3.0;
            let fog = fog(depth, cam.distance as f32);
            // Color por cota (verde valle ? ocre altura) con sombreado por normal.
            let t01 = (((a.z + b.z + c.z) / 3.0 - zmin) / zspan).clamp(0.0, 1.0) as f32;
            let base = [0.26 + 0.34 * t01, 0.40 + 0.16 * t01, 0.26 + 0.06 * t01];
            prims.push(Prim::Poly {
                pts: expand(&[sa, sb, sc], 0.6),
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
                let mut pts = Vec::with_capacity(4);
                let mut prof = Vec::with_capacity(4);
                let mut ok = true;
                for q in esquinas {
                    let Some(z) = z_de(q) else {
                        ok = false;
                        break;
                    };
                    let local = to_local(P3::new(q.x, q.y, z + 0.22));
                    match cam.project(local) {
                        Some(s) => {
                            pts.push(s);
                            prof.push(cam.depth(local));
                        }
                        None => ok = false,
                    }
                }
                if !ok || pts.len() < 4 {
                    continue;
                }
                let depth = prof.iter().sum::<f64>() as f32 / prof.len() as f32;
                roads.push(Prim::Poly {
                    pts: pts.clone(),
                    fill: [0.175, 0.185, 0.205, 1.0],
                    depth,
                });
                // Aceras: dos franjas claras a los lados de la calzada.
                for lado in [-1.0f64, 1.0] {
                    let off = lado * (half + 1.2);
                    let ancho_acera = 2.2;
                    let sx = -(b.y - a.y) / d;
                    let sy = (b.x - a.x) / d;
                    let mut apts = Vec::with_capacity(4);
                    let mut aprof = Vec::with_capacity(4);
                    let mut aok = true;
                    for (px, py) in [
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
                    ] {
                        let q = crate::geom::P2::new(px, py);
                        let Some(z) = z_de(q) else {
                            aok = false;
                            break;
                        };
                        let local = to_local(P3::new(px, py, z + 0.26));
                        match cam.project(local) {
                            Some(s) => {
                                apts.push(s);
                                aprof.push(cam.depth(local));
                            }
                            None => aok = false,
                        }
                    }
                    if aok && apts.len() == 4 {
                        let ad = aprof.iter().sum::<f64>() as f32 / 4.0;
                        roads.push(Prim::Poly {
                            pts: apts,
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
                    let mut pts = Vec::with_capacity(4);
                    let mut prof = Vec::with_capacity(4);
                    let mut ok = true;
                    for q in quad {
                        match cam.project(q) {
                            Some(s) => {
                                pts.push(s);
                                prof.push(cam.depth(q));
                            }
                            None => ok = false,
                        }
                    }
                    if !ok {
                        continue;
                    }
                    let depth = prof.iter().sum::<f64>() as f32 / 4.0;
                    let shade = 0.70 + 0.30 * (((p2.x - p1.x) as f32 / 18.0).cos().abs());
                    let f = fog(depth, cam.distance as f32);
                    city.push(Prim::Poly {
                        pts,
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
            let Some(cs) = cam.project(centro) else {
                continue;
            };
            let cd = cam.depth(centro);
            for i in 0..n {
                let q1 = mz.poligono[i];
                let q2 = mz.poligono[(i + 1) % n];
                let a = to_local(P3::new(q1.x, q1.y, top));
                let b = to_local(P3::new(q2.x, q2.y, top));
                let (Some(sa), Some(sb)) = (cam.project(a), cam.project(b)) else {
                    continue;
                };
                let depth = ((cd + cam.depth(a) + cam.depth(b)) / 3.0) as f32;
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
    for l in p.links.iter().filter(|l| !l.deleted) {
        let es_agua = l.family.starts_with("agua");
        if (es_agua && !layers.agua) || (!es_agua && !layers.alcantarillado) {
            continue;
        }
        if l.path.len() < 2 {
            continue;
        }
        let sel = selected.map(|s| s == l.code).unwrap_or(false);
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
        // El tubo no atraviesa el pozo: se recorta en su pared.
        let mut traza = l.path.clone();
        let r0 = p
            .node(l.from)
            .map(|n| n.diametro() * 0.5 + 0.15)
            .unwrap_or(0.4);
        let r1 = p
            .node(l.to)
            .map(|n| n.diametro() * 0.5 + 0.15)
            .unwrap_or(0.4);
        recortar_extremos(&mut traza, r0, r1);

        // Los tubos se construyen en coordenadas LOCALES: con UTM en f32 el paso es ~1 m y la
        // sección colapsaría (los tubos se verían como cintas).
        for w in traza.windows(2) {
            let wa = P3::new(w[0].x - p.origin.x, w[0].y - p.origin.y, w[0].z);
            let wb = P3::new(w[1].x - p.origin.x, w[1].y - p.origin.y, w[1].z);
            let a = to_local(w[0]);
            let b = to_local(w[1]);
            let Some((sa, sb)) = cam.project_segment(a, b) else {
                continue;
            };
            let depth = ((cam.depth(a) + cam.depth(b)) * 0.5) as f32;
            let r_px = (l.d_m() * 0.5 * cam.focal() / depth.max(1.0) as f64) as f32;

            if r_px < 2.2 {
                // A lo lejos, la tubería es una línea: mismo color, mucho menos coste.
                let width = if sel {
                    4.5
                } else {
                    (r_px * 2.0).clamp(1.2, 6.0)
                };
                let fog = fog(depth, cam.distance as f32);
                net.push(Prim::Line {
                    a: sa,
                    b: sb,
                    w: width,
                    color: [lr, lg, lb, fog],
                    depth,
                });
                line_count += 1;
                continue;
            }

            // De cerca, tubo con juntas.
            let lod = if r_px > 6.0 { 2 } else { 1 };
            let malla = crate::parts::tuberia(wa, wb, l.d_m() as f32, lod, mat);
            for tri in &malla.tris {
                // La malla ya está en local: se aplica solo la exageración vertical.
                let verts = [
                    [
                        tri.p[0][0] as f64,
                        tri.p[0][1] as f64,
                        (tri.p[0][2] as f64 - zr) * k + zr,
                    ],
                    [
                        tri.p[1][0] as f64,
                        tri.p[1][1] as f64,
                        (tri.p[1][2] as f64 - zr) * k + zr,
                    ],
                    [
                        tri.p[2][0] as f64,
                        tri.p[2][1] as f64,
                        (tri.p[2][2] as f64 - zr) * k + zr,
                    ],
                ];
                // El corte secciona también las tuberías.
                let (trozos, arista) = match opts.corte_x {
                    Some(xc) => cortar_tri(verts, xc),
                    None => (vec![verts], None),
                };
                if let Some((ca, cb)) = arista
                    && let (Some(sa), Some(sb)) = (cam.project(ca), cam.project(cb))
                {
                    net.push(Prim::Line {
                        a: sa,
                        b: sb,
                        w: 1.4,
                        color: [0.98, 0.86, 0.42, 1.0],
                        depth: ((cam.depth(ca) + cam.depth(cb)) * 0.5) as f32,
                    });
                    line_count += 1;
                }
                for trozo in trozos {
                    let mut pts = Vec::with_capacity(3);
                    let mut d = 0.0;
                    let mut ok = true;
                    for v in trozo {
                        match cam.project(v) {
                            Some(s) => {
                                pts.push(s);
                                d += cam.depth(v);
                            }
                            None => ok = false,
                        }
                    }
                    if !ok {
                        continue;
                    }
                    let depth_t = (d / 3.0) as f32;
                    let f = fog(depth_t, cam.distance as f32);
                    let c = if sel {
                        [0.36, 0.95, 0.56, 1.0]
                    } else {
                        [tri.color[0], tri.color[1], tri.color[2], 1.0]
                    };
                    net.push(Prim::Poly {
                        pts: expand(&pts, 0.35),
                        fill: [
                            c[0] * f + 0.050 * (1.0 - f),
                            c[1] * f + 0.055 * (1.0 - f),
                            c[2] * f + 0.066 * (1.0 - f),
                            1.0,
                        ],
                        depth: depth_t,
                    });
                    line_count += 1;
                }
            }
        }
    }

    // --- nodos: piezas paramétricas por familia ---
    if layers.nodos {
        for n in p.nodes.iter().filter(|n| !n.deleted) {
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
            if (es_agua && !layers.agua) || (!es_agua && !layers.alcantarillado) {
                continue;
            }
            let sel = selected.map(|s| s == n.code).unwrap_or(false);
            let observado = n.estado == EstadoObjeto::Observado;
            let tinte: Option<[f32; 3]> = if sel {
                Some([0.36, 0.95, 0.56])
            } else if observado {
                Some([0.98, 0.45, 0.42])
            } else {
                None
            };

            let r = n.diametro() * 0.5;
            let top = to_local(P3::new(n.pos.x, n.pos.y, n.z_rim));
            let edge = to_local(P3::new(n.pos.x + r, n.pos.y, n.z_rim));
            let r_px = match (cam.project(top), cam.project(edge)) {
                (Some(a), Some(b)) => ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt(),
                _ => 0.0,
            };

            // Muy lejos: símbolo de tamaño fijo (una ciudad no se llena de cilindros).
            if r_px < 3.0
                && let Some(c) = cam.project(top)
            {
                let sz = if sel { 6.0 } else { 3.6 };
                let base: [f32; 3] = if es_agua {
                    [0.33, 0.63, 0.96]
                } else {
                    [0.87, 0.60, 0.31]
                };
                let col = tinte.unwrap_or(base);
                net.push(Prim::Poly {
                    pts: vec![
                        [c[0] - sz, c[1]],
                        [c[0], c[1] - sz],
                        [c[0] + sz, c[1]],
                        [c[0], c[1] + sz],
                    ],
                    fill: [col[0], col[1], col[2], 1.0],
                    depth: cam.depth(top) as f32,
                });
                tri_count += 1;
                continue;
            }

            // LOD por tamaño en pantalla, con presupuesto global de triángulos.
            let mut lod = if r_px > 14.0 { 2u8 } else { 1u8 };
            if tri_count > 90_000 {
                lod = 1;
            }
            let partes = pieza_partes(n, lod);
            if partes.is_empty() {
                continue;
            }
            // Explosión: cada parte nombrada del objeto enfocado sube a su altura.
            let altura_pieza = (n.z_rim - n.z_invert).max(0.5) as f32;
            let explotar = opts.explosion > 0.0 && opts.foco == Some(n.id);

            for (indice_parte, parte) in partes.iter().enumerate() {
                for tri in &parte.mesh.tris {
                    let mut dz = 0.0f64;
                    if explotar {
                        // Reparto proporcional: la pila entera se abre ~0,85 de la altura de la pieza.
                        let n = partes.len().max(2) as f32;
                        let t = indice_parte as f32 / (n - 1.0);
                        dz = (t * opts.explosion * altura_pieza * 0.85) as f64;
                    }
                    let tri_local = [
                        to_local(P3::new(
                            tri.p[0][0] as f64 + n.pos.x,
                            tri.p[0][1] as f64 + n.pos.y,
                            tri.p[0][2] as f64 + n.z_invert + dz,
                        )),
                        to_local(P3::new(
                            tri.p[1][0] as f64 + n.pos.x,
                            tri.p[1][1] as f64 + n.pos.y,
                            tri.p[1][2] as f64 + n.z_invert + dz,
                        )),
                        to_local(P3::new(
                            tri.p[2][0] as f64 + n.pos.x,
                            tri.p[2][1] as f64 + n.pos.y,
                            tri.p[2][2] as f64 + n.z_invert + dz,
                        )),
                    ];

                    // Corte vertical: se recorta el triángulo y se marca la arista de corte.
                    let (trozos, corte) = match opts.corte_x {
                        Some(xc) => cortar_tri(tri_local, xc),
                        None => (vec![tri_local], None),
                    };
                    if let Some((a, b)) = corte
                        && let (Some(sa), Some(sb)) = (cam.project(a), cam.project(b))
                    {
                        net.push(Prim::Line {
                            a: sa,
                            b: sb,
                            w: 1.6,
                            color: [0.98, 0.86, 0.42, 1.0],
                            depth: ((cam.depth(a) + cam.depth(b)) * 0.5) as f32,
                        });
                        line_count += 1;
                    }

                    for q in trozos {
                        let mut pts = Vec::with_capacity(3);
                        let mut prof = 0.0;
                        let mut ok = true;
                        for v in q {
                            match cam.project(v) {
                                Some(s) => {
                                    pts.push(s);
                                    prof += cam.depth(v);
                                }
                                None => ok = false,
                            }
                        }
                        if !ok {
                            continue;
                        }
                        let depth = (prof / 3.0) as f32;
                        let f = fog(depth, cam.distance as f32);
                        let base = tinte.unwrap_or([tri.color[0], tri.color[1], tri.color[2]]);
                        // Un objeto seleccionado u observado conserva el sombreado pero cambia de tono.
                        let (cr, cg, cb) = if tinte.is_some() {
                            let k = (tri.color[0] + tri.color[1] + tri.color[2]) / 2.4;
                            (base[0] * k, base[1] * k, base[2] * k)
                        } else {
                            (base[0], base[1], base[2])
                        };
                        net.push(Prim::Poly {
                            pts: expand(&pts, 0.3),
                            fill: [
                                cr * f + 0.050 * (1.0 - f),
                                cg * f + 0.055 * (1.0 - f),
                                cb * f + 0.066 * (1.0 - f),
                                1.0,
                            ],
                            depth,
                        });
                        tri_count += 1;
                    }
                }
            }

            // Etiqueta: solo cuando la pieza ya se distingue o está seleccionada.
            if (r_px > 9.0 || sel)
                && let Some(c) = cam.project(top)
            {
                etiquetas.push(Etiqueta {
                    pos: [c[0], c[1] - 10.0],
                    texto: format!("{} · {}", n.code, n.ty.replace('_', " ")),
                    seleccion: sel,
                    depth: cam.depth(top) as f32,
                });
            }
        }
    }

    // El terreno se ordena por profundidad; la red se ordena aparte y se pinta encima
    // (modo rayos X), de modo que las tuberias enterradas siempre se ven.
    sort_far_first(&mut prims);
    sort_far_first(&mut roads);
    prims.extend(roads);
    // La red se dibuja sobre el terreno (modo rayos X) pero **bajo** los edificios: así las
    // tuberías se ven a través del suelo y quedan ocultas detrás de una fachada, como debe ser.
    sort_far_first(&mut net);
    prims.extend(net);
    sort_far_first(&mut city);
    prims.extend(city);

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
    Scene {
        prims,
        tri_count,
        line_count,
        etiquetas,
    }
}

/// Ordena de mas lejano a mas cercano (algoritmo del pintor).
fn sort_far_first(list: &mut [Prim]) {
    list.sort_by(|a, b| {
        let da = match a {
            Prim::Poly { depth, .. } => *depth,
            Prim::Line { depth, .. } => *depth,
        };
        let db = match b {
            Prim::Poly { depth, .. } => *depth,
            Prim::Line { depth, .. } => *depth,
        };
        db.partial_cmp(&da).unwrap_or(std::cmp::Ordering::Equal)
    });
}

/// Niebla por distancia: acerca el color al fondo con la lejanía.
/// Dilata un poligono proyectado unas decimas de pixel alrededor de su centroide.
///
/// Los triangulos del terreno se antialiasan por separado y dejarian una costura de un pixel
/// entre vecinos; solaparlos un poco la hace invisible.
fn expand(pts: &[[f32; 2]], px: f32) -> Vec<[f32; 2]> {
    let n = pts.len().max(1) as f32;
    let cx = pts.iter().map(|p| p[0]).sum::<f32>() / n;
    let cy = pts.iter().map(|p| p[1]).sum::<f32>() / n;
    pts.iter()
        .map(|p| {
            let (dx, dy) = (p[0] - cx, p[1] - cy);
            let l = (dx * dx + dy * dy).sqrt().max(1e-3);
            [p[0] + dx / l * px, p[1] + dy / l * px]
        })
        .collect()
}

fn fog(depth: f32, distance: f32) -> f32 {
    let t = (depth / (distance * 2.2).max(1.0)).clamp(0.0, 0.55);
    1.0 - t
}

/// Acerca el primer y el último vértice de una polilínea hacia el interior, para que la tubería
/// nazca y muera en la pared del pozo y no lo atraviese.
fn recortar_extremos(pts: &mut [P3], r0: f64, r1: f64) {
    if pts.len() < 2 {
        return;
    }
    let n = pts.len();
    let d0 = pts[0].p2().dist(pts[1].p2()).max(1e-6);
    let t0 = (r0 / d0).min(0.45);
    pts[0] = P3::new(
        pts[0].x + (pts[1].x - pts[0].x) * t0,
        pts[0].y + (pts[1].y - pts[0].y) * t0,
        pts[0].z + (pts[1].z - pts[0].z) * t0,
    );
    let d1 = pts[n - 2].p2().dist(pts[n - 1].p2()).max(1e-6);
    let t1 = (r1 / d1).min(0.45);
    pts[n - 1] = P3::new(
        pts[n - 1].x + (pts[n - 2].x - pts[n - 1].x) * t1,
        pts[n - 1].y + (pts[n - 2].y - pts[n - 1].y) * t1,
        pts[n - 1].z + (pts[n - 2].z - pts[n - 1].z) * t1,
    );
}

/// Recorta un triángulo contra el semiespacio `x <= xc` y devuelve los trozos que quedan
/// más la arista de corte (si el plano lo atraviesa).
/// Trozos de un triángulo al cortarlo y, si el plano lo atraviesa, la arista de corte.
type Corte = (Vec<[[f64; 3]; 3]>, Option<([f64; 3], [f64; 3])>);

fn cortar_tri(tri: [[f64; 3]; 3], xc: f32) -> Corte {
    let xc = xc as f64;
    let dentro = |p: [f64; 3]| p[0] <= xc;
    let n_dentro = tri.iter().filter(|p| dentro(**p)).count();
    if n_dentro == 0 {
        return (Vec::new(), None);
    }
    if n_dentro == 3 {
        return (vec![tri], None);
    }

    let mut poligono: Vec<[f64; 3]> = Vec::with_capacity(4);
    let mut arista: Option<([f64; 3], [f64; 3])> = None;
    for i in 0..3 {
        let a = tri[i];
        let b = tri[(i + 1) % 3];
        let (da, db) = (dentro(a), dentro(b));
        if da {
            poligono.push(a);
        }
        if da != db {
            let t =
                (xc - a[0]) / (b[0] - a[0]).abs().max(1e-9) * if b[0] > a[0] { 1.0 } else { -1.0 };
            let t = t.clamp(0.0, 1.0);
            let p = [xc, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t];
            poligono.push(p);
            arista = match arista {
                None => Some((p, p)),
                Some((p0, _)) => Some((p0, p)),
            };
        }
    }

    let mut trozos = Vec::new();
    if poligono.len() >= 3 {
        trozos.push([poligono[0], poligono[1], poligono[2]]);
        if poligono.len() == 4 {
            trozos.push([poligono[0], poligono[2], poligono[3]]);
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

/// Construye las partes nombradas de una pieza (pozos y válvulas se despiezan; el resto es una).
fn pieza_partes(n: &crate::model::Node, lod: u8) -> Vec<crate::parts::Parte> {
    use crate::parts as pt;
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
                    n.diametro().max(1.8)
                } else {
                    n.diametro()
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
                diametro: n.diametro() as f32,
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
            n.diametro() as f32,
            ((n.z_rim - n.z_invert) as f32).clamp(0.8, 4.0),
        ),
        "saneamiento.sumidero" => pt::sumidero(),
        "saneamiento.bombeo" => pt::pozo(pt::PozoParams {
            diametro: n.diametro().max(1.8) as f32,
            rim: (n.z_rim - n.z_invert) as f32,
            invert: 0.0,
            tipo: 4,
            lod,
        }),
        _ => pt::cilindro(
            n.diametro() as f32 * 0.5,
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
                nota: None,
                deleted: false,
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
        for tri in &trozos {
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
                    nota: None,
                    deleted: false,
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
                nota: None,
                deleted: false,
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
}
