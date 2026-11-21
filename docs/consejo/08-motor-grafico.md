# 08 — Motor gráfico: qué dibuja hoy, qué puede dibujar y qué no

**Autor:** `motor` (consejo de arquitectura) · **Fecha:** 2026-11-18 · **Estado:** propuesta para revisión
**Ámbito:** el visor 3D de **Redes** (`redes-app`, Rust + Rune, Windows primero), su núcleo `redes-core` y la CLI `redes`.
**Depende de:** 01 (modelo, coordenadas, `origin` local), 04 (composición de la ventana, regla «los elementos son el recurso escaso»), 05 (producto y plazos).

> **Método.** Todo lo que sigue sobre Rune está leído del checkout vendorizado
> (`vendor/rune`, rev **`78efd4a`**, 2026-11-18) y comprobado con `cargo`/ejecución real en esta
> máquina. Todo lo que sigue sobre rendimiento está **medido hoy en esta máquina** con el binario
> `--release` ya compilado (`target/release/redes-app.exe`, 23,4 MB) y con un banco aparte que
> llama a `redes_core::view::build_with` directamente. Los comandos están en el §9 para que
> cualquiera los repita. Lo que no pude verificar va marcado `[VERIFICAR]` y listado en el Anexo A.

---

## 0. Índice de decisiones

| # | Decisión | Sección |
|---|---|---|
| **D1** | La v1 se queda con el renderer propio (proyección en CPU + algoritmo del pintor sobre `Shape` de Rune). No se cambia de motor para entregar. | 3 |
| **D2** | El presupuesto se cuenta en **primitivas visibles**, no en objetos ni en segundos: **≈0,75 µs por primitiva** medida (usar **1 µs** como cifra de diseño), techo de 60 fps en ~20 000 y de 30 fps en ~40 000. | 6 |
| **D3** | **three.js no entra en la app.** Sí se usa (si hace falta) como visor de cortesía del GLB exportado, fuera del producto. | 3.3 |
| **D4** | Si algún día hace falta 3D real, la ruta es un **pase wgpu dentro de Rune** (`Compositor` + `El::effect` + `EffectPhase::Backdrop`), no un motor externo ni un webview. | 2.3, 4 |
| **D5** | **glTF/GLB** entra como tercera salida (junto a DXF y GeoJSON) y como seguro ante un cambio de motor. | 5 |
| **D6** | LOD por **tamaño en píxeles** (ya existe para pozos) y presupuesto por frame; el coste de la ciudad de demostración es **90 % contexto urbano, 1 % red**, así que la palanca es el contexto. | 1.4, 6.2 |
| **D7** | Sin materiales, sin texturas, sin sombras: color plano por familia + sombreado por normal. «Forma, no fotorrealismo» es el requisito y también la decisión técnica. | 3.1 |
| **D8** | La escena se reconstruye solo cuando cambia cámara, capas, selección o revisión; una ventana quieta no dibuja (ya es así y se conserva). | 1.3, 2.2 |
| **D9** | El «no hay búfer de profundidad» se declara **límite conocido** del visor, no bug: a la escala de una red (tubos de 110–600 mm, pozos de 1,2 m) no se percibe, y el detalle fino se entrega al CAD. | 1.3 |
| **D10** | Se conserva el criterio de producto: **una sola app nativa, un solo `.exe`, sin navegador, sin instalador**. Ninguna opción que rompa esto entra en la v1. | 3 |

---

## 1. Qué hay hoy, exactamente

### 1.1 La cadena de dibujo, en cinco pasos

```
*.redes (JSON)                     redes-core::model      Project, Node, Link, Terrain, Entorno
      │
      ▼
view::Camera  ──►  view::build_with(&project, &terrain, &cam, layers, selected) -> Scene { Vec<Prim> }
      │                 · proyección en perspectiva con recorte de plano cercano (project_segment)
      │                 · triángulos de terreno sombreados por normal + tinte por cota
      │                 · tubos = segmentos con ancho proporcional al diámetro
      │                 · pozos = cilindro de 12 caras, o rombo de tamaño fijo si miden < 3 px
      │                 · ciudad = calles, aceras y manzanas troceadas cada 12 m
      │                 · orden: de lejos a cerca (algoritmo del pintor), por capas
      ▼
redes-app:  un único `Shape::new()` con  shape.fill(pts, color)  y  shape.line(a, b, w, cap, color)
      │
      ▼
Rune:  tesela rellenos y trazos, los emite como instancias a la GPU (wgpu 30.0.2, Dx12 aquí)
      │
      ▼
Ventana.  El texto, los paneles, el inspector y la barra de estado son elementos de Rune,
          no geometría de la escena.
```

Dos propiedades de este diseño que conviene subrayar porque son las que lo hacen barato de mantener:

1. **`view` no dibuja: produce datos.** `Vec<Prim>` es la misma escena que consume la app y la que
   el exportador SVG de la CLI (`redes vista --salida x.svg`) convierte en vectores. Por eso un
   agente puede «ver» el modelo sin GPU y las pruebas comparan escenas sin GPU.
2. **Un solo `Shape` por frame.** No hay una shape por objeto: hay una lista de rellenos y trazos
   ya ordenada. Eso es lo que evita el coste de miles de elementos de UI.

### 1.2 Qué es Rune y qué no es (comprobado en el código)

Rune es un framework de **UI** en Rust sobre `wgpu` + `winit` (su propio README: *«A Rust GPU UI
framework (wgpu, winit) for apps that feel like SwiftUI on iOS»*). Lo que eso significa para nosotros:

| Comprobación | Resultado | Dónde |
|---|---|---|
| ¿Tiene cámara 3D, `mat4`, luces de escena? | **No.** No hay tipo de escena 3D ni matrices en la API. La cámara es nuestra (`view::Camera`, en `redes-core`). | `crates/rune/src/lib.rs` (superficie pública) |
| ¿Tiene búfer de profundidad? | **No.** Los render passes se abren con `depth_stencil_attachment: None`. | `render.rs:931–953`, `1109–1113` |
| ¿Qué puede dibujar una `Shape`? | Relleno de polígonos y trazos con ancho y remate: `fill`, `stroke`, `line`, `stroke_path`, `fill_path`. Nada de materiales, UVs ni texturas. | `shape.rs:62–111` |
| ¿Cómo resuelve el orden? | Orden de inserción (pintor). El z-buffer no existe, así que la app ordena antes de emitir. | `view.rs:597–618` |
| ¿Tiene antialiasing? | Sí, analítico en el shader (`sample_count: 1`, sin MSAA). | `render.rs:1160` |
| ¿Se puede pintar algo propio con la GPU? | **Sí, desde la API pública**: `Compositor`, `El::effect`, `Renderer::draw_with`, `EffectPhase::Backdrop`. Ver §3.2. | `compose.rs:34–137`, `el/effect.rs:43–72` |

