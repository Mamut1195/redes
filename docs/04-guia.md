# 04 — Guía de uso

> Para quien va a **ver, revisar y entregar** una red: ingeniería municipal, consultoría,
> delineación. 20 minutos de lectura; el arranque son 5.

---

## 1. Qué es y qué no es

**Es** un visor 3D de redes ya diseñadas: importa el modelo de otra aplicación, lo muestra sobre
el terreno, permite aprobarlo objeto por objeto y lo devuelve al CAD en DXF.

**No es** un programa de cálculo. No hay hidráulica, ni hidrología, ni cómputos. Si necesita un
caudal, una presión o un volumen, vienen del archivo de origen y la app los muestra con su
procedencia (`SWMM`, `Civil 3D`…). Si el origen no los trae, el inspector muestra `—`.

---

## 2. Antes de empezar

| Requisito | Versión |
|---|---|
| Windows | 10/11 con toolchain MSVC |
| Rust | 1.95 o superior (`rustup` con `x86_64-pc-windows-msvc`) |
| GPU | Cualquiera con DirectX 12 (integrada sirve) |
| Espacio | ~3 GB de `target/` la primera compilación |

```powershell
git clone <repo> ; cd rune-alcantarillado-agua-potable
cargo build --release          # la app necesita release para interactuar fluido
```

La primera compilación tarda varios minutos (compila Rune y wgpu). Es normal.

---

## 3. Arranque en cinco minutos

```powershell
# 1. Un proyecto de ejemplo con terreno, alcantarillado y agua potable
cargo run -p redes-cli --release -- proyecto demo --salida out\la-esperanza.redes

# 1b. La ciudad de demostración (ladera, calles, manzanas y redes)
cargo run -p redes-cli --release -- proyecto ciudad --salida out\ciudad.redes

# 2. Ver el resumen
cargo run -p redes-cli --release -- -p out\la-esperanza.redes proyecto info --pretty

# 3. Abrir el visor
$env:REDES_PROJECT = "out\la-esperanza.redes"
cargo run -p redes-app --release
```

Dentro del visor:

| Acción | Cómo |
|---|---|
| Girar | Arrastrar con el botón izquierdo |
| Desplazar | Arrastrar con el botón derecho (o central) |
| Acercar / alejar | Rueda del ratón |
| Encuadrar todo | Botón **Encuadrar** |
| Vistas fijas | **Planta**, **Isométrica**, botón *Perfil* en el panel izquierdo |
| Apagar una capa | Panel izquierdo → **Capas** |
| Seleccionar un objeto | Clic sobre el pozo o la tubería |
| Leer su ficha | Panel derecho (**Inspector**) |
| Aprobar / observar | Botones del inspector cuando hay algo seleccionado |
| Guardar la revisión | Botón **Guardar** (aparece `Guardar *` si hay cambios) |

---

## 4. La ventana

```
┌───────────────────────────────────────────────────────────────────────────┐
│ Redes   <proyecto> · <CRS> · <estado>          Encuadrar  Planta  Isométrica │
├──────────────┬──────────────────────────────────────────┬─────────────────┤
│ Revisión     │                                          │ Inspector       │
│  · aprobados │            vista 3D                      │  código         │
│  · pendientes│      (terreno + tuberías + pozos)        │  familia/tipo   │
│  · observados│                                          │  estado         │
│              │                                          │  geometría      │
│ Capas        │                                          │  datos de origen│
│  · terreno   │                                          │  atributos      │
│  · alcantar. │                                          │                 │
│  · agua      │                                          │                 │
├──────────────┴──────────────────────────────────────────┴─────────────────┤
│ az -55° el 32° d 884 m · 44 objetos · 2000 malla · 20 líneas   revisión… │
└───────────────────────────────────────────────────────────────────────────┘
```

**Colores:** naranja = alcantarillado; azul = agua potable; verde = objeto seleccionado; rojo =
objeto con observación. El terreno se colorea por cota y se sombrea por pendiente; la exageración
vertical por defecto es 2× (un relieve de 8 m en 600 m no se leería de otro modo); con ciudad es
2,6×.

