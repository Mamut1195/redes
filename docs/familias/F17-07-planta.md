# F17-07 — Unidades de planta (PTAP y PTAR) horneadas en Blender

Ticket MAM-1147. Hecho el 28/11/2026 con Blender 5.1.0. Sigue el camino de F17-03 (docs/17 §12):
una receta `bpy` por unidad, horneada con `redes biblioteca hornear`, caché por clave, `.blend` de
revisión siempre (D17) y la pieza en lugar de la de `parts_cadena.rs` en el visor, con y sin
`--gpu3d`. Nada de esto calcula proceso ni hidráulica (D4): la geometría sale solo de medidas
declaradas; lo que no está declarado y la receta necesita para dibujar es **forma** (fija en el
contrato, entra en la clave, nunca es `Dato` ni alimenta cantidades del proyecto).

## 1. Unidades cubiertas

Las unidades que contiene `redes proyecto cadena`, por orden de prioridad (las de proceso primero):

| Planta | Objeto | Familia / subtipo | Receta | Estado |
|---|---|---|---|---|
| PTAP | MZ-001 | `agua.mezcla` / `resalto_hidraulico` | `build_plant_mixer` | horneada |
| PTAP | FL-001 | `agua.floculador` / `pantallas_horizontales` | `build_plant_flocculator` | horneada |
| PTAP | SD-001 | `agua.sedimentador` / `convencional` | `build_plant_sedimentator` | horneada |
| PTAP | FI-001 | `agua.filtro` / `rapido_gravedad` | `build_plant_filter` | horneada |
| PTAP | TQ-001 | `agua.tanque` / `contacto` | `build_plant_contact_tank` | horneada |
| PTAP | LS-001 | `comun.lecho_secado` / `arena_grava` | `build_plant_drying_bed` | horneada |
| PTAR | RJ-002 | `comun.reja` / `gruesos_mecanica` | `build_plant_screen` | horneada |
| PTAR | DZ-002 | `comun.desarenador` / `dos_naves` | `build_plant_grit_chamber` | horneada |
| PTAR | RX-001 | `saneamiento.reactor` / `uasb` | `build_plant_uasb` | horneada |
| PTAR | SE-001 | `saneamiento.sedimentador` / `secundario` | `build_plant_clarifier` | horneada |
| PTAP | AF-001 | `comun.aforador` / `parshall` | — | respaldo: la Parshall pide las cotas de la tabla normalizada, que el registro no declara |
| PTAP | DI-001 | `agua.desinfeccion` / `hipoclorito_sodio` | — | respaldo: es un equipo (gabinete, tanque de solución); su tamaño saldría de la capacidad, y eso es cálculo |
| PTAP | CS-001, CS-002 | `comun.caseta` | — | respaldo (edificio, no unidad de proceso) |
| — | TQ-002 | `agua.tanque` / `apoyado_cilindrico` | — | respaldo: pendiente (reservorio; el registro pide `diametro` en mm y el demo trae `diametro_interior`) |
| PTAR | LG-001 | `saneamiento.laguna` / `maduracion` | — | respaldo: pendiente (terraplén, no estructura de hormigón) |

Las recetas aplican por familia **y subtipo**: la de reja (`gruesos_mecanica`) y la de desarenador
(`dos_naves`) no tocan las de la captación (RJ-001 `gruesos_manual`, DZ-001 `longitudinal`, F17-06).

## 2. Archivos

- `tools/blender/plant_units.py`: piezas comunes (cajas, prismas en planta, en alzado y en sección,
  sólidos de revolución, tolva con cavidad tronco-piramidal, losa con hueco, muros sin solapes,
  canaletas en U, anclas en la cara exterior, materiales con color). Cada receta lo declara en su
  contrato con `"modulos": ["plant_units.py"]`, y `bake.rs` lo mete en la clave: tocar el módulo
  invalida la caché de todas las unidades. Una receta sin `modulos` (el pozo) conserva su clave.
- `tools/blender/build_plant_<unidad>.py` y su `.receta.json`: una receta por unidad.
- `tools/blender/piloto/plant/`: el juego de datos de cada unidad del demo (`<receta>.json`) y su
  `<clave>.glb` (≈ 6–80 kB cada uno).