Ese último renglón es la noticia importante de este documento y por eso tiene sección propia.

### 1.3 Qué implica: ventajas y límites

| Ventaja | Consecuencia real |
|---|---|
| Una sola app nativa, un solo `.exe` de **23,4 MB** sin instalador ni runtime | El cliente copia un archivo y funciona. No hay navegador, ni WebView, ni `node_modules`. |
| Cero dependencias web | Arranque en frío ~0,3 s de proceso + ~110–240 ms de init de GPU; funciona sin red, en obra, en un portátil con la GPU integrada. |
| La CLI no depende de Rune | `redes.exe` pesa **1,9 MB** y produce la misma vista en SVG sin GPU: los agentes y el CI ven lo mismo que el humano. |
| Control total del orden y del LOD | El «modo rayos X» (la red se ve a través del terreno, pero queda oculta tras una fachada) es una decisión de producto que se implementa en 4 líneas de ordenación. En un motor 3D genérico eso es un shader de profundidad y una discusión. |
| La escena es un dato | Se puede volcar, comparar en pruebas, exportar a SVG y (ver §5) a glTF sin tocar el renderer. |

| Límite | Efecto | Mitigación actual |
|---|---|---|
| Sin búfer de profundidad | Intersecciones reales entre sólidos pueden ordenarse mal (un tubo que entra en un pozo, dos tubos que se cruzan) | Partición por capas (terreno → red → ciudad) y orden por profundidad dentro de cada una. Se documenta como límite. |
| Sin materiales ni texturas | No hay asfalto, ni hormigón, ni agua, ni sombras | Sombreado por normal + color por familia + niebla por distancia. Es lo que hay y es suficiente para leer una red. |
| Coste CPU por primitiva | ~0,4 µs por primitiva solo en la proyección propia; ~1 µs contando Rune | LOD, capas apagables, terreno aligerado (`Terrain::thinned`), y (nuevo, §7) presupuesto por frame. |
| La escena se rehace entera en cada gesto | Arrastrar la cámara recalcula y reordena todo | No hay animación de fondo; el coste se paga por evento, no 60 veces por segundo. |
| Texto dentro de la escena es caro | Etiquetas de pozo en el lienzo no son viables a esta escala | El detalle va al inspector y al CAD. |

### 1.4 Lo que se midió hoy en esta máquina

Máquina: GPU **AMD Radeon(TM) Graphics** (integrada), `wgpu` sobre **Dx12**, ventana 1280×800 puntos
a `--scale 2` (2560×1600 píxeles de dispositivo). Todo en `--release`.

**(a) Coste de la proyección propia** (`view::build_with`, banco que llama al núcleo directamente,
30 repeticiones, mediana; no incluye Rune):

| Escena | Primitivas | `build_with` p50 | Marginal |
|---|---|---|---|
| 2 000 pozos + 1 999 arcos | 3 999 | 1,13 ms | — |
| **Ciudad demo, TIN aligerado** | **5 328** | **2,07–2,31 ms** | — |
| 6 000 pozos + 5 999 arcos | 11 999 | 4,47 ms | — |
| 15 000 pozos + 14 999 arcos | 29 999 | 11,61 ms | — |
| 40 000 pozos + 39 999 arcos | 79 999 | 31,11 ms | **≈0,39 µs / primitiva** |

Desglose de la ciudad de demostración (misma cámara, capas encendidas de a una):

| Capa | Primitivas | Coste | % |
|---|---|---|---|
| Terreno (TIN aligerado) | 2 016 | 0,57 ms | 25 % |
| Ciudad: 10 calles + aceras + 108 manzanas | 3 225 | 1,51 ms | **65 %** |
| Red: 46 nodos + 41 arcos | 87 | 0,02 ms | **1 %** |
| **Total** | **5 328** | **2,31 ms** | |

> **Hallazgo 1.** El contexto urbano cuesta 65 veces más que la red. Si mañana el cliente trae
> 10 000 pozos, el problema no es la red: es no dibujar 108 manzanas detalladas cuando están a
> 1 500 m. La palanca está en el LOD del contexto, no en cambiar de motor.

**(b) Coste de extremo a extremo dentro de la app** (Rune informa `frame cpu mean/p50` de cada
frame de un guion de arrastre real: 2 arrastres × 1 s ≈ 206 frames, cámara orbitando de verdad —
se comprobó que el azimut pasa de −55° a −41° y la distancia de 1 422 a 884 m):

| Configuración | Instancias dibujadas | `p50` del frame de arrastre | fps implícito |
|---|---|---|---|
| **Ciudad demo** (al abrir) | 2 542 | **2,37 ms** | 60 (limitado por el reloj) |
| 6 000 pozos, vista de calle (cilindros de 12 caras) | 47 999 | **40,8 ms** | 24 |
| 15 000 pozos, vista general | 45 617 | **33,4 ms** | 30 |
| 40 000 pozos, vista general | 120 617 | **89,5 ms** | 11 |

De ahí sale la regla del presupuesto: **≈0,75 µs por instancia visible** en el camino completo
(0,39 µs nuestros + ~0,35 µs de Rune: teselado, instancias, upload).

**(c) El frame frío, que es el número que circula como «25 ms»**:

| Caso | `--release` | `--debug` |
|---|---|---|
| Primer frame de la ciudad demo (`--snap --scene ciudad`) | 20,0 ms | 222,8 ms |
| Primer frame, escenas iso / planta / detalle / calle / perfil | 18,9 – 24,0 ms | — |
| Init de GPU (una vez por proceso) | 110 – 240 ms | ~180 ms |

> **Hallazgo 2.** Los **25 ms / 230 ms** del informe son el **frame frío** (arranque, o el primero
> tras abrir/cambiar de escena), no el coste de cada frame de arrastre: en caliente, la ciudad de
> demostración orbita a **2,4 ms**. El frame frío paga una vez la compilación de pipelines, el
> atlas de fuentes y la primera subida de buffers. Esto cambia el diagnóstico: **no estamos
> gastando 25 ms por frame, estamos gastando 25 ms una vez y **≈0,75 µs por primitiva visible.** El
> techo real son las primitivas en pantalla, no los milisegundos de arranque.

**(d) Otros números que afectan al diseño**

