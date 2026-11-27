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
| `schema` | texto | `redes.plan/1` | exactamente `redes.plan/1` (D10); otra versión de `redes.plan/*` → `E_FMT_VERSION`; algo que no es un plan → `E_FMT_UNSUPPORTED`; si falta, se toma `/1` |
| `proyecto.nombre` | texto | **obligatorio** (puede ser `""`) | si no está vacío, renombra el proyecto |
| `proyecto.crs` | texto | `EPSG:32717` | **Pregúntalo** si el origen no lo declara. Ecuador continental: `EPSG:32717` (17S) o `EPSG:32718` (18S). `32617` es 17N: desplaza 10 000 km |
| `proyecto.fuente_app` | texto | `"plan"` | app y versión de origen (`"EPANET 2.2"`) |
| `proyecto.fuente_archivo` | texto | ruta del plan | nombre del archivo de origen, no del plan |
| `terreno` | `[[E,N,Z], …]` | sin terreno | puntos del TIN, en el mismo CRS. Hoy el TIN se etiqueta siempre como 32717 |
| `objetos` | lista | vacía | `nodo` y `arco`, en cualquier orden (los nodos se crean antes) |

### `tipo: "nodo"`

| Campo | Tipo | Va a | Regla |
|---|---|---|---|
| `codigo` | texto | código | el del origen, tal cual. Vacío → el registro pone uno correlativo. **Único entre nodos y tramos** (D11): un tramo no puede llevar el código de un nodo ni al revés; si el origen los repite (p. ej. SWMM permite un nodo y un conducto `C1`), desambígualo y anótalo |
| `familia` | texto | familia | **obligatorio**; clave de `redes esquema` |
| `subtipo` | texto | subtipo | si falta, el primero del registro: **no lo dejes vacío**, pregunta |
| `e`, `n` | número | planta | **obligatorios**, en metros del CRS. Sin planta no hay nodo: se lista |
| `cota` | número | `z_invert` | **solera**: fondo interior del pozo o cota de fondo de la estructura (m) |
| `cota_terreno` | número | `z_terrain` | rasante/terreno en el punto (m). Sin él y con `terreno` en el plan, se toma del TIN (con aviso) |
| `cota_tapa` | número | `z_rim` | tapa del pozo (m). Si falta queda ausente: **no** se copia el terreno |
| `cota_eje` | número | `datos.cota_eje` | **redes a presión**: cota del eje del tubo en el nudo (`Elev` de EPANET). No es solera: solo ubica el nodo en altura cuando no hay otra cota; el nodo queda sin solera, tapa ni terreno (se dibuja plano) |
| `diametro` | número | `attrs.diametro_interior` | diámetro interior del **pozo en metros** (1.2), no en mm. > 20 m → aviso de unidad |
| `desde`, `hasta` | texto | enlace | **solo dispositivos de enlace** (bomba, válvula, vertedero): `codigo` de los nodos que une en el origen. Ver §2.1 |
| `attrs` | objeto | atributos | claves del registro de la familia; valores número, texto o booleano |
| `datos` | objeto | `Dato` | `{ "valor": número, "unidad": "m", "fuente": "App (archivo)" }` |

### `tipo: "arco"`

| Campo | Tipo | Va a | Regla |
|---|---|---|---|
| `codigo`, `familia`, `subtipo` | texto | — | igual que en nodos |
| `desde`, `hasta` | texto | extremos | **obligatorios**; `codigo` de nodos del mismo plan (o ya en el proyecto). Inexistente → descarte; iguales → `E_TOPO_SELF_LOOP` |
| `diametro_mm` | entero | `d_mm` | **diámetro nominal (DN)** en mm, entero. < 10 → aviso de unidad. Sin DN el tramo queda sin diámetro (nunca se pone uno) |
| `diametro_interior_mm` | número | `attrs.diametro_interior` | **diámetro interior (DI)** en mm, el que dan SWMM (`Geom1`) o EPANET (`Diam`). < 10 → aviso |
| `seccion` | texto | `attrs.seccion` | forma: un valor de la lista `seccion` de la familia (`redes esquema --familia X`, campo `valores`). Fuera de lista o familia sin `seccion` → descarte con la lista |
| `alto_m`, `ancho_m` | número | `datos.altura_seccion`, `datos.ancho_seccion` | alto y ancho interiores (m) de una sección no circular. No se convierten en DN |
| `cota_solera_inicio` | número | tramo | solera del tubo en `desde` (m). Manda sobre la del pozo (caídas, saltos) |
| `cota_solera_fin` | número | tramo | solera del tubo en `hasta` (m) |
| `vertices` | `[[E,N,Z], …]` o `[[E,N], …]` | trazado | ≥ 2 puntos, del inicio al fin. Con Z: Z = **solera** en cada vértice. **Sin Z**: ver §2.2. Mezclar los dos → descarte. Con < 2 el trazado va recto de `cota_solera_*` (o la solera del pozo) |
| `attrs` | objeto | atributos | `espesor_pared` (m), `cota_clave`, `cota_clave_fin`, `material`… (el DI va mejor en `diametro_interior_mm`) |
| `datos` | objeto | `Dato` | igual que en nodos |

