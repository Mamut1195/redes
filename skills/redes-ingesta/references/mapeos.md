# Mapeos por aplicación de origen

Tablas de referencia de la skill `redes-ingesta`. La columna «Redes» es la clave de familia
que debe ir en `plan.objetos[].familia`.

---

## 1. Civil 3D / AutoCAD (DXF)

### Capas

| Capa típica en el plano | Familia Redes | Subtipo |
|---|---|---|
| `PZ`, `POZO`, `MH`, `SAN-POZO` | `saneamiento.pozo` | `inspeccion` |
| `PZ-CAIDA` | `saneamiento.pozo` | `caida` |
| `PZ-BOMBEO`, `EB` | `saneamiento.bombeo` | `camara_humeda` |
| `ALC`, `SAN`, `COLECTOR`, `CL` | `saneamiento.tuberia` | `colector` |
| `EMISARIO`, `EMIS` | `saneamiento.tuberia` | `emisario` |
| `DESCARGA`, `VERT` | `saneamiento.descarga` | `emisario_final` |
| `AGUA`, `AP`, `DIST`, `ACUEDUCTO` | `agua.tuberia` | `distribucion` |
| `ADUCCION` | `agua.tuberia` | `aduccion` |
| `VALVULA`, `VL` | `agua.valvula` | `compuerta` |
| `HIDRANTE`, `LL`, `HYD` | `agua.llave` | `hidrante` |
| `TANQUE`, `TQ`, `RESERVORIO` | `agua.tanque` | `apoyado` |
| `PLANTA`, `PTAP`, `PTAR` | `agua.planta` | `ptap` |
| `CAMARA`, `CM` | `saneamiento.camara` | `valvulas` |
| `SUMIDERO`, `SM` | `saneamiento.sumidero` | `calzada` |
| `HITO`, `BM` | `comun.hito` | `bm` |
| `CRUCE` | `comun.cruce` | `via` |

### Entidades

| Entidad DXF | Se interpreta como | Nota |
|---|---|---|
| `POINT` en capa de nodo | nodo en (10, 20, 30) | cota 30 = clave/solera |
| `CIRCLE` en capa de nodo | tapa del pozo: (10, 20, 30) + radio 40 | la cota 30 se usa como `z_rim` |
| `LINE` entre dos nodos | arco recto | se snapea a pozos dentro de la tolerancia |
| `POLYLINE` con flag 70 = 8 | arco con vértices 3D | el trazado completo entra en `vertices` |
| `LWPOLYLINE` con elevación 38 | arco en planta | la cota es constante (elevación) |
| `TEXT`/`MTEXT` de cota | atributo del nodo más cercano | requiere perfil (ING-05) |

**No se importan** bloques anidados, sombreados, cotas acotadas ni *proxies* de Civil 3D
(tuberías como objetos AECC). Para esos casos: `EXPLODE` a polilíneas 3D antes de exportar.

**Convención de nombres de capa:** el importador reconoce subcadenas (`POZO`, `MH`, `AGUA`…).
Si el estudio usa otra convención, se declara un perfil de mapeo (ING-05) en vez de renombrar
capas a mano.

---

## 2. SWMM (`.inp`)

Script: `scripts/inp_to_plan.py --tipo swmm`. Columnas según el manual de SWMM 5.2 (apéndice D).

**Unidades.** `[OPTIONS] FLOW_UNITS`: `CFS`, `GPM`, `MGD` → cotas, profundidades, longitudes,
desfases y geometrías de sección en **pies** (× 0,3048); `CMS`, `LPS`, `MLD` → metros. Si falta,
SWMM asume `CFS` y el script lo avisa. `LINK_OFFSETS` `DEPTH` (por defecto: altura sobre la solera
del nodo) o `ELEVATION` (cota absoluta); `*` = solera del nodo. Las coordenadas no se convierten.

**Sistema.** `combinado` si hay `[SUBCATCHMENTS]` con lluvia (sin `IGNORE_RAINFALL YES`) y `[DWF]`;
`pluvial` si sólo subcuencas; `sanitario` si sólo `[DWF]`; si no, pistas del `[TITLE]`. Si nada lo
dice, el script se detiene y pide `--sistema`. Decide el subtipo `colector_pluvial|sanitario|combinado`.

