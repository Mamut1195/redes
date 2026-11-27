//! Construcción de la escena del pintor (F14-04) y recorte por pantalla y selección (F14-07).
//!
//! El banco (`banco_escena`, ignorado) mide solo `build_opts`, sin ingesta, con cuatro cámaras:
//! la red sintética encuadrada entera, la misma red de cerca (tubos y pozos con malla, casi todo
//! fuera de pantalla) y la ciudad de demostración encuadrada y de cerca. Se corre en release:
//! `cargo test --release -p redes-core --test escena -- --ignored --nocapture`.
//!
//! Da el mínimo de varias pasadas en milisegundos y, en Windows, en millones de ciclos del hilo
//! (`QueryThreadCycleTime`): con otras compilaciones en la máquina el reloj de pared varía el
//! doble de una pasada a otra; los ciclos del hilo apenas. Con 20k tramos la escena se reparte entre
//! hilos y los ciclos son solo los del hilo principal: ahí vale el reloj de pared.
//!
//! Además, sin ignorar: la red de 20k tramos de cerca descarta casi todo antes de ordenar, y la
//! rejilla de selección elige lo mismo que el recorrido lineal mirando una fracción de la red.

mod comun;

use std::path::{Path, PathBuf};
use std::time::Instant;

use redes_core::model::Project;
use redes_core::seleccion::{RejillaSeleccion, elegir_lineal};
use redes_core::view::{Camera, Layers, Opciones, Scene, build_opts};

fn red_sintetica(tramos: usize) -> Project {
    comun::proyecto(tramos)
}

fn ciudad() -> Project {
    let ruta: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("demo")
        .join("ciudad-san-bartolo.redes");
    redes_core::project_io::load(&ruta).expect("abre la ciudad de demostración")
}

fn encuadre(p: &Project) -> Camera {
    let mut cam = Camera {
        viewport: [900.0, 600.0],
        z_scale: 2.0,
        ..Camera::default()
    };
    cam.fit(p);
    cam
}

/// Cámara cerca del primer nodo: las piezas se ven con malla y casi toda la red queda fuera.
fn de_cerca(p: &Project, distancia: f64) -> Camera {
    let mut cam = encuadre(p);
    let n = p.nodes.iter().find(|n| !n.deleted).expect("hay nodos");
    cam.target = [n.pos.x - p.origin.x, n.pos.y - p.origin.y, n.z_rim];
    cam.distance = distancia;
    cam.azimuth = 35.0;
    cam
}

fn escena(p: &Project, cam: &Camera) -> Scene {
    build_opts(
        p,
        p.terrain.as_ref(),
        cam,
        Layers::default(),
        None,
        &Opciones::default(),
    )
}

#[cfg(windows)]
fn ciclos() -> u64 {
    unsafe extern "system" {
        fn GetCurrentThread() -> isize;
        fn QueryThreadCycleTime(hilo: isize, ciclos: *mut u64) -> i32;
    }
    let mut c = 0u64;
    // SAFETY: pseudo-identificador del hilo actual y un puntero válido a un u64.
    unsafe { QueryThreadCycleTime(GetCurrentThread(), &mut c) };
    c
}

#[cfg(not(windows))]
fn ciclos() -> u64 {
    0
}

/// Mínimo de `veces` construcciones: (ms, millones de ciclos del hilo, primitivas).
fn minimo(p: &Project, cam: &Camera, veces: usize) -> (f64, f64, usize) {
    let (mut ms, mut mc, mut prims) = (f64::INFINITY, f64::INFINITY, 0);
    for _ in 0..veces {
        let (t0, c0) = (Instant::now(), ciclos());
        let e = escena(p, cam);
        let c1 = ciclos();
        ms = ms.min(t0.elapsed().as_secs_f64() * 1000.0);
        mc = mc.min((c1 - c0) as f64 / 1e6);
        prims = e.prims.len();
        std::hint::black_box(&e);
    }
    (ms, mc, prims)
}

/// Cifras de la construcción de escena. Ignorada por defecto: se corre en release.
#[test]
#[ignore]
fn banco_escena() {
    let red = red_sintetica(20_000);
    let ciudad = ciudad();
    for (nombre, p, cam) in [
        ("red 20k encuadrada", &red, encuadre(&red)),
        ("red 20k de cerca", &red, de_cerca(&red, 60.0)),
        ("ciudad encuadrada", &ciudad, encuadre(&ciudad)),
        ("ciudad de cerca", &ciudad, de_cerca(&ciudad, 40.0)),
    ] {
        let (ms, mc, prims) = minimo(p, &cam, 15);
        println!("{nombre}: {ms:.2} ms, {mc:.1} Mciclos ({prims} primitivas)");
    }
}

/// De cerca, casi toda la red de 20k tramos cae fuera de pantalla: no se tesela ni se ordena.
#[test]
fn de_cerca_lo_que_no_se_ve_no_se_ordena() {
    let red = red_sintetica(20_000);
    let lejos = escena(&red, &encuadre(&red));
    let cerca = escena(&red, &de_cerca(&red, 60.0));
    assert_eq!(lejos.descartadas, 0, "encuadrada, todo se ve");
    assert!(
        cerca.descartadas > 15_000,
        "de cerca solo se descartan {} objetos",
        cerca.descartadas
    );
    assert!(cerca.prims.len() < lejos.prims.len() / 10);
}

/// La rejilla de selección (F14-07) con 20k tramos encuadrados: elige lo mismo que el recorrido
/// lineal y un clic mira una fracción pequeña de la red.
#[test]
fn la_rejilla_elige_como_el_recorrido_lineal_mirando_poco() {
    let red = red_sintetica(20_000);
    let cam = encuadre(&red);
    let rejilla = RejillaSeleccion::nueva(&red, &cam);
    let total = rejilla.len();
    assert!(total > 40_000, "{total} objetos");
    let (mut peor, mut aciertos) = (0, 0);
    for y in (3..600).step_by(41) {
        for x in (5..900).step_by(53) {
            let at = [x as f32, y as f32];
            peor = peor.max(rejilla.examinados(at));
            let lineal = elegir_lineal(&red, &cam, at);
            aciertos += lineal.is_some() as usize;
            assert_eq!(
                rejilla.elegir(at).map(str::to_string),
                lineal,
                "clic en {at:?}"
            );
        }
    }
    assert!(aciertos > 20, "pocos clics sobre la red: {aciertos}");
    // Encuadrada, 40k objetos caben en 900×600 y las filas lejanas se apiñan: una celda junta
    // miles. De cerca, cada clic mira solo lo que está a su alcance.
    assert!(
        peor * 10 < total,
        "un clic mira {peor} de {total} objetos: la rejilla no reparte"
    );
    let cerca = de_cerca(&red, 60.0);
    let rejilla = RejillaSeleccion::nueva(&red, &cerca);
    let mut peor = 0;
    for y in (3..600).step_by(41) {
        for x in (5..900).step_by(53) {
            let at = [x as f32, y as f32];
            peor = peor.max(rejilla.examinados(at));
            assert_eq!(
                rejilla.elegir(at).map(str::to_string),
                elegir_lineal(&red, &cerca, at),
                "de cerca, clic en {at:?}"
            );
        }
    }
    assert!(peor < 50, "de cerca un clic mira {peor} objetos");
}
