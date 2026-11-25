//! `redes validar` (F13-01): cada regla con un caso que la dispara, uno que no y uno «no
//! evaluable» (falta un dato). Más el fixture de proyecto pequeño que usa también la CLI.
#![allow(clippy::too_many_arguments)]

use std::collections::BTreeMap;
use std::path::PathBuf;

use redes_core::families::Value;
use redes_core::geom::{P2, P3};
use redes_core::model::{
    COTA_SOLERA, COTA_TAPA, COTA_TERRENO, DIAMETRO_NOMINAL, ElementId, EstadoObjeto, Link, Node,
    Project,
};
use redes_core::validar::{Informe, ReglasValidacion, Severidad, validar};

// --- constructores ---------------------------------------------------------------------------

fn nodo(p: &mut Project, fam: &str, ty: &str, code: &str, x: f64, y: f64, sol: f64, ter: f64) {
    p.add_node(Node {
        id: ElementId::new(0, 0),
        family: fam.into(),
        ty: ty.into(),
        code: code.into(),
        pos: P2::new(x, y),
        z_terrain: ter,
        z_rim: ter,
        z_invert: sol,
        attrs: BTreeMap::new(),
        datos: BTreeMap::new(),
        modelo: None,
        estado: EstadoObjeto::Pendiente,
        observaciones: Vec::new(),
        deleted: false,
        sin_dato: Vec::new(),
    })
    .unwrap();
}

fn pozo(p: &mut Project, code: &str, x: f64, y: f64, sol: f64, ter: f64) {
    nodo(p, "saneamiento.pozo", "inspeccion", code, x, y, sol, ter);
}

/// Tramo recto entre dos nodos con soleras `za`→`zb` y DN; DI y espesor declarados.
fn tramo(
    p: &mut Project,
    fam: &str,
    ty: &str,
    code: &str,
    de: &str,
    a: &str,
    za: f64,
    zb: f64,
    dn: u32,
) {
    let (na, nb) = (p.node_by_code(de).unwrap(), p.node_by_code(a).unwrap());
    let (from, to, pa, pb) = (na.id, nb.id, na.pos, nb.pos);
    p.add_link(Link {
        id: ElementId::new(0, 0),
        family: fam.into(),
        ty: ty.into(),
        code: code.into(),
        from,
        to,
        path: vec![P3::new(pa.x, pa.y, za), P3::new(pb.x, pb.y, zb)],
        d_mm: dn,
        attrs: BTreeMap::from([
            ("diametro_interior".to_string(), Value::Num(dn as f64)),
            ("espesor_pared".to_string(), Value::Num(0.01)),
        ]),
        datos: BTreeMap::new(),
        estado: EstadoObjeto::Pendiente,
        observaciones: Vec::new(),
        deleted: false,
        cota_solera_inicio: Some(za),
        cota_solera_fin: Some(zb),
        sin_dato: Vec::new(),
    })
    .unwrap();
}

fn colector(p: &mut Project, code: &str, de: &str, a: &str, za: f64, zb: f64, dn: u32) {
    tramo(
        p,
        "saneamiento.tuberia",
        "colector_sanitario",
        code,
        de,
        a,
        za,
        zb,
        dn,
    );
}

fn link_mut<'a>(p: &'a mut Project, code: &str) -> &'a mut Link {
    p.links.iter_mut().find(|l| l.code == code).unwrap()
}
fn node_mut<'a>(p: &'a mut Project, code: &str) -> &'a mut Node {
    p.nodes.iter_mut().find(|n| n.code == code).unwrap()
}

/// Dos pozos a 50 m unidos por CL-1 con pendiente 1 %, terreno 2 m sobre la solera.
fn base() -> Project {
    let mut p = Project::new("validar", "EPSG:32717");
    pozo(&mut p, "PZ-1", 0.0, 0.0, 100.0, 102.0);
    pozo(&mut p, "PZ-2", 50.0, 0.0, 99.5, 101.8);
    colector(&mut p, "CL-1", "PZ-1", "PZ-2", 100.0, 99.5, 200);
    p
}

