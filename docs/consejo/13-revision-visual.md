# 13 — Revisión visual e interacción (fases 1–3)

Revisor: `rev-visual`. Alcance: catálogo paramétrico (fase 1), modos de inspección
corte/explosión/foco (fase 2) y etiquetas, leyenda y ficha (fase 3). **No se modificó código ni
assets**: solo se renderizaron vistas y se leyeron las imágenes.

Fecha: 2026-11-18. Build revisado: `target/debug/redes-app.exe` (18/11 18:07).

## 0. Cómo se revisó

Imágenes del repo (`docs/img/`): `catalogo-general.png`, `pieza-pozo.png`, `pieza-valvula.png`,
`modo-corte.png`, `modo-explosion.png`, `modos.gif`, `ciudad.gif`, `demo.gif`, `catalogo.gif`.
Además se generaron 30 vistas nuevas con el binario, en `out/rev/` (ignorado por git):

```powershell
$env:REDES_PROJECT="$PWD\demo\catalogo-piezas.redes"
.\target\debug\redes-app.exe --snap out\rev\sc-<escena>.png --scene <escena> --scale 1
# 13 escenas × proyecto catálogo; ciudad/calle/planta/iso/perfil repetidas con ciudad-san-bartolo
# y foco/pieza/corte/explosión/catálogo/vitrina otra vez con --scale 2 (y 3) para ver el detalle
```

Fotogramas de los GIF extraídos con `ffmpeg -ss` (los GIF son todos **760×475**, no 1280×800: eso
ya es un problema de entrega, ver §4).

Datos del proyecto usados para comprobar escala: la plataforma del catálogo es una losa de
**120 × 48 m** con pendiente del 3,5 % (`+0,035 m/m` en X). Comprobación clave: para cada pieza,
`z_rim` coincide **exactamente** con la cota de la plataforma en su (x, y) — p. ej. PZ-001 en
E 758 010 → z = 2500,35 = su `z_rim`. Es decir: **las piezas están enterradas y la app las dibuja
en modo rayos X**, no están de pie sobre el terreno.

---

## 1. ¿Se lee la pieza?

Veredicto por familia, mirando el render (no el código). "Se lee" = un ingeniero reconoce el
objeto sin la etiqueta.

| Familia (código) | Se lee | Qué se ve realmente | Por qué |
|---|---|---|---|
| **Pozo** (PZ-001…006) | **No / ambiguo** | Tambor o chimenea de 2,2 m con copete troncocónico y anillo; 0,7 m de Ø en la boca | Se dibuja entero **a través del terreno** (rayos X): el ojo lo lee como objeto *sobre* el pasto, no como registro enterrado. La tapa (Ø 0,60 × 45 mm) queda 6 cm dentro del cono (Ø 0,72) y casi enrasada → se lee como **hueco**, no como tapa. La escalera va dentro (invisible). Los 6 subtipos (inspección, caída, resalto, ciego, arenero, disipador) son **idénticos por fuera**: sus extras son interiores o de 5 cm. |
| **Válvula** (VL-001) | **Parcial** | Carrete horizontal con dos bridas gruesas + un "mango en T" | El volante es un **anillo plano de Ø 0,24 m** con 3 radios de Ø 16 mm (`parts.rs:552-556`): de canto se ve como una barra. El vástago (Ø 50 mm) sale de **~7 px** a 4,5 m y se pierde contra el fondo. No hay bonete/cuello: falta la silueta que identifica una compuerta. |
| **Hidrante** (HI-001) | **No (a distancia de catálogo)** | Punto de 40 px: poste con travesaño, tipo parquímetro | Está a ≥30 m de la cámara de catálogo; las 3 bocas y el bonete son <3 px. No existe escena de detalle para esta familia. |
| **Tanque** (TQ-001) | **Sí** | Cilindro oscuro con cubierta cónica | Es la única pieza que se reconoce de inmediato a 30 m: la cubierta cónica da la silueta. Contrasta con el pozo, que teniendo cono no lo lee por proporciones. |
| **Descarga** (DS-001) | **No** | Dos losas verticales dobladas sobre un dado | `descarga()` modela un muro de 1,5 m de −0,60 a +0,90 respecto de la cota de terreno, con alas y losa de fondo, **sin hueco de salida** (`parts.rs:643-661`); con rayos X se ve entero, pero sin el hueco no hay "vertedero": son dos placas. Además la placa izquierda muestra el pliegue de su triangulación. |
| **Cámara** (CM-002…008) | **Parcial** | Tubo corto con anillo en la boca, abierto | `camara()` usa `tubo()` con espesor (bien) pero **no tiene tapa**: se ve el agujero. Se lee "registro sin tapa", que es casi lo correcto, pero no se distingue de un pozo a simple vista. |
| **Sumidero** (SM-001) | **No evaluable** | Caja con una placa encima | A la distancia del catálogo (30–50 m) la rejilla (barras de Ø 30 mm) no existe en pantalla. No hay escena de detalle. |
| **PTAP** (PT-001) | **No** | Caja blanca de 13×8×4 m + 2 tambores oscuros + losa gris de 29×22 m | La losa del patio (0,12 m de espesor, `parts.rs:702`) es una lámina que **tapa el terreno** y domina el encuadre; el edificio es una caja sin huecos, más clara que todo lo demás (parece un placeholder). |

