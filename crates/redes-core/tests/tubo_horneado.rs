//! Contract of the pilot baked pipe segment (F17-04), without Blender.
//!
//! `tools/blender/piloto/pipe/` versions one bake of the pipe recipe: its data parameters
//! (`piloto.json`) and the `.glb` named after the key the current recipe gives them. If the recipe
//! or its driver changes, the name no longer matches and this test asks for a rebake; the rest
//! checks what the viewer relies on to lay the segment along a link.

mod comun;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use redes_core::bake::{LoadedRecipe, ParamSet};
use redes_core::biblioteca;
use redes_core::pipe_layout::{
    BARREL_PART, PieceKind, PipeSegment, SEGMENT_END_ANCHOR, SEGMENT_START_ANCHOR, lay_segments,
};

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn pilot() -> (ParamSet, Vec<u8>) {
    let loaded = LoadedRecipe::load(&repo().join("tools/blender/build_pipe.receta.json"))
        .expect("the pipe recipe loads");
    let dir = repo().join("tools/blender/piloto/pipe");
    let given: ParamSet =
        serde_json::from_str(&std::fs::read_to_string(dir.join("piloto.json")).unwrap()).unwrap();
    let set = loaded
        .recipe
        .params_from_given(&given)
        .expect("the pilot gives every datum");
    let key = loaded.key(&set);
    let glb = std::fs::read(dir.join(format!("{key}.glb"))).unwrap_or_else(|_| {
        panic!(
            "no tools/blender/piloto/pipe/{key}.glb: the recipe changed, rebake the pilot with\n\
             redes biblioteca hornear --receta tools/blender/build_pipe.receta.json \
             --parametros tools/blender/piloto/pipe/piloto.json --carpeta <tmp> --sin-blend\n\
             and copy <tmp>/biblioteca/horneado/{key}.glb there"
        )
    });
    (set, glb)
}

fn num(set: &ParamSet, name: &str) -> f64 {
    set[name].as_f64().unwrap()
}

/// Laying length the recipe gives the pilot's material.
fn laying_length(set: &ParamSet) -> f64 {
    set["segment_length_m"]["pvc"].as_f64().unwrap()
}

#[test]
fn the_pilot_is_a_closed_barrel_bell_and_ring_with_their_materials() {
    let (set, glb) = pilot();
    let parts: BTreeSet<String> = biblioteca::triangles_by_part(&glb)
        .unwrap()
        .into_keys()
        .collect();
    assert_eq!(
        parts,
        BTreeSet::from(["cuerpo".into(), "campana".into(), "junta".into()])
    );
    let measured = biblioteca::cantidades_glb(&glb).unwrap();
    assert!(
        measured.cerrada(),
        "{} open edges",
        measured.aristas_abiertas
    );
    let materials: BTreeSet<&str> = measured.por_material.keys().map(String::as_str).collect();
    assert_eq!(materials, BTreeSet::from(["caucho", "pvc"]));
    for (material, volume) in &measured.por_material {
        assert!(*volume > 0.0, "{material}: {volume} m3, outward faces");
    }
    let model = biblioteca::leer_glb(&glb).unwrap();
    assert_eq!(
        model.extras["nodo.tuberia__cuerpo.redes_material"],
        set["pipe_material"].as_str().unwrap()
    );
    assert_eq!(model.extras["nodo.tuberia__junta.redes_material"], "caucho");
}

#[test]
fn the_barrel_has_the_declared_bore_and_wall_and_the_bell_is_wider() {
    let (set, glb) = pilot();
    let ri = num(&set, "inner_diameter_m") / 2.0;
    let ro = ri + num(&set, "wall_thickness_m");
    let parts = biblioteca::triangles_by_part(&glb).unwrap();
    let radii = |part: &str| {
        let r: Vec<f64> = parts[part]
            .iter()
            .flat_map(|t| t.pos)
            .map(|q| q[1].hypot(q[2]))
            .collect();
        (
            r.iter().cloned().fold(f64::INFINITY, f64::min),
            r.iter().cloned().fold(0.0, f64::max),
        )
    };
    let (barrel_in, barrel_out) = radii(BARREL_PART);
    assert!((barrel_in - ri).abs() < 1e-4, "bore {barrel_in} vs {ri}");
    assert!(
        (barrel_out - ro).abs() < 1e-4,
        "outside {barrel_out} vs {ro}"
    );
    let (_, bell_out) = radii("campana");
    assert!(
        bell_out > ro + 0.004,
        "the bell {bell_out} stands out of the barrel {ro}"
    );
    // The ring grips the spigot of the next segment.
    let (ring_in, _) = radii("junta");
    assert!((ring_in - ro).abs() < 1e-4, "ring {ring_in} vs spigot {ro}");
}

