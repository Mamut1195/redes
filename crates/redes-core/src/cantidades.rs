//! Cantidades del proyecto: lo que se cuenta, se mide y se exporta a una hoja de cálculo.
//!
//! El producto **no calcula** hidráulica ni tierras: suma lo que ya está en el modelo (unidades,
//! longitudes, volúmenes medidos de los modelos y pesos por densidad) y deja cada cifra con su
//! procedencia. Nada se estima por defecto sin decirlo.

use std::collections::BTreeMap;

use crate::model::Project;

/// Una línea del listado.
#[derive(Clone, Debug, PartialEq)]
pub struct Fila {
    /// Agrupación: `familia`, `material`, `dn`, `subtipo`…
    pub grupo: String,
    pub clave: String,
    pub unidad: &'static str,
    pub cantidad: f64,
    /// De dónde sale la cifra (`modelo`, `atributo`, `conteo`, `medido`…).
    pub fuente: String,
    /// Longitud inclinada (3D) cuando la fila es una longitud; `cantidad` sigue siendo la de planta.
    pub cantidad_3d: Option<f64>,
}

/// Densidades por defecto (t/m³) para pasar volumen a peso cuando el material es conocido.
pub const DENSIDADES: &[(&str, f64)] = &[
    ("hormigon", 2.4),
    ("hormigon_armado", 2.5),
    ("acero", 7.85),
    ("hierro", 7.85),
    ("pvc", 1.4),
    ("pead", 0.95),
    ("fibrocemento", 1.9),
    ("mamposteria", 2.2),
    ("grp", 1.9),
    ("prfv", 1.9),
];

pub fn densidad(material: &str) -> Option<f64> {
    // Se normalizan acentos y separadores, y gana la clave **más específica**: «hormigón armado»
    // debe caer en `hormigon_armado` y no en `hormigon`.
    let m: String = material
        .to_lowercase()
        .chars()
        .map(|c| match c {
            'á' => 'a',
            'é' => 'e',
            'í' => 'i',
            'ó' => 'o',
            'ú' => 'u',
            'ü' => 'u',
            '-' | ' ' => '_',
            otro => otro,
        })
        .collect();
    DENSIDADES
        .iter()
        .filter(|(k, _)| m.contains(k))
        .max_by_key(|(k, _)| k.len())
        .map(|(_, d)| *d)
}

/// Unidades por familia (conteo de objetos), en orden alfabético de clave.
pub fn por_familia(p: &Project) -> Vec<Fila> {
    let mut cuenta: BTreeMap<(&str, &str), usize> = BTreeMap::new();
    for n in p.nodes.iter().filter(|n| !n.deleted) {
        *cuenta.entry((n.family.as_str(), "nodo")).or_default() += 1;
    }
    for l in p.links.iter().filter(|l| !l.deleted) {
        *cuenta.entry((l.family.as_str(), "arco")).or_default() += 1;
    }
    cuenta
        .into_iter()
        .map(|((familia, rol), n)| Fila {
            grupo: "familia".into(),
            clave: format!("{familia} ({rol})"),
            unidad: "ud",
            cantidad: n as f64,
            fuente: "conteo".into(),
            cantidad_3d: None,
        })
        .collect()
}

/// Longitud de tubería agrupada por familia, diámetro y material.
pub fn longitudes(p: &Project) -> Vec<Fila> {
    let mut acumula: BTreeMap<(String, u32, String), (f64, f64)> = BTreeMap::new();
    for l in p.links.iter().filter(|l| !l.deleted) {
        let material = l
            .get("material")
            .and_then(|v| v.as_str())
            .unwrap_or("sin_material")
            .to_string();
        let clave = (l.family.clone(), l.d_mm, material);
        let e = acumula.entry(clave).or_default();
        e.0 += l.length_2d();
        e.1 += l.length_3d();
    }
    acumula
        .into_iter()
        .map(|((familia, dn, material), (m, m3d))| Fila {
            grupo: "longitud".into(),
            clave: format!("{familia} · DN {dn} · {material}"),
            unidad: "m",
            cantidad: m,
            fuente: "geometria".into(),
            cantidad_3d: Some(m3d),
        })
        .collect()
}

