//! Reimportar una entrega sin perder la revisión (ING-10, F13-06, D9).
//!
//! Cada prueba escribe su plan en el momento: lo que se comprueba es qué pasa con el estado y las
//! observaciones de cada objeto cuando el diseño cambia, no la ingesta en sí.

use std::path::PathBuf;

use redes_core::diff::Opciones;
use redes_core::model::{EstadoObjeto, EstadoProyecto, Project};
use redes_core::reimportar::{Formato, preparar, reimportar};
use redes_core::revision::Observacion;

/// Entrega base: tres pozos y dos colectores con soleras declaradas.
fn plan(cota_cl1: f64, con_pz3: bool, fuente_caudal: &str) -> String {
    let pz3 = if con_pz3 {
        r#",
    { "tipo": "nodo", "codigo": "PZ-3", "familia": "saneamiento.pozo", "subtipo": "inspeccion",
      "e": 758070.0, "n": 9797014.0, "cota": 2497.2, "cota_terreno": 2499.5 },
    { "tipo": "arco", "codigo": "CL-2", "familia": "saneamiento.tuberia",
      "subtipo": "colector_sanitario", "desde": "PZ-2", "hasta": "PZ-3", "diametro_mm": 200,
      "cota_solera_inicio": 2497.9, "cota_solera_fin": 2497.2 }"#
    } else {
        r#",
    { "tipo": "nodo", "codigo": "PZ-4", "familia": "saneamiento.pozo", "subtipo": "inspeccion",
      "e": 758070.0, "n": 9797030.0, "cota": 2497.0, "cota_terreno": 2499.4 }"#
    };
    format!(
        r#"{{
  "schema": "redes.plan/1",
  "proyecto": {{ "nombre": "Reimportar", "crs": "EPSG:32717", "fuente_app": "Civil 3D" }},
  "objetos": [
    {{ "tipo": "nodo", "codigo": "PZ-1", "familia": "saneamiento.pozo", "subtipo": "inspeccion",
      "e": 758010.0, "n": 9797010.0, "cota": 2498.4, "cota_terreno": 2500.1,
      "datos": {{ "caudal": {{ "valor": 1.5, "unidad": "l/s", "fuente": "{fuente_caudal}" }} }} }},
    {{ "tipo": "nodo", "codigo": "PZ-2", "familia": "saneamiento.pozo", "subtipo": "inspeccion",
      "e": 758040.0, "n": 9797012.0, "cota": 2497.9, "cota_terreno": 2499.8 }},
    {{ "tipo": "arco", "codigo": "CL-1", "familia": "saneamiento.tuberia",
      "subtipo": "colector_sanitario", "desde": "PZ-1", "hasta": "PZ-2", "diametro_mm": 200,
      "cota_solera_inicio": {cota_cl1}, "cota_solera_fin": 2497.9 }}{pz3}
  ]
}}"#
    )
}

fn escribir(nombre: &str, texto: &str) -> PathBuf {
    let ruta = std::env::temp_dir().join(format!(
        "redes-reimportar-{nombre}-{}.json",
        std::process::id()
    ));
    std::fs::write(&ruta, texto).unwrap();
    ruta
}

/// Proyecto importado de la entrega base con revisión en marcha:
/// PZ-1 aprobado; PZ-2 observado (#1 abierta); CL-1 aprobado con una observación cerrada;
/// CL-2 aprobado; PZ-3 pendiente.
fn revisado(nombre: &str) -> Project {
    let ruta = escribir(nombre, &plan(2498.4, true, "SWMM"));
    let mut p = Project::new("Reimportar", "EPSG:32717");
    redes_core::ingest::import_plan(&mut p, &ruta).expect("plan base");
    let _ = std::fs::remove_file(&ruta);
    p.aprobar_objeto("PZ-1", None).unwrap();
    p.observar("PZ-2", "Tapa fuera de la calzada", None)
        .unwrap();
    p.observar("CL-1", "Revisar la pendiente", None).unwrap();
    p.aprobar_objeto("CL-1", Some("Corregido en origen"))
        .unwrap();
    p.aprobar_objeto("CL-2", None).unwrap();
    p
}

fn revision_de(p: &Project) -> Vec<(String, EstadoObjeto, Vec<Observacion>)> {
    let nodos = p
        .nodes
        .iter()
        .filter(|n| !n.deleted)
        .map(|n| (n.code.clone(), n.estado, n.observaciones.clone()));
    let tramos = p
        .links
        .iter()
        .filter(|l| !l.deleted)
        .map(|l| (l.code.clone(), l.estado, l.observaciones.clone()));
    let mut v: Vec<_> = nodos.chain(tramos).collect();
    v.sort_by(|a, b| a.0.cmp(&b.0));
    v
}

