//! Ciudad de demostración sobre ladera: topografía montañosa, calles, manzanas construidas y
//! redes de alcantarillado y agua potable con cotas coherentes.
//!
//! Reglas que se respetan (y que las pruebas verifican):
//! - **La tapa del pozo sale a la calle**: `z_rim` es la cota del terreno en el eje de la calle.
//! - **La clave va bajo tierra**: profundidad (tapa − solera) entre 1,2 y 6 m en todos los pozos y
//!   recubrimiento del tubo (rasante − (solera + DI + espesor)) positivo en todos sus extremos.
//! - **Todo colector baja**: la clave decrece en el sentido de flujo, con pendiente mínima 0,4 %.
//! - Los laterales acometen al troncal en el pozo de su misma calle; no hay arcos colgantes.

use std::collections::BTreeMap;

use crate::city::{Calle, Entorno, Manzana};
use crate::families::Value;
use crate::geom::{P2, P3};
use crate::model::{Dato, ElementId, EstadoObjeto, EstadoProyecto, Fuente, Link, Node, Project};
use crate::terrain::{SurfacePoint, Terrain};

/// Origen local del proyecto (UTM 17S, sur de Quito).
pub const ORIGEN_E: f64 = 758_000.0;
pub const ORIGEN_N: f64 = 9_797_000.0;

/// Extensión de la ciudad (m).
pub const ANCHO: f64 = 900.0;
pub const ALTO: f64 = 700.0;

/// Calles: tres avenidas que siguen las curvas de nivel y siete transversales que suben.
const AVENIDAS: [f64; 3] = [165.0, 355.0, 540.0];
const TRANSVERSALES: [f64; 7] = [100.0, 220.0, 340.0, 460.0, 580.0, 700.0, 820.0];
const ANCHO_CALLE: f64 = 14.0;

fn gauss(t: f64) -> f64 {
    (-(t * t)).exp()
}

/// Cota del terreno (m). Ladera que baja hacia el oeste, con dos lomas y un valle central.
pub fn cota(x: f64, y: f64) -> f64 {
    let base = 2500.0 + 0.026 * x;
    let loma_norte = 30.0 * gauss((y - 600.0) / 90.0);
    let loma_sur = 18.0 * gauss((y - 125.0) / 78.0);
    let valle = -13.0 * gauss((y - 355.0) / 50.0);
    let ondulacion = 5.0 * ((x / 145.0).sin() * (y / 185.0).cos());
    let detalle = 0.7 * ((x / 41.0).sin() + (y / 47.0).cos());
    base + loma_norte + loma_sur + valle + ondulacion + detalle
}

fn terreno() -> Terrain {
    let mut pts = Vec::new();
    let mut y = 0.0;
    while y <= ALTO {
        let mut x = 0.0;
        while x <= ANCHO {
            pts.push(SurfacePoint::new(ORIGEN_E + x, ORIGEN_N + y, cota(x, y)));
            x += 12.0;
        }
        y += 12.0;
    }
    Terrain::build("levantamiento-cerro-san-bartolo", 32717, pts)
}