| Dato | Medida | Consecuencia |
|---|---|---|
| Carga del JSON (1 MB / 87 objetos) | 2,0 ms | Irrelevante para el demo. |
| Carga del JSON (9,2 MB / 30 000 objetos) | 38 ms | Aceptable. |
| Carga del JSON (24,6 MB / 80 000 objetos) | 98 ms | A 100 000 objetos hay que pensar en binario o carga por regiones (§6.2 paso 6, riesgo R5). |
| `Terrain::thinned` (TIN completo → dibujo) | 0,88 ms | Barato; se puede bajar más el umbral si hace falta. |
| `redes vista` (TIN completo, 8 787 primitivas) | 12–13 ms, SVG de 644 KB | El camino sin GPU sigue siendo usable por agentes. |

---

## 2. Las cinco opciones, con números

### 2.1 Tabla comparativa

| | **(a) Seguir y mejorar** | **(b) Pase 3D wgpu dentro de Rune** | **(c) three.js en webview / app web** | **(d) Exportar glTF** | **(e) Motor 3D en Rust (bevy/rend3/wgpu puro)** |
|---|---|---|---|---|---|
| **Qué es** | Pulir el pintor actual: LOD, presupuesto, sombreado de tubos, orden por capas | Un `Compositor` propio que abre su propio render pass con profundidad en el rect del lienzo | Una vista WebGL/WebGPU dentro de un WebView2, o una web aparte | Un archivo `.glb` que se abre en Blender / Visor 3D / three.js | Rehacer el visor sobre bevy/rend3, o wgpu a mano |
| **Esfuerzo** | **1–2 semanas** (1 persona) | **3–5 semanas** (1 persona) | Webview: 6–10 semanas y deuda permanente. Web aparte: 4–6 semanas | **3–5 días** | 3–6 meses |
| **Rendimiento esperado** | 60 fps hasta ~20 000 primitivas visibles; hoy el demo usa 5 328 | 60 fps con **1–2 M triángulos** (GPU); deja de importar el orden | 60 fps con 100 k–1 M triángulos instanciados; +1 frame de latencia por el webview | N/A (no dibuja en la app) | 60 fps con millones de triángulos |
| **Gana** | Nada estructural; +2–3× de techo y mejor lectura | Profundidad real, sólidos reales, AA propio, misma app y mismo `.exe`, misma UI, mismo tooling (`--snap` sigue funcionando) | Materiales, texturas, sombras, picking 3D, ecosistema y ejemplos | Interoperabilidad total sin tocar la app; seguro ante cualquier cambio de motor | Todo el 3D moderno, ECS, física, animación |
| **Pierde** | El techo sigue siendo CPU; el orden por profundidad sigue siendo aproximado | 3–5 semanas y ~1 500–2 500 líneas de GPU que hay que mantener; MSAA y AA hay que ponerlos nosotros | Un solo `.exe` de 23 MB se convierte en app + WebView2 (aquí: **1 757 MB en disco, 2 049 archivos** del runtime, versión 157.0.4341.60); dos lenguajes, dos modelos de estado, IPC para selección/hover, `node`/`npm` en el build, y la CLI deja de tener paridad | No es un visor: el cliente sale de la app | Se rehace UI, texto, accesibilidad y tooling: es otro producto |
| **Riesgo** | Bajo | **Medio**: depende de una API de un framework pre-release (rev fijada) | **Alto**: dos runtimes, dos estados, y el ciclo de actualización de WebView2 | Bajo | **Muy alto**: 3–6 meses sin entregar |
| **¿Rompe «un solo .exe sin navegador»?** | No | No | **Sí** | No | No, pero rompe el plazo |
| **Veredicto** | **Ahora** | **Después, si hace falta** | **No** | **Sí, ya** | **No** |

### 2.2 (a) Seguir con el renderer propio y mejorarlo

Lo que se puede hacer sin cambiar de arquitectura, por orden de retorno:

| Mejora | Qué cuesta | Qué gana | Detalle |
|---|---|---|---|
| **LOD del contexto urbano** | 2–3 días | Elimina el 65 % del coste del demo | Manzanas a más de N metros: caja sin cubierta y sin caras por trozo; calles: una polilínea en vez de quads + aceras. Es el mayor retorno de toda la lista. |
| **Presupuesto por frame con degradación** | 2–3 días | Techo predecible en vez de sorpresas | Si `prims > budget`, bajar caras de cilindro (12 → 8 → 6 → rombo) y saltar aceras; el usuario no lo nota a esa escala. |
| **Culling por frustum antes de proyectar** | 1–2 días | 10–30 % en vistas de calle | Hoy se proyecta todo y se descarta después de proyectar. Filtrar por bbox contra el frustum es barato. |
| **Tubos con dos quads sombreados en vez de una línea** | 3–4 días | Lectura tridimensional sin motor 3D | Una línea tiene ancho constante en pantalla; un tubo de 6 m de largo y 400 mm de diámetro se lee mejor con dos quads con sombreado por cara. Sigue sin haber profundidad real. |
| **`memo` por (cámara, capas, selección)** | 1 día | Evita reconstruir cuando solo se repinta | Rune reejecuta `view()` en cada evento; memoizar la `Shape` por clave ahorra el 100 % en repintados sin cambio. |
| **Orden por profundidad con clave de objeto** | 2 días | Menos artefactos de solape | Ordenar por `depth` de la primitiva es ya lo que se hace; añadir un sesgo por capa (terreno < red < ciudad) y por tipo resuelve los solapes típicos. |

**Total: ~2 semanas** para bajar el demo de 2,3 ms a ~1,0–1,3 ms, subir el techo a ~40 000
primitivas en escenas de contexto y mejorar la lectura. Es lo que recomiendo para la entrega.

### 2.3 (b) Un pase 3D real con wgpu dentro de la app

**Sí es posible, y no es un hack: es una API pública de Rune.** Verificado en el checkout fijado:

```rust
// vendor/rune/crates/rune/src/compose.rs
pub trait Compositor<S: 'static>: 'static {
    fn features(&self) -> wgpu::Features { … }
    fn render(&mut self, f: &mut Frame<'_, S>);      // f.device, f.queue, f.encoder, f.target, f.size
    fn animating(&self) -> bool { false }            // false: la ventana descansa y no dibuja
    fn draws_own(&self) -> bool { true }
    fn summary(&mut self, device: &wgpu::Device) -> Option<String> { None }
}

pub struct Frame<'a, S> {
    pub device: &'a wgpu::Device,     // ← el mismo device de Rune, no uno nuevo
    pub queue: &'a wgpu::Queue,
    pub encoder: &'a mut wgpu::CommandEncoder,
    pub target: &'a wgpu::TextureView,
    pub format: wgpu::TextureFormat,
    pub size: (u32, u32),
    pub ui: &'a mut Renderer,
    …
}
```

