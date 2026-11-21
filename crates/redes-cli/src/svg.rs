//! Vista previa 3D en SVG: la CLI puede "ver" el modelo sin GPU.
//!
//! Usa la misma escena que la app (`redes_core::view`), así que un agente puede pedir una
//! imagen, mirarla y decidir, y lo que ve coincide con lo que ve el usuario.

use std::fmt::Write as _;

use redes_core::view::{Prim, Scene};

pub fn render(scene: &Scene, viewport: [f32; 2]) -> String {
    let mut s = String::with_capacity(64 * 1024);
    let _ = write!(
        s,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{:.0}\" height=\"{:.0}\" viewBox=\"0 0 {:.0} {:.0}\">\n",
        viewport[0], viewport[1], viewport[0], viewport[1]
    );
    s.push_str("<rect width=\"100%\" height=\"100%\" fill=\"#12141a\"/>\n");

    for p in &scene.prims {
        match p {
            Prim::Poly { pts, fill, .. } => {
                if pts.len() < 3 {
                    continue;
                }
                s.push_str("<polygon points=\"");
                for (i, q) in pts.iter().enumerate() {
                    if i > 0 {
                        s.push(' ');
                    }
                    let _ = write!(s, "{:.1},{:.1}", q[0], q[1]);
                }
                s.push_str("\" fill=\"");
                s.push_str(&rgba(*fill));
                s.push_str("\"/>\n");
            }
            Prim::Line { a, b, w, color, .. } => {
                let _ = write!(
                    s,
                    "<line x1=\"{:.1}\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\" stroke=\"{}\" stroke-width=\"{:.2}\" stroke-linecap=\"round\"/>\n",
                    a[0],
                    a[1],
                    b[0],
                    b[1],
                    rgba(*color),
                    w
                );
            }
        }
    }
    s.push_str("</svg>\n");
    s
}

fn rgba(c: [f32; 4]) -> String {
    let to = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("#{:02x}{:02x}{:02x}{:02x}", to(c[0]), to(c[1]), to(c[2]), to(c[3]))
}
