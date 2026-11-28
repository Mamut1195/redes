# Topografía de levantamiento → `redes.topografia/1`

Derivado de `crates/redes-core/src/survey.rs` (`SurveyFile`, `ElevationGrid`, `import_survey`) y
`terrain.rs` (`SurveyPoint`, `PointClass`, `VerticalDatum`, `Terrain::triangulate`). Si el código
cambia, manda el código: vuelve a leerlo. Decisión: D18 (`docs/22-decisiones.md`).

Tú lees el levantamiento (estación total, GNSS, Civil 3D, un MDT), **clasificas cada punto por
su descripción**, armas las líneas de quiebre, el contorno y los huecos, y escribes el archivo.
La app no traduce ni clasifica: valida, triangula (Delaunay restringido) y guarda la descripción
original y la clase de cada punto. **No inventes, no corrijas y no reclasifiques en silencio:
lo dudoso se pregunta.**

## 1. Contrato

```json
{
  "schema": "redes.topografia/1",
  "nombre": "levantamiento-quebrada",
  "crs": "EPSG:32717",
  "datum_vertical": "no_declarado",
  "fuente": "Leica TS06 (lev_2026-11-20.txt)",
  "puntos": [
    {"numero": "1",  "e": 760005.0, "n": 9796005.0, "z": 2503.70, "descripcion": "TN",  "clase": "terreno"},
    {"numero": "81", "e": 760050.0, "n": 9796002.0, "z": 2497.04, "descripcion": "QDA", "clase": "quiebre"},
    {"numero": "122","e": 760000.0, "n": 9796000.0, "z": 2504.00, "descripcion": "LIM", "clase": "contorno"},
    {"numero": "130","e": 760015.0, "n": 9796070.0, "z": 2504.40, "descripcion": "EDI CASA", "clase": "excluido"}
  ],
  "quiebres": [ {"descripcion": "QDA", "puntos": ["81", "82", "83", "84", "85"]} ],
  "contorno": ["122", "123", "124", "125", "126", "127", "128", "129"],
  "huecos":   [ {"descripcion": "EDI CASA", "puntos": ["130", "131", "132", "133"]} ],
  "malla": {
    "e_centro_noroeste": 759000.0, "n_centro_noroeste": 9797000.0, "celda_m": 30.0,
    "filas": 40, "columnas": 50, "sin_dato": -32767, "valores": [2510.2, 2511.0],
    "fuente": "Copernicus GLO-30 (Copernicus_DSM_COG_10_S02_00_W079_00_DEM.tif)"
  }
}
```

| Campo | Tipo | Regla |
|---|---|---|
| `schema` | texto | **obligatorio**, exactamente `redes.topografia/1`; otra versión → `E_FMT_VERSION`, otro esquema o ausente → `E_FMT_UNSUPPORTED` |
| `nombre` | texto | **obligatorio** (puede ser `""`: toma el nombre del archivo) |
| `crs` | texto | **obligatorio**: `EPSG:327NN` (UTM sur) o `EPSG:326NN` (UTM norte), WGS 84, metros. El mismo del proyecto. Redes no reproyecta (§5) |
| `datum_vertical` | texto | **obligatorio**: `egm2008`, `egm96`, `elipsoidal` o `no_declarado` (§5). Nunca lo supongas |
| `fuente` | texto | instrumento o programa y archivo de origen |
| `puntos[]` | lista | todos los puntos del levantamiento, **también los excluidos** |
| `puntos[].numero` | texto | el número del origen, tal cual y **como texto** (`"1001"`, `"P12A"`). Único. Si el origen no numera (vértices de una polilínea), créalo trazable: `"PL-QDA-3"` |
| `puntos[].e`, `n`, `z` | número | metros del CRS y del datum. JSON no admite `NaN`: un punto sin cota no entra (lístalo) |
| `puntos[].descripcion` | texto | la descripción/código **original**, sin traducir ni recortar (`"BV1 INI"`) |
| `puntos[].clase` | texto | `terreno`, `quiebre`, `contorno` o `excluido` (§4) |
| `quiebres[]` | lista | líneas de quiebre: `descripcion` (la del origen) y `puntos` (números en orden, ≥ 2) |
| `contorno` | lista | números del borde exterior en orden (horario o antihorario); el cierre es implícito (repetir el primero al final también vale) |
| `huecos[]` | lista | polígonos de exclusión (edificaciones, lagunas): `descripcion` y `puntos` (≥ 3) |
| `malla` | objeto | opcional: MDT en rejilla regular ya proyectado (§3.4) |