y el enganche por elemento:

```rust
// vendor/rune/crates/rune/src/el/effect.rs
pub fn effect(self, mark: u32) -> El              // este elemento es un «slot» que pinta el compositor
pub fn effect_data(self, data: [f32; 4]) -> El    // payload que llega al compositor (cámara, selección…)
pub fn effect_backdrop(self, on: bool) -> El      // el pass se cierra en este slot: podemos abrir el nuestro

// vendor/rune/crates/rune/src/render.rs
pub fn draw_with(&self, encoder, view, clear, hook: &mut EffectHook<'_>)
pub enum EffectPhase<'a> { Draw(&'a mut wgpu::RenderPass<'a>), Backdrop(&'a mut wgpu::CommandEncoder) }
```

Los detalles que importan para estimar el esfuerzo, todos comprobados:

| Detalle | Comprobado |
|---|---|
| El *scissor* del slot lo pone Rune: nuestro pass solo pinta dentro del rect del lienzo. | `render.rs:974–990`: «Scissor to the slot's clip rectangle, so a compositor that ignores `clip` still…», y se resetea al acabar. |
| Con `effect_backdrop(true)` recibimos el **encoder con el pass cerrado**, que es lo que hace falta para abrir un pass propio **con attachment de profundidad**. | `compose.rs` (doc de `draw_with_backdrop`), `render.rs` (`EffectPhase::Backdrop`). |
| Funciona también **headless** (`--snap`, `--tree`, `--script`): los mismos ganchos se ejecutan en la ventana y en el render offscreen. | `compose.rs:43–45`: «Runs after `Runtime::frame` built the instances, in the window and headless alike». |
| La app **no** necesita un device nuevo: es el de Rune. | `Frame::device`. Un device propio duplicaría memoria y no permitiría compartir texturas. |
| Hay que declarar `wgpu` en la app con **la misma versión que Rune** (`30.0.2` en `Cargo.lock`), sin reexport. | `vendor/rune/crates/rune/Cargo.toml`: `wgpu = "~30.0.1"`; los ejemplos que usan compositor lo añaden a su `Cargo.toml`. |
| La app pasa de `rune::run(AppSpec{…})` a `rune::App::new(AppSpec{…}).compositor(Mi3D::new()).run()`: **5 líneas**, `AppSpec` no tiene campo `compositor`. | `tooling.rs:17–60` (campos de `AppSpec`), `app.rs` (`App::new(spec)`, `.compositor(..)`, `.run()`). |
| Los passes de Rune van a **`sample_count: 1`** (su antialiasing es analítico, en el shader): nuestro pass 3D saldría con bordes duros. | `render.rs:1160`. Hay que añadir FXAA/SMAA o supersampling propio: 2–4 días. |
| El clip redondeado del lienzo (`clip_rounded(8.0)`) es del shader de Rune; nuestro pass no lo hereda. | El *scissor* es rectangular. Solución: lienzo sin radio, o una máscara de Rune encima. |

**Qué habría que escribir** (firmas, no implementación):

```rust
// crates/redes-app/src/gpu3d.rs  (~1 500–2 500 líneas con pruebas)
pub struct Escena3D { … }                 // buffers, pipeline, depth, cámara en mat4
impl Escena3D {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self;
    pub fn subir(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, malla: &Malla3D);
    pub fn dibujar(&self, pass: &mut wgpu::RenderPass<'_>, vista: Mat4, proj: Mat4, rect: [f32; 4]);
}
impl rune::compose::Compositor<State> for Escena3D {
    fn render(&mut self, f: &mut rune::compose::Frame<'_, State>) { … }
}

// crates/redes-core/src/mesh.rs  (~400 líneas, sin GPU, testeable)
pub struct Malla3D { pub vertices: Vec<Vert>, pub indices: Vec<u32> }
pub fn teselar(project: &Project, lod: Lod) -> Malla3D;   // pozos: cilindro; tubos: barrido del perfil
```

**Rendimiento esperado.** Un pozo con 12 caras son ~44 triángulos; 100 000 pozos son 4,4 M
triángulos, que en esta iGPU a 60 fps es justo el límite; con LOD (6 caras a distancia) baja a
~1,5 M y sobra. Lo importante: **el techo deja de ser la CPU** y pasa a ser la GPU, y el orden por
profundidad deja de ser un problema.

**Riesgo principal.** Depende de una API de un framework *pre-release* que no acepta PRs. Mitigación:
la rev está fijada (`78efd4a`), el uso queda confinado a `redes-app/src/gpu3d.rs`, y **el camino
2D se conserva** como respaldo (`--sin-gpu3d`), de modo que si Rune rompe el compositor, la app
sigue funcionando con el renderer de hoy. Ese respaldo es obligatorio en el diseño, no opcional.

### 2.4 (c) three.js en una vista web embebida, o app web aparte

**Lo que three.js hace mejor que nosotros, sin discusión:** materiales y texturas, sombras,
*instancing*, picking por rayo en GPU, post-proceso, y un ecosistema de ejemplos que ahorra semanas.
Si el requisito fuera «que se vea como en la realidad» con asfalto, hormigón y agua, three.js
sería la respuesta correcta y lo diría sin rodeos. **No es el requisito:** el cliente pidió que las
piezas «se vean como en la realidad **en forma**, no fotorrealismo».

**Lo que cuesta meterlo aquí:**

| Coste | Dato de esta máquina |
|---|---|
| Un segundo runtime en el escritorio | WebView2 **157.0.4341.60** instalado; **1 757 MB en disco en 2 049 archivos** (todas las versiones). No viaja en el `.exe`, pero es un segundo motor que hay que soportar y actualizar. |
| Una segunda toolchain | `node`/`npm` en el build; un bundler; y aun así el `.exe` no contiene la vista: la carga de un archivo local o de un `dist/`. |
| Dos modelos de estado | Selección, hover, capas, estados de revisión y cámara existen en Rust y en JS; cada gesto cruza el IPC (≈1 frame de latencia, suficiente para que el hover «flote»). |
| Se duplica la UI o se parte | O se rehace el inspector en HTML (perdiendo el inspector tipado y el `--tree`), o se mantiene Rune a un lado y el lienzo al otro, con dos verdades sobre qué está seleccionado. |
| Peritaje | El equipo tendría que mantener Rust **y** TypeScript/three.js. Con un equipo pequeño, eso es el riesgo real, no el rendimiento. |
| Se pierde la paridad con la CLI | Hoy `redes vista` y la app comparten `view::build`. Con three.js el SVG de la CLI y lo que ve el humano dejan de ser el mismo código. |

