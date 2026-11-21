# 00 — Investigación: Rune, el framework de UI

> Documento de referencia. Todo lo que aquí se afirma se verificó contra el repositorio clonado
> en `vendor/rune` (revisión `78efd4a7876b5478dd72033ce6fd2b2f13ab9f9a`, rama `dev`) y contra
> una compilación real ejecutada en esta máquina Windows el 2026-11-18, incluida una vista 3D
> renderizada headless. Lo que no se pudo verificar está marcado como **[sin verificar]**.
>
> **Nota de alcance.** El proyecto empezó como un *diseñador* de redes (con hidráulica y
> excavación) y terminó como un **visor 3D** que importa modelos ya calculados. Rune se usa
> igual en ambos casos: lo que cambió es qué se dibuja, no cómo. Las notas sobre el API siguen
> siendo válidas; las que hablaban de herramientas de dibujo 2D se leen ahora como historia.

---

## 1. Qué es Rune

Rune es un framework de UI nativo para Rust, renderizado por GPU, creado por **Anomaly**
(los autores de OpenCode). Es *pre-release*: la versión publicada es `0.0.1` y **no se
distribuye en crates.io**; se consume como dependencia git fijada a una revisión de `dev`.

| Dato | Valor |
|---|---|
| Repositorio | <https://github.com/anomalyco/rune> |
| Licencia | MIT, © Anomaly |
| Estado | Pre-release. `dev` es *fast-forward-only*: las apps fijan una rev y esa rev nunca se reescribe |
| Plataformas | macOS, Windows, Linux, web (wasm) e iOS |
| Stack | `wgpu` (GPU), `winit` (ventanas), `Taffy 0.15` (layout flex/grid/block), `Parley` + `swash`/`skrifa` (texto), `AccessKit` (accesibilidad) |
| Render | Una pasada instanciada de "uber-quad" (SDF) + atlas de glifos; un draw call por frame |
| Animación | Muelles analíticos (`rune-motion`), con retargeting que conserva posición y velocidad |
| Modelo de vista | Funciones Rust puras que devuelven un árbol de elementos; el runtime hace diff y conserva el estado de layout |

La consecuencia de diseño más importante para nosotros: **la vista se vuelve a ejecutar
completa ante cada evento** (salvo que el handler pida lo contrario, ver §6), y el runtime
difiere contra el árbol anterior. No hay "componentes con estado propio" ni retención de
nodos por parte del usuario: el estado vive en nuestro `State`, y el árbol es una función
de ese estado.

### 1.1 Por qué encaja con este proyecto

1. **Todo se dibuja con formas vectoriales propias** (`Shape`/`Path`): polilíneas, rellenos,
   trazos con extremos redondos. Un plano de redes es exactamente eso.
2. **Modo headless de primera clase**: cualquier app puede renderizar a PNG, volcar el árbol
   de elementos a JSON y recibir entrada sintética por script. Eso es lo que permite que un
   agente IA verifique su propio trabajo, y es la base de nuestra estrategia de pruebas.
3. **Cámara de canvas resuelta**: `live_offset` + `live_scale` mueven el mundo sin re-ejecutar
   la vista ni re-layoutar, pensado para mapas y visores de imágenes.
4. **Un solo binario nativo** para Windows: sin runtime web, sin Electron, sin Node.

---

## 2. Cómo se integra (receta verificada)

### 2.1 Toolchain

- Windows con toolchain **MSVC** (`x86_64-pc-windows-msvc`, el default de rustup). Verificado
  con `rustc 1.95.0` y `cargo 1.95.0`; edición 2024.
- No hace falta MinGW ni `dlltool` en MSVC. (En GNU sí: ver el README de Rune.)

### 2.2 `Cargo.toml` mínimo que funciona

```toml
[package]
name = "mi-app"
version = "0.1.0"
edition = "2024"

[dependencies]
rune = { path = "../../vendor/rune/crates/rune" }   # o git + rev

# Un `[patch]` de una dependencia NO aplica al workspace que la consume:
# toda app que compile Rune repite los parches de Rune.
[patch.crates-io]
wgpu-hal          = { path = "../../vendor/rune/vendor/wgpu-hal" }
winit             = { path = "../../vendor/rune/vendor/winit" }
parley_engine     = { path = "../../vendor/rune/vendor/parley_engine" }
accesskit_windows = { path = "../../vendor/rune/vendor/accesskit_windows" }
accesskit_macos   = { path = "../../vendor/rune/vendor/accesskit_macos" }
```

