//! Observaciones de la revisión con historial (F13-03).
//!
//! Cada objeto (nodo o tramo) lleva una lista de [`Observacion`]es que **no se sobrescriben**: se
//! añaden y se cierran. El estado de revisión del objeto se deriva de ellas:
//!
//! - **observado** ⇔ hay al menos una observación abierta;
//! - cerrar la última abierta devuelve el objeto a **pendiente** (hay que aprobarlo a propósito);
//! - **aprobar** con observaciones abiertas es un error, salvo que se cierren todas en el mismo
//!   acto con un texto de cierre ([`Project::aprobar_objeto`]).
//!
//! La revisión no identifica a nadie (D2): `autor` solo se rellena si se arranca con
//! `REDES_JOURNAL_USUARIO=1`, con el mismo criterio que el journal.
//!
//! Cada operación hace su propio `commit` en el journal con el código del objeto y el id de la
//! observación (`objeto.observar PZ-001 #2`).

use serde::{Deserialize, Serialize};

use crate::geom::P3;
use crate::model::{EstadoObjeto, Project, ahora_utc, usuario_del_sistema};

/// Estado de una observación.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EstadoObservacion {
    Abierta,
    Cerrada,
}

impl EstadoObservacion {
    pub fn key(self) -> &'static str {
        match self {
            EstadoObservacion::Abierta => "abierta",
            EstadoObservacion::Cerrada => "cerrada",
        }
    }
}

/// Una observación de la revisión sobre un objeto: qué hay que corregir en la app de origen.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Observacion {
    /// Correlativo dentro del objeto (1, 2, 3…); no se reutiliza.
    pub id: u32,
    /// Cuándo se abrió (UTC, `YYYY-MM-DDTHH:MM:SSZ`). `None` si el reloj del sistema no la da.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fecha_utc: Option<String>,
    pub texto: String,
    pub estado: EstadoObservacion,
    /// Cuándo se cerró (UTC).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cerrada_utc: Option<String>,
    /// Texto de cierre (qué se hizo), opcional.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cierre: Option<String>,
    /// Revisión del proyecto en la que se abrió (la del `commit` que la registró).
    #[serde(default)]
    pub revision: u64,
    /// Revisión del proyecto en la que se cerró.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision_cierre: Option<u64>,
    /// Punto concreto del objeto al que se refiere (coordenadas del proyecto), si se indicó.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ubicacion: Option<P3>,
    /// Usuario del sistema que la abrió; solo con `REDES_JOURNAL_USUARIO=1` (D2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub autor: Option<String>,
    /// Usuario del sistema que la cerró; solo con `REDES_JOURNAL_USUARIO=1` (D2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub autor_cierre: Option<String>,
}

impl Observacion {
    pub fn abierta(&self) -> bool {
        self.estado == EstadoObservacion::Abierta
    }
}

/// Texto de la última observación abierta (lo que antes era la `nota`).
pub fn nota_vigente(obs: &[Observacion]) -> Option<&str> {
    obs.iter()
        .rev()
        .find(|o| o.abierta())
        .map(|o| o.texto.as_str())
}

/// Estado coherente con las observaciones: observado si hay alguna abierta; si no, el que tenga
/// salvo que fuera observado (entonces pendiente).
pub fn estado_derivado(actual: EstadoObjeto, obs: &[Observacion]) -> EstadoObjeto {
    if obs.iter().any(Observacion::abierta) {
        EstadoObjeto::Observado
    } else if actual == EstadoObjeto::Observado {
        EstadoObjeto::Pendiente
    } else {
        actual
    }
}

/// Por qué no se pudo hacer una operación de revisión.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ErrorRevision {
    /// No hay ningún nodo ni tramo con ese código.
    NoExiste { codigo: String },
    /// El texto de la observación está vacío.
    TextoVacio,
    /// El objeto no tiene una observación con ese id.
    ObservacionNoExiste { codigo: String, id: u32 },
    /// La observación ya estaba cerrada.
    YaCerrada { codigo: String, id: u32 },
    /// El objeto tiene observaciones abiertas (al aprobar o reabrir).
    Abiertas { codigo: String, ids: Vec<u32> },
}

impl ErrorRevision {
    /// Código estable del error para el sobre de la CLI.
    pub fn codigo(&self) -> &'static str {
        match self {
            ErrorRevision::NoExiste { .. } => "E_STATE_NOT_FOUND",
            ErrorRevision::TextoVacio => "E_ARG_MISSING",
            ErrorRevision::ObservacionNoExiste { .. } => "E_STATE_NOT_FOUND",
            ErrorRevision::YaCerrada { .. } => "E_STATE_OBS_CERRADA",
            ErrorRevision::Abiertas { .. } => "E_STATE_OBS_ABIERTAS",
        }
    }
}

