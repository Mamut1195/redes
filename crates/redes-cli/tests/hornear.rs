//! Contract of `redes biblioteca hornear` with the real binary (F17-02, D16).
//!
//! The Blender test runs only where Blender is installed and skips cleanly elsewhere (the CI has
//! no Blender); the rest run everywhere with a fake Blender path.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use redes_core::bake::{self, LoadedRecipe};
use redes_core::gltf::Vertice;

const NO_BLENDER: &str = "Z:/no-blender-here/blender.exe";

fn run(args: &[&str], dir: &Path) -> (i32, serde_json::Value) {
    let out = Command::new(env!("CARGO_BIN_EXE_redes"))
        .args(args)
        .current_dir(dir)
        .env_remove("REDES_BLENDER")
        .output()
        .expect("runs the binary");
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let v: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("not an envelope ({e}): {stdout:?}"));
    let code = out.status.code().unwrap_or(-1);
    assert_eq!(
        v["ok"].as_bool() == Some(false),
        code != 0,
        "ok:false ⇔ exit ≠ 0: {v}"
    );
    (code, v)
}

fn tempdir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("redes-hornear-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("temp dir");
    d
}

fn recipe_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/blender/build_manhole.receta.json")
}

fn manhole(code: &str, e: f64, attrs: &str) -> String {
    format!(
        r#"{{ "tipo": "nodo", "codigo": "{code}", "familia": "saneamiento.pozo", "subtipo": "inspeccion",
  "e": {e}, "n": 9797000.0, "cota": 2490.0, "cota_tapa": 2492.5, "cota_terreno": 2492.5,
  "diametro": 1.2, "attrs": {{ {attrs} }} }}"#
    )
}

/// Two complete manholes and one without wall thickness.
fn project_with_manholes(dir: &Path) -> PathBuf {
    let complete = r#""espesor_pared": 0.15, "material": "hormigon""#;
    let objects = [
        manhole("PZ-1", 758000.0, complete),
        manhole("PZ-2", 758030.0, complete),
        manhole("PZ-3", 758060.0, r#""material": "hormigon""#),
    ];
    let plan = format!(
        r#"{{ "schema": "redes.plan/1", "proyecto": {{ "nombre": "H", "crs": "EPSG:32717" }},
  "objetos": [{}] }}"#,
        objects.join(",")
    );
    std::fs::write(dir.join("plan.json"), plan).unwrap();
    let project = dir.join("h.redes");
    let p = project.to_str().unwrap();
    assert_eq!(
        run(&["proyecto", "nuevo", "--nombre", "H", "--salida", p], dir).0,
        0
    );
    let (code, v) = run(
        &["-p", p, "importar", "plan", "--archivo", "plan.json"],
        dir,
    );
    assert_eq!(code, 0, "{v}");
    project
}

fn cube_glb() -> Vec<u8> {
    let v = [
        [-1.0f32, 0.0, -1.0],
        [1.0, 0.0, -1.0],
        [1.0, 0.0, 1.0],
        [-1.0, 0.0, 1.0],
        [-1.0, 2.0, -1.0],
        [1.0, 2.0, -1.0],
        [1.0, 2.0, 1.0],
        [-1.0, 2.0, 1.0],
    ];
    let faces = [
        [0, 1, 2],
        [0, 2, 3],
        [4, 6, 5],
        [4, 7, 6],
        [0, 4, 5],
        [0, 5, 1],
        [1, 5, 6],
        [1, 6, 2],
        [2, 6, 7],
        [2, 7, 3],
        [3, 7, 4],
        [3, 4, 0],
    ];
    let verts = faces
        .iter()
        .flatten()
        .map(|i| Vertice {
            pos: v[*i],
            normal: [0.0, 0.0, 1.0],
            color: [0.6, 0.6, 0.6],
        })
        .collect();
    redes_core::gltf::glb_desde_vertices(&BTreeMap::from([("modelo".to_string(), verts)]))
        .expect("fits in a .glb")
}

/// Keys the project's complete manholes should bake to.
fn expected_keys(project: &Path) -> Vec<String> {
    let loaded = LoadedRecipe::load(&recipe_path()).unwrap();
    let p = redes_core::project_io::load(project).unwrap();
    p.nodes
        .iter()
        .filter(|n| loaded.recipe.applies_to(n))
        .filter_map(|n| loaded.recipe.params_for_node(&p, n).ok())
        .map(|set| loaded.key(&set))
        .collect()
}

