# F17-05 — Accesorios, válvulas e hidrantes horneados en Blender

Hecho el 28/11/2026 con Blender 5.1.0 (MAM-1145). Igual que el pozo de F17-03 (docs/17 §12), cada
pieza sale de una receta `bpy` versionada, se hornea con `redes biblioteca hornear` y sustituye a la
paramétrica de `parts.rs` en el visor, con y sin `--gpu3d`. Pendiente de aprobación del usuario:
la partición dato/forma (§2), la forma de cada pieza (capturas, §6) y las decisiones de §7.

## 1. Familias y subtipos cubiertos

Del registro (`families.rs`) y de los demos:

| Familia / subtipo | Receta | Dónde aparece |
|---|---|---|
| `comun.accesorio` / `codo` | `build_elbow` | catálogo del registro; vitrina |
| `comun.accesorio` / `tee` | `build_tee` | `redes proyecto catalogo` (suelta, sin tubos); vitrina |
| `comun.accesorio` / `reduccion` | `build_reducer` | vitrina |
| `agua.valvula` / `compuerta`, `mariposa`, `retencion` | `build_valve` (una receta, `subtipos`) | cadena (11 compuertas en línea), catálogo, demo; vitrina |
| `agua.valvula` / `aire` | `build_air_valve` | cadena (VL-001); vitrina |
| `agua.hidrante` / `columna` | `build_hydrant` | catálogo (sin tubos ni datos); vitrina |

Fuera de esta entrega, con la pieza de respaldo: `curva`, `cruz`, `ampliacion`, `union`, `tapon`,
`brida`, `collarin`, `silla`; válvulas `purga` (VL-002 de la cadena), `reduccion_presion`,
`sostenedora`, `reguladora_caudal`, `alivio`, `flotador`, `pie`, `altitud`, `guillotina`, `pinch`;
hidrantes `arqueta`, `boca_riego`, `tipo_tapon`. Con las mismas piezas de `redes_mallas.py` cada una es
una receta corta.

## 2. Qué es dato y qué es forma (para aprobar)

**Dato** (del origen; si falta, no se hornea: `W_PARAMETRO_FALTANTE` y pieza de respaldo, D4/D16):

| Receta | Datos del objeto | Datos de los tubos (`puertos`) |
|---|---|---|
| codo | `diametro_nominal`, `material` | 2 bocas: dirección de cada tubo → **ángulo del codo** |
| tee | `diametro_nominal` (tramo recto), `dn_ramal`, `material` | 3 bocas: cuál es el recto y cuál el ramal, **ángulo del ramal** |
| reducción | `diametro_nominal` (entrada), `dn_salida`, `material` | 2 bocas; si el DN de los tubos dice lo contrario, la pieza se da la vuelta |
| válvula en línea | `diametro_nominal`, `material`, subtipo | 2 bocas; la retención abre del tubo que llega al que sale (flecha en la tapa) |
| ventosa | `diametro_nominal` (de la válvula), `material` | 2 bocas **con DN**: el DN del tubo principal fija el collarín y la altura |
| hidrante de columna | `diametro_nominal`, `numero_bocas`, `altura_sobre_rasante`, profundidad = `tapa − solera` | 1 boca: de dónde llega el ramal |

Los ángulos y los DN de cada boca **se derivan de los tubos conectados** (patrón `conexiones` de
F17-03, ahora fuente `puertos`, §3): no se usa el atributo `angulo` del codo ni `angulo_ramal` de la
tee, porque la pieza tiene que encajar con los tubos tal como están dibujados. Si el atributo y los
tubos discrepan, hoy no se avisa (decisión abierta, §7).