Un campo que no está en la tabla → `E_FMT_CAMPO`. Una clase o un datum fuera de la lista →
`E_FMT_PARSE`.

### Qué valida la app (y qué código da)

| Código | Cuándo |
|---|---|
| `E_TOPO_PUNTO` | punto sin número o número repetido |
| `E_TOPO_COINCIDENTES` | dos puntos que entran al TIN en la misma planta (±1 mm) con cotas distintas (> 1 mm). Misma cota: entra una vez y se avisa |
| `E_TOPO_REFERENCIA` | una cadena cita un número que no existe, o una línea de quiebre cita un punto que no es `quiebre` ni `contorno` |
| `E_TOPO_GEOMETRIA` | línea de quiebre con < 2 puntos o con un punto repetido seguido; polígono con < 3 puntos distintos o que se corta a sí mismo; hueco fuera del contorno; dos líneas de quiebre (o una y el contorno) que se cruzan fuera de un punto común |
| `E_TOPO_MALLA` | `filas × columnas ≠` número de valores, `celda_m` no positiva, malla demasiado grande |
| `E_TOPO_VACIA` | menos de 3 puntos entran al TIN, o no queda ningún triángulo |
| `E_CRS` | CRS ausente, no UTM WGS 84, distinto del proyecto, o coordenadas en grados |
| `E_DATUM` | `datum_vertical` declarado distinto del de la red (plan) del proyecto |

Avisos (no fallan): sin contorno (el TIN cubre el casco convexo y en las concavidades no hay
medida), puntos `quiebre` fuera de toda línea, puntos `contorno` fuera de todo polígono,
duplicados exactos, datum no declarado frente a uno declarado.

### Qué hace la app con el archivo

- Entran al TIN los puntos `terreno`, `quiebre` y `contorno` (y los nodos de la malla, §3.4).
  Los `excluido` se guardan con su número y descripción pero **no tocan la superficie**.
- Las líneas de quiebre son **aristas obligadas**: ningún triángulo las cruza.
- Contorno y huecos recortan el TIN. Si todos sus vértices entran al TIN, sus lados son aristas
  obligadas y el recorte es exacto. Si alguno es `excluido` (las esquinas de una casa), el polígono
  recorta por solape: se quita todo triángulo que lo pise, así que el hueco queda un poco más
  grande que el polígono y **nunca** se interpola dentro.
- Fuera del contorno y dentro de un hueco no hay cota (`—`): no se extrapola.

## 2. Flujo

1. Identifica el formato (§3) y lee el archivo entero.
2. Averigua **CRS y datum vertical** (§5). Si no constan, pregúntalos.
3. Lee las columnas o entidades y arma `puntos` con la descripción original.
4. Clasifica cada descripción con la tabla de códigos (§4) **ajustada con el usuario**: muéstrale
   los códigos distintos que encontraste, cuántos puntos tiene cada uno y la clase que propones.
5. Arma `quiebres`, `contorno` y `huecos` (§4.2).
6. Escribe el archivo y valida en un proyecto de prueba:
   `redes -p out\prueba.redes --dry-run importar topografia --archivo topografia.json`; corrige con
   los diagnósticos y repite; luego sin `--dry-run` en el proyecto real.
7. Mira el resultado: `redes -p <proyecto> vista --salida out\vista.svg` o la app
   (`--snap out\topo.png --scene iso`). Una quebrada rellena o triángulos sobre un patio dicen que
   falta un quiebre o un contorno.
