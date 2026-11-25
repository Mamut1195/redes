//! Salida: DXF para el detallado en CAD y GeoJSON para SIG.
//!
//! El visor no dibuja planos: aprueba el modelo y lo entrega. El DXF (R12, ASCII) sale listo para
//! que el dibujante trabaje en AutoCAD/Civil 3D/BricsCAD sin rehacer nada:
//!
//! - cabecera con `$INSUNITS = 6` (metros), `$DWGCODEPAGE = ANSI_1252` y extensión del dibujo;
//! - tabla de capas con color ACI por sistema (agua potable, sanitario, pluvial, combinado, común),
//!   capas de rotulación propias (`ROT_<sistema>_TRAMOS` / `ROT_<sistema>_NODOS`), la capa
//!   `OBSERVACIONES` de la revisión y la capa de datos `REDES_INFO` (apagada);
//! - nodos como bloque (círculo de diámetro interior, atributos ocultos con código, subtipo y
//!   cotas), punto en la solera y línea vertical de la tapa a la solera;
//! - tramos como polilínea 3D con su rótulo (DN, material, longitud, pendiente);
//! - solo se rotula lo que existe: un dato ausente no se escribe.
//!
//! Codificación: R12 no admite UTF-8 y su texto se interpreta con `$DWGCODEPAGE`. Para que el
//! archivo se lea igual en cualquier CAD, todo lo que ve el dibujante es ASCII: los nombres de capa
//! se escriben sin tildes (`CL_TUBERIA`) y los textos llevan los caracteres no ASCII como `\U+XXXX`,
//! que AutoCAD, BricsCAD y ezdxf decodifican. La capa `REDES_INFO` conserva su carga tal cual
//! (UTF-8) porque es la que lee `importar dxf` en la ida y vuelta (D1).

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

use crate::families::{Category, Value, family};
use crate::geom::P3;
use crate::model::{EstadoObjeto, Link, Node, Project};

/// Altura del texto de rotulación (m de modelo): 2 mm de papel a 1:500.
pub const ALTO_TEXTO_DXF: f64 = 1.0;
/// Capa de las observaciones de la revisión.
pub const CAPA_OBSERVACIONES: &str = "OBSERVACIONES";
/// Capa de datos de la ida y vuelta (la lee `importar dxf`).
pub const CAPA_INFO: &str = "REDES_INFO";

// --- Lectura de los datos que pueden faltar: se leen siempre con los lectores del modelo, que
// devuelven `None` cuando el origen no trajo el dato (F12-03), aunque el campo guarde un valor de
// dibujo. ---

fn cota_terreno(node: &Node) -> Option<f64> {
    node.cota_terreno().filter(|z| z.is_finite())
}

fn cota_tapa(node: &Node) -> Option<f64> {
    node.cota_tapa().filter(|z| z.is_finite())
}

fn cota_solera(node: &Node) -> Option<f64> {
    node.cota_solera().filter(|z| z.is_finite())
}

/// Diámetro interior declarado del pozo (m). Sin valor por defecto: si falta, no se acota.
fn diametro_pozo(node: &Node) -> Option<f64> {
    node.num("diametro_interior")
        .filter(|d| d.is_finite() && *d > 0.0)
}

/// DN declarado del tramo (mm); `None` si el origen no lo trajo.
fn dn_mm(link: &Link) -> Option<u32> {
    link.dn()
}

/// Exporta el modelo a DXF ASCII (R12 cómodo para cualquier CAD).
pub fn export_dxf(p: &Project, path: &Path) -> Result<usize, String> {
    export_dxf_filtered(p, path, false)
}

/// Sistema de la red al que pertenece un objeto: decide el color de su capa.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Sistema {
    AguaPotable,
    Sanitario,
    Pluvial,
    Combinado,
    Comun,
}

impl Sistema {
    fn sigla(self) -> &'static str {
        match self {
            Sistema::AguaPotable => "AP",
            Sistema::Sanitario => "SAN",
            Sistema::Pluvial => "PLU",
            Sistema::Combinado => "COMB",
            Sistema::Comun => "COM",
        }
    }
    /// Color ACI de la geometría.
    fn color(self) -> i16 {
        match self {
            Sistema::AguaPotable => 5,
            Sistema::Sanitario => 30,
            Sistema::Pluvial => 3,
            Sistema::Combinado => 6,
            Sistema::Comun => 8,
        }
    }
    /// Color ACI de la rotulación (el mismo tono, más claro).
    fn color_rotulo(self) -> i16 {
        match self {
            Sistema::AguaPotable => 150,
            Sistema::Sanitario => 32,
            Sistema::Pluvial => 92,
            Sistema::Combinado => 212,
            Sistema::Comun => 9,
        }
    }
}

/// Sistema por familia y, en saneamiento, por subtipo o tipo declarado (pluvial, combinado).
fn sistema(familia: &str, ty: &str, attrs: &BTreeMap<String, Value>) -> Sistema {
    let declarado = ["sistema", "tipo_acometida"]
        .iter()
        .find_map(|k| attrs.get(*k).and_then(|v| v.as_str()))
        .unwrap_or("");
    match family(familia).map(|f| f.category) {
        Some(Category::Agua) => Sistema::AguaPotable,
        Some(Category::Saneamiento) => {
            if declarado == "agua_potable" {
                Sistema::AguaPotable
            } else if declarado == "pluvial" || ty.contains("pluvial") {
                Sistema::Pluvial
            } else if declarado == "combinado" || ty.contains("combinado") {
                Sistema::Combinado
            } else {
                Sistema::Sanitario
            }
        }
        _ => Sistema::Comun,
    }
}

/// Capa de la geometría de un objeto. La familia da el nombre; si el sistema no es el propio de la
/// familia (un colector pluvial en la familia de tubería de saneamiento) se añade su sigla, para
/// que cada sistema tenga su color sin sobrescribir colores por entidad.
fn capa_objeto(familia: &str, ty: &str, attrs: &BTreeMap<String, Value>) -> (String, Sistema) {
    let s = sistema(familia, ty, attrs);
    let propio = match family(familia).map(|f| f.category) {
        Some(Category::Agua) => Sistema::AguaPotable,
        Some(Category::Saneamiento) => Sistema::Sanitario,
        _ => Sistema::Comun,
    };
    let base = layer_name(familia);
    if s == propio {
        (base, s)
    } else {
        (format!("{base}_{}", s.sigla()), s)
    }
}

/// Entidades y capas que se van escribiendo; la cabecera y las tablas se componen al final.
struct Dibujo {
    ent: String,
    /// Capa → color ACI (negativo = apagada).
    capas: BTreeMap<String, i16>,
    /// Bloques de nodo (nombre = capa).
    bloques: std::collections::BTreeSet<String>,
    lo: [f64; 3],
    hi: [f64; 3],
}

impl Dibujo {
    fn nuevo() -> Self {
        let mut capas = BTreeMap::new();
        capas.insert("0".to_string(), 7);
        capas.insert(CAPA_OBSERVACIONES.to_string(), 1);
        capas.insert(CAPA_INFO.to_string(), -8);
        Dibujo {
            ent: String::with_capacity(64 * 1024),
            capas,
            bloques: Default::default(),
            lo: [f64::INFINITY; 3],
            hi: [f64::NEG_INFINITY; 3],
        }
    }

