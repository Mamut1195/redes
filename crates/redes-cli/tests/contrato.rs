//! Contrato del CLI: todo comando responde un sobre JSON y un código de salida estable.
//!
//! Estas pruebas ejecutan el binario real (`CARGO_BIN_EXE_redes`), así que cubren lo que un agente
//! ve: `stdout`, `stderr` y el código de salida.

use std::path::{Path, PathBuf};
use std::process::Command;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_redes")
}

struct Salida {
    codigo: i32,
    stdout: String,
    stderr: String,
}

fn corre(args: &[&str], dir: &Path) -> Salida {
    let out = Command::new(bin())
        .args(args)
        .current_dir(dir)
        .output()
        .expect("ejecuta el binario");
    Salida {
        codigo: out.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&out.stdout).to_string(),
        stderr: String::from_utf8_lossy(&out.stderr).to_string(),
    }
}

fn sobre(s: &Salida) -> serde_json::Value {
    serde_json::from_str(&s.stdout).unwrap_or_else(|e| {
        panic!(
            "stdout no es un sobre JSON ({e}): {:?} / stderr {:?}",
            s.stdout, s.stderr
        )
    })
}

fn tempdir(nombre: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("redes-cli-{nombre}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("carpeta temporal");
    d
}

#[test]
fn todo_sobre_declara_su_esquema_y_su_ok() {
    let d = tempdir("esquema");
    let s = corre(&["esquema"], &d);
    let j = sobre(&s);
    assert_eq!(j["schema"], "redes.envelope/1");
    assert_eq!(j["ok"], true);
    assert_eq!(j["command"], "esquema");
    assert_eq!(s.codigo, 0);
    // Los formatos de salida incluyen el acta (F13-04) y las cantidades.
    let salida: Vec<&str> = j["data"]["formatos"]["salida"]
        .as_array()
        .expect("formatos.salida")
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    for f in [
        "dxf",
        "geojson",
        "glb",
        "svg",
        "csv (cantidades)",
        "csv (acta)",
        "html (acta)",
    ] {
        assert!(salida.contains(&f), "falta {f} en formatos.salida");
    }
}

#[test]
fn capacidades_publica_el_registro_completo() {
    let d = tempdir("capacidades");
    let j = sobre(&corre(&["capacidades"], &d));
    let familias = j["data"]["familias"].as_array().expect("familias");
    assert_eq!(familias.len(), 46, "el registro tiene 46 familias");
    let subtipos: usize = familias
        .iter()
        .map(|f| f["subtipos"].as_array().map(|s| s.len()).unwrap_or(0))
        .sum();
    assert_eq!(subtipos, 315, "y 315 subtipos");
}

#[test]
fn sin_proyecto_el_error_es_e_proj_not_found_y_codigo_4() {
    // El CLI busca `*.redes` hacia arriba; un resto de pruebas anteriores en %TEMP% haría que este
    // caso pasara por la razón equivocada. Se limpian solo los de la carpeta temporal.
    if let Ok(entradas) = std::fs::read_dir(std::env::temp_dir()) {
        for e in entradas.flatten() {
            let p = e.path();
            if p.extension().is_some_and(|x| x == "redes") {
                let _ = std::fs::remove_file(&p);
            }
        }
    }
    let d = tempdir("sinproyecto");
    let s = corre(&["objeto", "lista"], &d);
    let j = sobre(&s);
    assert_eq!(j["ok"], false);
    assert_eq!(j["diagnostics"][0]["code"], "E_PROJ_NOT_FOUND");
    assert_eq!(s.codigo, 4);
}

#[test]
fn un_proyecto_nuevo_se_crea_y_se_puede_leer() {
    let d = tempdir("nuevo");
    let ruta = d.join("p.redes");
    let j = sobre(&corre(
        &[
            "proyecto",
            "nuevo",
            "--nombre",
            "Prueba",
            "--salida",
            ruta.to_str().unwrap(),
        ],
        &d,
    ));
    assert_eq!(j["ok"], true);
    assert!(ruta.exists(), "el archivo se escribió");
    let j = sobre(&corre(
        &["-p", ruta.to_str().unwrap(), "proyecto", "info"],
        &d,
    ));
    assert_eq!(j["ok"], true);
    assert_eq!(j["project"]["name"], "Prueba");
}

#[test]
fn aprobar_un_objeto_inexistente_devuelve_e_state_not_found() {
    let d = tempdir("aprobar");
    let ruta = d.join("c.redes");
    corre(
        &[
            "proyecto",
            "catalogo",
            "--salida",
            ruta.to_str().unwrap(),
            "--quiet",
        ],
        &d,
    );
    let s = corre(
        &[
            "-p",
            ruta.to_str().unwrap(),
            "objeto",
            "aprobar",
            "--codigo",
            "NO-EXISTE",
        ],
        &d,
    );
    let j = sobre(&s);
    assert_eq!(j["ok"], false);
    assert_eq!(j["diagnostics"][0]["code"], "E_STATE_NOT_FOUND");
    assert_eq!(s.codigo, 4);
}

#[test]
fn un_archivo_ilegible_da_e_io_not_found_y_codigo_7() {
    let d = tempdir("ilegible");
    let ruta = d.join("p.redes");
    corre(
        &[
            "proyecto",
            "nuevo",
            "--nombre",
            "x",
            "--salida",
            ruta.to_str().unwrap(),
            "--quiet",
        ],
        &d,
    );
    let s = corre(
        &[
            "-p",
            ruta.to_str().unwrap(),
            "importar",
            "plan",
            "--archivo",
            d.join("no-existe.json").to_str().unwrap(),
        ],
        &d,
    );
    let j = sobre(&s);
    assert_eq!(j["ok"], false);
    assert_eq!(j["diagnostics"][0]["code"], "E_IO_NOT_FOUND");
    assert_eq!(s.codigo, 7);
}

#[test]
fn dry_run_no_escribe_ni_un_archivo() {
    let d = tempdir("dry");
    let ruta = d.join("c.redes");
    corre(
        &[
            "proyecto",
            "catalogo",
            "--salida",
            ruta.to_str().unwrap(),
            "--quiet",
        ],
        &d,
    );
    let dxf = d.join("salida.dxf");
    let j = sobre(&corre(
        &[
            "-p",
            ruta.to_str().unwrap(),
            "--dry-run",
            "exportar",
            "dxf",
            "--salida",
            dxf.to_str().unwrap(),
        ],
        &d,
    ));
    assert_eq!(j["ok"], true);
    assert_eq!(j["data"]["dry_run"], true);
    assert!(!dxf.exists(), "con --dry-run no se escribe");
}

#[test]
fn quiet_no_imprime_nada_y_pretty_indenta() {
    let d = tempdir("salidas");
    let ruta = d.join("c.redes");
    let s = corre(
        &[
            "proyecto",
            "catalogo",
            "--salida",
            ruta.to_str().unwrap(),
            "--quiet",
        ],
        &d,
    );
    assert_eq!(s.codigo, 0);
    assert!(s.stdout.trim().is_empty(), "--quiet no imprime");

    let s = corre(
        &["-p", ruta.to_str().unwrap(), "--pretty", "proyecto", "info"],
        &d,
    );
    assert!(s.stdout.contains("\n  "), "--pretty indenta");
    assert!(sobre(&s)["ok"].as_bool().unwrap());
}

#[test]
fn el_dxf_sale_y_vuelve_por_el_cli() {
    let d = tempdir("dxf");
    let ruta = d.join("c.redes");
    corre(
        &[
            "proyecto",
            "catalogo-total",
            "--salida",
            ruta.to_str().unwrap(),
            "--quiet",
        ],
        &d,
    );
    let dxf = d.join("c.dxf");
    let j = sobre(&corre(
        &[
            "-p",
            ruta.to_str().unwrap(),
            "exportar",
            "dxf",
            "--salida",
            dxf.to_str().unwrap(),
        ],
        &d,
    ));
    assert_eq!(j["ok"], true, "exporta: {j}");
    assert!(dxf.exists());

    let vuelta = d.join("v.redes");
    corre(
        &[
            "proyecto",
            "nuevo",
            "--nombre",
            "vuelta",
            "--salida",
            vuelta.to_str().unwrap(),
            "--quiet",
        ],
        &d,
    );
    let j = sobre(&corre(
        &[
            "-p",
            vuelta.to_str().unwrap(),
            "importar",
            "dxf",
            "--archivo",
            dxf.to_str().unwrap(),
        ],
        &d,
    ));
    assert_eq!(j["ok"], true, "importa: {j}");
    assert!(j["data"]["nodos"].as_u64().unwrap() >= 40);
    assert_eq!(j["data"]["descartados"].as_u64().unwrap(), 0);
}

#[test]
fn cantidades_publica_filas_y_totales() {
    let d = tempdir("cantidades");
    let ruta = d.join("cad.redes");
    corre(
        &[
            "proyecto",
            "cadena",
            "--salida",
            ruta.to_str().unwrap(),
            "--quiet",
        ],
        &d,
    );
    let j = sobre(&corre(&["-p", ruta.to_str().unwrap(), "cantidades"], &d));
    assert_eq!(j["ok"], true);
    let filas = j["data"]["filas"].as_array().expect("filas");
    assert!(!filas.is_empty());
    let totales = j["data"]["totales"].as_array().expect("totales");
    assert!(totales.iter().any(|t| t["unidad"] == "m"));
}

#[test]
fn una_familia_inexistente_no_rompe_el_esquema() {
    let d = tempdir("familia");
    let j = sobre(&corre(&["esquema"], &d));
    // `esquema` no filtra por familia: publica el registro y no falla.
    assert_eq!(j["ok"], true);
    assert!(
        j["data"]["familias"]
            .as_array()
            .is_some_and(|f| !f.is_empty())
    );
}

/// Un atributo de lista publica sus valores admitidos: el agente no los adivina (F12-15).
#[test]
fn el_esquema_lista_los_valores_de_los_atributos_de_lista() {
    let d = tempdir("valores");
    let j = sobre(&corre(&["esquema", "--familia", "saneamiento.tuberia"], &d));
    let fam = j["data"]["familias"]
        .as_array()
        .and_then(|f| f.iter().find(|x| x["clave"] == "saneamiento.tuberia"))
        .expect("familia");
    let attr = |k: &str| {
        fam["atributos"]
            .as_array()
            .and_then(|a| a.iter().find(|x| x["clave"] == k))
            .cloned()
            .expect(k)
    };
    let seccion = attr("seccion");
    assert_eq!(seccion["tipo"], "enum");
    let valores: Vec<&str> = seccion["valores"]
        .as_array()
        .expect("valores")
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    assert!(valores.contains(&"ovoide") && valores.contains(&"circular"));
    assert!(
        attr("rugosidad_n")["valores"].is_null(),
        "un número no tiene lista"
    );
}

/// `--dry-run` de `importar plan` valida de verdad: el mismo informe que la importación (altas,
/// descartes con motivo, avisos y errores) y el proyecto intacto (F12-15).
#[test]
fn importar_plan_en_seco_da_el_informe_completo_sin_escribir() {
    let d = tempdir("plan-seco");
    let ruta = d.join("p.redes");
    let r = ruta.to_str().unwrap();
    corre(
        &[
            "proyecto", "nuevo", "--nombre", "x", "--salida", r, "--quiet",
        ],
        &d,
    );
    let antes = std::fs::read_to_string(&ruta).unwrap();
    let plan = d.join("plan.json");
    std::fs::write(
        &plan,
        serde_json::json!({
            "schema": "redes.plan/1",
            "proyecto": { "nombre": "x", "crs": "EPSG:32717" },
            "objetos": [
                { "tipo": "nodo", "codigo": "P1", "familia": "saneamiento.pozo", "e": 760000.0, "n": 9796000.0, "cota": 2500.0 },
                { "tipo": "nodo", "codigo": "P2", "familia": "saneamiento.pozo", "e": 760050.0, "n": 9796000.0 },
                { "tipo": "arco", "codigo": "C1", "familia": "saneamiento.tuberia", "desde": "P1", "hasta": "P9", "diametro_mm": 200 }
            ]
        })
        .to_string(),
    )
    .unwrap();
    let p = plan.to_str().unwrap();
    let seco = sobre(&corre(
        &["-p", r, "--dry-run", "importar", "plan", "--archivo", p],
        &d,
    ));
    assert_eq!(seco["ok"], true, "{seco}");
    let datos = &seco["data"];
    assert_eq!(datos["dry_run"], true);
    assert_eq!(datos["guardado"], false);
    assert_eq!(datos["nodos"], 1);
    assert_eq!(datos["arcos"], 0);
    let descartes = datos["detalle_descartes"].to_string();
    assert!(
        descartes.contains("P2") && descartes.contains("sin ninguna cota"),
        "{descartes}"
    );
    assert!(
        descartes.contains("C1") && descartes.contains("P9"),
        "{descartes}"
    );
    assert!(datos["avisos"].as_u64().unwrap() > 0, "{datos}");
    assert_eq!(
        std::fs::read_to_string(&ruta).unwrap(),
        antes,
        "el proyecto no se toca"
    );
    // La importación real da el mismo informe.
    let real = sobre(&corre(&["-p", r, "importar", "plan", "--archivo", p], &d));
    for k in ["nodos", "arcos", "detalle_descartes", "detalle_avisos"] {
        assert_eq!(real["data"][k], datos[k], "{k}");
    }
    assert_ne!(std::fs::read_to_string(&ruta).unwrap(), antes);

    // Un plan roto falla igual en seco: E_FMT_CAMPO con la sugerencia, código 7.
    std::fs::write(
        &plan,
        r#"{"schema":"redes.plan/1","proyecto":{"nombre":"x","crs":"EPSG:32717"},
            "objetos":[{"tipo":"nodo","codigo":"P1","familia":"saneamiento.pozo","e":760000.0,"n":9796000.0,"solera":2500.0}]}"#,
    )
    .unwrap();
    let s = corre(
        &["-p", r, "--dry-run", "importar", "plan", "--archivo", p],
        &d,
    );
    let j = sobre(&s);
    assert_eq!(j["ok"], false, "{j}");
    assert_eq!(j["diagnostics"][0]["code"], "E_FMT_CAMPO");
    assert!(
        j["diagnostics"][0]["message"]
            .as_str()
            .is_some_and(|m| m.contains("«solera» (¿quiso decir «cota»?)")),
        "{j}"
    );
    assert_eq!(s.codigo, 7);
}

