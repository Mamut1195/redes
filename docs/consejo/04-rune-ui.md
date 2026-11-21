# 04 — Rune UI: arquitectura de render e interacción

**Autor:** `rune-ui` (consejo de arquitectura) · **Fecha:** 2026-11-18 · **Estado:** propuesta para revisión
**Ámbito:** app de escritorio `alc-desk` (Rust + Rune), Windows primero.
**Depende de:** 01 (modelo de datos, coordenadas y IDs estables), 02 (kernel geométrico), 03 (hidráulica, HGL para el perfil).

> **Método.** Las decisiones de este documento se apoyan en el código y la documentación de
> `anomalyco/rune` leídos directamente del repositorio vendorizado (rev `78efd4a`, rama `dev`),
> no en suposiciones. Lo que no pude verificar está marcado `[VERIFICAR]` y listado en el
> Anexo A. Las citas de rendimiento (2.0 ms vs 13.7 ms) son las medidas publicadas por Rune en
> su propio bench de pinch.

---

## 0. Índice de decisiones

| # | Decisión | Sección |
|---|---|---|
| **D1** | Shell de 5 regiones con tamaño fijo en barras y redimensionable en paneles; el lienzo es un `grow()` con `min_w/min_h 0` | 1 |
| **D2** | La cámara vive en `State` (f64) y se publica a dos `Live` (`pan`, `zoom`) para el gesto | 2 |
| **D3** | Geometría en **pirámide de tiles** en CPU; se emiten **3–6 `Shape` fusionados**, nunca una por objeto | 2 |
| **D4** | Reconstrucción en **espacio de pantalla** (f64→f32), no en espacio mundo | 2 |
| **D5** | Pan = `live_offset` + `tx.live_only()`; zoom = `live_scale` acotado a `[0.80, 1.25]` y commit al asentarse | 2 |
| **D6** | Etiquetas como **elementos `text()`** en espacio de pantalla (tamaño constante), nunca dentro de la geometría escalada | 2, 5 |
| **D7** | Picking propio con **rejilla uniforme de 4 m**; la prueba de Rune solo decide *qué elemento* recibe el evento (`inert` en las capas decorativas) | 3 |
| **D8** | Tolerancia en **píxeles de pantalla** (6 px líneas, 9 px símbolos) convertida a mundo con `tol_px / (scale·zoom)` | 3 |
| **D9** | Caja de selección con `Drag.start_x/start_y/w/h` (capturados al presionar) — sin estado propio de press | 3 |
| **D10** | Máquina de estados explícita `Tool` con `rune::machine` (`Attempt`/`Effects`/`Seq`) | 4 |
| **D11** | Snap con prioridad fija y **chip flotante** movido por `live_offset` + `live_text` (cero re-ejecuciones de vista) | 4 |
| **D12** | LOD por umbrales de `px/m` + decluttering voraz con presupuesto de 250 etiquetas | 5 |
| **D13** | Perfil longitudinal con **dos escalas independientes** (`sx` cadena, `sv` cota) y exageración vertical explícita | 6 |
| **D14** | Inspector generado desde descriptores (`AttrDesc`), validación al confirmar, tabla virtualizada con `.growth().virtualize()` | 7 |
| **D15** | Modales con `.modal(...)` de Rune (trampa de Tab, Escape, `inert` del fondo) — no se implementa a mano | 1, 7 |
| **D16** | Verificación headless como **puerta de CI**: `--snap`, `--tree`, scripts JSON, `live_only_events()`, `RUNE_COMMIT_BUDGET_MS` | 9 |

---

## 1. Arquitectura de pantalla

### 1.1 Regiones y medidas

```
┌──────────────────────────────────────────────────────────────────────┐
│ topbar 44 pt   proyecto · herramientas · buscador · estado de red    │
├───────────┬──────────────────────────────────────┬───────────────────┤
│ izquierda │            LIENZO                    │  inspector 320 pt │
│ 272 pt    │  (grow, min 0)                       │  (min 280/max 460)│
│ familias  │  ┌ barra flotante de herramientas    │   pestañas:       │
│ leyenda   │  └ overlay: snap, banda, caja        │   props · familia │
│ capas     │                                      │   · historial     │
├───────────┴──────────────────────────────────────┴───────────────────┤
│ dock inferior 260 pt (min 120 / max 480): diagnóstico │ perfil │ tablas│
├──────────────────────────────────────────────────────────────────────┤
│ statusbar 22 pt   X,Y · escala 1:500 · snap activo · LOD · FPS       │
└──────────────────────────────────────────────────────────────────────┘
```

**Decisión.** Barras con altura fija (44/22 pt). Paneles con ancho/alto en `State` y
redimensionado por *splitters* de 5 pt (`on_drag` sobre un `el().size(5.0, _).cursor(Cursor::ResizeCol)`).
Lienzo `grow()` con `.min_w(0.0).min_h(0.0)`. Ventana por defecto 1440×900, mínima 1024×640.

**Por qué.** El lienzo es lo único que debe crecer con la ventana; todo lo demás es chrome con
medidas predecibles. Un CAD con paneles elásticos produce un canvas que cambia de tamaño en cada
`on_drag` de splitter, lo que dispara reconstrucción de geometría (caro) además de layout.

**Consecuencias.**
- Las medidas de panel se guardan en `State` y se persisten por proyecto (no por máquina).
- El splitter debe llamar `tx.live_only()` **solo** si mueve un `Live`; como el ancho del panel
  entra en el layout, aquí **no** aplica: cada arrastre re-ejecuta la vista. Es aceptable porque
  la geometría está memoizada por `rev` (§2.5) y el coste es de layout, no de teselado.
- Presupuesto de redimensionado: si el lienzo cambia de tamaño, la geometría se reconstruye una
  vez al soltar (`on_drag` final / `Drag` con delta 0), no en cada punto.

### 1.2 Árbol de elementos

```rust
fn view(s: &App, cx: &mut Cx<App>) -> El {
    let t = tok::theme(s.ui.dark);
    col().h_full().w_full().fill(t.bg).gap(0.0)
        .child(top_bar(s, cx))                       // 44 pt fijos
        .child(row().grow().min_h(0.0)               // ← trampa flex: min_h(0)
            .child(left_panel(s, cx))                // 272 pt
            .child(col().grow().min_w(0.0).min_h(0.0)
                .child(canvas_region(s, cx)))        // lienzo + overlays
            .child(right_panel(s, cx)))              // 320 pt
        .child(bottom_dock(s, cx))                   // 260 pt
        .child(status_bar(s, cx))                    // 22 pt
        .child(modals(s, cx))                        // paleta / diálogos
}
```

**Trampa verificada (obligatoria en el estándar de código).** `.grow(g)` es `flex: g 1 0`: en una
columna de altura automática no hay espacio libre y un `.scroll()` colapsa a 0 pt. Hay que acotar
la altura del ancestro (`.h_full()`) y poner `.min_h(0.0)` en **cada** columna intermedia entre el
ancestro acotado y el viewport con scroll. Rune emite un warning en debug cuando esto pasa; en
nuestro proyecto ese warning se trata como error de CI.

### 1.3 Scroll por región

| Región | Scroll | Notas |
|---|---|---|
| Panel izquierdo | vertical (`.scroll(ScrollSpec::default())`) | árbol de capas + leyenda; contenedor hijo con `.min_h(0.0)` |
| Inspector | vertical | formulario; **no** virtualizar (pocos campos) |
| Dock / tablas | vertical virtualizado | `.growth(GrowthSpec::default()).virtualize(24.0)` como **primer hijo** del viewport |
| Dock / perfil | ninguno | el perfil se ajusta al ancho del panel (recalcula `sx`) |
| Consola de diagnóstico | vertical virtualizado | filas de 22 pt, miles de avisos |

**Decisión.** La tabla de resultados y la lista de diagnósticos usan
`.growth(GrowthSpec::default()).virtualize(alto_estimado)` en el mismo elemento, como primer hijo
del viewport de scroll.

**Por qué.** Es la única forma soportada: `.virtualize` sin `.growth` **entra en pánico** con la
ruta del elemento; y el stack debe ser el primer hijo del viewport de scroll. Además, las filas
necesitan `key` único y padding vertical interno (no márgenes ni `gap` del stack), y el alto
estimado se corrige al medir.

**Consecuencias.** Filas de altura fija (24 pt tabla, 22 pt diagnóstico) para que la estimación sea
exacta y el scroll no “salte”. Si una fila necesita dos líneas, se usan dos filas.

### 1.4 Foco, teclado y modalidad

**Decisión.** Orden de tabulación: `topbar → panel izq → lienzo → inspector → dock → statusbar`,
con `tab_index(0)` en orden de documento y `1..n` solo en la barra de herramientas. El lienzo es
`focusable(true)`, `role("application")`, `label("Área de dibujo")`. Los modales usan `.modal(..)`.

**Por qué.** Rune ya resuelve lo difícil: `.modal(click_outside, close)` atrapa Tab, maneja Escape,
deja el fondo `inert` y restaura el elemento que lo abrió, con anidamiento en pila. Reimplementarlo
es la fuente clásica de bugs de foco.