fn corre(p: &Project, regla: &str) -> Informe {
    validar(p, &ReglasValidacion::default(), Some(&[regla]))
}

/// (evaluados, incumplen, no_evaluables) de la única regla pedida.
fn cuenta(i: &Informe) -> (usize, usize, usize) {
    assert_eq!(i.resumen.len(), 1);
    let r = &i.resumen[0];
    (r.evaluados, r.incumplen, r.no_evaluables)
}

fn objetos(i: &Informe, sev: Severidad) -> Vec<Vec<String>> {
    i.hallazgos
        .iter()
        .filter(|h| h.severidad == sev)
        .map(|h| h.objetos.clone())
        .collect()
}

// --- contrapendiente -------------------------------------------------------------------------

#[test]
fn contrapendiente_dispara_cumple_y_no_evaluable() {
    let p = base();
    assert_eq!(cuenta(&corre(&p, "contrapendiente")), (1, 0, 0));

    let mut q = base();
    let l = link_mut(&mut q, "CL-1");
    l.cota_solera_fin = Some(100.2);
    l.path[1].z = 100.2;
    let i = corre(&q, "contrapendiente");
    assert_eq!(cuenta(&i), (1, 1, 0));
    assert_eq!(i.hallazgos[0].severidad, Severidad::Grave);
    assert!((i.hallazgos[0].medido.unwrap() - 0.2).abs() < 1e-9);

    let mut q = base();
    let l = link_mut(&mut q, "CL-1");
    l.cota_solera_inicio = None;
    l.cota_solera_fin = None;
    l.sin_dato.push(COTA_SOLERA.into());
    let i = corre(&q, "contrapendiente");
    assert_eq!(cuenta(&i), (0, 0, 1));
    assert_eq!(i.hallazgos[0].falta.as_deref(), Some("cota_solera"));
    assert!(i.hallazgos[0].medido.is_none(), "nunca un valor supuesto");
}

// --- pendiente -------------------------------------------------------------------------------

#[test]
fn pendiente_fuera_de_rango_por_sistema() {
    assert_eq!(cuenta(&corre(&base(), "pendiente")), (1, 0, 0));

    let mut q = base();
    let l = link_mut(&mut q, "CL-1");
    l.cota_solera_fin = Some(99.9); // 0,2 % < 0,5 % sanitario
    let i = corre(&q, "pendiente");
    assert_eq!(cuenta(&i), (1, 1, 0));
    assert!((i.hallazgos[0].umbral.unwrap() - 0.005).abs() < 1e-12);
    // El mismo tramo como pluvial (mínimo 0,3 %) tampoco cumple; con 0,4 % sí.
    let l = link_mut(&mut q, "CL-1");
    l.ty = "colector_pluvial".into();
    l.cota_solera_fin = Some(99.8);
    assert_eq!(cuenta(&corre(&q, "pendiente")), (1, 0, 0));
    // Umbrales del usuario.
    let r = ReglasValidacion {
        pendiente_pluvial: redes_core::validar::Rango {
            min: 0.01,
            max: 0.2,
        },
        ..Default::default()
    };
    assert_eq!(
        validar(&q, &r, Some(&["pendiente"])).resumen[0].incumplen,
        1
    );

    let mut q = base();
    let l = link_mut(&mut q, "CL-1");
    l.cota_solera_fin = None;
    l.sin_dato.push(COTA_SOLERA.into());
    assert_eq!(cuenta(&corre(&q, "pendiente")), (0, 0, 1));
}

// --- salto en pozo ---------------------------------------------------------------------------