Conclusión de §1: **solo el tanque se lee**. Pozo y válvula tienen la *forma correcta en sección*
pero no la *lectura*: falta el contexto (enterrado, pavimento) y faltan los 2–3 detalles que
identifican (tapa con marco, volante con bonete).

---

## 2. Corte

Escena `corte` (`sc-corte.png`, `z-corte.png`, `z-corte2.png`, `docs/img/modo-corte.png`).

**No se entiende el interior.** Lo que hay y lo que falta:

- **Se ve**: el trasdós de la pared del fuste, la escalera al fondo y el fondo de la solera.
- **Falta el relleno de sección (poché)**: la arista se marca con **una línea amarilla de 1,6 px**
  (`view.rs:671-677`, color 0,98/0,86/0,42) y nada más. El material cortado conserva el color de la
  cara exterior, así que el corte no se distingue de una pared vista de frente.
- **No hay espesor**: el fuste del pozo es un `cilindro()` de una sola cara (`parts.rs:405-409`), no
  un `tubo()`. La sección es una **lámina de espesor cero**: por eso el "corte" parece una caja
  abierta y no un macizo de hormigón. La cámara sí usa `tubo()`; el pozo no.
- **La arista amarilla no basta y además está mal**: no sigue el contorno. En `z-corte2.png` hay un
  tramo horizontal **por debajo de la pieza y fuera de su silueta**, que acaba en un codo vertical
  en el aire (a la derecha), y el borde del cono sale en escalera. Causa: se emite **un segmento por
  triángulo** (`view.rs:669-680`) con la profundidad de su punto medio y se ordena con el mismo
  pintor que los polígonos (`view.rs:762-768`) → segmentos que "atraviesan" el terreno y el propio
  cuerpo. No se fusionan en una polilínea.
- **El corte no corta tubos ni terreno**: `corte_x` solo se aplica en el bucle de nodos
  (`view.rs:665`). Las tuberías siguen enteras y el suelo también, así que la sección es
  incoherente: un pozo seccionado flotando sobre un terreno sin seccionar.
- **El interior no se lee como interior**: misma luz y mismo tono que el exterior (no hay
  oscurecimiento por oclusión), no hay canal/invert ni acometidas; el fondo es un abanico de
  triángulos visible (`z-corte2.png`).
- **No se ve dónde está el plano**: no hay indicador del plano de corte ni su cota; solo
  `Shift+rueda`. En la escena, el plano queda casi de canto (az −20°, el 12°).

Respuesta directa: **falta el relleno de la sección y falta el espesor; la arista amarilla no es
suficiente** (1,6 px, sin relleno, con segmentos sueltos).

---

## 3. Explosión

Escena `explosion` (`sc-explosion.png`, `z-explosion.png`, `docs/img/modo-explosion.png`).

- **Se separan 3 sólidos y la escalera**: solera, fuste, cono y escalera. El **marco y la tapa
  suben** (1,23 y 1,65 m) pero son un anillo sin espesor y un disco de 45 mm vistos **casi de
  canto**: quedan como una raya de 1–2 px, y la etiqueta `PZ-001 · inspeccion` cae justo encima. El
  resultado visual es una pila **sin tapa**.
- **El orden solera→cono sí se lee**, pero la pila **termina en el cono**: no hay "marco" ni "tapa"
  reconocibles, así que no se puede leer la secuencia completa que promete el modo.