    fn capa(&mut self, nombre: &str, color: i16) {
        self.capas.entry(nombre.to_string()).or_insert(color);
    }

    fn extension(&mut self, x: f64, y: f64, z: f64) {
        for (i, v) in [x, y, z].into_iter().enumerate() {
            if v.is_finite() {
                self.lo[i] = self.lo[i].min(v);
                self.hi[i] = self.hi[i].max(v);
            }
        }
    }

    /// TEXT de una línea. `centrado` alinea al centro de la base (72 = 1) en el punto dado.
    #[allow(clippy::too_many_arguments)]
    fn texto(
        &mut self,
        capa: &str,
        q: [f64; 3],
        alto: f64,
        giro_grados: f64,
        contenido: &str,
        centrado: bool,
    ) {
        let [x, y, z] = q;
        write!(
            self.ent,
            "0\nTEXT\n8\n{capa}\n10\n{x:.4}\n20\n{y:.4}\n30\n{z:.4}\n40\n{alto:.3}\n1\n{}\n50\n{giro_grados:.3}\n",
            texto_dxf(contenido)
        )
        .ok();
        if centrado {
            write!(self.ent, "72\n1\n11\n{x:.4}\n21\n{y:.4}\n31\n{z:.4}\n").ok();
        }
    }

    /// TEXT de la capa de datos: la carga va tal cual (la lee el importador).
    fn info(&mut self, q: [f64; 3], carga: &str) {
        write!(
            self.ent,
            "0\nTEXT\n8\n{CAPA_INFO}\n10\n{:.4}\n20\n{:.4}\n30\n{:.4}\n40\n0.200\n1\n{}\n",
            q[0],
            q[1],
            q[2],
            carga.replace(['\r', '\n'], " ")
        )
        .ok();
    }

    fn circulo(&mut self, capa: &str, q: [f64; 3], r: f64) {
        write!(
            self.ent,
            "0\nCIRCLE\n8\n{capa}\n10\n{:.4}\n20\n{:.4}\n30\n{:.4}\n40\n{r:.4}\n",
            q[0], q[1], q[2]
        )
        .ok();
    }

    /// Observación de la revisión junto al objeto: texto (código, estado, nota) y, si está
    /// observado, una nube (círculo) que lo rodea. Entra solo si está observado o tiene nota
    /// (cualquier estado): un aprobado o pendiente sin nota no lleva rótulo.
    fn observacion(
        &mut self,
        code: &str,
        estado: EstadoObjeto,
        nota: Option<&str>,
        q: [f64; 3],
        giro: f64,
        nube: f64,
    ) {
        let nota = nota.map(str::trim).filter(|s| !s.is_empty());
        if nota.is_none() && estado == EstadoObjeto::Pendiente {
            return;
        }
        let mut t = format!("{code} {}", estado.key().to_uppercase());
        if let Some(n) = nota {
            write!(t, ": {n}").ok();
        }
        self.texto(CAPA_OBSERVACIONES, q, ALTO_TEXTO_DXF, giro, &t, false);
        if estado == EstadoObjeto::Observado {
            self.circulo(CAPA_OBSERVACIONES, q, nube);
        }
    }
}

/// Igual que [export_dxf], pero puede entregar solo lo aprobado (--solo-aprobados).
pub fn export_dxf_filtered(
    p: &Project,
    path: &Path,
    solo_aprobados: bool,
) -> Result<usize, String> {
    let incluido = |borrado: bool, estado: EstadoObjeto| {
        !borrado && (!solo_aprobados || estado == EstadoObjeto::Aprobado)
    };
    let nodos: Vec<&Node> = p
        .nodes
        .iter()
        .filter(|x| incluido(x.deleted, x.estado))
        .collect();
    let arcos: Vec<&Link> = p
        .links
        .iter()
        .filter(|x| incluido(x.deleted, x.estado) && x.path.len() >= 2)
        .collect();

    let mut d = Dibujo::nuevo();
    for node in &nodos {
        escribe_nodo(&mut d, node);
    }
    for link in &arcos {
        escribe_tramo(&mut d, link);
    }
    // Capa de datos: lo que el CAD no guarda en la geometría (familia, subtipo, código, atributos y
    // datos). Es lo que permite que la ida y vuelta no pierda nada; está apagada para no tapar el
    // dibujo.
    for node in &nodos {
        info_nodo(&mut d, node);
    }
    for link in &arcos {
        info_tramo(&mut d, link);
    }
    let n = nodos.len() + arcos.len();

    let mut s = String::with_capacity(d.ent.len() + 16 * 1024);
    cabecera(&mut s, &d);
    tablas(&mut s, &d);
    bloques(&mut s, &d);
    s.push_str("0\nSECTION\n2\nENTITIES\n");
    s.push_str(&d.ent);
    s.push_str("0\nENDSEC\n0\nEOF\n");

    if let Some(dir) = path.parent()
        && !dir.as_os_str().is_empty()
    {
        std::fs::create_dir_all(dir).map_err(|e| format!("E_IO_PERMISSION: {e}"))?;
    }
    std::fs::write(path, s).map_err(|e| format!("E_IO_PERMISSION: {e}"))?;
    Ok(n)
}

fn cabecera(s: &mut String, d: &Dibujo) {
    let (lo, hi) = if d.lo[0].is_finite() {
        (d.lo, d.hi)
    } else {
        ([0.0; 3], [0.0; 3])
    };
    write!(
        s,
        "0\nSECTION\n2\nHEADER\n\
9\n$ACADVER\n1\nAC1009\n\
9\n$DWGCODEPAGE\n3\nANSI_1252\n\
9\n$INSUNITS\n70\n6\n\
9\n$EXTMIN\n10\n{:.4}\n20\n{:.4}\n30\n{:.4}\n\
9\n$EXTMAX\n10\n{:.4}\n20\n{:.4}\n30\n{:.4}\n\
9\n$TEXTSIZE\n40\n{ALTO_TEXTO_DXF:.3}\n\
0\nENDSEC\n",
        lo[0], lo[1], lo[2], hi[0], hi[1], hi[2]
    )
    .ok();
}

fn tablas(s: &mut String, d: &Dibujo) {
    s.push_str("0\nSECTION\n2\nTABLES\n");
    s.push_str(
        "0\nTABLE\n2\nLTYPE\n70\n1\n\
0\nLTYPE\n2\nCONTINUOUS\n70\n0\n3\nSolid line\n72\n65\n73\n0\n40\n0.0\n\
0\nENDTAB\n",
    );
    write!(s, "0\nTABLE\n2\nLAYER\n70\n{}\n", d.capas.len()).ok();
    for (nombre, color) in &d.capas {
        write!(
            s,
            "0\nLAYER\n2\n{nombre}\n70\n0\n62\n{color}\n6\nCONTINUOUS\n"
        )
        .ok();
    }
    s.push_str("0\nENDTAB\n");
    s.push_str(
        "0\nTABLE\n2\nSTYLE\n70\n1\n\
0\nSTYLE\n2\nSTANDARD\n70\n0\n40\n0.0\n41\n1.0\n50\n0.0\n71\n0\n42\n1.0\n3\ntxt\n4\n\n\
0\nENDTAB\n",
    );
    s.push_str("0\nENDSEC\n");
}

