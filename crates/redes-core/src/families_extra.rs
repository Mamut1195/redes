//! Huecos del registro (docs/20 §4 y §5), aplicados sobre el catálogo base.
//!
//! Se mantiene aquí, como dato, lo que el registro original no tenía: 17 subtipos que faltaban en
//! familias existentes y 62 atributos nuevos. Las 13 claves retiradas (docs/20 §4.2) ya no están en
//! el catálogo base y no tienen alias. El motor no cambia: `build()` construye el catálogo y luego
//! llama a [`aplicar`].

use crate::families::{AttrType, Category, FamilyDef, Group, TopoRole, Unit, attr, subtype};

/// (familia, clave, etiqueta, unidad, tipo, obligatorio, grupo)
type AttrSpec = (
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    bool,
    &'static str,
);

const UNIDADES: &[(&str, Unit)] = &[
    ("m", Unit::M),
    ("mm", Unit::Mm),
    ("m2", Unit::M2),
    ("m3", Unit::M3),
    ("lps", Unit::Lps),
    ("mps", Unit::Mps),
    ("pct", Unit::Percent),
    ("mperm", Unit::Mperm),
    ("mca", Unit::Mca),
    ("deg", Unit::Deg),
    ("kw", Unit::Kw),
    ("m3h", Unit::M3h),
    ("m3d", Unit::M3d),
    ("mgl", Unit::MgL),
    ("h", Unit::H),
    ("min", Unit::Min),
    ("unt", Unit::Unt),
    ("s1", Unit::S1),
    ("none", Unit::None),
];

fn unidad(k: &str) -> Unit {
    UNIDADES
        .iter()
        .find(|(n, _)| *n == k)
        .map(|(_, u)| *u)
        .unwrap_or(Unit::None)
}

fn grupo(k: &str) -> Group {
    match k {
        "geo" => Group::Geometrico,
        "hid" => Group::Hidraulico,
        "est" => Group::Estructural,
        "mat" => Group::Material,
        "ope" => Group::Operacion,
        _ => Group::General,
    }
}

fn tipo(k: &str) -> AttrType {
    match k {
        "num" => AttrType::Num,
        "int" => AttrType::Int,
        "bool" => AttrType::Bool,
        "text" => AttrType::Text,
        _ => AttrType::Text,
    }
}