Los cinco parches son obligatorios también en Windows (los de macOS se declaran igual y
cargo avisa que no se usan; es inofensivo). `tiny-skia-path` **no** es un parche: Rune lo
referencia como *path dependency* a `vendor/tiny-skia-path` desde su propio manifiesto, así
que basta con que el checkout exista.

> **Decisión de vendorizado.** Usamos `path` a un clon en `vendor/rune` en vez de `git` para
> que la compilación sea reproducible sin red y para poder leer el código del framework. La
> rev queda registrada en `vendor/rune/REVISION.txt` y en este documento.

### 2.3 Tiempos medidos en esta máquina (Windows, AMD Radeon integrada, Dx12)

| Operación | Resultado |
|---|---|
| `cargo check` inicial (todas las dependencias) | ~6 min |
| `cargo build` del binario de prueba (deps ya compiladas, `rune` en debug) | **2 min 34 s** |
| Tamaño del `.exe` debug | 57 MB |
| Init de GPU + primer frame | 635 ms / 20,4 ms |
| `--snap` headless (1 frame, 640×520 pt a 2×) | PNG 1280×1040, 11 KB |
| `--tree` headless | JSON con árbol de accesibilidad + árbol de elementos |

El `--snap` headless **sí** inicializa GPU (no hay renderizador por software en el camino
normal); en esta máquina funciona y es determinista.

---

## 3. El modelo de programación

### 3.1 Anatomía de una app

```rust
use rune::{AppSpec, Cx, El, col, text};

#[derive(Default)]
struct State { on: bool }

fn view(s: &State, _cx: &mut Cx<State>) -> El {
    col().w_full().h_full().gap(16.0)
        .child(text(if s.on { "encendido" } else { "apagado" }))
        .child(
            rune::el().key("switch").size(38.0, 22.0)
                .on_press(|s: &mut State| s.on = !s.on),
        )
}

fn main() {
    rune::run(AppSpec {
        scenes: &[("on", |s: &mut State| s.on = true)],
        ..AppSpec::new("Mi app", (900.0, 600.0), State::default, view)
    });
}
```

- `AppSpec::new(título, tamaño_inicial, init, view)`. `init` es `Fn() -> S` (o `S` con `Default`).
- `AppSpec` tiene campos opcionales que se pueden poblar: `scenes` (estados nombrados para
  snapshots), `poll`, `on_key`, `setup`, `on_ime`, `fonts`, `min_size`, `titlebar`, `chrome`…
- Los **handlers** se encolan y corren entre frames con `&mut State`; después la vista se
  re-ejecuta y el diff reintenta las pistas de animación. Hover y press *no* re-ejecutan la
  vista: solo mueven pistas visuales.

### 3.2 Constructores de elementos

| Constructor | Qué hace |
|---|---|
| `el()` | caja genérica |
| `row()` / `col()` | flex horizontal / vertical |
| `text(s)` | texto de una línea; estilos con `.font(TextStyle)`, `.color(c)`, `.wrap()`, `.truncate()`, `.mono()` |
| `rich(s, spans)` | texto con spans (color, peso, mono) |
| `canvas(shape)` | dibuja un `Shape` en las coordenadas propias del elemento |
| `spacer()`, `rule(w, c)` | separador flexible / línea |
| `grid(cols, cell_w, cell_h, rows)` | rejilla tipo terminal (útil para tablas densas) |
| `text_input(&edit, style)` | campo de texto enlazado a un `TextEdit` del estado |
| `icon(&Icon, size)` | icono vectorial |

Métodos de `El` que usaremos (verificados en `crates/rune/src/el/`):

- **Layout**: `.w/.h/.size/.w_full/.h_full/.grow/.w_frac/.min_w/.min_h/.max_w/.max_h`,
  `.padding/.px/.py/.pt/.pb/.pl/.pr`, `.mx/.my/.mt/.mb/.ml/.mr`, `.gap/.gaps`,
  `.center/.center_y/.justify_between/.justify_end/.items_*/.self_*`,
  `.absolute(x,y)/.pin(l,t,r,b)`, `.shrink()`, `.grid_cell(n)`.
- **Visual**: `.fill/.radius/.corners/.border/.border_sides/.shadow/.box_shadow/.gradient/
  .linear_gradient/.radial_gradient/.opacity/.blur/.scale/.rotate/.origin/.clip_rounded/
  .clip_soft/.cache()/.cache_key()`.
