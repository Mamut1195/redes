//! Piezas paramétricas de la cadena de agua cruda (docs/20): captación, conducción,
//! pretratamiento, tratamiento y salida del alcantarillado.
//!
//! Cada familia compone primitivas de [`crate::parts`] con las medidas que trae el objeto. El
//! dibujo **no depende** de los datos marcados ▣ (caudales, dosis, tiempos): si faltan, se usa un
//! tamaño por defecto razonable y la pieza sigue siendo legible.

use crate::model::Node;
use crate::parts::{
    self, AGUA, HIERRO, HORMIGON, HORMIGON_OSCURO, LATON, Material, PLASTICO, PVC, Parte, barra,
    barra_tapada, caja, cilindro, marco_rectangular, prisma, rampa, tronco,
};

/// Medida del objeto o su valor por defecto (m).
fn m(n: &Node, key: &str, defecto: f32) -> f32 {
    n.num(key)
        .filter(|v| v.is_finite() && *v > 0.0)
        .map(|v| v as f32)
        .unwrap_or(defecto)
        .clamp(0.2, 400.0)
}

fn entero(n: &Node, key: &str, defecto: usize) -> usize {
    n.num(key)
        .filter(|v| v.is_finite() && *v >= 0.0)
        .map(|v| v as usize)
        .unwrap_or(defecto)
        .clamp(0, 40)
}

fn parte(nombre: &'static str, mesh: crate::parts::Mesh) -> Parte {
    Parte { nombre, mesh }
}

/// Caja hueca abierta por arriba: muros y losa de fondo (depósitos y unidades de proceso).
fn deposito(largo: f32, ancho: f32, alto: f32, espesor: f32, mat: Material) -> crate::parts::Mesh {
    let mut mesh = crate::parts::Mesh::default();
    mesh.extend(caja(largo, ancho, espesor, mat).mover([0.0, 0.0, espesor * 0.5]));
    mesh.extend(caja(largo, espesor, alto, mat).mover([
        0.0,
        ancho * 0.5 - espesor * 0.5,
        alto * 0.5,
    ]));
    mesh.extend(caja(largo, espesor, alto, mat).mover([
        0.0,
        -ancho * 0.5 + espesor * 0.5,
        alto * 0.5,
    ]));
    mesh.extend(caja(espesor, ancho, alto, mat).mover([
        largo * 0.5 - espesor * 0.5,
        0.0,
        alto * 0.5,
    ]));
    mesh.extend(caja(espesor, ancho, alto, mat).mover([
        -largo * 0.5 + espesor * 0.5,
        0.0,
        alto * 0.5,
    ]));
    mesh
}

/// Tolva troncocónica invertida bajo el depósito (arenas y lodos).
fn tolva(largo: f32, ancho: f32, alto: f32) -> crate::parts::Mesh {
    let r1 = (largo.min(ancho) * 0.5).clamp(0.3, 6.0);
    tronco(r1, r1 * 0.22, alto, 12, HORMIGON_OSCURO).mover([0.0, 0.0, -alto * 0.5])
}