**City context:** cuando el proyecto trae `entorno` (calles y manzanas), la capa **Edificios**
permite apagarlo para ver las redes bajo la ciudad. Los pozos se dibujan como rombos de tamaño
fijo para que sigan visibles en planta general.

### Cómo se garantizan las cotas

En la ciudad de demostración (`redes proyecto ciudad`) las cotas no son decorativas:

| Regla | Cómo se cumple | Prueba |
|---|---|---|
| La tapa sale a la calle | `z_rim` = cota del terreno en el eje de la calle | `la_tapa_del_pozo_esta_en_la_calle` (±0,35 m) |
| La clave va enterrada | profundidad (tapa − solera) entre 1,1 y 6 m; recubrimiento del tubo (rasante − (solera + DI + espesor)) > 0,5 m | `la_profundidad_del_pozo_es_razonable`, `el_recubrimiento_del_tubo_es_positivo` |
| Todo colector baja | clave decreciente; pendiente ≥ 0,4 % | `todos_los_colectores_bajan` |
| Los laterales acometen al troncal | cada lateral termina en el pozo de su calle | sin arcos colgantes en la importación |

El troncal se diseña aguas abajo (cada pozo baja al menos 0,6 % y nunca queda a menos de 1,2 m de
la tapa); los laterales se diseñan aguas arriba con la clave del troncal como condición de borde.

---

## 5. La CLI

Todo lo que hace la app lo hace la CLI, y al revés. La CLI es la interfaz para agentes: cada
respuesta es un sobre JSON.

```json
{
  "schema": "redes.envelope/1",
  "ok": true,
  "command": "proyecto.info",
  "project": { "name": "Redes La Esperanza", "revision": 1, … },
  "data": { … },
  "diagnostics": [],
  "warnings": [],
  "meta": { "exit_code": 0, … },
  "error": null
}
```

Regla dura: **`ok:false` ⇔ código de salida ≠ 0**.

### 5.1 Proyecto

```powershell
redes proyecto nuevo --nombre "Alcantarillado Norte" --crs EPSG:32717 --salida norte.redes
redes proyecto demo --salida la-esperanza.redes
redes proyecto info --pretty
redes proyecto aprobar          # falla si quedan objetos pendientes (E_STATE_PENDING) u observados (E_STATE_OBSERVED)
```

Resolución del proyecto: `--project` / `-p` → `$REDES_PROJECT` → el primer `*.redes` de la
carpeta o de sus padres.

### 5.2 Importar

```powershell
# El contrato de la skill: todo lo que viene de otra aplicación entra así (D5)
redes importar plan --archivo plan.json

# La vuelta del DXF que exportó Redes (capa REDES_INFO)
redes importar dxf --archivo plano-devuelto.dxf

# Puntos de terreno E,N,Z (sin --epsg toma el del proyecto)
redes importar terreno --archivo puntos.csv --epsg 32717
```

**Sistema de coordenadas (F12-13).** Redes trabaja en UTM sur WGS 84, en metros
(`EPSG:327NN`; 17S = 32717, 18S = 32718). El plan **declara** su CRS en `proyecto.crs` y no se
supone ninguno. Devuelven `E_CRS` sin tocar el proyecto: un plan sin CRS o con uno que no es UTM
sur (p. ej. 32617, que es 17**N**, o 4326); un plan o un terreno en otro CRS que el del proyecto
(Redes no reproyecta; un proyecto vacío adopta el del primer plan o terreno, y se avisa); y
coordenadas que parecen grados lon/lat (|E| ≤ 180 y |N| ≤ 90).

La app **no adivina** formatos ajenos. Un DXF de Civil 3D, un GeoJSON de un SIG, un `.inp` o un
Excel los adapta un agente con la skill `skills/redes-ingesta`, que escribe un `redes.plan/1`.
`importar dxf` con un DXF que no exportó Redes devuelve `E_FMT_NOT_REDES` (y uno vacío o
truncado, `E_FMT_PARSE`) sin tocar el proyecto.

Cada importación devuelve cuántos entraron, **por qué** se descartó el resto y qué datos
faltaban en lo que entró:

```json
{ "formato": "plan", "nodos": 24, "arcos": 20, "descartados": 0, "detalle_descartes": [],
  "avisos": 1, "detalle_avisos": ["arco C7: sin diametro_nominal (se muestra «—» y se dibuja como línea sin dato)"] }
```

**No se rellena nada (F12-03).** Lo que el plan no trae queda ausente: sin `cota_terreno` no se
usa la solera, sin `cota` (solera) no se pone «terreno − 1,5», la tapa (`cota_tapa`) no se copia
del terreno y sin `diametro_mm` no hay DN 200. El inspector y `objeto lista` muestran `—` (o
`null`) y la vista dibuja «sin dato»: un disco gris plano para el nodo sin altura y una línea gris
fina para el tramo sin DN o sin cota. La única cota que se completa es la de terreno desde el TIN
del propio plan, y se avisa. Un nodo sin ninguna cota se descarta con motivo.

### 5.3 Revisar

```powershell
redes objeto lista --estado pendiente
redes objeto aprobar --codigo PZ-001
redes objeto observar --codigo CL-014 --nota "Revisar cota de salida en campo"
redes objeto reabrir --codigo CL-014
```

La nota se guarda con el objeto y se ve en el inspector de la app. Lo mismo se puede hacer
desde el visor: seleccionar y pulsar **Aprobar** u **Observar**, y luego **Guardar**.

#### Comparar dos entregas (`proyecto diff`)

```powershell
redes proyecto diff --a entrega-1.redes --b entrega-2.redes
redes proyecto diff --a entrega-1.redes --b entrega-2.redes --tolerancia-pos 0.05 --solo diseno --salida diff.csv
```

Empareja los objetos por **código** (el `ElementId` cambia entre entregas) y reporta altas, bajas y,
por objeto, los campos que cambiaron: familia y subtipo, posición, cotas (tolerancia 0,001 m), DN,
atributos, datos (valor, unidad y fuente), tramo (desde, hasta y vértices) y los datos ausentes:
un dato que aparece o desaparece (`—`) es un cambio. Un movimiento en planta menor que
`--tolerancia-pos` (0,01 m por defecto) no se reporta.

Los campos de revisión (estado, nota y, si el proyecto las trae, observaciones) llevan
`clase: "revision"`; el resto es `clase: "diseno"`. `--solo diseno|revision` filtra por clase. Dos
proyectos con CRS distinto dan `E_STATE_CRS_DISTINTO` (código de salida 3).

El sobre `redes.envelope/1` trae `data.resumen` (altas, bajas, objetos modificados, cambios por clase
y `por_familia`) y el detalle en `data.altas`, `data.bajas` y `data.modificados[].cambios[]`
(`campo`, `antes`, `despues`, `clase`), en orden determinista (nodos antes que tramos, por código).
`--salida` escribe un CSV con BOM y `;`, una fila por cambio, alta o baja.

### 5.4 Grabar una demo (o una prueba de interfaz)

No hace falta Playwright: la app se graba a sí misma. Un guion de eventos y la bandera `--video`:

```powershell
# docs/demo/recorrido.json describe los clics, arrastres y rueda
cargo run -p redes-app --release -- --scene detalle `
  --script docs\demo\recorrido.json `
  --video out\demo.mp4 --duration 11 --pointer
```

- `--script` admite `click`, `drag`, `wheel`, `key`, `type` y `paste`, con `key` (nombre de un
  elemento) o `x`/`y` en puntos de la ventana.
- `--pointer` dibuja el ripple del clic; `--keys` superpone las teclas.
- `--snap` toma una foto en un instante y `--tree` vuelca el árbol de elementos: con eso se
  verifica una interfaz headless, sin abrir ventana.
- Los chips y botones tienen claves estables (`btn-aprobar`, `btn-observar`, `btn-guardar`,
  `btn-planta`, `btn-iso`, `btn-encuadrar`) para que un guion los encuentre.

Resultado: [demo.mp4](img/demo.mp4) y su vista previa [demo.gif](img/demo.gif).

### 5.5 Ver sin abrir la app

```powershell
redes vista --salida vista.svg --azimut -55 --elevacion 32
```