#[test]
fn the_axis_anchors_give_the_laying_length_and_the_bell_takes_the_next_spigot() {
    let (set, glb) = pilot();
    let anchors = biblioteca::anclas_de_glb(&glb).unwrap();
    let start = anchors[SEGMENT_START_ANCHOR];
    let end = anchors[SEGMENT_END_ANCHOR];
    assert!(start.iter().all(|v| v.abs() < 1e-5), "{start:?}");
    assert!(
        ((end[0] as f64) - laying_length(&set)).abs() < 1e-4,
        "{end:?}"
    );
    assert!(end[1].abs() < 1e-5 && end[2].abs() < 1e-5);

    let seg = PipeSegment::from_glb(&glb).unwrap();
    assert!((seg.laying_length_m - laying_length(&set)).abs() < 1e-4);
    assert!(
        seg.barrel_end_m < seg.laying_length_m,
        "the bell starts before the end"
    );
    assert!(seg.span_m.0.abs() < 1e-5, "the spigot end is the origin");
    assert!(
        seg.span_m.1 > seg.laying_length_m,
        "the socket reaches over the next spigot"
    );
    assert_eq!(seg.full.len() % 3, 0);
    assert!(seg.barrel.len() < seg.full.len());
}

#[test]
fn the_pilot_laid_on_a_sloped_link_ends_exactly_at_its_axis_end() {
    let (_, glb) = pilot();
    let seg = PipeSegment::from_glb(&glb).unwrap();
    let axis = [[0.0, 0.0, 2500.0], [30.0, 40.0, 2499.0]];
    let pieces = lay_segments(&axis, &seg);
    let total = (50.0f64.powi(2) + 1.0).sqrt();
    assert_eq!(pieces.len(), (total / seg.laying_length_m).ceil() as usize);
    let last = pieces.last().unwrap();
    assert_eq!(last.kind, PieceKind::Barrel);
    let far = seg
        .barrel
        .iter()
        .map(|v| last.place(v.pos))
        .fold(f64::NEG_INFINITY, |m, q| m.max(q[0] * 0.6 + q[1] * 0.8));
    assert!(
        (far - 50.0).abs() < 0.01,
        "the cut barrel ends at the axis end, {far}"
    );
    for w in pieces.windows(2) {
        let gap = (0..3)
            .map(|k| (w[0].end()[k] - w[1].start[k]).powi(2))
            .sum::<f64>();
        assert!(gap < 1e-12, "segments follow each other");
    }
}

/// Figures for docs/familias/F17-04-tuberia.md: the 20 000-link bench network with every link
/// drawn by the pilot segment, in the painter and in the GPU scene. Ignored by default:
/// `cargo test --release -p redes-core --test tubo_horneado -- --ignored --nocapture`.
#[test]
#[ignore]
fn figures_of_20k_links_drawn_with_baked_segments() {
    use redes_core::pipe_layout::PipePieces;
    use redes_core::view::{Camera, Layers, Opciones};
    let (_, glb) = pilot();
    let seg = std::sync::Arc::new(PipeSegment::from_glb(&glb).unwrap());
    let p = comun::proyecto(20_000);
    let pipes = PipePieces {
        by_link: p.links.iter().map(|l| (l.id, seg.clone())).collect(),
        node_anchors: Default::default(),
    };
    let laid: usize = p
        .links
        .iter()
        .map(|l| {
            lay_segments(
                &redes_core::pipe_layout::drawn_axis_local(&p, l, &Default::default()),
                &seg,
            )
            .len()
        })
        .sum();
    let timed = |f: &mut dyn FnMut()| {
        let mut t: Vec<f64> = (0..5)
            .map(|_| {
                let t0 = std::time::Instant::now();
                f();
                t0.elapsed().as_secs_f64() * 1000.0
            })
            .collect();
        t.sort_by(f64::total_cmp);
        t[2]
    };
    let mut cam = Camera {
        viewport: [900.0, 600.0],
        ..Camera::default()
    };
    cam.fit(&p);
    let painter = |pipes: &PipePieces| {
        redes_core::view::build_with_library(
            &p,
            p.terrain.as_ref(),
            &cam,
            Layers::default(),
            None,
            &Opciones::default(),
            &Default::default(),
            pipes,
        )
    };
    let painter_plain = timed(&mut || {
        std::hint::black_box(painter(&PipePieces::default()));
    });
    let painter_baked = timed(&mut || {
        std::hint::black_box(painter(&pipes));
    });
    let gpu = |pipes: &PipePieces| {
        redes_core::scene3d::Scene::build_with_pipes(
            &p,
            Layers::default(),
            &[],
            pipes,
            &mut Default::default(),
            redes_core::scene3d::GPU_BUDGET,
        )
    };
    let gpu_plain_ms = timed(&mut || {
        std::hint::black_box(gpu(&PipePieces::default()));
    });
    let gpu_baked_ms = timed(&mut || {
        std::hint::black_box(gpu(&pipes));
    });
    let (plain, baked) = (gpu(&PipePieces::default()), gpu(&pipes));
    let mb = |s: &redes_core::scene3d::Scene| s.gpu_bytes() as f64 / (1024.0 * 1024.0);
    println!(
        "20k links, {laid} segments laid; painter {painter_plain:.1} -> {painter_baked:.1} ms; \
         GPU scene {gpu_plain_ms:.1} -> {gpu_baked_ms:.1} ms, {:.1} -> {:.1} MB, warning {:?}",
        mb(&plain),
        mb(&baked),
        baked.warning
    );
}
