# 14 — Revisión de verificación y rendimiento (fases 1–3)

| Campo | Valor |
|---|---|
| Documento | `docs/consejo/14-revision-pruebas.md` |
| Autor | `rev-tests` — revisor de verificación y rendimiento |
| Versión | 0.1 |
| Fecha | 2026-11-18, 18:32–18:52 (America/Guayaquil) |
| Estado | Revisión ejecutada. 6 defectos reproducidos, 7 afirmaciones contrastadas, 12 pruebas que faltan. El árbol se rompió y se arregló **durante** la revisión (§1, §2.2) |
| Audiencia | implementadores de `redes-core` / `redes-cli` / `redes-app`, cliente |
| Alcance | Suite de pruebas, cobertura real, rendimiento medido y afirmaciones de las fases 1–3 |
| Fuera de alcance | Diseño de la ingesta, hidráulica, calidad visual de las imágenes de `docs/img` |

**Cómo leer este documento.** §1 fija el estado exacto del árbol y por qué eso condiciona todos los
números. §2 y §3 son la suite y la cobertura. §4 contrasta una por una las afirmaciones. §5 son las
mediciones de rendimiento. §6 recoge los defectos que aparecieron *al verificar*, no al leer. §7 es
la lista priorizada de pruebas que faltan. §8 dice qué números no son comparables.

**Convención numérica.** Punto decimal en tablas y valores medidos; coma en prosa. Milisegundos
salvo que se diga «s». Todos los números de este informe salen de una ejecución propia en esta
sesión; cuando algo no se pudo medir, se dice explícitamente.

---

## 0. Resumen

| # | Afirmación a verificar | Veredicto | Dato |
|---|---|---|---|
| 1 | 29 familias / 216 subtipos | **Verdad exacta** | `redes esquema` → 29 y 216 |
| 2 | «Cada familia dibuja» | **Verdad, pero la prueba no lo demuestra** | 29/29 familias en el catálogo (49 objetos); el test no puede fallar por geometría errónea |
| 3 | Tapas a cota de calle ±0,35 m | **Verdad con margen** | máx 0,187 m; media 0,046 m |
| 4 | Recubrimiento 1,2–6 m | **Verdad exacta** | mín 1,200 m; máx 6,000 m |
| 5 | Pendiente ≥0,4 % | **Verdad** | mín 1,245 %; 0 tramos por debajo |
| 6 | Etiquetas ≤24 | **Falso en sentido estricto** | el código emite hasta 25; la app dibuja hasta 28; la prueba admite ≤28. Medido: máx 16 |
| 7 | release ~30 ms/frame | **Verdad solo como frame frío** | 27,4–40,5 ms el primero; en régimen p50 0,7–0,9 ms, pero **p95 de hasta 247 ms** en `corte` (ruta 2D) |

**Lo más importante (rendimiento):** el pase 3D nuevo es **opcional** (`--gpu3d` o `REDES_GPU3D=1`)
y, cuando se activa, **baja el coste de CPU por frame de forma drástica**: p95 de 33,9 ms a 2,3 ms
en la ciudad, y de **247,0 ms a 2,5 ms** en la escena de corte (§5.4). Pero en volcado headless **no
mejora el fps de pared** y **empeora el primer frame** (41–46 ms frente a 29–31 ms). Dicho de otro
modo: el problema de fluidez no era la CPU, y el peor número de toda la revisión —**p95 de 247 ms en
`corte` por la ruta 2D por defecto**— sigue siendo el que hay que arreglar.

**Lo segundo (proceso):** el árbol **no compilaba** a las 18:43:52 (6 errores en `gpu3d.rs`) y volvió
a compilar a las 18:46:59. Una ventana de unos 5 minutos con el workspace roto mientras entraba la
fase 5. Al cierre (18:48) la suite está verde con 57 pruebas. Como no hay git (§1), ese hueco no
queda registrado en ningún sitio salvo en este informe.

**Lo tercero (verificación):** la suite mide bien el núcleo geométrico y muy mal los bordes.
`redes-cli` tiene **0 pruebas**, y las tres rutas de importación (DXF, GeoJSON, terreno) tienen
**0 pruebas**. Al ejercitarlas a mano aparecieron 5 defectos que la suite no ve: exportar e importar
DXF **no son inversos**, la importación de GeoJSON **pierde 3 objetos**, `default_subtype`
**devuelve un subtipo inexistente**, el plan de ejemplo del repo **choca con el catálogo** y
`exportar gltf` **miente en `primitivas`**.

---

## 1. Estado del árbol y método

El proyecto **no es un repositorio git** (`git rev-parse` → *not a git repository*), así que no hay
línea base ni `git status` con la que separar cambios propios de ajenos. Durante la revisión otros
agentes editaron el árbol en paralelo. Cronología observada (mtime de los archivos):

| Hora | Cambio observado |
|---|---|
| 18:32:54 | Inicio de la revisión. `gltf.rs` aún no existía |
| 18:33:51 | Aparece `crates/redes-core/src/gltf.rs` (11.066 B) |
| 18:34:13 | `lib.rs` y `view.rs` cambian; se declara `pub mod gltf` |
| 18:34:38 | `redes-cli/src/main.rs` cambia (variante `Gltf`) |
| 18:36:23 | `redes-cli/src/main.rs` vuelve a cambiar (el build de 18:36:02 falló por eso) |
| 18:40:49 | `redes-app/Cargo.toml` añade `wgpu = "~30.0.1"` |
| 18:41:15 | Aparece `crates/redes-app/src/gpu3d.rs` (12.183 B), pase 3D de la fase 5 |
| 18:41:40 | `view.rs` crece a 52.710 B |
| 18:42:47 | `redes-app/src/main.rs` crece a 49.165 B |
| 18:43:52 | **El workspace no compila** (6 errores en `gpu3d.rs`) |
| 18:46:59 | `gpu3d.rs` y `main.rs` se corrigen |
| 18:47:52 | `cargo build --release` vuelve a salir **ok** (44,4 s) |
| 18:48:36 | Binario de app nuevo; suite verde otra vez (57 pruebas, 25,1 s) |

