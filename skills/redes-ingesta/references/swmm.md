# EPA SWMM 5 (`.inp`) → `redes.plan/1`

Columnas según el manual de SWMM 5.2 (apéndice D, *Input File Format*). Antes de escribir el
plan, lee `plan.md` (contrato y cotas).

## 1. Cómo leer el archivo

- Texto plano. `[SECCION]` abre una sección; `;` empieza un comentario (`;;` son las cabeceras de
  columnas que escribe la interfaz: útiles para confirmar el orden, nunca son datos).
- Campos separados por espacios o tabuladores; los nombres no llevan espacios; las cadenas con
  espacios van entre comillas (`"DIARIO"`). `*` en un campo numérico = «valor por defecto de
  SWMM», no cero.
- Un objeto se identifica por su nombre en toda la red: las secciones se cruzan por nombre
  (`[XSECTIONS]`, `[COORDINATES]`, `[VERTICES]`, `[TAGS]` se refieren a nodos y enlaces de otras
  secciones).
- Si el archivo es grande, parte el texto por secciones con un script de usar y tirar; no hace
  falta más: el mapeo lo decides tú con esta referencia.

## 2. Unidades (`[OPTIONS]`)

| Opción | Valores | Implica |
|---|---|---|
| `FLOW_UNITS` | `CFS`, `GPM`, `MGD` (US) · `CMS`, `LPS`, `MLD` (SI) | US: cotas, profundidades, longitudes, desfases y **geometrías de sección en pies** (× 0,3048; el diámetro también va en pies, no en pulgadas). SI: metros. Caudales a L/s: CFS × 28,3168; GPM × 0,0630902; MGD × 43,8126; CMS × 1000; MLD × 11,5741. **Si falta, SWMM asume CFS** (dilo en el informe) |
| `LINK_OFFSETS` | `DEPTH` (por defecto) · `ELEVATION` | cómo leer `InOffset`/`OutOffset` de `[CONDUITS]`, `Offset` de `[ORIFICES]` y `CrestHt` de `[WEIRS]` (ver §4) |
| `FORCE_MAIN_EQUATION` | `H-W` · `D-W` | qué es `Geom2` de una sección `FORCE_MAIN` |
| `IGNORE_RAINFALL` | `YES` · `NO` | con `YES`, las subcuencas no indican red pluvial |

Las **coordenadas no dependen de `FLOW_UNITS`**: son unidades de mapa. `[MAP] Units` dice
`Meters`, `Feet`, `Degrees` o `None`. `Degrees` → lon/lat: pide reproyectar (ver
`geojson-sig.md` §1). `Feet` con coordenadas UTM: pregunta (suele ser una etiqueta mal puesta).

## 3. Secciones de nodos

| Sección | Columnas | Lectura |
|---|---|---|
| `[JUNCTIONS]` | `Name Elev MaxDepth InitDepth SurDepth Aponded` | `Elev` = **solera** del nodo. `MaxDepth` = solera → terreno/tapa; **0 significa «hasta la clave del tubo más alto»**, no terreno: terreno ausente. `SurDepth` = sobrecarga sobre `MaxDepth` (no es tapa) |
| `[OUTFALLS]` | `Name Elev Type [Stage\|Tcurve\|Tseries] [Gated] [RouteTo]` | `Elev` = solera de la descarga. `Type`: `FREE`, `NORMAL`, `FIXED` (+ cota), `TIDAL` (+ curva), `TIMESERIES` (+ serie). El nivel de `FIXED` es agua, no terreno |
| `[DIVIDERS]` | `Name Elev DivLink Type params… MaxDepth InitDepth SurDepth Aponded` | params según `Type`: `OVERFLOW` 0, `CUTOFF` 1 (Qmin), `TABULAR` 1 (curva), `WEIR` 3 (Qmin Ht Cd). `MaxDepth` va **después** de los params |
| `[STORAGE]` | `Name Elev MaxDepth InitDepth Shape params… [SurDepth] Fevap [Psi Ksat IMD]` | `Shape`: `TABULAR Curve`, `FUNCTIONAL A1 A2 A0`, `CYLINDRICAL L W Z`, `CONICAL`, `PARABOLIC`, `PYRAMIDAL`. En 5.1 no hay `SurDepth` y la columna tras la forma es `Aponded`: mira la versión y la cabecera `;;`. El área no es volumen: **no calcules `volumen_util`** |

## 4. Secciones de enlaces

