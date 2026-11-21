# 20 · Revisión adversarial — datos, persistencia e intercambio

**Revisor:** `adv-datos` · **Fecha:** 2026‑11‑20 · **Binario usado:** `target/debug/redes.exe` (`redes 0.1.0`, `rustc 1.95.0`), árbol limpio (`git status --porcelain` vacío al empezar).

**Alcance:** `crates/redes-core/src/{model,families,project_io,ingest,export,city,envelope}.rs`, la parte de importación/exportación de `crates/redes-cli/src/main.rs` y la skill `skills/redes-ingesta/`.

**Método:** auditoría de código + reproducción ejecutando la CLI real sobre archivos hostiles construidos para cada caso. Todo se ejecutó en `out/adv-datos/` (borrado al terminar). Cada fila remite a una receta numerada del **Apéndice A**; las recetas son autocontenidas y se pueden volver a correr tal cual. Las columnas de salida citadas (`exit`, `ok`, `descartados`) son las observadas, no las esperadas.

**Criterio de severidad:** `CRÍTICO` = pérdida de datos, archivo irrecuperable o pánico · `ALTO` = resultado falso o dato descartado en silencio con `ok:true` · `MEDIO` = contrato incumplido con efecto verificable en el entregable · `BAJO` = ruido de contrato sin pérdida.

---

## Tabla de hallazgos