**Consecuencias.**
- Anillo de foco **solo teclado** con `.focus_ring(2.0, 2.0, tok::accent(0, dark))`; nunca se dibuja
  en clic con ratón (Rune distingue `focusVisible`). Esto es requisito de accesibilidad y de
  estética CAD.
- Enrutado de teclas (documentado por Rune, se respeta):
  1. Return/Escape durante composición IME van a la composición y a nadie más.
  2. Return sin modificadores pulsa `default_button()` salvo que el foco lo consuma.
  3. Sin foco → `on_key` de la ventana.
  4. El campo enfocado recibe `on_input_key`.
- Los atajos globales se implementan en `AppSpec::on_key`, con guarda `s.ui.text_focus.is_none()`
  para no robar `V`, `P`, `M` mientras se escribe.
- Las capas decorativas del lienzo llevan `.inert()` para no participar del hit test ni del árbol AX.

---

## 2. El lienzo

### 2.1 El problema, en números

Un proyecto mediano: 70 000 tramos de tubería + 25 000 pozos + 5 000 válvulas ≈ **100 000 objetos**,
que como polilíneas son ~150 000 segmentos. A 1440×900 el viewport cubre entre 40 m × 25 m
(zoom de detalle, 40 px/m) y 4 km × 2.5 km (encuadre, 0.4 px/m). Es decir: **entre 0.5 % y 100 %**
del proyecto es visible.

Dos hechos de Rune dominan el diseño:

1. Un `Shape` se dibuja **un cápsula analítica por segmento** (`KIND_SEGMENT`), sin atlas, y
   **puede cambiar cada frame**. El coste GPU es proporcional a primitivas emitidas, y no hay
   reutilización entre frames más allá de `.cache()`.
2. Un **elemento** cuesta ~2 µs de vista/layout (bench publicado: 6 000 elementos ⇒ 13.7 ms por
   frame; `live_only` ⇒ 2.0 ms). Con 100 000 objetos, “un elemento por objeto” son ~200 ms/frame:
   **inviable por dos órdenes de magnitud**.

De ahí la regla que ordena todo lo demás:

> **Los elementos son el recurso escaso; los segmentos son baratos.**
> Presupuesto por frame: **3–6 elementos de geometría, ≤ 20 000 segmentos, ≤ 250 etiquetas,
> ≤ 12 elementos de overlay.**

### 2.2 Decisión: pirámide de tiles + `Shape` fusionados

**Decisión.** La geometría vive en una **pirámide de tiles** (quadtree) en CPU, construida una vez
fuera del frame loop. En cada reconstrucción se emiten **3–6 `Shape` fusionados** por estilo
(agua, alcantarillado, topografía/curvas, rellenos), no uno por tile ni uno por objeto.

**Por qué.**
- *Una `Shape` por objeto* → 100 000 elementos: descartado por (2).
- *Una `Shape` gigante en coordenadas mundo con `live_scale`* → mantiene **todas** las primitivas
  instanciadas a cualquier zoom, no permite culling y arrastra error de grosor de línea. Descartado.
- *Un elemento por tile* (20–80 elementos visibles) → funciona, pero multiplica layout y diffs sin
  ganancia: como el pan se resuelve con `live_offset` (§2.4), no hace falta que los tiles sean
  elementos independientes para “no re-teselar”.
- *Tiles como caché de CPU + fusión en pocos `Shape`* → el culling y el LOD ocurren antes de emitir,
  el coste por frame es lineal en lo visible, y el número de elementos queda acotado. **Elegido.**

**Consecuencias.**
- La pirámide se construye en `AppSpec::setup`/hilo de fondo (`rune::Latest` para publicar
  resultados) y se guarda en `Arc<TileStore>`. **Cero IO en el frame loop** (regla de Rune).
- Memoria: ~150 000 puntos × 8 B = 1.2 MB por nivel base; la pirámide completa ≈ 1.6× ⇒ **< 4 MB**.
  Irrelevante frente al modelo.
- Coste de reconstrucción medido a ojo: transformar 20 000 puntos + emitir 20 000 cápsulas
  ≈ **0.8–1.5 ms**. Cabe en el presupuesto y solo ocurre al hacer commit (§2.4).

### 2.3 Escala, nivel y sello de tiempo

```rust
/// Un tile debe medir ~512 pt en pantalla; el nivel se elige por escala.
pub const TILE_BASE_M: f64 = 8.0;          // lado del tile en el nivel 0

pub fn level_for(scale_px_m: f64) -> u8 {
    // 8·2^L·scale ∈ [384, 768]  ⇒  L = round(log2(512 / (8·scale)))
    let l = (512.0 / (TILE_BASE_M * scale_px_m)).log2().round();
    l.clamp(0.0, 16.0) as u8
}

pub fn tile_span_m(level: u8) -> f64 { TILE_BASE_M * (1u64 << level) as f64 }
```

Cada nivel guarda sus polilíneas **decimadas** con ε = 0.75 pt convertido a metros en ese nivel
(`ε_m = 0.75 / (scale_del_nivel)`), mediante Douglas–Peucker. Esto es lo que hace viable la
topografía: un levantamiento con 500 000 puntos de curva de nivel se reduce a ~8 000 puntos en el
nivel de encuadre.

**Decisión.** Se almacenan puntos en **metros relativos al origen del tile** en `f32` (el tile mide
≤ 512 pt / escala ⇒ a 1 px/m son 512 m; a `f32` el error es < 0.06 mm: suficiente) y el origen del
tile en `f64`.

**Por qué.** Evita el error de `f32` en coordenadas UTM (a 1 000 000 el paso es 0.06 m: inaceptable
para cotas de invert). Es el mismo truco que usan los SIG: origen local + delta.

**Consecuencias.** El modelo guarda `f64` con origen de sitio (`site_origin`) y el motor de
render convierte a local una sola vez, al cargar. Un proyecto no puede cruzar husos UTM sin
re-proyectar (fuera de alcance).

### 2.4 Flujo de datos y las dos escalas de tiempo

```
Modelo (f64, m, IDs estables)
   │  (carga / edición)
   ▼
TileStore (por nivel, decimado, m locales al tile)  ── Arc, hilo de fondo
   │  selección de tiles visibles: nivel = level_for(scale), AABB = viewport + margen 40 %
   ▼
Emisión (commit): p_px = (p_tile + origen_tile − centro) · scale + vp/2      [f64 → f32]
   │                                        ↑ eje Y invertido (norte arriba)
   ▼
Shape(agua) · Shape(alcantarillado) · Shape(topo) · Shape(rellenos)   [3–6 elementos]
   │
   ▼
world_host  .live_offset(&pan)  .live_scale(&zoom)  .cache()
   │  gesto (wheel/drag/magnify) → handlers .set() + tx.live_only()
   ▼
Presentación
```

**Dos relojes.** El *commit* es caro (ms) y el *gesto* es barato (µs). Rune está diseñado
exactamente para eso: `Live<T>` es un handle “latest-wins” que no re-ejecuta vista ni layout.

```rust
// crates/canvas/src/camera.rs
#[derive(Clone)]
pub struct Camera {
    pub center: DVec2,            // m, origen de sitio
    pub scale: f64,               // pt por metro (verdad, solo cambia en commit)
    pub y_up: bool,               // topografía: norte arriba ⇒ Y de pantalla invertida
    pub pan: Live<[f32; 2]>,      // delta transitorio del gesto, pt
    pub zoom: Live<f32>,          // factor transitorio del gesto
    pub vp: [f32; 2],             // tamaño del viewport en pt
}

impl Camera {
    #[inline]
    pub fn to_screen(&self, p: DVec2) -> [f32; 2] {
        let d = p - self.center;
        let y = if self.y_up { -d.y } else { d.y };
        [ (d.x * self.scale) as f32 + self.vp[0] * 0.5,
          (y   * self.scale) as f32 + self.vp[1] * 0.5 ]
    }

    /// Inversa exacta, incluidos los valores live (para picking y snapping).
    pub fn to_world(&self, p_local: [f32; 2]) -> DVec2 {
        let z = self.zoom.get() as f64;
        let pan = self.pan.get();
        let x = (p_local[0] as f64 - pan[0] as f64) / z;
        let y = (p_local[1] as f64 - pan[1] as f64) / z;
        let dx = (x - self.vp[0] as f64 * 0.5) / self.scale;
        let dy = (y - self.vp[1] as f64 * 0.5) / self.scale;
        DVec2::new(self.center.x + dx,
                   self.center.y + if self.y_up { -dy } else { dy })
    }
}
```

**Por qué reconstruir en pantalla y no en mundo.** (a) Los grosores y las tolerancias son
constantes en píxeles por construcción, sin compensación. (b) La GPU nunca ve coordenadas grandes:
la precisión la resuelve `f64` en CPU. (c) Permite culling y LOD antes de emitir. El precio es que
un cambio de cámara “real” obliga a reemitir; lo pagamos solo en commit.