Consecuencia metodológica: **hay tres estados**, y los distingo en todo el informe.

- **Estado A (verde)** — árbol de las 18:36:43, con `gltf` dentro y sin `gpu3d`. Es el que produce
  los números de catálogo y la primera tanda de rendimiento.
- **Estado B (roto)** — árbol de las 18:43:52, con `gpu3d` dentro. No compila (§2.2).
- **Estado C (verde, actual)** — árbol de las 18:47:52, con `gpu3d` compilando y **desactivado por
  defecto**. Es el estado que cito como definitivo; los números de §5.4 y §5.5 son suyos.

El pase 3D es **opt-in**: `gpu3d::activo()` devuelve verdadero solo con `--gpu3d` o
`REDES_GPU3D=1`. Por eso el comportamiento por defecto no cambió entre A y C, y los números de §5.1
siguen siendo válidos para la configuración que ve el usuario.

Máquina y cadena de herramientas (de `Get-CimInstance` y `cargo --version`):

| Dato | Valor |
|---|---|
| CPU | AMD Ryzen 9 5900HX, 8 núcleos / 16 hilos |
| GPU discrete | NVIDIA GeForce RTX 3060 Laptop |
| GPU que elige Rune | **AMD Radeon(TM) Graphics (IntegratedGpu)** — «lowest power» |
| `cargo` / `rustc` | 1.95.0 / 1.95.0, edition 2024 |
| SO | Windows, host `windows` en el sobre del CLI |

Que Rune elija la **GPU integrada** y no la RTX 3060 está impreso por el propio runtime en cada
arranque: `rune: GPU "AMD Radeon(TM) Graphics" (wgpu; Dx12, IntegratedGpu, 0000:05:00.0): lowest
power`. Todo lo que se mide abajo es, por tanto, la iGPU.

---

## 2. La suite

### 2.1 Ejecuciones

| Hora | Comando | Resultado | Tiempo |
|---|---|---|---|
| 18:33 | `cargo test --workspace` | **54 ok, 0 fallos** (46 núcleo + 7 app + 0 cli + 1 doctest) | 2,6 s |
| 18:37 | `cargo test --workspace` | **57 ok, 0 fallos** (49 + 7 + 0 + 1) | 15,0 s |
| 18:37 | `cargo build --release -p redes-app -p redes-cli` | **ok** | 4,4 s (incremental) |
| 18:36 | `cargo build --release -p redes-app -p redes-cli` | **falló, exit 101** | 49,6 s |
| 18:43 | `cargo test --workspace` | **falló, exit 101** | 5,9 s |
| 18:43 | `cargo build --release -p redes-app -p redes-cli` | **falló, exit 101** | 12,2 s |
| 18:47 | `cargo build --release -p redes-app -p redes-cli` | **ok** (estado C) | 44,4 s |
| 18:48 | `cargo test --workspace` | **57 ok, 0 fallos** (49 + 7 + 0 + 1) | 25,1 s |

Los tiempos son incrementales: `target/` ya traía artefactos y Rune vendorizado compilado. **No medí
una compilación limpia**; con Rune + wgpu dentro serían varios minutos y no era lo que se pedía.

El salto de 54 a 57 pruebas entre las 18:33 y las 18:37 lo explican las 3 pruebas de `gltf` que
llegaron con el módulo:

```
test gltf::tests::el_glb_tiene_cabecera_y_json_validos ... ok
test gltf::tests::el_documento_serializado_pesa_lo_razonable ... ok
test gltf::tests::las_coordenadas_estan_en_metros_y_dentro_del_modelo ... ok
```

### 2.2 El fallo de compilación del estado B (durante 5 minutos)

`cargo build --release` y `cargo test --workspace` fallaron con **6 errores**, todos en
`crates/redes-app/src/gpu3d.rs`, por desajuste con la API de `wgpu` 30.0.2 que resuelve `Cargo.lock`:

```
error[E0308]: mismatched types
   --> crates\redes-app\src\gpu3d.rs:144:35
    bind_group_layouts: &[&modulo],
                        ^^^^^^^ expected `Option<&BindGroupLayout>`, found `&BindGroupLayout`

error[E0560]: struct `wgpu::PipelineLayoutDescriptor<'_>` has no field named `push_constant_ranges`
   --> crates\redes-app\src\gpu3d.rs:145:13
    = note: available fields are: `immediate_size`