| ID | severidad | archivo:línea | qué falla | reproducción | arreglo propuesto |
|---|---|---|---|---|---|
| **D-01** | CRÍTICO | `ingest.rs:641-653` + `project_io.rs:28,42` | Un CSV de terreno con `NaN` se acepta (`"NaN".parse::<f64>()` funciona) y el `.redes` se guarda con `null` en `points`/`bbox`: **el proyecto queda ilegible para siempre**. El import devuelve `ok:true`, `exit 0`, `puntos_terreno: 3`, `descartados: 0`. | **R1** | Rechazar todo valor no finito (`is_finite()` sobre `e`, `n` y `z`) en `import_terreno` y en `SurfacePoint`, y en `save()` validar el proyecto antes de serializar (`E_GEO_OUT_OF_BOUNDS`/`E_FMT_PARSE` en vez de escribir `null`). |
| **D-02** | CRÍTICO | `ingest.rs:292-297` (y `331-333`) | Un número no numérico en un `POINT`/`VERTEX` de DXF se convierte en `f64::NAN` (`unwrap_or(f64::NAN)`) y entra al modelo sin validación. Al guardar, `serde_json` escribe `null` y **el proyecto deja de abrirse** (`E_FMT_PARSE: invalid type: null, expected f64`). | **R2** | Igual que D-01: `parse` fallido ⇒ descartar la entidad con motivo (`descartados`), nunca `NaN`; y validación de finitud en `save()`. |
| **D-03** | CRÍTICO | `terrain.rs:56` (vía `project_io.rs:47-50`) | Un `.redes` sintácticamente válido con un índice de triángulo fuera de rango hace **panic** en `reindexar()` al abrir: `index out of bounds: the len is 2400 but the index is 999999`. `exit 101`, sin sobre JSON. | **R3** | Validar en `load()` que `tris[i] < points.len()`; `TriIndex::build` con `get()` y descarte; devolver `E_FMT_PARSE`/`E_PROJ_DAMAGED` con el detalle. |
| **D-04** | CRÍTICO | `project_io.rs:43` (+ `model.rs:104-227`) | `load()` acepta **cualquier** `schema: "redes.project/*"` (incluido `/3` y `/99`) y serde descarta los campos desconocidos en silencio: abrir un proyecto de una versión futura y guardarlo **borra** esos campos. `ok:true`, `exit 0`. | **R4** | Comparar contra la versión soportada (lista blanca `redes.project/2`), emitir `E_PROJ_SCHEMA_TOO_NEW` (exit 6 según contrato) y/o `deny_unknown_fields` + migrador; nunca reescribir un archivo de versión superior. |
| **D-05** | CRÍTICO | `skills/redes-ingesta/scripts/inp_to_plan.py:92,115,202,223` (+ `ingest.rs:296-301`) | El plan que produce la skill usa subtipos que **no existen** en el registro (`emisario_final`, `colector`, `apoyado`, `distribucion`). `add_link`/`add_node` los rechaza: de un SWMM mínimo se importan **0 arcos y 0 descargas** (solo sobreviven las junctions), con `ok:true` y `exit 0`. La skill, por su lado, imprime «4 nodos, 3 arcos» como éxito. | **R5** | Validar el plan contra `families()` al generarlo (la skill debe leer `redes esquema`), o mapear a `emisario_muro`/`colector_sanitario`/`apoyado_cilindrico`/`distribucion_primaria`; y que `importar plan` devuelva `ok:false` (exit 3) si se descarta el 100 % de los objetos. |
| **D-06** | CRÍTICO | `ingest.rs:239-252` | `default_subtype()` devuelve subtipos inexistentes: `agua.llave → "columna"` y, por el `_ => "inspeccion"`, **21 de las 29 familias** no se pueden importar sin `subtipo` explícito. Un GeoJSON/DXF/plan normal pierde esos objetos con una lista de descartes que el usuario no lee. | **R6** | Derivar el subtipo por defecto del propio registro (`f.types.first()`), con un mapa explícito solo donde el primero no sea el correcto; test que recorra las 29 familias. |
| **D-07** | ALTO | `project_io.rs:40-52` + `model.rs:255-259` | `load()` no valida ni recalcula `next_slot` (ni los `ElementId`). Un `.redes` fusionado/editado con el contador atrasado hace que el siguiente `alloc()` **reutilice un `ElementId` existente**; a partir de ahí `node(id)` devuelve el primero y la CLI muta el objeto equivocado. | **R7** | En `load()`: `next_slot = max(slot)+1` (o error si hay ids duplicados) y validar el resto de invariantes (D-08, D-24, arcos a nodos `deleted`). |
| **D-08** | ALTO | `model.rs:303` y `:327` | `add_node` compara el código solo contra `nodes` y `add_link` solo contra `links`: un nodo y un arco pueden compartir código (se viola «código legible único en el proyecto», `model.rs:110`). `objeto aprobar --codigo M1` siempre resuelve al nodo, así que el arco homónimo es **inalcanzable** y `proyecto aprobar` queda bloqueado para siempre. | **R8** | Un único índice (`BTreeMap<String, ElementId>` case‑insensitive) para nodos+arcos, con `E_STATE_DUPLICATE_NAME`; en `cambiar_estado`, error `E_STATE_AMBIGUOUS` si el código existe en ambos. |
| **D-09** | ALTO | `ingest.rs:525-568` | `import_geojson` ignora el CRS y no comprueba unidades: un GeoJSON en grados (lon/lat, sin bloque `crs`) entra como metros en un proyecto `EPSG:32717`. Resultado: `bbox [-78.51, -0.20]`, `cota_min = cota_max = 0.0`, `descartados: 0`, ningún diagnóstico (el contrato define `E_GEO_CRS_MISSING` / `E_GEO_CRS_MISMATCH`). | **R9** | Exigir `crs` (o `--crs`) y comparar con `p.crs`; detectar planta en grados (x dentro de ±180, y dentro de ±90); si no coincide, `E_GEO_CRS_MISMATCH` (exit 3) sin importar. |
| **D-10** | ALTO | `ingest.rs:128` y `:221-223` | Importar un plan **reemplaza `p.crs`** por el del plan y deja la geometría anterior reinterpretada; además el `Terrain` se construye con `32717` fijo aunque el plan declare otro CRS. Queda un proyecto con dos CRS (`crs: EPSG:32617`, `terrain.epsg: 32717`) y `ok:true`. | **R10** | Usar `plan.proyecto.crs` para el terreno (`parse` del EPSG), y si `plan.crs != p.crs` con geometría existente ⇒ `E_GEO_CRS_MISMATCH` (o reproyección explícita con `--reproyectar`). |
| **D-11** | ALTO | `ingest.rs:273-367` y `:494-510` | DXF: los `INSERT` (las pozos reales) se ignoran, la geometría del `BLOCK` se importa como objeto real en el origen (0,0), un `POINT` en capa `TEXTOS` se vuelve pozo y una `LINE` en `CURVAS-NIVEL` se vuelve tubería creando dos pozos implícitos a 2,5 km con cota 0. `nodos: 5`, `arcos: 1`, `descartados: 0`, sin un solo aviso. | **R11** | Rastrear secciones (`BLOCKS` vs `ENTITIES`), resolver `INSERT` (transformar la geometría del bloque por el punto de inserción) y **rechazar** capas no reconocidas con aviso (`familia` por perfil de mapeo, no por heurística silenciosa). |
| **D-12** | ALTO | `ingest.rs:125-139` + `terrain.rs:89-98` | Terreno degenerado (2 puntos o 3 colineales) ⇒ `tris: 0` y `z_at()` siempre `None`; el fallback `unwrap_or(0.0)` deja **todas las cotas en 0.0 y las soleras en -1.5** aunque el terreno declare 2500 m. `ok:true`, `puntos_terreno: 3`, `descartados: 0`. | **R12** | Tras `Terrain::build`, si `is_empty()` ⇒ error `E_GEO_NO_TIN_AT_POINT` (o al menos `ok:false`); nunca inventar cota 0: dejar `z_terrain` como `None`/`NaN` explícito y reportarlo. Misma validación para `import_terreno` (hoy exige 3 puntos pero acepta colineales). |
| **D-13** | ALTO | `main.rs:369,387-398,887-895` + `export.rs:81,145` | `--dry-run` («valida y reporta sin escribir») **escribe** DXF, GeoJSON, GLB y SVG con `exit 0`, y en las importaciones ni siquiera valida: `importar plan --archivo NO-EXISTE.json --dry-run` devuelve `ok:true`, `bytes: 0`. | **R13** | Pasar `dry` a `exportar`/`vista` y no abrir el archivo (o escribir a un buffer y reportar `bytes` calculados); en `dry_import`, abrir y parsear el archivo sin aplicar, y devolver `descartados`/diagnósticos. |
| **D-14** | MEDIO | `envelope.rs:79-103` vs `docs/consejo/03-cli-ai.md:517-544` | Los códigos de salida no cumplen la tabla normativa: `E_IO_NOT_FOUND` → 7 (norma 4), `E_FMT_PARSE` → 7 (norma 4), `E_STATE_NOT_FOUND` → 4 (norma 2), `E_PROJ_SCHEMA_TOO_NEW` → 4 (norma 6); `exit::CONFLICT = 5` mientras la norma usa 6. El registro publicado en `esquema.codigos_error[]` tampoco coincide con la norma. | **R14** | Tabla `código → exit` como dato único (una `match` exhaustiva por código, no por prefijo) y test que compare `CODIGOS` con el doc; añadir `E_PROJ_SCHEMA_TOO_NEW` a `CODIGOS`. |
| **D-15** | MEDIO | `project_io.rs:41` | Todos los errores de `read_to_string` se rotulan `E_IO_NOT_FOUND`, incluido «Access is denied (os error 5)» al pasar un directorio y «stream did not contain valid UTF‑8» con un `.redes` en ANSI. El usuario recibe «archivo inexistente» y una pista que no aplica. | **R15** | Mapear `e.kind()`: `NotFound → E_IO_NOT_FOUND`, `PermissionDenied → E_IO_PERMISSION`, `InvalidData → E_FMT_PARSE`, resto `E_IO_PERMISSION`. |
| **D-16** | MEDIO | `main.rs:258-262` | Los errores de `clap` (p. ej. falta `--codigo`) salen como texto plano por stderr con `exit 2`, **sin sobre JSON**: incumple «Toda salida es un sobre JSON `redes.envelope/1`» (`main.rs:4`) y el código `E_ARG_MISSING` nunca se emite. Un agente que parsea stdout se queda sin diagnóstico. | **R16** | `Cli::try_parse()` y, en `Err(e)`, emitir `Envelope::fail` con `E_ARG_MISSING`/`E_ARG_UNKNOWN` (exit 2) y `hint` con el uso; `exit(2)` después de imprimir. |
| **D-17** | MEDIO | `ingest.rs:97` (y `:194,606`) | `diametro_mm: Option<u32>`: un plan escrito por Python/JS con `"diametro_mm": 200.0` (JSON no distingue enteros) **rechaza el plan entero** con `E_FMT_PARSE: invalid type: floating point 200.0, expected u32`. | **R17** | Deserializar como `f64` y redondear a `u32` con validación de rango (50..3000), o `#[serde(deserialize_with)]` que acepte 200 y 200.0. |
| **D-18** | MEDIO | `export.rs:85-89` (y `:24`) | Las capas DXF se derivan de la etiqueta: 23 de 53 capas son inválidas para DXF R12 (20 con acentos UTF‑8: `CL_TUBER�A`, `CM_C�MARA`…; 3 con `/`, reservado en nombres de tabla: `DS_DESCARGA_/_VERTEDERO`, `HT_HITO_/_BM`, `TQ_TANQUE_/_RESERVORIO`). La tabla `LAYER` se declara con `70 0` (cero capas) aunque las entidades las usan. | **R18** | `layer_name()` con un slug ASCII (`[A-Z0-9_-]`, sin acentos ni `/`) y emitir las definiciones `LAYER` en `TABLES` con el conteo real. |
| **D-19** | MEDIO | `ingest.rs:543-567` y `:619` | `import_geojson` ignora `properties.cota` y `properties.estado`, que `mapeos.md` §5 declara leídos: un punto con `cota: 2500.5` entra con `z_invert: 0.0` y `estado: pendiente`. Además no hace `p.fuentes.push(...)`: la procedencia desaparece del proyecto (`fuentes: []` tras importar). | **R19** | Leer `cota`/`cota_terreno`/`estado` (y `attrs`), y registrar `Fuente { app: "SIG (GeoJSON)", archivo, formato: "geojson", importado_en }` como hacen plan/DXF. |
| **D-20** | MEDIO | `inp_to_plan.py:108-109` | En SWMM los campos 6 y 7 de `[CONDUITS]` son `InOffset`/`OutOffset`, no diámetros; el script hace `max(in,out)*1000` como `diametro_mm`. Un `InOffset = 0.30` produce **DN 300**. | **R20** | Diámetro solo desde `[XSECTIONS]` (`Geom1`); si falta, dejar el default y avisar; los offsets van a `attrs`. |
| **D-21** | MEDIO | `inp_to_plan.py:30-46` | `read_sections()` no lee `[COORDINATES]` (que el `.inp` sí trae) y fuerza `e = n = 0.0` en todos los nodos: el aviso final admite «el visor los apilará». Se descarta planta que estaba en el archivo. | **R21** | Parsear `[COORDINATES]` (`Node X Y`) y `[VERTICES]`; usar `--coords` solo como override. |
| **D-22** | MEDIO | `inp_to_plan.py:56-145` + `mapeos.md:57` | `[STORAGE]` está documentado (`→ saneamiento.camara / reunion`) pero no se implementa: el nodo desaparece y las conducciones que lo tocan se descartan como «extremo inexistente». | **R22** | Implementar `[STORAGE]` (y `[DIVIDERS]`) o quitar la fila del mapeo; mientras tanto, avisar de la sección no soportada. |
| **D-23** | MEDIO | `ingest.rs:208-224` | Un plan vacío (lo que produce la skill con un `.inp` sin secciones conocidas) **renombra el proyecto**, añade una `Fuente` y sube la revisión, importando 0 objetos y con `ok:true`. | **R23** | No aplicar metadatos (`name`, `crs`, `fuentes`, `commit`) si `rep.nodos + rep.arcos == 0`; devolver `ok:false` con `E_FMT_EMPTY`. |
| **D-24** | MEDIO | `model.rs:313-334` | `add_link` no valida el rol topológico de la familia: un arco con `familia: "saneamiento.pozo"` se acepta (`arcos: 2`, `descartados: 0`) y se exporta como POLYLINE en capa `PZ_POZO` / LineString con `familia: saneamiento.pozo`. | **R24** | Exigir `def.topo == TopoRole::Link` en `add_link` y `Node/Device` en `add_node` (`E_STATE_WRONG_ROLE`); validar también los atributos `required` del subtipo. |
| **D-25** | MEDIO | `city.rs:39-43` | `Entorno`/`Calle`/`Manzana` no tienen `#[serde(default)]` en sus campos: un `entorno` con solo `calles` invalida **todo el proyecto** (`E_FMT_PARSE: missing field manzanas`), aunque el tipo tenga `Default`. | **R25** | `#[serde(default)]` en `Entorno.calles/manzanas` y en los campos de `Calle`/`Manzana`; el contexto urbano es opcional por definición. |
| **D-26** | MEDIO | `inp_to_plan.py:162` | EPANET: las `[JUNCTIONS]` (nodos de consumo de **agua potable**) se mapean a `saneamiento.camara / valvulas`. Un proyecto de agua queda con cámaras de alcantarillado en la capa `CM_CÁMARA` del DXF y en el GeoJSON. | **R26** | Mapear a `agua.llave` (o `agua.arqueta`) con el subtipo válido, y actualizar `mapeos.md` §3. |
| **D-27** | MEDIO | `inp_to_plan.py:246-289` | Un `.inp` sin secciones conocidas termina con `exit 0` y «plan: 0 nodos, 0 arcos»: no hay error ni código de salida distinto, así que una cadena automática sigue adelante y produce el D-23. | **R22** | Si `nodos + arcos == 0`, escribir el plan igual pero `exit 1` con mensaje a stderr (y `schema` con `avisos`), o no escribir el archivo. |
| **D-28** | BAJO | `families.rs:507/532` y `510/535`; `main.rs:719-745` | `saneamiento.tuberia` publica `diametro_nominal` y `material` **dos veces** con tipos/unidades distintos (24 atributos, 2 duplicados); `agua.llave` duplica `diametro_nominal`. Además `esquema` no publica las opciones de los `enum` (`tipo_llave`, `tipo_valvula`…), así que un agente no puede saber los valores válidos; y `tipo_llave` no incluye `pase`, que sí es un subtipo. | **R27** | Unicidad de `key` por familia (test), publicar `opciones` de los enum y derivar el enum del subtipo (o viceversa). |
| **D-29** | BAJO | `main.rs:289-293` | `proyecto nuevo/demo --dry-run` reporta `artifacts[].path` y `data.archivo` de un archivo que no existe (`bytes: 0`): un consumidor puede creer que el proyecto se creó. | **R28** | En `--dry-run`, omitir `with_artifact` (o marcar `"creado": false`) y no prometer `data.archivo`. |