/// Volumen y área medidos de los modelos, por familia.
pub fn de_modelos(p: &Project) -> Vec<Fila> {
    let mut vol: BTreeMap<String, f64> = BTreeMap::new();
    let mut area: BTreeMap<String, f64> = BTreeMap::new();
    for n in p.nodes.iter().filter(|n| !n.deleted) {
        if let Some(d) = n.datos.get("volumen_m3") {
            *vol.entry(n.family.clone()).or_default() += d.valor;
        }
        if let Some(d) = n.datos.get("area_m2") {
            *area.entry(n.family.clone()).or_default() += d.valor;
        }
    }
    let mut filas = Vec::new();
    for (familia, v) in vol {
        filas.push(Fila {
            grupo: "volumen".into(),
            clave: familia.clone(),
            unidad: "m3",
            cantidad: v,
            fuente: "modelo".into(),
            cantidad_3d: None,
        });
    }
    for (familia, a) in area {
        filas.push(Fila {
            grupo: "area".into(),
            clave: familia,
            unidad: "m2",
            cantidad: a,
            fuente: "modelo".into(),
            cantidad_3d: None,
        });
    }
    filas
}

/// Peso estimado por densidad del material declarado (solo cuando el material es conocido).
pub fn pesos(p: &Project) -> Vec<Fila> {
    let mut acumula: BTreeMap<(String, u64), f64> = BTreeMap::new();
    for n in p.nodes.iter().filter(|n| !n.deleted) {
        let Some(d) = n.datos.get("volumen_m3") else {
            continue;
        };
        let material = n
            .num("material")
            .map(|v| v.to_string())
            .or_else(|| {
                n.get("material")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            })
            .unwrap_or_else(|| "sin_material".into());
        let dens = densidad(&material).unwrap_or(f64::NAN);
        // La densidad puede no ser finita: se guarda su patrón de bits para poder ordenar.
        let clave = (material, dens.to_bits());
        *acumula.entry(clave).or_default() += d.valor;
    }
    acumula
        .into_iter()
        .filter_map(|((material, bits), v)| {
            let dens = f64::from_bits(bits);
            if !dens.is_finite() {
                return None;
            }
            Some(Fila {
                grupo: "peso".into(),
                clave: format!("{material} ({dens:.2} t/m³)"),
                unidad: "t",
                cantidad: v * dens,
                fuente: "densidad_declarada".into(),
                cantidad_3d: None,
            })
        })
        .collect()
}

/// Discrepancia entre lo que declara un modelo y lo que mide la geometría.
#[derive(Clone, Debug)]
pub struct Discrepancia {
    pub codigo: String,
    pub familia: String,
    pub declarado: f64,
    pub medido: f64,
    pub desvio_pct: f64,
}

/// Compara lo que declara la app de origen con lo que mide el modelo de la biblioteca.
///
/// No corrige nada: si SWMM dice 3,4 m³ y la pieza mide 1,2 m³, se muestran los dos y se avisa.
/// Esa es la regla del producto: una discrepancia se ve, no se resuelve sola.
pub fn discrepancias(
    p: &Project,
    bib: Option<&crate::biblioteca::Biblioteca>,
    tolerancia_pct: f64,
) -> Vec<Discrepancia> {
    let mut out = Vec::new();
    let Some(bib) = bib else { return out };
    for n in p.nodes.iter().filter(|n| !n.deleted) {
        let Some(modelo_id) = n.modelo.as_ref() else {
            continue;
        };
        let Some(pieza) = bib.piezas.iter().find(|x| &x.id == modelo_id) else {
            continue;
        };
        let Some(medido) = pieza.volumen_m3 else {
            continue;
        };
        let Some(declarado) = n.datos.get("volumen_m3") else {
            continue;
        };
        let es_del_modelo = declarado.fuente.starts_with("modelo:");
        if es_del_modelo || medido <= 0.0 || !declarado.valor.is_finite() {
            continue;
        }
        let desvio = (declarado.valor - medido) / medido * 100.0;
        if desvio.abs() > tolerancia_pct {
            out.push(Discrepancia {
                codigo: n.code.clone(),
                familia: n.family.clone(),
                declarado: declarado.valor,
                medido,
                desvio_pct: desvio,
            });
        }
    }
    out
}

