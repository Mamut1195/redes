//! `redes perfil`: perfil longitudinal de un colector en SVG y JSON (F13-02).
//!
//! El cálculo está en `redes_core::perfil`; aquí solo se elige el recorrido, se dibuja y se
//! empaqueta en el sobre. Lo que falta en el archivo se rotula `—` y se deja en hueco.

use std::fmt::Write as _;
use std::path::PathBuf;

use redes_core::envelope::{Diagnostic, Envelope, Severity};
use redes_core::model::Project;
use redes_core::perfil::{self, Perfil, PerfilError};
use serde_json::json;

#[derive(clap::Args)]
pub struct PerfilArgs {
    /// Pozo de inicio (se sigue el sentido declarado de los tramos).
    #[arg(long, requires = "hasta", conflicts_with = "tramos")]
    pub desde: Option<String>,
    /// Pozo de fin.
    #[arg(long, requires = "desde")]
    pub hasta: Option<String>,
    /// Tramos consecutivos, separados por comas (`T1,T2,T3`).
    #[arg(long, value_delimiter = ',')]
    pub tramos: Vec<String>,
    /// SVG de salida.
    #[arg(long)]
    pub salida: Option<PathBuf>,
    /// Exageración vertical respecto a la horizontal.
    #[arg(long, default_value_t = 10.0)]
    pub exageracion: f64,
    /// Separación (m) del muestreo del terreno entre pozos; 0 = solo los pozos.
    #[arg(long, default_value_t = 10.0)]
    pub paso: f64,
}

fn falla(e: PerfilError) -> Envelope {
    Envelope::fail(
        "perfil",
        vec![
            Diagnostic::new(e.code, Severity::Error, e.message)
                .hint("Use `--tramos T1,T2` (consecutivos) o `--desde P1 --hasta P9`."),
        ],
    )
}

pub fn ejecutar(p: &Project, a: &PerfilArgs, dry: bool) -> Envelope {
    if !(a.exageracion.is_finite() && a.exageracion > 0.0) {
        return falla(PerfilError {
            code: "E_ARG_MISSING",
            message: "--exageracion debe ser un número mayor que 0".into(),
        });
    }
    let r = match (&a.desde, &a.hasta, a.tramos.is_empty()) {
        (Some(d), Some(h), true) => perfil::desde_pozos(p, d, h, a.paso),
        (None, None, false) => perfil::desde_tramos(p, &a.tramos, a.paso),
        _ => Err(PerfilError {
            code: "E_ARG_MISSING",
            message: "indique `--desde P1 --hasta P9` o `--tramos T1,T2` (una de las dos formas)"
                .into(),
        }),
    };
    let pf = match r {
        Ok(pf) => pf,
        Err(e) => return falla(e),
    };
    let (svg, efectiva) = render(&pf, a.exageracion);
    let mut env = Envelope::ok("perfil");
    let mut escrito = None;
    if let Some(salida) = &a.salida {
        if dry {
            escrito = Some(false);
        } else {
            if let Some(dir) = salida.parent()
                && !dir.as_os_str().is_empty()
            {
                let _ = std::fs::create_dir_all(dir);
            }
            if let Err(e) = std::fs::write(salida, svg) {
                return Envelope::fail(
                    "perfil",
                    vec![Diagnostic::new(
                        "E_IO_PERMISSION",
                        Severity::Error,
                        format!("no se pudo escribir {}: {e}", salida.display()),
                    )],
                );
            }
            env = env.with_artifact("perfil", &salida.display().to_string());
            escrito = Some(true);
        }
    }
    env.with_data(json!({
        "perfil": redondea(serde_json::to_value(&pf).unwrap_or_default()),
        "exageracion_pedida": a.exageracion,
        "exageracion_efectiva": (efectiva * 100.0).round() / 100.0,
        "salida": a.salida.as_ref().map(|s| s.display().to_string()),
        "escrito": escrito,
    }))
}

/// Redondea los números del JSON a 4 decimales (ruido de coma flotante fuera).
fn redondea(v: serde_json::Value) -> serde_json::Value {
    use serde_json::Value;
    match v {
        Value::Number(n) if n.is_f64() => {
            json!((n.as_f64().unwrap_or(0.0) * 1e4).round() / 1e4)
        }
        Value::Array(a) => Value::Array(a.into_iter().map(redondea).collect()),
        Value::Object(o) => Value::Object(o.into_iter().map(|(k, v)| (k, redondea(v))).collect()),
        o => o,
    }
}

// ---------------------------------------------------------------------------------------------
// SVG

fn num(v: Option<f64>, d: usize) -> String {
    v.map_or("—".to_string(), |v| fmt(v, d))
}

fn fmt(v: f64, d: usize) -> String {
    format!("{v:.d$}").replace('.', ",")
}