impl std::fmt::Display for ErrorRevision {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ErrorRevision::NoExiste { codigo } => write!(f, "No existe el código {codigo}."),
            ErrorRevision::TextoVacio => write!(f, "La observación necesita un texto."),
            ErrorRevision::ObservacionNoExiste { codigo, id } => {
                write!(f, "{codigo} no tiene la observación #{id}.")
            }
            ErrorRevision::YaCerrada { codigo, id } => {
                write!(f, "La observación #{id} de {codigo} ya está cerrada.")
            }
            ErrorRevision::Abiertas { codigo, ids } => {
                let lista: Vec<String> = ids.iter().map(|i| format!("#{i}")).collect();
                write!(
                    f,
                    "{codigo} tiene {} observación(es) abierta(s): {}.",
                    ids.len(),
                    lista.join(", ")
                )
            }
        }
    }
}

/// Lo que una operación necesita de un nodo o un tramo.
struct Revisable<'a> {
    codigo: String,
    estado: &'a mut EstadoObjeto,
    obs: &'a mut Vec<Observacion>,
}

impl Project {
    fn revisable(&mut self, codigo: &str) -> Result<Revisable<'_>, ErrorRevision> {
        if let Some(i) = self.pos_nodo_codigo(codigo) {
            let n = &mut self.nodes[i];
            return Ok(Revisable {
                codigo: n.code.clone(),
                estado: &mut n.estado,
                obs: &mut n.observaciones,
            });
        }
        if let Some(i) = self.pos_tramo_codigo(codigo) {
            let l = &mut self.links[i];
            return Ok(Revisable {
                codigo: l.code.clone(),
                estado: &mut l.estado,
                obs: &mut l.observaciones,
            });
        }
        Err(ErrorRevision::NoExiste {
            codigo: codigo.to_string(),
        })
    }

    /// Historial de observaciones de un objeto (nodo o tramo), o `None` si no existe.
    pub fn observaciones_de(&self, codigo: &str) -> Option<&[Observacion]> {
        self.node_by_code(codigo)
            .map(|n| n.observaciones.as_slice())
            .or_else(|| {
                self.link_by_code(codigo)
                    .map(|l| l.observaciones.as_slice())
            })
    }

    /// Todas las observaciones del proyecto con el código de su objeto, en orden de objeto y de
    /// id. Es la materia del acta (F13-04).
    pub fn todas_las_observaciones(&self) -> Vec<(&str, &Observacion)> {
        let nodos = self
            .nodes
            .iter()
            .filter(|n| !n.deleted)
            .flat_map(|n| n.observaciones.iter().map(move |o| (n.code.as_str(), o)));
        let tramos = self
            .links
            .iter()
            .filter(|l| !l.deleted)
            .flat_map(|l| l.observaciones.iter().map(move |o| (l.code.as_str(), o)));
        nodos.chain(tramos).collect()
    }

    /// Cuántas observaciones abiertas quedan en el proyecto.
    pub fn observaciones_abiertas(&self) -> usize {
        self.todas_las_observaciones()
            .iter()
            .filter(|(_, o)| o.abierta())
            .count()
    }

    /// Añade una observación abierta (no sobrescribe las anteriores) y deja el objeto observado.
    /// Devuelve el id de la observación.
    pub fn observar(
        &mut self,
        codigo: &str,
        texto: &str,
        ubicacion: Option<P3>,
    ) -> Result<u32, ErrorRevision> {
        let texto = texto.trim();
        if texto.is_empty() {
            return Err(ErrorRevision::TextoVacio);
        }
        let revision = self.revision + 1;
        let r = self.revisable(codigo)?;
        let id = r.obs.iter().map(|o| o.id).max().unwrap_or(0) + 1;
        r.obs.push(Observacion {
            id,
            fecha_utc: ahora_utc(),
            texto: texto.to_string(),
            estado: EstadoObservacion::Abierta,
            cerrada_utc: None,
            cierre: None,
            revision,
            revision_cierre: None,
            ubicacion,
            autor: usuario_del_sistema(),
            autor_cierre: None,
        });
        *r.estado = EstadoObjeto::Observado;
        let codigo = r.codigo;
        self.commit(&format!("objeto.observar {codigo} #{id}"));
        Ok(id)
    }

    /// Cierra una observación. Si era la última abierta, el objeto pasa a pendiente (no a
    /// aprobado: aprobar es otro acto). Devuelve el estado resultante del objeto.
    pub fn cerrar_observacion(
        &mut self,
        codigo: &str,
        id: u32,
        cierre: Option<&str>,
    ) -> Result<EstadoObjeto, ErrorRevision> {
        let revision = self.revision + 1;
        let r = self.revisable(codigo)?;
        let Some(o) = r.obs.iter_mut().find(|o| o.id == id) else {
            return Err(ErrorRevision::ObservacionNoExiste {
                codigo: r.codigo,
                id,
            });
        };
        if !o.abierta() {
            return Err(ErrorRevision::YaCerrada {
                codigo: r.codigo,
                id,
            });
        }
        cerrar(o, cierre, revision);
        *r.estado = estado_derivado(*r.estado, r.obs);
        let (codigo, estado) = (r.codigo, *r.estado);
        self.commit(&format!("objeto.cerrar_observacion {codigo} #{id}"));
        Ok(estado)
    }

    /// Aprueba un objeto. Con observaciones abiertas es un error ([`ErrorRevision::Abiertas`]),
    /// salvo que `cerrar_todas` traiga el texto con que se cierran todas en el mismo acto.
    /// Devuelve los ids cerrados.
    pub fn aprobar_objeto(
        &mut self,
        codigo: &str,
        cerrar_todas: Option<&str>,
    ) -> Result<Vec<u32>, ErrorRevision> {
        let revision = self.revision + 1;
        let r = self.revisable(codigo)?;
        let abiertas: Vec<u32> = r.obs.iter().filter(|o| o.abierta()).map(|o| o.id).collect();
        let cierre = cerrar_todas.map(str::trim).filter(|t| !t.is_empty());
        if !abiertas.is_empty() && cierre.is_none() {
            return Err(ErrorRevision::Abiertas {
                codigo: r.codigo,
                ids: abiertas,
            });
        }
        for o in r.obs.iter_mut().filter(|o| o.abierta()) {
            cerrar(o, cierre, revision);
        }
        *r.estado = EstadoObjeto::Aprobado;
        let codigo = r.codigo;
        let sufijo: String = abiertas.iter().map(|i| format!(" #{i}")).collect();
        self.commit(&format!("objeto.aprobar {codigo}{sufijo}"));
        Ok(abiertas)
    }

    /// Devuelve a pendiente un objeto aprobado (o pendiente). Con observaciones abiertas es un
    /// error: se cierran con [`Project::cerrar_observacion`].
    pub fn reabrir_objeto(&mut self, codigo: &str) -> Result<(), ErrorRevision> {
        let r = self.revisable(codigo)?;
        let abiertas: Vec<u32> = r.obs.iter().filter(|o| o.abierta()).map(|o| o.id).collect();
        if !abiertas.is_empty() {
            return Err(ErrorRevision::Abiertas {
                codigo: r.codigo,
                ids: abiertas,
            });
        }
        *r.estado = EstadoObjeto::Pendiente;
        let codigo = r.codigo;
        self.commit(&format!("objeto.reabrir {codigo}"));
        Ok(())
    }
}

