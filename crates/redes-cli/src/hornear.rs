//! `redes biblioteca hornear`: bakes a Blender family recipe for the objects of a project, or for
//! parameter sets given by hand, into the `.glb` cache (D16, docs/17 section F17-02).
//!
//! Every parameter set is looked up in the cache first; only the missing ones go to Blender,
//! all of them in one process. Without Blender the cache still serves what it has, and the
//! viewer keeps drawing the fallback piece of `parts.rs` for the rest.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use redes_core::bake::{self, LoadedRecipe, ParamSet};
use redes_core::biblioteca::Biblioteca;
use redes_core::envelope::{Diagnostic, Envelope, Severity};
use redes_core::model::Project;
use serde_json::json;

pub const CMD: &str = "biblioteca.hornear";

pub const CODIGOS: &[(&str, &str, i32)] = &[
    (
        "E_RECETA_INVALIDA",
        "la receta no es redes.receta/1, no se llama X.receta.json o le falta su guion X.py",
        1,
    ),
    (
        "E_PARAMETRO_FALTANTE",
        "un juego de parámetros no trae todos los datos de la receta: no se hornea ni se rellena",
        1,
    ),
    (
        "E_BLENDER_NO_ENCONTRADO",
        "hay piezas sin hornear y no se encontró Blender (--blender, $REDES_BLENDER, instalación estándar o PATH)",
        1,
    ),
    (
        "E_HORNEADO_FALLIDO",
        "Blender salió con error o no escribió todas las piezas; la caché queda como estaba",
        1,
    ),
];

#[derive(clap::Args)]
pub struct HornearArgs {
    /// Contrato de la receta (`tools/blender/X.receta.json`, junto a su guion `X.py`).
    #[arg(long)]
    pub receta: PathBuf,
    /// Juegos de parámetros de dato a mano (JSON: un objeto o una lista). Sin él se hornean los
    /// objetos del proyecto de la familia de la receta.
    #[arg(long)]
    pub parametros: Option<PathBuf>,
    /// Carpeta de la biblioteca (donde vive `biblioteca.json`); por defecto, la del proyecto o
    /// el directorio actual.
    #[arg(long)]
    pub carpeta: Option<PathBuf>,
    /// Ejecutable de Blender 5.1; por defecto, la instalación estándar o el PATH.
    #[arg(long, env = "REDES_BLENDER")]
    pub blender: Option<PathBuf>,
    /// No guardar el `.blend` de revisión junto a cada `.glb` (por defecto se guarda, para
    /// abrir la pieza en Blender y ver cómo se construyó).
    #[arg(long)]
    pub sin_blend: bool,
}

fn fail(code: &str, msg: impl Into<String>, hint: &str) -> Envelope {
    Envelope::fail(
        CMD,
        vec![Diagnostic::new(code, Severity::Error, msg).hint(hint)],
    )
}

fn from_core_error(e: &str) -> Envelope {
    let code = e.split(':').next().unwrap_or("E_RECETA_INVALIDA");
    fail(
        code,
        e,
        "La receta es `tools/blender/X.receta.json` y su guion `X.py`, junto a `redes_bake.py`.",
    )
}

/// Bakes hand-given parameter sets; does not need a project.
pub fn from_given_params(args: &HornearArgs, dry: bool) -> Envelope {
    let loaded = match LoadedRecipe::load(&args.receta) {
        Ok(l) => l,
        Err(e) => return from_core_error(&e),
    };
    let path = args.parametros.as_ref().expect("given-params mode");
    let sets = match read_given_sets(path) {
        Ok(s) => s,
        Err(e) => {
            return fail(
                "E_FMT_PARSE",
                e,
                "Una lista JSON de objetos con los parámetros de dato de la receta.",
            );
        }
    };
    let mut keyed = Vec::with_capacity(sets.len());
    for (i, given) in sets.iter().enumerate() {
        match loaded.recipe.params_from_given(given) {
            Ok(set) => keyed.push((loaded.key(&set), set)),
            Err(problems) => {
                return fail(
                    "E_PARAMETRO_FALTANTE",
                    format!("juego {i}: {}", problems.join(", ")),
                    "Cada juego trae todos los parámetros de dato de la receta; los de forma los pone la receta.",
                );
            }
        }
    }
    let library = args.carpeta.clone().unwrap_or_else(|| PathBuf::from("."));
    let distinct: BTreeMap<String, ParamSet> = keyed.iter().cloned().collect();
    let report = match bake_missing(&loaded, &library, &distinct, args, dry) {
        Ok(r) => r,
        Err(env) => return *env,
    };
    Envelope::ok(CMD).with_data(json!({
        "receta": loaded.name,
        "carpeta": library.display().to_string(),
        "juegos": keyed.iter().map(|(key, _)| json!({
            "clave": key,
            "archivo": absolute(&bake::cached_glb(&library, key)).display().to_string(),
            "blend": (!args.sin_blend)
                .then(|| absolute(&bake::cached_blend(&library, key)).display().to_string()),
        })).collect::<Vec<_>>(),
        "horneado": report.to_json(dry),
    }))
}