/// Genera la ciudad completa.
pub fn ciudad() -> Project {
    let mut p = Project::new("Ciudad Cerro San Bartolo", "EPSG:32717");
    p.origin = P2::new(ORIGEN_E, ORIGEN_N);
    p.estado = EstadoProyecto::EnRevision;
    p.fuentes = vec![
        Fuente {
            app: "Civil 3D".into(),
            archivo: "levantamiento-cerro-san-bartolo.dxf".into(),
            formato: "dxf".into(),
            importado_en: "2026-11-18".into(),
        },
        Fuente {
            app: "SWMM".into(),
            archivo: "alcantarillado-san-bartolo.inp".into(),
            formato: "swmm".into(),
            importado_en: "2026-11-18".into(),
        },
    ];

    let t = terreno();
    p.terrain = Some(t);

    // --- calles ---
    let mut calles = Vec::new();
    for (i, y) in AVENIDAS.iter().enumerate() {
        let nombre = match i {
            0 => "Av. de los Álamos",
            1 => "Av. Central",
            _ => "Av. Mirador",
        };
        calles.push(Calle {
            nombre: nombre.into(),
            eje: eje_horizontal(*y),
            ancho: ANCHO_CALLE,
        });
    }
    for (i, x) in TRANSVERSALES.iter().enumerate() {
        let nombre = format!("Calle {}-{}", (b'A' + i as u8) as char, i + 1);
        calles.push(Calle {
            nombre,
            eje: eje_vertical(*x),
            ancho: if i % 3 == 1 { 16.0 } else { 12.0 },
        });
    }

    // --- manzanas construidas ---
    let mut manzanas = Vec::new();
    let mut semilla = 0x1234_5678u32;
    let mut azar = |lo: f64, hi: f64| -> f64 {
        semilla = semilla.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let u = ((semilla >> 8) & 0xFFFF) as f64 / 65535.0;
        lo + (hi - lo) * u
    };

    for bx in 0..TRANSVERSALES.len() - 1 {
        for by in 0..AVENIDAS.len() - 1 {
            let x0 = TRANSVERSALES[bx] + ANCHO_CALLE * 0.5 + 4.0;
            let x1 = TRANSVERSALES[bx + 1] - ANCHO_CALLE * 0.5 - 4.0;
            let y0 = AVENIDAS[by] + ANCHO_CALLE * 0.5 + 4.0;
            let y1 = AVENIDAS[by + 1] - ANCHO_CALLE * 0.5 - 4.0;
            if x1 - x0 < 18.0 || y1 - y0 < 18.0 {
                continue;
            }
            // Manzana dividida en lotes 3x3, con retranqueo entre ellos.
            let lotes_x = 3;
            let lotes_y = 3;
            let ancho = (x1 - x0) / lotes_x as f64;
            let fondo = (y1 - y0) / lotes_y as f64;
            for kx in 0..lotes_x {
                for ky in 0..lotes_y {
                    let g = 1.2;
                    let lx0 = x0 + ancho * kx as f64 + g;
                    let lx1 = x0 + ancho * (kx + 1) as f64 - g;
                    let ly0 = y0 + fondo * ky as f64 + g;
                    let ly1 = y0 + fondo * (ky + 1) as f64 - g;
                    if lx1 - lx0 < 6.0 || ly1 - ly0 < 6.0 {
                        continue;
                    }
                    // Más altura sobre la avenida central (comercio) y esquinas.
                    let centro = ky == 1 && kx == 1;
                    let altura = if centro && by == 1 {
                        azar(7.0, 11.0)
                    } else if centro {
                        azar(5.5, 9.0)
                    } else {
                        azar(3.5, 7.5)
                    };
                    let uso = if by == 1 && ky == 1 {
                        "comercio"
                    } else {
                        "vivienda"
                    };
                    manzanas.push(Manzana {
                        poligono: vec![
                            P2::new(ORIGEN_E + lx0, ORIGEN_N + ly0),
                            P2::new(ORIGEN_E + lx1, ORIGEN_N + ly0),
                            P2::new(ORIGEN_E + lx1, ORIGEN_N + ly1),
                            P2::new(ORIGEN_E + lx0, ORIGEN_N + ly1),
                        ],
                        altura: (altura * 10.0).round() / 10.0,
                        uso: uso.into(),
                        tono: azar(0.0, 1.0) as f32,
                    });
                }
            }
        }
    }

    p.entorno = Some(Entorno { calles, manzanas });

    // --- alcantarillado ---
    let mut pozos: BTreeMap<(i64, i64), ElementId> = BTreeMap::new();
    let mut troncal: Vec<ElementId> = Vec::new();

    // Troncal: Av. Central, de este a oeste (el terreno baja hacia el oeste).
    for x in TRANSVERSALES.iter().rev() {
        let id = pozo(
            &mut p,
            ORIGEN_E + x,
            ORIGEN_N + AVENIDAS[1],
            "inspeccion",
            1.30,
        );
        troncal.push(id);
        pozos.insert(((*x * 10.0) as i64, (AVENIDAS[1] * 10.0) as i64), id);
    }
    disenar_troncal(&mut p, &troncal, 0.006);

    // Laterales: cada transversal baja desde la Av. Mirador (norte) y sube desde la Av. de los
    // Álamos (sur) hasta el troncal de su misma calle.
    for x in TRANSVERSALES.iter().rev() {
        let medio_n = pozo(&mut p, ORIGEN_E + x, ORIGEN_N + 448.0, "inspeccion", 1.10);
        let alto_n = pozo(
            &mut p,
            ORIGEN_E + x,
            ORIGEN_N + AVENIDAS[2],
            "inspeccion",
            1.10,
        );
        let medio_s = pozo(&mut p, ORIGEN_E + x, ORIGEN_N + 262.0, "inspeccion", 1.10);
        let alto_s = pozo(
            &mut p,
            ORIGEN_E + x,
            ORIGEN_N + AVENIDAS[0],
            "inspeccion",
            1.10,
        );

        let cruce = pozos[&(((*x * 10.0) as i64), (AVENIDAS[1] * 10.0) as i64)];
        // Norte: alto → medio → cruce
        disenar_lateral(&mut p, &[alto_n, medio_n, cruce], 0.010);
        // Sur: alto → medio → cruce
        disenar_lateral(&mut p, &[alto_s, medio_s, cruce], 0.010);
    }

    // --- agua potable ---
    let tanque_pos = P2::new(ORIGEN_E + 862.0, ORIGEN_N + 636.0);
    let tanque_z = cota(862.0, 636.0);
    let tanque = p
        .add_node(Node {
            id: ElementId::new(0, 0),
            family: "agua.tanque".into(),
            ty: "apoyado_cilindrico".into(),
            code: String::new(),
            pos: tanque_pos,
            z_terrain: tanque_z,
            z_rim: tanque_z + 7.0,
            z_invert: tanque_z,
            attrs: BTreeMap::from([
                (
                    "tipo_tanque".to_string(),
                    Value::Text("apoyado_cilindrico".into()),
                ),
                ("volumen_util".to_string(), Value::Num(850.0)),
                ("cota_piso".to_string(), Value::Num(tanque_z)),
                ("cota_nivel_max".to_string(), Value::Num(tanque_z + 6.0)),
            ]),
            datos: BTreeMap::from([(
                "poblacion_servida".to_string(),
                Dato {
                    valor: 9_400.0,
                    unidad: "hab".into(),
                    fuente: "Municipio".into(),
                },
            )]),
            estado: EstadoObjeto::Aprobado,
            nota: None,
            modelo: None,
            deleted: false,
            sin_dato: Vec::new(),
        })
        .unwrap();

    let mut red_agua = vec![tanque];
    for x in TRANSVERSALES.iter().rev() {
        let z = cota(*x, AVENIDAS[2]);
        let id = p
            .add_node(Node {
                id: ElementId::new(0, 0),
                family: "agua.camara".into(),
                ty: "valvulas".into(),
                code: String::new(),
                pos: P2::new(ORIGEN_E + x, ORIGEN_N + AVENIDAS[2]),
                z_terrain: z,
                z_rim: z,
                z_invert: z - 1.4,
                attrs: BTreeMap::from([
                    ("tipo_camara".to_string(), Value::Text("valvulas".into())),
                    ("diametro_interior".to_string(), Value::Num(1.20)),
                    ("cota_solera".to_string(), Value::Num(z - 1.4)),
                ]),
                datos: BTreeMap::new(),
                estado: EstadoObjeto::Pendiente,
                nota: None,
                modelo: None,
                deleted: false,
                sin_dato: Vec::new(),
            })
            .unwrap();
        red_agua.push(id);
    }
    for w in red_agua.windows(2) {
        let (a, b) = {
            let a = p.node(w[0]).unwrap();
            let b = p.node(w[1]).unwrap();
            ((a.pos, a.z_terrain), (b.pos, b.z_terrain))
        };
        let (pa, pb) = (a.0, b.0);
        p.add_link(Link {
            id: ElementId::new(0, 0),
            family: "agua.tuberia".into(),
            ty: "aduccion".into(),
            code: String::new(),
            from: w[0],
            to: w[1],
            path: vec![
                P3::new(pa.x, pa.y, a.1 - 1.4),
                P3::new(pb.x, pb.y, b.1 - 1.4),
            ],
            d_mm: 200,
            attrs: BTreeMap::from([
                ("material".to_string(), Value::Text("PVC".into())),
                ("coef_hazen".to_string(), Value::Num(140.0)),
            ]),
            datos: BTreeMap::new(),
            estado: EstadoObjeto::Pendiente,
            nota: None,
            cota_solera_inicio: None,
            cota_solera_fin: None,
            deleted: false,
            sin_dato: Vec::new(),
        })
        .ok();
    }

    // Hidrantes en las esquinas del troncal.
    for x in [TRANSVERSALES[1], TRANSVERSALES[3], TRANSVERSALES[5]] {
        let z = cota(x, AVENIDAS[1] + 10.0);
        p.add_node(Node {
            id: ElementId::new(0, 0),
            family: "agua.llave".into(),
            ty: "columna".into(),
            code: String::new(),
            pos: P2::new(ORIGEN_E + x + 9.0, ORIGEN_N + AVENIDAS[1] + 9.0),
            z_terrain: z,
            z_rim: z,
            z_invert: z - 1.2,
            attrs: BTreeMap::from([
                ("tipo_llave".to_string(), Value::Text("columna".into())),
                ("diametro_nominal".to_string(), Value::Int(100)),
            ]),
            datos: BTreeMap::new(),
            estado: EstadoObjeto::Pendiente,
            nota: None,
            modelo: None,
            deleted: false,
            sin_dato: Vec::new(),
        })
        .ok();
    }

    p.commit("demo ciudad");
    p
}