Es la misma escena 3D que dibuja la app, en SVG: se abre en el navegador y un agente puede
inspeccionarla o convertirla a PNG.

### 5.6 Entregar

```powershell
redes exportar dxf --salida plano.dxf                     # todo el modelo
redes exportar dxf --salida aprobado.dxf --solo-aprobados # sólo lo aprobado
redes exportar geojson --salida red.geojson               # para SIG, con los datos de origen
```

El DXF es el que se abre en AutoCAD/Civil 3D/BricsCAD para el **detallado y el DWG final**
(R12 ASCII, `$INSUNITS = 6` metros, `$DWGCODEPAGE = ANSI_1252`, extensión declarada para que
«Zoom extensión» encuadre la red):

- **Capas** declaradas en la tabla LAYER, una por familia y sin tildes (`PZ_POZO`, `CL_TUBERIA`,
  `TB_TUBERIA_DE_AGUA_POTABLE`…), con color ACI por sistema: agua potable 5 (azul), sanitario 30
  (naranja), pluvial 3 (verde, sufijo `_PLU` cuando la familia es de saneamiento), combinado 6,
  común 8. Rotulación aparte en `ROT_<AP|SAN|PLU|COMB|COM>_TRAMOS` y `ROT_…_NODOS`.
- **Nodos**: bloque con círculo del diámetro interior declarado (radio = DI/2; sin DI declarado,
  símbolo a escala 1) y atributos ocultos `CODIGO`,
  `SUBTIPO`, `COTA_TAPA`, `COTA_SOLERA`, `PROFUNDIDAD`; punto en la solera; línea vertical que baja
  de la tapa a la solera. Rótulo: código, `CT` (tapa), `CS` (solera) y `H` (profundidad).
- **Tramos**: polilínea 3D y rótulo `código DN200 PVC L=45.20 m i=0.50 %`. La pendiente es la
  declarada (atributo `pendiente`); si no hay, se deriva de las soleras declaradas del tramo
  (inicio y fin) y la longitud en planta y se marca `(soleras)`; si hay las dos y no cuadran, salen
  ambas (`i=2.00 % (soleras 0.00 %)`): es una discrepancia, no se corrige.
- **Solo lo que existe**: si falta un dato (DN, material, una cota) no se escribe; nunca se inventa.
- **`OBSERVACIONES`** (rojo): código, estado y nota de cada objeto observado, aprobado o con nota;
  los observados llevan además un círculo (nube) alrededor.
- **`REDES_INFO`** (apagada): la carga de la ida y vuelta que lee `redes importar dxf` (familia,
  subtipo, código, atributos y datos de nodos y tramos).
- Texto de 1,0 m (2 mm a 1:500). Lo visible es ASCII: las tildes de los textos van como `\U+00ED`,
  que el CAD muestra como «í». Solo la carga de `REDES_INFO` va en UTF-8 tal cual (es para el
  importador; en el CAD puede verse con caracteres raros, como `mÂ³`).

---

## 6. Flujo de aprobación

```
importar  →  revisar en 3D  →  aprobar / observar  →  proyecto aprobar  →  exportar
                │                     │
                │                     └─ observado: vuelve a la app de origen
                └─ pendiente: nadie lo ha mirado
```

1. Importar (los objetos nacen **pendientes**).
2. Recorrer el modelo con la lista de pendientes y la vista 3D.
3. Aprobar lo correcto; observar lo que debe corregirse en la herramienta de diseño.
4. `redes proyecto aprobar` verifica que no quede nada pendiente.
5. Exportar a DXF y entregar.

**Por qué por objeto y no un botón:** el acta de revisión debe poder decir qué se aprobó y qué
no. Un «aprobar todo» convierte la revisión en un trámite.

---

## 7. Entrada de datos desde otras aplicaciones

Los formatos ajenos los convierte un agente con la skill `skills/redes-ingesta` a un plan
`redes.plan/1`, que se valida e importa con `redes importar plan` (D5). La skill tiene una
referencia por formato en `skills/redes-ingesta/references/`:

