# 22 — Revisión adversarial de las pruebas y de las cifras de los documentos

> Revisión hecha **contra el árbol de hoy** (commit `807a984`, árbol limpio salvo los entregables de
> las revisiones hermanas). No se modificó ningún archivo del producto: las mutaciones se hicieron
> en una copia en `out/adv-tests/copia` (borrada al terminar) y los binarios/artefactos de prueba en
> `out/adv-tests/`.
>
> Pregunta que responde este documento: **¿qué afirmaciones siguen en pie si se ataca la suite?**
> Respuesta corta: **cinco pruebas del núcleo siguen verdes con el código roto** (mutado y
> comprobado), seis más tienen umbrales tan flojos que no sostienen lo que prometen, y de las **34
> cifras** publicadas en `README.md` y en los documentos 10, 11, 15, 16 y 17, **14 son falsas** hoy.

## 0. Cómo se midió

| Qué | Cómo |
|---|---|
| Suite | `cargo test --workspace --all-targets` + `cargo test --workspace --doc` |
| Conteos | `redes esquema --pretty` (29 familias / 216 subtipos, desglose por categoría) |
| Ciudad | sonda de integración en la copia: `view::build`, `demo_city::ciudad()`, TIN, pendientes |
| glTF | `redes exportar gltf` + lectura propia de accessors + **Blender 5.1.0 headless real** |
| Rendimiento | `view::build` en release, 20 muestras, p50/p95 (no hay banco de pruebas en el repo) |
| Mutaciones | 8 mutaciones sobre la copia, ejecutando las 58 pruebas del núcleo cada vez |
| Lint | `cargo clippy --workspace --all-targets`, `cargo fmt --check` |

Entorno: Windows, `rustc 1.95.0`, Blender 5.1.0 (2026-03-17), Ryzen/Windows local. Los ms son de
esta máquina y valen como orden de magnitud, no como certificado.

---

## 1. Tabla de hallazgos

Severidad: **B** = bloqueante (la suite certifica algo falso), **A** = alta (hueco de red en un
camino que el producto usa), **M** = media, **b** = baja.

