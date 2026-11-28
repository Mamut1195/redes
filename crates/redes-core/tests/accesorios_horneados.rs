//! Contract of the pilot baked fittings, valve and hydrant (F17-05), without Blender.
//!
//! `tools/blender/piloto/fittings/` versions one bake per recipe kind: its data parameters
//! (`<kind>.json`) and the `.glb` named after the key the current recipe gives them. If a recipe,
//! `redes_mallas.py` or the driver changes, the name no longer matches and the test asks for a
//! rebake (docs/familias/F17-05-accesorios.md); the rest checks what the viewer relies on.

use std::path::{Path, PathBuf};

use redes_core::bake::{self, LoadedRecipe, ParamSet};
use redes_core::biblioteca::{self, ANCLA_SOLERA};
use redes_core::model::Project;

/// Pilot parameter set, its recipe and the parts the piece must have.
const PILOTS: [(&str, &str, &[&str]); 4] = [
    ("codo", "build_elbow", &["cuerpo", "embocaduras"]),
    ("tee", "build_tee", &["cuerpo", "embocaduras"]),
    ("compuerta", "build_valve", &["cuerpo", "accionamiento"]),
    (
        "hidrante",
        "build_hydrant",
        &["zapata_y_tubo", "columna", "tapas_y_cuadradillo"],
    ),
];

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn recipe(name: &str) -> LoadedRecipe {
    LoadedRecipe::load(&repo().join(format!("tools/blender/{name}.receta.json")))
        .unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn pilot(kind: &str, recipe_name: &str) -> (LoadedRecipe, ParamSet, String, Vec<u8>) {
    let loaded = recipe(recipe_name);
    let dir = repo().join("tools/blender/piloto/fittings");
    let text = std::fs::read_to_string(dir.join(format!("{kind}.json"))).unwrap();
    let given: ParamSet = serde_json::from_str(&text).unwrap();
    let set = loaded
        .recipe
        .params_from_given(&given)
        .unwrap_or_else(|m| panic!("{kind}: the pilot lacks {m:?}"));
    let key = loaded.key(&set);
    let glb = std::fs::read(dir.join(format!("{key}.glb"))).unwrap_or_else(|_| {
        panic!(
            "no tools/blender/piloto/fittings/{key}.glb: the recipe changed, rebake the pilot with\n\
             redes biblioteca hornear --receta tools/blender/{recipe_name}.receta.json \
             --parametros tools/blender/piloto/fittings/{kind}.json --carpeta <tmp> --sin-blend\n\
             and copy <tmp>/biblioteca/horneado/{key}.glb there"
        )
    });
    (loaded, set, key, glb)
}

fn gltf_json(glb: &[u8]) -> serde_json::Value {
    let len = u32::from_le_bytes(glb[12..16].try_into().unwrap()) as usize;
    serde_json::from_slice(&glb[20..20 + len]).unwrap()
}

#[test]
fn every_pilot_is_closed_solids_with_its_parts_and_materials() {
    for (kind, recipe_name, parts) in PILOTS {
        let (_, set, _, glb) = pilot(kind, recipe_name);
        let doc = gltf_json(&glb);
        let mut found: Vec<String> = doc["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|n| n.get("mesh").is_some())
            .map(|n| n["extras"]["redes_parte"].as_str().unwrap().to_string())
            .collect();
        found.sort();
        let mut want: Vec<String> = parts.iter().map(|s| s.to_string()).collect();
        want.sort();
        assert_eq!(found, want, "{kind}");

        let measured = biblioteca::cantidades_glb(&glb).unwrap();
        assert!(
            measured.cerrada(),
            "{kind}: {} open edges",
            measured.aristas_abiertas
        );
        for (material, volume) in &measured.por_material {
            assert!(
                *volume > 0.0,
                "{kind} {material}: {volume} m3, outward faces"
            );
        }
        if let Some(material) = set.get("material").and_then(|m| m.as_str()) {
            assert!(
                measured.por_material.contains_key(material),
                "{kind}: the body is drawn in the datum material {material}"
            );
        }
    }
}

#[test]
fn every_port_has_its_anchor_on_its_axis_pointing_to_its_pipe() {
    for (kind, recipe_name, _) in PILOTS {
        let (_, set, _, glb) = pilot(kind, recipe_name);
        let anchors = biblioteca::anclas_de_glb(&glb).unwrap();
        let invert = anchors[ANCLA_SOLERA];
        assert_eq!(
            invert,
            [0.0, 0.0, 0.0],
            "{kind}: the bore invert is the origin"
        );
        let axis_z = set["dn_mm"].as_f64().unwrap() / 2000.0;
        let ports = set["ports"].as_array().unwrap();
        for q in ports {
            let name = q["name"].as_str().unwrap();
            let a = anchors
                .get(name)
                .unwrap_or_else(|| panic!("{kind}: no anchor {name}: {anchors:?}"));
            let azimuth = (a[1] as f64).atan2(a[0] as f64).to_degrees();
            let want = q["azimuth_deg"].as_f64().unwrap();
            let off = (azimuth - want + 540.0).rem_euclid(360.0) - 180.0;
            assert!(
                off.abs() < 0.01,
                "{kind} {name}: at {azimuth}°, port {want}°"
            );
            assert!(
                (a[2] as f64 - axis_z).abs() < 1e-4,
                "{kind} {name}: z {}",
                a[2]
            );
        }
        assert_eq!(
            anchors.len(),
            ports.len() + 1,
            "{kind}: ports plus the invert"
        );
    }
}

#[test]
fn a_pilot_enters_the_manifest_placed_by_its_ports() {
    for (kind, recipe_name, _) in PILOTS {
        let (loaded, set, key, glb) = pilot(kind, recipe_name);
        let entry = bake::baked_entry(&loaded, &key, &set, &glb).unwrap();
        assert!(
            entry.extras[bake::PLACEMENT_EXTRA].starts_with("puertos"),
            "{kind}: {:?}",
            entry.extras
        );
        assert_eq!(
            entry.dn_mm,
            set["dn_mm"].as_f64().map(|d| d.round() as u32),
            "{kind}"
        );
        assert!(entry.subtipo.is_some(), "{kind}: one subtype per piece");
    }
    let (loaded, set, key, glb) = pilot("compuerta", "build_valve");
    let entry = bake::baked_entry(&loaded, &key, &set, &glb).unwrap();
    assert_eq!(entry.subtipo.as_deref(), Some("compuerta"));
    assert!(entry.id.starts_with("agua.valvula-"), "{}", entry.id);
}

#[test]
fn the_hydrant_column_stands_its_height_over_the_ground() {
    let (_, set, _, glb) = pilot("hidrante", "build_hydrant");
    let model = biblioteca::leer_glb(&glb).unwrap();
    let ground = set["bury_depth_m"].as_f64().unwrap();
    let height = set["height_m"].as_f64().unwrap();
    // The operating nut stands on the cap: 5 cm over the column.
    assert!(
        (model.bbox.max[2] - (ground + height + 0.05)).abs() < 1e-3,
        "top {} vs ground {ground} + height {height}",
        model.bbox.max[2]
    );
}

/// Every fitting of the showcase (`vitrina.plan.json`) has the data its recipe needs: what the
/// screenshots for approval show is what the recipes bake.
#[test]
fn every_fitting_of_the_showcase_bakes() {
    let mut p = Project::new("vitrina", "EPSG:32717");
    redes_core::ingest::import_plan(
        &mut p,
        &repo().join("tools/blender/piloto/fittings/vitrina.plan.json"),
    )
    .unwrap();
    let recipes: Vec<LoadedRecipe> = [
        "build_elbow",
        "build_tee",
        "build_reducer",
        "build_valve",
        "build_air_valve",
        "build_hydrant",
    ]
    .iter()
    .map(|r| recipe(r))
    .collect();
    let mut baked = 0;
    for n in &p.nodes {
        let Some(r) = recipes.iter().find(|r| r.recipe.applies_to(n)) else {
            continue;
        };
        r.recipe
            .params_for_node(&p, n)
            .unwrap_or_else(|m| panic!("{}: missing {m:?}", n.code));
        baked += 1;
    }
    assert_eq!(baked, 9, "4 fittings, 4 valves and the hydrant");
}