fn eje_horizontal(y: f64) -> Vec<P3> {
    let mut out = Vec::new();
    let mut x = 0.0;
    while x <= ANCHO {
        out.push(P3::new(ORIGEN_E + x, ORIGEN_N + y, cota(x, y) + 0.06));
        x += 15.0;
    }
    out
}

fn eje_vertical(x: f64) -> Vec<P3> {
    let mut out = Vec::new();
    let mut y = AVENIDAS[0];
    while y <= AVENIDAS[2] {
        out.push(P3::new(ORIGEN_E + x, ORIGEN_N + y, cota(x, y) + 0.06));
        y += 15.0;
    }
    out
}

/// Crea un pozo con la tapa a la cota del terreno (la calle) y una clave provisional.
fn pozo(p: &mut Project, e: f64, n: f64, tipo: &str, diametro: f64) -> ElementId {
    let (x, y) = (e - ORIGEN_E, n - ORIGEN_N);
    let z = cota(x, y);
    p.add_node(Node {
        id: ElementId::new(0, 0),
        family: "saneamiento.pozo".into(),
        ty: tipo.into(),
        code: String::new(),
        pos: P2::new(e, n),
        z_terrain: z,
        z_rim: z,
        z_invert: z - 1.6,
        attrs: BTreeMap::from([
            ("diametro_interior".to_string(), Value::Num(diametro)),
            ("cota_tapa".to_string(), Value::Num(z)),
            ("cota_solera".to_string(), Value::Num(z - 1.6)),
            ("material".to_string(), Value::Text("hormigon".into())),
        ]),
        datos: BTreeMap::new(),
        estado: EstadoObjeto::Pendiente,
        nota: None,
        modelo: None,
        deleted: false,
        sin_dato: Vec::new(),
    })
    .unwrap()
}

