# Contrato `redes.plan/1`

Derivado de `crates/redes-core/src/ingest.rs` (`Plan`, `PlanProyecto`, `PlanObjeto`, `PlanDato`,
`import_plan`). Si el código cambia, manda el código: vuelve a leerlo.

## 1. Estructura

```json
{
  "schema": "redes.plan/1",
  "proyecto": {
    "nombre": "Colector Av. Los Cedros",
    "crs": "EPSG:32717",
    "fuente_app": "SWMM 5.2",
    "fuente_archivo": "cedros.inp"
  },
  "terreno": [[760000.0, 9796000.0, 2512.34]],
  "objetos": [
    { "tipo": "nodo", "codigo": "J1", "familia": "saneamiento.pozo", "subtipo": "inspeccion",
      "e": 760000.0, "n": 9796000.0, "cota": 2510.50, "cota_terreno": 2512.90,
      "diametro": 1.2, "attrs": { "profundidad": 2.4 },
      "datos": { "profundidad_max": { "valor": 2.4, "unidad": "m", "fuente": "SWMM (cedros.inp)" } } },
    { "tipo": "arco", "codigo": "C1", "familia": "saneamiento.tuberia", "subtipo": "colector_pluvial",
      "desde": "J1", "hasta": "J2", "diametro_mm": 600,
      "cota_solera_inicio": 2510.50, "cota_solera_fin": 2510.05,
      "vertices": [[760000.0, 9796000.0, 2510.50], [760080.0, 9795990.0, 2510.05]],
      "attrs": { "rugosidad_n": 0.013, "cota_clave": 2511.10, "cota_clave_fin": 2510.65 },
      "datos": { "longitud_origen": { "valor": 85.0, "unidad": "m", "fuente": "SWMM (cedros.inp)" } } }
  ]
}
```

## 2. Campos

### Raíz y `proyecto`

| Campo | Tipo | Si falta | Regla |
|---|---|---|---|
| `schema` | texto | `redes.plan/1` | debe empezar por `redes.plan/`; otro → `E_FMT_UNSUPPORTED` |
| `proyecto.nombre` | texto | **obligatorio** (puede ser `""`) | si no está vacío, renombra el proyecto |
| `proyecto.crs` | texto | `EPSG:32717` | **Pregúntalo** si el origen no lo declara. Ecuador continental: `EPSG:32717` (17S) o `EPSG:32718` (18S). `32617` es 17N: desplaza 10 000 km |
| `proyecto.fuente_app` | texto | `"plan"` | app y versión de origen (`"EPANET 2.2"`) |
| `proyecto.fuente_archivo` | texto | ruta del plan | nombre del archivo de origen, no del plan |
| `terreno` | `[[E,N,Z], …]` | sin terreno | puntos del TIN, en el mismo CRS. Hoy el TIN se etiqueta siempre como 32717 |
| `objetos` | lista | vacía | `nodo` y `arco`, en cualquier orden (los nodos se crean antes) |

### `tipo: "nodo"`

| Campo | Tipo | Va a | Regla |
|---|---|---|---|
| `codigo` | texto | código | el del origen, tal cual. Vacío → el registro pone uno correlativo |
| `familia` | texto | familia | **obligatorio**; clave de `redes esquema` |
| `subtipo` | texto | subtipo | si falta, el primero del registro: **no lo dejes vacío**, pregunta |
| `e`, `n` | número | planta | **obligatorios**, en metros del CRS. Sin planta no hay nodo: se lista |
| `cota` | número | `z_invert` | **solera**: fondo interior del pozo o cota de fondo de la estructura (m) |
| `cota_terreno` | número | `z_terrain` y `z_rim` | rasante/terreno en el punto (m). También es la tapa (ver §4) |
| `diametro` | número | `attrs.diametro_interior` | diámetro interior del **pozo en metros** (1.2), no en mm |
| `attrs` | objeto | atributos | claves del registro de la familia; valores número, texto o booleano |
| `datos` | objeto | `Dato` | `{ "valor": número, "unidad": "m", "fuente": "App (archivo)" }` |

