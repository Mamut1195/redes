//! CLI de **Redes**: visor 3D de redes ya diseñadas.
//!
//! No calcula ingeniería. Importa lo que producen otras aplicaciones, lo deja revisar y aprobar,
//! y lo devuelve al CAD. Toda salida es un sobre JSON `redes.envelope/1`.

mod diff;
mod observaciones;
mod perfil;
mod svg;
mod validar;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use clap::{CommandFactory, Parser, Subcommand};
use redes_core::envelope::{Diagnostic, ENGINE_VERSION, ENVELOPE_SCHEMA, Envelope, Severity};
use redes_core::export;
use redes_core::families::{self, caps};
use redes_core::ingest;
use redes_core::model::{EstadoProyecto, Project};
use redes_core::view::{self, Camera, Layers};
use serde_json::json;

#[derive(Parser)]
#[command(
    name = "redes",
    version,
    about = "Redes — visor 3D de redes de agua potable y alcantarillado (CLI para IA y humanos)",
    long_about = "Importa modelos de otras aplicaciones, los deja revisar y aprobar en 3D, y los\nentrega en DXF para el detallado. No calcula hidráulica, hidrología ni movimientos de tierra:\nesos números vienen en el archivo de origen y se muestran con su procedencia."
)]
struct Cli {
    /// Proyecto (`*.redes`). Si se omite: $REDES_PROJECT o búsqueda ascendente.
    #[arg(
        short = 'p',
        long,
        global = true,
        env = "REDES_PROJECT",
        value_name = "ARCHIVO"
    )]
    project: Option<PathBuf>,

    #[arg(long, global = true)]
    pretty: bool,

    /// Valida y reporta sin escribir.
    #[arg(long, global = true)]
    dry_run: bool,

    #[arg(long, global = true)]
    quiet: bool,

    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Introspección: familias, comandos y códigos de error.
    Esquema {
        #[arg(long)]
        familia: Option<String>,
    },
    /// Alias de `esquema`.
    Capacidades,
    Proyecto {
        #[command(subcommand)]
        cmd: ProyectoCmd,
    },
    /// Entrada de datos desde otras aplicaciones.
    Importar {
        #[command(subcommand)]
        cmd: ImportarCmd,
    },
    /// Consulta y revisión de objetos.
    Objeto {
        #[command(subcommand)]
        cmd: ObjetoCmd,
    },
    /// Salida hacia CAD y SIG.
    Exportar {
        #[command(subcommand)]
        cmd: ExportarCmd,
    },
    /// Biblioteca de modelos 3D (piezas hechas en Blender).
    Biblioteca {
        #[command(subcommand)]
        cmd: BibliotecaCmd,
    },
    /// Listado de cantidades: objetos, longitudes, volúmenes y pesos, con su procedencia.
    ///
    /// No valora: emite cantidades. La hoja de precios es de otra aplicación.
    Cantidades {
        /// `familia`, `longitud`, `volumen`, `area`, `peso` o `todo`.
        #[arg(long, default_value = "todo")]
        grupo: String,
        /// Escribe el listado en CSV (Excel lo abre con acentos).
        #[arg(long)]
        csv: Option<PathBuf>,
        /// Umbral de discrepancia declarado/medido, en porcentaje.
        #[arg(long, default_value_t = 10.0)]
        tolerancia: f64,
    },
    /// Perfil longitudinal de un colector (SVG y JSON): rasante, solera, clave, pozos, DN y pendiente.
    Perfil(perfil::PerfilArgs),
    /// Vista 3D del modelo en SVG (sin GPU, apta para agentes).
    Vista {
        #[arg(long)]
        salida: PathBuf,
        #[arg(long, default_value_t = -55.0)]
        azimut: f32,
        #[arg(long, default_value_t = 32.0)]
        elevacion: f32,
        /// Distancia de la cámara (0 = encuadrar automáticamente).
        #[arg(long, default_value_t = 0.0)]
        distancia: f64,
        #[arg(long, default_value_t = 1200.0)]
        ancho: f32,
        #[arg(long, default_value_t = 760.0)]
        alto: f32,
    },
    /// Validación geométrica de la red importada: informe de hallazgos (no es hidráulica ni norma).
    Validar(validar::ValidarArgs),
}

#[derive(Subcommand)]
enum ProyectoCmd {
    /// Crea un proyecto vacío.
    Nuevo {
        #[arg(long)]
        nombre: String,
        #[arg(long, default_value = "EPSG:32717")]
        crs: String,
        #[arg(long)]
        salida: Option<PathBuf>,
    },
    /// Crea el proyecto de demostración (red pequeña).
    Demo {
        #[arg(long)]
        salida: Option<PathBuf>,
    },
    /// Crea la ciudad de demostración: topografía montañosa, calles, manzanas y redes.
    Ciudad {
        #[arg(long)]
        salida: Option<PathBuf>,
    },
    /// Plantilla: terreno y tres pozos con sus colectores, para empezar a dibujar.
    Plantilla {
        #[arg(long)]
        salida: Option<PathBuf>,
    },
    /// Cadena completa: captación, conducción, planta, reservorio, distribución y alcantarillado.
    Cadena {
        #[arg(long)]
        salida: Option<PathBuf>,
    },
    /// Catálogo completo: una muestra de **todas** las familias del registro, en rejilla.
    ///
    /// Sirve como prueba de cobertura: si una familia no dibuja, se ve aquí.
    CatalogoTotal {
        #[arg(long)]
        salida: Option<PathBuf>,
    },
    /// Catálogo de piezas: una muestra de cada familia para ver el detalle de cerca.
    Catalogo {
        #[arg(long)]
        salida: Option<PathBuf>,
    },
    /// Resumen del proyecto.
    Info,
    /// Compara dos entregas: altas, bajas y cambios por objeto (emparejados por código).
    Diff {
        /// Entrega anterior.
        #[arg(long = "a")]
        a: PathBuf,
        /// Entrega nueva.
        #[arg(long = "b")]
        b: PathBuf,
        /// Movimiento en planta que se ignora (m).
        #[arg(long, default_value_t = 0.01)]
        tolerancia_pos: f64,
        /// Solo cambios de `diseno` o de `revision` (estado, nota).
        #[arg(long)]
        solo: Option<String>,
        /// Escribe una fila por cambio en CSV (BOM y `;`).
        #[arg(long)]
        salida: Option<PathBuf>,
    },
    /// Cierra la revisión: exige que no queden objetos pendientes.
    Aprobar,
}

#[derive(Subcommand)]
enum ImportarCmd {
    /// Plan JSON (`redes.plan/1`), el contrato que produce la skill de ingesta.
    Plan {
        #[arg(long)]
        archivo: PathBuf,
    },
    /// DXF exportado por Redes (capa `REDES_INFO`). Un DXF de otra aplicación se adapta con la
    /// skill `skills/redes-ingesta` y entra como plan.
    Dxf {
        #[arg(long)]
        archivo: PathBuf,
    },
    /// Puntos de terreno E,N,Z (CSV/TXT).
    Terreno {
        #[arg(long)]
        archivo: PathBuf,
        /// EPSG de los puntos (UTM sur). Si se omite, el del proyecto; otro distinto → `E_CRS`.
        #[arg(long)]
        epsg: Option<u32>,
    },
}

