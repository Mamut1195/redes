//! Panel «Perfil» (F13-02): perfil longitudinal del colector del tramo seleccionado.
//!
//! El cálculo es `redes_core::perfil` (el mismo de `redes perfil`); aquí solo se dibuja con las
//! cotas que trae el archivo. Lo que falta se rotula `—` y queda en hueco.

use redes_core::model::{Project, SIN_DATO};
use redes_core::perfil::{self, Perfil};
use rune::tok::type_;
use rune::{LineCap, Shape, canvas, col, el, row, text};

use crate::{AL, AP, DIM, ERR, INK, PANEL, State, button};

fn color_rgb(r: f32, g: f32, b: f32, a: f32) -> rune::Color {
    rune::Color { r, g, b, a }
}

const ANCHO: f32 = 760.0;
const ALTO: f32 = 170.0;
const EXAGERACION: f64 = 10.0;

/// Perfil del colector que contiene el tramo `codigo`, de la cabecera a la descarga.
pub fn abrir(p: &Project, codigo: &str) -> Result<Perfil, String> {
    let tramos = perfil::colector_de(p, codigo).map_err(|e| e.to_string())?;
    perfil::desde_tramos(p, &tramos, 10.0).map_err(|e| e.to_string())
}

fn num(v: Option<f64>, d: usize) -> String {
    v.map_or(SIN_DATO.to_string(), |v| format!("{v:.d$}"))
}

/// Dibujo del perfil: rasante, tubos entre solera y clave y pozos. Sin datos no se dibuja nada.
fn dibujo(pf: &Perfil) -> Shape {
    let mut sh = Shape::new();
    let mut zs: Vec<f64> = pf.rasante.iter().filter_map(|r| r.z).collect();
    for t in &pf.tramos {
        zs.extend(
            [t.solera_ini, t.solera_fin, t.clave_ini, t.clave_fin]
                .into_iter()
                .flatten(),
        );
    }
    if zs.is_empty() {
        return sh;
    }
    let zmin = zs.iter().cloned().fold(f64::MAX, f64::min);
    let zmax = zs.iter().cloned().fold(f64::MIN, f64::max);
    let m = 10.0_f32;
    let sx = (ANCHO - 2.0 * m) as f64 / pf.longitud_total.max(1.0);
    // Exageración 10, ajustada si no cabe.
    let sy = (sx * EXAGERACION).min((ALTO - 2.0 * m) as f64 / (zmax - zmin).max(0.5));
    let x = |s: f64| m + (s * sx) as f32;
    let y = |z: f64| ALTO - m - ((z - zmin) * sy) as f32;
    let base = ALTO - 2.0;

    let mut serie: Vec<[f32; 2]> = Vec::new();
    let cierra = |serie: &mut Vec<[f32; 2]>, sh: Shape| -> Shape {
        let mut sh = sh;
        if serie.len() >= 2 {
            let mut poly = serie.clone();
            poly.push([serie[serie.len() - 1][0], base]);
            poly.push([serie[0][0], base]);
            sh = sh.fill(poly, color_rgb(0.87, 0.60, 0.31, 0.18));
            for w in serie.windows(2) {
                sh = sh.line(w[0], w[1], 1.5, LineCap::Round, AL);
            }
        }
        serie.clear();
        sh
    };
    for r in &pf.rasante {
        match r.z {
            Some(z) => serie.push([x(r.abscisa), y(z)]),
            None => sh = cierra(&mut serie, sh),
        }
    }
    sh = cierra(&mut serie, sh);

    for t in &pf.tramos {
        let (x0, x1) = (x(t.abscisa_ini), x(t.abscisa_fin));
        match (t.solera_ini, t.solera_fin, t.clave_ini, t.clave_fin) {
            (Some(a), Some(b), Some(c), Some(d)) => {
                sh = sh.fill(
                    vec![[x0, y(a)], [x1, y(b)], [x1, y(d)], [x0, y(c)]],
                    color_rgb(0.33, 0.63, 0.96, 0.85),
                );
                sh = sh.line([x0, y(a)], [x1, y(b)], 1.2, LineCap::Round, AP);
            }
            (Some(a), Some(b), _, _) => {
                sh = sh.line([x0, y(a)], [x1, y(b)], 1.2, LineCap::Round, AP);
            }
            // Sin solera: hueco, con una marca roja al pie.
            _ => sh = sh.line([x0, base], [x1, base], 2.0, LineCap::Butt, ERR),
        }
    }
    for pz in &pf.pozos {
        if let (Some(a), Some(s)) = (pz.tapa.or(pz.terreno), pz.solera) {
            sh = sh.line(
                [x(pz.abscisa), y(a)],
                [x(pz.abscisa), y(s)],
                3.0,
                LineCap::Butt,
                INK,
            );
        }
    }
    sh
}

/// Panel inferior con el dibujo y la tabla de pozos y tramos.
pub fn panel(pf: &Perfil) -> rune::El {
    let primero = pf.pozos.first().map_or("?", |p| p.codigo.as_str());
    let ultimo = pf.pozos.last().map_or("?", |p| p.codigo.as_str());
    let cab = row()
        .w_full()
        .gap(8.0)
        .items_center()
        .child(
            text(format!("Perfil {primero} → {ultimo}"))
                .font(type_::TITLE)
                .color(INK),
        )
        .child(
            text(format!(
                "{} tramos · L = {:.1} m · vertical ×{EXAGERACION:.0}",
                pf.tramos.len(),
                pf.longitud_total
            ))
            .font(type_::LABEL)
            .color(DIM),
        )
        .child(el().grow(1.0))
        .child(
            button("Cerrar perfil")
                .key("btn-perfil-cerrar")
                .on_press(|s: &mut State| s.perfil = None),
        );
    let mut filas = col().gap(2.0).w_full();
    filas = filas.child(
        text("pozo · abscisa · terreno · tapa · solera · prof.")
            .font(type_::LABEL)
            .color(DIM),
    );
    for pz in &pf.pozos {
        filas = filas.child(
            text(format!(
                "{} · {:.1} m · {} · {} · {} · {}",
                pz.codigo,
                pz.abscisa,
                num(pz.terreno, 2),
                num(pz.tapa, 2),
                num(pz.solera, 2),
                num(pz.profundidad, 2)
            ))
            .font(type_::MONO_CAPTION)
            .color(INK),
        );
    }
    filas = filas.child(text("tramo · DN · L · S").font(type_::LABEL).color(DIM));
    for t in &pf.tramos {
        filas = filas.child(
            text(format!(
                "{} · {} · {:.1} m · {} %",
                t.codigo,
                t.dn_mm.map_or(SIN_DATO.to_string(), |d| format!("DN{d}")),
                t.longitud,
                num(t.pendiente.map(|v| v * 100.0), 2)
            ))
            .font(type_::MONO_CAPTION)
            .color(INK),
        );
    }
    col()
        .key("panel-perfil")
        .w_full()
        .h(300.0)
        .padding(8.0)
        .gap(6.0)
        .fill(PANEL)
        .child(cab)
        .child(
            row()
                .w_full()
                .grow(1.0)
                .min_h(0.0)
                .gap(10.0)
                .child(canvas(dibujo(pf)).key("perfil-dibujo").w(ANCHO).h(ALTO))
                .child(filas.scroll(rune::ScrollSpec {
                    stick_bottom: false,
                })),
        )
}
