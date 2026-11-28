//! Redes — visor 3D (Rune).
//!
//! No calcula nada: dibuja el modelo que importó la CLI, deja revisarlo y aprobarlo, y lo
//! entrega en DXF. La escena 3D sale de `redes_core::view`, la misma que usa `redes vista`.
//!
//! `cargo run -p redes-app` · `-- --snap vista.png --scene iso` · `-- --tree arbol.json`

mod gpu3d;
mod modelos;
mod observaciones;
mod perfil_panel;
mod validacion;

use std::sync::{Arc, Mutex};

use redes_core::families::{self, family};
use redes_core::model::{EstadoObjeto, Project};
use redes_core::seleccion::RejillaSeleccion;
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

/// Rejilla (m) del proxy de los modelos de biblioteca sin pase 3D (F6-05).
const PASO_PROXY: f32 = 0.05;

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
    /// Undo/redo of the review and of the assigned model: 20 steps, by differences (F14-08).
    historial: redes_core::undo::History,
    /// Aviso breve en la barra de estado.
    aviso: Option<String>,
    /// Pase 3D con profundidad activo (fase 5).
    gpu3d: bool,
    /// Geometría compartida con el compositor GPU.
    geom: Arc<Mutex<gpu3d::Geometry>>,
    /// De qué se generó la malla del pase 3D: si no cambia, la cámara no la regenera (F14-02).
    clave_malla: Option<ClaveMalla>,
    /// Rectángulo del lienzo en puntos (x, y, ancho, alto): el pase 3D se limita a él.
    canvas_rect: [f32; 4],
    /// Caché de mallas de la biblioteca (un `.glb` se lee una vez).
    modelos: modelos::Cache,
    /// Mallas paramétricas por pieza (F14-06): una pieza sin cambios no se vuelve a generar.
    piezas: redes_core::mallas::CachePiezas,
    /// Manifiesto del proyecto y su carpeta, si declara biblioteca. En `Arc`: se comparte, no se
    /// clona en cada reconstrucción (F14-06).
    bib: Option<(std::path::PathBuf, Arc<redes_core::biblioteca::Biblioteca>)>,
    /// Aviso de política de dibujo (modelos pesados sin `--gpu3d`).
    aviso_modelos: Option<String>,
    /// Aviso del pase 3D cuando la escena no cabe en el tope de GPU (el compositor baja el tope
    /// al límite del dispositivo y regenera la escena).
    aviso_gpu: Option<String>,
    /// Perfil longitudinal abierto desde el inspector de un tramo (F13-02).
    perfil: Option<redes_core::perfil::Perfil>,
    /// Última validación geométrica (F13-01).
    validacion: validacion::Panel,
    /// Campo de texto del inspector para observar o cerrar observaciones (F13-03).
    obs_texto: rune::TextEdit,
    /// Rejilla de selección de la cámara actual (F14-07); `None` tras cada `rebuild`.
    rejilla: Option<RejillaSeleccion>,
}

/// Lo que define la malla del pase 3D. La cámara, la selección y los modos de detalle no entran:
/// la malla se dibuja con su color de material y la exageración vertical va en la matriz.
#[derive(Clone, Copy, Debug, PartialEq)]
struct ClaveMalla {
    /// Identidad del proyecto: cambia al deshacer o al sustituirlo entero.
    proyecto: usize,
    /// Revisión del journal: cambia con cada edición confirmada, aunque el `Arc` sea el mismo.
    revision: u64,
    terreno: bool,
    agua: bool,
    alcantarillado: bool,
    edificios: bool,
    /// La biblioteca se resuelve al primer `rebuild`: al llegar hay que añadir sus modelos.
    biblioteca: bool,
}