8. Informa (§8).

## 3. Lectura por formato

### 3.1 CSV/TXT de estación total o GNSS (PNEZD, PENZD, NEZ, ENZ…)

Una fila por punto; separador `,` `;` tabulador o espacios; a veces cabecera.

| Orden | Columnas | Quién lo usa |
|---|---|---|
| PNEZD | número, **norte**, este, cota, descripción | Civil 3D por defecto, Leica, Topcon, la mayoría de libretas |
| PENZD | número, **este**, norte, cota, descripción | Trimble, algunos colectores, Excel |
| NEZ / NEZD | norte, este, cota (descripción) | exportes sin número |
| ENZ / XYZ | este, norte, cota | nubes, MDT exportados |

**El orden de columnas no se adivina por costumbre.** Compruébalo con los valores:

- En UTM, **E** va de ~166 000 a ~834 000 (6 cifras). **N** sur va de ~1 000 000 a 10 000 000
  (7 cifras; Ecuador continental ~9 500 000–9 990 000). Si una columna tiene 7 cifras y la otra 6,
  la de 7 es N **en el hemisferio sur**.
- En el **norte**, cerca del ecuador, N también tiene 6 cifras (Colombia, Venezuela,
  Centroamérica: N 200 000–1 500 000): E y N **no se distinguen por magnitud**. Pregunta.
- El **número de punto** suele ser entero y correlativo (1, 2, 3…); la **cota** suele estar en
  cientos o miles con 2–3 decimales y varía poco entre puntos vecinos.
- Si hay cabecera (`PUNTO,NORTE,ESTE,COTA,DESC`), úsala, pero contrástala con los valores.
- Ninguna regla resuelve el caso → **pregunta** mostrando 3 filas y tu propuesta.

Trampas:

- Una descripción con espacios (`EDI CASA 3`) en un archivo separado por espacios: la
  descripción es **todo lo que queda** tras la cota.
- Descripciones con atributos o comentarios (`BV1 INI`, `PZ T=2.35`): conserva el texto entero en
  `descripcion`; para clasificar usa el código inicial.
- Decimales con coma (`2503,70`) en archivos con `;`: conviértelos a punto.
- Puntos de control repetidos (BM, estaciones) con la misma planta y otra cota: son excluidos; si
  un punto de terreno repite planta con otra cota, la CLI da `E_TOPO_COINCIDENTES`: **pregunta**
  cuál vale, no elijas.
- Puntos con cota 0 o −9999: falta la cota. No entran; lístalos.

### 3.2 DXF (topografía dibujada)

- **POINT** con Z: un punto por entidad; la descripción suele estar en la **capa** o en un TEXT
  cercano. Numera `DXF-<capa>-<n>` si no hay número.
- **INSERT** de bloque de punto (Civil 3D «COGO» explotado, bloques `PUNTO`, `PT`): la Z es la del
  punto de inserción (código 30) o un atributo `COTA`/`ELEV`/`Z`; número y descripción en
  atributos (`NUMERO`, `PUNTO`, `DESC`, `CODIGO`). Si la Z de inserción es 0 y hay atributo de
  cota, manda el atributo; si no hay ninguno, el punto no tiene cota.
- **POLYLINE 3D** (`AcDb3dPolyline`) o **LWPOLYLINE con elevación**: candidatas a línea de quiebre;
  la capa dice qué son (`BORDE_VIA`, `QUEBRADA`, `TALUD_PIE`). Cada vértice es un punto `quiebre`
  numerado `PL-<capa>-<polilínea>-<vértice>` con `descripcion` = capa.
- Polilíneas 2D sin Z (curvas de nivel con elevación constante, código 38): **no** son quiebres
  medidos; hoy no hay entrada de curvas de nivel. Pregunta antes de convertirlas en puntos.