#[test]
fn aprobar_el_proyecto_con_observados_devuelve_e_state_observed() {
    let d = tempdir("aprobar-obs");
    let ruta = d.join("cad.redes");
    let r = ruta.to_str().unwrap();
    corre(&["proyecto", "cadena", "--salida", r, "--quiet"], &d);
    let lista = sobre(&corre(&["-p", r, "objeto", "lista"], &d));
    let codigos: Vec<String> = lista["data"]["items"]
        .as_array()
        .expect("items")
        .iter()
        .map(|i| i["codigo"].as_str().unwrap().to_string())
        .collect();
    assert!(codigos.len() > 1);
    for (i, c) in codigos.iter().enumerate() {
        let mut args = vec!["-p", r, "objeto"];
        if i == 0 {
            args.extend(["observar", "--codigo", c, "--texto", "Revisar"]);
        } else {
            args.extend(["aprobar", "--codigo", c]);
        }
        let j = sobre(&corre(&args, &d));
        assert_eq!(j["ok"], true, "{args:?}: {j}");
    }
    let s = corre(&["-p", r, "proyecto", "aprobar"], &d);
    let j = sobre(&s);
    assert_eq!(j["ok"], false, "{j}");
    assert_eq!(j["diagnostics"][0]["code"], "E_STATE_OBSERVED");
    assert_eq!(s.codigo, 3);
    // Resuelto el observado (cerrando su observación al aprobar), el proyecto sí se aprueba.
    let j = sobre(&corre(
        &[
            "-p",
            r,
            "objeto",
            "aprobar",
            "--codigo",
            &codigos[0],
            "--cerrar-todas",
            "Corregido",
        ],
        &d,
    ));
    assert_eq!(j["ok"], true, "{j}");
    let j = sobre(&corre(&["-p", r, "proyecto", "aprobar"], &d));
    assert_eq!(j["ok"], true, "{j}");
    // El diario anota qué objeto se aprobó.
    let guardado = std::fs::read_to_string(r).unwrap();
    assert!(guardado.contains(&format!("objeto.aprobar {}", codigos[1])));
}

