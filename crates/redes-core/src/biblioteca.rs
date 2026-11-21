//! Lectura y revisión de modelos glTF 2.0 (`.glb`) para la biblioteca de piezas.
//!
//! Es el camino inverso de [`crate::gltf`]: un modelo hecho en Blender se exporta a `.glb`, se lee
//! aquí y se coloca en el visor. Por ahora **lee y valida**; la colocación en la escena depende del
//! manifiesto de la biblioteca (ver `docs/17-biblioteca-modelos.md`).

use serde_json::Value;

/// Caja envolvente en coordenadas del modelo.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Bbox3 {
    pub min: [f64; 3],
    pub max: [f64; 3],
}

impl Bbox3 {
    pub fn vacia() -> Bbox3 {
        Bbox3 {
            min: [f64::INFINITY; 3],
            max: [f64::NEG_INFINITY; 3],
        }
    }
    pub fn add(&mut self, p: [f64; 3]) {
        for ((mn, mx), v) in self.min.iter_mut().zip(self.max.iter_mut()).zip(p) {
            *mn = mn.min(v);
            *mx = mx.max(v);
        }
    }
    pub fn dimensiones(&self) -> [f64; 3] {
        [
            self.max[0] - self.min[0],
            self.max[1] - self.min[1],
            self.max[2] - self.min[2],
        ]
    }
    pub fn centro(&self) -> [f64; 3] {
        [
            (self.min[0] + self.max[0]) * 0.5,
            (self.min[1] + self.max[1]) * 0.5,
            (self.min[2] + self.max[2]) * 0.5,
        ]
    }
}

/// Una primitiva (una malla con un material).
#[derive(Clone, Debug, Default)]
pub struct Primitiva {
    pub nombre: String,
    pub material: Option<String>,
    pub triangulos: usize,
    pub vertices: usize,
    pub con_normales: bool,
    pub con_colores: bool,
    pub bbox: Bbox3,
}

/// Lo que se puede decir de un `.glb` sin colocarlo en la escena.
#[derive(Clone, Debug, Default)]
pub struct Modelo {
    pub nombre: String,
    pub primitivas: Vec<Primitiva>,
    pub triangulos: usize,
    pub bbox: Bbox3,
    pub materiales: Vec<String>,
    /// Giro del nodo raíz, si lo hay (para detectar Y-arriba).
    pub giro: Option<[f64; 4]>,
    /// Datos maestros declarados en Blender (propiedades personalizadas → `extras` de glTF).
    pub extras: std::collections::BTreeMap<String, String>,
    pub avisos: Vec<String>,
}

/// Aplana un objeto `extras` de glTF en pares clave → texto.
fn leer_extras(
    valor: &Value,
    prefijo: &str,
    destino: &mut std::collections::BTreeMap<String, String>,
) {
    let Some(obj) = valor.as_object() else { return };
    for (k, v) in obj {
        let clave = format!("{prefijo}.{k}");
        let texto = match v {
            Value::String(s) => s.clone(),
            Value::Number(n) => n.to_string(),
            Value::Bool(b) => b.to_string(),
            otro => otro.to_string(),
        };
        destino.insert(clave, texto);
    }
}

/// Medidas sacadas de la geometría (no declaradas): volumen, área y si la malla es cerrada.
#[derive(Clone, Debug, Default)]
pub struct Cantidades {
    /// Volumen firmado; en una malla cerrada y bien orientada es positivo.
    pub volumen_m3: f64,
    pub area_m2: f64,
    /// Volumen por material (la clave es el nombre del material o `modelo`).
    pub por_material: std::collections::BTreeMap<String, f64>,
    /// Aristas que pertenecen a un solo triángulo: si hay, la malla está abierta.
    pub aristas_abiertas: usize,
}

impl Cantidades {
    pub fn cerrada(&self) -> bool {
        self.aristas_abiertas == 0
    }
    /// Peso estimado con una densidad (t/m³).
    pub fn peso_t(&self, densidad: f64) -> f64 {
        self.volumen_m3.abs() * densidad
    }
}