impl ClaveMalla {
    fn de(s: &State) -> ClaveMalla {
        ClaveMalla {
            proyecto: Arc::as_ptr(&s.project) as usize,
            revision: s.project.revision,
            terreno: s.layers.terreno,
            agua: s.layers.agua,
            alcantarillado: s.layers.alcantarillado,
            edificios: s.layers.edificios,
            biblioteca: s.bib.is_some(),
        }
    }
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
        // La escena se arma en el `rebuild` del final, ya con el modo de dibujo decidido.
        let scene = Scene::default();
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
            historial: Default::default(),
            gpu3d: gpu3d::activo(),
            aviso: error_carga,
            geom: Arc::new(Mutex::new(gpu3d::Geometry::default())),
            clave_malla: None,
            // Estimación para el primer frame (el layout aún no ha corrido): 1280×800 con los
            // paneles de 214 y 292 pt. `on_layout` la sustituye por el rectángulo real.
            canvas_rect: [230.0, 51.0, 742.0, 714.0],
            modelos: modelos::Cache::default(),
            piezas: Default::default(),
            bib: None,
            aviso_modelos: None,
            aviso_gpu: None,
            perfil: None,
            validacion: Default::default(),
            obs_texto: rune::TextEdit::new(""),
            rejilla: None,
        };
        // Modo rayos X por línea de comandos (útil para capturas y para los agentes).
        if rune::extra_args().iter().any(|a| a == "--rayos-x") {
            estado.opts.rayos_x = true;
        }
        estado.rebuild();
        estado
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

    /// Pone la vista al día tras cualquier cambio (cámara, capas, selección o proyecto).
    ///
    /// Sin pase 3D, el pintor necesita la escena entera proyectada para cada cámara. Con
    /// `--gpu3d` la escena del pintor no se dibuja: solo se recalculan las etiquetas y la matriz,
    /// y la malla se regenera únicamente si cambió lo que la define (F14-02).
    fn rebuild(&mut self) {
        self.cam.viewport = self.viewport;
        // La selección se proyecta con esta cámara y este proyecto: se rehace al primer clic.
        self.rejilla = None;
        if self.gpu3d {
            self.scene = Scene {
                etiquetas: view::etiquetas(
                    &self.project,
                    &self.cam,
                    self.layers,
                    self.sel.as_deref(),
                ),
                ..Scene::default()
            };
        }
        // Manifiesto de la biblioteca: se resuelve una vez por proyecto.
        if self.bib.is_none() && self.project.biblioteca.is_some() {
            let dir = self
                .path
                .as_ref()
                .and_then(|p| p.parent())
                .map(|d| d.to_path_buf())
                .unwrap_or_else(|| std::path::PathBuf::from("."));
            self.bib = modelos::biblioteca_del_proyecto(&self.project, &dir);
        }
        let biblioteca_rota = self.bib.is_none() && self.project.biblioteca.is_some();
        self.aviso_modelos = biblioteca_rota
            .then(|| "El proyecto declara una biblioteca que no se pudo abrir".to_string());

        if !self.gpu3d {
            let (modelos, simplificados) = self.piezas_pintor();
            self.scene = view::build_with_models(
                &self.project,
                self.display.as_ref(),
                &self.cam,
                self.layers,
                self.sel.as_deref(),
                &self.opts,
                &modelos,
            );
            if simplificados > 0 {
                self.aviso_modelos = Some(format!(
                    "{simplificados} objetos tienen un modelo de más de {} triángulos: sin el pase 3D se ven simplificados. Arranque con --gpu3d para verlos enteros.",
                    modelos::TRIANGULOS_PINTOR
                ));
            }
        }

        if self.gpu3d {
            let clave = ClaveMalla::de(self);
            let instancias = if self.clave_malla != Some(clave) {
                match &self.bib {
                    Some((carpeta, bib)) => Some(modelos::instancias(
                        &self.project,
                        carpeta,
                        bib,
                        &mut self.modelos,
                    )),
                    None => Some(Vec::new()),
                }
            } else {
                None
            };
            if let Ok(mut g) = self.geom.lock() {
                if let Some(instancias) = instancias {
                    g.load_with_models(&self.project, self.layers, &instancias, &mut self.piezas);
                    self.clave_malla = Some(clave);
                    self.aviso_gpu = g.scene.warning.clone();
                }
                g.set_camera(&self.cam);
                // La barra de estado cuenta los triángulos que dibuja la GPU con esta cámara.
                self.scene.tri_count = g.plan.triangles as usize;
            }
        }
    }

    /// Replaces the project with an edited copy and records the difference for undo.
    fn commit_edit(&mut self, edited: Project) {
        self.historial.record(&self.project, &edited);
        self.project = Arc::new(edited);
        self.dirty = true;
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
        let (pos, rim, inv, diam) = (n.pos, n.z_rim, n.z_invert, n.drawing_diameter_m());
        let id = n.id;
        let modelo_id = n.modelo.clone();
        let alto = (rim - inv).max(0.4);
        let tam = diam.max(alto);

        // Si el objeto tiene modelo, se encuadra el modelo, no la pieza paramétrica: una tapa
        // modelada de 3 m no cabe con el encuadre de un pozo de 1,2 m.
        let mut centro = [
            pos.x - self.project.origin.x,
            pos.y - self.project.origin.y,
            rim,
        ];
        let mut tamano = tam;
        if let (Some(modelo_id), Some((carpeta, bib))) = (modelo_id, self.bib.clone())
            && let Some(pieza) = bib.piezas.iter().find(|x| x.id == modelo_id).cloned()
            && let Some(malla) = self.modelos.malla(&carpeta, &pieza)
            && let Some(n) = self.project.node_by_code(&code)
        {
            let base = redes_core::biblioteca::cota_base_modelo(n, &pieza.anclas);
            let (mut min, mut max) = ([f32::MAX; 3], [f32::MIN; 3]);
            for v in malla.iter() {
                for k in 0..3 {
                    min[k] = min[k].min(v.pos[k]);
                    max[k] = max[k].max(v.pos[k]);
                }
            }
            if min[0] < max[0] {
                centro = [
                    (pos.x - self.project.origin.x) + ((min[0] + max[0]) * 0.5) as f64,
                    (pos.y - self.project.origin.y) + ((min[1] + max[1]) * 0.5) as f64,
                    base + ((min[2] + max[2]) * 0.5) as f64,
                ];
                tamano = ((max[0] - min[0]).hypot(max[1] - min[1])).max(max[2]) as f64;
            }
        }
        self.cam.target = centro;
        self.cam.distance = (tamano * 3.4).clamp(2.5, 400.0);
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

    /// The library models the painter draws in place of the parametric pieces (no `--gpu3d`).
    ///
    /// The library is shared by `Arc` and each piece is shaded (or decimated) once (F14-06).
    fn piezas_pintor(&mut self) -> (view::ModelPieces, usize) {
        match self.bib.clone() {
            Some((carpeta, bib)) => {
                modelos::piezas_pintor(&self.project, &carpeta, &bib, &mut self.modelos, PASO_PROXY)
            }
            None => (view::ModelPieces::new(), 0),
        }
    }

    /// Asigna un modelo de la biblioteca al objeto seleccionado y le pasa sus medidas.
    fn aplicar_modelo(&mut self, modelo: &str) {
        let Some((_, bib)) = self.bib.clone() else {
            self.aviso = Some("El proyecto no tiene biblioteca de modelos".into());
            return;
        };
        let Some(pieza) = bib.piezas.iter().find(|x| x.id == modelo).cloned() else {
            return;
        };
        let Some(code) = self.sel.clone() else { return };
        // Se copia el proyecto para no dejar el historial a medias si algo falla.
        let mut nuevo = (*self.project).clone();
        let conservados = {
            let Some(n) = nuevo.node_by_code_mut(&code) else {
                return;
            };
            redes_core::biblioteca::aplicar_a(n, &pieza, 2.4)
        };
        // Los arcos que tocan el objeto entran por el puerto del modelo, si lo declara.
        let anclados = redes_core::biblioteca::anclar_conexiones(&mut nuevo, &code, &pieza.anclas);
        nuevo.commit("ui.modelo");
        self.commit_edit(nuevo);
        self.aviso = Some(if conservados.is_empty() {
            if anclados > 0 {
                format!("Modelo {modelo} aplicado a {code}; {anclados} extremos anclados")
            } else {
                format!("Modelo {modelo} aplicado a {code}")
            }
        } else {
            format!(
                "Modelo {modelo} aplicado; {} datos conservados por otra fuente",
                conservados.len()
            )
        });
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
            if self.opts.foco.is_none()
                && let Some(code) = self.sel.clone()
            {
                self.opts.foco = self.project.node_by_code(&code).map(|n| n.id);
            }
            self.opts.explosion = 0.9;
            self.aviso = Some("Explosión activa: Shift + rueda la abre".into());
        }
        self.rebuild();
    }

    fn deshacer(&mut self) {
        // In place: the step restores only what changed, and its commit moves `revision`, so
        // `ClaveMalla` still sees the change.
        if self.historial.undo(Arc::make_mut(&mut self.project)) {
            self.after_history_step("Deshecho");
        } else {
            self.aviso = Some("Nada que deshacer".into());
        }
    }

    fn rehacer(&mut self) {
        if self.historial.redo(Arc::make_mut(&mut self.project)) {
            self.after_history_step("Rehecho");
        } else {
            self.aviso = Some("Nada que rehacer".into());
        }
    }

    fn after_history_step(&mut self, notice: &str) {
        self.dirty = true;
        self.aviso = Some(notice.into());
        self.rebuild();
    }

    fn fit(&mut self) {
        self.cam.fit(&self.project);
        self.rebuild();
        self.fitted = true;
    }

    /// Selecciona el objeto más cercano al puntero en pantalla.
    ///
    /// Con la rejilla de selección (F14-07): se arma una vez por cámara (`rebuild` la invalida)
    /// y cada clic mira solo los objetos de su celda.
    fn pick(&mut self, at: [f32; 2]) -> Option<String> {
        let rejilla = self
            .rejilla
            .get_or_insert_with(|| RejillaSeleccion::nueva(&self.project, &self.cam));
        rejilla.elegir(at).map(str::to_string)
    }
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

