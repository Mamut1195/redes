//! Redes — visor 3D (Rune).
//!
//! No calcula nada: dibuja el modelo que importó la CLI, deja revisarlo y aprobarlo, y lo
//! entrega en DXF. La escena 3D sale de `redes_core::view`, la misma que usa `redes vista`.
//!
//! `cargo run -p redes-app` · `-- --snap vista.png --scene iso` · `-- --tree arbol.json`

mod gpu3d;
mod modelos;

use std::sync::{Arc, Mutex};

use redes_core::families::{self, caps, family};
use redes_core::model::{EstadoObjeto, Project};
use redes_core::terrain::Terrain;
use redes_core::view::{self, Camera, Layers, Opciones, Prim, Scene};
use rune::tok::type_;
use rune::{
    AppSpec, Color, Cx, Drag, El, LineCap, Phase, PointerButton, ScrollSpec, Shape, Tx, WheelEvent,
    canvas, col, el, row, text,
};
use std::path::PathBuf;

// ---------------------------------------------------------------------------------------------
// Paleta

const BG: Color = Color {
    r: 0.062,
    g: 0.068,
    b: 0.082,
    a: 1.0,
};
const AL: Color = Color {
    r: 0.87,
    g: 0.60,
    b: 0.31,
    a: 1.0,
};
const AP: Color = Color {
    r: 0.33,
    g: 0.63,
    b: 0.96,
    a: 1.0,
};
const PANEL: Color = Color {
    r: 0.100,
    g: 0.107,
    b: 0.124,
    a: 1.0,
};
const PANEL_HI: Color = Color {
    r: 0.145,
    g: 0.152,
    b: 0.174,
    a: 1.0,
};
const LINE: Color = Color {
    r: 0.185,
    g: 0.194,
    b: 0.220,
    a: 1.0,
};
const INK: Color = Color {
    r: 0.910,
    g: 0.920,
    b: 0.945,
    a: 1.0,
};
const DIM: Color = Color {
    r: 0.560,
    g: 0.588,
    b: 0.645,
    a: 1.0,
};
const OK: Color = Color {
    r: 0.360,
    g: 0.900,
    b: 0.560,
    a: 1.0,
};
const WARN: Color = Color {
    r: 0.980,
    g: 0.760,
    b: 0.300,
    a: 1.0,
};
const ERR: Color = Color {
    r: 0.980,
    g: 0.420,
    b: 0.400,
    a: 1.0,
};

// ---------------------------------------------------------------------------------------------
// Estado

struct State {
    project: Arc<Project>,
    cam: Camera,
    scene: Scene,
    /// Terreno aligerado para dibujar (el TIN completo se reserva a consultas).
    display: Option<Terrain>,
    layers: Layers,
    sel: Option<String>,
    cursor: [f32; 2],
    /// Tamaño del viewport en puntos (lo reporta `on_layout`).
    viewport: [f32; 2],
    fitted: bool,
    /// Archivo del proyecto, si se abrió desde disco.
    path: Option<PathBuf>,
    /// Hay cambios de revisión sin guardar.
    dirty: bool,
    /// Último `dx/dy` aplicado del gesto (los de `Drag` son acumulados desde la pulsación).
    drag_last: [f32; 2],
    /// Modos de detalle: corte, explosión y foco.
    opts: Opciones,
    /// Pila de deshacer para la edición paramétrica (máx. 20).
    historial: Vec<Project>,
    /// Aviso breve en la barra de estado.
    aviso: Option<String>,
    /// Pase 3D con profundidad activo (fase 5).
    gpu3d: bool,
    /// Geometría compartida con el compositor GPU.
    geom: Arc<Mutex<gpu3d::Geometria>>,
    /// Rectángulo del lienzo en puntos (x, y, ancho, alto): el pase 3D se limita a él.
    canvas_rect: [f32; 4],
    /// Caché de mallas de la biblioteca (un `.glb` se lee una vez).
    modelos: modelos::Cache,
    /// Manifiesto del proyecto y su carpeta, si declara biblioteca.
    bib: Option<(std::path::PathBuf, redes_core::biblioteca::Biblioteca)>,
    /// Aviso de política de dibujo (modelos pesados sin `--gpu3d`).
    aviso_modelos: Option<String>,
}

impl State {
    fn new() -> State {
        let (project, path, error_carga) = load_project();
        let mut cam = Camera::default();
        // Coherente con `canvas_rect` inicial: si no, el primer frame (y cualquier `--snap`)
        // sale con el aspecto de la ventana y el modelo estirado.
        cam.viewport = [742.0, 714.0];
        cam.fit(&project);
        let display = project.terrain.as_ref().map(|t| {
            let diag = (t.bbox.width().powi(2) + t.bbox.height().powi(2)).sqrt();
            t.thinned((diag / 46.0).max(2.0))
        });
        // La ciudad en ladera se lee mejor con algo más de exageración vertical.
        cam.z_scale = if project.entorno.is_some() { 2.6 } else { 2.0 };
        let scene = view::build_with(&project, display.as_ref(), &cam, Layers::default(), None);
        let mut estado = State {
            project: Arc::new(project),
            cam,
            scene,
            display,
            layers: Layers::default(),
            sel: None,
            cursor: [0.0, 0.0],
            viewport: [742.0, 714.0],
            fitted: false,
            path,
            dirty: false,
            drag_last: [0.0, 0.0],
            opts: Opciones::default(),
            historial: Vec::new(),
            gpu3d: gpu3d::activo(),
            aviso: error_carga,
            geom: Arc::new(Mutex::new(gpu3d::Geometria::default())),
            // Estimación para el primer frame (el layout aún no ha corrido): 1280×800 con los
            // paneles de 214 y 292 pt. `on_layout` la sustituye por el rectángulo real.
            canvas_rect: [230.0, 51.0, 742.0, 714.0],
            modelos: modelos::Cache::default(),
            bib: None,
            aviso_modelos: None,
        };
        // Modo rayos X por línea de comandos (útil para capturas y para los agentes).
        if rune::extra_args().iter().any(|a| a == "--rayos-x") {
            estado.opts.rayos_x = true;
        }
        estado.rebuild();
        estado
    }

    /// Cambia el estado de revision de un objeto y marca el documento como sucio.
    fn set_estado(&mut self, code: &str, nuevo: EstadoObjeto, nota: Option<String>) {
        // La revisión también va al historial: si no, Deshacer la borraba sin avisar.
        self.recordar();
        let p = Arc::make_mut(&mut self.project);
        let mut touched = false;
        if let Some(n) = p.nodes.iter_mut().find(|n| n.code == code && !n.deleted) {
            n.estado = nuevo;
            n.nota = nota;
            touched = true;
        } else if let Some(l) = p.links.iter_mut().find(|l| l.code == code && !l.deleted) {
            l.estado = nuevo;
            l.nota = nota.clone();
            touched = true;
        }
        if touched {
            p.commit("ui.revision");
            self.dirty = true;
            self.rebuild();
        }
        let _ = nota;
    }