- **Interacción**: `.on_press/.on_press_tx/.on_double_press/.on_middle_press/.on_drag/
  .on_drag_start/.on_drag_end/.on_secondary_press(_local)/.on_pointer_move(_local)/
  .on_hover/.on_hover_change/.on_wheel/.on_magnify/.on_drop_files/.pointer_buttons(..)`.
- **Teclado y foco**: `.focusable/.tab_index/.focus_group/.focus_selected/.on_key/
  .on_key_up/.modal(..)/.default_button()`.
- **Accesibilidad**: `.role("button")/.label(..)/.checked(..)/.disabled(..)/.expanded(..)`.
  El árbol de accesibilidad es también lo que `--tree` serializa.
- **Ciclo de vida**: `.key(..)` (identidad estable, necesaria para el diff y para `--tree`),
  `.motion(..)`, `.transition(..)`, `.hover_group()`, `.cursor(Cursor::Pointer)`.

### 3.3 Layout: las tres trampas de flexbox

Rune usa Taffy, es decir, CSS flexbox. Tres comportamientos que cuestan horas:

1. `.grow(g)` es `flex: g 1 0`: base 0, el elemento mide solo el espacio libre que le deja el
   padre. En una columna de altura automática no hay espacio libre y un `.scroll(..)` con
   `.grow(1.0)` colapsa a 0 pt. Hay que acotar la altura de un ancestro (`.h(..)`,
   `.h_full()`) o poner `.min_h(..)`.
2. Los hijos flex tienen mínimo de tamaño de contenido: una columna anidada desborda en vez
   de encogerse y el scroll interno nunca scrollea. Se arregla con `.min_h(0.0)` en cada
   columna entre el ancestro acotado y el viewport.
3. `.virtualize(..)` solo funciona sobre un *growth stack*: hay que llamar
   `.growth(GrowthSpec)` en el mismo elemento y hacerlo el primer hijo del viewport de scroll.

---

## 4. Dibujo vectorial: `Shape` y `Path`

```rust
use rune::{Color, LineCap, Path, Shape, canvas};

let s = Shape::new()
    .line([0.0, 0.0], [200.0, 100.0], 2.0, LineCap::Round, Color::WHITE)   // segmento con extremos
    .stroke(puntos.clone(), 1.5, Color::WHITE)                              // polilínea
    .fill(poligono, Color::WHITE)                                           // relleno (ear-clipping)
    .stroke_path(&path, 3.0, LineCap::Round, Color::WHITE)                  // Bézier aplanada
    .fill_path(&path, Color::WHITE)
    .view_box(200.0, 100.0);                                                // sistema propio estirable

col().child(canvas(s).key("plano").size(800.0, 600.0));
```

Hechos relevantes:

- Un trazo es **un cuádruple analítico por segmento** (`KIND_SEGMENT`); un relleno son
  triángulos sin costuras. No hay atlas: un `Shape` puede cambiar en cada frame.
- `Path` soporta `move_to`, `line_to`, `quad_to`, `cubic_to`, `close`; se aplana a segmentos.
- `Shape` puede animarse con `.drawn(t)` (trazo progresivo) y `.trimmed(a, b)`.
- El coste escala con el número de segmentos y de primitivas, no con el número de elementos.
  **Esto es lo que decide nuestra estrategia de dibujo**: agregaremos la red en pocos `Shape`
  (uno por capa/familia) en lugar de un elemento por objeto.

---

## 5. Texto

- Fuente embebida Inter (Regular/Medium/SemiBold); se pueden registrar fuentes propias con
  `AppSpec::fonts` y seleccionarlas con `TextStyle::new(13.0, 600.0).family(Family::Named("..."))`.
- Tokens en `rune::tok`: `theme(dark)` (colores), `accent(i, dark)`, `type_::{BODY, LABEL,
  SECTION, TITLE}`, `space::{S1..S6}`, `radius::{SM, MD, LG}`, y la paleta de movimiento
  (`motion::{SNAPPY, DEFAULT, CALM}`).
- El texto se rastrea a la escala del dispositivo (Retina) y se re-rasteriza nítido al
  asentarse una escala; hay modos de suavizado (`FontSmoothing`) por plataforma.
- Para etiquetas que cambian a alta frecuencia existe `LiveText` (no re-layouta).

---