#[test]
fn without_blender_a_miss_fails_and_leaves_the_project_untouched() {
    let dir = tempdir("sin-blender");
    let project = project_with_manholes(&dir);
    let before = std::fs::read(&project).unwrap();
    let recipe = recipe_path();
    let (code, v) = run(
        &[
            "-p",
            project.to_str().unwrap(),
            "biblioteca",
            "hornear",
            "--receta",
            recipe.to_str().unwrap(),
            "--blender",
            NO_BLENDER,
        ],
        &dir,
    );
    assert_ne!(code, 0);
    assert_eq!(v["error"]["code"], "E_BLENDER_NO_ENCONTRADO", "{v}");
    assert_eq!(std::fs::read(&project).unwrap(), before);
    assert!(!dir.join("biblioteca.json").exists());
}

#[test]
fn cached_pieces_need_no_blender_and_missing_data_keeps_the_fallback() {
    let dir = tempdir("cache");
    let project = project_with_manholes(&dir);
    let keys = expected_keys(&project);
    assert_eq!(keys.len(), 2);
    assert_eq!(keys[0], keys[1], "two equal manholes share one piece");
    let cached = bake::cached_glb(&dir, &keys[0]);
    std::fs::create_dir_all(cached.parent().unwrap()).unwrap();
    std::fs::write(&cached, cube_glb()).unwrap();

    let recipe = recipe_path();
    let (code, v) = run(
        &[
            "-p",
            project.to_str().unwrap(),
            "biblioteca",
            "hornear",
            "--receta",
            recipe.to_str().unwrap(),
            "--blender",
            NO_BLENDER,
            "--sin-blend",
        ],
        &dir,
    );
    assert_eq!(code, 0, "{v}");
    let d = &v["data"];
    assert_eq!(d["objetos"], 3);
    assert_eq!(d["con_pieza"], 2);
    assert_eq!(d["piezas_distintas"], 1);
    assert_eq!(d["horneado"]["de_cache"], 1);
    assert_eq!(d["horneado"]["horneadas"], 0);
    assert_eq!(d["sin_hornear"][0]["codigo"], "PZ-3");
    assert_eq!(d["sin_hornear"][0]["faltan"][0], "espesor_pared");
    assert_eq!(v["warnings"][0]["code"], "W_PARAMETRO_FALTANTE", "{v}");
    // Lone manholes bake without anchors, and the envelope says so (F17-03).
    assert_eq!(d["sin_ancla"].as_array().unwrap().len(), 2, "{v}");
    assert_eq!(d["sin_ancla"][0]["motivo"], "sin tramos conectados");
    assert_eq!(v["warnings"][1]["code"], "W_SIN_ANCLA", "{v}");

    let p = redes_core::project_io::load(&project).unwrap();
    let piece = format!("saneamiento.pozo.inspeccion-{}", &keys[0][..12]);
    assert_eq!(
        p.node_by_code("PZ-1").unwrap().modelo.as_deref(),
        Some(piece.as_str())
    );
    assert_eq!(
        p.node_by_code("PZ-3").unwrap().modelo,
        None,
        "no shape for missing data"
    );
    assert_eq!(p.biblioteca.as_deref(), Some("biblioteca.json"));

    let manifest =
        redes_core::biblioteca::Biblioteca::cargar(&dir.join("biblioteca.json")).unwrap();
    assert_eq!(manifest.piezas.len(), 1);
    let entry = &manifest.piezas[0];
    assert_eq!(entry.id, piece);
    assert_eq!(entry.extras["horneado.clave"], keys[0]);
    assert_eq!(entry.extras["horneado.wall_thickness_m"], "0.15");
    assert_eq!(entry.archivo, bake::cached_glb_relative(&keys[0]));
}

