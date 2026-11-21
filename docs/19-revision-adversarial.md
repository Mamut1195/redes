# 19 — Revisión adversarial: síntesis

> Cuatro revisores atacaron el código con archivos hostiles, mutaciones y mediciones. Resultado:
> **37 hallazgos**, de los cuales **10 son críticos** (corrupción de archivos, pánicos y mutación
> del objeto equivocado). Informes completos:
> [19-render](consejo/19-adversarial-render.md) · [20-datos](consejo/20-adversarial-datos.md) ·
> [21-app](consejo/21-adversarial-app.md) · [22-pruebas](consejo/22-adversarial-pruebas.md).

## El hallazgo que importa más

La revisión de pruebas **mutó** el código de ocho formas distintas y ejecutó la suite:

| Mutación | Resultado |
|---|---|
| Volver al filtro que ocultaba 7 familias | **60/60 verdes** |
| No generar ninguna etiqueta | **60/60 verdes** |
| Con corte activo, no dibujar ninguna pieza | **60/60 verdes** |
| Profundidad del terreno constante (sin orden del pintor) | **60/60 verdes** |
| El corte deja de cortar tuberías | **60/60 verdes** |
| `base_previa` desplazada 12 345 m | **60/60 verdes** |
| Quitar el `reindexar()` del TIN al abrir | 1 falla ✅ |

**Seis de ocho mutaciones no las detecta ninguna prueba.** La cobertura existente cuenta cosas
(«29 familias», «≤28 etiquetas») pero no comprueba el comportamiento. Es el defecto estructural
del proyecto, no un bug suelto.

## Críticos (corregidos en `aed6d4a`)

| ID | Qué pasaba | Arreglo |
|---|---|---|
| D-01 · D-02 · R-01 | La ingesta aceptaba `NaN` en CSV y DXF; `serde_json` lo escribe como `null` y **el proyecto deja de abrirse**. `Terrain::build` además paniqueaba dentro de `delaunator` (índice `-1`) | filtrado de no finitos en CSV, DXF y `Terrain::build`; `save` rechaza con `E_VALOR_NO_FINITO` sin tocar el archivo |
| APP-01 | La app escribía el archivo vivo en un hilo y ponía `dirty = false` **antes** de escribir: se midió una lectura de **0 bytes** durante el guardado, y un fallo dejaba la ventana diciendo «guardado» | guardado atómico con `project_io::save`; `dirty` solo si tuvo éxito; el error va a la barra de estado |

Pruebas nuevas: `no_guarda_valores_no_finitos` y `el_terreno_descarta_lineas_no_finitas`.
Suite: **60 núcleo + 7 app + 1 doctest** en verde.

## Críticos abiertos (tickets)