### 2.5 Pan, zoom y cuándo reconstruir

**Decisión.** `pan` y `zoom` se aplican como `Live` sobre `world_host` con `tx.live_only()`.
Se hace **commit** cuando ocurre cualquiera de estas condiciones:

| Condición | Umbral | Motivo |
|---|---|---|
| Desplazamiento acumulado | > 35 % del viewport en cualquier eje | el margen construido (40 %) se agota |
| Factor de zoom live | fuera de `[0.80, 1.25]` | acota el error de grosor a ±25 % |
| Fin de gesto | `WheelEvent::phase == Ended`, `Drag` final, o 200 ms sin eventos | deja el frame nítido y exacto |
| Cambio de selección / edición | `rev` de escena o de etiquetas | la geometría depende de ellos |
| Cambio de tamaño del lienzo | — | cambia `vp` y el nivel |

```rust
// handlers del lienzo (firmas verificadas en el fuente de Rune)
.on_drag(|s: &mut App, d: Drag, tx| -> bool {
    if tx.button() == Some(PointerButton::Middle) || s.ui.space_down {
        let p = s.cam.pan.get();
        s.cam.pan.set([p[0] + d.dx as f32, p[1] + d.dy as f32]); // dx,dy en puntos de ventana
        if s.cam.pan_exhausted() { s.commit_camera(); }          // ~1 ms, 1 vez cada 1/3 de pantalla
        tx.live_only();
        return true;                                             // toma el evento
    }
    false
})
.on_wheel(|s: &mut App, e: WheelEvent, tx| -> bool {
    if e.mods.shift { s.cam.pan_by([-e.dy as f32, 0.0]); }        // rueda = pan horizontal
    else            { s.cam.zoom_about(e.x, e.y, (e.dy as f32 * 0.0015).exp()); }
    if e.phase == WheelPhase::Ended || s.cam.zoom_out_of_band() { s.commit_camera(); }
    tx.live_only();
    true
})
.on_magnify(|s: &mut App, e: MagnifyEvent, tx| -> bool {
    s.cam.zoom_about(e.x, e.y, 1.0 + e.delta);
    if s.cam.zoom_out_of_band() { s.commit_camera(); }
    tx.live_only();
    true
})
```

```rust
impl Camera {
    pub fn zoom_about(&mut self, x: f32, y: f32, k: f32) {
        let z0 = self.zoom.get();
        let z1 = (z0 * k).clamp(0.80, 1.25);
        let k = z1 / z0;                       // factor efectivo tras el clamp
        let p = self.pan.get();
        // escala alrededor del puntero: p' = k·(p − pivote) + pivote
        self.pan.set([k * (p[0] - x) + x, k * (p[1] - y) + y]);
        self.zoom.set(z1);
    }
    pub fn zoom_out_of_band(&self) -> bool {
        let z = self.zoom.get();
        z <= 0.801 || z >= 1.249
    }
}
```

**Por qué una banda y no zoom libre.** `live_scale` escala el render, incluido el grosor de los
trazos: un factor 2 haría tuberías de 3.2 px. La banda acota el error a ±25 %, que durante un gesto
de 100–200 ms es imperceptible, y el commit devuelve el grosor exacto. Nota: el texto bajo
`live_scale` se rasteriza en cubos de `2^(1/4)` y se re-rasteriza nítido al reposar 0.1 s; aun así
**no** metemos etiquetas en el mundo escalado (§2.7), porque el tamaño debe ser constante, no nítido
a tamaño variable.

**Consecuencias y salvaguardas.**
- `tx.live_only()` tiene un chequeo de debug: un evento live-only **≥ 250 ms después del último
  commit vuelve a ejecutar la vista igualmente**, y si lo construido difiere, Rune imprime el
  handler. Por eso el umbral de 200 ms de commit: mantenemos la invariante “nunca 250 ms sin
  commit durante un gesto”.
- Un handler live-only **no puede** cambiar estado que la vista lea. La cámara “verdad”
  (`center`,`scale`) solo se toca en `commit_camera()`.
- `.cache()` en `world_host`: Rune compone un solo quad mientras un ancestro mueve `live_offset`
  o `live_scale`, y re-rasteriza a la resolución de dispositivo cuando la escala se queda quieta
  0.1 s. Límite: **4096×4096 px de dispositivo**. A 4K con `scale = 2` el contenido excede el tope
  ⇒ política: **no cachear si `vp·pixel_scale > 4000`** en cualquier eje, y limitar el contenido al
  viewport + margen (nunca “todo el proyecto”).

### 2.6 Culling

**Decisión.** Culling en tres niveles, todos O(1) amortizado:

1. **Tiles**: AABB del tile contra viewport + margen 40 %, recorriendo el quadtree por nivel
   (4–16 tiles visibles típicos).
2. **Primitivas**: dentro de un tile visible se emite todo (ya está decimado y es pequeño);
   no hay test por segmento.
3. **Anotaciones**: AABB por etiqueta contra el viewport, antes del decluttering.

**Por qué no test por objeto.** A 20 000 segmentos visibles, un test AABB por segmento cuesta más
que emitirlo (una suma y dos comparaciones por punto ya es lo mínimo). El tile es la unidad
correcta.

**Consecuencias.** El margen del 40 % es el que fija la frecuencia de commit (§2.5): cuanto mayor,
menos commits y más memoria/instancias. 40 % es el punto donde el coste de emisión de lo que sobra
(~30 %) sigue por debajo del coste de un commit extra.

### 2.7 Grosores y texto por escala

**Regla.** Todo lo que debe medir lo mismo en píxeles se construye en **puntos de pantalla**; todo
lo que representa magnitud física se construye en **metros**. Nunca se mezcla.

| Elemento | Unidad | Regla |
|---|---|---|
| Tubería (trazo) | pt constante | 1.4 pt agua · 1.7 pt alcantarillado · 1.0 pt existente |
| Ancho mínimo | 1.0 pt | por debajo, la tubería se dibuja como línea de 1 pt con el color de la familia |
| Símbolo (pozo, válvula) | pt constante | 7–9 pt de diámetro, independiente de `scale` |
| Curvas de nivel | pt constante | 0.8 pt maestra / 0.5 pt secundaria |
| Cota / diámetro / pendiente | `text()` 9.5–10.5 pt | tamaño fijo; nunca escala con el zoom |
| Hatch / relleno | patrón | solo si `scale > 8 px/m` (§5.3) |
| Cámara | m | solo `center` y `scale` |

**Texto que no debe escalar como la geometría.** No hay texto en un `Shape` (el canvas solo dibuja
trazos y rellenos). Las etiquetas son elementos `text("Ø 200")` posicionados con `.absolute(x, y)`
en **espacio de pantalla**, recalculados en cada commit y solo re-posicionados (por `live_offset`)
durante el gesto. Esto resuelve de golpe: tamaño constante, nitidez garantizada por el rasterizador
de glifos (que ajusta baselines al píxel) y selección/lectura por accesibilidad.

```rust
// emisión de una etiqueta (dentro del subtree memoizado de etiquetas)
fn label(l: &Label, cam: &Camera) -> El {
    let p = cam.to_screen(l.anchor);
    text(&l.text)
        .font(TextStyle::new(10.0, 500.0).features("'tnum' 1")) // cifras tabulares: no bailan
        .color(if l.selected { t.accent } else { t.text_dim })
        .absolute(p[0] + l.dx, p[1] + l.dy)
        .key(&l.key)                       // clave estable ⇒ el diff no remonta
        .role("img").label(&l.a11y)        // "Diámetro 200 milímetros"
}
```

**Por qué `'tnum' 1`.** Con cifras proporcionales, un cambio de “Ø 160” a “Ø 200” mueve el ancho y
la etiqueta “baila” al arrastrar. Las cifras tabulares fijan el avance.

### 2.8 Presupuesto de frame

| Etapa | 120 fps (8.3 ms) | 60 fps (16.6 ms) |
|---|---|---|
| Entrada + máquina de estados | 0.3 ms | 0.3 ms |
| Culling de tiles + picking | 0.2 ms | 0.2 ms |
| Emisión de geometría (**solo commit**) | 1.5 ms | 1.5 ms |
| Etiquetas + decluttering (**solo commit**) | 0.8 ms | 0.8 ms |
| Overlay (snap, banda) | 0.2 ms | 0.2 ms |
| Layout + render + GPU | resto | resto |
| **Gesto típico (sin commit)** | **< 1 ms** | **< 1 ms** |

Referencia medida por Rune: un pinch de 120 frames sobre 6 000 elementos con `live_only` cuesta
**2.0 ms/frame** frente a **13.7 ms** re-ejecutando la vista. Nuestro caso de gesto es más barato
(una decena de elementos estáticos y un quad cacheado).

---

## 3. Hit testing y selección

### 3.1 Decisión: índice propio, hit test de Rune solo para el enrutado

**Decisión.** El picking se hace con una **rejilla uniforme** propia sobre el modelo. El hit test de
Rune se usa únicamente para decidir qué *elemento* recibe el evento; las capas de dibujo van
`.inert()` para que el evento caiga siempre en el elemento-lienzo.