fn estado(p: &Project, codigo: &str) -> EstadoObjeto {
    p.node_by_code(codigo)
        .map(|n| n.estado)
        .or_else(|| p.link_by_code(codigo).map(|l| l.estado))
        .unwrap()
}

#[test]
fn reimportar_la_misma_entrega_conserva_toda_la_revision() {
    let mut p = revisado("misma");
    let antes = revision_de(&p);
    let rev = p.revision;
    let ruta = escribir("misma-b", &plan(2498.4, true, "SWMM"));
    let inf = reimportar(&mut p, &ruta, Formato::Plan, &Opciones::default(), false).unwrap();
    let _ = std::fs::remove_file(&ruta);

    assert_eq!(
        revision_de(&p),
        antes,
        "el 100 % de la revisión se conserva"
    );
    assert!(inf.altas.is_empty() && inf.bajas.is_empty() && inf.cambiados.is_empty());
    assert!(inf.reiniciados.is_empty());
    assert_eq!(inf.sin_cambios, 5);
    assert_eq!(inf.revision_conservada, 5);
    assert_eq!(inf.observaciones_conservadas, 2);
    assert_eq!(p.revision, rev + 1, "un único commit");
    assert!(
        p.journal
            .last()
            .unwrap()
            .command
            .starts_with("importar.reimportar "),
        "{:?}",
        p.journal.last()
    );
    assert_eq!(p.count(), 5);
}

#[test]
fn cambiar_la_cota_de_un_tramo_aprobado_lo_devuelve_a_pendiente_con_sus_observaciones() {
    let mut p = revisado("cota");
    let obs_cl1 = p.observaciones_de("CL-1").unwrap().to_vec();
    assert_eq!(obs_cl1.len(), 1);
    let ruta = escribir("cota-b", &plan(2498.1, true, "SWMM"));
    let inf = reimportar(&mut p, &ruta, Formato::Plan, &Opciones::default(), false).unwrap();
    let _ = std::fs::remove_file(&ruta);

    assert_eq!(estado(&p, "CL-1"), EstadoObjeto::Pendiente);
    assert_eq!(p.observaciones_de("CL-1").unwrap(), obs_cl1.as_slice());
    assert_eq!(inf.reiniciados, vec!["CL-1".to_string()]);
    let c = inf.cambiados.iter().find(|c| c.codigo == "CL-1").unwrap();
    assert_eq!(c.revision, "reiniciada");
    assert_eq!(
        (c.estado_antes, c.estado_despues),
        ("aprobado", "pendiente")
    );
    assert!(
        c.cambios.iter().any(|x| x.campo == "cota_solera_inicio"),
        "{:?}",
        c.cambios
    );
    // El resto conserva su estado.
    assert_eq!(estado(&p, "PZ-1"), EstadoObjeto::Aprobado);
    assert_eq!(estado(&p, "PZ-2"), EstadoObjeto::Observado);
    assert_eq!(estado(&p, "CL-2"), EstadoObjeto::Aprobado);
    assert!(
        p.journal.last().unwrap().command.ends_with("[CL-1]"),
        "el journal nombra lo reiniciado"
    );
}

#[test]
fn un_observado_cuyo_diseno_cambia_sigue_observado() {
    let mut p = revisado("observado");
    p.reabrir_objeto("CL-1").unwrap();
    p.observar("CL-1", "Cota de llegada dudosa", None).unwrap();
    let ruta = escribir("observado-b", &plan(2498.1, true, "SWMM"));
    let inf = reimportar(&mut p, &ruta, Formato::Plan, &Opciones::default(), false).unwrap();
    let _ = std::fs::remove_file(&ruta);
    assert_eq!(estado(&p, "CL-1"), EstadoObjeto::Observado);
    assert_eq!(p.observaciones_de("CL-1").unwrap().len(), 2);
    assert!(inf.reiniciados.is_empty());
    let c = inf.cambiados.iter().find(|c| c.codigo == "CL-1").unwrap();
    assert_eq!(c.revision, "conservada");
}