---

## Apéndice A — recetas de reproducción

Todas asumen este preámbulo (desde la raíz del repo, PowerShell):

```powershell
$d = "out/adv-datos"; New-Item -ItemType Directory -Force $d | Out-Null; cd $d
$R = "..\..\target\debug\redes.exe"
& $R proyecto demo --salida base.redes --quiet   # proyecto de 43 objetos
```

### R1 · D-01 — NaN en CSV de terreno corrompe el proyecto
```powershell
"NaN,NaN,NaN`nNaN,NaN,NaN`nNaN,NaN,NaN" | Set-Content -Encoding ascii nano.csv
& $R -p base.redes importar terreno --archivo nano.csv      # exit 0, ok:true, puntos_terreno:3
& $R -p base.redes proyecto info                            # exit 7, E_FMT_PARSE: invalid type: null, expected f64
```
Observado además en el archivo guardado: `"points":[{"e":null,"n":null,"z":null}, …]`, `"bbox":{"xmin":null,…}`.

### R2 · D-02 — número no numérico en DXF
```powershell
@'
0
SECTION
2
ENTITIES
0
POINT
8
POZOS
10
760.100,50
20
9796100.0
30
2498.5
0
ENDSEC
0
EOF
'@ | Set-Content -Encoding ascii punto-malo.dxf
& $R -p base.redes importar dxf --archivo punto-malo.dxf    # exit 0, ok:true, descartados:0
& $R -p base.redes proyecto info                            # exit 7, E_FMT_PARSE … at line N
```
El nodo importado queda `{"x": null, "y": 9796100.0}`.

