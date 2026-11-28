//! Contract of the intake pieces baked by Blender (F17-06), without Blender.
//!
//! `tools/blender/piloto/intake/` versions one bake of each intake recipe: its data parameters
//! (`<pilot>.json`) and the `.glb` named after the key the current recipe gives them. If a recipe
//! or the driver changes, the name no longer matches and the test asks for a rebake; the rest
//! checks what the viewer relies on: closed parts, the invert anchor and one anchor per pipe.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use redes_core::bake::{LoadedRecipe, ParamSet};
use redes_core::biblioteca::{self, ANCLA_SOLERA};
use redes_core::model::COTA_SOLERA;

struct Pilot {
    recipe: &'static str,
    fixture: &'static str,
    parts: &'static [&'static str],
    materials: &'static [&'static str],
}

const WEIR: Pilot = Pilot {
    recipe: "build_intake_weir",
    fixture: "weir",
    parts: &["azud", "muros", "toma"],
    materials: &["hormigon"],
};
const DESANDER: Pilot = Pilot {
    recipe: "build_intake_desander",
    fixture: "desander",
    parts: &["cuerpo", "purga", "agua"],
    materials: &["agua", "hormigon", "pvc"],
};
const LOAD_CHAMBER: Pilot = Pilot {
    recipe: "build_intake_load_chamber",
    fixture: "load_chamber",
    parts: &["cuerpo", "vertedero", "tapa", "agua", "escalones"],
    materials: &["agua", "fundicion", "hormigon", "polipropileno"],
};
const BREAK_CHAMBER: Pilot = Pilot {
    recipe: "build_intake_break_chamber",
    fixture: "break_chamber",
    parts: &["cuerpo", "tapa", "agua", "escalones"],
    materials: &["agua", "fundicion", "hormigon", "polipropileno"],
};
const PILOTS: [&Pilot; 4] = [&WEIR, &DESANDER, &LOAD_CHAMBER, &BREAK_CHAMBER];
/// The pilots whose pipes cross a rectangular wall around the origin.
const BOXES: [&Pilot; 3] = [&DESANDER, &LOAD_CHAMBER, &BREAK_CHAMBER];

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn recipe(pilot: &Pilot) -> LoadedRecipe {
    let path = repo().join(format!("tools/blender/{}.receta.json", pilot.recipe));
    LoadedRecipe::load(&path).unwrap_or_else(|e| panic!("{}: {e}", pilot.recipe))
}

fn bake_of(pilot: &Pilot) -> (ParamSet, Vec<u8>) {
    let loaded = recipe(pilot);
    let dir = repo().join("tools/blender/piloto/intake");
    let text = std::fs::read_to_string(dir.join(format!("{}.json", pilot.fixture))).unwrap();
    let given: ParamSet = serde_json::from_str(&text).unwrap();
    let set = loaded
        .recipe
        .params_from_given(&given)
        .unwrap_or_else(|m| panic!("{}: the pilot lacks {m:?}", pilot.fixture));
    let key = loaded.key(&set);
    let glb = std::fs::read(dir.join(format!("{key}.glb"))).unwrap_or_else(|_| {
        panic!(
            "no tools/blender/piloto/intake/{key}.glb: the recipe changed, rebake the pilot with\n\
             redes biblioteca hornear --receta tools/blender/{}.receta.json \
             --parametros tools/blender/piloto/intake/{}.json --carpeta <tmp> --sin-blend\n\
             and copy <tmp>/biblioteca/horneado/{key}.glb there",
            pilot.recipe, pilot.fixture
        )
    });
    (set, glb)
}

fn num(set: &ParamSet, name: &str) -> f64 {
    set[name]
        .as_f64()
        .unwrap_or_else(|| panic!("{name} is a number"))
}

/// Rise above the invert that z = 0 of the piece stands for: the underside of its lowest slab.
fn lowest_rise(pilot: &Pilot, set: &ParamSet) -> f64 {
    if pilot.recipe == WEIR.recipe {
        let bed = num(set, "bed_rise_m");
        (bed - num(set, "foundation_depth_m"))
            .min(bed - num(set, "apron_thickness_m"))
            .min(num(set, "sill_rise_m") - num(set, "wall_thickness_m"))
    } else {
        num(set, "bottom_rise_m") - num(set, "slab_thickness_m")
    }
}

