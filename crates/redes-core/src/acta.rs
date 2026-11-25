//! Acta de revisión (F13-04): observaciones y estados del proyecto en CSV y HTML imprimible.
//!
//! El acta **solo lista** lo que hay en el `.redes`: cada objeto (código, familia, subtipo,
//! estado) y cada observación con su historial (id, abierta/cerrada, fechas, revisiones, texto,
//! cierre, ubicación). No calcula, no valora y no juzga: lo ausente sale como «—».
//!
//! La revisión no identifica a nadie (D2): las columnas de autor solo aparecen si alguna
//! observación lo trae (se arrancó con `REDES_JOURNAL_USUARIO=1`). No hay firma.
//!
//! - CSV: BOM UTF-8 y `;` (como `cantidades`), para que Excel en español abra las tildes. Unas
//!   líneas de cabecera con el proyecto y el resumen, una fila en blanco y la tabla: una fila por
//!   observación, y una por objeto sin observaciones.
//! - HTML: un único archivo autocontenido (CSS en línea, sin scripts ni recursos externos), con
//!   `@media print` para imprimirlo o pasarlo a PDF desde el navegador.

use std::fmt::Write as _;

use crate::families;
use crate::geom::P3;
use crate::model::{EstadoObjeto, Project};
use crate::revision::Observacion;

/// Lo que se muestra cuando falta un dato.
pub const AUSENTE: &str = "—";

/// Un objeto del acta con su historial de observaciones.
#[derive(Clone, Debug)]
pub struct ObjetoActa {
    pub codigo: String,
    /// `nodo` o `tramo`.
    pub elemento: &'static str,
    pub familia: String,
    /// Nombre legible de la familia (registro), o la clave si no está registrada.
    pub familia_etiqueta: String,
    pub subtipo: String,
    /// Nombre legible del subtipo, o la clave; vacío si no hay subtipo.
    pub subtipo_etiqueta: String,
    pub estado: EstadoObjeto,
    pub observaciones: Vec<Observacion>,
}

impl ObjetoActa {
    pub fn abiertas(&self) -> usize {
        self.observaciones.iter().filter(|o| o.abierta()).count()
    }
}

/// Recuento por estado de objeto y de observación.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Resumen {
    pub objetos: usize,
    pub pendientes: usize,
    pub aprobados: usize,
    pub observados: usize,
    pub observaciones: usize,
    pub abiertas: usize,
    pub cerradas: usize,
}

/// El acta: cabecera del proyecto, resumen y objetos (nodos y luego tramos, por código).
#[derive(Clone, Debug)]
pub struct Acta {
    pub proyecto: String,
    pub id: String,
    pub crs: String,
    pub revision: u64,
    /// Estado global del proyecto (`importado`, `en_revision`, `aprobado`).
    pub estado_proyecto: &'static str,
    /// Cuándo se emitió el acta (UTC); `None` si el reloj no la da.
    pub emitida_utc: Option<String>,
    pub resumen: Resumen,
    pub objetos: Vec<ObjetoActa>,
}

/// Fecha UTC actual para la emisión (`YYYY-MM-DDTHH:MM:SSZ`).
pub fn ahora() -> Option<String> {
    crate::model::ahora_utc()
}

fn etiquetas(familia: &str, subtipo: &str) -> (String, String) {
    let def = families::family(familia);
    let fam = def.map_or(familia, |f| f.label).to_string();
    let ty = if subtipo.is_empty() {
        String::new()
    } else {
        def.and_then(|f| f.ty(subtipo))
            .map_or(subtipo, |t| t.label)
            .to_string()
    };
    (fam, ty)
}

impl Acta {
    /// Reúne el acta del proyecto. `emitida_utc` se pasa desde fuera para que sea reproducible.
    pub fn de(p: &Project, emitida_utc: Option<String>) -> Acta {
        let mut nodos: Vec<ObjetoActa> = p
            .nodes
            .iter()
            .filter(|n| !n.deleted)
            .map(|n| {
                let (fe, te) = etiquetas(&n.family, &n.ty);
                ObjetoActa {
                    codigo: n.code.clone(),
                    elemento: "nodo",
                    familia: n.family.clone(),
                    familia_etiqueta: fe,
                    subtipo: n.ty.clone(),
                    subtipo_etiqueta: te,
                    estado: n.estado,
                    observaciones: n.observaciones.clone(),
                }
            })
            .collect();
        let mut tramos: Vec<ObjetoActa> = p
            .links
            .iter()
            .filter(|l| !l.deleted)
            .map(|l| {
                let (fe, te) = etiquetas(&l.family, &l.ty);
                ObjetoActa {
                    codigo: l.code.clone(),
                    elemento: "tramo",
                    familia: l.family.clone(),
                    familia_etiqueta: fe,
                    subtipo: l.ty.clone(),
                    subtipo_etiqueta: te,
                    estado: l.estado,
                    observaciones: l.observaciones.clone(),
                }
            })
            .collect();
        nodos.sort_by(|a, b| a.codigo.cmp(&b.codigo));
        tramos.sort_by(|a, b| a.codigo.cmp(&b.codigo));
        nodos.append(&mut tramos);
        let objetos = nodos;

        let mut r = Resumen {
            objetos: objetos.len(),
            ..Resumen::default()
        };
        for o in &objetos {
            match o.estado {
                EstadoObjeto::Pendiente => r.pendientes += 1,
                EstadoObjeto::Aprobado => r.aprobados += 1,
                EstadoObjeto::Observado => r.observados += 1,
            }
            r.observaciones += o.observaciones.len();
            r.abiertas += o.abiertas();
        }
        r.cerradas = r.observaciones - r.abiertas;

        Acta {
            proyecto: p.name.clone(),
            id: p.id.clone(),
            crs: p.crs.clone(),
            revision: p.revision,
            estado_proyecto: p.estado.key(),
            emitida_utc,
            resumen: r,
            objetos,
        }
    }