### R3 · D-03 — panic por índice de triángulo
```powershell
python -c "import json;d=json.load(open('base.redes',encoding='utf-8'));d['terrain']['tris'][0]=[0,1,999999];json.dump(d,open('tri-roto.redes','w',encoding='utf-8'))"
& $R -p tri-roto.redes proyecto info    # exit 101; stderr: panicked at terrain.rs:56 … index out of bounds
```
No se imprime ningún sobre JSON (el contrato «toda salida es un sobre» se rompe).

### R4 · D-04 — esquema futuro + campos desconocidos
```powershell
python -c "import json;d=json.load(open('base.redes',encoding='utf-8'));d['schema']='redes.project/3';d['campo_futuro']={'x':1};d['nodes'][0]['cota_rasante_municipal']=2512.3;json.dump(d,open('futuro.redes','w',encoding='utf-8'))"
& $R -p futuro.redes proyecto info        # exit 0, ok:true  (acepta la versión 3)
& $R -p futuro.redes objeto aprobar --codigo PZ-001 --quiet   # guarda
python -c "import json;d=json.load(open('futuro.redes',encoding='utf-8'));print('campo_futuro' in d,'cota_rasante_municipal' in d['nodes'][0])"  # False False
```

### R5 · D-05 — la skill produce subtipos inexistentes (SWMM)
```powershell
@'
[JUNCTIONS]
J1  2498.50  2.0
J2  2498.20  2.0
[OUTFALLS]
OUT1  2496.00  FREE
[CONDUITS]
C1  J1  J2  45.0  0.013  0.0  0.0
C2  J2  OUT1 20.0 0.013  0.0  0.0
[XSECTIONS]
C1  CIRCULAR  0.300  0  0  0  1
'@ | Set-Content -Encoding utf8 swmm-min.inp
python ..\..\skills\redes-ingesta\scripts\inp_to_plan.py --tipo swmm swmm-min.inp --salida plan-swmm.json
#   -> "plan: 3 nodos, 2 arcos" (exit 0)
& $R proyecto nuevo --nombre s --salida s.redes --quiet
& $R -p s.redes importar plan --archivo plan-swmm.json
#   -> ok:true, exit 0, "nodos":2, "arcos":0, detalle_descartes:
#      "nodo OUT1: subtipo 'emisario_final' no existe en saneamiento.descarga"
#      "arco C1: subtipo 'colector' no existe en saneamiento.tuberia"
```