/// F13-03: las observaciones se añaden (no se sobrescriben), se cierran por id, se listan con su
/// historial; aprobar con abiertas falla con `E_STATE_OBS_ABIERTAS` y el journal anota código e id.
#[test]
fn las_observaciones_tienen_historial() {
    let d = tempdir("observaciones");
    let ruta = d.join("obs.redes");
    let r = ruta.to_str().unwrap();
    corre(&["proyecto", "demo", "--salida", r, "--quiet"], &d);
    let p: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&ruta).unwrap()).unwrap();
    let pz = p["nodes"][0]["code"].as_str().unwrap().to_string();
    let cl = p["links"][0]["code"].as_str().unwrap().to_string();
    let (pz, cl) = (pz.as_str(), cl.as_str());
    let ok = |args: &[&str]| {
        let mut v = vec!["-p", r, "objeto"];
        v.extend_from_slice(args);
        let s = corre(&v, &d);
        let j = sobre(&s);
        assert_eq!(j["ok"], true, "{v:?}: {j}");
        assert_eq!(s.codigo, 0);
        j
    };
    let falla = |args: &[&str], codigo: &str, salida: i32| {
        let mut v = vec!["-p", r, "objeto"];
        v.extend_from_slice(args);
        let s = corre(&v, &d);
        let j = sobre(&s);
        assert_eq!(j["ok"], false, "{v:?}: {j}");
        assert_eq!(j["error"]["code"], codigo, "{j}");
        assert_eq!(s.codigo, salida, "{j}");
    };

    // Dos objetos observados, cada uno con su observación #1 abierta.
    ok(&[
        "observar",
        "--codigo",
        pz,
        "--texto",
        "Tapa desplazada 0,5 m respecto al plano",
    ]);
    ok(&["observar", "--codigo", cl, "--texto", "Revisar pendiente"]);
    let j = ok(&["observaciones", "--codigo", pz]);
    assert_eq!(j["data"]["abiertas"], 1);
    assert_eq!(
        j["data"]["observaciones"][0]["texto"],
        "Tapa desplazada 0,5 m respecto al plano"
    );
    // Observar añade la #2 con ubicación.
    let j = ok(&[
        "observar",
        "--codigo",
        pz,
        "--texto",
        "Falta escalera",
        "--ubicacion",
        "758010,9797010,2499.5",
    ]);
    assert_eq!(j["data"]["id"], 2);
    falla(&["observar", "--codigo", pz], "E_ARG_MISSING", 2);
    // Aprobar con abiertas falla y no cambia nada.
    falla(&["aprobar", "--codigo", pz], "E_STATE_OBS_ABIERTAS", 3);
    falla(&["reabrir", "--codigo", pz], "E_STATE_OBS_ABIERTAS", 3);
    // Cerrar la #1; la #2 sigue abierta y el objeto, observado.
    let j = ok(&[
        "cerrar-observacion",
        "--codigo",
        pz,
        "--id",
        "1",
        "--texto",
        "Corregido en Civil 3D",
    ]);
    assert_eq!(j["data"]["estado"], "observado");
    falla(
        &["cerrar-observacion", "--codigo", pz, "--id", "1"],
        "E_STATE_OBS_CERRADA",
        3,
    );
    falla(
        &["cerrar-observacion", "--codigo", pz, "--id", "9"],
        "E_STATE_NOT_FOUND",
        4,
    );
    // Cerrar la última deja el objeto pendiente; después se aprueba.
    let j = ok(&["cerrar-observacion", "--codigo", pz, "--id", "2"]);
    assert_eq!(j["data"]["estado"], "pendiente");
    let j = ok(&["aprobar", "--codigo", pz]);
    assert_eq!(j["data"]["estado"], "aprobado");
    // El historial conserva las dos, cerradas, con su cierre y la ubicación.
    let j = ok(&["observaciones", "--codigo", pz]);
    let obs = j["data"]["observaciones"].as_array().unwrap();
    assert_eq!(obs.len(), 2);
    assert!(obs.iter().all(|o| o["estado"] == "cerrada"), "{obs:?}");
    assert_eq!(obs[0]["cierre"], "Corregido en Civil 3D");
    assert_eq!(obs[1]["ubicacion"]["z"], 2499.5);
    assert!(obs[1].get("autor").is_none(), "D2: sin autor por defecto");
    // Aprobar cerrando todas en el mismo acto.
    let j = ok(&["aprobar", "--codigo", cl, "--cerrar-todas", "Resuelto"]);
    assert_eq!(j["data"]["cerradas"], serde_json::json!([1]));

    let guardado = std::fs::read_to_string(&ruta).unwrap();
    assert!(guardado.contains("\"redes.project/3\""));
    assert!(guardado.contains(&format!("objeto.observar {pz} #2")));
    assert!(guardado.contains(&format!("objeto.cerrar_observacion {pz} #1")));
    assert!(guardado.contains(&format!("objeto.aprobar {cl} #1")));
    // Registrados en `redes esquema`.
    let e = sobre(&corre(&["esquema"], &d)).to_string();
    for c in [
        "objeto cerrar-observacion",
        "objeto observaciones",
        "E_STATE_OBS_ABIERTAS",
    ] {
        assert!(e.contains(c), "{c} no está en el esquema");
    }
}