    /// ¿Alguna observación trae autor? Solo entonces se muestran esas columnas (D2).
    pub fn con_autor(&self) -> bool {
        self.objetos
            .iter()
            .flat_map(|o| &o.observaciones)
            .any(|o| o.autor.is_some() || o.autor_cierre.is_some())
    }

    /// CSV con BOM y `;`: cabecera del acta, una fila en blanco y la tabla.
    pub fn a_csv(&self) -> String {
        let autor = self.con_autor();
        let r = &self.resumen;
        let mut s = String::from("\u{feff}");
        let mut linea = |campos: &[&str]| {
            let v: Vec<String> = campos.iter().map(|c| csv_campo(c)).collect();
            s.push_str(&v.join(";"));
            s.push('\n');
        };
        linea(&["Acta de revisión", &self.proyecto]);
        linea(&["Proyecto (id)", &self.id]);
        linea(&["CRS", &self.crs]);
        linea(&["Revisión del proyecto", &self.revision.to_string()]);
        linea(&["Estado del proyecto", self.estado_proyecto]);
        linea(&[
            "Emitida (UTC)",
            self.emitida_utc.as_deref().unwrap_or(AUSENTE),
        ]);
        linea(&[
            "Objetos",
            &r.objetos.to_string(),
            "pendientes",
            &r.pendientes.to_string(),
            "aprobados",
            &r.aprobados.to_string(),
            "observados",
            &r.observados.to_string(),
        ]);
        linea(&[
            "Observaciones",
            &r.observaciones.to_string(),
            "abiertas",
            &r.abiertas.to_string(),
            "cerradas",
            &r.cerradas.to_string(),
        ]);
        linea(&[]);
        let mut cab = vec![
            "codigo",
            "elemento",
            "familia",
            "subtipo",
            "estado",
            "obs_id",
            "obs_estado",
            "abierta_utc",
            "revision",
            "texto",
            "ubicacion",
            "cerrada_utc",
            "revision_cierre",
            "cierre",
        ];
        if autor {
            cab.extend(["autor", "autor_cierre"]);
        }
        linea(&cab);
        for o in &self.objetos {
            let base = [
                o.codigo.as_str(),
                o.elemento,
                o.familia_etiqueta.as_str(),
                o.subtipo_mostrado(),
                o.estado.key(),
            ];
            if o.observaciones.is_empty() {
                let mut f: Vec<&str> = base.to_vec();
                f.extend([AUSENTE; 9]);
                if autor {
                    f.extend([AUSENTE; 2]);
                }
                linea(&f);
                continue;
            }
            for ob in &o.observaciones {
                let id = format!("#{}", ob.id);
                let rev = ob.revision.to_string();
                let rev_c = ob
                    .revision_cierre
                    .map_or(AUSENTE.to_string(), |v| v.to_string());
                let ubic = ubicacion(ob.ubicacion);
                let mut f: Vec<&str> = base.to_vec();
                f.extend([
                    id.as_str(),
                    ob.estado.key(),
                    opt(&ob.fecha_utc),
                    rev.as_str(),
                    ob.texto.as_str(),
                    ubic.as_str(),
                    opt(&ob.cerrada_utc),
                    rev_c.as_str(),
                    opt(&ob.cierre),
                ]);
                if autor {
                    f.extend([opt(&ob.autor), opt(&ob.autor_cierre)]);
                }
                linea(&f);
            }
        }
        s
    }