/// Atributos de los bloques de nodo, ocultos: son datos para extraer (DATAEXTRACTION), la
/// rotulación visible va en las capas `ROT_*` para que el dibujante la mueva o apague.
const ATRIBUTOS_NODO: [&str; 5] = [
    "CODIGO",
    "SUBTIPO",
    "COTA_TAPA",
    "COTA_SOLERA",
    "PROFUNDIDAD",
];

fn bloques(s: &mut String, d: &Dibujo) {
    s.push_str("0\nSECTION\n2\nBLOCKS\n");
    for nombre in &d.bloques {
        // 70 = 2: el bloque tiene definiciones de atributo. Geometría en la capa 0 para que tome
        // la capa del INSERT.
        write!(
            s,
            "0\nBLOCK\n8\n0\n2\n{nombre}\n70\n2\n10\n0.0\n20\n0.0\n30\n0.0\n3\n{nombre}\n"
        )
        .ok();
        // Círculo de diámetro 1: el INSERT lo escala por el diámetro interior (radio = DI/2).
        s.push_str("0\nCIRCLE\n8\n0\n10\n0.0\n20\n0.0\n30\n0.0\n40\n0.5\n");
        for (i, etiqueta) in ATRIBUTOS_NODO.iter().enumerate() {
            write!(
                s,
                "0\nATTDEF\n8\n0\n10\n0.6\n20\n{:.2}\n30\n0.0\n40\n0.25\n1\n\n3\n{etiqueta}\n2\n{etiqueta}\n70\n1\n",
                -(i as f64) * 0.35
            )
            .ok();
        }
        s.push_str("0\nENDBLK\n8\n0\n");
    }
    s.push_str("0\nENDSEC\n");
}

fn escribe_nodo(d: &mut Dibujo, node: &Node) {
    let (capa, sis) = capa_objeto(&node.family, &node.ty, &node.attrs);
    let rot = format!("ROT_{}_NODOS", sis.sigla());
    d.capa(&capa, sis.color());
    d.capa(&rot, sis.color_rotulo());
    d.bloques.insert(capa.clone());
    let (x, y) = (node.pos.x, node.pos.y);
    let tapa = cota_tapa(node);
    let solera = cota_solera(node);
    let profundidad = match (tapa, solera) {
        (Some(t), Some(s)) if t > s => Some(t - s),
        _ => None,
    };
    let z = tapa.or(solera).unwrap_or(0.0);
    // Sin DI declarado el bloque sale a escala 1 (símbolo de 1 m), sin cota de diámetro.
    let di = diametro_pozo(node).unwrap_or(1.0);
    d.extension(x - di, y - di, z);
    d.extension(x + di, y + di, solera.unwrap_or(z));

    // Bloque con atributos: el objeto del CAD. Solo se escriben los atributos con valor.
    write!(
        d.ent,
        "0\nINSERT\n8\n{capa}\n66\n1\n2\n{capa}\n10\n{x:.4}\n20\n{y:.4}\n30\n{z:.4}\n41\n{di:.3}\n42\n{di:.3}\n43\n1.0\n"
    )
    .ok();
    let valores = [
        Some(node.code.clone()),
        Some(node.ty.clone()),
        tapa.map(|v| format!("{v:.3}")),
        solera.map(|v| format!("{v:.3}")),
        profundidad.map(|v| format!("{v:.3}")),
    ];
    for (i, (etiqueta, valor)) in ATRIBUTOS_NODO.iter().zip(valores).enumerate() {
        let Some(valor) = valor.filter(|v| !v.is_empty()) else {
            continue;
        };
        write!(
            d.ent,
            "0\nATTRIB\n8\n{capa}\n10\n{:.4}\n20\n{:.4}\n30\n{z:.4}\n40\n0.25\n1\n{}\n2\n{etiqueta}\n70\n1\n",
            x + 0.6 * di,
            y - (i as f64) * 0.35 * di,
            texto_dxf(&valor)
        )
        .ok();
    }
    write!(d.ent, "0\nSEQEND\n8\n{capa}\n").ok();

    // Punto en la solera y altura del pozo: línea vertical de la tapa hacia abajo, a la solera.
    if let Some(s) = solera {
        write!(
            d.ent,
            "0\nPOINT\n8\n{capa}\n10\n{x:.4}\n20\n{y:.4}\n30\n{s:.4}\n"
        )
        .ok();
    }
    if let (Some(t), Some(s), Some(_)) = (tapa, solera, profundidad) {
        write!(
            d.ent,
            "0\nLINE\n8\n{capa}\n10\n{x:.4}\n20\n{y:.4}\n30\n{t:.4}\n11\n{x:.4}\n21\n{y:.4}\n31\n{s:.4}\n"
        )
        .ok();
    }

    // Rótulo: código y cotas (CT tapa, CS solera, H profundidad), solo las que existen.
    let h = ALTO_TEXTO_DXF;
    let mut lineas = vec![node.code.clone()];
    if let Some(t) = tapa {
        lineas.push(format!("CT {t:.2}"));
    }
    if let Some(s) = solera {
        lineas.push(format!("CS {s:.2}"));
    }
    if let Some(p) = profundidad {
        lineas.push(format!("H {p:.2}"));
    }
    let x0 = x + di * 0.5 + 0.5 * h;
    for (i, l) in lineas.iter().filter(|l| !l.is_empty()).enumerate() {
        d.texto(&rot, [x0, y - (i as f64) * 1.5 * h, z], h, 0.0, l, false);
    }

    d.observacion(
        &node.code,
        node.estado,
        node.nota.as_deref(),
        [x0, y + 1.5 * h, z],
        0.0,
        (di * 0.5 + 1.0).max(2.0),
    );
}

fn escribe_tramo(d: &mut Dibujo, link: &Link) {
    let (capa, sis) = capa_objeto(&link.family, &link.ty, &link.attrs);
    let rot = format!("ROT_{}_TRAMOS", sis.sigla());
    d.capa(&capa, sis.color());
    d.capa(&rot, sis.color_rotulo());

    // POLYLINE 3D (70 = 8).
    write!(d.ent, "0\nPOLYLINE\n8\n{capa}\n66\n1\n70\n8\n").ok();
    for v in &link.path {
        d.extension(v.x, v.y, v.z);
        write!(
            d.ent,
            "0\nVERTEX\n8\n{capa}\n10\n{:.4}\n20\n{:.4}\n30\n{:.4}\n70\n32\n",
            v.x, v.y, v.z
        )
        .ok();
    }
    write!(d.ent, "0\nSEQEND\n8\n{capa}\n").ok();

    let (medio, giro) = punto_medio(&link.path);
    let h = ALTO_TEXTO_DXF;
    let (sn, cs) = giro.to_radians().sin_cos();
    let desplaza = |k: f64| [medio.x - sn * k, medio.y + cs * k, medio.z];
    let rotulo = rotulo_tramo(link);
    if !rotulo.is_empty() {
        d.texto(&rot, desplaza(0.5 * h), h, giro, &rotulo, true);
    }
    d.observacion(
        &link.code,
        link.estado,
        link.nota.as_deref(),
        desplaza(-2.0 * h),
        giro,
        2.0_f64.max(link.diametro_interior_m() + 1.0),
    );
}

