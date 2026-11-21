# 03 — Interfaz CLI / IA / MCP

**Autor:** arquitecto de interfaz IA/CLI (consejo de arquitectura)
**Estado:** propuesta normativa para implementación
**Alcance:** superficie de comandos, sobre JSON, afordances para agentes, wrapper MCP, ergonomía humana y pruebas de contrato.
**No cubre:** modelo de datos del motor, algoritmo hidráulico, formato del archivo de proyecto (los fija el consejo de dominio).
**Documentos hermanos con los que se contrasta:** `01-domain.md` (modelo, familias F01–F17, identidad, unidades) y `05-product.md` (requisitos CLI-01…CLI-07, perfiles, vocabulario de cliente). El **Anexo A** resuelve cada delta.

> **Convención de los bloques de código.** Los fragmentos JSON están abreviados: `…` marca elisión y no forma parte del documento. Los bloques de `--stream` contienen varios documentos NDJSON, uno por línea. Los identificadores son ilustrativos; su forma canónica está en §4.3.

---

## 1. Resumen de decisiones

| # | Decisión | Razón |
|---|---|---|
| D1 | Binario único `redes`; el motor es headless y la GUI es un consumidor más. | Un solo binario garantiza que la IA y el humano ejecutan exactamente el mismo código. |
| D2 | Gramática `familia verbo` (`manhole create`, `hydraulics solve`), 25 grupos, ~127 comandos. | Los objetos de dominio son familias con muchos verbos; agrupar por familia hace la lista descubrible y estable. |
| D3 | **Un** sobre de respuesta (`redes.envelope/1`) para todos los comandos, incluidos `capabilities` y `schema`. | Un solo parser para el agente; un solo JSON Schema que validar. |
| D4 | `ok:false` ⟺ `meta.exit_code ≠ 0`. Invariante duro. | El agente tiene una señal, no dos contradictorias. |
| D5 | JSON por defecto cuando stdout no es TTY; tabla cuando sí lo es. | El caso de uso principal es la IA; el humano no paga la verbosidad. |
| D6 | Identidad de `01-domain`: `id` UUID v7 opaco + `name` = `codigo` legible; con `--deterministic` el v7 se deriva del código. | Importaciones reproducibles y tests byte a byte sin inventar un esquema de IDs paralelo al del dominio. |
| D7 | Los criterios de diseño fallidos son *diagnósticos*, no excepciones; el comando de evaluación devuelve `E_CHECK_FAILED` sombrilla. | Separa "no pude ejecutar" de "ejecuté y el diseño no cumple". |
| D8 | `--explain` produce un DAG de trazabilidad con `node_id` en cada número. | Requisito del cliente: todo número debe poder justificarse. |
| D9 | Escrituras destructivas: dos fases (`--dry-run` → `confirm_token` → ejecución). | Un LLM no debe poder borrar un pozo por un `--force` mal puesto. |
| D10 | MCP **generado** desde el mismo registro de acciones que el CLI; perfiles `core` (12 tools, el número de CLI-06) / `full` / `read-only`. | Deriva cero entre superficies; el test de cobertura es trivial. |
| D11 | Sin REPL en v1; `redes.toml` con tareas + `plan run`. | El REPL introduce estado implícito que choca con el modelo de revisiones. |
| D12 | Todo número con unidad es `{value, unit}`; las coordenadas son la única excepción. | Ambigüedad de unidades es el error #1 de un agente en CAD. |
| D13 | Id de acción en inglés (manifiesto, MCP, `command`); etiqueta CLI en español por tabla de locale. | El cliente ya leyó `redes proyecto nuevo`; el agente necesita ids estables. Ver Anexo A.2. |
| D14 | v1 expone 7 familias; el patrón CRUD se hereda a las 17 de `01-domain`. | Cubre el caso de uso real (AP + AL sanitario) sin 60 comandos muertos. Ver Anexo A.3. |

---

## 2. Principios de diseño

1. **La IA es el usuario primario, el humano el secundario.** El humano tiene `--help`, tablas y tareas; la IA tiene esquemas, códigos y determinismo. Ningún comando es exclusivo de uno.
2. **Una acción, un comando, tres adaptadores.** La GUI (Rune), el CLI y el servidor MCP son tres adaptadores sobre el mismo enum `Action` del motor. No hay lógica de negocio en el CLI.
3. **Todo lo que la GUI puede hacer, el CLI lo puede hacer.** Y al revés: lo que no está en el registro no existe en la GUI.
4. **Estable por defecto, extensible sin romper.** El sobre sólo crece de forma aditiva (`redes.envelope/1` no cambia de forma dentro de 1.x). Los comandos se deprecan, no se renombran.
5. **Silencio en stdout.** stdout lleva **un** documento JSON (o un flujo NDJSON con `--stream`). Progreso, logs y barras van a stderr. Un agente nunca tiene que separar ruido de datos.
6. **Reproducible o no es un comando.** Misma entrada + misma revisión ⇒ mismo sobre byte a byte (`--deterministic`). Sin esto no hay snapshot testing ni confianza.
7. **Los errores enseñan.** Cada error trae `code`, `field`, `expected`, `hint` y `hint_command`: el siguiente comando ejecutable.

---

## 3. Arquitectura de la superficie

```
                 ┌──────────────────────────────┐
   Rune GUI ────►│   Action registry (Rust)     │◄──── CLI (clap)
                 │   enum Action { .. }         │◄──── MCP server (rmcp)
                 │   id · mutates · schema · ex │
                 └──────────────┬───────────────┘
                                ▼
                     ┌────────────────────┐
                     │  Engine (headless) │  proyecto · TIN · hidráulica · tierras
                     └────────────────────┘
```

Un `Action` declara: `id()` (`"manhole.create"`), `mutates()`, `destructive()`, `idempotent()`, `dry_run()`, `input_schema()`, `output_schema()`, `examples()`, `error_codes()`, `since()`, `stability()`.

De ese registro se generan, sin intervención manual:

| Artefacto | Consumidor |
|---|---|
| Árbol clap + flags + `--help` con ejemplos | humano / shell |
| JSON Schema (entrada y salida) | IA, validadores, CI |
| `tools/list` + `inputSchema` + `outputSchema` + anotaciones | MCP |
| `capabilities` / `schema` | descubrimiento del agente |
| Fixtures de test | CI |

**Regla de oro:** ningún comando se escribe a mano en el CLI ni en el MCP. Si hace falta un `if` específico en el adaptador, se ha modelado mal la acción.

---

## 4. Convenciones globales

### 4.1 Resolución del proyecto

Orden: `--project <ruta|nombre>` → `$REDES_PROJECT` → `[project].path` de `redes.toml` → búsqueda ascendente de `*.redes` → error `E_PROJ_NOT_FOUND` (exit 4, con `hint_command`).

Archivo de proyecto: `project.redes` (SQLite, single-file) + directorio lateral `.redes/` (journal, caché, claves de idempotencia). Los comandos de descubrimiento (`capabilities`, `schema`, `errors`, `units`, `version`) **no** abren el proyecto: deben responder en < 15 ms sin tocar disco de proyecto.

### 4.2 Flags globales (en todos los comandos, `global = true`)

| Flag | Tipo / valores | Default | Efecto |
|---|---|---|---|
| `--project <p>` | ruta o nombre | autodescubierto | proyecto objetivo |
| `--read-only` / `--solo-lectura` | flag | no | abre el proyecto en modo lectura: toda escritura falla con `E_PROJ_READONLY` |
| `--format <f>` | `json\|ndjson\|table\|csv\|md` | `json` si no-TTY, `table` si TTY | formato de stdout |
| `--json` | atajo | — | `= --format json` |
| `--pretty` | flag | compacto | indentación 2 (tests, humanos) |
| `--compact` | flag | — | quita `unit` cuando coincide con el sistema del proyecto (ahorro de tokens) |
| `--units <u>` | `si\|project\|imperial` | `project` | convierte todas las magnitudes de salida |
| `--fields <a,b.c>` | lista | todos | proyección de `data.object` / `data.items[]` |
| `--limit <n>` / `--cursor <c>` / `--all` | 1..1000 | 100 | paginación |
| `--sort <campo[:asc\|desc]>` | campo de la whitelist | canónico | orden explícito |
| `--out <ruta\|->` | ruta | — | artefacto; `-` = binario a stdout |
| `--outdir <dir>` | ruta | `./out` | varios artefactos |
| `--mkdir` / `--overwrite` | flag | no | creación de directorios / reemplazo |
| `--dry-run` | flag | no | planifica y valida, no muta |
| `--explain[=<nivel>]` | `off\|summary\|full` | `summary` si presente | traza de cálculo |
| `--idempotency-key <k>` | string | — | repetición segura |
| `--expect-revision <n>` | entero | — | bloqueo optimista |
| `--fail-on <s>` | `never\|warning\|error` | `error` en `*.validate`/`*.check`, `never` resto | elevación a exit ≠ 0 |
| `--confirm-token <t>` | string | — | segunda fase de operaciones destructivas |
| `--stream` | flag | no | eventos NDJSON + resultado final |
| `--deterministic` | flag | no | fija reloj, IDs y elimina volátiles |
| `--echo-input` | flag | no | eco de argv resuelto en `meta.input` |
| `--timeout-ms <n>` | entero | 120000 | aborta con `E_LIMIT_TIMEOUT` |
| `--quiet` / `--no-color` / `--no-progress` | flag | — | silencio, sin ANSI |
| `--lang <es\|en>` | idioma | `es` | idioma de `message`/`hint` (los `code` son siempre en inglés) |
| `-v` / `--log <nivel>` | — | `warn` | logging a stderr |

Precedencia: **flag > variable de entorno > `redes.toml` > default**. Variables: `REDES_PROJECT`, `REDES_JSON`, `REDES_UNITS`, `REDES_FORMAT`, `REDES_NO_COLOR`, `NO_COLOR`, `REDES_IDEMPOTENCY_KEY`, `REDES_PROFILE`, `REDES_LOG`, `REDES_OUT`.

### 4.3 Identidad, códigos y orden

Se adoptan las dos claves de `01-domain §2.4`, sin inventar una tercera:

| Clave | Tipo | Regla |
|---|---|---|
| `id` | UUID v7 | opaco, inmutable, nunca se muestra si el agente puede usar el código |
| `name` | texto = `codigo` del dominio | legible, único por familia, patrón por familia (`PZ-001`); `--codigo` es alias aceptado |
| `rev` | entero | revisión de la entidad (`≥ 1`), independiente de la revisión del proyecto |

El CLI llama `name` al `codigo` del modelo a propósito: un solo término en toda la interfaz (`--name`, `"name"`, orden canónico) y `--codigo` aceptado como alias. La etiqueta libre del dominio (`nombre`) se expone como `label`, porque `name` ya está tomado por el código único.

| Familia | Patrón de `name` | `natural_key` (CLI-04) |
|---|---|---|
| pozo | `PZ-001` | `name` |
| tubería | `TB-001` | `name` |
| descarga / vertedero | `DF-001` | `name` |
| válvula | `VL-001` | `name` |
| llave (F05) | `LL-001` | `name` |
| tanque | `TK-001` | `name` |
| planta | `PL-001` | `name` |
| medidor | `MD-001` | `name` |
| cámara | `CM-001` | `name` |
| sifón | `SF-001` | `name` |
| cruce | `CR-001` | `name` |
| acometida | `AC-001` | `name` |
| sumidero | `SM-001` | `name` |
| canal | `CN-001` | `name` |
| captación | `CP-001` | `name` |
| estación de bombeo | `EB-001` | `name` |
| elemento de apoyo | `AP-001` | `name` |
| TIN | `TIN-<slug>` | `name` |
| proyecto | `project.redes` (o el `*.redes` que se abra) | ruta canónica |

Reglas:

* Los patrones son la **propuesta** del registro; `01-domain §4.3` manda sobre los catálogos. Si una familia tiene subtipos con prefijo propio (`PZ-C` para pozo de caída), se declara en el manifiesto y se valida con `E_STATE_BAD_CODE`.
* Orden canónico: `(name, id)` con comparación byte a byte. El UUID nunca es el criterio de orden visible.
* **Determinismo.** Con `--deterministic`, el UUID v7 se deriva de `BLAKE3(project_id ‖ familia ‖ name)` conservando los bits de versión (7) y variante de v7: importaciones repetidas producen los mismos IDs y los golden tests comparan byte a byte. Sin el flag se emite un v7 real (orden temporal, no adivinable por el agente).
* Recrear un objeto borrado con el mismo `name` lo revive con el mismo `id` en modo determinista; en modo normal recibe un `id` nuevo.
* La clave natural se declara en el manifiesto (`esquema.comandos[].natural_key`), que es lo que exige CLI-04.

### 4.4 Orden determinista

| Colección | Orden canónico |
|---|---|
| `diagnostics`, `warnings` | `(severity_rank, code, object.family, object.id, path)` — `severity_rank`: error=0, warning=1, info=2 |
| `data.items[]` | `(sort_key, id)` con `sort_key` = `name` documentado por familia; comparación **byte a byte** (nunca *collation* local) |
| `artifacts` | `path` ascendente |
| `explain.nodes` | orden topológico estable (post-orden de dependencias, desempate por `id`) |
| eventos `--stream` | orden de emisión del plan, estrictamente secuencial |
| claves JSON | orden declarado en el struct Rust (nunca alfabético, nunca aleatorio) |

Números: `serde_json` con round-trip más corto y redondeo **una sola vez** según `units.decimals` (`length: 3`, `volume: 3`, `flow: 3`, `velocity: 2`). Prohibido `NaN`/`Infinity` en JSON; si el cálculo los produce → `E_HYD_NO_SOLUTION` o `null` + diagnóstico.

### 4.5 Sistema de unidades

`units.system = "si"` por defecto. Presión en **mca** (metros columna de agua) porque es la unidad de la práctica de redes en la región; diámetros en **mm** (DN); caudal en **L/s** en la interfaz y **m³/s** en la traza de cálculo (`explain` declara ambas).

Regla: toda magnitud es `{"value": <number>, "unit": "<símbolo>"}`. **Única excepción:** las coordenadas, `{"x":…,"y":…,"z":…,"crs":"EPSG:32717"}`, porque el CRS ya define unidad y datum, y triplicar el `unit` en cada vértice es un coste de tokens sin ganancia.
`--compact` sustituye magnitudes por números desnudos cuando la unidad es la del sistema del proyecto; el bloque `units` del sobre sigue diciendo cuál es.

---

## 5. Árbol de comandos

25 grupos, 127 comandos. Todos aceptan los flags globales de §4.2; las tablas listan sólo los específicos. `R` = requerido, `O` = opcional.

### 5.1 Vista de grupos