**Rendimiento:** three.js *no degradaría* el rendimiento; lo mejoraría en geometría (100 k–1 M
triángulos instanciados a 60 fps es rutina). El cuello de botella de hoy no es el rasterizador: es
que proyectamos 5 328 polígonos en CPU y el 65 % son manzanas de contexto. Cambiar de motor no
arregla eso; el LOD sí.

**Veredicto:** no para la app. Sí como **visor de cortesía**: el GLB del §5 se abre en una página
three.js de 40 líneas (o en cualquier visor público) y se comparte por enlace. Ahí three.js es
exactamente la herramienta correcta, sin tocar el producto.

### 2.5 (d) Exportar glTF

No compite con las demás: es una salida. Detalle completo en §5.

- **Esfuerzo:** 3–5 días (exportador + pruebas + verificación).
- **Gana:** interoperabilidad inmediata (Blender, Visor 3D de Windows, three.js, Revit/Navisworks
  vía importadores), y un **seguro**: si algún día se cambia de motor, el modelo ya está en el
  formato que todos los motores leen.
- **Pierde:** no es un visor. El cliente sale de la app para verlo, y pierde el inspector y los
  estados de revisión… salvo que los metamos en `extras` (lo haremos, §5.4).

### 2.6 (e) Motor 3D en Rust (bevy / rend3 / wgpu puro)

| | bevy | rend3 | wgpu puro |
|---|---|---|---|
| Madurez | Alta, pero en movimiento rápido (API cambia entre versiones) | Mantenimiento irregular | Es lo que ya usa Rune |
| Trae UI propia | Sí (bevy_ui): habría que **rehacer** paneles, inspector, texto, accesibilidad | No: habría que escribir UI | No: habría que escribir todo |
| Encaja con lo que hay | Mal: dos runtimes de ventana, dos relojes, dos sistemas de input | Mal | Bien a nivel GPU, mal a nivel UI |
| Esfuerzo de migración | 3–6 meses | 3–5 meses | 4–6 meses |

**Por qué no:** el 80 % de esta app no es 3D: es UI tipada, inspector, revisión por objeto, CLI y
tooling headless. Un motor 3D no aporta nada de eso y obliga a rehacerlo. La opción (b) da el mismo
resultado gráfico **conservando** ese 80 %. Si algún día bevy madura lo suficiente como para tener
UI y accesibilidad equivalentes, la decisión se revisa; hoy no.

---

## 3. Recomendación

### 3.1 Para el objetivo del cliente y el plazo de un equipo pequeño

**Fase 1 — entregar con lo que hay (ahora, 2 semanas).**
Nada de cambiar de motor. Se hace la lista del §2.2, en este orden: **LOD del contexto urbano**,
presupuesto por frame con degradación, culling por frustum, `memo` por estado de cámara, y solo si
sobra tiempo, tubos con dos quads sombreados. Resultado esperado: la ciudad demo baja de 2,3 ms a
~1,0–1,3 ms de proyección, y el frame frío deja de asustar (o se explica).

El techo sube **solo en escenas dominadas por contexto** (la demo, que es el caso de hoy):
de ~20 k a ~40 k primitivas visibles a 60 fps. En una escena dominada por **red** —10 000 pozos en
primer plano— el LOD urbano no ayuda: hay que bajar caras de pozo y trocear por regiones (§6.2,
pasos 3–6). Decirlo así importa, porque el cliente va a crecer por el lado de la red.

**Fase 2 — glTF (3–5 días, en paralelo o justo después).**
`redes exportar gltf --salida ciudad.glb`. Es barato, es un seguro y es una función que el cliente
puede enseñar a su cliente.

**Fase 3 — el pase 3D, solo si el uso lo pide (3–5 semanas, después de la entrega).**
Disparadores concretos, no capricho:
1. el cliente trae redes de más de 20 000 objetos y quiere verlas todas a la vez;
2. aparecen quejas de solapes/orden (un tubo que «atraviesa» un pozo);
3. se pide ver el interior de los pozos, o tubos con diámetro real a corta distancia;
4. se pide que el modelo se vea «como en la realidad» con más que color plano.

Mientras no se cumpla uno, (b) es dinero que no se recupera.

### 3.2 Por qué (b) y no (c)/(e), en una frase cada una

- **(b)** añade profundidad y sólidos reales **sin** añadir un runtime, un lenguaje ni un modelo de
  estado: el `.exe` sigue siendo uno, la UI sigue siendo Rune, el tooling headless sigue igual, y
  el camino 2D queda como respaldo.
- **(c)** mejora el rendimiento **geométrico** (es su terreno), pero lo paga con dos runtimes, dos
  lenguajes y dos modelos de estado: complejidad permanente para un problema que el LOD arregla en
  dos días.
- **(e)** tira el 80 % del producto (UI, inspector, CLI, tooling) para reescribir el 20 %.

### 3.3 Decisión sobre three.js (explícita)

> **three.js no entra en la aplicación.** No porque sea peor —para 3D es mejor que lo que
> tenemos— sino porque el producto es un `.exe` nativo con CLI, sin navegador, y meterlo
> significa un segundo runtime (1,7 GB de WebView2 en esta máquina), un segundo lenguaje, un
> segundo modelo de estado y perder la paridad app/CLI. Se usa three.js **fuera** del producto,
> como visor del `.glb` exportado, que es donde su ecosistema es una ventaja sin coste.
> **Se revisa** si el producto se convierte en un visor web compartido: en ese escenario three.js
> (o babylon.js) pasa a ser la elección correcta y este documento se reescribe.

### 3.4 Lo que se le responde al cliente (texto para copiar)

> El motor gráfico es **Rune**, un framework de interfaz en Rust que dibuja con la GPU (DirectX 12,
> Vulkan o Metal). Rune es 2D: no trae cámara 3D ni búfer de profundidad, así que el 3D lo hacemos
> nosotros —proyectamos la geometría en CPU y la dibujamos ordenada de atrás hacia delante, como se
> ha hecho siempre en los visores de ingeniería—. Eso es lo que permite que sea **una sola
> aplicación nativa de 23 MB, sin instalador, sin navegador y sin dependencias web**, y que la misma
> vista se pueda generar desde la línea de comandos para que la revise un agente.
> Los 25 ms que se midieron son el **arranque**, no el coste de cada frame: girando la ciudad de
> demostración el coste real es de 2,4 ms por frame. El techo práctico está en unas **20 000 piezas
> visibles a la vez a 60 fps** y 40 000 a 30 fps; por encima de eso, el siguiente paso no es
> three.js sino un pase 3D con profundidad real dentro de la misma app, que ya está identificado y
> presupuestado (3–5 semanas) para cuando el tamaño de las redes lo pida.
> three.js es mejor que lo nuestro para materiales, texturas y sombras. Si el objetivo fuera
> fotorrealismo, sería la respuesta. Para «que las piezas se vean como son en forma», no compra
> nada que no tengamos y sí cuesta un segundo motor dentro de la app.