**Por qué.**
- `.on_press` **no da punto** (verificado). Depender de él para seleccionar obliga a guardar el
  último `pointer_move`, que es frágil.
- La rejilla se reconstruye localmente al editar (mover un pozo toca 1–9 celdas); un R-tree o un
  BVH obligan a reconstrucción global al cambiar topología, justo lo que un editor hace todo el día.
- 100 000 objetos en celdas de 4 m sobre 12 km² ≈ 750 000 celdas potenciales pero solo ~25 000
  ocupadas: `HashMap<CellId, SmallVec<u32>>`, ~6 MB. Aceptable. `[VERIFICAR]` si preferimos
  `Vec` disperso por filas cuando el proyecto crezca.

**Consecuencias.** La rejilla indexa **segmentos** (no objetos), con el AABB de cada segmento
insertado en todas las celdas que cruza (DDA). Consulta típica: 9 celdas × ~20 segmentos = 180
candidatos; el test exacto punto-segmento es trivial.

### 3.2 Tolerancia y prioridad

```rust
pub const TOL_LINE_PX: f32 = 6.0;    // tuberías, curvas
pub const TOL_SYMBOL_PX: f32 = 9.0;  // pozos, válvulas (Fitts: el símbolo es pequeño)

pub fn pick(cam: &Camera, idx: &Grid, p_px: [f32; 2]) -> Option<Hit> {
    let w = cam.to_world(p_px);
    let r = TOL_SYMBOL_PX as f64 / (cam.scale * cam.zoom.get() as f64);
    let mut best: Option<Hit> = None;
    for &seg in idx.query(w, r) {
        let (tier, tol) = tier_of(seg);                  // símbolo 0 · válvula 1 · tubería 2 · cota 3
        let d_px = dist_point_segment_px(seg, w, cam);
        if d_px <= tol {
            let cand = Hit { seg, d_px, tier };
            if best.map_or(true, |b| (cand.tier, cand.d_px) < (b.tier, b.d_px)) { best = Some(cand); }
        }
    }
    best
}
```

**Decisión.** Tolerancia en píxeles, convertida a mundo con la escala **incluido el zoom live**.
Prioridad por *tier* antes que por distancia: un pozo a 8 px gana a una tubería a 2 px.

**Por qué.** En un CAD, el usuario apunta al símbolo, no a la línea que pasa por debajo. Ordenar
solo por distancia hace imposible seleccionar un pozo sobre una tubería.

**Consecuencias.** La prioridad es una constante documentada y testeable (escena headless
`pick-priority`). Cambiarla requiere un test nuevo.

### 3.3 Selección: clic, aditiva y caja

**Decisión.**
- **Clic**: `.on_press` → `pick()` con el último punto local conocido (`on_pointer_move_local`).
- **Caja**: `.on_drag` con `Drag { start_x, start_y, w, h }` — capturados **al presionar**, en
  puntos locales. No guardamos estado de press.
- **Modificadores**: `Shift` suma, `Ctrl` alterna, `Alt` resta. El estado de modificadores se
  mantiene con `on_key` (`Key::Modifier`), porque los eventos de puntero no lo traen.
- **Caja con `Shift`** = suma a la selección existente.

```rust
.on_drag(|s: &mut App, d: Drag, tx| -> bool {
    if s.tool != Tool::Select { return false; }
    let (x0, y0, w, h) = (d.start_x, d.start_y, d.w, d.h);
    if w.abs() > 3.0 || h.abs() > 3.0 {                 // umbral: 3 pt ⇒ es caja, no clic
        let r = Rect::from_xywh(x0.min(x0 + w), y0.min(y0 + h), w.abs(), h.abs());
        s.box_select.set([r.x, r.y, r.w, r.h]);          // Live<[f32;4]> → se dibuja sin re-ejecutar
        s.selection = s.index.query_rect_world(s.cam.rect_to_world(r), s.selection_mode());
        tx.live_only();
        return true;
    }
    false
})
```

**Por qué la caja con `Live<[f32;4]>` + `.live_rect`.** Rune ofrece `.live_rect(&Live<[f32;4]>)`:
un rectángulo que se mueve sin re-ejecutar vista ni layout. La caja de selección es el caso de uso
exacto. Sin esto, cada movimiento del ratón re-ejecutaría la vista completa.

### 3.4 Hover

**Decisión.** Hover con histéresis: se recalcula solo si el puntero se movió ≥ 2 pt o cambió el
modificador. Se guarda `hover: Option<ObjId>`; si no cambia, **no** se toca el estado.

**Consecuencias.** Hover y press en Rune solo re-apuntan *tracks*, nunca re-ejecutan la vista
(verificado). El resaltado se dibuja con un `Shape` extra en el overlay (halo de 4 pt bajo la
geometría seleccionada), reconstruido solo cuando cambia `hover`.

**Aviso de render.** Los trazos translúcidos **mezclan dos veces** donde los segmentos se solapan
en una unión. Por eso los halos de selección/hover se dibujan **opacos** (color mezclado con el
fondo precalculado) o con `opacity` sobre el elemento, no con alfa en el color del trazo.

### 3.5 Flujo de eventos completo

| Gesto | Handler | Punto | Efecto |
|---|---|---|---|
| Mover | `on_pointer_move_local` | local | actualiza `last_local`; hover + snap; `live_only` si nada más cambia |
| Clic | `on_press` | — | `pick()` en `last_local`; aplica modificadores |
| Arrastrar (izq, vacío) | `on_drag` | local + `start_*` | caja de selección (`live_rect`) |
| Arrastrar (medio) | `on_drag` + `.pointer_buttons([Middle])` | local, `dx/dy` ventana | pan (`live_offset`) |
| Rueda | `on_wheel` | local | zoom al puntero o pan horizontal |
| Pinza | `on_magnify` | local | zoom al puntero (**en Windows llega como Ctrl+rueda**, `-dy/200`) |
| Doble clic | `on_double_press` | — | entra al modo de edición del objeto bajo `hover` |

---

## 4. Snapping y herramientas de dibujo

### 4.1 Modos y prioridad de snap

| Prioridad | Snap | Condición | Marca |
|---|---|---|---|
| 1 | Extremo de tubería / nodo | distancia ≤ 10 pt | cuadrado |
| 2 | Intersección | cruce de 2 segmentos candidatos | aspa |
| 3 | Centro de pozo/válvula | ≤ 10 pt | rombo |
| 4 | Perpendicular | desde el último punto a un segmento | ángulo recto |
| 5 | Punto medio / borde | ≤ 8 pt | triángulo |
| 6 | Rejilla | ≤ 7 pt | cruz pequeña |
| 7 | Ortogonal | `Shift` con el tramo anterior | guía 0/90/45° |

**Decisión.** Se evalúan en ese orden y gana la primera coincidencia dentro de tolerancia; las
ortogonales se aplican **después** del snap geométrico (proyectan el punto sobre el eje si no hay
snap de mayor prioridad).

**Por qué.** Es el orden de AutoCAD/Civil 3D: el usuario espera que el extremo gane a la rejilla.
Invertirlo produce “saltos” a la rejilla al cerrar un circuito.

### 4.2 Máquina de estados de herramienta (`rune::machine`)

**Decisión.** Un `enum` con una transición pura que devuelve *efectos*, tal como documenta Rune
(`Attempt` para cercar respuestas asíncronas, `Effects` sin asignar, timers como eventos).

```rust
use rune::machine::{Attempt, Effects, Seq};

#[derive(Clone, Debug)]
pub enum Tool { Idle, Placing { run: RunId, pts: Vec<Pt>, cur: Preview }, Done { run: RunId } }

pub enum Ev {
    Move { at: [f32; 2], snap: Option<SnapHit>, now: f64 },
    Press { snap: Option<SnapHit> },
    Esc, Enter, Undo,
}

pub enum Eff { CommitSegment { a: Pt, b: Pt }, ShowHint(&'static str), Flash(SnapKind), OpenRunDialog }

pub fn step(seq: &mut Seq, tool: &mut Tool, ev: Ev) -> Effects<Eff> {
    match (&*tool, ev) {
        (Tool::Idle, Ev::Press { snap: Some(s) }) => {
            *tool = Tool::Placing { run: RunId::next(), pts: vec![s.point], cur: Preview::from(s) };
            Effects::one(Eff::ShowHint("Extremo fijado · clic para el siguiente tramo · Esc cancela"))
        }
        (Tool::Placing { run, pts, cur }, Ev::Press { snap: Some(s) }) => {
            if s.point == *pts.last().unwrap() { return Effects::none(); }
            let a = *pts.last().unwrap();
            pts.push(s.point);
            *cur = Preview::from(s);
            Effects::one(Eff::CommitSegment { a, b: s.point })   // un solo comando de undo
        }
        (Tool::Placing { cur, .. }, Ev::Move { snap, .. }) => { *cur = Preview::from_opt(snap); Effects::none() }
        (Tool::Placing { .. }, Ev::Esc) | (Tool::Placing { .. }, Ev::Enter) => {
            *tool = Tool::Idle; Effects::one(Eff::ShowHint("Tramo terminado"))
        }
        _ => Effects::none(),
    }
}
```