error[E0560]: struct `wgpu::RenderPipelineDescriptor<'_>` has no field named `multiview`
```

`redes-core` y `redes-cli` **nunca** se rompieron: `redes.exe` se regeneró a las 18:43:52 y funciona.
El bloqueo fue solo de `redes-app`, y por tanto de toda la suite, porque el workspace incluye la app.

**Quedó resuelto a las 18:46:59** (los dos archivos tocados a esa hora) y a las 18:47:52 el build de
release sale ok en 44,4 s. Lo anoto por dos motivos: (a) mientras duró, `cargo test --workspace` no
podía ejecutarse, de modo que «la suite está verde» es una afirmación que caduca en minutos; y (b) sin
git (§1) no hay forma de saber, mirando el repo, que el workspace estuvo roto. Si la fase 5 entra así
en una entrega, el cliente se encuentra un `cargo build` que falla y ningún registro.

### 2.3 Avisos relevantes

El build de release emite 13–14 avisos, **todos de `rune` vendorizado** (`edge_scroll.rs`,
`live_latch.rs`, `system_fonts.rs`, `glyph_emit.rs`, `win32.rs`…). Ninguno de `redes-core`,
`redes-cli` ni `redes-app` en las compilaciones observadas.

Sí hay basura en el árbol de fuentes: `crates/redes-core/src/families.rs.bak` (47.423 B) es una
copia de seguridad dentro de `src/`. No entra en la compilación, pero engorda el diff y confunde.

---

## 3. Cobertura real: qué se prueba y qué no

### 3.1 Reparto por módulo (49 pruebas de núcleo + 7 de app + 1 doctest)

| Módulo | Pruebas | Qué afirma de verdad |
|---|---|---|
| `view.rs` | 10 | cámara encuadra; capas apagan primitivas; proyección centra el objetivo; **las 29 familias dibujan**; corte parte un triángulo en dos; corte descarta lo del otro lado; corte reduce triángulos; explosión mueve geometría; etiquetas pocas y con texto; terreno ordenado lejos→cerca |
| `families.rs` | 5 | claves y prefijos únicos; todo `required` existe; defaults con su tipo; **≥29 familias y ≥216 subtipos**; hay familias de las dos redes |
| `terrain.rs` | 4 | interpola un plano 3×3 exacto; fuera del TIN → `None`; pendiente de un plano; genera curvas |
| `demo_city.rs` | 4 | tapas a ±0,35 m; recubrimiento en 1,0–6,2 m; pendiente ≥0,4 %; «hay ciudad y red» |
| `topology.rs` | 3 | grados de una cadena; detecta aislados; sentido de flujo por cotas |
| `model.rs` | 3 | códigos correlativos únicos; no se crea arco colgante; cuenta estados |
| `geom.rs` | 3 | distancia a segmento; área; longitud 3D |
| `parts.rs` | 3 | el pozo genera triángulos y su bbox va de solera a tapa; las válvulas tienen forma distinta; LOD reduce |
| `gltf.rs` | 3 | cabecera y largo del GLB; coordenadas dentro de rango; tamaño del documento |
| `envelope.rs` | 3 | contrato ok/exit code; error produce exit≠0; están las 14 claves |
| `export.rs` | 2 | el DXF sale con entidades; el GeoJSON tiene `features` |
| `ingest.rs` | 2 | importa un plan mínimo; la fecha tiene forma ISO |
| `demo.rs` | 2 | el demo trae red y terreno; trae datos importados y no los calcula |
| `demo_catalog.rs` | 1 | hay pieza de **8** familias principales |
| `project_io.rs` | 1 | guardar y abrir un proyecto **vacío** conserva nombre y revisión |
| `city.rs` | 0 | — |
| `redes-cli` | **0** | — |
| `redes-app` | 7 | observar desde el inspector; foco acerca; corte activa y desactiva; explosión; +DN y deshacer; salir; sin selección no hay chips |

Total: **57 pruebas**. Es una suite rápida (0,10 s el núcleo) y bien orientada al núcleo geométrico.

### 3.2 Lo que **no** se prueba

Ordenado por lo que puede doler:

1. **`redes-cli`: cero pruebas.** Toda la superficie que consumen los agentes IA —`esquema`,
   `proyecto *`, `importar *`, `objeto *`, `exportar *`, `vista`— no tiene una sola aserción. No se
   prueba el código de salida por comando, ni `--dry-run`, ni `--quiet`, ni `--pretty`, ni el
   descubrimiento de proyecto por `$REDES_PROJECT` / búsqueda ascendente.
2. **Ingesta de DXF, GeoJSON y terreno: cero pruebas.** `import_dxf` (264 líneas), `import_geojson`
   (97) e `import_terreno` (36) no tienen ni un caso. Las tres las ejercité a mano (§6) y las tres
   tienen defectos.
3. **Ida y vuelta de exportación.** `el_dxf_sale_con_entidades` solo comprueba `n > 10`,
   `contains("POLYLINE")` y `ends_with("EOF")`. `el_geojson_tiene_features` solo comprueba
   `n == p.count()`. Nadie verifica que el DXF exportado se pueda volver a leer, ni que las capas
   por familia sean las que el importador reconoce.
4. **`export_dxf_filtered` (`--solo-aprobados`)** no tiene prueba. El recuento de `omitidos` se
   calcula en el CLI, no en el núcleo, y tampoco se prueba.
5. **Persistencia real.** `project_io` hace ida y vuelta de un proyecto **vacío**: sin nodos, sin
   arcos, sin terreno, sin entorno. No hay prueba de que `attrs`, `datos`, procedencia, `estado`,
   `journal` ni la topología sobrevivan a guardar y abrir.
6. **`Terrain` con datos reales.** Las 4 pruebas usan un plano 3×3 perfecto. No se prueba el TIN de
   la ciudad (4.484 puntos, 8.700 triángulos), ni `z_at` justo en una arista, ni `thinned()`, ni la
   robustez de Delaunay.
7. **Despiece con nombre** (`pozo_partes`, `valvula_partes`), que es la base de la explosión: no hay
   prueba de que las partes se llamen `solera`, `fuste`, `cono`, `marco`, `tapa`… El test de
   explosión solo comprueba que la geometría se mueve.
8. **Interacción real.** Las 7 pruebas de la app pulsan chips por clave. No se prueba órbita,
   arrastre, rueda, `Shift+rueda` (que según `docs/11` desplaza el plano de corte y abre la
   explosión), ni el teclado, ni el guardado con `Ctrl+S`.
9. **El catálogo del demo.** `demo_catalog` solo exige 8 familias; el artefacto tiene 29 (§4.2). La
   prueba no protege el resto.
10. **`city.rs`**: `Calle::longitud`, `Entorno::bbox`, `Entorno::is_empty` sin cubrir.
11. **Conformidad del demo con el catálogo.** Nada comprueba que los `ty` que usa `demo_city`
    existan en `families`. Por eso pasó inadvertido el defecto D2 de §6.
12. **Rendimiento.** No hay ninguna prueba de presupuesto: ni triángulos por escena, ni tiempo de
    frame, ni regresión. Nada falla si el frame se dobla.
13. **El pase 3D nuevo** (`gpu3d.rs`, 12 KB): 0 pruebas. Es la única pieza que cambia el p95 de
    247 ms a 2,5 ms (§5.4) y no tiene ni un caso que compruebe que dibuja lo mismo que la ruta 2D.

---

## 4. Las siete afirmaciones, contrastadas

### 4.1 «29 familias / 216 subtipos» — **verdad exacta**

`redes esquema --pretty` (166.518 B de JSON) devuelve **29** familias y, sumando `subtipos`, **216**
exactos. Reparto: `saneamiento.descarga` 13, `saneamiento.pozo` 12, `agua.valvula` 12,
`saneamiento.camara` 11, `agua.planta` 11, `comun.accesorio` 11, `saneamiento.tuberia` 10,
`comun.cruce` 10, `saneamiento.canal` 9, `agua.tuberia` 8, `agua.tanque` 8,
`saneamiento.sumidero` 8, `agua.captacion` 8, `saneamiento.caja` 8, `agua.medidor` 7,
`comun.tapa` 7, `comun.cuerpo_receptor` 7, `agua.arqueta` 6, `saneamiento.trampa_grasa` 6,
`comun.hito` 6, `saneamiento.acometida` 5, `saneamiento.bombeo` 5, `agua.llave` 4,
`saneamiento.sifon` 4, `comun.escalera` 4, `agua.hidrante` 4, `saneamiento.arenero` 4,
`agua.dado_anclaje` 4, `agua.acometida` 4.

Matiz: la prueba `el_catalogo_tiene_la_escala_del_consejo` afirma `>= 29` y `>= 216`, no la igualdad.
El documento lo dice abiertamente («falla si baja de ahí»), así que es una decisión consciente; pero
significa que **la suite no detecta** un catálogo que crezca a 30/220 sin actualizar la documentación.

### 4.2 «Cada familia dibuja» — **verdad, pero la prueba no lo demuestra**

Por el lado del dato, es verdad y con holgura: el proyecto de catálogo
(`demo/catalogo-piezas.redes`) tiene 38 nodos y 11 arcos = **49 objetos** y cubre **29 de 29**
familias (23 como nodo y las 6 de arco —`saneamiento.tuberia`, `agua.tuberia`,
`saneamiento.acometida`, `saneamiento.canal`, `saneamiento.sifon`, `agua.acometida`— como arco).
`docs/10` dice «27 de las 29»: se queda corto.

Por el lado de la prueba, el problema es que **no puede fallar**. `view::pieza_local` termina en

```rust
_ => pt::cilindro(n.diametro() as f32 * 0.5, ((n.z_rim - n.z_invert) as f32).max(0.5), 10, pt::HIERRO),
```

es decir, cualquier familia sin rama propia dibuja **un cilindro genérico**. Como ese cilindro nunca
está vacío, `malla.is_empty()` es siempre falso en `todas_las_familias_dibujan`, y la rama que
comprueba `parts::tuberia` para las familias de arco es **código muerto**. La prueba acredita «no
está vacío», no «dibuja lo que es»: si mañana las 6 familias de arco perdieran su geometría real,
seguiría en verde con un cilindro en su lugar.

### 4.3 Tapas a cota de calle ±0,35 m — **verdad con margen**

Medido por mi cuenta sobre `out/ciudad.redes`, reimplementando la interpolación baricéntrica del TIN
(4.484 puntos, 8.700 triángulos) para los 35 pozos:

| Métrica | Valor |
|---|---|
| máx &#124;`z_rim` − `z_at`&#124; | **0,187 m** (PZ-006) |
| media | 0,046 m |
| pozos fuera del TIN | 0 |

El límite de la prueba (`< 0,35`) se cumple con casi el doble de margen. La desviación que queda es
la discretización del terreno (rejilla de 12 m en `demo_city::terreno`), no un error de diseño.

### 4.4 Recubrimiento 1,2–6 m — **verdad exacta, pero es un tope, no un emergente**

| Métrica (35 pozos) | Valor |
|---|---|
| mín `z_rim` − `z_invert` | **1,200 m** |
| máx | **6,000 m** |

Coincide al milímetro con la afirmación… porque son exactamente los topes que impone el diseño:
`COVER_MIN = 1.20` y `COVER_MAX = 6.00` en `disenar_troncal`. Es correcto, pero conviene saber que
la prueba no demuestra que el rango «salga bien»: demuestra que el `clamp` está puesto.

Dos matices:
- La prueba `el_recubrimiento_es_razonable` acepta **1,0–6,2 m**, más laxa que la documentación
  (1,2–6). Un valor de 1,05 m pasaría la suite y contradiría `docs/04` y el `README`.
- En los laterales el mínimo de diseño es 1,10 m (`disenar_lateral`), no 1,20.

### 4.5 Pendiente ≥0,4 % — **verdad**

Sobre los 34 colectores de `saneamiento.tuberia` de la ciudad:

| Métrica | Valor |
|---|---|
| pendiente mínima | **1,245 %** |
| pendiente máxima | 21,794 % |
| tramos con pendiente < 0,4 % | **0** |

La regla se cumple con holgura (el mínimo realizado triplica el exigido). La prueba
`todos_los_colectores_bajan` es, por tanto, un test que hoy no puede fallar por este motivo: es una
red de seguridad, no una medida de calidad del diseño.

### 4.6 Etiquetas ≤24 — **falso en sentido estricto**

`docs/11` promete «**24 etiquetas** como máximo». El código hace:

```rust
let mut elegida = seleccionada.map(|i| etiquetas.remove(i));   // saca la seleccionada
etiquetas.truncate(24);                                        // corta a 24
if let Some(e) = elegida.take() { etiquetas.push(e); }          // y la vuelve a meter
```

→ el máximo real es **25**, no 24. Y aguas abajo:
- la app dibuja `s.etiquetas.iter().take(28)`;
- la prueba afirma `s.etiquetas.len() <= 28`.

Así que el presupuesto documentado (24) no lo sostiene ni el código (25), ni el dibujo (28), ni la
prueba (≤28). Si una regresión llevara las etiquetas a 28, la suite seguiría verde.

Medido en las 13 escenas sobre `out/ciudad.redes`, el recuento real es muy inferior:

| Escena | etiquetas | Escena | etiquetas |
|---|---|---|---|
| `iso` | 0 | `vitrina` | 15 |
| `planta` | 0 | `pozos` | **16** |
| `detalle` | 4 | `pieza` | 0 |
| `ciudad` | 0 | `foco` | 3 |
| `calle` | 6 | `corte` | 6 |
| `catalogo` | 8 | `explosion` | 3 |
| | | `perfil` | 0 |

Máximo **16**. En las vistas generales (`iso`, `planta`, `ciudad`, `pieza`, `perfil`) hay **cero**
etiquetas, porque el umbral `r_px > 9.0` no se alcanza a esa distancia. La afirmación «≤24» es
cierta pero **vacua** en la vista que el usuario ve primero.

### 4.7 release ~30 ms/frame — **verdad solo para el frame frío**

Ver §5. Resumen: el primer frame cae en **27,4–40,5 ms** (mediana ≈30), que es de donde sale la
cifra; en régimen estacionario el **p50 es 0,7–1,3 ms** y el `mean` engaña porque lo cargan unos
pocos frames caros: el p95 llega a **33,9 ms** en `ciudad` y a **247,0 ms** en `corte` por la ruta
2D. Los propios documentos ya matizan la cifra (`docs/08` §«Hallazgo 2»: «los 25 ms / 230 ms del
informe son el frame frío»; `docs/07` §nota: «25–30 ms de un frame único es aceptable»). La cifra es
correcta **si se dice «frame frío»**; dicha como «30 ms/frame» se lee como coste por frame y no lo
es. Y hay un matiz nuevo: con `--gpu3d` el p95 baja a 2,3–2,5 ms, así que el «frame de 30 ms» ya no
describe ni la ruta 2D ni la 3D, sino solo su arranque.

---

## 5. Rendimiento medido

### 5.1 `--snap` de una escena (frame único)

Binario de release del **estado A** (18:36:15). 4 repeticiones por celda, mediana. Comando:
`redes-app <archivo> --snap m.png --scene <escena>`.

> Repetido sobre el estado C (18:48:36) da 33,0 / 36,5 / 40,5 / 32,6 ms para `ciudad` / `vitrina` /
> `corte` / `explosion` en `out/ciudad.redes`: el mismo orden de magnitud con la máquina más
> cargada (§5.4). Ninguna conclusión de esta tabla cambia.

| Archivo | Escena | `first frame` (ms) | `frame cpu mean` (ms) | rango de las 4 | instancias |
|---|---|---|---|---|---|
| `demo/catalogo-piezas.redes` | `ciudad` | 30,0 | 30,0 | 27,4–30,3 | 2.044 |
| `demo/catalogo-piezas.redes` | `vitrina` | **35,7** | 35,7 | 30,6–35,9 | 5.661 |
| `demo/catalogo-piezas.redes` | `corte` | 29,7 | 29,7 | 28,8–30,4 | 1.816 |
| `demo/catalogo-piezas.redes` | `explosion` | 30,6 | 30,6 | 29,2–32,8 | 1.855 |
| `out/ciudad.redes` | `ciudad` | 30,0 | 30,0 | 27,7–32,3 | 8.805 |
| `out/ciudad.redes` | `vitrina` | 30,5 | 30,5 | 30,2–30,8 | 12.586 |
| `out/ciudad.redes` | `corte` | 31,9 | 31,9 | 31,3–32,1 | 10.531 |
| `out/ciudad.redes` | `explosion` | 32,2 | 32,3 | 31,4–35,2 | 10.266 |

**Advertencia de lectura:** con `--snap` se dibuja **un solo frame**, así que `frame cpu mean` es
una muestra de tamaño 1 y sale **idéntico** a `first frame` en las 8 celdas (y también `p50`, `p95` y
`max`). No es una media: es el frame frío, con compilación de pipelines y primer layout dentro.
`gpu init` aparte costó 102,8–124,8 ms en estas corridas.

### 5.2 Régimen estacionario (60 frames)

Para no confundir «frame frío» con «coste por frame», repetí con volcado de 60 frames
(`--frames 60 --out DIR`), que sí da una distribución:

| Caso | pared (60 frames) | fps de pared | `first frame` | `frame cpu mean` | p50 | p95 |
|---|---|---|---|---|---|---|
| ciudad / `ciudad` | 1,42 s | 42,3 | 29,92 | 5,755 | **0,801** | 27,723 |
| ciudad / `vitrina` | 1,20 s | 50,1 | 31,57 | 5,008 | **0,873** | 26,815 |
| ciudad / `corte` | 1,82 s | **32,9** | 32,00 | 23,609 | **0,799** | **72,556** |
| ciudad / `explosion` | 1,66 s | 36,1 | 31,71 | 21,102 | **0,756** | **65,318** |
| catálogo / `vitrina` | 1,41 s | 42,6 | 28,05 | 6,813 | 0,696 | 38,489 |

Lectura:
- El frame típico (p50) es **sub-milisegundo**: 0,70–0,87 ms. El motor no «cuesta 30 ms por frame».
- El `mean` (5,0–23,6 ms) está dominado por una cola: unos pocos frames de **27–73 ms**. En `corte`
  y `explosion` la cola es mucho peor (p95 72,6 y 65,3 ms) porque cada frame recalcula el recorte
  por triángulo y el despiece.
- **No se sostienen 60 fps** en ninguno de los cinco casos (32,9–50,1 fps de pared en volcado
  headless). Es el dato que de verdad importa para la promesa de fluidez, y hoy no lo vigila nadie.

> La misma escena `corte`, remedida a las 18:49 con el binario del estado C y la máquina más
> cargada, dio un p95 de **247,0 ms** (§5.4). Las dos cifras son ciertas y miden lo mismo: la cola
> de la ruta 2D en `corte` es enorme y muy sensible a la carga. No es una cifra para citar sin
> decir en qué condiciones se tomó.

### 5.3 CLI `redes vista` sobre la ciudad

| Comando | Pared (5 corridas) | `duration_ms` del sobre |
|---|---|---|
| `redes vista -p out/ciudad.redes --salida v.svg` | **0,025 s** (0,025–0,029) | 11 ms |
| `redes vista -p demo/catalogo-piezas.redes` | 0,018 s | 3 ms |
| `redes proyecto info -p out/ciudad.redes` | 0,016 s | 2 ms |
| `redes exportar gltf -p out/ciudad.redes` | 0,22 s | 11 ms |

La vista SVG de la ciudad produce **8.787 primitivas**, 8.746 triángulos y un SVG de **643.831 B**
en 25 ms de pared. Con el CLI reconstruido a las 18:43:52 vuelve a dar 0,027 s, así que la cifra es
estable. `redes vista` es, con diferencia, la ruta más rápida y la más adecuada para agentes.

### 5.4 Pase 3D con GPU (`--gpu3d`) frente a la ruta 2D

El pase 3D llegó durante la revisión. Es **opt-in**, así que lo mido aparte. Binario del estado C
(18:48:36), `out/ciudad.redes`, 3 corridas, mediana del primer frame:

| Escena | 2D `first frame` | GPU 3D `first frame` | Factor |
|---|---|---|---|
| `ciudad` | 33,0 | **117,3** | 3,6× peor |
| `vitrina` | 36,5 | **73,2** | 2,0× peor |
| `corte` | 40,5 | **60,3** | 1,5× peor |
| `explosion` | 32,6 | **45,8** | 1,4× peor |

El primer frame del pase 3D es más caro (crea pipeline y búferes). Pero el frame frío no es lo que
decide: con 60 frames la historia se da la vuelta.

| Caso (60 frames) | fps de pared | `first frame` | `mean` | **p50** | **p95** |
|---|---|---|---|---|---|
| `ciudad` / 2D | 20,3 | 28,82 | 8,679 | 0,743 | **33,854** |
| `ciudad` / GPU 3D | 13,4 | 41,06 | 1,276 | 1,134 | **2,327** |
| `corte` / 2D | 13,5 | 31,30 | 43,010 | 0,834 | **247,030** |
| `corte` / GPU 3D | 17,3 | 46,28 | 1,387 | 1,269 | **2,525** |

Tres conclusiones que me parecen las más útiles de todo el informe:

1. **La cola se aplasta.** En `corte`, el p95 pasa de **247,0 ms a 2,5 ms** (≈100×). En `ciudad`, de
   33,9 ms a 2,3 ms. La ruta 2D (orden del pintor, todo en CPU) tenía un problema serio de cola que
   nadie estaba midiendo, y el pase 3D lo resuelve.
2. **El p50, en cambio, empeora ligeramente**: 0,74 → 1,13 ms en `ciudad` y 0,83 → 1,27 ms en
   `corte`. Es el precio de subir geometría a la GPU cada frame.
3. **El fps de pared no mejora** (13,4 frente a 20,3 en `ciudad`; 17,3 frente a 13,5 en `corte`): el
   cuello de botella se mudó del CPU al envío/presente. En esta ruta headless, el pase 3D no es más
   rápido en pared, solo mucho más regular.

Ojo con la comparación entre tandas: los fps de pared de esta tabla (13,4–20,3) son bastante peores
que los de §5.2 (32,9–50,1) para el **mismo** caso 2D. La diferencia no es del código: durante estas
corridas había otros agentes compilando y ejecutando en la misma máquina. El p50 de CPU, en cambio,
se mantuvo estable (0,743 aquí frente a 0,801 en §5.2), y por eso es el número en el que me apoyo.

### 5.5 debug frente a release

Misma escena (`out/ciudad.redes`, `ciudad`), binario de debug (58,5 MB) contra release:

| Build | `first frame` |
|---|---|
| release | **34,97 ms** |
| debug | 265,50 / 256,38 / 264,42 ms |

Factor ≈ **7,3×**. Cualquier cifra tomada en debug es inservible como presupuesto; `docs/01` ya cita
«5.000 primitivas ≈ 250 ms/frame en debug», coherente con esto.

---

## 6. Defectos encontrados al verificar

Ninguno de estos los detecta la suite. Todos están reproducidos con el binario de release del
estado A (18:36:43).

### D1 — Exportar e importar DXF no son inversos

Exporto la ciudad con el propio CLI:

```
redes exportar dxf -p out/ciudad.redes --salida ciudad.dxf
→ ok, 87 entidades, 19.829 B, "capas": "una por familia"
```

Capas escritas (código de grupo 8): `PZ_POZO` (70), `PZ_POZO_TAPA` (35), `CM_CÁMARA` (14),
`CM_CÁMARA_TAPA` (7), `LL_LLAVE` (6), `LL_LLAVE_TAPA` (3), `TQ_TANQUE_/_RESERVORIO` (2),
`CL_TUBERÍA` (136), `TB_TUBERÍA_DE_AGUA_POTABLE` (28).

Vuelvo a leer ese mismo archivo en un proyecto nuevo:

```
redes importar dxf -p rt.redes --archivo ciudad.dxf
→ ok, 46 nodos, 41 arcos, 0 descartados
```

El recuento cuadra (46/41), pero **el contenido no**:

| Original | Reimportado |
|---|---|
| 35 `saneamiento.pozo` + 7 `saneamiento.camara` + 3 `agua.llave` + 1 `agua.tanque` | **46 `saneamiento.pozo`** |
| códigos `PZ-001…035`, `CM-001…007`, `LL-001…003`, `TQ-001` | `PZ-001…PZ-046` (regenerados) |

Causa: `family_for_layer` mapea `CAMARA` → `saneamiento.pozo` (no a `saneamiento.camara`) y no tiene
ninguna regla para `LLAVE` ni `TANQUE`, así que caen en el comodín de punto → `saneamiento.pozo`.
Las familias se colapsan a una y los códigos originales se pierden. Además, de los 35 nodos que
coinciden en código, **los 35 pierden `attrs`** (`cota_clave`, `cota_tapa`, `diametro_interior`,
`material`) y **34 de 41 arcos pierden `datos`** (caudal de diseño, volumen de excavación y su
fuente). La cota sí se conserva (error máximo de `z_rim`: 0 m).

Es decir: el DXF sirve como entrega a CAD, pero **no** como formato de ida y vuelta. O se documenta,
o el importador acepta las capas que el exportador escribe.

### D2 — `default_subtype` devuelve un subtipo que no existe

`ingest.rs:247`:

```rust
"agua.llave" => "columna",
```

Pero `agua.llave` declara `corporacion, banqueta, pase, tapon`. `columna` es subtipo de
`agua.hidrante`. Como `Project::add_node` valida (`subtipo '{}' no existe en {}`), **cualquier
importación que caiga en `agua.llave` pierde el objeto**.

Reproducido importando el GeoJSON que el propio CLI acababa de exportar:

```
redes importar geojson -p g.redes --archivo ciudad.geojson
→ ok, 43 nodos, 41 arcos, 84 objetos, 3 descartados
   "subtipo 'columna' no existe en agua.llave" ×3
