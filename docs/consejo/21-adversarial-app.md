# 21 — Revisión adversarial de la app de escritorio

| Campo | Valor |
|---|---|
| Documento | `docs/consejo/21-adversarial-app.md` |
| Autor | `adv-app` — revisor adversarial de `crates/redes-app` |
| Versión | 0.1 |
| Fecha | 2026-11-20, 23:02–23:40 (America/Guayaquil) |
| Árbol | `807a984` (sin cambios; no se modificó ningún archivo del repositorio) |
| Alcance | `crates/redes-app/src/main.rs`, `crates/redes-app/src/gpu3d.rs`, y lo que prometen `docs/04-guia.md` y `docs/11-modos-inspeccion.md` |
| Fuera de alcance | `redes-core` como motor de cálculo, la CLI como interfaz de agente, la ingesta |
| Método | Ejecución del binario `target/release/redes-app.exe` con `--snap`, `--tree`, `--script`, `--scene`, `--gpu3d`; proyectos sintéticos en `out/adv-app/`; medición de memoria y del tamaño del archivo durante el guardado |

**Cómo leerlo.** §1 es la tabla de defectos, ordenada por severidad. §2 trae la evidencia
reproducible de cada uno (comando, salida, números). §3 dice qué se probó y **no** falló, para que
no se confunda «no lo miré» con «está bien». §4 es la lista de arreglos en orden de esfuerzo.

Convención: `archivo:línea` siempre sobre el árbol `807a984`. Punto decimal en los números
medidos.

---

## 1. Defectos

