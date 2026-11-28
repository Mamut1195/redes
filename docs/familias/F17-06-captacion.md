# F17-06 — Captación horneada: bocatoma, desarenador y cámaras de la conducción

Hecho el 28/11/2026 con Blender 5.1.0 (MAM-1146). Cuatro recetas nuevas en `tools/blender/`, cada
una con su contrato `redes.receta/1`, horneadas con `redes biblioteca hornear` (D15, D16) y
colocadas en lugar de la pieza de respaldo de `parts_cadena.rs`, con y sin `--gpu3d`.

| Receta | Familia y subtipo | Objeto del demo `cadena` | Partes |
|---|---|---|---|
| `build_intake_weir` | `agua.bocatoma` · `barraje_fijo` | BC-001 | `azud` (barraje y colchón), `muros` (encauzamiento con la ventana), `toma` (caja tras la ventana) |
| `build_intake_desander` | `comun.desarenador` · `longitudinal` | DZ-001 | `cuerpo` (losa y muros), `purga`, `agua` |
| `build_intake_load_chamber` | `agua.camara` · `carga` | CA-001 | `cuerpo` (losa, muros y techo con boca), `vertedero`, `tapa`, `escalones`, `agua` |
| `build_intake_break_chamber` | `agua.camara` · `quiebre` | CA-002 | `cuerpo`, `tapa`, `escalones`, `agua` |

Fuera de este ticket: los otros cinco subtipos de bocatoma, los otros cuatro de desarenador
(`dos_naves`, DZ-002 de la PTAR, puede ir con F17-07) y los otros seis de cámara; la reja RJ-001 es
su propio objeto (`comun.reja`) y sigue con su pieza de respaldo.

## 1. Lo nuevo en el núcleo

- **`"desde": "menos_solera"`** (`bake::ParamSource::AttributeMinusInvert`): una cota del objeto
  (`cota_cresta`, `cota_umbral`, `cota_fondo`, `cota_agua`, `cota_tapa`) menos su cota de solera,
  las dos del origen y redondeada al micrómetro. Las obras dibujan sus niveles en el marco que el
  visor ya apoya en la solera (ancla `solera`, docs/17 §12.4), y la misma obra a otra cota es la
  misma pieza (misma clave). Si falta la cota o la solera, no se hornea (`W_PARAMETRO_FALTANTE`).
- Registro: `comun.desarenador` gana el atributo opcional `espesor_muro` (m), como ya tenían la
  bocatoma y la cámara.
- App: `--objeto <código>` elige el objeto de las escenas `foco`, `corte`, `explosion` y
  `pozo-modelo`; escena nueva `captacion` (docs/04 §5.4).

## 2. Qué es dato y qué es forma (para aprobar)

Dato = viene del diseño con su fuente; si falta, la obra no se hornea y se ve la de respaldo.
Forma = detalle de dibujo fijado en la receta; entra en la clave y en `horneado.*`, nunca es `Dato`
ni alimenta cantidades.

### Bocatoma de barraje fijo

| Parámetro | Desde | Demo BC-001 |
|---|---|---|
| `weir_length_m` | `largo_barraje` | 8,0 |
| `crest_rise_m`, `sill_rise_m`, `bed_rise_m` | `cota_cresta`, `cota_umbral`, `cota_fondo` menos solera | 1,8 · 1,0 · 0,0 |
| `window_width_m` | `ancho_ventana` | 1,2 |
| `window_height_m` | `altura_ventana` | **0,5 (dato de demo nuevo)** |
| `wall_thickness_m` | `espesor_muro` | **0,3 (dato de demo nuevo)** |
| `body_material` | `material` | **hormigon (nuevo)** |
| `connections` | tramos | salida TB-001 |
| forma | ancho de cresta 0,6 m · talud aguas abajo 0,75 H:1 V · cimiento 0,6 m bajo el cauce · colchón 3,0 × 0,4 m · muros 0,8 m sobre la cresta y 1,5 m aguas arriba de la ventana · ventana a 1,0 m del paramento · caja de toma de 1,5 m | |

- La ventana se abre en el muro de la orilla por la que sale el primer tubo de salida (−Y si no sale
  ninguno), aguas arriba del barraje; detrás, una caja abierta con piso en el umbral de la que sale
  el ancla `salida__<tramo>`.
- **Forma que parece hidráulica** (perfil del barraje, colchón, resguardo de los muros): no se
  calcula, es un dibujo fijo. Si el diseño la trae, propuesta: atributos `ancho_cresta`,
  `longitud_colchon`, `cota_coronacion_muros` y pasan a dato.
- `altura_barraje` **no** entra: la altura la dan las cotas. En el demo no cuadran (1,4 m declarados
  frente a 2530,4 − 2528,6 = 1,8 m); no se corrige: es una discrepancia del demo a decidir.
- Sin canal de limpia: necesitaría `ancho_canal_limpia` y `longitud_canal_limpia` como dato.

### Desarenador longitudinal

| Parámetro | Desde | Demo DZ-001 |
|---|---|---|
| `length_m`, `width_m` | `largo`, `ancho` | 9,0 · 3,0 |
| `bottom_rise_m` | `cota_fondo` menos solera | 0,2 |
| `wall_height_m` | `altura_util` | 1,6 |
| `water_depth_m` | `tirante` | 1,0 |
| `purge_dn_mm` | `diametro_purga` | 200 |
| `wall_thickness_m` | `espesor_muro` (atributo nuevo) | **0,25 (dato de demo nuevo)** |
| `body_material` | `material` | **hormigon (nuevo)** |
| forma | losa de fondo 0,25 m · lámina de agua 0,02 m · purga de PVC que sobresale 0,6 m · 24 segmentos | |