| Grupo | Comandos | Muta | En MCP `core` |
|---|---|---|---|
| `project` | 11 | sí (algunos) | 1 |
| `schema` | 6 | no | 1 |
| `terrain` | 9 | sí | 0 |
| `manhole` | 8 | sí | 1 |
| `pipe` | 13 | sí | 1 |
| `outfall` | 5 | sí | 0 |
| `valve` | 6 | sí | 0 |
| `llave` (F05) | 5 | sí | 0 |
| `tank` | 6 | sí | 0 |
| `service` | 6 | sí | 0 |
| `network` | 8 | sí (2) | 1 |
| `hydraulics` | 7 | sí (scenarios) | 2 |
| `earthwork` | 3 | no | 1 |
| `profile` | 2 | no | 0 |
| `report` | 2 | no (artefactos) | 0 |
| `export` | 5 | no (artefactos) | 1 |
| `render` | 5 | no (artefactos) | 1 |
| `query` | 1 | no | 1 |
| `ui` | 3 | no | 0 (excluido) |
| `bulk` | 5 | sí | 0 |
| `plan` | 4 | sí | 1 |
| `task` | 2 | sí | 0 |
| `mcp` | 3 | no | — |
| `doctor` | 1 | no | 0 |
| `completion` | 1 | no | 0 |

### 5.2 `project` — ciclo de vida

| Comando | Propósito | Flags | `data` |
|---|---|---|---|
| `project init` | crea `project.redes` con CRS, unidades y normas | **R** `--dir`; O `--name --crs --units --codes --template` | `project_init` |
| `project info` | estado, conteos por familia, revisión, TINs | O `--fields` | `project_info` |
| `project set` | cambia metadatos del proyecto | O `--name --crs --units --codes` (≥1) | `project_change` |
| `project history` | journal de revisiones | O `--limit --since <rev> --actor` | `project_history` |
| `project diff` | diferencias entre revisiones o entre archivos | O `--from <rev\|archivo.redes> --to <rev\|archivo.redes>` (default: última vs anterior), `--include-geometry` | `project_diff` |
| `project undo` | revierte N revisiones creando una nueva | O `--steps 1` \| `--to <rev>` | `project_undo` |
| `project redo` | rehace lo deshecho | O `--steps 1` | `project_undo` |
| `project restore` ⟂ | vuelve exactamente a una revisión | **R** `--to`; **R** confirmación | `project_undo` |
| `project check` | integridad (SQLite, huérfanos, CRS, checksums) | — | `check_report` |
| `project upgrade` | migra `schema_version` | O `--to`; crea backup | `project_upgrade` |
| `project gc` | compacta y purga caché/idempotencia | O `--keep-revisions 50` | `gc_report` |

⟂ = destructivo (requiere `confirm_token`).

`undo` nunca reescribe historia: appendea una revisión inversa. Eso mantiene `--expect-revision` y el journal como fuente de verdad auditable.

### 5.3 `schema` — introspección

| Comando | Propósito | Flags | `data` |
|---|---|---|---|
| `capabilities` | catálogo completo: comandos, códigos de error, unidades, vocabularios, versiones | O `--brief --profile --group <g>` | `capabilities` |
| `schema <comando>` | JSON Schema 2020-12 de entrada y salida + ejemplos; `--all` vuelca todo el catálogo | O `--brief` (sólo propiedades top-level), `--all --outdir <dir>` | `schema_doc` |
| `errors` | registro de códigos con `exit`, `retryable`, `recoverable`, `hint` | O `--code <c>` | `error_catalog` |
| `units` | registro de magnitudes, símbolos, decimales y conversiones | — | `unit_catalog` |
| `envelope` | JSON Schema del sobre + ejemplo canónico | — | `schema_doc` |
| `version` | versiones de CLI, motor, sobre, MCP, base CRS | — | `versions` |

`esquema` es la etiqueta española de `capabilities` y `esquema <comando>` la de `schema <comando>` (Anexo A.2): el id es estable, la etiqueta es locale.

`capabilities --brief` es el **primer comando que un agente debe ejecutar**: ~900 tokens para 127 comandos. `capabilities` completo ronda 14k tokens, reservado para exploración puntual.

### 5.4 `terrain` — TIN y superficie

| Comando | Propósito | Flags | `data` |
|---|---|---|---|
| `terrain import` | ingesta nube/TIN | **R** `--file`; O `--format auto\|xyz\|csv\|las\|geojson\|tif --crs --name --breaklines --decimate <m> --clip <bbox>` | `tin_import` |
| `terrain list` / `terrain info` | catálogo y calidad | `--tin <id\|name>` | `tin_list` / `tin_info` |
| `terrain sample` | cota en puntos o a lo largo de una línea | **R** `--tin`; **R** `--at x,y` \| `--line "x1,y1;x2,y2"`; O `--step <m> --snap` | `tin_sample` |
| `terrain contour` | curvas de nivel | **R** `--tin --interval`; O `--bbox` | `artifacts` |
| `terrain grid` | ráster/XYZ regular | **R** `--tin --cell`; O `--bbox --format tif\|xyz` | `artifacts` |
| `terrain clip` | recorta el TIN | **R** `--tin --bbox`; O `--name` | `tin_import` |
| `terrain cut-fill` | movimiento de tierras superficie vs diseño | **R** `--tin --design`; O `--grid 1.0` | `cut_fill` |
| `terrain delete` ⟂ | borra un TIN | **R** `--tin` | `unit` |

`terrain sample` es la primitiva que usa el agente para "poner el pozo en el terreno": devuelve `inside:false` con `E_GEO_OUT_OF_BOUNDS` si el punto cae fuera del TIN, en vez de extrapolar en silencio.

### 5.5 Familias de objetos — patrón CRUD canónico

Toda familia implementa el mismo cuádruple, con idéntica semántica de paginación, `--fields`, revisiones y errores:

| Comando | Propósito | Flags | `data` |
|---|---|---|---|
| `<familia> create` | crea un objeto; valida topología y criterios | **R** `--name` + geométricos propios; O `--id --tags --dry-run` | `object_envelope` |
| `<familia> get` | lee uno por `--id` o `--name` | **R** `--id` \| `--name`; O `--fields` | `object_envelope` |
| `<familia> list` | lista filtrada | O `--bbox xmin,ymin,xmax,ymax --near x,y,radio --where <expr> --sort --limit --cursor --fields` | `object_list` |
| `<familia> update` | modifica campos; `changes[]` con `from`/`to` | **R** `--id` \| `--name`; O cualquier campo + `--if-rev <n>` | `object_envelope` |
| `<familia> delete` ⟂ | borra; falla si hay dependientes | **R** `--id` \| `--name`; O `--cascade` | `delete_result` |

Diferencias por familia (sólo lo propio):

| Familia | `create` — flags propios | Comandos extra |
|---|---|---|
| `manhole` | **R** `--at x,y`; O `--elev <m>` (default: cota TIN) `--rim <m>` `--depth <m>` `--inside-diameter <m>` `--material --cover --access --code` | `move --to x,y [--recompute-elev]`, `set-inverts --invert-in --invert-out`, `set-rim --rim` |
| `pipe` | **R** `--from <nodo> --to <nodo> --diameter <mm>`; O `--material --shape circular\|rect --roughness n=<v> --invert-from <m>` + **exactamente uno** de `--invert-to\|--slope\|--drop` | `split --at <estación\|objeto>`, `merge --into <pipe>`, `reverse`, `set-slope`, `set-inverts`, `resize --diameter`, `route --from --to [--avoid]`, `check --rules cover,slope,velocity` |
| `outfall` | **R** `--at x,y --invert <m>`; O `--type river\|channel\|treatment --receiving "<nombre>" --angle` | — |
| `valve` | **R** `--on-pipe <pipe> --station <m>`; O `--type gate\|check\|air\|reducer --state open\|closed --setpoint <mca>` | `set-state --state` |
| `llave` (F05) | **R** `--on-pipe --station`; O `--type hydrant\|corporation\|curb\|cap --diameter --outlets` | — |
| `tank` | **R** `--at x,y --bottom-elev`; O `--diameter\|--volume --top-elev --min-level --max-level` | `set-levels` |
| `service` | **R** `--from-pipe --to-address`; O `--station --side left\|right --diameter --length` | `connect --auto` (acometida a la red más cercana) |

`pipe route` no muta: devuelve 1..n alineamientos candidatos con longitud, movimiento de tierras estimado y diagnósticos; el agente elige y luego crea.

### 5.6 `network` — conectividad y topología

| Comando | Propósito | Flags | `data` |
|---|---|---|---|
| `network validate` | reglas de red; `--fail-on` default `error` | O `--rules all\|topology,cover,slope,geometry,attach --fail-on` | `check_report` |
| `network topology` | grados, componentes, nodos frontera | O `--components --orphans` | `topology_report` |
| `network trace` | recorrido aguas arriba/abajo | **R** `--from`; O `--upstream\|--downstream --depth --include-attachments` | `trace_result` |
| `network path` | camino entre dos nodos | **R** `--from --to`; O `--weight length\|slope\|cost` | `path_result` |
| `network components` | subredes desconectadas | — | `component_list` |
| `network stats` | longitudes por material/DN, conteos | O `--group-by` | `network_stats` |
| `network snap` | ajusta extremos dentro de tolerancia | O `--tolerance 0.05` | `snap_result` |
| `network heal` | crea uniones faltantes y reporta | O `--tolerance --dry-run` (default ON) | `heal_result` |

`network heal` muta: por eso `--dry-run` está **activo por defecto** y hay que pasar `--apply` para ejecutar. Es la única excepción al default `--dry-run=false` y está documentada en el esquema (`"default_apply": false`).

### 5.7 `hydraulics`

| Comando | Propósito | Flags | `data` |
|---|---|---|---|
| `hydraulics solve` | resuelve la red para un escenario | **R** `--scenario <n>` \| `--storm <años>y --duration <min>`; O `--method manning\|hazen --iterations 200 --tolerance 1e-6 --checks --explain` | `solve_result` |
| `hydraulics check` | contrasta resultados contra criterios | O `--codes <pack> --fail-on error --checks pressure,velocity,fill,surcharge` | `check_report` |
| `hydraulics design` | propone diámetros/pendientes | O `--min-velocity 0.6 --max-velocity 5.0 --fill-max 0.75 --min-cover 1.0 --apply` | `design_proposal` |
| `hydraulics scenario create` | escenario nombrado (lluvia, demanda) | **R** `--name`; O `--storm --duration --demand-pattern` | `scenario` |
| `hydraulics scenario list` / `compare` | catálogo / comparación | `compare` **R** `--scenarios a,b` | `scenario_list` / `scenario_compare` |
| `hydraulics demand set` | patrón de aporte | **R** `--pattern <archivo\|json>`; O `--apply` | `demand_result` |

`hydraulics design` **no muta sin `--apply`**: devuelve propuestas con `would_change[]`, `diagnostics` de la propuesta y `confirm_token`. Un agente puede iterar diseño sin tocar el proyecto.

### 5.8 `earthwork`, `profile`, `report`, `export`, `render`

| Comando | Propósito | Flags | `data` |
|---|---|---|---|
| `earthwork compute` | volúmenes de excavación por zanja, estructura y capas | O `--selection all\|--pipes --nodes --bbox --trench-extra 0.30 --bedding 0.15 --side-slope 1:0.25 --topsoil 0.20 --stations 1.0 --swell 1.25 --explain` | `earthwork_result` |
| `earthwork balance` | corte/relleno y excedente | O `--include-structures --compact` | `earthwork_balance` |
| `earthwork export` | planilla de volúmenes | **R** `--out`; O `--format csv\|xlsx` | `artifacts` |
| `profile compute` | perfil longitudinal de un camino de tuberías | **R** `--path "MH-01,MH-02,MH-03"` \| `--selection`; O `--stations 20 --interval <m> --include-ground --exaggeration 10 --explain` | `profile_result` |
| `profile export` | perfil a archivo | **R** `--out`; O `--format csv\|svg\|dxf\|png` | `artifacts` |
| `report templates` | plantillas disponibles | — | `template_list` |
| `report build` | informe (PDF/HTML/MD) | **R** `--template --out`; O `--format pdf\|html\|md --sections --include-artifacts` | `artifacts` |
| `export dxf` | plano DXF por capas | **R** `--out`; O `--version R2018 --layers --blocks` | `artifacts` |
| `export geojson` | objetos a GeoJSON | **R** `--out`; O `--crs EPSG:4326 --families --precision 6` | `artifacts` |
| `export landxml` | red a LandXML | **R** `--out`; O `--version 1.2 --units metric` | `artifacts` |
| `export csv` | tablas planas | **R** `--out`; O `--families manhole,pipe --delimiter "," --header` | `artifacts` |
| `export pdf` | PDF de plano/informe simple | **R** `--out`; O `--template --size A1\|A3 --dpi` | `artifacts` |
| `render plan` | PNG de planta | **R** `--out`; O `--bbox --size 1600x1200 --dpi 96 --layers --theme light\|dark --labels --scale-bar --legend --baseline <png>` | `render_result` |
| `render profile` | PNG de perfil | **R** `--out --path`; O `--size --exaggeration` | `render_result` |
| `render view` | PNG 3D/orto | **R** `--out`; O `--camera iso\|ortho --azimuth --elevation --bbox` | `render_result` |
| `render object` | PNG de un objeto aislado | **R** `--id --out` | `render_result` |
| `render tree` | árbol de elementos (Rune `--tree`) de una vista | O `--view plan --settle 0` | `ui_tree` |

Renderizado: se apoya en el modo headless de Rune (`--snap` a PNG, reloj virtual, `--tree`, `--script`). `--snap` inicializa GPU —Rune no tiene renderizador por software en el camino normal—, así que `redes render` exige un dispositivo gráfico headless; si no lo hay: `E_RENDER_DEVICE`, nunca una imagen distinta en silencio. `--deterministic-fonts` fuerza el rasterizador propio de Rune (Windfoil), que produce los mismos píxeles en macOS, Windows y Linux — es lo que permite comparar contra una línea base en CI.

### 5.9 `query`, `ui`, `bulk`, `plan`, `task`, `mcp`, `doctor`

| Comando | Propósito | Flags | `data` |
|---|---|---|---|
| `query objects` | lectura transversal con filtro y proyección (una o varias familias) | O `--families manhole,pipe --where <expr> --bbox --near x,y,radio --fields --sort --limit` | `query_result` |
| `ui open` | abre la GUI en el proyecto | O `--view plan --select <id>` | `ui_handle` |
| `ui tree` | volcado del árbol de elementos (verificación visual del agente) | O `--settle 0 --wait-ms` | `ui_tree` |
| `ui script` | entrada guionizada headless (Rune `--script`) | **R** `--file`; O `--trace` | `ui_script_result` |
| `bulk create` | alta masiva desde CSV/JSONL | **R** `--family --file`; O `--atomic --on-error abort\|continue --batch-size 500` | `bulk_result` |
| `bulk update` | modificación masiva | **R** `--family --file`; O `--atomic --on-error` | `bulk_result` |
| `bulk delete` ⟂ | baja masiva por filtro | **R** `--family --where`; O `--atomic` | `bulk_result` |
| `bulk import` | importa desde DXF/GeoJSON/LandXML/CSV | **R** `--file`; O `--format --mapping --atomic --dry-run` | `bulk_result` |
| `bulk validate` | valida un lote sin escribirlo | **R** `--family --file` | `bulk_result` |
| `plan run` | ejecuta un plan JSON de N comandos | **R** `--file <ruta\|->`; O `--atomic --on-error` | `plan_result` |
| `plan validate` | valida el plan y resuelve referencias sin ejecutar | **R** `--file` | `plan_validation` |
| `plan explain` | muestra pasos resueltos y diff previsto | **R** `--file` | `plan_explanation` |
| `plan schema` | JSON Schema del plan | — | `schema_doc` |
| `task list` / `task run` | tareas de `redes.toml` | `run` **R** `--name` | `plan_result` |
| `mcp serve` | servidor MCP | O `--stdio\|--http --profile --allow-out <dir>` | (protocolo) |
| `mcp manifest` | vuelca el manifiesto de tools | **R** `--out`; O `--profile` | `artifacts` |
| `mcp selftest` | ejecuta llamadas de humo y produce evidencia | **R** `--out` | `artifacts` |
| `doctor` | autodiagnóstico (CRS, fuentes, GPU, permisos) | O `--fix` | `doctor_report` |
| `completion <shell>` | completado para humano | **R** shell | texto |