| Sección SWMM | Columnas | Redes |
|---|---|---|
| `[JUNCTIONS]` | `Name Elev MaxDepth InitDepth SurDepth Aponded` | `saneamiento.pozo:inspeccion` · `cota = Elev`; si `MaxDepth > 0`, `cota_terreno = Elev + MaxDepth` y `attrs.profundidad`; si `MaxDepth = 0`, terreno **ausente** (aviso). Sin `diametro` |
| `[DIVIDERS]` | `Name Elev DivLink Type … MaxDepth` | igual que un pozo |
| `[OUTFALLS]` | `Name Elev Type …` | `saneamiento.descarga:emisario_muro` (genérico, con aviso) · `attrs.cota_entrega = Elev`, `tipo_descarga = emisario`. Sin terreno |
| `[STORAGE]` | `Name Elev MaxDepth InitDepth Shape …` | si una bomba aspira de él: `saneamiento.bombeo:camara_humeda` (`attrs.profundidad = MaxDepth`); si no, `saneamiento.camara:compensacion` (`attrs.altura_util = MaxDepth`) |
| `[CONDUITS]` | `Name From To Length Roughness InOffset OutOffset InitFlow MaxFlow` | arco · `attrs.rugosidad_n`, `datos.longitud_origen`, `datos.cota_solera_inicio/fin` y, si hay salto, `datos.desfase_entrada/salida`. El trazado lleva la cota de cada extremo (solera + desfase): **el salto en el pozo se ve** |
| `[XSECTIONS]` | `Link Shape Geom1 Geom2 Geom3 Geom4 Barrels Culvert` | `CIRCULAR`, `FILLED_CIRCULAR` → `saneamiento.tuberia`, `diametro_mm = Geom1`; `FORCE_MAIN` → `saneamiento.tuberia:impulsion` (+ `coef_hazen` o `rugosidad_abs` de `Geom2`); `RECT_CLOSED` → `saneamiento.canal:box_culvert` (`altura_celda`, `ancho_celda`, `numero_celdas`); `RECT_OPEN`, `TRAPEZOIDAL`, `TRIANGULAR` → `saneamiento.canal` del mismo nombre; ovoide, herradura, arco y demás cerradas → `saneamiento.tuberia` **sin diámetro** y `datos.altura_seccion` (aviso); `IRREGULAR`, `PARABOLIC`, `POWER`, `STREET` → descarte |
| `[VERTICES]` | `Link X Y` | vértices del trazado; la cota se reparte en proporción a la longitud en planta entre los extremos (el tramo de SWMM es recto en perfil) |
| `[WEIRS]` | `Name From To Type CrestHt Qcoeff …` | `SIDEFLOW` → `saneamiento.descarga:alivio_lateral`; `TRANSVERSE` → `saneamiento.descarga:vertedero_cresta` (`altura_cresta`). `cota_cresta`, `longitud_vertedero = Geom2`. Se ubica en el nodo `From`. `V-NOTCH`, `TRAPEZOIDAL`, `ROADWAY` → descarte |
| `[PUMPS]` | `Name From To Curve Status Startup Shutoff` | la estación es el `[STORAGE]` de aspiración (ver arriba); la bomba no tiene arco. Si no aspira de un almacenamiento → descarte |
| `[ORIFICES]`, `[OUTLETS]` | — | descarte (sin familia de orificio ni de salida regulada) |
| `[SUBCATCHMENTS]`, `[RAINGAGES]`, `[DWF]`… | — | no son objetos; sólo deciden el sistema |
| `[POLLUTANTS]`, `[LANDUSES]`, `[TIMESERIES]`, `[CONTROLS]`, `[Polygons]` | — | se ignoran |

**Lo que no se copia:** caudales simulados, calados, volúmenes. Si el `.rpt` trae resultados y
se quieren mostrar, se añaden como `datos` con `fuente: "SWMM"`.

**Coordenadas:** SWMM sí guarda planta en `[COORDINATES]` y `[VERTICES]`. `--coords codigo,E,N`
completa o sustituye; un nodo sin planta se descarta (y sus tramos con él).

---

## 3. EPANET (`.inp`)

Script: `scripts/inp_to_plan.py --tipo epanet`. Columnas según el manual de EPANET 2.2 (apéndice C).

**Unidades.** `[OPTIONS] Units`: `CFS`, `GPM`, `MGD`, `IMGD`, `AFD` → cotas y longitudes en pies,
diámetros de tubería y válvula en **pulgadas** (× 25,4), diámetro de tanque en pies, presión en psi
(× 0,3048/0,4333, la constante de EPANET), potencia en hp (× 0,7457); `LPS`, `LPM`, `MLD`, `CMH`,
`CMD`, `CMS` → metros y milímetros. Si falta, EPANET asume `GPM` (aviso). Caudales a L/s.
`Headloss`: `H-W` → `coef_hazen`; `D-W` → `rugosidad_abs` en mm (milipies × 0,3048 en US);
`C-M` → `datos.rugosidad_n` (agua.tuberia no tiene ese atributo).

