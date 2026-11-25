# DXF de terceros (Civil 3D, AutoCAD, BricsCAD) → `redes.plan/1`

Un DXF **escrito por Redes** (tiene textos en la capa `REDES_INFO`) no pasa por aquí: entra
directo con `redes importar dxf --archivo x.dxf`. Esta referencia es para DXF de otros.

## 0. Antes de leer geometría: pide algo mejor

Un DXF es dibujo, no datos: las cotas y diámetros suelen estar en etiquetas, no en las entidades.
Pregunta primero si el usuario puede exportar desde Civil 3D:

1. **LandXML** de la red (`Salida → Exportar a LandXML`): `<PipeNetwork>` con `<Struct name
   elevRim elevSump>` (`<Center>` en **N E**, norte primero), `<Invert elev refPipe flowDir>`,
   `<Pipe refStart refEnd>` con `<CircPipe diameter thickness>`. Trae solera por tubo y por
   estructura: es la mejor fuente.
2. **Informes de tuberías y estructuras** (Toolspace → *Reports Manager*, CSV/Excel): sigue
   `tablas.md`.
3. Si solo hay DXF, sigue aquí.

## 1. Leer el DXF

- ASCII: pares de líneas *código de grupo* / *valor*. Si empieza por `AutoCAD Binary DXF`, pide
  guardarlo como ASCII. Un DWG no se lee (D1): pide DXF.
- `HEADER`: `$INSUNITS` (1 pulgadas, 2 pies, 4 mm, 6 m), `$DWGCODEPAGE` (`ANSI_1252` para capas
  con tildes), `$EXTMIN/$EXTMAX` (¿parecen UTM?).
- `TABLES/LAYER`: lista de capas (código 2). Léela entera antes de mapear.
- `BLOCKS`: definiciones de bloque; `ENTITIES`: lo dibujado.

| Entidad | Códigos útiles | Uso |
|---|---|---|
| `INSERT` | `2` nombre del bloque, `8` capa, `10/20/30` punto de inserción, `41/42/43` escala, `50` rotación, `66 = 1` → siguen `ATTRIB` hasta `SEQEND` | **estructuras** (pozos, sumideros, válvulas, hidrantes) |
| `ATTRIB` | `2` etiqueta (`CODIGO`, `CT`, `CF`, `PROF`, `DIAM`), `1` valor | datos de la estructura: los mejores que tiene el DXF |
| `POLYLINE` + `VERTEX` … `SEQEND` | `70` con bit 8 = polilínea 3D; `10/20/30` por vértice | **tramos** con perfil |
| `LWPOLYLINE` | `10/20` repetidos, `38` elevación única, `70` bit 1 cerrada | tramos en planta (Z constante: no es perfil) |
| `LINE` | `10/20/30` y `11/21/31` | tramo recto |
| `POINT`, `CIRCLE` (`40` radio) | `10/20/30` | símbolos de nodo; el círculo no dice qué cota es |
| `TEXT`, `MTEXT` (`1` contenido) | `10/20/30` | etiquetas («PZ-12», «CT 2512.40», «Ø250 PVC», «S=1.2 %») |

Trampas de lectura: con extrusión (`210/220/230`) distinta de (0,0,1) las coordenadas de
`LWPOLYLINE`, `CIRCLE` e `INSERT` están en el sistema del objeto (bloques en espejo); los bloques
anidados y dinámicos (`*U…`) cambian de nombre; los objetos AECC de Civil 3D salen como proxy o
desaparecen: pide `EXPORTTOAUTOCAD` o LandXML.

## 2. Qué capa es qué

Mapea por **palabra completa** de la capa (separa por `-`, `_`, espacio, `.`), no por subcadena,
y por **tipo de entidad**: un punto o bloque solo puede dar una familia de nodo; una polilínea,
una de tramo. Presenta el mapeo capa → familia:subtipo al usuario y que lo confirme.

