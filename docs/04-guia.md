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
| La clave va enterrada | recubrimiento entre 1,1 y 6 m | `el_recubrimiento_es_razonable` |
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
redes proyecto aprobar          # falla si quedan objetos pendientes
```

Resolución del proyecto: `--project` / `-p` → `$REDES_PROJECT` → el primer `*.redes` de la
carpeta o de sus padres.

### 5.2 Importar

```powershell
# Lo que exporta Civil 3D (capas POZO, AGUA, ALC, EMIS…)
redes importar dxf --archivo levantamiento.dxf --tolerancia 1.0

# Puntos de terreno E,N,Z
redes importar terreno --archivo puntos.csv --epsg 32717

# GeoJSON de un SIG
redes importar geojson --archivo red.geojson

# El contrato de la skill (recomendado para SWMM/EPANET/Excel)
redes importar plan --archivo plan.json
```

Cada importación devuelve cuántos entraron y **por qué** se descartó el resto:

```json
{ "formato": "dxf", "nodos": 24, "arcos": 20, "descartados": 0, "detalle_descartes": [] }
```

### 5.3 Revisar

```powershell
redes objeto lista --estado pendiente
redes objeto aprobar --codigo PZ-001
redes objeto observar --codigo CL-014 --nota "Revisar cota de salida en campo"
redes objeto reabrir --codigo CL-014
```

La nota se guarda con el objeto y se ve en el inspector de la app. Lo mismo se puede hacer
desde el visor: seleccionar y pulsar **Aprobar** u **Observar**, y luego **Guardar**.

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

El DXF es el que se abre en AutoCAD/Civil 3D para el **detallado y el DWG final**. Las capas
salen nombradas (`PZ_POZO`, `CL_TUBERÍA`, `TB_TUBERÍA_DE_AGUA_POTABLE`…), y los pozos llevan
punto, círculo de diámetro y línea de tapa a solera.

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

| Origen | Camino | Documento |
|---|---|---|
| Civil 3D / AutoCAD | `redes importar dxf` (mapeo por capa) | `skills/redes-ingesta/references/mapeos.md` §1 |
| SWMM | `python skills/redes-ingesta/scripts/inp_to_plan.py --tipo swmm` → `importar plan` | §2 |
| EPANET | ídem `--tipo epanet` | §3 |
| Excel / CSV | plan JSON escrito a mano o con un agente | §4 |
| SIG | `redes importar geojson` | §5 |

Detalle completo del contrato y del procedimiento: `skills/redes-ingesta/SKILL.md`.

**Regla:** si el `.inp` no trae coordenadas (SWMM y EPANET no las guardan), hay que pasar un CSV
`codigo,E,N`. Sin planta no hay 3D.

---

## 8. Problemas frecuentes

| Síntoma | Causa | Solución |
|---|---|---|
| El proyecto aparece a 10 000 km del terreno | CRS del hemisferio equivocado (`32617` es 17**N**) | Usar `32717` (17S) o `32718` (18S) |
| Todo apilado en un punto | El origen no traía coordenadas | Pasar `--coords` o importar el DXF |
| «E_TOPO_ORPHAN_LINK» | Un arco apunta a un nodo que no entró | Revisar `desde`/`hasta` en el plan |
| «E_STATE_DUPLICATE_NAME» | Código repetido | Renombrar en el origen |
| El pozo no tiene altura | Sólo llegó la clave, sin tapa | Añadir `cota_terreno` o el TIN |
| El inspector muestra `—` | El origen no traía ese dato | Es correcto: no se inventa |
| La app va lenta al girar | Compilada en debug | `cargo build --release` y usar `--release` |
| El DXF vuelve incompleto | Capas con nombres no reconocidos | Renombrar con la convención o usar un perfil (ING-05) |

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
| **Cota clave (invert)** | Fondo interior de la tubería o solera del pozo |
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
