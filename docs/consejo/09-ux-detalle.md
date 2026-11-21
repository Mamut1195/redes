# 09 — UX de detalle: mirar una pieza de cerca

**Autor:** `ux-detalle` (consejo de Redes) · **Fecha:** 2026-11-18 · **Estado:** propuesta para revisión
**Ámbito:** app de escritorio `redes` (Rust + Rune) sobre `redes-core::view`. **No** toca el modelo de
dominio, ni la CLI, ni el formato `*.redes`.
**Depende de:** 01 (las cuatro cotas, taxonomía de familias), 02 (capas, `ElementId`), 04 (cámara,
picking, LOD, etiquetas), 05 (personas P1 ingeniera, P2 municipal, P3 obra), 06 (catálogo de familias,
vocabulario de partes y parámetros).
**Vocabulario:** los nombres de partes y de parámetros son los de `06-catalogo-familias.md` (§1.4 y
§3); aquí **no** se redefine ninguno. Si hay discrepancia, manda 06.

> **Método.** Todo lo que digo del estado actual está leído en el código de este repositorio, no
> supuesto: `crates/redes-core/src/view.rs` (cámara, escena, orden del pintor) y
> `crates/redes-app/src/main.rs` (paneles, selección, gestos). Los parámetros de cámara, zoom,
> picking y etiquetas que propongo están marcados `[NUEVO]`; no propongo ni un atributo de dominio
> nuevo: la ficha de §5 solo expone los que ya fija 06.

---

## 0. Índice de decisiones

| # | Decisión | Sección |
|---|---|---|
| **D1** | Tres modos de inspección apilables: **foco** (cámara), **corte** (plano), **explosión** (ensamblaje). `Esc` deshace una capa por pulsación (pila LIFO) | 1 |
| **D2** | El foco **no reorienta** la cámara: conserva acimut y elevación del usuario y solo cambia objetivo y distancia. El encuadre se resuelve por esfera envolvente, no por prueba y error | 1.2 |
| **D3** | El corte es un **plano vertical con recorte real**, no un truco de transparencia; las caras cortadas se rellenan con trama | 1.3 |
| **D4** | El objeto enfocado se dibuja con **oclusión real** (calando el terreno); el resto de la red sigue en rayos X como hoy | 1.2, 2.5 |
| **D5** | La distancia mínima baja de **5 m a 0,30 m**; el *near plane* pasa de constante (0,05 m) a **proporcional a la distancia** | 2.2 |
| **D6** | Zoom **multiplicativo con paso por banda** (12 % en pieza, 35 % en ciudad) + `Ctrl+rueda` fino y `Shift+rueda` grueso; `Ctrl+0` = **escala 1:1 en pantalla** | 2.3 |
| **D7** | El LOD de pieza se decide por **tamaño proyectado del objeto** (px), no por la escala global de la vista | 2.2 |
| **D8** | Seis reglas contra el *z-fighting* del pintor: sesgo relativo por pieza, no coplanaridad de 2 mm, corte en la intersección, calado de terreno, silueta de contacto, y prohibición de alfa en detalle | 2.5 |
| **D9** | `z_scale` (exageración vertical) se **anula a 1,0** en foco, con aviso explícito de que las cotas etiquetadas son reales | 2.6 |
| **D10** | Etiquetas por **prioridad fija** (selección > hover > observado > chincheta), carriles de 14 px y colapso a chip `+N`; presupuesto 250 | 3.3 |
| **D11** | Estado de revisión **nunca solo por color**: forma + patrón de borde + insignia + texto en la ficha | 4.2 |
| **D12** | Ficha paramétrica con tres clases de campo: **E** editable (recalcula en vivo), **R** derivado, **B** bloqueado con motivo visible | 5.2 |
| **D13** | Aprobar **congela** la geometría: editar un parámetro geométrico de una pieza aprobada pide reabrirla o duplicarla | 5.6 |
| **D14** | El tour es un **guion declarativo** en JSON (mismo formato que `docs/demo/recorrido.json`), reproducible y grabable; nunca modifica el proyecto | 6.2 |
| **D15** | Un parámetro faltante **no se inventa**: proxy de alambre con insignia `?`, nunca a cota 0; panel de vacíos como lista de tareas | 8 |

---

## 1. Modos de inspección

### 1.1 El problema real: tres escalas en la misma ventana

Hoy la cámara está pensada para 1 km: `distance` se acota a `[5, 200_000]` y el encuadre inicial
sale de `diag·1,25` (`view.rs`, `Camera::fit`). Un pozo de 1,20 m a 5 m de distancia ocupa unos
230 px: se ve, pero **no se puede entrar**. Y aunque se pudiera, no hay forma de ver el interior ni
el ensamblaje.

La inspección de detalle responde a tres preguntas distintas del ingeniero, y por eso son tres
modos, no uno:

| Pregunta del ingeniero | Modo | Qué cambia |
|---|---|---|
| «¿Este pozo está bien ubicado y a qué cota?» | **foco** | cámara y encuadre |
| «¿Entran los tubos a la cota correcta? ¿Hay banqueo?» | **corte** | geometría (plano de recorte) |
| «¿Cómo está armado? ¿Qué pieza es cuál?» | **explosión** | geometría (separación) |

Y una cuarta pregunta, la del promotor: «¿cómo es esto en la realidad?». La responde la suma de
foco + corte + explosión, que es exactamente el plano del tour (§6).

### 1.2 Modo foco

**Qué hace.**

| Paso | Comportamiento |
|---|---|
| Entrada | Doble clic sobre el objeto, o `F` con el objeto seleccionado, o `Enter` en el inspector |
| Cámara | Animación de **450 ms** (ease-out cúbico) hacia `target` = centro del volumen del objeto y `distance` = la que encuadra su esfera envolvente con **15 % de margen** |
| Orientación | Se **conserva** acimut y elevación; si la elevación es < 8° se sube a 12° (para no mirar el pozo de canto) |
| `fov` | 45° → **38°** en foco (menos distorsión, lectura de cotas más honesta) |
| Selección | El objeto queda seleccionado y el inspector salta a la pestaña **Ficha** |
| Contexto | Los objetos a menos de 20 m del objetivo suben a su LOD de pieza; el resto no cambia |
| Salida | `Esc`, clic en vacío, `F` de nuevo, o alejar la rueda más de **1,6×** la distancia de entrada (histéresis para no salir por un temblor de rueda) |

**Por qué no reorientar.** Un doble clic que gira la cámara 180° desorienta: el usuario pierde la
referencia de la calle y del norte. El encuadre automático debe ser **predecible**: mismo acimut,
misma elevación, solo más cerca.

**Cómo se calcula el encuadre (sin iterar).** Para una esfera de radio `R` y un `fov` vertical:

```
d = R / sen(fov/2) · 1,15        // 1,15 = margen del 15 %
d = clamp(d, 0,30 m, 2 000 m)
```

Si el objeto es muy alargado (una tubería de 40 m), se encuadra por su **punto medio** y se limita
la distancia a `8 × Ø`, no a la longitud: en foco se inspecciona una junta, no el tubo entero. Para
ver el tubo entero está la vista de planta.

**Lo que aparece al entrar (no todo a la vez).** En los primeros 250 ms solo se ve la geometría y
el contorno de selección; a los 250 ms aparecen las etiquetas de objeto (§3) y, si el objeto tiene
piezas, la lista de piezas en la ficha. Las etiquetas de pieza (brocal, cono…) **solo** con corte o
explosión activos: si no, compiten con las cotas.

### 1.3 Modo corte

**Qué hace.** Corta el objeto (y lo que lo rodea) con **un plano vertical** y dibuja las caras
cortadas con relleno de trama, como una sección de plano. No es transparencia: es recorte.

| Elemento | Comportamiento |
|---|---|
| Plano | Vertical, pasa por el eje del objeto; normal horizontal inicial = la del acimut actual |
| Cota del plano | Se muestra en el chip del modo: `corte a 1,35 m del eje` |
| Mover | `Shift+rueda` (paso 10 cm) · `Shift+rueda` con `Alt` (paso 1 cm) · arrastrar el gizmo del plano |
| Girar | `Alt+arrastrar` sobre el disco del gizmo (giro de 15°), o botón `Girar 90°` en el chip |
| Qué se recorta | El objeto enfocado y los objetos a menos de 1,5 m; el terreno dentro de un radio de 2 m |
| Caras cortadas | Relleno de trama 45° a 6 px, borde 1,5 px del color de la familia |
| Mitad descartada | Se dibuja al 18 % de opacidad y **sin** etiquetas (es «fantasma», contexto) |
| Lo que revela | Tubos entrando con su DN y su cota clave, banqueo (media caña), peldaños, espesor de pared, solera, brocal y cono |
| Salida | `X` o `Esc` |

