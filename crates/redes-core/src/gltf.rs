//! Exportador glTF 2.0 (`.glb`) para abrir el modelo en Blender, three.js o cualquier visor
//! estándar.
//!
//! Una malla por familia, con posiciones, normales y color por vértice; material `unlit` para que
//! el visor externo muestre el mismo color que la app (que ya sombrea por triángulo). El nodo raíz
//! lleva la rotación de Z-arriba (modelo) a Y-arriba (glTF).

use std::collections::BTreeMap;

use crate::geom::{P2, P3};
use crate::model::Project;
use crate::parts;

/// Opciones de exportación.
#[derive(Clone, Copy, Debug)]
pub struct OpcionesGltf {
    /// Incluir el terreno como malla (por defecto sí).
    pub con_terreno: bool,
}

impl Default for OpcionesGltf {
    fn default() -> Self {
        OpcionesGltf { con_terreno: true }
    }
}

pub struct Vertice {
    pub pos: [f32; 3],
    pub normal: [f32; 3],
    pub color: [f32; 3],
}

/// Triángulo en coordenadas locales del proyecto (origen restado), listo para un motor 3D.
#[derive(Clone, Copy, Debug)]
pub struct Triangulo {
    pub pos: [[f32; 3]; 3],
    pub normal: [f32; 3],
    pub color: [f32; 3],
    /// Índice en `Malla::familias`.
    pub familia: u16,
}

/// Malla completa con la lista de familias que la componen.
#[derive(Clone, Debug, Default)]
pub struct Malla {
    pub familias: Vec<String>,
    pub tris: Vec<Triangulo>,
}