/// Bocatoma: azud, muros de encauzamiento, reja y canal de limpia.
fn bocatoma(n: &Node, lod: u8) -> Vec<Parte> {
    let largo = m(n, "largo_barraje", 6.0);
    let alto = m(n, "altura_barraje", 1.2);
    let ancho = m(n, "ancho_obra", largo * 0.4);
    let mut azud = caja(largo, ancho.max(0.8), alto * 0.6, HORMIGON);
    azud = azud.mover([0.0, 0.0, alto * 0.3]);
    let mut muros = crate::parts::Mesh::default();
    if lod >= 2 {
        muros.extend(caja(0.6, ancho * 0.4, alto, HORMIGON).mover([
            largo * 0.5,
            -ancho * 0.2,
            alto * 0.5,
        ]));
        muros.extend(caja(0.6, ancho * 0.4, alto, HORMIGON).mover([
            -largo * 0.5,
            -ancho * 0.2,
            alto * 0.5,
        ]));
    }
    let mut reja = crate::parts::Mesh::default();
    let ventana = m(n, "ancho_ventana", largo * 0.5);
    if lod >= 1 {
        let barras = entero(n, "numero_compuertas", 5).max(3);
        for k in 0..barras {
            let x = -ventana * 0.5 + k as f32 * (ventana / (barras - 1) as f32);
            reja.extend(barra(
                [x, ancho * 0.35, alto * 0.2],
                [x, ancho * 0.35, alto * 1.1],
                0.035,
                5,
                HIERRO,
            ));
        }
    }
    let limpia = parts::canal(
        crate::geom::P3::new(-largo as f64 * 0.5, -ancho as f64 * 0.5, alto as f64 * 0.1),
        crate::geom::P3::new(
            -largo as f64 * 0.5 - 4.0,
            -ancho as f64 * 0.5,
            alto as f64 * 0.05,
        ),
        1.2,
        0.5,
    );
    vec![
        parte("azud", azud),
        parte("muros", muros),
        parte("reja", reja),
        parte("canal de limpia", limpia),
    ]
}

/// Cámara de conducción: caja visitable, losa, tapa, escalera y la tubería que la atraviesa.
fn camara_conduccion(n: &Node, lod: u8) -> Vec<Parte> {
    let largo = m(n, "largo", 2.0);
    let ancho = m(n, "ancho", 2.0);
    let alto = m(n, "altura_util", 2.2);
    let mut cuerpo = deposito(largo, ancho, alto, 0.2, HORMIGON);
    let mut tapa =
        caja(largo * 0.9, ancho * 0.9, 0.12, HORMIGON_OSCURO).mover([0.0, 0.0, alto + 0.06]);
    let mut accesorios = crate::parts::Mesh::default();
    if lod >= 1 {
        let diam = m(n, "diametro_tapa", 0.6);
        tapa.extend(
            crate::parts::anillo(diam * 0.5, diam * 0.4, 16, HIERRO).mover([0.0, 0.0, alto + 0.13]),
        );
        for k in 0..5 {
            accesorios.extend(barra(
                [-largo * 0.4, -ancho * 0.4, 0.3 + k as f32 * 0.45],
                [-largo * 0.4, -ancho * 0.4, 0.6 + k as f32 * 0.45],
                0.025,
                5,
                HIERRO,
            ));
        }
        let dn = m(n, "diametro_interior", 1.5);
        cuerpo.extend(caja(dn, dn, 0.25, AGUA).mover([0.0, 0.0, 0.25]));
    }
    let tuberia = barra_tapada(
        [-largo * 0.8, 0.0, alto * 0.45],
        [largo * 0.8, 0.0, alto * 0.35],
        m(n, "diametro_tuberia", 200.0) / 2000.0,
        10,
        PVC,
    );
    vec![
        parte("cámara", cuerpo),
        parte("tapa", tapa),
        parte("escalera", accesorios),
        parte("tubería", tuberia),
    ]
}