**Decisión de alcance.** Un solo plano activo en v1. Dos planos simultáneos (corte en caja) son
posibles con el mismo mecanismo, pero multiplican los casos de borde del pintor y no los necesita
ninguna de las tres personas.

**Trampa que evito.** El corte no debe cortar solo el pozo: si el tubo que entra no se corta
también, se ve un tubo flotando contra una pared. El plano se aplica a **todo** lo que está dentro
del radio de corte, y el tubo se parte exactamente en el plano (con su elipse de sección y su
espesor de pared: dos elipses concéntricas).

### 1.4 Modo explosión

**Qué hace.** Separa las piezas del ensamblaje a lo largo de un eje, con animación de 300 ms, para
ver el orden de montaje.

| Elemento | Comportamiento |
|---|---|
| Eje | Por defecto el vertical del objeto. Válvula: el eje del tubo. Vertedero: la normal del muro |
| Factor | `0` ensamblado … `1` legible (separación = 0,35 × altura de cada pieza) … `2,5` despiece completo |
| Control | `E` alterna 0/1 · `Ctrl+rueda` varía fino · `+`/`-` en pasos de 0,1 · deslizador en el chip |
| Etiquetas | Cada pieza se etiqueta con su nombre y su parámetro que la define (`cono · h 0,90 m`) |
| Selección | En explosión, `Tab` recorre las piezas y el inspector muestra la **ficha de pieza** (solo lectura, §5.2) |
| Combinación | Explosión + corte = vista de despiece seccionado: es el plano que vende |
| Salida | `Esc` (vuelve a 0 y conserva el foco) |

**Regla dura.** Las piezas **no son objetos del modelo**: no se aprueban, no se exportan, no tienen
código. Aprobar el brocal no existe; se aprueba el pozo. La ficha de pieza lo dice en su cabecera
(`pieza de PZ-001`). Esto evita que la revisión municipal entre en un nivel de detalle que el DWG
tampoco va a contener.

### 1.5 La pila de modos y cómo se sale

Los modos se apilan en el orden en que se activan. `Esc` **deshace la última capa**, no todo:

```
[foco]                      Esc → vista anterior
[foco > corte]              Esc → foco
[foco > corte > explosión]  Esc → corte
[foco > explosión]          Esc → foco
```

Al salir de foco se restaura la **cámara exacta** anterior (posición, objetivo, fov y `z_scale`),
guardada en una pila de 8 entradas. Nada de «volver a encuadrar todo»: el usuario vuelve donde
estaba.

### 1.6 Tabla de interacción

| Acción | Resultado | Atajo |
|---|---|---|
| Doble clic en objeto | Foco + encuadre + selección + ficha | doble clic |
| Seleccionar y enfocar | Ídem, sin ratón | `F` |
| Enfocar siguiente objeto de la misma familia | Recorre la red sin volver a la vista general | `F` `F` |
| Alternar corte | Plano vertical en el eje, cota 1,35 m | `X` |
| Mover el plano de corte | ±10 cm por muesca | `Shift+rueda` |
| Mover fino | ±1 cm | `Alt+Shift+rueda` |
| Girar el plano | 15° por arrastre | `Alt+arrastrar` |
| Alternar explosión | 0 ↔ 1 con animación | `E` |
| Graduar explosión | ±0,1 | `+` / `-` |
| Recorrer piezas | Resalta y etiqueta una pieza por vez | `Tab` / `Shift+Tab` |
| Salir de un modo | Desapila una capa | `Esc` |
| Salir de todos | Vuelve a la vista anterior a la inspección | `Esc` `Esc` |
| Reencuadrar el objeto | Recalcula el encuadre sin salir del foco | `Espacio` |
| Escala 1:1 en pantalla | 1 m del modelo = 1 m del monitor (según PPP) | `Ctrl+0` |
| Vista de catálogo | Foco + fov 30° + fondo neutro + sin terreno | `Ctrl+1` |

### 1.7 Qué piezas dibuja cada familia

Las partes son las del vocabulario de 06 §1.4; el generador no inventa nombres nuevos.

| Familia | Partes del ensamblaje (06 §1.4) | El corte revela | La explosión revela |
|---|---|---|---|
| **Pozo** | solera, cuerpo, cono, brocal, marco, tapa, peldaños, banqueo, caída, N entradas/salidas | tubos entrando con su DN y su clave, banqueo, peldaños, espesor de pared, caída | orden brocal→marco→tapa, cono→cuerpo→solera |
| **Tubería** | tubo, junta, campana (si `tipo_junta = espiga_campana`), revestimiento | `od_mm`/`id_mm`, junta, cama, clave | campana, empaque, secuencia de montaje |
| **Válvula** | cuerpo, brida, bonete, vástago, volante, disco | disco y `angulo_disco`, paso libre, sentido de flujo | volante, vástago, bonete, brida |
| **Vertedero** | muro, cresta, canal, difusor, enrocado, compuerta | perfil de cresta, `cota_cresta`, `altura_muro` | muro, cresta, enrocado |
| **Tanque** | losa, fuste, cubierta | nivel, espesor, dado de fondo | losa, fuste, cubierta |
| **Llave** | collarín, cuerpo, vástago, volante, caja | paso, asiento | cuerpo, vástago, cuadro |
| **Cámara** | solera, muros, losa, tapa, vanos, deflector, compuerta | solera, resalto, vanos | losa, muros, compuerta |

**Costo de realismo.** Las piezas se generan por **reglas**, no se modelan a mano, y cada regla se
apoya en un parámetro que ya existe en 06: el cono sale de `altura_cono` con talud 1:1,5 entre
`diametro_interior` y `diametro_tapa`; los peldaños van cada 0,30 m según B-ESCALERA; el banqueo es
una media caña de radio `ancho_banqueo`; las campanas aparecen cada 6 m si `tipo_junta =
espiga_campana`; el volante solo se dibuja si `tipo_accionamiento = manual`; el muro de cabeza usa
`altura_muro` y `longitud_vertedero`. Seis reglas cubren las cinco familias que el cliente va a
mirar de cerca, y cualquier familia nueva del catálogo las hereda sin tocar el motor.

---

## 2. Navegación a escala de pieza

### 2.1 Bandas de distancia

| Banda | Distancia | Qué se ve | Qué manda |
|---|---|---|---|
| **B0 Ciudad** | 400 m – 5 km | red completa, terreno, edificios | caja/rombo por nodo, LOD 0 |
| **B1 Barrio** | 60 – 400 m | manzanas, troncal, pozos como cilindros | LOD 1 (12 caras) |
| **B2 Manzana** | 12 – 60 m | pozos con brocal y cono, tubos con juntas | LOD 2 (24 caras) |
| **B3 Pieza** | 1,5 – 12 m | pozo completo, válvula con volante, vertedero con muro | LOD 3 (48 caras + interior) |
| **B4 Detalle** | 0,30 – 1,5 m | junta, tuerca, marca de fundición, peldaño | LOD 4 (chaflanes) |

La banda se muestra en la barra de estado (`B3 · 4,2 m`) y cambia el cursor y la tolerancia de
picking. Al cruzar una banda se **debounce** de 80 ms para no reconstruir la escena en el borde.

### 2.2 Parámetros de cámara

| Parámetro | Hoy (verificado) | Propuesta | Motivo |
|---|---|---|---|
| `distance` mín. | 5,0 m | **0,30 m** | con 5 m no se entra a un pozo de 1,20 m |
| `distance` máx. | 200 000 m | 200 000 m | se queda |
| *near clip* | **0,05 m fijo** (`project`, `project_segment`) | `clamp(0,004·d, 0,010, 6,0)` | near proporcional = precisión relativa constante: a 2 m la pieza no se recorta y a 1 km no se pierde el terreno |
| `fov` | 45° fijo | 45° global · **38° foco** · **30° catálogo** | menos distorsión al leer cotas |
| Órbita | 0,35°/px acimut, 0,28°/px elevación | 0,35 city · **0,55 / 0,45 en pieza** (interpolado por banda) | a 1 m, 0,35°/px obliga a tres arrastres para media vuelta |
| Elevación | 2°–88° | 2°–88°; en foco **8°–80°** | no mirar el pozo de canto |
| `z_scale` | 2,0 / 2,6 fijo | **1,0 en foco**; interpolado 2,6→1,0 entre 120 m y 25 m | una exageración ×2,6 convierte un pozo de 4 m en 10,4 m |
| `pan` | `distance/focal` | igual + `Shift` = ×0,25 | a 2 m el pan de un píxel mueve 2 mm |
| Niebla | siempre | **apagada** por debajo de 40 m | a 2 m la niebla lava el color de la pieza |
| Inercia | ninguna | 120 ms en órbita y pan | el gesto deja de ser «pegajoso» |
| Pivote | siempre `target` | `Alt+clic` pone el pivote bajo el cursor | orbitar alrededor de la junta, no del centro del tubo |