### 5.10 Formas de `data` (contrato de salida)

| Forma | Campos |
|---|---|
| `object_envelope` | `{object, changes:[{field,from,to}], placement:{tin_id,z_source,triangle_id}}` |
| `object_list` | `{items:[…], count, total}` + bloque `pagination` del sobre |
| `query_result` | `{items:[{family,id,name,…}], count, total, facets:{family:{…}}}` — `items` heterogéneos, siempre con `family` |
| `delete_result` | `{deleted:[{id,name}], references_removed:n, revision}` |
| `tin_import` | `{tin:{id,name,points,triangles,bbox:{min:{x,y,z},max:{x,y,z}},z_range,area,sha256}}` |
| `tin_sample` | `{samples:[{at:{x,y},z,inside,tin_id,triangle_id}], count}` |
| `check_report` | `{verdict:"pass"\|"fail", checked:{rules:[…],objects:n}, counts:{error:n,warning:n,info:n}}` |
| `solve_result` | `{scenario, convergence:{iterations,residual,tolerance}, nodes:[…], links:[…], totals:{inflow,outflow,storage}}` |
| `design_proposal` | `{proposals:[{object_id,field,from,to,reason}], would_change:n, confirm_token}` |
| `earthwork_result` | `{quantities:[{item,volume,unit}], by_pipe:[…], by_structure:[…], totals, assumptions}` |
| `profile_result` | `{path, stations:[{station,ground_z,invert_z,depth,pipe_id,diameter,slope}], length, exaggeration}` |
| `render_result` | `{image:{width,height,dpi,bbox,layers,theme}, diff?:{pixels_changed,ratio,diff_png}}` |
| `bulk_result` | `{rows:n, applied:n, failed:n, rejected:[{row,code,message}], revision}` |
| `plan_result` | `{plan_id, steps:n, applied:n, failed:n, rolled_back:bool, revision_before, revision_after, results:[{step,ok,data_ref}]}` |
| `capabilities` | `{cli_version, engine_version, envelope_schema, mcp_server_version, profiles, commands:[…], count, error_codes:[…], vocabularies}` |
| `schema_doc` | `{command, input_schema, output_schema, examples:[…], error_codes:[…], stability}` |

---

## 6. El sobre JSON

### 6.1 Forma canónica

Todas las claves están **siempre presentes** (con `null`, `[]` o `{}`). Orden canónico fijo:

```json
{
  "schema": "redes.envelope/1",
  "ok": true,
  "command": "manhole.create",
  "request_id": "req_01JCV8Z3Q5K7",
  "project": {
    "id": "0193f2f1-5f60-7719-c273-4e5f6a7b8c09",
    "path": "C:\\obra\\project.redes",
    "name": "Alcantarillado La Esperanza",
    "crs": "EPSG:32717",
    "units": "si",
    "revision": 41
  },
  "data": { "object": { "id": "0193f2c1-7a4e-7c3b-9e21-5b8a4c6d1f03", "name": "MH-01" } },
  "artifacts": [],
  "diagnostics": [],
  "warnings": [],
  "explain": null,
  "pagination": null,
  "units": {
    "system": "si", "length": "m", "area": "m2", "volume": "m3",
    "flow": "L/s", "velocity": "m/s", "pressure": "mca", "slope": "m/m",
    "diameter": "mm", "angle": "deg", "headloss": "m",
    "decimals": { "length": 3, "volume": 3, "flow": 3, "velocity": 2 }
  },
  "meta": {
    "cli_version": "0.1.0",
    "engine_version": "0.1.0",
    "envelope_schema": "redes.envelope/1",
    "exit_code": 0,
    "deterministic": true,
    "replayed": false,
    "duration_ms": 7,
    "started_at": "2026-11-18T13:55:27Z",
    "host": "windows",
    "locale": "es-EC"
  },
  "error": null
}
```

| Clave | Contrato |
|---|---|
| `schema` | constante `redes.envelope/1`; cambia sólo en un major |
| `ok` | **`ok:false` ⟺ `meta.exit_code ≠ 0`** (invariante testeable) |
| `command` | id canónico de la acción (`"manhole.create"`), no la línea de comandos |
| `request_id` | `req_` + ULID; con `--idempotency-key` se deriva de la clave (reproducible) |
| `project` | `null` en comandos que no abren proyecto |
| `data` | carga de dominio; `null` si el comando falló antes de producirla |
| `artifacts` | archivos producidos (§8.6) |
| `diagnostics` | **todas** las incidencias, con severidad explícita |
| `warnings` | subconjunto derivado de `diagnostics` con `severity:"warning"` (compatibilidad y resumen barato) |
| `explain` | traza de cálculo o `null` (§8.2) |
| `pagination` | `{limit, returned, total, next_cursor, has_more}` o `null` |
| `units` | contexto de unidades de todo el sobre |
| `meta` | versiones, `exit_code`, determinismo, replay, tiempos |
| `error` | primer diagnóstico con `severity:"error"`, o `null` |

### 6.2 Diagnóstico (unidad atómica de información)

```json
{
  "code": "E_TOPO_DISCONNECTED",
  "severity": "error",
  "message": "El pozo MH-07 no tiene ninguna tubería conectada.",
  "object": { "family": "manhole", "id": "0193f2c3-9c32…", "name": "MH-07" },
  "path": "network.nodes['0193f2c3-9c32…'].links",
  "field": "--to",
  "expected": "grado ≥ 1 en una red de gravedad",
  "accepted": null,
  "details": { "degree": 0, "component": 3 },
  "related": ["0193f2c5-be54-7067-d3ca-4f5a6b7c8d09"],
  "hint": "Conecte MH-07 al pozo aguas arriba más cercano o elimínelo.",
  "hint_command": "redes pipe create --from MH-06 --to MH-07 --diameter 200 --json",
  "docs": "https://docs.redes.dev/errors#E_TOPO_DISCONNECTED",
  "retryable": false,
  "recoverable": true
}
```

`hint_command` es un **comando ejecutable tal cual** (o plantilla con `<…>` si faltan datos). Es la diferencia entre un error que el agente puede resolver y uno que sólo puede reportar.

### 6.3 Sobre de error

```json
{
  "schema": "redes.envelope/1",
  "ok": false,
  "command": "pipe.create",
  "data": null,
  "artifacts": [],
  "diagnostics": [ { "code": "E_GEO_SLOPE_MISMATCH", "severity": "error", "…": "…" } ],
  "warnings": [],
  "explain": null,
  "pagination": null,
  "units": { "system": "si", "length": "m", "…": "…" },
  "meta": { "exit_code": 5, "…": "…" },
  "error": {
    "code": "E_GEO_SLOPE_MISMATCH",
    "severity": "error",
    "message": "La pendiente indicada no coincide con los inverts dados.",
    "field": "--slope",
    "expected": "−0.004167 m/m ± 1e-6",
    "accepted": null,
    "details": { "given": -0.0042, "derived": -0.0041667, "delta": 3.33e-5, "tolerance": 1e-6 },
    "hint": "Omita --slope y deje que se derive de --invert-to, o corrija el invert.",
    "hint_command": "redes pipe create --name T-01 --from MH-01 --to MH-02 --diameter 250 --invert-from 2448.120 --invert-to 2447.620 --json",
    "docs": "https://docs.redes.dev/errors#E_GEO_SLOPE_MISMATCH",
    "retryable": false,
    "recoverable": true
  }
}
```

### 6.4 Taxonomía de códigos

Convención: `E_<ÁREA>_<DETALLE>` para violaciones, `W_` para avisos. Áreas: `ARG`, `IO`, `FMT`, `PROJ`, `STATE`, `GEO`, `TOPO`, `HYD`, `EARTH`, `CHECK`, `RPT`, `RENDER`, `LIMIT`, `CONFIRM`, `INTERNAL`, `NUM`.

**Los códigos de salida son los de CLI-05**: `0` ok · `2` validación · `3` norma · `4` E/S · `5` topografía/geometría · `6` conflicto de versión. Se añaden cinco que CLI-05 no cubre y que el agente necesita distinguir: `1` interno, `7` límite/paginación, `8` confirmación requerida, `9` solver numérico, `130` interrumpido. La columna `Exit` de la tabla es normativa: el registro viaja en `esquema.codigos_error[]` y el proceso devuelve exactamente ese número.

| Código | Exit | Retryable | Recoverable | Cuándo |
|---|---|---|---|---|
| `E_ARG_MISSING` | 2 | no | sí | falta un flag requerido |
| `E_ARG_UNKNOWN` | 2 | no | sí | flag o subcomando inexistente |
| `E_ARG_BAD_VALUE` | 2 | no | sí | valor fuera de dominio (`accepted` lo lista) |
| `E_ARG_CONFLICT` | 2 | no | sí | flags mutuamente excluyentes |
| `E_ARG_UNKNOWN_FIELD` | 2 | no | sí | `--fields` o `--sort` fuera de whitelist |
| `E_MCP_PROFILE_DENIED` | 2 | no | sí | tool de escritura con perfil `read-only` |
| `E_IO_NOT_FOUND` | 4 | no | sí | archivo inexistente |
| `E_IO_PERMISSION` | 4 | no | no | sin permiso de escritura |
| `E_IO_EXISTS` | 4 | no | sí | `--out` ya existe sin `--overwrite` |
| `E_FMT_PARSE` | 4 | no | sí | archivo ilegible (con `line`/`column`) |
| `E_FMT_UNSUPPORTED` | 4 | no | sí | formato no soportado |
| `E_PROJ_NOT_FOUND` | 4 | no | sí | no hay proyecto resoluble |
| `E_PROJ_LOCKED` | 6 | **sí** (200 ms) | sí | otro proceso escribe |
| `E_PROJ_VERSION_CONFLICT` | 6 | no | sí | `--expect-revision` no coincide |
| `E_PROJ_SCHEMA_TOO_NEW` | 6 | no | no | proyecto de versión superior |
| `E_PROJ_READONLY` | 6 | no | no | el proyecto se abrió con `--read-only` |
| `E_STATE_NOT_FOUND` | 2 | no | sí | objeto inexistente |
| `E_STATE_DUPLICATE_NAME` | 2 | no | sí | nombre ya usado en la familia |
| `E_STATE_REFERENCED` | 3 | no | sí | borrar dejaría referencias colgando |
| `E_STATE_IDEMPOTENCY_MISMATCH` | 6 | no | sí | misma clave, distinta entrada |
| `E_GEO_OUT_OF_BOUNDS` | 5 | no | sí | punto fuera del TIN/bbox |
| `E_GEO_NO_TIN_AT_POINT` | 5 | no | sí | sin TIN que cubra el punto |
| `E_GEO_CRS_MISSING` / `E_GEO_CRS_MISMATCH` | 3 | no | sí | CRS ausente o incoherente |
| `E_GEO_TRANSFORM_FAILED` | 5 | no | sí | reproyección imposible |
| `E_GEO_SLOPE_MISMATCH` | 5 | no | sí | pendiente/inverts incoherentes |
| `E_GEO_ZERO_LENGTH` | 5 | no | sí | tubería de longitud 0 |
| `E_TOPO_DISCONNECTED` | 3 | no | sí | nodo de grado 0 |
| `E_TOPO_ORPHAN_LINK` | 3 | no | sí | link con extremo inexistente |
| `E_TOPO_SELF_LOOP` | 3 | no | sí | `--from` = `--to` |
| `E_TOPO_DUPLICATE_ENDPOINT` | 3 | no | sí | dos links idénticos |
| `E_TOPO_GRADE_ADVERSE` | 3 | no | sí | pendiente contraria al flujo |
| `E_TOPO_LOOP_GRAVITY` | 3 | no | sí | ciclo en red de gravedad |
| `E_TOPO_ATTACHMENT_OUT_OF_RANGE` | 3 | no | sí | válvula fuera de la tubería |
| `E_TOPO_NODE_OVERLOADED` | 3 | no | sí | grado > máximo del material |
| `E_HYD_NO_SOLUTION` | 9 | sí (1 vez) | sí | no converge con la tolerancia |
| `E_HYD_DIVERGED` | 9 | no | sí | residuo creciente |
| `E_HYD_PRESSURE_LOW` | 3 | no | sí | presión < mínimo del código |
| `E_HYD_PRESSURE_HIGH` | 3 | no | sí | presión > máximo |
| `E_HYD_VELOCITY_LOW` / `_HIGH` | 3 | no | sí | velocidad fuera de rango |
| `E_HYD_FILL_EXCEEDED` | 3 | no | sí | llenado > máximo |
| `E_HYD_SURCHARGE` | 3 | no | sí | nodo en carga |
| `E_HYD_NEGATIVE_SLOPE` | 3 | no | sí | pendiente ≤ 0 en gravedad |
| `E_HYD_ROUGHNESS_MISSING` | 3 | no | sí | material sin `n`/`C` |
| `E_HYD_MISSING_DEMAND` | 3 | no | sí | nodo sin aporte |
| `E_EARTH_TIN_GAP` | 3 | no | sí | zanja sin terreno que la cubra |
| `E_EARTH_NEGATIVE_DIMENSION` | 3 | no | sí | ancho/profundidad ≤ 0 |
| `E_EARTH_COVER_INSUFFICIENT` | 3 | no | sí | recubrimiento < mínimo |
| `E_EARTH_BALANCE_UNRESOLVED` | 3 | no | sí | balance corte/relleno imposible |
| `E_CHECK_FAILED` | 3 | no | sí | **sombrilla** de un `*.validate`/`*.check` con bloqueos |
| `E_RPT_TEMPLATE_MISSING` | 4 | no | sí | plantilla inexistente |
| `E_RENDER_DEVICE` | 4 | sí | sí | sin GPU/dispositivo headless |
| `E_RENDER_VIEW_EMPTY` | 3 | no | sí | bbox sin objetos |
| `E_LIMIT_PAGE_SIZE` | 7 | no | sí | `--limit` > 1000 |
| `E_LIMIT_CURSOR_STALE` | 7 | no | sí | cursor de una revisión anterior |
| `E_LIMIT_TIMEOUT` | 7 | sí | sí | superó `--timeout-ms` |
| `E_CONFIRM_REQUIRED` | 8 | no | sí | operación destructiva sin token válido |
| `E_NUM_NON_FINITE` | 3 | no | no | resultado no representable en JSON |
| `E_INTERNAL` | 1 | sí | no | bug; incluye `details.incident_id` |
| `E_NOT_IMPLEMENTED` | 1 | no | no | acción registrada pero sin motor |
| `W_UNITS_ASSUMED` | 0 | — | — | unidades inferidas |
| `W_CRS_ASSUMED` | 0 | — | — | CRS inferido del proyecto |
| `W_TIN_LOW_DENSITY` | 0 | — | — | TIN por debajo de densidad recomendada |
| `W_TIN_EXTRAPOLATED` | 0 | — | — | cota extrapolada fuera del casco |
| `W_HYD_MINOR_LOSSES_IGNORED` | 0 | — | — | pérdidas locales despreciadas |
| `W_PROFILE_EXTRAPOLATED` | 0 | — | — | perfil prolongado más allá de la red |
| `W_PRECISION_REDUCED` | 0 | — | — | redondeo por `--compact`/decimales |
| `W_DEPRECATED_FLAG` | 0 | — | — | flag en desuso, con reemplazo |