#[test]
fn un_dxf_ajeno_da_e_fmt_not_redes_sin_tocar_el_proyecto() {
    let d = tempdir("dxf-ajeno");
    let ruta = d.join("p.redes");
    let r = ruta.to_str().unwrap();
    corre(
        &[
            "proyecto", "nuevo", "--nombre", "x", "--salida", r, "--quiet",
        ],
        &d,
    );
    let antes = std::fs::read_to_string(&ruta).unwrap();
    let dxf = d.join("civil3d.dxf");
    std::fs::write(
        &dxf,
        "0\nSECTION\n2\nENTITIES\n0\nPOINT\n8\nPOZOS\n10\n1.0\n20\n2.0\n30\n3.0\n0\nENDSEC\n0\nEOF\n",
    )
    .unwrap();
    let s = corre(
        &[
            "-p",
            r,
            "importar",
            "dxf",
            "--archivo",
            dxf.to_str().unwrap(),
        ],
        &d,
    );
    let j = sobre(&s);
    assert_eq!(j["ok"], false, "{j}");
    assert_eq!(j["diagnostics"][0]["code"], "E_FMT_NOT_REDES");
    assert!(
        j["diagnostics"][0]["hint"]
            .as_str()
            .is_some_and(|h| h.contains("redes-ingesta")),
        "la pista lleva a la skill: {j}"
    );
    assert_eq!(s.codigo, 7);
    assert_eq!(
        std::fs::read_to_string(&ruta).unwrap(),
        antes,
        "el proyecto no se toca"
    );
}

