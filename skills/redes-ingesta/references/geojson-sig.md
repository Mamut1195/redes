# SIG municipal (GeoJSON, Shapefile, GeoPackage) → `redes.plan/1`

## 1. Sistema de coordenadas: lo primero

Redes trabaja en metros proyectados (UTM). **No conviertas lon/lat a mano.**

| Formato | Dónde está el CRS |
|---|---|
| GeoJSON (RFC 7946) | por norma, **WGS84 lon/lat** (x = longitud primero). Archivos antiguos traen `"crs": {"properties": {"name": "urn:ogc:def:crs:EPSG::32717"}}` |
| Shapefile | `.prj` (WKT): busca `PROJCS["WGS_1984_UTM_Zone_17S"` o `GEOGCS` sin `PROJCS` (= grados) |
| GeoPackage | tabla `gpkg_spatial_ref_sys` |

- Si todas las X están en [−180, 180] y las Y en [−90, 90], son **grados**: rechaza y pide
  reproyectar, por ejemplo `ogr2ogr -t_srs EPSG:32717 red_utm.geojson red.geojson`, o en QGIS
  *Exportar → Guardar objetos como…* con SRC EPSG:32717.
- Ecuador continental: 17S (`EPSG:32717`) o 18S (`EPSG:32718`); `PSAD56` (`EPSG:24877`) en
  catastros viejos: confirma con el usuario y pide reproyectar a WGS84 UTM.
- El CRS va a `proyecto.crs` del plan.

## 2. Leer el archivo

- GeoJSON: `features[]` con `geometry` (`Point`, `LineString`, `MultiLineString`, `PointZ` como
  tercer número) y `properties`.
- Shapefile: `.shp` + `.shx` + `.dbf` (+ `.prj`, `.cpg`). Conviértelo a GeoJSON con `ogr2ogr -f
  GeoJSON` o pide al usuario que lo exporte. Trampas del `.dbf`: nombres de campo **truncados a
  10 caracteres** (`COTA_SOLER`, `DIAMETRO_N`), codificación Latin-1 sin `.cpg` (tildes rotas),
  números guardados como texto, `0` en lugar de vacío (un `0` en una cota casi nunca es dato:
  pregunta).
- `MultiLineString` con varias partes: cada parte puede ser un tramo distinto; no las unas sin
  confirmar.

## 3. Nombres de campo típicos

| Significado | Campos habituales | Plan |
|---|---|---|
| código | `CODIGO`, `COD_POZO`, `ID`, `FACILITYID`, `ASSETID`, `NOMBRE` | `codigo` (el `OBJECTID`/`FID` es interno del SIG: solo si no hay otro) |
| tipo | `TIPO`, `CLASE`, `SUBTYPE`, `SUBTIPO` | familia:subtipo (tabla de valores al usuario) |
| cota de tapa | `COTA_TAPA`, `CT`, `RIMELEV`, `Z_TAPA` | `cota_terreno` / `attrs.cota_tapa` |
| cota de fondo | `COTA_FONDO`, `CF`, `CS`, `INVERTELEV`, `SOLERA` | nodo `cota` |
| solera de tramo | `CS_INI`/`CS_FIN`, `UPELEV`/`DOWNELEV`, `INV_ENTRADA`/`INV_SALIDA` | `cota_solera_inicio/fin` (comprueba cuál es aguas arriba) |
| clave | `COTA_CLAVE`, `CC`, `CROWN` | `attrs.cota_clave` (**nunca** solera) |
| profundidad | `PROF`, `H`, `DEPTH` | `attrs.profundidad` |
| diámetro | `DIAM`, `DIAMETRO`, `DN`, `DIAMETER`, `D_MM` | `diametro_mm` (tramo) o `diametro` en m (pozo). **Unidad**: `8` suele ser pulgadas, `200` mm, `0.20` m: pregunta |
| material | `MATERIAL`, `MAT` | `attrs.material` |
| extremos | `NODO_INI`/`NODO_FIN`, `DESDE`/`HASTA`, `FROM_NODE`/`TO_NODE`, `UPSTREAM`/`DOWNSTREAM` | `desde`/`hasta` |
| pendiente | `PEND`, `PENDIENTE`, `SLOPE`, `S` | `attrs.pendiente` en **m/m**: un 1,5 suele ser %; ‰ también existe: pregunta |
| longitud | `LONG`, `LONGITUD`, `SHAPE_LEN`, `LENGTH` | `datos.longitud_origen` (la `SHAPE_Length` es geometría, no dato de diseño) |
| año, estado | `ANIO`, `INSTALLDATE`, `ESTADO`, `LIFECYCLESTATUS` | `attrs.anio`, `attrs.estado` (texto del origen) |

Modelo ESRI *Local Government*: `ssGravityMain`, `ssManhole`, `ssForceMain`, `wMain`,
`wHydrant`, `wSystemValve` dan la familia por la capa.

## 4. Nodos y tramos

- Puntos → nodos; líneas → arcos. Una capa de líneas nunca crea nodos con familia de tramo.
- Tramos con `desde`/`hasta` por código: úsalos. Sin ellos, une cada extremo al punto más
  cercano a ≤ 1 m (confirma la tolerancia). Sin punto, o con dos candidatos: **no crees un nodo
  implícito**, lista el tramo y pregunta.
- La Z de la geometría (si existe) suele ser terreno o nada: no la uses como solera sin confirmar.
- Una propiedad que es atributo de la familia (`redes esquema --familia X`) va a `attrs`; un
  número con procedencia, a `datos` (`{valor, unidad, fuente}`); el resto se cita en el informe.

## 5. GeoJSON exportado por Redes

`redes exportar geojson` escribe `crs`, y en `properties` `codigo`, `familia`, `subtipo`,
`cota_solera`, `cota_terreno`, `diametro_mm`, `cota_solera_inicio/fin`, `estado`, `nota`, los
atributos y los datos como `{valor, unidad, fuente}`. Se pasa al plan uno a uno (la Z de la
geometría es la solera). `estado` y `nota` no tienen campo en el plan: se pierden (dilo).

## 6. Lo que la antigua importación directa hacía mal (no lo repitas)

`import_geojson` ponía `saneamiento.pozo` a todo punto y `saneamiento.tuberia` a toda línea sin
`familia`, DN 200 si faltaba el diámetro, Z = 0 si no había tercera coordenada y no miraba el CRS.