    /// Guarda de forma atómica (temporal + rename) y solo marca limpio si se escribió.
    ///
    /// Antes se escribía el archivo vivo en un hilo y se ponía `dirty = false` sin esperar: una
    /// escritura fallida dejaba la ventana diciendo «guardado» y un lector podía ver 0 bytes.
    fn save(&mut self) {
        let Some(path) = self.path.clone() else {
            self.aviso =
                Some("Este proyecto no tiene archivo: usa `redes proyecto nuevo --salida`".into());
            return;
        };
        match redes_core::project_io::save(&self.project, &path) {
            Ok(()) => {
                self.dirty = false;
                self.aviso = Some(format!("Guardado {}", path.display()));
            }
            Err(e) => {
                self.aviso = Some(format!("No se pudo guardar: {e}"));
            }
        }
    }

    fn rebuild(&mut self) {
        self.cam.viewport = self.viewport;
        self.scene = view::build_opts(
            &self.project,
            self.display.as_ref(),
            &self.cam,
            self.layers,
            self.sel.as_deref(),
            &self.opts,
        );
        // Manifiesto de la biblioteca: se resuelve una vez por proyecto.
        if self.bib.is_none() && self.project.biblioteca.is_some() {
            let dir = self
                .path
                .as_ref()
                .and_then(|p| p.parent())
                .map(|d| d.to_path_buf())
                .unwrap_or_else(|| std::path::PathBuf::from("."));
            self.bib = modelos::biblioteca_del_proyecto(&self.project, &dir);
            if self.bib.is_none() {
                self.aviso_modelos =
                    Some("El proyecto declara una biblioteca que no se pudo abrir".to_string());
            }
        }

        let con_modelo = self
            .project
            .nodes
            .iter()
            .filter(|n| n.modelo.is_some())
            .count();
        if con_modelo > 0 && !self.gpu3d {
            self.aviso_modelos = Some(format!(
                "{con_modelo} objetos tienen modelo y el pase 3D está apagado: se dibuja la pieza paramétrica. Arranque con --gpu3d para verlos."
            ));
        } else {
            self.aviso_modelos = None;
        }

        if self.gpu3d {
            let instancias = match &self.bib {
                Some((carpeta, bib)) => {
                    modelos::instancias(&self.project, carpeta, bib, &mut self.modelos)
                }
                None => Vec::new(),
            };
            if let Ok(mut g) = self.geom.lock() {
                g.cargar_con_modelos(&self.project, &self.cam, self.layers, &instancias);
            }
        }
    }

    /// Empuja una copia del proyecto a la pila de deshacer.
    fn recordar(&mut self) {
        self.historial.push((*self.project).clone());
        if self.historial.len() > 20 {
            self.historial.remove(0);
        }
    }

    /// Enfoca el objeto seleccionado: cámara encuadrada y modo foco.
    fn enfocar(&mut self) {
        let Some(code) = self.sel.clone() else {
            self.aviso = Some("Selecciona un objeto para enfocar".into());
            return;
        };
        let Some(n) = self.project.node_by_code(&code) else {
            return;
        };
        let (pos, rim, inv, diam) = (n.pos, n.z_rim, n.z_invert, n.diametro());
        let id = n.id;
        let alto = (rim - inv).max(0.4);
        let tam = diam.max(alto);
        self.cam.target = [
            pos.x - self.project.origin.x,
            pos.y - self.project.origin.y,
            rim,
        ];
        self.cam.distance = (tam * 3.4).clamp(2.5, 60.0);
        self.cam.azimuth = -48.0;
        self.cam.elevation = 16.0;
        self.cam.z_scale = 1.0;
        self.opts.foco = Some(id);
        self.opts.explosion = 0.0;
        self.opts.corte_x = None;
        self.fitted = true;
        self.aviso = Some("Foco: rueda para acercar, Flechas para girar".into());
        self.rebuild();
    }

    /// Sale de foco/corte/explosión.
    fn salir_modo(&mut self) {
        let habia =
            self.opts.foco.is_some() || self.opts.corte_x.is_some() || self.opts.explosion > 0.0;
        self.opts = Opciones::default();
        self.cam.z_scale = if self.project.entorno.is_some() {
            2.6
        } else {
            2.0
        };
        if habia {
            self.fit();
            self.aviso = Some("Modo normal".into());
        }
    }

    /// Activa o desactiva el corte vertical por el centro del objeto seleccionado.
    fn alternar_corte(&mut self) {
        if self.opts.corte_x.is_some() {
            self.opts.corte_x = None;
            self.aviso = Some("Corte desactivado".into());
        } else {
            let x = self
                .sel
                .as_deref()
                .and_then(|c| self.project.node_by_code(c))
                .map(|n| (n.pos.x - self.project.origin.x) as f32)
                .unwrap_or(self.cam.target[0] as f32);
            self.opts.corte_x = Some(x);
            self.aviso = Some("Corte activo: Shift + rueda lo desplaza".into());
        }
        self.rebuild();
    }

    /// Activa o desactiva los rayos X: lo enterrado se dibuja translúcido.
    fn alternar_rayos_x(&mut self) {
        self.opts.rayos_x = !self.opts.rayos_x;
        self.aviso = Some(if self.opts.rayos_x {
            "Rayos X: lo enterrado se ve translúcido".into()
        } else {
            "Rayos X desactivado".into()
        });
        self.rebuild();
    }

    /// Activa o desactiva la explosión de la pieza enfocada.
    fn alternar_explosion(&mut self) {
        if self.opts.explosion > 0.0 {
            self.opts.explosion = 0.0;
            self.aviso = Some("Explosión desactivada".into());
        } else {
            if self.opts.foco.is_none() {
                if let Some(code) = self.sel.clone() {
                    self.opts.foco = self.project.node_by_code(&code).map(|n| n.id);
                }
            }
            self.opts.explosion = 0.9;
            self.aviso = Some("Explosión activa: Shift + rueda la abre".into());
        }
        self.rebuild();
    }

    fn deshacer(&mut self) {
        if let Some(prev) = self.historial.pop() {
            self.project = Arc::new(prev);
            self.dirty = true;
            self.aviso = Some("Deshecho".into());
            self.rebuild();
        } else {
            self.aviso = Some("Nada que deshacer".into());
        }
    }

    /// Edición paramétrica en vivo: ajusta un parámetro y reconstruye.
    fn ajustar(&mut self, clave: &str, delta: f64) {
        let Some(code) = self.sel.clone() else { return };
        self.recordar();
        let project = Arc::make_mut(&mut self.project);
        if let Some(n) = project
            .nodes
            .iter_mut()
            .find(|n| n.code == code && !n.deleted)
        {
            match clave {
                "profundidad" => {
                    // Más profundo = clave más baja; los límites se ordenan para no invertir el clamp.
                    let (bajo, alto) = (n.z_terrain - 12.0, n.z_rim - 0.6);
                    n.z_invert = (n.z_invert + delta).clamp(bajo.min(alto), bajo.max(alto));
                    n.attrs
                        .insert("cota_clave".to_string(), families::Value::Num(n.z_invert));
                }
                "diametro" => {
                    let actual = n.diametro();
                    let nuevo = (actual + delta).clamp(0.4, 4.0);
                    n.attrs
                        .insert("diametro_interior".to_string(), families::Value::Num(nuevo));
                }
                _ => {
                    let actual = n.num("diametro_nominal").unwrap_or(200.0);
                    let nuevo = (actual + delta).clamp(25.0, 1200.0);
                    n.attrs.insert(
                        "diametro_nominal".to_string(),
                        families::Value::Int(nuevo as i64),
                    );
                }
            }
        }
        // La edición entra en el journal del proyecto, como cualquier otro cambio.
        project.commit("ui.parametro");
        self.dirty = true;
        self.rebuild();
    }