### R6 · D-06 — 21 de 29 familias no importables sin subtipo
```powershell
python -c "import json,subprocess;e=json.loads(subprocess.run([r'..\..\target\debug\redes.exe','esquema'],capture_output=True,text=True,encoding='utf-8').stdout);f=[{'type':'Feature','geometry':{'type':'Point','coordinates':[760000.0+i*10,9796000.0,2500.0]},'properties':{'familia':x['clave'],'codigo':'X%02d'%i}} for i,x in enumerate(e['data']['familias'])];json.dump({'type':'FeatureCollection','features':f},open('todas.geojson','w',encoding='utf-8'))"
& $R proyecto nuevo --nombre f --salida f.redes --quiet
& $R -p f.redes importar geojson --archivo todas.geojson
#   -> nodos: 8, descartados: 21   ("subtipo 'inspeccion' no existe en agua.medidor", "…'columna'… agua.llave", etc.)
```

### R7 · D-07 — ElementId reutilizado por `next_slot` atrasado
```powershell
python -c "import json;d=json.load(open('base.redes',encoding='utf-8'));d['next_slot']=1;json.dump(d,open('slot-bajo.redes','w',encoding='utf-8'))"
@'
{"type":"FeatureCollection","features":[{"type":"Feature","geometry":{"type":"Point","coordinates":[760100.0,9796100.0,2500.0]},"properties":{"familia":"saneamiento.pozo","codigo":"PZ-NUEVO"}}]}
'@ | Set-Content -Encoding utf8 un-punto.geojson
& $R -p slot-bajo.redes importar geojson --archivo un-punto.geojson --quiet
python -c "import json,collections;d=json.load(open('slot-bajo.redes',encoding='utf-8'));c=collections.Counter((n['id']['slot'],n['id']['generation']) for n in d['nodes']);print([k for k,v in c.items() if v>1])"   # [(1, 1)]
& $R -p slot-bajo.redes objeto observar --codigo PZ-NUEVO --nota "revisar"
#   -> ok:true "codigo":"PZ-NUEVO","estado":"observado"
python -c "import json;d=json.load(open('slot-bajo.redes',encoding='utf-8'));print([(n['code'],n['estado']) for n in d['nodes'] if n['code'] in ('PZ-001','PZ-NUEVO')])"
#   -> [('PZ-001','observado'), ('PZ-NUEVO','pendiente')]   <-- mutó el objeto equivocado
```