/// Un valor en metros, o `—` si el origen no lo trajo (F12-03).
fn metros(v: Option<f64>, decimales: usize) -> String {
    v.map(|x| format!("{x:.decimales$} m"))
        .unwrap_or_else(|| redes_core::model::SIN_DATO.to_string())
}

/// The declared inner diameter, or `—`: the fallback drawing size is not data (D4).
fn inner_diameter_text(n: &redes_core::model::Node) -> String {
    metros(n.inner_diameter_m(), 2)
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
    // --- escena a un solo Shape (con `--gpu3d` la escena no trae primitivas: queda vacío) ---
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
    let stage = canvas(shape)
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
                    s.opts.corte_x = Some(x + (e.dy * 0.02));
                } else if s.opts.explosion > 0.0 {
                    s.opts.explosion = (s.opts.explosion + (e.dy * 0.004)).clamp(0.0, 2.5);
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
    left = left.child(el().h(10.0)).child(validacion::seccion(s));

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
                                .on_press(move |s: &mut State| s.aprobar(&code_a)),
                        )
                        .child(
                            chip("Observar", n.estado == EstadoObjeto::Observado)
                                .key("btn-observar")
                                .on_press(move |s: &mut State| s.observar(&code_o)),
                        ),
                );
                right = right.child(observaciones::panel(s, &n.code, &n.observaciones));
            }
            right = right.child(el().h(6.0));
            right = right.child(section("Geometría"));
            right = right.child(kv("E", format!("{:.3}", n.pos.x)));
            right = right.child(kv("N", format!("{:.3}", n.pos.y)));
            right = right.child(kv("cota terreno", metros(n.cota_terreno(), 3)));
            right = right.child(kv("cota tapa", metros(n.cota_tapa(), 3)));
            right = right.child(kv("cota solera", metros(n.cota_solera(), 3)));
            right = right.child(kv("altura", metros(n.altura_dato(), 2)));
            right = right.child(kv("diámetro", inner_diameter_text(n)));
            // Modelos de la biblioteca para esta familia: se eligen aquí, sin tocar la CLI.
            if let Some((_, bib)) = s.bib.clone() {
                let candidatos: Vec<String> = bib
                    .piezas
                    .iter()
                    .filter(|x| x.familia == n.family)
                    .map(|x| x.id.clone())
                    .collect();
                if !candidatos.is_empty() {
                    right = right.child(el().h(6.0));
                    right = right.child(section("Modelos de la biblioteca"));
                    for id in candidatos {
                        let actual = n.modelo.as_deref() == Some(id.as_str());
                        let elegido = id.clone();
                        let etiqueta = if actual {
                            format!("{id} *")
                        } else {
                            id.clone()
                        };
                        right = right.child(
                            chip(&etiqueta, actual)
                                .on_press(move |s: &mut State| s.aplicar_modelo(&elegido)),
                        );
                    }
                }
            }

            // Sin controles de edición: la app no edita el diseño (D12).
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
                                .on_press(move |s: &mut State| s.aprobar(&code_a)),
                        )
                        .child(
                            chip("Observar", l.estado == EstadoObjeto::Observado)
                                .key("btn-observar")
                                .on_press(move |s: &mut State| s.observar(&code_o)),
                        ),
                );
                right = right.child(observaciones::panel(s, &l.code, &l.observaciones));
            }
            {
                let code_p = l.code.clone();
                right = right.child(button("Perfil").key("btn-perfil").on_press(
                    move |s: &mut State| match perfil_panel::abrir(&s.project, &code_p) {
                        Ok(pf) => s.perfil = Some(pf),
                        Err(e) => s.aviso = Some(e),
                    },
                ));
            }
            right = right.child(el().h(6.0));
            right = right.child(section("Geometría"));
            right = right.child(kv(
                "DN",
                l.dn()
                    .map(|d| format!("{d} mm"))
                    .unwrap_or_else(|| redes_core::model::SIN_DATO.to_string()),
            ));
            right = right.child(kv("longitud", format!("{:.2} m", l.length_2d())));
            let cotas = l.cotas_trazado();
            right = right.child(kv("cota inicial", metros(cotas.map(|c| c.0), 3)));
            right = right.child(kv("cota final", metros(cotas.map(|c| c.1), 3)));
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
        )
        .child(
            button("Rehacer")
                .key("btn-rehacer")
                .on_press(|s: &mut State| s.rehacer()),
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
                s.aviso_gpu
                    .clone()
                    .or_else(|| s.aviso_modelos.clone())
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
        .children(s.perfil.as_ref().map(perfil_panel::panel))
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
            ("pozos", camara_pozos as fn(&mut State)),
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
                    s.sel = sample_manhole(&s.project);
                    s.enfocar();
                }) as fn(&mut State),
            ),
            (
                "pozo-modelo",
                (|s: &mut State| {
                    // The sample manhole up close, without terrain or selection tint: the piece
                    // as it was baked, with its materials, also under the ground in `--gpu3d`.
                    s.sel = sample_manhole(&s.project);
                    s.enfocar();
                    s.layers.terreno = false;
                    s.sel = None;
                    s.cam.elevation = 24.0;
                    s.rebuild();
                }) as fn(&mut State),
            ),
            (
                "corte",
                (|s: &mut State| {
                    s.sel = sample_manhole(&s.project);
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
                    s.sel = sample_manhole(&s.project);
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
            ("validar", validacion::escena as fn(&mut State)),
        ],
        ..AppSpec::new("Redes — visor 3D", (1280.0, 800.0), State::new, view)
    };
    if gpu3d::activo() {
        rune::run_with(spec, gpu3d::Compositor3d::new());
    } else {
        rune::run(spec);
    }
}