/// Rótulo de tramo: `código DN200 PVC L=45.20 m i=0.50 %`. Cada dato solo si existe. La pendiente
/// declarada (atributo `pendiente`, m/m) se escribe tal cual; si no hay, se deriva de las soleras
/// declaradas del tramo y se marca `(soleras)`; si hay las dos y no cuadran, salen ambas.
fn rotulo_tramo(link: &Link) -> String {
    let mut partes: Vec<String> = Vec::new();
    if !link.code.is_empty() {
        partes.push(link.code.clone());
    }
    if let Some(dn) = dn_mm(link) {
        partes.push(format!("DN{dn}"));
    }
    if let Some(m) = link
        .get("material")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|m| !m.is_empty())
    {
        partes.push(m.to_uppercase());
    }
    let l = link.length_2d();
    if l.is_finite() && l > 0.0 {
        partes.push(format!("L={l:.2} m"));
    }
    let declarada = link.num("pendiente").filter(|i| i.is_finite());
    let derivada = pendiente_por_soleras(link);
    match (declarada, derivada) {
        (Some(i), Some(g)) if (i - g).abs() * 100.0 >= 0.005 => {
            // No cuadran: se escriben las dos, cada una con su procedencia (no se corrige).
            partes.push(format!("i={:.2} % (soleras {:.2} %)", i * 100.0, g * 100.0));
        }
        (Some(i), _) => partes.push(format!("i={:.2} %", i * 100.0)),
        (None, Some(g)) => partes.push(format!("i={:.2} % (soleras)", g * 100.0)),
        (None, None) => {}
    }
    partes.join(" ")
}

/// Pendiente derivada (m/m) de las soleras declaradas en los extremos del tramo y su longitud en
/// planta: geometría, no hidráulica. `None` si falta alguna solera o la longitud.
fn pendiente_por_soleras(link: &Link) -> Option<f64> {
    let (a, b) = (link.cota_solera_inicio?, link.cota_solera_fin?);
    let l = link.length_2d();
    (a.is_finite() && b.is_finite() && l.is_finite() && l > 0.0).then(|| (a - b) / l)
}

/// Punto a mitad de longitud del trazado y giro (grados) del tramo que lo contiene, siempre
/// legible (entre −90° y 90°).
fn punto_medio(path: &[P3]) -> (P3, f64) {
    let total: f64 = path.windows(2).map(|w| w[0].p2().dist(w[1].p2())).sum();
    let mut resto = total * 0.5;
    for w in path.windows(2) {
        let l = w[0].p2().dist(w[1].p2());
        if l <= 0.0 {
            continue;
        }
        if resto <= l {
            let t = resto / l;
            let q = P3::new(
                w[0].x + (w[1].x - w[0].x) * t,
                w[0].y + (w[1].y - w[0].y) * t,
                w[0].z + (w[1].z - w[0].z) * t,
            );
            return (q, giro_legible(w[0], w[1]));
        }
        resto -= l;
    }
    (path[path.len() / 2], 0.0)
}

fn giro_legible(a: P3, b: P3) -> f64 {
    let mut g = (b.y - a.y).atan2(b.x - a.x).to_degrees();
    if g > 90.0 {
        g -= 180.0;
    } else if g <= -90.0 {
        g += 180.0;
    }
    g
}

fn info_nodo(d: &mut Dibujo, node: &Node) {
    let q = [
        node.pos.x,
        node.pos.y,
        cota_tapa(node)
            .or(cota_solera(node))
            .or(cota_terreno(node))
            .unwrap_or(0.0),
    ];
    // Una cota ausente va vacía: el importador la lee como «sin dato» (F12-03).
    let cota = |z: Option<f64>| z.map(|z| z.to_string()).unwrap_or_default();
    let carga = format!(
        "REDES|1|N|{}|{}|{}|{}|{}|{}|{}|{}|{}",
        node.family,
        node.ty,
        node.code,
        node.estado.key(),
        node.pos.x,
        node.pos.y,
        cota(cota_tapa(node)),
        cota(cota_solera(node)),
        cota(cota_terreno(node))
    );
    d.info(q, &carga);
    info_attrs_datos(d, q, &node.code, &node.attrs, &node.datos);
}

fn info_tramo(d: &mut Dibujo, link: &Link) {
    let medio = link.path[link.path.len() / 2];
    let q = [medio.x, medio.y, medio.z];
    let trazado: Vec<String> = link
        .path
        .iter()
        .map(|v| format!("{:.4},{:.4},{:.4}", v.x, v.y, v.z))
        .collect();
    let carga = format!(
        "REDES|1|L|{}|{}|{}|{}|{}|{}",
        link.family,
        link.ty,
        link.code,
        link.estado.key(),
        dn_mm(link).map(|d| d.to_string()).unwrap_or_default(),
        trazado.join(";")
    );
    d.info(q, &carga);
    info_attrs_datos(d, q, &link.code, &link.attrs, &link.datos);
}

/// `ATTRS|código|k=v;…` y `DATOS|código|k=valor=unidad=fuente;…` (formato que lee el importador).
fn info_attrs_datos(
    d: &mut Dibujo,
    q: [f64; 3],
    code: &str,
    attrs: &BTreeMap<String, Value>,
    datos: &BTreeMap<String, crate::model::Dato>,
) {
    if !attrs.is_empty() {
        let a: Vec<String> = attrs
            .iter()
            .map(|(k, v)| format!("{k}={}", valor_a_texto(v)))
            .collect();
        d.info(q, &format!("ATTRS|{code}|{}", a.join(";")));
    }
    let limpio = |s: &str| s.replace([';', '|', '='], " ");
    let ds: Vec<String> = datos
        .iter()
        .filter(|(_, x)| x.valor.is_finite())
        .map(|(k, x)| {
            format!(
                "{k}={}={}={}",
                x.valor,
                limpio(&x.unidad),
                x.fuente.replace([';', '|'], " ")
            )
        })
        .collect();
    if !ds.is_empty() {
        d.info(q, &format!("DATOS|{code}|{}", ds.join(";")));
    }
}

/// Texto visible en ASCII: los caracteres no ASCII van como `\U+XXXX` (lo decodifican AutoCAD,
/// BricsCAD y ezdxf en cualquier versión de DXF) y los saltos de línea pasan a espacios (un valor
/// DXF ocupa una sola línea).
fn texto_dxf(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\r' | '\n' | '\t' => out.push(' '),
            ' '..='~' => out.push(c),
            c if (c as u32) <= 0xFFFF => {
                write!(out, "\\U+{:04X}", c as u32).ok();
            }
            _ => out.push('?'),
        }
    }
    out
}

/// Nombre de capa en ASCII y sin caracteres prohibidos por AutoCAD (`<>/\":;?*|,=` y comilla
/// invertida): `Tubería de agua potable` → `TB_TUBERIA_DE_AGUA_POTABLE`.
fn layer_name(family_key: &str) -> String {
    let Some(f) = family(family_key) else {
        return "REDES".into();
    };
    let crudo = format!("{}_{}", f.code_prefix, f.label).to_uppercase();
    crudo
        .chars()
        .map(|c| match c {
            'Á' | 'À' | 'Ä' | 'Â' => 'A',
            'É' | 'È' | 'Ë' | 'Ê' => 'E',
            'Í' | 'Ì' | 'Ï' | 'Î' => 'I',
            'Ó' | 'Ò' | 'Ö' | 'Ô' => 'O',
            'Ú' | 'Ù' | 'Ü' | 'Û' => 'U',
            'Ñ' => 'N',
            'Ç' => 'C',
            ' ' => '_',
            '<' | '>' | '/' | '\\' | '"' | ':' | ';' | '?' | '*' | '|' | ',' | '=' | '`' => '_',
            c if c.is_ascii() => c,
            _ => '_',
        })
        .collect()
}