Marco común (igual que el pozo): Z arriba, metros, origen en el centro en planta de la **cara
inferior de la losa de fondo**. El fondo interior (la `cota_fondo` declarada) queda en
`z = floor_thickness_m` y el *empty* `solera` lo marca; el visor apoya la pieza con
`cota_base_modelo` (solera − altura del ancla), con la resta del origen en `f64` (docs/15) y la
corrección de ejes de F17-03. **Largo, ancho y diámetro son interiores**: los muros van por fuera.
El largo va en +X (el sentido del flujo del demo).

## 3. Qué es dato y qué es forma (para aprobar)

Dato = atributo del registro (o un `Dato` con la clave y la unidad del registro); si falta, no se
hornea: `W_PARAMETRO_FALTANTE` y se dibuja la pieza de respaldo.

| Unidad | Dato (del proyecto) | Forma (fija en la receta) |
|---|---|---|
| Común a todas | `espesor_pared` (m, nuevo en el registro de estas familias), `material` del cuerpo, `conexiones` (anclas) | losa de fondo 0,25 m (0,20 en el lecho, 0,30 en UASB y secundario) |
| Mezcla por resalto | `largo`, `ancho`, `altura_util`, `altura_caida`, `longitud_resalto` | la cara de la caída inclinada con 0,40 m de desarrollo |
| Floculador de pantallas horizontales | `largo`, `ancho`, `altura_util`, `numero_camaras`, `numero_pantallas` (total, repartido entre cámaras desde la primera), `separacion_pantallas` | pantallas de 0,06 m de polietileno, paso de 0,60 m en el extremo, alterno; los muros entre cámaras (de hormigón, del `espesor_pared`) también dejan el paso alterno |
| Sedimentador convencional | `largo`, `ancho`, `altura_util`, `tirante` (= cresta de las canaletas), `diametro_purga` | tolva de lodos de 2,0 m × ancho, 1,0 m de hondo y fondo de 0,6 m en la entrada; pantalla difusora de tablillas (0,25 m con ranuras de 0,10 m, a 1,0 m de la entrada, abierta 0,40 m abajo, polietileno); 2 canaletas transversales de 0,40 × 0,40 m cada 1,2 m en la salida (fibra de vidrio); purga de PVC desde el fondo de la tolva |
| Filtro rápido | `largo`, `ancho`, `altura_total`, `altura_lecho`, `numero_celdas`, `tipo_lecho` (material del lecho) | cámara bajo el falso fondo de 0,40 m; falso fondo de 0,10 m (polietileno); capa de grava de 0,30 m; 2 canaletas de lavado por celda de 0,35 × 0,35 m con la cresta 0,60 m sobre el lecho |
| Tanque de contacto | `largo`, `ancho`, `altura_util`, `numero_bafles` | bafles longitudinales de hormigón de 0,15 m con paso de 0,60 m alterno (serpentín) |
| Lecho de secado | `largo`, `ancho`, `altura_muro`, `numero_celdas`, `espesor_arena`, `espesor_grava`, `diametro_dren` | un dren por celda a lo ancho; fondo plano (la `pendiente_fondo` declarada no se dibuja) |
| Reja mecánica | `ancho`, `altura`, `inclinacion`, `abertura`, `espesor_barra` | canal de 3,0 m abierto en los extremos y de hormigón (la familia no tiene `material`); barras de acero de 0,05 m de fondo; carcasa del rastrillo 0,6 × 0,5 m. El número de barras sale solo de la disposición: las que caben con la abertura declarada entre ellas, centradas |
| Desarenador de dos naves | `largo`, `ancho`, `altura_util`, `tirante` (= cresta del vertedero de salida), `numero_naves`, `numero_compuertas` (igual en cada nave), `diametro_purga` | banquetas que bajan 0,30 m a una canaleta de 0,40 m por nave; vertedero de 0,15 m a 0,60 m de la salida; compuertas de acero dibujadas **levantadas** (la placa ocupa la altura seca) con vástago de 0,8 m; purga de PVC en la entrada |
| UASB | `largo`, `ancho`, `altura_util`, `tirante` (= cresta de las canaletas), `numero_camaras`, `numero_difusores` (puntos de entrada, repartidos entre cámaras) | 2 separadores trifásicos por cámara (V invertida de 1,6 × 1,0 m, chapa de 0,08 m, 0,30 m bajo el agua, fibra de vidrio); canaletas de acero inoxidable entre separadores; bajantes de entrada DN 100 de PVC junto al muro aguas arriba, a 0,20 m del fondo |
| Sedimentador secundario | `diametro_interior`, `altura_util`, `tirante` (= cresta de la canaleta perimetral), `diametro_purga` | el fondo sube 0,50 m en cono desde un pozo central de 1,2 m; campana de alimentación de 2,0 m y 1,2 m de alto; columna central de 0,5 m; canaleta perimetral de 0,40 × 0,40 m; puente radial de 0,9 × 0,3 m (acero) |