/// Suma de una lista de filas por unidad.
pub fn totales(filas: &[Fila]) -> Vec<(String, &'static str, f64)> {
    let mut t: BTreeMap<(String, &'static str), f64> = BTreeMap::new();
    for f in filas {
        *t.entry((f.grupo.clone(), f.unidad)).or_default() += f.cantidad;
    }
    t.into_iter().map(|((g, u), c)| (g, u, c)).collect()
}

/// CSV con cabecera y BOM (Excel en español abre bien los acentos y las comas).
pub fn a_csv(filas: &[Fila]) -> String {
    let mut s = String::from("\u{feff}grupo;clave;unidad;cantidad;fuente;cantidad_3d\n");
    for f in filas {
        s.push_str(&format!(
            "{};{};{};{:.3};{};{}\n",
            f.grupo,
            f.clave.replace(';', ","),
            f.unidad,
            f.cantidad,
            f.fuente,
            f.cantidad_3d.map(|v| format!("{v:.3}")).unwrap_or_default()
        ));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cuenta_objetos_y_mide_longitudes() {
        let p = crate::demo_cadena::cadena();
        let fam = por_familia(&p);
        assert!(fam.iter().any(|f| f.clave.starts_with("agua.bocatoma")));
        let long = longitudes(&p);
        let total: f64 = long.iter().map(|f| f.cantidad).sum();
        assert!(
            total > 100.0,
            "la cadena tiene más de 100 m de red: {total}"
        );
        // La longitud por tramo cuadra con la suma de los arcos.
        let directo: f64 = p
            .links
            .iter()
            .filter(|l| !l.deleted)
            .map(|l| l.length_2d())
            .sum();
        assert!((total - directo).abs() < 1e-6, "{total} vs {directo}");
    }

    #[test]
    fn el_csv_abre_en_excel() {
        let p = crate::demo_cadena::cadena();
        let csv = a_csv(&por_familia(&p));
        assert!(csv.starts_with('\u{feff}'));
        assert!(csv.lines().count() > 5);
        assert!(csv.lines().next().unwrap().ends_with("fuente;cantidad_3d"));
    }

    #[test]
    fn la_longitud_3d_va_junto_a_la_de_planta() {
        let mut p = crate::demo_cadena::cadena();
        // Un tramo inclinado: la 3D supera a la de planta y la de planta no cambia.
        let l = p.links.iter_mut().find(|l| !l.deleted).unwrap();
        l.path.last_mut().unwrap().z += 25.0;
        let plana: f64 = longitudes(&p).iter().map(|f| f.cantidad).sum();
        let inclinada: f64 = longitudes(&p).iter().filter_map(|f| f.cantidad_3d).sum();
        let directo: f64 = p
            .links
            .iter()
            .filter(|l| !l.deleted)
            .map(|l| l.length_2d())
            .sum();
        assert!((plana - directo).abs() < 1e-6);
        assert!(inclinada > plana + 1e-6, "{inclinada} vs {plana}");
        let csv = a_csv(&longitudes(&p));
        let fila = csv.lines().nth(1).unwrap();
        assert_eq!(fila.split(';').count(), 6);
        assert!(!fila.ends_with(';'));
    }
    fn biblioteca_con_volumen(volumen: f64) -> crate::biblioteca::Biblioteca {
        crate::biblioteca::Biblioteca {
            schema: "redes.biblioteca/1".into(),
            piezas: vec![crate::biblioteca::Entrada {
                id: "pozo-tipo".into(),
                familia: "saneamiento.pozo".into(),
                subtipo: Some("inspeccion".into()),
                dn_mm: None,
                archivo: "biblioteca/pozo-tipo.glb".into(),
                escala: 1.0,
                triangulos: 120,
                dimensiones_m: [1.2, 1.2, 2.4],
                bytes: 4096,
                volumen_m3: Some(volumen),
                area_m2: Some(9.0),
                cerrada: Some(true),
                extras: Default::default(),
                anclas: Default::default(),
                licencia: None,
                autor: None,
            }],
        }
    }

    #[test]
    fn avisa_cuando_lo_declarado_no_cuadra_con_el_modelo() {
        let mut p = crate::demo_catalog::catalogo();
        p.nodes[0].modelo = Some("pozo-tipo".into());
        p.nodes[0].datos.insert(
            "volumen_m3".to_string(),
            crate::model::Dato {
                valor: 3.4,
                unidad: "m3".into(),
                fuente: "SWMM".into(),
            },
        );
        let bib = biblioteca_con_volumen(1.2);
        let d = discrepancias(&p, Some(&bib), 10.0);
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].codigo, p.nodes[0].code);
        assert!((d[0].declarado - 3.4).abs() < 1e-9);
        assert!((d[0].medido - 1.2).abs() < 1e-9);
        assert!(d[0].desvio_pct > 100.0, "{:?}", d[0]);

        // Dentro de tolerancia no hay aviso, y un dato del propio modelo tampoco.
        let cerca = discrepancias(&p, Some(&biblioteca_con_volumen(3.5)), 10.0);
        assert!(cerca.is_empty());
    }

    #[test]
    fn las_densidades_se_reconocen_por_nombre() {
        assert_eq!(densidad("Hormigón armado"), Some(2.5));
        assert_eq!(densidad("PVC-U"), Some(1.4));
        assert_eq!(densidad("acero"), Some(7.85));
        assert_eq!(densidad("madera"), None);
    }
}