/// Valor de atributo como texto estable para el DXF (`numo`, `int`, `bool`, `texto`).
fn valor_a_texto(v: &crate::families::Value) -> String {
    match v {
        crate::families::Value::Num(n) => format!("{n}"),
        crate::families::Value::Int(i) => format!("{i}"),
        crate::families::Value::Bool(b) => format!("{b}"),
        crate::families::Value::Text(s) => s.replace(';', ",").replace('|', "/"),
    }
}

/// Exporta a GeoJSON (puntos y líneas con sus datos importados).
pub fn export_geojson(p: &Project, path: &Path) -> Result<usize, String> {
    let mut features = Vec::new();
    for n in p.nodes.iter().filter(|x| !x.deleted) {
        let mut props = serde_json::json!({
            "codigo": n.code,
            "familia": n.family,
            "subtipo": n.ty,
            "cota_terreno": cota_terreno(n),
            "estado": n.estado.key(),
            "nota": n.nota,
        });
        for (k, v) in &n.attrs {
            props[k] = serde_json::json!(valor_a_texto(v));
        }
        for (k, d) in &n.datos {
            props[k] =
                serde_json::json!({ "valor": d.valor, "unidad": d.unidad, "fuente": d.fuente });
        }
        // La cota del nodo es su solera (fondo interior), no la clave (F12-01). Va después de
        // los atributos para que el valor tipado no se pise con su copia en texto.
        props["cota_solera"] = serde_json::json!(cota_solera(n));
        features.push(serde_json::json!({
            "type": "Feature",
            "geometry": { "type": "Point", "coordinates": match cota_solera(n) {
                Some(z) => vec![n.pos.x, n.pos.y, z],
                None => vec![n.pos.x, n.pos.y],
            } },
            "properties": props,
        }));
    }
    for l in p.links.iter().filter(|x| !x.deleted) {
        if l.path.len() < 2 {
            continue;
        }
        let mut props = serde_json::json!({
            "codigo": l.code,
            "familia": l.family,
            "subtipo": l.ty,
            "diametro_mm": dn_mm(l),
            "longitud_m": l.length_2d(),
            "estado": l.estado.key(),
            "nota": l.nota,
        });
        for (k, v) in &l.attrs {
            props[k] = serde_json::json!(valor_a_texto(v));
        }
        for (k, d) in &l.datos {
            props[k] =
                serde_json::json!({ "valor": d.valor, "unidad": d.unidad, "fuente": d.fuente });
        }
        // Soleras declaradas del tramo (F12-02); si el origen no las trajo, no se escriben.
        if let Some(z) = l.cota_solera_inicio {
            props["cota_solera_inicio"] = serde_json::json!(z);
        }
        if let Some(z) = l.cota_solera_fin {
            props["cota_solera_fin"] = serde_json::json!(z);
        }
        features.push(serde_json::json!({
            "type": "Feature",
            "geometry": { "type": "LineString", "coordinates": l.path.iter().map(|q| if l.cotas_trazado().is_some() { vec![q.x, q.y, q.z] } else { vec![q.x, q.y] }).collect::<Vec<_>>() },
            "properties": props,
        }));
    }
    let fc = serde_json::json!({
        "type": "FeatureCollection",
        "crs": { "type": "name", "properties": { "name": p.crs } },
        "features": features,
    });
    if let Some(dir) = path.parent()
        && !dir.as_os_str().is_empty()
    {
        std::fs::create_dir_all(dir).map_err(|e| format!("E_IO_PERMISSION: {e}"))?;
    }
    std::fs::write(path, serde_json::to_string_pretty(&fc).unwrap_or_default())
        .map_err(|e| format!("E_IO_PERMISSION: {e}"))?;
    Ok(features.len())
}