**Regla de seguridad.** El ojo de la cámara nunca queda a menos de **0,25 m** de una cara sólida
(se consulta la cota del terreno y el volumen del objeto enfocado y se empuja la distancia). Con
pintor y sin *depth buffer*, meterse dentro de una pared produce un desastre visual; con esta regla
no ocurre nunca. El interior se ve con **corte**, no atravesando la pared.

### 2.3 Zoom: paso, no factor

Hoy el zoom es `exp(dy · 0,0015)` sin dependencia de la escala: pasar de 200 m a 1 m son ~27
muescas. Propuesta:

| Gesto | Comportamiento |
|---|---|
| Rueda | factor por muesca **continuo por banda**: 1,12× a ≤3 m · 1,22× entre 3 y 50 m · 1,35× por encima de 50 m |
| `Ctrl+rueda` | **1,04×** (afinado: no pasarse de 5 cm al buscar una junta) |
| `Shift+rueda` | **2,0×** (salto de banda) |
| Doble rueda | Salta a la banda siguiente anclada en el cursor |
| `Ctrl+0` | **Escala 1:1**: 1 m del modelo = 1 m físicos de pantalla, corregido por `Env.scale` (PPP). El chip muestra `1:1 · 96 ppp` |
| `Ctrl+1` | Vista de catálogo (foco, fov 30°, fondo neutro, sin terreno ni edificios) |

El zoom es **siempre al cursor**, no al centro: a escala de pieza, el cursor es la intención.

**Detalle fino.** A 0,30 m el paso de 1,04× mueve 12 mm por muesca; con `Ctrl+0` el usuario
comprueba el tamaño real y con `Ctrl+rueda` lo ajusta. No hace falta un campo numérico de distancia
en la UI; el chip de banda ya informa (`B4 · 0,42 m`).

### 2.4 Picking a 1 m

Hoy: una sola tolerancia (18 px nodos, 14 px arcos), el nodo se prueba **solo a cota de tapa** y el
arco **solo por el segmento de extremo a extremo** (`State::pick`). A escala de pieza eso falla de
tres maneras: no se puede seleccionar el vértice de un colector con quiebre, se selecciona el pozo
cuando se apunta al tubo que pasa por delante, y no hay forma de elegir entre dos objetos alineados.

| Parámetro | Valor | Nota |
|---|---|---|
| Tolerancia de símbolo | `tol = 10 px + 8 px · (1 − d/60)` acotada a `[10, 18]` | a 1 m, 10 px es 1 cm de mundo: precisión de dedo, no de ratón |
| Tolerancia de línea | 7 px a B3/B4, 14 px a B0 | el tubo se elige por su **eje proyectado completo**, no por el segmento extremo |
| Tolerancia de pieza | 7 px | solo en corte/explosión |
| Prioridad | **dispositivo > nodo > arco > anotación** | fija; cambiarla exige test nuevo |
| Desempate | menor distancia en pantalla; si empatan a < 3 px, gana el de **menor profundidad** | coincide con lo que tapa |
| Ambiguos | `Alt+clic` cicla los candidatos bajo el cursor (chip `2 de 3`) | resuelve el solape sin adivinar |
| Atravesar | `Alt+clic` mantenido con la capa en rayos X selecciona lo tapado | coherente con el modo rayos X existente |
| Cursor | `Cursor::Crosshair` sobre el lienzo en B2–B4 | el cursor de flecha no señala un punto |
| Punto de mira | Cruz de 1 px con hueco de 3 px dibujada solo si `d < 3 m` | señala el píxel exacto sin tapar la pieza |
| Halo de tolerancia | Anillo de radio `tol` durante 400 ms tras `Alt` | enseña la tolerancia; se apaga solo |

**Regla de selección a escala de pieza.** Un clic en el brocal, en el cono o en un peldaño
selecciona **el pozo**. Las piezas no son seleccionables como objetos (solo con `Tab` en
explosión, y en modo lectura). Si el usuario quiere el tubo, apunta al tubo; el tubo tiene su
propia tolerancia y su prioridad menor que el nodo, pero su eje es más largo, así que gana en el
90 % de los clics reales.

### 2.5 z-fighting del pintor: seis reglas

El motor ordena de lejos a cerca por la **profundidad del centroide** y dibuja la red **después**
del terreno (rayos X: `view.rs`, líneas 597-607). A 1 m eso produce tres artefactos clásicos:
tapa y brocal parpadean, el tubo atraviesa la pared, los peldaños se hunden en el muro.

| # | Regla | Cómo |
|---|---|---|
| **1** | **Sesgo relativo por capa de pieza** | `profundidad_de_orden = profundidad · (1 − 0,0015 · capa)` con capas 0…8 (terreno 0, solera 1, cuerpo 2, cono 3, brocal 4, tapa 5, tubos 6, peldaños 7, gizmos 8). Es **relativo**, así funciona igual a 1 m que a 1 km. Máximo 1,2 % de separación |
| **2** | **No coplanaridad de 2 mm** | Regla de autoría del generador: dos piezas nunca comparten plano; el ensamblaje deja 2 mm de luz. Es la regla que evita el 80 % de los casos |
| **3** | **Corte en la intersección** | Un tubo que entra en el pozo se parte en el plano de la pared y cada mitad lleva su propia profundidad. Nunca se dibuja un tubo que atraviesa una cara |
| **4** | **Calado de terreno (cookie cut)** | En foco, se eliminan los triángulos del TIN dentro del círculo de la tapa y se añade un anillo interior (0,15 m) para que el hueco no se vea hueco. Así se mira **dentro** del pozo sin plano de corte, y el terreno no tapa el objeto enfocado |
| **5** | **Silueta de contacto** | Contorno de 1 px, 25 % más oscuro, en la base de cada pieza. Tapa las costuras residuales de 1 px sin coste |
| **6** | **Cero alfa en detalle** | A B3/B4 toda la geometría de pieza es opaca. La única transparencia permitida es la mitad fantasma del corte (18 %), que se dibuja **al final** y sin ordenar |

**Troceado.** Un tubo largo ordenado por centroide se ve mal desde un extremo. A B3/B4 los tubos se
trocean en segmentos de **≤ 0,30 m de profundidad proyectada** (el mismo truco que ya usa `TROZO =
12,0` para las manzanas).

**Plan B (si el pintor no alcanza).** Un *z-buffer* de software de 512×512 **solo para el objeto
enfocado** (~3 ms): resuelve la oclusión exacta del ensamblaje sin tocar el resto del pipeline. No
lo propongo de entrada porque las seis reglas cubren el caso; queda como palanca si la escena
`pieza-pozo` no pasa la prueba visual.

### 2.6 `z_scale`: la trampa de la exageración

La app aplica `z_scale = 2,0` (o `2,6` con entorno urbano) a **toda** la geometría, escalando las
alturas alrededor de la cota del objetivo. En ciudad es correcto: el relieve se lee. En detalle es
un error de lectura: un pozo de 4 m parece de 10,4 m y una tapa de 0,15 m parece de 0,39 m.

| Situación | `z_scale` | Aviso |
|---|---|---|
| B0/B1 | 2,0 / 2,6 (como hoy) | chip en la barra de estado: `alturas ×2,6` |
| Transición 120 → 25 m | interpolación lineal 2,6 → 1,0 | el chip parpadea a `alturas ×1,3` |
| Foco / B3 / B4 | **1,0 exacto** | chip fijo `escala real · cotas reales` |

Las etiquetas de cota **siempre** muestran el valor del modelo, nunca el escalado. Si el usuario
pregunta «¿por qué el pozo se ve más bajo ahora?», el chip responde: `escala real`.

---

## 3. Etiquetas y anotaciones

### 3.1 Contenido por familia

Dos niveles: **globo** (hover, 2 datos) y **etiqueta fija** (selección, línea de anclaje). Nunca se
muestra todo: el globo es la pregunta, la ficha es la respuesta.