### 6.5 `--json` vs humano

| Contexto | Salida |
|---|---|
| stdout no-TTY (agente, pipe) | JSON compacto en una línea |
| stdout TTY | tabla alineada con encabezado, colores sólo si `NO_COLOR` ausente |
| `--format table` | tabla forzada (con `--fields` para elegir columnas) |
| `--format md` | tabla Markdown (para pegar en informes/PR) |
| `--format ndjson` | un objeto JSON por línea (`data.items[]`), sin sobre |
| `--format csv` | sólo comandos tabulares; cabecera + filas |

En modo humano, `diagnostics` se imprime como bloque `código · severidad · objeto · mensaje`, con `hint_command` en la línea siguiente, y el código de salida es el mismo que en modo JSON. La máquina y el humano ven la misma verdad.

### 6.6 Cómo descubre y reintenta un agente

```
1. redes capabilities --brief --json          → ¿existe el comando?
2. redes schema <comando> --brief --json      → ¿qué flags y tipos?
3. redes <comando> --dry-run --json           → ¿qué va a pasar? (IDs, diff, diagnósticos)
4. redes <comando> --idempotency-key <k> --json
5. si ok:false:
     error.retryable  → esperar retry_after_ms y repetir (misma entrada, misma clave)
     error.recoverable→ leer hint / hint_command, corregir y repetir (clave NUEVA)
     ninguno          → escalar a humano con error.details + incident_id
6. si data.verdict=="fail" en un check → leer diagnostics, priorizar severity=error
```

| Señal | Interpretación | Acción del agente |
|---|---|---|
| `retryable:true` | transitorio (lock, timeout, GPU) | reintentar con backoff; la misma clave es segura |
| `recoverable:true` | error de la petición | corregir según `hint_command`, clave nueva |
| `retryable:false, recoverable:false` | estado irrecuperable | detenerse y reportar |
| `severity:"warning"` con `ok:true` | se ejecutó, hay riesgo | decidir si `--fail-on warning` aplica |
| `meta.replayed:true` | la clave ya se usó | el resultado es el original; no re-aplicar |

---

## 7. Invariantes del contrato (testeables)

1. `ok:false` ⟺ `meta.exit_code ≠ 0`.
2. `schema` nunca cambia dentro de un major.
3. Toda clave de §6.1 está presente en todo sobre, con `null`/`[]` cuando no aplica.
4. `warnings` ⊆ `diagnostics` (mismo `code`, mismo `object`).
5. Ningún número lleva unidad implícita: o es `{value,unit}`, o es coordenada con `crs`, o es un conteo/índice.
6. Dos ejecuciones idénticas con `--deterministic` producen bytes idénticos.
7. Toda escritura acepta `--dry-run` y el plan del dry-run coincide con lo aplicado (diff vacío).
8. Misma `--idempotency-key` + misma entrada ⇒ mismo sobre (`meta.replayed:true`), revisión intacta.
9. Todo `error.code` pertenece al registro de `capabilities.error_codes`, con `exit` idéntico al proceso.
10. Todo `hint` es no vacío; todo `hint_command`, si existe, es sintácticamente válido (`plan validate` lo acepta).
11. `artifacts[].sha256` coincide con el archivo en disco; orden por `path`.
12. `explain` cubre todo número de `data` que provenga de un cálculo (o lo marca `"trace":"unavailable"` con `reason`).

---

## 8. Afordances para IA

### 8.1 Descubrimiento sin documentación

* `capabilities --brief` (≈900 tokens, 127 comandos) → rutas, `mutates`, `stability`.
* `schema <cmd> --brief` (≈200–400 tokens) → flags y tipos; `schema <cmd>` completo (≈1–3k) → JSON Schema + ejemplos + códigos.
* `errors` → catálogo de códigos, exits y hints (el agente puede cachearlo).
* `units` → magnitudes y decimales.
* `vocabularies` dentro de `capabilities`: `pipe.material`, `valve.type`, `manhole.material`, `check.pack`. Evita el error clásico de inventar un material: `E_ARG_BAD_VALUE` devuelve `accepted`.
* `--help --json` es alias de `schema <cmd>`.

Presupuesto de tokens recomendado (medido sobre el registro, no estimado a ojo):

| Llamada | Tokens aprox. | Cuándo |
|---|---|---|
| `capabilities --brief --compact` | 900 | siempre, al inicio |
| `schema <cmd> --brief` | 250 | antes de cada comando nuevo |
| `schema <cmd>` | 1 800 | cuando `--brief` no basta |
| `capabilities` completo | 14 000 | sólo auditoría |
| sobre de `manhole.create` | 450 | respuesta típica |
| sobre de `network validate` (24 objetos, 2 errores) | 1 200 | — |

### 8.2 `--explain`: la traza de todo número

`--explain` (o `--explain=full`) añade el bloque `explain` y un `trace` en cada magnitud calculada:

```json
"data": { "links": [ { "id": "0193f2d1-1b2c…", "flow": { "value": 30.9, "unit": "L/s", "trace": "n3" } } ] },
"explain": {
  "trace_id": "trc_01JCV9A7M2",
  "subject": { "kind": "pipe", "id": "0193f2d1-1b2c…" },
  "method": { "id": "manning.partial_flow", "version": "1" },
  "root": "n3",
  "nodes": [
    { "id": "n1", "label": "Área mojada (sección parcial)",
      "formula": "A = (D²/8)·(θ − sin θ)",
      "inputs": [ { "name": "D", "value": 0.25, "unit": "m" },
                  { "name": "θ", "value": 4.052, "unit": "rad" },
                  { "name": "y/D", "value": 0.70, "unit": "1" } ],
      "value": 0.0356, "unit": "m2" },
    { "id": "n2", "label": "Radio hidráulico",
      "formula": "R = A/P",
      "inputs": [ { "name": "A", "ref": "n1" },
                  { "name": "P", "value": 0.5065, "unit": "m" } ],
      "value": 0.0731, "unit": "m" },
    { "id": "n3", "label": "Caudal capacidad (Manning)",
      "formula": "Q = (1/n)·A·R^(2/3)·S^(1/2)",
      "inputs": [ { "name": "n", "value": 0.013, "unit": "1" },
                  { "name": "A", "ref": "n1" },
                  { "name": "R", "ref": "n2" },
                  { "name": "S", "value": 0.004167, "unit": "m/m" } ],
      "value": 0.0309, "unit": "m3/s",
      "result_ref": "data.links[0].flow" }
  ],
  "assumptions": [ { "key": "regimen", "value": "uniforme" },
                   { "key": "perdidas_locales", "value": "despreciadas" } ],
  "constants": [ { "key": "g", "value": 9.80665, "unit": "m/s2" } ],
  "tolerances": [ { "key": "convergencia", "value": 1e-6, "unit": "m3/s" } ],
  "code_refs": [ { "standard": "NEC-11", "clause": "9.3.2", "note": "velocidad mínima" } ]
}
```

Reglas:

* `nodes` es un DAG; un `input` referencia otro nodo con `ref`, nunca repite su valor (una sola fuente de verdad).
* `id` de nodo es estable para una misma ejecución: `n1..nN` en orden topológico; los tests pueden citarlo.
* `--explain=summary` emite un nodo por magnitud de salida (sin intermedios); `full` emite el DAG completo más las tablas de estaciones.
* `--explain-depth <n>` y `--explain-max-bytes <n>` acotan tamaño; al truncar, `explain.truncated:true` y `explain.omitted_nodes:n`.
* `formula` es texto plano en notación matemática simple (no LaTeX) para que un LLM lo lea sin parsear.
* `code_refs` enlaza el criterio con su norma; si el proyecto no tiene paquete de códigos, va `[]` y `W_CRS_ASSUMED`-equivalente en `diagnostics`.

### 8.3 `--dry-run`

| Clase de comando | Efecto |
|---|---|
| lectura | no-op; `data.plan = null`, exit 0 |
| escritura | calcula IDs, valida topología/criterios, devuelve `data.plan` + `data.diff` + `confirm_token`; **no** abre transacción de escritura |
| destructivo | obligatorio antes de ejecutar; el token resultante es el único camino a la ejecución |
| `plan run` | ejecuta `plan validate` completo: resuelve refs, valida tipos, no aplica |

```json
"data": {
  "plan": {
    "would_change": 1,
    "steps": [ { "op": "manhole.create", "id": "0193f2c1-7a4e…", "target": { "name": "MH-01" } } ],
    "diff": { "added": ["0193f2c1-7a4e…"], "modified": [], "removed": [] },
    "confirm_token": "cfm_01JCV9B…"
  }
}
```

### 8.4 Identidad, orden y revisión

* `name` (= `codigo` del dominio) es la clave natural y la referencia que usa el agente: `--from PZ-001 --to PZ-002` es estable y legible; el UUID queda para enlazar resultados entre revisiones.
* Con `--deterministic` el UUID v7 se deriva del código (§4.3), así que el agente puede predecir el `id` antes de crear el objeto y usarlo en el mismo `plan`.
* Repetir `create` con la misma clave natural no duplica (CLI-04): `--on-duplicate error|reuse|update`, default `error` (`E_STATE_DUPLICATE_NAME`); `reuse` devuelve el objeto existente con `meta.replayed:true` y revisión intacta.
* Revisión monótona `+1` por transacción aplicada (un `plan` atómico = una revisión).
* `--expect-revision N` en toda escritura: si el proyecto avanzó, `E_PROJ_VERSION_CONFLICT` con `details:{expected, actual}`.
* `--sort` sólo sobre whitelist por familia (`name`, `id`, `depth`, `length`, `diameter`, `flow`, `created_rev`); fuera de ella → `E_ARG_UNKNOWN_FIELD` con `accepted`.

### 8.5 Idempotencia

Dos niveles, en este orden:

1. **Clave natural (implícita, CLI-04).** Toda creación es idempotente por su `name`: repetir `manhole create --name PZ-001` no crea un segundo pozo. Política en `--on-duplicate`: `error` (default, `E_STATE_DUPLICATE_NAME`), `reuse` (devuelve el existente con `meta.replayed:true`) o `update`.
2. **Clave de idempotencia (explícita).** Para comandos sin clave natural (cálculos, exportaciones, `bulk`, `plan run`): `--idempotency-key <k>`, guardada como `(project_id, command, key)` con hash canónico de la entrada + sobre resultante en `.redes/idempotency/` (TTL 7 días, purgado por `project gc`). Misma entrada ⇒ sobre original + `meta.replayed:true`, `duration_ms:0`, revisión intacta. Entrada distinta con la misma clave ⇒ `E_STATE_IDEMPOTENCY_MISMATCH` (exit 6) con `details.input_hash`.

* `plan run` acepta la clave a nivel de plan: o se aplica entero, o no se aplica.

### 8.6 `--out` y artefactos

| Regla | Detalle |
|---|---|
| `--out -` | escribe el artefacto crudo a stdout; sólo si hay exactamente un artefacto, si no `E_ARG_CONFLICT` |
| default | `<proyecto>/out/<comando>-<rev>.<ext>` (p. ej. `out/render.plan-41.png`) |
| colisión | falla con `E_IO_EXISTS` salvo `--overwrite` |
| directorios | no se crean sin `--mkdir` |
| eco | siempre ruta absoluta en `path` + `file_url` (`file:///C:/…`) para consumidores no-Windows |
| integridad | `sha256` calculado antes de reportar |
| atomicidad | escribe a `.tmp` y renombra al confirmar; un fallo no deja artefactos parciales |

```json
"artifacts": [
  { "path": "C:\\obra\\out\\planta-41.png",
    "file_url": "file:///C:/obra/out/planta-41.png",
    "media_type": "image/png", "bytes": 1243118,
    "sha256": "9f2c…", "kind": "render.plan" }
]
```

### 8.7 Paginación y selección de campos

* Cursor opaco: `base64({"rev":41,"after":"0193f2c1-7a4e…","filter_hash":"a91…"})`. Ligado a la revisión: si el proyecto muta, `E_LIMIT_CURSOR_STALE` (exit 8) con `hint_command` para reiniciar.
* `--limit` default 100, máximo 1000. `--all` sigue cursores y emite NDJSON (no acumula en memoria).
* `--fields id,name,invert_in` proyecta y **ordena** las claves en el orden pedido; aplica a `data.object` y `data.items[]`, nunca al sobre.
* `total` se calcula siempre con `COUNT(*)` sobre el mismo filtro (barato en SQLite) para que el agente sepa cuánto falta.

### 8.8 Streaming de trabajos largos

`--stream` cambia stdout a NDJSON. Vocabulario de eventos **compartido con MCP** (`notifications/progress`):

```
{"event":"start","command":"hydraulics.solve","project_revision":41,"started_at":"…"}
{"event":"stage","stage":"topology","pct":5}
{"event":"stage","stage":"assemble","pct":20,"links":184,"nodes":191}
{"event":"progress","stage":"solve","pct":65,"iterations":37,"residual":4.2e-5}
{"event":"warn","code":"W_HYD_MINOR_LOSSES_IGNORED","message":"…"}
{"event":"result","ok":true,"data_ref":"solve","envelope":{ "…": "sobre completo" }}
```

* El **último** evento es siempre `result` con el sobre completo: un parser de una pasada obtiene todo.
* `--progress text` (default en TTY) escribe la barra en stderr; `--no-progress` la apaga.
* Cancelación: `SIGINT` cierra el NDJSON con `{"event":"canceled"}` y exit 130; el proyecto queda en la revisión anterior (rollback de la transacción).

### 8.9 Modos de silencio

| Flag | Efecto |
|---|---|
| `--quiet` | stdout sólo el sobre; stderr silenciado salvo `error` |
| `--no-color` / `NO_COLOR=1` | sin ANSI (siempre en no-TTY) |
| `--no-progress` | sin barra ni eventos de progreso |
| `--log <nivel>` | stderr estructurado NDJSON cuando `--format json` |
| `--echo-input` | `meta.input.{argv,resolved}` — imprescindible para depurar coordenadas mal parseadas |

### 8.10 Modo lote: `plan`

Un solo documento JSON, ejecutable por un agente, con referencias entre pasos y atomicidad.