/// Extrae la geometría del proyecto en triángulos con normal y color.
///
/// Es la fuente única del `.glb` y del pase 3D con profundidad: así el visor externo y el interno
/// dibujan exactamente la misma malla.
pub fn triangulos_exportables(p: &Project, con_terreno: bool) -> Malla {
    let mut out = Malla::default();
    let mut indice: BTreeMap<String, u16> = BTreeMap::new();
    let mut familia = |nombre: &str| -> u16 {
        if let Some(i) = indice.get(nombre) {
            return *i;
        }
        let i = out.familias.len() as u16;
        out.familias.push(nombre.to_string());
        indice.insert(nombre.to_string(), i);
        i
    };
    let a_local = |q: P3| -> [f32; 3] {
        [
            (q.x - p.origin.x) as f32,
            (q.y - p.origin.y) as f32,
            q.z as f32,
        ]
    };

    for n in p.nodes.iter().filter(|n| !n.deleted) {
        let malla = crate::view::pieza_para_exportar(n);
        for tri in &malla.tris {
            let pos = tri.p.map(|v| {
                a_local(P3::new(
                    v[0] as f64 + n.pos.x,
                    v[1] as f64 + n.pos.y,
                    v[2] as f64 + n.z_invert,
                ))
            });
            out.tris.push(Triangulo {
                pos,
                normal: normal_de(pos),
                color: [tri.color[0], tri.color[1], tri.color[2]],
                familia: familia(&n.family),
            });
        }
    }

    for l in p.links.iter().filter(|l| !l.deleted) {
        if l.path.len() < 2 {
            continue;
        }
        let es_agua = l.family.starts_with("agua");
        let mat = if es_agua {
            parts::Material::new(0.34, 0.55, 0.82)
        } else {
            parts::Material::new(0.78, 0.62, 0.44)
        };
        for w in l.path.windows(2) {
            // En local: construir tubos con coordendas UTM en f32 los colapsa.
            let wa = P3::new(w[0].x - p.origin.x, w[0].y - p.origin.y, w[0].z);
            let wb = P3::new(w[1].x - p.origin.x, w[1].y - p.origin.y, w[1].z);
            let malla = parts::tuberia(wa, wb, l.d_m() as f32, 1, mat);
            for tri in &malla.tris {
                // `tuberia` ya devuelve coordenadas locales (x,y) y z absoluta: no hay que
                // volver a restar el origen.
                let pos = tri.p.map(|v| [v[0], v[1], v[2]]);
                out.tris.push(Triangulo {
                    pos,
                    normal: normal_de(pos),
                    color: [tri.color[0], tri.color[1], tri.color[2]],
                    familia: familia(&l.family),
                });
            }
        }
    }

    // Contexto urbano: calles y manzanas, con la cota del terreno.
    if let Some(ent) = &p.entorno {
        let cota = |q: P2| -> Option<f64> { p.terrain.as_ref().and_then(|t| t.z_at(q)) };
        let asfalto = [0.18f32, 0.19, 0.21];
        for calle in &ent.calles {
            let half = calle.ancho as f32 * 0.5;
            for w in calle.eje.windows(2) {
                let (a, b) = (w[0].p2(), w[1].p2());
                let d = ((b.x - a.x).powi(2) + (b.y - a.y).powi(2)).sqrt();
                if d < 0.05 {
                    continue;
                }
                let nx = -(b.y - a.y) / d * half as f64;
                let ny = (b.x - a.x) / d * half as f64;
                let esquinas = [
                    P2::new(a.x + nx, a.y + ny),
                    P2::new(b.x + nx, b.y + ny),
                    P2::new(b.x - nx, b.y - ny),
                    P2::new(a.x - nx, a.y - ny),
                ];
                let mut pt = [[0f32; 3]; 4];
                let mut ok = true;
                for (i, q) in esquinas.iter().enumerate() {
                    match cota(*q) {
                        Some(z) => pt[i] = a_local(P3::new(q.x, q.y, z + 0.2)),
                        None => ok = false,
                    }
                }
                if !ok {
                    continue;
                }
                for tri in [[pt[0], pt[1], pt[2]], [pt[0], pt[2], pt[3]]] {
                    out.tris.push(Triangulo {
                        pos: tri,
                        normal: normal_de(tri),
                        color: asfalto,
                        familia: familia(&calle.nombre),
                    });
                }
            }
        }
        for mz in &ent.manzanas {
            if mz.poligono.len() < 3 {
                continue;
            }
            let cotas: Vec<f64> = mz.poligono.iter().filter_map(|q| cota(*q)).collect();
            if cotas.len() < 3 {
                continue;
            }
            let base = cotas.iter().fold(f64::INFINITY, |a, b| a.min(*b)) - 1.0;
            let alta = cotas.iter().fold(f64::NEG_INFINITY, |a, b| a.max(*b));
            let top = alta + mz.altura.max(3.0);
            let pared = if mz.uso == "comercio" {
                [0.50f32, 0.55, 0.62]
            } else {
                [0.70f32, 0.64, 0.55]
            };
            let n = mz.poligono.len();
            for i in 0..n {
                let q1 = mz.poligono[i];
                let q2 = mz.poligono[(i + 1) % n];
                let (b1, b2) = (
                    a_local(P3::new(q1.x, q1.y, base)),
                    a_local(P3::new(q2.x, q2.y, base)),
                );
                let (t1, t2) = (
                    a_local(P3::new(q1.x, q1.y, top)),
                    a_local(P3::new(q2.x, q2.y, top)),
                );
                for tri in [[b1, b2, t2], [b1, t2, t1]] {
                    out.tris.push(Triangulo {
                        pos: tri,
                        normal: normal_de(tri),
                        color: pared,
                        familia: familia(&mz.uso),
                    });
                }
            }
            // Cubierta en abanico.
            let cx = mz.poligono.iter().map(|q| q.x).sum::<f64>() / n as f64;
            let cy = mz.poligono.iter().map(|q| q.y).sum::<f64>() / n as f64;
            let centro = a_local(P3::new(cx, cy, top));
            for i in 0..n {
                let q1 = mz.poligono[i];
                let q2 = mz.poligono[(i + 1) % n];
                let tri = [
                    centro,
                    a_local(P3::new(q1.x, q1.y, top)),
                    a_local(P3::new(q2.x, q2.y, top)),
                ];
                out.tris.push(Triangulo {
                    pos: tri,
                    normal: normal_de(tri),
                    color: [0.38, 0.36, 0.35],
                    familia: familia("cubierta"),
                });
            }
        }
    }

    if con_terreno
        && let Some(t) = &p.terrain {
            let (zmin, zmax) = t.z_range();
            let zspan = (zmax - zmin).max(1e-6);
            for tri in &t.tris {
                let pts = [
                    &t.points[tri[0] as usize],
                    &t.points[tri[1] as usize],
                    &t.points[tri[2] as usize],
                ];
                let pos = pts.map(|q| a_local(P3::new(q.e, q.n, q.z)));
                let zc = (pts[0].z + pts[1].z + pts[2].z) / 3.0;
                let t01 = ((zc - zmin) / zspan).clamp(0.0, 1.0) as f32;
                out.tris.push(Triangulo {
                    pos,
                    normal: normal_de(pos),
                    color: [0.26 + 0.34 * t01, 0.40 + 0.16 * t01, 0.26 + 0.06 * t01],
                    familia: familia("terreno"),
                });
            }
        }
    out
}