/// Unidad de proceso rectangular con tabiques (mezcla, floculación, sedimentación, filtración).
fn unidad_tabiques(
    n: &Node,
    lod: u8,
    tabiques: usize,
    con_tolva: bool,
    con_lecho: bool,
) -> Vec<Parte> {
    let largo = m(n, "largo", 6.0);
    let ancho = m(n, "ancho", 4.0);
    let alto = m(n, "altura_util", 3.0);
    let cuerpo = deposito(largo, ancho, alto, 0.25, HORMIGON);
    let mut agua = crate::parts::Mesh::default();
    if lod >= 1 {
        agua.extend(caja(largo - 0.6, ancho - 0.6, 0.05, AGUA).mover([0.0, 0.0, alto * 0.72]));
    }
    let mut pantallas = crate::parts::Mesh::default();
    let n_tab = entero(n, "numero_camaras", tabiques).max(1);
    for k in 0..n_tab {
        let x = -largo * 0.5 + (k as f32 + 1.0) * largo / (n_tab as f32 + 1.0);
        pantallas.extend(caja(0.12, ancho * 0.9, alto * 0.85, HORMIGON).mover([
            x,
            0.0,
            alto * 0.425,
        ]));
    }
    let mut extras = crate::parts::Mesh::default();
    if con_tolva {
        extras.extend(tolva(largo * 0.6, ancho * 0.6, alto * 0.35).mover([0.0, 0.0, -0.1]));
    }
    if con_lecho {
        let lecho = m(n, "altura_lecho", 0.8);
        let celdas = entero(n, "numero_celdas", 2).max(1);
        for k in 0..celdas {
            let w = largo / celdas as f32;
            let x = -largo * 0.5 + (k as f32 + 0.5) * w;
            extras.extend(caja(w * 0.88, ancho * 0.85, lecho, PLASTICO).mover([
                x,
                0.0,
                lecho * 0.5 + 0.25,
            ]));
        }
    }
    vec![
        parte("unidad", cuerpo),
        parte("agua", agua),
        parte("tabiques", pantallas),
        parte("equipo", extras),
    ]
}

/// Sedimentador y desarenador: depósito con tolva, rasqueta y (si procede) placas inclinadas.
fn sedimentador(n: &Node, lod: u8, placas: bool) -> Vec<Parte> {
    let largo = m(n, "largo", 8.0);
    let ancho = m(n, "ancho", 5.0);
    let alto = m(n, "altura_util", 3.0);
    let mut partes = Vec::new();
    let mut cuerpo = deposito(largo, ancho, alto, 0.3, HORMIGON);
    if lod >= 1 {
        cuerpo.extend(tolva(largo * 0.35, ancho * 0.35, alto * 0.4).mover([
            largo * 0.2,
            0.0,
            -0.1,
        ]));
    }
    partes.push(parte("depósito", cuerpo));
    let mut agua = crate::parts::Mesh::default();
    if lod >= 1 {
        agua.extend(caja(largo - 0.7, ancho - 0.7, 0.05, AGUA).mover([0.0, 0.0, alto * 0.66]));
    }
    partes.push(parte("agua", agua));
    if placas && lod >= 3 {
        let mut modulos = crate::parts::Mesh::default();
        let separacion = m(n, "separacion_placas", 0.6);
        let filas = ((largo * 0.8) / separacion).ceil() as usize;
        for k in 0..filas.min(12) {
            let x = -largo * 0.4 + k as f32 * separacion;
            modulos.extend(
                rampa(
                    &[[-0.3, 0.0], [0.3, 0.0], [0.3, 1.0], [-0.3, 1.0]],
                    0.05,
                    alto * 0.55,
                    PLASTICO,
                )
                .mover([x, 0.0, alto * 0.2]),
            );
        }
        partes.push(parte("placas", modulos));
    }
    if lod >= 2 {
        let mut rasqueta = crate::parts::Mesh::default();
        rasqueta.extend(barra(
            [-largo * 0.45, 0.0, alto + 0.15],
            [largo * 0.45, 0.0, alto + 0.15],
            0.04,
            6,
            HIERRO,
        ));
        partes.push(parte("rasqueta", rasqueta));
    }
    partes
}