### Capas NCS (EE. UU., en inglés)

`Disciplina-Grupo-Detalle[-Estado]`: `C-SSWR-*` alcantarillado sanitario, `C-STRM-*` pluvial,
`C-WATR-*` agua potable, `C-FIRE-*` contra incendios. Detalle habitual: `PIPE` tubería, `STRC`
estructuras, `MHOL` pozos, `INLT` sumideros, `VALV` válvulas, `HYDT` hidrantes, `CULV`
alcantarillas, `TEXT`/`ANNO`/`IDEN` etiquetas, `PROF` perfil. Estado: `-N` nuevo, `-E` existente,
`-D` a demoler, `-F` futuro. **Pregunta si se importa lo existente.**

| Capa | Entidad | Familia:subtipo propuesta |
|---|---|---|
| `C-SSWR-PIPE` | polilínea | `saneamiento.tuberia:colector_sanitario` |
| `C-STRM-PIPE` | polilínea | `saneamiento.tuberia:colector_pluvial` |
| `C-STRM-CULV` | polilínea | `saneamiento.canal:box_culvert` o tubería: pregunta |
| `C-SSWR-STRC`, `C-SSWR-MHOL`, `C-STRM-MHOL` | bloque/punto | `saneamiento.pozo:inspeccion` |
| `C-STRM-INLT`, `C-STRM-STRC` con bloque de rejilla | bloque | `saneamiento.sumidero:<pregunta>` |
| `C-WATR-PIPE` | polilínea | `agua.tuberia:<pregunta>` |
| `C-WATR-VALV` | bloque | `agua.valvula:<pregunta>` |
| `C-WATR-HYDT`, `C-FIRE-HYDT` | bloque | `agua.hidrante:columna` (pregunta) |

### Capas en español (Latinoamérica, España)

| Palabra en la capa | Entidad | Familia:subtipo propuesta | Ojo |
|---|---|---|---|
| `POZO`, `PZ`, `MH`, `BZ`/`BUZON` (Perú), `REGISTRO` | bloque/punto | `saneamiento.pozo:inspeccion` | `POZO` en agua puede ser `agua.captacion:pozo_profundo` |
| `ALC`, `ALCANT`, `SAN`, `AASS`, `AS`, `COLECTOR`, `CL` | polilínea | `saneamiento.tuberia:colector_sanitario` | — |
| `PLUV`, `AALL`, `AL`, `DREN` | polilínea | `saneamiento.tuberia:colector_pluvial` | — |
| `EMISARIO`, `EMIS` | polilínea | `saneamiento.tuberia:emisario` | — |
| `DESCARGA`, `VERT`, `DESC` | bloque/punto | `saneamiento.descarga:<pregunta>` | — |
| `SUMIDERO`, `IMBORNAL`, `REJILLA`, `SM` | bloque/punto | `saneamiento.sumidero:<pregunta>` | `REJILLA` en una planta es `comun.reja` |
| `CAMARA`, `CAM`, `CM` | bloque/punto | **pregunta**: `agua.camara` (válvulas, quiebre…), `saneamiento.camara` o `saneamiento.caja` (en Chile, «cámara» es el pozo) | **no es** `saneamiento.pozo` sin confirmar |
| `AP`, `AAPP`, `AGUA`, `ACUEDUCTO`, `DIST`, `RED` | polilínea | `agua.tuberia:<pregunta>` | en puntos no da familia de tramo |
| `CONDUCCION`, `ADUCCION`, `IMPULSION` | polilínea | `agua.tuberia:conduccion` / `aduccion` / `impulsion` | — |
| `VALV`, `VALVULA`, `VL` | bloque/punto | `agua.valvula:<pregunta>` | — |
| `HIDR`, `HIDRANTE`, `HYD`, `GRIFO` | bloque/punto | `agua.hidrante:<pregunta>` | **nodo**, nunca `agua.tuberia` |
| `TANQUE`, `TQ`, `RESERVORIO`, `DEPOSITO` | bloque/polígono | `agua.tanque:<pregunta>` | — |
| `BOCATOMA`, `BARRAJE`, `AZUD`, `CAPTACION` | bloque/polígono | `agua.bocatoma:<pregunta>` | — |
| `DESAREN` | — | `comun.desarenador` | — |
| `FILTRO`, `FLOCUL`, `SEDIMENT`/`DECANT` | — | `agua.filtro`, `agua.floculador`, `agua.sedimentador` | `DECANT` en saneamiento: `saneamiento.sedimentador` |
| `LAGUNA`, `PTAR`, `REACTOR` | — | `saneamiento.laguna`, `saneamiento.reactor` | — |
| `COMPUERTA` | bloque | `comun.compuerta` | — |
| `TEXTO`, `TXT`, `COTA`, `ETIQ`, `PERFIL`, `EJE`, `VIA`, `MANZANA`, `LOTE` | — | no son red | — |