```json
{
  "schema": "redes.plan/1",
  "project": "./obra",
  "atomic": true,
  "on_error": "abort",
  "steps": [
    { "id": "tin", "command": "terrain.import",
      "args": { "file": "levantamiento.xyz", "format": "xyz", "crs": "EPSG:32717", "name": "TIN-BASE" } },
    { "id": "mh1", "command": "manhole.create",
      "args": { "name": "MH-01", "code": "PZ-01", "at": [512345.678, 9876543.210], "depth": 3.20 } },
    { "id": "mh2", "command": "manhole.create",
      "args": { "name": "MH-02", "code": "PZ-02", "at": [512465.100, 9876570.900], "depth": 3.48 } },
    { "id": "t1", "command": "pipe.create",
      "args": { "name": "T-01", "from": "$mh1.data.object.id", "to": "$mh2.data.object.id",
                "diameter": 250, "material": "PVC", "invert_from": 2448.120, "invert_to": 2447.620 } },
    { "id": "val", "command": "network.validate", "args": { "fail_on": "error" } }
  ]
}
```

* Referencias: `$<step>.data.<ruta>` (JSONPath-lite, `.` y `[n]`); `$$` escapa un literal.
* `atomic:true` (default): todo el plan es **una** transacción SQLite + artefactos con rename diferido. Fallo en el paso *k* ⇒ rollback total, revisión sin cambios, `data.rolled_back:true`.
* `on_error`: `abort` (default) | `continue` (no atómico; reporta `failed[]`).
* `plan validate` resuelve referencias **simbólicamente** contra los esquemas de salida: detecta un `$mh1.data.object.id` que no existe sin ejecutar nada.
* Ejecución estrictamente secuencial; `"parallel": ["stepA","stepB"]` sólo se permite para pasos de lectura declarados como puros.
* Un plan puede contener otro plan (`command: "plan.run"`, un nivel de anidación) para reutilizar tareas.

`redes.toml` reutiliza el mismo motor:

```toml
[project]
path = "./obra/project.redes"
units = "si"

[tasks.entrega]
steps = [
  "network.validate --fail-on error",
  "hydraulics.solve --storm 10y --duration 15min",
  "earthwork.compute --stations 1.0",
  "profile.export --path MH-01,MH-05 --out perfil.csv --format csv",
  "export.landxml --out red.xml",
  "render.plan --bbox 512300,9876400,512600,9876700 --out planta.png",
]
```

`redes task run entrega` ejecuta el mismo `plan` (misma atomicidad, mismos eventos, mismo sobre).

### 8.11 Confirmación en dos fases (destructivos)

```
$ redes manhole delete --name MH-07 --dry-run --json
→ ok:false, exit 3, error.code:"E_STATE_REFERENCED",
  details.references:["0193f2d2-2c3d…","0193f2d3-3d4e-76f7-a051-2c3d4e5f6a07"]        ← no se puede: hay tuberías

$ redes manhole delete --name MH-07 --cascade --dry-run --json
→ ok:true, data.plan.would_change:3, data.plan.confirm_token:"cfm_01JCV9B…"

$ redes manhole delete --name MH-07 --cascade --confirm-token cfm_01JCV9B… --json
→ ok:true, data.deleted:[{id:"0193f2c3-9c32…",name:"MH-07"}, …], revision:7
```

Si el token no cubre exactamente los objetos y la revisión actuales: `E_CONFIRM_REQUIRED` (exit 9), y la operación no se aplica. El token liga `(ids exactos, revisión, comando, expiración 120 s)`: evita el borrado en cascada de un objeto que cambió entre la inspección y la acción.

---

## 9. Ejemplos

> Convención: los bloques están **truncados**; `…` marca elisión y no forma parte del JSON. Las claves mostradas y sus tipos son exactos. Los bloques `units` y `meta` se omiten cuando son idénticos al de E1. Los IDs se muestran completos o truncados con `…`; su longitud canónica es 26 caracteres tras el prefijo (§4.3).

### E1 — Descubrimiento (sin abrir proyecto)

```bash
$ redes capabilities --brief --compact
```

```json
{
  "schema": "redes.envelope/1", "ok": true, "command": "capabilities",
  "request_id": "req_01JCV8Z3Q5K7", "project": null,
  "data": {
    "cli_version": "0.1.0", "engine_version": "0.1.0",
    "envelope_schema": "redes.envelope/1", "mcp_server_version": "0.1.0",
    "profiles": ["core", "full", "read-only"],
    "commands": [
      { "path": "project.info",      "summary": "Estado del proyecto",        "mutates": false, "stability": "stable", "since": "0.1.0" },
      { "path": "terrain.import",    "summary": "Importa un TIN",             "mutates": true,  "stability": "stable", "since": "0.1.0" },
      { "path": "manhole.create",    "summary": "Crea un pozo",               "mutates": true,  "stability": "stable", "since": "0.1.0" },
      { "path": "network.validate",  "summary": "Valida la red",              "mutates": false, "stability": "stable", "since": "0.1.0" },
      { "path": "hydraulics.solve",  "summary": "Resuelve la hidráulica",     "mutates": false, "stability": "stable", "since": "0.1.0" },
      { "path": "earthwork.compute","summary": "Volúmenes de excavación",     "mutates": false, "stability": "stable", "since": "0.1.0" },
      { "path": "query.objects",     "summary": "Consulta transversal",        "mutates": false, "stability": "beta",   "since": "0.2.0" },
      "…"
    ],
    "count": 127,
    "error_codes": [
      { "code": "E_TOPO_DISCONNECTED", "exit": 3, "severity": "error", "retryable": false, "recoverable": true }
    ]
  },
  "artifacts": [], "diagnostics": [], "warnings": [],
  "explain": null, "pagination": null,
  "units": { "system": "si", "length": "m", "volume": "m3", "flow": "L/s",
             "pressure": "mca", "decimals": { "length": 3, "volume": 3, "flow": 3, "velocity": 2 } },
  "meta": { "cli_version": "0.1.0", "engine_version": "0.1.0", "envelope_schema": "redes.envelope/1",
            "exit_code": 0, "deterministic": true, "replayed": false, "duration_ms": 6,
            "started_at": "2026-11-18T13:55:27Z", "host": "windows", "locale": "es-EC" },
  "error": null
}
```

### E2 — Esquema de un comando

```bash
$ redes schema pipe.create --brief --json
```

```json
{
  "schema": "redes.envelope/1", "ok": true, "command": "schema",
  "data": {
    "command": "pipe.create", "stability": "stable", "since": "0.1.0",
    "mutates": true, "destructive": false, "idempotent": true, "dry_run": true,
    "input_schema": {
      "$id": "redes://schema/pipe.create/input",
      "type": "object",
      "required": ["name", "from", "to", "diameter"],
      "properties": {
        "name":        { "type": "string", "minLength": 1 },
        "from":        { "type": "string", "description": "ID o nombre de nodo aguas arriba" },
        "to":          { "type": "string" },
        "diameter":    { "type": "integer", "minimum": 50, "maximum": 3000, "unit": "mm" },
        "material":    { "type": "string", "enum": ["PVC", "HDPE", "HORMIGON", "HIERRO_FUNDIDO", "ACERO"] },
        "invert_from": { "type": "number", "unit": "m" },
        "invert_to":   { "type": "number", "unit": "m" },
        "slope":       { "type": "number", "unit": "m/m" },
        "drop":        { "type": "number", "unit": "m" },
        "roughness":   { "type": "object", "properties": { "model": { "enum": ["manning", "hazen"] }, "n": { "type": "number" } } }
      },
      "oneOf": [ { "required": ["invert_to"] }, { "required": ["slope"] }, { "required": ["drop"] } ]
    },
    "output_schema": { "$ref": "redes://schema/envelope#/definitions/object_envelope" },
    "error_codes": ["E_ARG_CONFLICT", "E_GEO_SLOPE_MISMATCH", "E_STATE_DUPLICATE_NAME", "E_TOPO_SELF_LOOP"],
    "examples": [ "redes pipe create --name T-01 --from MH-01 --to MH-02 --diameter 250 --invert-from 2448.120 --invert-to 2447.620" ]
  },
  "artifacts": [], "diagnostics": [], "warnings": [], "explain": null, "pagination": null,
  "error": null
}
```

### E3 — Crear proyecto

```bash
$ redes project init --dir ./obra --name "Alcantarillado La Esperanza" \
    --crs EPSG:32717 --units si --codes NEC-11 --json
```

```json
{
  "schema": "redes.envelope/1", "ok": true, "command": "project.init",
  "request_id": "req_01JCV90001AA",
  "project": { "id": "0193f2f1-5f60-7719-c273-4e5f6a7b8c09", "path": "C:\\obra\\project.redes",
               "name": "Alcantarillado La Esperanza", "crs": "EPSG:32717", "units": "si", "revision": 0 },
  "data": {
    "project": { "id": "0193f2f1-5f60-7719-c273-4e5f6a7b8c09", "name": "Alcantarillado La Esperanza",
                 "crs": "EPSG:32717", "units": "si", "codes": ["NEC-11"],
                 "schema_version": 1, "revision": 0, "created_at": "2026-11-18T13:56:02Z" },
    "path": "C:\\obra\\project.redes"
  },
  "artifacts": [], "diagnostics": [], "warnings": [], "explain": null, "pagination": null,
  "units": { "system": "si", "length": "m", "volume": "m3", "flow": "L/s", "pressure": "mca" },
  "meta": { "cli_version": "0.1.0", "engine_version": "0.1.0", "envelope_schema": "redes.envelope/1",
            "exit_code": 0, "deterministic": true, "replayed": false, "duration_ms": 41,
            "started_at": "2026-11-18T13:56:02Z", "host": "windows", "locale": "es-EC" },
  "error": null
}
```

### E4 — Importar el levantamiento

```bash
$ redes terrain import --file levantamiento.xyz --format xyz \
    --crs EPSG:32717 --name TIN-BASE --json
```

```json
{
  "schema": "redes.envelope/1", "ok": true, "command": "terrain.import",
  "project": { "id": "0193f2f1-5f60…", "revision": 1, "crs": "EPSG:32717", "units": "si" },
  "data": { "tin": { "id": "0193f2e1-4e5f-7708-b162-3d4e5f6a7b08",
                     "name": "TIN-BASE", "points": 48213, "triangles": 96380,
                     "bbox": { "min": { "x": 512300.114, "y": 9876401.882, "z": 2447.912 },
                               "max": { "x": 512702.553, "y": 9876812.447, "z": 2461.308 } },
                     "z_range": { "value": 13.396, "unit": "m" },
                     "area": { "value": 162480.4, "unit": "m2" },
                     "sha256": "4c1a…" } },
  "artifacts": [], "diagnostics": [],
  "warnings": [ { "code": "W_TIN_LOW_DENSITY", "severity": "warning",
                  "message": "Densidad 0.30 pts/m2; se recomienda ≥ 1.0 para excavación.",
                  "object": { "family": "tin", "id": "0193f2e1-4e5f…", "name": "TIN-BASE" },
                  "hint": "Importe el levantamiento completo o use --decimate 0.5." } ],
  "explain": null, "pagination": null,
  "error": null
}
```

### E5 — Pozo con coordenadas reales (cota desde el TIN)

```bash
$ redes manhole create --name MH-01 --code PZ-01 \
    --at 512345.678,9876543.210 --depth 3.20 --inside-diameter 1.20 \
    --material HORMIGON --idempotency-key alta-mh01 --json
```

```json
{
  "schema": "redes.envelope/1", "ok": true, "command": "manhole.create",
  "project": { "id": "0193f2f1-5f60…", "revision": 2 },
  "data": {
    "object": {
      "id": "0193f2c1-7a4e-7c3b-9e21-5b8a4c6d1f03", "name": "MH-01", "code": "PZ-01",
      "family": "manhole",
      "geometry": { "x": 512345.678, "y": 9876543.210, "z": 2451.320,
                    "crs": "EPSG:32717", "z_source": "tin", "tin_id": "0193f2e1-4e5f…" },
      "rim":        { "value": 2451.320, "unit": "m" },
      "invert_in":  { "value": 2448.120, "unit": "m" },
      "invert_out": { "value": 2448.120, "unit": "m" },
      "depth":      { "value": 3.200, "unit": "m" },
      "inside_diameter": { "value": 1.200, "unit": "m" },
      "material": "HORMIGON", "cover": "LOSA", "access": "TRANSITABLE",
      "tags": [], "rev": 1, "born_rev": 2, "checksum": "blake3:9a3f…"
    },
    "changes": [],
    "placement": { "tin_id": "0193f2e1-4e5f…", "z_source": "tin", "triangle_id": 41207 }
  },
  "artifacts": [], "diagnostics": [],
  "warnings": [],
  "explain": null, "pagination": null,
  "meta": { "exit_code": 0, "deterministic": true, "replayed": false, "duration_ms": 9, "…": "…" },
  "error": null
}
```

### E6 — Tubería con inverts y pendiente derivada

```bash
$ redes manhole create --name MH-02 --code PZ-02 --at 512465.100,9876570.900 --depth 3.48 --json
$ redes pipe create --name T-01 --from MH-01 --to MH-02 --diameter 250 \
    --material PVC --invert-from 2448.120 --invert-to 2447.620 --json
```

```json
{
  "schema": "redes.envelope/1", "ok": true, "command": "pipe.create",
  "project": { "id": "0193f2f1-5f60…", "revision": 4 },
  "data": {
    "object": {
      "id": "0193f2d1-1b2c-74d5-8e3f-0a1b2c3d4e05", "name": "T-01", "family": "pipe",
      "from": "0193f2c1-7a4e-7c3b-9e21-5b8a4c6d1f03",
      "to":   "0193f2c2-8b21-7d44-a0f7-1c2d3e4f5a06",
      "length":     { "value": 120.000, "unit": "m" },
      "diameter":   { "value": 250, "unit": "mm" },
      "material": "PVC", "shape": "circular",
      "roughness":  { "model": "manning", "n": 0.013 },
      "invert_from": { "value": 2448.120, "unit": "m" },
      "invert_to":   { "value": 2447.620, "unit": "m" },
      "slope":      { "value": -0.004167, "unit": "m/m" },
      "grade_pct":  { "value": -0.4167, "unit": "%" },
      "cover_up":   { "value": 2.950, "unit": "m" },
      "cover_down": { "value": 2.988, "unit": "m" },
      "rev": 1, "born_rev": 4, "checksum": "blake3:1b7e…"
    },
    "changes": [],
    "placement": null
  },
  "diagnostics": [], "warnings": [],
  "error": null
}
```

### E7 — Validar la red: falla, pero el comando se ejecutó

```bash
$ redes network validate --rules all --fail-on error --json
$ echo $?
```