/// Calcula volumen y área de un `.glb`.
///
/// El volumen se obtiene por el teorema de la divergencia (suma de tetraedros desde el origen), así
/// que solo es fiable en mallas cerradas; [`Cantidades::cerrada`] avisa de lo contrario.
pub fn cantidades_glb(bytes: &[u8]) -> Result<Cantidades, String> {
    let (doc, bin) = abrir_glb(bytes)?;
    let mut c = Cantidades::default();
    let mut aristas: std::collections::HashMap<(i64, i64, i64, i64, i64, i64), u32> =
        std::collections::HashMap::new();

    let materiales: Vec<String> = doc
        .get("materials")
        .and_then(|m| m.as_array())
        .map(|a| {
            a.iter()
                .enumerate()
                .map(|(i, m)| {
                    m.get("name")
                        .and_then(|n| n.as_str())
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| format!("material_{i}"))
                })
                .collect()
        })
        .unwrap_or_default();

    let raices: Vec<usize> = match doc
        .get("scenes")
        .and_then(|s| s.as_array())
        .and_then(|s| s.first())
        .and_then(|s| s.get("nodes"))
    {
        Some(Value::Array(a)) => a
            .iter()
            .filter_map(|v| v.as_u64())
            .map(|v| v as usize)
            .collect(),
        _ => (0..doc
            .get("nodes")
            .and_then(|n| n.as_array())
            .map(|a| a.len())
            .unwrap_or(0))
            .collect(),
    };
    let mut pendientes: Vec<(usize, [f64; 16])> =
        raices.into_iter().map(|i| (i, identidad())).collect();
    let mut visitados = 0;
    while let Some((indice, padre)) = pendientes.pop() {
        visitados += 1;
        if visitados > 10_000 {
            break;
        }
        let Some(nodo) = doc.get("nodes").and_then(|n| n.get(indice)) else {
            continue;
        };
        let mundo = multiplicar(padre, matriz_del_nodo(nodo));
        if let Some(malla_id) = nodo.get("mesh").and_then(|m| m.as_u64())
            && let Some(prims) = doc
                .get("meshes")
                .and_then(|m| m.get(malla_id as usize))
                .and_then(|m| m.get("primitives"))
                .and_then(|p| p.as_array())
        {
            for prim in prims {
                let material = prim
                    .get("material")
                    .and_then(|m| m.as_u64())
                    .map(|m| {
                        materiales
                            .get(m as usize)
                            .cloned()
                            .unwrap_or_else(|| format!("material_{m}"))
                    })
                    .unwrap_or_else(|| "modelo".to_string());
                let Some(pos) = prim
                    .get("attributes")
                    .and_then(|a| a.get("POSITION"))
                    .and_then(|v| v.as_u64())
                    .and_then(|i| leer_vec3(&doc, bin, i as usize))
                else {
                    continue;
                };
                let puntos: Vec<[f64; 3]> = pos.iter().map(|q| transformar(&mundo, *q)).collect();
                let indices = prim
                    .get("indices")
                    .and_then(|v| v.as_u64())
                    .and_then(|i| leer_escalares(&doc, bin, i as usize))
                    .unwrap_or_else(|| (0..puntos.len() as u32).collect());

                for tri in indices.chunks(3) {
                    if tri.len() < 3 {
                        break;
                    }
                    let (ia, ib, ic) = (tri[0] as usize, tri[1] as usize, tri[2] as usize);
                    if ia >= puntos.len() || ib >= puntos.len() || ic >= puntos.len() {
                        continue;
                    }
                    let (a, b, cc) = (puntos[ia], puntos[ib], puntos[ic]);
                    // Volumen del tetraedro (0, a, b, c).
                    let vol = (a[0] * (b[1] * cc[2] - b[2] * cc[1])
                        - a[1] * (b[0] * cc[2] - b[2] * cc[0])
                        + a[2] * (b[0] * cc[1] - b[1] * cc[0]))
                        / 6.0;
                    c.volumen_m3 += vol;
                    *c.por_material.entry(material.clone()).or_insert(0.0) += vol;

                    let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
                    let v = [cc[0] - a[0], cc[1] - a[1], cc[2] - a[2]];
                    let n = [
                        u[1] * v[2] - u[2] * v[1],
                        u[2] * v[0] - u[0] * v[2],
                        u[0] * v[1] - u[1] * v[0],
                    ];
                    c.area_m2 += 0.5 * (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();

                    // Aristas (cuantizadas a 0,1 mm) para saber si la malla cierra.
                    for (p, q) in [(ia, ib), (ib, ic), (ic, ia)] {
                        let k = arista(puntos[p], puntos[q]);
                        *aristas.entry(k).or_insert(0) += 1;
                    }
                }
            }
        }
        if let Some(hijos) = nodo.get("children").and_then(|c| c.as_array()) {
            for h in hijos.iter().filter_map(|v| v.as_u64()) {
                pendientes.push((h as usize, mundo));
            }
        }
    }
    c.aristas_abiertas = aristas.values().filter(|n| **n == 1).count();
    Ok(c)
}

fn arista(p: [f64; 3], q: [f64; 3]) -> (i64, i64, i64, i64, i64, i64) {
    let k = 10_000.0;
    let a = (
        (p[0] * k).round() as i64,
        (p[1] * k).round() as i64,
        (p[2] * k).round() as i64,
    );
    let b = (
        (q[0] * k).round() as i64,
        (q[1] * k).round() as i64,
        (q[2] * k).round() as i64,
    );
    if a <= b {
        (a.0, a.1, a.2, b.0, b.1, b.2)
    } else {
        (b.0, b.1, b.2, a.0, a.1, a.2)
    }
}

/// Grado de dureza de un aviso.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gravedad {
    Info,
    Aviso,
    Error,
}

#[derive(Clone, Debug)]
pub struct Aviso {
    pub gravedad: Gravedad,
    pub texto: String,
}

/// Qué se espera del modelo para una familia (para validar escala y presupuesto).
#[derive(Clone, Copy, Debug)]
pub struct Expectativa {
    /// Tamaño máximo razonable en metros (la dimensión mayor).
    pub max_m: f64,
    /// Tamaño mínimo razonable en metros.
    pub min_m: f64,
    /// Triángulos máximos para el visor con GPU.
    pub max_triangulos: usize,
}

impl Expectativa {
    /// Valores por defecto para una pieza de red (válvula, pozo, cámara…).
    pub fn pieza() -> Expectativa {
        Expectativa {
            max_m: 30.0,
            min_m: 0.05,
            max_triangulos: 120_000,
        }
    }
    /// Valores para una planta de tratamiento.
    pub fn planta() -> Expectativa {
        Expectativa {
            max_m: 400.0,
            min_m: 2.0,
            max_triangulos: 400_000,
        }
    }
}