Además, tres constantes de dibujo en los guiones (entran en la clave con ellos): las purgas
sobresalen 0,5 m del muro, las bajantes del UASB 0,3 m sobre él, y el tanque de contacto no admite
canales de menos de 0,1 m entre bafles.

No se dibujan (forma que se podría añadir si se aprueba): barandas, pasarelas, escaleras de
acceso, agua. Las barandas y pasarelas no son dato del registro y cargarían triángulos sin ayudar a
revisar; el agua taparía el interior en el pase 3D.

## 4. Datos que se añadieron al demo

`redes proyecto cadena` declara, con la fuente «demo: dato de ejemplo, no de un diseño», lo que
las recetas piden y el demo no traía (`declare_demo_plant_units` en `demo_cadena.rs`):

- Todas: `espesor_pared` (`Dato` en m: 0,20 mezcla, contacto, lecho y reja; 0,25 floculador y
  desarenador; 0,30 sedimentadores, filtro y UASB) y `material` = `hormigon`.
- La **solera** de cada unidad es su `cota_fondo` declarada (la reja, su `cota_umbral`; el tanque de
  contacto, su `cota_piso`), como la lee la ingesta del plan (`cota_fondo` es sinónimo de `cota`).
  Antes el demo la ponía 1,8 m bajo la tapa sin relación con la `cota_fondo`.
- FI-001: `tipo_lecho` = `arena`.
- TQ-001: el demo lo dibujaba como un círculo de 4 m con `diametro_interior`, que `agua.tanque` no
  tiene; ahora trae lo que pide su subtipo: `largo` 6, `ancho` 3, `altura_util` 2,8 m,
  `numero_bafles` 4 y `cota_piso` 2513,0 (≈ los 50 m³ declarados; el volumen no se usa).
- LS-001: `diametro_dren` 100 mm. RJ-002: `espesor_barra` 10 mm. SE-001: `altura_util` 3,0 m.

Registro: `espesor_pared` (m, opcional, geométrico) se añade en `families_extra.rs` a las diez
familias horneadas.

## 5. Anclas

Una por tramo que llega a la unidad (`entrada__<código>`, `salida__<código>`), en la cara exterior
del muro donde sale el rayo desde el centro con el acimut del tramo, a la solera del tramo en ese
extremo (`bake::node_connections`, igual que el pozo). La reja, que es un canal abierto, las pone
en las bocas del canal. El lecho de secado no tiene tramos: `W_SIN_ANCLA` «sin tramos
conectados». `hornear` no mueve los tramos: en el demo siguen llegando a los puntos que declara su
trazado (a veces el centro de la unidad). `redes biblioteca aplicar`, que lleva los extremos a las
anclas, solo mueve los que están a menos de 5 m del nodo: en las unidades largas (sedimentador,
UASB) el extremo queda más lejos y no se movería.

## 6. Visor

Sin cambios en la colocación: cualquier objeto con `modelo` dibuja su pieza en lugar de la
paramétrica (F17-03). Nuevas escenas para revisar (docs/04 §5.4):

- `--scene unidad`: primer plano de la unidad sin terreno ni tinte de selección. Elige el objeto de
  `$REDES_OBJETO` o, sin él, el primero con modelo que no es un pozo.
- `--scene unidad-corte`: la misma unidad cortada por el plano vertical que pasa por su centro
  (solo el pintor; el pase 3D no corta).

## 7. Pruebas

- `crates/redes-core/tests/planta_horneada.rs` (sin Blender): por cada receta, el piloto se llama
  como la clave de hoy (receta + `plant_units.py` + driver), las partes esperadas, malla cerrada con
  volumen positivo por material y al menos dos materiales, el ancla `solera` sobre la losa, un ancla
  por tubo en la cara exterior a su cota, capas del filtro con sus materiales y muros a la altura
  total; las recetas cuadran con las unidades del registro; cada unidad del demo tiene sus datos y
  su solera en el fondo declarado; sin `espesor_pared` ni `tirante` no se hornea y se dice qué falta.