| Familia | Globo (hover) | Etiqueta fija (selección) |
|---|---|---|
| Pozo | `PZ-001 · Øi 1,20` | `PZ-001 · inspección · Øi 1,20 m · tapa 2512,34 · clave 2510,10 · prof. 2,24 m` |
| Tubería | `CL-014 · DN 200` | `CL-014 · colector · DN 200 · L 42,63 m · S 8,4 ‰ · clave 2510,10 → 2509,74` |
| Válvula | `VL-003 · compuerta` | `VL-003 · compuerta · DN 150 · PN 100 · abierta` |
| Llave | `LL-007 · hidrante` | `LL-007 · hidrante · DN 100 · caudal 2,5 l/s` |
| Vertedero | `DS-001 · alivio` | `DS-001 · alivio · cresta 2508,90 · L 3,00 m · entrega 2508,50` |
| Tanque | `TQ-001 · apoyado` | `TQ-001 · apoyado · V 250 m³ · NAMO 2515,20 · fondo 2509,00` |
| Cámara | `CM-002 · reunión` | `CM-002 · reunión · 3 aportes · solera 2507,80` |
| Cualquiera en corte | — | añade `· corte a 1,35 m` al final de la fija |
| Cualquiera en explosión | nombre de pieza | `cono · h 0,90 m` (etiqueta de pieza, no de objeto) |

**Regla de las dos líneas.** El globo nunca pasa de 34 caracteres; la etiqueta fija, de 90. Si un
objeto necesita más, no es etiqueta: es la ficha (§5). Esto es la misma disciplina de 140 caracteres
que ya fija el documento 05 para los tooltips.

### 3.2 Cuándo aparece

| Condición | Qué se ve |
|---|---|
| Hover 180 ms | globo, con línea de anclaje de 1 px y fondo tipo píldora |
| Hover < 180 ms | nada (evita el parpadeo al barrer la pantalla) |
| Selección | etiqueta fija + globo del objeto bajo el cursor si lo hay (máx. 2 etiquetas) |
| B3/B4 con < 12 objetos en pantalla | etiqueta fija automática de todos (es una escena de detalle: hay sitio) |
| B0/B1 | **nada** salvo chinchetas y el seleccionado. A 1 km las etiquetas son ruido |
| Chincheta (`Shift+clic` en la etiqueta) | siempre visible, hasta 20 por proyecto, se guardan en la vista |
| Objeto observado | globo persistente 2 s al entrar en la escena, luego chincheta automática |
| Modo tour | las etiquetas las enciende el guion, no el cursor |

### 3.3 Evitar el amontonamiento

Cuatro reglas, en este orden:

1. **Prioridad fija.** seleccionado (100) > hover (90) > observado (70) > chincheta (60) > objeto
   grande (40) > resto (10). Con presupuesto de **250 etiquetas** (el que ya fija el documento 04).
2. **Carriles.** Las etiquetas se colocan en 6 carriles verticales de 14 px a la derecha del ancla.
   Si el carril está ocupado, baja al siguiente. Una etiqueta nunca tapa su propio ancla.
3. **Colapso.** Si un objeto no cabe en ningún carril, su etiqueta se convierte en un **chip de
   conteo** (`+7`) en el borde del cúmulo; al pasar el cursor, se despliegan sus 7 etiquetas en
   abanico. El chip es un botón: al pulsarlo se aísla el cúmulo.
4. **Separación mínima.** Dos cajas de etiqueta no pueden solaparse más de **2 px²**; si el
   algoritmo no encuentra hueco, se descarta la de menor prioridad antes que moverla encima de otra.

A escala de pieza hay una quinta regla: las etiquetas de **pieza** (brocal, cono, cuerpo…) van
ancladas con línea y **no compiten** con las de objeto, porque solo aparecen en corte/explosión.

### 3.4 Unidades y procedencia

| Aspecto | Decisión |
|---|---|
| Unidad | Siempre visible, con espacio fino (`1,20 m`, `DN 200`, `8,4 ‰`). Nunca `1.20m` |
| Decimales | Cotas 3 · longitudes 2 · DN entero (mm) · pendiente 1 decimal en ‰ · caudales 1 en l/s |
| Separador decimal | El del proyecto (`formato.decimal`, por defecto `,` en es-EC) |
| Cifras | Ancho tabular y alineación a la derecha de los números: la etiqueta no «baila» al cambiar de objeto |
| Pendiente | En **‰**, no en %: un 0,8 % se lee `8,0 ‰` sin ambigüedad |
| Diámetro | `Øi` = interior, `Øe` = exterior, `DN` = nominal. Tres cosas distintas, tres etiquetas |
| Procedencia | Un glifo por dato, y solo si **no** es dato de diseño del proyecto: `†` importado · `≈` derivado · `◇` supuesto (default aplicado) · `?` faltante |
| Detalle | Al pasar el cursor sobre el glifo: píldora con `app · archivo · formato · fecha` (de `Dato.fuente`). Máx. 140 caracteres |
| Sin glifo | Dato de diseño del proyecto, con su revisión |

**Por qué glifos y no colores.** Cuatro estados de procedencia por color es ilegible para un
daltónico y para una impresión en blanco y negro. Un glifo sobrevive a todo. La leyenda de glifos
vive al pie del inspector y en el bloque de procedencia de la ficha.

### 3.5 Tabla de interacción

| Acción | Resultado | Atajo |
|---|---|---|
| Pasar el cursor | Globo a los 180 ms | — |
| Clic | Etiqueta fija + ficha | clic |
| Fijar etiqueta | Chincheta permanente (máx. 20) | `Shift+clic` en la etiqueta |
| Quitar chincheta | Desancla | `Shift+clic` de nuevo |
| Ver procedencia | Píldora con fuente y fecha | hover sobre el glifo |
| Ocultar etiquetas | Apaga todas menos la selección | `Ctrl+L` |
| Desplegar cúmulo | Abanico de etiquetas del chip | clic en `+N` |
| Aislar el cúmulo | Solo esos objetos | `Alt+clic` en `+N` |

---

## 4. Leyenda y simbología

### 4.1 Familia = forma (nunca solo color)

| Familia | Símbolo | Forma base | En 3D |
|---|---|---|---|
| Pozo | ◎ | círculo con anillo | cilindro con brocal y cono |
| Tubería | —·— | línea con muescas de junta | tubo con campana |
| Válvula | ⌧ | cuadrado con aspa | cuerpo con volante |
| Llave | △ | triángulo | cuerpo con cuadro |
| Vertedero | ⏢ | trapecio | muro de cabeza |
| Tanque | ⬡ | hexágono | losa y muro |
| Medidor | ▣M | cuadrado con M | caja con esfera |
| Cámara | ▣· | cuadrado con punto | caja con tapa |
| Bombeo | ▣→ | cuadrado con flecha | caja con grupo |
| Hito / BM | ✛ | cruz | marca en el terreno |
| Cruce | ⧉ | dos cuadrados | — |

El color sigue siendo redundante y útil: **agua = azul** (`0,33 0,63 0,96`), **alcantarillado =
ocre** (`0,87 0,60 0,31`), los mismos que ya usa `view.rs`. Pero la forma distingue las familias
aunque el monitor esté en blanco y negro o el usuario sea daltónico.

### 4.2 Estado = patrón de borde + insignia

| Estado | Borde del símbolo | Insignia | Texto en ficha | En 3D |
|---|---|---|---|---|
| **Aprobado** | continuo, 1,5 px | ✓ (12 px, esquina) | «Aprobado» | contorno continuo |
| **Pendiente** | **discontinuo 6/4** | ○ | «Pendiente» | contorno discontinuo |
| **Observado** | **doble** (2 líneas) | `!` | «Observado» | contorno doble + trama diagonal 45° 4 px sobre el cuerpo |
| Sin revisar | ninguno | — | «Sin revisar» | sin contorno |

Los colores de estado se mantienen (verde/ámbar/rojo, los `OK`/`WARN`/`ERR` de `main.rs`) pero
**nunca son el único canal**. Prueba de aceptación: una captura en escala de grises debe seguir
distinguiendo los tres estados y las siete familias.

Contraste mínimo: trazos ≥ 3:1 contra el fondo `0,062`; texto ≥ 4,5:1. La trama de «observado» va
al 30 % de opacidad para no ensuciar la pieza a 1 m.

### 4.3 Capas apagadas

