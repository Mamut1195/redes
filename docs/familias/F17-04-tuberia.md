# F17-04: tubería con campana y espiga, y juntas

Hecho el 28/11/2026 con Blender 5.1.0 (Linear MAM-1144). La receta `tools/blender/build_pipe.py`
(contrato `build_pipe.receta.json`) hornea **un tramo comercial** de tubo con su espiga, su campana y
su junta; el visor lo repite a lo largo de cada tramo de la red, con y sin `--gpu3d`. Incluye los dos
ajustes aprobados en F17-03: el dibujo del tubo se recorta en el ancla del pozo (su pared) y el cono
excéntrico del pozo abre hacia la salida.

## 1. Qué es dato y qué es forma (para aprobar)

Lo que traen hoy los tramos (`families.rs`, `agua.tuberia` y `saneamiento.tuberia`): `diametro_nominal`
(DN, mm), `diametro_interior` (mm), `espesor_pared` (m), `material`, `sdr`, `clase_tuberia`. Ninguno
trae largo comercial ni tipo de junta. Partición propuesta:

| Parámetro de la receta | Desde | Valor |
|---|---|---|
| `dn_mm` | `dn` (DN del tramo, si es dato) | dato |
| `inner_diameter_m` | `diametro_interior_o_dn`: el diámetro interior declarado o, sin él, el DN | dato (la misma regla de dibujo que `Link::diametro_interior_m`, F12-02) |
| `wall_thickness_m` | atributo `espesor_pared` (o un `Dato` en m, con su fuente) | dato |
| `pipe_material` | atributo `material` | dato |
| `system` | `sistema`: `agua` o `saneamiento`, de la familia | dato (decide el color de los plásticos) |
| `segment_length_m` | forma, tabla por material | PVC 6 m, PEAD 12, hierro dúctil/fundición 6, acero 12, asbesto-cemento 5, hormigón 2,5, gres 2, `otro` 6 |
| `joint_type` | forma, tabla por material | `campana_espiga` (anillo de caucho); PEAD `termofusion` (cordón de soldadura, sin campana) |
| `socket_depth_ratio`, `socket_depth_min_m` | forma | campana de 0,5 · DE de fondo, al menos 8 cm |
| `socket_clearance_m` | forma | holgura de 2 mm entre espiga y campana |
| `bell_wall_ratio`, `bell_taper_ratio` | forma | pared de la campana = pared del tubo; transición de 3 veces el resalte |
| `spigot_chamfer_ratio` | forma | bisel de la espiga de 1,5 · pared, a media pared |
| `gasket_span`, `gasket_material` | forma | anillo entre el 55 % y el 80 % del fondo de la campana, en una ranura de media pared; caucho |
| `bead_width_ratio`, `bead_height_ratio` | forma | cordón de termofusión de 1,5 · pared de ancho y 0,6 · pared de alto |
| `segments` | forma | 24 |

- **Nada se inventa (D4, D16)**: un tramo sin DN (y sin diámetro interior) o sin `espesor_pared` no se
  hornea, `hornear` lo lista en `sin_hornear` con `W_PARAMETRO_FALTANTE` y el visor sigue con el tubo
  paramétrico. El `sdr` no se usa para derivar el espesor: sería calcular.
- Un material que no está en la tabla usa la fila `otro` de la **forma** (6 m, campana y espiga). Es
  un detalle de dibujo, no un dato; para aprobar si se prefiere no hornearlo.
- El id de las piezas lleva el prefijo de la familia principal de la receta
  (`saneamiento.tuberia-<clave>`) también en los tubos de agua; la pieza no depende de la familia
  sino de sus parámetros (`system` incluido).
- Demo (`redes proyecto cadena`): los 36 tramos declaran `espesor_pared` como `Dato` con la fuente
  «demo: dato de ejemplo, no de un diseño» (PVC ≈ DN/41; hormigón ≈ DN/12 + 2 cm). El paramétrico
  sigue leyendo solo el atributo (`Link::espesor_pared_m`, 8 mm sin él).

## 2. La pieza

- Marco: X a lo largo del eje, Z arriba, metros. El origen es la punta de la espiga sobre el eje; la
  siguiente espiga queda en `x = largo de tendido`, dentro de la campana de esta. Los *empties*
  `entrada` (x = 0) y `salida` (x = largo) dan el largo de tendido.
