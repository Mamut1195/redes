//! Reimportar una entrega nueva sin perder la revisión (ING-10, F13-06, D9).
//!
//! El revisor recibe otra versión del diseño (un `redes.plan/1` nuevo o el DXF de ida y vuelta) y
//! la carga **encima** de su proyecto. El diseño se sustituye entero por el de la entrega (Redes
//! no edita el diseño: lo que manda es el archivo de origen); la revisión se traslada por
//! **código**, con el mismo emparejamiento que `proyecto diff` ([`crate::diff`]):
//!
//! - objeto sin cambios de diseño → conserva estado y observaciones;
//! - objeto con cambios de diseño → conserva sus observaciones; si estaba **aprobado** vuelve a
//!   **pendiente** (se aprobó otra cosa). Un observado sigue observado y un pendiente, pendiente;
//! - un cambio que solo toca la **procedencia** de un dato (`datos.X.fuente`, p. ej. porque el
//!   archivo de origen se llama distinto) se informa pero no reinicia la aprobación;
//! - **alta** → entra pendiente;
//! - **baja** → desaparece del modelo; si tenía revisión (aprobado, observado u observaciones) no
//!   se pierde en silencio: el informe la trae completa y [`Informe::bloqueo`] exige aceptarla.
//!
//! [`preparar`] no toca el proyecto (sirve al `--dry-run`); [`Preparado::aplicar`] lo sustituye y
//! anota un único `commit` en el journal.

use std::path::Path;

use serde::Serialize;

use crate::diff::{self, Cambio, Clase, Presencia};
use crate::ingest::{self, ImportReport};
use crate::model::{Dato, EstadoObjeto, EstadoProyecto, Project};
use crate::revision::Observacion;

/// Qué se reimporta.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Formato {
    /// `redes.plan/1`.
    Plan,
    /// DXF exportado por Redes (capa `REDES_INFO`).
    Dxf,
}

impl Formato {
    pub fn key(self) -> &'static str {
        match self {
            Formato::Plan => "plan",
            Formato::Dxf => "dxf",
        }
    }

    /// Por la extensión del archivo: `.dxf` es DXF; lo demás, plan.
    pub fn por_extension(archivo: &Path) -> Formato {
        match archivo.extension().and_then(|e| e.to_str()) {
            Some(e) if e.eq_ignore_ascii_case("dxf") => Formato::Dxf,
            _ => Formato::Plan,
        }
    }
}

/// Objeto emparejado por código cuyo diseño cambió.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Cambiado {
    pub tipo: &'static str,
    pub codigo: String,
    pub familia: String,
    pub estado_antes: &'static str,
    pub estado_despues: &'static str,
    /// `conservada` o `reiniciada` (aprobado que vuelve a pendiente).
    pub revision: &'static str,
    /// Solo cambió la procedencia de algún dato: no reinicia la aprobación.
    pub solo_procedencia: bool,
    pub observaciones: usize,
    pub cambios: Vec<Cambio>,
}

/// Objeto que la entrega nueva ya no trae, con la revisión que tenía.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Baja {
    pub tipo: &'static str,
    pub codigo: String,
    pub familia: String,
    pub subtipo: String,
    pub estado: &'static str,
    pub observaciones: Vec<Observacion>,
}

impl Baja {
    /// ¿Se perdería algo de la revisión al darla de baja?
    pub fn con_revision(&self) -> bool {
        self.estado != EstadoObjeto::Pendiente.key() || !self.observaciones.is_empty()
    }
}

/// Informe de la reimportación.
#[derive(Clone, Debug, Serialize)]
pub struct Informe {
    pub formato: &'static str,
    /// Lo que dijo la ingesta de la entrega (descartes y avisos).
    pub importacion: ImportReport,
    pub altas: Vec<Presencia>,
    pub bajas: Vec<Baja>,
    pub cambiados: Vec<Cambiado>,
    /// Emparejados sin ningún cambio de diseño.
    pub sin_cambios: usize,
    /// Emparejados que conservan su estado tal cual.
    pub revision_conservada: usize,
    /// Códigos de los aprobados que vuelven a pendiente.
    pub reiniciados: Vec<String>,
    /// Observaciones trasladadas a la entrega nueva.
    pub observaciones_conservadas: usize,
    pub estado_proyecto_antes: &'static str,
    pub estado_proyecto_despues: &'static str,
}

impl Informe {
    pub fn bajas_con_revision(&self) -> impl Iterator<Item = &Baja> {
        self.bajas.iter().filter(|b| b.con_revision())
    }

    /// Por qué no se aplica: bajas con revisión sin `aceptar_bajas`.
    pub fn bloqueo(&self, aceptar_bajas: bool) -> Option<String> {
        let codigos: Vec<&str> = self
            .bajas_con_revision()
            .map(|b| b.codigo.as_str())
            .collect();
        (!aceptar_bajas && !codigos.is_empty()).then(|| {
            format!(
                "E_STATE_BAJAS_CON_REVISION: la entrega ya no trae {} objeto(s) con revisión ({}); su estado y sus observaciones se perderían",
                codigos.len(),
                codigos.join(", ")
            )
        })
    }