/// Filtro: celdas con lecho, canaletas de lavado y falso fondo.
fn filtro(n: &Node, lod: u8) -> Vec<Parte> {
    let largo = m(n, "largo", 6.0);
    let ancho = m(n, "ancho", 4.0);
    let alto = m(n, "altura_total", 3.2);
    let lecho = m(n, "altura_lecho", 0.9);
    let celdas = entero(n, "numero_celdas", 2).max(1);
    let mut cuerpo = deposito(largo, ancho, alto, 0.3, HORMIGON);
    let mut arena = crate::parts::Mesh::default();
    let mut canaletas = crate::parts::Mesh::default();
    let w = largo / celdas as f32;
    for k in 0..celdas {
        let x = -largo * 0.5 + (k as f32 + 0.5) * w;
        arena.extend(caja(w * 0.9, ancho * 0.85, lecho, PLASTICO).mover([
            x,
            0.0,
            0.3 + lecho * 0.5,
        ]));
        if lod >= 2 {
            for j in 0..3 {
                let y = -ancho * 0.3 + j as f32 * ancho * 0.3;
                canaletas.extend(barra(
                    [x - w * 0.4, y, alto * 0.85],
                    [x + w * 0.4, y, alto * 0.85],
                    0.06,
                    6,
                    HORMIGON_OSCURO,
                ));
            }
        }
    }
    if lod >= 2 {
        cuerpo.extend(caja(largo - 0.6, ancho - 0.6, 0.1, HORMIGON_OSCURO).mover([0.0, 0.0, 0.25]));
    }
    vec![
        parte("celdas", cuerpo),
        parte("lecho", arena),
        parte("canaletas", canaletas),
    ]
}

/// Desinfección en línea: gabinete, tanque de solución y conexión.
fn desinfeccion(n: &Node, lod: u8) -> Vec<Parte> {
    let alto = if n.ty == "uv" || n.ty == "ozono" {
        1.6
    } else {
        1.2
    };
    let mut gabinete = caja(1.4, 0.8, alto, PLASTICO);
    gabinete = gabinete.mover([0.0, 0.0, alto * 0.5]);
    let mut tanque = crate::parts::Mesh::default();
    if lod >= 1 {
        let r = (m(n, "capacidad_tanque", 1.0).sqrt() * 0.25).clamp(0.25, 1.2);
        tanque.extend(cilindro(r, 1.4, 12, PLASTICO).mover([0.0, -1.2, 0.7]));
    }
    let dn = m(n, "diametro_conexion", 100.0) / 2000.0;
    let tuberia = barra_tapada([-1.2, 0.8, 0.4], [1.2, 0.8, 0.4], dn, 10, PVC);
    vec![
        parte("gabinete", gabinete),
        parte("tanque de solución", tanque),
        parte("tubería", tuberia),
    ]
}

/// Estación de bombeo: cámara enterrada, tapa, bombas y las dos tuberías.
fn bombeo(n: &Node, lod: u8) -> Vec<Parte> {
    let prof = m(n, "profundidad", 3.0);
    let diam = m(n, "diametro_interior", 2.4);
    let mut camara = crate::parts::Mesh::default();
    if n.ty == "camara_seca" {
        let largo = m(n, "largo", 4.0);
        let ancho = m(n, "ancho", 3.0);
        camara.extend(caja(largo, ancho, 0.3, HORMIGON).mover([0.0, 0.0, -prof + 0.15]));
        camara.extend(deposito(largo, ancho, prof, 0.25, HORMIGON).mover([0.0, 0.0, -prof * 0.5]));
    } else {
        camara.extend(
            crate::parts::tubo(diam * 0.5, diam * 0.5 - 0.2, prof, 16, HORMIGON).mover([
                0.0,
                0.0,
                -prof * 0.5,
            ]),
        );
    }
    let mut tapa = crate::parts::Mesh::default();
    if lod >= 1 {
        tapa.extend(
            crate::parts::anillo(diam * 0.45, diam * 0.38, 16, HIERRO).mover([0.0, 0.0, 0.02]),
        );
    }
    let bombas = entero(n, "numero_bombas", 2).max(1);
    let mut equipo = crate::parts::Mesh::default();
    for k in 0..bombas {
        let ang = k as f32 / bombas as f32 * std::f32::consts::TAU;
        let (x, y) = (ang.cos() * diam * 0.2, ang.sin() * diam * 0.2);
        equipo.extend(cilindro(0.18, 0.7, 8, HIERRO).mover([x, y, -prof + 0.35]));
        if lod >= 2 {
            equipo.extend(barra_tapada([x, y, -prof + 0.7], [x, y, 0.0], 0.06, 8, PVC));
        }
    }
    let succion = barra_tapada(
        [-diam * 0.6, 0.0, -prof * 0.6],
        [0.0, 0.0, -prof * 0.5],
        0.12,
        10,
        PVC,
    );
    let impulsion = barra_tapada(
        [0.0, 0.0, -prof * 0.3],
        [diam * 0.9, 0.0, 0.6],
        0.1,
        10,
        PVC,
    );
    vec![
        parte("cámara", camara),
        parte("tapa", tapa),
        parte("bombas", equipo),
        parte("succión", succion),
        parte("impulsión", impulsion),
    ]
}