    fn fit(&mut self) {
        self.cam.fit(&self.project);
        self.rebuild();
        self.fitted = true;
    }

    /// Selecciona el objeto más cercano al puntero en pantalla.
    fn pick(&self, at: [f32; 2]) -> Option<String> {
        let o = self.project.origin;
        let mut best: Option<(f32, String)> = None;
        for n in self.project.nodes.iter().filter(|n| !n.deleted) {
            let p = [n.pos.x - o.x, n.pos.y - o.y, n.z_rim];
            if let Some(s) = self.cam.project(p) {
                let d = ((s[0] - at[0]).powi(2) + (s[1] - at[1]).powi(2)).sqrt();
                if d < 18.0 && best.as_ref().is_none_or(|(bd, _)| d < *bd) {
                    best = Some((d, n.code.clone()));
                }
            }
        }
        for l in self.project.links.iter().filter(|l| !l.deleted) {
            if l.path.len() < 2 {
                continue;
            }
            let a = l.path[0];
            let b = l.path[l.path.len() - 1];
            let (Some(sa), Some(sb)) = (
                self.cam.project([a.x - o.x, a.y - o.y, a.z]),
                self.cam.project([b.x - o.x, b.y - o.y, b.z]),
            ) else {
                continue;
            };
            let d = dist_seg(at, sa, sb);
            if d < 14.0 && best.as_ref().is_none_or(|(bd, _)| d < *bd) {
                best = Some((d, l.code.clone()));
            }
        }
        best.map(|(_, c)| c)
    }
}

fn dist_seg(p: [f32; 2], a: [f32; 2], b: [f32; 2]) -> f32 {
    let ab = [b[0] - a[0], b[1] - a[1]];
    let len2 = ab[0] * ab[0] + ab[1] * ab[1];
    if len2 <= 1e-6 {
        return ((p[0] - a[0]).powi(2) + (p[1] - a[1]).powi(2)).sqrt();
    }
    let t = (((p[0] - a[0]) * ab[0] + (p[1] - a[1]) * ab[1]) / len2).clamp(0.0, 1.0);
    let q = [a[0] + ab[0] * t, a[1] + ab[1] * t];
    ((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2)).sqrt()
}

fn load_project() -> (Project, Option<PathBuf>, Option<String>) {
    // 1) Argumento propio: `--abrir <ruta>` (o una ruta `*.redes` suelta).
    let extra = rune::extra_args();
    let mut pedido: Option<PathBuf> = None;
    let mut i = 0;
    while i < extra.len() {
        let a = &extra[i];
        if a == "--abrir" || a == "--project" || a == "-p" {
            if let Some(v) = extra.get(i + 1) {
                pedido = Some(PathBuf::from(v));
                i += 1;
            }
        } else if a.ends_with(".redes") {
            pedido = Some(PathBuf::from(a));
        }
        i += 1;
    }

    let mut candidatos: Vec<PathBuf> = pedido.into_iter().collect();
    if let Ok(env) = std::env::var("REDES_PROJECT") {
        candidatos.push(PathBuf::from(env));
    }
    cargar_de(candidatos)
}

/// Abre el primer candidato que funcione.
///
/// Si el usuario pidió un archivo y falla, **no** se cae al demo: se devuelve un proyecto vacío
/// con el motivo, para que la barra de estado lo muestre. Antes la app mostraba el proyecto de
/// demostración sin avisar y el usuario creía haber abierto el suyo.
fn cargar_de(candidatos: Vec<PathBuf>) -> (Project, Option<PathBuf>, Option<String>) {
    let pidieron_algo = !candidatos.is_empty();
    let mut ultimo: Option<String> = None;
    for ruta in candidatos {
        match redes_core::project_io::load(&ruta) {
            Ok(p) => return (p, Some(ruta), None),
            Err(e) => ultimo = Some(format!("No se pudo abrir {}: {e}", ruta.display())),
        }
    }
    if pidieron_algo {
        let vacio = Project::new("Sin proyecto", "EPSG:32717");
        return (vacio, None, ultimo);
    }
    (redes_core::demo::project(), None, None)
}

fn color(c: [f32; 4]) -> Color {
    Color {
        r: c[0],
        g: c[1],
        b: c[2],
        a: c[3],
    }
}

// ---------------------------------------------------------------------------------------------
// Vista

fn button(label: &str) -> El {
    el().padding(6.0)
        .radius(6.0)
        .fill(PANEL_HI)
        .border(1.0, LINE)
        .cursor(rune::Cursor::Pointer)
        .child(text(label).font(type_::LABEL).color(INK))
}

fn chip(label: &str, on: bool) -> El {
    el().padding(5.0)
        .radius(5.0)
        .fill(if on { PANEL_HI } else { PANEL })
        .border(1.0, LINE)
        .cursor(rune::Cursor::Pointer)
        .child(
            text(label)
                .font(type_::LABEL)
                .color(if on { INK } else { DIM }),
        )
}

fn kv(k: &str, v: String) -> El {
    row()
        .w_full()
        .justify_between()
        .gap(8.0)
        .child(text(k).font(type_::LABEL).color(DIM))
        .child(text(v).font(type_::MONO_CAPTION).color(INK))
}

fn section(t: &str) -> El {
    text(t).font(type_::SECTION).color(DIM).padding(4.0)
}

