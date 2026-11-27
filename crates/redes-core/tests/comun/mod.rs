//! Red sintética compartida por el banco de escala (`tests/banco.rs`, F14-10) y las pruebas de
//! índices (`tests/indices.rs`, F14-01). Cada binario usa una parte: de ahí el `dead_code`.
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Instant;

use redes_core::model::Project;
use redes_core::view::{Camera, Layers, Opciones, build_opts};

/// Plan `redes.plan/1` de una red en peine: filas de 200 pozos unidas por colectores, con soleras
/// declaradas. `tramos` es aproximado (se redondea a filas completas). No hay azar: el mismo
/// `tramos` da siempre los mismos bytes, así que el banco es reproducible entre máquinas.
pub fn plan_sintetico(tramos: usize) -> String {
    let por_fila = 200usize;
    let filas = tramos.div_ceil(por_fila - 1).max(1);
    let mut objetos = Vec::with_capacity(filas * por_fila * 2);
    for f in 0..filas {
        for c in 0..por_fila {
            let cota = 2500.0 - c as f64 * 0.05;
            objetos.push(format!(
                r#"{{"tipo":"nodo","codigo":"PZ-{f}-{c}","familia":"saneamiento.pozo","subtipo":"inspeccion","e":{},"n":{},"cota":{cota},"cota_terreno":{}}}"#,
                758000.0 + c as f64 * 30.0,
                9797000.0 + f as f64 * 40.0,
                cota + 2.0
            ));
        }
        for c in 0..por_fila - 1 {
            objetos.push(format!(
                r#"{{"tipo":"arco","codigo":"CL-{f}-{c}","familia":"saneamiento.tuberia","subtipo":"colector_sanitario","desde":"PZ-{f}-{c}","hasta":"PZ-{f}-{}","diametro_mm":200,"cota_solera_inicio":{},"cota_solera_fin":{}}}"#,
                c + 1,
                2500.0 - c as f64 * 0.05,
                2500.0 - (c + 1) as f64 * 0.05
            ));
        }
    }
    format!(
        r#"{{"schema":"redes.plan/1","proyecto":{{"nombre":"Sintetica","crs":"EPSG:32717"}},"objetos":[{}]}}"#,
        objetos.join(",")
    )
}

pub fn temporal(nombre: &str) -> PathBuf {
    std::env::temp_dir().join(format!("redes-sintetica-{nombre}-{}", std::process::id()))
}

fn ms(t0: Instant) -> f64 {
    t0.elapsed().as_secs_f64() * 1000.0
}

/// Proyecto con la red sintética de `tramos` tramos ya importada.
pub fn proyecto(tramos: usize) -> Project {
    let ruta = temporal(&format!("plan-p-{tramos}.json"));
    std::fs::write(&ruta, plan_sintetico(tramos)).unwrap();
    let mut p = Project::new("Sintetica", "EPSG:32717");
    redes_core::ingest::import_plan(&mut p, &ruta).expect("importa");
    let _ = std::fs::remove_file(&ruta);
    p
}

/// Mediana de `veces` construcciones de la escena completa (`build_opts`, vista ajustada a la
/// red, 900×600), en ms.
pub fn escena_ms(p: &Project, veces: usize) -> f64 {
    let mut cam = Camera {
        viewport: [900.0, 600.0],
        ..Camera::default()
    };
    cam.fit(p);
    let mut t: Vec<f64> = (0..veces.max(1))
        .map(|_| {
            let t0 = Instant::now();
            let e = build_opts(
                p,
                p.terrain.as_ref(),
                &cam,
                Layers::default(),
                None,
                &Opciones::default(),
            );
            std::hint::black_box(&e);
            ms(t0)
        })
        .collect();
    t.sort_by(f64::total_cmp);
    t[t.len() / 2]
}

/// GPU scene of the 3D pass (F14-05): instanced prototypes, LODs and static batches.
pub fn gpu_scene(p: &Project) -> redes_core::scene3d::Scene {
    redes_core::scene3d::Scene::build(
        p,
        Layers::default(),
        &[],
        &mut redes_core::mallas::CachePiezas::default(),
        redes_core::scene3d::GPU_BUDGET,
    )
}

/// Size of the GPU scene measured by the bench.
#[derive(Debug)]
pub struct GpuMesh {
    pub bytes_per_object: f64,
    pub megabytes: f64,
    pub warning: Option<String>,
}

/// Importa, dibuja, arma la malla 3D, busca, guarda, abre y reimporta una red de `tramos`
/// tramos; devuelve los tiempos en ms (`ingesta`, `escena` —mediana de 9 `build_opts`—,
/// `malla3d`, `busquedas`, `abrir`, `reimportar`) y el tamaño de la malla 3D.
pub fn banco(tramos: usize) -> (BTreeMap<&'static str, f64>, GpuMesh) {
    let ruta = temporal(&format!("plan-{tramos}.json"));
    std::fs::write(&ruta, plan_sintetico(tramos)).unwrap();
    let mut t = BTreeMap::new();

    let mut p = Project::new("Sintetica", "EPSG:32717");
    let t0 = Instant::now();
    let rep = redes_core::ingest::import_plan(&mut p, &ruta).expect("importa");
    t.insert("ingesta", ms(t0));
    assert!(rep.descartados.is_empty(), "{:?}", rep.descartados);
    let vivos = p.links.iter().filter(|l| !l.deleted).count();
    assert!(vivos >= tramos, "{vivos} tramos");

    t.insert("escena", escena_ms(&p, 9));

    let t0 = Instant::now();
    let scene = gpu_scene(&p);
    t.insert("malla3d", ms(t0));
    let gpu = GpuMesh {
        bytes_per_object: scene.gpu_bytes() as f64 / scene.objects.len().max(1) as f64,
        megabytes: scene.gpu_bytes() as f64 / (1024.0 * 1024.0),
        warning: scene.warning.clone(),
    };
    drop(scene);

    let t0 = Instant::now();
    for l in p.links.iter().filter(|l| !l.deleted) {
        std::hint::black_box(p.node_by_code(&l.code));
        std::hint::black_box(p.link_by_code(&l.code));
        std::hint::black_box(p.node(l.from));
    }
    t.insert("busquedas", ms(t0));

    let archivo = temporal(&format!("{tramos}.redes"));
    redes_core::project_io::save(&p, &archivo).expect("guarda");
    let t0 = Instant::now();
    let abierto = redes_core::project_io::load(&archivo).expect("abre");
    t.insert("abrir", ms(t0));
    assert_eq!(abierto.links.len(), p.links.len());

    let t0 = Instant::now();
    redes_core::reimportar::reimportar(
        &mut p,
        &ruta,
        redes_core::reimportar::Formato::Plan,
        &redes_core::diff::Opciones::default(),
        false,
    )
    .expect("reimporta");
    t.insert("reimportar", ms(t0));

    let _ = std::fs::remove_file(&ruta);
    let _ = std::fs::remove_file(&archivo);
    (t, gpu)
}