#[derive(Subcommand)]
enum ObjetoCmd {
    /// Lista objetos con su estado de revisión.
    Lista {
        #[arg(long)]
        familia: Option<String>,
        #[arg(long)]
        estado: Option<String>,
    },
    /// Revisión: aprobar, observar, cerrar observaciones, historial, reabrir (F13-03).
    #[command(flatten)]
    Revision(observaciones::RevisionCmd),
}

#[derive(Subcommand)]
enum BibliotecaCmd {
    /// Revisa un .glb: mallas, triángulos, medidas y avisos antes de integrarlo.
    Revisar {
        #[arg(long)]
        glb: PathBuf,
        /// Familia de destino (para validar la escala); por defecto, pieza genérica.
        #[arg(long)]
        familia: Option<String>,
    },
    /// Importa un .glb: lo normaliza (base en 0, centrado, escala) y lo anota en el manifiesto.
    Importar {
        #[arg(long)]
        glb: PathBuf,
        /// Familia a la que pertenece la pieza (`agua.valvula`, `saneamiento.pozo`…).
        #[arg(long)]
        familia: String,
        #[arg(long)]
        subtipo: Option<String>,
        #[arg(long)]
        dn: Option<u32>,
        /// Escala a aplicar (1.0 = dejar como viene).
        #[arg(long, default_value_t = 1.0)]
        escala: f64,
        /// Carpeta del proyecto (donde vive `biblioteca.json`); por defecto, el directorio actual.
        #[arg(long)]
        carpeta: Option<PathBuf>,
        /// Identificador de la pieza; por defecto, a partir del archivo.
        #[arg(long)]
        nombre: Option<String>,
        /// Licencia del modelo (MIT, CC-BY-4.0, propietaria…).
        #[arg(long)]
        licencia: Option<String>,
        /// Autor o estudio que modeló la pieza.
        #[arg(long)]
        autor: Option<String>,
    },
    /// Mide un .glb: volumen, área, materiales y datos declarados en Blender.
    Cantidades {
        #[arg(long)]
        glb: PathBuf,
        /// Densidad para el peso estimado (t/m³). 2,4 hormigón armado; 7,85 acero.
        #[arg(long, default_value_t = 2.4)]
        densidad: f64,
    },
    /// Lista las piezas de la biblioteca del proyecto.
    Lista {
        #[arg(long)]
        carpeta: Option<PathBuf>,
    },
    /// Asigna un modelo de la biblioteca a los objetos de una familia y les pasa sus medidas.
    ///
    /// Escribe `modelo` en cada objeto y, en `datos`, el volumen, el área y el peso con la fuente
    /// `modelo:<id>`. No pisa un dato que venga de otra fuente: lo avisa.
    Aplicar {
        /// Identificador de la pieza en `biblioteca.json`.
        #[arg(long)]
        modelo: String,
        /// Familia a la que se aplica (`agua.valvula`, `saneamiento.pozo`…).
        #[arg(long)]
        familia: String,
        /// Restringe a un subtipo.
        #[arg(long)]
        subtipo: Option<String>,
        /// Carpeta del proyecto (donde vive `biblioteca.json`).
        #[arg(long)]
        carpeta: Option<PathBuf>,
        /// Densidad para el peso (t/m³).
        #[arg(long, default_value_t = 2.4)]
        densidad: f64,
    },
}

#[derive(Subcommand)]
enum ExportarCmd {
    /// DXF con una capa por familia (para el detallado en CAD).
    Dxf {
        #[arg(long)]
        salida: PathBuf,
        /// Entrega solo los objetos aprobados.
        #[arg(long)]
        solo_aprobados: bool,
    },
    /// GeoJSON con los datos importados como propiedades.
    Geojson {
        #[arg(long)]
        salida: PathBuf,
    },
    /// glTF binario (.glb) para verlo en Blender, three.js o cualquier visor estándar.
    Gltf {
        #[arg(long)]
        salida: PathBuf,
        /// No incluir el terreno.
        #[arg(long)]
        sin_terreno: bool,
    },
}

fn main() {
    let cli = Cli::parse();
    let t0 = Instant::now();
    let code = run(cli, t0);
    std::process::exit(code);
}