#[test]
fn given_sets_without_every_datum_are_refused() {
    let dir = tempdir("faltan");
    std::fs::write(dir.join("juegos.json"), r#"[{ "inner_diameter_m": 1.2 }]"#).unwrap();
    let recipe = recipe_path();
    let (code, v) = run(
        &[
            "biblioteca",
            "hornear",
            "--receta",
            recipe.to_str().unwrap(),
            "--parametros",
            "juegos.json",
            "--blender",
            NO_BLENDER,
        ],
        &dir,
    );
    assert_ne!(code, 0);
    assert_eq!(v["error"]["code"], "E_PARAMETRO_FALTANTE", "{v}");
    let msg = v["error"]["message"].as_str().unwrap();
    assert!(
        msg.contains("depth_m") && msg.contains("wall_thickness_m"),
        "{msg}"
    );
}

fn installed_blender() -> Option<PathBuf> {
    let candidates = std::env::var_os("REDES_BLENDER")
        .map(PathBuf::from)
        .into_iter()
        .chain(
            std::env::var_os("ProgramFiles")
                .map(|p| PathBuf::from(p).join("Blender Foundation/Blender 5.1/blender.exe")),
        );
    candidates.into_iter().find(|p| p.is_file())
}

#[test]
fn blender_bakes_a_batch_once_and_the_cache_serves_it_again() {
    let Some(blender) = installed_blender() else {
        eprintln!("skipped: Blender 5.1 is not installed (set REDES_BLENDER to run it)");
        return;
    };
    let dir = tempdir("blender");
    let sets = r#"[
      { "inner_diameter_m": 1.2, "depth_m": 2.5, "wall_thickness_m": 0.15, "body_material": "hormigon", "connections": [] },
      { "inner_diameter_m": 1.5, "depth_m": 4.0, "wall_thickness_m": 0.2, "body_material": "hormigon", "connections": [] }
    ]"#;
    std::fs::write(dir.join("juegos.json"), sets).unwrap();
    let recipe = recipe_path();
    let args = [
        "biblioteca",
        "hornear",
        "--receta",
        recipe.to_str().unwrap(),
        "--parametros",
        "juegos.json",
        "--blender",
        blender.to_str().unwrap(),
    ];
    let (code, first) = run(&args, &dir);
    assert_eq!(code, 0, "{first}");
    assert_eq!(first["data"]["horneado"]["horneadas"], 2);
    let paths: Vec<PathBuf> = first["data"]["juegos"]
        .as_array()
        .unwrap()
        .iter()
        .map(|j| PathBuf::from(j["archivo"].as_str().unwrap()))
        .collect();
    let bytes: Vec<Vec<u8>> = paths.iter().map(|p| std::fs::read(p).unwrap()).collect();
    for j in first["data"]["juegos"].as_array().unwrap() {
        let blend = std::fs::read(j["blend"].as_str().unwrap()).expect("review .blend saved");
        assert!(blend.starts_with(b"BLENDER"), "an uncompressed .blend");
    }
    for b in &bytes {
        let m = redes_core::biblioteca::cantidades_glb(b).unwrap();
        assert!(m.cerrada(), "a baked manhole is a closed solid");
    }

    let (code, second) = run(&args, &dir);
    assert_eq!(code, 0, "{second}");
    assert_eq!(second["data"]["horneado"]["de_cache"], 2);
    assert_eq!(second["data"]["horneado"]["horneadas"], 0);
    assert!(
        second["data"]["horneado"]["blender"].is_null(),
        "no Blender on a hit"
    );

    // A fresh bake of the same sets gives the same bytes (D15 determinism, now in a batch).
    for p in &paths {
        std::fs::remove_file(p).unwrap();
    }
    assert_eq!(run(&args, &dir).0, 0);
    let again: Vec<Vec<u8>> = paths.iter().map(|p| std::fs::read(p).unwrap()).collect();
    assert_eq!(again, bytes);
}

#[test]
fn blender_rebakes_the_versioned_pilot_byte_for_byte() {
    let Some(blender) = installed_blender() else {
        eprintln!("skipped: Blender 5.1 is not installed (set REDES_BLENDER to run it)");
        return;
    };
    let dir = tempdir("piloto");
    let pilot = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/blender/piloto");
    let recipe = recipe_path();
    let given = pilot.join("piloto.json");
    let (code, v) = run(
        &[
            "biblioteca",
            "hornear",
            "--receta",
            recipe.to_str().unwrap(),
            "--parametros",
            given.to_str().unwrap(),
            "--blender",
            blender.to_str().unwrap(),
            "--sin-blend",
        ],
        &dir,
    );
    assert_eq!(code, 0, "{v}");
    let key = v["data"]["juegos"][0]["clave"].as_str().unwrap();
    let fresh = std::fs::read(bake::cached_glb(&dir, key)).unwrap();
    let versioned = std::fs::read(pilot.join(format!("{key}.glb")))
        .expect("the versioned pilot is named after the current key");
    assert!(fresh == versioned, "the pilot .glb must be today's bake");
}
