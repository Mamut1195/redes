# Hojas de diseño (Excel, CSV, informes de Civil 3D) → `redes.plan/1`

## 1. Leer la hoja

- `.xlsx`: lee cada hoja con una librería (p. ej. `openpyxl` en modo `data_only=True` para ver
  **valores**, no fórmulas). Las hojas de diseño tienen **cabeceras en varias filas**, celdas
  combinadas, unidades en una fila aparte (`(m)`, `(mm)`, `(‰)`) y filas de totales o subtítulos
  («TRAMO 1 – CALLE A»): identifica la fila de cabecera y la de unidades antes de leer.
- `.csv`: separador `;` y coma decimal en configuración regional española (`2512,40`); detecta
  ambos. Codificación: UTF-8 con BOM o Windows-1252.
- Una fila es un tramo (hoja de cálculo hidráulico) o un pozo (cuadro de pozos). Si la hoja de
  tramos trae los pozos solo como «desde/hasta», los nodos salen de otra hoja o de la planta: sin
  E/N no hay nodo (pregúntalo).

## 2. Columnas típicas

| Significado | Cabeceras habituales | Plan |
|---|---|---|
| tramo | `TRAMO`, `CÓDIGO`, `DE-A` («PZ1-PZ2»: pártelo) | arco `codigo` |
| pozo inicial / final | `DESDE`/`HASTA`, `P. INICIAL`/`P. FINAL`, `PZ AGUAS ARRIBA`/`ABAJO`, `NODO I`/`NODO J` | `desde`/`hasta` |
| E, N | `ESTE`/`NORTE`, `X`/`Y`, `COORD. E` | nodo `e`, `n` |
| longitud | `L`, `LONG.`, `LONGITUD (m)` | `datos.longitud_origen` |
| diámetro | `Ø`, `D`, `DN`, `DIÁM. (mm)`, `D (pulg)` | `diametro_mm` = DN; si la hoja da DI, `attrs.diametro_interior` (mm). Pulgadas × 25,4 |
| pendiente | `S`, `PEND. (%)`, `S (‰)`, `J` | `attrs.pendiente` en **m/m** (% ÷ 100, ‰ ÷ 1000) |
| cota de terreno / rasante | `CT`, `COTA TERRENO`, `RASANTE`, `CR` inicio y fin | nodo `cota_terreno`; tramo `attrs.cota_terreno` si es por tramo |
| cota de tapa | `CT`, `COTA TAPA` | `cota_terreno` o `attrs.cota_tapa` (pozo) |
| solera | `CS`, `CF`, `COTA FONDO`, `COTA SOLERA`, `INVERT`, `COTA BATEA` (Colombia) | tramo `cota_solera_inicio/fin`; pozo `cota` |
| clave | `CC`, `COTA CLAVE`, `COTA CORONA` | tramo `attrs.cota_clave` / `attrs.cota_clave_fin` |
| profundidad / altura de pozo | `H`, `PROF.`, `H POZO` | `attrs.profundidad` (pozo) |
| recubrimiento | `REC.`, `RECUBRIMIENTO`, `COBERTURA` | `attrs.recubrimiento` (agua) o `datos.recubrimiento` |
| caudal, velocidad, tirante | `Q`, `Qd`, `V`, `y/D` | `datos` con unidad (`L/s`, `m/s`) y fuente: son resultados del diseño, no se recalculan |
| material, clase | `MATERIAL`, `TIPO`, `CLASE`, `SN`, `PN` | `attrs.material`, `attrs.clase_tuberia`, `attrs.clase_presion` |

Abreviaturas ambiguas: **`CT`** es «cota de terreno» en unas hojas y «cota de tapa» en otras;
**`CF`** puede ser «cota de fondo» (solera) del pozo o del tramo; **`H`** puede ser profundidad o
altura de caída. Mira la leyenda de la hoja o pregunta.

## 3. Clave y solera

- **solera = clave − DI**, nunca al revés y nunca con un DI supuesto. Solo si la hoja trae la
  clave **y** el diámetro interior (o el usuario lo confirma); dilo en el informe.
- Si solo hay clave y no hay DI: la solera queda **ausente**; la clave va a `attrs.cota_clave`.
- Si la hoja trae solera **y** clave, copia ambas; si clave − solera no coincide con el DI, es
  una discrepancia del origen: no la corrijas, cítala.
- La «cota de batea» o «invert» es solera; la «cota de corona» es clave.
- La solera del tramo al llegar a un pozo puede estar más alta que la del pozo (caída): copia la
  del tramo en `cota_solera_fin` y la del pozo en el `cota` del nodo; no las igualas.

## 4. Informes de Civil 3D

*Pipe report* (por tubo): `Pipe Name`, `Start Structure`, `End Structure`, `Inner Diameter`,
`Start Invert`/`End Invert` (solera), `Start Crown`/`End Crown` (clave), `2D Length`, `Slope`.
*Structure report*: `Structure Name`, `Northing`, **`Easting`** (cuidado con el orden), `Rim
Elevation` (tapa), `Sump Elevation` (fondo de la estructura; puede estar bajo la solera de
salida), `Inner Diameter`. Unidades: las del dibujo (mira el encabezado del informe).

## 5. Lo que no se hace

- Rellenar celdas vacías con la fila anterior (las combinadas sí se propagan: es formato, y dilo).
- Recalcular pendiente, velocidad o caudal para «comprobar»: si no cuadran, se cita.
- Convertir `0` en dato: en una cota o diámetro, `0` casi siempre es «sin dato»; pregunta.