**Por qué.** “Los estados ilegales no existen”: `Placing` sin run o `Done` con puntos no se puede
construir. `Attempt` evita que una respuesta tardía (p. ej. validación hidráulica al cerrar el
tramo) aplique sobre un estado que ya cambió. `Effects` devuelve lo que hay que hacer sin hacerlo,
lo que permite testear la máquina sin UI ni GPU.

**Consecuencias.** `CommitSegment` produce **un** comando (undo atómico). La vista previa nunca
muta el modelo: se dibuja en el overlay.

### 4.3 Vista previa y marcadores

**Decisión.** La goma elástica (línea del último punto al cursor) se construye en el `Shape` del
overlay **en cada movimiento** (re-ejecuta la vista), pero con la geometría de fondo memoizada por
`rev`, de modo que el coste es ~0.2 ms. El chip de snap y las coordenadas usan `Live`, sin
re-ejecución.

```rust
// chip flotante: se mueve y cambia de texto sin re-ejecutar la vista
el().absolute(0.0, 0.0).live_offset(&s.snap.pos).inert()
    .child(
        row().gap(6.0).padding(6.0).radius(6.0)
            .fill(t.chip_bg).shadow(2.0, 8.0, t.shadow)
            .child(canvas(snap_glyph(s.snap.kind)))          // cuadrado/aspa/rombo, 8 pt
            .child(text("EXTREMO").font(TextStyle::new(10.0, 600.0)).color(t.accent))
            .child(text("0.00, 0.00")
                .live_text(&s.snap.readout)                   // LiveText: sin re-layout
                .font(TextStyle::new(10.0, 400.0).mono())
                .color(t.text_dim)))
```

**Por qué `LiveText`.** Es el mecanismo documentado para actualizar una etiqueta de una línea sin
reconstruir la vista ni mover a sus hermanos; el placeholder fija el layout y el texto real solo
reforma ese run. Es exactamente un HUD de coordenadas.

**Consecuencias.** El chip se actualiza en `on_pointer_move_local` con `tx.live_only()`. La marca
de snap *geométrica* (la cruz sobre el punto) sí vive en el overlay reconstruido, porque cambia de
posición en el mundo.

### 4.4 Atajos de herramienta

`V` seleccionar · `T` tubería · `P` pozo · `N` nodo · `W` válvula · `D` cota · `M` medir ·
`Espacio` pan temporal · `F3` osnap on/off · `F8` orto on/off · `Esc` cancelar ·
`Enter` terminar · `Ctrl+Z`/`Ctrl+Shift+Z` deshacer/rehacer.

---

## 5. Anotaciones y detalle dependiente de escala

### 5.1 Reglas de LOD

**Decisión.** Umbrales en `scale` (px/m), no en número de objetos. Una sola tabla, en un módulo,
testeable.

| `scale` (px/m) | Equivalente | Se dibuja |
|---|---|---|
| > 20 | 1:50 | todo: tuberías, símbolos, cotas de invert, pendientes, diámetros, hatch, curvas 0.25 m |
| 8–20 | 1:100–1:250 | tuberías, símbolos, diámetros; hatch solo en secciones; curvas 0.5 m |
| 2–8 | 1:500 | tuberías, símbolos, diámetros solo en troncales; curvas 1 m |
| 0.5–2 | 1:2000 | tuberías, símbolos a 5 pt, sin etiquetas salvo selección; curvas 2 m |
| < 0.5 | encuadre | solo troncales (Ø ≥ 200 mm) y pozos principales; sin símbolos menores |

**Por qué umbrales de escala y no de conteo.** El usuario percibe el LOD como “detalle según
zoom”; un umbral por cantidad haría que el mismo zoom se viera distinto en proyectos grandes y
pequeños.

**Consecuencias.** `level_for()` y la tabla de LOD comparten la misma noción de escala; se prueban
juntas en la escena `lod-matrix` (una imagen por banda).

### 5.2 Decluttering de etiquetas

**Decisión.** Colocación voraz por prioridad con rejilla de ocupación en pantalla:

1. Orden: seleccionadas > hover > válvulas > pozos > troncales (Ø) > pendientes > cotas.
2. Para cada etiqueta: AABB estimado `w ≈ chars · 0.58 · size + 4`, alto `size · 1.35`.
3. Se prueba contra una rejilla de 16 pt; si el AABB pisa una celda ocupada, se descarta.
4. Presupuesto global 250 etiquetas; al agotarse, se corta por prioridad (no por orden de modelo).

**Por qué “descartar” y no “mover”.** Mover etiquetas (leader lines automáticas) es un problema de
optimización que produce saltos entre frames. Descartar es determinista, estable y barato; el
usuario que necesita una cota concreta selecciona el objeto (prioridad 1).

**Consecuencias.** La selección **siempre** etiqueta, aunque haya 250 ya colocadas: las
seleccionadas se reservan primero. Test: `labels-stable` (dos frames seguidos con el mismo estado
producen la misma imagen).

### 5.3 Rellenos y hatch

**Decisión.** Por defecto, relleno con **tinte plano** (color de familia a 12 % de alfa). Hatch
real (líneas a 45° cada 4–8 pt) solo si `scale > 8 px/m` y el polígono está íntegramente en
pantalla, limitado a 12 polígonos y 400 líneas.

**Por qué.** Rune no tiene clipping de un `Shape` contra un polígono: el hatch hay que generarlo
por intersección línea-polígono en CPU. Es barato para 12 polígonos, caro para 2 000. Y a zoom
bajo un hatch es una mancha gris: no aporta.

**Consecuencias.** El hatch se calcula en el commit con el mismo ε de pantalla; a zoom alto el
patrón no “late” porque se ancla al origen del polígono, no a la pantalla.

### 5.4 Curvas de nivel y sus etiquetas

**Decisión.** Curvas maestras (cada 5 m) a 0.8 pt y con etiqueta; secundarias a 0.5 pt.
La etiqueta se coloca **en un hueco de la polilínea**: se parte el trazo en dos y el texto va
horizontal en el hueco, cada ~200 m de curva y solo en curvas maestras.

**Por qué.** No hay texto en un `Shape` y no hay texto sobre camino: una etiqueta rotada siguiendo
la curva es imposible sin rasterizar el texto a mano. El hueco con etiqueta horizontal es la
solución que usan los planos impresos y no requiere rotación.

**Consecuencias.** El corte se calcula en el commit (requiere conocer el ancho del texto, así que
se mide una vez con `rune::text::measure` y se cachea por `(fuente, tamaño, texto)`).

### 5.5 Líneas de cota

**Decisión.** Cota con líneas de extensión y flechas de tamaño constante en **puntos** (no en
metros): 8 pt de flecha, extensión de 4 pt más allá de la línea. El texto va centrado arriba y
siempre horizontal.

**Consecuencias.** A zoom bajo las cotas se agrupan y se descartan por decluttering (prioridad
baja). A zoom alto, la flecha no crece: es el comportamiento esperado en planos.

### 5.6 Leyenda

Panel izquierdo, generada desde el registro de familias: muestra color, patrón de línea y
símbolo. Se cachea con `.cache()` (subárbol estático, se re-rasteriza solo al cambiar tema).

---

## 6. Vista de perfil longitudinal

### 6.1 Datos y escalas

**Decisión.** Para un *run* (secuencia ordenada de pozos y tramos) se calcula:

```
chainage_i = Σ |P_{i} − P_{i−1}|            (m, horizontal)
terreno_i  = cota de rasante en el pozo
invert_i   = cota de clave inferior (invert)
clave_i    = invert_i + D_i                  (corona)
HGL_i      = de 03 (hidráulica); si falta, se dibuja “sin cálculo”
```

Dos escalas **independientes**:

```rust
let sx = plot_w / total_chainage;              // pt por metro de cadena
let sv = plot_h / (z_max - z_min).max(1.0);    // pt por metro de cota
let ve = sv / sx;                              // exageración vertical (se muestra: "VE ×5")
```

**Por qué independientes.** Un perfil de saneamiento real: 300 m de cadena con 8 m de desnivel.
A escala 1:1 el perfil es una línea recta ilegible. La exageración vertical es una convención de
la industria, y debe ser **explícita y visible** para que nadie mida pendientes con el escalímetro
equivocado.

**Consecuencias.** El eje de cotas se rotula con “números bonitos” (paso 1-2-5) y la etiqueta
`VE ×N` va siempre visible en la esquina. Cambiar `VE` (rueda con `Ctrl` sobre el perfil) solo
recalcula `sv` y reemite los trazos del perfil: es barato (cientos de primitivas).

### 6.2 Dibujo