#[test]
fn las_bajas_con_revision_no_se_pierden_en_silencio() {
    let mut p = revisado("bajas");
    let rev = p.revision;
    let ruta = escribir("bajas-b", &plan(2498.4, false, "SWMM"));

    // Sin aceptar: error y el proyecto intacto.
    let err = reimportar(&mut p, &ruta, Formato::Plan, &Opciones::default(), false).unwrap_err();
    assert!(err.starts_with("E_STATE_BAJAS_CON_REVISION:"), "{err}");
    assert!(err.contains("CL-2"), "{err}");
    assert!(
        !err.contains("PZ-3"),
        "PZ-3 estaba pendiente y sin observaciones: {err}"
    );
    assert_eq!(p.revision, rev);
    assert!(p.link_by_code("CL-2").is_some());

    // El informe en seco las trae completas.
    let prep = preparar(&p, &ruta, Formato::Plan, &Opciones::default()).unwrap();
    let bajas: Vec<&str> = prep
        .informe
        .bajas
        .iter()
        .map(|b| b.codigo.as_str())
        .collect();
    assert_eq!(bajas, vec!["PZ-3", "CL-2"]);
    let cl2 = &prep.informe.bajas[1];
    assert_eq!(cl2.estado, "aprobado");
    assert!(cl2.con_revision() && !prep.informe.bajas[0].con_revision());

    // Aceptadas: entran las altas pendientes y salen las bajas.
    let inf = reimportar(&mut p, &ruta, Formato::Plan, &Opciones::default(), true).unwrap();
    let _ = std::fs::remove_file(&ruta);
    assert_eq!(inf.altas.len(), 1);
    assert_eq!(inf.altas[0].codigo, "PZ-4");
    assert_eq!(estado(&p, "PZ-4"), EstadoObjeto::Pendiente);
    assert!(p.link_by_code("CL-2").is_none() && p.node_by_code("PZ-3").is_none());
    assert_eq!(estado(&p, "PZ-1"), EstadoObjeto::Aprobado);
}

#[test]
fn un_cambio_solo_de_procedencia_no_reinicia_la_aprobacion() {
    let mut p = revisado("fuente");
    let ruta = escribir("fuente-b", &plan(2498.4, true, "SWMM v2"));
    let inf = reimportar(&mut p, &ruta, Formato::Plan, &Opciones::default(), false).unwrap();
    let _ = std::fs::remove_file(&ruta);
    assert_eq!(estado(&p, "PZ-1"), EstadoObjeto::Aprobado);
    let c = inf.cambiados.iter().find(|c| c.codigo == "PZ-1").unwrap();
    assert!(c.solo_procedencia, "{:?}", c.cambios);
    assert!(inf.reiniciados.is_empty());
    assert!(!inf.cambia_el_diseno());
}

#[test]
fn un_proyecto_aprobado_vuelve_a_revision_si_cambia_el_diseno() {
    let mut p = revisado("proyecto");
    p.estado = EstadoProyecto::Aprobado;
    let misma = escribir("proyecto-a", &plan(2498.4, true, "SWMM"));
    reimportar(&mut p, &misma, Formato::Plan, &Opciones::default(), false).unwrap();
    assert_eq!(
        p.estado,
        EstadoProyecto::Aprobado,
        "sin cambios sigue aprobado"
    );
    let otra = escribir("proyecto-b", &plan(2498.1, true, "SWMM"));
    let inf = reimportar(&mut p, &otra, Formato::Plan, &Opciones::default(), false).unwrap();
    let _ = std::fs::remove_file(&misma);
    let _ = std::fs::remove_file(&otra);
    assert_eq!(p.estado, EstadoProyecto::EnRevision);
    assert_eq!(
        (inf.estado_proyecto_antes, inf.estado_proyecto_despues),
        ("aprobado", "en_revision")
    );
}

#[test]
fn la_vuelta_del_dxf_conserva_la_revision() {
    let mut p = revisado("dxf");
    let antes = revision_de(&p);
    let dxf = std::env::temp_dir().join(format!("redes-reimportar-{}.dxf", std::process::id()));
    redes_core::export::export_dxf(&p, &dxf).expect("dxf");
    let inf = reimportar(
        &mut p,
        &dxf,
        Formato::por_extension(&dxf),
        &Opciones::default(),
        false,
    )
    .unwrap();
    let _ = std::fs::remove_file(&dxf);
    assert_eq!(inf.formato, "dxf");
    assert!(inf.reiniciados.is_empty(), "{:?}", inf.cambiados);
    assert_eq!(revision_de(&p), antes);
}

#[test]
fn una_entrega_ilegible_no_toca_el_proyecto() {
    let mut p = revisado("ilegible");
    let antes = revision_de(&p);
    let rev = p.revision;
    let ruta = escribir("ilegible-b", "{ no es json");
    let err = reimportar(&mut p, &ruta, Formato::Plan, &Opciones::default(), true).unwrap_err();
    let _ = std::fs::remove_file(&ruta);
    assert!(err.starts_with("E_FMT_PARSE"), "{err}");
    assert_eq!(p.revision, rev);
    assert_eq!(revision_de(&p), antes);
}