fn paso_nice(min: f64) -> f64 {
    let mut m = 1e-3;
    loop {
        for k in [1.0, 2.0, 5.0] {
            if m * k >= min {
                return m * k;
            }
        }
        m *= 10.0;
    }
}

/// Dibuja el perfil. Devuelve el SVG y la exageración vertical efectiva.
pub fn render(pf: &Perfil, exageracion: f64) -> (String, f64) {
    const ML: f64 = 140.0;
    const MR: f64 = 56.0;
    const MT: f64 = 56.0;
    const W: f64 = 1200.0;
    let largo = pf.longitud_total.max(1.0);
    let sx = W / largo;
    let mut sy = sx * exageracion;

    // Rango vertical con los datos que hay.
    let mut zs: Vec<f64> = pf.rasante.iter().filter_map(|r| r.z).collect();
    for z in &pf.pozos {
        zs.extend([z.terreno, z.tapa, z.solera].into_iter().flatten());
    }
    for t in &pf.tramos {
        zs.extend(
            [t.solera_ini, t.solera_fin, t.clave_ini, t.clave_fin]
                .into_iter()
                .flatten(),
        );
    }
    let (zmin, zmax) = zs
        .iter()
        .fold((f64::MAX, f64::MIN), |(a, b), z| (a.min(*z), b.max(*z)));
    let (zmin, zmax) = if zs.is_empty() {
        (0.0, 10.0)
    } else {
        (zmin - 0.3, zmax + 0.3)
    };
    let mut rango = (zmax - zmin).max(0.5);
    if rango * sy > 900.0 {
        sy = 900.0 / rango;
    }
    if rango * sy < 220.0 {
        rango = 220.0 / sy;
    }
    let gz = paso_nice(30.0 / sy);
    let zlo = (zmin / gz).floor() * gz;
    let zhi = (zmax.max(zlo + rango) / gz).ceil() * gz;
    let alto = (zhi - zlo) * sy;
    let efectiva = sy / sx;
    let x = |s: f64| ML + s * sx;
    let y = |z: f64| MT + (zhi - z) * sy;

    // Tabla inferior: ¿hay que girar el texto?
    let hueco_min = pf
        .pozos
        .windows(2)
        .map(|w| (w[1].abscisa - w[0].abscisa) * sx)
        .fold(f64::MAX, f64::min);
    let gira = hueco_min < 62.0;
    let rh = if gira { 54.0 } else { 18.0 };
    let filas_pozo = 6;
    let filas_tramo = 4;
    let tabla = rh * (filas_pozo + filas_tramo) as f64 + 14.0;
    let ancho = ML + W + MR;
    let total = MT + alto + 24.0 + tabla + 16.0;

    let mut s = String::with_capacity(64 * 1024);
    let _ = writeln!(
        s,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{ancho:.0}\" height=\"{total:.0}\" viewBox=\"0 0 {ancho:.0} {total:.0}\" font-family=\"Arial, Helvetica, sans-serif\" font-size=\"11\">"
    );
    s.push_str("<rect width=\"100%\" height=\"100%\" fill=\"#ffffff\"/>\n");
    let primero = pf.pozos.first().map_or("?", |p| p.codigo.as_str());
    let ultimo = pf.pozos.last().map_or("?", |p| p.codigo.as_str());
    let _ = writeln!(
        s,
        "<text x=\"{ML}\" y=\"24\" font-size=\"15\" font-weight=\"bold\" fill=\"#111\">Perfil longitudinal {primero} → {ultimo}</text>"
    );
    let _ = writeln!(
        s,
        "<text x=\"{ML}\" y=\"42\" fill=\"#444\">{} tramos · L = {} m · escala vertical exagerada ×{} (horizontal 1 : {:.0})</text>",
        pf.tramos.len(),
        fmt(pf.longitud_total, 1),
        fmt(efectiva, 1),
        1000.0 / (sx * 0.2646)
    );

    // Cuadrícula y ejes.
    let mut z = zlo;
    while z <= zhi + 1e-9 {
        let yy = y(z);
        let _ = writeln!(
            s,
            "<line x1=\"{ML}\" y1=\"{yy:.1}\" x2=\"{:.1}\" y2=\"{yy:.1}\" stroke=\"#d9d9d9\" stroke-width=\"0.7\"/><text x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"end\" fill=\"#555\">{}</text>",
            ML + W,
            ML - 6.0,
            yy + 4.0,
            fmt(z, if gz < 1.0 { 1 } else { 0 })
        );
        z += gz;
    }
    let gx = paso_nice(70.0 / sx);
    let mut sa = 0.0;
    while sa <= largo + 1e-9 {
        let xx = x(sa);
        let _ = writeln!(
            s,
            "<line x1=\"{xx:.1}\" y1=\"{MT}\" x2=\"{xx:.1}\" y2=\"{:.1}\" stroke=\"#e6e6e6\" stroke-width=\"0.7\"/><text x=\"{xx:.1}\" y=\"{:.1}\" text-anchor=\"middle\" fill=\"#555\">{}</text>",
            MT + alto,
            MT + alto + 14.0,
            fmt(sa, 0)
        );
        sa += gx;
    }
    let _ = writeln!(
        s,
        "<rect x=\"{ML}\" y=\"{MT}\" width=\"{W}\" height=\"{alto:.1}\" fill=\"none\" stroke=\"#888\"/>"
    );
    let _ = writeln!(
        s,
        "<text x=\"14\" y=\"{:.1}\" transform=\"rotate(-90 14 {:.1})\" text-anchor=\"middle\" fill=\"#555\">Cota (m s. n. m.)</text>",
        MT + alto / 2.0,
        MT + alto / 2.0
    );

    // Rasante: tramos continuos de puntos con dato; el hueco rompe la línea.
    let mut serie: Vec<(f64, f64)> = Vec::new();
    let cierra = |serie: &mut Vec<(f64, f64)>, s: &mut String| {
        if serie.len() >= 2 {
            let pts: String = serie
                .iter()
                .map(|(a, b)| format!("{:.1},{:.1}", x(*a), y(*b)))
                .collect::<Vec<_>>()
                .join(" ");
            let _ = writeln!(
                s,
                "<polygon points=\"{pts} {:.1},{:.1} {:.1},{:.1}\" fill=\"#f1e8d8\" stroke=\"none\"/>",
                x(serie[serie.len() - 1].0),
                MT + alto,
                x(serie[0].0),
                MT + alto
            );
            let _ = writeln!(
                s,
                "<polyline points=\"{pts}\" fill=\"none\" stroke=\"#8a6232\" stroke-width=\"1.6\"/>"
            );
        }
        serie.clear();
    };
    for r in &pf.rasante {
        match r.z {
            Some(z) => serie.push((r.abscisa, z)),
            None => cierra(&mut serie, &mut s),
        }
    }
    cierra(&mut serie, &mut s);

    // Tubos.
    for t in &pf.tramos {
        let (x0, x1) = (x(t.abscisa_ini), x(t.abscisa_fin));
        match (t.solera_ini, t.solera_fin, t.clave_ini, t.clave_fin) {
            (Some(a), Some(b), Some(c), Some(d)) => {
                let _ = writeln!(
                    s,
                    "<polygon points=\"{x0:.1},{:.1} {x1:.1},{:.1} {x1:.1},{:.1} {x0:.1},{:.1}\" fill=\"#8fbbe3\" stroke=\"#1f4e79\" stroke-width=\"1\"/>",
                    y(a),
                    y(b),
                    y(d),
                    y(c)
                );
            }
            (Some(a), Some(b), _, _) => {
                let _ = writeln!(
                    s,
                    "<line x1=\"{x0:.1}\" y1=\"{:.1}\" x2=\"{x1:.1}\" y2=\"{:.1}\" stroke=\"#1f4e79\" stroke-width=\"1.5\" stroke-dasharray=\"5 3\"/>",
                    y(a),
                    y(b)
                );
            }
            _ => {
                let _ = writeln!(
                    s,
                    "<rect x=\"{x0:.1}\" y=\"{MT}\" width=\"{:.1}\" height=\"{alto:.1}\" fill=\"#fbe9e7\" fill-opacity=\"0.6\" stroke=\"#c0392b\" stroke-dasharray=\"4 3\" stroke-width=\"0.8\"/><text x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"middle\" fill=\"#c0392b\" font-size=\"10\">sin cota de solera</text>",
                    (x1 - x0).max(1.0),
                    (x0 + x1) / 2.0,
                    MT + 14.0
                );
            }
        }
    }
    // Pozos: del terreno o la tapa hasta la solera, solo si hay ambos datos.
    for pz in &pf.pozos {
        let xx = x(pz.abscisa);
        let _ = writeln!(
            s,
            "<line x1=\"{xx:.1}\" y1=\"{MT}\" x2=\"{xx:.1}\" y2=\"{:.1}\" stroke=\"#bbb\" stroke-dasharray=\"2 3\" stroke-width=\"0.6\"/>",
            MT + alto
        );
        if let (Some(arriba), Some(sol)) = (pz.tapa.or(pz.terreno), pz.solera) {
            let _ = writeln!(
                s,
                "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"8\" height=\"{:.1}\" fill=\"#e4e4e4\" stroke=\"#444\" stroke-width=\"1\"/>",
                xx - 4.0,
                y(arriba),
                (y(sol) - y(arriba)).max(0.5)
            );
        }
    }

    // Tabla de pozos y de tramos.
    let y0 = MT + alto + 24.0;
    let mut fila = 0usize;
    let rotulo = |s: &mut String, k: usize, txt: &str| {
        let yy = y0 + rh * k as f64;
        let _ = writeln!(
            s,
            "<line x1=\"{:.1}\" y1=\"{yy:.1}\" x2=\"{:.1}\" y2=\"{yy:.1}\" stroke=\"#ccc\" stroke-width=\"0.6\"/><text x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"end\" font-weight=\"bold\" fill=\"#333\">{txt}</text>",
            10.0,
            ML + W,
            ML - 36.0,
            yy + rh / 2.0 + 4.0
        );
    };
    let celda = |s: &mut String, k: usize, xx: f64, txt: &str, color: &str| {
        let yy = y0 + rh * k as f64;
        if gira {
            let (cx, cy) = (xx + 4.0, yy + rh - 4.0);
            let _ = writeln!(
                s,
                "<text x=\"{cx:.1}\" y=\"{cy:.1}\" transform=\"rotate(-90 {cx:.1} {cy:.1})\" fill=\"{color}\">{txt}</text>"
            );
        } else {
            let _ = writeln!(
                s,
                "<text x=\"{xx:.1}\" y=\"{:.1}\" text-anchor=\"middle\" fill=\"{color}\">{txt}</text>",
                yy + rh / 2.0 + 4.0
            );
        }
    };
    let filas: [(&str, Box<dyn Fn(&redes_core::perfil::PozoPerfil) -> String>); 6] = [
        ("Pozo", Box::new(|p| p.codigo.clone())),
        ("Abscisa (m)", Box::new(|p| fmt(p.abscisa, 1))),
        ("Terreno", Box::new(|p| num(p.terreno, 2))),
        ("Tapa", Box::new(|p| num(p.tapa, 2))),
        ("Solera", Box::new(|p| num(p.solera, 2))),
        ("Prof. (m)", Box::new(|p| num(p.profundidad, 2))),
    ];
    for (k, (nombre, f)) in filas.iter().enumerate() {
        rotulo(&mut s, k, nombre);
        for pz in &pf.pozos {
            let t = f(pz);
            let color = if t == "—" { "#c0392b" } else { "#111" };
            celda(&mut s, k, x(pz.abscisa), &t, color);
        }
        fila += 1;
    }
    let tramo_rows: [(
        &str,
        Box<dyn Fn(&redes_core::perfil::TramoPerfil) -> String>,
    ); 4] = [
        ("Tramo", Box::new(|t| t.codigo.clone())),
        (
            "DN (mm)",
            Box::new(|t| t.dn_mm.map_or("—".into(), |d| d.to_string())),
        ),
        ("L (m)", Box::new(|t| fmt(t.longitud, 1))),
        (
            "S (%)",
            Box::new(|t| num(t.pendiente.map(|v| v * 100.0), 2)),
        ),
    ];
    for (k, (nombre, f)) in tramo_rows.iter().enumerate() {
        rotulo(&mut s, fila + k, nombre);
        for t in &pf.tramos {
            let txt = f(t);
            let color = if txt == "—" { "#c0392b" } else { "#111" };
            let mid = x((t.abscisa_ini + t.abscisa_fin) / 2.0);
            let ancho_t = (t.abscisa_fin - t.abscisa_ini) * sx;
            if ancho_t < 62.0 && !gira {
                // Sin espacio horizontal: se gira solo esta celda.
                let (cx, cy) = (mid + 4.0, y0 + rh * (fila + k) as f64 + rh - 4.0);
                let _ = writeln!(
                    s,
                    "<text x=\"{cx:.1}\" y=\"{cy:.1}\" transform=\"rotate(-90 {cx:.1} {cy:.1})\" fill=\"{color}\">{txt}</text>"
                );
            } else {
                celda(&mut s, fila + k, mid, &txt, color);
            }
        }
    }
    let yf = y0 + rh * (filas_pozo + filas_tramo) as f64;
    let _ = writeln!(
        s,
        "<line x1=\"10\" y1=\"{yf:.1}\" x2=\"{:.1}\" y2=\"{yf:.1}\" stroke=\"#ccc\" stroke-width=\"0.6\"/>",
        ML + W
    );
    let _ = writeln!(
        s,
        "<text x=\"{ML}\" y=\"{:.1}\" fill=\"#666\" font-size=\"10\">Clave = solera + diámetro interior. S = pendiente de solera en el sentido del flujo. — = el archivo no trae el dato (no se interpola).</text>",
        yf + 14.0
    );
    s.push_str("</svg>\n");
    (s, efectiva)
}