```json
{
  "schema": "redes.envelope/1", "ok": false, "command": "network.validate",
  "project": { "id": "0193f2f1-5f60…", "revision": 4 },
  "data": { "verdict": "fail",
            "checked": { "rules": ["topology", "cover", "slope", "geometry", "attach"], "objects": 47 },
            "counts": { "error": 2, "warning": 1, "info": 0 } },
  "artifacts": [],
  "diagnostics": [
    { "code": "E_TOPO_DISCONNECTED", "severity": "error",
      "message": "El pozo MH-07 no tiene ninguna tubería conectada.",
      "object": { "family": "manhole", "id": "0193f2c3-9c32-7e55-b1a8-2d3e4f5a6b07", "name": "MH-07" },
      "path": "network.nodes['0193f2c3-9c32…'].links", "field": "--to",
      "details": { "degree": 0, "component": 3 },
      "hint": "Conéctelo al pozo aguas arriba más cercano o elimínelo.",
      "hint_command": "redes pipe create --from MH-06 --to MH-07 --diameter 200 --material PVC --json",
      "docs": "https://docs.redes.dev/errors#E_TOPO_DISCONNECTED",
      "retryable": false, "recoverable": true },
    { "code": "E_HYD_NEGATIVE_SLOPE", "severity": "error",
      "message": "La tubería T-04 tiene pendiente positiva (0.0012 m/m).",
      "object": { "family": "pipe", "id": "0193f2d2-2c3d-75e6-9f40-1b2c3d4e5f06", "name": "T-04" },
      "path": "network.links['0193f2d2-2c3d…'].slope",
      "details": { "slope": 0.0012 },
      "hint": "Corrija los inverts para que el flujo baje.",
      "hint_command": "redes pipe set-inverts --name T-04 --invert-to 2447.100 --json",
      "retryable": false, "recoverable": true },
    { "code": "W_HYD_MINOR_LOSSES_IGNORED", "severity": "warning",
      "message": "Pérdidas locales despreciadas en el cálculo de capacidad.",
      "object": null, "path": null,
      "hint": "Incluya pérdidas locales si el tramo tiene codos, tees o cambios de diámetro.",
      "retryable": false, "recoverable": false }
  ],
  "warnings": [ { "code": "W_HYD_MINOR_LOSSES_IGNORED", "severity": "warning", "…": "…" } ],
  "explain": null, "pagination": null,
  "meta": { "exit_code": 3, "…": "…" },
  "error": {
    "code": "E_CHECK_FAILED", "severity": "error",
    "message": "La validación encontró 2 violaciones bloqueantes.",
    "details": { "blocking_codes": ["E_TOPO_DISCONNECTED", "E_HYD_NEGATIVE_SLOPE"] },
    "hint": "Corrija los diagnósticos con severity=error y repita la validación.",
    "hint_command": "redes network validate --rules all --fail-on error --json",
    "retryable": false, "recoverable": true
  }
}
```

`$?` → `3`. El comando **sí** se ejecutó (`data.verdict` existe); `ok:false` significa "no cumple el umbral pedido". Un agente debe leer `diagnostics`, no el exit code, para saber *qué* corregir.

### E8 — Resolver hidráulica con traza

```bash
$ redes hydraulics solve --storm 10y --duration 15min --explain summary --json
```

```json
{
  "schema": "redes.envelope/1", "ok": true, "command": "hydraulics.solve",
  "project": { "id": "0193f2f1-5f60…", "revision": 4 },
  "data": {
    "scenario": { "id": "sc_10y_15min", "storm": { "return_period": 10, "duration_min": 15 },
                  "method": "manning", "inflow": { "value": 31.2, "unit": "L/s" } },
    "convergence": { "iterations": 23, "residual": 4.2e-9, "tolerance": 1e-6, "converged": true },
    "nodes": [
      { "id": "0193f2c1-7a4e…", "name": "MH-01",
        "hgl": { "value": 2448.352, "unit": "m", "trace": "n4" },
        "pressure": { "value": 0.412, "unit": "mca" },
        "surcharged": false }
    ],
    "links": [
      { "id": "0193f2d1-1b2c…", "name": "T-01",
        "flow":      { "value": 30.9, "unit": "L/s", "trace": "n3" },
        "velocity":  { "value": 0.87, "unit": "m/s", "trace": "n5" },
        "fill":      { "value": 0.70, "unit": "1" },
        "headloss":  { "value": 0.412, "unit": "m", "trace": "n6" },
        "regime": "uniforme" }
    ],
    "totals": { "inflow": { "value": 31.2, "unit": "L/s" },
                "outflow": { "value": 31.2, "unit": "L/s" },
                "storage": { "value": 0.0, "unit": "m3" } }
  },
  "diagnostics": [], "warnings": [],
  "explain": {
    "trace_id": "trc_01JCV9A7M2",
    "subject": { "kind": "pipe", "id": "0193f2d1-1b2c…" },
    "method": { "id": "manning.partial_flow", "version": "1" },
    "root": "n3",
    "nodes": [
      { "id": "n1", "label": "Área mojada (sección parcial)", "formula": "A = (D²/8)·(θ − sin θ)",
        "inputs": [ { "name": "D", "value": 0.25, "unit": "m" },
                    { "name": "θ", "value": 4.052, "unit": "rad" },
                    { "name": "y/D", "value": 0.70, "unit": "1" } ],
        "value": 0.0356, "unit": "m2" },
      { "id": "n2", "label": "Radio hidráulico", "formula": "R = A/P",
        "inputs": [ { "name": "A", "ref": "n1" }, { "name": "P", "value": 0.5065, "unit": "m" } ],
        "value": 0.0731, "unit": "m" },
      { "id": "n3", "label": "Caudal (Manning)", "formula": "Q = (1/n)·A·R^(2/3)·S^(1/2)",
        "inputs": [ { "name": "n", "value": 0.013, "unit": "1" }, { "name": "A", "ref": "n1" },
                    { "name": "R", "ref": "n2" }, { "name": "S", "value": 0.004167, "unit": "m/m" } ],
        "value": 0.0309, "unit": "m3/s", "result_ref": "data.links[0].flow" }
    ],
    "assumptions": [ { "key": "regimen", "value": "uniforme" } ],
    "constants": [ { "key": "g", "value": 9.80665, "unit": "m/s2" } ],
    "tolerances": [ { "key": "convergencia", "value": 1e-6, "unit": "m3/s" } ],
    "code_refs": [ { "standard": "NEC-11", "clause": "9.3.2" } ],
    "truncated": true, "omitted_nodes": 3
  },
  "pagination": null,
  "meta": { "exit_code": 0, "duration_ms": 384, "deterministic": true, "…": "…" }
}
```

### E9 — Contraste de criterios (hallazgo bloqueante)

```bash
$ redes hydraulics check --checks pressure,velocity,fill --fail-on error --json
```

```json
{
  "schema": "redes.envelope/1", "ok": false, "command": "hydraulics.check",
  "data": { "verdict": "fail",
            "checked": { "rules": ["pressure", "velocity", "fill"], "objects": 47 },
            "counts": { "error": 1, "warning": 1, "info": 0 } },
  "diagnostics": [
    { "code": "E_HYD_PRESSURE_LOW", "severity": "error",
      "message": "Presión insuficiente en MH-12: 8.4 mca < 10.0 mca.",
      "object": { "family": "manhole", "id": "0193f2c4-ad43-7f66-c2b9-3e4f5a6b7c08", "name": "MH-12" },
      "path": "results.nodes['0193f2c4-ad43-7f66-c2b9-3e4f5a6b7c08'].pressure",
      "expected": "≥ 10.0 mca", "details": { "value": 8.4, "min": 10.0, "deficit": 1.6 },
      "hint": "Suba el nivel del tanque o reduzca la pérdida en T-11.",
      "hint_command": "redes hydraulics design --min-pressure 10.0 --dry-run --json",
      "code_ref": { "standard": "NEC-11", "clause": "9.4.1" },
      "retryable": false, "recoverable": true },
    { "code": "W_HYD_MINOR_LOSSES_IGNORED", "severity": "warning", "…": "…" }
  ],
  "warnings": [ { "code": "W_HYD_MINOR_LOSSES_IGNORED", "severity": "warning", "…": "…" } ],
  "error": { "code": "E_CHECK_FAILED", "severity": "error",
             "details": { "blocking_codes": ["E_HYD_PRESSURE_LOW"] }, "…": "…" },
  "meta": { "exit_code": 3, "…": "…" }
}
```

### E10 — Excavación con traza completa

```bash
$ redes earthwork compute --selection all --stations 1.0 --bedding 0.15 \
    --trench-extra 0.30 --topsoil 0.20 --explain full --json
```

```json
{
  "schema": "redes.envelope/1", "ok": true, "command": "earthwork.compute",
  "data": {
    "quantities": [
      { "item": "excavacion_zanja",        "volume": { "value": 300.90, "unit": "m3" } },
      { "item": "excavacion_estructuras",  "volume": { "value": 7.24,   "unit": "m3" } },
      { "item": "cama_arena",              "volume": { "value": 15.30,  "unit": "m3" } },
      { "item": "relleno_compactado",      "volume": { "value": 279.71, "unit": "m3" } },
      { "item": "desbroce_superficial",    "volume": { "value": 20.85,  "unit": "m3" } },
      { "item": "excedente_transporte",    "volume": { "value": 13.13,  "unit": "m3" } }
    ],
    "by_pipe": [
      { "id": "0193f2d1-1b2c…", "name": "T-01",
        "length": { "value": 120.000, "unit": "m" },
        "trench_width": { "value": 0.850, "unit": "m" },
        "depth_mean": { "value": 2.950, "unit": "m" },
        "volume": { "value": 300.90, "unit": "m3", "trace": "n4" } }
    ],
    "by_structure": [
      { "id": "0193f2c1-7a4e…", "name": "MH-01", "volume": { "value": 3.62, "unit": "m3", "trace": "n6" } }
    ],
    "totals": { "cut": { "value": 308.14, "unit": "m3" },
                "fill": { "value": 295.01, "unit": "m3" },
                "net_export": { "value": 13.13, "unit": "m3" } },
    "assumptions": { "swell_factor": 1.25, "compaction_factor": 0.95, "side_slope": "1:0.25" }
  },
  "explain": {
    "trace_id": "trc_01JCVB4K7P", "subject": { "kind": "earthwork", "id": "0193f2d1-1b2c…" },
    "method": { "id": "prismoidal.stations", "version": "1" }, "root": "n4",
    "nodes": [
      { "id": "n1", "label": "Ancho de zanja", "formula": "b = D_ext + 2·(extra)",
        "inputs": [ { "name": "D_ext", "value": 0.25, "unit": "m" },
                    { "name": "extra", "value": 0.30, "unit": "m" } ],
        "value": 0.85, "unit": "m" },
      { "id": "n2", "label": "Profundidad media", "formula": "h̄ = (h₁ + h₂)/2",
        "inputs": [ { "name": "h₁", "value": 3.20, "unit": "m" },
                    { "name": "h₂", "value": 2.70, "unit": "m" } ],
        "value": 2.95, "unit": "m" },
      { "id": "n3", "label": "Longitud", "value": 120.0, "unit": "m",
        "formula": "L = Σ |Pᵢ₊₁ − Pᵢ|" },
      { "id": "n4", "label": "Volumen de zanja", "formula": "V = b·h̄·L",
        "inputs": [ { "name": "b", "ref": "n1" }, { "name": "h̄", "ref": "n2" },
                    { "name": "L", "ref": "n3" } ],
        "value": 300.90, "unit": "m3", "result_ref": "data.by_pipe[0].volume" },
      { "id": "n5", "label": "Volumen de cama de arena", "formula": "V = b·e·L",
        "inputs": [ { "name": "b", "ref": "n1" }, { "name": "e", "value": 0.15, "unit": "m" },
                    { "name": "L", "ref": "n3" } ],
        "value": 15.30, "unit": "m3", "result_ref": "data.quantities[2].volume" },
      { "id": "n6", "label": "Volumen de excavación de estructura", "formula": "V = (π/4)·D²·h",
        "inputs": [ { "name": "D", "value": 1.20, "unit": "m" },
                    { "name": "h", "value": 3.20, "unit": "m" } ],
        "value": 3.62, "unit": "m3", "result_ref": "data.by_structure[0].volume" }
    ],
    "assumptions": [ { "key": "seccion", "value": "prismoidal, estaciones cada 1.0 m" },
                     { "key": "esponjamiento", "value": 1.25 } ],
    "truncated": false
  },
  "artifacts": [], "diagnostics": [], "warnings": [],
  "error": null
}
```

### E11 — Perfil longitudinal a archivo

```bash
$ redes profile compute --path MH-01,MH-02,MH-03,MH-04,MH-05 \
    --stations 20 --exaggeration 10 --out perfil.csv --json
```

```json
{
  "schema": "redes.envelope/1", "ok": true, "command": "profile.compute",
  "data": {
    "path": ["0193f2c1-7a4e…", "0193f2c2-8b21…", "0193f2c6-cf65-7178-e4db-5a6b7c8d9e0a", "0193f2c7-d076-7289-f5ec-6b7c8d9e0f1b", "0193f2c8-e187-739a-06fd-7c8d9e0f1a2c"],
    "length": { "value": 480.000, "unit": "m" },
    "exaggeration": 10,
    "stations": [
      { "station": { "value": 0.000, "unit": "m" },
        "ground_z": { "value": 2451.320, "unit": "m" },
        "invert_z": { "value": 2448.120, "unit": "m", "trace": "n2" },
        "depth":    { "value": 3.200, "unit": "m" },
        "pipe_id": null, "diameter": null, "slope": null },
      { "station": { "value": 20.000, "unit": "m" },
        "ground_z": { "value": 2451.104, "unit": "m" },
        "invert_z": { "value": 2448.037, "unit": "m" },
        "depth":    { "value": 3.067, "unit": "m" },
        "pipe_id": "0193f2d1-1b2c…", "diameter": { "value": 250, "unit": "mm" },
        "slope": { "value": -0.004167, "unit": "m/m" } }
    ]
  },
  "artifacts": [
    { "path": "C:\\obra\\out\\perfil.csv", "file_url": "file:///C:/obra/out/perfil.csv",
      "media_type": "text/csv", "bytes": 4821, "sha256": "b7d1…", "kind": "profile.csv" }
  ],
  "diagnostics": [], "warnings": [], "explain": null,
  "error": null
}
```

### E12 — Render PNG de planta con línea base

```bash
$ redes render plan --bbox 512300,9876400,512600,9876700 --size 1600x1200 --dpi 96 \
    --layers manholes,pipes,labels,scale-bar --theme light \
    --baseline out/planta-base.png --out planta.png --json
```

```json
{
  "schema": "redes.envelope/1", "ok": true, "command": "render.plan",
  "data": {
    "image": { "width": 1600, "height": 1200, "dpi": 96, "theme": "light",
               "bbox": { "min": { "x": 512300.0, "y": 9876400.0 },
                         "max": { "x": 512600.0, "y": 9876700.0 } },
               "layers": ["manholes", "pipes", "labels", "scale-bar"],
               "scale": "1:1000", "device": "headless-gpu" },
    "diff": { "pixels_changed": 0, "ratio": 0.0, "diff_png": null }
  },
  "artifacts": [
    { "path": "C:\\obra\\out\\planta.png", "file_url": "file:///C:/obra/out/planta.png",
      "media_type": "image/png", "bytes": 1243118, "sha256": "9f2c…", "kind": "render.plan" }
  ],
  "diagnostics": [], "warnings": [],
  "meta": { "exit_code": 0, "duration_ms": 612, "deterministic": true,
            "fonts": "windfoil", "…": "…" },
  "error": null
}
```

### E13 — Exportar LandXML

```bash
$ redes export landxml --out red.xml --version 1.2 --units metric --json
```