/// Exporta el proyecto a un binario glTF (`.glb`).
/// Número de primitivas (una por familia) que tendría el `.glb` del proyecto.
pub fn contar_primitivas(p: &Project, con_terreno: bool) -> usize {
    let malla = triangulos_exportables(p, con_terreno);
    let mut vistas: std::collections::BTreeSet<u16> = std::collections::BTreeSet::new();
    for t in &malla.tris {
        vistas.insert(t.familia);
    }
    vistas.len()
}

pub fn exportar_glb(p: &Project, opts: OpcionesGltf) -> Vec<u8> {
    let malla = triangulos_exportables(p, opts.con_terreno);
    // La z se lleva al origen (base en 0): en glTF la altura pasa a Y con el giro del nodo, y
    // dejar 2 500 m de cota haría que el modelo flote a 2,5 km en el visor externo.
    let z_min = malla
        .tris
        .iter()
        .flat_map(|t| t.pos.iter().map(|v| v[2]))
        .fold(f32::INFINITY, f32::min);
    let z0 = if z_min.is_finite() { z_min } else { 0.0 };
    let mut por_familia: BTreeMap<String, Vec<Vertice>> = BTreeMap::new();
    for tri in &malla.tris {
        let nombre = &malla.familias[tri.familia as usize];
        let destino = por_familia.entry(nombre.clone()).or_default();
        for k in 0..3 {
            let mut pos = tri.pos[k];
            pos[2] -= z0;
            destino.push(Vertice {
                pos,
                normal: tri.normal,
                color: tri.color,
            });
        }
    }
    construir_glb(&por_familia)
}

