//! Inspector: historial de observaciones de un objeto, añadir y cerrar (F13-03).
//!
//! La lógica (estado derivado, aprobar bloqueado con abiertas) está en `redes_core::revision`;
//! aquí solo se dibuja y se llama al dominio. Cada acción va al historial de deshacer y marca el
//! documento como sucio.

use redes_core::revision::{ErrorRevision, Observacion};
use rune::tok::type_;
use rune::{El, TextEdit, col, row, text, text_input};

use crate::{DIM, INK, LINE, OK, PANEL, State, WARN, chip, section};

/// Texto con que se observa si el campo está vacío (el chip «Observar» del inspector).
pub const TEXTO_POR_DEFECTO: &str = "Observado en revisión";

impl State {
    /// Aplica una operación de revisión del dominio; si falla, la explica en la barra de estado.
    fn revisar(
        &mut self,
        op: impl FnOnce(&mut redes_core::model::Project) -> Result<(), ErrorRevision>,
    ) {
        // Se opera sobre una copia: si el dominio lo rechaza, el proyecto y el deshacer no cambian.
        let mut p = (*self.project).clone();
        match op(&mut p) {
            Ok(()) => {
                self.commit_edit(p);
                self.aviso = None;
                self.rebuild();
            }
            Err(e) => self.aviso = Some(e.to_string()),
        }
    }

    /// Añade una observación con el texto del campo (o el texto por defecto) y vacía el campo.
    pub(crate) fn observar(&mut self, code: &str) {
        let texto = self.obs_texto.text.trim().to_string();
        let texto = if texto.is_empty() {
            TEXTO_POR_DEFECTO.to_string()
        } else {
            texto
        };
        self.revisar(|p| p.observar(code, &texto, None).map(|_| ()));
        self.obs_texto = TextEdit::new("");
    }

    /// Cierra una observación; el texto del campo, si lo hay, es el cierre.
    pub(crate) fn cerrar_observacion(&mut self, code: &str, id: u32) {
        let cierre = self.obs_texto.text.trim().to_string();
        self.revisar(|p| {
            p.cerrar_observacion(code, id, Some(cierre.as_str()))
                .map(|_| ())
        });
        self.obs_texto = TextEdit::new("");
    }

    /// Aprueba; con observaciones abiertas el dominio lo impide y se avisa.
    pub(crate) fn aprobar(&mut self, code: &str) {
        self.revisar(|p| p.aprobar_objeto(code, None).map(|_| ()));
    }
}