/// Reja o criba: marco rectangular y barras.
fn reja(n: &Node, lod: u8) -> Vec<Parte> {
    let ancho = m(n, "ancho", 1.2);
    let alto = m(n, "altura", 1.4);
    let mut marco = marco_rectangular(ancho, alto, 0.08, HIERRO);
    marco = marco.mover([0.0, 0.0, alto * 0.5]);
    let mut barras = crate::parts::Mesh::default();
    let nb = entero(n, "numero_barras", 8).max(2);
    if lod >= 1 {
        for k in 0..nb {
            let x = -ancho * 0.5 + (k as f32 + 0.5) * ancho / nb as f32;
            barras.extend(barra(
                [x, 0.0, 0.05],
                [x, 0.0, alto - 0.05],
                0.02,
                5,
                HIERRO,
            ));
        }
    }
    vec![parte("marco", marco), parte("barras", barras)]
}

/// Compuerta: marco, hoja, husillo y volante.
fn compuerta(n: &Node, lod: u8) -> Vec<Parte> {
    let ancho = m(n, "ancho", 1.0);
    let alto = m(n, "altura", 1.0);
    let vastago = m(n, "altura_vastago", 1.6);
    let marco =
        marco_rectangular(ancho + 0.16, alto + 0.16, 0.1, HIERRO).mover([0.0, 0.0, alto * 0.5]);
    let hoja = caja(ancho, 0.08, alto, HIERRO).mover([0.0, 0.0, alto * 0.5]);
    let mut husillo = crate::parts::Mesh::default();
    if lod >= 1 {
        husillo.extend(barra(
            [0.0, 0.0, alto],
            [0.0, 0.0, alto + vastago],
            0.04,
            6,
            HIERRO,
        ));
        husillo.extend(crate::parts::anillo(0.16, 0.13, 12, HIERRO).mover([
            0.0,
            0.0,
            alto + vastago,
        ]));
    }
    vec![
        parte("marco", marco),
        parte("hoja", hoja),
        parte("husillo", husillo),
    ]
}

/// Aforador: garganta, transiciones y limnímetro.
fn aforador(n: &Node, lod: u8) -> Vec<Parte> {
    let ancho = m(n, "ancho_canal", 1.6);
    let garganta = m(n, "ancho_garganta", ancho * 0.45);
    let alto = 0.9;
    let mut cuerpo = crate::parts::Mesh::default();
    cuerpo.extend(caja(1.4, ancho, alto, HORMIGON).mover([0.0, 0.0, alto * 0.5]));
    cuerpo.extend(caja(1.0, garganta, alto * 0.8, HORMIGON).mover([0.0, 0.0, alto * 0.4]));
    let mut transiciones = crate::parts::Mesh::default();
    if lod >= 1 {
        let a = [
            [-1.4, -ancho * 0.5],
            [-0.7, -garganta * 0.5],
            [-0.7, garganta * 0.5],
            [-1.4, ancho * 0.5],
        ];
        transiciones.extend(prisma(&a, alto * 0.5, HORMIGON).mover([0.0, 0.0, alto * 0.25]));
    }
    let mut limnimetro = crate::parts::Mesh::default();
    if lod >= 2 {
        limnimetro.extend(barra(
            [0.6, ancho * 0.45, 0.0],
            [0.6, ancho * 0.45, alto * 1.6],
            0.02,
            6,
            LATON,
        ));
    }
    vec![
        parte("garganta", cuerpo),
        parte("transiciones", transiciones),
        parte("limnímetro", limnimetro),
    ]
}