### R8 · D-08 — código duplicado nodo/arco deja el arco inalcanzable
```powershell
@'
{"schema":"redes.plan/1","proyecto":{"nombre":"dup","crs":"EPSG:32717"},"objetos":[
 {"tipo":"nodo","codigo":"M1","familia":"saneamiento.pozo","subtipo":"inspeccion","e":760000.0,"n":9796000.0,"cota":2498.0},
 {"tipo":"nodo","codigo":"M2","familia":"saneamiento.pozo","subtipo":"inspeccion","e":760050.0,"n":9796000.0,"cota":2497.8},
 {"tipo":"arco","codigo":"M1","familia":"saneamiento.tuberia","subtipo":"colector_sanitario","desde":"M1","hasta":"M2","diametro_mm":200}]}
'@ | Set-Content -Encoding utf8 plan-dup.json
& $R proyecto nuevo --nombre dup --salida dup.redes --quiet
& $R -p dup.redes importar plan --archivo plan-dup.json --quiet       # ok:true, 3 objetos
& $R -p dup.redes objeto aprobar --codigo M1 --quiet                  # aprueba el NODO
& $R -p dup.redes objeto aprobar --codigo M1 --quiet                  # otra vez el nodo: no-op
& $R -p dup.redes proyecto aprobar                                    # ok:false "Quedan 2 objetos sin revisar" para siempre
```

### R9 · D-09 — GeoJSON en grados sin CRS
```powershell
@'
{"type":"FeatureCollection","features":[
 {"type":"Feature","geometry":{"type":"Point","coordinates":[-78.5123,-0.2045]},"properties":{"familia":"saneamiento.pozo","codigo":"G-1"}},
 {"type":"Feature","geometry":{"type":"Point","coordinates":[-78.5120,-0.2045]},"properties":{"familia":"saneamiento.pozo","codigo":"G-2"}}]}
'@ | Set-Content -Encoding utf8 grados.geojson
& $R proyecto nuevo --nombre geo --salida geo.redes --quiet
& $R -p geo.redes importar geojson --archivo grados.geojson   # ok:true, descartados:0
& $R -p geo.redes proyecto info                                # bbox [-78.5123,-0.2045,…], cota_min=cota_max=0.0
```

### R10 · D-10 — el plan pisa el CRS del proyecto
```powershell
@'
{"schema":"redes.plan/1","proyecto":{"nombre":"otro","crs":"EPSG:32617"},"objetos":[
 {"tipo":"nodo","codigo":"Q1","familia":"saneamiento.pozo","subtipo":"inspeccion","e":760000.0,"n":9796000.0,"cota":2498.0}]}
'@ | Set-Content -Encoding utf8 plan-crs.json
Copy-Item base.redes crs.redes
& $R -p crs.redes importar plan --archivo plan-crs.json --quiet
python -c "import json;d=json.load(open('crs.redes',encoding='utf-8'));print(d['crs'],d['terrain']['epsg'])"   # EPSG:32617 32717
```

### R11 · D-11 — bloques DXF y capas desconocidas
```powershell
@'
0
SECTION
2
BLOCKS
0
BLOCK
2
POZO_TIPO
10
0.0
20
0.0
0
POINT
8
POZOS
10
0.0
20
0.0
30
0.0
0
ENDBLK
0
ENDSEC
0
SECTION
2
ENTITIES
0
INSERT
8
POZOS
2
POZO_TIPO
10
760100.0
20
9796100.0
0
POINT
8
TEXTOS
10
759500.0
20
9795500.0
30
0.0
0
LINE
8
CURVAS-NIVEL
10
759000.0
20
9795000.0
30
0.0
11
761000.0
21
9797000.0
31
0.0
0
ENDSEC
0
EOF
'@ | Set-Content -Encoding ascii bloques.dxf
& $R proyecto nuevo --nombre d --salida d.redes --quiet
& $R -p d.redes importar dxf --archivo bloques.dxf
#   -> nodos:4, arcos:1, descartados:0; PZ-001 en (0,0) (¡el BLOCK!), no hay nodo en (760100,9796100)
```

### R12 · D-12 — terreno colineal ⇒ cotas 0
```powershell
@'
{"schema":"redes.plan/1","proyecto":{"nombre":"col","crs":"EPSG:32717"},
 "terreno":[[760000.0,9796000.0,2500.0],[760100.0,9796000.0,2501.0],[760300.0,9796000.0,2502.0]],
 "objetos":[{"tipo":"nodo","codigo":"P1","familia":"saneamiento.pozo","subtipo":"inspeccion","e":760100.0,"n":9796000.0}]}
'@ | Set-Content -Encoding utf8 plan-colineal.json
& $R proyecto nuevo --nombre c --salida c.redes --quiet
& $R -p c.redes importar plan --archivo plan-colineal.json   # ok:true, puntos_terreno:3, descartados:0
python -c "import json;d=json.load(open('c.redes',encoding='utf-8'));print(len(d['terrain']['tris']),[ (n['z_terrain'],n['z_invert']) for n in d['nodes']])"  # 0 [(0.0,-1.5)]
```