---

## 4. Ruta de migración si algún día hace falta un motor 3D real

La clave: **el motor gráfico es la capa más reemplazable del sistema**, porque `view` no dibuja,
produce datos. Lo que se conserva es todo lo que cuesta meses; lo que se reescribe es lo que cuesta
semanas.

| Se conserva tal cual | Se reescribe | Por qué |
|---|---|---|
| `model`, `families`, `topology`, `ingest`, `export`, `terrain`, `project_io` | — | No saben nada de dibujo. |
| `view::Camera` (órbita, encuadre, `z_scale`, FOV, `pan`/`dolly`) | La proyección: de `project()` a `Mat4` de vista/proyección | La matemática de la cámara es la misma; solo cambia dónde se multiplica. |
| La política de capas (`Layers`) y el modo rayos X | El orden por profundidad → z-buffer | La profundidad pasa a ser del hardware; el rayos X pasa a ser un shader de profundidad en el terreno. |
| El LOD por píxeles (rombo < 3 px) | El cálculo de tamaño en pantalla, que pasa al shader o a la matriz | La regla de producto («un pozo siempre se ve») se conserva; cambia la implementación. |
| El picking por CPU (`State::pick`) | Nada, si el picking por CPU sigue siendo suficiente (lo es: 100 k objetos = ~2 ms) | No hace falta picking por GPU a esta escala. |
| La CLI completa, el sobre JSON, la skill de ingesta, los exportadores | — | No tocan la GPU. |
| `redes vista --salida x.svg` | — | El camino sin GPU se queda como referencia y como prueba. |
| El LOD y el presupuesto del §7 | — | Sirven igual en GPU. |

| Se reescribe | Coste |
|---|---|
| El renderer (`view::build` → `mesh::teselar` + `gpu3d`) | 3–5 semanas |
| El orden por profundidad (deja de existir) | 2 días |
| El antialiasing (hoy es del shader de Rune; pasa a FXAA/SMAA o MSAA propio) | 2–4 días |
| Las pruebas de escena (hoy comparan `Vec<Prim>`; pasan a comparar mallas y a leer píxeles) | 1 semana |
| El respaldo 2D (se queda como `--sin-gpu3d`) | 0 (ya está) |

**Regla de oro para que la migración sea posible:** todo lo que sea **política de dibujo** (qué se
dibuja, con qué color, con qué LOD, en qué orden) se queda en `redes-core` y no en la app. Hoy se
cumple casi siempre; el único sitio donde se rompe es el color por familia, que está duplicado en
`view.rs` y que debería vivir en `families` (§5.3).

---

## 5. glTF: cómo exportar el modelo

### 5.1 Formato y decisión

| Aspecto | Decisión | Motivo |
|---|---|---|
| Contenedor | **`.glb`** (binario, JSON + BIN en un archivo) por defecto; `.gltf` + `.bin` con `--separado` | Un archivo se manda por correo y se abre en todas partes. |
| Versión | glTF **2.0** | Es lo que leen Blender 4/5, el Visor 3D de Windows, three.js y Navisworks. |
| Extensiones | **Ninguna obligatoria.** Opcional `KHR_materials_unlit` con `--plano` | Las extensiones son lo que rompe la compatibilidad. `EXT_mesh_gpu_instancing` se evita a propósito: Blender no lo importa. |
| Unidades | **Metros** (la unidad de glTF) | El modelo ya está en metros. Sin conversión. |
| Eje | glTF es **Y-arriba, diestro**. Nuestro modelo es **Z-arriba** | Se hornea la rotación −90° sobre X al exportar (el importador de Blender la deshace y devuelve Z-arriba). |
| Precisión | Restar `project.origin` a todas las posiciones | Posiciones en `f32`: a 760 000 m se resuelven ~6 cm. Ya lo hacemos al dibujar; hay que hacerlo igual al exportar. |
| Procedencia | `asset.extras = { schema, crs, origin_x, origin_y, revision, generador }` | Sin esto el archivo deja de ser auditable. |

### 5.2 Mapeo del modelo a la jerarquía glTF

```
Redes                                (nodo raíz; extras: crs, origin, revision)
├── Terreno                          (una malla; material "terreno", vértices con color por cota)
├── Ciudad
│   ├── Calles                       (una malla fusionada)
│   └── Manzanas                     (una malla fusionada por uso, o un nodo por manzana con --detallado)
├── Saneamiento
│   ├── PZ-001 … PZ-046              (nodo por objeto: malla de cilindro compartida + traslación)
│   ├── CL-001 … CL-041              (nodo por objeto: malla de tubo propia)
│   └── …
├── Agua potable
│   └── …                            (misma estructura)
└── Apoyo                            (cruzues, hitos)
```

| Elemento del modelo | Geometría glTF | Material | `extras` del nodo |
|---|---|---|---|
| `Node` con capacidad `hydraulic_node` (pozo, cámara, tanque) | Cilindro cerrado, radio `diametro()/2`, altura `altura()` | por familia | `code`, `family`, `ty`, `estado`, `attrs`, `datos` |
| `Node` `attachable` (válvula, llave, medidor) | Caja pequeña o cilindro corto sobre el arco | por familia | ídem |
| `Link` (tubería, canal, sifón) | Barrido del perfil circular a lo largo de `path` con radio `d_mm/2000`; tapas en los extremos | por familia | `code`, `family`, `d_mm`, `estado`, `attrs`, `datos` |
| `Terrain` | La misma malla del TIN que se dibuja (`tris` + `points`) | `terreno` | `name`, `version` |
| `Entorno.calles` | Quads de calzada (sin aceras en el GLB) | `calle` | — |
| `Entorno.manzanas` | Prisma por manzana (planta × altura) | `manzana_comercio` / `manzana_vivienda` | `uso`, `altura` |

**Clave para que el archivo no explote:** una sola malla de cilindro, referenciada por los 46 nodos
de pozo (`nodes[i].mesh = 0`, `translation` distinto). glTF permite reutilizar mallas; así un
proyecto con 10 000 pozos pesa lo que pesan 10 000 traslaciones, no 10 000 cilindros.

### 5.3 Colores por familia: una sola fuente de verdad