/// Bakes the project objects the recipe draws, records the pieces in the manifest and points
/// each object at its piece. Objects with missing data keep the fallback piece.
pub fn from_project(
    p: &mut Project,
    project_path: &Path,
    args: &HornearArgs,
    dry: bool,
) -> Envelope {
    let loaded = match LoadedRecipe::load(&args.receta) {
        Ok(l) => l,
        Err(e) => return from_core_error(&e),
    };
    let project_dir = project_path
        .parent()
        .filter(|d| !d.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let library = args.carpeta.clone().unwrap_or_else(|| project_dir.clone());

    let mut objects: Vec<(String, Result<String, Vec<String>>)> = Vec::new();
    let mut distinct: BTreeMap<String, ParamSet> = BTreeMap::new();
    let mut unanchored: Vec<(String, bake::Unanchored)> = Vec::new();
    for n in p.nodes.iter().filter(|n| loaded.recipe.applies_to(n)) {
        let outcome = loaded.recipe.params_for_node(p, n).map(|set| {
            let key = loaded.key(&set);
            distinct.insert(key.clone(), set);
            key
        });
        if outcome.is_ok() && loaded.recipe.uses_connections() {
            for u in bake::node_connections(p, n).unanchored {
                unanchored.push((n.code.clone(), u));
            }
        }
        objects.push((n.code.clone(), outcome));
    }

    let report = match bake_missing(&loaded, &library, &distinct, args, dry) {
        Ok(r) => r,
        Err(env) => return *env,
    };
    let lacking: Vec<_> = objects
        .iter()
        .filter_map(|(code, o)| o.as_ref().err().map(|m| (code.clone(), m.clone())))
        .collect();

    if !dry && let Err(env) = record_pieces(p, project_path, &library, &loaded, &objects, &distinct)
    {
        return *env;
    }

    let mut warnings: Vec<Diagnostic> = lacking
        .iter()
        .map(|(code, missing)| {
            Diagnostic::new(
                "W_PARAMETRO_FALTANTE",
                Severity::Warning,
                format!(
                    "{code}: sin {}; se dibuja la pieza de respaldo",
                    missing.join(", ")
                ),
            )
        })
        .collect();
    warnings.extend(unanchored.iter().map(|(code, u)| {
        let link = u
            .link
            .as_deref()
            .map(|l| format!(" {l}"))
            .unwrap_or_default();
        Diagnostic::new(
            "W_SIN_ANCLA",
            Severity::Warning,
            format!("{code}{link}: {}; la pieza no lleva ancla ahí", u.reason),
        )
    }));
    Envelope::ok(CMD)
        .with_data(json!({
            "receta": loaded.name,
            "carpeta": library.display().to_string(),
            "objetos": objects.len(),
            "con_pieza": objects.len() - lacking.len(),
            "piezas_distintas": distinct.len(),
            "sin_hornear": lacking.iter().map(|(code, missing)| json!({
                "codigo": code,
                "faltan": missing,
            })).collect::<Vec<_>>(),
            "sin_ancla": unanchored.iter().map(|(code, u)| json!({
                "codigo": code,
                "tramo": u.link,
                "motivo": u.reason,
            })).collect::<Vec<_>>(),
            "horneado": report.to_json(dry),
            "guardado": !dry,
        }))
        .with_diagnostics(warnings)
}

fn record_pieces(
    p: &mut Project,
    project_path: &Path,
    library: &Path,
    loaded: &LoadedRecipe,
    objects: &[(String, Result<String, Vec<String>>)],
    distinct: &BTreeMap<String, ParamSet>,
) -> Result<(), Box<Envelope>> {
    let io = |e: String| Box::new(from_core_error(&e));
    let stale_prefix = format!("{}-", loaded.id_prefix());
    for (code, outcome) in objects {
        let Some(n) = p.node_by_code_mut(code) else {
            continue;
        };
        n.modelo = match outcome {
            Ok(key) => Some(loaded.piece_id(key)),
            // A piece baked from older data would show a shape the data no longer supports.
            Err(_)
                if n.modelo
                    .as_deref()
                    .is_some_and(|m| m.starts_with(&stale_prefix)) =>
            {
                None
            }
            Err(_) => n.modelo.clone(),
        };
    }

    let manifest_path = library.join("biblioteca.json");
    let mut manifest = Biblioteca::cargar(&manifest_path).map_err(io)?;
    for (key, set) in distinct {
        let glb = std::fs::read(bake::cached_glb(library, key))
            .map_err(|e| io(format!("E_IO_NOT_FOUND: {e}")))?;
        manifest.insertar(bake::baked_entry(loaded, key, set, &glb).map_err(io)?);
    }
    let in_use: BTreeSet<&str> = p.nodes.iter().filter_map(|n| n.modelo.as_deref()).collect();
    manifest
        .piezas
        .retain(|e| !e.id.starts_with(&stale_prefix) || in_use.contains(e.id.as_str()));
    manifest.guardar(&manifest_path).map_err(io)?;

    let project_dir = project_path.parent().unwrap_or(Path::new(""));
    p.biblioteca = Some(
        manifest_path
            .strip_prefix(project_dir)
            .unwrap_or(&manifest_path)
            .to_string_lossy()
            .replace('\\', "/"),
    );
    p.commit(&format!("{CMD} {}: {} piezas", loaded.name, distinct.len()));
    redes_core::project_io::save(p, project_path).map_err(io)
}

fn read_given_sets(path: &Path) -> Result<Vec<ParamSet>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let value: serde_json::Value = serde_json::from_str(text.trim_start_matches('\u{feff}'))
        .map_err(|e| format!("{}: {e}", path.display()))?;
    let list = match value {
        serde_json::Value::Array(items) => items,
        one => vec![one],
    };
    list.into_iter()
        .map(|v| serde_json::from_value(v).map_err(|e| e.to_string()))
        .collect()
}