```

Se pierden los 3 hidrantes de la ciudad.

### D3 — `import_geojson` ignora el `subtipo` que exporta `export_geojson`

La exportación escribe `familia` y `subtipo`:

```json
"properties": { "codigo": "LL-001", "familia": "agua.llave", "subtipo": "hidrante", … }
```

y la importación hace `ty: default_subtype(&fam)`, tirando el `subtipo` (y también `estado`, `nota`,
`cota_clave` y `cota_terreno`). El GeoJSON exportado **no se puede reimportar sin pérdida** ni
aunque D2 estuviera arreglado: el subtipo `hidrante` se sustituiría por el comodín de la familia.

### D4 — El plan de ejemplo del repo no encaja con el catálogo

`redes importar plan -p g.redes --archivo out/ejemplo-swmm/plan.json` → **0 nodos**, 7 descartados:

```
nodo DS-001: subtipo 'emisario_final' no existe en saneamiento.descarga
nodo PZ-001: E_STATE_DUPLICATE_NAME: código duplicado PZ-001   (×3)
arco CL-001: extremo inexistente (PZ-001 → PZ-002)             (×3)
```

`saneamiento.descarga` declara `emisario_muro, emisario_talud, emisario_subacuatico,
alivio_lateral, alivio_camara, rebose_tanque, vertedero_cresta, disipador_salto, disipador_tanque,
disipador_enrocado, pozo_infiltracion, descarga_emergencia, cso_reja`. `emisario_final` no está.
El artefacto de ejemplo y el catálogo se han separado. (Los duplicados y los extremos inexistentes
son consecuencia de importar sobre un proyecto ya poblado, no un defecto.)

### D5 — `importar terreno` no tiene ni prueba ni un archivo de ejemplo válido

```
redes importar terreno -p g.redes --archivo out/ejemplo-swmm/coords.csv
→ ok=false, E_FMT_PARSE
```

El archivo es `código,E,N` (sin Z):

```
PZ-001,760060,9796090
```

`import_terreno` exige `E,N,Z` numéricos, así que descarta las 4 líneas y falla con
«se necesitan al menos 3 puntos válidos E,N,Z» sin decir cuáles ni por qué. No es un fallo del
parser (ese archivo no es un terreno), pero deja la ruta **sin ninguna cobertura y sin fixture**: el
único CSV del repo no sirve para probarla.

### D6 — `exportar gltf` miente en `primitivas`

```
redes exportar gltf -p out/ciudad.redes --salida ciudad.glb
→ {"bytes": 2684376, "primitivas": 0, …}
```

El GLB es válido (magic `glTF`, versión 2, 2,68 MB, 0,22 s) y el bloque tiene primitivas, pero el
sobre informa `"primitivas": 0` **en duro** (`crates/redes-cli/src/main.rs`). Un agente que se fíe
del sobre concluirá que el archivo no tiene geometría.

---

## 7. Pruebas que faltan para poder decir «fase 1–3 terminadas»

Priorizadas por riesgo × facilidad. Las cinco primeras son las que yo pondría como puerta de salida.

### Puerta de salida (las 5 imprescindibles)

1. **Ida y vuelta DXF.** Exportar el catálogo y la ciudad, reimportar en un proyecto limpio y exigir
   igualdad de `familia`, `ty` y número de objetos por familia. Hoy **falla** (D1): 4 familias se
   colapsan en `saneamiento.pozo`. Es la prueba que convierte la entrega a CAD en un contrato.
2. **Ingesta con fixtures propios.** Un test por ruta (`import_dxf`, `import_geojson`,
   `import_terreno`) con archivos pequeños en `crates/redes-core/tests/fixtures/`, incluyendo: capa
   desconocida, subtipo inválido, CSV con cabecera, CSV sin Z, y **un caso que recorra las 29
   familias exigiendo que `add_node` acepte su subtipo por defecto**. Ese último habría cazado D2 y
   D4 en un segundo.
3. **Contrato del CLI.** Pruebas de integración de `redes-cli` (hoy 0): por cada subcomando, el
   sobre cumple `redes.envelope/1`, `ok`/`error.code`/`exit_code` son coherentes, `--dry-run` no
   escribe, y `--pretty` no cambia el contenido. Empezaría por `vista`, `proyecto info`,
   `exportar dxf|geojson|gltf`, `importar *`, `objeto *`.
4. **Presupuesto de frame en release, por p95.** Una prueba (o un ejemplo `--bench`) que mida **p95**
   de 60 frames en release sobre `ciudad`, `corte` y `explosion`, y falle si p95 > 16,7 ms. Hoy
   `corte` por la ruta 2D está en **247,0 ms** y por la ruta 3D en 2,5 ms (§5.4): sin esta prueba no
   hay forma de saber cuál de las dos rutas se está entregando.
5. **Persistencia de un proyecto real.** `project_io`: guardar y abrir la ciudad y el catálogo y
   exigir que se conserven nodos, arcos, `attrs`, `datos` (valor, unidad **y fuente**), `estado`,
   `entorno` y el TIN. Hoy solo se prueba un proyecto vacío.

### Después, por orden

6. **Conformidad del demo con el catálogo.** Recorrer `demo_city` y `demo_catalog` y exigir que todo
   `ty` exista en `families`; y que el catálogo cubra 29/29 familias (hoy la prueba pide 8).
7. **Etiquetas: el tope exacto.** Fijar la prueba en `<= 24` (no 28), construir una escena con ≥30
   candidatas y comprobar que la seleccionada sigue presente. Arreglar antes el 25 de §4.6.
8. **Corte y explosión en el núcleo, con invariantes.** Además de «se mueve» / «se reduce»:
   el corte no deja vértices al otro lado del plano (ya se hace para un triángulo; falta a nivel de
   escena), la arista de corte es continua, y la explosión conserva el número de partes y el orden
   de la pila.
9. **Despiece con nombre.** `pozo_partes` y `valvula_partes` deben devolver las partes que promete
   `docs/11` (`solera`, `fuste`, `cono`, `marco`, `tapa`, `escalera`; `cuerpo`, `bridas`,
   `accionamiento`) para cada subtipo.
10. **Terreno con datos reales.** `Terrain::build` + `z_at` sobre la rejilla de la ciudad, `z_at` en
    una arista y en un vértice, `thinned()` conservando el rango de cotas, y `slope_at` en un
    triángulo degenerado.
11. **Interacción, no solo chips.** Órbita, arrastre, rueda, `Shift+rueda` (corte y explosión),
    teclado y guardar. Es donde vive la sensación de producto y es lo único que las 7 pruebas de la
    app no tocan.
12. **Humo del pase 3D.** Con `--gpu3d` y con `--sin-gpu3d`/sin flag, la misma escena debe dar el
    mismo `tri_count`, las mismas etiquetas y la misma imagen (o una tolerancia declarada). Hoy el
    pase 3D compila pero nadie comprueba que dibuje lo mismo; y su primer frame es 3,6× más caro en
    `ciudad` (§5.4), lo que pide una prueba que separe coste de arranque de coste por frame.

---

## 8. Riesgos de medición: qué números no son comparables

1. **debug frente a release.** 34,97 ms contra 256–266 ms en la misma escena: **7,3×**. Cualquier
   cifra de debug (incluida la de `docs/01`) sirve para ordenar magnitud, nunca como presupuesto.
2. **GPU integrada.** Rune elige la **AMD Radeon integrada** («lowest power»), no la RTX 3060, y lo
   imprime en cada arranque. Todo lo medido aquí es iGPU; con la 3060 los números no son
   transportables, ni en un sentido ni en otro.
3. **`frame cpu mean` con `--snap` no es una media.** Con un frame, `mean = p50 = p95 = max =
   first frame`. Citar ese «mean» como coste por frame es el error más fácil de cometer con estos
   datos. Para media de verdad hay que usar `--frames` (§5.2), y aun así el `mean` está dominado por
   la cola: el p50 es el dato honesto del frame típico.
4. **Caché fría.** `gpu init` osciló entre 102,8 y 157,9 ms en las mismas escenas, y el primer frame
   entre 27,4 y 35,9 ms. La primera ejecución tras un build paga compilación de pipelines; comparar
   una corrida fría con una caliente produce diferencias de decenas de ms que no son del modelo.
5. **Volcado headless frente a ventana real.** `--frames 60` da 32,9–50,1 fps de pared en una ruta
   offscreen sin presentación a pantalla. El fps de una ventana real (vsync, compositor, foco) no es
   comparable con esto, ni al alza ni a la baja.
6. **El árbol se movió durante la revisión.** Los números de §5.1–5.3 son del binario de 18:36:15;
   los de §5.4–5.5, del de 18:48:36. Entre ambos entraron `gltf.rs`, el pase 3D y cambios en
   `view.rs` y `main.rs`. §5.4 y §5.5 ya cubren la ruta nueva; si el pase 3D pasa a estar activo
   **por defecto**, medir la app con los números de §5.1 sería medir otra aplicación.
7. **Máquina compartida.** Los fps de pared del mismo caso 2D bajaron de 42,3 a 20,3 entre las
   18:37 y las 18:49 sin que cambiara el código de esa ruta: otros agentes estaban compilando y
   ejecutando a la vez. Los tiempos de CPU (`p50`) aguantaron; los de pared, no. Cualquier cifra de
   fps o de «tarda X segundos» de este informe es una **cota pesimista** en esas condiciones.
8. **Tamaño de muestra.** Las medianas de §5.1 son de 4 corridas, las de §5.3 de 5 y las de §5.4 de
   3. Sirven para decir «≈30 ms» o «≈25 ms»; no para afirmar diferencias de 1–2 ms entre escenas (la
   dispersión observada, 27,4–40,5, es mayor que esas diferencias).
9. **Máquina única.** Todo es un Ryzen 9 5900HX con iGPU AMD en Windows/Dx12. No hay medida en otra
   máquina, ni en Vulkan, ni en macOS.

---

## 9. Cómo reproducir

```powershell
# Suite y build (el árbol estuvo roto entre 18:43 y 18:47: ver §2.2)
cargo test --workspace
cargo build --release -p redes-app -p redes-cli

