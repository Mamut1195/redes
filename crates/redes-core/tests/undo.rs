//! Undo by differences (F14-08): every edit the app records comes back exactly, and the memory
//! of 20 steps is a tiny fraction of 20 copies of the project.

mod comun;

use redes_core::biblioteca::{Entrada, anclar_conexiones, aplicar_a};
use redes_core::model::Project;
use redes_core::undo::{Change, History};

fn serialized(p: &Project) -> String {
    serde_json::to_string(p).expect("serializes")
}

fn model_entry() -> Entrada {
    serde_json::from_str(
        r#"{"id":"pozo-tipo","familia":"saneamiento.pozo","archivo":"pozo.glb","escala":1.0,
            "triangulos":12,"dimensiones_m":[1.2,1.2,2.0],"bytes":640,"volumen_m3":2.0,
            "anclas":{"entrada":[0.6,0.0,-1.5],"salida":[-0.6,0.0,-1.5]}}"#,
    )
    .expect("model entry")
}

/// The same edit the app makes when a model is picked in the inspector (`aplicar_modelo`).
fn assign_model(p: &mut Project, code: &str) {
    let entry = model_entry();
    aplicar_a(p.node_by_code_mut(code).expect("node"), &entry, 2.4);
    anclar_conexiones(p, code, &entry.anclas);
    p.commit("ui.modelo");
}

type Edit = fn(&mut Project);

/// Every operation that enters the app's undo stack, plus reopening.
fn edits() -> Vec<(&'static str, Edit)> {
    vec![
        ("observe", |p| {
            p.observar("PZ-0-5", "Tapa desplazada", None).unwrap();
        }),
        ("close observation", |p| {
            p.cerrar_observacion("PZ-0-5", 1, Some("Corregido"))
                .unwrap();
        }),
        ("approve", |p| {
            p.aprobar_objeto("PZ-0-5", None).unwrap();
        }),
        ("reopen", |p| {
            p.reabrir_objeto("PZ-0-5").unwrap();
        }),
        ("observe link", |p| {
            p.observar("CL-1-7", "Pendiente invertida", None).unwrap();
        }),
        ("approve link closing all", |p| {
            p.aprobar_objeto("CL-1-7", Some("Revisado")).unwrap();
        }),
        ("assign model", |p| assign_model(p, "PZ-1-8")),
    ]
}

/// Each step undone gives back exactly the previous project (same serialization, indices up to
/// date) and each step redone gives back the edited one.
#[test]
fn undo_and_redo_restore_every_kind_of_edit_exactly() {
    let mut p = comun::proyecto(400);
    let mut history = History::default();
    let mut states = vec![serialized(&p)];
    for (_, edit) in edits() {
        let before = p.clone();
        edit(&mut p);
        history.record(&before, &p);
        states.push(serialized(&p));
    }

    for (step, (name, _)) in edits().iter().enumerate().rev() {
        assert!(history.undo(&mut p), "{name}: undo");
        assert_eq!(serialized(&p), states[step], "undo {name}");
        assert!(p.indice_al_dia(), "undo {name}: indices");
    }
    assert!(!history.undo(&mut p), "nothing left to undo");

    for (step, (name, _)) in edits().iter().enumerate() {
        assert!(history.redo(&mut p), "{name}: redo");
        assert_eq!(serialized(&p), states[step + 1], "redo {name}");
    }
    assert!(!history.redo(&mut p), "nothing left to redo");

    let anchored = p.link_by_code("CL-1-8").expect("link leaving the node");
    let node = p.node_by_code("PZ-1-8").unwrap();
    assert_eq!(anchored.from, node.id, "lookups still work after redo");
    assert_eq!(node.modelo.as_deref(), Some("pozo-tipo"));
}

/// The model edit moves link ends: undoing it puts every vertex back.
#[test]
fn undoing_a_model_puts_the_link_ends_back() {
    let mut p = comun::proyecto(400);
    let before = p.clone();
    assign_model(&mut p, "PZ-1-8");
    let change = Change::between(&before, &p);
    assert_ne!(
        p.link_by_code("CL-1-8").unwrap().path,
        before.link_by_code("CL-1-8").unwrap().path,
        "the anchor moved the end"
    );
    let mut history = History::default();
    history.record(&before, &p);
    history.undo(&mut p);
    assert_eq!(serialized(&p), serialized(&before));
    assert!(change.estimated_bytes() > 0);
}

/// An edit that adds an object changes the vector lengths: undo and redo rebuild the indices.
#[test]
fn undoing_an_added_node_keeps_the_indices_current() {
    let mut p = comun::proyecto(400);
    let before = p.clone();
    let mut node = p.node_by_code("PZ-0-0").unwrap().clone();
    node.code = "PZ-NEW".into();
    p.add_node(node).unwrap();
    p.commit("test.add");
    let mut history = History::default();
    history.record(&before, &p);

    history.undo(&mut p);
    assert_eq!(serialized(&p), serialized(&before));
    assert!(p.indice_al_dia(), "indices rebuilt after undo");
    assert!(p.node_by_code("PZ-NEW").is_none());

    history.redo(&mut p);
    assert!(p.indice_al_dia(), "indices rebuilt after redo");
    assert!(p.node_by_code("PZ-NEW").is_some());
}

/// A new edit after undoing drops what could be redone, as every editor does.
#[test]
fn a_new_edit_discards_the_redo_stack() {
    let mut p = comun::proyecto(400);
    let mut history = History::default();
    let before = p.clone();
    p.observar("PZ-0-1", "Uno", None).unwrap();
    history.record(&before, &p);
    history.undo(&mut p);
    let before = p.clone();
    p.observar("PZ-0-2", "Dos", None).unwrap();
    history.record(&before, &p);
    assert_eq!(history.redo_steps(), 0);
    assert!(!history.redo(&mut p));
    assert_eq!(history.undo_steps(), 1);
}

/// The stack keeps only the last `limit` steps.
#[test]
fn the_history_keeps_only_the_last_steps() {
    let mut p = comun::proyecto(400);
    let mut history = History::with_limit(3);
    for c in 0..5 {
        let before = p.clone();
        p.observar(&format!("PZ-0-{c}"), "Nota", None).unwrap();
        history.record(&before, &p);
    }
    assert_eq!(history.undo_steps(), 3);
    while history.undo(&mut p) {}
    assert_eq!(p.observaciones_de("PZ-0-1").unwrap().len(), 1, "too old");
    assert!(p.observaciones_de("PZ-0-2").unwrap().is_empty(), "undone");
}

/// 20 review steps on the 20k network weigh well under 1 % of 20 copies of the project.
#[test]
fn twenty_steps_on_20k_links_weigh_a_tiny_fraction_of_copies() {
    let mut p = comun::proyecto(20_000);
    let copy_bytes = serialized(&p).len();
    let mut history = History::default();
    for c in 0..20 {
        let before = p.clone();
        p.observar(&format!("PZ-3-{c}"), "Tapa desplazada", None)
            .unwrap();
        history.record(&before, &p);
    }
    let steps_bytes = history.estimated_bytes();
    let copies_bytes = 20 * copy_bytes;
    eprintln!(
        "undo 20k: 20 copies {:.1} MB, 20 differences {:.1} kB",
        copies_bytes as f64 / 1e6,
        steps_bytes as f64 / 1e3
    );
    assert_eq!(history.undo_steps(), 20);
    assert!(
        steps_bytes * 100 < copies_bytes,
        "{steps_bytes} B for 20 steps vs {copies_bytes} B for 20 copies"
    );
    assert!(
        steps_bytes < 20 * 4096,
        "{steps_bytes} B: about one node per step"
    );
}