/// Lee un `.glb` y resume su contenido.
pub fn leer_glb(bytes: &[u8]) -> Result<Modelo, String> {
    if bytes.len() < 20 {
        return Err("E_GLB_CORTO: el archivo no tiene cabecera glTF".into());
    }
    let magia = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    if magia != 0x4654_6C67 {
        return Err("E_GLB_MAGIA: no es un .glb (¿un .gltf de texto?)".into());
    }
    let version = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
    if version != 2 {
        return Err(format!("E_GLB_VERSION: glTF {version}, se espera 2"));
    }
    let total = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]) as usize;
    if total > bytes.len() {
        return Err(format!(
            "E_GLB_TRUNCADO: declara {total} bytes y hay {}",
            bytes.len()
        ));
    }
    let largo_json = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]) as usize;
    if 20 + largo_json > bytes.len() {
        return Err("E_GLB_JSON: el bloque JSON se sale del archivo".into());
    }
    let json = std::str::from_utf8(&bytes[20..20 + largo_json])
        .map_err(|e| format!("E_GLB_JSON: {e}"))?
        .trim_end_matches([' ', '\0'])
        .to_string();
    let doc: Value = serde_json::from_str(&json).map_err(|e| format!("E_GLB_JSON: {e}"))?;

    // Bloque binario (opcional si todo viene en data: URIs, que no soportamos).
    let mut bin: &[u8] = &[];
    let offset_bin = 20 + largo_json;
    if offset_bin + 8 <= bytes.len() {
        let largo = u32::from_le_bytes([
            bytes[offset_bin],
            bytes[offset_bin + 1],
            bytes[offset_bin + 2],
            bytes[offset_bin + 3],
        ]) as usize;
        let tipo = u32::from_le_bytes([
            bytes[offset_bin + 4],
            bytes[offset_bin + 5],
            bytes[offset_bin + 6],
            bytes[offset_bin + 7],
        ]);
        if tipo == 0x004E_4942 && offset_bin + 8 + largo <= bytes.len() {
            bin = &bytes[offset_bin + 8..offset_bin + 8 + largo];
        }
    }

    let mut modelo = Modelo {
        nombre: doc
            .get("asset")
            .and_then(|a| a.get("generator"))
            .and_then(|g| g.as_str())
            .unwrap_or("sin nombre")
            .to_string(),
        bbox: Bbox3::vacia(),
        ..Modelo::default()
    };

    // Materiales por índice.
    let materiales: Vec<String> = doc
        .get("materials")
        .and_then(|m| m.as_array())
        .map(|arr| {
            arr.iter()
                .enumerate()
                .map(|(i, m)| {
                    m.get("name")
                        .and_then(|n| n.as_str())
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| format!("material_{i}"))
                })
                .collect()
        })
        .unwrap_or_default();
    modelo.materiales = materiales.clone();

    if let Some(extras) = doc.get("asset").and_then(|a| a.get("extras")) {
        leer_extras(extras, "asset", &mut modelo.extras);
    }

    let escenas = doc.get("scenes").and_then(|s| s.as_array());
    let nodos_raiz: Vec<usize> = match escenas.and_then(|s| s.first()).and_then(|s| s.get("nodes"))
    {
        Some(Value::Array(a)) => a
            .iter()
            .filter_map(|v| v.as_u64())
            .map(|v| v as usize)
            .collect(),
        _ => (0..doc
            .get("nodes")
            .and_then(|n| n.as_array())
            .map(|a| a.len())
            .unwrap_or(0))
            .collect(),
    };

    let mut pendientes: Vec<(usize, [f64; 16])> =
        nodos_raiz.into_iter().map(|i| (i, identidad())).collect();
    let mut visitados = 0usize;

    while let Some((indice, padre)) = pendientes.pop() {
        visitados += 1;
        if visitados > 10_000 {
            modelo
                .avisos
                .push("E_GLB_CICLO: demasiados nodos".to_string());
            break;
        }
        let Some(nodo) = doc.get("nodes").and_then(|n| n.get(indice)) else {
            continue;
        };
        let local = matriz_del_nodo(nodo);
        if modelo.giro.is_none()
            && let Some(giro) = nodo.get("rotation").and_then(|r| r.as_array())
            && giro.len() == 4
        {
            let q: Vec<f64> = giro.iter().filter_map(|v| v.as_f64()).collect();
            if q.len() == 4 {
                modelo.giro = Some([q[0], q[1], q[2], q[3]]);
            }
        }
        let mundo = multiplicar(padre, local);

        if let Some(extras) = nodo.get("extras") {
            leer_extras(extras, "nodo", &mut modelo.extras);
        }
        if let Some(malla_id) = nodo.get("mesh").and_then(|m| m.as_u64())
            && let Some(extras) = doc
                .get("meshes")
                .and_then(|m| m.get(malla_id as usize))
                .and_then(|m| m.get("extras"))
        {
            leer_extras(extras, "malla", &mut modelo.extras);
        }

        if let Some(malla) = nodo.get("mesh").and_then(|m| m.as_u64()) {
            leer_malla(&doc, bin, malla as usize, &mundo, &materiales, &mut modelo);
        }
        if let Some(hijos) = nodo.get("children").and_then(|c| c.as_array()) {
            for h in hijos.iter().filter_map(|v| v.as_u64()) {
                pendientes.push((h as usize, mundo));
            }
        }
    }

    if modelo.primitivas.is_empty() {
        modelo
            .avisos
            .push("E_GLB_SIN_MALLAS: el archivo no contiene triángulos".to_string());
    }
    Ok(modelo)
}

fn leer_malla(
    doc: &Value,
    bin: &[u8],
    indice: usize,
    mundo: &[f64; 16],
    materiales: &[String],
    modelo: &mut Modelo,
) {
    let Some(malla) = doc.get("meshes").and_then(|m| m.get(indice)) else {
        return;
    };
    let nombre_malla = malla
        .get("name")
        .and_then(|n| n.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("malla_{indice}"));
    let Some(prims) = malla.get("primitives").and_then(|p| p.as_array()) else {
        return;
    };
    for (i, prim) in prims.iter().enumerate() {
        let mut p = Primitiva {
            nombre: format!("{nombre_malla}[{i}]"),
            bbox: Bbox3::vacia(),
            ..Primitiva::default()
        };
        let atributos = prim.get("attributes");
        let posiciones = atributos
            .and_then(|a| a.get("POSITION"))
            .and_then(|v| v.as_u64())
            .and_then(|idx| leer_vec3(doc, bin, idx as usize));
        let Some(posiciones) = posiciones else {
            modelo.avisos.push(format!(
                "E_GLB_POSICIONES: la primitiva {} no tiene POSITION legible",
                p.nombre
            ));
            continue;
        };
        p.vertices = posiciones.len();
        p.con_normales = atributos
            .map(|a| a.get("NORMAL").is_some())
            .unwrap_or(false);
        p.con_colores = atributos
            .map(|a| a.get("COLOR_0").is_some())
            .unwrap_or(false);
        p.material = prim.get("material").and_then(|m| m.as_u64()).map(|m| {
            materiales
                .get(m as usize)
                .cloned()
                .unwrap_or_else(|| format!("material_{m}"))
        });

        // Índices (si no hay, se toman los vértices en orden).
        let indices = prim
            .get("indices")
            .and_then(|v| v.as_u64())
            .and_then(|idx| leer_escalares(doc, bin, idx as usize));
        let tri = match &indices {
            Some(ix) => ix.len() / 3,
            None => posiciones.len() / 3,
        };
        p.triangulos = tri;

        for pos in &posiciones {
            let m = de_y_arriba(transformar(mundo, *pos));
            p.bbox.add(m);
            modelo.bbox.add(m);
        }
        modelo.triangulos += tri;
        modelo.primitivas.push(p);
    }
}

fn leer_vec3(doc: &Value, bin: &[u8], indice: usize) -> Option<Vec<[f64; 3]>> {
    let (datos, cuenta) = datos_del_accessor(doc, bin, indice, 3)?;
    let mut out = Vec::with_capacity(cuenta);
    for i in 0..cuenta {
        let base = i * 12;
        if base + 12 > datos.len() {
            break;
        }
        let x = f32::from_le_bytes([
            datos[base],
            datos[base + 1],
            datos[base + 2],
            datos[base + 3],
        ]);
        let y = f32::from_le_bytes([
            datos[base + 4],
            datos[base + 5],
            datos[base + 6],
            datos[base + 7],
        ]);
        let z = f32::from_le_bytes([
            datos[base + 8],
            datos[base + 9],
            datos[base + 10],
            datos[base + 11],
        ]);
        if !x.is_finite() || !y.is_finite() || !z.is_finite() {
            return None;
        }
        out.push([x as f64, y as f64, z as f64]);
    }
    Some(out)
}

