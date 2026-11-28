//! `redes biblioteca hornear` with the fitting, valve and hydrant recipes (F17-05), real binary.
//!
//! The Blender tests run only where Blender is installed and skip cleanly elsewhere (the CI has
//! no Blender); the cache test runs everywhere with a fake Blender path.

use std::path::{Path, PathBuf};
use std::process::Command;

use redes_core::bake::{self, LoadedRecipe};

const NO_BLENDER: &str = "Z:/no-blender-here/blender.exe";
const RECIPES: [&str; 6] = [
    "build_elbow",
    "build_tee",
    "build_reducer",
    "build_valve",
    "build_air_valve",
    "build_hydrant",
];
const PILOTS: [(&str, &str); 4] = [
    ("codo", "build_elbow"),
    ("tee", "build_tee"),
    ("compuerta", "build_valve"),
    ("hidrante", "build_hydrant"),
];

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
        "ok:false iff exit != 0: {v}"
    );
    (code, v)
}

fn tempdir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("redes-accesorios-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("temp dir");
    d
}

fn blender_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/blender")
}

fn recipe_path(name: &str) -> PathBuf {
    blender_dir().join(format!("{name}.receta.json"))
}

fn showcase(dir: &Path) -> PathBuf {
    let project = dir.join("vitrina.redes");
    let p = project.to_str().unwrap();
    assert_eq!(
        run(&["proyecto", "nuevo", "--nombre", "V", "--salida", p], dir).0,
        0
    );
    let plan = blender_dir().join("piloto/fittings/vitrina.plan.json");
    let (code, v) = run(
        &[
            "-p",
            p,
            "importar",
            "plan",
            "--archivo",
            plan.to_str().unwrap(),
        ],
        dir,
    );
    assert_eq!(code, 0, "{v}");
    project
}

fn hornear(project: &Path, recipe: &str, blender: &str, dir: &Path) -> (i32, serde_json::Value) {
    let recipe = recipe_path(recipe);
    run(
        &[
            "-p",
            project.to_str().unwrap(),
            "biblioteca",
            "hornear",
            "--receta",
            recipe.to_str().unwrap(),
            "--blender",
            blender,
            "--sin-blend",
        ],
        dir,
    )
}

/// The versioned pilots stand in for a bake: with them in the cache the showcase's tee, gate
/// valve and hydrant get their pieces with no Blender, and the viewer is told how to place them.
#[test]
fn cached_pilots_need_no_blender_and_carry_their_placement() {
    let dir = tempdir("cache");
    let project = showcase(&dir);
    let p = redes_core::project_io::load(&project).unwrap();
    for (kind, recipe) in [("tee", "build_tee"), ("hidrante", "build_hydrant")] {
        let loaded = LoadedRecipe::load(&recipe_path(recipe)).unwrap();
        let n = p
            .nodes
            .iter()
            .find(|n| loaded.recipe.applies_to(n))
            .unwrap();
        let key = loaded.key(&loaded.recipe.params_for_node(&p, n).unwrap());
        let pilot = blender_dir().join(format!("piloto/fittings/{key}.glb"));
        let Ok(bytes) = std::fs::read(&pilot) else {
            panic!(
                "{kind}: the showcase {} should bake to the pilot {key}",
                n.code
            );
        };
        let cached = bake::cached_glb(&dir, &key);
        std::fs::create_dir_all(cached.parent().unwrap()).unwrap();
        std::fs::write(&cached, bytes).unwrap();
        let (code, v) = hornear(&project, recipe, NO_BLENDER, &dir);
        assert_eq!(code, 0, "{v}");
        assert_eq!(v["data"]["horneado"]["de_cache"], 1, "{v}");
    }
    let manifest =
        redes_core::biblioteca::Biblioteca::cargar(&dir.join("biblioteca.json")).unwrap();
    assert_eq!(manifest.piezas.len(), 2);
    for entry in &manifest.piezas {
        assert!(
            entry.extras.contains_key(bake::PLACEMENT_EXTRA),
            "{}",
            entry.id
        );
    }
    let p = redes_core::project_io::load(&project).unwrap();
    assert!(p.node_by_code("AX-01").unwrap().modelo.is_some());
    assert!(p.node_by_code("HI-01").unwrap().modelo.is_some());
}

fn installed_blender() -> Option<PathBuf> {
    std::env::var_os("REDES_BLENDER")
        .map(PathBuf::from)
        .into_iter()
        .chain(
            std::env::var_os("ProgramFiles")
                .map(|p| PathBuf::from(p).join("Blender Foundation/Blender 5.1/blender.exe")),
        )
        .find(|p| p.is_file())
}

#[test]
fn blender_rebakes_the_versioned_fitting_pilots_byte_for_byte() {
    let Some(blender) = installed_blender() else {
        eprintln!("skipped: Blender 5.1 is not installed (set REDES_BLENDER to run it)");
        return;
    };
    let dir = tempdir("pilotos");
    for (kind, recipe) in PILOTS {
        let given = blender_dir().join(format!("piloto/fittings/{kind}.json"));
        let recipe = recipe_path(recipe);
        let (code, v) = run(
            &[
                "biblioteca",
                "hornear",
                "--receta",
                recipe.to_str().unwrap(),
                "--parametros",
                given.to_str().unwrap(),
                "--carpeta",
                dir.to_str().unwrap(),
                "--blender",
                blender.to_str().unwrap(),
                "--sin-blend",
            ],
            &dir,
        );
        assert_eq!(code, 0, "{v}");
        let key = v["data"]["juegos"][0]["clave"].as_str().unwrap();
        let fresh = std::fs::read(bake::cached_glb(&dir, key)).unwrap();
        let versioned = std::fs::read(blender_dir().join(format!("piloto/fittings/{key}.glb")))
            .unwrap_or_else(|_| panic!("{kind}: the versioned pilot is named after the key"));
        assert!(
            fresh == versioned,
            "{kind}: the pilot .glb must be today's bake"
        );
    }
}

#[test]
fn blender_bakes_every_fitting_of_the_showcase_and_the_cache_serves_them_again() {
    let Some(blender) = installed_blender() else {
        eprintln!("skipped: Blender 5.1 is not installed (set REDES_BLENDER to run it)");
        return;
    };
    let dir = tempdir("vitrina");
    let project = showcase(&dir);
    let blender = blender.to_str().unwrap();
    let mut pieces = 0;
    for recipe in RECIPES {
        let (code, v) = hornear(&project, recipe, blender, &dir);
        assert_eq!(code, 0, "{recipe}: {v}");
        assert_eq!(
            v["data"]["objetos"], v["data"]["con_pieza"],
            "{recipe}: {v}"
        );
        pieces += v["data"]["piezas_distintas"].as_u64().unwrap();
        let (_, again) = hornear(&project, recipe, NO_BLENDER, &dir);
        assert_eq!(
            again["data"]["horneado"]["horneadas"], 0,
            "{recipe}: {again}"
        );
    }
    assert_eq!(
        pieces, 9,
        "two elbows of other angles, and one of everything else"
    );
    let manifest =
        redes_core::biblioteca::Biblioteca::cargar(&dir.join("biblioteca.json")).unwrap();
    for entry in &manifest.piezas {
        let glb = std::fs::read(dir.join(&entry.archivo)).unwrap();
        let m = redes_core::biblioteca::cantidades_glb(&glb).unwrap();
        assert!(m.cerrada(), "{}: a closed solid", entry.id);
    }
}