| ID | Qué falla | Impacto |
|---|---|---|
| D-04 · **[MAM-1071](https://linear.app/mamutsystems/issue/MAM-1071)** | Un `.redes` con `schema: redes.project/3` se acepta y al guardar **borra los campos desconocidos** | pérdida silenciosa |
| D-03 · **[MAM-1072](https://linear.app/mamutsystems/issue/MAM-1072)** | Un TIN con `tris:[[0,1,999999]]` **panica al abrir** (`exit 101`, sin sobre) | app y CLI caen |
| D-05 · D-06 · **[MAM-1073](https://linear.app/mamutsystems/issue/MAM-1073)** | La skill emite subtipos inexistentes (`emisario_final`, `colector`…) y `default_subtype` es inválido en 21 de 29 familias: de un SWMM se importan **0 arcos** con `ok:true` | la promesa principal (importar de otras apps) no funciona |
| D-07 · D-08 · **[MAM-1074](https://linear.app/mamutsystems/issue/MAM-1074)** | `next_slot` sin validar ⇒ `ElementId` duplicado ⇒ se aprueba el objeto equivocado; códigos duplicados bloquean `proyecto aprobar` para siempre | revisión corrupta |
| APP-02 · R-02 · **[MAM-1075](https://linear.app/mamutsystems/issue/MAM-1075)** | El pase GPU pide más vértices de los que caben: 558 pozos ⇒ `wgpu Validation Error`, salida 101 | pánico en producción |
| R-10 · **[MAM-1072](https://linear.app/mamutsystems/issue/MAM-1072)** | `clamp(0.5, -0.1)` con TIN menor de 1 m | pánico |

## Altos (abiertos)

| ID | Qué falla |
|---|---|
| D-13 · **[MAM-1077](https://linear.app/mamutsystems/issue/MAM-1077)** | `--dry-run` **escribe** DXF/GeoJSON/GLB/SVG y devuelve `ok:true` |
| D-12 | Terreno colineal ⇒ todas las cotas a 0,0 (proyecto de 2 500 m) |
| APP-03 · **[MAM-1077](https://linear.app/mamutsystems/issue/MAM-1077)** | `Deshacer` pierde la revisión hecha después de una edición paramétrica |
| APP-04 · **[MAM-1077](https://linear.app/mamutsystems/issue/MAM-1077)** | Un `.redes` inexistente o corrupto cae **en silencio al demo** |
| APP-06 · R-03 | En `--gpu3d` las capas no apagan nada y se declaran vértices que no se escriben |
| R-04 | El pase GPU usa el aspecto del lienzo y rasteriza 1280×800: el modelo cae bajo los paneles |
| R-05 | Al redimensionar se recrea el búfer y el modelo desaparece hasta el siguiente cambio |
| — · **[MAM-1076](https://linear.app/mamutsystems/issue/MAM-1076)** | `cargo clippy` **falla** (`approx_constant`), `cargo fmt --check` con 376 diferencias, 34 `unwrap` en producción |

## Documentación

De 34 afirmaciones verificadas en `README` y docs 10, 11, 15, 16, 17: **14 ciertas, 14 falsas,
2 mixtas, 4 sin evidencia**. Ejemplos:

| Afirmación | Medido |
|---|---|
| «57 pruebas (49+7+1), 25 s» | 66 pruebas, ~3 s |
| «vitrina 27 de 29 familias» | 29 de 29 en 49 objetos |
| «`.glb` de la ciudad 1,1 MB, 20 primitivas, 26 810 triángulos» | 2,89 MiB, 19 primitivas, 27 878 |
| «pozo completo 260 triángulos» | 332 (LOD completo), 176 (simple) |
| «p95 247 ms → 2,5 ms con `--gpu3d`» | sin banco reproducible; el 2D mide 9–12 ms y `--gpu3d` **sigue** ejecutando `view::build_opts` por frame |
| «la cobertura pasa por el mismo camino que el render» | falso: la prueba llama a `pieza_local`, no al render |

## Qué hacer, en orden

1. **Red de pruebas de comportamiento** (**[MAM-1076](https://linear.app/mamutsystems/issue/MAM-1076)**, antes de cualquier feature): reescribir las cinco pruebas señaladas y añadir mutación al CI.
2. **Los seis críticos abiertos**, empezando por D-04 y D-03 (archivos que entran) y D-05 (la skill).
3. **`--dry-run` y capas del pase GPU**: son contratos que un agente usa a ciegas.
4. **Limpieza**: clippy, fmt, `unwrap` en producción.
5. **Corregir los números de los documentos** o dejar de citarlos.

## Nota sobre el método

Los cuatro informes incluyen reproducción y, cuando se pudo, el comando exacto. Los revisores
trabajaron sobre una copia de trabajo y no modificaron archivos versionados; el único cambio en el
árbol es este documento y los cuatro informes.

## Tickets abiertos

| Ticket | Título | Severidad |
|---|---|---|
| [MAM-1071](https://linear.app/mamutsystems/issue/MAM-1071) | Esquemas futuros y pérdida de campos | Urgente |
| [MAM-1072](https://linear.app/mamutsystems/issue/MAM-1072) | TIN inválido panica al cargar | Urgente |
| [MAM-1073](https://linear.app/mamutsystems/issue/MAM-1073) | La skill y los subtipos por defecto no importan | Urgente |
| [MAM-1074](https://linear.app/mamutsystems/issue/MAM-1074) | Identidad de objetos duplicada | Urgente |
| [MAM-1075](https://linear.app/mamutsystems/issue/MAM-1075) | Pase GPU: vértices, capas y aspecto | Urgente |
| [MAM-1076](https://linear.app/mamutsystems/issue/MAM-1076) | Pruebas de comportamiento y mutación en CI | Urgente |
| [MAM-1077](https://linear.app/mamutsystems/issue/MAM-1077) | Contratos: `--dry-run`, apertura, deshacer | Alta |