fn run(cli: Cli, t0: Instant) -> i32 {
    let (pretty, quiet, dry) = (cli.pretty, cli.quiet, cli.dry_run);
    let cmd_name = command_id(&cli.cmd).to_string();

    // `biblioteca` no necesita proyecto: revisa un .glb suelto.
    if let Cmd::Biblioteca { cmd } = &cli.cmd
        && !matches!(cmd, BibliotecaCmd::Aplicar { .. })
    {
        return emit(biblioteca(cmd).within(elapsed(t0)), pretty, quiet);
    }

    match &cli.cmd {
        Cmd::Esquema { familia } => {
            return emit(
                esquema(familia.as_deref()).within(elapsed(t0)),
                pretty,
                quiet,
            );
        }
        Cmd::Capacidades => return emit(esquema(None).within(elapsed(t0)), pretty, quiet),
        Cmd::Proyecto { cmd } => match cmd {
            ProyectoCmd::Diff {
                a,
                b,
                tolerancia_pos,
                solo,
                salida,
            } => {
                let env = diff::ejecutar(
                    a,
                    b,
                    *tolerancia_pos,
                    solo.as_deref(),
                    salida.as_deref(),
                    dry,
                );
                return emit(env.within(elapsed(t0)), pretty, quiet);
            }
            ProyectoCmd::Nuevo {
                nombre,
                crs,
                salida,
            } => {
                let mut p = Project::new(nombre, crs);
                p.commit("proyecto.nuevo");
                let path = salida
                    .clone()
                    .unwrap_or_else(|| PathBuf::from(format!("{}.redes", slug(nombre))));
                if !dry && let Err(e) = redes_core::project_io::save(&p, &path) {
                    return emit(io_error("proyecto.nuevo", &e), pretty, quiet);
                }
                let env = Envelope::ok("proyecto.nuevo")
                    .with_project(&p, &path.display().to_string())
                    .with_data(json!({ "nombre": p.name, "crs": p.crs, "archivo": path.display().to_string() }))
                    .with_artifact("proyecto", &path.display().to_string())
                    .within(elapsed(t0));
                return emit(env, pretty, quiet);
            }
            ProyectoCmd::Demo { salida }
            | ProyectoCmd::Plantilla { salida }
            | ProyectoCmd::Ciudad { salida }
            | ProyectoCmd::Cadena { salida }
            | ProyectoCmd::CatalogoTotal { salida }
            | ProyectoCmd::Catalogo { salida } => {
                let nombre = match &cmd {
                    ProyectoCmd::Plantilla { .. } => "proyecto.plantilla",
                    ProyectoCmd::Plantilla { .. } => "proyecto.plantilla",
                    ProyectoCmd::Cadena { .. } => "proyecto.cadena",
                    ProyectoCmd::Ciudad { .. } => "proyecto.ciudad",
                    ProyectoCmd::CatalogoTotal { .. } => "proyecto.catalogo_total",
                    ProyectoCmd::Catalogo { .. } => "proyecto.catalogo",
                    _ => "proyecto.demo",
                };
                let es_ciudad = matches!(cmd, ProyectoCmd::Ciudad { .. });
                let es_catalogo = matches!(
                    cmd,
                    ProyectoCmd::Catalogo { .. } | ProyectoCmd::CatalogoTotal { .. }
                );
                let es_plantilla = matches!(cmd, ProyectoCmd::Plantilla { .. });
                let es_cadena = matches!(cmd, ProyectoCmd::Cadena { .. });
                let total = matches!(cmd, ProyectoCmd::CatalogoTotal { .. });
                let p = if es_plantilla {
                    redes_core::demo::plantilla()
                } else if es_cadena {
                    redes_core::demo_cadena::cadena()
                } else if es_ciudad {
                    redes_core::demo_city::ciudad()
                } else if total {
                    redes_core::demo_catalog::catalogo_completo()
                } else if es_catalogo {
                    redes_core::demo_catalog::catalogo()
                } else {
                    redes_core::demo::project()
                };
                let por_defecto = if es_plantilla {
                    "proyecto-nuevo.redes"
                } else if es_cadena {
                    "cadena-completa.redes"
                } else if es_ciudad {
                    "ciudad-san-bartolo.redes"
                } else if total {
                    "catalogo-total.redes"
                } else if es_catalogo {
                    "catalogo.redes"
                } else {
                    "la-esperanza.redes"
                };
                let path = salida.clone().unwrap_or_else(|| PathBuf::from(por_defecto));
                if !dry && let Err(e) = redes_core::project_io::save(&p, &path) {
                    return emit(io_error(nombre, &e), pretty, quiet);
                }
                let (pnd, ok, obs) = p.revision_counts();
                let env = Envelope::ok(nombre)
                    .with_project(&p, &path.display().to_string())
                    .with_data(json!({
                        "nombre": p.name,
                        "objetos": p.count(),
                        "revision": { "pendientes": pnd, "aprobados": ok, "observados": obs },
                        "archivo": path.display().to_string()
                    }))
                    .with_artifact("proyecto", &path.display().to_string())
                    .within(elapsed(t0));
                return emit(env, pretty, quiet);
            }
            _ => {}
        },
        _ => {}
    }

    let (mut p, path) = match open_project(&cli) {
        Ok(v) => v,
        Err(env) => return emit(env.within(elapsed(t0)), pretty, quiet),
    };

    let destructive = false;
    let mut env = match cli.cmd {
        Cmd::Cantidades {
            grupo,
            csv,
            tolerancia,
        } => {
            let mut filas = Vec::new();
            let todo = grupo == "todo" || grupo.is_empty();
            if todo || grupo == "familia" {
                filas.extend(redes_core::cantidades::por_familia(&p));
            }
            if todo || grupo == "longitud" || grupo == "longitudes" {
                filas.extend(redes_core::cantidades::longitudes(&p));
            }
            if todo || grupo == "volumen" || grupo == "area" {
                filas.extend(redes_core::cantidades::de_modelos(&p));
            }
            if todo || grupo == "peso" {
                filas.extend(redes_core::cantidades::pesos(&p));
            }
            // La biblioteca del proyecto permite comparar lo declarado con lo medido.
            let bib = p.biblioteca.as_ref().and_then(|ruta| {
                let base = path
                    .parent()
                    .map(|d| d.join(ruta))
                    .unwrap_or_else(|| PathBuf::from(ruta));
                redes_core::biblioteca::Biblioteca::cargar(&base).ok()
            });
            let discrepancias = redes_core::cantidades::discrepancias(&p, bib.as_ref(), tolerancia);
            let totales: Vec<_> = redes_core::cantidades::totales(&filas)
                .into_iter()
                .map(|(g, u, c)| json!({ "grupo": g, "unidad": u, "cantidad": round(c, 3) }))
                .collect();
            let mut env = Envelope::ok("cantidades")
                .with_project(&p, &path.display().to_string())
                .with_data(json!({
                    "filas": filas.iter().map(|f| json!({
                        "grupo": f.grupo,
                        "clave": f.clave,
                        "unidad": f.unidad,
                        "cantidad": round(f.cantidad, 3),
                        "fuente": f.fuente,
                        "cantidad_3d": f.cantidad_3d.map(|v| round(v, 3)),
                    })).collect::<Vec<_>>(),
                    "totales": totales,
                    "discrepancias": discrepancias.iter().map(|d| json!({
                        "objeto": d.codigo,
                        "familia": d.familia,
                        "declarado": round(d.declarado, 4),
                        "medido": round(d.medido, 4),
                        "desvio_pct": round(d.desvio_pct, 2),
                    })).collect::<Vec<_>>(),
                    "precios": "fuera de alcance: la app emite cantidades, no valoraciones",
                }))
                .within(elapsed(t0));
            if let Some(destino) = csv {
                let texto = redes_core::cantidades::a_csv(&filas);
                if !dry && let Err(e) = std::fs::write(&destino, texto) {
                    return emit(io_error("cantidades", &e.to_string()), pretty, quiet);
                }
                env = env.with_artifact("csv", &destino.display().to_string());
            }
            env
        }
        Cmd::Biblioteca {
            cmd:
                BibliotecaCmd::Aplicar {
                    modelo,
                    familia,
                    subtipo,
                    carpeta,
                    densidad,
                },
        } => {
            let dir = carpeta.unwrap_or_else(|| {
                path.parent()
                    .map(|d| d.to_path_buf())
                    .unwrap_or_else(|| PathBuf::from("."))
            });
            let ruta = dir.join("biblioteca.json");
            let bib = match redes_core::biblioteca::Biblioteca::cargar(&ruta) {
                Ok(b) => b,
                Err(e) => return emit(io_error("biblioteca.aplicar", &e), pretty, quiet),
            };
            match redes_core::biblioteca::aplicar(
                &mut p,
                &bib,
                &modelo,
                &familia,
                subtipo.as_deref(),
                densidad,
            ) {
                Ok(a) => {
                    // El proyecto recuerda dónde está su biblioteca para que el visor sepa qué
                    // .glb cargar; se guarda relativa a la carpeta del proyecto.
                    p.biblioteca = Some(
                        ruta.strip_prefix(dir_parent(&path))
                            .unwrap_or(&ruta)
                            .to_string_lossy()
                            .to_string(),
                    );
                    // Si el modelo declara anclas, los arcos entran por el puerto y no por el
                    // centro del pozo (F6-08).
                    let mut movidos = 0usize;
                    if let Some(pieza) = bib.piezas.iter().find(|x| x.id == modelo) {
                        let codigos: Vec<String> = p
                            .nodes
                            .iter()
                            .filter(|n| !n.deleted && n.family == familia)
                            .filter(|n| subtipo.as_deref().is_none_or(|s| n.ty == s))
                            .map(|n| n.code.clone())
                            .collect();
                        for code in codigos {
                            movidos += redes_core::biblioteca::anclar_conexiones(
                                &mut p,
                                &code,
                                &pieza.anclas,
                            );
                        }
                    }
                    if !dry && let Err(e) = redes_core::project_io::save(&p, &path) {
                        return emit(io_error("biblioteca.aplicar", &e), pretty, quiet);
                    }
                    Envelope::ok("biblioteca.aplicar")
                        .with_project(&p, &path.display().to_string())
                        .with_data(json!({
                            "modelo": modelo,
                            "familia": familia,
                            "subtipo": subtipo,
                            "aplicados": a.aplicados,
                            "conservados_por_otra_fuente": a.conservados,
                            "volumen_m3": round(a.volumen_m3, 4),
                            "area_m2": round(a.area_m2, 3),
                            "peso_t": round(a.peso_t, 3),
                            "guardado": !dry,
                        }))
                        .with_artifact("proyecto", &path.display().to_string())
                        .within(elapsed(t0))
                }
                Err(e) => io_error("biblioteca.aplicar", &e),
            }
        }
        Cmd::Proyecto {
            cmd: ProyectoCmd::Info,
        } => project_info(&p, &path),
        Cmd::Proyecto {
            cmd: ProyectoCmd::Aprobar,
        } => {
            let (pnd, _, obs) = p.revision_counts();
            if pnd == 0 && obs > 0 {
                Envelope::fail(
                    "proyecto.aprobar",
                    vec![
                        Diagnostic::new(
                            "E_STATE_OBSERVED",
                            Severity::Error,
                            format!(
                                "Quedan {obs} objetos observados sin resolver ({} observaciones abiertas).",
                                p.observaciones_abiertas()
                            ),
                        )
                        .hint(
                            "Use `redes objeto lista --estado observado` y `objeto observaciones --codigo X`; cierre cada una con `objeto cerrar-observacion` o apruebe con `--cerrar-todas`.",
                        ),
                    ],
                )
            } else if pnd > 0 {
                Envelope::fail(
                    "proyecto.aprobar",
                    vec![
                        Diagnostic::new(
                            "E_STATE_PENDING",
                            Severity::Error,
                            format!("Quedan {pnd} objetos sin revisar."),
                        )
                        .hint(
                            "Use `redes objeto lista --estado pendiente` y apruébelos uno a uno.",
                        ),
                    ],
                )
            } else if dry {
                Envelope::ok("proyecto.aprobar").with_data(json!({ "dry_run": true }))
            } else {
                p.estado = EstadoProyecto::Aprobado;
                p.commit("proyecto.aprobar");
                Envelope::ok("proyecto.aprobar").with_data(json!({ "estado": p.estado.key() }))
            }
        }
        Cmd::Importar { cmd } => importar(&mut p, cmd, dry),
        Cmd::Objeto { cmd } => objeto(&mut p, cmd, dry),
        Cmd::Exportar { cmd } => exportar(&p, cmd, dry),
        Cmd::Biblioteca { cmd } => biblioteca(&cmd),
        Cmd::Perfil(a) => perfil::ejecutar(&p, &a, dry),
        Cmd::Validar(args) => validar::ejecutar(&mut p, &path, &args, dry),
        Cmd::Vista {
            salida,
            azimut,
            elevacion,
            distancia,
            ancho,
            alto,
        } => {
            if dry {
                let env = Envelope::ok("vista").with_data(json!({
                    "dry_run": true,
                    "salida": salida.display().to_string(),
                }));
                return emit(env, pretty, quiet);
            }
            let mut cam = Camera::default();
            cam.viewport = [ancho, alto];
            cam.fit(&p);
            cam.azimuth = azimut;
            cam.elevation = elevacion;
            if distancia > 0.0 {
                cam.distance = distancia;
            }
            let scene = view::build(&p, &cam, Layers::default(), None);
            let text = svg::render(&scene, cam.viewport);
            if let Some(dir) = salida.parent()
                && !dir.as_os_str().is_empty()
            {
                let _ = std::fs::create_dir_all(dir);
            }
            match std::fs::write(&salida, text) {
                Ok(()) => Envelope::ok("vista")
                    .with_data(json!({
                        "salida": salida.display().to_string(),
                        "primitivas": scene.prims.len(),
                        "triangulos": scene.tri_count,
                        "lineas": scene.line_count,
                        "camara": { "azimut": cam.azimuth, "elevacion": cam.elevation, "distancia": cam.distance }
                    }))
                    .with_artifact("vista", &salida.display().to_string()),
                Err(e) => io_error("vista", &format!("E_IO_PERMISSION: {e}")),
            }
        }
        _ => unreachable!(),
    };
    let writes = mutates(&cmd_name);
    env.command = cmd_name;
    env = env.with_project(&p, &path.display().to_string());

    if !dry
        && env.ok
        && writes
        && !destructive
        && let Err(e) = redes_core::project_io::save(&p, &path)
    {
        return emit(io_error(&env.command.clone(), &e), pretty, quiet);
    }
    emit(env.within(elapsed(t0)), pretty, quiet)
}