    /// ¿Cambió algo del diseño (altas, bajas o algún cambio que no sea solo de procedencia)?
    pub fn cambia_el_diseno(&self) -> bool {
        !self.altas.is_empty()
            || !self.bajas.is_empty()
            || self.cambiados.iter().any(|c| !c.solo_procedencia)
    }
}

/// Resultado de [`preparar`]: el proyecto como quedaría y el informe.
pub struct Preparado {
    pub proyecto: Project,
    pub informe: Informe,
    archivo: String,
}

impl Preparado {
    /// Sustituye el proyecto por la entrega con la revisión trasladada y anota el journal.
    pub fn aplicar(self, p: &mut Project) -> Informe {
        let mut nuevo = self.proyecto;
        let i = &self.informe;
        let mut msg = format!(
            "importar.reimportar {}: altas {}, bajas {}, cambiados {}, reiniciados {}",
            self.archivo,
            i.altas.len(),
            i.bajas.len(),
            i.cambiados.len(),
            i.reiniciados.len()
        );
        if !i.reiniciados.is_empty() {
            msg.push_str(&format!(" [{}]", i.reiniciados.join(", ")));
        }
        nuevo.commit(&msg);
        *p = nuevo;
        self.informe
    }
}

/// Importa la entrega en un proyecto aparte, la compara por código con `p` y traslada la
/// revisión. No modifica `p`.
pub fn preparar(
    p: &Project,
    archivo: &Path,
    formato: Formato,
    tolerancias: &diff::Opciones,
) -> Result<Preparado, String> {
    // La entrega entra en un proyecto vacío con el CRS y el terreno del actual: el terreno da la
    // cota de terreno que el plan no declara, igual que en la primera importación.
    let mut entrega = Project::new(&p.name, &p.crs);
    entrega.terrain = p.terrain.clone();
    entrega.vertical_datum = p.vertical_datum;
    let importacion = match formato {
        Formato::Plan => ingest::import_plan(&mut entrega, archivo)?,
        Formato::Dxf => ingest::import_dxf(&mut entrega, archivo, 0.0)?,
    };
    if p.count() > 0 && ingest::epsg_de(&entrega.crs) != ingest::epsg_de(&p.crs) {
        return Err(format!(
            "E_CRS: la entrega está en {} y el proyecto en {}; Redes no reproyecta",
            entrega.crs, p.crs
        ));
    }
    conservar_biblioteca(p, &mut entrega);

    let op = diff::Opciones {
        solo: Some(Clase::Diseno),
        ..*tolerancias
    };
    // Un proyecto vacío adopta el CRS de la entrega (como en la primera importación).
    let base_crs;
    let base = if p.crs == entrega.crs {
        p
    } else {
        base_crs = Project {
            crs: entrega.crs.clone(),
            ..p.clone()
        };
        &base_crs
    };
    let d = diff::comparar(base, &entrega, &op)?;

    let mut reiniciados = Vec::new();
    let mut cambiados = Vec::new();
    let mut observaciones_conservadas = 0usize;
    let mut emparejados = 0usize;

    for n in entrega.nodes.iter_mut().filter(|n| !n.deleted) {
        let Some(viejo) = p.nodo_codigo_exacto(&n.code) else {
            continue;
        };
        emparejados += 1;
        let m = d
            .modificados
            .iter()
            .find(|m| m.tipo == "nodo" && m.codigo == n.code);
        let (estado, c) = trasladar(viejo.estado, &viejo.observaciones, m, &mut reiniciados);
        n.estado = estado;
        n.observaciones = viejo.observaciones.clone();
        observaciones_conservadas += n.observaciones.len();
        cambiados.extend(c);
    }
    for l in entrega.links.iter_mut().filter(|l| !l.deleted) {
        let Some(viejo) = p.tramo_codigo_exacto(&l.code) else {
            continue;
        };
        emparejados += 1;
        let m = d
            .modificados
            .iter()
            .find(|m| m.tipo == "tramo" && m.codigo == l.code);
        let (estado, c) = trasladar(viejo.estado, &viejo.observaciones, m, &mut reiniciados);
        l.estado = estado;
        l.observaciones = viejo.observaciones.clone();
        observaciones_conservadas += l.observaciones.len();
        cambiados.extend(c);
    }

    let bajas: Vec<Baja> = d
        .bajas
        .iter()
        .map(|b| {
            let (estado, obs) = if b.tipo == "nodo" {
                p.node_by_code(&b.codigo)
                    .map(|n| (n.estado, n.observaciones.clone()))
            } else {
                p.link_by_code(&b.codigo)
                    .map(|l| (l.estado, l.observaciones.clone()))
            }
            .unwrap_or((EstadoObjeto::Pendiente, Vec::new()));
            Baja {
                tipo: b.tipo,
                codigo: b.codigo.clone(),
                familia: b.familia.clone(),
                subtipo: b.subtipo.clone(),
                estado: estado.key(),
                observaciones: obs,
            }
        })
        .collect();

    // El proyecto resultante: todo lo del actual (journal, revisión, entorno, biblioteca, reglas,
    // origen) con el diseño de la entrega.
    let mut nuevo = p.clone();
    nuevo.nodes = entrega.nodes;
    nuevo.links = entrega.links;
    nuevo.enlaces = entrega.enlaces;
    nuevo.next_slot = entrega.next_slot;
    nuevo.terrain = entrega.terrain;
    nuevo.vertical_datum = entrega.vertical_datum;
    nuevo.name = entrega.name;
    nuevo.crs = entrega.crs;
    nuevo.fuentes.extend(entrega.fuentes);
    // Los vectores son otros: el índice del proyecto actual ya no les corresponde (F14-01).
    nuevo.reindexar();

    let mut informe = Informe {
        formato: formato.key(),
        importacion,
        altas: d.altas,
        bajas,
        sin_cambios: emparejados - cambiados.len(),
        revision_conservada: emparejados - reiniciados.len(),
        cambiados,
        reiniciados,
        observaciones_conservadas,
        estado_proyecto_antes: p.estado.key(),
        estado_proyecto_despues: p.estado.key(),
    };
    // Un proyecto aprobado deja de estarlo si cambió el diseño que se aprobó.
    if p.estado == EstadoProyecto::Aprobado && informe.cambia_el_diseno() {
        nuevo.estado = EstadoProyecto::EnRevision;
    }
    informe.estado_proyecto_despues = nuevo.estado.key();

    let archivo = archivo
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| archivo.display().to_string());
    Ok(Preparado {
        proyecto: nuevo,
        informe,
        archivo,
    })
}