| Situación | Leyenda | Escena |
|---|---|---|
| Capa encendida | Fila normal, contador activo | Objetos visibles, seleccionables, etiquetables |
| Capa apagada | Fila **atenuada al 40 %** con la casilla vacía y el rótulo `apagada` | Objetos no dibujados, no seleccionables, no etiquetables |
| Familia sin objetos en el proyecto | Fila oculta | — |
| Capa `nodos` apagada, `alcantarillado` encendida | Las familias de nodo aparecen atenuadas; las de arco siguen normales | Tubos sin pozos |
| Todas las capas apagadas | Cabecera `capas apagadas (5)` desplegable | Lienzo con el terreno solo si está encendido |

**Por qué atenuar y no ocultar.** La leyenda es un mapa del proyecto, no un reflejo de lo visible.
Si las filas desaparecen al apagar la capa, el usuario cree que la familia se borró. Atenuar dice
la verdad: «existe, está apagada».

Al apagar una capa, los contadores de la izquierda (`aprobados / pendientes / observados`) se
recalculan **sin** esa capa y muestran el valor anterior entre paréntesis: `12 (18)`. Así el
revisor no pierde de vista el total del proyecto.

### 4.4 La leyenda como filtro

| Acción | Resultado | Atajo |
|---|---|---|
| Clic en fila | **Aísla** la familia (las demás al 15 %, no seleccionables) | clic |
| `Alt+clic` | Oculta esa familia (no dibuja) | `Alt+clic` |
| `Shift+clic` | Añade la familia al conjunto aislado | `Shift+clic` |
| Clic en cabecera de grupo | Aísla el grupo (Saneamiento / Agua / Comunes) | clic |
| `Esc` | Limpia el aislamiento | `Esc` |
| Botón `Ver todas` | Devuelve la leyenda completa al proyecto | — |

El aislamiento se anuncia en el lienzo con un chip (`solo Tubería · 3 412`) que se cierra con una
`×`. Nunca se queda «pegado» sin que el usuario lo vea.

### 4.5 Modo revisión

`R` activa el **modo revisión**: los aprobados pasan a gris al 35 % y solo pendientes y observados
conservan color y contorno. Es la vista que usa P2 (municipal) todo el día y no existía.

| Regla | Valor |
|---|---|
| Aprobados | gris 0,45, 35 % de opacidad, sin etiquetas, no seleccionables (salvo `Alt+clic`) |
| Pendientes y observados | normales, con etiqueta fija si caben |
| Contador | `R · 86 aprobados atenuados` en la barra de estado |
| Salida | `R` de nuevo |

En modo revisión el tour no entra (el tour es promoción; la revisión es trabajo).

### 4.6 Leyenda mínima en foco

Al entrar en foco, la leyenda se colapsa a **tres filas** (familia del objeto, estado, glifos de
procedencia) con un enlace `ver todas`. A 1 m, una lista de 20 familias es ruido; lo que el
ingeniero necesita es saber qué está mirando.

---

## 5. Panel de familia (ficha paramétrica)

### 5.1 Estructura de la ficha

El inspector pasa de una lista plana a cuatro pestañas:

| Pestaña | Contenido | Cuándo |
|---|---|---|
| **Ficha** | Identidad, las cuatro cotas, parámetros que la generan, piezas derivadas | Al seleccionar |
| **Datos** | Atributos con unidad y procedencia, agrupados como en `families.rs` | Al seleccionar |
| **Revisión** | Estado, nota, historial de revisiones del objeto, botones Aprobar/Observar | Al seleccionar |
| **Historial** | Journal del proyecto filtrado a este objeto | Bajo demanda |

Cabecera fija de la Ficha:

```
PZ-001                              [Aprobado ✓]
Pozo · inspección · saneamiento.pozo · id 4f2a
```

Y un bloque **«Piezas derivadas»** que hace visible la causalidad (es lo que vende el paramétrico):

```
brocal · marco · tapa · cono · cuerpo · solera · banqueo · 9 peldaños · 3 entradas
```

### 5.2 Qué se edita, qué se recalcula, qué se bloquea

Tres clases, con su marca visible en el campo:

- **E** — editable: recalcula la geometría en vivo.
- **R** — derivado: se muestra en gris, con `calc.` y sin caja de edición. Cambia al cambiar un **E**.
- **B** — bloqueado: se muestra con candado y **el motivo en una línea** al pasar el cursor.

Los nombres, unidades y rangos son los de `06-catalogo-familias.md` §3. La ficha **no** añade
campos: si un parámetro no está en 06, no se puede editar aquí.

**Pozo**

| Campo | Clase | Rango (06) | Nota |
|---|---|---|---|
| `diametro_interior` | **E** | 0,90 / 1,20 / 1,50 (hasta 3,00) | cambia cuerpo, cono, solera y el radio de las bocas |
| `espesor_pared` | **E** | 0,10 – 0,30 m | cambia el anillo exterior |
| `espesor_solera` | **E** | 0,15 – 0,30 m | baja la solera sin mover `cota_solera` |
| `altura_cono` | **E**/R | 0,45 – 0,90 m | si falta, derivada del talud 1:1,5 y marcada `◇` |
| `diametro_tapa` | **E** | 0,60 / 0,70 m | fija el cono y el marco |
| `altura_brocal` | **E** | 0,10 – 0,30 m | 0,20 en calzada |
| `ancho_brocal` | **E** | 0,15 – 0,30 m | cambia el ancho del brocal |
| `tipo_tapa` | **E** | fundición · hormigón · rejilla · ciega | tapa y marco (bloque B-TAPA) |
| `ancho_banqueo` | **E** | 0,15 – 0,60 m | por defecto = DN de salida |
| `altura_banqueo` | **E** | 0,05 – 0,30 m | 0,10 por defecto |
| `numero_peldanos` | **E**/R | 1 – 12 | si falta, ⌊(`profundidad_total` − 0,40)/0,30⌋, máx. 12 |
| `altura_caida` | **E** | 0,30 – 5,00 m | solo subtipos de caída; si falta, se marca `?` |
| `cota_tapa` | **E**/B | — | **B** si viene de levantamiento y el objeto está aprobado |
| `cota_solera` | **E**/B | — | **B** si la manda la clave de salida |
| `cota_terreno` | **B** | — | dato de levantamiento; se corrige en el origen |
| `altura_cuerpo` | **R** | — | `profundidad_total − altura_cono − espesor_solera` |
| `profundidad_total` | **R** | 0,60 – 12,0 m | `cota_terreno − cota_solera` |
| `area_infiltracion` | **R** | — | de la geometría del fuste |

**Tubería**

| Campo | Clase | Rango (06) | Nota |
|---|---|---|---|
| `dn_mm` | **E** | 75 – 1 200 mm | cambia radio, junta y boca en el pozo |
| `espesor_pared` | **E** | 2 – 80 mm | `od_mm` e `id_mm` son **R** |
| `material` | **E** | PVC · PEAD · HG · HD · hormigón · GRC · acero · mampostería | cambia acabado |
| `tipo_junta` | **E** | espiga_campana · bridada · soldada · electrofusión · mecánica | decide si hay campanas (paso 6 m) |
| `seccion` | **E** | circular · ovoide · herradura · rectangular | cambia la forma de la sección de corte |
| `pendiente` | **E** | 0,001 – 0,05 m/m | se edita en ‰ y se guarda en m/m; recalcula la clave de salida |
| `cota_clave_inicio` | **E**/B | — | **B** si la fija el pozo aguas arriba |
| `cota_clave_fin` | **R** | — | `cota_clave_inicio − pendiente · longitud` |
| `od_mm`, `id_mm` | **R** | — | de `dn_mm` y `espesor_pared` |
| `longitud`, `tirante`, `velocidad` | **R** | — | derivados; nunca editables aquí |

**Válvula**

| Campo | Clase | Rango (06) | Nota |
|---|---|---|---|
| `dn_mm` | **E** | 25 – 1 200 mm | cambia cuerpo, bridas y volante |
| `tipo_cuerpo` | **E** | husillo_ascendente · husillo_no_ascendente · wafer · lug · doble_brida · globo · angular | cambia la silueta entera |
| `tipo_conexion` | **E** | bridada · roscada · espiga · ranurada · wafer | fija `numero_bridas` |
| `tipo_accionamiento` | **E** | manual · eléctrico · hidráulico · neumático | volante solo si manual; si no, caja de motor |
| `diametro_volante` | **E**/B | 100 – 600 mm | **B** si no es manual (no hay volante) |
| `angulo_disco` | **E** | 0 – 90° | mariposa; gira el disco en la vista |
| `posicion` | **E** | abierta · cerrada · parcial | mueve el disco y las vueltas del volante |
| `longitud_cara_a_cara` | **B** | 150 – 1 200 mm | dato de catálogo del fabricante; cambiarlo es cambiar de modelo |
| `altura_total`, `diametro_cuerpo` | **R** | — | de `dn_mm` y del tipo |