- **El fuste es un cilindro cerrado**: en la explosión se ve su **tapa superior plana** (un disco
  macizo), así que la pila son "tres quesos" en vez de un fuste hueco con su tapa. Es el mismo
  problema que en el corte: el fuste no tiene espesor.
- **El desplazamiento es por índice de parte, no por posición**: `dz = i/(n-1) · explosión · altura ·
  0,85` (`view.rs:649-657`). Con 6 partes, h = 2,2 m y explosión = 1,1, los saltos son
  0 / 0,41 / 0,82 / 1,24 / 1,65 / 2,06 m: las separaciones no guardan relación con el tamaño de cada
  parte, y por eso la escalera (índice 5) es la que más sube y **se va al aire, desplazada del eje**,
  por encima del cono. Parece un objeto suelto.
- **No se salen de cuadro** (la escena encuadra la pila completa), pero la escalera rompe la vertical.
- La explosión **no separa tubos ni terreno**: la tubería queda en su sitio, atravesando la pila.

Respuesta directa: **las partes se separan con sentido (solera → fuste → cono) pero el orden no
llega a leerse completo** (marco y tapa son invisibles de canto) y la escalera, que debería quedar
dentro, se va al aire. Es un `dz` acumulativo por índice, no un "exploded view".

---

## 4. Etiquetas

Implementación: `main.rs:461-473` y `view.rs:723-733`.

- **Tamaño**: `MONO_CAPTION` = **12 px** (`vendor/rune/crates/rune/src/tok.rs:142`), peso 500,
  color 0,86/0,88/0,92 con **alfa 0,92** sobre fondo 0,04/0,045/0,055 con alfa 0,78.
  A 1280×800 se leen, pero son **el texto más pequeño de la interfaz** y el único que hay que leer
  en el lienzo. Contraste aceptable (≈10:1), tamaño justo.
- **En los GIF de entrega no se leen**: los `.gif` son 760×475, así que la etiqueta cae a **7 px**.
  En `modos.gif` y `catalogo.gif` el texto es ilegible, y con él la única prueba de qué pieza es cuál.
- **Se cortan**: no hay recorte ni reposicionamiento. `sc-pozos.png` muestra `…eccion` (la etiqueta
  de PZ-001, seleccionada, se sale del lienzo por la izquierda y la tapa el panel); `sc-vitrina.png`
  y `sc-foco.png` muestran `TG-001 · pre…` y `CM-00…` cortadas por el borde derecho. En
  `modos.gif` (fotograma 7,2 s) hay 5 etiquetas apiladas en la esquina superior izquierda, dos de
  ellas cortadas.
- **Se amontonan y no hay guía**: se dibujan a `+8 / −6` del punto proyectado del *rim*, sin línea
  guía, sin caja que las una a su pieza y **sin detección de solape**. Con 16 etiquetas
  (`gif-modos-072.png`) no se sabe cuál pertenece a qué.
- **Se etiquetan objetos que no se ven**: el umbral es `r_px > 9` **o** selección (`view.rs:724`),
  pero se etiqueta también lo que queda a 30 m con 4 px de ancho, y lo que está fuera de cuadro
  (se proyecta igual). Resultado: etiquetas sobre el aire y sobre piezas que no se distinguen.
- **Presupuesto**: 24 en `view.rs:754` y `take(28)` en `main.rs:464`. El tope existe, pero no
  prioriza por cercanía **en pantalla** ni evita el solape: solo ordena por profundidad.
- **Etiqueta seleccionada**: va en verde (`OK`) pero sin fondo distinto: es la única jerarquía y se
  pierde entre las demás porque el objeto también es verde.

Respuesta directa: **se leen a 1280×800 (12 px, contraste correcto), pero molestan: se cortan en
los bordes, se amontonan sin guía y en los GIF son ilegibles.**

---

## 5. Color y materiales

- **Hormigón vs hierro vs PVC**: la paleta está a 0,13 de luminancia entre sí
  (`parts.rs:38-44`: HORMIGÓN 0,62/0,60/0,56 · HIERRO 0,47/0,48/0,51 · PVC 0,72/0,70/0,62 ·
  HORMIGÓN_OSCURO 0,42/0,41/0,39), **sin tono que los separe** (los tres son grises cálidos). El
  sombreado es `0,50 + 0,62·lambert` (+ especular si metal) **sin gamma, sin sombras y sin
  oclusión ambiental** (`parts.rs:68-82`): todo queda entre 0,5 y 1,1 de brillo, plano.
  En `hz-pozo.png` la tapa de hierro y el fuste de hormigón son el mismo gris verdoso; hay que
  mirar 3 veces para ver el anillo del marco. **Se distinguen mal.**