/// Lecho de secado: celdas bajas con drenes.
fn lecho_secado(n: &Node, lod: u8) -> Vec<Parte> {
    let largo = m(n, "largo", 8.0);
    let ancho = m(n, "ancho", 6.0);
    let alto = m(n, "altura_muro", 0.9);
    let celdas = entero(n, "numero_celdas", 3).max(1);
    let mut cuerpo = deposito(largo, ancho, alto, 0.2, HORMIGON).mover([0.0, 0.0, alto * 0.5]);
    let mut drenes = crate::parts::Mesh::default();
    if lod >= 1 {
        for k in 0..celdas {
            let x = -largo * 0.5 + (k as f32 + 0.5) * largo / celdas as f32;
            cuerpo.extend(caja(0.15, ancho * 0.95, alto, HORMIGON).mover([
                x - largo / celdas as f32 * 0.5,
                0.0,
                alto * 0.5,
            ]));
            drenes.extend(barra(
                [x, -ancho * 0.4, 0.25],
                [x, ancho * 0.4, 0.25],
                0.05,
                6,
                PLASTICO,
            ));
        }
    }
    vec![parte("celdas", cuerpo), parte("drenes", drenes)]
}

/// Caseta de proceso: volumen, cubierta y puerta.
fn caseta(n: &Node, lod: u8) -> Vec<Parte> {
    let largo = m(n, "largo", 4.0);
    let ancho = m(n, "ancho", 3.0);
    let alto = m(n, "altura", 2.6);
    let cuerpo = caja(largo, ancho, alto, HORMIGON).mover([0.0, 0.0, alto * 0.5]);
    let cubierta =
        caja(largo * 1.08, ancho * 1.08, 0.18, HORMIGON_OSCURO).mover([0.0, 0.0, alto + 0.09]);
    let mut puerta = crate::parts::Mesh::default();
    if lod >= 1 {
        puerta.extend(caja(0.02, 0.9, 2.0, HIERRO).mover([largo * 0.5 + 0.01, 0.0, 1.0]));
    }
    vec![
        parte("caseta", cuerpo),
        parte("cubierta", cubierta),
        parte("puerta", puerta),
    ]
}

/// Laguna de estabilización: taludes, fondo y vertedero de salida.
fn laguna(n: &Node, lod: u8) -> Vec<Parte> {
    let largo = m(n, "largo", 40.0).min(200.0);
    let ancho = m(n, "ancho", 25.0).min(150.0);
    let prof = m(n, "profundidad", 2.0);
    let talud = m(n, "talud", 2.0).min(6.0);
    let dx = prof * talud;
    let poly = vec![
        [-largo * 0.5, -ancho * 0.5],
        [largo * 0.5, -ancho * 0.5],
        [largo * 0.5, ancho * 0.5],
        [-largo * 0.5, ancho * 0.5],
    ];
    let mut cuerpo = prisma(&poly, 0.2, HORMIGON_OSCURO).mover([0.0, 0.0, -prof]);
    if lod >= 1 {
        // Taludes: cuatro rampas que suben del fondo a la corona.
        cuerpo.extend(rampa(
            &[
                [-largo * 0.5, -ancho * 0.5],
                [largo * 0.5, -ancho * 0.5],
                [largo * 0.5, -ancho * 0.5 + dx],
                [-largo * 0.5, -ancho * 0.5 + dx],
            ],
            -prof,
            0.0,
            HORMIGON,
        ));
        cuerpo.extend(rampa(
            &[
                [-largo * 0.5, ancho * 0.5],
                [largo * 0.5, ancho * 0.5],
                [largo * 0.5, ancho * 0.5 - dx],
                [-largo * 0.5, ancho * 0.5 - dx],
            ],
            -prof,
            0.0,
            HORMIGON,
        ));
        cuerpo.extend(rampa(
            &[
                [-largo * 0.5, -ancho * 0.5],
                [-largo * 0.5, ancho * 0.5],
                [-largo * 0.5 + dx, ancho * 0.5],
                [-largo * 0.5 + dx, -ancho * 0.5],
            ],
            -prof,
            0.0,
            HORMIGON,
        ));
        cuerpo.extend(rampa(
            &[
                [largo * 0.5, -ancho * 0.5],
                [largo * 0.5, ancho * 0.5],
                [largo * 0.5 - dx, ancho * 0.5],
                [largo * 0.5 - dx, -ancho * 0.5],
            ],
            -prof,
            0.0,
            HORMIGON,
        ));
    }
    let agua = caja(largo - 2.0 * dx, ancho - 2.0 * dx, 0.06, AGUA).mover([0.0, 0.0, -prof * 0.35]);
    vec![parte("vaso", cuerpo), parte("agua", agua)]
}