fn leer_escalares(doc: &Value, bin: &[u8], indice: usize) -> Option<Vec<u32>> {
    let acc = doc.get("accessors")?.get(indice)?;
    let tipo = acc.get("componentType")?.as_u64()?;
    let cuenta = acc.get("count")?.as_u64()? as usize;
    let datos = datos_crudos(doc, bin, indice)?;
    let mut out = Vec::with_capacity(cuenta);
    match tipo {
        5123 => {
            for i in 0..cuenta {
                let b = i * 2;
                if b + 2 > datos.len() {
                    break;
                }
                out.push(u16::from_le_bytes([datos[b], datos[b + 1]]) as u32);
            }
        }
        5125 => {
            for i in 0..cuenta {
                let b = i * 4;
                if b + 4 > datos.len() {
                    break;
                }
                out.push(u32::from_le_bytes([
                    datos[b],
                    datos[b + 1],
                    datos[b + 2],
                    datos[b + 3],
                ]));
            }
        }
        5121 => out.extend(datos.iter().take(cuenta).map(|v| *v as u32)),
        _ => return None,
    }
    Some(out)
}

/// Datos crudos del accessor y su número de elementos.
fn datos_del_accessor<'a>(
    doc: &Value,
    bin: &'a [u8],
    indice: usize,
    componentes: usize,
) -> Option<(&'a [u8], usize)> {
    let acc = doc.get("accessors")?.get(indice)?;
    if acc.get("componentType")?.as_u64()? != 5126 {
        return None;
    }
    let tipo = acc.get("type")?.as_str()?;
    let esperado = match tipo {
        "VEC3" => 3,
        "SCALAR" => 1,
        _ => return None,
    };
    if esperado != componentes {
        return None;
    }
    let cuenta = acc.get("count")?.as_u64()? as usize;
    let datos = datos_crudos(doc, bin, indice)?;
    Some((datos, cuenta))
}

fn datos_crudos<'a>(doc: &Value, bin: &'a [u8], indice: usize) -> Option<&'a [u8]> {
    let acc = doc.get("accessors")?.get(indice)?;
    let vista = doc
        .get("bufferViews")?
        .get(acc.get("bufferView")?.as_u64()? as usize)?;
    let offset_vista = vista
        .get("byteOffset")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as usize;
    let offset_acc = acc.get("byteOffset").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    let largo = vista.get("byteLength")?.as_u64()? as usize;
    let inicio = offset_vista + offset_acc;
    let fin = (offset_vista + largo).min(bin.len());
    if inicio >= fin {
        return None;
    }
    Some(&bin[inicio..fin])
}

/// Convierte de Y-arriba (glTF) a Z-arriba (la aplicación).
fn de_y_arriba(p: [f64; 3]) -> [f64; 3] {
    [p[0], -p[2], p[1]]
}

fn identidad() -> [f64; 16] {
    let mut m = [0.0; 16];
    m[0] = 1.0;
    m[5] = 1.0;
    m[10] = 1.0;
    m[15] = 1.0;
    m
}

fn matriz_del_nodo(nodo: &Value) -> [f64; 16] {
    if let Some(m) = nodo.get("matrix").and_then(|m| m.as_array())
        && m.len() == 16
    {
        let mut out = [0.0; 16];
        for i in 0..16 {
            out[i] = m[i].as_f64().unwrap_or(0.0);
        }
        return out;
    }
    let t = nodo
        .get("translation")
        .and_then(|v| v.as_array())
        .map(|a| {
            [
                a[0].as_f64().unwrap_or(0.0),
                a[1].as_f64().unwrap_or(0.0),
                a[2].as_f64().unwrap_or(0.0),
            ]
        })
        .unwrap_or([0.0; 3]);
    let s = nodo
        .get("scale")
        .and_then(|v| v.as_array())
        .map(|a| {
            [
                a[0].as_f64().unwrap_or(1.0),
                a[1].as_f64().unwrap_or(1.0),
                a[2].as_f64().unwrap_or(1.0),
            ]
        })
        .unwrap_or([1.0; 3]);
    let q = nodo
        .get("rotation")
        .and_then(|v| v.as_array())
        .map(|a| {
            [
                a[0].as_f64().unwrap_or(0.0),
                a[1].as_f64().unwrap_or(0.0),
                a[2].as_f64().unwrap_or(0.0),
                a[3].as_f64().unwrap_or(1.0),
            ]
        })
        .unwrap_or([0.0, 0.0, 0.0, 1.0]);
    let (x, y, z, w) = (q[0], q[1], q[2], q[3]);
    let r = [
        1.0 - 2.0 * (y * y + z * z),
        2.0 * (x * y - z * w),
        2.0 * (x * z + y * w),
        2.0 * (x * y + z * w),
        1.0 - 2.0 * (x * x + z * z),
        2.0 * (y * z - x * w),
        2.0 * (x * z - y * w),
        2.0 * (y * z + x * w),
        1.0 - 2.0 * (x * x + y * y),
    ];
    // Columna mayor, como glTF: la columna c es (R[0][c], R[1][c], R[2][c]).
    // `r` está en filas, así que hay que transponer al empaquetar (si no, se aplica la
    // rotación inversa y el modelo sale del revés).
    [
        r[0] * s[0],
        r[3] * s[0],
        r[6] * s[0],
        0.0,
        r[1] * s[1],
        r[4] * s[1],
        r[7] * s[1],
        0.0,
        r[2] * s[2],
        r[5] * s[2],
        r[8] * s[2],
        0.0,
        t[0],
        t[1],
        t[2],
        1.0,
    ]
}

fn multiplicar(a: [f64; 16], b: [f64; 16]) -> [f64; 16] {
    let mut out = [0.0; 16];
    for col in 0..4 {
        for row in 0..4 {
            let mut suma = 0.0;
            for k in 0..4 {
                suma += a[k * 4 + row] * b[col * 4 + k];
            }
            out[col * 4 + row] = suma;
        }
    }
    out
}