**Vertedero**

| Campo | Clase | Rango (06) | Nota |
|---|---|---|---|
| `tipo_entrega` | **E** | muro · talud · subacuática · infiltración | cambia el conjunto entero |
| `longitud_vertedero` | **E** | 0,30 – 20,0 m | ancho del muro de cabeza |
| `altura_cresta` | **E** | 0,10 – 2,00 m | sobre `cota_entrega` |
| `altura_muro` | **E** | 0,10 – 2,00 m | solo `tipo_entrega = muro` |
| `tipo_cresta` | **E** | delgada · ancha · redondeada | perfil de la cresta |
| `cota_entrega` | **E** | — | obligatorio; sin él no hay vertedero |
| `cota_cresta` | **R** | — | `cota_entrega + altura_cresta`; editable si viene de diseño |

### 5.3 Qué se recalcula solo (y en qué orden)

Al cambiar un campo **E**, se recalcula en cascada y **solo** lo que depende de él:

```
parámetro → piezas del objeto → bocas de los tubos conectados (radio) → etiqueta del objeto
          → totales de la ficha (volumen, longitud) → contador de la leyenda
```

Lo que **no** se toca nunca desde un parámetro de pieza: cotas de otros objetos, topología, códigos,
estado de revisión y procedencia. Si el usuario cambia el DN de un tubo, la boca del pozo se
agranda, pero la cota clave del pozo no se mueve.

**Caché.** La geometría de pieza se memoiza por `(familia, subtipo, hash(parámetros), LOD, modo)`.
Arrastrar un deslizador de ida y vuelta no regenera: acierta en caché el 70 % de los frames.

### 5.4 Validación en vivo

| Momento | Qué pasa |
|---|---|
| Al teclear | Validación **suave**: borde del campo en `WARN` y mensaje bajo el campo; la pieza se ve con el valor provisional |
| Fuera de rango | El campo se marca y la pieza se dibuja **en `WARN` y semitransparente**; no se confirma |
| Falta un obligatorio | Bloquea `Aprobar` de ese objeto y muestra `Falta el diámetro interior` con botón `Completar` |
| Al confirmar (`Enter`/blur) | Validación **dura**: o se aplica (una entrada de journal) o se revierte al último valor válido con aviso |
| Cruce de regla | `dn > Øi del pozo` → bloqueado con explicación y sugerencia (`subir Øi a 1,35 m`) |

Los límites vienen de `AttrDef::bounds`/`positive` de `families.rs`; la UI **no** inventa rangos.

### 5.5 Deshacer y trazabilidad

| Acción | Resultado | Atajo |
|---|---|---|
| Deshacer | Revierte el último cambio (parámetro, estado, nota) | `Ctrl+Z` |
| Rehacer | Vuelve a aplicarlo | `Ctrl+Shift+Z` |
| Deshacer agrupado | Un arrastre de deslizador = **una** entrada (coalescencia 600 ms) | `Ctrl+Z` |
| Revertir el objeto | Vuelve a los valores de ingesta y lo marca `origen: importado` | botón `Revertir a como vino` |
| Ver el cambio | La ficha muestra `1,20 → 1,50 m · ui.familia.diametro_interior · rev 42` | — |

Cada confirmación escribe una entrada de journal (`ui.familia.<param>`) con la revisión, así que el
cambio es auditable y exportable: el revisor municipal puede ver **qué** se cambió después de
aprobar. Pila de 50 entradas por sesión.

El aviso de deshacer es un *toast* no bloqueante: `Diámetro interior 1,20 → 1,50 m · Deshacer
(Ctrl+Z)`, 6 s.

### 5.6 La puerta de aprobación

Aprobar **congela**. Si la pieza está `Aprobado` y se intenta cambiar un parámetro geométrico, no se
edita en silencio: aparece una decisión.

| Opción | Efecto |
|---|---|
| **Cambiar y reabrir** | Aplica el cambio, el objeto vuelve a `Pendiente`, se registra en el journal |
| **Duplicar como variante** | Crea una copia `PZ-001b` con el cambio; la original sigue aprobada |
| **Cancelar** | Nada cambia |

Esto evita el peor escenario de una revisión: que la pieza aprobada y firmada cambie de diámetro sin
que nadie lo note. Y es una decisión de producto, no técnica: la elevo al consejo en §10.

---

## 6. Tour de 60 s

### 6.1 Guion

Objetivo doble: que el promotor lo grabe sin editar y que el ingeniero aprenda el producto mirando.
Nueve planos: corte seco al cambiar de objeto (planos 2, 4, 6 y 9) y movimiento continuo dentro de
cada objeto.

| t (s) | Plano | Cámara / modo | En pantalla | Subtítulo (≤ 60 car.) |
|---|---|---|---|---|
| 0–7 | Ciudad | Órbita lenta 8°/s, `d` 1,8 km → 1,2 km | 4 capas, ~1 400 objetos, red bajo la ciudad | «El proyecto completo, en 3D» |
| 7–14 | Lo que el plano no muestra | Apagar Edificios, dolly a 300 m | Rayos X: tuberías enterradas, pozos | «La red bajo la calle» |
| 14–22 | Pozo `PZ-001` | Doble clic → foco (3,6 m) → corte a 1,35 m | Tubos entrando, banqueo, peldaños, cotas | «Pozo: cono, brocal y banqueo» |
| 22–29 | Despiece | Explosión 0 → 1 en 1,2 s, órbita 15° | 8 piezas etiquetadas | «Las piezas, en orden de montaje» |
| 29–36 | Tubería `CL-014` | Foco a 4 m, cámara al ras de la junta | DN 200, juntas cada 6 m, cama | «Cada tubo con su junta» |
| 36–43 | Paramétrico | Deslizador DN 200 → 250 en la ficha | El tubo engorda en vivo, la junta se recalcula | «El diámetro se cambia en vivo» |
| 43–50 | Válvula `VL-003` | Foco 1,8 m + corte por el eje | Volante, vástago, disco girando 90° | «Válvula: volante, vástago y disco» |
| 50–57 | Vertedero `DS-001` | Foco 6 m + corte por el muro | Muro de cabeza, cresta, disipador | «Vertedero: muro de cabeza» |
| 57–60 | Cierre | Vuelta a ciudad (fit) y fundido a placa | `12 observados · 0 errores · Exportar DXF` | «Aprobado. Listo para DWG.» |

**Variante de 90 s** (para sala de ventas): añade un plano de revisión entre 50 y 57 — activar `R`,
observar un pozo, ver subir el contador y la nota en la ficha. Es el único plano que muestra el
trabajo real del municipal; en 60 s no cabe sin romper el ritmo.

### 6.2 Formato del guion

El tour se escribe con el **mismo formato** que ya usan `docs/demo/recorrido.json` y
`recorrido-ciudad.json` (`{ "t", "type", ... }`), extendido con cuatro tipos nuevos. Así el tour es
un dato versionado, reproducible en CI y grabable con el mismo mecanismo que las demos actuales.

| `type` | Campos | Efecto |
|---|---|---|
| `click` | `key` | Pulsa un control por su `key` (los `btn-*` existentes) |
| `drag` | `x`, `y`, `dx`, `dy`, `dur` | Órbita o pan |
| `wheel` | `x`, `y`, `dy` | Zoom al cursor |
| `focus` `[NUEVO]` | `code`, `dur` | Foco por código de objeto |
| `modo` `[NUEVO]` | `nombre` (`corte`/`explosion`), `on`, `valor` | Activa o gradua un modo |
| `caption` `[NUEVO]` | `texto`, `dur` | Subtítulo inferior con fundido |
| `ficha` `[NUEVO]` | `campo`, `valor`, `dur` | Edita un parámetro con la animación de deslizador |

Ejemplo de dos entradas (no es el guion completo):

```json
[{"t": 14.0, "type": "focus", "code": "PZ-001", "dur": 0.45},
 {"t": 15.0, "type": "modo", "nombre": "corte", "on": true, "valor": 1.35}]
```

**Comandos y teclas.**

