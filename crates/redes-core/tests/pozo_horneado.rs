//! Contract of the pilot baked manhole (F17-03), without Blender.
//!
//! `tools/blender/piloto/` versions one bake of the manhole recipe: its data parameters
//! (`piloto.json`) and the `.glb` named after the key the current recipe gives them. If the recipe
//! or its driver changes, the name no longer matches and this test asks for a rebake (docs/17
//! section F17-03); the rest checks the structure the viewer relies on.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use redes_core::bake::{LoadedRecipe, ParamSet};
use redes_core::biblioteca::{self, ANCLA_SOLERA};

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn pilot() -> (ParamSet, Vec<u8>) {
    let loaded = LoadedRecipe::load(&repo().join("tools/blender/build_manhole.receta.json"))
        .expect("the manhole recipe loads");
    let dir = repo().join("tools/blender/piloto");
    let given: ParamSet =
        serde_json::from_str(&std::fs::read_to_string(dir.join("piloto.json")).unwrap()).unwrap();
    let set = loaded
        .recipe
        .params_from_given(&given)
        .expect("the pilot gives every datum");
    let key = loaded.key(&set);
    let glb = std::fs::read(dir.join(format!("{key}.glb"))).unwrap_or_else(|_| {
        panic!(
            "no tools/blender/piloto/{key}.glb: the recipe changed, rebake the pilot with\n\
             redes biblioteca hornear --receta tools/blender/build_manhole.receta.json \
             --parametros tools/blender/piloto/piloto.json --carpeta <tmp> --sin-blend\n\
             and copy <tmp>/biblioteca/horneado/{key}.glb there"
        )
    });
    (set, glb)
}

fn gltf_json(glb: &[u8]) -> serde_json::Value {
    let len = u32::from_le_bytes(glb[12..16].try_into().unwrap()) as usize;
    serde_json::from_slice(&glb[20..20 + len]).unwrap()
}

#[test]
fn the_pilot_is_one_closed_solid_per_part_with_its_own_material() {
    let (set, glb) = pilot();
    let doc = gltf_json(&glb);
    let parts: BTreeSet<String> = doc["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|n| n.get("mesh").is_some())
        .map(|n| n["extras"]["redes_parte"].as_str().unwrap().to_string())
        .collect();
    let expected: BTreeSet<String> = ["cuerpo", "marco", "tapa", "escalones", "banqueta"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    assert_eq!(parts, expected);

    let measured = biblioteca::cantidades_glb(&glb).unwrap();
    assert!(
        measured.cerrada(),
        "{} open edges",
        measured.aristas_abiertas
    );
    let materials: BTreeSet<&str> = measured.por_material.keys().map(String::as_str).collect();
    assert_eq!(
        materials,
        BTreeSet::from(["fundicion", "hormigon", "polipropileno"])
    );
    for (material, volume) in &measured.por_material {
        assert!(*volume > 0.0, "{material}: {volume} m3, outward faces");
    }

    // Every part keeps its own extras: the cover no longer overwrites the body (docs/17 §11.6).
    let model = biblioteca::leer_glb(&glb).unwrap();
    let body = &model.extras["nodo.saneamiento_pozo__inspeccion__cuerpo.redes_material"];
    let cover = &model.extras["nodo.saneamiento_pozo__inspeccion__tapa.redes_material"];
    assert_eq!(body, set["body_material"].as_str().unwrap());
    assert_eq!(cover, set["cover_material"].as_str().unwrap());
}

#[test]
fn the_pilot_sits_on_its_slab_and_reaches_the_rim() {
    let (set, glb) = pilot();
    let model = biblioteca::leer_glb(&glb).unwrap();
    let slab = set["base_thickness_m"].as_f64().unwrap();
    let depth = set["depth_m"].as_f64().unwrap();
    assert!(model.bbox.min[2].abs() < 1e-5, "base at z = 0");
    assert!(
        (model.bbox.max[2] - (slab + depth)).abs() < 1e-4,
        "top {} = slab + depth",
        model.bbox.max[2]
    );
    let anchors = biblioteca::anclas_de_glb(&glb).unwrap();
    let invert = anchors[ANCLA_SOLERA];
    assert!((invert[2] as f64 - slab).abs() < 1e-5, "{invert:?}");
}

#[test]
fn the_pilot_has_one_anchor_per_pipe_at_its_invert() {
    let (set, glb) = pilot();
    let anchors = biblioteca::anclas_de_glb(&glb).unwrap();
    let slab = set["base_thickness_m"].as_f64().unwrap();
    let outer =
        set["inner_diameter_m"].as_f64().unwrap() / 2.0 + set["wall_thickness_m"].as_f64().unwrap();
    let pipes = set["connections"].as_array().unwrap();
    assert!(pipes.len() >= 2);
    for c in pipes {
        let name = c["name"].as_str().unwrap();
        let a = anchors
            .get(name)
            .unwrap_or_else(|| panic!("no anchor {name}: {anchors:?}"));
        let azimuth = c["azimuth_deg"].as_f64().unwrap().to_radians();
        let rise = c["invert_rise_m"].as_f64().unwrap();
        let want = [outer * azimuth.cos(), outer * azimuth.sin(), slab + rise];
        for k in 0..3 {
            assert!(
                (a[k] as f64 - want[k]).abs() < 1e-4,
                "{name}: {a:?} vs {want:?}"
            );
        }
    }
    assert_eq!(anchors.len(), pipes.len() + 1, "pipes plus the invert");
}