#[test]
fn salto_de_solera_sin_pozo_de_caida() {
    // CL-1 llega a 99,5 y el pozo tiene la solera en 99,5: sin salto.
    assert_eq!(cuenta(&corre(&base(), "salto_pozo")), (1, 0, 0));

    let mut q = base();
    node_mut(&mut q, "PZ-2").z_invert = 98.5; // salto 1,0 m > 0,75
    let i = corre(&q, "salto_pozo");
    assert_eq!(cuenta(&i), (1, 1, 0));
    assert_eq!(i.hallazgos[0].objetos, vec!["PZ-2", "CL-1"]);
    // Con subtipo de caída deja de ser hallazgo.
    node_mut(&mut q, "PZ-2").ty = "caida_exterior".into();
    assert_eq!(corre(&q, "salto_pozo").resumen[0].incumplen, 0);

    let mut q = base();
    node_mut(&mut q, "PZ-2").sin_dato.push(COTA_SOLERA.into());
    let i = corre(&q, "salto_pozo");
    assert_eq!(cuenta(&i), (0, 0, 1));
    assert!(i.hallazgos[0].falta.as_deref().unwrap().contains("PZ-2"));
}

// --- recubrimiento ---------------------------------------------------------------------------

#[test]
fn recubrimiento_minimo_en_extremos() {
    // PZ-1: 102 − (100 + 0,20 + 0,01) = 1,79; PZ-2: 101,8 − 99,71 = 2,09.
    assert_eq!(cuenta(&corre(&base(), "recubrimiento")), (1, 0, 0));

    let mut q = base();
    node_mut(&mut q, "PZ-2").z_terrain = 100.5; // 100,5 − 99,71 = 0,79 < 1,0
    let i = corre(&q, "recubrimiento");
    assert_eq!(cuenta(&i), (1, 1, 0));
    assert!((i.hallazgos[0].medido.unwrap() - 0.79).abs() < 1e-9);
    assert_eq!(i.hallazgos[0].ubicacion, Some([50.0, 0.0]));

    // Sin espesor no se supone uno: no evaluable...
    let mut q = base();
    link_mut(&mut q, "CL-1").attrs.remove("espesor_pared");
    let i = corre(&q, "recubrimiento");
    assert_eq!(cuenta(&i), (0, 0, 1));
    assert!(
        i.hallazgos[0]
            .falta
            .as_deref()
            .unwrap()
            .contains("espesor_pared")
    );
    // ...salvo que, aun sin contarlo, ya no cumpla (cota segura).
    node_mut(&mut q, "PZ-2").z_terrain = 100.4; // ≤ 100,4 − 99,7 = 0,70
    let i = corre(&q, "recubrimiento");
    assert_eq!(cuenta(&i), (1, 1, 0));
    assert!(i.hallazgos[0].mensaje.contains("sin contar"));

    // Sin terreno en el nodo ni TIN: falta la rasante.
    let mut q = base();
    node_mut(&mut q, "PZ-1").sin_dato.push(COTA_TERRENO.into());
    let i = corre(&q, "recubrimiento");
    assert_eq!(cuenta(&i), (0, 0, 1));
    assert!(i.hallazgos[0].falta.as_deref().unwrap().contains("rasante"));
}

// --- profundidad -----------------------------------------------------------------------------

#[test]
fn profundidad_maxima_del_pozo() {
    assert_eq!(cuenta(&corre(&base(), "profundidad_pozo")), (2, 0, 0));

    let mut q = base();
    let n = node_mut(&mut q, "PZ-1");
    n.z_rim = 106.0; // 6 m > 5 m
    let i = corre(&q, "profundidad_pozo");
    assert_eq!(cuenta(&i), (2, 1, 0));
    assert_eq!(i.hallazgos[0].objetos, vec!["PZ-1"]);

    let mut q = base();
    let n = node_mut(&mut q, "PZ-1");
    n.sin_dato.push(COTA_TAPA.into());
    n.sin_dato.push(COTA_TERRENO.into());
    let i = corre(&q, "profundidad_pozo");
    assert_eq!(cuenta(&i), (1, 0, 1));
}