/// Reimporta y aplica en un paso. Error si la ingesta falla o si hay bajas con revisión y no se
/// aceptan; en ambos casos `p` queda intacto.
pub fn reimportar(
    p: &mut Project,
    archivo: &Path,
    formato: Formato,
    tolerancias: &diff::Opciones,
    aceptar_bajas: bool,
) -> Result<Informe, String> {
    let prep = preparar(p, archivo, formato, tolerancias)?;
    if let Some(e) = prep.informe.bloqueo(aceptar_bajas) {
        return Err(e);
    }
    Ok(prep.aplicar(p))
}

/// Estado que conserva un objeto emparejado y, si cambió su diseño, su entrada del informe.
fn trasladar(
    estado: EstadoObjeto,
    obs: &[Observacion],
    m: Option<&diff::Modificado>,
    reiniciados: &mut Vec<String>,
) -> (EstadoObjeto, Option<Cambiado>) {
    let Some(m) = m else {
        return (estado, None);
    };
    let solo_procedencia = m.cambios.iter().all(|c| c.campo.ends_with(".fuente"));
    let nuevo = if estado == EstadoObjeto::Aprobado && !solo_procedencia {
        reiniciados.push(m.codigo.clone());
        EstadoObjeto::Pendiente
    } else {
        estado
    };
    let c = Cambiado {
        tipo: m.tipo,
        codigo: m.codigo.clone(),
        familia: m.familia.clone(),
        estado_antes: estado.key(),
        estado_despues: nuevo.key(),
        revision: if nuevo == estado {
            "conservada"
        } else {
            "reiniciada"
        },
        solo_procedencia,
        observaciones: obs.len(),
        cambios: m.cambios.clone(),
    };
    (nuevo, Some(c))
}

/// Lo que puso `biblioteca aplicar` (el modelo 3D y sus medidas con fuente `modelo:<id>`) no es
/// del diseño: se traslada a la entrega si esta no lo trae, para que no cuente como cambio.
fn conservar_biblioteca(p: &Project, entrega: &mut Project) {
    let de_modelo = |d: &Dato| d.fuente.starts_with("modelo:");
    for n in entrega.nodes.iter_mut() {
        if let Some(v) = p.node_by_code(&n.code) {
            if n.modelo.is_none() {
                n.modelo = v.modelo.clone();
            }
            for (k, d) in v.datos.iter().filter(|(_, d)| de_modelo(d)) {
                n.datos.entry(k.clone()).or_insert_with(|| d.clone());
            }
        }
    }
    for l in entrega.links.iter_mut() {
        if let Some(v) = p.link_by_code(&l.code) {
            for (k, d) in v.datos.iter().filter(|(_, d)| de_modelo(d)) {
                l.datos.entry(k.clone()).or_insert_with(|| d.clone());
            }
        }
    }
}
