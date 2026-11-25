//! Acta de revisión (F13-04): lista estados y observaciones sin inventar nada.

use redes_core::acta::{AUSENTE, Acta};
use redes_core::demo;
use redes_core::geom::P3;
use redes_core::model::{EstadoObjeto, Project};

/// Demo con un pozo observado dos veces (una cerrada), un tramo observado y un pozo aprobado.
fn revisado() -> (Project, String, String, String) {
    let mut p = demo::project();
    let mut nodos: Vec<String> = p
        .nodes
        .iter()
        .filter(|n| !n.deleted)
        .map(|n| n.code.clone())
        .collect();
    nodos.sort();
    let tramo = p.links.iter().find(|l| !l.deleted).unwrap().code.clone();
    let (a, b) = (nodos[0].clone(), nodos[1].clone());
    p.observar(&a, "Tapa desplazada; revisar <cota> & \"rasante\"", None)
        .unwrap();
    p.observar(
        &a,
        "Falta escalón",
        Some(P3::new(758010.5, 9797010.0, 2498.4)),
    )
    .unwrap();
    p.cerrar_observacion(&a, 1, Some("Corregido en Civil 3D"))
        .unwrap();
    p.observar(&tramo, "=HYPERLINK(\"x\") pendiente", None)
        .unwrap();
    p.aprobar_objeto(&b, None).unwrap();
    (p, a, b, tramo)
}

#[test]
fn el_resumen_cuenta_estados_y_observaciones() {
    let (p, a, b, tramo) = revisado();
    let acta = Acta::de(&p, Some("2026-11-25T10:00:00Z".into()));
    let r = acta.resumen;
    let total = p.nodes.iter().filter(|n| !n.deleted).count()
        + p.links.iter().filter(|l| !l.deleted).count();
    assert_eq!(r.objetos, total);
    let (pnd, apr, obs) = p.revision_counts();
    assert_eq!((r.pendientes, r.aprobados, r.observados), (pnd, apr, obs));
    assert_eq!(r.observados, 2);
    assert_eq!(r.pendientes + r.aprobados + r.observados, total);
    assert_eq!((r.observaciones, r.abiertas, r.cerradas), (3, 2, 1));
    assert_eq!(acta.revision, p.revision);
    // Nodos primero, por código; luego tramos.
    assert_eq!(acta.objetos[0].codigo, a);
    assert_eq!(acta.objetos[1].codigo, b);
    assert_eq!(acta.objetos[1].estado, EstadoObjeto::Aprobado);
    let t = acta.objetos.iter().find(|o| o.codigo == tramo).unwrap();
    assert_eq!(t.elemento, "tramo");
    assert!(!acta.con_autor(), "D2: sin autor si no se registró");
}

#[test]
fn el_csv_abre_en_excel_con_tildes_y_una_fila_por_observacion() {
    let (p, a, b, tramo) = revisado();
    let csv = Acta::de(&p, Some("2026-11-25T10:00:00Z".into())).a_csv();
    assert!(csv.starts_with('\u{feff}'), "BOM para Excel");
    assert!(csv.contains("Acta de revisión;"));
    assert!(csv.contains("Emitida (UTC);2026-11-25T10:00:00Z"));
    let cab = csv
        .lines()
        .find(|l| l.starts_with("codigo;"))
        .expect("cabecera de la tabla");
    assert!(!cab.contains("autor"), "D2: sin columnas de autor");
    let ncol = cab.split(';').count();
    let filas_a: Vec<&str> = csv
        .lines()
        .filter(|l| l.starts_with(&format!("{a};")))
        .collect();
    assert_eq!(filas_a.len(), 2, "una fila por observación");
    // El `;` del texto va entre comillas: la fila no gana columnas.
    assert!(filas_a[0].contains(";#1;cerrada;"));
    assert!(filas_a[0].contains("\"Tapa desplazada; revisar <cota> & \"\"rasante\"\"\""));
    assert!(filas_a[0].contains("Corregido en Civil 3D"));
    assert!(filas_a[1].contains(";#2;abierta;"));
    assert!(filas_a[1].contains("Falta escalón"));
    assert!(filas_a[1].contains("758010.500, 9797010.000, 2498.400"));
    // Lo ausente sale «—»: la #2 no tiene cierre.
    assert!(filas_a[1].ends_with(&format!(";{AUSENTE};{AUSENTE};{AUSENTE}")));
    // Un objeto sin observaciones tiene una fila con su estado.
    let fila_b = csv
        .lines()
        .find(|l| l.starts_with(&format!("{b};")))
        .unwrap();
    assert!(fila_b.contains(";aprobado;"));
    assert_eq!(fila_b.split(';').count(), ncol);
    // Excel no ejecuta como fórmula el texto del usuario.
    let fila_t = csv
        .lines()
        .find(|l| l.starts_with(&format!("{tramo};")))
        .unwrap();
    assert!(fila_t.contains(";\"'=HYPERLINK(\"\"x\"\") pendiente\";"));
}

#[test]
fn el_html_es_autocontenido_imprimible_y_escapa_el_texto() {
    let (p, a, _, _) = revisado();
    let html = Acta::de(&p, None).a_html();
    assert!(html.starts_with("<!doctype html>"));
    assert!(html.contains("<meta charset=\"utf-8\">"));
    assert!(html.contains("@media print"));
    assert!(!html.contains("<script"), "sin JS");
    assert!(!html.contains("http"), "sin recursos externos");
    assert!(html.contains(&a));
    assert!(html.contains("Falta escalón"));
    assert!(
        html.contains("revisar &lt;cota&gt; &amp; &quot;rasante&quot;"),
        "texto del usuario escapado"
    );
    assert!(!html.contains("<cota>"));
    assert!(html.contains(&format!("<dt>Emitida (UTC)</dt><dd>{AUSENTE}</dd>")));
    assert!(html.contains("Estado por objeto"));
}

#[test]
fn el_autor_solo_aparece_si_esta_en_los_datos() {
    let (mut p, a, _, _) = revisado();
    let n = p.nodes.iter_mut().find(|n| n.code == a).unwrap();
    n.observaciones[1].autor = Some("ingeniera".into());
    let acta = Acta::de(&p, None);
    assert!(acta.con_autor());
    let csv = acta.a_csv();
    let cab = csv.lines().find(|l| l.starts_with("codigo;")).unwrap();
    assert!(cab.ends_with(";autor;autor_cierre"));
    assert!(csv.contains(&format!(";ingeniera;{AUSENTE}")));
    assert!(acta.a_html().contains("<th>Autor</th>"));
}

#[test]
fn un_proyecto_sin_observaciones_no_inventa_ninguna() {
    let p = demo::project();
    let acta = Acta::de(&p, None);
    assert_eq!(acta.resumen.observaciones, 0);
    assert_eq!(acta.resumen.observados, 0);
    assert_eq!(
        acta.resumen.pendientes + acta.resumen.aprobados,
        acta.resumen.objetos
    );
    assert!(acta.a_html().contains("Sin observaciones registradas"));
}