# Catálogo
.\target\release\redes.exe esquema --pretty

# Rendimiento de una escena (frame único)
.\target\release\redes-app.exe out\ciudad.redes --snap m.png --scene corte

# Rendimiento en régimen (60 frames, da p50 y p95)
.\target\release\redes-app.exe out\ciudad.redes --frames 60 --out frames --scene corte

# Rendimiento con el pase 3D (opt-in) frente a la ruta 2D
.\target\release\redes-app.exe out\ciudad.redes --frames 60 --out frames --scene corte --gpu3d

# Etiquetas de una escena (el árbol trae "N objetos · N malla · N líneas · N etq")
.\target\release\redes-app.exe out\ciudad.redes --tree arbol.json --scene pozos

# CLI
.\target\release\redes.exe vista -p out\ciudad.redes --salida v.svg
.\target\release\redes.exe exportar dxf -p out\ciudad.redes --salida c.dxf
.\target\release\redes.exe importar dxf -p nuevo.redes --archivo c.dxf
```

Las comprobaciones geométricas de §4.3–4.5 se hicieron leyendo `out/ciudad.redes` como JSON
(`terrain.points`, `terrain.tris`, `nodes`, `links`) y reimplementando la interpolación baricéntrica
del TIN, sin tocar el código del proyecto.