struct BakeReport {
    from_cache: usize,
    baked: usize,
    blender: Option<PathBuf>,
    blender_seconds: f64,
}

impl BakeReport {
    fn to_json(&self, dry: bool) -> serde_json::Value {
        let baked_label = if dry { "por_hornear" } else { "horneadas" };
        let mut out = json!({
            "de_cache": self.from_cache,
            "blender": self.blender.as_ref().map(|b| b.display().to_string()),
            "segundos_blender": (self.blender_seconds * 1000.0).round() / 1000.0,
        });
        out[baked_label] = json!(self.baked);
        out
    }
}

/// Bakes, in one Blender process, the sets whose `.glb` is not cached yet.
fn bake_missing(
    loaded: &LoadedRecipe,
    library: &Path,
    sets: &BTreeMap<String, ParamSet>,
    args: &HornearArgs,
    dry: bool,
) -> Result<BakeReport, Box<Envelope>> {
    // The `.glb` is what the viewer draws; the `.blend` is the same bake opened for review.
    let mut outputs = vec![("glb", "output")];
    if !args.sin_blend {
        outputs.push(("blend", "blend"));
    }
    let cached = |key: &str, ext: &str| library.join(bake::CACHE_DIR).join(format!("{key}.{ext}"));
    let missing: Vec<&String> = sets
        .keys()
        .filter(|k| outputs.iter().any(|(ext, _)| !cached(k, ext).is_file()))
        .collect();
    let mut report = BakeReport {
        from_cache: sets.len() - missing.len(),
        baked: missing.len(),
        blender: None,
        blender_seconds: 0.0,
    };
    if missing.is_empty() || dry {
        return Ok(report);
    }
    let Some(exe) = find_blender(args.blender.as_deref()) else {
        return Err(Box::new(fail(
            "E_BLENDER_NO_ENCONTRADO",
            format!(
                "{} piezas sin hornear y no se encontró Blender",
                missing.len()
            ),
            "Instale Blender 5.1 o indíquelo con --blender / $REDES_BLENDER. El visor sigue abriendo y dibuja la pieza de respaldo.",
        )));
    };
    let cache = library.join(bake::CACHE_DIR);
    std::fs::create_dir_all(&cache).map_err(permission_failure)?;

    let staged = |key: &str, ext: &str| cache.join(format!("{key}.tmp.{ext}"));
    let jobs: Vec<serde_json::Value> = missing
        .iter()
        .map(|key| {
            let mut job = serde_json::to_value(&sets[*key]).expect("params serialise");
            for (ext, job_key) in &outputs {
                job[*job_key] = json!(absolute(&staged(key, ext)).display().to_string());
            }
            job
        })
        .collect();
    let jobs_file = cache.join(format!("lote-{}.json", std::process::id()));
    std::fs::write(&jobs_file, json!({ "jobs": jobs }).to_string()).map_err(permission_failure)?;

    let t0 = Instant::now();
    let run = Command::new(&exe)
        .args([
            "-b",
            "--factory-startup",
            "--python-exit-code",
            "1",
            "--python",
        ])
        .arg(&loaded.script)
        .arg("--")
        .arg(&jobs_file)
        .output();
    report.blender_seconds = t0.elapsed().as_secs_f64();
    report.blender = Some(exe);
    let _ = std::fs::remove_file(&jobs_file);

    let all_written = missing
        .iter()
        .all(|k| outputs.iter().all(|(ext, _)| staged(k, ext).is_file()));
    let failure = match &run {
        Err(e) => Some(e.to_string()),
        Ok(out) if !out.status.success() || !all_written => Some(tail(&out.stderr, &out.stdout)),
        Ok(_) => None,
    };
    if let Some(why) = failure {
        for key in &missing {
            for (ext, _) in &outputs {
                let _ = std::fs::remove_file(staged(key, ext));
            }
        }
        return Err(Box::new(fail(
            "E_HORNEADO_FALLIDO",
            why,
            "Pruebe la receta a mano: blender -b --factory-startup --python-exit-code 1 --python <receta.py> -- <params.json>.",
        )));
    }
    // The cache only ever holds complete files: each one is written aside and renamed in.
    for key in &missing {
        for (ext, _) in &outputs {
            std::fs::rename(staged(key, ext), cached(key, ext)).map_err(permission_failure)?;
        }
    }
    Ok(report)
}