| Sección EPANET | Columnas | Redes |
|---|---|---|
| `[JUNCTIONS]` | `ID Elev Demand Pattern` | `comun.accesorio` según cuántos enlaces llegan: 1 `tapon`, 2 `union` (`reduccion` si cambian los diámetros), 3 `tee`, 4 `cruz`; 0 o más de 4 → descarte. `cota = Elev`, **sin** `cota_terreno` (la cota del nudo es hidráulica). `datos.demanda_base` en L/s. **No** es una cámara de válvulas |
| `[DEMANDS]` | `Junction Demand Pattern ;Category` | sustituye a la demanda de `[JUNCTIONS]`; varias categorías → `demanda_base_1`, `_2`… |
| `[RESERVOIRS]` | `ID Head Pattern` | descarte salvo `--reservorio familia:subtipo`; entonces `datos.carga_hidraulica = Head`. **Head no es cota de terreno ni de fondo** |
| `[TANKS]` | `ID Elev InitLvl MinLvl MaxLvl Diam MinVol VolCurve Overflow` | `agua.tanque:apoyado_cilindrico` (o `--subtipo-tanque`; aviso) · `cota = cota_piso = Elev`, `cota_nivel_min = Elev + MinLvl`, `cota_nivel_max = Elev + MaxLvl` (columna 5, **no** la 6 que es el diámetro), `diametro` (mm) si no hay `VolCurve`, `datos.nivel_inicial`. **Sin `volumen_util`** |
| `[PIPES]` | `ID Node1 Node2 Length Diam Roughness MinorLoss Status` | `agua.tuberia:distribucion_primaria` (o `--subtipo-tuberia`; aviso) · `diametro_mm`, `datos.longitud_origen`. `Status CV` → `attrs.tipo_valvula = retencion` |
| `[PUMPS]` | `ID Node1 Node2 HEAD c \| POWER p …` | `agua.bombeo:booster_en_linea` en el nodo `Node1`, con su cota; `potencia_unitaria` (kW) si hay `POWER` |
| `[VALVES]` | `ID Node1 Node2 Diam Type Setting MinorLoss` | `agua.valvula` en `Node1`: `PRV` → `reduccion_presion` (`consigna_presion`), `PSV` → `sostenedora`, `FCV` → `reguladora_caudal` (`consigna_caudal`); `TCV`, `PBV`, `GPV`, `PCV` → descarte |
| `[VERTICES]` | `Link X Y` | vértices del trazado, cota repartida entre las de los extremos |
| `[PATTERNS]`, `[CURVES]`, `[CONTROLS]`, `[RULES]`, `[ENERGY]` | — | se ignoran |

**Dispositivos de enlace.** En EPANET y SWMM, bombas, válvulas y vertederos son *enlaces* entre dos
nodos; en Redes son objetos puntuales y no se unen con arcos. Se ubican en el nodo de entrada (su
planta y su cota, que son las del mismo punto) y la conexión queda en el aviso. No se inventan
tramos para unirlos.

---

## 4. Excel / CSV de inventario

Formato mínimo cómodo de convertir:

```csv
codigo,tipo,familia,e,n,cota,cota_terreno,diametro_mm,desde,hasta
PZ-001,nodo,saneamiento.pozo,760060,9796090,2510.92,2512.52,,
CL-001,arco,saneamiento.tuberia,760060,9796090,2510.92,2512.52,250,PZ-001,PZ-002
```

Reglas:

- Una fila por objeto; `tipo` decide si es nodo o arco.
- En arcos, `desde`/`hasta` son códigos de nodo, no coordenadas.
- Las columnas vacías se omiten (no se rellenan con ceros).
- Si el inventario trae «longitud» o «volumen», van a `datos` con su fuente.

---

## 5. SIG / GeoJSON

| Geometría | Familia por defecto | Propiedades leídas |
|---|---|---|
| `Point` | `saneamiento.pozo` | `familia`, `codigo`, `cota`, `estado` |
| `LineString` | `saneamiento.tuberia` | `familia`, `codigo`, `desde`, `hasta`, `diametro_mm` |

Si el SIG ya trae códigos de nodo en los extremos de cada línea, se usa `desde`/`hasta`; si no,
se empareja por proximidad (≤ 1 m) con los puntos importados antes.

---

## 6. Lo que ninguna fuente puede darnos

| Dato | Por qué | Qué hacer |
|---|---|---|
| Volumen de excavación | Es cálculo | Traerlo de Civil 3D como `dato` |
| Caudal de diseño | Es cálculo hidrológico | Traerlo de SWMM/Excel como `dato` |
| Presión en un nodo | Es cálculo hidráulico | Traerlo de EPANET como `dato` |
| Cumplimiento normativo | Es un juicio con norma y firma | Fuera del producto |
| DWG nativo | Formato propietario | Exportar DXF y guardar como DWG en el CAD |