## 6. Canvas, cámara y coordenadas

El patrón que Rune documenta para mapas y visores infinitos:

```rust
// en el estado: dos valores "live" que el frame dibuja sin re-ejecutar la vista
cam_pan:  Live<[f32; 2]>,
cam_zoom: Live<f32>,

// en la vista
el().key("mundo")
    .live_offset(&s.cam_pan)
    .live_scale(&s.cam_zoom)
    .on_drag(|s: &mut S, d: &Drag, tx: &mut Tx| {
        s.pan_by(d.dx, d.dy);      // mueve los Live
        tx.live_only();            // <- no re-ejecuta la vista, no re-layouta
    })
    .on_magnify(|s: &mut S, e: MagnifyEvent, tx: &mut Tx| {
        s.zoom_about(e.x, e.y, 1.0 + e.delta);
        tx.live_only();
        true
    })
```

Reglas de coordenadas:

| Handler | Punto que entrega | Espacio |
|---|---|---|
| `on_drag` | `Drag::x, y`, `start_x, start_y` | local (puntos del elemento, ya des-escalados) |
| `on_wheel` | `WheelEvent::x, y` | local |
| `on_magnify` | `MagnifyEvent::x, y` | local |
| `on_pointer_move_local` | `PointerEvent::x, y` | local |
| `on_layout` / `on_drawn_layout` | `[x, y, w, h]` | ventana |
| `on_press` | *(no entrega punto)* | — |

Consecuencia: para saber **dónde** hizo clic el usuario hay que combinar `on_pointer_move_local`
(que sí da punto local) con `on_press`, o usar `on_drag` con `dx = dy = 0`. Y como el canvas
es un solo elemento, el hit-testing de objetos lo hacemos nosotros contra un índice espacial.

`tx.live_only()` es la optimización clave de un CAD: pan y zoom no deben reconstruir el árbol.
Los eventos que solo mueven la cámara llaman `live_only()`; el frame dibuja los nuevos valores,
hace hit-testing contra ellos y reporta `on_drawn_layout`.

---

## 7. Tooling headless (la base de nuestra estrategia de verificación)

Cualquier binario Rune acepta estas banderas en build debug (y en release con la feature
`tooling`):

| Bandera | Efecto |
|---|---|
| `--snap ARCHIVO.png` | renderiza un frame a PNG |
| `--tree ARCHIVO.json` | vuelca el árbol de accesibilidad + árbol de elementos con `path`, `rect`, `fill`, `key`, estados de foco |
| `--scene NOMBRE` | aplica un estado nombrado de `AppSpec::scenes` antes de renderizar |
| `--script ARCHIVO` | reproduce entrada sintética (clics, arrastres, rueda, teclas, texto) sobre un reloj virtual |
| `--at SEGUNDOS` / `--at mark:NOMBRE±S` | fija el instante del frame |
| `--trace ARCHIVO` / `--trace-all` | traza de frames, commits, layout |
| `--window CLAVE`, `--list-windows`, `--list-scenes` | selección de ventana / descubrimiento |
| `--strip`, `--video`, `--html`, `--desktop` | tira de frames, video, página estática, escritorio virtual |
| `--crop CLAVE`, `--whole-window`, `--chrome`, `--keys`, `--pointer` | recorte, overlays de depuración |

Ejemplo verificado en esta máquina:

```powershell
cargo run -p mi-app -- --snap plano.png --scene demo --at 1.5
cargo run -p mi-app -- --tree arbol.json --scene demo
```

El `--tree` produce JSON con dos vistas: `accessibility` (roles, nombres, bounds, acciones) y
`elements` (path, rect, fill, opacity, flags de foco/inert). Es suficiente para que un agente
verifique que un panel existe, que un objeto está en pantalla y que un botón es pulsable.

### 7.1 Pruebas sin ventana (`Runtime`)

```rust
use rune::{Env, Runtime};

let mut rt = Runtime::new(State::default(), view, (900.0, 600.0), Env { system_dark: true, scale: 2.0 });
rt.frame(0.0);
let (x, y) = rt.find("switch").unwrap();
rt.pointer_down(x, y);
rt.pointer_up(x, y);
for _ in 0..6 { rt.frame(1.0 / 60.0); }
assert!(rt.state.on);
let rect = rt.presented("canvas").unwrap();
```