| Sección | Columnas | Lectura |
|---|---|---|
| `[CONDUITS]` | `Name From To Length Roughness InOffset OutOffset [InitFlow] [MaxFlow]` | `Roughness` = n de Manning. **Columnas 6 y 7 son desfases, no diámetros.** Con `DEPTH`: solera del tubo = solera del nodo + desfase. Con `ELEVATION`: el desfase **es** la cota de solera. `*` o vacío = solera del nodo |
| `[XSECTIONS]` | `Link Shape Geom1 Geom2 Geom3 Geom4 [Barrels] [Culvert]` (o `Link IRREGULAR Transect`, `Link CUSTOM Geom1 Curve`, `Link STREET Street`) | `Geom1` = altura total (o diámetro). Ver §5 |
| `[PUMPS]` | `Name From To Curve Status Startup Shutoff` | `Curve` `*` = bomba ideal. Los `PumpN` de `[CURVES]` dicen el tipo; `Pump1` aspira de un almacenamiento |
| `[ORIFICES]` | `Name From To Type Offset Qcoeff [Gated] [CloseTime]` | `Type` `SIDE` / `BOTTOM`. Sección en `[XSECTIONS]` (`CIRCULAR` o `RECT_CLOSED`) |
| `[WEIRS]` | `Name From To Type CrestHt Qcoeff [Gated] [EndCon] [EndCoeff] [Surcharge] [RoadWidth] [RoadSurf] [CoeffCurve]` | `Type`: `TRANSVERSE`, `SIDEFLOW`, `V-NOTCH`, `TRAPEZOIDAL`, `ROADWAY`. `CrestHt` sigue a `LINK_OFFSETS` (altura sobre la solera de `From` o cota). Longitud de cresta = `Geom2` de su `[XSECTIONS]` |
| `[OUTLETS]` | `Name From To Offset Type …` | salida regulada por curva |
| `[COORDINATES]` | `Node X Y` | planta de los nodos. Sin fila → el nodo **no tiene planta**: no lo pongas en (0,0); pide coordenadas o descártalo con sus tramos |
| `[VERTICES]` | `Link X Y` (en orden) | vértices intermedios. Escríbelos `[E,N]` (sin Z): Redes interpola la Z por longitud entre las soleras del tramo y la marca como derivada (`plan.md` §2.2) |
| `[TAGS]` | `Node\|Link Name Tag` | etiquetas del usuario: a veces dicen qué es (`Manhole`, `Inlet`, `Camara`). Úsalas como pista, confírmalas |

Otras secciones (`[SUBCATCHMENTS]`, `[SUBAREAS]`, `[INFILTRATION]`, `[RAINGAGES]`, `[DWF]`,
`[INFLOWS]`, `[RDII]`, `[TIMESERIES]`, `[PATTERNS]`, `[CURVES]`, `[CONTROLS]`, `[POLLUTANTS]`,
`[LANDUSES]`, `[Polygons]`, `[SYMBOLS]`, `[LABELS]`) no son objetos de la red. Solo sirven de pista
(sistema, tipo de bomba) o se ignoran. Resultados (`.rpt`, `.out`) no se copian salvo que el
usuario lo pida; entonces van a `datos` con `fuente` del `.rpt`.

## 5. Mapeo a familias de Redes

Comprueba subtipos y atributos con `redes esquema --familia <clave>`.