fn transformar(m: &[f64; 16], p: [f64; 3]) -> [f64; 3] {
    [
        m[0] * p[0] + m[4] * p[1] + m[8] * p[2] + m[12],
        m[1] * p[0] + m[5] * p[1] + m[9] * p[2] + m[13],
        m[2] * p[0] + m[6] * p[1] + m[10] * p[2] + m[14],
    ]
}

/// Revisa un modelo contra lo que espera la familia (escala, presupuesto, origen).
pub fn revisar(modelo: &Modelo, esperado: Expectativa) -> Vec<Aviso> {
    let mut out = Vec::new();
    let d = modelo.bbox.dimensiones();
    let mayor = d[0].max(d[1]).max(d[2]);
    let menor = d[0].min(d[1]).min(d[2]);

    if !mayor.is_finite() || mayor <= 0.0 {
        out.push(Aviso {
            gravedad: Gravedad::Error,
            texto: "el modelo no tiene volumen".into(),
        });
        return out;
    }
    if mayor > esperado.max_m {
        out.push(Aviso {
            gravedad: Gravedad::Error,
            texto: format!(
                "mide {mayor:.1} m; para esta familia el máximo es {:.1} m (¿está en milímetros o en otra escala?)",
                esperado.max_m
            ),
        });
    }
    if menor < esperado.min_m {
        out.push(Aviso {
            gravedad: Gravedad::Aviso,
            texto: format!("la dimensión menor es {menor:.3} m; parece demasiado pequeño"),
        });
    }
    if modelo.triangulos > esperado.max_triangulos {
        out.push(Aviso {
            gravedad: Gravedad::Aviso,
            texto: format!(
                "{} triángulos; el visor va cómodo hasta {} (habría que simplificar)",
                modelo.triangulos, esperado.max_triangulos
            ),
        });
    }
    // Origen: la base debe apoyar en z = 0 y el centro en planta estar cerca de (0,0).
    let c = modelo.bbox.centro();
    if modelo.bbox.min[2].abs() > 0.05 {
        out.push(Aviso {
            gravedad: Gravedad::Aviso,
            texto: format!(
                "la base está a {:.3} m de z=0: conviene poner el origen en la base",
                modelo.bbox.min[2]
            ),
        });
    }
    if c[0].abs() > 0.5 || c[1].abs() > 0.5 {
        out.push(Aviso {
            gravedad: Gravedad::Info,
            texto: format!(
                "el centro en planta está en ({:.2}, {:.2}); se puede centrar al colocar",
                c[0], c[1]
            ),
        });
    }
    if let Some(q) = modelo.giro {
        // Giro de −90° en X: X≈-0.707, W≈0.707 (Y-arriba de glTF → Z-arriba de Blender).
        if (q[0] + std::f64::consts::FRAC_1_SQRT_2).abs() < 0.02 && q[3].abs() > 0.7 {
            out.push(Aviso {
                gravedad: Gravedad::Info,
                texto: "el nodo raíz gira −90° en X: el modelo viene en Y-arriba (normal en glTF)"
                    .into(),
            });
        }
    }
    if modelo.materiales.is_empty() {
        out.push(Aviso {
            gravedad: Gravedad::Info,
            texto: "sin materiales: se usará el color de la familia".into(),
        });
    }
    out
}

/// Triángulo leído de un `.glb`, ya en coordenadas de la aplicación (Z-arriba).
#[derive(Clone, Copy, Debug)]
pub struct TrianguloLeido {
    pub pos: [[f64; 3]; 3],
    pub color: Option<[f32; 3]>,
}

/// Resultado de normalizar un modelo.
#[derive(Clone, Debug, Default)]
pub struct Normalizacion {
    pub triangulos: usize,
    /// Escala aplicada (1.0 si no se pidió).
    pub escala: f64,
    /// Centro en planta que tenía el modelo antes de centrarlo.
    pub centro_previo: [f64; 3],
    /// Cota de la base antes de apoyarla en z = 0.
    pub base_previa: f64,
    /// Caja resultante tras normalizar.
    pub bbox: Bbox3,
    pub materiales: Vec<String>,
}

/// Extrae los triángulos de un `.glb` aplicando la transformación de los nodos.
pub fn triangulos_de_glb(bytes: &[u8]) -> Result<Vec<TrianguloLeido>, String> {
    let (doc, bin) = abrir_glb(bytes)?;
    let materiales = doc
        .get("materials")
        .and_then(|m| m.as_array())
        .map(|a| a.len())
        .unwrap_or(0);
    let _ = materiales;

    let mut out = Vec::new();
    let raices: Vec<usize> = match doc
        .get("scenes")
        .and_then(|s| s.as_array())
        .and_then(|s| s.first())
        .and_then(|s| s.get("nodes"))
    {
        Some(Value::Array(a)) => a
            .iter()
            .filter_map(|v| v.as_u64())
            .map(|v| v as usize)
            .collect(),
        _ => (0..doc
            .get("nodes")
            .and_then(|n| n.as_array())
            .map(|a| a.len())
            .unwrap_or(0))
            .collect(),
    };
    let mut pendientes: Vec<(usize, [f64; 16])> =
        raices.into_iter().map(|i| (i, identidad())).collect();
    let mut visitados = 0;
    while let Some((indice, padre)) = pendientes.pop() {
        visitados += 1;
        if visitados > 10_000 {
            return Err("E_GLB_CICLO: jerarquía de nodos demasiado profunda o circular".into());
        }
        let Some(nodo) = doc.get("nodes").and_then(|n| n.get(indice)) else {
            continue;
        };
        let mundo = multiplicar(padre, matriz_del_nodo(nodo));
        if let Some(malla) = nodo.get("mesh").and_then(|m| m.as_u64())
            && let Some(prims) = doc
                .get("meshes")
                .and_then(|m| m.get(malla as usize))
                .and_then(|m| m.get("primitives"))
                .and_then(|p| p.as_array())
        {
            for prim in prims {
                let atributos = prim.get("attributes");
                let Some(posiciones) = atributos
                    .and_then(|a| a.get("POSITION"))
                    .and_then(|v| v.as_u64())
                    .and_then(|i| leer_vec3(&doc, bin, i as usize))
                else {
                    continue;
                };
                let colores = atributos
                    .and_then(|a| a.get("COLOR_0"))
                    .and_then(|v| v.as_u64())
                    .and_then(|i| leer_colores(&doc, bin, i as usize));
                let indices = prim
                    .get("indices")
                    .and_then(|v| v.as_u64())
                    .and_then(|i| leer_escalares(&doc, bin, i as usize));

                let mut mundo_pos: Vec<[f64; 3]> = posiciones
                    .iter()
                    .map(|p| de_y_arriba(transformar(&mundo, *p)))
                    .collect();
                // Si no hay índices, el orden es el de los vértices.
                let lista: Vec<u32> = match indices {
                    Some(ix) => ix,
                    None => (0..mundo_pos.len() as u32).collect(),
                };
                for trozo in lista.chunks(3) {
                    if trozo.len() < 3 {
                        break;
                    }
                    let (a, b, c) = (trozo[0] as usize, trozo[1] as usize, trozo[2] as usize);
                    if a >= mundo_pos.len() || b >= mundo_pos.len() || c >= mundo_pos.len() {
                        continue;
                    }
                    let color = colores
                        .as_ref()
                        .and_then(|cs| cs.get(a).or_else(|| cs.first()).copied());
                    out.push(TrianguloLeido {
                        pos: [mundo_pos[a], mundo_pos[b], mundo_pos[c]],
                        color,
                    });
                }
                mundo_pos.clear();
            }
        }
        if let Some(hijos) = nodo.get("children").and_then(|c| c.as_array()) {
            for h in hijos.iter().filter_map(|v| v.as_u64()) {
                pendientes.push((h as usize, mundo));
            }
        }
    }
    if out.is_empty() {
        return Err("E_GLB_SIN_MALLAS: el archivo no contiene triángulos".into());
    }
    Ok(out)
}