| ID | severidad | archivo:línea | qué falla | reproducción | arreglo propuesto |
|---|---|---|---|---|---|
| APP-01 | **CRÍTICO** | `crates/redes-app/src/main.rs:129-138` | `save()` escribe el archivo **vivo** con `std::fs::write` (trunca y escribe, sin `fsync`, sin temporal, sin `rename`) y pone `dirty = false` **antes** de que el hilo escriba. Un fallo de escritura (permisos, disco lleno, antivirus, unidad de red) deja el `.redes` truncado o vacío **y** la interfaz dice «Guardar» sin asterisco: la revisión se pierde al cerrar y nadie se enteró. El `eprintln!` del error no se ve en una ventana. | Ver §2.1. Con el archivo en solo lectura: `Access is denied` por stderr, hash del archivo idéntico, y el árbol sigue mostrando `'Guardar'` (sin `*`) y ningún aviso. Sondeando el tamaño mientras se guarda un proyecto de 7,8 MB se observó **una muestra de 0 bytes** (§2.2): en ese instante la CLI no puede leer el proyecto. | Llamar a `redes_core::project_io::save` (temporal + `sync_all` + `rename`, ya existe y ya es atómico) desde el hilo; no tocar `dirty` hasta que el hilo confirme por canal/`AtomicBool`; pintar el error en `aviso` y dejar el asterisco; añadir un `on_close` que pregunte o guarde. |
| APP-02 | **CRÍTICO** | `crates/redes-app/src/gpu3d.rs:119`, `:242`, `:273-274`, `:306` | Con `--gpu3d`, `self.vertices` se calcula como `malla.tris.len()*3` (todas las familias) pero al buffer solo se copian los triángulos visibles y **recortados a `capacidad` = 200 000 vértices**; luego se dibuja `0..vertices`. Si la malla pasa de 200 000 vértices, wgpu aborta con error de validación (pánico, no aviso). `docs/16` §Límites promete que «por encima se recorta»; no se recorta: se cae. | Ver §2.3: proyecto con 276 120 triángulos → `panicked … wgpu error: Validation Error … Vertex 848238 extends beyond limit 600000 imposed by the buffer in slot 0`, salida 101. | `vertices = bytes_escritos / 36` (contar lo que realmente se empujó) y, si la malla no cabe, recrear el buffer con `bytes.len()` en vez de recortar; alternativamente trocear el `draw` en varios rangos. |
| APP-03 | **ALTO** | `crates/redes-app/src/main.rs:237-246` (+ `:108-126`, `:158-163`) | `deshacer()` sustituye el proyecto entero por el último estado de la pila, pero `set_estado` (aprobar/observar) **no apila** nada. Resultado: deshacer una edición paramétrica revierte también todas las aprobaciones y observaciones hechas después, en silencio, y el aviso dice «Deshecho». Contradice `docs/11` §7 («la revisión se guarda en el archivo, no en la pila»). | Ver §2.4: 11 aprobados / 0 observados → `+ DN` → `Observar` (10/0/1) → `Deshacer` → **11/0/0**. La observación desaparece y `Guardar *` queda activo, así que el archivo se escribe sin ella. | Apilar también en `set_estado` (una entrada por objeto, con su `revision`), o guardar en la pila solo los parámetros editados y reaplicarlos al proyecto actual; mientras no haya redo, no revertir estados de revisión. |
| APP-04 | **ALTO** | `crates/redes-app/src/main.rs:330-361` (y `:129-130`) | Si el `.redes` pedido no existe, no es JSON o no valida, `load_project()` imprime por `stderr` y **devuelve el demo interno** con `path = None`. La ventana se abre como si nada: el usuario revisa y aprueba el demo creyendo que es su proyecto, y `save()` retorna sin hacer nada (no hay ruta) mientras el asterisco sigue puesto. | Ver §2.5: `--abrir out\adv-app\no-existe.redes` → título `Redes La Esperanza` (43 objetos), tras `+ DN` y `Guardar` el árbol muestra `'Guardar *'` y `'cambios sin guardar'`, y `Test-Path no-existe.redes` = `False`. | Devolver `Result<Project, String>`; sin fallback silencioso: banner de error bloqueante con la ruta y el código (`E_IO_NOT_FOUND`, `E_FMT_PARSE`…). Con `path = None`, deshabilitar `Guardar` y decirlo en la barra de estado. |
| APP-05 | **ALTO** | `crates/redes-core/src/terrain.rs:56` (vía `project_io.rs:49` ← `main.rs:355`) | Abrir un `.redes` con un índice de triángulo del TIN fuera de rango **panica** en `TriIndex::build` (`terrain_points[i as usize]`). No hay validación del TIN al cargar: el archivo es JSON válido y el esquema es correcto. La app muere en el arranque (salida 101) y la CLI también. | Ver §2.6: `terrain.tris[0] = [999999,0,0]` → `panicked … index out of bounds: the len is 2400 but the index is 999999`; app y CLI con salida 101. | En `project_io::load`, validar `tris` contra `points.len()` (y `bbox` contra `points`) y devolver `E_PROJ_CORRUPT`; o hacer que `TriIndex::build` ignore los triángulos fuera de rango con un warning en vez de indexar. |
| APP-06 | **ALTO** | `crates/redes-app/src/gpu3d.rs:72-91`, `:119`, `:262-276` | Con `--gpu3d`, apagar **Alcantarillado**, **Agua potable** o **Edificios** no apaga nada: el filtro se aplica a los bytes que se copian, pero no a `vertices`, y el resto del buffer conserva la geometría del pase anterior, que se vuelve a dibujar. El comentario de `:71` afirma lo contrario («apagar «Edificios» también los apaga aquí») y `docs/04` §4 vende el chip como control de capa. Solo «Terreno» funciona, porque se filtra dentro de `triangulos_exportables`. | Ver §2.7: mismo clic en el chip «Alcantarillado», canvas del viewport: 2D cambia **7 331 px**; `--gpu3d` cambia **19 px**. | Calcular `vertices` con los triángulos realmente empujados (mismo arreglo que APP-02) y, además, no reutilizar el resto del buffer: `write_buffer` del rango completo o un `clear` lógico. |
| APP-07 | **MEDIO** | `crates/redes-app/src/main.rs:330-361` | La app **no** resuelve el proyecto como la CLI y como dice `docs/04` §5.1 («`-p` → `$REDES_PROJECT` → el primer `*.redes` de la carpeta o de sus padres»): no implementa el tercer paso. En una carpeta con un único `.redes`, la CLI lo abre y la app abre el demo. | Ver §2.8: en `out/adv-app/cwd/` con `zz-proyecto.redes` («Vacio»): CLI → `ok true, name Vacio`; app → título `Redes La Esperanza`. | Reutilizar `redes_core::project_io::find_upwards(std::env::current_dir())` como último candidato, en el mismo orden que la CLI. |
| APP-08 | **MEDIO** | `crates/redes-app/src/main.rs:166-173` | `Foco` con un **arco** seleccionado no hace nada y no dice nada: `enfocar()` sale por el `let Some(n) = self.project.node_by_code(&code) else { return }` sin fijar `aviso` (el mensaje «Selecciona un objeto para enfocar» solo aparece cuando `sel` es `None`). | Ver §2.9: clic en el viewport (selecciona `CL-004`), `Foco` → la barra sigue en `az -55° el 32° d 884 m` y `aviso` vacío. | Enfocar también arcos (bbox de `path`), o poner `aviso = Some("El foco es para pozos y válvulas")` y no dejar el botón como si hubiera funcionado. |
| APP-09 | **MEDIO** | `crates/redes-app/src/main.rs:221-235` (+ `crates/redes-core/src/view.rs:734`) | `Explosión` sin objeto enfocado (p. ej. con un arco seleccionado, que no tiene `id` de nodo) deja `opts.explosion = 0.9` con `opts.foco = None`: el botón pasa a «Explosión \*», la barra de estado dice «explosión · » y el aviso «Explosión activa: Shift + rueda la abre», pero **nada se separa** y `Shift + rueda` deja de hacer zoom para mover un factor invisible. El renderizador exige `opts.foco == Some(n.id)`, así que la interfaz y la escena se contradicen. | Ver §2.9: mismo guion, tras `Explosión` el árbol muestra `'Explosión *'`, `'explosión · '` y el aviso, con `foco` nulo. | Exigir nodo enfocado para activar la explosión (si no, `aviso` y no tocar `opts`), y derivar el texto de la barra de estado de la misma condición que usa `build_opts`. |
| APP-10 | **MEDIO** | `crates/redes-app/src/main.rs:158-163` | La pila de deshacer guarda **20 clones completos del `Project`**, con el TIN y el `journal` dentro. En un proyecto de 7,8 MB son ~172 MB extra; en un proyecto real de 50 MB, ~1 GB. Cada clic de `± DN`, `± Ø`, `± prof.` clona el documento entero. | Ver §2.10: pico de memoria 261 MB con una edición y **434 MB** con veinte, mismo proyecto. | Guardar deltas (código + clave + valor + `revision`) o, como mínimo, compartir el `Terrain` con `Arc` y poner tope por bytes en vez de por número de pasos. |
| APP-11 | **BAJO** | `crates/redes-app/src/main.rs:76` (+ `crates/redes-core/src/view.rs:49-52`) | `Camera::fit` sobre un proyecto sin ningún objeto usa un `Bbox` vacío (`xmin = inf`, `xmax = -inf`): `cx = (inf + -inf)*0.5 = NaN`, así que `cam.target` queda en `NaN` para toda la sesión. Hoy no se ve porque no hay nada que proyectar, pero `Encuadrar`, `Planta` y el arrastre trabajan sobre NaN. | §2.11 (leído, no visible): `proyecto nuevo` + `--abrir` → estado `d 120 m` sin errores; el `target` no se imprime, no hay síntoma en pantalla. | En `fit`: si `p.bbox().is_empty()`, `target = [0,0, (z0+z1)*0.5]` y `distance = 120`. |
| APP-12 | **BAJO** | `crates/redes-app/src/main.rs:482` (+ `view.rs:841`) | El presupuesto de etiquetas está duplicado y descuadrado: el core trunca a 24, la app vuelve a cortar a 28, `docs/11` §4 dice 24 y su tabla de verificación admite ≤28. Además se crean y maquetan etiquetas de objetos fuera del viewport (se vieron en `x = -9611`), que solo se salvan por el `clip_rounded`. | §2.12: `--scene iso` con zoom ×4 → `2 etq`, una a `x=-9611.2`, otra a `x=-1839.29`. | Un solo presupuesto (24) en el core, y descartar en `view::build` las etiquetas con `pos` fuera de `viewport`. Ya reportado como afirmación falsa en `docs/consejo/14` §0 #6; sigue sin arreglar en la app. |
| APP-13 | **BAJO** | `crates/redes-core/src/project_io.rs:43` | El control de esquema es `p.schema.starts_with("redes.project/")`, así que `redes.project/99` (una versión futura) se abre sin avisar y se interpreta con las reglas de la 2; después, guardar desde la app lo reescribe como `redes.project/2` sin decirlo. | §2.13: `schema = "redes.project/99"` → la app abre el proyecto con normalidad (sin `E_PROJ_SCHEMA_TOO_NEW`). | Aceptar solo las versiones conocidas (`== "redes.project/2"`) y devolver `E_PROJ_SCHEMA_TOO_NEW` con la versión encontrada. |
| APP-14 | **BAJO** | `crates/redes-app/src/main.rs:208-214` | `Corte` con un arco seleccionado no corta por el arco: cae al `unwrap_or(self.cam.target[0])` y el plano aparece por el centro de la cámara, no por el objeto, aunque el aviso diga «Corte activo». | §2.14 (leído): con `CL-004` seleccionado, `opts.corte_x` = `cam.target[0]`; en pantalla el corte no coincide con la tubería. | Con un arco, usar la `x` del punto medio de `path`; con nada seleccionado, mantener el centro de la vista pero decirlo en el aviso. |