`Runtime` da `frame(dt)`, `find(key)`, `presented(key)`, `pointer_down/up/move`, `wheel`,
`key`, `update`, y acceso al `state`. Es la vía para pruebas de UI deterministas en CI, sin GPU
visible y sin reloj real.

---

## 8. Rendimiento: reglas que nos obligan a diseñar distinto

1. **Nada de I/O de archivos en el bucle de frame.** Nunca leer, escribir, `stat` ni renombrar
   durante frames, entrada o animación. En Windows cada apertura dispara filtros antivirus.
   → El guardado, la importación de DXF y el cálculo pesado van a **hilos de trabajo**; la UI
   recibe resultados por estado y los aplica con `tx.instant()`.
2. `poll` debe devolver `true` **solo** si cambió algo visible; una ventana ociosa no dibuja.
3. Construir solo las filas montadas (`virtual_rows` + `.growth(..)`) y memoizar lo que cambia
   poco (`memo`).
4. Leer tamaños con `Cx::layout_size` en vez de rects completos para no reconstruir en cada scroll.
5. Acotar toda caché en entradas y bytes (`.cache()` usa un presupuesto LRU de 64 MiB por defecto).
6. Un handler que solo mueve valores `live` llama `tx.live_only()`.

Para nuestro plano esto se traduce en: **el modelo y la topología viven fuera del árbol de UI**;
la vista consume una "escena de dibujo" ya proyectada y agregada; pan/zoom no reconstruyen.

---

## 9. Límites y riesgos (lo que Rune no hace)

| Necesidad | Estado | Mitigación |
|---|---|---|
| 3D / modelo BIM | No existe | Fuera de alcance v1; vista 2D + perfil longitudinal |
| Texto rotado siguiendo una polilínea | No nativo (sí `.rotate(radians)` por elemento) | Etiquetas como elementos rotados; para curvas, segmentar |
| Miles de etiquetas | Cada una es un elemento con layout | LOD: etiquetas solo por encima de un umbral de zoom; `memo` |
| Impresión / PDF | No nativo | Exportar SVG/PNG por composición propia y/o DXF; PDF vía crate |
| Selección de texto en canvas | Manual | Implementar caret/selection propios si se edita en canvas |
| Diálogos nativos de archivo | Feature `native-file-dialog` (rfd) | Activar la feature o hacer un explorador propio |
| Estabilidad de API | Pre-release, rev fijada | Fijar `78efd4a`; actualizar Rune es un proyecto aparte, con pruebas headless como red |
| Documentación | README extenso + ejemplos + código fuente | Este dossier + leer `crates/rune/src/el/*` |

Riesgo mayor: **Rune es pre-release y no acepta pull requests**. Si encontramos un bug de
framework, el camino es abrir un issue. Mitigación: encapsular todo uso de Rune en un crate
(`agua-ui`) con una capa fina propia, para que un cambio de framework no toque el dominio.

---

## 10. Qué copiamos de los ejemplos

| Ejemplo | Qué demuestra | Qué tomamos |
|---|---|---|
| `examples/hello` | app mínima, `scenes`, test con `Runtime` | esqueleto inicial |
| `examples/shapes` | `Shape`, `Path`, `canvas`, `memo`, `view_box` | dibujo del plano |
| `examples/lab` / `sketch` | entrada de canvas, cursores, trazos | herramientas de dibujo |
| `examples/settings` | layout de app completa con paneles | arquitectura de la ventana |
| `examples/kit-gallery` | catálogo de componentes `rune-kit` | botones, campos, listas |
| `examples/live-values` | `Live` con productores en otros hilos | cámara y progreso |
| `examples/background` | `App` con hilos y ciclo de vida | trabajos largos |

---

## 11. Checklist de "¿esto funciona en Rune?" antes de prometer una feature

- ¿Cabe en un `El` con layout flex? Si necesita posicionamiento libre, `.absolute()` o un
  `canvas` con `Shape`.
- ¿Se dibuja como geometría? → `Shape`/`Path`.
- ¿Necesita texto que no se deforme al hacer zoom? → elemento de texto posicionado, no dentro
  del `Shape` escalado.
- ¿Necesita reaccionar al puntero con coordenadas? → `on_pointer_move_local` / `on_drag` /
  `on_wheel` / `on_magnify`.
- ¿Es una operación larga? → hilo + `AppHandle::update` + `tx.instant()`.
- ¿Se puede verificar headless? Si no, la feature no entra: definimos una `scene` y un
  `--snap`/`--tree` que la prueben.