- **El verde de selección borra el material**: `tinte = [0,36 · 0,95 · 0,56]` con
  `k = (r+g+b)/2,4` (`view.rs:699-707`). Multiplica el color sombreado por un escalar y lo tiñe: el
  objeto conserva el sombreado (la forma **no** se pierde) pero **pierde toda la identidad de
  material** y el contraste con el fondo verde del terreno (el pozo seleccionado casi se funde con
  la plataforma en `sc-foco.png`). El rojo de "observado" tiene el mismo problema.
- **Terreno y niebla**: `fog()` lleva el color al fondo en 2,2·distancia (`view.rs:788-791`); de
  cerca el terreno es verde y de lejos **vira a arena/oliva** (`sc-ciudad.png` vs `sc-catalogo.png`),
  un cambio de tono que se lee como otro material, no como niebla.
- **Sin sombras ni oclusión**: ninguna pieza proyecta sombra; los edificios de la ciudad se funden
  entre sí porque dos caras con la misma orientación tienen el mismo color y no hay línea de arista
  (`gif-ciudad-044.png`).

---

## 6. Errores visuales concretos

1. **Piezas enterradas dibujadas en rayos X.** El terreno nunca ocluye la red (`view.rs:737-745`
   dibuja `net` después de `prims`). En el catálogo, PZ-001 tiene su `z_rim` = cota exacta de la
   plataforma, o sea que está enterrado 2,2 m; sin embargo se ve completo "de pie" sobre el pasto.
   Efecto: todas las piezas parecen maquetas apoyadas. **Es el error que más daña el veredicto.**
2. **Escalera que atraviesa el cono.** Largueros en `x = -(r-0,12) = -0,48` (`parts.rs:431-444`);
   el radio del cono a la altura del último peldaño (z = 2,10) es 0,404. Los **~18 cm superiores**
   de la escalera **salen por la pared del cono** (el "pincho" de 2 px de ancho y ~18 px de alto que
   se ve arriba a la izquierda en `sc-foco.png` y `hz-pozo.png`).
3. **Marco sin espesor y tapa enrasada.** Marco: anillo plano Ø 0,76/0,60 a z+0,01; tapa: cilindro
   Ø 0,60 × 0,045 a z+0,005 (`parts.rs:422-429`). El marco son 6 cm de ancho **sin altura** y queda
   5 mm por debajo de la cara de la tapa, así que se ve como una raya; la tapa, 6 cm dentro del cono
   y 45 mm de canto, se lee como hueco. Nunca se lee "marco + tapa".
4. **Descarga sin hueco de salida** (`parts.rs:643-661`): un muro ciego. Además las alas son
   prismas de polígono no plano y muestran el pliegue de la triangulación (`z-descarga.png`).
5. **Losa de patio de la PTAP como lámina suelta** (`parts.rs:702`, 29×22×0,12 m): tapa el terreno,
   tapa las piezas vecinas en `sc-pozos.png` (el gran plano gris del primer plano) y sus bordes
   duros la delatan como caja.
6. **Arista de corte fuera de la pieza** (§2) y **arista en escalera** por segmentos por triángulo.
7. **Triangulación visible en los discos**: las tapas de los cilindros se generan en abanico
   (`parts.rs:169-206`) y a 2× se ven los radios, sobre todo en la solera y en las bridas
   (`hz-valvula.png`, `z-explosion.png`). Se nota la malla.
8. **Aristas duras en los cilindros**: 8 caras con `lod 1`, 14–16 con `lod 2`, sin suavizado: el
   fuste del pozo muestra 16 bandas verticales y el contorno es un polígono visible en la boca
   (`hz-pozo.png`).
9. **Escenas clavadas al catálogo**: `detalle` apunta a `[320, 95, 2511]` (`main.rs:889`), que en
   coordenadas absolutas cae **200 m al este de la plataforma** (que acaba en E 758 120): el
   lienzo sale **negro, sin geometría y sin aviso** (`sc-detalle.png`). `pozos`, `catalogo` y
   `vitrina` usan coordenadas fijas (`main.rs:939-971`): con `ciudad-san-bartolo` dan una escena
   vacía o el municipio entero (`city-pozos.png`, `city-pieza.png`).