---

## 2. Evidencia

Todo se ejecutó desde la raíz del repositorio con `target\release\redes-app.exe` y
`target\release\redes.exe` (compilados a partir de `807a984`), y temporales en `out\adv-app\`
(borrados al terminar). Los proyectos sintéticos se derivaron de `redes proyecto demo`.

### 2.1 Guardar cuando la escritura falla (APP-01)

```
Copy-Item out\adv-app\demo.redes out\adv-app\ro.redes
(Get-Item out\adv-app\ro.redes).IsReadOnly = $true
redes-app.exe --abrir out\adv-app\ro.redes --scene iso --script s-save.json --at mark:fin --tree t.json
```

`stdout/stderr`:

```
redes: no se pudo guardar out\adv-app\ro.redes: Access is denied. (os error 5)
```

y en el árbol de elementos:

```
[709.09,14.0,46.29,15.0] 'Guardar'        <- sin asterisco
(hash del archivo antes == después)
```

Es decir: el error existe, se escribe en `stderr` (invisible), `dirty` ya está en `false` y la
ventana afirma que no hay cambios pendientes.

### 2.2 El archivo queda a medias mientras se escribe (APP-01)

Proyecto de 7,8 MB (`journal` de 160 000 entradas, geometría normal) sondeando
`os.path.getsize` cada 0,2 ms mientras la app guarda:

```
before 7812897  after 11609814
samples 837  distinct 3  min 0  max 11609814
zero-size samples 1
```

Una muestra midió **0 bytes**: entre el `File::create` (truncado) y el `write_all` el archivo no es
un proyecto. Un `redes proyecto info` en ese instante devuelve `E_FMT_PARSE`; un cierre forzado o un
corte de luz ahí deja el proyecto irrecuperable. `project_io::save` ya evita esto con
`redes.tmp` + `sync_all` + `rename`; la app no lo usa.

### 2.3 `--gpu3d` con más de 200 000 vértices (APP-02)

Proyecto con el TIN del demo repetido 60 veces (276 120 triángulos, 14,8 MB):

```
redes-app.exe --abrir out\adv-app\big.redes --gpu3d --snap out\adv-app\gpu-big.png
```

```
thread 'main' panicked at ...wgpu-30.0.2\src\backend\wgpu_core.rs:2678:18:
wgpu error: Validation Error
Caused by:
  In a CommandEncoder
    In a draw command, kind: Draw
      Vertex 848238 extends beyond limit 600000 imposed by the buffer in slot 0.