### 2.1 Dispositivos de enlace (bombas, válvulas, vertederos)

En SWMM y EPANET una bomba, una válvula o un vertedero es un **enlace** entre dos nodos; en Redes
es un **nodo**. Para que la red no quede cortada, el nodo declara `desde` y `hasta` con los
códigos del origen (`Node1`/`Node2`, `From`/`To`). Redes no crea tramos: la topología los ve
unidos. Si el propio dispositivo es un extremo (la cámara húmeda que representa a la bomba de
SWMM), declara solo el otro lado. Un código inexistente o el propio nodo → aviso, sin unir ese
lado. La unión viaja en el `.redes` y en el DXF de ida y vuelta (`REDES|1|E`).

```json
{ "tipo": "nodo", "codigo": "PRV1", "familia": "agua.valvula", "subtipo": "reduccion_presion",
  "e": 760420.0, "n": 9795300.0, "cota_eje": 2505.2, "desde": "N4", "hasta": "N5" }
```

No partas un tramo en dos para meter el dispositivo ni inventes tramos cortos que lo unan.

### 2.2 Vértices sin Z

Si el origen da la planta de los vértices pero no su cota (EPANET, un SIG 2D), escríbelos como
`[E,N]`. Redes interpola la Z **por longitud en planta** entre la solera del inicio y la del fin
(`cota_solera_inicio/fin`; si faltan, la del pozo del extremo). Los extremos siguen siendo dato;
la Z de los vértices intermedios se marca como derivada (`sin_dato: cota_vertices`, aviso). Si
falta la solera de algún extremo, el tramo queda sin cotas de dato (línea gris). No calcules tú
la Z de los vértices.

```json
{ "tipo": "arco", "codigo": "T1", "familia": "agua.tuberia", "subtipo": "impulsion",
  "desde": "NB", "hasta": "TQ1", "diametro_mm": 160,
  "vertices": [[759505.0, 9795005.0], [759800.0, 9795200.0], [760100.0, 9795450.0]] }
```

### 2.3 Secciones no circulares y diámetro interior

```json
{ "tipo": "arco", "codigo": "C1", "familia": "saneamiento.tuberia", "subtipo": "colector_pluvial",
  "desde": "J1", "hasta": "J2", "diametro_mm": 600, "diametro_interior_mm": 600.0 },
{ "tipo": "arco", "codigo": "G-5", "familia": "saneamiento.tuberia", "subtipo": "colector_sanitario",
  "desde": "MH4", "hasta": "OUT1", "seccion": "ovoide", "alto_m": 0.6096 }
```

`G-5` no lleva `diametro_mm`: un ovoide no tiene DN y Redes no le pone uno.

### 2.4 Unidades (siempre las mismas)

| Magnitud | Unidad | Campos |
|---|---|---|
| Coordenadas, cotas, longitudes | m | `e`, `n`, `cota*`, `vertices`, `alto_m`, `ancho_m` |
| Diámetro de un **nodo** (pozo, estructura) | **m** | `diametro` |
| Diámetro de un **tubo** (DN y DI) | **mm** | `diametro_mm`, `diametro_interior_mm`, `attrs.diametro_interior` del arco |
| Caudal / presión | L/s / m.c.a. | atributos y `datos` |

La CLI **avisa** (no corrige) lo que parece otra unidad: `diametro` de nodo > 20 m, DN o DI de tubo
< 10 mm, `alto_m`/`ancho_m` > 20 m. Si sale el aviso, revisa la conversión; no lo ignores.

### `datos` frente a `attrs`

- `attrs`: atributos del **registro de la familia** (lista con `redes esquema --familia X`). Se
  editan, se exportan y vuelven en la ida y vuelta.
- `datos`: números **del origen** con unidad y fuente que no son atributos (longitud declarada,
  caudal, demanda, desfases, nivel inicial…). La `fuente` es `"<App> (<archivo>)"`.
- Un número que no cabe en ninguno de los dos no se fuerza: se cita en el informe.

## 3. Trampas del contrato (lee esto antes de escribir)

1. **Un campo desconocido rechaza el plan entero** con `E_FMT_CAMPO` (código 7): el mensaje dice
   el objeto (`objetos[3] (arco C1)`), el campo y el nombre válido más parecido (`«trazado»
   (¿quiso decir «vertices»?)`). Vale también para `proyecto` y para las claves de cada dato
   (`valor`, `unidad`, `fuente`). Corrige y repite; no se importa nada hasta que esté limpio.