fn top_rise(pilot: &Pilot, set: &ParamSet) -> f64 {
    match pilot.recipe {
        r if r == WEIR.recipe => num(set, "crest_rise_m") + num(set, "wall_freeboard_m"),
        r if r == DESANDER.recipe => num(set, "bottom_rise_m") + num(set, "wall_height_m"),
        _ => num(set, "top_rise_m"),
    }
}

fn gltf_json(glb: &[u8]) -> serde_json::Value {
    let len = u32::from_le_bytes(glb[12..16].try_into().unwrap()) as usize;
    serde_json::from_slice(&glb[20..20 + len]).unwrap()
}

#[test]
fn every_intake_pilot_is_one_closed_solid_per_part_with_its_material() {
    for pilot in PILOTS {
        let (set, glb) = bake_of(pilot);
        let doc = gltf_json(&glb);
        let parts: BTreeSet<&str> = doc["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|n| n.get("mesh").is_some())
            .map(|n| n["extras"]["redes_parte"].as_str().unwrap())
            .collect();
        assert_eq!(
            parts,
            pilot.parts.iter().copied().collect(),
            "{}",
            pilot.fixture
        );

        let measured = biblioteca::cantidades_glb(&glb).unwrap();
        assert!(
            measured.cerrada(),
            "{}: {} open edges",
            pilot.fixture,
            measured.aristas_abiertas
        );
        let materials: BTreeSet<&str> = measured.por_material.keys().map(String::as_str).collect();
        assert_eq!(
            materials,
            pilot.materials.iter().copied().collect(),
            "{}",
            pilot.fixture
        );
        for (material, volume) in &measured.por_material {
            assert!(*volume > 0.0, "{}: {material} {volume} m3", pilot.fixture);
        }
        let model = biblioteca::leer_glb(&glb).unwrap();
        let body = model
            .extras
            .iter()
            .find(|(k, _)| k.ends_with(&format!("__{}.redes_material", pilot.parts[0])))
            .map(|(_, v)| v.as_str());
        assert_eq!(body, set["body_material"].as_str(), "{}", pilot.fixture);
    }
}

#[test]
fn every_intake_pilot_stands_on_its_lowest_slab_and_marks_the_invert() {
    for pilot in PILOTS {
        let (set, glb) = bake_of(pilot);
        let model = biblioteca::leer_glb(&glb).unwrap();
        let lowest = lowest_rise(pilot, &set);
        assert!(
            model.bbox.min[2].abs() < 1e-5,
            "{}: base at z = 0",
            pilot.fixture
        );
        let top = top_rise(pilot, &set) - lowest;
        assert!(
            (model.bbox.max[2] - top).abs() < 1e-4,
            "{}: top {} vs {top}",
            pilot.fixture,
            model.bbox.max[2]
        );
        let anchors = biblioteca::anclas_de_glb(&glb).unwrap();
        let invert = anchors[ANCLA_SOLERA];
        assert!(
            (invert[2] as f64 + lowest).abs() < 1e-5,
            "{}: {invert:?}",
            pilot.fixture
        );
    }
}

#[test]
fn every_pipe_anchor_is_at_its_invert_on_the_wall_it_crosses() {
    for pilot in BOXES {
        let (set, glb) = bake_of(pilot);
        let anchors = biblioteca::anclas_de_glb(&glb).unwrap();
        let t = num(&set, "wall_thickness_m");
        let (hx, hy) = (
            num(&set, "length_m") / 2.0 + t,
            num(&set, "width_m") / 2.0 + t,
        );
        let invert = anchors[ANCLA_SOLERA][2] as f64;
        let pipes = set["connections"].as_array().unwrap();
        assert!(pipes.len() >= 2, "{}", pilot.fixture);
        for c in pipes {
            let name = c["name"].as_str().unwrap();
            let a = anchors
                .get(name)
                .unwrap_or_else(|| panic!("{}: no anchor {name}", pilot.fixture));
            let azimuth = c["azimuth_deg"].as_f64().unwrap().to_radians();
            let (dx, dy) = (azimuth.cos(), azimuth.sin());
            let reach = (hx / dx.abs()).min(hy / dy.abs());
            let rise = c["invert_rise_m"].as_f64().unwrap();
            let want = [reach * dx, reach * dy, invert + rise];
            for k in 0..3 {
                assert!(
                    (a[k] as f64 - want[k]).abs() < 1e-4,
                    "{}: {name} {a:?} vs {want:?}",
                    pilot.fixture
                );
            }
        }
        let purge = usize::from(pilot.recipe == DESANDER.recipe);
        assert_eq!(anchors.len(), pipes.len() + 1 + purge, "{}", pilot.fixture);
    }
}

#[test]
fn the_intake_outlet_leaves_from_the_intake_box_on_its_own_bank() {
    let (set, glb) = bake_of(&WEIR);
    let anchors = biblioteca::anclas_de_glb(&glb).unwrap();
    let pipe = &set["connections"][0];
    let outlet = anchors[pipe["name"].as_str().unwrap()];
    let bank = num(&set, "weir_length_m") / 2.0
        + 2.0 * num(&set, "wall_thickness_m")
        + num(&set, "intake_box_length_m");
    let side = pipe["azimuth_deg"]
        .as_f64()
        .unwrap()
        .to_radians()
        .sin()
        .signum();
    assert!((outlet[1] as f64 - side * bank).abs() < 1e-4, "{outlet:?}");
    let invert = anchors[ANCLA_SOLERA][2] as f64;
    let sill = num(&set, "sill_rise_m");
    assert!((outlet[2] as f64 - (invert + pipe["invert_rise_m"].as_f64().unwrap())).abs() < 1e-4);
    assert!(
        (pipe["invert_rise_m"].as_f64().unwrap() - sill).abs() < 1e-9,
        "the pilot pipe leaves at the sill"
    );
}

#[test]
fn the_intake_recipes_agree_with_the_family_registry() {
    for pilot in PILOTS {
        assert_eq!(
            recipe(pilot).recipe.unit_mismatches(),
            Vec::<String>::new(),
            "{}",
            pilot.recipe
        );
    }
}

#[test]
fn every_intake_structure_of_the_chain_demo_has_the_data_to_bake() {
    let p = redes_core::demo_cadena::cadena();
    for pilot in PILOTS {
        let loaded = recipe(pilot);
        let objects: Vec<_> = p
            .nodes
            .iter()
            .filter(|n| loaded.recipe.applies_to(n))
            .collect();
        assert!(!objects.is_empty(), "{}: no demo object", pilot.recipe);
        for n in objects {
            loaded
                .recipe
                .params_for_node(&p, n)
                .unwrap_or_else(|m| panic!("{} {}: missing {m:?}", pilot.recipe, n.code));
            for d in n.datos.values() {
                assert_eq!(d.fuente, redes_core::demo_cadena::DEMO_SOURCE, "{}", n.code);
            }
        }
    }
    let intake = p.node_by_code("BC-001").unwrap();
    let set = recipe(&WEIR).recipe.params_for_node(&p, intake).unwrap();
    // Crest 2530.4, sill 2529.6 and bed 2528.6 over an invert at 2528.6.
    assert_eq!(set["crest_rise_m"], serde_json::json!(1.8));
    assert_eq!(set["sill_rise_m"], serde_json::json!(1.0));
    assert_eq!(set["bed_rise_m"], serde_json::json!(0.0));
}

#[test]
fn a_level_without_the_object_invert_is_missing_not_filled_in() {
    let p = redes_core::demo_cadena::cadena();
    let weir = recipe(&WEIR);
    let mut intake = p.node_by_code("BC-001").unwrap().clone();
    intake.sin_dato.push(COTA_SOLERA.into());
    assert_eq!(
        weir.recipe.params_for_node(&p, &intake).unwrap_err(),
        vec![COTA_SOLERA.to_string()]
    );
    let mut intake = p.node_by_code("BC-001").unwrap().clone();
    intake.attrs.remove("cota_cresta");
    intake.datos.remove("altura_ventana");
    assert_eq!(
        weir.recipe.params_for_node(&p, &intake).unwrap_err(),
        vec!["altura_ventana".to_string(), "cota_cresta".to_string()]
    );
}