```rust
fn profile_shape(p: &Profile, w: f32, h: f32) -> Shape {
    let (sx, sv) = (w / p.total_ch, h / (p.z_max - p.z_min));
    let x = |ch: f64| (ch * sx) as f32;
    let y = |z: f64|  (h - (z - p.z_min) * sv) as f32;

    let mut sh = Shape::new();
    // 1. rejilla y ejes (0.5 pt)
    for t in &p.z_ticks { sh = sh.line([0.0, y(t.z)], [w, y(t.z)], 0.5, LineCap::Butt, grid); }
    // 2. terreno (1.2 pt)
    sh = sh.stroke(&p.terrain.iter().map(|q| [x(q.ch), y(q.z)]).collect::<Vec<_>>(),
                   1.2, LineCap::Round, terrain);
    // 3. colector: barriles de espesor = diámetro real convertido con sv
    for b in &p.barrels {
        sh = sh.line([x(b.a_ch), y(b.a_z)], [x(b.b_ch), y(b.b_z)],
                     (b.d_m * sv) as f32, LineCap::Butt, sewer);
    }
    // 4. pozos: rectángulo de 12 pt de ancho, del terreno al invert
    for m in &p.manholes {
        sh = sh.stroke(&[[x(m.ch) - 6.0, y(m.terrain)], [x(m.ch) - 6.0, y(m.invert)],
                         [x(m.ch) + 6.0, y(m.invert)], [x(m.ch) + 6.0, y(m.terrain)]],
                       1.0, LineCap::Butt, manhole);
    }
    // 5. HGL: línea discontinua hecha a mano (no hay dash nativo)
    for seg in dash(&p.hgl, 6.0, 4.0) { sh = sh.line(seg.0, seg.1, 1.0, LineCap::Butt, hgl); }
    sh
}
```

**Por qué el barril es un trazo de ancho `D·sv`.** Así el diámetro es *físicamente correcto* bajo
la exageración vertical: una tubería de 0.30 m con `VE ×10` se ve 10× más gruesa, igual que el
terreno. Dibujarla con un ancho fijo en puntos mentiría sobre la relación diámetro/longitud.

**Consecuencias.** Con `D·sv < 1.5 pt` el barril se dibuja a 1.5 pt mínimo (legibilidad), y se
avisa en el HUD si eso ocurre.

### 6.3 Interacción

**Decisión.** Cursor vertical de lectura: `on_pointer_move_local` → cadena `ch` → interpola
terreno/invert/HGL y muestra profundidad, pendiente y velocidad en un chip con `LiveText`
(cero re-ejecuciones). Clic fija el cursor; `←/→` lo mueven un pozo.

**Consecuencias.** El perfil no necesita zoom propio: `Ctrl+rueda` cambia `VE`, `Shift+rueda`
hace zoom horizontal. Seleccionar un tramo en el perfil selecciona la tubería en planta y
viceversa (misma `ObjId`, un solo `selection`).

---

## 7. Paneles y formularios

### 7.1 Inspector dirigido por esquema

**Decisión.** El modelo expone descriptores; el inspector se genera por `match`, sin `match` sobre
nombres de atributo en la UI.

```rust
pub enum AttrKind {
    Num { unit: Unit, decimals: u8, min: Option<f64>, max: Option<f64> },
    Enum { options: Vec<Opt> },
    Multi { options: Vec<Opt> },
    Bool,
    Ref { domain: Domain },          // familia, capa, pozo aguas arriba…
    Text { max: usize, pattern: Option<&'static str> },
    ReadOnly,
}

pub struct AttrDesc { pub key: AttrKey, pub label: &'static str, pub kind: AttrKind, pub required: bool, pub help: &'static str }
```

```rust
fn form(s: &App, schema: &[AttrDesc]) -> El {
    let mut c = col().w_full().gap(10.0);
    for d in schema {
        let w = match &d.kind {
            AttrKind::Num { unit, .. }   => num_field(s, d, *unit),
            AttrKind::Enum { options }   => picker(s, d, options),
            AttrKind::Ref { domain }     => ref_picker(s, d, *domain),
            AttrKind::Bool               => toggle(s, d),
            AttrKind::Text { max, .. }   => text_input(&s.form.edits[&d.key], TextStyle::new(12.0, 400.0)),
            AttrKind::ReadOnly           => read_only(s, d),
            AttrKind::Multi { options }  => chips(s, d, options),
        };
        c = c.child(field_row(s, d, w));
    }
    c
}
```

**Por qué.** Añadir un atributo a una familia no debe tocar la UI. Y el mismo descriptor alimenta:
formulario, validación, tabla, importación CSV y la exportación a PDF (documento 06).

**Consecuencias.** `AttrKey` es un `enum`/`u16` interning, no `String`: comparar y hashear importa
en 100 000 filas.

### 7.2 Validación

**Decisión.** Se valida **al confirmar** (blur/Enter), no por pulsación. El resultado es un
`Diagnostic { severity, obj, attr, message, fix: Option<Fix> }` que alimenta a la vez el borde del
campo, el tooltip, la lista de diagnósticos del dock y el contador del statusbar.

**Por qué.** Validar por pulsación marca “PENDIENTE -3.5 %” como error mientras se escribe y
produce ruido. Un único tipo de diagnóstico evita tener tres listas de errores incoherentes.

**Consecuencias.** El campo inválido usa `.invalid(true)` (mapea a `aria-invalid` en AccessKit) y el
mensaje va en un `text()` bajo el campo, no en un tooltip: tiene que ser legible con teclado.

### 7.3 Tablas virtualizadas

**Decisión.** Virtualización con `.growth(GrowthSpec::default()).virtualize(24.0)` en el mismo
elemento, primer hijo del viewport de scroll; filas de 24 pt con `key` estable (`obj.id`); columnas
con ancho en `State`.

**Consecuencias.** Solo las filas montadas existen para layout, texto y accesibilidad (el árbol AX
expone las montadas más el total). `tx.scroll_into_view(key)` para “ir al objeto” desde el lienzo.

### 7.4 Paleta de comandos

**Decisión.** `Ctrl+K` abre un overlay con `.modal(true, true)` sobre el que se monta un
`text_input` + lista filtrada. `↑/↓` mueven, `Enter` ejecuta, `Esc` cierra (lo maneja `.modal`).

**Por qué.** `.modal` ya atrapa Tab, cierra con Escape, deja el fondo `inert` y **restaura el
elemento que lo abrió**. Reimplementarlo es la fuente clásica de foco perdido.

**Consecuencias.** El registro de comandos es el mismo que el de atajos: un comando = `{ id,
etiqueta, chord, disponible(state), ejecutar(state) }`. La paleta muestra el chord al lado.

### 7.5 Conjunto de atajos (contrato)

| Chord | Acción |
|---|---|
| `V` `T` `P` `N` `W` `D` | herramientas (seleccionar, tubería, pozo, nodo, válvula, cota) |
| `Espacio` (mantener) | pan temporal |
| `F3` / `F8` | osnap / orto |
| `Ctrl+Z` / `Ctrl+Shift+Z` / `Ctrl+Y` | deshacer / rehacer |
| `Ctrl+S` / `Ctrl+Shift+S` | guardar / guardar como |
| `Ctrl+O` / `Ctrl+N` | abrir / nuevo |
| `Ctrl+K` | paleta de comandos |
| `Ctrl+F` | buscar objeto (ID, código, dirección) |
| `F` / `Shift+F` | encuadrar todo / encuadrar selección |
| `Ctrl+0` / `Ctrl+1` | zoom 100 % de la escala de dibujo / 1:500 |
| `L` | perfil longitudinal del run seleccionado |
| `Supr` | eliminar selección |
| `Ctrl+A` | seleccionar todo lo visible |
| `F1` | ayuda contextual |
| `F12` | HUD de rendimiento (solo dev) |

**Regla.** Todo atajo tiene entrada en la paleta y en el menú; ningún atajo es la única vía a una
función (accesibilidad).

---

## 8. Theming y lenguaje visual

### 8.1 Tokens

**Decisión.** Se parte de `tok::theme(dark)` / `tok::accent(i, dark)` para la UI (fondo, texto,
bordes, controles) y se define una **paleta de dominio** propia, derivada, en OKLCH para que el
contraste se pueda razonar.

```rust
pub struct Palette {
    pub bg: Color, pub panel: Color, pub text: Color, pub text_dim: Color,
    pub water: Color, pub water_dim: Color,
    pub sewer: Color, pub sewer_dim: Color,
    pub storm: Color,          // pluvial
    pub existing: Color, pub proposed: Color, pub abandoned: Color,
    pub ok: Color, pub warn: Color, pub error: Color,
    pub grid: Color, pub contour: Color, pub contour_major: Color,
    pub selection: Color, pub hover: Color, pub snap: Color,
}
```

| Rol | Claro | Oscuro | Uso |
|---|---|---|---|
| agua potable | `#1F6FEB` | `#4D9BFF` | tuberías, válvulas, leyenda |
| alcantarillado sanitario | `#8A5A2B` | `#C08552` | colectores, pozos |
| pluvial | `#2E8B57` | `#4FBF86` | red de lluvias |
| existente | gris 55 % | gris 60 % | trazo continuo fino |
| proyectado | color de familia | color de familia | trazo continuo grueso |
| fuera de servicio | gris | gris | **discontinuo** + 60 % opacidad |
| error / aviso / ok | `#D64545` / `#E0A100` / `#2E9E5B` | versiones +10 L | badges, bordes, diagnósticos |