10. **Símbolos de 7 px en vez de piezas**: con `r_px < 3` se dibuja un rombo de 7,2 px
    (`view.rs:616-634`). A 165 m (escena `calle`) **no hay ni una pieza**: la "vista a pie de calle"
    muestra rombos (`city-calle.png`). Los tubos pasan a línea de 1,2–6 px (`view.rs:534-547`): el
    colector DN 1200 y la acometida DN 200 se dibujan igual.
11. **Escena `perfil`**: cámara a 4° de elevación; la ciudad queda como una franja de 12 px
    (`sc-perfil.png`). No es un perfil: no hay corte longitudinal, ni cotas, ni escala vertical.
12. **Archivos de entrega a 760×475** (`*.gif`): las etiquetas y el inspector son ilegibles; el
    `modo-corte.png`/`modo-explosion.png` sí están a 1280×800, pero no coinciden con los GIF.
13. **Ficha del inspector con 9 guiones**: para PZ-001, "Material —", "Espesor —", "Cota de terreno
    —", "Profundidad —"… La geometría se dibuja con los valores por defecto de la familia (el
    `.redes` no trae diámetro ni altura), y **nada avisa** de que la pieza es genérica. Incumple el
    criterio 3 de `10-sintesis-detalle.md` ("ninguna pieza se dibuja a cota 0 por falta de datos: se
    dibuja un marcador y se avisa").
14. **Ninguna sombra, ninguna oclusión ambiental, ningún borde**: causa raíz de que el hierro, el
    hormigón y el PVC se lean como el mismo plástico gris.

---

## 7. Top 10 arreglos visuales (por impacto)

| # | Arreglo | Escena donde se nota | Archivo probable |
|---|---|---|---|
| 1 | **Ocluir de verdad**: que el terreno tape lo enterrado (o una vitrina explícita con fosa/terreno fantasma). Sin esto, todo se lee como maqueta sobre el pasto. | `foco`, `pozos`, `catalogo` | `view.rs:737-747` (+ orden por profundidad real) |
| 2 | **Sección con poché y espesor**: fuste como `tubo()` (pared real), relleno del plano de corte, tubos y terreno también cortados, arista limpia (polilínea fusionada, 2 px). | `corte` | `parts.rs:405-409`, `view.rs:664-680` |
| 3 | **Tapa y marco legibles**: tapa al ras del cono, con marco de 6 cm de ancho y 3 de alto, juntas/agarradera; que el Ø de la tapa sea ≥ Ø de la boca. | `foco`, `corte`, `piezas` | `parts.rs:420-429` |
| 4 | **Explosión por altura de parte** (no por índice) y con la escalera dentro del eje; separar también marco y tapa y que ninguno quede tapado. | `explosion` | `view.rs:645-657` |
| 5 | **Volante y bonete de válvula**: volante con sección (no anillo plano), radios de 25 mm, vástago de 80 mm, bonete troncocónico sobre el cuerpo. | `pieza` | `parts.rs:547-557` |
| 6 | **Etiquetas**: recorte/reposición dentro del lienzo, anti-solape (carriles), línea guía de 10 px, umbral por tamaño en pantalla, y subir a 13 px. | todas | `main.rs:461-473`, `view.rs:723-733` |
| 7 | **Materiales y luz**: 3 tonos distintos (hormigón cálido, hierro azulado, PVC claro), sombra proyectada simple u oclusión por profundidad, y no teñir el material al seleccionar (usar contorno o halo). | todas | `parts.rs:38-44`, `68-82`; `view.rs:699-707` |
| 8 | **LOD**: subir el umbral de símbolo (r_px < 3 → < 1,5), y dar ancho real a los tubos hasta 1:2000. | `calle`, `ciudad`, `planta` | `view.rs:534-547`, `616-634` |
| 9 | **Escalera dentro del cono**: bajar el último peldaño a `z_marco - 0,35` o llevar los largueros a `x = -(0,36-0,06)`. | `foco` | `parts.rs:431-444` |
| 10 | **Escenas robustas**: calcular los objetivos desde el proyecto (no coordenadas fijas) y avisar cuando la escena queda vacía; `detalle` hoy es un lienzo negro. | `detalle`, `pozos`, `calle` | `main.rs:886-897`, `960-971` |

**Arreglos de una tarde, mucho retorno**: #3 (tapa/marco), #9 (escalera), #10 (detalle vacío),
recorte de etiquetas (#6, solo el clamp) y subir el GIF a 1280×800.

---

## 8. Veredicto

**Todavía no: no se ve como en la realidad.** Se ve la *intención* de la forma (el pozo tiene
solera, fuste, cono, marco, tapa y escalera; la válvula tiene cuerpo, bridas, vástago y volante),
pero **no se lee** por cuatro razones, en este orden:

1. **No hay oclusión**: las piezas enterradas se dibujan enteras sobre el terreno (error #1). Eso
   convierte un registro de 1,2 m de boca en una chimenea de 2,2 m de alto sobre el césped.
2. **La tapa no se lee** (error #3): el detalle que identifica un pozo es justo el que no existe
   visualmente; y el corte, que debería resolverlo, no tiene relleno ni espesor (§2).
3. **El material es uno solo** (§5): sin sombras ni tonos, hormigón, hierro y PVC son el mismo gris
   verdoso, y el verde de selección lo iguala todo.
4. **El detalle desaparece antes de importar**: a 40 px (el criterio 1 de `10-sintesis-detalle.md`)
   solo queda la silueta; a 30 m ya no hay tapa, ni peldaños, ni volante.

Contra los cuatro criterios de aceptación de `docs/10-sintesis-detalle.md`:

| Criterio | Estado | Evidencia |
|---|---|---|
| 1. A 40 px: pozo con cono, marco, tapa y peldaños; válvula con bridas, vástago y volante | **No** | A 40 px el pozo es una silueta de tambor; el marco/tapa son 1–2 px y los peldaños no existen (`sc-catalogo.png`, `catalogo-general.png`). La válvula muestra bridas y un mango, no un volante (`hz-valvula.png`). |
| 2. Tapa en `z_rim` y solera en `z_invert` (±1 mm) | **Parcial** | La solera arranca en `z_invert` (bien); la tapa se modela de `z_rim+5 mm` a `z_rim+50 mm` (`parts.rs:421-429`), no al ras. |
| 3. Ninguna pieza a cota 0 por falta de datos: marcador + aviso | **No** | El catálogo dibuja todo con valores por defecto de la familia y la ficha muestra 9 guiones sin avisar (§6.13). |
| 4. 200 piezas a 30 fps orbitando | **No verificado aquí** | Fuera del alcance de esta revisión (hace falta medir en ventana, no en `--snap`). |

Lo que sí está a la altura: la **silueta del tanque**, la **topología del catálogo** (49 objetos,
29 familias, leyenda con conteos), la **ficha paramétrica en vivo** y la **estabilidad** del render
(30 snaps sin parpadeos, sin artefactos de textura y sin errores del binario).

Camino corto para pasar de "maqueta" a "se ve como en la realidad": **#1, #2 y #3 del Top 10**
(oclusión + sección con espesor + tapa/marco). Con esos tres, el pozo se defiende; el resto es
pulido.

---

## Anexo — evidencia

Imágenes propias (en `out/rev/`, no versionadas):

- Escenas catálogo: `sc-iso|planta|detalle|ciudad|calle|catalogo|vitrina|pozos|pieza|foco|corte|explosion|perfil.png`
- Escenas ciudad: `city-*.png` (mismas 9)
- 2× y 3×: `hi-foco.png`, `hi-pieza.png`, `hi-corte.png`, `hi-explosion.png`, `hi-catalogo.png`,
  `h3-catalogo.png`, `h3-vitrina.png`
- Recortes: `z-pozo-foco.png`, `z-valvula.png`, `z-descarga.png`, `z-corte.png`, `z-corte2.png`,
  `z-explosion.png`, `z-top.png`, `z-planta.png`, `z-tanque.png`, `z-stars.png`, `z-roofs.png`,
  `hz-pozo.png`, `hz-valvula.png`, `hz-gris.png`, `h3-cat-left.png`
- GIF: `gif-modos-{002,036,072}.png`, `gif-ciudad-044.png`, `gif-demo-032.png`, `gif-catalogo-048.png`

Código citado: `crates/redes-core/src/parts.rs` (38-44, 68-82, 164-206, 389-494, 506-560,
643-661, 696-704), `crates/redes-core/src/view.rs` (534-547, 616-634, 645-680, 699-733, 737-768,
788-791), `crates/redes-app/src/main.rs` (461-473, 886-897, 935-971, 1007-1040).
Datos: `demo/catalogo-piezas.redes` (plataforma 120×48 m, `z_rim` = cota del terreno en cada pieza).