```

Salida 101. El mismo proyecto sin `--gpu3d` se dibuja (lento) sin caerse: el defecto es del pase 3D.
`docs/16` §Límites dice «por encima se recorta», que es exactamente lo que **no** pasa.

### 2.4 Deshacer revierte la revisión (APP-03)

Guion `s-undo.json` sobre `--scene iso` (selecciona `PZ-001`, que nace **aprobado**):

| paso | acción | aprobados | pendientes | observados |
|---|---|---|---|---|
| 1 | `+ DN` (`ui.parametro`) | 11 | 32 | 0 |
| 2 | `Observar` (`ui.revision`) | **10** | 32 | **1** |
| 3 | `Deshacer` | **11** | 32 | **0** |

Los dos controles (`Observar` solo; `+ DN` + `Observar`) dan 10 aprobados, así que el paso 3 es el
que deshace la observación. Después del paso 3 el árbol muestra `'Guardar *'`.

### 2.5 Abrir un archivo que no existe o no es JSON (APP-04)

```
redes-app.exe --abrir out\adv-app\no-existe.redes --scene iso --script s-save.json --at mark:fin --tree t.json
```

`stderr`: `redes: no se pudo abrir out\adv-app\no-existe.redes: E_IO_NOT_FOUND: ...`; en el árbol:

```
'Redes La Esperanza'      <- el demo, con 43 objetos
'Guardar *'
'cambios sin guardar'
```

y después de pulsar `Guardar`: `Test-Path out\adv-app\no-existe.redes` → `False` (no se creó nada,
no se dijo nada).

Lo mismo con un archivo con basura (`E_FMT_PARSE`): la app abre el demo y no hay ningún aviso en la
interfaz. Combinado con APP-01, el usuario puede aprobar 43 objetos y perderlo todo.

### 2.6 Un `.redes` corrupto tumba app y CLI (APP-05)

`terrain.tris[0] = [999999, 0, 0]` sobre el demo (2400 puntos):

```
panicked at crates\redes-core\src\terrain.rs:56:24:
index out of bounds: the len is 2400 but the index is 999999
```

`redes-app.exe` salida 101; `redes.exe -p … proyecto info` salida 101 (mismo pánico): no hay
validación del TIN en `project_io::load`, así que un archivo escrito por cualquier herramienta con
un índice malo deja el proyecto inabrible sin diagnóstico.

### 2.7 Capas que no se apagan con `--gpu3d` (APP-06)

Mismo clic en el chip «Alcantarillado» (centro `115,212`), diferencia de píxeles dentro del viewport
(1800×1080 px de dispositivo) entre la captura con la capa encendida y apagada:

| renderizador | píxeles distintos |
|---|---|
| 2D (pintor) | **7 331** (0,48 %) |
| `--gpu3d` | **19** (0,00 %) |

Con «Agua potable»: 2 813 px (2D) frente a 580 px (`--gpu3d`). Con «Terreno» sí funciona en los dos
(33,8 % y 34,4 %), porque ese filtro se aplica antes de contar los vértices.

### 2.8 Resolución del proyecto distinta de la CLI (APP-07)

Carpeta `out/adv-app/cwd/` con un único `zz-proyecto.redes` (nombre «Vacio»):

```
CLI: redes.exe proyecto info        -> ok true, name "Vacio", path ...\cwd\zz-proyecto.redes
App: redes-app.exe --tree tree.json -> título "Redes La Esperanza"
```

### 2.9 Foco y explosión con un arco seleccionado (APP-08, APP-09)

Clic en el viewport (`640,420`) selecciona `CL-004` (un colector). Después:

```
'Foco'                                  <- el botón se pulsa, no pasa nada
'az -55°  el 32°  d 884 m'              <- la cámara no se movió
(aviso vacío)                           <- ningún mensaje
'Explosión *'                           <- tras pulsar Explosión
'explosión · '                          <- la barra de estado afirma el modo
'Explosión activa: Shift + rueda la abre'
```

`opts.foco` queda en `None`, así que `view::build` (`:734`, `opts.foco == Some(n.id)`) no separa
ninguna pieza: el modo está «activo» solo en la interfaz.

### 2.10 Memoria de la pila de deshacer (APP-10)

Pico de *working set* del proceso (medido con `GetProcessMemoryInfo` cada 10 ms), proyecto de
7,8 MB, `--scene iso`:

| guion | pico |
|---|---|
| `+ DN` y `Guardar` | 261 MB |
| `+ DN` ×20 | **434 MB** |

Diferencia ≈ 173 MB ≈ 20 × 8,6 MB: cada paso de deshacer es una copia completa del documento.
Con el proyecto de la ciudad (1 MB) es inocuo; con un levantamiento real (decenas de MB de TIN y
journal) es un OOM probable.

### 2.11 `fit` con la caja vacía (APP-11)

Leído, sin síntoma visible: `Bbox::default()` es `{inf, inf, -inf, -inf}`, así que en `Camera::fit`
`cx = (bb.xmin + bb.xmax) * 0.5` es `inf + (-inf)` = `NaN`. Con `--abrir out\adv-app\vacio.redes`
(proyecto sin nodos, sin enlaces, sin terreno) la app abre sin error, la barra dice `d 120 m` y no
hay nada que delate el `NaN`; cualquier operación de cámara posterior lo propaga.

### 2.12 Etiquetas fuera del viewport y presupuesto duplicado (APP-12)

`--scene iso` + cuatro pasos de rueda (`dy -600`) → `2 objetos · 664 malla · 0 líneas · 2 etq` y en
el árbol:

```
rect=796.95,297.37,115.91,15.0  [PZ-002 · inspeccion]
rect=412.22,327.84,113.50,15.0  [PZ-001 · inspeccion]
```

En el proyecto completo, las etiquetas se colocan en `x = -9611.2` y `x = -1839.29`. Se crean y se
maquetan aunque estén fuera; solo el recorte del viewport las esconde.

### 2.13 Esquema futuro aceptado (APP-13)

`"schema": "redes.project/99"` → la app abre el proyecto con normalidad, sin
`E_PROJ_SCHEMA_TOO_NEW`; `starts_with("redes.project/")` da por buena cualquier versión.

### 2.14 Corte con un arco (APP-14)

Leído: `alternar_corte` toma `node_by_code` y, si no hay nodo, `unwrap_or(self.cam.target[0])`;
`opts.corte_x` queda en la `x` del centro de la cámara aunque el aviso diga «Corte activo: Shift +
rueda lo desplaza». No corta por la tubería seleccionada.

---

## 3. Probado sin defecto (para no confundir con «sin mirar»)

| Caso | Resultado |
|---|---|
| Proyecto vacío (sin nodos, enlaces ni terreno) | Abre, dibuja la interfaz y no panica (salvo el `NaN` de APP-11); `--gpu3d` también, con `vertices == 0` y `draw_ui` |
| `--gpu3d` con un `.redes` inexistente | No hay pánico propio del pase 3D: se aplica el fallback de APP-04 y además `Guardar` no escribe |
| Cambio de tamaño con `--gpu3d` (`{"type":"resize","dw":-600,"dh":-400}`, 1280×800 → 680×400) | Recrea los recursos de tamaño y de profundidad y termina con salida 0 |
| Clic fuera del canvas (panel izquierdo, etiqueta) | No rompe nada; el clic sobre una etiqueta llega al canvas (deshace la selección), las etiquetas **no** se comen el puntero |
| Etiquetas superpuestas / `Scene.etiquetas` vacío | `take(28)` sobre vacío es inocuo; el solape ya está declarado como pendiente en `docs/11` §9.2 |
| Compatibilidad del archivo app → CLI | `redes.exe -p out\adv-app\p1.redes proyecto info` → `ok true`, `revision 2`, 43 objetos: la app escribe un `.redes` que la CLI lee sin cambios (APP-01 es de atomicidad, no de formato) |
| `--gpu3d` con el proyecto de la ciudad (26 810 triángulos) | Dibuja, sin pánico ni error de validación |
| Proyecto de 276 120 triángulos **sin** `--gpu3d` | Dibuja y sale con 0 (3,0 s): el pánico de APP-02 es del pase 3D, no del modelo |
| Foco → Salir | `--scene foco` deja la cámara en `d 5 m`; `Salir` la devuelve a `d 884 m` y avisa «Modo normal» |
| Corte con `corte_x` fuera del modelo (`Shift`+rueda de −20 000 puntos) | La red se recorta (malla 1999 → 1229, líneas 20 → 74 por las aristas de corte) y se recupera apagando `Corte`; el terreno no se corta, como está declarado en `docs/11` §2 |

---

## 4. Orden de arreglo sugerido

1. **APP-01** (atomicidad + `dirty` después del éxito): es el único que destruye trabajo del usuario
   sin que se entere, y el arreglo es reutilizar `project_io::save`.
2. **APP-02** (contar los vértices escritos): una línea, y quita el pánico del pase 3D.
3. **APP-03** (apilar la revisión en el historial): el usuario pierde aprobaciones sin señal.
4. **APP-04** y **APP-05** (abrir: error visible y validación del TIN): los dos son «la app no abre
   y no dice por qué».
5. **APP-06** (mismo arreglo que APP-02 más no reutilizar el buffer).
6. **APP-07**…**APP-10** (resolución, foco/explosión con arcos, memoria del historial).
7. **APP-11**…**APP-14** (higiene: `fit` vacío, presupuesto de etiquetas, esquema, corte con arcos).