// --- separación agua / alcantarillado --------------------------------------------------------

/// Agua que cruza CL-1 en x = 25 (solera del colector 99,75; clave exterior 99,96).
fn con_agua(solera_agua: f64, x: f64) -> Project {
    let mut p = base();
    nodo(
        &mut p,
        "agua.hidrante",
        "columna",
        "HI-1",
        x,
        -20.0,
        solera_agua,
        102.0,
    );
    nodo(
        &mut p,
        "agua.hidrante",
        "columna",
        "HI-2",
        x,
        20.0,
        solera_agua,
        102.0,
    );
    tramo(
        &mut p,
        "agua.tuberia",
        "distribucion_primaria",
        "TB-1",
        "HI-1",
        "HI-2",
        solera_agua,
        solera_agua,
        100,
    );
    p
}

#[test]
fn separacion_agua_alcantarillado() {
    // Agua 0,5 m por encima: fondo exterior 100,49 − clave 99,96 = 0,53 ≥ 0,30.
    assert_eq!(
        cuenta(&corre(&con_agua(100.5, 25.0), "separacion")),
        (1, 0, 0)
    );
    // Lejos en planta: cumple aunque esté por debajo.
    assert_eq!(
        cuenta(&corre(&con_agua(98.0, 60.0), "separacion")),
        (1, 0, 0)
    );

    // Agua 0,2 m por encima: aviso.
    let i = corre(&con_agua(100.17, 25.0), "separacion");
    assert_eq!(cuenta(&i), (1, 1, 0));
    assert_eq!(i.hallazgos[0].severidad, Severidad::Aviso);
    assert_eq!(i.hallazgos[0].objetos, vec!["TB-1", "CL-1"]);
    assert_eq!(i.hallazgos[0].ubicacion, Some([25.0, 0.0]));
    // Agua por debajo del colector: grave.
    let i = corre(&con_agua(99.0, 25.0), "separacion");
    assert_eq!(i.hallazgos[0].severidad, Severidad::Grave);
    // Paralela a 2 m (dentro de los 3 m) y por debajo: también.
    let mut p = base();
    nodo(
        &mut p,
        "agua.hidrante",
        "columna",
        "HI-1",
        0.0,
        2.0,
        99.0,
        102.0,
    );
    nodo(
        &mut p,
        "agua.hidrante",
        "columna",
        "HI-2",
        50.0,
        2.0,
        99.0,
        102.0,
    );
    tramo(
        &mut p,
        "agua.tuberia",
        "distribucion_primaria",
        "TB-1",
        "HI-1",
        "HI-2",
        99.0,
        99.0,
        100,
    );
    let i = corre(&p, "separacion");
    assert_eq!(cuenta(&i), (1, 1, 0));
    assert!((i.hallazgos[0].mensaje.contains("2.00 m en planta")));

    // Sin espesor del agua y holgura suficiente sin él: no evaluable.
    let mut p = con_agua(100.5, 25.0);
    link_mut(&mut p, "TB-1").attrs.remove("espesor_pared");
    let i = corre(&p, "separacion");
    assert_eq!(cuenta(&i), (0, 0, 1));
    assert!(
        i.hallazgos[0]
            .falta
            .as_deref()
            .unwrap()
            .contains("espesor_pared del agua")
    );
}

/// La rejilla no compara todos los pares: dos redes lejanas no generan pares.
#[test]
fn separacion_solo_mira_los_cercanos() {
    let mut p = base();
    for k in 0..50 {
        let y = 1000.0 + k as f64 * 40.0;
        nodo(
            &mut p,
            "agua.hidrante",
            "columna",
            &format!("HI-{k}a"),
            0.0,
            y,
            100.0,
            102.0,
        );
        nodo(
            &mut p,
            "agua.hidrante",
            "columna",
            &format!("HI-{k}b"),
            50.0,
            y,
            100.0,
            102.0,
        );
        tramo(
            &mut p,
            "agua.tuberia",
            "ramal",
            &format!("TB-{k}"),
            &format!("HI-{k}a"),
            &format!("HI-{k}b"),
            100.0,
            100.0,
            100,
        );
    }
    let i = corre(&p, "separacion");
    assert_eq!(cuenta(&i), (0, 0, 0), "ningún par cercano");
}