### R13 · D-13 — `--dry-run` escribe
```powershell
& $R -p base.redes exportar dxf     --salida dry.dxf     --dry-run --quiet
& $R -p base.redes exportar geojson --salida dry.geojson --dry-run --quiet
& $R -p base.redes exportar gltf    --salida dry.glb     --dry-run --quiet
& $R -p base.redes vista            --salida dry.svg     --dry-run --quiet
Get-ChildItem dry.* | Select-Object Name,Length      # los 4 existen (exit 0 en todos)
& $R -p base.redes importar plan --archivo NO-EXISTE.json --dry-run   # ok:true, bytes:0
```

### R14 · D-14 — códigos de salida
```powershell
& $R -p no-existe.redes proyecto info           ; echo $LASTEXITCODE   # 7  (norma: 4)
& $R -p base.redes objeto aprobar --codigo X    ; echo $LASTEXITCODE   # 4  (norma: 2)
# schema ajeno: exit 4 (norma: 6 para E_PROJ_SCHEMA_TOO_NEW)
python -c "import json;d=json.load(open('base.redes',encoding='utf-8'));d['schema']='otro.formato/9';json.dump(d,open('otro.redes','w',encoding='utf-8'))"
& $R -p otro.redes proyecto info | Select-String '"code"|"exit_code"'
```

### R15 · D-15 — error de E/S rotulado como «no encontrado»
```powershell
& $R -p . proyecto info            # "E_IO_NOT_FOUND: Access is denied. (os error 5)"
python -c "b=open('base.redes','rb').read();open('no-utf8.redes','wb').write(b[:100]+b'\xff'+b[100:])"
& $R -p no-utf8.redes proyecto info   # "E_IO_NOT_FOUND: stream did not contain valid UTF-8"
```

### R16 · D-16 — clap sin sobre JSON
```powershell
& $R -p base.redes objeto aprobar    # exit 2, stderr "error: the following required arguments were not provided", stdout vacío
```

### R17 · D-17 — diámetro como float
```powershell
@'
{"schema":"redes.plan/1","proyecto":{"nombre":"fl","crs":"EPSG:32717"},"objetos":[
 {"tipo":"nodo","codigo":"A","familia":"saneamiento.pozo","subtipo":"inspeccion","e":0.0,"n":0.0,"cota":1.0},
 {"tipo":"nodo","codigo":"B","familia":"saneamiento.pozo","subtipo":"inspeccion","e":10.0,"n":0.0,"cota":0.8},
 {"tipo":"arco","codigo":"C1","familia":"saneamiento.tuberia","subtipo":"colector_sanitario","desde":"A","hasta":"B","diametro_mm":200.0}]}
'@ | Set-Content -Encoding utf8 plan-float.json
& $R proyecto nuevo --nombre fl --salida fl.redes --quiet
& $R -p fl.redes importar plan --archivo plan-float.json   # exit 7 E_FMT_PARSE: invalid type: floating point 200.0, expected u32
```

### R18 · D-18 — nombres de capa DXF
```powershell
& $R proyecto catalogo --salida cat.redes --quiet
& $R -p cat.redes exportar dxf --salida cat.dxf --quiet
Select-String -Path cat.dxf -Pattern 'TUBER|DESCARGA|C.MARA' | Select-Object -First 6
#   capas: CL_TUBERÍA, CM_CÁMARA, DS_DESCARGA_/_VERTEDERO, HT_HITO_/_BM, TQ_TANQUE_/_RESERVORIO
#   23 de 53 capas con bytes no ASCII; tabla LAYER declarada con 70 0
```

### R19 · D-19 — GeoJSON ignora `cota`/`estado` y no deja `fuente`
```powershell
@'
{"type":"FeatureCollection","features":[{"type":"Feature","geometry":{"type":"Point","coordinates":[760000.0,9796000.0]},"properties":{"familia":"saneamiento.pozo","codigo":"G1","cota":2500.5,"estado":"aprobado"}}]}
'@ | Set-Content -Encoding utf8 geo-cota.geojson
& $R proyecto nuevo --nombre gc --salida gc.redes --quiet
& $R -p gc.redes importar geojson --archivo geo-cota.geojson --quiet
python -c "import json;d=json.load(open('gc.redes',encoding='utf-8'));print('fuentes',d['fuentes'],'zt',d['nodes'][0]['z_terrain'],'estado',d['nodes'][0]['estado'])"
#   -> fuentes [] zt 0.0 estado pendiente
```