- Partes, cada una un sólido de revolución cerrado con vértices y caras explícitos:
  - `cuerpo`: fuste con el bisel de la espiga, hasta donde empieza la campana; es lo único que el visor
    estira (es prismático a lo largo del eje, así que estirarlo es exacto);
  - `campana`: transición, embocadura y ranura del anillo (o el cordón, en termofusión);
  - `junta`: anillo de caucho en la ranura, apretado contra la espiga siguiente (se ve en el corte).
- Materiales con color (`baseColorFactor`): PVC naranja en saneamiento y azul en agua (los colores que
  el visor ya daba a cada sistema), hormigón gris, caucho casi negro.
- Piloto versionado: `tools/blender/piloto/pipe/piloto.json` (DN 200 PVC de saneamiento, pared
  4,9 mm) y su `<clave>.glb` (960 triángulos, 48 kB). `crates/redes-core/tests/tubo_horneado.rs`
  comprueba sin Blender la clave, las tres partes, malla cerrada con volumen positivo por material,
  diámetro interior y pared del fuste, campana más ancha, anillo contra la espiga, anclas de tendido
  y que el tramo cortado acaba en el extremo del eje. Con Blender, `redes-cli/tests/hornear.rs`
  rehornea el piloto byte a byte y hornea la cadena entera.

## 3. Cómo se instancia (un tramo comercial, no un `.glb` por tramo de la red)

- La clave es (DN, diámetro interior, pared, material, sistema) más la forma: la cadena (31 tramos)
  da **pocas piezas** y una red de 20 000 tramos da tantas como combinaciones de DN, material y pared.
- `redes_core::pipe_layout`:
  - `drawn_axis`: el trazado subido medio diámetro interior y recortado en la pared de cada nodo: en el
    ancla `entrada__<código>`/`salida__<código>` del pozo horneado (su distancia en planta al centro) o,
    sin ella, en el borde de la pieza de respaldo (la regla de antes). El trazado y los datos no se tocan.
  - `lay_segments`: tramos enteros cada largo de tendido y el último cortado donde acaba el eje, dibujado
    con el cuerpo solo y estirado: el tubo entra al pozo con un **corte limpio**, nunca con una campana.
    Un tramo sobre un quiebre del trazado sigue su cuerda.
  - `plain_runs`: los tramos de una misma recta, como un único cuerpo estirado (de lejos).
- **Pintor**: cerca, los tramos enteros con campana; a media distancia, el cuerpo estirado tramo a
  tramo; de lejos, la línea de siempre. Se ilumina en el marco del mundo, con corte, tinte de selección
  y niebla como el paramétrico.
- **`--gpu3d`**: dos prototipos por pieza (entera y cuerpo); una instancia por tramo comercial con
  giro **y cabeceo** (la instancia pasa de 32 a 40 bytes: `pitch`, aplicado antes del giro en
  `Instance::apply` y en el sombreador). Un objeto por cada 4 tramos, con su nivel de detalle por el
  radio de la campana (`Object::detail_radius`), no por la caja: de cerca los tramos enteros, de lejos
  un cuerpo estirado por recta. Los tramos horneados salen de los lotes fijos; el resto de tubos
  (también el glTF exportado) se recorta en la pared de los nodos en vez de cruzar el pozo.
- Origen: el eje se baja a local en `f64` y solo la instancia pasa a `f32` (docs/15).

## 4. Pozo: cono hacia la salida

`build_manhole.py` construye cuerpo, marco, tapa y escalones con la boca hacia −X y los gira
(`opening_turn`) para que la pared vertical, la boca y los escalones queden sobre la **primera salida**
(por nombre); sin salida, quedan en −X. La banqueta y las anclas ya iban por el acimut de cada tubo.
El piloto del pozo se rehorneó (`tools/blender/piloto/<clave>.glb`, la clave cambió con la receta).

## 5. Medido

Windows 11, Blender 5.1.0, CLI y app en release, 28/11/2026 (con otros cuatro agentes compilando a
la vez: los tiempos son cotas altas).