```json
{
  "schema": "redes.envelope/1", "ok": true, "command": "export.landxml",
  "data": { "export": { "format": "landxml", "version": "1.2", "units": "metric",
                        "objects": { "manholes": 24, "pipes": 23, "outfalls": 1 },
                        "warnings_before_export": 0 } },
  "artifacts": [
    { "path": "C:\\obra\\out\\red.xml", "file_url": "file:///C:/obra/out/red.xml",
      "media_type": "application/xml", "bytes": 184203, "sha256": "e5a0…", "kind": "export.landxml" }
  ],
  "diagnostics": [], "warnings": [], "explain": null,
  "error": null
}
```

### E14 — Fallo de argumento con corrección incluida

```bash
$ redes pipe create --name T-02 --from MH-02 --to MH-03 --diameter 250 \
    --invert-from 2447.620 --invert-to 2447.120 --slope -0.0052 --json
$ echo $?
```

```json
{
  "schema": "redes.envelope/1", "ok": false, "command": "pipe.create",
  "data": null, "artifacts": [], "diagnostics": [], "warnings": [],
  "explain": null, "pagination": null,
  "meta": { "exit_code": 5, "…": "…" },
  "error": {
    "code": "E_GEO_SLOPE_MISMATCH", "severity": "error",
    "message": "La pendiente indicada no coincide con los inverts dados.",
    "field": "--slope",
    "expected": "−0.005000 m/m ± 1e-6 (derivada de los inverts)",
    "accepted": null,
    "details": { "given": -0.0052, "derived": -0.005, "delta": 2.0e-4, "tolerance": 1e-6 },
    "hint": "Omita --slope y déjelo derivar de --invert-to, o corrija el invert de llegada.",
    "hint_command": "redes pipe create --name T-02 --from MH-02 --to MH-03 --diameter 250 --invert-from 2447.620 --invert-to 2447.120 --json",
    "docs": "https://docs.redes.dev/errors#E_GEO_SLOPE_MISMATCH",
    "retryable": false, "recoverable": true
  }
}
```

`$?` → `5`. No hay `data.plan` porque el comando no llegó a planificar. `hint_command` es ejecutable tal cual y crea la tubería.

### E15 — Plan atómico con streaming y rollback

```bash
$ redes plan run obra.plan.json --atomic --stream --json
```

```json
{"event":"start","command":"plan.run","plan_id":"pln_01JCVB9","steps":5,"atomic":true,"revision_before":6}
{"event":"step.start","step":"tin","command":"terrain.import","index":0}
{"event":"step.result","step":"tin","ok":true,"data_ref":"tin","duration_ms":214}
{"event":"progress","stage":"apply","pct":40,"done":2,"total":5}
{"event":"step.start","step":"t1","command":"pipe.create","index":3}
{"event":"step.result","step":"t1","ok":false,"error_code":"E_STATE_DUPLICATE_NAME"}
{"event":"rollback","steps_reverted":3,"revision_restored":6}
{"event":"result","ok":false,"envelope":{
  "schema":"redes.envelope/1","ok":false,"command":"plan.run",
  "project":{"id":"0193f2f1-5f60…","revision":6},
  "data":{"plan_id":"pln_01JCVB9","steps":5,"applied":0,"failed":1,"rolled_back":true,
          "revision_before":6,"revision_after":6,
          "results":[{"step":"tin","ok":true,"data_ref":"tin"},
                     {"step":"mh1","ok":true,"data_ref":"mh1"},
                     {"step":"mh2","ok":true,"data_ref":"mh2"},
                     {"step":"t1","ok":false,"data_ref":null},
                     {"step":"val","ok":false,"data_ref":null}]},
  "diagnostics":[{"code":"E_STATE_DUPLICATE_NAME","severity":"error",
                  "message":"Ya existe una tubería con el nombre T-01.",
                  "object":{"family":"pipe","id":null,"name":"T-01"},
                  "path":"steps[3].args.name","field":"--name",
                  "details":{"existing_id":"0193f2d1-1b2c…","existing_rev":4},
                  "hint":"Use otro nombre o actualice la tubería existente.",
                  "hint_command":"redes pipe update --name T-01 --diameter 250 --json",
                  "retryable":false,"recoverable":true}],
  "warnings":[],"artifacts":[],
  "meta":{"exit_code":2,"duration_ms":2411,"deterministic":true,"…":"…"},
  "error":{"code":"E_STATE_DUPLICATE_NAME","severity":"error","…":"…"}
}}
```

`revision_after == revision_before` demuestra el rollback: el proyecto quedó **exactamente** como estaba, incluidos los IDs de los pozos creados y luego revertidos.

---

## 10. Wrapper MCP

### 10.1 Estrategia: generado, no escrito

El servidor MCP se **genera** del mismo `Action` registry. No hay una segunda definición de la superficie. Consecuencias:

* El `inputSchema` de cada tool es, byte a byte, el `input_schema` de `redes schema <cmd>` (mismo `$id`: `redes://schema/<cmd>/input`). Test: hash igualdad.
* La descripción de la tool es `summary` + `description` + los 2 primeros `examples` del registro: lo que el modelo lee en `tools/list` es lo mismo que un humano lee en `--help`.
* `outputSchema` se publica (structured output): el host puede validar la respuesta, no sólo parsearla.

### 10.2 Nombres

`familia_verbo` en `snake_case`, sin prefijo del servidor: `manhole_create`, `pipe_update`, `network_validate`, `hydraulics_solve`, `earthwork_compute`, `export_landxml`, `render_plan`, `plan_run`. La identidad del servidor (`redes`) ya da el espacio de nombres; un prefijo por tool sólo gasta tokens en cada llamada. `REDES_MCP_PREFIX` existe para hosts que aplanan nombres.

Nunca: `createManhole`, `rune_manhole_create`, `redes.create.manhole`.

### 10.3 Perfiles

| Perfil | Tools | Para |
|---|---|---|
| `read-only` | 9 (las de lectura del perfil `core`) | hosts sin permiso de escritura, exploración |
| `core` (default) | 12 (exactamente los de CLI-06) | agente de obra típico |
| `full` | 118 | agente experto, automatización |

`core` (12): `capabilities`, `project_info`, `query_objects`, `manhole_create`, `pipe_create`, `network_validate`, `hydraulics_solve`, `hydraulics_check`, `earthwork_compute`, `export_landxml`, `render_plan`, `plan_run`. Las escrituras que no están en `core` (update, delete, bulk) se ejecutan dentro de `plan_run`, que sí lleva token de confirmación: menos tools, misma potencia.

Excluidos de MCP siempre (127 − 9 = 118): grupo `ui` (3, ejecuta guiones en la GUI), grupo `mcp` (3), `doctor` (1), `completion` (1) y `project upgrade` (1). No son peligrosos por sí mismos: son vectores de confusión y no aportan a un agente.

### 10.4 Anotaciones

| Tool | `readOnlyHint` | `destructiveHint` | `idempotentHint` | `openWorldHint` |
|---|---|---|---|---|
| `capabilities`, `schema_get`, `*_list`, `*_get`, `network_*`, `hydraulics_solve`, `hydraulics_check`, `earthwork_compute`, `profile_compute`, `export_*`, `render_plan` | `true` | `false` | `true` | `false` |
| `manhole_create`, `pipe_create`, `plan_run` | `false` | `false` | `true` | `false` |
| `manhole_update`, `pipe_update`, `project_set` | `false` | `false` | `true` | `false` |
| `manhole_delete`, `pipe_delete`, `project_restore`, `bulk_delete` | `false` | **`true`** | `false` | `false` |

`openWorldHint:false` en todo: el servidor no accede a red. `export`/`render` escriben sólo bajo `--allow-out`.

### 10.5 Argumentos

* Mismos nombres que los flags largos, en `snake_case`: `--invert-from` → `invert_from`.
* Mismas unidades declaradas: `{"diameter": {"type":"integer","unit":"mm"}}` (extensión `x-redes-unit`, no inventada en la descripción).
* Tipos estrictos (`additionalProperties:false`); un argumento desconocido devuelve `E_ARG_UNKNOWN` con `accepted` — el modelo se autocorrige en un turno.
* Los flags globales relevantes se exponen como argumentos opcionales: `dry_run`, `explain`, `fields`, `limit`, `expect_revision`, `idempotency_key`. `out` sólo si `--allow-out` está configurado.
* `unit` nunca se omite en la descripción: el skill `mcp-tool-surface` exige que un número desnudo no implique unidad, y el esquema lo cumple.

### 10.6 Seguridad

| Rail | Mecánica |
|---|---|
| Perfil | `--profile read-only` rechaza toda tool `mutates:true` con `E_MCP_PROFILE_DENIED` (exit 2) |
| Dos fases | Toda tool destructiva exige `confirm_token` obtenido con `dry_run:true`; TTL 120 s; ligado a ids + revisión |
| Revisión | `expect_revision` opcional en escrituras; si falta, el servidor usa la revisión de la última lectura del mismo tool en la sesión |
| Rutas | `--allow-out <dir>` (repetible); escribir fuera → `E_IO_PERMISSION` |
| Reversibilidad | Toda escritura MCP se journaliza con `actor:"mcp:<clientInfo.name>"`; `project history` y `project undo` la revierten |
| Auditoría | Cada llamada emite una entrada de journal con `args_hash`, `tool`, `outcome` |
| Sin shell | El servidor no expone ninguna tool que ejecute comandos arbitrarios |

La regla cultural: **en MCP, destructivo = reversible + confirmado**. Un agente puede borrar un pozo si pidió el token y el host lo aprueba; el humano puede deshacerlo porque está en el journal.

### 10.7 Recursos y prompts

| Recurso | Contenido |
|---|---|
| `redes://project` | resumen del proyecto, revisión, conteos (lectura barata de contexto) |
| `redes://project/history?limit=20` | journal reciente |
| `redes://schema/{command}` | JSON Schema de entrada/salida |
| `redes://catalog/errors` | registro de códigos, exits, hints |
| `redes://catalog/units` | magnitudes y decimales |
| `redes://project/objects/{family}` | listado compacto paginado |

Prompts: dos, no más. `diseñar_red_desde_csv` (importa, crea pozos y tuberías, valida, resuelve) y `revisar_cumplimiento` (valida, resuelve, contrasta y produce informe). Ambos son el mismo `plan` que un humano puede ejecutar con `redes task run`.

### 10.8 Progreso y errores

* `notifications/progress`: `progressToken` del `_meta` de la petición; etapas idénticas a los eventos de `--stream` (`topology`, `assemble`, `solve`, `earthwork`). Un solo vocabulario para ambos adaptadores.
* Errores: `isError:true` **y** el sobre completo en `structuredContent`; el mensaje de texto es `code: message` + `hint`, para hosts que sólo muestran texto.
* `E_LIMIT_TIMEOUT` devuelve `structuredContent.data` parcial con `partial:true`: el agente no pierde 4 minutos de cómputo.

### 10.9 Versionado y evidencia

* `initialize` reporta `serverInfo.version` = versión del motor + perfil activo (`0.1.0+core`).
* Deprecaciones: la tool sigue existiendo, su descripción empieza por `[DEPRECADO en 0.4, se elimina en 0.6]` y `capabilities` lo refleja.
* **Afirmación de capacidad verificada:** una frase del tipo "el servidor MCP expone la excavación" no vale. Debe citar tool, versión del servidor, host, versión del host y el run: `redes mcp selftest --out evidence.json` produce ese artefacto (tool, argumentos, `sha256` del sobre, versión, timestamp). Ese JSON es la evidencia; reutilizar el binario no reutiliza la evidencia.

---

## 11. Ergonomía humana

Sin sobre-ingeniería: cinco medidas.

1. **Defaults inteligentes.** Proyecto autodescubierto, `si`, `out/`, tabla en TTY, colores sólo en TTY, `--dry-run` activo por defecto en `network heal` y `hydraulics design`.
2. **`--help` con ejemplos reales.** Cada comando imprime 2–3 invocaciones copiables (las mismas del registro, no escritas a mano), los códigos de error frecuentes y el comando siguiente sugerido.
3. **Tabla por defecto.** `redes manhole list` muestra `NOMBRE · COTA · PROF · Ø · TUBERÍAS · REV`; `--fields` cambia columnas; `--format md` para pegar en un informe.
4. **`redes.toml` con tareas.** El flujo de entrega de una obra es una tarea versionada en el repo, no un script bash perdido:

```toml
[tasks.revisar]
steps = ["network.validate --fail-on warning", "hydraulics.solve --storm 10y", "hydraulics.check"]
```

`redes task list` / `redes task run revisar`. Mismo motor que `plan run`: si el humano lo puede ejecutar, el agente lo puede componer.

5. **Completado de shell** (`redes completion powershell|bash|zsh|fish`) generado por clap.

**No** en v1: REPL. Razón: el estado implícito de un REPL (¿qué proyecto?, ¿qué revisión?, ¿qué resultado del comando anterior?) choca con el modelo de revisiones y con la reproducibilidad. `plan run` con `$refs` cubre la composición; el shell cubre la exploración.

---

## 12. Pruebas del contrato

| # | Test | Qué garantiza | Cómo |
|---|---|---|---|
| T1 | Snapshot de esquemas | el contrato no cambia sin querer | `insta` sobre `schema --all` (claves ordenadas, sin volátiles); un cambio exige actualizar el snapshot y una entrada de changelog |
| T2 | Golden outputs | la forma exacta del sobre | proyecto fixture + `--deterministic --pretty`; comparación byte a byte; 1 golden por comando y por camino de error principal |
| T3 | Conformidad al sobre | ninguna clave falta o sobra | validar cada golden contra `redes.envelope/1` con `jsonschema` en CI |
| T4 | Cobertura de capacidades (UI→CLI) | "la app expone todo por CLI" | enumerar `Action::ALL` y afirmar que cada uno tiene comando CLI y entrada en `capabilities` |
| T5 | Cobertura inversa (CLI→UI) | nada es CLI-only por accidente | todo comando debe estar ligado a una acción de UI **o** marcado `headless_only:true` con `reason` en el registro; el test falla ante un `headless_only` sin razón |
| T6 | Cobertura de MCP | paridad CLI/MCP | por cada perfil, `tools/list` ↔ `capabilities`: uno a uno, y `sha256(inputSchema)` igual al del CLI |
| T7 | Taxonomía ejercitada | ningún código muerto ni hint vacío | test parametrizado por código: provoca el error, verifica `code`, `exit`, `hint ≠ ""`, `hint_command` aceptado por `plan validate` |
| T8 | Invariantes | `ok ⟺ exit 0`, `warnings ⊆ diagnostics`, orden canónico | property tests sobre la matriz de comandos × entradas válidas/inválidas |
| T9 | Determinismo | dos corridas idénticas | ejecutar dos veces con `--deterministic` y comparar `sha256` del stdout; también entre `--dry-run` y ejecución real (diff vacío) |
| T10 | Idempotencia y atomicidad | nada se aplica dos veces ni a medias | replay de clave (sobre idéntico, revisión intacta); fallo inyectado en el paso *k* de un plan ⇒ hash del archivo de proyecto sin cambios |
| T11 | Completitud de `explain` | todo número es justificable | recorrer `data` y afirmar que toda magnitud con `trace` referencia un nodo existente (salvo `explain.truncated:true`, donde el nodo puede estar entre los `omitted_nodes`); lista blanca explícita de valores sin traza (`"trace":"unavailable"` con `reason`) |
| T12 | Render determinista | el PNG es comparable | `render plan --baseline` en CI con `--deterministic-fonts` sobre GPU headless (o *skip* visible); umbral ≤ 0.1 % de píxeles distintos; el diff se publica como artefacto |
| T13 | Presupuesto de tokens | el descubrimiento no explota | medir tokens de `capabilities --brief` (< 1200) y `schema <cmd> --brief` (< 400) en CI; falla si crecen > 20 % |