fn view(s: &State, _cx: &mut Cx<State>) -> El {
    // --- escena a un solo Shape ---
    let mut shape = Shape::new();
    for p in &s.scene.prims {
        match p {
            Prim::Poly { pts, fill, .. } => {
                if pts.len() >= 3 {
                    shape = shape.fill(pts.clone(), color(*fill));
                }
            }
            Prim::Line {
                a, b, w, color: c, ..
            } => {
                shape = shape.line(*a, *b, *w, LineCap::Round, color(*c));
            }
        }
    }
    let stage = canvas(if s.gpu3d { Shape::new() } else { shape })
        .key("plano3d")
        .w_full()
        .h_full()
        .on_layout(|s: &mut State, r: [f32; 4]| {
            let cambia = (s.viewport[0] - r[2]).abs() > 0.5 || (s.viewport[1] - r[3]).abs() > 0.5;
            s.viewport = [r[2], r[3]];
            s.canvas_rect = r;
            if !s.fitted && r[2] > 20.0 {
                s.fit();
            } else if cambia && r[2] > 20.0 {
                // La matriz vista-proyección usa el aspecto del lienzo: al redimensionar hay que
                // reconstruir, o el pase 3D queda estirado hasta el siguiente cambio.
                s.rebuild();
            }
        })
        .on_pointer_move_local(|s: &mut State, e, _tx| {
            s.cursor = [e.x, e.y];
        })
        .pointer_buttons([PointerButton::Middle, PointerButton::Secondary])
        .on_drag(|s: &mut State, d: &mut Drag, tx: &mut Tx| {
            let primary = tx.button() == PointerButton::Primary;
            match d.phase {
                Phase::Down => {
                    s.drag_last = [0.0, 0.0];
                }
                Phase::Move => {
                    // `Drag::dx/dy` son acumulados desde la pulsación: se aplica el incremento.
                    let dx = d.dx - s.drag_last[0];
                    let dy = d.dy - s.drag_last[1];
                    s.drag_last = [d.dx, d.dy];
                    if primary {
                        s.cam.orbit(dx * 0.35, -dy * 0.28);
                    } else {
                        s.cam.pan(dx, dy);
                    }
                    s.rebuild();
                    tx.live_only();
                }
                Phase::Up => {
                    s.drag_last = [0.0, 0.0];
                    // Un clic es una pulsación sin recorrido: selecciona en el punto soltado.
                    if primary && d.dx.abs() + d.dy.abs() < 5.0 {
                        s.sel = s.pick([d.x, d.y]);
                        s.rebuild();
                    }
                }
            }
        })
        .on_wheel(|s: &mut State, e: WheelEvent, _tx| {
            // Shift + rueda: mueve el plano de corte o abre la explosión.
            if e.mods.shift {
                if let Some(x) = s.opts.corte_x {
                    s.opts.corte_x = Some(x + (e.dy * 0.02) as f32);
                } else if s.opts.explosion > 0.0 {
                    s.opts.explosion = (s.opts.explosion + (e.dy * 0.004) as f32).clamp(0.0, 2.5);
                } else {
                    s.cam.dolly(((e.dy * 0.0015).exp()) as f64);
                }
            } else {
                // Rueda hacia arriba (dy negativo) acerca.
                s.cam.dolly(((e.dy * 0.0015).exp()) as f64);
            }
            s.rebuild();
            true
        });

    // Etiquetas: texto absoluto sobre el canvas, en coordenadas de pantalla.
    // La capa de etiquetas se superpone al canvas (pin a los cuatro bordes).
    let mut capa = el().pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0));
    for et in s.scene.etiquetas.iter().take(28) {
        let color_txt = if et.seleccion {
            OK
        } else {
            Color {
                r: 0.86,
                g: 0.88,
                b: 0.92,
                a: 0.92,
            }
        };
        capa = capa.child(
            el().absolute(et.pos[0] + 8.0, et.pos[1] - 6.0)
                .padding(2.0)
                .fill(Color {
                    r: 0.04,
                    g: 0.045,
                    b: 0.055,
                    a: 0.78,
                })
                .child(
                    text(et.texto.clone())
                        .font(type_::MONO_CAPTION)
                        .color(color_txt),
                ),
        );
    }

    let stage = stage;
    let viewport = el()
        .key("viewport")
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .clip_rounded(8.0)
        .fill(if s.gpu3d {
            // Transparente: deja ver el pase 3D que se dibuja debajo de la interfaz.
            Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.0,
            }
        } else {
            Color {
                r: 0.050,
                g: 0.055,
                b: 0.066,
                a: 1.0,
            }
        })
        .child(stage)
        .child(capa);

    // --- panel izquierdo ---
    let (pend, ok, obs) = s.project.revision_counts();
    let mut left = col()
        .w(214.0)
        .h_full()
        .padding(10.0)
        .gap(3.0)
        .fill(PANEL)
        .scroll(ScrollSpec {
            stick_bottom: false,
        });
    left = left
        .child(text("Revisión").font(type_::TITLE).color(INK))
        .child(
            row()
                .gap(6.0)
                .child(text(format!("{ok}")).font(type_::TITLE).color(OK))
                .child(text("aprobados").font(type_::LABEL).color(DIM)),
        )
        .child(
            row()
                .gap(6.0)
                .child(text(format!("{pend}")).font(type_::TITLE).color(WARN))
                .child(text("pendientes").font(type_::LABEL).color(DIM)),
        )
        .child(
            row()
                .gap(6.0)
                .child(text(format!("{obs}")).font(type_::TITLE).color(ERR))
                .child(text("observados").font(type_::LABEL).color(DIM)),
        )
        .child(el().h(10.0))
        .child(text("Capas").font(type_::TITLE).color(INK))
        .child(
            col()
                .gap(4.0)
                .child(chip("Terreno", s.layers.terreno).on_press(|s: &mut State| {
                    s.layers.terreno = !s.layers.terreno;
                    s.rebuild();
                }))
                .child(
                    chip("Alcantarillado", s.layers.alcantarillado).on_press(|s: &mut State| {
                        s.layers.alcantarillado = !s.layers.alcantarillado;
                        s.rebuild();
                    }),
                )
                .child(
                    chip("Agua potable", s.layers.agua).on_press(|s: &mut State| {
                        s.layers.agua = !s.layers.agua;
                        s.rebuild();
                    }),
                )
                .child(
                    chip("Edificios", s.layers.edificios)
                        .key("btn-edificios")
                        .on_press(|s: &mut State| {
                            s.layers.edificios = !s.layers.edificios;
                            s.rebuild();
                        }),
                ),
        )
        .child(el().h(10.0))
        .child(text("Leyenda").font(type_::TITLE).color(INK));

    // La leyenda usa forma + texto, no solo color: se lee también sin distinguir tonos.
    let (n_alc, n_agua) = contar_redes(&s.project);
    for (glifo, nombre, n, col) in [
        ("●", "Alcantarillado", n_alc, AL),
        ("◆", "Agua potable", n_agua, AP),
        ("◇", "Aprobado", s.project.revision_counts().1, OK),
        ("○", "Pendiente", s.project.revision_counts().0, WARN),
        ("✕", "Observado", s.project.revision_counts().2, ERR),
    ] {
        left = left.child(
            row()
                .gap(6.0)
                .items_center()
                .child(text(glifo).font(type_::TITLE).color(col))
                .child(
                    text(format!("{nombre} · {n}"))
                        .font(type_::LABEL)
                        .color(DIM),
                ),
        );
    }

    left = left
        .child(el().h(10.0))
        .child(text("Cámaras").font(type_::TITLE).color(INK));

    for (label, az, ev) in [
        // Azimut -90: se mira desde el sur, así que el norte queda arriba y el este a la derecha.
        ("Planta", -90.0f32, 88.0f32),
        ("Isométrica", -55.0, 32.0),
        ("Perfil", -90.0, 4.0),
    ] {
        left = left.child(chip(label, false).on_press(move |s: &mut State| {
            s.cam.azimuth = az;
            s.cam.elevation = ev;
            s.rebuild();
        }));
    }

    // --- panel derecho: inspector ---
    let mut right = col()
        .w(292.0)
        .h_full()
        .padding(12.0)
        .gap(6.0)
        .fill(PANEL)
        .scroll(ScrollSpec {
            stick_bottom: false,
        });
    right = right.child(text("Inspector").font(type_::TITLE).color(INK));

    let sel_node = s.sel.as_deref().and_then(|c| s.project.node_by_code(c));
    let sel_link = s.sel.as_deref().and_then(|c| s.project.link_by_code(c));
    match (sel_node, sel_link) {
        (Some(n), _) => {
            right = right.child(text(n.code.clone()).font(type_::TITLE).color(OK));
            right = right.child(kv("familia", family_label(&n.family).to_string()));
            right = right.child(kv("subtipo", n.ty.clone()));
            right = right.child(kv("estado", n.estado.key().to_string()));
            {
                let code_a = n.code.clone();
                let code_o = n.code.clone();
                right = right.child(
                    row()
                        .gap(4.0)
                        .child(
                            chip("Aprobar", n.estado == EstadoObjeto::Aprobado)
                                .key("btn-aprobar")
                                .on_press(move |s: &mut State| {
                                    s.set_estado(&code_a, EstadoObjeto::Aprobado, None)
                                }),
                        )
                        .child(
                            chip("Observar", n.estado == EstadoObjeto::Observado)
                                .key("btn-observar")
                                .on_press(move |s: &mut State| {
                                    s.set_estado(
                                        &code_o,
                                        EstadoObjeto::Observado,
                                        Some("Observado en revisión".to_string()),
                                    )
                                }),
                        ),
                );
                if let Some(nota) = &n.nota {
                    right = right.child(text(nota.clone()).font(type_::LABEL).color(WARN).wrap());
                }
            }
            right = right.child(el().h(6.0));
            right = right.child(section("Geometría"));
            right = right.child(kv("E", format!("{:.3}", n.pos.x)));
            right = right.child(kv("N", format!("{:.3}", n.pos.y)));
            right = right.child(kv("cota tapa", format!("{:.3} m", n.z_rim)));
            right = right.child(kv("cota clave", format!("{:.3} m", n.z_invert)));
            right = right.child(kv("altura", format!("{:.2} m", n.altura())));
            right = right.child(kv("diámetro", format!("{:.2} m", n.diametro())));
            right = right.child(el().h(6.0));
            right = right.child(section("Parámetros (en vivo)"));
            right = right.child(
                row()
                    .gap(4.0)
                    .child(
                        chip("− DN", false)
                            .key("btn-dn-menos")
                            .on_press(|s: &mut State| s.ajustar("dn", -25.0)),
                    )
                    .child(
                        chip("+ DN", false)
                            .key("btn-dn-mas")
                            .on_press(|s: &mut State| s.ajustar("dn", 25.0)),
                    )
                    .child(
                        chip("− Ø", false)
                            .key("btn-dia-menos")
                            .on_press(|s: &mut State| s.ajustar("diametro", -0.1)),
                    )
                    .child(
                        chip("+ Ø", false)
                            .key("btn-dia-mas")
                            .on_press(|s: &mut State| s.ajustar("diametro", 0.1)),
                    ),
            );
            right = right.child(
                row()
                    .gap(4.0)
                    .child(
                        chip("− prof.", false)
                            .key("btn-prof-menos")
                            .on_press(|s: &mut State| s.ajustar("profundidad", 0.25)),
                    )
                    .child(
                        chip("+ prof.", false)
                            .key("btn-prof-mas")
                            .on_press(|s: &mut State| s.ajustar("profundidad", -0.25)),
                    ),
            );

            if !n.datos.is_empty() {
                right = right.child(el().h(6.0));
                right = right.child(section("Datos importados (no calculados)"));
                for (k, d) in &n.datos {
                    right =
                        right.child(kv(k, format!("{:.2} {} · {}", d.valor, d.unidad, d.fuente)));
                }
            }
            if let Some(def) = family(&n.family) {
                right = right.child(el().h(6.0));
                right = right.child(section("Atributos"));
                for a in def.attrs.iter().take(10) {
                    let v = n
                        .attrs
                        .get(a.key)
                        .map(|v| match v {
                            families::Value::Num(x) => format!("{x:.3}"),
                            families::Value::Int(x) => x.to_string(),
                            families::Value::Bool(b) => b.to_string(),
                            families::Value::Text(t) => t.clone(),
                        })
                        .unwrap_or_else(|| "—".into());
                    right = right.child(kv(a.label, format!("{v} {}", a.unit.symbol())));
                }
            }
        }
        (None, Some(l)) => {
            right = right.child(text(l.code.clone()).font(type_::TITLE).color(OK));
            right = right.child(kv("familia", family_label(&l.family).to_string()));
            right = right.child(kv("subtipo", l.ty.clone()));
            right = right.child(kv("estado", l.estado.key().to_string()));
            {
                let code_a = l.code.clone();
                let code_o = l.code.clone();
                right = right.child(
                    row()
                        .gap(4.0)
                        .child(
                            chip("Aprobar", l.estado == EstadoObjeto::Aprobado)
                                .key("btn-aprobar")
                                .on_press(move |s: &mut State| {
                                    s.set_estado(&code_a, EstadoObjeto::Aprobado, None)
                                }),
                        )
                        .child(
                            chip("Observar", l.estado == EstadoObjeto::Observado)
                                .key("btn-observar")
                                .on_press(move |s: &mut State| {
                                    s.set_estado(
                                        &code_o,
                                        EstadoObjeto::Observado,
                                        Some("Observado en revisión".to_string()),
                                    )
                                }),
                        ),
                );
                if let Some(nota) = &l.nota {
                    right = right.child(text(nota.clone()).font(type_::LABEL).color(WARN).wrap());
                }
            }
            right = right.child(el().h(6.0));
            right = right.child(section("Geometría"));
            right = right.child(kv("DN", format!("{} mm", l.d_mm)));
            right = right.child(kv("longitud", format!("{:.2} m", l.length_2d())));
            if let (Some(a), Some(b)) = (l.path.first(), l.path.last()) {
                right = right.child(kv("cota inicial", format!("{:.3} m", a.z)));
                right = right.child(kv("cota final", format!("{:.3} m", b.z)));
            }
            if !l.datos.is_empty() {
                right = right.child(el().h(6.0));
                right = right.child(section("Datos importados (no calculados)"));
                for (k, d) in &l.datos {
                    right =
                        right.child(kv(k, format!("{:.2} {} · {}", d.valor, d.unidad, d.fuente)));
                }
            }
        }
        _ => {
            right = right.child(text("Nada seleccionado").font(type_::BODY).color(DIM));
            right = right.child(
                text("Gire con el botón izquierdo, desplace con el derecho, acerque con la rueda.")
                    .font(type_::LABEL)
                    .color(DIM)
                    .wrap(),
            );
            right = right.child(el().h(10.0));
            right = right.child(section("Proyecto"));
            right = right.child(kv("nombre", s.project.name.clone()));
            right = right.child(kv("CRS", s.project.crs.clone()));
            right = right.child(kv("estado", s.project.estado.key().to_string()));
            right = right.child(kv("objetos", s.project.count().to_string()));
            right = right.child(el().h(8.0));
            right = right.child(section("Procedencia"));
            for f in &s.project.fuentes {
                right = right.child(kv(&f.app, f.archivo.clone()));
            }
        }
    }

    // --- barra superior ---
    let header = row()
        .w_full()
        .padding(8.0)
        .gap(8.0)
        .items_center()
        .fill(PANEL)
        .child(text("Redes").font(type_::TITLE).color(INK))
        .child(text(s.project.name.clone()).font(type_::LABEL).color(DIM))
        .child(
            text(format!("· {} · {}", s.project.crs, s.project.estado.key()))
                .font(type_::LABEL)
                .color(DIM),
        )
        .child(el().grow(1.0))
        .child(
            button(if s.dirty { "Guardar *" } else { "Guardar" })
                .key("btn-guardar")
                .on_press(|s: &mut State| s.save()),
        )
        .child(
            button("Encuadrar")
                .key("btn-encuadrar")
                .on_press(|s: &mut State| s.fit()),
        )
        .child(
            button("Planta")
                .key("btn-planta")
                .on_press(|s: &mut State| {
                    s.cam.azimuth = -90.0;
                    s.cam.elevation = 88.0;
                    s.rebuild();
                }),
        )
        .child(
            button("Isométrica")
                .key("btn-iso")
                .on_press(|s: &mut State| {
                    s.cam.azimuth = -55.0;
                    s.cam.elevation = 32.0;
                    s.rebuild();
                }),
        )
        .child(
            button("Foco")
                .key("btn-foco")
                .on_press(|s: &mut State| s.enfocar()),
        )
        .child(
            button(if s.opts.corte_x.is_some() {
                "Corte *"
            } else {
                "Corte"
            })
            .key("btn-corte")
            .on_press(|s: &mut State| s.alternar_corte()),
        )
        .child(
            button(if s.opts.rayos_x {
                "Rayos X *"
            } else {
                "Rayos X"
            })
            .key("btn-rayos-x")
            .on_press(|s: &mut State| s.alternar_rayos_x()),
        )
        .child(
            button(if s.opts.explosion > 0.0 {
                "Explosión *"
            } else {
                "Explosión"
            })
            .key("btn-explosion")
            .on_press(|s: &mut State| s.alternar_explosion()),
        )
        .child(
            button("Salir")
                .key("btn-salir")
                .on_press(|s: &mut State| s.salir_modo()),
        )
        .child(
            button("Deshacer")
                .key("btn-deshacer")
                .on_press(|s: &mut State| s.deshacer()),
        );

    // --- barra de estado ---
    let status = row()
        .w_full()
        .padding(6.0)
        .gap(16.0)
        .items_center()
        .fill(PANEL)
        .child(
            text(format!(
                "az {:.0}°  el {:.0}°  d {:.0} m",
                s.cam.azimuth, s.cam.elevation, s.cam.distance
            ))
            .font(type_::MONO_CAPTION)
            .color(DIM),
        )
        .child(
            text(format!(
                "{} objetos · {} malla · {} líneas · {} etq",
                s.project.count(),
                s.scene.tri_count,
                s.scene.line_count,
                s.scene.etiquetas.len()
            ))
            .font(type_::MONO_CAPTION)
            .color(DIM),
        )
        .child(el().grow(1.0))
        .child(
            text(format!(
                "{}{}",
                if s.opts.foco.is_some() {
                    "foco · "
                } else {
                    ""
                },
                if s.opts.corte_x.is_some() {
                    "corte · "
                } else if s.opts.explosion > 0.0 {
                    "explosión · "
                } else {
                    ""
                },
            ))
            .font(type_::MONO_CAPTION)
            .color(OK),
        )
        .child(
            text(
                s.aviso_modelos
                    .clone()
                    .or_else(|| s.aviso.clone())
                    .unwrap_or_default(),
            )
            .font(type_::LABEL)
            .color(DIM),
        )
        .child(
            text(if s.dirty { "cambios sin guardar" } else { "" })
                .font(type_::LABEL)
                .color(WARN),
        )
        .child(
            text(if obs_count(&s.project) > 0 {
                "hay observaciones"
            } else {
                "revisión al día"
            })
            .font(type_::LABEL)
            .color(if obs_count(&s.project) > 0 { ERR } else { DIM }),
        );

    col()
        .w_full()
        .h_full()
        // Con el pase 3D, el fondo lo pone el compositor: si el contenedor pintara, taparía el
        // modelo antes de que se dibujara la interfaz.
        .fill(if s.gpu3d {
            Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.0,
            }
        } else {
            BG
        })
        .child(header)
        .child(
            row()
                .w_full()
                .grow(1.0)
                .min_h(0.0)
                .padding(8.0)
                .gap(8.0)
                .child(left)
                .child(viewport)
                .child(right),
        )
        .child(status)
}