fn leer_colores(doc: &Value, bin: &[u8], indice: usize) -> Option<Vec<[f32; 3]>> {
    let acc = doc.get("accessors")?.get(indice)?;
    let componentes = match acc.get("type")?.as_str()? {
        "VEC3" => 3,
        "VEC4" => 4,
        _ => return None,
    };
    let tipo = acc.get("componentType")?.as_u64()?;
    let cuenta = acc.get("count")?.as_u64()? as usize;
    let datos = datos_crudos(doc, bin, indice)?;
    let mut out = Vec::with_capacity(cuenta);
    match tipo {
        5126 => {
            for i in 0..cuenta {
                let base = i * componentes * 4;
                if base + componentes * 4 > datos.len() {
                    break;
                }
                let leer = |k: usize| {
                    let o = base + k * 4;
                    f32::from_le_bytes([datos[o], datos[o + 1], datos[o + 2], datos[o + 3]])
                };
                out.push([leer(0), leer(1), leer(2)]);
            }
        }
        5121 => {
            for i in 0..cuenta {
                let base = i * componentes;
                if base + componentes > datos.len() {
                    break;
                }
                out.push([
                    datos[base] as f32 / 255.0,
                    datos[base + 1] as f32 / 255.0,
                    datos[base + 2] as f32 / 255.0,
                ]);
            }
        }
        _ => return None,
    }
    Some(out)
}

/// Normaliza un `.glb` para el visor: centra en planta, apoya la base en `z = 0`, aplica la
/// escala pedida y devuelve el binario reescrito junto con lo que se cambió.
///
/// El color por defecto se usa cuando el modelo no trae `COLOR_0`.
pub fn normalizar_glb(
    bytes: &[u8],
    escala: f64,
    color_defecto: [f32; 3],
) -> Result<(Vec<u8>, Normalizacion), String> {
    let tris = triangulos_de_glb(bytes)?;
    let mut bbox = Bbox3::vacia();
    for t in &tris {
        for p in t.pos {
            bbox.add(p);
        }
    }
    let centro = bbox.centro();
    let base = bbox.min[2];
    let k = if escala.is_finite() && escala > 0.0 {
        escala
    } else {
        1.0
    };

    let mut por_familia: std::collections::BTreeMap<String, Vec<crate::gltf::Vertice>> =
        std::collections::BTreeMap::new();
    let mut nuevo_bbox = Bbox3::vacia();
    for t in &tris {
        let mut p = [[0f32; 3]; 3];
        for (i, q) in t.pos.iter().enumerate() {
            let x = (q[0] - centro[0]) * k;
            let y = (q[1] - centro[1]) * k;
            let z = (q[2] - base) * k;
            p[i] = [x as f32, y as f32, z as f32];
            nuevo_bbox.add([x, y, z]);
        }
        let n = normal_de_f32(p);
        let color = t.color.unwrap_or(color_defecto);
        let destino = por_familia.entry("modelo".to_string()).or_default();
        for pos in p {
            destino.push(crate::gltf::Vertice {
                pos,
                normal: n,
                color,
            });
        }
    }
    let glb = crate::gltf::glb_desde_vertices(&por_familia);
    Ok((
        glb,
        Normalizacion {
            triangulos: tris.len(),
            escala: k,
            centro_previo: centro,
            base_previa: base,
            bbox: nuevo_bbox,
            materiales: vec!["modelo".to_string()],
        },
    ))
}

fn normal_de_f32(tri: [[f32; 3]; 3]) -> [f32; 3] {
    let u = [
        tri[1][0] - tri[0][0],
        tri[1][1] - tri[0][1],
        tri[1][2] - tri[0][2],
    ];
    let v = [
        tri[2][0] - tri[0][0],
        tri[2][1] - tri[0][1],
        tri[2][2] - tri[0][2],
    ];
    let n = [
        u[1] * v[2] - u[2] * v[1],
        u[2] * v[0] - u[0] * v[2],
        u[0] * v[1] - u[1] * v[0],
    ];
    let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-9);
    [n[0] / l, n[1] / l, n[2] / l]
}

/// Abre un `.glb` y devuelve su JSON y su bloque binario.
fn abrir_glb(bytes: &[u8]) -> Result<(Value, &[u8]), String> {
    if bytes.len() < 20 {
        return Err("E_GLB_CORTO: el archivo no tiene cabecera glTF".into());
    }
    if u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) != 0x4654_6C67 {
        return Err("E_GLB_MAGIA: no es un .glb (¿un .gltf de texto?)".into());
    }
    let largo_json = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]) as usize;
    if 20 + largo_json > bytes.len() {
        return Err("E_GLB_JSON: el bloque JSON se sale del archivo".into());
    }
    let json = std::str::from_utf8(&bytes[20..20 + largo_json])
        .map_err(|e| format!("E_GLB_JSON: {e}"))?
        .trim_end_matches([' ', '\0']);
    let doc: Value = serde_json::from_str(json).map_err(|e| format!("E_GLB_JSON: {e}"))?;
    let mut bin: &[u8] = &[];
    let offset = 20 + largo_json;
    if offset + 8 <= bytes.len() {
        let largo = u32::from_le_bytes([
            bytes[offset],
            bytes[offset + 1],
            bytes[offset + 2],
            bytes[offset + 3],
        ]) as usize;
        let tipo = u32::from_le_bytes([
            bytes[offset + 4],
            bytes[offset + 5],
            bytes[offset + 6],
            bytes[offset + 7],
        ]);
        if tipo == 0x004E_4942 && offset + 8 + largo <= bytes.len() {
            bin = &bytes[offset + 8..offset + 8 + largo];
        }
    }
    Ok((doc, bin))
}