fn permission_failure(e: std::io::Error) -> Box<Envelope> {
    Box::new(fail(
        "E_IO_PERMISSION",
        e.to_string(),
        "Revise los permisos de la carpeta.",
    ))
}

fn tail(stderr: &[u8], stdout: &[u8]) -> String {
    let text = String::from_utf8_lossy(if stderr.is_empty() { stdout } else { stderr });
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    lines[lines.len().saturating_sub(6)..].join(" | ")
}

fn absolute(path: &Path) -> PathBuf {
    std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf())
}

/// Blender given by the user, else the newest standard Windows install, else the `PATH`.
pub fn find_blender(given: Option<&Path>) -> Option<PathBuf> {
    if let Some(path) = given {
        return path.is_file().then(|| path.to_path_buf());
    }
    newest_standard_install().or_else(on_path)
}

fn newest_standard_install() -> Option<PathBuf> {
    let root = std::env::var_os("ProgramFiles").map(PathBuf::from)?;
    let mut installs: Vec<PathBuf> = std::fs::read_dir(root.join("Blender Foundation"))
        .ok()?
        .filter_map(|e| e.ok().map(|e| e.path().join("blender.exe")))
        .filter(|p| p.is_file())
        .collect();
    installs.sort();
    installs.pop()
}

fn on_path() -> Option<PathBuf> {
    let exe = if cfg!(windows) {
        "blender.exe"
    } else {
        "blender"
    };
    std::env::split_paths(&std::env::var_os("PATH")?)
        .map(|dir| dir.join(exe))
        .find(|p| p.is_file())
}