/// Cuenta objetos por red (para la leyenda).
fn contar_redes(p: &Project) -> (usize, usize) {
    let n = p.nodes.iter().filter(|n| !n.deleted).count();
    let l = p.links.iter().filter(|l| !l.deleted).count();
    let agua = p
        .nodes
        .iter()
        .filter(|n| !n.deleted && n.family.starts_with("agua"))
        .count()
        + p.links
            .iter()
            .filter(|l| !l.deleted && l.family.starts_with("agua"))
            .count();
    (n + l - agua, agua)
}

fn obs_count(p: &Project) -> usize {
    p.nodes
        .iter()
        .filter(|n| !n.deleted && n.estado == EstadoObjeto::Observado)
        .count()
        + p.links
            .iter()
            .filter(|l| !l.deleted && l.estado == EstadoObjeto::Observado)
            .count()
}

fn family_label(key: &str) -> &'static str {
    family(key).map(|f| f.label).unwrap_or("Otro")
}

// ---------------------------------------------------------------------------------------------
// Arranque

fn main() {
    let spec = AppSpec {
        scenes: &[
            (
                "iso",
                (|s: &mut State| {
                    s.cam.azimuth = -55.0;
                    s.cam.elevation = 32.0;
                    s.fitted = true;
                    s.sel = s
                        .project
                        .nodes
                        .iter()
                        .find(|n| !n.deleted)
                        .map(|n| n.code.clone());
                    s.rebuild();
                }) as fn(&mut State),
            ),
            (
                "ptap",
                (|s: &mut State| {
                    // La planta de tratamiento: mezcla, floculación, sedimentación, filtración y
                    // desinfección en fila, con las casetas y el lecho de secado.
                    s.cam.target = [238.0, 158.0, 2516.0];
                    s.cam.distance = 95.0;
                    s.cam.azimuth = -62.0;
                    s.cam.elevation = 26.0;
                    s.fitted = true;
                    s.rebuild();
                }) as fn(&mut State),
            ),
            (
                "ptar",
                (|s: &mut State| {
                    // La salida del alcantarillado: reja, desarenador, reactor, sedimentador y laguna.
                    s.cam.target = [500.0, 74.0, 2499.0];
                    s.cam.distance = 130.0;
                    s.cam.azimuth = -70.0;
                    s.cam.elevation = 22.0;
                    s.fitted = true;
                    s.rebuild();
                }) as fn(&mut State),
            ),
            (
                "planta",
                (|s: &mut State| {
                    s.cam.azimuth = -90.0;
                    s.cam.elevation = 88.0;
                    // En planta se encuadra el proyecto completo: es la vista de revisión general.
                    s.fitted = false;
                    s.fit();
                    s.fitted = true;
                }) as fn(&mut State),
            ),
            (
                "detalle",
                (|s: &mut State| {
                    // Primer plano del troncal: se ven los pozos como cilindros.
                    s.cam.target = [320.0, 95.0, 2511.0];
                    s.cam.distance = 140.0;
                    s.cam.azimuth = -68.0;
                    s.cam.elevation = 12.0;
                    s.fitted = true;
                    s.sel = s
                        .project
                        .nodes
                        .iter()
                        .find(|n| !n.deleted)
                        .map(|n| n.code.clone());
                    s.rebuild();
                }) as fn(&mut State),
            ),
            (
                "ciudad",
                (|s: &mut State| {
                    s.cam.azimuth = -62.0;
                    s.cam.elevation = 26.0;
                    s.cam.z_scale = 2.6;
                    s.fitted = true;
                    s.fit();
                    s.cam.azimuth = -62.0;
                    s.cam.elevation = 26.0;
                    s.rebuild();
                }) as fn(&mut State),
            ),
            (
                "calle",
                (|s: &mut State| {
                    // Cámara a nivel de calle sobre el pozo más céntrico del troncal.
                    let centro = s.project.bbox().center();
                    let elegido = s
                        .project
                        .nodes
                        .iter()
                        .filter(|n| !n.deleted && n.family == "saneamiento.pozo")
                        .min_by(|a, b| a.pos.dist(centro).total_cmp(&b.pos.dist(centro)))
                        .map(|n| (n.code.clone(), n.pos, n.z_rim));
                    if let Some((code, pos, z)) = elegido {
                        s.cam.target = [pos.x - s.project.origin.x, pos.y - s.project.origin.y, z];
                        s.cam.distance = 165.0;
                        s.cam.azimuth = -90.0;
                        s.cam.elevation = 27.0;
                        s.sel = Some(code);
                    }
                    s.fitted = true;
                    s.rebuild();
                }) as fn(&mut State),
            ),
            (
                "catalogo",
                (|s: &mut State| {
                    // Vista de catálogo: las dos hileras de piezas, cerca.
                    // Hilera de pozos, vista amplia: seis tipos juntos.
                    s.cam.target = [32.0, 36.0, 2502.4];
                    s.cam.distance = 30.0;
                    s.cam.azimuth = 118.0;
                    s.cam.elevation = 22.0;
                    s.fitted = true;
                    s.rebuild();
                }) as fn(&mut State),
            ),
            (
                "vitrina",
                (|s: &mut State| {
                    // Las cuatro hileras del catálogo, desde arriba.
                    s.cam.target = [32.0, 24.0, 2501.6];
                    s.cam.distance = 52.0;
                    s.cam.azimuth = -58.0;
                    s.cam.elevation = 34.0;
                    s.fitted = true;
                    s.rebuild();
                }) as fn(&mut State),
            ),
            (
                "pozos",
                (|s: &mut State| {
                    // Hilera de pozos: se ven cono, marco, tapa y escalera.
                    s.cam.target = [32.0, 36.0, 2502.6];
                    s.cam.distance = 16.0;
                    s.cam.azimuth = -64.0;
                    s.cam.elevation = 22.0;
                    s.sel = s
                        .project
                        .nodes
                        .iter()
                        .find(|n| !n.deleted && n.family == "saneamiento.pozo")
                        .map(|n| n.code.clone());
                    s.fitted = true;
                    s.rebuild();
                }) as fn(&mut State),
            ),
            (
                "pieza",
                (|s: &mut State| {
                    // Primer plano de una válvula: se ven cuerpo, bridas y volante.
                    let objetivo = s
                        .project
                        .nodes
                        .iter()
                        .filter(|n| !n.deleted && n.family == "agua.valvula")
                        .map(|n| (n.code.clone(), n.pos, n.z_rim))
                        .next();
                    if let Some((code, pos, z)) = objetivo {
                        s.cam.target = [pos.x - s.project.origin.x, pos.y - s.project.origin.y, z];
                        s.cam.distance = 4.5;
                        s.cam.azimuth = -48.0;
                        s.cam.elevation = 18.0;
                        s.sel = Some(code);
                    }
                    s.fitted = true;
                    s.rebuild();
                }) as fn(&mut State),
            ),
            (
                "foco",
                (|s: &mut State| {
                    s.sel = s
                        .project
                        .nodes
                        .iter()
                        .find(|n| !n.deleted && n.family == "saneamiento.pozo")
                        .map(|n| n.code.clone());
                    s.enfocar();
                }) as fn(&mut State),
            ),
            (
                "corte",
                (|s: &mut State| {
                    s.sel = s
                        .project
                        .nodes
                        .iter()
                        .find(|n| !n.deleted && n.family == "saneamiento.pozo")
                        .map(|n| n.code.clone());
                    s.enfocar();
                    s.alternar_corte();
                    // Un paso de corte para que se vea el interior.
                    if let Some(x) = s.opts.corte_x {
                        s.opts.corte_x = Some(x + 0.15);
                    }
                    s.cam.azimuth = -20.0;
                    s.cam.elevation = 12.0;
                    s.rebuild();
                }) as fn(&mut State),
            ),
            (
                "explosion",
                (|s: &mut State| {
                    s.sel = s
                        .project
                        .nodes
                        .iter()
                        .find(|n| !n.deleted && n.family == "saneamiento.pozo")
                        .map(|n| n.code.clone());
                    s.enfocar();
                    s.alternar_explosion();
                    s.opts.explosion = 1.1;
                    s.rebuild();
                }) as fn(&mut State),
            ),
            (
                "perfil",
                (|s: &mut State| {
                    s.cam.azimuth = -90.0;
                    s.cam.elevation = 4.0;
                    s.fitted = true;
                    s.rebuild();
                }) as fn(&mut State),
            ),
        ],
        ..AppSpec::new("Redes — visor 3D", (1280.0, 800.0), State::new, view)
    };
    if gpu3d::activo() {
        rune::run_with(spec, gpu3d::Compositor3d::new());
    } else {
        rune::run(spec);
    }
}