/// Diseña las claves del troncal en el sentido del flujo: cada pozo queda más bajo que el
/// anterior al menos `pendiente`, y con profundidad (tapa − solera) entre `COVER_MIN` y `COVER_MAX`.
fn disenar_troncal(p: &mut Project, cadena: &[ElementId], pendiente: f64) {
    const COVER_MIN: f64 = 1.20;
    const COVER_MAX: f64 = 6.00;
    let rims: Vec<f64> = cadena.iter().map(|id| p.node(*id).unwrap().z_rim).collect();
    let mut claves = vec![0.0; cadena.len()];
    claves[0] = rims[0] - 2.30;

    for i in 1..cadena.len() {
        let (a, b) = (
            p.node(cadena[i - 1]).unwrap().pos,
            p.node(cadena[i]).unwrap().pos,
        );
        let l = a.dist(b).max(1.0);
        // Al menos `pendiente`, y nunca más somero que COVER_MIN.
        let mut z = (claves[i - 1] - pendiente * l).min(rims[i] - COVER_MIN);
        if rims[i] - z > COVER_MAX {
            z = rims[i] - COVER_MAX;
        }
        claves[i] = z;
    }

    for (i, id) in cadena.iter().enumerate() {
        let clave = claves[i];
        if let Some(n) = p.nodes.iter_mut().find(|n| n.id == *id) {
            n.z_invert = clave;
            n.attrs.insert("cota_solera".to_string(), Value::Num(clave));
        }
        if i + 1 < cadena.len() {
            conectar(
                p,
                cadena[i],
                cadena[i + 1],
                claves[i],
                claves[i + 1],
                315,
                "interceptor",
            );
        }
    }
}