**Forma** (en el contrato, entra en la clave, nunca es `Dato` ni alimenta cantidades del proyecto):
proporciones relativas al DN — diámetro exterior del cuerpo 1,2 DN, embocaduras 1,45 DN × 0,6 DN,
radio del codo 1,0 DN (y brazo máximo 3 DN cerca de un giro en U), cono de la reducción 1,5 × ΔDN +
0,5 DN mín.; válvulas: bridas 1,5 DN + 8 cm × 25 mm; compuerta 0,1 m + 0,9 DN de largo con bonete,
vástago y volante; mariposa 0,05 m + 0,25 DN con cuello, reductor y volante; retención 0,15 m +
1,4 DN con tapa sobre la bisagra y flecha; accionamiento de `acero`; ventosa sobre collarín de
`hierro_ductil`; hidrante con zapata de radio 1 DN, columna 1,8 DN pintada (`pintura_roja`), brida de
rotura en la rasante, bocas de 65 mm (`laton`) repartidas desde +X y cuadradillo. Ángulos al grado
(`paso_angulo_deg`: 1). Segmentos 24–32.

Simplificaciones: sólidos macizos (sin interior), sin juntas ni pernos; la tee y el ramal a la misma
altura de eje; reducción concéntrica; solo desvíos en planta (la pendiente de los tubos no entra).

## 3. Marco de puertos y colocación

- `bake::node_port_frame`: con los tubos que tocan el objeto (ordenados por código) elige la boca
  de referencia — una del par más recto (el tramo recto de una tee), la que **llega** si solo una
  llega, si no la primera por código — y gira el marco para que apunte a −X. Las demás van por
  ángulo y se llaman `entrada__<n>`/`salida__<n>` según el sentido del tubo; **el número es la boca,
  no el código del tubo**, así que dos codos iguales que apuntan a sitios distintos comparten pieza.
  `PortFrame::ports[i].link` da qué tubo va a cada ancla.
- Fuente de parámetro nueva `puertos` (`por_subtipo`: cuántas bocas; otro número es dato que falta,
  no se adivina; `paso_angulo_deg`; `con_dn`: todo tubo trae DN; `recto`: la pieza es recta y la boca
  lejana se dibuja en la prolongación, así que un tubo que sale torcido no crea otra pieza). Sin la
  cota o el trazado de un tubo: `conexiones (TB-x: el tramo no trae cota de solera…)`.
- Fuente nueva `subtipo` y campo `subtipos` del contrato: una receta para compuerta, mariposa y
  retención. `redes_mallas.py` (mallas explícitas comunes) va en `modulos`, el campo de F17-07, y entra en la
  clave.
- Campo `colocacion` del contrato → extra `horneado.colocacion` en el manifiesto.
  `bake::model_placement` lo usa en el pase 3D, el pintor y el foco:
  - `puertos` (accesorios y válvulas): giro = el del marco; altura = solera del tubo de referencia en
    el objeto − ancla `solera` (la solera del interior bajo el centro de la pieza, en z = 0). No usa
    la cota del nodo: en la cadena las válvulas tienen la solera 1,8 m bajo los tubos.
  - `puertos_en_objeto` (hidrante): giro del marco, altura en la solera del hidrante (su profundidad
    es `tapa − solera` del objeto).
  - sin el extra (pozo): `giro` y `cota_base_modelo`, como en F17-03. Si los tubos ya no dan marco,
    también.
- La resta del origen sigue en `f64` (docs/15) y el doble giro de ejes de F17-03 no vuelve: la
  malla se usa tal como la da el lector.
- Dónde acaba un tubo (`pipe_layout::wall_offset`, una sola regla para el pintor, `--gpu3d` y glTF,
  unida a la de F17-04): en el ancla con el código del tubo (pozo horneado, F17-04); si no, en la boca de
  su puerto en un accesorio, válvula o hidrante horneado (el marco de puertos dice qué ancla es la suya:
  así el tubo ya no asoma por la esquina del codo); si no hay pieza horneada, en el borde de la pieza de
  respaldo de un pozo o cámara, y en el centro de un accesorio, válvula o hidrante de respaldo (no tienen
  pared: la pieza tapa la unión; antes quedaba un hueco de 0,75 m).