| Origen | Familia:subtipo | Campos |
|---|---|---|
| `JUNCTION` (pozo) | `saneamiento.pozo:inspeccion` | `cota = Elev`; si `MaxDepth > 0`: `cota_terreno = Elev + MaxDepth`, `attrs.profundidad` y `datos.profundidad_max`. Sin `diametro` (SWMM no lo da) |
| `JUNCTION` con `MaxDepth 0`, o descarga de bomba | **pregunta**: puede ser un nudo sin estructura (unión de impulsión, caja) | si el usuario no sabe, `saneamiento.pozo:inspeccion` y dilo en el informe |
| `DIVIDER` | `saneamiento.pozo:inspeccion` (o `saneamiento.camara`, pregunta) | como un pozo |
| `OUTFALL` | `saneamiento.descarga:<pregunta>` (`emisario_muro`, `emisario_talud`, `emisario_subacuatico`…) | `cota = Elev`, `attrs.cota_entrega = Elev`, `attrs.tipo_descarga = "emisario"`. SWMM no describe la estructura |
| `STORAGE` del que aspira una bomba | `saneamiento.bombeo:camara_humeda` | `cota = Elev`, `attrs.profundidad = MaxDepth`, `datos.profundidad_max` |
| `STORAGE` sin bomba | `saneamiento.camara:compensacion` (tanque de tormenta: pregunta) | `attrs.altura_util = MaxDepth`. **Sin** `volumen_util` |
| `CONDUIT` `CIRCULAR` / `FILLED_CIRCULAR` | `saneamiento.tuberia:colector_<sistema>` | `diametro_mm = Geom1` (pies × 304,8 o m × 1000; es el DN que asume el plan, dilo) y `diametro_interior_mm = Geom1` (SWMM da el interior), `cota_solera_inicio/fin`, `attrs.rugosidad_n`, `datos.longitud_origen`; desfase ≠ 0 → `datos.desfase_entrada/salida`; `FILLED_CIRCULAR`: `datos.altura_sedimento = Geom2` |
| `CONDUIT` `FORCE_MAIN` | `saneamiento.tuberia:impulsion` | `diametro_mm = Geom1`; `Geom2` → `attrs.coef_hazen` (H-W) o `attrs.rugosidad_abs` en mm (D-W; pies × 304,8) |
| `CONDUIT` `RECT_CLOSED` | `saneamiento.canal:box_culvert` | `attrs.altura_celda = Geom1`, `ancho_celda = Geom2`, `numero_celdas = Barrels`, `seccion = "rectangular"` |
| `CONDUIT` `RECT_OPEN` / `TRAPEZOIDAL` / `TRIANGULAR` | `saneamiento.canal:rectangular` / `trapezoidal` / `triangular` | `altura_muro = Geom1`; `ancho_plantilla` (y `base`) = `Geom2`; `talud = Geom3` si `Geom3 = Geom4` (si no, ambos a `datos`); triangular: `profundidad = Geom1`, `ancho = Geom2` (ancho superior) |
| `CONDUIT` ovoide, herradura, arco, elipses, `CUSTOM`… | `saneamiento.tuberia:colector_<sistema>` **sin** `diametro_mm` | `seccion` (`ovoide`, `herradura`, `arco`, `elipse`, `otra`), `alto_m = Geom1`, `ancho_m = Geom2` si es > 0. Redes dibuja tubo circular: dilo |
| `CONDUIT` `IRREGULAR`, `PARABOLIC`, `POWER`, `STREET` | `saneamiento.canal` si el usuario lo confirma; si no, fuera | sin geometría: lista en el informe |
| `PUMP` | sin arco: la representa su `STORAGE` (`saneamiento.bombeo`). Bomba en línea (no aspira de un almacenamiento): sin familia, se lista | `datos` del `STORAGE` si el usuario quiere el estado/arranque |
| `WEIR` `SIDEFLOW` | `saneamiento.descarga:alivio_lateral`, ubicado en el nodo `From` | `cota` = solera de `From`, `attrs.tipo_descarga = "alivio"`, `cota_cresta`, `longitud_vertedero = Geom2` |
| `WEIR` `TRANSVERSE` | `saneamiento.descarga:vertedero_cresta`, en `From` | ídem + `attrs.altura_cresta = cresta − solera de From` |
| `WEIR` `V-NOTCH`, `TRAPEZOIDAL`, `ROADWAY`; `ORIFICE`; `OUTLET` | sin familia directa: pregunta (¿`comun.aforador`, `comun.compuerta`?) o lista | — |

**Sistema (subtipo del colector).** `combinado` si hay `[SUBCATCHMENTS]` con lluvia (sin
`IGNORE_RAINFALL YES`) **y** `[DWF]`; `pluvial` si solo subcuencas; `sanitario` si solo `[DWF]`.
El `[TITLE]` es una pista. Si no hay certeza, **pregunta**; no elijas.

**Dispositivos de enlace** (bombas, vertederos, orificios): en Redes son nodos puntuales y no se
unen con arcos. Se ubican en el nodo `From` con su planta y cota y declaran `desde = From`,
`hasta = To` (`plan.md` §2.1): la topología los ve conectados. La cámara húmeda que representa a
una bomba declara solo `hasta = To`. No inventes tramos para unirlos.

## 6. Ejemplos trabajados (`crates/redes-core/tests/fixtures/`)

### `swmm_si_cms.inp` → `swmm_si_cms.plan.json` (CMS, `DEPTH`, pluvial)