// ---------------------------------------------------------------------------------------------

/// Revisa un `.glb` de la biblioteca de modelos (no necesita proyecto).
/// Importa un `.glb` a la biblioteca del proyecto.
#[allow(clippy::too_many_arguments)]
fn biblioteca_importar(
    glb: &PathBuf,
    familia: &str,
    subtipo: Option<&str>,
    dn: Option<u32>,
    escala: f64,
    carpeta: Option<&std::path::Path>,
    nombre: Option<&str>,
    licencia: Option<&str>,
    autor: Option<&str>,
) -> Envelope {
    let bytes = match std::fs::read(glb) {
        Ok(b) => b,
        Err(e) => return io_error("biblioteca.importar", &e.to_string()),
    };
    let color = redes_core::biblioteca::color_sugerido(familia);
    let (norm, info) = match redes_core::biblioteca::normalizar_glb(&bytes, escala, color) {
        Ok(v) => v,
        Err(e) => return io_error("biblioteca.importar", &format!("E_GLB_INVALIDO: {e}")),
    };

    let dir = carpeta
        .map(|c| c.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));
    let id = nombre.map(slug).unwrap_or_else(|| {
        let base = glb.file_stem().and_then(|s| s.to_str()).unwrap_or("pieza");
        slug(base)
    });
    let destino_dir = dir.join("biblioteca");
    if let Err(e) = std::fs::create_dir_all(&destino_dir) {
        return io_error("biblioteca.importar", &e.to_string());
    }
    let archivo = destino_dir.join(format!("{id}.glb"));
    if let Err(e) = std::fs::write(&archivo, &norm) {
        return io_error("biblioteca.importar", &e.to_string());
    }

    let ruta_manifiesto = dir.join("biblioteca.json");
    let mut bib = match redes_core::biblioteca::Biblioteca::cargar(&ruta_manifiesto) {
        Ok(b) => b,
        Err(e) => return io_error("biblioteca.importar", &e),
    };
    let dims = info.bbox.dimensiones();
    let medidas = redes_core::biblioteca::cantidades_glb(&norm).ok();
    let extras = redes_core::biblioteca::leer_glb(&norm)
        .map(|m| m.extras)
        .unwrap_or_default();
    let entrada = redes_core::biblioteca::Entrada {
        id: id.clone(),
        familia: familia.to_string(),
        subtipo: subtipo.map(|s| s.to_string()),
        dn_mm: dn,
        archivo: format!("biblioteca/{id}.glb"),
        escala: info.escala,
        triangulos: info.triangulos,
        dimensiones_m: [round(dims[0], 3), round(dims[1], 3), round(dims[2], 3)],
        bytes: norm.len(),
        volumen_m3: medidas.as_ref().map(|c| round(c.volumen_m3.abs(), 4)),
        area_m2: medidas.as_ref().map(|c| round(c.area_m2, 3)),
        cerrada: medidas.as_ref().map(|c| c.cerrada()),
        extras,
        anclas: redes_core::biblioteca::anclas_de_glb(&bytes).unwrap_or_default(),
        licencia: licencia.map(|s| s.to_string()),
        autor: autor.map(|s| s.to_string()),
    };
    bib.insertar(entrada.clone());
    if let Err(e) = bib.guardar(&ruta_manifiesto) {
        return io_error("biblioteca.importar", &e);
    }

    let mut env = Envelope::ok("biblioteca.importar")
        .with_data(json!({
            "id": entrada.id,
            "familia": entrada.familia,
            "subtipo": entrada.subtipo,
            "dn_mm": entrada.dn_mm,
            "archivo": archivo.display().to_string(),
            "manifiesto": ruta_manifiesto.display().to_string(),
            "escala": entrada.escala,
            "triangulos": entrada.triangulos,
            "dimensiones_m": entrada.dimensiones_m,
            "bytes": entrada.bytes,
            "centro_previo": [round(info.centro_previo[0], 3), round(info.centro_previo[1], 3), round(info.centro_previo[2], 3)],
            "base_previa": round(info.base_previa, 3),
            "volumen_m3": entrada.volumen_m3,
            "area_m2": entrada.area_m2,
            "cerrada": entrada.cerrada,
            "extras": entrada.extras,
        }))
        .with_artifact("glb", &archivo.display().to_string());
    if info.triangulos > 60_000 {
        env = env.with_diagnostics(vec![Diagnostic::new(
            "W_GLB_PESADO",
            Severity::Warning,
            format!(
                "{} triángulos: el visor 2D irá lento; conviene usar --gpu3d",
                info.triangulos
            ),
        )]);
    }
    env
}