    /// HTML autocontenido e imprimible (sin JS ni recursos externos).
    pub fn a_html(&self) -> String {
        let autor = self.con_autor();
        let r = &self.resumen;
        let mut h = String::new();
        let _ = write!(
            h,
            "<!doctype html>\n<html lang=\"es\">\n<head>\n<meta charset=\"utf-8\">\n\
             <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
             <title>Acta de revisión · {}</title>\n<style>{CSS}</style>\n</head>\n<body>\n",
            esc(&self.proyecto)
        );
        let _ = write!(
            h,
            "<header><h1>Acta de revisión</h1><p class=\"proy\">{}</p>\n<dl>\
             <dt>Proyecto (id)</dt><dd>{}</dd><dt>CRS</dt><dd>{}</dd>\
             <dt>Revisión del proyecto</dt><dd>{}</dd><dt>Estado del proyecto</dt><dd>{}</dd>\
             <dt>Emitida (UTC)</dt><dd>{}</dd></dl></header>\n",
            esc(&self.proyecto),
            esc(&self.id),
            esc(&self.crs),
            self.revision,
            esc(self.estado_proyecto),
            esc(self.emitida_utc.as_deref().unwrap_or(AUSENTE)),
        );
        let _ = write!(
            h,
            "<section><h2>Resumen</h2>\n<table class=\"resumen\"><thead><tr>\
             <th>Objetos</th><th>Pendientes</th><th>Aprobados</th><th>Observados</th>\
             <th>Observaciones</th><th>Abiertas</th><th>Cerradas</th></tr></thead>\n\
             <tbody><tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td>\
             <td>{}</td></tr></tbody></table></section>\n",
            r.objetos,
            r.pendientes,
            r.aprobados,
            r.observados,
            r.observaciones,
            r.abiertas,
            r.cerradas
        );

        // Observaciones, una fila por observación.
        h.push_str("<section><h2>Observaciones</h2>\n");
        if r.observaciones == 0 {
            h.push_str("<p class=\"vacio\">Sin observaciones registradas.</p>\n");
        } else {
            h.push_str(
                "<table class=\"obs\"><thead><tr><th>Objeto</th><th>Familia / subtipo</th>\
                 <th>Estado</th><th>#</th><th>Observación</th><th>Abierta</th>\
                 <th>Ubicación</th><th>Cierre</th>",
            );
            if autor {
                h.push_str("<th>Autor</th>");
            }
            h.push_str("</tr></thead>\n<tbody>\n");
            for o in self.objetos.iter().filter(|o| !o.observaciones.is_empty()) {
                for ob in &o.observaciones {
                    let cierre = if ob.abierta() {
                        AUSENTE.to_string()
                    } else {
                        format!(
                            "{}<br><small>{} · rev. {}</small>",
                            esc(opt(&ob.cierre)),
                            esc(opt(&ob.cerrada_utc)),
                            ob.revision_cierre
                                .map_or(AUSENTE.to_string(), |v| v.to_string())
                        )
                    };
                    let _ = write!(
                        h,
                        "<tr class=\"{}\"><td class=\"cod\">{}</td><td>{}<br><small>{}</small></td>\
                         <td><span class=\"e {}\">{}</span></td><td>#{}<br>\
                         <span class=\"o {}\">{}</span></td><td class=\"txt\">{}</td>\
                         <td><small>{} · rev. {}</small></td><td><small>{}</small></td>\
                         <td class=\"txt\">{}</td>",
                        ob.estado.key(),
                        esc(&o.codigo),
                        esc(&o.familia_etiqueta),
                        esc(o.subtipo_mostrado()),
                        o.estado.key(),
                        o.estado.key(),
                        ob.id,
                        ob.estado.key(),
                        ob.estado.key(),
                        esc(&ob.texto),
                        esc(opt(&ob.fecha_utc)),
                        ob.revision,
                        esc(&ubicacion(ob.ubicacion)),
                        cierre
                    );
                    if autor {
                        let _ = write!(
                            h,
                            "<td><small>{} / {}</small></td>",
                            esc(opt(&ob.autor)),
                            esc(opt(&ob.autor_cierre))
                        );
                    }
                    h.push_str("</tr>\n");
                }
            }
            h.push_str("</tbody></table>\n");
        }
        h.push_str("</section>\n");

        // Estado de cada objeto.
        h.push_str(
            "<section class=\"estados\"><h2>Estado por objeto</h2>\n\
             <table><thead><tr><th>Objeto</th><th>Elemento</th><th>Familia</th><th>Subtipo</th>\
             <th>Estado</th><th>Abiertas</th><th>Cerradas</th></tr></thead>\n<tbody>\n",
        );
        for o in &self.objetos {
            let ab = o.abiertas();
            let _ = writeln!(
                h,
                "<tr><td class=\"cod\">{}</td><td>{}</td><td>{}</td><td>{}</td>\
                 <td><span class=\"e {}\">{}</span></td><td>{}</td><td>{}</td></tr>",
                esc(&o.codigo),
                o.elemento,
                esc(&o.familia_etiqueta),
                esc(o.subtipo_mostrado()),
                o.estado.key(),
                o.estado.key(),
                ab,
                o.observaciones.len() - ab
            );
        }
        h.push_str("</tbody></table></section>\n");
        h.push_str(
            "<footer>Generado por Redes. El acta lista lo registrado en el proyecto: no calcula, \
             no valora ni verifica normativa. «—» indica un dato ausente. Sin firma ni \
             identificación de personas (D2).</footer>\n</body>\n</html>\n",
        );
        h
    }
}