Hoy el color está en `view.rs` (agua `(0.33, 0.63, 0.96)`, saneamiento `(0.87, 0.60, 0.31)`, más
verde de selección y rojo de observado) y el exportador necesitaría los mismos. **Antes de escribir
el exportador, mover la paleta a `families`** (`FamilyDef::color: [f32; 3]` + `color_estado()`), de
modo que app y glTF lean lo mismo. Es un cambio de 1 día y evita dos paletas que se separan con el
tiempo.

Material glTF por familia: `pbrMetallicRoughness` con `baseColorFactor` de la paleta,
`metallicFactor: 0`, `roughnessFactor: 0.9`, `doubleSided: false`. Estado de revisión: un material
`observado` (rojo) y `pendiente` (ámbar) que sustituye al de familia cuando el objeto lo está; el
`estado` queda igualmente en `extras` para filtrar.

### 5.4 Interfaz propuesta (firmas y comandos)

```rust
// crates/redes-core/src/export_gltf.rs  (nuevo, ~600 líneas, sin GPU, sin dependencias nuevas)
pub struct OpcionesGltf {
    pub solo_aprobados: bool,   // entrega solo lo aprobado (igual que el DXF)
    pub con_terreno: bool,
    pub con_ciudad: bool,
    pub detallado: bool,        // un nodo por manzana y por calle, en vez de mallas fusionadas
    pub plano: bool,            // KHR_materials_unlit en vez de PBR
    pub separado: bool,         // .gltf + .bin en vez de .glb
}
pub fn gltf(p: &Project, opt: &OpcionesGltf) -> Result<Vec<u8>, String>;  // bytes del .glb
pub fn gltf_a_json(p: &Project, opt: &OpcionesGltf) -> serde_json::Value; // para las pruebas
```

```text
redes exportar gltf --salida ciudad.glb [--solo-aprobados] [--sin-terreno] [--sin-ciudad]
                    [--detallado] [--plano] [--separado]
```

Se añade a `ExportarCmd` como tercera variante (junto a `Dxf` y `Geojson`), con su código de
comando `exportar.gltf` en el sobre JSON y su `hint_command`, igual que las otras dos.

### 5.5 Cómo verificarlo (con lo que hay en esta máquina)

| # | Verificación | Comando | Criterio |
|---|---|---|---|
| 1 | Estructura del contenedor y del schema, sin GPU | `npx --yes gltf-validator ciudad.glb` | 0 errores, 0 warnings de severidad alta |
| 2 | **Ida y vuelta sin dependencias** (la que va al CI) | prueba en `redes-core`: leer el `.glb` con un lector propio (cabecera + chunk JSON), y comparar | nº de nodos de red == objetos aprobados; bbox == bbox del proyecto menos `origin`; cada nodo tiene `extras.code` |
| 3 | Abre en Blender | `blender --background --python-expr "import bpy; bpy.ops.import_scene.gltf(filepath=r'C:\...\ciudad.glb'); print(len(bpy.data.objects), len(bpy.data.meshes))"` | Blender **5.1.0** verificado en esta máquina; imprime objetos y mallas > 0 |
| 4 | Abre en el Visor 3D de Windows | `start ciudad.glb` | Se ve con volumen, no plano |
| 5 | Abre en three.js / visor público | arrastrar el `.glb` a un visor glTF | Idem |
| 6 | Coherencia con la app | comparar `redes vista --salida v.svg` y una captura de la app | Misma silueta y mismas capas |

> **Prueba 2 es la importante.** Las otras cinco dependen de que haya red, de que Blender esté
> instalado o de que el visor de turno no cambie. La prueba propia corre en CI, sin GPU, y es la
> que impide que el exportador se pudra.

---

## 6. Presupuesto: cuántas primitivas aguantan 60 fps

### 6.1 El número

Con la regla medida (≈0,75 µs por instancia visible, release, en esta máquina):

| Objetivo | Presupuesto de primitivas visibles | Ejemplo real |
|---|---|---|
| 60 fps (16,7 ms) con margen para UI y texto | **~20 000** | la ciudad demo con el TIN completo (8 787) va sobrada |
| 30 fps (33 ms) | **~40 000** | 15 000 pozos + 15 000 arcos en vista general |
| 20 fps (50 ms) | ~65 000 | — |
| 10 fps | ~120 000 | 40 000 pozos: la vista general se arrastra, no se usa |

Y con el pase 3D del §2.3 el presupuesto cambia de unidad: **~1–2 M triángulos por frame** a 60 fps
en esta iGPU con LOD (4,4 M sin LOD, que ya es el límite).

### 6.2 Qué se hace cuando no alcanza (en este orden, y se para en la primera que baste)

1. **LOD del contexto** (manzanas y calles): −65 % en el demo. Es la primera y la más rentable.
2. **Culling por frustum** antes de proyectar: −10 a −30 % en vistas de calle.
3. **Bajar el LOD de los pozos**: 12 caras → 8 → 6 → rombo. Cada paso es un 30–50 % de la red.
4. **Presupuesto por frame**: si `prims > presupuesto`, se degrada solo (pozos a rombo, sin aceras).
   Nunca bajar de «el pozo se ve» ni de «el tubo se ve»: eso es el producto.
5. **Terreno más aligerado** (`thinned` con umbral mayor) y **capas apagadas por defecto** en redes
   grandes: el terreno y los edificios son contexto, no el dato.
6. **Teselado por regiones** (tiles de 500 m): solo se construye la escena de los tiles que toca el
   frustum. Es la solución para 100 000+ objetos con el renderer de hoy; ~1 semana.
7. **El pase 3D del §2.3**: cambia el techo de 20 000 a millones y elimina el problema de orden.

### 6.3 Señal en pantalla (para no discutir de memoria)

La barra de estado ya muestra `N objetos · M malla · L líneas`. Añadir dos números, que son los que
gobiernan todo esto:

```text
… · 5 328 prim · 2,1 ms · 60 fps
```

Cuando `prim` pase de 20 000 o `ms` de 16, la conversación deja de ser una opinión.

---

## 7. Riesgos