/// Bloque del inspector: historial, campo de texto y botones.
pub fn panel(s: &State, code: &str, obs: &[Observacion]) -> El {
    let mut c = col().w_full().gap(4.0).child(section("Observaciones"));
    if obs.is_empty() {
        c = c.child(text("Sin observaciones").font(type_::LABEL).color(DIM));
    }
    for o in obs.iter().rev() {
        let cab = format!(
            "#{} · {} · rev. {}{}",
            o.id,
            o.estado.key(),
            o.revision,
            o.fecha_utc
                .as_deref()
                .map(|f| format!(" · {}", &f[..f.len().min(10)]))
                .unwrap_or_default()
        );
        let mut fila = row().w_full().justify_between().gap(4.0).child(
            text(cab)
                .font(type_::LABEL)
                .color(if o.abierta() { WARN } else { DIM }),
        );
        if o.abierta() {
            let (cc, id) = (code.to_string(), o.id);
            fila = fila.child(
                chip("Cerrar", false)
                    .key(format!("btn-cerrar-obs-{id}"))
                    .on_press(move |s: &mut State| s.cerrar_observacion(&cc, id)),
            );
        }
        let mut item = col()
            .w_full()
            .gap(2.0)
            .padding(4.0)
            .radius(4.0)
            .border(1.0, LINE)
            .child(fila)
            .child(
                text(o.texto.clone())
                    .font(type_::LABEL)
                    .color(if o.abierta() { INK } else { DIM })
                    .wrap(),
            );
        if let Some(cierre) = &o.cierre {
            item = item.child(
                text(format!("Cierre: {cierre}"))
                    .font(type_::LABEL)
                    .color(OK)
                    .wrap(),
            );
        }
        c = c.child(item);
    }
    let ca = code.to_string();
    c.child(
        text_input(&s.obs_texto, type_::LABEL)
            .key("obs-texto")
            .label("Texto de la observación o del cierre")
            .w_full()
            .padding(4.0)
            .radius(4.0)
            .fill(PANEL)
            .border(1.0, LINE)
            .placeholder("Texto de la observación o del cierre", DIM)
            .on_edit(|s: &mut State| &mut s.obs_texto),
    )
    .child(
        chip("Añadir observación", false)
            .key("btn-anadir-obs")
            .on_press(move |s: &mut State| s.observar(&ca)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use redes_core::model::EstadoObjeto;
    use rune::{Env, Runtime};

    fn pulsa(rt: &mut Runtime<State>, clave: &str) {
        // Pausa entre pulsaciones: varias seguidas cuentan como multiclic y la cuarta se pierde.
        rt.frame(1.0);
        rt.frame(1.0 / 60.0);
        let (x, y) = rt
            .find(clave)
            .unwrap_or_else(|| panic!("{clave} debe estar montado"));
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(1.0 / 60.0);
    }

    /// El inspector añade observaciones con el texto del campo, no deja aprobar con abiertas y
    /// las cierra desde su historial.
    #[test]
    fn el_inspector_anade_y_cierra_observaciones() {
        let mut st = State::new();
        st.sel = Some("PZ-001".to_string());
        st.rebuild();
        let mut rt = Runtime::new(
            st,
            crate::view,
            (1280.0, 800.0),
            Env {
                system_dark: true,
                scale: 1.0,
            },
        );
        rt.frame(0.0);
        rt.state.obs_texto = TextEdit::new("Tapa desplazada");
        pulsa(&mut rt, "btn-anadir-obs");
        rt.state.obs_texto = TextEdit::new("Falta escalera");
        pulsa(&mut rt, "btn-anadir-obs");
        let obs = rt.state.project.observaciones_de("PZ-001").unwrap();
        assert_eq!(obs.len(), 2, "se añaden, no se sobrescriben");
        assert_eq!(obs[0].texto, "Tapa desplazada");
        assert!(rt.state.obs_texto.text.is_empty(), "el campo se vacía");

        // Aprobar con abiertas: el dominio lo impide y se avisa.
        pulsa(&mut rt, "btn-aprobar");
        let n = rt.state.project.node_by_code("PZ-001").unwrap();
        assert_eq!(n.estado, EstadoObjeto::Observado);
        assert!(rt.state.aviso.as_deref().unwrap_or("").contains("abierta"));

        rt.state.obs_texto = TextEdit::new("Corregido");
        pulsa(&mut rt, "btn-cerrar-obs-1");
        pulsa(&mut rt, "btn-cerrar-obs-2");
        let n = rt.state.project.node_by_code("PZ-001").unwrap();
        assert_eq!(n.estado, EstadoObjeto::Pendiente);
        assert_eq!(n.observaciones[0].cierre.as_deref(), Some("Corregido"));
        pulsa(&mut rt, "btn-aprobar");
        let n = rt.state.project.node_by_code("PZ-001").unwrap();
        assert_eq!(n.estado, EstadoObjeto::Aprobado);
        assert!(rt.state.dirty);
    }

    fn serialized(rt: &Runtime<State>) -> String {
        serde_json::to_string(&*rt.state.project).unwrap()
    }

    /// Undo and Redo walk back and forth through observe, close and approve, leaving the project
    /// exactly as it was at each step (F14-08).
    #[test]
    fn undo_and_redo_buttons_restore_each_review_step() {
        let mut st = State::new();
        st.sel = Some("PZ-001".to_string());
        st.rebuild();
        let mut rt = Runtime::new(
            st,
            crate::view,
            (1280.0, 800.0),
            Env {
                system_dark: true,
                scale: 1.0,
            },
        );
        rt.frame(0.0);
        let mut states = vec![serialized(&rt)];
        for button in ["btn-anadir-obs", "btn-cerrar-obs-1", "btn-aprobar"] {
            pulsa(&mut rt, button);
            states.push(serialized(&rt));
        }
        assert_ne!(states[2], states[3], "approve changed the project");

        for expected in states[..3].iter().rev() {
            pulsa(&mut rt, "btn-deshacer");
            assert_eq!(&serialized(&rt), expected);
        }
        pulsa(&mut rt, "btn-deshacer");
        assert_eq!(rt.state.aviso.as_deref(), Some("Nada que deshacer"));

        for expected in &states[1..] {
            pulsa(&mut rt, "btn-rehacer");
            assert_eq!(&serialized(&rt), expected);
        }
        pulsa(&mut rt, "btn-rehacer");
        assert_eq!(rt.state.aviso.as_deref(), Some("Nada que rehacer"));
        let n = rt.state.project.node_by_code("PZ-001").unwrap();
        assert_eq!(n.estado, EstadoObjeto::Aprobado);
    }
}