| Origen | Camino | Referencia |
|---|---|---|
| DXF escrito por Redes (capa `REDES_INFO`) | `redes importar dxf` | — |
| Civil 3D / AutoCAD (DXF ajeno, LandXML) | skill → `importar plan` | `dxf-civil3d.md` |
| SWMM | skill → `importar plan` | `swmm.md` |
| EPANET | skill → `importar plan` | `epanet.md` |
| Excel / CSV / informes de Civil 3D | skill → `importar plan` | `tablas.md` |
| SIG (GeoJSON, Shapefile) | skill → `importar plan` | `geojson-sig.md` |
| Terreno `E,N,Z` | `redes importar terreno` | — |

Contrato del plan: `references/plan.md`. Procedimiento y reglas: `skills/redes-ingesta/SKILL.md`.

**Regla:** un nodo sin planta no se dibuja ni se coloca en (0,0): se pide la coordenada o se
descarta con su motivo. Sin planta no hay 3D.

---

## 8. Problemas frecuentes

| Síntoma | Causa | Solución |
|---|---|---|
| «E_CRS» al importar | El plan no declara CRS, no es UTM sur (`32617` es 17**N**), no coincide con el proyecto o trae grados lon/lat | Declarar `EPSG:32717` (17S) o `EPSG:32718` (18S) y coordenadas UTM en metros; reproyectar en origen |
| Todo apilado en un punto | El origen no traía coordenadas | Pasar `--coords` a la skill |
| «E_TOPO_ORPHAN_LINK» | Un arco apunta a un nodo que no entró | Revisar `desde`/`hasta` en el plan |
| «E_STATE_DUPLICATE_NAME» | Código repetido | Renombrar en el origen |
| El pozo sale como un disco gris plano | Falta la solera, o faltan a la vez tapa y terreno | Declarar `cota` (solera) y `cota_terreno`/`cota_tapa` en el plan, o el TIN |
| El tramo sale como una línea gris fina | Le falta el DN o la cota de solera (ni la suya ni la de los pozos) | Declarar `diametro_mm` y `cota_solera_inicio/fin` |
| El inspector muestra `—` | El origen no traía ese dato | Es correcto: no se inventa |
| Un nodo aparece en `detalle_descartes` «sin ninguna cota» | No trae solera, tapa ni terreno, y no hay TIN en su planta | Darle al menos una cota en el plan |
| La app va lenta al girar | Compilada en debug | `cargo build --release` y usar `--release` |
| «E_FMT_NOT_REDES» al importar un DXF | El DXF no lo exportó Redes (no trae la capa `REDES_INFO`) | Adaptarlo con la skill de ingesta e `importar plan` |

---

## 9. Glosario mínimo

| Término | Significado |
|---|---|
| **Pozo** | Cámara de registro de alcantarillado (manhole) |
| **Colector** | Tubería de alcantarillado por gravedad |
| **Emisario** | Tramo final hasta el cuerpo receptor |
| **Vertedero / descarga** | Estructura de entrega o alivio |
| **Llave** | Dispositivo en la frontera red–usuario (corporación, banqueta, hidrante) |
| **Válvula** | Dispositivo dentro de la red (compuerta, mariposa, retención, aire…) |
| **Cota de solera (invert)** | Fondo interior de la tubería o del pozo |
| **Cota de clave** | Punto más alto del interior de la tubería (clave = solera + DI). No es la solera |
| **Cota de tapa** | Boca del pozo / rasante |
| **CRS** | Sistema de referencia de coordenadas (aquí UTM) |
| **TIN** | Malla triangular del terreno |
| **Dato** | Número que vino de otra app, con unidad y fuente; nunca se recalcula |

---

## 10. Límites (leer antes de prometer)

1. **No calcula.** Ni hidráulica, ni hidrología, ni excavación, ni presupuesto.
2. **No verifica cumplimiento normativo.**
3. **No edita el diseño**: se corrige en la aplicación de origen y se reimporta.
4. **No genera DWG nativo**: entrega DXF; el DWG se guarda desde el CAD.
5. **No es colaborativo en tiempo real**: un archivo, un revisor.
6. La vista 3D usa algoritmo del pintor: con geometría que se cruza puede haber algún artefacto
   de orden; el detalle fino se ve en el CAD.
