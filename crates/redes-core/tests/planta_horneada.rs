//! Contract of the baked treatment units (F17-07), without Blender.
//!
//! `tools/blender/piloto/plant/` versions one bake per plant recipe: its data parameters
//! (`<recipe>.json`, the demo unit's) and the `.glb` named after the key the current recipe, its
//! modules and its driver give them. If any of them changes, the name no longer matches and the
//! test asks for a rebake (docs/familias/F17-07-planta.md); the rest checks what the viewer and
//! a reviewer rely on: closed solids per part with their own materials, the floor anchor, and one
//! anchor per pipe on the outer face at the pipe invert.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use redes_core::bake::{LoadedRecipe, ParamSet};
use redes_core::biblioteca::{self, ANCLA_SOLERA};
use serde_json::Value as Json;

/// Recipe, the parts it draws and whether its plan is round.
const RECIPES: &[(&str, &[&str], bool)] = &[
    ("build_plant_mixer", &["cuerpo", "caida"], false),
    ("build_plant_flocculator", &["cuerpo", "pantallas"], false),
    (
        "build_plant_sedimentator",
        &["cuerpo", "pantalla_difusora", "canaletas", "purga"],
        false,
    ),
    (
        "build_plant_filter",
        &["cuerpo", "falso_fondo", "grava", "lecho", "canaletas"],
        false,
    ),
    ("build_plant_contact_tank", &["cuerpo", "bafles"], false),
    (
        "build_plant_drying_bed",
        &["cuerpo", "grava", "arena", "drenes"],
        false,
    ),
    (
        "build_plant_screen",
        &["canal", "barras", "rastrillo"],
        false,
    ),
    (
        "build_plant_grit_chamber",
        &["cuerpo", "compuertas", "purga"],
        false,
    ),
    (
        "build_plant_uasb",
        &["cuerpo", "separadores", "canaletas", "entradas"],
        false,
    ),
    (
        "build_plant_clarifier",
        &["cuerpo", "canaleta", "campana", "puente", "purga"],
        true,
    ),
];

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn recipe(name: &str) -> LoadedRecipe {
    LoadedRecipe::load(&repo().join(format!("tools/blender/{name}.receta.json")))
        .unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn pilot(name: &str) -> (ParamSet, Vec<u8>) {
    let loaded = recipe(name);
    let dir = repo().join("tools/blender/piloto/plant");
    let text = std::fs::read_to_string(dir.join(format!("{name}.json"))).unwrap();
    let given: ParamSet = serde_json::from_str(&text).unwrap();
    let set = loaded
        .recipe
        .params_from_given(&given)
        .unwrap_or_else(|m| panic!("{name}: the pilot lacks {m:?}"));
    let key = loaded.key(&set);
    let glb = std::fs::read(dir.join(format!("{key}.glb"))).unwrap_or_else(|_| {
        panic!(
            "no tools/blender/piloto/plant/{key}.glb: {name} or plant_units.py changed, rebake the \
             pilot with\nredes biblioteca hornear --receta tools/blender/{name}.receta.json \
             --parametros tools/blender/piloto/plant/{name}.json --carpeta <tmp> --sin-blend\n\
             and copy <tmp>/biblioteca/horneado/{key}.glb there"
        )
    });
    (set, glb)
}

fn gltf_json(glb: &[u8]) -> Json {
    let len = u32::from_le_bytes(glb[12..16].try_into().unwrap()) as usize;
    serde_json::from_slice(&glb[20..20 + len]).unwrap()
}

fn num(set: &ParamSet, key: &str) -> f64 {
    set[key]
        .as_f64()
        .unwrap_or_else(|| panic!("{key}: {:?}", set[key]))
}

#[test]
fn every_pilot_is_one_closed_solid_per_part_with_its_materials() {
    for (name, parts, _) in RECIPES {
        let (set, glb) = pilot(name);
        let doc = gltf_json(&glb);
        let found: BTreeSet<&str> = doc["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|n| n.get("mesh").is_some())
            .map(|n| n["extras"]["redes_parte"].as_str().unwrap())
            .collect();
        assert_eq!(found, parts.iter().copied().collect(), "{name}");

        let measured = biblioteca::cantidades_glb(&glb).unwrap();
        assert!(
            measured.cerrada(),
            "{name}: {} open edges",
            measured.aristas_abiertas
        );
        for (material, volume) in &measured.por_material {
            assert!(
                *volume > 0.0,
                "{name} {material}: {volume} m3, outward faces"
            );
        }
        let body = set
            .get("body_material")
            .or_else(|| set.get("channel_material"))
            .and_then(Json::as_str)
            .unwrap();
        assert!(
            measured.por_material.contains_key(body),
            "{name}: {:?}",
            measured.por_material.keys()
        );
    }
}

#[test]
fn the_filter_layers_stack_in_order_with_their_own_materials() {
    let (set, glb) = pilot("build_plant_filter");
    let measured = biblioteca::cantidades_glb(&glb).unwrap();
    for material in [
        set["bed_material"].as_str().unwrap(),
        set["gravel_material"].as_str().unwrap(),
        set["false_floor_material"].as_str().unwrap(),
    ] {
        assert!(measured.por_material.contains_key(material), "{material}");
    }
    let model = biblioteca::leer_glb(&glb).unwrap();
    let height = num(&set, "floor_thickness_m") + num(&set, "height_m");
    assert!(
        (model.bbox.max[2] as f64 - height).abs() < 1e-4,
        "walls end at the declared total height: {}",
        model.bbox.max[2]
    );
}

#[test]
fn every_pilot_sits_its_floor_anchor_on_the_slab() {
    for (name, _, _) in RECIPES {
        let (set, glb) = pilot(name);
        let anchors = biblioteca::anclas_de_glb(&glb).unwrap();
        let floor = anchors[ANCLA_SOLERA];
        let slab = num(&set, "floor_thickness_m");
        assert!((floor[2] as f64 - slab).abs() < 1e-5, "{name}: {floor:?}");
        assert!(floor[0].abs() < 1e-6 && floor[1].abs() < 1e-6, "{name}");
        let model = biblioteca::leer_glb(&glb).unwrap();
        assert!(
            model.bbox.min[2] <= 1e-5,
            "{name}: the slab underside is z = 0 or a hopper goes below it"
        );
    }
}

#[test]
fn every_pilot_has_one_anchor_per_pipe_on_its_outer_face() {
    for (name, _, round) in RECIPES {
        let (set, glb) = pilot(name);
        let anchors = biblioteca::anclas_de_glb(&glb).unwrap();
        let pipes = set["connections"].as_array().unwrap();
        assert_eq!(
            anchors.len(),
            pipes.len() + 1,
            "{name}: pipes plus the floor"
        );
        let slab = num(&set, "floor_thickness_m");
        let t = num(&set, "wall_thickness_m");
        for c in pipes {
            let pipe = c["name"].as_str().unwrap();
            let a = anchors
                .get(pipe)
                .unwrap_or_else(|| panic!("{name}: no anchor {pipe}: {anchors:?}"));
            let rise = c["invert_rise_m"].as_f64().unwrap();
            assert!(
                (a[2] as f64 - (slab + rise)).abs() < 1e-4,
                "{name} {pipe}: {a:?}"
            );
            let (x, y) = (a[0] as f64, a[1] as f64);
            if *round {
                let r = num(&set, "inner_diameter_m") / 2.0 + t;
                assert!((x.hypot(y) - r).abs() < 1e-4, "{name} {pipe}: {a:?}");
            } else {
                let (hx, hy) = match set.get("channel_length_m") {
                    Some(l) => (l.as_f64().unwrap() / 2.0, num(&set, "width_m") / 2.0 + t),
                    None => (
                        num(&set, "length_m") / 2.0 + t,
                        num(&set, "width_m") / 2.0 + t,
                    ),
                };
                let on_face = (x.abs() - hx).abs() < 1e-4 || (y.abs() - hy).abs() < 1e-4;
                assert!(on_face, "{name} {pipe}: {a:?} not on {hx} x {hy}");
            }
        }
    }
}

#[test]
fn every_plant_recipe_agrees_with_the_registry() {
    for (name, _, _) in RECIPES {
        let r = recipe(name);
        assert_eq!(r.recipe.unit_mismatches(), Vec::<String>::new(), "{name}");
        assert_eq!(
            r.recipe.modules,
            vec!["plant_units.py".to_string()],
            "{name}"
        );
    }
}

#[test]
fn every_demo_treatment_unit_has_the_data_to_bake_on_its_floor() {
    let p = redes_core::demo_cadena::cadena();
    for (name, _, _) in RECIPES {
        let r = recipe(name);
        let units: Vec<_> = p.nodes.iter().filter(|n| r.recipe.applies_to(n)).collect();
        assert_eq!(units.len(), 1, "{name}: one unit of the demo");
        let n = units[0];
        r.recipe
            .params_for_node(&p, n)
            .unwrap_or_else(|m| panic!("{}: missing {m:?}", n.code));
        assert_eq!(
            n.datos["espesor_pared"].fuente,
            redes_core::demo_cadena::DEMO_SOURCE,
            "{}: the thickness says it is demo data",
            n.code
        );
        let floor = n
            .num("cota_fondo")
            .or(n.num("cota_umbral"))
            .or(n.datos.get("cota_piso").map(|d| d.valor))
            .unwrap();
        assert_eq!(n.z_invert, floor, "{}: the piece sits on the floor", n.code);
    }
}

#[test]
fn a_unit_without_a_datum_is_not_baked_and_says_what_it_lacks() {
    let mut p = redes_core::demo_cadena::cadena();
    let r = recipe("build_plant_sedimentator");
    let n = p.node_by_code_mut("SD-001").unwrap();
    n.datos.remove("espesor_pared");
    n.attrs.remove("tirante");
    let n = p.node_by_code("SD-001").unwrap();
    assert_eq!(
        r.recipe.params_for_node(&p, n).unwrap_err(),
        vec!["espesor_pared".to_string(), "tirante".to_string()]
    );
}