- Polilínea cerrada en capa `LIMITE`, `LINDERO`, `AREA`: candidata a `contorno`; en capa
  `EDIFICACION`, `CASA`, `LAGUNA`: candidata a `huecos`.
- TEXT/MTEXT con cotas sueltas: no son puntos fiables; pregunta.

### 3.3 LandXML de Civil 3D

Ejemplo: `crates/redes-core/tests/fixtures/topografia_civil3d.landxml.xml` y su traducción
`topografia_civil3d.topografia.json` (probada en `tests/topografia.rs`).

- `<CoordinateSystem epsgCode="32717" verticalDatum="EGM2008">`: CRS y datum. Si faltan, pregunta.
- `<Units><Metric linearUnit="meter">`; con `Imperial`/`foot` o `USSurveyFoot`, convierte a metros
  (pie = 0,3048 m; pie topográfico EE. UU. = 1200/3937 m) y dilo.
- `<CgPoints><CgPoint name="12" code="BV1" desc="…">N E Z</CgPoint>`: **el orden es norte, este,
  cota**. `name` → `numero`; `code` (o `desc`) → `descripcion`.
- `<Surface><SourceData><Breaklines><Breakline desc="EJE"><PntList3D>N E Z N E Z…`: línea de
  quiebre. Si sus vértices coinciden con CgPoints, cita sus números; si no, crea puntos `quiebre`
  numerados `BRK-<desc>-<n>`.
- `<Boundaries><Boundary bndType="outer">` → `contorno`; `bndType="hide"` o `"void"` → `huecos`
  (`PntList2D` es `N E`; sus vértices sin Z no pueden entrar al TIN: cítalos como puntos
  `excluido` con la Z del TIN de Civil 3D si existe, o pregunta).
- `<Definition surfType="TIN"><Pnts><P id>…` y `<Faces>`: es la superficie **ya triangulada** de
  Civil 3D. Redes triangula de nuevo desde puntos y quiebres; usa los `Pnts` solo si no hay
  CgPoints, como puntos `terreno` numerados `TIN-<id>`, y avisa de que se pierde la triangulación
  original.

### 3.4 MDT en rejilla (Copernicus GLO-30, GeoTIFF, ESRI ASCII)

La app no lee GeoTIFF ni `.asc`: tú conviertes la rejilla a `malla`.

- Proyecta antes a UTM (Copernicus viene en grados WGS 84 con alturas **EGM2008**):
  `gdalwarp -t_srs EPSG:32717 -tr 30 30 -r bilinear dem.tif dem_utm.tif` y recorta a la zona.
- `e_centro_noroeste`, `n_centro_noroeste`: **centro** de la celda de la fila 0, columna 0 (la del
  noroeste). En ESRI ASCII: `xllcorner + celda/2` y `yllcorner + (nrows − 0,5) × celda`. En
  GeoTIFF: origen de la geotransformada + celda/2 (con el signo de la celda).
- `valores`: fila a fila desde el norte; dentro de la fila, de oeste a este. `filas × columnas`
  valores exactos.
- `sin_dato`: el NoData del origen (Copernicus `-32767`; `.asc` `NODATA_value`).
- `datum_vertical`: `egm2008` para Copernicus; si mezclas con un levantamiento GNSS en elipsoidal,
  **no van en el mismo archivo**: pregunta (la app no convierte).
- Regla de mezcla de la app: un nodo de la malla **no entra** si hay un punto del levantamiento o
  una línea de quiebre a menos de una celda, ni si cae fuera del contorno o en un hueco. El
  levantamiento manda donde existe; la malla rellena alrededor.

## 4. Clasificación por código

### 4.1 Tabla de códigos típicos (valores por defecto que el usuario ajusta)

La tabla la aprobó el usuario como **referencia**, no como regla del programa (D18). Cada
levantamiento tiene sus códigos: muéstrale los que encontraste y confirma.