### `tipo: "arco"`

| Campo | Tipo | Va a | Regla |
|---|---|---|---|
| `codigo`, `familia`, `subtipo` | texto | — | igual que en nodos |
| `desde`, `hasta` | texto | extremos | **obligatorios**; `codigo` de nodos del mismo plan (o ya en el proyecto). Inexistente → descarte; iguales → `E_TOPO_SELF_LOOP` |
| `diametro_mm` | entero | `d_mm` | **diámetro nominal (DN)** en mm, entero |
| `cota_solera_inicio` | número | tramo | solera del tubo en `desde` (m). Manda sobre la del pozo (caídas, saltos) |
| `cota_solera_fin` | número | tramo | solera del tubo en `hasta` (m) |
| `vertices` | `[[E,N,Z], …]` | trazado | ≥ 2 puntos, del inicio al fin, Z = **solera** en cada vértice. Con < 2 el trazado va recto de `cota_solera_*` (o la solera del pozo) |
| `attrs` | objeto | atributos | `diametro_interior` (mm, lo usa el dibujo), `espesor_pared` (m), `cota_clave`, `cota_clave_fin`, `material`… |
| `datos` | objeto | `Dato` | igual que en nodos |

### `datos` frente a `attrs`

- `attrs`: atributos del **registro de la familia** (lista con `redes esquema --familia X`). Se
  editan, se exportan y vuelven en la ida y vuelta.
- `datos`: números **del origen** con unidad y fuente que no son atributos (longitud declarada,
  caudal, demanda, desfases, nivel inicial…). La `fuente` es `"<App> (<archivo>)"`.
- Un número que no cabe en ninguno de los dos no se fuerza: se cita en el informe.

## 3. Trampas del contrato (lee esto antes de escribir)

1. **Los campos desconocidos se ignoran en silencio.** `trazado` en vez de `vertices`,
   `cota_solera_inicial`, `nota`, `estado` o `unidades` no dan error: se pierden. Usa solo los
   nombres de §2.
2. **El importador aún rellena huecos** (F12-03 pendiente): sin `cota_terreno` toma `cota`; sin
   `cota` pone terreno − 1,5; sin `diametro_mm` pone DN 200; sin `subtipo` el primero. Por eso
   **todo lo ausente se lista en el informe** al usuario: lo que el visor muestre ahí no es dato.
3. **No hay campo de tapa separado de la rasante**: `cota_terreno` alimenta ambos. Si el origen
   trae tapa y rasante distintas, `cota_terreno` = rasante y `attrs.cota_tapa` = tapa (pozo).
4. **Unidades mezcladas**: `diametro` del nodo en **m**; `diametro_mm` del arco y
   `attrs.diametro_interior` del arco en **mm**; cotas y longitudes en m; caudales en L/s; presión
   en m.c.a. (unidades de los atributos: columna `unidad` de `redes esquema`).
5. **Códigos repetidos** (también contra el proyecto abierto) → descarte. Importa en un proyecto
   nuevo; reimportar sobre el mismo duplica códigos.
6. Las **claves retiradas** se traducen solas (`agua.captacion:toma_rio` →
   `agua.bocatoma:barraje_fijo`) y salen en `renombrados`. No las uses a propósito.
7. **Atributos `enum`**: `redes esquema` dice `"tipo": "enum"` pero no lista las opciones. Búscalas en
   `crates/redes-core/src/families*.rs` (`AttrType::Enum(&[...])` junto a la clave) o pregunta.

## 4. Cotas: solera, clave, rasante, tapa y recubrimiento