fn biblioteca(cmd: &BibliotecaCmd) -> Envelope {
    match cmd {
        BibliotecaCmd::Importar {
            glb,
            familia,
            subtipo,
            dn,
            escala,
            carpeta,
            nombre,
            licencia,
            autor,
        } => biblioteca_importar(
            glb,
            familia,
            subtipo.as_deref(),
            *dn,
            *escala,
            carpeta.as_deref(),
            nombre.as_deref(),
            licencia.as_deref(),
            autor.as_deref(),
        ),
        // `aplicar` necesita el proyecto abierto: se resuelve en el flujo principal.
        BibliotecaCmd::Aplicar { .. } => Envelope::fail(
            "biblioteca.aplicar",
            vec![Diagnostic::new(
                "E_ARG_MISSING",
                Severity::Error,
                "biblioteca aplicar necesita un proyecto: use -p <archivo.redes>",
            )],
        ),
        BibliotecaCmd::Lista { carpeta } => {
            let dir = carpeta.clone().unwrap_or_else(|| PathBuf::from("."));
            let ruta = dir.join("biblioteca.json");
            match redes_core::biblioteca::Biblioteca::cargar(&ruta) {
                Ok(b) => {
                    let sin_licencia = b
                        .piezas
                        .iter()
                        .filter(|x| x.licencia.as_deref().unwrap_or("").trim().is_empty())
                        .count();
                    Envelope::ok("biblioteca.lista").with_data(json!({
                        "manifiesto": ruta.display().to_string(),
                        "piezas": b.piezas,
                        "sin_licencia": sin_licencia,
                        "aviso_licencias": if sin_licencia > 0 {
                            format!("{sin_licencia} piezas no declaran licencia: anótela al importar con --licencia")
                        } else {
                            String::new()
                        },
                    }))
                }
                Err(e) => io_error("biblioteca.lista", &e),
            }
        }
        BibliotecaCmd::Revisar { glb, familia } => biblioteca_revisar(glb, familia.as_deref()),
        BibliotecaCmd::Cantidades { glb, densidad } => biblioteca_cantidades(glb, *densidad),
    }
}

/// Mide un `.glb`: cantidades y datos maestros declarados.
fn biblioteca_cantidades(glb: &PathBuf, densidad: f64) -> Envelope {
    let bytes = match std::fs::read(glb) {
        Ok(b) => b,
        Err(e) => return io_error("biblioteca.cantidades", &e.to_string()),
    };
    let modelo = match redes_core::biblioteca::leer_glb(&bytes) {
        Ok(m) => m,
        Err(e) => return io_error("biblioteca.cantidades", &format!("E_GLB_INVALIDO: {e}")),
    };
    let c = match redes_core::biblioteca::cantidades_glb(&bytes) {
        Ok(c) => c,
        Err(e) => return io_error("biblioteca.cantidades", &format!("E_GLB_INVALIDO: {e}")),
    };
    let dims = modelo.bbox.dimensiones();
    let por_material: Vec<_> = c
        .por_material
        .iter()
        .map(|(k, v)| json!({ "material": k, "volumen_m3": round(v.abs(), 4) }))
        .collect();
    Envelope::ok("biblioteca.cantidades").with_data(json!({
        "archivo": glb.display().to_string(),
        "triangulos": modelo.triangulos,
        "volumen_m3": round(c.volumen_m3.abs(), 4),
        "area_m2": round(c.area_m2, 3),
        "peso_t": round(c.peso_t(densidad), 3),
        "densidad_t_m3": densidad,
        "cerrada": c.cerrada(),
        "aristas_abiertas": c.aristas_abiertas,
        "dimensiones_m": [round(dims[0], 3), round(dims[1], 3), round(dims[2], 3)],
        "por_material": por_material,
        "extras": modelo.extras,
    }))
}