#[allow(dead_code)]
fn caps_of(family_key: &str) -> u32 {
    family(family_key).map(|f| f.caps).unwrap_or(0)
}

#[allow(dead_code)]
fn is_viewable(family_key: &str) -> bool {
    family(family_key)
        .map(|f| f.can(caps::HYDRAULIC_NODE) || f.can(caps::ATTACHABLE))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rune::{Env, Runtime};

    /// Un clic en el chip del inspector cambia el estado y marca el documento como sucio.
    #[test]
    fn observar_desde_el_inspector_cambia_el_estado() {
        let mut st = State::new();
        st.sel = Some("PZ-001".to_string());
        st.rebuild();
        let mut rt = Runtime::new(
            st,
            view,
            (1280.0, 800.0),
            Env {
                system_dark: true,
                scale: 1.0,
            },
        );
        rt.frame(0.0);
        rt.frame(1.0 / 60.0);
        let (x, y) = rt
            .find("btn-observar")
            .expect("el chip Observar debe estar montado");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(1.0 / 60.0);
        let n = rt
            .state
            .project
            .node_by_code("PZ-001")
            .expect("PZ-001 existe");
        assert_eq!(
            n.estado,
            EstadoObjeto::Observado,
            "el clic debe observar el pozo"
        );
        assert!(n.nota.is_some(), "la observacion deja nota");
        assert!(rt.state.dirty, "queda un cambio sin guardar");
    }

    /// Foco: la cámara se acerca a la pieza y queda marcada como enfocada.
    #[test]
    fn el_boton_foco_acerca_la_camara() {
        let mut st = State::new();
        st.sel = Some("PZ-001".to_string());
        st.rebuild();
        let mut rt = Runtime::new(
            st,
            view,
            (1280.0, 800.0),
            Env {
                system_dark: true,
                scale: 1.0,
            },
        );
        rt.frame(0.0);
        let (x, y) = rt
            .find("btn-foco")
            .expect("el botón Foco debe estar montado");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(1.0 / 60.0);
        assert!(rt.state.opts.foco.is_some(), "queda un objeto enfocado");
        assert!(
            rt.state.cam.distance < 20.0,
            "la cámara se acerca: {}",
            rt.state.cam.distance
        );
        assert!(
            (rt.state.cam.z_scale - 1.0).abs() < 1e-9,
            "en foco se dibuja a escala real"
        );
    }

    /// Corte: el plano se activa y se puede desactivar.
    #[test]
    fn el_boton_corte_activa_y_desactiva_el_plano() {
        let mut st = State::new();
        st.sel = Some("PZ-001".to_string());
        st.rebuild();
        let mut rt = Runtime::new(
            st,
            view,
            (1280.0, 800.0),
            Env {
                system_dark: true,
                scale: 1.0,
            },
        );
        rt.frame(0.0);
        let (x, y) = rt
            .find("btn-corte")
            .expect("el botón Corte debe estar montado");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(1.0 / 60.0);
        assert!(rt.state.opts.corte_x.is_some(), "el corte queda activo");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(1.0 / 60.0);
        assert!(rt.state.opts.corte_x.is_none(), "el segundo clic lo apaga");
    }

    /// Explosión: las piezas del objeto enfocado se separan.
    #[test]
    fn el_boton_explosion_separa_la_pieza() {
        let mut st = State::new();
        st.sel = Some("PZ-001".to_string());
        st.rebuild();
        let mut rt = Runtime::new(
            st,
            view,
            (1280.0, 800.0),
            Env {
                system_dark: true,
                scale: 1.0,
            },
        );
        rt.frame(0.0);
        let (x, y) = rt
            .find("btn-explosion")
            .expect("el botón Explosión debe estar montado");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(1.0 / 60.0);
        assert!(rt.state.opts.explosion > 0.0, "la explosión queda activa");
    }

    /// Edición paramétrica: + DN cambia el atributo, marca sucio y Deshacer lo revierte.
    #[test]
    fn los_parametros_se_editan_en_vivo_y_se_deshacen() {
        let mut st = State::new();
        st.sel = Some("PZ-001".to_string());
        st.rebuild();
        let mut rt = Runtime::new(
            st,
            view,
            (1280.0, 800.0),
            Env {
                system_dark: true,
                scale: 1.0,
            },
        );
        rt.frame(0.0);
        let (x, y) = rt
            .find("btn-dn-mas")
            .expect("el chip + DN debe estar montado");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(1.0 / 60.0);

        let n = rt
            .state
            .project
            .node_by_code("PZ-001")
            .expect("PZ-001 existe");
        assert_eq!(n.num("diametro_nominal"), Some(225.0), "el DN sube 25 mm");
        assert!(rt.state.dirty, "el cambio deja el documento sucio");
        assert_eq!(rt.state.historial.len(), 1, "se apila un paso de deshacer");

        let (x, y) = rt
            .find("btn-deshacer")
            .expect("el botón Deshacer debe estar montado");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(1.0 / 60.0);
        let n = rt
            .state
            .project
            .node_by_code("PZ-001")
            .expect("PZ-001 existe");
        assert_eq!(
            n.num("diametro_nominal"),
            None,
            "Deshacer revierte el parámetro"
        );
    }

    /// Salir vuelve al modo normal.
    #[test]
    fn salir_del_modo_vuelve_a_la_vista_general() {
        let mut st = State::new();
        st.sel = Some("PZ-001".to_string());
        st.rebuild();
        let mut rt = Runtime::new(
            st,
            view,
            (1280.0, 800.0),
            Env {
                system_dark: true,
                scale: 1.0,
            },
        );
        rt.frame(0.0);
        let (x, y) = rt.find("btn-corte").expect("Corte montado");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(1.0 / 60.0);
        let (x, y) = rt.find("btn-salir").expect("Salir montado");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(1.0 / 60.0);
        assert!(rt.state.opts.corte_x.is_none());
        assert!(rt.state.opts.foco.is_none());
        assert!(rt.state.cam.distance > 20.0, "vuelve al encuadre general");
    }

    /// Abrir un archivo que no existe no debe caer en silencio al proyecto de demostración.
    #[test]
    fn abrir_un_archivo_inexistente_no_cae_al_demo() {
        let (p, ruta, error) = cargar_de(vec![PathBuf::from("no-existe-de-verdad.redes")]);
        assert!(ruta.is_none());
        assert_eq!(p.count(), 0, "no debe cargar el demo");
        let msg = error.expect("debe explicar el fallo");
        assert!(msg.contains("no-existe-de-verdad.redes"), "{msg}");
    }

    /// Sin candidatos sí se carga el demo (primer arranque sin proyecto).
    #[test]
    fn sin_candidatos_carga_el_demo() {
        let (p, ruta, error) = cargar_de(Vec::new());
        assert!(ruta.is_none() && error.is_none());
        assert!(p.count() > 0, "el demo debe tener objetos");
    }

    /// Sin seleccion no hay botones de revision.
    #[test]
    fn sin_seleccion_no_hay_chips() {
        let st = State::new();
        let mut rt = Runtime::new(
            st,
            view,
            (1280.0, 800.0),
            Env {
                system_dark: true,
                scale: 1.0,
            },
        );
        rt.frame(0.0);
        assert!(rt.find("btn-observar").is_none());
    }
}