/// Diseña un lateral que **termina** en un pozo del troncal con clave fija.
fn disenar_lateral(p: &mut Project, cadena: &[ElementId], pendiente: f64) {
    const COVER_MIN: f64 = 1.10;
    const COVER_MAX: f64 = 6.00;
    let n = cadena.len();
    let rims: Vec<f64> = cadena.iter().map(|id| p.node(*id).unwrap().z_rim).collect();
    let mut claves = vec![0.0; n];
    claves[n - 1] = p.node(cadena[n - 1]).unwrap().z_invert;

    // Aguas arriba, la clave sube con la pendiente, pero nunca por encima del recubrimiento.
    for i in (0..n - 1).rev() {
        let (a, b) = (
            p.node(cadena[i]).unwrap().pos,
            p.node(cadena[i + 1]).unwrap().pos,
        );
        let l = a.dist(b).max(1.0);
        let z = (claves[i + 1] + pendiente * l).min(rims[i] - COVER_MIN);
        // Si la loma obliga a enterrar más de la cuenta, se sube la clave (pozo de caída).
        claves[i] = z.max(rims[i] - COVER_MAX);
    }

    for (i, id) in cadena.iter().enumerate() {
        let clave = claves[i];
        if let Some(node) = p.nodes.iter_mut().find(|node| node.id == *id) {
            node.z_invert = clave;
            node.attrs
                .insert("cota_solera".to_string(), Value::Num(clave));
        }
    }
    for i in 0..n - 1 {
        conectar(
            p,
            cadena[i],
            cadena[i + 1],
            claves[i],
            claves[i + 1],
            200,
            "colector_sanitario",
        );
    }
}

