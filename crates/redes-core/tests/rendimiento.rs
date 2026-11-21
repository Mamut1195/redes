//! Presupuesto de frame: la reconstrucción de la escena no puede degradarse sin que se note.
//!
//! En depuración el pintor es ~10 veces más lento que en release, así que el umbral es holgado a
//! propósito: sirve de trinquete contra regresiones grandes, no de medida de rendimiento fina.

use std::time::Instant;

use redes_core::model::Project;
use redes_core::view::{Camera, Layers, Opciones, build_opts};

fn p95(mut muestras: Vec<f64>) -> f64 {
    muestras.sort_by(f64::total_cmp);
    let i = ((muestras.len() as f64 * 0.95).ceil() as usize).saturating_sub(1);
    muestras[i.min(muestras.len() - 1)]
}

fn mide(p: &Project, cam: &Camera, opts: Opciones, repeticiones: usize) -> (f64, f64) {
    let mut tiempos = Vec::new();
    for _ in 0..repeticiones {
        let t0 = Instant::now();
        let escena = build_opts(p, p.terrain.as_ref(), cam, Layers::default(), None, &opts);
        std::hint::black_box(&escena);
        tiempos.push(t0.elapsed().as_secs_f64() * 1000.0);
    }
    let p50 = {
        let mut t = tiempos.clone();
        t.sort_by(f64::total_cmp);
        t[t.len() / 2]
    };
    (p50, p95(tiempos))
}

#[test]
fn la_ciudad_y_el_corte_caben_en_el_presupuesto() {
    let p = redes_core::demo_city::ciudad();
    let mut cam = Camera::default();
    cam.viewport = [900.0, 600.0];
    cam.fit(&p);

    let (p50_c, p95_c) = mide(&p, &cam, Opciones::default(), 15);
    let (p50_x, p95_x) = mide(
        &p,
        &cam,
        Opciones {
            corte_x: Some(320.0),
            ..Opciones::default()
        },
        15,
    );

    println!("ciudad: p50 {p50_c:.1} ms, p95 {p95_c:.1} ms");
    println!("corte:  p50 {p50_x:.1} ms, p95 {p95_x:.1} ms");

    assert!(p95_c < 800.0, "la ciudad tardó {p95_c:.1} ms (p95)");
    assert!(p95_x < 800.0, "el corte tardó {p95_x:.1} ms (p95)");
    // El corte no puede costar más de tres veces la escena completa: si pasa, algo se ha roto.
    assert!(
        p95_x < p95_c * 3.0 + 50.0,
        "el corte cuesta {p95_x:.1} ms frente a {p95_c:.1} ms de la ciudad"
    );
}

#[test]
fn el_catalogo_completo_se_construye_rapido() {
    let p = redes_core::demo_catalog::catalogo_completo();
    let mut cam = Camera::default();
    cam.viewport = [900.0, 600.0];
    cam.fit(&p);
    let (p50, p95) = mide(&p, &cam, Opciones::default(), 10);
    println!("catálogo (46 familias): p50 {p50:.1} ms, p95 {p95:.1} ms");
    assert!(p95 < 800.0, "el catálogo tardó {p95:.1} ms (p95)");
}
