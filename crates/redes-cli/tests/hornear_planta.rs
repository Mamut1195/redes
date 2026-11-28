//! The plant recipes rebake their versioned pilots byte for byte (F17-07).
//!
//! Runs only where Blender 5.1 is installed and skips cleanly elsewhere (the CI has no Blender);
//! `redes-core/tests/planta_horneada.rs` checks the same pilots without it.

use std::path::{Path, PathBuf};
use std::process::Command;

use redes_core::bake;

const RECIPES: &[&str] = &[
    "build_plant_mixer",
    "build_plant_flocculator",
    "build_plant_sedimentator",
    "build_plant_filter",
    "build_plant_contact_tank",
    "build_plant_drying_bed",
    "build_plant_screen",
    "build_plant_grit_chamber",
    "build_plant_uasb",
    "build_plant_clarifier",
];

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

fn tools() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/blender")
}

#[test]
fn blender_rebakes_every_plant_pilot_byte_for_byte() {
    let Some(blender) = installed_blender() else {
        eprintln!("skipped: Blender 5.1 is not installed (set REDES_BLENDER to run it)");
        return;
    };
    let dir = std::env::temp_dir().join(format!("redes-hornear-planta-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for name in RECIPES {
        let recipe = tools().join(format!("{name}.receta.json"));
        let given = tools().join(format!("piloto/plant/{name}.json"));
        let out = Command::new(env!("CARGO_BIN_EXE_redes"))
            .args(["biblioteca", "hornear", "--receta"])
            .arg(&recipe)
            .arg("--parametros")
            .arg(&given)
            .arg("--blender")
            .arg(&blender)
            .args(["--sin-blend", "--carpeta"])
            .arg(&dir)
            .env_remove("REDES_BLENDER")
            .output()
            .expect("runs the binary");
        let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("an envelope");
        assert!(out.status.success(), "{name}: {v}");
        let key = v["data"]["juegos"][0]["clave"].as_str().unwrap();
        let fresh = std::fs::read(bake::cached_glb(&dir, key)).unwrap();
        let versioned = std::fs::read(tools().join(format!("piloto/plant/{key}.glb")))
            .unwrap_or_else(|_| panic!("{name}: the versioned pilot is not named {key}"));
        assert!(
            fresh == versioned,
            "{name}: the pilot .glb must be today's bake"
        );
    }
}