| Acción | Atajo / comando |
|---|---|
| Iniciar el tour | `T` · `redes-app --tour catalogo` |
| Pausar / seguir | `Espacio` |
| Plano anterior / siguiente | `←` / `→` |
| Salir | `Esc` |
| Grabar el tour a vídeo | `redes-app --tour catalogo --grabar tour.mp4` |
| Tour sin interfaz (solo lienzo) | `--tour catalogo --limpio` |

### 6.3 Reglas de presentación

| Regla | Valor |
|---|---|
| Proyecto | Se ejecuta sobre una **copia de solo lectura**: el tour no escribe ni cambia estados |
| Interrupción | Cualquier clic del usuario pausa y entrega el control; no secuestra el ratón |
| Movimiento | Easing 450 ms dentro de un plano; corte seco al cambiar de objeto (planos 2, 4, 6 y 9) |
| Accesibilidad | Respeta «reducir movimiento»: los 450 ms pasan a 80 ms de fundido |
| Texto | ≥ 14 pt, subtítulo a 90 px del borde inferior, contraste ≥ 4,5:1 |
| Velocidad | 30 fps mínimo, 60 fps objetivo; si baja de 24 fps, baja un LOD antes que tartamudear |
| Cursor | Oculto salvo en los planos 6 y 7 (donde el gesto es el mensaje) |
| Sonido | Sin audio. El tour se entiende en silencio |
| Marca | Sin logotipos ni claims: el producto se vende solo con la geometría |

---

## 7. Rendimiento percibido

### 7.1 Presupuesto por gesto

Objetivo: **60 fps** (16,6 ms) con una pieza en B3; **nunca** bajar de 30 fps (33 ms) en un gesto
interactivo.

| Gesto | ¿En vivo? | Coste esperado | Estrategia |
|---|---|---|---|
| Órbita | **Sí**, siempre | 3–6 ms | Escena memoizada; solo reproyecta; sin re-teselar. `live_only` mientras dura el arrastre |
| Zoom | **Sí** | 3–8 ms | Re-LOD solo al cruzar banda (debounce 80 ms) |
| Pan | Sí | 3–5 ms | Igual que órbita |
| Hover / etiquetas | Sí | 0,2–0,6 ms | Decluttering voraz, presupuesto 250 |
| Picking | Sí (al clic) | 0,3 ms con 5 000 objetos | Rejilla espacial propia solo si se supera 1 ms (hoy es lineal) |
| Cambio de capa | **No** (reconstrucción) | 40–180 ms | Escena anterior visible + fundido; chip de progreso si > 120 ms |
| Cambio de parámetro | **Sí** (arrastre) | 1–4 ms | Solo se regenera **la pieza enfocada**; el resto sale de caché por revisión |
| Cambio de parámetro (confirmado) | Sí | 6–14 ms | Refina a LOD 3/4 una vez, al soltar |
| Corte | Sí (al mover) | 4–9 ms | El plano se aplica a ≤ 40 objetos del radio de corte; el terreno se recorta en una sola pasada |
| Explosión | Sí (al graduar) | 2–5 ms | Solo traslada matrices de pieza; no regenera geometría |
| Foco (transición) | Sí | 1 ms/frame | Interpola cámara; la geometría de pieza se genera una vez al llegar |

### 7.2 Qué se difiere (y por qué es aceptable)

| Se difiere | Cuándo | Motivo |
|---|---|---|
| Juntas de tubería | Fuera de 200 m del objetivo | A 1 km no se ven; cuestan la mitad de los triángulos de la red |
| LOD de detalle de los vecinos | Objetos a más de 20 m del foco | En foco solo importa uno; los vecinos sirven de contexto |
| Re-*thinning* del terreno | Solo al entrar en B3, en un radio de 60 m | El TIN se aligera al cargar; en detalle se necesita el original en una ventana |
| Sombras y oclusión ambiental | Nunca | No aportan lectura técnica y cuestan 2–4× el frame |
| Refracción / caudal animado | Nunca en v1 | El agua es un material, no una simulación; el caudal es un dato, no una animación |
| Etiquetas de objeto en B0/B1 | Nunca automáticas | El decluttering no salva 1 400 etiquetas: no se intenta |

### 7.3 Cómo se comunica el progreso

| Duración | Qué ve el usuario |
|---|---|
| < 120 ms | **Nada**. Un *spinner* que aparece y desaparece es peor que la espera |
| 120–400 ms | Chip discreto arriba a la derecha del lienzo: `generando piezas…`. No bloquea ni tapa |
| 400 ms–2 s | Barra de progreso en el chip con la **fase concreta**: `recortando terreno (2/3)`. `Esc` cancela |
| > 2 s | Pasa a trabajo en segundo plano; aviso al terminar. La vista sigue navegable |
| Error | Nunca modal por un objeto (§8); chip con `!` y el número de vacíos |

Durante cualquier espera, la **última escena válida** permanece en pantalla. Nunca hay destello
blanco ni lienzo vacío: es la diferencia entre «va lento» y «se rompió».

### 7.4 Degradación automática

La barra de estado muestra `FPS · ms del último rebuild · triángulos · banda · LOD` (ya existe el
FPS en el diseño de 04; se añaden las otras columnas).

| Condición | Reacción |
|---|---|
| 3 frames seguidos > 33 ms | Baja **un** nivel de LOD y muestra el chip `detalle reducido · restaurar` |
| Vuelve a < 20 ms durante 2 s | Sube el LOD y retira el chip |
| > 200 ms de reconstrucción | Se salta el fundido y se pinta directo |
| Memoria de escena > 250 MB | Purga la caché de piezas no enfocadas |

El usuario siempre puede forzar el máximo: `Alt+clic` en el chip de LOD fija el nivel y desactiva la
degradación automática (para la captura de promoción).

---

## 8. Errores y vacíos

### 8.1 Principio: no inventar

**Un parámetro que falta no se sustituye por un número silencioso.** Se dibuja un **proxy honesto**:
caja de alambre con aristas discontinuas y una `×` en las caras, color neutro (gris 0,45), del
tamaño que permitan los datos disponibles. La pieza existe en la escena —para que el usuario sepa
dónde está— pero se ve que no está resuelta.

Y tres reglas duras:

1. **Nunca a cota 0.** Un objeto sin cota se ancla a la cota del terreno en su posición, o no se
   dibuja. Cota 0 es una mentira que se propaga al DXF.
2. **Nunca un default invisible.** Todo default aplicado lleva `◇` y suma al contador `supuestos`.
3. **Nunca un modal.** Un objeto ilegible no interrumpe la revisión de los otros 1 399.

### 8.2 Qué se muestra cuando falta algo

| Falta | Qué se dibuja | Insignia | Mensaje (≤ 140 car.) |
|---|---|---|---|
| `diametro_interior` (pozo) | Caja de alambre de 1,00 m de lado, del alto de la profundidad conocida | `?` | «Falta el diámetro interior: no se puede dibujar el pozo. [Completar]» |
| `cota_tapa` y `cota_terreno` | **Pin de posición**: línea discontinua vertical de 2 m sobre el terreno, con rombo | `?` | «Sabemos dónde está, no a qué cota. [Completar cota]» |
| `cota_tapa` pero hay `profundidad` | Pozo completo con la tapa derivada | `≈` | «Cota de tapa derivada del terreno. [Revisar]» |
| `cota_clave` en pozo con salida | Pozo dibujado; la clave sale del tubo | `≈` | «Clave tomada del colector de salida» |
| `dn_mm` en tubería | Tubo de 1 px constante, sin juntas | `?` | «Sin DN no hay diámetro real: el tubo se dibuja como línea» |
| `dn_mm` fuera de rango | Tubo con el valor, en `WARN` | `!` | «DN 1 400 mm fuera del catálogo (75–1 200). [Corregir]» |
| `altura_caida` en pozo de caída | Cono recto, sin caída | `?` | «Pozo de caída sin altura: se dibuja sin caída» |
| `altura_cresta` en vertedero | Cresta 0,30 m sobre la clave de llegada | `◇` | «Cresta supuesta a 0,30 m sobre la clave. [Definir]» |
| `altura_muro` en vertedero | Muro de 0,40 m (default de familia) | `◇` | «Altura de muro por defecto (0,40 m)» |
| Terreno ausente | Red flotando en el vacío, con rejilla de referencia | `?` | «Sin terreno: no hay cotas de referencia. [Importar levantamiento]» |
| Familia desconocida | Caja de alambre con el código de la familia | `?` | «Familia no reconocida: se muestra el objeto sin geometría» |

Nada de esto rompe la vista: el proxy ocupa el lugar correcto, la escena sigue a 60 fps y el resto
del proyecto es revisable.