// --- DN aguas abajo --------------------------------------------------------------------------

fn cadena_dn(dn1: u32, dn2: u32) -> Project {
    let mut p = base();
    pozo(&mut p, "PZ-3", 100.0, 0.0, 99.0, 101.5);
    link_mut(&mut p, "CL-1").d_mm = dn1;
    colector(&mut p, "CL-2", "PZ-2", "PZ-3", 99.5, 99.0, dn2);
    p
}

#[test]
fn dn_que_disminuye_aguas_abajo() {
    assert_eq!(
        cuenta(&corre(&cadena_dn(200, 250), "dn_aguas_abajo")),
        (1, 0, 0)
    );
    let i = corre(&cadena_dn(300, 250), "dn_aguas_abajo");
    assert_eq!(cuenta(&i), (1, 1, 0));
    assert_eq!(i.hallazgos[0].objetos, vec!["CL-2", "CL-1", "PZ-2"]);

    let mut p = cadena_dn(200, 250);
    let l = link_mut(&mut p, "CL-1");
    l.d_mm = 0;
    l.sin_dato.push(DIAMETRO_NOMINAL.into());
    assert_eq!(cuenta(&corre(&p, "dn_aguas_abajo")), (0, 0, 1));
}

// --- huérfanos y aislados --------------------------------------------------------------------

/// Reglas estructurales: no dependen de cotas ni diámetros, así que nunca son «no evaluables».
#[test]
fn tramos_huerfanos_y_nodos_aislados() {
    let p = base();
    assert_eq!(cuenta(&corre(&p, "tramo_huerfano")), (1, 0, 0));
    assert_eq!(cuenta(&corre(&p, "nodo_aislado")), (2, 0, 0));

    let mut q = base();
    pozo(&mut q, "PZ-9", 300.0, 0.0, 100.0, 102.0);
    let i = corre(&q, "nodo_aislado");
    assert_eq!(cuenta(&i), (3, 1, 0));
    assert_eq!(i.hallazgos[0].objetos, vec!["PZ-9"]);
    // Sin datos de cota sigue siendo evaluable.
    node_mut(&mut q, "PZ-9").sin_dato.push(COTA_SOLERA.into());
    assert_eq!(cuenta(&corre(&q, "nodo_aislado")).2, 0);

    let mut q = base();
    node_mut(&mut q, "PZ-2").deleted = true; // queda el tramo colgando (archivo editado a mano)
    let i = corre(&q, "tramo_huerfano");
    assert_eq!(cuenta(&i), (1, 1, 0));
    assert_eq!(i.hallazgos[0].severidad, Severidad::Grave);
    assert!(i.hallazgos[0].mensaje.contains("fin"));
}

// --- longitud --------------------------------------------------------------------------------

#[test]
fn longitud_declarada_contra_medida() {
    let mut p = base();
    link_mut(&mut p, "CL-1")
        .attrs
        .insert("longitud".into(), Value::Num(50.2));
    assert_eq!(cuenta(&corre(&p, "longitud")), (1, 0, 0));
    link_mut(&mut p, "CL-1")
        .attrs
        .insert("longitud".into(), Value::Num(53.0));
    let i = corre(&p, "longitud");
    assert_eq!(cuenta(&i), (1, 1, 0));
    assert!((i.hallazgos[0].medido.unwrap() - 50.0025).abs() < 1e-3);
    // Sin longitud declarada no hay con qué comparar.
    assert_eq!(cuenta(&corre(&base(), "longitud")), (0, 0, 1));
}