| Caso | Cifra |
|---|---:|
| Cadena: tramos / con pieza / piezas distintas | 36 / 36 / **6** (DN 150, 200, 250 PVC de agua; DN 200, 250, 300 hormigón de saneamiento) |
| Hornear las 6 piezas, un proceso de Blender, con `.blend` | **3,9 s** |
| Repetir con todo en caché (sin Blender) | 45 ms |
| Pieza: triángulos / `.glb` / `.blend` | 960 / 45–48 kB / 596 kB |
| Caché de tubos de la cadena (`.glb` + `.blend`) | 0,28 MB + 3,6 MB |
| Red de 20 000 tramos (banco), todos con la pieza piloto: tramos comerciales colocados | 100 495 |
| ídem, escena del pase 3D (sin tubos horneados → con ellos) | 126,8 → **52,6 ms**; 32,5 → **17,1 MB**; sin aviso de tope |
| ídem, escena del pintor encuadrada (de lejos todo son líneas) | 11,3 → 10,5 ms |
| Banco del CI (`tests/banco.rs`, release), sin tubos horneados | pasa: escena 11,5 ms, malla 3D 156,9 ms (umbral 1 000), 32,5 MB, 1 688 B/objeto (tope 4 000) |

- Proyección: el número de piezas depende de las **clases** de tubo (DN × pared × material ×
  sistema), no de los tramos. Una red real de 20 000 tramos con ~50 clases son ~50 piezas: ~6 s de
  Blender la primera vez, ~2,4 MB de `.glb` y ~30 MB de `.blend`. Un `.glb` por tramo serían 20 000
  piezas, ~1 GB de caché y ~20 min de horneado.
- En el pase 3D los tubos horneados **abaratan** la escena: un tramo comercial es una instancia de
  40 bytes de un prototipo compartido, frente a los triángulos propios de cada tubo paramétrico.
- La instancia del pase 3D pasa de 32 a 40 bytes (el cabeceo) en toda la escena.

Capturas y revisión (carpeta `out/f17-04/`, ignorada; se rehacen con los pasos de abajo):

- `iso.png`, `iso-gpu3d.png`, `foco.png`, `foco-gpu3d.png`, `pozo-modelo.png`, `pozo-modelo-gpu3d.png`;
- `tubo.png` y `tubo-gpu3d.png` (escena nueva `tubo`): el colector que entra en PZ-302, con la última
  campana, la espiga metida en ella y el corte limpio en la pared del pozo;
- `tubo-corte.png` (escena nueva `tubo-corte`, solo pintor): corte transversal por el anillo de la
  última junta (campana, anillo y espiga);
- `revision/tubo_DN<dn>_<material>_<sistema>.glb` y `.blend` de las seis piezas de la cadena.

```powershell
redes proyecto cadena --salida out\f17-04\cadena\cadena.redes
redes -p out\f17-04\cadena\cadena.redes biblioteca hornear --receta tools\blender\build_manhole.receta.json
redes -p out\f17-04\cadena\cadena.redes biblioteca hornear --receta tools\blender\build_pipe.receta.json
$env:REDES_PROJECT = "out\f17-04\cadena\cadena.redes"
redes-app --snap out\f17-04\tubo.png --scene tubo        # y --gpu3d; también iso, foco, tubo-corte
```

Cifras de 20k: `cargo test --release -p redes-core --test tubo_horneado -- --ignored --nocapture`.

## 6. Para aprobar

- La partición dato/forma de §1, en particular: el diámetro interior cae al DN si no se declara (como
  el dibujo actual), la tabla de largos y juntas por material, la fila `otro` y que `sistema` decida el
  color de los plásticos.
- Que el tramo que llega a un nodo acabe siempre en corte limpio (sin campana) y que el primero arranque
  con su espiga en la pared.
- El cono excéntrico girado hacia la primera salida.
- Las capturas y los `.blend` de revisión (§5).

## 7. Queda

- Codos, tes y reducciones en los quiebres (F17-05): hoy el tramo sobre un quiebre sigue su cuerda.
- La limpieza de la caché (piezas que ya no usa nadie) sigue abierta (docs/17 §11.6).
- Exportar el glTF con los tubos horneados (hoy exporta el paramétrico, ya recortado en los pozos).