| Códigos típicos | Qué es | `clase` | Además |
|---|---|---|---|
| `TN`, `T`, `PT`, `TERR` | terreno natural | `terreno` | |
| `BV`, `BC`, `BV1`/`BV2`, `BORDE` | borde de vía / de calzada | `quiebre` | línea por código |
| `EJE`, `CL` | eje de vía | `quiebre` | línea por código |
| `CUN` | cuneta (fondo o borde) | `quiebre` | línea por código |
| `PIE`, `CAB` (o `COR`) | pie y cabeza (corona) de talud | `quiebre` | una línea cada uno |
| `QDA`, `CAU`, `RIO` | fondo de quebrada, cauce, río (thalweg) | `quiebre` | línea por código |
| `ORI`, `BOR` | orilla, borde de cauce | `quiebre` | una línea por margen |
| `MUR` | pie de muro | `quiebre` | línea por código |
| `VER`, `ACE` | vereda, acera (borde) | `quiebre` | línea por código |
| `CERCO`, `CER` | cerco (al pie) | `terreno` | se conserva como referencia |
| `ARB`, `POS`, `POSTE` | árbol, poste | `excluido` | |
| `PZ`, `BZ`, `VAL` | pozo, buzón, válvula **existentes** | `excluido` | la tapa no es terreno; no es la red a revisar |
| `BM`, `EST`, `PI` | banco de nivel, estación, punto de inflexión | `excluido` | control, no superficie |
| `EDI`, `CASA` | esquina de edificación | `excluido` | + contorno de hueco |
| `LIM` | límite del levantamiento | `contorno` | vértice del contorno |

Un código que no está en la tabla, o uno de la tabla usado de forma rara (un `PT` a 3 m por
encima de sus vecinos, un `PZ` donde no hay red), **se pregunta**. No lo mandes a `terreno` por
descarte.

### 4.2 De puntos a líneas de quiebre

- **Puntos consecutivos con el mismo código forman una línea**, en el orden en que se tomaron
  (orden del archivo o del número). Un código distinto en medio la corta… salvo que el usuario diga
  que su libreta intercala (p. ej. un `TN` suelto en mitad de un borde).
- **Sufijo numérico = otra línea**: `BV1` y `BV2` son los dos bordes; `ORI1`/`ORI2` las dos
  márgenes; `PIE3` el tercer pie de talud.
- **`INI`/`FIN`** (`BV INI`, `BV FIN`, o `BV` + `I`/`F`): abre y cierra una línea; dos `INI`
  seguidos sin `FIN` = libreta rota, pregunta.
- **Número de cadena** en la descripción (`BV 3`, `QDA-2`): agrupa por cadena; dentro de cada una,
  por orden de toma.
- **`C`/`CERRAR`** al final: la línea se cierra (repite el primer número al final de `puntos`).
- Una línea de un solo punto no existe: el punto queda `quiebre` sin línea (la CLI avisa) o
  pregunta si es `terreno`.
- Dos líneas no pueden **cruzarse** fuera de un punto común (la superficie tendría dos cotas ahí):
  `E_TOPO_GEOMETRIA`. Si el levantamiento las cruza (un borde de vía sobre una quebrada en una
  alcantarilla), pregunta qué tramo manda; no muevas puntos.

### 4.3 Contorno y huecos

- `contorno`: los puntos `LIM` en orden de recorrido. Sin `LIM`, pregunta si hay un límite (un
  lindero en el DXF, el `outer` del LandXML); **no** inventes uno con el casco convexo ni con un
  «buffer». Sin contorno la app cubre el casco convexo y lo avisa.
- `huecos`: las esquinas `EDI`/`CASA` de **cada** edificación, en orden. Si las esquinas de varias
  casas van seguidas con el mismo código, sepáralas por número de cadena o pregunta. Un hueco
  debe quedar dentro del contorno.

## 5. CRS y datum vertical (F17-12)