## 4. Recetas y pruebas

- `tools/blender/redes_mallas.py` (anillos barridos, torno, tubo con curvas, caja, prisma, piezas con
  sus etiquetas y anclas), `build_elbow.py`, `build_tee.py`, `build_reducer.py`, `build_valve.py`,
  `build_air_valve.py`, `build_hydrant.py` y sus `.receta.json`. Cada parte es un sólido cerrado con
  su material de color; anclas `solera` y una por boca (`redes_puerto`, `redes_azimut_deg`,
  `redes_eje`, `redes_dn_mm`).
- Pilotos versionados en `tools/blender/piloto/fittings/`: `codo.json` (45°), `tee.json`,
  `compuerta.json`, `hidrante.json` y sus `<clave>.glb` (23–80 kB). La tee, la compuerta y el hidrante
  son las piezas de la vitrina (mismas claves).
- `vitrina.plan.json`: plan de ejemplo (fuente «vitrina F17-05: datos de ejemplo») con codo 90° y
  45°, tee con hidrante en el ramal, ventosa, reducción 200→150, compuerta, mariposa y retención.
- Pruebas sin Blender: `bake.rs` (marco, giro, referencia de la tee, paso de ángulo, faltas, subtipos,
  ventosa sin DN, colocación, módulos en la clave, válvulas de la cadena),
  `tests/accesorios_horneados.rs` (pilotos: nombre = clave actual, partes, sólidos cerrados con volumen
  positivo por material, anclas en su eje y dirección, manifiesto, hidrante a su altura, toda la
  vitrina tiene sus datos), `redes-cli/tests/hornear_accesorios.rs` (caché sembrada con los pilotos,
  sin Blender). Con Blender: rehornea los pilotos byte a byte y hornea la vitrina entera; sin Blender se
  salta con aviso. `pipe_layout.rs`: el tubo acaba en la boca de su puerto, entra en las válvulas de respaldo y para en
  los pozos.
- Cinco mutaciones nuevas en `scripts/mutar.ps1` (giro, referencia de la tee, pieza recta, tubo en la
  boca del puerto, corte del tubo en el respaldo); la de F17-03 «modelo-en-la-tapa» sigue el nuevo `model_placement`.
- Rehornear un piloto si cambia una receta, `redes_mallas.py` o `redes_bake.py` (la prueba dice cómo).

## 5. Medido (Windows 11, Blender 5.1.0, CLI en release)

| Pieza | Triángulos |
|---|---|
| codo 90° DN150 / 45° DN150 | 720 / 528 |
| tee 200×100 | 480 |
| reducción 200→150 | 288 |
| compuerta DN200 / mariposa DN150 / retención DN150 | 968 / 740 / 696 |
| ventosa DN50 sobre DN200 | 768 |
| hidrante de columna DN100, 2 bocas | 1 612 |

- Vitrina: 9 objetos → 9 piezas (todas distintas a propósito); cada receta en un proceso, 2,3–2,8 s
  (las tres válvulas juntas, 3,8 s); todo en caché, sin Blender, 0,1–0,2 s.
- `redes proyecto cadena`: 12 compuertas → **1 pieza** para 11 (horneada en 3,6 s); VL-112 está al
  final de la línea con un solo tubo y queda con la de respaldo (`conexiones (1 tramos conectados; la
  pieza tiene 2 bocas)`). VL-001 (ventosa): 1 pieza, 3,9 s. VL-002 (purga) no tiene receta.
  Sin `recto`, las 11 compuertas daban 4 piezas (el tubo sale a 62° o 162° en algunas).