| ID | Sev. | archivo:línea | Prueba / afirmación | Por qué no prueba | Arreglo |
|---|---|---|---|---|---|
| **P‑01** | **B** | `crates/redes-core/src/view.rs:1161` | `todas_las_familias_dibujan` — «cada familia dibuja» | Llama a `pieza_local(&nodo, 2)` directamente (`:1190`), **no** al camino del render. Mutación **M1**: se reintroduce el filtro viejo `HYDRAULIC_NODE || ATTACHABLE` en `view.rs:673-679` (el bug de las 7 familias invisibles) → **58/58 verdes**. Además, para las 6 familias `Link` la prueba solo comprueba `!tuberia(...).is_empty()` (`:1193-1204`), una función que no depende de la familia: esas 6 se certifican solas. | Construir un `Project` con un nodo/arco por cada familia y llamar a `build()`; exigir `prims` de esa familia en la escena. Para las de arco, comprobar el prim del link concreto. |
| **P‑02** | **B** | `crates/redes-core/src/view.rs:1306` | `las_etiquetas_son_pocas_y_tienen_texto` — «≤ 28 etiquetas, con texto» | En la escena que usa la prueba (ciudad encuadrada con `fit`) **hay 0 etiquetas** (medido). El `for` de `:1313` no ejecuta ni una aserción. Mutación **M2**: se desactiva la generación de etiquetas entera → 58/58 verdes. Mutación **M3**: presupuesto 24 → 28 → 58/58 verdes. | Cámara a distancia 15–30 m (donde sí hay 8–11 etiquetas), `assert!(!s.etiquetas.is_empty())`, comprobar que el texto contiene el código del nodo y que `len() <= 24`. Añadir un caso con 60 nodos para probar el truncado. |
| **P‑03** | **A** | `crates/redes-core/src/view.rs:1238` | `el_corte_reduce_la_geometria_dibujada` | Solo comprueba `cortado.tri_count < completo.tri_count` (`:1252`). Mutación **M5**: con `corte_x` activo **no se dibuja ninguna pieza** (0 triángulos) → 58/58 verdes. Destruir la escena satisface la prueba. | Exigir rango: `0.5 * completo < cortado < 0.98 * completo`, y que la arista de corte amarilla exista (`line_count` sube) y que la geometría sobreviva al lado bueno del plano. |
| **P‑04** | **A** | `crates/redes-core/src/biblioteca.rs:1105` | `normalizar_centra_y_apoya_la_base` | Aserción **tautológica**: `assert!(info.base_previa.abs() > 0.001 \|\| info.base_previa.abs() < 0.001)` es cierta para todo valor finito. Mutación **M6**: `base_previa: base + 12345.0` → 58/58 verdes. | `assert!((info.base_previa - antes.bbox.min[2]).abs() < 1e-6)`, y comprobar que la base normalizada es 0. |
| **P‑05** | **A** | `crates/redes-core/src/view.rs:1320` | `el_terreno_queda_ordenado_de_lejos_a_cerca` | La monotonía se cumple con **todas las profundidades iguales**. Mutación **M8**: `depth = 0.0` para todo el terreno → 58/58 verdes. Y `assert!(depths.len() >= terreno)` (`:1336`) es casi tautológico: `depths` sale de `.take(terreno)`. | Comprobar que cada `depth` coincide con `cam.depth()` del centroide (±1e-3) y que hay al menos dos valores distintos ordenados de forma estricta. |
| **P‑06** | **A** | `crates/redes-core/src/view.rs:621` | «el corte también corta tuberías» (doc 11 §2, doc 15 #3) | No hay ninguna prueba del corte de tubos. Mutación **M4**: el corte deja de aplicarse a las tuberías (`Some(_xc) => vec![verts]`) → 58/58 verdes; las pruebas de corte solo ven piezas de nodo. | Escena con un tubo que cruce el plano; comparar `prims` del link antes/después y que aparezca la arista amarilla sobre el tubo. |
| **P‑07** | M | `crates/redes-core/src/view.rs:1216` y `:1229` | `el_corte_parte_un_triangulo_en_dos`, `el_corte_descarta_lo_que_queda_entero_al_otro_lado` | Prueban `cortar_tri` (función pura) y nada del render: exactamente la trampa que el propio doc 15 denuncia en su hallazgo #2. | Mantenerlas como unitarias y añadir la prueba de integración de P‑03/P‑06. |
| **P‑08** | M | `crates/redes-core/src/demo_catalog.rs:343` | `hay_una_pieza_de_cada_familia_principal` | Solo comprueba **8 familias nombradas** (`:348-357`). El doc 10 dice que la vitrina cubre «27 de las 29»; medido: **29/29 en 49 objetos**. La prueba no sostiene ni el 27 ni el 29. | `assert_eq!(p.count(), 49)` y que el conjunto de `family` de nodos∪arcos sea exactamente las 29 claves de `families::families()`. |
| **P‑09** | M | `crates/redes-core/src/gltf.rs:445` | `el_glb_tiene_cabecera_y_json_validos` — «una primitiva por familia» | `assert!(primitivas >= 10)` con 29 familias: se pueden perder 19 familias y sigue verde. Medido hoy: 30 primitivas (29 familias + terreno). | `primitivas == familias_con_geometria + 1` (o `>= 29`) y comprobar los nombres de familia repetidos. |
| **P‑10** | M | `crates/redes-core/src/gltf.rs:455-464` | `las_coordenadas_estan_en_metros_y_dentro_del_modelo` | «Metros» no se prueba: solo `abs(v) < 20_000`. Un `.glb` en milímetros o con 15 km de deriva pasa. Además toma los 6 primeros números del primer `"min":[` del JSON, que no tiene por qué ser el `POSITION`. | Comparar la caja del glb con la del proyecto en metros (±1 %) y medir una tubería conocida (p. ej. 10 m → 10 unidades). |
| **P‑11** | M | `crates/redes-core/src/demo_city.rs:486, 501, 516` | Criterios de la ciudad | Los umbrales son **más flojos que las cifras publicadas**: tapas `< 0,35` (medido máx 0,187), recubrimiento `1,0..=6,2` (medido 1,200–6,000), pendiente `>= 0,004` (medido mín 1,2449 %). Un generador que se salga de la promesa y quede dentro del umbral pasa. | Fijar las constantes publicadas (`TAPA_TOL`, `REC_MIN`, `REC_MAX`, `PEND_MIN`) en un solo sitio y comprobar también la media y el máximo. |
| **P‑12** | M | `crates/redes-core/src/demo.rs:359` | `el_demo_tiene_red_y_terreno` | `assert_eq!(p.count(), p.nodes.len() + p.links.len())` es la definición de `count()` (`model.rs:402-404`) sin el filtro `deleted`: tautología sobre el demo. | Marcar un nodo `deleted` y comprobar que `count()` baja. |
| **P‑13** | b | `crates/redes-core/src/ingest.rs:719` | `la_fecha_tiene_forma_iso` | Solo longitud 10 y `-` en la posición 4: `"2026-99-99"` pasa. Depende del reloj. | Parsear a `(año, mes, día)` y validar rangos. |
| **P‑14** | b | `crates/redes-core/src/model.rs:492` | `cuenta_estados_de_revision` | Solo el caso `(2, 0, 0)` con dos nodos recién creados: no prueba aprobados, observados ni arcos. | Un nodo y un arco de cada estado; comprobar la terna. |
| **P‑15** | M | `crates/redes-app/src/main.rs:1161` | `el_boton_explosion_separa_la_pieza` | El nombre promete separación; la aserción es `explosion > 0.0` (`:1171`). No comprueba que la geometría se mueva (eso lo hace, mejor, `view.rs:1261`). | Reutilizar la comprobación de `view.rs:1261` sobre el estado de la app, o renombrar la prueba a `el_boton_explosion_activa_el_factor`. |
| **F‑01** | M | `export.rs:176,190` · `ingest.rs:695` · `project_io.rs:80` | Rutas fijas en `%TEMP%` | `redes_dxf_test/p.dxf`, `redes_geo_test/p.geojson`, `redes_plan_test/p.json`, `redes_io_test/p.redes`: **el mismo archivo para todos los checkouts y usuarios**. Dos ejecuciones en paralelo en la misma máquina se pisan. | `std::env::temp_dir().join(format!("redes-{}-{}", process::id(), nombre))` o `tempfile`. |
| **F‑02** | b | `gltf.rs:407` | Temp por PID | Mejor que F‑01, pero deja el `.redes` si la prueba falla (no hay `Drop`). | `tempfile::NamedTempFile` o limpieza en un guard. |
| **F‑03** | b | `crates/redes-app/src/main.rs:1105-1228` | Las 7 pruebas de la app | Dependen de que el demo tenga `PZ-001` y de los `key` de los chips: un cambio de demo o de etiqueta tumba las 7 a la vez sin que haya contrato escrito de esos identificadores. | Proyecto de prueba propio y mínimo por prueba. |
| **F‑04** | b | — | Determinismo | Ninguna prueba comprueba que dos ejecuciones producen el mismo `.redes`/`.glb` byte a byte (y el sobre publica `"deterministic": false`). | Prueba de hash del `.glb` con el catálogo. |

### Mutaciones ejecutadas (evidencia)

Cada fila: se copia el árbol a `out/adv-tests/copia`, se aplica el cambio, se ejecutan **las 58
pruebas** del núcleo y se restaura.

| Mutación | Cambio | Resultado |
|---|---|---|
| M0 | ninguno (control) | 58 verdes |
| **M1** | filtro del render vuelve a `HYDRAULIC_NODE \|\| ATTACHABLE` | **58 verdes** |
| **M2** | ninguna etiqueta se genera | **58 verdes** |
| M3 | presupuesto de etiquetas 24 → 28 | 58 verdes |
| **M4** | el corte deja de cortar tuberías | **58 verdes** |
| **M5** | con corte activo no se dibuja ninguna pieza | **58 verdes** |
| **M6** | `base_previa` desplazada +12 345 m | **58 verdes** |
| **M8** | profundidad del terreno constante 0 | **58 verdes** |
| M7 | se quita `Terrain::reindexar()` al abrir | **1 falla**: `el_proyecto_guardado_conserva_el_entorno` |

M7 es la buena noticia: el hallazgo #4 del doc 15 **sí** tiene red.

---

## 2. Las cinco pruebas que hay que reescribir primero

1. **`todas_las_familias_dibujan`** (`view.rs:1161`). Es la que sostiene la promesa «cada familia
   dibuja» y hoy no toca el render: M1 la deja verde con el bug original reintroducido. Es la de
   mayor riesgo porque el fallo que vigila ya ocurrió una vez.
2. **`las_etiquetas_son_pocas_y_tienen_texto`** (`view.rs:1306`). Cero aserciones ejecutadas en su
   propia escena; el presupuesto real (24) no se comprueba y la app pinta hasta 28.
3. **`el_corte_reduce_la_geometria_dibujada`** (`view.rs:1238`). Acepta que el corte borre la escena.
   Es la única prueba de integración del corte y el corte es una de las tres funciones de la fase 2.
4. **`normalizar_centra_y_apoya_la_base`** (`biblioteca.rs:1105`). Aserción tautológica en el camino
   que normaliza los modelos de Blender (base en `z=0`, centro en planta), del que dependen las
   cantidades y la colocación.
5. **`el_terreno_queda_ordenado_de_lejos_a_cerca`** (`view.rs:1320`). El orden del pintor es la
   premisa de todo el render 2D y la prueba pasa con profundidades constantes.

Siguientes en la cola: el corte de tuberías (P‑06, sin prueba alguna), `hay_una_pieza_de_cada_familia_principal`
(P‑08) y los umbrales de `demo_city` (P‑11).

---

## 3. Afirmaciones de los documentos

| # | Afirmación (documento:línea) | Valor medido | Veredicto |
|---|---|---|---|
| 1 | «58 pruebas de núcleo» | 58 `#[test]` en `redes-core`, 58 verdes | **cierta** |
| 2 | «7 de interfaz» | 7 `#[test]` en `redes-app`, 7 verdes | **cierta** |
| 3 | doc 15:38 «57 verdes (49 núcleo + 7 app + 1 doctest), 25 s» | **66** = 58 + 7 + 1; 0,27 s + 1,24 s + 2,44 s | **falsa** |
| 4 | «29 familias / 216 subtipos» | 29 / 216 (exactos) | **cierta** |
| 5 | doc 10:34-39 desglose «Saneamiento 12/95 · Agua 11/76 · Común 6/45» | 12/95 · 11/76 · **Común 5/39** · **Topografía 1/6** (el documento omite una categoría) | **falsa** |
| 6 | «cada familia dibuja» (doc 10:157, README) | Las 29 dibujan hoy (29/29 en la vitrina); la prueba no lo protege (P‑01) | **cierta de hecho, sin red** |
| 7 | doc 10:159 «vitrina con 27 de las 29 familias en 49 objetos» | **29/29** en 49 objetos | **falsa** |
| 8 | doc 15:39 «el demo cubre 29/29 en 49 objetos» | 29/29, 49 objetos | **cierta** |
| 9 | «tapas a ±0,35 m» (doc 15:40 da 0,187 / 0,046) | máx **0,1874 m**, media **0,0460 m** | **cierta** |
| 10 | README:152 «las tapas están a la cota de la calle» | hasta **18,7 cm** por debajo/encima del TIN | **engañosa** (la tolerancia real es ±35 cm) |
| 11 | «recubrimiento 1,2–6 m» | **1,2000 – 6,0000 m** | **cierta** |
| 12 | «pendiente mínima 0,4 %» (README:153) | mínimo real **1,2449 %**; el 0,4 % es solo el umbral de la prueba | **falsa como valor**, cierta como criterio |
| 13 | README:152 «108 edificios» | 108 `manzanas` construidas con altura (doc 16 las llama manzanas) | **cierta en número**, término inconsistente |
| 14 | README:151 «tres avenidas, siete calles» | 10 calles: 3 `Av.` + 7 `Calle` | **cierta** |
| 15 | README:89-90 «el .glb de la ciudad pesa 1,1 MB, 20 primitivas y 26 810 triángulos» | **3 025 100 B (2,89 MiB)**, **19** primitivas, **27 878** triángulos | **falsa** (los tres números) |
| 16 | doc 16:24 «Blender 5.1 headless: `REDES_MESH 2 REDES_TRIS 26816`» | Blender 5.1.0 headless real sobre el `.glb` de hoy: **1 malla, 27 878 triángulos, 83 634 vértices** | cifras **falsas**; la verificación en sí **reproducible** |
| 17 | doc 16:25 «catálogo 30 primitivas, 6 696 triángulos» | 30 primitivas, **7 032** triángulos | primitivas cierta · triángulos **falsa** |
| 18 | doc 16:26 «ciudad 20 primitivas, 26 810 triángulos» | **19** primitivas, **27 878** triángulos | **falsa** |
| 19 | README:86-87 / doc 16:51-52 «p95 33,9 ms ciudad, 247,0 ms corte; 2,3 y 2,5 ms con `--gpu3d`» | No hay banco ni artefacto. Reconstrucción propia en release (solo `view::build`): ciudad **9,31 ms p50 / 11,59 ms p95**; corte **5,98 / 11,82 ms**; catálogo 1,62 / 2,02 ms | **sin evidencia**; el 247 ms **no se reproduce** |
| 20 | doc 16:54-56 «el corte cuesta ~250 ms y por eso el pase 3D lo baja a 2,5 ms» | La escena de corte tiene **menos** primitivas que la ciudad (5 695 vs 12 009) y aun así el documento le atribuye **7,3×** el tiempo; además `--gpu3d` **sigue** ejecutando `view::build_opts` en cada frame de órbita (`main.rs:142` + `:448`), así que el suelo por frame es el build medido, no 2,5 ms | **incoherente y sin evidencia** |
| 21 | doc 11:48-49 y 10:169 «24 etiquetas como máximo» | `view.rs:841` trunca a 24, pero la app pinta `take(28)` (`main.rs:482`) y la prueba admite 28; medido: máx **11** en la ciudad | **falsa en la UI**; sin prueba |
| 22 | doc 10:69 «pozo completo 118 caras / 260 triángulos; simple 17/38» | **332** triángulos (lod 2) y **176** (lod 1); la tubería, 34 | **falsa** |
| 23 | doc 10:91 «0,39 µs por primitiva (5 328 prims → 2,1 ms)» | **0,616 µs/primitiva** (52 940 prims en 0,033 s, release) | **no coincide** (1,6×) |
| 24 | README:215 «ida y vuelta sin pérdida: 44 objetos → DXF → 44» | demo: **43 → 43**; familias tras reimportar: **4** (`agua.tuberia`, `saneamiento.pozo`, `saneamiento.tuberia`, `saneamiento.descarga`). El propio doc 15:19 lo da por abierto | **falsa** («sin pérdida» y «44») |
| 25 | doc 17:56-58 «volumen_m3 8494.75 · area_m2 7515.83 · cerrada false · aristas_abiertas 2688» (un `.glb` de 7 032 triángulos) | Sobre el catálogo de hoy (7 032 tris): volumen **4 462,80**, área **7 515,825** ✓, `cerrada false` ✓, abiertas **2 688** ✓ | volumen **falsa**; área y aristas **ciertas** |
| 26 | doc 16:64 «buffer de vértices dimensionado a 200 000 vértices (≈ 7,2 MB)» | `gpu3d.rs:242`: `36 * 3 * 200_000` = **21,6 MB** (600 000 vértices) | **falsa** (3×) |
| 27 | doc 16:64 «por encima se recorta» | El `write_buffer` recorta (`:273`) pero `draw(0..vertices)` **no** clampa el conteo (`:306`): una escena por encima del buffer pide dibujar fuera del rango | **riesgo real**, no «recorte» |
| 28 | README:193 «checkout de Rune fijado (`78efd4a`)» | `git --git-dir=vendor/rune/.git-upstream rev-parse HEAD` → **78efd4a7876b…** | **cierta** |
| 29 | README:205 «una sola rama, `main`» | `git branch -a`: `main` **y** `mam-1066-campo-modelo` | **falsa** |
| 30 | README:163 «demo.mp4 (11 s, 1280×800)» | `ffprobe`: 1280×800, **11,000 s**, 330 frames | **cierta** |
| 31 | doc 16:27 «`--sin-terreno` reduce la malla al modelo» | 27 878 → **19 178** triángulos; 19 → 18 primitivas | **cierta** |
| 32 | doc 16:22-23 «accessors: count × 12 B, offsets dentro del BIN» | 57 accessors, **0** fuera del BIN, chunk BIN declarado = real = 3 010 824 B | **cierta** (verificado a mano; **sin prueba automática**) |
| 33 | doc 17:29 «exit ≠ 0 si hay errores» | `biblioteca revisar` sobre un glb de otra familia: `exit_code 1`, `E_GLB_INVALIDO` | **cierta** |
| 34 | doc 11:99 nombre `los_parametros_se_editan_en_vivo_y_se_desHacen` | La prueba real es `..._y_se_deshacen` (minúscula) | **falsa** (errata que rompe un `cargo test <nombre>`) |

Resumen: **14 ciertas, 14 falsas, 2 mixtas y 4 sin evidencia o engañosas** de las 34 comprobadas.
Ninguna de las cifras de rendimiento del README/doc 16 se puede reproducir con lo que hay en el
repositorio.

---

## 4. Cobertura real: líneas de producción y pruebas

`lineas_prod` = líneas antes de `#[cfg(test)]`.

| Archivo | líneas prod | pruebas | Comentario |
|---|---|---|---|
| `crates/redes-core/src/families.rs` | 1 472 | 5 | 91 KB de catálogo; las 5 pruebas son estructurales (claves, tipos), no de contenido |
| `crates/redes-core/src/view.rs` | 1 119 | 10 | el módulo más crítico; **cinco** de sus diez pruebas son flojas (P‑02, P‑03, P‑05 y las dos de P‑07) |
| `crates/redes-app/src/main.rs` | 1 098 | 7 | solo chips por `key`; órbita, teclado, guardado y capas sin prueba |
| `crates/redes-core/src/biblioteca.rs` | 1 074 | 7 | una de ellas (P‑04) tautológica |
| **`crates/redes-cli/src/main.rs`** | **1 054** | **0** | **el contrato del CLI (sobre JSON, códigos de salida, `--dry-run`, rutas) no tiene una sola prueba** |
| `crates/redes-core/src/parts.rs` | 859 | 3 | toda la geometría paramétrica con 3 pruebas de conteo |
| `crates/redes-core/src/ingest.rs` | 688 | 2 | un solo plan mínimo; DXF entra por aquí |
| `crates/redes-core/src/demo_city.rs` | 470 | 4 | cubre la ciudad (bien, con umbrales flojos) |
| **`crates/redes-app/src/gpu3d.rs`** | **346** | **0** | **el pase 3D con profundidad, sin prueba** |
| `crates/redes-core/src/gltf.rs` | 394 | 5 | exportador con 5 pruebas, dos flojas (P‑09/P‑10) |
| `crates/redes-core/src/terrain.rs` | 292 | 4 | correcto |
| `crates/redes-core/src/model.rs` | 437 | 3 | |
| `crates/redes-core/src/envelope.rs` | 298 | 3 | |
| `crates/redes-core/src/demo.rs` | 348 | 2 | |
| `crates/redes-core/src/demo_catalog.rs` | 337 | 1 | P‑08 |
| `crates/redes-core/src/export.rs` | 168 | 2 | ida y vuelta DXF: **no** se prueba |
| `crates/redes-core/src/topology.rs` | 113 | 3 | |
| `crates/redes-core/src/geom.rs` | 192 | 3 | |
| `crates/redes-core/src/project_io.rs` | 73 | 1 | |
| **`crates/redes-core/src/city.rs`** | **65** | **0** | modelo del entorno urbano |
| **`crates/redes-cli/src/svg.rs`** | **57** | **0** | salida SVG para agentes |
| `crates/redes-core/src/lib.rs` | 44 | 0 (1 doctest) | |

**Módulo más frágil sin red: `crates/redes-cli/src/main.rs`** — 1 054 líneas, cero pruebas, y es el
único camino que usan los agentes y el que produce los sobres JSON, los códigos de salida y los
mensajes de error (`E_GLB_INVALIDO`, `E_PROYECTO_*`). Cualquier regresión ahí no la ve nadie hasta
que un consumidor la sufre; el propio doc 15:20 la lista como deuda pendiente y doc 15:57 la deja
abierta.

Segundo: **`gpu3d.rs`** (346 líneas, 0 pruebas) con `geom.lock().unwrap()` en el bucle de render
(`:259`): un `panic` de otro hilo envenena el mutex y tumba el frame.

Tercero: **`parts.rs`**, 859 líneas de geometría con 3 pruebas que solo cuentan triángulos y una caja:
una pieza que salga con la forma equivocada pero con más de 200 triángulos pasa.

---

## 5. Pruebas frágiles

| Tipo | Dónde | Por qué |
|---|---|---|
| Rutas fijas compartidas | `export.rs:176`, `export.rs:190`, `ingest.rs:695`, `project_io.rs:80` | `%TEMP%/redes_*_test/...` es el mismo archivo para todos los checkouts de la máquina: dos `cargo test` en paralelo (o dos agentes) se pisan y fallan de forma intermitente |
| Deja basura | `export.rs:177,191`, `ingest.rs:696` | crean el directorio y solo borran el archivo |
| Reloj del sistema | `ingest.rs:721` (`today()`) | el resultado depende de la fecha; además la aserción no valida el calendario |
| Acoplamiento a datos del demo | `redes-app/src/main.rs` (las 7) | dependen de `PZ-001`, de los `key` `btn-*` y del proyecto demo: un cambio de demo tumba las 7 sin contrato |
| Orden de ejecución | suite completa | las de `%TEMP%` son las únicas sensibles si se ejecutan dos suites a la vez; por lo demás el orden no importa |
| Escriben en el repo | ninguna | **bien**: todo va a `%TEMP%`; los artefactos de `out/` los generan los comandos de los documentos, no las pruebas |
| GPU / ventana | las 7 de la app | corren headless y sin GPU (bien), pero el pase `--gpu3d` no se prueba nunca |
| Tiempos | ninguna mide tiempo | el rendimiento publicado no tiene prueba que lo respalde (ni la tendrá si no se crea el banco) |

---

## 6. `cargo clippy` y `cargo fmt --check`

### clippy: **no compila**

```
error: approximate value of `f{32, 64}::consts::FRAC_1_SQRT_2` found
  --> crates\redes-core\src\biblioteca.rs:706:20
   = note: `#[deny(clippy::approx_constant)]` on by default
error: could not compile `redes-core` (lib) due to 1 previous error; 29 warnings emitted
```

`cargo clippy --workspace --all-targets` **falla con código ≠ 0** por un lint `deny` por defecto
(`0.7071` escrito a mano en la comprobación del giro Y-arriba). Es decir: el proyecto no tiene
clippy verde y el `Cargo.toml` no lo declara como gate.

| Paquete | Avisos de nuestro código | Avisos de terceros (vendor) |
|---|---|---|
| `redes-core` | **25** (lib) + 4 extra en pruebas = **29** | — |
| `redes-cli` | **19** | — |
| `redes-app` | **9** | `rune` 13, `wgpu-hal` 1, `parley_engine` 1, `tiny-skia-path` 4 |
| **Total propio** | **57 avisos + 1 error** | 19 |

Los más repetidos: «this `if` statement can be collapsed» (16 apariciones en `redes-core` —se
duplican entre lib y pruebas—, 9 en `redes-cli`, 2 en `redes-app`), «unneeded `return`» (4, CLI),
«field assignment outside of initializer» (4), «manual implementation of `.is_multiple_of()`» (2),
«casting to the same type» (2, app) y código muerto en la app (`publish`, `running`, `latched`,
`símbolos`, `depth`), que apunta a API que se dejó de usar.

### fmt: **376 diffs en 21 archivos**

`cargo fmt --check` no pasa ni en nuestro código. Con `--all` además formatea `vendor/rune` (9 diffs
en `tiny-skia-path`), así que el comando del repo no sirve como gate tal cual. No hay `rustfmt.toml`
ni `clippy.toml` en la raíz.

| Archivo | diffs |
|---|---|
| `crates/redes-app/src/main.rs` | 50 |
| `crates/redes-core/src/families.rs` | 48 |
| `crates/redes-core/src/view.rs` | 48 |
| `crates/redes-core/src/biblioteca.rs` | 40 |
| `crates/redes-core/src/parts.rs` | 35 |
| `crates/redes-core/src/ingest.rs` | 31 |
| `crates/redes-cli/src/main.rs` | 23 |
| `crates/redes-core/src/demo_city.rs` | 16 |
| `crates/redes-core/src/demo.rs` | 14 |
| `crates/redes-core/src/model.rs` | 13 |
| `crates/redes-app/src/gpu3d.rs` | 10 |
| resto (10 archivos) | 48 |
| **Total** | **376** |

### `unwrap` en rutas de producción

| archivo:línea | Contexto | Riesgo |
|---|---|---|
| `redes-app/src/gpu3d.rs:259` | `geom.lock().unwrap()` en `Compositor::render` | **real**: mutex envenenado por un panic de otro hilo → panic en el bucle de render |
| `redes-core/src/ingest.rs:176` | `p.node(from).unwrap()` / `p.node(to).unwrap()` al importar un arco | latente: hoy está guardado por `codes.get(...)`, pero un cambio en la tabla de códigos lo convierte en panic con un archivo de entrada malformado |
| `redes-core/src/envelope.rs:254` | `find(|x| x.is_error()).unwrap()` | seguro (guardado por `any()`), pero es el patrón «buscar dos veces» que invita al fallo |
| `demo.rs` 13, `demo_city.rs` 14, `demo_catalog.rs` 7 (todas en producción) | generadores de demo | panic si se rompe el invariante interno; ocurre al arrancar sin `-p` |
| `redes-cli/src/main.rs:400` | `_ => unreachable!()` en el `match` de comandos | un subcomando nuevo sin rama **panica** en vez de devolver sobre de error: rompe el contrato del CLI |

En el núcleo «de verdad» (`view`, `gltf`, `export`, `terrain`, `topology`, `project_io`, `families`)
**no hay `unwrap` en producción**. El problema no son los `unwrap`: es que el camino del CLI no tiene
pruebas que los ejerciten.

---

## 7. Qué queda abierto para el consejo

1. **La suite mide la geometría, no el render.** Las pruebas de `view` llaman a las funciones puras
   (`pieza_local`, `cortar_tri`) y luego una prueba de integración débil. El bug de las 7 familias
   invisibles volvería sin que la suite se enterara (M1). Es el hallazgo que hay que arreglar antes
   de añadir cualquier fase.
2. **El rendimiento publicado no existe como artefacto.** No hay banco, ni volcado de tiempos, ni
   `--stats`; los números 33,9 / 247 / 2,3 / 2,5 ms no se pueden reproducir y el de 247 ms es
   internamente imposible con el reparto de primitivas que el propio documento publica.
3. **Las cifras del `.glb` están congeladas en un artefacto que ya no existe** (1,1 MB / 20 / 26 810
   frente a 3,03 MB / 19 / 27 878). Tres documentos dan tres números distintos para el mismo archivo.
4. **El CLI, que es el contrato con los agentes, no tiene pruebas.** Es la deuda más cara de las que
   quedan y la única que puede romper a un consumidor externo sin avisar.