/// Atributos nuevos por familia (§5.1). `enum:` se resuelve a `AttrType::Enum` con sus opciones.
const ATTRS: &[(&str, &[AttrSpec])] = &[
    (
        "agua.captacion",
        &[
            (
                "caudal_disponible",
                "Caudal disponible aforado",
                "lps",
                "num",
                true,
                "hid",
            ),
            (
                "caudal_ecologico",
                "Caudal ecológico",
                "lps",
                "num",
                false,
                "hid",
            ),
            ("nivel_estatico", "Nivel estático", "m", "num", false, "hid"),
            ("nivel_dinamico", "Nivel dinámico", "m", "num", false, "hid"),
            ("cota_agua", "Cota de agua", "m", "num", false, "geo"),
            ("cota_umbral", "Cota de umbral", "m", "num", false, "geo"),
            (
                "tipo_bomba",
                "Tipo de bomba",
                "none",
                "enum:sumergible|centrifuga_horizontal|vertical|eyector",
                false,
                "gen",
            ),
            (
                "potencia_bomba",
                "Potencia de bomba",
                "kw",
                "num",
                false,
                "ope",
            ),
            (
                "diametro_succion",
                "Diámetro de succión",
                "mm",
                "num",
                false,
                "hid",
            ),
            (
                "numero_unidades",
                "Número de unidades",
                "none",
                "int",
                false,
                "gen",
            ),
            ("calidad_turbiedad", "Turbiedad", "unt", "num", false, "ope"),
        ],
    ),
    (
        "agua.tuberia",
        &[
            (
                "cota_clave_fin",
                "Cota clave final",
                "m",
                "num",
                true,
                "geo",
            ),
            (
                "regimen",
                "Régimen",
                "none",
                "enum:gravedad|bombeo|lamina_libre",
                true,
                "hid",
            ),
            (
                "tipo_junta",
                "Tipo de junta",
                "none",
                "enum:espiga_campana|bridada|soldada|electrofusion|mecanica",
                false,
                "mat",
            ),
            ("recubrimiento", "Recubrimiento", "m", "num", false, "geo"),
            (
                "numero_tramos",
                "Número de tramos",
                "none",
                "int",
                false,
                "gen",
            ),
        ],
    ),
    (
        "agua.tanque",
        &[
            ("cota_rebose", "Cota de rebose", "m", "num", true, "hid"),
            ("cota_desague", "Cota de desagüe", "m", "num", false, "hid"),
            ("borde_libre", "Borde libre", "m", "num", true, "geo"),
            (
                "tipo_entrada",
                "Tipo de entrada",
                "none",
                "enum:flotador|compuerta|vertedero",
                false,
                "hid",
            ),
            ("volumen_total", "Volumen total", "m3", "num", false, "hid"),
            (
                "diametro_entrada",
                "Diámetro de entrada",
                "mm",
                "num",
                false,
                "hid",
            ),
            (
                "diametro_salida",
                "Diámetro de salida",
                "mm",
                "num",
                false,
                "hid",
            ),
        ],
    ),
    (
        "agua.valvula",
        &[
            (
                "tipo_accionamiento",
                "Accionamiento",
                "none",
                "enum:manual|electrico|hidraulico|neumatico",
                true,
                "ope",
            ),
            (
                "consigna_presion",
                "Consigna de presión",
                "mca",
                "num",
                false,
                "hid",
            ),
            (
                "consigna_caudal",
                "Consigna de caudal",
                "lps",
                "num",
                false,
                "hid",
            ),
            (
                "rango_tarado",
                "Rango de tarado",
                "mca",
                "num",
                false,
                "hid",
            ),
            (
                "diametro_bypass",
                "Diámetro de by-pass",
                "mm",
                "num",
                false,
                "gen",
            ),
        ],
    ),
    (
        "agua.medidor",
        &[
            ("q_max", "Caudal máximo", "m3h", "num", true, "hid"),
            ("q_min", "Caudal mínimo", "m3h", "num", true, "hid"),
            (
                "perdida_carga_qnom",
                "Pérdida de carga a Q nominal",
                "mca",
                "num",
                false,
                "hid",
            ),
            (
                "clase_metrologica",
                "Clase metrológica",
                "none",
                "enum:A|B|C",
                false,
                "gen",
            ),
            (
                "tipo_lectura",
                "Tipo de lectura",
                "none",
                "enum:local|remota|pulsos",
                false,
                "ope",
            ),
            (
                "numero_serie",
                "Número de serie",
                "none",
                "text",
                false,
                "ope",
            ),
        ],
    ),
    (
        "saneamiento.pozo",
        &[
            (
                "diametro_tapa",
                "Diámetro de tapa",
                "mm",
                "num",
                true,
                "geo",
            ),
            (
                "tipo_tapa",
                "Tipo de tapa",
                "none",
                "enum:peatonal|vehicular_ligera|vehicular_pesada",
                false,
                "gen",
            ),
            (
                "sistema_constructivo",
                "Sistema constructivo",
                "none",
                "enum:fundido|prefabricado",
                false,
                "mat",
            ),
            ("cota_rasante", "Cota de rasante", "m", "num", false, "geo"),
        ],
    ),
    (
        "saneamiento.camara",
        &[
            (
                "numero_tapas",
                "Número de tapas",
                "none",
                "int",
                true,
                "gen",
            ),
            (
                "diametro_tapa",
                "Diámetro de tapa",
                "mm",
                "num",
                false,
                "geo",
            ),
            ("cota_tapa", "Cota de tapa", "m", "num", true, "geo"),
            (
                "espesor_pared",
                "Espesor de pared",
                "m",
                "num",
                false,
                "est",
            ),
            ("espesor_losa", "Espesor de losa", "m", "num", false, "est"),
            (
                "tipo_vertedero",
                "Tipo de vertedero",
                "none",
                "enum:delgada|ancha",
                false,
                "hid",
            ),
        ],
    ),
    (
        "saneamiento.tuberia",
        &[
            (
                "cota_clave_fin",
                "Cota clave final",
                "m",
                "num",
                true,
                "geo",
            ),
            (
                "tipo_junta",
                "Tipo de junta",
                "none",
                "enum:espiga_campana|bridada|soldada|electrofusion|mecanica",
                false,
                "mat",
            ),
            (
                "rigidez_anular",
                "Rigidez anular",
                "none",
                "enum:SN2|SN4|SN8",
                false,
                "mat",
            ),
        ],
    ),
    (
        "saneamiento.bombeo",
        &[
            (
                "potencia_unitaria",
                "Potencia unitaria",
                "kw",
                "num",
                true,
                "ope",
            ),
            (
                "tipo_bomba",
                "Tipo de bomba",
                "none",
                "enum:sumergible|centrifuga_horizontal|vertical|eyector",
                true,
                "gen",
            ),
            (
                "nivel_arranque",
                "Nivel de arranque",
                "m",
                "num",
                false,
                "ope",
            ),
            ("nivel_parada", "Nivel de parada", "m", "num", false, "ope"),
            (
                "tiempo_retencion",
                "Tiempo de retención",
                "min",
                "num",
                false,
                "hid",
            ),
        ],
    ),
    (
        "agua.planta",
        &[
            (
                "tipo_tratamiento",
                "Tipo de tratamiento",
                "none",
                "enum:I|II_A|II_B|II_C|III",
                true,
                "gen",
            ),
            ("caudal_punta", "Caudal punta", "lps", "num", false, "hid"),
            (
                "tren_proceso",
                "Tren de proceso",
                "none",
                "text",
                false,
                "gen",
            ),
            (
                "cota_hidraulica",
                "Cota hidráulica",
                "m",
                "num",
                true,
                "geo",
            ),
            ("area", "Área", "m2", "num", false, "geo"),
            (
                "punto_descarga",
                "Punto de descarga",
                "none",
                "text",
                false,
                "gen",
            ),
        ],
    ),
    (
        "comun.cruce",
        &[
            (
                "angulo_cruce",
                "Ángulo de cruce",
                "deg",
                "num",
                false,
                "geo",
            ),
            (
                "permiso_requerido",
                "Permiso requerido",
                "none",
                "text",
                false,
                "ope",
            ),
            (
                "entidad_autorizante",
                "Entidad autorizante",
                "none",
                "text",
                false,
                "ope",
            ),
        ],
    ),
    (
        "comun.cuerpo_receptor",
        &[(
            "uso",
            "Uso del cuerpo receptor",
            "none",
            "enum:abastecimiento|riego|recreacion|vertido",
            false,
            "gen",
        )],
    ),
];