- Fondo plano: la tolva, la transición de entrada (`longitud_transicion`) y el vertedero de salida
  (`cota_vertederos`) se dibujarían solo con su dato. La purga sale por el muro de aguas abajo, al
  nivel del fondo (ancla `purga`).

### Cámaras de carga y de quiebre

| Parámetro | Desde | Demo CA-001 / CA-002 |
|---|---|---|
| `length_m`, `width_m` | `largo`, `ancho` | 3,0 · 2,6 |
| `bottom_rise_m`, `top_rise_m`, `water_rise_m` | `cota_fondo`, `cota_tapa`, `cota_agua` menos solera | −0,2 · 1,8 · 1,4 / 0,0 · 1,8 · 1,4 |
| `crest_rise_m`, `weir_length_m` (solo carga) | `cota_cresta` menos solera, `longitud_vertedero` | 1,4 · 1,6 |
| `wall_thickness_m`, `slab_thickness_m` | `espesor_muro`, `espesor_losa` | **0,2 · 0,2 (datos de demo nuevos)** |
| `body_material` | `material` | **hormigon (nuevo)** |
| forma | boca de 0,60 m junto al muro de aguas arriba · tapa de fundición de 0,05 m · muro vertedero de 0,15 m en el centro · escalones como el pozo (F17-03) · lámina de agua 0,02 m | |

- El techo tiene su cara superior en `cota_tapa` y la losa de fondo bajo `cota_fondo`;
  `altura_util` no entra (lo dicen las cotas).
- Carga: el muro vertedero cruza la cámara; la cresta ocupa `longitud_vertedero` centrada y a los
  lados el muro sube hasta el techo.
- Quiebre: la caída es la del tubo de llegada sobre el agua, con los datos del tramo;
  `altura_caida` no entra y no hay pantalla disipadora (necesitaría su dato). En el demo el tubo
  TB-005 llega a 2519,0 = `cota_agua`, frente a `altura_caida` = 1,2: otra discrepancia del demo.

Todas: anclas `entrada__/salida__<tramo>` en la cara exterior del muro que cruza cada tubo, a la
solera del tramo (como el pozo, docs/17 §12.3), y ancla `solera` en la solera del objeto.

## 3. Pruebas

- `crates/redes-core/tests/captacion_horneada.rs` (sin Blender): cada piloto versionado en
  `tools/blender/piloto/intake/` se llama con la clave de la receta actual; partes, una malla
  cerrada con volumen positivo por material, base en z = 0 y cara superior en su cota, ancla
  `solera`, un ancla por tubo en su muro y a su cota, la salida de la bocatoma en la caja de toma;
  unidades de las recetas contra el registro; las cuatro obras del demo tienen los datos para
  hornear (falla sin los datos de demo nuevos), y una cota sin solera falta, no se rellena.
- `crates/redes-cli/tests/hornear_captacion.rs` (con Blender, se salta sin él): rehornea los
  cuatro pilotos byte a byte y hornea las cuatro obras del demo con su `.blend` de revisión.

Rehornear un piloto si se toca su receta:

```powershell
redes biblioteca hornear --receta tools\blender\build_intake_weir.receta.json `
  --parametros tools\blender\piloto\intake\weir.json --carpeta <tmp> --sin-blend
# y copiar <tmp>\biblioteca\horneado\<clave>.glb a tools\blender\piloto\intake\
```

## 4. Para aprobar

```powershell
redes proyecto cadena --salida out\f17-06\cadena\cadena.redes
foreach ($r in "weir","desander","load_chamber","break_chamber") {
  redes -p out\f17-06\cadena\cadena.redes biblioteca hornear --receta tools\blender\build_intake_$r.receta.json }
$env:REDES_PROJECT = "out\f17-06\cadena\cadena.redes"
redes-app --snap out\f17-06\BC-001-cpu.png --scene pozo-modelo --objeto BC-001   # y --gpu3d; DZ-001, CA-001, CA-002
redes-app --snap out\f17-06\BC-001-corte.png --scene corte --objeto BC-001       # y DZ-001
redes-app --snap out\f17-06\captacion-iso.png --scene captacion                  # y --gpu3d
```

- Capturas en `out/f17-06/`; `.blend` de revisión de las cuatro piezas en `out/f17-06/revision/`.
- El corte solo existe en el pintor y solo en un plano X: en la bocatoma corta el barraje a lo
  largo (se ve la sección de muros y cresta), no el perfil del azud.
- Medido: 4 piezas distintas, 136 (bocatoma), 164 (desarenador), 312 (carga) y 276 (quiebre)
  triángulos, 11–22 kB cada `.glb`; 3,3–5,1 s de Blender por receta con `.blend` (un proceso por
  receta, con otros cuatro agentes compilando a la vez: cifra orientativa).
- Mutación nueva en `scripts/mutar.ps1`: `cota-sin-solera` (hornear una obra sin cota de solera
  como si estuviera a cota 0), detectada por `captacion_horneada`.