**El test que importa de verdad es T4/T5 juntos.** No demuestran que el CLI sea *bueno*; demuestran que el requisito del cliente ("un CLI que expone todas sus capacidades") es verificable y no una promesa. Se apoyan en que GUI y CLI comparten el enum `Action`; si alguien añade un botón con lógica propia, T4 no lo ve y el diseño ya se rompió — por eso la regla es que la UI **no** tiene lógica, sólo dispatcher.

**Trazabilidad con `05-product`:** T4+T5 = CLI-01 · T1+T2+T3 = CLI-02 · T1+T6 = CLI-03 · T8+T10 = CLI-04 · T7 = CLI-05 · T6 = CLI-06 · T10 = CLI-07.

Complemento: `redes selftest --out evidence.json` ejecuta la secuencia de los 15 ejemplos de §9 contra un proyecto temporal y valida hashes. Es el artefacto que se cita cuando alguien pregunta "¿esto funciona?".

---

## 13. Versionado y compatibilidad

| Superficie | Política |
|---|---|
| `redes.envelope/1` | sólo cambios aditivos; una clave nueva puede aparecer, ninguna desaparece ni cambia de tipo |
| Comandos | `stability: stable` (no rompe en 1.x), `beta` (puede cambiar con aviso de 2 minors), `experimental` (puede desaparecer) |
| Flags | deprecación con `W_DEPRECATED_FLAG` + alias funcional durante 2 minors; nunca se reutiliza un nombre para otra semántica |
| Códigos de error | nunca se eliminan; se retiran con `retired:true` en el catálogo y el exit se conserva |
| JSON Schema | `$id` versionado (`redes://schema/pipe.create/input`), `additionalProperties:false` |
| Archivo de proyecto | `schema_version` + `project upgrade` con backup automático y `E_PROJ_SCHEMA_TOO_NEW` |
| MCP | `serverInfo.version` incluye perfil; tools deprecadas marcadas en la descripción; `capabilities` es la fuente de verdad |

---

## 14. Riesgos y decisiones abiertas

| Riesgo | Mitigación | Dueño |
|---|---|---|
| 127 comandos abruman al modelo | `capabilities --brief` + `schema` bajo demanda + perfil `core` (12) + `plan` para componer | este documento |
| Dos fases añaden un turno | obligatorio sólo en destructivos; `plan run` lo resuelve en un turno para lotes | implementación |
| `explain` incompleto en algún cálculo derivado | T11 con lista blanca explícita y `reason`; se revisa en cada PR que añada una magnitud | motor |
| Códigos normativos (NEC-11 vs norma local) | el paquete de códigos es dato del proyecto (`--codes`), no constante del binario; `code_refs` puede quedar vacío con `W_` | ingeniero civil |
| Reproyección CRS en runtime | PROJ vía binding; si no está, `E_GEO_TRANSFORM_FAILED` con `hint` de instalar el paquete CRS, nunca silencio | motor |
| Render GPU distinto entre máquinas | `--deterministic-fonts` (rasterizador propio de Rune, igual en todo SO) + umbral de T12 (≤ 0.1 % de píxeles); CI necesita GPU headless o marca T12 como *skipped* de forma visible, nunca verde falso | UI |

**Decisiones abiertas que este documento no puede cerrar solo:**

1. ¿El proyecto es SQLite single-file o un directorio de objetos? Asumido SQLite (transacciones y `--expect-revision` son triviales); si el consejo elige otro almacén, la atomicidad de `plan` necesita un WAL propio.
2. ¿`hydraulics design` muta con `--apply` o devuelve siempre propuesta + `plan`? Recomiendo lo segundo a medio plazo (más componible), con `--apply` como azúcar.
3. ¿`outfall` es un tipo de nodo o una familia aparte? Asumido familia propia con `family:"outfall"` y `degree` 1; afecta a `network.validate`.
4. Nombre definitivo del binario: `redes` es el propuesto; si el cliente prefiere uno en español (`redes`, `pluvia`), cambiar aquí y en `REDES_*` antes de 1.0.

---

## 15. Apéndice — checklist de implementación

1. `Action` registry con `id`, `mutates`, `destructive`, `idempotent`, `dry_run`, esquemas, ejemplos, `error_codes`, `since`, `stability`.
2. Sobre `redes.envelope/1` como struct único de serialización; test de invariantes §7.
3. `schemars` para input/output; `$id` `redes://schema/<cmd>/{input,output}`.
4. Generador clap desde el registro; `--help` con ejemplos.
5. Generador MCP desde el registro; perfiles; anotaciones.
6. IDs BLAKE3 + Crockford base32; orden canónico en todos los listados.
7. Diagnósticos con `hint`/`hint_command`; catálogo de códigos como dato, no como `match` disperso.
8. `explain` como DAG con `trace` en cada magnitud; `summary`/`full`; límites de tamaño.
9. `--dry-run` + `confirm_token` en destructivos; `plan run` atómico con rollback.
10. Idempotencia en `.redes/idempotency/`; TTL 7 días.
11. Streaming NDJSON con vocabulario compartido con MCP; evento `result` final.
12. Artefactos con `sha256`, rename atómico y `file_url`.
13. CI: T1–T13; `selftest` como artefacto de evidencia.
14. Presupuesto de tokens en CI (T13) para que el descubrimiento no degrade.

---

## Anexo A — Contrato con `01-domain` y `05-product`

Este documento es normativo para la **interfaz**; `01-domain` manda sobre el modelo y `05-product` sobre requisitos y vocabulario de cliente. Aquí se resuelve cada delta, con la decisión y su coste.

### A.1 Conformidad con los requisitos CLI

| Requisito | Dónde se cumple | Delta / decisión |
|---|---|---|
| CLI-01 paridad UI ↔ CLI | §3 (un `Action`), §12 T4+T5, §12 T5 (`headless_only` con razón) | Sin delta. La paridad es por construcción: misma acción, tres adaptadores. |
| CLI-02 `--json` estable y versionado | §6 (sobre), §13 (versionado), §12 T2+T3 | **Delta en la forma de la unidad**: se emite `{value,unit}` en lugar de `diametro_mm`. Ver A.5. |
| CLI-02 esquema validado en CI | §12 T1, T3 | Sin delta. |
| CLI-03 manifiesto autodescriptivo | §5.3, §8.1, `redes esquema` | Sin delta de diseño; el nombre canónico es `esquema` (A.2). |
| CLI-04 `--dry-run` e idempotencia | §8.3, §8.5, §8.4 | Se añade idempotencia por clave natural (implícita) además de la explícita. |
| CLI-05 códigos de salida | §6.4 | Adoptada la numeración 0/2/3/4/5/6; se añaden 1/7/8/9/130 (A.6). |
| CLI-06 servidor MCP | §10, §12 T6 | `core` = **12 tools exactas**. |
| CLI-07 modo lote y CI | §8.10, §5.9 (`plan`, `task`), §12 T10 | Sin delta: `on_error:abort` por defecto, exit ≠ 0 si el check normativo falla, resumen JSON. |

### A.2 Idioma de comandos y flags

`05-product` publica invocaciones en español (`redes proyecto nuevo`, `redes esquema`, `--solo-lectura`); este documento razona en ids en inglés (`project.init`, `capabilities`). No son incompatibles si se separan **id de acción** y **etiqueta de interfaz**:

| Capa | Idioma | Ejemplo |
|---|---|---|
| `Action::id` (manifiesto, `command` del sobre, MCP tool) | inglés, estable | `project.init`, `manhole.create` |
| Etiqueta CLI (`--lang es`, default) | español | `redes proyecto nuevo`, `redes pozo crear` |
| Etiqueta CLI (`--lang en`) | inglés | `redes project init`, `redes manhole create` |
| Flags | alias por locale | `--nombre/--name`, `--unidades/--units`, `--solo-lectura/--read-only` |

La tabla de locale se genera del registro (una entrada `id → {es, en}` por acción); no hay código duplicado. `redes esquema` es la etiqueta española del comando cuyo id es `capabilities`, y `redes esquema <comando>` la de `schema <comando>`.

**Coste y recomendación.** Mantener las dos etiquetas cuesta ~120 pares de strings y una columna más en las pruebas de snapshots; el beneficio es que el cliente ve lo que ya leyó y el agente ve ids estables en inglés. Si el consejo prefiere una sola etiqueta, la elección barata es **sólo español** (se borra la columna inglesa, los ids no cambian); la cara es **sólo inglés** (hay que reescribir los walkthroughs de `05-product`).

### A.3 Familias F01–F17

La tabla de §5 cubre v1 con siete familias; el patrón CRUD de §5.5 aplica a las diecisiete sin cambios de diseño, y `query objects` las recorre todas.

| # | Familia (`01-domain`) | Grupo CLI | v1 |
|---|---|---|---|
| F01 | Pozo | `manhole` | ✔ |
| F02 | Tubería | `pipe` | ✔ |
| F03 | Descarga / vertedero | `outfall` | ✔ |
| F04 | Válvula | `valve` | ✔ |
| F05 | Llave (corporación, banqueta, hidrante, tapón) | `llave` | ✔ (`--type hydrant` en v1) |
| F06 | Tanque / reservorio | `tank` | ✔ |
| F07 | Planta (PTAP/PTAR) | `plant` | v1.x |
| F08 | Medidor | `meter` | v1.x |
| F09 | Cámara | `chamber` | v1.x |
| F10 | Sifón | `siphon` | v1.x |
| F11 | Cruce | `crossing` | v1.x |
| F12 | Acometida | `service` | ✔ |
| F13 | Sumidero | `inlet` | v1.x |
| F14 | Canal | `channel` | v1.x |
| F15 | Captación | `intake` | v1.x |
| F16 | Estación de bombeo (nodo compuesto) | `pump_station` | v1.x |
| F17 | Elementos de apoyo | `support` | v1.x |

Regla de extensión: añadir una familia = añadir 1 entrada al registro con sus atributos, y se obtienen gratis `create/get/list/update/delete`, validación, `query`, exportación y tools MCP. El test T4 falla si `01-domain` declara una familia sin grupo.

**Nota sobre F05.** Se renombra el grupo `hydrant` a `llave` porque F05 incluye corporación de acometida, banqueta y tapón terminal, no sólo hidrantes; "hydrant" habría sido un nombre falso. Se conserva el término inglés en el id de la acción (`llave.create`) por coherencia con el prefijo del dominio.

### A.4 Identidad

`01-domain §2.4` fija `id` = UUID v7 y `codigo` legible. §4.3 lo adopta: `name` en la interfaz **es** el `codigo`; `--codigo` es alias; el `nombre` libre del dominio se expone como `label`. `--deterministic` deriva el v7 del código para que importaciones y tests sean reproducibles, sin cambiar el tipo ni el formato.

### A.5 Unidades: `{value, unit}` frente a `diametro_mm`

CLI-02 pide "la unidad en el nombre". Se cumple el **intento** (ningún número sin unidad inequívoca) con una forma que no se rompe al convertir:

| CLI-02 literal | Este documento | Motivo |
|---|---|---|
| `"diametro_mm": 250` | `"diameter": {"value": 250, "unit": "mm"}` | con `--units imperial` la clave `diametro_mm` miente; el objeto no |
| `"longitud_m": 120.0` | `"length": {"value": 120.0, "unit": "m"}` | una sola regla para todas las magnitudes y para el redondeo |
| — | `--compact` | quita `unit` cuando coincide con el sistema del proyecto (ahorro de tokens) |
| — | `--keys=units` | **modo de compatibilidad** que emite `diametro_mm`, `longitud_m`… derivado del registro; para consumidores que exijan la forma literal de CLI-02 |

Las coordenadas son la excepción declarada (`{x,y,z,crs}`): el CRS ya define unidad y datum, y sufijar cada vértice triplica el coste en tokens sin ganancia.

### A.6 Códigos de salida

| Exit | Significado (CLI-05) | Áreas que caen aquí |
|---|---|---|
| 0 | ok | `W_*` (avisos) |
| 2 | validación | `E_ARG_*`, `E_STATE_NOT_FOUND`, `E_STATE_DUPLICATE_NAME`, `E_MCP_PROFILE_DENIED` |
| 3 | norma | `E_CHECK_FAILED`, `E_TOPO_*`, `E_HYD_*` de criterio, `E_EARTH_*`, `E_STATE_REFERENCED`, `E_RENDER_VIEW_EMPTY` |
| 4 | E/S | `E_IO_*`, `E_FMT_*`, `E_PROJ_NOT_FOUND`, `E_RPT_TEMPLATE_MISSING`, `E_RENDER_DEVICE` |
| 5 | topografía / geometría | `E_GEO_*` |
| 6 | conflicto de versión | `E_PROJ_LOCKED`, `E_PROJ_VERSION_CONFLICT`, `E_PROJ_SCHEMA_TOO_NEW`, `E_PROJ_READONLY`, `E_STATE_IDEMPOTENCY_MISMATCH` |
| 1 | interno | `E_INTERNAL`, `E_NOT_IMPLEMENTED` |
| 7 | límite / paginación | `E_LIMIT_*` |
| 8 | confirmación requerida | `E_CONFIRM_REQUIRED` |
| 9 | solver numérico | `E_HYD_NO_SOLUTION`, `E_HYD_DIVERGED` |
| 130 | interrumpido | `SIGINT` |

Un mismo código conserva su exit entre versiones: la tabla es dato del registro, no un `match` disperso.

### A.7 Huecos detectados en `05-product` y cubiertos aquí

1. `redes proyecto comparar --con revision-1.redes` compara **archivos**, no revisiones: `project diff` acepta `--from/--to` con revisión o ruta `*.redes`.
2. `--solo-lectura`: se añade `--read-only` global con `E_PROJ_READONLY` (exit 6).
3. `esquema` debe declarar la `natural_key` de cada comando (CLI-04): se añade al manifiesto.
4. CLI-06 exige 12 tools pero no dice cuáles: §10.3 fija la lista y el criterio (crear + validar + calcular + exportar + render + plan; update/delete van por `plan_run`).

### A.8 Pendiente de arbitraje del consejo

| Tema | Opciones | Recomendación |
|---|---|---|
| Idioma de etiquetas | es+en generadas / sólo es / sólo en | es+en (A.2) |
| Nombres de atributos en el JSON | inglés (`name`, `depth`) / español del dominio (`codigo`, `profundidad`) | inglés en el sobre y en `data`; español sólo en etiquetas. Si el cliente exige el diccionario del dominio en el JSON, el cambio es de serialización (`serde(rename_all)`), no de comandos. |
| Forma de la unidad | `{value,unit}` / sufijo en la clave | `{value,unit}` + `--keys=units` (A.5) |
| Alcance de familias en v1 | 7 / 17 | 7 en v1, 10 en v1.x con el patrón heredado (A.3) |