**Por qué colores por familia y no por diámetro.** El plano se lee por sistema (agua vs
alcantarillado); el diámetro va en la etiqueta. Codificar el diámetro por color produce 40 colores
indistinguibles.

**Consecuencias.** El daltonismo rojo-verde es frecuente: **nunca** se codifica solo por tono. Cada
familia tiene además patrón de línea y, en la leyenda, símbolo propio. Agua vs alcantarillado
difieren en luminancia, no solo en tono.

### 8.2 Convenciones de línea

| Familia | Estilo | Ancho | Símbolo |
|---|---|---|---|
| Agua potable | continuo | 1.4 pt | válvula = círculo con vástago |
| Alcantarillado sanitario | continuo | 1.7 pt | pozo = cuadrado redondeado |
| Pluvial | continuo | 1.7 pt | rejilla |
| Existente | continuo fino | 1.0 pt | hueco |
| Proyectado | continuo | 1.7 pt | relleno |
| Fuera de servicio | discontinuo 6/4 | 1.4 pt | aspa |

**Decisión.** El discontinuo se implementa con un helper `dash(poly, on, off)` que parte la
polilínea en pantalla, porque los trazos de `Shape` no tienen patrón.

**Consecuencias.** A zoom bajo, un patrón de 6/4 pt se convierte en ruido: por debajo de 0.5 px/m
todo se dibuja continuo y la distinción va por color/leyenda.

### 8.3 Accesibilidad

**Decisión.**
- Contraste mínimo: 4.5:1 texto, 3:1 elementos de UI y trazos con significado.
- Anillo de foco solo teclado: `.focus_ring(2.0, 2.0, accent)`, 2 pt, offset 2 pt.
- Cada control tiene `.role(..)` y `.label(..)`; los iconos sin texto llevan `.label("Cerrar")`.
- El lienzo declara `role("application")` y un `label` con el resumen (“Plano, 1 245 objetos
  visibles, escala 1:500”); además hay una **tabla de objetos navegable** que da acceso por teclado
  a todo lo que el ratón puede seleccionar.
- Cambios importantes (nuevo diagnóstico, fin de cálculo) se anuncian con
  `.live_region(LiveRegion::Polite)`.

**Por qué.** AccessKit está integrado (UI Automation en Windows) y se construye **solo cuando un
cliente lo pide**, así que no cuesta nada hasta que hay un lector de pantalla. Pero el lienzo es
intrínsecamente visual: sin la tabla paralela, la app es inaccesible por teclado.

**Consecuencias.** `--tree` incluye el árbol de accesibilidad: los roles y etiquetas se verifican en
CI (§9), no a ojo.

---

## 9. Arnés de verificación headless

### 9.1 Escenas

| Escena | Qué fija | Qué prueba |
|---|---|---|
| `empty` | proyecto vacío, `--chrome` | shell, paneles vacíos, estados “sin selección” |
| `net-1k` | red pequeña encuadrada | lo básico, etiquetas, leyenda |
| `net-100k` | red completa, 100 000 objetos | presupuesto de emisión, LOD de encuadre |
| `zoom-detail` | `scale = 40 px/m` | texto nítido, hatch, cotas, símbolos |
| `zoom-lod-matrix` | misma vista en 5 bandas | tabla de LOD §5.1 |
| `labels-dense` | 400 etiquetas candidatas | decluttering, estabilidad entre frames |
| `profile` | run de 12 pozos con HGL | escalas independientes, cotas, cursor |
| `inspector-error` | objeto con 2 validaciones fallidas | borde inválido, mensajes, `invalid` en AX |
| `palette` | `Ctrl+K` abierto | modal, trampa de Tab, `inert` del fondo |
| `tool-run` | tubería a medio dibujar con snap | goma elástica, chip, marca |
| `select-box` | caja de selección a medio arrastrar | `live_rect`, resaltado múltiple |
| `dark` / `light` | mismo estado, dos temas | contraste y tokens |

**Decisión.** Cada escena es determinista: estado inicial fijo, sin animaciones (se llama
`tx.settle()` / se desactivan transiciones), `Env { system_dark, scale }` explícito, reloj virtual.

### 9.2 Comandos

```powershell
# 1. PNG por escena, claro y oscuro
cargo run -p alc-desk --release -- --scene net-100k --snap out\net-100k.light.png --size 1440x900 --scale 1 --theme light --chrome
cargo run -p alc-desk --release -- --scene net-100k --snap out\net-100k.dark.png  --size 1440x900 --scale 1 --theme dark  --chrome

# 2. Matriz de LOD (5 bandas)
foreach ($s in 0.3,0.7,2,8,40) {
  cargo run -p alc-desk --release -- --scene zoom-lod-matrix --snap "out\lod-$s.png" --size 1440x900 --scale 1
}

# 3. Árbol de elementos + accesibilidad (JSON)
cargo run -p alc-desk --release -- --scene inspector-error --tree out\tree-inspector.json --at 0.1

# 4. Entrada guiada por script (JSON de Rune: move/down/up/drag/wheel/key/type)
cargo run -p alc-desk --release -- --script tests\scripts\draw-run.json --out out\frames --frames 24 --snap out\run-end.png --at 0.4

# 5. Presupuesto de commit: imprime cada commit > 8 ms
$env:RUNE_COMMIT_BUDGET_MS="8"; cargo run -p alc-desk --release -- --scene net-100k --script tests\scripts\pan-120.json --frames 120
```

**Script de paneo (`tests/scripts/pan-120.json`, extracto):** 120 eventos `drag` con el botón
medio. Es la prueba central de la decisión D5.

### 9.3 Aserciones en Rust

```rust
#[test]
fn paneo_no_reejecuta_la_vista() {
    let mut rt = Runtime::new(app(), App::view, (1440.0, 900.0), Env { system_dark: true, scale: 1.0 });
    rt.frame(1.0 / 60.0);
    let before = rt.live_only_events();
    for i in 0..60 {
        rt.pointer_down([720.0, 450.0]);      // botón medio (según mapa de botones del test)
        rt.pointer_move([720.0 + i as f32 * 4.0, 450.0]);
        rt.pointer_up([720.0 + i as f32 * 4.0, 450.0]);
        rt.frame(1.0 / 60.0);
    }
    assert!(rt.live_only_events() > before + 100, "el paneo debe ser live-only");
    assert!(rt.find("canvas.geo").is_some());
    assert!(rt.presented("canvas.geo").unwrap().w > 1400.0);
}

#[test]
fn etiquetas_estables_entre_frames() {
    let mut rt = Runtime::new(app(), App::view, (1440.0, 900.0), Env { system_dark: true, scale: 1.0 });
    rt.frame(1.0 / 60.0);
    let a = rt.snap_png();
    rt.frame(1.0 / 60.0);
    assert_eq!(a, rt.snap_png(), "un frame sin cambios no puede cambiar píxeles");
}
```

**Decisión.** Tres tipos de aserción, en este orden de preferencia:
1. **Estructural** (`rt.find`, `rt.presented`, `--tree` JSON): rápida y estable.
2. **Conteo de eventos** (`live_only_events()`): prueba la decisión de rendimiento, no la estética.
3. **Imagen** (`--snap` + diff perceptual): solo para regresiones visuales, con tolerancia.

**Por qué ese orden.** Los PNG son frágiles entre GPUs y drivers; si la CI corre en otra máquina,
un diff de imagen agresivo produce falsos positivos y se acaba ignorando. La estructura y los
contadores son deterministas.

### 9.4 Bucle de CI

```
fmt ─► clippy -D warnings ─► test (unit + Runtime) ─► snap --check ─► budget --check
                                   │                        │              │
                            lógica de UI           14 escenas PNG     commits < 8 ms
```

- Los PNG dorados se comparan con tolerancia (`ΔE ≤ 2` por canal, ≤ 0.1 % de píxeles distintos) y
  `--bless` los regenera.
- **Determinismo**: mismo `--size`, `--scale`, `--theme`, reloj virtual, sin animaciones. Aun así,
  los snapshots corren en un runner **auto-hospedado con GPU fija**; en un runner sin GPU el
  render por software cambia el antialiasing y las doradas no sirven.
- `--list-scenes` verifica que toda escena declarada se puede construir (ataja escenas rotas).

---

## 10. Lo que Rune no hace, y cómo lo cubrimos

