//! Panel «Validación» (F13-01): el botón **Validar** corre `redes_core::validar` con los umbrales
//! del proyecto (o los de referencia) y lista los hallazgos; un clic en uno selecciona el objeto
//! y lleva la cámara a su ubicación. Es lo mismo que `redes validar` en la CLI.

use redes_core::validar::{self, Informe, Severidad};
use rune::tok::type_;
use rune::{El, col, el, row, text};

use crate::{DIM, ERR, INK, OK, State, WARN, button};

/// Cuántos hallazgos se listan como mucho (el resto se ve con la CLI).
const MAX_LISTA: usize = 60;

/// Resultado de la última validación.
#[derive(Default)]
pub struct Panel {
    pub informe: Option<Informe>,
}

impl State {
    /// Valida el proyecto tal como está ahora.
    pub fn validar(&mut self) {
        let reglas = self.project.reglas_validacion.clone().unwrap_or_default();
        let informe = validar::validar(&self.project, &reglas, None);
        self.aviso = Some(format!(
            "Validación: {} incumplen, {} no evaluables",
            informe.incumplen(),
            informe.no_evaluables()
        ));
        self.validacion.informe = Some(informe);
    }

    /// Selecciona el objeto principal del hallazgo `i` y centra la cámara en su ubicación.
    pub fn ir_a_hallazgo(&mut self, i: usize) {
        let Some(h) = self
            .validacion
            .informe
            .as_ref()
            .and_then(|inf| inf.hallazgos.get(i))
            .cloned()
        else {
            return;
        };
        let Some(code) = h.objetos.first().cloned() else {
            return;
        };
        let p = &self.project;
        // Cota de referencia para la cámara: la del objeto (geometría, no un dato del informe).
        let (pos, z) = if let Some(n) = p.node_by_code(&code) {
            (Some(n.pos), n.z_rim)
        } else if let Some(l) = p.link_by_code(&code) {
            let z = l.path.iter().map(|q| q.z).sum::<f64>() / l.path.len().max(1) as f64;
            (l.path.first().map(|q| q.p2()), z)
        } else {
            (None, self.cam.target[2])
        };
        let en = h
            .ubicacion
            .map(|u| redes_core::geom::P2::new(u[0], u[1]))
            .or(pos);
        if let Some(en) = en {
            self.cam.target = [en.x - p.origin.x, en.y - p.origin.y, z];
            self.cam.distance = 40.0;
            self.cam.elevation = 35.0;
            self.fitted = true;
        }
        self.sel = Some(code);
        self.opts.foco = None;
        self.rebuild();
    }
}

/// Escena `--scene validar`: valida y va al primer incumplimiento (para capturas `--snap`).
pub fn escena(s: &mut State) {
    s.cam.azimuth = -55.0;
    s.cam.elevation = 32.0;
    s.fitted = true;
    s.validar();
    let primero = s.validacion.informe.as_ref().and_then(|inf| {
        inf.hallazgos
            .iter()
            .position(|h| h.severidad != Severidad::NoEvaluable)
    });
    match primero {
        Some(i) => s.ir_a_hallazgo(i),
        None => s.rebuild(),
    }
}

fn glifo(s: Severidad) -> (&'static str, rune::Color) {
    match s {
        Severidad::Grave => ("✕", ERR),
        Severidad::Aviso => ("▲", WARN),
        Severidad::NoEvaluable => ("?", DIM),
    }
}

/// Sección del panel izquierdo: botón, recuento y lista navegable.
pub fn seccion(s: &State) -> El {
    let mut c = col()
        .gap(3.0)
        .child(text("Validación").font(type_::TITLE).color(INK))
        .child(
            button("Validar")
                .key("btn-validar")
                .on_press(|s: &mut State| s.validar()),
        );
    let Some(inf) = s.validacion.informe.as_ref() else {
        return c.child(
            text("Comprobaciones geométricas; umbrales del proyecto o de referencia")
                .font(type_::LABEL)
                .color(DIM)
                .wrap(),
        );
    };
    let (inc, ne) = (inf.incumplen(), inf.no_evaluables());
    c = c.child(
        row()
            .gap(6.0)
            .child(
                text(format!("{inc}"))
                    .font(type_::TITLE)
                    .color(if inc > 0 { WARN } else { OK }),
            )
            .child(text("incumplen").font(type_::LABEL).color(DIM))
            .child(text(format!("{ne}")).font(type_::TITLE).color(DIM))
            .child(text("sin dato").font(type_::LABEL).color(DIM)),
    );
    for (i, h) in inf.hallazgos.iter().enumerate().take(MAX_LISTA) {
        let (g, color) = glifo(h.severidad);
        let elegido = h.objetos.first() == s.sel.as_ref();
        c = c.child(
            row()
                .key(format!("hallazgo-{i}"))
                .w_full()
                .gap(5.0)
                .padding(3.0)
                .radius(4.0)
                .fill(if elegido {
                    crate::PANEL_HI
                } else {
                    crate::PANEL
                })
                .cursor(rune::Cursor::Pointer)
                .child(text(g).font(type_::LABEL).color(color))
                .child(
                    text(h.mensaje.clone())
                        .font(type_::LABEL)
                        .color(if elegido { INK } else { DIM })
                        .wrap()
                        .max_lines(3)
                        .grow(1.0)
                        .min_w(0.0),
                )
                .on_press(move |s: &mut State| s.ir_a_hallazgo(i)),
        );
    }
    if inf.hallazgos.len() > MAX_LISTA {
        c = c.child(
            text(format!(
                "… y {} más: `redes validar`",
                inf.hallazgos.len() - MAX_LISTA
            ))
            .font(type_::LABEL)
            .color(DIM),
        );
    }
    c.child(el().h(4.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view;
    use rune::{Env, Runtime};

    /// Validar lista hallazgos y un clic en uno selecciona su objeto y mueve la cámara.
    #[test]
    fn validar_y_navegar_a_un_hallazgo() {
        let mut st = State::new();
        // Un pozo demasiado profundo para tener al menos un hallazgo con ubicación.
        let mut p = (*st.project).clone();
        p.reglas_validacion = Some(redes_core::validar::ReglasValidacion {
            profundidad_max_pozo_m: 0.1,
            ..Default::default()
        });
        st.project = std::sync::Arc::new(p);
        st.rebuild();
        let mut rt = Runtime::new(
            st,
            view,
            (1280.0, 800.0),
            Env {
                system_dark: true,
                scale: 1.0,
            },
        );
        rt.frame(0.0);
        let (x, y) = rt.find("btn-validar").expect("botón Validar montado");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(1.0 / 60.0);
        let inf = rt.state.validacion.informe.clone().expect("hay informe");
        assert!(inf.incumplen() > 0, "{:?}", inf.resumen);
        let objetivo = inf.hallazgos[0].objetos[0].clone();
        let destino = inf.hallazgos[0].ubicacion.expect("con ubicación");
        rt.frame(1.0 / 60.0);
        let (x, y) = rt.find("hallazgo-0").expect("lista de hallazgos montada");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(1.0 / 60.0);
        assert_eq!(rt.state.sel.as_deref(), Some(objetivo.as_str()));
        let o = rt.state.project.origin;
        assert!((rt.state.cam.target[0] - (destino[0] - o.x)).abs() < 1e-6);
        assert!((rt.state.cam.target[1] - (destino[1] - o.y)).abs() < 1e-6);
    }
}
