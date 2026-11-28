//! The intake recipes (F17-06) with the real binary and Blender.
//!
//! Both tests run only where Blender 5.1 is installed and skip cleanly elsewhere (the CI has no
//! Blender); `redes-core/tests/captacion_horneada.rs` checks the versioned pilots without it.

use std::path::{Path, PathBuf};
use std::process::Command;

use redes_core::bake;

const RECIPES: [&str; 4] = ["weir", "desander", "load_chamber", "break_chamber"];

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn recipe_path(name: &str) -> PathBuf {
    repo().join(format!("tools/blender/build_intake_{name}.receta.json"))
}

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
    (out.status.code().unwrap_or(-1), v)
}

fn tempdir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("redes-captacion-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("temp dir");
    d
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
fn blender_rebakes_every_intake_pilot_byte_for_byte() {
    let Some(blender) = installed_blender() else {
        eprintln!("skipped: Blender 5.1 is not installed (set REDES_BLENDER to run it)");
        return;
    };
    let pilots = repo().join("tools/blender/piloto/intake");
    for name in RECIPES {
        let dir = tempdir(name);
        let recipe = recipe_path(name);
        let given = pilots.join(format!("{name}.json"));
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
        assert_eq!(code, 0, "{name}: {v}");
        let key = v["data"]["juegos"][0]["clave"].as_str().unwrap();
        let fresh = std::fs::read(bake::cached_glb(&dir, key)).unwrap();
        let versioned = std::fs::read(pilots.join(format!("{key}.glb")))
            .unwrap_or_else(|_| panic!("{name}: the versioned pilot is named after {key}"));
        assert!(
            fresh == versioned,
            "{name}: the pilot .glb must be today's bake"
        );
    }
}

#[test]
fn blender_bakes_every_intake_structure_of_the_chain_demo() {
    let Some(blender) = installed_blender() else {
        eprintln!("skipped: Blender 5.1 is not installed (set REDES_BLENDER to run it)");
        return;
    };
    let dir = tempdir("cadena");
    let (code, v) = run(&["proyecto", "cadena", "--salida", "cadena.redes"], &dir);
    assert_eq!(code, 0, "{v}");
    for name in RECIPES {
        let recipe = recipe_path(name);
        let (code, v) = run(
            &[
                "-p",
                "cadena.redes",
                "biblioteca",
                "hornear",
                "--receta",
                recipe.to_str().unwrap(),
                "--blender",
                blender.to_str().unwrap(),
            ],
            &dir,
        );
        assert_eq!(code, 0, "{name}: {v}");
        let data = &v["data"];
        assert!(data["objetos"].as_u64().unwrap() >= 1, "{name}: {data}");
        assert_eq!(data["con_pieza"], data["objetos"], "{name}: {data}");
        assert_eq!(data["sin_ancla"], serde_json::json!([]), "{name}: {data}");
        assert_eq!(
            data["horneado"]["horneadas"], data["piezas_distintas"],
            "{name}: {data}"
        );
    }
    let cache = dir.join(bake::CACHE_DIR);
    let glbs: Vec<PathBuf> = std::fs::read_dir(&cache)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "glb"))
        .collect();
    assert_eq!(glbs.len(), RECIPES.len(), "one piece per demo structure");
    for glb in glbs {
        let blend = std::fs::read(glb.with_extension("blend")).expect("review .blend saved");
        assert!(blend.starts_with(b"BLENDER"), "an uncompressed .blend");
    }
}