fn conectar(
    p: &mut Project,
    desde: ElementId,
    hasta: ElementId,
    z_in: f64,
    z_out: f64,
    d_mm: u32,
    tipo: &str,
) {
    let (a, b) = {
        let a = p.node(desde).unwrap();
        let b = p.node(hasta).unwrap();
        (a.pos, b.pos)
    };
    let longitud = a.dist(b);
    let pendiente = if longitud > 0.0 {
        (z_in - z_out) / longitud
    } else {
        0.0
    };
    let mut datos = BTreeMap::new();
    datos.insert(
        "caudal_diseno".to_string(),
        Dato {
            valor: (d_mm as f64 / 100.0) * 3.2,
            unidad: "L/s".into(),
            fuente: "SWMM".into(),
        },
    );
    datos.insert(
        "volumen_excavacion".to_string(),
        Dato {
            valor: (longitud * 3.4).round(),
            unidad: "m³".into(),
            fuente: "Civil 3D".into(),
        },
    );
    p.add_link(Link {
        id: ElementId::new(0, 0),
        family: "saneamiento.tuberia".into(),
        ty: tipo.into(),
        code: String::new(),
        from: desde,
        to: hasta,
        path: vec![P3::new(a.x, a.y, z_in), P3::new(b.x, b.y, z_out)],
        d_mm,
        attrs: BTreeMap::from([
            ("material".to_string(), Value::Text("PVC".into())),
            ("diametro_nominal".to_string(), Value::Int(d_mm as i64)),
            ("pendiente".to_string(), Value::Num(pendiente)),
        ]),
        datos,
        estado: EstadoObjeto::Pendiente,
        nota: None,
        cota_solera_inicio: None,
        cota_solera_fin: None,
        deleted: false,
        sin_dato: Vec::new(),
    })
    .ok();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn proyecto() -> Project {
        ciudad()
    }

    #[test]
    fn la_tapa_del_pozo_esta_en_la_calle() {
        let p = proyecto();
        let t = p.terrain.as_ref().unwrap();
        for n in p
            .nodes
            .iter()
            .filter(|n| !n.deleted && n.family == "saneamiento.pozo")
        {
            let z = t.z_at(n.pos).expect("el pozo debe caer dentro del TIN");
            assert!(
                (n.z_rim - z).abs() < 0.35,
                "{}: tapa {:.2} vs terreno {:.2}",
                n.code,
                n.z_rim,
                z
            );
        }
    }

    #[test]
    fn la_profundidad_del_pozo_es_razonable() {
        let p = proyecto();
        for n in p
            .nodes
            .iter()
            .filter(|n| !n.deleted && n.family == "saneamiento.pozo")
        {
            // Profundidad = tapa − solera. No es el recubrimiento del tubo.
            let profundidad = n.z_rim - n.z_invert;
            assert!(
                (1.0..=6.2).contains(&profundidad),
                "{}: profundidad {:.2} m",
                n.code,
                profundidad
            );
        }
    }

    #[test]
    fn el_recubrimiento_del_tubo_es_positivo() {
        let p = proyecto();
        for l in p.links.iter().filter(|l| !l.deleted) {
            // Recubrimiento = rasante − (clave + espesor) = rasante − (solera + DI + espesor).
            for (id, q) in [(l.from, l.path.first()), (l.to, l.path.last())] {
                let (Some(n), Some(q)) = (p.node(id), q) else {
                    continue;
                };
                let recubrimiento = n.z_rim - (q.z + l.diametro_interior_m() + l.espesor_pared_m());
                assert!(
                    recubrimiento > 0.5,
                    "{}: recubrimiento {:.2} m",
                    l.code,
                    recubrimiento
                );
            }
        }
    }

    #[test]
    fn todos_los_colectores_bajan() {
        let p = proyecto();
        for l in p
            .links
            .iter()
            .filter(|l| !l.deleted && l.family == "saneamiento.tuberia")
        {
            let (zi, zo) = (l.path[0].z, l.path[1].z);
            let pendiente = (zi - zo) / l.length_2d().max(1e-6);
            assert!(
                pendiente >= 0.004,
                "{}: pendiente {:.4} ({zi:.2} → {zo:.2})",
                l.code,
                pendiente
            );
        }
    }

    #[test]
    fn hay_ciudad_y_red() {
        let p = proyecto();
        let e = p.entorno.as_ref().unwrap();
        assert!(e.calles.len() >= 10);
        assert!(e.manzanas.len() >= 30);
        assert!(p.count() > 60);
    }
}