- Proyección a 20 000 nodos (sin medir): el número de piezas no crece con los nodos sino con las
  combinaciones de datos. Válvulas: subtipo × DN × material × sentido de los tubos (≈ 3 × 10 × 3 × 3 =
  ≲ 270, en la práctica decenas). Codos: DN × material × ángulo al grado (cota 180 × 10 × 3 = 5 400;
  con los ángulos comerciales 11¼/22½/45/90, decenas). Tees: pares de DN × ángulo del ramal (decenas).
  Hidrantes: la profundidad es dato continuo (al micrómetro), así que hoy es una pieza por hidrante
  (≈ 1 por cada 100–150 m de red; 20 000 nodos ≈ 200–400 hidrantes). A ~0,05 s por pieza en lote
  (docs/17 §11.5): 500 piezas ≈ 30 s la primera vez; cada `.blend` de revisión pesa ~0,55 MB (500 →
  ~275 MB), cada `.glb` 20–80 kB.

## 6. Para aprobar

Capturas en `out/f17-05/` (carpeta ignorada; se rehacen con los pasos de abajo): `iso.png`,
`codo-modelo.png`, `tee-modelo.png`, `reduccion-modelo.png`, `valvula-modelo.png`,
`hidrante-modelo.png` y las mismas con `-gpu3d`. `.blend` de revisión de cada pieza distinta en
`out/f17-05/revision/` (las nueve de la vitrina y las dos de la cadena).

```powershell
redes proyecto nuevo --nombre V --salida out\f17-05\vitrina\vitrina.redes
redes -p out\f17-05\vitrina\vitrina.redes importar plan --archivo tools\blender\piloto\fittings\vitrina.plan.json
# una vez por receta: build_elbow, build_tee, build_reducer, build_valve, build_air_valve, build_hydrant
redes -p out\f17-05\vitrina\vitrina.redes biblioteca hornear --receta tools\blender\build_tee.receta.json
$env:REDES_PROJECT = "out\f17-05\vitrina\vitrina.redes"
redes-app --snap out\f17-05\tee-modelo.png --scene tee-modelo     # y --gpu3d; codo-, reduccion-, valvula-, hidrante-modelo
```

Escenas nuevas de la app: `codo-modelo`, `tee-modelo`, `reduccion-modelo`, `valvula-modelo`,
`hidrante-modelo` (el primer objeto con pieza horneada de esa familia, de cerca, sin terreno).

## 7. Decisiones para el usuario y huecos

1. Aprobar la partición de §2: ángulos y DN de cada boca **desde los tubos**, no desde los atributos
   `angulo`/`angulo_ramal`; ¿avisar `W_DISCREPANCIA` cuando el atributo y los tubos no cuadran?
2. Ángulos al grado (`paso_angulo_deg` 1): ¿vale, o 0,5°, o los comerciales?
3. Piezas rectas (`recto`): la válvula se dibuja recta aunque el tubo salga torcido. ¿Avisar del desvío?
4. Hidrantes: ¿redondear la profundidad (p. ej. al centímetro) para compartir piezas en redes grandes?
   Hoy es dato al micrómetro, una pieza por hidrante.
5. Válvulas importadas de EPANET como dispositivo de enlace (F12-15, `Enlace` sin tramos): no tienen
   tubos, así que no se hornean (`conexiones (sin tramos conectados)`). Podrían orientarse con `desde`
   → `hasta` y la `cota_eje`; queda pendiente.
6. Hallado de paso (fuera de F17-05): un proyecto nuevo con un plan importado deja `origin` en (0, 0),
   y con `--gpu3d` la geometría UTM en `f32` se pierde de cerca (pantalla negra). Para las capturas se
   puso el origen a mano en el `.redes` de `out/`. Arreglo propuesto: que `importar plan` fije el origen
   si el proyecto no tiene objetos.
7. `biblioteca aplicar` / `anclar_conexiones` no aplica giro (tampoco para el pozo): con una pieza en
   marco de puertos movería los extremos de los tubos a anclas sin girar. Hornear no lo usa.
8. Resuelto al integrar con F17-04: el tubo acaba en la boca del puerto y ya no asoma por la esquina del
   codo (`codo-modelo.png`).