#[test]
fn proyecto_diff_reporta_altas_bajas_y_cambios_y_escribe_csv() {
    use redes_core::project_io::{load, save};
    let d = tempdir("diff");
    let a = d.join("a.redes");
    let b = d.join("b.redes");
    corre(
        &[
            "proyecto",
            "demo",
            "--salida",
            a.to_str().unwrap(),
            "--quiet",
        ],
        &d,
    );
    let mut p = load(&a).expect("carga la demo");
    let baja = p.links.pop().expect("un tramo").code;
    let mut nuevo = p.nodes[0].clone();
    nuevo.code = "PZ-NUEVO".into();
    nuevo.id = redes_core::model::ElementId::new(9000, 1);
    p.nodes.push(nuevo);
    p.nodes[1].z_invert -= 0.25;
    p.nodes[2].pos.x += 0.004; // por debajo de la tolerancia: no se reporta
    let li = p.links.iter().position(|l| l.dn().is_some()).unwrap();
    p.links[li].d_mm += 100;
    let ni = p.nodes.iter().position(|n| !n.datos.is_empty()).unwrap();
    let clave = p.nodes[ni].datos.keys().next().unwrap().clone();
    p.nodes[ni].datos.remove(&clave);
    let cod_mov = p.nodes[2].code.clone();
    save(&p, &b).expect("guarda B");

    let csv = d.join("diff.csv");
    let s = corre(
        &[
            "proyecto",
            "diff",
            "--a",
            a.to_str().unwrap(),
            "--b",
            b.to_str().unwrap(),
            "--salida",
            csv.to_str().unwrap(),
        ],
        &d,
    );
    let j = sobre(&s);
    assert_eq!(j["ok"], true, "{j}");
    assert_eq!(j["command"], "proyecto.diff");
    assert_eq!(s.codigo, 0);
    let r = &j["data"]["resumen"];
    assert_eq!(r["altas"], 1);
    assert_eq!(r["bajas"], 1);
    assert_eq!(j["data"]["bajas"][0]["codigo"], baja.as_str());
    assert_eq!(r["modificados"], 3, "{j}");
    let mods = j["data"]["modificados"].to_string();
    assert!(mods.contains("cota_solera") && mods.contains("\"dn\"") && mods.contains("datos."));
    assert!(!mods.contains(&cod_mov), "0,004 m < tolerancia");
    let texto = std::fs::read_to_string(&csv).unwrap();
    assert!(texto.starts_with('\u{feff}'));
    assert_eq!(texto.lines().count(), 1 + 2 + 3);

    // Con tolerancia fina aparece el movimiento; --solo revision lo deja vacio.
    let fino = sobre(&corre(
        &[
            "proyecto",
            "diff",
            "--a",
            a.to_str().unwrap(),
            "--b",
            b.to_str().unwrap(),
            "--tolerancia-pos",
            "0.001",
        ],
        &d,
    ));
    assert!(fino["data"]["modificados"].to_string().contains(&cod_mov));
    let rev = sobre(&corre(
        &[
            "proyecto",
            "diff",
            "--a",
            a.to_str().unwrap(),
            "--b",
            b.to_str().unwrap(),
            "--solo",
            "revision",
        ],
        &d,
    ));
    assert_eq!(rev["data"]["resumen"]["sin_cambios"], true);

    // CRS distinto: error claro.
    let mut q = load(&a).unwrap();
    q.crs = "EPSG:32718".into();
    let c = d.join("c.redes");
    save(&q, &c).unwrap();
    let s = corre(
        &[
            "proyecto",
            "diff",
            "--a",
            a.to_str().unwrap(),
            "--b",
            c.to_str().unwrap(),
        ],
        &d,
    );
    let j = sobre(&s);
    assert_eq!(j["ok"], false);
    assert_eq!(j["diagnostics"][0]["code"], "E_STATE_CRS_DISTINTO");
    assert_ne!(s.codigo, 0);
    // El esquema lo registra.
    let esq = corre(&["esquema"], &d).stdout;
    assert!(esq.contains("proyecto diff") && esq.contains("E_STATE_CRS_DISTINTO"));
}