Corrige estos fallos de la antigua heurística de `ingest.rs` (`family_for_layer`): comparaba en
mayúsculas contra `"BARRaje"` (nunca coincidía); `AP` como subcadena casaba con `TAPA`, `CAPA`,
`MAPA` y `HIDR` daba `agua.tuberia` **también a puntos**; `CAMARA` iba siempre a pozo sanitario;
una capa desconocida acababa en pozo o colector por defecto. Aquí: palabra completa, nodo o tramo
según la entidad, y lo que no se reconoce **se pregunta o se lista**, no se adivina.

## 3. Estructuras (nodos)

- Un `INSERT` por estructura: `e`, `n` = punto de inserción (comprueba que el bloque tiene el
  origen en el centro: si el símbolo está desplazado, pregunta).
- Código, cotas y diámetro salen de los `ATTRIB` (o del texto más cercano **solo** si el usuario
  confirma que esa etiqueta es de ese pozo). Etiquetas típicas: `CT`/`COTA_TAPA` (tapa),
  `CF`/`CS`/`COTA_FONDO` (solera), `CR` (rasante), `H`/`PROF` (profundidad), `DIAM`/`D`.
- La Z del punto de inserción no se sabe qué es (tapa, solera o 0): no la uses sin confirmar.
- Escala de bloque ≠ diámetro: no derives `diametro` de `41/42`.

## 4. Tramos (arcos)

- Polilínea 3D: sus vértices dan el trazado. Su Z puede ser **solera, eje o clave**: compárala
  con una etiqueta de solera o una cota de tapa conocida (Z − solera ≈ DI/2 → eje; ≈ DI →
  clave). Convierte a solera (`plan.md` §4) solo con el DI declarado y dilo; si no se puede
  saber, no pongas Z (sin `vertices`) y usa `cota_solera_*` de las etiquetas.
- `LWPOLYLINE` y `LINE` con Z 0 o constante: solo planta.
- **Extremos a nodos.** En Civil 3D el tubo termina en la pared de la estructura, no en su
  centro: busca la estructura cuyo centro esté a ≤ radio + 0,5 m del extremo. Si no hay ninguna o
  hay dos, no crees un nodo implícito: lista el tramo y pregunta.
- Sentido: el del dibujo no siempre es el del flujo. Si las etiquetas de solera dicen lo
  contrario, invierte `desde`/`hasta` y los vértices, y dilo.
- Diámetro y material del texto del tramo («Ø250 PVC»): `diametro_mm` = DN; si la etiqueta dice
  DI, va a `diametro_interior_mm`.

## 5. Coordenadas

Civil 3D trabaja en coordenadas reales: E en cientos de miles y N en millones (UTM sur). Valores
pequeños (0–10 000) son coordenadas locales: pide el sistema o los puntos de amarre. Pregunta
siempre el EPSG si el dibujo no lo dice (el `.prj` o el *Drawing Settings* de Civil 3D lo saben).