// --- conjunto --------------------------------------------------------------------------------

#[test]
fn una_red_correcta_no_tiene_incumplimientos() {
    let mut p = con_agua(100.5, 25.0);
    for l in p.links.iter_mut() {
        let largo = l.length_2d();
        l.attrs.insert("longitud".into(), Value::Num(largo));
    }
    let i = validar(&p, &ReglasValidacion::default(), None);
    assert_eq!(i.resumen.len(), redes_core::validar::REGLAS.len());
    assert_eq!(i.incumplen(), 0, "{:#?}", i.hallazgos);
    assert_eq!(i.no_evaluables(), 0, "{:#?}", i.hallazgos);
}

#[test]
fn las_reglas_se_guardan_y_un_redes_viejo_abre_sin_ellas() {
    let mut p = base();
    let texto = serde_json::to_string(&p).unwrap();
    assert!(!texto.contains("reglas_validacion"));
    let mut v: serde_json::Value = serde_json::from_str(&texto).unwrap();
    v.as_object_mut().unwrap().remove("reglas_validacion");
    let viejo: Project = serde_json::from_value(v).unwrap();
    assert!(viejo.reglas_validacion.is_none());

    p.reglas_validacion = Some(ReglasValidacion {
        profundidad_max_pozo_m: 1.5,
        ..Default::default()
    });
    let vuelta: Project = serde_json::from_str(&serde_json::to_string(&p).unwrap()).unwrap();
    assert_eq!(vuelta.reglas_validacion, p.reglas_validacion);
    // Un archivo de reglas parcial completa el resto con los de referencia.
    let parcial: ReglasValidacion =
        serde_json::from_str(r#"{"profundidad_max_pozo_m": 3.0}"#).unwrap();
    assert_eq!(parcial.separacion_horizontal_min_m, 3.0);
    assert!(parcial.comprobar().is_ok());
    let malo = ReglasValidacion {
        salto_max_sin_caida_m: -1.0,
        ..Default::default()
    };
    assert!(malo.comprobar().is_err());
}

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/validar.plan.json")
}

/// El fixture pequeño dispara una vez cada regla de cotas y deja ver los no evaluables.
#[test]
fn el_fixture_de_validacion_da_los_hallazgos_esperados() {
    let mut p = Project::new("fixture", "EPSG:32717");
    redes_core::ingest::import_plan(&mut p, &fixture()).expect("importa");
    let i = validar(&p, &ReglasValidacion::default(), None);
    let por_regla = |r: &str| {
        i.resumen
            .iter()
            .find(|x| x.regla == r)
            .map(|x| (x.incumplen, x.no_evaluables))
            .unwrap()
    };
    assert_eq!(por_regla("contrapendiente"), (1, 0));
    assert_eq!(por_regla("pendiente"), (1, 0));
    assert_eq!(por_regla("salto_pozo"), (2, 0));
    assert_eq!(por_regla("recubrimiento"), (0, 1));
    assert_eq!(por_regla("profundidad_pozo"), (1, 0));
    assert_eq!(por_regla("separacion"), (1, 0));
    assert_eq!(por_regla("dn_aguas_abajo"), (1, 0));
    assert_eq!(por_regla("tramo_huerfano"), (0, 0));
    assert_eq!(por_regla("nodo_aislado"), (1, 0));
    assert_eq!(por_regla("longitud"), (1, 1));
    assert_eq!(
        objetos(&i, Severidad::Grave),
        vec![vec!["CL-2".to_string()]]
    );
    // Las ubicaciones son coordenadas del proyecto (UTM).
    assert!(i.hallazgos.iter().all(|h| {
        h.ubicacion
            .is_none_or(|u| u[0] >= 758_000.0 && u[1] >= 9_796_000.0)
    }));
}