/// Manhole the focus, cut and explosion scenes show: the first one drawn with a library model
/// and connected to pipes (its anchors and channel are then in view), else the first manhole.
fn sample_manhole(p: &Project) -> Option<String> {
    let manholes = || {
        p.nodes
            .iter()
            .filter(|n| !n.deleted && n.family == "saneamiento.pozo")
    };
    manholes()
        .find(|n| n.modelo.is_some() && p.tramos_de(n.id).count() >= 2)
        .or_else(|| manholes().next())
        .map(|n| n.code.clone())
}

/// Escena `pozos`: primer plano del primer pozo (cono, marco, tapa y escalera). El objetivo sale
/// del pozo, en coordenadas locales: un objetivo fijo quedaba lejos de la red al cambiar el demo y
/// el lienzo salía vacío, con y sin `--gpu3d`.
fn camara_pozos(s: &mut State) {
    let pozo = s
        .project
        .nodes
        .iter()
        .find(|n| !n.deleted && n.family == "saneamiento.pozo")
        .map(|n| (n.code.clone(), n.pos, n.z_rim));
    if let Some((code, pos, z)) = pozo {
        s.cam.target = [
            pos.x - s.project.origin.x,
            pos.y - s.project.origin.y,
            z - 0.5,
        ];
        s.cam.distance = 16.0;
        s.cam.azimuth = -64.0;
        s.cam.elevation = 22.0;
        s.sel = Some(code);
    }
    s.fitted = true;
    s.rebuild();
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_manhole_without_inner_diameter_shows_no_diameter() {
        let mut n = redes_core::demo::project()
            .nodes
            .into_iter()
            .find(|n| n.family == "saneamiento.pozo")
            .unwrap();
        n.attrs.insert(
            "diametro_interior".into(),
            redes_core::families::Value::Num(1.5),
        );
        assert_eq!(super::inner_diameter_text(&n), "1.50 m");
        n.attrs.remove("diametro_interior");
        assert_eq!(super::inner_diameter_text(&n), redes_core::model::SIN_DATO);
        assert!(
            n.drawing_diameter_m() > 0.0,
            "the fallback piece still has a size"
        );
    }

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
        assert_eq!(
            n.nota_vigente(),
            Some(observaciones::TEXTO_POR_DEFECTO),
            "la observacion queda en el historial"
        );
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

    /// La escena `pozos` mira al pozo elegido: su tapa y su solera caen dentro del lienzo.
    #[test]
    fn la_escena_pozos_encuadra_el_pozo() {
        let mut st = State::new();
        camara_pozos(&mut st);
        let code = st.sel.clone().expect("elige un pozo");
        let n = st.project.node_by_code(&code).expect("pozo vivo");
        let (x, y) = (n.pos.x - st.project.origin.x, n.pos.y - st.project.origin.y);
        let [w, h] = st.cam.viewport;
        for z in [n.z_rim, n.z_invert] {
            let q = st.cam.project([x, y, z]).expect("delante de la cámara");
            assert!(
                (0.0..=w).contains(&q[0]) && (0.0..=h).contains(&q[1]),
                "{code} a cota {z} cae fuera del lienzo: {q:?} en {w}x{h}"
            );
        }
    }

    /// Con `--gpu3d`, arrastrar, girar con la rueda o cambiar de cámara solo mueve la matriz: la
    /// malla (`triangulos_exportables`) no se regenera. Apagar una capa sí la regenera (F14-02).
    #[test]
    fn mover_la_camara_en_3d_no_regenera_la_malla() {
        let mut st = State::new();
        st.gpu3d = true;
        st.rebuild();
        let generacion = |rt: &Runtime<State>| rt.state.geom.lock().unwrap().generation;
        let vp = |rt: &Runtime<State>| rt.state.geom.lock().unwrap().view_proj;
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
        let g0 = generacion(&rt);
        assert!(g0 > 0, "la malla inicial se generó");
        assert!(!rt.state.geom.lock().unwrap().scene.is_empty());
        assert!(
            rt.state.scene.prims.is_empty(),
            "en 3D no se arma la escena del pintor"
        );
        let (az0, vp0) = (rt.state.cam.azimuth, vp(&rt));

        let (x, y) = rt.find("plano3d").expect("lienzo montado");
        rt.pointer_down(x, y);
        for i in 1..=6 {
            rt.pointer_move(x + 12.0 * i as f32, y + 4.0 * i as f32);
            rt.frame(1.0 / 60.0);
        }
        rt.pointer_up(x + 72.0, y + 24.0);
        rt.wheel(x, y, -120.0);
        rt.frame(1.0 / 60.0);

        assert!(
            (rt.state.cam.azimuth - az0).abs() > 1.0,
            "el arrastre orbitó la cámara"
        );
        assert_ne!(vp(&rt), vp0, "la matriz sigue a la cámara");
        assert_eq!(
            generacion(&rt),
            g0,
            "mover la cámara no debe regenerar la malla"
        );

        let (x, y) = rt.find("btn-edificios").expect("chip Edificios montado");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(1.0 / 60.0);
        assert_eq!(generacion(&rt), g0 + 1, "apagar una capa regenera una vez");
    }

    /// Perfil: el botón del inspector de un tramo abre el perfil del colector y se puede cerrar.
    #[test]
    fn el_boton_perfil_abre_y_cierra_el_perfil_del_colector() {
        let mut st = State::new();
        st.sel = Some("CL-003".to_string());
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
        let (x, y) = rt.find("btn-perfil").expect("botón Perfil montado");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(1.0 / 60.0);
        let pf = rt.state.perfil.as_ref().expect("perfil abierto");
        assert_eq!(
            pf.tramos.len(),
            8,
            "troncal más emisario, de cabecera a descarga"
        );
        assert!(rt.find("panel-perfil").is_some());
        let (x, y) = rt.find("btn-perfil-cerrar").expect("botón cerrar");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(1.0 / 60.0);
        assert!(rt.state.perfil.is_none());
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

    /// La app no edita el diseño (D12): con un pozo seleccionado el inspector no monta ningún
    /// control para cambiar DN, diámetro o profundidad (se retiró «Parámetros (en vivo)»).
    #[test]
    fn el_inspector_no_edita_parametros_del_diseno() {
        let mut st = State::new();
        st.sel = Some("PZ-001".to_string());
        st.rebuild();
        let revision = st.project.revision;
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
        // El inspector sí está montado: la revisión sigue disponible.
        assert!(rt.find("btn-deshacer").is_some(), "Deshacer montado");
        for clave in [
            "btn-dn-menos",
            "btn-dn-mas",
            "btn-dia-menos",
            "btn-dia-mas",
            "btn-prof-menos",
            "btn-prof-mas",
        ] {
            assert!(
                rt.find(clave).is_none(),
                "{clave}: la app no edita parámetros del diseño (D12)"
            );
        }
        assert_eq!(rt.state.project.revision, revision);
        assert!(!rt.state.dirty);
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