/// Subtipos nuevos en familias existentes (§4.1).
/// (familia, subtipos) y cada subtipo es (clave, etiqueta, atributos obligatorios).
type Subtipos = (
    &'static str,
    &'static [(&'static str, &'static str, &'static [&'static str])],
);

const SUBTIPOS: &[Subtipos] = &[
    (
        "agua.captacion",
        &[
            (
                "toma_directa",
                "Toma directa",
                &[
                    "cota_umbral",
                    "ancho_ventana",
                    "altura_ventana",
                    "ancho_reja",
                    "abertura_reja",
                ],
            ),
            (
                "toma_flotante",
                "Toma flotante",
                &["diametro_succion", "longitud_flexible", "volumen_flotador"],
            ),
            (
                "pozo_radial",
                "Pozo radial",
                &["longitud_drenes", "numero_drenes", "diametro_dren"],
            ),
        ],
    ),
    (
        "agua.tuberia",
        &[
            (
                "rebose",
                "Rebose",
                &["diametro_nominal", "cota_cresta", "longitud_vertedero"],
            ),
            ("desague", "Desagüe", &["diametro_nominal", "tipo_valvula"]),
            (
                "conduccion_forzada",
                "Conducción forzada",
                &["clase_presion", "cota_clave", "cota_clave_fin"],
            ),
            ("limpia", "Limpia", &["diametro_nominal", "pendiente"]),
        ],
    ),
    (
        "agua.tanque",
        &[
            (
                "succion",
                "Tanque de succión",
                &["volumen_util", "diametro_succion", "cota_nivel_min"],
            ),
            (
                "elevado_domiciliario",
                "Tanque elevado domiciliario",
                &["volumen_util", "diametro", "altura"],
            ),
        ],
    ),
    (
        "agua.valvula",
        &[
            (
                "guillotina",
                "Guillotina",
                &["diametro_nominal", "tipo_accionamiento"],
            ),
            ("pinch", "Pinch", &["diametro_nominal", "material_cuerpo"]),
        ],
    ),
    (
        "agua.arqueta",
        &[(
            "muestreo",
            "Arqueta de muestreo",
            &["diametro", "altura", "llave_muestreo"],
        )],
    ),
    (
        "agua.planta",
        &[
            (
                "ptap_remocion_hierro_manganeso",
                "PTAP de remoción de hierro y manganeso",
                &["numero_unidades", "largo", "ancho"],
            ),
            (
                "ptap_ablandamiento",
                "PTAP de ablandamiento",
                &["numero_unidades", "diametro_tanque"],
            ),
            (
                "ptar_primario",
                "PTAR primaria",
                &["numero_unidades", "largo", "ancho"],
            ),
        ],
    ),
    (
        "comun.cruce",
        &[(
            "hinca",
            "Hinca / perforación dirigida",
            &["longitud_cruce", "diametro_camisa", "profundidad_minima"],
        )],
    ),
    (
        "saneamiento.canal",
        &[(
            "bypass",
            "By-pass de planta",
            &["base", "pendiente", "longitud"],
        )],
    ),
];

/// Atributos que piden los subtipos nuevos y que hay que declarar si la familia no los tenía.
const ATTRS_DE_SUBTIPO: &[(&str, &[AttrSpec])] = &[
    (
        "agua.captacion",
        &[
            (
                "ancho_ventana",
                "Ancho de ventana de toma",
                "m",
                "num",
                false,
                "geo",
            ),
            (
                "altura_ventana",
                "Altura de ventana de toma",
                "m",
                "num",
                false,
                "geo",
            ),
            ("ancho_reja", "Ancho de reja", "m", "num", false, "geo"),
            (
                "abertura_reja",
                "Abertura de reja",
                "mm",
                "num",
                false,
                "hid",
            ),
            (
                "longitud_flexible",
                "Longitud de manguera flexible",
                "m",
                "num",
                false,
                "geo",
            ),
            (
                "volumen_flotador",
                "Volumen del flotador",
                "m3",
                "num",
                false,
                "geo",
            ),
            (
                "longitud_drenes",
                "Longitud de drenes",
                "m",
                "num",
                false,
                "geo",
            ),
            (
                "numero_drenes",
                "Número de drenes",
                "none",
                "int",
                false,
                "gen",
            ),
            (
                "diametro_dren",
                "Diámetro de dren",
                "mm",
                "num",
                false,
                "geo",
            ),
        ],
    ),
    (
        "agua.tuberia",
        &[
            (
                "tipo_valvula",
                "Tipo de válvula",
                "none",
                "text",
                false,
                "gen",
            ),
            ("pendiente", "Pendiente", "mperm", "num", false, "geo"),
            (
                "cota_cresta",
                "Cota de cresta del rebose",
                "m",
                "num",
                false,
                "geo",
            ),
            (
                "longitud_vertedero",
                "Longitud de vertedero",
                "m",
                "num",
                false,
                "geo",
            ),
        ],
    ),
    (
        "agua.valvula",
        &[(
            "material_cuerpo",
            "Material del cuerpo",
            "none",
            "text",
            false,
            "mat",
        )],
    ),
    (
        "agua.tanque",
        &[
            (
                "diametro_succion",
                "Diámetro de succión",
                "mm",
                "num",
                false,
                "hid",
            ),
            (
                "cota_nivel_min",
                "Cota de nivel mínimo",
                "m",
                "num",
                false,
                "hid",
            ),
            ("diametro", "Diámetro", "m", "num", false, "geo"),
            ("altura", "Altura", "m", "num", false, "geo"),
        ],
    ),
    (
        "agua.arqueta",
        &[
            ("diametro", "Diámetro", "m", "num", false, "geo"),
            ("altura", "Altura", "m", "num", false, "geo"),
        ],
    ),
    (
        "agua.arqueta",
        &[(
            "llave_muestreo",
            "Llave de muestreo",
            "none",
            "bool",
            false,
            "gen",
        )],
    ),
    (
        "agua.planta",
        &[
            (
                "numero_unidades",
                "Número de unidades",
                "none",
                "int",
                false,
                "gen",
            ),
            (
                "diametro_tanque",
                "Diámetro de tanque",
                "m",
                "num",
                false,
                "geo",
            ),
            ("largo", "Largo", "m", "num", false, "geo"),
            ("ancho", "Ancho", "m", "num", false, "geo"),
        ],
    ),
    (
        "comun.cruce",
        &[
            (
                "longitud_cruce",
                "Longitud de cruce",
                "m",
                "num",
                false,
                "geo",
            ),
            (
                "diametro_camisa",
                "Diámetro de camisa",
                "mm",
                "num",
                false,
                "geo",
            ),
            (
                "profundidad_minima",
                "Profundidad mínima",
                "m",
                "num",
                false,
                "geo",
            ),
        ],
    ),
    (
        "saneamiento.canal",
        &[
            ("base", "Base del canal", "m", "num", false, "geo"),
            ("longitud", "Longitud", "m", "num", false, "geo"),
        ],
    ),
];

/// Treatment units baked by a Blender recipe (F17-07): their wall thickness is a design datum
/// the recipe needs (D17), and the base registry did not declare it.
const PLANT_UNITS_WITH_WALLS: &[&str] = &[
    "agua.mezcla",
    "agua.floculador",
    "agua.sedimentador",
    "agua.filtro",
    "agua.tanque",
    "comun.reja",
    "comun.desarenador",
    "comun.lecho_secado",
    "saneamiento.reactor",
    "saneamiento.sedimentador",
];

const WALL_THICKNESS: AttrSpec = (
    "espesor_pared",
    "Espesor de pared",
    "m",
    "num",
    false,
    "geo",
);

/// Aplica los huecos sobre el catálogo recién construido.
pub fn aplicar(catalogo: &mut Vec<FamilyDef>) {
    let walls: Vec<(&str, &[AttrSpec])> = PLANT_UNITS_WITH_WALLS
        .iter()
        .map(|f| (*f, std::slice::from_ref(&WALL_THICKNESS)))
        .collect();
    for f in catalogo.iter_mut() {
        // Atributos nuevos (y los que piden los subtipos nuevos).
        for lista in [ATTRS, ATTRS_DE_SUBTIPO, walls.as_slice()] {
            for (familia, specs) in lista {
                if *familia != f.key {
                    continue;
                }
                for (k, label, u, t, req, g) in *specs {
                    if f.attrs.iter().any(|a| a.key == *k) {
                        continue;
                    }
                    let mut a = attr(k, label, unidad(u), tipo(t), *req, grupo(g));
                    if let Some(opciones) = t.strip_prefix("enum:") {
                        let opts: Vec<&'static str> = opciones.split('|').collect();
                        a.ty = AttrType::Enum(Box::leak(opts.into_boxed_slice()));
                    }
                    f.attrs.push(a);
                }
            }
        }
        // Subtipos nuevos.
        for (familia, subs) in SUBTIPOS {
            if *familia != f.key {
                continue;
            }
            for (k, label, req) in *subs {
                if f.types.iter().any(|s| s.key == *k) {
                    continue;
                }
                f.types.push(subtype(k, label, req));
            }
        }
    }
    // Los retiros de `agua.planta` dejan la familia con menos subtipos; el mínimo del registro es 4.
    debug_assert!(
        catalogo
            .iter()
            .all(|f| f.types.len() >= 4 || f.key == "agua.planta")
    );
    let _ = (Category::Agua, TopoRole::Node);
}