/// Revisa un `.glb` sin integrarlo.
fn biblioteca_revisar(glb: &PathBuf, familia: Option<&str>) -> Envelope {
    let bytes = match std::fs::read(glb) {
        Ok(b) => b,
        Err(e) => return io_error("biblioteca.revisar", &e.to_string()),
    };
    match redes_core::biblioteca::leer_glb(&bytes) {
        Ok(modelo) => {
            let esperado = match familia {
                Some(f) if f.contains("planta") => redes_core::biblioteca::Expectativa::planta(),
                _ => redes_core::biblioteca::Expectativa::pieza(),
            };
            let avisos = redes_core::biblioteca::revisar(&modelo, esperado);
            let errores = avisos
                .iter()
                .filter(|a| a.gravedad == redes_core::biblioteca::Gravedad::Error)
                .count();
            let dims = modelo.bbox.dimensiones();
            let mut env = Envelope::ok("biblioteca.revisar").with_data(json!({
                "archivo": glb.display().to_string(),
                "bytes": bytes.len(),
                "primitivas": modelo.primitivas.len(),
                "triangulos": modelo.triangulos,
                "materiales": modelo.materiales,
                "dimensiones_m": [round(dims[0], 3), round(dims[1], 3), round(dims[2], 3)],
                "base_z": round(modelo.bbox.min[2], 3),
                "giro_y_arriba": modelo.giro.is_some(),
                "anclas": redes_core::biblioteca::anclas_de_glb(&bytes).unwrap_or_default(),
                "errores": errores,
                "avisos": avisos.iter().map(|a| json!({
                    "gravedad": match a.gravedad {
                        redes_core::biblioteca::Gravedad::Error => "error",
                        redes_core::biblioteca::Gravedad::Aviso => "aviso",
                        redes_core::biblioteca::Gravedad::Info => "info",
                    },
                    "texto": a.texto,
                })).collect::<Vec<_>>(),
            }));
            if errores > 0 {
                env = env.with_diagnostics(
                    avisos
                        .iter()
                        .filter(|a| a.gravedad == redes_core::biblioteca::Gravedad::Error)
                        .map(|a| {
                            Diagnostic::new("E_GLB_INVALIDO", Severity::Error, a.texto.clone())
                                .hint(
                                    "Ajuste la escala o el origen en Blender y vuelva a exportar.",
                                )
                        })
                        .collect(),
                );
            }
            env
        }
        Err(e) => io_error("biblioteca.revisar", &format!("E_GLB_INVALIDO: {e}")),
    }
}

fn emit(env: Envelope, pretty: bool, quiet: bool) -> i32 {
    if !quiet {
        println!("{}", env.to_json(pretty));
    }
    env.exit_code()
}

fn elapsed(t0: Instant) -> u64 {
    t0.elapsed().as_millis() as u64
}

fn mutates(id: &str) -> bool {
    matches!(
        id,
        "importar.plan" | "importar.dxf" | "importar.terreno" | "proyecto.aprobar"
    ) || observaciones::muta(id)
}

fn io_error(command: &str, msg: &str) -> Envelope {
    let code = msg.split(':').next().unwrap_or("E_IO_NOT_FOUND");
    Envelope::fail(
        command,
        vec![
            Diagnostic::new(code, Severity::Error, msg.to_string())
                .hint("Verifique la ruta; use --dry-run para inspeccionar sin escribir."),
        ],
    )
}

fn command_id(c: &Cmd) -> &'static str {
    match c {
        Cmd::Esquema { .. } => "esquema",
        Cmd::Capacidades => "capacidades",
        Cmd::Proyecto { cmd } => match cmd {
            ProyectoCmd::Nuevo { .. } => "proyecto.nuevo",
            ProyectoCmd::Demo { .. } => "proyecto.demo",
            ProyectoCmd::Plantilla { .. } => "proyecto.plantilla",
            ProyectoCmd::Cadena { .. } => "proyecto.cadena",
            ProyectoCmd::Ciudad { .. } => "proyecto.ciudad",
            ProyectoCmd::CatalogoTotal { .. } => "proyecto.catalogo_total",
            ProyectoCmd::Catalogo { .. } => "proyecto.catalogo",
            ProyectoCmd::Info => "proyecto.info",
            ProyectoCmd::Diff { .. } => "proyecto.diff",
            ProyectoCmd::Aprobar => "proyecto.aprobar",
        },
        Cmd::Importar { cmd } => match cmd {
            ImportarCmd::Plan { .. } => "importar.plan",
            ImportarCmd::Dxf { .. } => "importar.dxf",
            ImportarCmd::Terreno { .. } => "importar.terreno",
        },
        Cmd::Objeto { cmd } => match cmd {
            ObjetoCmd::Lista { .. } => "objeto.lista",
            ObjetoCmd::Revision(c) => c.id(),
        },
        Cmd::Exportar { cmd } => match cmd {
            ExportarCmd::Dxf { .. } => "exportar.dxf",
            ExportarCmd::Geojson { .. } => "exportar.geojson",
            ExportarCmd::Gltf { .. } => "exportar.gltf",
        },
        Cmd::Cantidades { .. } => "cantidades",
        Cmd::Perfil(_) => "perfil",
        Cmd::Vista { .. } => "vista",
        Cmd::Validar(_) => "validar",
        Cmd::Biblioteca { cmd } => match cmd {
            BibliotecaCmd::Revisar { .. } => "biblioteca.revisar",
            BibliotecaCmd::Importar { .. } => "biblioteca.importar",
            BibliotecaCmd::Lista { .. } => "biblioteca.lista",
            BibliotecaCmd::Aplicar { .. } => "biblioteca.aplicar",
            BibliotecaCmd::Cantidades { .. } => "biblioteca.cantidades",
        },
    }
}

fn open_project(cli: &Cli) -> Result<(Project, PathBuf), Envelope> {
    let path = cli.project.clone().or_else(|| {
        std::env::current_dir()
            .ok()
            .and_then(|d| redes_core::project_io::find_upwards(&d))
    });
    let Some(path) = path else {
        return Err(Envelope::fail(
            "proyecto.abrir",
            vec![Diagnostic::new(
                "E_PROJ_NOT_FOUND",
                Severity::Error,
                "No hay proyecto: ni --project, ni $REDES_PROJECT, ni un *.redes en esta carpeta o sus padres.",
            )
            .hint("Cree uno con `redes proyecto demo --salida la-esperanza.redes`.")],
        ));
    };
    match redes_core::project_io::load(&path) {
        Ok(p) => Ok((p, path)),
        Err(e) => Err(io_error("proyecto.abrir", &e)),
    }
}