| # | Riesgo | Probabilidad | Impacto | Mitigación |
|---|---|---|---|---|
| **R1** | **El techo de primitivas se alcanza antes de lo previsto** (el cliente trae 50 000 objetos o pide el TIN completo + ciudad a la vez) | Media | Alto (la app se siente lenta) | Presupuesto por frame con degradación automática (§6.2) **antes** de la entrega, no después; medidor visible en la barra de estado; tiles por regiones como siguiente paso. |
| **R2** | **El algoritmo del pintor produce artefactos** en cuanto el modelo tiene cruces reales (tubo que entra en pozo, dos colectores que se cruzan), y el cliente lo lee como un error del visor | Alta | Medio (credibilidad) | Partición por capas ya existente + sesgo por tipo; documentar el límite en la guía; y el disparador nº 2 del §3.1 para pasar al pase 3D. |
| **R3** | **Dependencia de una API pre-release de Rune** (el `Compositor` o los slots de efecto cambian o desaparecen en la próxima rev) | Media | Alto si ya se depende de ella | Rev fijada (`78efd4a`); todo el 3D nuevo confinado a `redes-app/src/gpu3d.rs`; el renderer 2D se conserva como respaldo permanente (`--sin-gpu3d`); la CLI nunca depende de Rune. |
| R4 | El glTF exporta una geometría que «no se ve como en la app» (materiales, ejes, escala) | Media | Bajo | Prueba de ida y vuelta en CI (§5.5 prueba 2) + comparación de silueta contra `redes vista`. |
| R5 | JSON de 25 MB tarda 98 ms en abrir y crece con el proyecto | Media | Medio | Aceptable hasta ~100 000 objetos; a partir de ahí, binario o carga por regiones. |
| R6 | La paleta de colores vive en dos sitios (app y exportador) y se separan | Alta | Bajo | Mover la paleta a `families` antes de escribir el exportador (§5.3). |

---

## 8. Resumen de la recomendación

1. **No se cambia de motor.** El renderer propio con proyección en CPU y algoritmo del pintor es la
   decisión correcta para un visor de revisión de un solo `.exe` sin navegador, y hoy **no es el
   cuello de botella**: el demo orbita a 2,4 ms y el 65 % de ese coste son las manzanas de contexto,
   no la red.
2. **Se invierten 2 semanas en el renderer actual**: LOD del contexto, presupuesto por frame,
   culling por frustum, `memo`. En escenas dominadas por contexto eso sube el techo de 20 000 a
   40 000 primitivas a 60 fps; en escenas dominadas por red hacen falta además los pasos 3–6 del §6.2.
3. **Se añade glTF/GLB en 3–5 días**, con paleta unificada y prueba de ida y vuelta en CI.
4. **three.js no entra en el producto**; se usa, si acaso, como visor del `.glb` exportado. Se
   revisa solo si el producto pasa a ser un visor web.
5. **Si hace falta 3D real, la ruta es el `Compositor` de Rune** (3–5 semanas, con respaldo 2D
   permanente), no un motor externo. Los disparadores están escritos en §3.1.

---

## 9. Anexo: cómo se midió (reproducible)

```powershell
# 1. La escena sin GPU: primitivas, triángulos, líneas.
.\target\release\redes.exe vista -p docs\demo\ciudad-san-bartolo.redes --salida $env:TEMP\v.svg --pretty
#   -> data.primitivas = 8787, data.triangulos = 8746, data.lineas = 41, meta.duration_ms ~ 12

# 2. El frame frío (release y debug) de cada escena.
.\target\release\redes-app.exe --snap $env:TEMP\s.png --scene ciudad
.\target\debug\redes-app.exe   --snap $env:TEMP\s.png --scene ciudad
#   -> "first frame 20.03 ms" / "first frame 222.82 ms"; "GPU AMD Radeon(TM) Graphics (wgpu; Dx12, IntegratedGpu)"

# 3. El frame de arrastre: un guion JSON de dos arrastres de 1 s (240 movimientos), cámara orbitando.
.\target\release\redes-app.exe --snap $env:TEMP\o.png --script orbit.json --scene ciudad
#   -> "frame cpu mean 2.597 ms p50 2.075 ... 2542 instances"
#      "animating frames 146 (cpu p50 2.185 p95 3.944 ms)"

# 4. El coste de la proyección propia, aislado (banco aparte que llama a redes_core::view::build_with,
#    30 repeticiones, cámara de la app, TIN aligerado).
cargo run --release   # en un crate temporal con redes-core por ruta:
#   -> ciudad demo: prims=5328  build_p50=2.20 ms
#      capa terreno 2016 prims 0.57 ms | capa ciudad 3225 prims 1.51 ms | capa red 87 prims 0.02 ms
#      4k prims 1.13 ms | 12k 4.47 ms | 30k 11.61 ms | 80k 31.11 ms  (≈0.39 µs/primitiva)
```

El guion de arrastre (`orbit.json`) son dos eventos `{"type":"drag","dx":240,"dy":60,"dur":1.0}` sobre
el centro del lienzo; Rune lo reproduce con reloj virtual y reporta el coste de CPU de cada frame.
Se comprobó en el volcado `--tree` que la cámara se movió de verdad (azimut −55° → −41°, elevación
32° → 20°, distancia 1 422 → 884 m), es decir que el coste medido **incluye reconstruir la escena**
en cada frame del gesto.

---

## Anexo A — Lo que no pude verificar

| # | Cuestión | Estado |
|---|---|---|
| A1 | Coste de GPU (no de CPU) por frame: Rune reporta `frame cpu` (update: handlers, view, layout, animación) y no el tiempo de teselado/subida. Las cifras de «fps implícito» del §1.4(b) son CPU de la app; sirven para comparar configuraciones, no como FPS de GPU. | `[VERIFICAR]` con `--trace`/temporizadores de GPU de Rune cuando haga falta el número fino. |
| A2 | El tiempo de pared de un guion headless en esta máquina es inestable (una corrida tardó 79 s y las siguientes 1,5–2,3 s, probablemente por compilación de pipelines Dx12 en frío y antivirus). Por eso **no** uso tiempo de pared en ninguna conclusión. | `[VERIFICAR]` si se quiere un FPS de pared. |
| A3 | Que `El::effect` + `EffectPhase::Backdrop` permita abrir un pass con profundidad **sobre el target de la ventana** (sí lo permite sobre un target propio; el doc de Rune lo plantea para copias de fondo, no para un pass 3D). | `[VERIFICAR]` con un prototipo de 1 día antes de comprometer las 3–5 semanas. |
| A4 | Que el clip redondeado del lienzo (`clip_rounded(8.0)`) pueda convivir con el pass propio. El scissor es rectangular; el redondeo es del shader de Rune. | `[VERIFICAR]` en el mismo prototipo; alternativa: lienzo sin radio. |
| A5 | Que el Visor 3D de Windows esté instalado en la máquina del cliente (en Windows 11 suele estarlo; en Windows 10 no siempre). | `[VERIFICAR]` en la máquina destino. |
| A6 | El tamaño del WebView2 (1 757 MB, 2 049 archivos) cuenta **todas las versiones** instaladas en esta máquina; una sola versión es bastante menor. | `[VERIFICAR]` si se va a citar la cifra al cliente. |