fn cerrar(o: &mut Observacion, cierre: Option<&str>, revision: u64) {
    o.estado = EstadoObservacion::Cerrada;
    o.cerrada_utc = ahora_utc();
    o.cierre = cierre
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(str::to_string);
    o.revision_cierre = Some(revision);
    o.autor_cierre = usuario_del_sistema();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::demo;

    fn proyecto() -> (Project, String) {
        let p = demo::project();
        let c = p.nodes.iter().find(|n| !n.deleted).unwrap().code.clone();
        (p, c)
    }

    #[test]
    fn observar_anade_sin_sobrescribir_y_deja_observado() {
        let (mut p, c) = proyecto();
        let r0 = p.revision;
        assert_eq!(p.observar(&c, "Tapa desplazada", None), Ok(1));
        assert_eq!(p.observar(&c, "Falta escalera", None), Ok(2));
        let obs = p.observaciones_de(&c).unwrap();
        assert_eq!(obs.len(), 2);
        assert_eq!(obs[0].texto, "Tapa desplazada");
        assert_eq!(obs[0].revision, r0 + 1);
        assert_eq!(obs[1].revision, r0 + 2);
        assert!(obs[1].fecha_utc.is_some());
        assert!(obs[0].autor.is_none(), "D2: sin autor por defecto");
        let n = p.node_by_code(&c).unwrap();
        assert_eq!(n.estado, EstadoObjeto::Observado);
        assert_eq!(n.nota_vigente(), Some("Falta escalera"));
        assert_eq!(
            p.journal.last().unwrap().command,
            format!("objeto.observar {c} #2")
        );
    }

    #[test]
    fn un_texto_vacio_no_crea_observacion() {
        let (mut p, c) = proyecto();
        assert_eq!(p.observar(&c, "  ", None), Err(ErrorRevision::TextoVacio));
        assert!(p.observaciones_de(&c).unwrap().is_empty());
    }

    #[test]
    fn cerrar_la_ultima_abierta_deja_pendiente() {
        let (mut p, c) = proyecto();
        p.observar(&c, "a", None).unwrap();
        p.observar(&c, "b", None).unwrap();
        assert_eq!(
            p.cerrar_observacion(&c, 1, Some("corregido")),
            Ok(EstadoObjeto::Observado),
            "queda la #2 abierta"
        );
        assert_eq!(
            p.cerrar_observacion(&c, 1, None),
            Err(ErrorRevision::YaCerrada {
                codigo: c.clone(),
                id: 1
            })
        );
        assert!(matches!(
            p.cerrar_observacion(&c, 9, None),
            Err(ErrorRevision::ObservacionNoExiste { id: 9, .. })
        ));
        assert_eq!(
            p.cerrar_observacion(&c, 2, None),
            Ok(EstadoObjeto::Pendiente)
        );
        let o = &p.observaciones_de(&c).unwrap()[0];
        assert_eq!(o.cierre.as_deref(), Some("corregido"));
        assert!(o.cerrada_utc.is_some() && o.revision_cierre.is_some());
        assert_eq!(p.node_by_code(&c).unwrap().nota_vigente(), None);
        assert_eq!(
            p.journal.last().unwrap().command,
            format!("objeto.cerrar_observacion {c} #2")
        );
    }

    #[test]
    fn aprobar_con_abiertas_falla_salvo_que_se_cierren_todas() {
        let (mut p, c) = proyecto();
        p.observar(&c, "a", None).unwrap();
        p.observar(&c, "b", None).unwrap();
        let rev = p.revision;
        assert_eq!(
            p.aprobar_objeto(&c, None),
            Err(ErrorRevision::Abiertas {
                codigo: c.clone(),
                ids: vec![1, 2]
            })
        );
        assert_eq!(p.revision, rev, "un error no toca el journal");
        assert_eq!(p.node_by_code(&c).unwrap().estado, EstadoObjeto::Observado);
        assert_eq!(
            p.aprobar_objeto(&c, Some("  ")),
            Err(ErrorRevision::Abiertas {
                codigo: c.clone(),
                ids: vec![1, 2]
            }),
            "un cierre en blanco no vale"
        );
        assert_eq!(
            p.aprobar_objeto(&c, Some("Corregido en Civil 3D")),
            Ok(vec![1, 2])
        );
        assert_eq!(p.node_by_code(&c).unwrap().estado, EstadoObjeto::Aprobado);
        assert!(p.observaciones_de(&c).unwrap().iter().all(|o| !o.abierta()));
        assert_eq!(p.observaciones_abiertas(), 0);
    }

    #[test]
    fn reabrir_con_abiertas_falla_y_sin_ellas_vuelve_a_pendiente() {
        let (mut p, c) = proyecto();
        p.aprobar_objeto(&c, None).unwrap();
        p.reabrir_objeto(&c).unwrap();
        assert_eq!(p.node_by_code(&c).unwrap().estado, EstadoObjeto::Pendiente);
        p.observar(&c, "a", None).unwrap();
        assert!(matches!(
            p.reabrir_objeto(&c),
            Err(ErrorRevision::Abiertas { .. })
        ));
        assert!(matches!(
            p.observar("NO-EXISTE", "x", None),
            Err(ErrorRevision::NoExiste { .. })
        ));
    }

    #[test]
    fn la_ubicacion_se_guarda_y_vuelve() {
        let (mut p, c) = proyecto();
        let q = P3::new(1.0, 2.0, 3.0);
        p.observar(&c, "aquí", Some(q)).unwrap();
        let texto = serde_json::to_string(&p).unwrap();
        let q2: Project = serde_json::from_str(&texto).unwrap();
        assert_eq!(q2.observaciones_de(&c).unwrap()[0].ubicacion, Some(q));
    }
}