- **CRS**: `EPSG:327NN` (UTM sur, Ecuador 17S `32717` / 18S `32718`, Perú 17S–19S) o
  `EPSG:326NN` (UTM norte, Colombia 18N `32618`, Venezuela 19N–20N). Todo WGS 84 en metros.
  PSAD56 (`248NN`) o SIRGAS-Ecuador (`EPSG:31977`…) **no** entran: reproyecta en origen
  (`ogr2ogr`, QGIS) y dilo. Un CRS distinto del proyecto → `E_CRS`.
- **Datum vertical**: de qué superficie se miden las cotas.
  - `egm2008`: ortométrico con el geoide EGM2008 (Copernicus, la mayoría de GNSS con geoide
    moderno).
  - `egm96`: ortométrico con EGM96 (SRTM, equipos antiguos).
  - `elipsoidal`: altura sobre el elipsoide WGS 84 (GNSS sin geoide): en Ecuador difiere ~20–30 m
    de la ortométrica.
  - `no_declarado`: el origen no lo dice (estación total con cota arbitraria o un BM sin
    referencia). Es válido y la app lo muestra como «no declarado»; **no** lo cambies por otro
    «probable».
- El plan también puede declararlo (`proyecto.datum_vertical`, `plan.md`). Si plan y topografía
  declaran datums distintos → `E_DATUM`: la app no convierte. Pregunta al usuario y convierte en
  origen (no en el JSON a mano) o pide la entrega en el mismo datum.

## 6. Preguntar (agrupa todo en un mensaje, con tu propuesta)

- Orden de columnas si E y N no se distinguen (§3.1) o la cabecera no cuadra con los valores.
- CRS y datum vertical si no constan.
- La tabla de códigos: cada código encontrado, su número de puntos y la clase propuesta; los
  códigos desconocidos.
- Cómo encadena la libreta (consecutivos, sufijos, `INI`/`FIN`, cadenas).
- Qué es el contorno y qué edificaciones son huecos.
- Coincidentes con distinta cota, puntos sin cota, líneas que se cruzan.
- Si la malla y el levantamiento están en el mismo datum.

## 7. Ejemplo trabajado

`crates/redes-core/tests/fixtures/topografia_quebrada.pnezd.txt` (137 puntos, PNEZD: número,
norte, este, cota, descripción) → `topografia_quebrada.topografia.json`. Se comprueba en
`crates/redes-core/tests/topografia.rs` y con el binario en `crates/redes-cli/tests/topografia.rs`.

| Fila del origen | Lectura |
|---|---|
| `1,9796005.000,760005.000,2503.700,TN` | N = 9 796 005 (7 cifras → norte, hemisferio sur), E = 760 005, Z = 2503,70; `TN` → `terreno` |
| `81,9796002.000,760050.000,2497.040,QDA` … `85` | cinco `QDA` seguidos → una línea `quiebre` «QDA» (fondo de la quebrada) |
| `86`…`94` `ORI1`, `95`…`103` `ORI2` | dos márgenes, una línea cada una |
| `104`…`112` `BV1`, `113`…`121` `BV2` | los dos bordes de la vía |
| `122`…`129` `LIM` | contorno en orden; tiene una muesca al este (sin medida dentro) |
| `130`…`133` `EDI CASA` | `excluido` + un hueco con sus cuatro esquinas |
| `134 ARB`, `135 POSTE`, `136 BM-1`, `137 PZ EXIST` | `excluido` (descripción íntegra) |

CRS: el usuario confirmó `EPSG:32717`; datum: la libreta no lo dice → `no_declarado`.

Con el `importar terreno` antiguo esta misma fila entraba como E = 1, N = 9 796 005, Z = 760 005:
por eso ese comando ya no existe y la topografía solo entra por este contrato.

## 8. Informe al usuario

- origen, orden de columnas, CRS y datum (y quién los confirmó);
- puntos por código y por clase; códigos que se preguntaron y qué se decidió;
- líneas de quiebre (cuántas, de qué códigos), contorno sí/no, huecos;
- puntos que no entraron y por qué (sin cota, coincidentes);
- avisos de la CLI (`detalle` de `importar topografia`);
- la vista generada.