/// Una pieza de la biblioteca: un modelo propio asociado a una familia y subtipo.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Entrada {
    pub id: String,
    pub familia: String,
    #[serde(default)]
    pub subtipo: Option<String>,
    #[serde(default)]
    pub dn_mm: Option<u32>,
    /// Ruta relativa al manifiesto.
    pub archivo: String,
    pub escala: f64,
    pub triangulos: usize,
    pub dimensiones_m: [f64; 3],
    pub bytes: usize,
    /// Volumen medido de la malla (m³) al importar.
    #[serde(default)]
    pub volumen_m3: Option<f64>,
    /// Área de superficie medida (m²).
    #[serde(default)]
    pub area_m2: Option<f64>,
    /// La malla cierra (sin aristas de un solo triángulo).
    #[serde(default)]
    pub cerrada: Option<bool>,
    /// Datos maestros declarados en Blender (`extras` de glTF).
    #[serde(default)]
    pub extras: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    pub licencia: Option<String>,
    #[serde(default)]
    pub autor: Option<String>,
}

/// Manifiesto de la biblioteca del proyecto (`biblioteca.json`).
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Biblioteca {
    pub schema: String,
    #[serde(default)]
    pub piezas: Vec<Entrada>,
}

impl Default for Biblioteca {
    fn default() -> Self {
        Biblioteca {
            schema: "redes.biblioteca/1".to_string(),
            piezas: Vec::new(),
        }
    }
}

impl Biblioteca {
    /// Carga el manifiesto; si no existe, devuelve uno vacío.
    pub fn cargar(ruta: &std::path::Path) -> Result<Biblioteca, String> {
        if !ruta.exists() {
            return Ok(Biblioteca::default());
        }
        let texto = std::fs::read_to_string(ruta).map_err(|e| format!("E_IO_PERMISSION: {e}"))?;
        let b: Biblioteca =
            serde_json::from_str(&texto).map_err(|e| format!("E_FMT_PARSE: {e}"))?;
        if !b.schema.starts_with("redes.biblioteca/") {
            return Err(format!("E_BIB_SCHEMA: esquema desconocido {}", b.schema));
        }
        Ok(b)
    }

    pub fn guardar(&self, ruta: &std::path::Path) -> Result<(), String> {
        let texto = serde_json::to_string_pretty(self).map_err(|e| format!("E_FMT_PARSE: {e}"))?;
        std::fs::write(ruta, texto).map_err(|e| format!("E_IO_PERMISSION: {e}"))
    }

    /// Inserta o reemplaza por `id`.
    pub fn insertar(&mut self, entrada: Entrada) {
        match self.piezas.iter_mut().find(|e| e.id == entrada.id) {
            Some(slot) => *slot = entrada,
            None => self.piezas.push(entrada),
        }
        self.piezas.sort_by(|a, b| a.id.cmp(&b.id));
    }

    /// La entrada que corresponde a una familia y subtipo (y DN, si se indica).
    pub fn buscar(&self, familia: &str, subtipo: &str, dn_mm: Option<u32>) -> Option<&Entrada> {
        self.piezas.iter().find(|e| {
            e.familia == familia
                && e.subtipo.as_deref().map(|s| s == subtipo).unwrap_or(true)
                && match (dn_mm, e.dn_mm) {
                    (Some(a), Some(b)) => a == b,
                    (Some(_), None) => true,
                    _ => true,
                }
        })
    }
}