/// Resumen de revisión para la barra de estado y los informes.
pub fn revision_summary(p: &Project) -> (usize, usize, usize) {
    let (mut pend, mut ok, mut obs) = (0, 0, 0);
    for e in p
        .nodes
        .iter()
        .filter(|n| !n.deleted)
        .map(|n| n.estado)
        .chain(p.links.iter().filter(|l| !l.deleted).map(|l| l.estado))
    {
        match e {
            EstadoObjeto::Pendiente => pend += 1,
            EstadoObjeto::Aprobado => ok += 1,
            EstadoObjeto::Observado => obs += 1,
        }
    }
    (pend, ok, obs)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// La ida y vuelta por DXF no pierde familia, subtipo, código ni atributos (F8-01).
    #[test]
    fn el_dxf_vuelve_con_los_mismos_objetos() {
        let original = crate::demo_catalog::catalogo_completo();
        let ruta = std::env::temp_dir().join(format!("redes-rt-{}.dxf", std::process::id()));
        let exportados = export_dxf(&original, &ruta).expect("exporta");
        assert!(exportados > 0);

        let mut destino = crate::model::Project::new("vuelta", "EPSG:32717");
        crate::ingest::import_dxf(&mut destino, &ruta, 0.5).expect("importa");

        assert_eq!(
            destino.nodes.len(),
            original.nodes.len(),
            "nodos: {} vs {}",
            destino.nodes.len(),
            original.nodes.len()
        );
        assert_eq!(destino.links.len(), original.links.len(), "arcos");
        for n in &original.nodes {
            let otro = destino
                .nodes
                .iter()
                .find(|x| x.code == n.code)
                .unwrap_or_else(|| panic!("falta {}", n.code));
            assert_eq!(otro.family, n.family, "familia de {}", n.code);
            assert_eq!(otro.ty, n.ty, "subtipo de {}", n.code);
            assert_eq!(
                otro.attrs.len(),
                n.attrs.len(),
                "atributos de {}: {:?} vs {:?}",
                n.code,
                otro.attrs.keys().collect::<Vec<_>>(),
                n.attrs.keys().collect::<Vec<_>>()
            );
            assert!((otro.pos.x - n.pos.x).abs() < 1e-3, "x de {}", n.code);
            assert!((otro.z_rim - n.z_rim).abs() < 1e-3, "cota de {}", n.code);
        }
        let _ = std::fs::remove_file(&ruta);
    }

    #[test]
    fn el_dxf_sale_con_entidades() {
        let p = crate::demo::project();
        let dir = std::env::temp_dir().join("redes_dxf_test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("p.dxf");
        let n = export_dxf(&p, &path).unwrap();
        assert!(n > 10);
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("POLYLINE"));
        assert!(text.ends_with("EOF\n"));
        let _ = std::fs::remove_file(&path);
    }

    // --- Lector independiente de DXF (pares grupo/valor) para verificar la estructura (F12-08) ---

    type Ent = Vec<(i32, String)>;

    /// Lee el archivo en pares y lo parte en secciones → entidades. No usa nada del exportador.
    struct Leido {
        cabecera: BTreeMap<String, Vec<(i32, String)>>,
        capas: BTreeMap<String, i32>,
        bloques: BTreeMap<String, Vec<Ent>>,
        entidades: Vec<(String, Ent)>,
    }

    fn lee_dxf(texto: &str) -> Leido {
        let lineas: Vec<&str> = texto.lines().collect();
        assert!(lineas.len().is_multiple_of(2), "número impar de líneas");
        let pares: Vec<(i32, String)> = lineas
            .chunks(2)
            .map(|c| {
                let g = c[0].trim().parse::<i32>().unwrap_or_else(|_| {
                    panic!("código de grupo no numérico: {:?}", c[0]);
                });
                (g, c[1].to_string())
            })
            .collect();
        assert_eq!(pares.last().map(|p| p.1.as_str()), Some("EOF"));
        // Registros: cada `0` abre uno.
        let mut regs: Vec<(String, Ent)> = Vec::new();
        for (g, v) in pares {
            if g == 0 {
                regs.push((v, Vec::new()));
            } else {
                regs.last_mut()
                    .expect("par antes del primer 0")
                    .1
                    .push((g, v));
            }
        }
        let mut l = Leido {
            cabecera: BTreeMap::new(),
            capas: BTreeMap::new(),
            bloques: BTreeMap::new(),
            entidades: Vec::new(),
        };
        let mut seccion = String::new();
        let mut bloque: Option<String> = None;
        let mut secciones = Vec::new();
        for (tipo, datos) in regs {
            let val = |g: i32| datos.iter().find(|p| p.0 == g).map(|p| p.1.clone());
            match tipo.as_str() {
                "SECTION" => {
                    seccion = val(2).expect("sección sin nombre");
                    secciones.push(seccion.clone());
                    if seccion == "HEADER" {
                        let mut var = String::new();
                        for (g, v) in &datos[1..] {
                            if *g == 9 {
                                var = v.clone();
                            } else {
                                l.cabecera
                                    .entry(var.clone())
                                    .or_default()
                                    .push((*g, v.clone()));
                            }
                        }
                    }
                }
                "ENDSEC" => seccion.clear(),
                "EOF" | "TABLE" | "ENDTAB" | "SEQEND" => {}
                "LAYER" if seccion == "TABLES" => {
                    let color = val(62).expect("capa sin color").parse().unwrap();
                    assert!(val(6).is_some(), "capa sin tipo de línea");
                    l.capas.insert(val(2).expect("capa sin nombre"), color);
                }
                _ if seccion == "TABLES" => {}
                "BLOCK" => {
                    let n = val(2).expect("bloque sin nombre");
                    l.bloques.insert(n.clone(), Vec::new());
                    bloque = Some(n);
                }
                "ENDBLK" => bloque = None,
                _ if seccion == "BLOCKS" => {
                    let b = bloque.clone().expect("entidad fuera de BLOCK");
                    l.bloques.get_mut(&b).unwrap().push(datos);
                }
                _ if seccion == "ENTITIES" => l.entidades.push((tipo, datos)),
                otro => panic!("registro {otro} fuera de sección ({seccion})"),
            }
        }
        assert_eq!(secciones, ["HEADER", "TABLES", "BLOCKS", "ENTITIES"]);
        l
    }

    fn g(e: &Ent, grupo: i32) -> Option<&str> {
        e.iter().find(|p| p.0 == grupo).map(|p| p.1.as_str())
    }
    fn gf(e: &Ent, grupo: i32) -> f64 {
        g(e, grupo)
            .unwrap_or_else(|| panic!("falta el grupo {grupo}"))
            .parse()
            .unwrap()
    }

    fn exporta(p: &Project, nombre: &str) -> String {
        let ruta = std::env::temp_dir().join(format!("redes-{nombre}-{}.dxf", std::process::id()));
        export_dxf(p, &ruta).expect("exporta");
        let t = std::fs::read_to_string(&ruta).unwrap();
        let _ = std::fs::remove_file(&ruta);
        t
    }

    /// F12-08: cabecera con unidades y página de códigos, y tabla de capas con todas las capas
    /// usadas, en ASCII y con color por sistema.
    #[test]
    fn el_dxf_declara_unidades_codificacion_y_capas() {
        let l = lee_dxf(&exporta(&crate::demo_catalog::catalogo_completo(), "cab"));
        assert_eq!(l.cabecera["$INSUNITS"], [(70, "6".to_string())], "metros");
        assert_eq!(l.cabecera["$DWGCODEPAGE"], [(3, "ANSI_1252".to_string())]);
        assert_eq!(l.cabecera["$ACADVER"], [(1, "AC1009".to_string())]);
        assert!(l.cabecera.contains_key("$EXTMIN") && l.cabecera.contains_key("$EXTMAX"));
        assert!(l.capas.len() > 5, "tabla LAYER con entradas: {:?}", l.capas);
        for (nombre, color) in &l.capas {
            assert!(nombre.is_ascii(), "capa no ASCII: {nombre}");
            assert!(
                !nombre.contains(['<', '>', '/', '\\', '"', ':', ';', '?', '*', '|', ',', '=']),
                "capa con carácter prohibido: {nombre}"
            );
            assert!(
                (1..=255).contains(&color.abs()),
                "color ACI de {nombre}: {color}"
            );
        }
        // Toda capa usada por una entidad está declarada.
        for (tipo, e) in &l.entidades {
            let c = g(e, 8).unwrap_or_else(|| panic!("{tipo} sin capa"));
            assert!(l.capas.contains_key(c), "{tipo} en capa no declarada {c}");
        }
        // Colores por sistema: agua potable azul, sanitario naranja; datos apagados.
        assert_eq!(l.capas["TB_TUBERIA_DE_AGUA_POTABLE"], 5);
        assert_eq!(l.capas["CL_TUBERIA"], 30);
        assert_eq!(l.capas[CAPA_OBSERVACIONES], 1);
        assert!(l.capas[CAPA_INFO] < 0, "REDES_INFO apagada");
        // Lo visible es ASCII (lo único con UTF-8 es la carga de REDES_INFO).
        for (tipo, e) in &l.entidades {
            if g(e, 8) != Some(CAPA_INFO) {
                for (_, v) in e {
                    assert!(v.is_ascii(), "{tipo} con texto no ASCII: {v}");
                }
            }
        }
    }

    /// F12-08: el bloque del pozo mide DI (radio DI/2), la línea va de la tapa a la solera hacia
    /// abajo y los atributos llevan código y cotas.
    #[test]
    fn el_pozo_sale_con_su_diametro_su_altura_y_sus_atributos() {
        let p = crate::demo::project();
        let pozo = p
            .nodes
            .iter()
            .find(|n| n.family == "saneamiento.pozo" && n.num("diametro_interior").is_some())
            .expect("la demo tiene pozos");
        let l = lee_dxf(&exporta(&p, "pozo"));
        let ins = l
            .entidades
            .iter()
            .position(|(t, e)| {
                t == "INSERT"
                    && (gf(e, 10) - pozo.pos.x).abs() < 1e-3
                    && (gf(e, 20) - pozo.pos.y).abs() < 1e-3
            })
            .expect("INSERT del pozo");
        let e = &l.entidades[ins].1;
        let bloque = &l.bloques[g(e, 2).unwrap()];
        let circ = bloque
            .iter()
            .find(|b| b.iter().any(|p| p.0 == 40) && g(b, 2).is_none())
            .expect("círculo en el bloque");
        let radio = gf(circ, 40) * gf(e, 41);
        assert!(
            (radio - pozo.diametro() / 2.0).abs() < 1e-3,
            "radio dibujado {radio} para DI {}",
            pozo.diametro()
        );
        assert!((gf(e, 30) - pozo.z_rim).abs() < 1e-3, "inserto en la tapa");
        // Atributos tras el INSERT hasta el SEQEND.
        let mut attribs = BTreeMap::new();
        for (t, a) in &l.entidades[ins + 1..] {
            if t != "ATTRIB" {
                break;
            }
            attribs.insert(g(a, 2).unwrap().to_string(), g(a, 1).unwrap().to_string());
        }
        assert_eq!(attribs["CODIGO"], pozo.code);
        assert_eq!(attribs["COTA_TAPA"], format!("{:.3}", pozo.z_rim));
        assert_eq!(attribs["COTA_SOLERA"], format!("{:.3}", pozo.z_invert));
        assert_eq!(
            attribs["PROFUNDIDAD"],
            format!("{:.3}", pozo.z_rim - pozo.z_invert)
        );
        // Cada ATTRIB tiene su ATTDEF en el bloque.
        let defs: Vec<&str> = bloque.iter().filter_map(|b| g(b, 2)).collect();
        assert!(attribs.keys().all(|k| defs.contains(&k.as_str())));

        let linea = l
            .entidades
            .iter()
            .find(|(t, e)| {
                t == "LINE"
                    && (gf(e, 10) - pozo.pos.x).abs() < 1e-3
                    && (gf(e, 20) - pozo.pos.y).abs() < 1e-3
            })
            .map(|(_, e)| e)
            .expect("línea del pozo");
        assert!(
            (gf(linea, 30) - pozo.z_rim).abs() < 1e-3,
            "arranca en la tapa"
        );
        assert!(
            (gf(linea, 31) - pozo.z_invert).abs() < 1e-3,
            "baja a la solera"
        );

        // Rótulo del pozo en su capa de rotulación.
        let rot: Vec<&str> = l
            .entidades
            .iter()
            .filter(|(t, e)| t == "TEXT" && g(e, 8) == Some("ROT_SAN_NODOS"))
            .filter_map(|(_, e)| g(e, 1))
            .collect();
        assert!(rot.contains(&pozo.code.as_str()));
        assert!(rot.contains(&format!("CT {:.2}", pozo.z_rim).as_str()));
        assert!(rot.contains(&format!("CS {:.2}", pozo.z_invert).as_str()));
        assert!(rot.contains(&format!("H {:.2}", pozo.z_rim - pozo.z_invert).as_str()));
    }

    /// F12-08: un pozo sin solera ni diámetro declarados no recibe cotas ni diámetro inventados.
    #[test]
    fn el_pozo_sin_datos_no_inventa_cotas() {
        let mut p = crate::demo::project();
        let i = p
            .nodes
            .iter()
            .position(|n| n.family == "saneamiento.pozo")
            .unwrap();
        p.nodes[i].z_invert = f64::NAN;
        p.nodes[i].attrs.remove("diametro_interior");
        let pozo = p.nodes[i].clone();
        let l = lee_dxf(&exporta(&p, "sindatos"));
        let en_pozo = |e: &Ent| {
            (gf(e, 10) - pozo.pos.x).abs() < 1e-3 && (gf(e, 20) - pozo.pos.y).abs() < 1e-3
        };
        let ins = l
            .entidades
            .iter()
            .position(|(t, e)| t == "INSERT" && en_pozo(e))
            .unwrap();
        assert_eq!(
            gf(&l.entidades[ins].1, 41),
            1.0,
            "sin DI: símbolo a escala 1"
        );
        let etiquetas: Vec<&str> = l.entidades[ins + 1..]
            .iter()
            .take_while(|(t, _)| t == "ATTRIB")
            .filter_map(|(_, a)| g(a, 2))
            .collect();
        assert!(etiquetas.contains(&"CODIGO") && etiquetas.contains(&"COTA_TAPA"));
        assert!(!etiquetas.contains(&"COTA_SOLERA"), "{etiquetas:?}");
        assert!(!etiquetas.contains(&"PROFUNDIDAD"), "{etiquetas:?}");
        assert!(
            !l.entidades
                .iter()
                .any(|(t, e)| (t == "LINE" || t == "POINT") && en_pozo(e)),
            "sin solera no hay línea ni punto"
        );
        // La carga de REDES_INFO no cambia de formato (F8); lo visible no lleva NaN.
        for (_, e) in l
            .entidades
            .iter()
            .filter(|(t, e)| t == "TEXT" && g(e, 8) != Some(CAPA_INFO))
        {
            let v = g(e, 1).unwrap_or("");
            assert!(!v.contains("NaN"), "texto con NaN: {v}");
        }
    }

    /// F12-08: rótulo de tramo con DN, material, longitud y pendiente; sin dato no hay rótulo de
    /// ese dato; la pendiente derivada de las soleras se marca, y si no cuadra con la declarada
    /// salen las dos.
    #[test]
    fn el_tramo_se_rotula_solo_con_lo_que_existe() {
        let mut p = crate::demo::project();
        let i = p
            .links
            .iter()
            .position(|x| x.get("material").is_some() && x.num("pendiente").is_some())
            .expect("tramo con material y pendiente");
        let otros: Vec<usize> = (0..p.links.len()).filter(|k| *k != i).take(3).collect();
        let (j, k, m) = (otros[0], otros[1], otros[2]);
        // Tramo j: sin material, sin pendiente, sin DN y sin soleras declaradas.
        for x in [j, k] {
            p.links[x].attrs.remove("material");
            p.links[x].attrs.remove("pendiente");
            p.links[x].d_mm = 0;
            p.links[x].cota_solera_inicio = None;
            p.links[x].cota_solera_fin = None;
        }
        // Tramo k: solo soleras declaradas (1 m de caída).
        p.links[k].cota_solera_inicio = Some(100.0);
        p.links[k].cota_solera_fin = Some(99.0);
        // Tramo m: pendiente declarada que no cuadra con sus soleras.
        p.links[m]
            .attrs
            .insert("pendiente".into(), Value::Num(0.02));
        p.links[m].cota_solera_inicio = Some(100.0);
        p.links[m].cota_solera_fin = Some(100.0);
        let (ti, tj, tk, tm) = (
            p.links[i].clone(),
            p.links[j].clone(),
            p.links[k].clone(),
            p.links[m].clone(),
        );
        let l = lee_dxf(&exporta(&p, "tramo"));
        let rotulos: Vec<String> = l
            .entidades
            .iter()
            .filter(|(t, e)| t == "TEXT" && g(e, 8).is_some_and(|c| c.ends_with("_TRAMOS")))
            .filter_map(|(_, e)| g(e, 1).map(String::from))
            .collect();
        let de = |code: &str| {
            rotulos
                .iter()
                .find(|r| r.split(' ').next() == Some(code))
                .unwrap_or_else(|| panic!("sin rótulo para {code}: {rotulos:?}"))
                .clone()
        };
        let ri = de(&ti.code);
        assert!(ri.contains(&format!("DN{}", ti.d_mm)), "{ri}");
        let mat = ti.get("material").unwrap().as_str().unwrap().to_uppercase();
        assert!(ri.contains(&mat), "{ri}");
        assert!(ri.contains(&format!("L={:.2} m", ti.length_2d())), "{ri}");
        let pend = ti.num("pendiente").unwrap() * 100.0;
        assert!(ri.contains(&format!("i={pend:.2} %")), "{ri}");
        assert!(!ri.contains("soleras"), "pendiente declarada: {ri}");

        let rj = de(&tj.code);
        assert!(!rj.contains("DN"), "sin DN no se rotula DN: {rj}");
        assert!(!rj.contains(&mat), "sin material no se inventa: {rj}");
        assert!(
            !rj.contains("i="),
            "sin pendiente ni soleras no hay pendiente: {rj}"
        );
        assert!(rj.contains("L="), "la longitud es geometría: {rj}");

        let rk = de(&tk.code);
        let g_k = 100.0 / tk.length_2d();
        assert!(rk.contains(&format!("i={g_k:.2} % (soleras)")), "{rk}");

        let rm = de(&tm.code);
        assert!(
            rm.contains("i=2.00 % (soleras 0.00 %)"),
            "discrepancia: {rm}"
        );
    }

    /// F12-08: notas y estados de la revisión en la capa OBSERVACIONES, con tildes escapadas.
    #[test]
    fn las_observaciones_van_a_su_capa() {
        let mut p = crate::demo::project();
        p.nodes[0].estado = EstadoObjeto::Observado;
        p.nodes[0].nota = Some("Revisar cota de tubería\nen campo".into());
        p.links[0].nota = Some("Falta junta".into());
        let (cn, cl) = (p.nodes[0].code.clone(), p.links[0].code.clone());
        let l = lee_dxf(&exporta(&p, "obs"));
        let obs: Vec<&Ent> = l
            .entidades
            .iter()
            .filter(|(_, e)| g(e, 8) == Some(CAPA_OBSERVACIONES))
            .map(|(_, e)| e)
            .collect();
        let textos: Vec<&str> = obs.iter().filter_map(|e| g(e, 1)).collect();
        assert!(
            textos.contains(
                &format!("{cn} OBSERVADO: Revisar cota de tuber\\U+00EDa en campo").as_str()
            ),
            "{textos:?}"
        );
        assert!(
            textos
                .iter()
                .any(|t| t.starts_with(&cl) && t.ends_with("Falta junta")),
            "{textos:?}"
        );
        // Nube alrededor del observado.
        assert!(obs.iter().any(|e| g(e, 1).is_none() && g(e, 40).is_some()));
        // Un pendiente sin nota no genera observación.
        let pendiente = p
            .nodes
            .iter()
            .find(|n| n.estado == EstadoObjeto::Pendiente && n.nota.is_none())
            .unwrap();
        assert!(
            !textos
                .iter()
                .any(|t| t.starts_with(&format!("{} ", pendiente.code)))
        );
    }

    /// F12-03: un pozo sin terreno y un tramo sin DN no se rotulan y siguen sin ese dato tras
    /// exportar e importar.
    #[test]
    fn lo_que_falta_no_se_rotula_y_vuelve_como_falta() {
        let mut p = crate::demo::project();
        p.nodes[0].z_terrain = -777.0;
        p.nodes[0].sin_dato = vec![crate::model::COTA_TERRENO.to_string()];
        p.links[0].d_mm = 200;
        p.links[0].sin_dato = vec![crate::model::DIAMETRO_NOMINAL.to_string()];
        let (cn, cl) = (p.nodes[0].code.clone(), p.links[0].code.clone());
        let t = exporta(&p, "faltas");
        assert!(!t.contains("-777"), "el valor de dibujo no se escribe");
        let l = lee_dxf(&t);
        for (_, e) in &l.entidades {
            if let Some(x) = g(e, 1)
                && x.starts_with(&cl)
                && g(e, 8).is_some_and(|c| c.starts_with("ROT_"))
            {
                assert!(!x.contains("DN"), "{x}");
            }
        }
        let ruta = std::env::temp_dir().join(format!("redes-faltas-{}.dxf", std::process::id()));
        export_dxf(&p, &ruta).unwrap();
        let mut d = crate::model::Project::new("vuelta", "EPSG:32717");
        crate::ingest::import_dxf(&mut d, &ruta, 0.5).unwrap();
        let _ = std::fs::remove_file(&ruta);
        let n = d.nodes.iter().find(|x| x.code == cn).unwrap();
        assert_eq!(n.cota_terreno(), None);
        let k = d.links.iter().find(|x| x.code == cl).unwrap();
        assert_eq!(k.dn(), None);
        let n1 = d.nodes.iter().find(|x| x.code == p.nodes[1].code).unwrap();
        assert!(n1.cota_terreno().is_some());
    }

    /// Los datos de los tramos también vuelven (antes no se escribían y los de los nodos salían en
    /// un formato que el importador descartaba).
    #[test]
    fn los_datos_vuelven_por_el_dxf() {
        let mut original = crate::demo::project();
        original.links[0].datos.insert(
            "caudal".into(),
            crate::model::Dato {
                valor: 12.5,
                unidad: "L/s".into(),
                fuente: "SWMM".into(),
            },
        );
        let ruta = std::env::temp_dir().join(format!("redes-datos-{}.dxf", std::process::id()));
        export_dxf(&original, &ruta).unwrap();
        let mut destino = crate::model::Project::new("vuelta", "EPSG:32717");
        crate::ingest::import_dxf(&mut destino, &ruta, 0.5).unwrap();
        let _ = std::fs::remove_file(&ruta);
        for n in original.nodes.iter().filter(|n| !n.deleted) {
            let otro = destino.nodes.iter().find(|x| x.code == n.code).unwrap();
            assert_eq!(otro.datos.len(), n.datos.len(), "datos de {}", n.code);
        }
        let l0 = &original.links[0];
        let otro = destino.links.iter().find(|x| x.code == l0.code).unwrap();
        let d = &otro.datos["caudal"];
        assert_eq!(
            (d.valor, d.unidad.as_str(), d.fuente.as_str()),
            (12.5, "L/s", "SWMM")
        );
    }

    #[test]
    fn el_geojson_tiene_features() {
        let p = crate::demo::project();
        let dir = std::env::temp_dir().join("redes_geo_test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("p.geojson");
        let n = export_geojson(&p, &path).unwrap();
        assert_eq!(n, p.count());
        let _ = std::fs::remove_file(&path);
    }

    /// El GeoJSON escribe la solera del nodo como `cota_solera`, nunca como `cota_clave` (F12-01).
    #[test]
    fn el_geojson_exporta_la_solera_y_no_la_llama_clave() {
        let p = crate::demo::project();
        let path =
            std::env::temp_dir().join(format!("redes-solera-{}.geojson", std::process::id()));
        export_geojson(&p, &path).unwrap();
        let fc: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let _ = std::fs::remove_file(&path);
        let features = fc["features"].as_array().unwrap();
        let mut puntos = 0;
        for f in features.iter().filter(|f| f["geometry"]["type"] == "Point") {
            let props = &f["properties"];
            assert!(
                props.get("cota_clave").is_none(),
                "nodo con cota_clave: {props}"
            );
            let codigo = props["codigo"].as_str().unwrap();
            let n = p.nodes.iter().find(|n| n.code == codigo).unwrap();
            assert_eq!(props["cota_solera"].as_f64(), Some(n.z_invert), "{codigo}");
            puntos += 1;
        }
        assert!(puntos > 0);
    }
}