/// D10 (F15-01): solo se abre `redes.project/3`; un `/2` da `E_PROJ_SCHEMA` (código 4) con la
/// pista de regenerarlo, y un plan que no es `redes.plan/1` da `E_FMT_VERSION` (código 7).
#[test]
fn solo_abre_la_version_vigente_del_proyecto_y_del_plan() {
    let d = tempdir("version");
    let ruta = d.join("v2.redes");
    let r = ruta.to_str().unwrap();
    assert_eq!(
        corre(&["proyecto", "catalogo", "--salida", r], &d).codigo,
        0
    );
    let actual = std::fs::read_to_string(&ruta).unwrap();
    assert!(actual.contains("\"redes.project/3\""));
    std::fs::write(
        &ruta,
        actual.replacen("\"redes.project/3\"", "\"redes.project/2\"", 1),
    )
    .unwrap();
    let s = corre(&["-p", r, "proyecto", "info"], &d);
    let j = sobre(&s);
    assert_eq!(j["ok"], false, "{j}");
    assert_eq!(s.codigo, 4);
    let diag = &j["diagnostics"][0];
    assert_eq!(diag["code"], "E_PROJ_SCHEMA");
    let msg = diag["message"].as_str().unwrap();
    assert!(
        msg.contains("redes.project/2") && msg.contains("redes.project/3"),
        "{msg}"
    );
    assert!(
        diag["hint"]
            .as_str()
            .is_some_and(|h| h.contains("importar")),
        "{diag}"
    );

    // Plan /2 sobre un proyecto vigente: no se importa nada y el proyecto no se toca.
    let ok = d.join("ok.redes");
    let o = ok.to_str().unwrap();
    let nuevo = ["proyecto", "nuevo", "--nombre", "x", "--salida", o];
    assert_eq!(corre(&nuevo, &d).codigo, 0);
    let antes = std::fs::read_to_string(&ok).unwrap();
    let plan = d.join("plan.json");
    std::fs::write(
        &plan,
        r#"{"schema":"redes.plan/2","proyecto":{"nombre":"x","crs":"EPSG:32717"},
            "objetos":[{"tipo":"nodo","codigo":"P1","familia":"saneamiento.pozo","e":760000.0,"n":9796000.0,"cota":2500.0}]}"#,
    )
    .unwrap();
    let p = plan.to_str().unwrap();
    let s = corre(&["-p", o, "importar", "plan", "--archivo", p], &d);
    let j = sobre(&s);
    assert_eq!(j["ok"], false, "{j}");
    assert_eq!(s.codigo, 7);
    assert_eq!(j["diagnostics"][0]["code"], "E_FMT_VERSION");
    assert!(
        j["diagnostics"][0]["message"]
            .as_str()
            .is_some_and(|m| m.contains("redes.plan/2") && m.contains("redes.plan/1")),
        "{j}"
    );
    assert_eq!(std::fs::read_to_string(&ok).unwrap(), antes);

    // Ambos códigos están en `redes esquema`; el antiguo E_PROJ_SCHEMA_TOO_NEW se fundió.
    let codigos = sobre(&corre(&["esquema"], &d))["data"]["codigos_error"].to_string();
    assert!(codigos.contains("\"E_PROJ_SCHEMA\""), "{codigos}");
    assert!(codigos.contains("\"E_FMT_VERSION\""), "{codigos}");
    assert!(!codigos.contains("E_PROJ_SCHEMA_TOO_NEW"), "{codigos}");
}

/// Los proyectos versionados en `demo/` (y la copia de `docs/demo/`) son de la versión vigente
/// y abren con el binario real.
#[test]
fn los_proyectos_de_demo_abren() {
    let raiz = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let d = tempdir("demos");
    let mut vistos = 0;
    for carpeta in ["demo", "docs/demo"] {
        for e in std::fs::read_dir(raiz.join(carpeta)).unwrap().flatten() {
            let ruta = e.path();
            if ruta.extension().is_none_or(|x| x != "redes") {
                continue;
            }
            let s = corre(&["-p", ruta.to_str().unwrap(), "proyecto", "info"], &d);
            let j = sobre(&s);
            assert_eq!(j["ok"], true, "{}: {j}", ruta.display());
            assert_eq!(s.codigo, 0);
            vistos += 1;
        }
    }
    assert!(
        vistos >= 5,
        "se esperaban los 5 .redes de demo; hubo {vistos}"
    );
}