| Limitación real | Impacto | Workaround | Riesgo |
|---|---|---|---|
| **No hay 3D** (sin búfer de profundidad; el compositor es 2D por capas) | Sin vista isométrica real | 2D + perfil + (opcional) isométrico por proyección afín dibujada con `Shape`, con orden de dibujo manual (painter's algorithm). Sin caras ocultas | Medio: el isométrico se ve mal con cruces |
| **No hay texto dentro de un `Shape`** | Etiquetas, cotas, rótulos de eje | Todas las etiquetas son elementos `text()` en overlay (§2.7) | Bajo: ya es el diseño |
| **No hay rotación de texto documentada** | Cotas inclinadas, rótulos sobre curva | Etiquetas horizontales + línea de referencia (leader). `[VERIFICAR]` si `El` admite rotación; si no, es una limitación permanente | Medio para planos “bonitos” |
| **Texto sobre camino** | Rótulo siguiendo una curva de nivel | Hueco en la polilínea + etiqueta horizontal (§5.4) | Bajo |
| **Sin `dash` nativo en trazos** | Líneas de proyecto/fuera de servicio | Helper `dash()` que parte la polilínea (§8.2) | Bajo (más segmentos) |
| **Sin clipping de `Shape`** | Hatch dentro de polígono, recortes | Hatch por intersección en CPU; recortes por construcción (no por máscara) | Bajo |
| **Sin PDF ni impresión** | Entrega de planos | Exportador propio headless (SVG/PDF) que **reutiliza la misma proyección y el mismo modelo de estilos**; el PDF no pasa por Rune | Medio: hay que mantener dos salidas |
| **Coste por elemento (~2 µs)** | 100 000 objetos | Presupuesto de 3–6 `Shape` + ≤250 etiquetas (§2.1) | Bajo, pero es la restricción que ordena todo |
| **`.cache()` limitado a 4096² px** | 4K a escala 2 | No cachear si `vp·pixel_scale > 4000` (§2.5) | Bajo |
| **`live_only` con chequeo a 250 ms** | Pan muy largo sin commit | Commit por umbral de 35 % de viewport (§2.5) | Bajo |
| **Atlas de texto y `f32` en GPU** | Precisión con coordenadas grandes | Reconstrucción en pantalla con `f64` + origen de sitio (§2.3) | Bajo |
| **`[patch.crates-io]` obligatorio** | Build en Windows | Parches a `wgpu-hal`, `winit`, `parley_engine`, `accesskit_windows`, `accesskit_macos` → `vendor/*` del repo de Rune | **Alto**: el clon vendorizado actual está roto (§Anexo A) |
| **Pre-release, API inestable** | Refactors caros | Rev fijada de `dev`; actualización como tarea con presupuesto, nunca “cargo update” a ciegas | Medio |

### Correcciones a supuestos previos del consejo

Tres cosas que se daban por limitadas **no lo son** (verificado en el fuente):

1. **Arrastrar y soltar desde el SO sí existe.** `.on_drop_files(..)` acepta archivos de Explorer
   (multiplataforma); `.drag_out(..)` (arrastrar *hacia* otras apps) es solo macOS. Importar
   `.csv`/`.dxf` arrastrando al lienzo es viable en Windows.
2. **IME sí está soportado**: preedit por winit, y el enrutado de teclas le da prioridad sobre
   todo lo demás (Return/Escape durante composición van a la composición).
3. **AccessKit sí está integrado** (UI Automation en Windows). `role`/`label` no son decorativos:
   construyen el árbol de accesibilidad real, y `--tree` lo escribe para testearlo.

---

## Anexo A — Checklist de verificación contra el vendor

Estado del árbol vendorizado en el momento de escribir: `vendor/rune` contiene **solo `.git`**
(clon interrumpido: `shallow.lock` presente, sin worktree, `git status` falla con “your current
branch appears to be broken”). Los objetos sí están y se pueden leer con
`git --git-dir=vendor\rune\.git show dev:<ruta>` (rev `78efd4a`). **Antes de aplicar los
`[patch.crates-io]`, hay que re-clonar el worktree.**

Verificado leyendo el fuente:

- [x] `.virtualize(est)` exige `.growth(..)` en el mismo elemento y ser el primer hijo del viewport
      (`README`, “Virtual rows” y “trampas de layout”).
- [x] `.grow()` = `flex: g 1 0`; `.scroll()` colapsa sin ancestro acotado; `.min_h(0.0)` obligatorio.
- [x] `on_press`/`on_hover`/`on_double_press` **no dan punto**; `on_pointer_move_local` y
      `on_wheel` dan punto **local**; `on_drag` da local + `start_x/start_y/w/h` + `dx/dy` de ventana.
- [x] `tx.live_only()` válido en `on_wheel`, `on_magnify`, `on_drag`, `on_pointer_move_local`,
      `on_key`, `on_press`, `on_hover`, `on_layout_tx`, `on_drawn_layout_tx`.
- [x] Chequeo de debug: live-only ≥ 250 ms tras el último commit re-ejecuta la vista.
- [x] `.cache()` compone bajo `live_offset`/`live_scale`, re-rasteriza al reposar 0.1 s, tope
      4096×4096 px de dispositivo, presupuesto LRU 64 MiB.
- [x] Pinza en Windows = `Ctrl+rueda` → `on_magnify` con `-dy/200`.
- [x] `.modal(click_outside, close)`: trampa de Tab, Escape, fondo `inert`, restaura el opener.
- [x] `Live<T>` (`f32`, `[f32;2]`, `[f32;4]`, `Color`) con `.set()`; `LiveText` para una línea sin
      re-layout; `live_offset/live_scale/live_rect/live_fill/live_text`.
- [x] `.focus_ring(w, offset, color)`, `.focus_group()`, `.tab_index(n)`, roles ARIA.
- [x] Herramientas headless: `--snap`, `--scene`, `--at`, `--size WxH`, `--scale`, `--theme`,
      `--tree` (JSON con accesibilidad), `--list-scenes`, `--script` (JSON con `wheel`, `magnify`,
      `drag`, `key`, `type`, `resize`, `chord`), `--video`, `--trace`, `RUNE_COMMIT_BUDGET_MS`.
- [x] Bench de Rune: 6 000 elementos ⇒ 13.7 ms con re-ejecución vs 2.0 ms con `live_only`.

Pendiente de verificar en el fuente (marcado `[VERIFICAR]` en el texto):

- [ ] Pivote de `live_scale` (origen vs centro): la fórmula `zoom_about` asume **origen**; ajustar si
      es centro. Verificable con `cargo run -p rune --example canvas_input`.
- [ ] Rotación de texto en `El` (¿existe `.rotate()` o similar?). Si no existe, confirmar el
      workaround de etiquetas horizontales como definitivo.
- [ ] Si `memo(key, deps, ..)` evita re-ejecutar la clausura cuando `deps` no cambia (lo asumimos
      para la memoización de geometría; el fuente lo describe como “reruns memoized subtrees” solo
      ante cambios de fuente).
- [ ] Coste real de emisión de un `Shape` con 20 000 cápsulas en Windows/D3D12 (el bench publicado
      es Apple silicon). Medir con `RUNE_COMMIT_BUDGET_MS`.
- [ ] Comportamiento de `.on_wheel` cuando `dx ≠ 0` en Windows (rueda horizontal real).

## Anexo B — Riesgos, ordenados por impacto

| # | Riesgo | Prob. | Impacto | Mitigación |
|---|---|---|---|---|
| R1 | El clon de Rune está roto y bloquea el build | Alta | Alto | Re-clonar `vendor/rune` y fijar rev; script `scripts/fetch-rune.ps1` idempotente |
| R2 | 100 000 objetos no caben en el presupuesto de frame en D3D12 | Media | Alto | Escena `net-100k` en CI desde el día 1; LOD más agresivo; presupuesto de 20 000 segmentos |
| R3 | Sin rotación de texto, los planos no cumplen la convención del cliente | Media | Medio | Validar con el cliente; si es bloqueante, exportar PDF con el rotador propio (no con Rune) |
| R4 | Snapshots de CI inestables por GPU/driver | Alta | Medio | Runner con GPU fija; aserciones estructurales primero; imagen con tolerancia |
| R5 | Cambios de API de Rune rompen el build | Alta | Medio | Rev fijada; actualización como tarea planificada con su propia rama |
| R6 | PDF/impresión sin soporte nativo | Alta | Medio | Exportador headless propio reutilizando proyección y estilos |
| R7 | `.cache()` supera 4096² en 4K | Media | Bajo | Regla de no cachear (§2.5) |
| R8 | Deriva de grosor durante zoom | Baja | Bajo | Banda `[0.80, 1.25]` + commit al asentarse |

## Anexo C — Estructura de módulos propuesta

```
crates/
  app/        # main, AppSpec, escenas, CLI (--scene/--snap/--tree/--script)
  shell/      # topbar, paneles, dock, statusbar, paleta, atajos
  canvas/     # cámara, tiles, LOD, emisión de Shape, overlay, snapping
  profile/    # vista longitudinal
  index/      # rejilla espacial, picking, box select
  model/      # datos y descriptores de atributo (doc 01)
  diag/       # validación, comandos, undo/redo
  export/     # SVG/PDF/csv (headless)
```

**Frontera.** `canvas/` no conoce el modelo de negocio más allá de `ObjId`, `FamilyId` y las
polilíneas ya proyectadas; `model/` no conoce Rune. Esto permite testear el LOD y la emisión sin
GPU, y el exportador sin UI.