impl ObjetoActa {
    fn subtipo_mostrado(&self) -> &str {
        if self.subtipo_etiqueta.is_empty() {
            AUSENTE
        } else {
            &self.subtipo_etiqueta
        }
    }
}

fn opt(v: &Option<String>) -> &str {
    v.as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(AUSENTE)
}

/// `E, N, Z` con 3 decimales; con la coma y el espacio Excel lo deja como texto (no lo convierte
/// en un número con separador de miles).
fn ubicacion(u: Option<P3>) -> String {
    u.map_or(AUSENTE.to_string(), |q| {
        format!("{:.3}, {:.3}, {:.3}", q.x, q.y, q.z)
    })
}

/// Campo CSV: entre comillas si lleva `;`, comillas o saltos de línea. Un texto de usuario que
/// empiece por `=`, `+`, `-` o `@` se prefija con `'` para que Excel no lo ejecute como fórmula.
fn csv_campo(c: &str) -> String {
    let c = if c.starts_with(['=', '+', '-', '@']) && c != AUSENTE {
        format!("'{c}")
    } else {
        c.to_string()
    };
    if c.contains([';', '"', '\n', '\r']) {
        format!("\"{}\"", c.replace('"', "\"\""))
    } else {
        c
    }
}

/// Escapa texto para HTML.
pub fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\'' => o.push_str("&#39;"),
            c => o.push(c),
        }
    }
    o
}

const CSS: &str = "\
*{box-sizing:border-box}\
body{font:13px/1.4 'Segoe UI',Arial,sans-serif;color:#1d2430;background:#fff;margin:24px auto;max-width:1200px;padding:0 16px}\
h1{font-size:22px;margin:0}h2{font-size:16px;margin:24px 0 8px;border-bottom:2px solid #1d2430;padding-bottom:4px}\
.proy{font-size:16px;margin:4px 0 12px;color:#3b4656}\
dl{display:grid;grid-template-columns:max-content 1fr;gap:2px 16px;margin:0}dt{color:#5b6676}dd{margin:0}\
table{border-collapse:collapse;width:100%}th,td{border:1px solid #c9ced6;padding:4px 6px;text-align:left;vertical-align:top}\
thead th{background:#eef1f5;font-weight:600}\
.resumen td{text-align:center;font-size:15px}.resumen th{text-align:center}\
.cod{font-family:Consolas,monospace;white-space:nowrap}.txt{white-space:pre-wrap;max-width:320px}\
small{color:#5b6676}.vacio{color:#5b6676;font-style:italic}\
.e,.o{display:inline-block;padding:0 6px;border-radius:8px;font-size:11px;border:1px solid}\
.e.pendiente{color:#7a5a00;border-color:#d9b44a;background:#fff7dd}\
.e.aprobado{color:#1d6b35;border-color:#5fb27a;background:#e9f7ee}\
.e.observado{color:#9b2c1f;border-color:#e08a7f;background:#fdecea}\
.o.abierta{color:#9b2c1f;border-color:#e08a7f}.o.cerrada{color:#5b6676;border-color:#c9ced6}\
tr.cerrada td.txt{color:#5b6676}\
footer{margin-top:24px;font-size:11px;color:#5b6676;border-top:1px solid #c9ced6;padding-top:6px}\
@media print{@page{size:A4 landscape;margin:12mm}body{margin:0;max-width:none;font-size:10.5px}\
thead{display:table-header-group}tr{break-inside:avoid}h2{break-after:avoid}\
.e,.o{-webkit-print-color-adjust:exact;print-color-adjust:exact}}\
";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_campo_cita_y_neutraliza_formulas() {
        assert_eq!(csv_campo("a;b"), "\"a;b\"");
        assert_eq!(csv_campo("dice \"x\""), "\"dice \"\"x\"\"\"");
        assert_eq!(csv_campo("=1+1"), "'=1+1");
        assert_eq!(csv_campo(AUSENTE), AUSENTE);
        assert_eq!(csv_campo("ñandú"), "ñandú");
    }

    #[test]
    fn esc_escapa_html() {
        assert_eq!(esc("<b>&\"'</b>"), "&lt;b&gt;&amp;&quot;&#39;&lt;/b&gt;");
    }
}