/// Color de material por defecto según la familia (cuando el modelo no trae `COLOR_0`).
pub fn color_sugerido(familia: &str) -> [f32; 3] {
    match familia {
        f if f.starts_with("agua.") => [0.36, 0.52, 0.68],
        "saneamiento.pozo" | "saneamiento.camara" | "saneamiento.caja" => [0.62, 0.60, 0.56],
        "saneamiento.tuberia" | "saneamiento.canal" | "saneamiento.sifon" => [0.70, 0.62, 0.48],
        "comun.accesorio" | "comun.tapa" | "comun.escalera" => [0.48, 0.49, 0.52],
        _ => [0.60, 0.58, 0.55],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ida_y_vuelta_del_exportador() {
        let p = crate::demo_catalog::catalogo();
        let glb = crate::gltf::exportar_glb(&p, crate::gltf::OpcionesGltf { con_terreno: false });
        let m = leer_glb(&glb).expect("se lee lo que exportamos");
        assert!(m.triangulos > 1000, "triángulos: {}", m.triangulos);
        assert!(!m.primitivas.is_empty());
        let d = m.bbox.dimensiones();
        assert!(d[0] > 1.0 && d[0] < 500.0, "ancho fuera de rango: {d:?}");
        // El catálogo entero ocupa ~110 m: se valida con la expectativa de conjunto, no de pieza.
        let avisos = revisar(&m, Expectativa::planta());
        let errores: Vec<_> = avisos
            .iter()
            .filter(|a| a.gravedad == Gravedad::Error)
            .collect();
        assert!(errores.is_empty(), "errores inesperados: {errores:?}");
    }

    #[test]
    fn normalizar_centra_y_apoya_la_base() {
        let p = crate::demo_catalog::catalogo();
        let glb = crate::gltf::exportar_glb(&p, crate::gltf::OpcionesGltf { con_terreno: false });
        let antes = leer_glb(&glb).expect("legible");
        let (norm, info) = normalizar_glb(&glb, 1.0, [0.6, 0.6, 0.6]).expect("normaliza");
        let despues = leer_glb(&norm).expect("el normalizado se vuelve a leer");
        assert_eq!(
            info.triangulos, antes.triangulos,
            "no se pierden triángulos"
        );
        let c = despues.bbox.centro();
        assert!(
            c[0].abs() < 0.01 && c[1].abs() < 0.01,
            "centro en planta: {c:?}"
        );
        assert!(
            despues.bbox.min[2].abs() < 0.01,
            "base en z=0: {:?}",
            despues.bbox.min
        );
        // La normalización informa la base del modelo original (no un valor cualquiera): debe
        // coincidir con el mínimo en z del glb de entrada.
        assert!(
            (info.base_previa - antes.bbox.min[2]).abs() < 1e-4,
            "base_previa {:?} vs min z original {:?}",
            info.base_previa,
            antes.bbox.min[2]
        );
        // Y es idempotente: normalizar lo ya normalizado no mueve nada.
        let (otra, info2) =
            normalizar_glb(&norm, 1.0, [0.6, 0.6, 0.6]).expect("normaliza otra vez");
        let b2 = leer_glb(&otra).expect("legible").bbox;
        assert!(info2.base_previa.abs() < 1e-4, "base ya en cero");
        for i in 0..3 {
            assert!(
                (b2.min[i] - despues.bbox.min[i]).abs() < 1e-4
                    && (b2.max[i] - despues.bbox.max[i]).abs() < 1e-4,
                "eje {i}: {:?} vs {:?}",
                b2,
                despues.bbox
            );
        }
    }

    #[test]
    fn normalizar_aplica_la_escala() {
        let p = crate::demo_catalog::catalogo();
        let glb = crate::gltf::exportar_glb(&p, crate::gltf::OpcionesGltf { con_terreno: false });
        let (_, uno) = normalizar_glb(&glb, 1.0, [0.6, 0.6, 0.6]).expect("normaliza");
        let (_, medio) = normalizar_glb(&glb, 0.5, [0.6, 0.6, 0.6]).expect("normaliza");
        let d1 = uno.bbox.dimensiones()[0];
        let d2 = medio.bbox.dimensiones()[0];
        assert!((d2 - d1 * 0.5).abs() < 0.01, "{d2} vs {d1} * 0.5");
    }

    #[test]
    fn el_manifiesto_busca_por_familia_y_subtipo() {
        let mut b = Biblioteca::default();
        b.insertar(Entrada {
            id: "v1".into(),
            familia: "agua.valvula".into(),
            subtipo: Some("compuerta".into()),
            dn_mm: Some(200),
            archivo: "biblioteca/v1.glb".into(),
            escala: 1.0,
            triangulos: 100,
            dimensiones_m: [0.3, 0.6, 0.9],
            bytes: 1000,
            volumen_m3: Some(0.05),
            area_m2: Some(1.2),
            cerrada: Some(true),
            extras: Default::default(),
            licencia: None,
            autor: None,
        });
        assert!(b.buscar("agua.valvula", "compuerta", Some(200)).is_some());
        assert!(b.buscar("agua.valvula", "mariposa", Some(200)).is_none());
    }

    #[test]
    fn el_volumen_de_un_cubo_es_exacto() {
        // Cubo de 2 m de lado construido a mano (12 triángulos).
        let mut m = std::collections::BTreeMap::new();
        let mut verts = Vec::new();
        let caras_tri =
            |a: [f32; 3], b: [f32; 3], c: [f32; 3], d: [f32; 3]| vec![[a, b, c], [a, c, d]];
        let v = |x: f32, y: f32, z: f32| [x, y, z];
        let mut tris = Vec::new();
        // Cada cara del cubo [-1,1]^3.
        tris.extend(caras_tri(
            v(-1.0, -1.0, 1.0),
            v(1.0, -1.0, 1.0),
            v(1.0, 1.0, 1.0),
            v(-1.0, 1.0, 1.0),
        ));
        tris.extend(caras_tri(
            v(-1.0, -1.0, -1.0),
            v(-1.0, 1.0, -1.0),
            v(1.0, 1.0, -1.0),
            v(1.0, -1.0, -1.0),
        ));
        tris.extend(caras_tri(
            v(-1.0, -1.0, -1.0),
            v(1.0, -1.0, -1.0),
            v(1.0, -1.0, 1.0),
            v(-1.0, -1.0, 1.0),
        ));
        tris.extend(caras_tri(
            v(1.0, 1.0, -1.0),
            v(-1.0, 1.0, -1.0),
            v(-1.0, 1.0, 1.0),
            v(1.0, 1.0, 1.0),
        ));
        tris.extend(caras_tri(
            v(1.0, -1.0, -1.0),
            v(1.0, 1.0, -1.0),
            v(1.0, 1.0, 1.0),
            v(1.0, -1.0, 1.0),
        ));
        tris.extend(caras_tri(
            v(-1.0, 1.0, -1.0),
            v(-1.0, -1.0, -1.0),
            v(-1.0, -1.0, 1.0),
            v(-1.0, 1.0, 1.0),
        ));
        for t in tris {
            let n = normal_de_f32(t);
            for p in t {
                verts.push(crate::gltf::Vertice {
                    pos: p,
                    normal: n,
                    color: [0.6, 0.6, 0.6],
                });
            }
        }
        m.insert("hormigon".to_string(), verts);
        let glb = crate::gltf::glb_desde_vertices(&m);
        let c = cantidades_glb(&glb).expect("cantidades");
        assert!(
            (c.volumen_m3.abs() - 8.0).abs() < 0.01,
            "volumen: {}",
            c.volumen_m3
        );
        assert!((c.area_m2 - 24.0).abs() < 0.05, "área: {}", c.area_m2);
        assert!(c.cerrada(), "aristas abiertas: {}", c.aristas_abiertas);
        assert!((c.peso_t(2.4) - 19.2).abs() < 0.05);
    }

    #[test]
    fn un_archivo_que_no_es_glb_se_rechaza() {
        let err = leer_glb(b"no soy un glb").unwrap_err();
        assert!(err.starts_with("E_GLB_CORTO"), "{err}");
        let err = leer_glb(&[0u8; 32]).unwrap_err();
        assert!(err.starts_with("E_GLB_MAGIA"), "{err}");
    }

    #[test]
    fn detecta_un_modelo_gigante() {
        let m = Modelo {
            bbox: Bbox3 {
                min: [0.0, 0.0, 0.0],
                max: [500.0, 20.0, 30.0],
            },
            triangulos: 100,
            ..Modelo::default()
        };
        let avisos = revisar(&m, Expectativa::pieza());
        assert!(avisos.iter().any(|a| a.gravedad == Gravedad::Error));
    }
}
