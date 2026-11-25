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
| `POINT` en capa de nodo | nodo en (10, 20, 30) | cota 30 = solera |
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

| Sección SWMM | Campo | Redes |
|---|---|---|
| `[JUNCTIONS]` | `Name Invert Elev Max Depth` | `saneamiento.pozo` · `cota = Invert`, `cota_terreno = Invert + Max Depth` |
| `[OUTFALLS]` | `Name Invert Elev Type` | `saneamiento.descarga` · `subtipo = emisario_final` |
| `[STORAGE]` | `Name Invert Elev …` | `saneamiento.camara` · `subtipo = reunion` |
| `[CONDUITS]` | `Name From To Length Roughness InOffset OutOffset` | `saneamiento.tuberia` · `attrs.rugosidad_n = Roughness` |
| `[XSECTIONS]` | `Link Shape Geom1 …` | diámetro del arco (`Geom1` en m → mm) |
| `[SUBCATCHMENTS]` | — | se ignora (es hidrología) |
| `[POLLUTANTS]`, `[LANDUSES]` | — | se ignoran |
| `[TIMESERIES]`, `[CONTROLS]` | — | se ignoran |

**Lo que no se copia:** caudales simulados, calados, volúmenes. Si el `.rpt` trae resultados y
se quieren mostrar, se añaden como `datos` con `fuente: "SWMM"`.

**Coordenadas:** SWMM no guarda planta. Se pasa `--coords` con `codigo,E,N`, o se importa el
DXF del mismo proyecto y se empareja por código.

---

## 3. EPANET (`.inp`)

| Sección EPANET | Campo | Redes |
|---|---|---|
| `[JUNCTIONS]` | `ID Elev Demand` | `saneamiento.camara` · `subtipo = valvulas` (nodo de consumo) |
| `[RESERVOIRS]` | `ID Head Pattern` | `agua.tanque` · `cota = Head` |
| `[TANKS]` | `ID Elev InitLvl MinLvl MaxLvl Diam …` | `agua.tanque` · `cota_nivel_max = Elev + MaxLvl` |
| `[PIPES]` | `ID Node1 Node2 Length Diameter Roughness …` | `agua.tuberia` · `attrs.coef_hazen = Roughness` |
| `[PUMPS]` | `ID Node1 Node2 …` | `agua.tuberia` · `subtipo = impulsion` (marcar en `attrs`) |
| `[VALVES]` | `ID Node1 Node2 Diam Type …` | `agua.valvula` · `subtipo` según `Type` (TCV→reguladora, PRV→reduccion_presion, FCV→reguladora) |
| `[DEMANDS]` | `Junction Demand` | `datos.demanda` del nodo |
| `[PATTERNS]`, `[CONTROLS]`, `[RULES]` | — | se ignoran |

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
- **Solera ≠ clave.** `cota`, `cota_solera`, `solera`, `invert` son la solera (fondo interior).
  `cota_clave`/`clave` es el punto más alto del interior del tubo (clave = solera + DI) y
  **nunca** se copia como solera.
- En tramos: `cota_solera_inicio`/`cota_solera_fin` (alias `solera_inicio`, `invert_inicio`…),
  `cota_clave_inicio`/`cota_clave_fin` (alias `clave_inicio`, `clave_fin`) y
  `diametro_interior` (alias `di`, en mm). Si el tramo trae solo la clave y declara el DI,
  `plan_desde_tabla.py` escribe solera = clave − DI y lo avisa; sin DI la solera queda sin dato.
  La clave declarada se conserva en `attrs.cota_clave` / `attrs.cota_clave_fin`.
- En nodos, una columna de clave sin solera se conserva en `attrs.cota_clave` y se avisa.
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