2. **Nada se rellena** (F12-03): sin `cota_terreno` no hay terreno (salvo el TIN del plan, con
   aviso); sin `cota` no hay solera; sin `cota_tapa` no hay tapa; sin `diametro_mm` no hay DN; sin
   `subtipo`, el primero del registro (**no lo dejes vacío**, pregunta). Un nodo sin ninguna cota
   (ni `cota_eje`) se descarta. Todo lo ausente sale en `detalle_avisos`.
3. **Tapa y rasante**: `cota_tapa` es la tapa; `cota_terreno`, la rasante. Si el origen solo da
   una y es la tapa, ponla en `cota_tapa` (y en `cota_terreno` solo si el origen dice que coinciden).
4. **Unidades**: §2.4. Diámetro de nodo en m; de tubo en mm.
5. **Códigos repetidos** (también contra el proyecto abierto) → descarte. Importa en un proyecto
   nuevo; reimportar sobre el mismo duplica códigos.
6. Las **claves retiradas** no se admiten: un objeto con `agua.captacion:toma_rio` se descarta
   con `subtipo 'toma_rio' no existe en agua.captacion`. Escribe la clave nueva
   (`agua.bocatoma:barraje_fijo`; tabla completa en docs/20 §8.1).
7. **Atributos de lista** (`"tipo": "enum"`): `redes esquema --familia X` da sus valores
   admitidos en `valores`. Usa uno de ellos tal cual; si ninguno encaja, pregunta.

## 4. Cotas: solera, clave, rasante, tapa y recubrimiento

| Nombre | Qué es | En el plan |
|---|---|---|
| **Solera** (invert, fondo, *cota de fondo*) | punto más bajo del **interior** del tubo o del pozo | nodo `cota`; tramo `cota_solera_inicio/fin` y Z de `vertices` |
| **Clave** (crown, *corona interior*) | punto más alto del interior del tubo: **clave = solera + DI** | tramo `attrs.cota_clave` (inicio) y `attrs.cota_clave_fin` |
| **Eje** de un tramo | centro del tubo: solera + DI/2 | no va en el plan (lo dibuja Redes) |
| **Eje** de una red a presión | cota del nudo en EPANET (`Elev`) | nodo `cota_eje` (no `cota`) |
| **Rasante / terreno** | superficie del terreno o de la vía | nodo `cota_terreno`; tramo `attrs.cota_terreno` si el origen lo da |
| **Tapa** (rim) | cota de la tapa del pozo | nodo `cota_tapa` |
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
`prefijo`, `atributos[]` (`clave`, `unidad`, `tipo`, `requerido` y, si es una lista, `valores`)
y `subtipos[]` (`clave`,
`requeridos`). Reglas:

- Familias con `rol: link` van como `arco`; las demás (`node`, `device`, `support`) como `nodo`.
- Un atributo `requerido` que el origen no trae **queda ausente**: se lista, no se rellena.
- No inventes claves de atributo: si el dato no tiene atributo, va a `datos`.

## 6. Validar e importar

```powershell
redes proyecto nuevo --nombre prueba --crs EPSG:32717 --salida out\prueba.redes
redes -p out\prueba.redes --pretty --dry-run importar plan --archivo plan.json   # valida, no escribe
redes -p out\prueba.redes --pretty importar plan --archivo plan.json
redes -p out\prueba.redes objeto lista
redes -p out\prueba.redes vista --salida out\vista.svg
```

- `--dry-run` hace **toda** la validación y la importación en una copia en memoria y devuelve el
  mismo informe que la importación real (`nodos`, `arcos`, `detalle_descartes`,
  `detalle_avisos`, errores), con `dry_run: true` y `guardado: false`. El proyecto
  no se escribe. Úsalo siempre primero.
- Importar con descartes sigue siendo `ok:true`: **lee siempre** `detalle_descartes` y
  `detalle_avisos`.
- Errores de archivo (nada se importa): `E_FMT_PARSE` (JSON roto o campo obligatorio ausente,
  p. ej. `e` o `familia`), `E_FMT_CAMPO` (campo que no es del contrato), `E_FMT_UNSUPPORTED`
  (esquema), `E_CRS`, `E_IO_NOT_FOUND`.
- Motivos de descarte típicos: `familia desconocida`, `subtipo … no existe en …`, `código
  duplicado` (también entre un nodo y un tramo, D11), `extremo inexistente (A → B)`, `E_TOPO_SELF_LOOP`, `sin ninguna cota`,
  `seccion «x» no está en la lista`, vértices mezclados.

## 7. Ejemplos trabajados

`crates/redes-core/tests/fixtures/`: `swmm_si_cms`, `swmm_us_cfs`, `epanet_si_lps` y
`epanet_us_gpm` (`.inp` de origen y `.plan.json` resultante). La prueba
`crates/redes-core/tests/ingesta.rs` los importa. Cómo se leyó cada línea: `swmm.md` §6 y
`epanet.md` §6.