fn normal_de(tri: [[f32; 3]; 3]) -> [f32; 3] {
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

/// Construye un `.glb` a partir de vértices ya agrupados por material/familia.
///
/// Lo usa el importador de la biblioteca para reescribir un modelo ya normalizado (Y-arriba,
/// origen en la base, escala) sin volver a pasar por el proyecto.
pub fn glb_desde_vertices(por_familia: &BTreeMap<String, Vec<Vertice>>) -> Vec<u8> {
    construir_glb(por_familia)
}

fn construir_glb(por_familia: &BTreeMap<String, Vec<Vertice>>) -> Vec<u8> {
    let mut bin: Vec<u8> = Vec::new();
    let mut vistas: Vec<String> = Vec::new();
    let mut accesores: Vec<String> = Vec::new();
    let mut primitivas: Vec<String> = Vec::new();
    let mut materiales: Vec<String> = Vec::new();
    let mut color_medio: BTreeMap<&str, [f32; 3]> = BTreeMap::new();

    let mut indice_material = 0usize;
    for (familia, verts) in por_familia {
        if verts.is_empty() {
            continue;
        }
        let cuenta = verts.len();
        let mut pos = Vec::with_capacity(cuenta * 12);
        let mut nor = Vec::with_capacity(cuenta * 12);
        let mut col = Vec::with_capacity(cuenta * 12);
        let mut min = [f32::INFINITY; 3];
        let mut max = [f32::NEG_INFINITY; 3];
        let mut suma = [0.0f32; 3];
        for v in verts {
            for k in 0..3 {
                pos.extend_from_slice(&v.pos[k].to_le_bytes());
                nor.extend_from_slice(&v.normal[k].to_le_bytes());
                col.extend_from_slice(&v.color[k].to_le_bytes());
                min[k] = min[k].min(v.pos[k]);
                max[k] = max[k].max(v.pos[k]);
                suma[k] += v.color[k];
            }
        }
        let medio = [
            suma[0] / cuenta as f32,
            suma[1] / cuenta as f32,
            suma[2] / cuenta as f32,
        ];
        color_medio.insert(familia.as_str(), medio);

        let n = verts.len() as u64;
        for (datos, tipo, extra) in [
            (pos, "VEC3", Some((min, max))),
            (nor, "VEC3", None),
            (col, "VEC3", None),
        ] {
            let offset = bin.len();
            bin.extend_from_slice(&datos);
            while !bin.len().is_multiple_of(4) {
                bin.push(0);
            }
            vistas.push(format!(
                r#"{{"buffer":0,"byteOffset":{offset},"byteLength":{},"target":34962}}"#,
                datos.len()
            ));
            let indice_vista = vistas.len() - 1;
            let extra_json = match extra {
                Some((mn, mx)) => format!(
                    r#","min":[{},{},{}],"max":[{},{},{}]"#,
                    mn[0], mn[1], mn[2], mx[0], mx[1], mx[2]
                ),
                None => String::new(),
            };
            accesores.push(format!(
                r#"{{"bufferView":{indice_vista},"componentType":5126,"count":{n},"type":"{tipo}"{extra_json}}}"#
            ));
        }
        let base = accesores.len() - 3;
        primitivas.push(format!(
            r#"{{"attributes":{{"POSITION":{base},"NORMAL":{},"COLOR_0":{}}},"material":{indice_material},"mode":4}}"#,
            base + 1,
            base + 2
        ));
        materiales.push(format!(
            r#"{{"name":"{familia}","pbrMetallicRoughness":{{"baseColorFactor":[{},{},{},1.0],"metallicFactor":0.0,"roughnessFactor":0.9}},"doubleSided":true,"extensions":{{"KHR_materials_unlit":{{}}}}}}"#,
            medio[0], medio[1], medio[2]
        ));
        indice_material += 1;
    }

    let json = format!(
        r#"{{"asset":{{"version":"2.0","generator":"Redes"}},"extensionsUsed":["KHR_materials_unlit"],"scene":0,"scenes":[{{"nodes":[0]}}],"nodes":[{{"name":"Redes","mesh":0,"rotation":[-0.7071068,0.0,0.0,0.7071068]}}],"meshes":[{{"name":"redes","primitives":[{}]}}],"materials":[{}],"accessors":[{}],"bufferViews":[{}],"buffers":[{{"byteLength":{}}}]}}"#,
        primitivas.join(","),
        materiales.join(","),
        accesores.join(","),
        vistas.join(","),
        bin.len()
    );

    // Contenedor GLB: cabecera + chunk JSON + chunk BIN.
    let mut json_bytes = json.into_bytes();
    while json_bytes.len() % 4 != 0 {
        json_bytes.push(b' ');
    }
    while !bin.len().is_multiple_of(4) {
        bin.push(0);
    }
    let total = 12 + 8 + json_bytes.len() + 8 + bin.len();
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(&0x4654_6C67u32.to_le_bytes()); // "glTF"
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&(total as u32).to_le_bytes());
    out.extend_from_slice(&(json_bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(&0x4E4F_534Au32.to_le_bytes()); // "JSON"
    out.extend_from_slice(&json_bytes);
    out.extend_from_slice(&(bin.len() as u32).to_le_bytes());
    out.extend_from_slice(&0x004E_4942u32.to_le_bytes()); // "BIN\0"
    out.extend_from_slice(&bin);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leer_json(glb: &[u8]) -> String {
        let largo_json = u32::from_le_bytes([glb[12], glb[13], glb[14], glb[15]]) as usize;
        String::from_utf8(glb[20..20 + largo_json].to_vec()).unwrap()
    }

    #[test]
    fn el_proyecto_guardado_conserva_el_entorno() {
        let p = crate::demo_city::ciudad();
        let ruta = std::env::temp_dir().join(format!("redes-entorno-{}.redes", std::process::id()));
        crate::project_io::save(&p, &ruta).expect("guardar");
        let q = crate::project_io::load(&ruta).expect("cargar");
        let n = q.entorno.as_ref().map(|e| e.calles.len()).unwrap_or(0);
        assert!(n >= 10, "el archivo perdió las calles: {n}");
        let m = triangulos_exportables(&q, false);
        assert!(
            m.familias
                .iter()
                .any(|f| f.starts_with("Av.") || f.starts_with("Calle")),
            "sin calles en la malla: {:?}",
            m.familias
        );
        let _ = std::fs::remove_file(&ruta);
    }

    #[test]
    fn la_ciudad_exporta_calles_y_edificios() {
        let p = crate::demo_city::ciudad();
        let malla = triangulos_exportables(&p, false);
        let hay_calle = malla
            .familias
            .iter()
            .any(|f| f.starts_with("Av.") || f.starts_with("Calle"));
        let hay_cubierta = malla.familias.iter().any(|f| f == "cubierta");
        assert!(hay_calle, "faltan calles: {:?}", malla.familias);
        assert!(hay_cubierta, "faltan cubiertas: {:?}", malla.familias);
    }

    #[test]
    fn el_glb_tiene_cabecera_y_json_validos() {
        let p = crate::demo_catalog::catalogo();
        let glb = exportar_glb(&p, OpcionesGltf::default());
        assert_eq!(&glb[0..4], b"glTF", "magic");
        assert_eq!(
            u32::from_le_bytes([glb[4], glb[5], glb[6], glb[7]]),
            2,
            "versión"
        );
        let total = u32::from_le_bytes([glb[8], glb[9], glb[10], glb[11]]) as usize;
        assert_eq!(total, glb.len(), "el largo declarado debe coincidir");

        let json = leer_json(&glb);
        assert!(json.contains(r#""version":"2.0""#));
        assert!(json.contains(r#""primitives""#));
        // Al menos una primitiva por familia del catálogo.
        let primitivas = json.matches(r#""mode":4"#).count();
        assert!(primitivas >= 10, "primitivas: {primitivas}");
        assert!(json.contains("KHR_materials_unlit"));
    }

    #[test]
    fn las_coordenadas_estan_en_metros_y_dentro_del_modelo() {
        let p = crate::demo_city::ciudad();
        let glb = exportar_glb(&p, OpcionesGltf { con_terreno: false });
        let json = leer_json(&glb);
        // El primer accessor POSITION lleva min/max: deben ser coordenadas locales en metros.
        let inicio = json.find(r#""min":["#).expect("min del POSITION");
        let trozo = &json[inicio..(inicio + 160).min(json.len())];
        let nums: Vec<f32> = trozo
            .split(|c: char| !(c.is_ascii_digit() || c == '-' || c == '.'))
            .filter(|s| !s.is_empty() && *s != "-" && *s != ".")
            .filter_map(|s| s.parse().ok())
            .collect();
        for v in nums.iter().take(6) {
            assert!(v.abs() < 20_000.0, "coordenada fuera de rango: {v}");
        }
    }

    #[test]
    fn el_documento_serializado_pesa_lo_razonable() {
        let p = crate::demo_catalog::catalogo();
        let glb = exportar_glb(&p, OpcionesGltf::default());
        assert!(
            glb.len() > 10_000,
            "un catálogo no puede pesar {} bytes",
            glb.len()
        );
        assert!(glb.len() < 40_000_000, "demasiado grande: {}", glb.len());
    }
}