fn esquema(filtro: Option<&str>) -> Envelope {
    let mut fams = Vec::new();
    for f in families::families() {
        if let Some(k) = filtro
            && !f.key.contains(k)
        {
            continue;
        }
        let attrs: Vec<serde_json::Value> = f
            .attrs
            .iter()
            .map(|a| {
                json!({
                    "clave": a.key,
                    "etiqueta": a.label,
                    "unidad": a.unit.key(),
                    "tipo": a.ty.key(),
                    "requerido": a.required,
                    "grupo": a.group.key(),
                    // Lista cerrada: los valores admitidos, para no adivinarlos (F12-15).
                    "valores": match a.ty {
                        families::AttrType::Enum(v) => json!(v),
                        _ => serde_json::Value::Null,
                    },
                })
            })
            .collect();
        let types: Vec<serde_json::Value> = f
            .types
            .iter()
            .map(|t| json!({ "clave": t.key, "etiqueta": t.label, "requeridos": t.required }))
            .collect();
        fams.push(json!({
            "clave": f.key,
            "etiqueta": f.label,
            "categoria": f.category.key(),
            "rol": f.topo.key(),
            "geometria": f.geom.key(),
            "prefijo": f.code_prefix,
            "capacidades": caps::names(f.caps),
            "atributos": attrs,
            "subtipos": types,
        }));
    }

    let root = Cli::command();
    let mut comandos = Vec::new();
    for sc in root.get_subcommands() {
        let leaves: Vec<_> = sc.get_subcommands().collect();
        if leaves.is_empty() {
            comandos.push(json!({ "nombre": sc.get_name(), "descripcion": about(sc) }));
        }
        for leaf in leaves {
            comandos.push(json!({
                "nombre": format!("{} {}", sc.get_name(), leaf.get_name()),
                "descripcion": about(leaf),
                "argumentos": leaf.get_arguments().filter(|a| a.get_id() != "help").map(|a| json!({
                    "nombre": a.get_long().unwrap_or(a.get_id().as_str()),
                    "requerido": a.is_required_set(),
                    "ayuda": a.get_help().map(|h| h.to_string()).unwrap_or_default(),
                })).collect::<Vec<_>>()
            }));
        }
    }

    Envelope::ok("esquema").with_data(json!({
        "motor": ENGINE_VERSION,
        "sobre": ENVELOPE_SCHEMA,
        "alcance": "visualización 3D y revisión; no calcula hidráulica, hidrología ni tierras",
        "familias": fams,
        "total_familias": families::families().len(),
        "comandos": comandos,
        "formatos": {
            "entrada": ["redes.plan/1", "dxf (exportado por Redes, capa REDES_INFO)", "csv(E,N,Z)"],
            "salida": ["dxf", "geojson", "svg"]
        },
        "codigos_error": CODIGOS.iter().chain(observaciones::CODIGOS).collect::<Vec<_>>()
    }))
}

fn about(c: &clap::Command) -> String {
    c.get_about().map(|a| a.to_string()).unwrap_or_default()
}

const CODIGOS: &[(&str, &str, i32)] = &[
    ("E_ARG_MISSING", "falta un argumento requerido", 2),
    ("E_ARG_INVALID", "valor de argumento no admitido", 2),
    (
        "E_STATE_CRS_DISTINTO",
        "las dos entregas usan CRS distinto: no se comparan",
        3,
    ),
    ("E_PROJ_NOT_FOUND", "no hay proyecto resoluble", 4),
    ("E_STATE_NOT_FOUND", "el objeto no existe", 4),
    ("E_STATE_DUPLICATE_NAME", "código duplicado", 3),
    ("E_STATE_PENDING", "quedan objetos sin revisar", 3),
    (
        "E_STATE_OBSERVED",
        "quedan objetos observados sin resolver",
        3,
    ),
    (
        "E_STATE_REFERENCED",
        "borrar dejaría referencias colgando",
        3,
    ),
    ("E_TOPO_ORPHAN_LINK", "arco con extremo inexistente", 3),
    (
        "E_TOPO_SELF_LOOP",
        "arco con el mismo nodo en ambos extremos",
        3,
    ),
    (
        "E_TOPO_NO_CONSECUTIVO",
        "perfil: los tramos no se siguen en el sentido declarado",
        3,
    ),
    (
        "E_TOPO_SIN_RUTA",
        "perfil: no hay recorrido entre los pozos siguiendo el sentido declarado",
        3,
    ),
    ("E_FMT_PARSE", "archivo ilegible", 7),
    ("E_FMT_UNSUPPORTED", "formato no soportado", 7),
    (
        "E_FMT_CAMPO",
        "el plan usa un campo que no es del contrato redes.plan/1 (se perdería); el mensaje sugiere el válido",
        7,
    ),
    (
        "E_FMT_NOT_REDES",
        "DXF que no exportó Redes (sin capa REDES_INFO): adáptelo con la skill de ingesta",
        7,
    ),
    ("E_IO_NOT_FOUND", "archivo inexistente", 7),
    (
        "E_CRS",
        "CRS ausente, no admitido (solo UTM sur EPSG:327NN), distinto del proyecto o coordenadas en grados",
        1,
    ),
];

fn project_info(p: &Project, path: &Path) -> Envelope {
    let (pnd, ok, obs) = p.revision_counts();
    let bbox = p.bbox();
    let (z0, z1) = p.z_range();
    Envelope::ok("proyecto.info").with_data(json!({
        "nombre": p.name,
        "crs": p.crs,
        "estado": p.estado.key(),
        "revision": p.revision,
        "archivo": path.display().to_string(),
        "objetos": p.count(),
        "nodos": p.nodes.iter().filter(|n| !n.deleted).count(),
        "arcos": p.links.iter().filter(|l| !l.deleted).count(),
        "revision_objetos": { "pendientes": pnd, "aprobados": ok, "observados": obs },
        "terreno": p.terrain.as_ref().map(|t| json!({
            "nombre": t.name, "puntos": t.points.len(), "triangulos": t.tris.len()
        })),
        "bbox": bbox.to_array(),
        "cota_min": round(z0, 3),
        "cota_max": round(z1, 3),
        "fuentes": p.fuentes.iter().map(|f| json!({
            "app": f.app, "archivo": f.archivo, "formato": f.formato, "importado_en": f.importado_en
        })).collect::<Vec<_>>(),
    }))
}