| Nombre | Qué es | En el plan |
|---|---|---|
| **Solera** (invert, fondo, *cota de fondo*) | punto más bajo del **interior** del tubo o del pozo | nodo `cota`; tramo `cota_solera_inicio/fin` y Z de `vertices` |
| **Clave** (crown, *corona interior*) | punto más alto del interior del tubo: **clave = solera + DI** | tramo `attrs.cota_clave` (inicio) y `attrs.cota_clave_fin` |
| **Eje** | centro del tubo: solera + DI/2 | no va en el plan (lo dibuja Redes) |
| **Rasante / terreno** | superficie del terreno o de la vía | nodo `cota_terreno`; tramo `attrs.cota_terreno` si el origen lo da |
| **Tapa** (rim) | cota de la tapa del pozo | `cota_terreno` si coincide con la rasante; si no, `attrs.cota_tapa` |
| **Profundidad del pozo** | tapa − solera | `attrs.profundidad` solo si el origen la declara |
| **Recubrimiento** | rasante − (clave + espesor de pared) = tierra sobre el tubo | `attrs.recubrimiento` (agua) o dato, **solo si el origen lo declara** |

Reglas:

- Una columna de **clave nunca se copia como solera**. Si el origen solo trae clave y declara el
  DI, solera = clave − DI (conversión de referencia; díselo al usuario). Sin DI, la solera queda
  ausente. Nunca al revés con un DI supuesto.
- «Profundidad» no es «recubrimiento»: la primera mide hasta la solera, el segundo hasta el lomo
  exterior del tubo.
- Una cota que en el origen es **hidráulica** (nivel de agua, carga de un reservorio, cota de un
  nudo EPANET) no es terreno ni solera: va a `datos` con su nombre, salvo lo que dicen las
  referencias de formato.

## 5. Familias, subtipos y atributos válidos

```powershell
redes esquema --pretty > esquema.json          # todo: familias, comandos, códigos de error
redes esquema --familia saneamiento.pozo --pretty   # filtra por subcadena de la clave
redes capacidades                               # alias de `redes esquema` (sin filtro)
```

Por familia, `esquema` da: `clave`, `rol` (`node`, `link`, `device`, `support`), `geometria`,
`prefijo`, `atributos[]` (`clave`, `unidad`, `tipo`, `requerido`) y `subtipos[]` (`clave`,
`requeridos`). Reglas:

- Familias con `rol: link` van como `arco`; las demás (`node`, `device`, `support`) como `nodo`.
- Un atributo `requerido` que el origen no trae **queda ausente**: se lista, no se rellena.
- No inventes claves de atributo: si el dato no tiene atributo, va a `datos`.

## 6. Validar e importar

```powershell
redes proyecto nuevo --nombre prueba --crs EPSG:32717 --salida out\prueba.redes
redes -p out\prueba.redes --dry-run importar plan --archivo plan.json   # ver nota
redes -p out\prueba.redes --pretty importar plan --archivo plan.json
redes -p out\prueba.redes objeto lista
redes -p out\prueba.redes vista --salida out\vista.svg
```

- **Nota:** hoy `--dry-run` solo confirma que el archivo existe (devuelve `bytes`); no valida
  objetos. Mientras sea así, valida importando en un **proyecto de prueba** nuevo y lee
  `detalle_descartes` y `renombrados`. Cuando `--dry-run` informe descartes, úsalo primero.
- Importar con descartes sigue siendo `ok:true`: **lee siempre** `detalle_descartes`.
- Errores de archivo: `E_FMT_PARSE` (JSON roto o campo obligatorio ausente, p. ej. `e` o
  `familia`), `E_FMT_UNSUPPORTED` (esquema), `E_IO_NOT_FOUND`.
- Motivos de descarte típicos: `familia desconocida`, `subtipo … no existe en …`, `código
  duplicado`, `extremo inexistente (A → B)`, `E_TOPO_SELF_LOOP`.

## 7. Ejemplos trabajados

`crates/redes-core/tests/fixtures/`: `swmm_si_cms`, `swmm_us_cfs`, `epanet_si_lps` y
`epanet_us_gpm` (`.inp` de origen y `.plan.json` resultante). La prueba
`crates/redes-core/tests/ingesta.rs` los importa. Cómo se leyó cada línea: `swmm.md` §6 y
`epanet.md` §6.