/// Reactor biológico: celdas, difusores y sopladores.
fn reactor(n: &Node, lod: u8) -> Vec<Parte> {
    let largo = m(n, "largo", 12.0);
    let ancho = m(n, "ancho", 8.0);
    let alto = m(n, "altura_util", 4.5);
    let mut cuerpo = deposito(largo, ancho, alto, 0.3, HORMIGON);
    let celdas = entero(n, "numero_camaras", 2).max(1);
    let mut difusores = crate::parts::Mesh::default();
    if lod >= 1 {
        for k in 1..celdas {
            let x = -largo * 0.5 + k as f32 * largo / celdas as f32;
            cuerpo.extend(caja(0.2, ancho * 0.95, alto * 0.9, HORMIGON).mover([
                x,
                0.0,
                alto * 0.45,
            ]));
        }
        let filas = entero(n, "numero_difusores", 6).clamp(2, 12);
        for k in 0..filas {
            let x = -largo * 0.4 + k as f32 * (largo * 0.8 / filas as f32);
            difusores.extend(barra(
                [x, -ancho * 0.35, 0.4],
                [x, ancho * 0.35, 0.4],
                0.05,
                6,
                PLASTICO,
            ));
        }
    }
    let agua = caja(largo - 0.6, ancho - 0.6, 0.05, AGUA).mover([0.0, 0.0, alto * 0.7]);
    vec![
        parte("reactor", cuerpo),
        parte("difusores", difusores),
        parte("agua", agua),
    ]
}

/// Pieza de una familia de la cadena. `None` si la familia no es de este módulo.
pub fn piezas(n: &Node, lod: u8) -> Option<Vec<Parte>> {
    let v = match n.family.as_str() {
        "agua.bocatoma" => bocatoma(n, lod),
        "agua.camara" => camara_conduccion(n, lod),
        "agua.mezcla" => unidad_tabiques(n, lod, 1, false, false),
        "agua.floculador" => unidad_tabiques(n, lod, 3, false, false),
        "agua.sedimentador" => sedimentador(n, lod, true),
        "agua.filtro" => filtro(n, lod),
        "agua.desinfeccion" => desinfeccion(n, lod),
        "agua.bombeo" => bombeo(n, lod),
        "comun.desarenador" => sedimentador(n, lod, false),
        "comun.reja" => reja(n, lod),
        "comun.compuerta" => compuerta(n, lod),
        "comun.aforador" => aforador(n, lod),
        "comun.lecho_secado" => lecho_secado(n, lod),
        "comun.caseta" => caseta(n, lod),
        "saneamiento.sedimentador" => sedimentador(n, lod, n.ty == "lamela"),
        "saneamiento.reactor" => reactor(n, lod),
        "saneamiento.laguna" => laguna(n, lod),
        _ => return None,
    };
    Some(v)
}