fn importar(p: &mut Project, cmd: ImportarCmd, dry: bool) -> Envelope {
    // Con `--dry-run` se importa de verdad, pero en una copia en memoria (F12-15): el informe
    // (altas, descartes con motivo, avisos, errores) es el mismo y el proyecto no se escribe.
    let mut copia;
    let p: &mut Project = if dry {
        copia = p.clone();
        &mut copia
    } else {
        p
    };
    let (command, result) = match cmd {
        ImportarCmd::Plan { archivo } => ("importar.plan", ingest::import_plan(p, &archivo)),
        ImportarCmd::Dxf { archivo } => ("importar.dxf", ingest::import_dxf(p, &archivo, 0.0)),
        ImportarCmd::Terreno { archivo, epsg } => {
            // Sin `--epsg`, el del proyecto (F12-13): antes se suponía 32717 aunque el proyecto
            // estuviera en 18S.
            let epsg = epsg.or_else(|| ingest::epsg_de(&p.crs));
            let resultado = match epsg {
                Some(epsg) => ingest::import_terreno(p, &archivo, epsg),
                None => Err(format!(
                    "E_CRS: el proyecto tiene un CRS ilegible ({}); indique --epsg",
                    p.crs
                )),
            };
            ("importar.terreno", resultado)
        }
    };
    match result {
        Ok(rep) => Envelope::ok(command).with_data(json!({
            "formato": rep.formato,
            "nodos": rep.nodos,
            "arcos": rep.arcos,
            "puntos_terreno": rep.puntos_terreno,
            "descartados": rep.descartados.len(),
            "detalle_descartes": rep.descartados,
            "renombrados": rep.renombrados,
            "avisos": rep.avisos.len(),
            "detalle_avisos": rep.avisos,
            "objetos_totales": p.count(),
            "dry_run": dry,
            "guardado": !dry,
            "siguiente": if dry {
                "sin --dry-run para importar"
            } else {
                "redes vista --salida vista.svg"
            }
        })),
        Err(e) => {
            let code = e.split(':').next().unwrap_or("E_FMT_PARSE").to_string();
            let pista = match code.as_str() {
                "E_FMT_NOT_REDES" | "E_FMT_PARSE" if command == "importar.dxf" => {
                    "`importar dxf` solo lee DXF exportados por Redes (capa REDES_INFO). Un DXF de otra aplicación se adapta con la skill `skills/redes-ingesta`, que escribe un `redes.plan/1`; luego `redes importar plan --archivo plan.json`."
                }
                "E_FMT_CAMPO" => {
                    "Use solo los campos del contrato (skills/redes-ingesta/references/plan.md §2); el mensaje sugiere el nombre válido más parecido."
                }
                "E_CRS" => {
                    "Declare el CRS real (en el plan, `proyecto.crs`; en el terreno, `--epsg`): UTM sur en metros, p. ej. EPSG:32717 (17S) o EPSG:32718 (18S), el mismo del proyecto. Redes no reproyecta: hágalo en origen o cree un proyecto con `--crs`."
                }
                _ => {
                    "Revise el archivo. Los formatos ajenos se adaptan con la skill `skills/redes-ingesta` y entran como `redes.plan/1`."
                }
            };
            Envelope::fail(
                command,
                vec![Diagnostic::new(&code, Severity::Error, e).hint(pista)],
            )
        }
    }
}

fn objeto(p: &mut Project, cmd: ObjetoCmd, dry: bool) -> Envelope {
    match cmd {
        ObjetoCmd::Lista { familia, estado } => {
            let filtro_estado = estado.as_deref().map(|e| e.to_ascii_lowercase());
            let mut items = Vec::new();
            for n in p.nodes.iter().filter(|n| !n.deleted) {
                if let Some(f) = &familia
                    && !n.family.contains(f.as_str())
                {
                    continue;
                }
                if let Some(e) = &filtro_estado
                    && n.estado.key() != e
                {
                    continue;
                }
                items.push(json!({
                    "codigo": n.code, "familia": n.family, "subtipo": n.ty,
                    "estado": n.estado.key(), "e": round(n.pos.x, 3), "n": round(n.pos.y, 3),
                    "cota_solera": n.cota_solera().map(|z| round(z, 3)), "nota": n.nota_vigente()
                }));
            }
            for l in p.links.iter().filter(|l| !l.deleted) {
                if let Some(f) = &familia
                    && !l.family.contains(f.as_str())
                {
                    continue;
                }
                if let Some(e) = &filtro_estado
                    && l.estado.key() != e
                {
                    continue;
                }
                items.push(json!({
                    "codigo": l.code, "familia": l.family, "subtipo": l.ty,
                    "estado": l.estado.key(), "diametro_mm": l.dn(), "nota": l.nota_vigente(),
                    "longitud_m": round(l.length_2d(), 2)
                }));
            }
            Envelope::ok("objeto.lista").with_data(json!({ "total": items.len(), "items": items }))
        }
        ObjetoCmd::Revision(c) => observaciones::ejecutar(p, c, dry),
    }
}

fn exportar(p: &Project, cmd: ExportarCmd, dry: bool) -> Envelope {
    match cmd {
        ExportarCmd::Dxf {
            salida,
            solo_aprobados,
        } => {
            if dry {
                return Envelope::ok("exportar.dxf").with_data(json!({
                    "dry_run": true,
                    "salida": salida.display().to_string(),
                    "solo_aprobados": solo_aprobados,
                }));
            }
            match export::export_dxf_filtered(p, &salida, solo_aprobados) {
                Ok(n) => Envelope::ok("exportar.dxf")
                    .with_data(json!({
                        "salida": salida.display().to_string(),
                        "entidades": n,
                        "capas": "una por familia",
                        "solo_aprobados": solo_aprobados,
                        "omitidos": if solo_aprobados {
                            let (pend, _, obs) = redes_core::export::revision_summary(p);
                            serde_json::json!(pend + obs)
                        } else {
                            serde_json::json!(0)
                        }
                    }))
                    .with_artifact("dxf", &salida.display().to_string()),
                Err(e) => io_error("exportar.dxf", &e),
            }
        }
        ExportarCmd::Gltf {
            salida,
            sin_terreno,
        } => {
            let opts = redes_core::gltf::OpcionesGltf {
                con_terreno: !sin_terreno,
            };
            let bytes = redes_core::gltf::exportar_glb(p, opts);
            if dry {
                return Envelope::ok("exportar.gltf").with_data(json!({
                    "dry_run": true,
                    "salida": salida.display().to_string(),
                    "bytes": bytes.len(),
                    "primitivas": redes_core::gltf::contar_primitivas(p, !sin_terreno),
                }));
            }
            match std::fs::write(&salida, &bytes) {
                Ok(()) => Envelope::ok("exportar.gltf")
                    .with_data(json!({
                        "salida": salida.display().to_string(),
                        "bytes": bytes.len(),
                        "primitivas": redes_core::gltf::contar_primitivas(p, !sin_terreno),
                    }))
                    .with_artifact("glb", &salida.display().to_string()),
                Err(e) => io_error("exportar.gltf", &e.to_string()),
            }
        }
        ExportarCmd::Geojson { salida } => {
            if dry {
                return Envelope::ok("exportar.geojson").with_data(json!({
                    "dry_run": true,
                    "salida": salida.display().to_string(),
                }));
            }
            match export::export_geojson(p, &salida) {
                Ok(n) => Envelope::ok("exportar.geojson")
                    .with_data(json!({ "salida": salida.display().to_string(), "features": n }))
                    .with_artifact("geojson", &salida.display().to_string()),
                Err(e) => io_error("exportar.geojson", &e),
            }
        }
    }
}

/// Carpeta del proyecto (o el directorio actual si no tiene).
fn dir_parent(path: &std::path::Path) -> std::path::PathBuf {
    path.parent()
        .map(|d| d.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."))
}

fn slug(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

fn round(v: f64, d: i32) -> f64 {
    let f = 10f64.powi(d);
    (v * f).round() / f
}

#[allow(dead_code)]
fn counts_by_family(p: &Project) -> BTreeMap<String, usize> {
    let mut m = BTreeMap::new();
    for n in p.nodes.iter().filter(|n| !n.deleted) {
        *m.entry(n.family.clone()).or_insert(0) += 1;
    }
    for l in p.links.iter().filter(|l| !l.deleted) {
        *m.entry(l.family.clone()).or_insert(0) += 1;
    }
    m
}