### R20/R21/R22/R26/R27 · D-20, D-21, D-22, D-26, D-27 — skill de ingesta
```powershell
@'
[JUNCTIONS]
J1  2498.50  2.0
J2  2498.10  2.1
[CONDUITS]
C1  J1  J2  40.0  0.013  0.30  0.10
[STORAGE]
ST1  2496.5  1.5  0.0  FUNCTIONAL  100  0  0
[COORDINATES]
J1  760000.0  9796000.0
J2  760040.0  9796000.0
'@ | Set-Content -Encoding utf8 casos.inp
python ..\..\skills\redes-ingesta\scripts\inp_to_plan.py --tipo swmm casos.inp --salida plan-casos.json
python -c "import json;d=json.load(open('plan-casos.json',encoding='utf-8'));print([(o.get('codigo'),o.get('diametro_mm'),o.get('e')) for o in d['objetos']])"
#   -> C1 con diametro_mm 300 (viene de InOffset 0.30, no de [XSECTIONS]); nodos con e=n=0.0 pese a [COORDINATES]
#   -> STORAGE no aparece y no hay aviso de sección ignorada
@'
[TITLE]
nada
[OTRA_COSA]
1 2 3
'@ | Set-Content -Encoding utf8 raro.inp
python ..\..\skills\redes-ingesta\scripts\inp_to_plan.py --tipo swmm raro.inp --salida plan-raro.json   # exit 0, "plan: 0 nodos, 0 arcos"
Copy-Item base.redes vacio-import.redes
& $R -p vacio-import.redes importar plan --archivo plan-raro.json --quiet
python -c "import json;d=json.load(open('vacio-import.redes',encoding='utf-8'));print(d['name'],d['revision'],len(d['fuentes']))"
#   -> 'raro' 2 3   (renombró el proyecto y añadió una fuente con 0 objetos importados)
```
EPANET (D-26):
```powershell
@'
[JUNCTIONS]
N1  100.0  5.0
[RESERVOIRS]
R1  120.0
[PIPES]
P1  R1  N1  120.0  200  130  0  OPEN
'@ | Set-Content -Encoding utf8 epanet.inp
python ..\..\skills\redes-ingesta\scripts\inp_to_plan.py --tipo epanet epanet.inp --salida plan-epa.json
python -c "import json;d=json.load(open('plan-epa.json',encoding='utf-8'));print([(o['familia'],o.get('subtipo')) for o in d['objetos']])"
#   -> ('saneamiento.camara','valvulas')  ('agua.tanque','apoyado')  ('agua.tuberia','distribucion')
```

### R23 · D-23 — plan vacío renombra el proyecto (misma receta que R22, bloque `vacio-import`).

### R24 · D-24 — rol topológico
```powershell
@'
{"schema":"redes.plan/1","proyecto":{"nombre":"rol","crs":"EPSG:32717"},"objetos":[
 {"tipo":"nodo","codigo":"A","familia":"saneamiento.pozo","subtipo":"inspeccion","e":0.0,"n":0.0,"cota":1.0},
 {"tipo":"nodo","codigo":"B","familia":"saneamiento.pozo","subtipo":"inspeccion","e":10.0,"n":0.0,"cota":0.8},
 {"tipo":"arco","codigo":"ARCO-POZO","familia":"saneamiento.pozo","subtipo":"inspeccion","desde":"A","hasta":"B"}]}
'@ | Set-Content -Encoding utf8 plan-rol.json
& $R proyecto nuevo --nombre r --salida r.redes --quiet
& $R -p r.redes importar plan --archivo plan-rol.json   # ok:true, arcos:1, descartados:0
python -c "import json;d=json.load(open('r.redes',encoding='utf-8'));print([(l['code'],l['family']) for l in d['links']])"
```

### R25 · D-25 — `entorno` parcial
```powershell
python -c "import json;d=json.load(open('base.redes',encoding='utf-8'));d['entorno']={'calles':[{'nombre':'C1','eje':[{'x':0.0,'y':0.0,'z':0.0}],'ancho':6.0}]};json.dump(d,open('entorno-parcial.redes','w',encoding='utf-8'))"
& $R -p entorno-parcial.redes proyecto info   # exit 7 E_FMT_PARSE: missing field `manzanas`
```

### R28 · D-29 — artefacto fantasma en dry-run
```powershell
& $R proyecto nuevo --nombre x --salida nodry.redes --dry-run
#   -> artifacts[0] = {kind:"proyecto", path:"nodry.redes", bytes:0}; el archivo no existe
Test-Path nodry.redes   # False
```

---

## Apéndice B — lo que probé y **no** falló

Para que el Consejo sepa dónde no hace falta gastar esfuerzo:

* **JSON truncado**: `E_FMT_PARSE: EOF while parsing a value at line 14758 column 0`, exit 7, sin pánico.
* **`.redes` sin `terrain` ni `entorno`**: carga bien (`Option` con `default`), exit 0.
* **Guardado**: las ~40 escrituras de esta revisión produjeron siempre un archivo completo y parseable (temporal + `sync_all` + `rename`).
* **`1e400` en un `.redes`**: `serde_json` lo rechaza (`number out of range`) ⇒ no se puede inyectar infinito por el archivo de proyecto.
* **glTF de proyecto vacío**: GLB de 352 bytes con cabecera `glTF` v2 válida, `primitivas: 0`, `ok:true` (un glTF sin triángulos es válido).
* **IDs y revisión en el uso normal**: `alloc` es monótono y `commit` incrementa la revisión (el problema es solo la entrada de archivos no validados, D-07/D-08).