| Línea del `.inp` | Objeto del plan |
|---|---|
| 3 `[TITLE]` «Colector pluvial…» y 51-55 `[SUBCATCHMENTS]` sin `[DWF]` | sistema pluvial → `colector_pluvial` |
| 72 `J1 2510.50 2.40` | `J1` pozo, `cota 2510.5`, `cota_terreno 2512.9`, `profundidad 2.4` |
| 74 `J3 2508.90 0` | `J3` pozo **sin** `cota_terreno` (MaxDepth 0) |
| 80 `OF1 2505.00 FREE` | `OF1` `saneamiento.descarga:emisario_muro` (subtipo confirmado con el usuario), `cota_entrega 2505.0` |
| 85 `ST1 2506.00 4.00 … FUNCTIONAL 0 0 120` + 98 `P1 ST1 OF1` | `ST1` `saneamiento.bombeo:camara_humeda`, `profundidad 4.0`; sin volumen; `P1` no es objeto |
| 90 `C1 J1 J2 85.0 0.013 0 0.25` + 113 `CIRCULAR 0.60` + 159 vértice | `C1` DN 600, `cota_solera_inicio 2510.5`, `cota_solera_fin 2510.05` (2509.80 + 0.25), `desfase_salida 0.25`, `diametro_interior_mm 600`, 3 vértices `[E,N]` |
| 92 `C3 … 0.30 0` + 115 `RECT_CLOSED 1.00 1.50` | `C3` `saneamiento.canal:box_culvert`, `altura_celda 1.0`, `ancho_celda 1.5`, solera inicio 2509.2 |
| 93 `C4` + 116 `TRAPEZOIDAL 1.20 2.00 1.5 1.5` | `C4` `saneamiento.canal:trapezoidal`, `altura_muro 1.2`, `ancho_plantilla 2.0`, `talud 1.5` |
| 103 `OR1` orificio | no está en el plan: sin familia, listado |
| 108 `W1 J4 ST1 SIDEFLOW 0.90` + 118 `RECT_OPEN 0.50 3.00` | `W1` `alivio_lateral` en la planta de `J4`, `cota_cresta 2509.1`, `longitud_vertedero 3.0` |

### `swmm_us_cfs.inp` → `swmm_us_cfs.plan.json` (CFS, `ELEVATION`, sanitario)

| Línea del `.inp` | Objeto del plan |
|---|---|
| 7 `FLOW_UNITS CFS`, 10 `LINK_OFFSETS ELEVATION`, 14 `IGNORE_RAINFALL YES`, 89 `[DWF]` | pies → m; desfases son cotas; sanitario |
| 44 `MH1 820.00 8.00` | `MH1` `cota 249.936`, `cota_terreno 252.3744` |
| 47 `PJ1 801.00 0` | `PJ1` pozo sin terreno; es la descarga de la bomba (ver §5: pregunta) |
| 58 `WW1 800.00 15.00 … TABULAR` + 72 `PMP1 WW1 PJ1` | `WW1` `saneamiento.bombeo:camara_humeda`, `profundidad 4.572` |
| 63 `P-1 … 820.00 816.25` + 82 `CIRCULAR 1.0` | `P-1` DN 305 (1 ft), soleras 249.936 → 248.793; `desfase_salida 0.2286` |
| 64 `P-2 … * *` + 137-138 vértices | `P-2` soleras = las de `MH2` y `MH3`; 4 vértices |
| 66 `FM-1` + 85 `FORCE_MAIN 0.5 130` | `FM-1` `impulsion`, DN 152, `coef_hazen 130` |
| 67 `G-5` + 86 `EGG 2.0` | `G-5` sin `diametro_mm`, `seccion "ovoide"`, `alto_m 0.6096` |
| 77 `WR1 MH2 OUT1 TRANSVERSE 819.00` + 87 `RECT_OPEN 1.0 4.0` | `WR1` `vertedero_cresta`, `cota_cresta 249.6312`, `altura_cresta 1.0668`, `longitud_vertedero 1.2192` |

## 7. Trampas conocidas

- Leer `InOffset`/`OutOffset` como diámetro (DN 300 por un desfase de 0,30).
- Tomar el diámetro en pulgadas en un archivo `CFS`: en SWMM va en **pies**.
- Suponer terreno = solera + algo cuando `MaxDepth = 0`.
- Usar la cota de la `JUNCTION` como solera del tubo cuando hay desfase: el salto en el pozo se
  pierde. Ese salto es lo que el revisor tiene que ver.
- Calcular volumen de un `STORAGE` con su curva de área: es cálculo.
- `[OUTFALLS]` `FIXED` 2505.5: es nivel de agua, no la cota de la estructura.
- Confundir `[MAP] Units` con `FLOW_UNITS`.
- Nombres que solo difieren en mayúsculas (`j1` y `J1`): en Redes serían dos códigos; avisa y
  pregunta antes de unirlos o separarlos.
- `Barrels > 1` en un tubo: Redes dibuja uno; dilo en el informe.