### 8.3 Panel de vacíos

`V` abre el panel de vacíos: la **lista de tareas** del revisor, agrupada por parámetro faltante.

| Columna | Contenido |
|---|---|
| Grupo | `Falta diámetro interior (7)` — desplegable |
| Objetos | Código, familia, cota si la hay |
| Acción | `Completar` (abre la ficha con el campo enfocado) · `Ignorar` (marca como aceptado y lo cuenta aparte) |
| Totales | `3 vacíos · 5 supuestos · 1 392 completos` |
| Exportar | `Exportar lista (CSV)` para pasársela al que hizo la ingesta |

El contador vive también en la barra de estado (`3 vacíos · 5 supuestos`), siempre visible. Un
proyecto no se puede marcar como `Aprobado` con vacíos en objetos marcados `Ignorar` sin dejar
constancia: el botón de aprobación del proyecto pide confirmación y lo escribe en el journal.

### 8.4 Errores de inconsistencia

| Caso | Comportamiento |
|---|---|
| Tubo que no llega a ningún pozo | Se dibuja; el extremo libre lleva un anillo `!` y aparece en vacíos |
| Pozo sin tubos | Normal en una red en edición; no es error, se marca `sin conexiones` en la ficha |
| Tubo con clave por encima de la tapa | Se dibuja; chip `!` y mensaje `la clave sale del pozo`. No se corrige solo |
| Objeto borrado con vecinos | Se respeta `deleted`; los vecinos muestran `sin conexión` |
| Proyecto de esquema más nuevo | Banner de solo lectura, sin geometría de detalle |
| Falla la generación de una pieza | Se conserva la última geometría válida (fantasma) y se marca `!`; el resto del ensamblaje se dibuja igual |

Los códigos de error reutilizan la taxonomía del sobre JSON (`E_GEO`, `E_TOPO`, `E_STATE`); un
faltante de parámetro es `E_GEO` con los campos `objeto` y `campo`, sin inventar una familia nueva de
códigos.

---

## 9. Verificación y aceptación

Escenas headless (el mecanismo `--scene` / `--snap` que ya usa la app), con imagen de referencia:

| Escena | Qué fija |
|---|---|
| `pieza-pozo` | LOD 3 a 2 m: brocal, marco, tapa, cono, cuerpo, solera, peldaños, 3 entradas |
| `pieza-pozo-corte` | Corte a 1,35 m: tubos con espesor, banqueo, caras tramadas |
| `pieza-pozo-explosion` | Factor 1: 8 piezas etiquetadas, sin solapes |
| `pieza-valvula` | Volante de 8 radios, vástago, disco a 90° |
| `pieza-vertedero` | Muro de cabeza, cresta, disipador |
| `etiquetas-30m` | 16 objetos en pantalla: cero solapes > 2 px² |
| `vacios` | Un pozo sin diámetro, una tubería sin DN, un pin sin cota |
| `tour-catalogo` | Los 9 planos, 60 s, sin cambios de revisión |

Pruebas automáticas mínimas:

| Prueba | Afirma |
|---|---|
| `foco_encuadra_sin_reorientar` | Acimut y elevación no cambian; la esfera cabe con margen 15 % |
| `esc_desapila_modos_en_orden` | foco > corte > explosión → tres `Esc` dejan la vista original |
| `zoom_a_30cm_no_clipea_la_pieza` | Con `d = 0,30 m` ningún vértice del pozo cae tras el near |
| `corte_no_deja_tubos_flotando` | Toda boca intersectada tiene su elipse de sección |
| `sin_diametro_hay_proxy_y_un_vacio` | Prims > 0, `vacios == 1`, ninguna cota 0 |
| `cambiar_dn_recalcula_solo_la_pieza` | Cambian los prims del objeto; los del vecino son idénticos |
| `etiquetas_no_se_solapan` | 0 pares con intersección > 2 px² |
| `orbitar_es_live_only` | `live_only_events()` no vacío; ninguna entrada de journal |
| `tour_no_modifica_el_proyecto` | Revisión y estados idénticos antes y después |
| `aprobar_congela_la_geometria` | Editar un **E** de un aprobado exige reabrir o duplicar |

Criterios numéricos de aceptación: p95 de frame < 16,6 ms en `pieza-pozo` (1 objeto en LOD 3 + 40
vecinos); 0 solapes de etiqueta en `etiquetas-30m`; 100 % de acierto de picking en una matriz de 5
objetos × 9 puntos; 0 objetos dibujados a cota 0 en toda la batería.

---

## 10. Lo que no se hace (para que no se pida)

| No se hace | Por qué | Alternativa |
|---|---|---|
| Modelado libre de piezas | Es un visor, no un CAD de sólidos | Parámetros y reglas de familia |
| Booleanas / CSG reales | Coste y fragilidad desproporcionados | Recorte por plano + corte en la intersección |
| Dos planos de corte | Multiplica casos de borde del pintor | Un plano, giratorio |
| Medir en 3D (cinta métrica) | Es alcance de DWG, no del visor | Cota en la etiqueta y en la ficha |
| Editar la topología en 3D | Rompe la revisión y el journal | Se edita en la CLI / la skill de ingesta |
| Aprobar piezas sueltas | El DWG detalla, no certifica piezas | Se aprueba el objeto |
| Sombras, agua animada, reflejos | No aportan lectura técnica | Materiales planos y cotas |
| Guardar el tour como estado del proyecto | El tour es un guion, no un dato | JSON versionado en `docs/demo/` |

**Decisiones que subo al consejo** (no son mías):

1. **Ningún parámetro de dominio nuevo**: la ficha de §5 y las piezas de §1.7 solo usan nombres de
   `06-catalogo-familias.md`. Si 06 cambia, esta sección cambia con ella.
2. Si aprobar congela la geometría (§5.6) o solo avisa: afecta al flujo de P2 y al journal.
3. Si el corte debe poder exportarse al DXF como sección: hoy la exportación es del modelo, no de la
   vista.
4. Unidad de pendiente por defecto (‰ frente a %): lo fija el code pack, no la UI.

---

## Anexo A — Mapa de teclas

| Tecla | Acción | Contexto |
|---|---|---|
| `F` | Foco / salir del foco | Lienzo |
| `X` | Corte on/off | Lienzo |
| `E` | Explosión 0/1 | Lienzo |
| `+` `-` | Graduar explosión | Explosión activa |
| `Shift+rueda` | Mover plano de corte (10 cm) | Corte activo |
| `Alt+Shift+rueda` | Mover plano de corte (1 cm) | Corte activo |
| `Tab` | Pieza siguiente | Explosión activa |
| `Esc` | Desapilar modo / cerrar panel | Global |
| `Espacio` | Reencuadrar | Lienzo |
| `Ctrl+0` | Escala 1:1 | Global |
| `Ctrl+1` | Vista de catálogo | Global |
| `R` | Modo revisión | Global |
| `V` | Panel de vacíos | Global |
| `L` | Colapsar leyenda | Global |
| `T` | Tour | Global |
| `Ctrl+Z` / `Ctrl+Shift+Z` | Deshacer / rehacer | Global |
| `Alt+clic` | Ciclar candidatos / atravesar | Lienzo |
| `Shift+clic` | Chincheta de etiqueta | Etiqueta |
| `clic` / `Alt+clic` / `Shift+clic` | Aislar / ocultar / sumar al aislamiento | Fila de leyenda |
| `←` `→` | Plano anterior / siguiente | Tour |
| `F1` | Ayuda contextual del modo activo | Global |

Los atajos de una letra se inhiben cuando hay foco de texto en un campo
(`ui.text_focus.is_none()`), regla que ya fija el documento 04.

## Anexo B — Glosario de estados de vista

| Estado | Significado | Se ve en |
|---|---|---|
| `B0`…`B4` | Banda de distancia | Barra de estado |
| `LOD 0`…`LOD 4` | Nivel de detalle de una pieza | Barra de estado |
| `foco` | Modo de cámara con encuadre automático | Chip del modo |
| `corte` | Plano de sección activo y su cota | Chip del modo |
| `explosión` | Factor de despiece (0–2,5) | Chip del modo |
| `1:1` | Escala real en pantalla | Chip de cámara |
| `alturas ×n` | Exageración vertical activa | Barra de estado |
| `vacíos` | Objetos con parámetros faltantes | Barra de estado |
| `supuestos` | Defaults aplicados y marcados | Barra de estado |
| `detalle reducido` | Degradación automática por FPS | Chip de LOD |