- `bake.rs`: un módulo importado entra en la clave y una receta sin módulos conserva la suya.
- `crates/redes-cli/tests/hornear_planta.rs` (con Blender): rehornea los diez pilotos y exige los
  mismos bytes; sin Blender se salta con aviso.

Rehornear un piloto (si se toca una receta o `plant_units.py`, la prueba lo pide):

```powershell
redes biblioteca hornear --receta tools\blender\build_plant_filter.receta.json `
  --parametros tools\blender\piloto\plant\build_plant_filter.json --carpeta $env:TEMP\p --sin-blend
# y copiar $env:TEMP\p\biblioteca\horneado\<clave>.glb a tools\blender\piloto\plant\
```

## 8. Capturas y medido

Capturas para aprobar (`out/f17-07/`, carpeta ignorada; se rehacen con estos pasos):

```powershell
redes proyecto cadena --salida out\f17-07\cadena\cadena.redes
redes -p out\f17-07\cadena\cadena.redes biblioteca hornear --receta tools\blender\build_manhole.receta.json
Get-ChildItem tools\blender\build_plant_*.receta.json | ForEach-Object {
  redes -p out\f17-07\cadena\cadena.redes biblioteca hornear --receta $_.FullName }
$env:REDES_PROJECT = "out\f17-07\cadena\cadena.redes"
redes-app --snap out\f17-07\ptap.png --scene ptap      # y --gpu3d; igual con ptar
$env:REDES_OBJETO = "SD-001"
redes-app --snap out\f17-07\unidades\SD-001.png --scene unidad          # y --gpu3d, por unidad
redes-app --snap out\f17-07\corte-SD-001.png --scene unidad-corte       # y FI-001
```

- `ptap.png`, `ptar.png` y sus `-gpu3d.png`; `unidades/<código>.png` y `<código>-gpu3d.png` de las
  diez unidades; `corte-SD-001.png` (tolva, pantalla difusora y canaletas) y `corte-FI-001.png`
  (falso fondo, grava, arena y canaletas).
- `.blend` de revisión de las diez piezas distintas: `out/f17-07/revision/<código>-<receta>.blend`.
- En el pintor (sin `--gpu3d`) las caras grandes de las unidades (losas, capas) a veces se ordenan
  mal por profundidad (el pintor ordena triángulos enteros); en `--gpu3d` se ven bien. Es el límite
  conocido del pintor, no de las piezas; partir las caras grandes es una opción si molesta.

Medido (Windows 11, Blender 5.1.0, CLI en release): cada unidad se hornea en 2,7–6,5 s (un proceso
de Blender por receta, la mayor parte es el arranque). Piezas:

| Receta | Triángulos | `.glb` | Caja (m) |
|---|---|---|---|
| mixer | 72 | 6 kB | 3,4 × 2,8 × 1,85 |
| flocculator | 156 | 11 kB | 9,5 × 5,5 × 3,05 |
| sedimentator | 460 | 28 kB | 12,1 × 6,6 × 4,25 (con la tolva) |
| filter | 408 | 27 kB | 8,6 × 6,6 × 3,45 |
| contact_tank | 108 | 8 kB | 6,4 × 3,4 × 3,05 |
| drying_bed | 348 | 20 kB | 12,4 × 8,4 × 1,2 |
| screen | 588 | 35 kB | 3,0 × 2,0 × 1,95 |
| grit_chamber | 408 | 23 kB | 11,0 × 5,5 × 2,85 |
| uasb | 624 | 33 kB | 12,6 × 8,6 × 5,1 |
| clarifier | 1 740 | 79 kB | 10,1 × 9,6 × 3,6 |

## 9. Para el usuario

1. Aprobar la partición dato/forma de §3, sobre todo: losa de fondo como forma (igual que el pozo),
   tolva y pantalla difusora del sedimentador, capas del filtro bajo el lecho (falso fondo y grava
   como forma, porque el registro no las declara), compuertas del desarenador dibujadas levantadas,
   separadores del UASB, y que `numero_pantallas` del floculador sea el total de la unidad.
2. Aprobar los datos de demo de §4 (sobre todo TQ-001, que pasa de círculo a tanque con bafles, y la
   solera de las unidades en su `cota_fondo`).
3. Decidir si se añaden barandas, pasarelas o agua (forma) y si se hacen las unidades pendientes
   (Parshall, reservorio, laguna, casetas).
4. Si conviene una orden que hornee todas las recetas de un proyecto de una vez (hoy, una llamada por
   receta).
