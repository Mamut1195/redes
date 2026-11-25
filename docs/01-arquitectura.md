# 01 — Arquitectura de Redes

> **Alcance.** Redes es una herramienta de **visualización y revisión** de redes de agua potable
> y alcantarillado. Lee modelos producidos por otras aplicaciones, los muestra en 3D, permite
> aprobarlos y los entrega al CAD. **No calcula ingeniería.**

---

## 1. El límite duro

| Dentro | Fuera |
|---|---|
| Leer y mapear datos de otras apps | Hidráulica (Manning, Hazen-Williams, EPANET…) |
| Guardar geometría, atributos y **datos de origen con procedencia** | Hidrología, caudales de diseño, tormentas |
| Topología para *navegar* y *contar* (qué se conecta con qué) | Movimiento de tierras, cómputos, presupuesto |
| Vista 3D, capas, picking, encuadres | Verificación normativa o «cumplimiento» |
| Revisión por objeto (pendiente / aprobado / observado) | Cualquier número que no venga del archivo fuente |
| Exportar DXF/GeoJSON | Editar el diseño (se corrige en la app de origen) |

**Por qué.** El cliente ya tiene herramientas que calculan (Civil 3D, SWMM, EPANET, Excel). El
riesgo real de un visor que calcula es peor que el de uno que no calcula: dos motores, dos
verdades y nadie sabe cuál es la buena. Redes muestra; el número lo firma quien lo calculó.

**Consecuencia de diseño.** Cada número que aparece en pantalla vive en `Dato { valor, unidad,
fuente }` y se muestra con su origen (`SWMM`, `Civil 3D`, `EPANET`). Si un objeto no trae datos,
el inspector no inventa: muestra `—`.

---

## 2. Flujo de trabajo

```
 ┌────────────┐   exporta    ┌──────────────┐   convierte   ┌────────────────┐
 │ Civil 3D   │─────────────►│  DXF / CSV   │──────────────►│                │
 │ SWMM       │─────────────►│  .inp / .rpt │──(skill)─────►│  redes.plan/1  │
 │ EPANET     │─────────────►│  .inp        │               │  (JSON)        │
 │ Revit/IFC  │─────────────►│  IFC         │               │                │
 │ Excel      │─────────────►│  XLSX/CSV    │               └───────┬────────┘
 └────────────┘              └──────────────┘                       │
                                                                    ▼
                                            ┌──────────────────────────────────┐
                                            │ CLI `redes importar …`           │
                                            │  · valida estructura             │
                                            │  · informa leídos / descartados  │
                                            └───────────────┬──────────────────┘
                                                            ▼
                                     ┌─────────────────────────────────────────┐
                                     │ Documento `*.redes` (JSON, versionado)  │
                                     │  terreno + nodos + arcos + datos +      │
                                     │  procedencia + estado de revisión       │
                                     └───────────────┬─────────────────────────┘
                                                     ▼
                          ┌──────────────────────────────────────────────┐
                          │ Visor 3D (Rune)  ·  CLI `vista` (SVG)        │
                          │ revisar → aprobar / observar → reabrir       │
                          └───────────────┬──────────────────────────────┘
                                          ▼
                          ┌──────────────────────────────────────────────┐
                          │ `redes exportar dxf` → detallado en DWG      │
                          │ `redes exportar geojson` → SIG               │
                          └──────────────────────────────────────────────┘
```

La aprobación es un **estado por objeto**, no un botón global: `redes proyecto aprobar` falla si
queda algo pendiente. Así el visto bueno es auditable: quién aprobó qué y en qué revisión.

---

## 3. Componentes

```
crates/
├── redes-core     modelo · familias · topología · ingest · export · view   (sin UI, sin cálculo)
├── redes-cli      binario `redes` — sobre JSON, mismo motor                (sin GPU)
└── redes-app      visor Rune — órbita, capas, inspector, aprobación       (única capa con UI)
```

| Módulo (`redes-core`) | Responsabilidad | Depende de |
|---|---|---|
| `geom` | P2/P3, distancias, áreas, bbox, índice de segmentos | — |
| `terrain` | Nube de puntos → TIN (Delaunay), cota interpolada, curvas, aligerado para dibujo | `geom` |
| `families` | Registro de las 17 familias: atributos, unidades, subtipos, capacidades | — |
| `model` | `Project`, `Node`, `Link`, `Dato`, estados de revisión, identidad | `geom`, `families` |
| `indice` | Índices de `Project` por id, por código y nodo → tramos (no se guardan) | `model` |
| `topology` | Grafo derivado: incidencia, grados, componentes, sentido de flujo | `model` |
| `ingest` | Plan JSON, DXF, GeoJSON, terreno CSV → `Project` | `model`, `terrain` |
| `export` | DXF (una capa por familia) y GeoJSON | `model` |
| `view` | Cámara orbital, proyección, escena de primitivas ordenadas | `model`, `terrain` |
| `envelope` | Sobre JSON de la CLI y diagnósticos | — |
| `project_io` | Guardado atómico y carga con validación de esquema | `model` |

`view` no dibuja: produce datos (`Vec<Prim>`). Por eso la app y `redes vista` (SVG) muestran
**exactamente** lo mismo, y las pruebas verifican la escena sin GPU.

---

## 4. Modelo de objetos

### 4.1 Tres niveles

| Nivel | Qué es | Ejemplo |
|---|---|---|
| **Familia** (`FamilyDef`) | El contrato: geometría, rol topológico, capacidades, atributos con unidad, subtipos | `saneamiento.pozo` |
| **Tipo** (`TypeDef`) | Subtipo de catálogo con campos obligatorios propios | `saneamiento.pozo.caida` |
| **Instancia** | Un objeto colocado: `ElementId` + familia + tipo + geometría + atributos + datos | `PZ-003` |

Una familia nueva se añade **en un solo lugar** (`families::build()`): la UI, el inspector, la
CLI y el exportador la leen del mismo esquema. No hay `match` por familia en el motor.

### 4.2 Las 17 familias

| Familia | Rol | Geometría | Subtipos |
|---|---|---|---|
| `saneamiento.pozo` | nodo | punto | inspección, caída, resalto, ciego, bombeo, limpieza, ventilación |
| `saneamiento.tuberia` | arco | polilínea 3D | colector, interceptor, emisario, conducto, rebose |
| `saneamiento.descarga` | nodo | punto | emisario final, alivio, rebose de tanque, disipador |
| `saneamiento.camara` | nodo | punto | reunión, carga, quiebre, válvulas, medición |
| `saneamiento.acometida` | arco | polilínea | agua potable, alcantarillado, pluvial |
| `saneamiento.sumidero` | nodo | punto | calzada, cuneta |
| `saneamiento.canal` | arco | polilínea | rectangular, trapezoidal, triangular, circular |
| `saneamiento.sifon` | arco | polilínea | invertido, normal, cruce |
| `saneamiento.bombeo` | nodo | punto | cámara húmeda |
| `agua.tuberia` | arco | polilínea 3D | conducción, aducción, distribución, impulsión |
| `agua.valvula` | dispositivo | punto | compuerta, mariposa, retención, aire, purga, RP, reguladora, alivio |
| `agua.llave` | dispositivo | punto | corporación, banqueta, hidrante, tapón |
| `agua.tanque` | nodo | polígono | apoyado, elevado, cisterna, contacto |
| `agua.captacion` | nodo | punto | manantial, pozo profundo, galería, toma de río |
| `agua.medidor` | dispositivo | punto | domiciliario, macromedidor, electromagnético, ultrasónico |
| `agua.planta` | nodo | polígono | PTAP, PTAR, laguna, desalinizadora |
| `comun.cruce` | apoyo | punto | río, vía, otra red, aéreo, subfluvial |
| `comun.hito` | apoyo | punto | BM, punto de control, sondeo |

Cada familia declara **capacidades** (`hydraulic_node`, `hydraulic_edge`, `structural`,
`attachable`, `source`, `consumes_flow`, `reportable`). La UI y los exportadores preguntan por
capacidades, no por nombres.

### 4.3 Las cuatro cotas

| Cota | Campo | Para qué |
|---|---|---|
| Terreno natural | `z_terrain` | apoyo visual del objeto y altura del pozo |
| Tapa / rasante | `z_rim` | boca del pozo, tapa |
| Clave (invert) | `z_invert` | fondo interior; extremo del tubo |
| Fondo de excavación | *(dato del origen)* | si la app de origen lo trae, se muestra como `Dato` |

### 4.4 Identidad y procedencia

- `ElementId { slot, generation }`: 8 bytes, generacional, estable entre archivo, CLI y UI.
- `code` legible por familia (`PZ-001`, `CL-014`), único en el proyecto.
- `datos: BTreeMap<String, Dato>`: números del origen, con unidad y app.
- `fuentes: Vec<Fuente>`: qué archivos y apps entraron y cuándo.
- `estado: EstadoObjeto` por objeto; `estado: EstadoProyecto` global.
- Índices (F14-01, `redes_core::indice`): `node`/`link`/`node_by_code`/`link_by_code` en O(1) y
  `tramos_de(nodo)` en O(grado). Guardan posiciones en `nodes`/`links`, **borrados incluidos**
  (una baja no los toca: se filtra al leer). No se serializan: `project_io::load` y la reimportación
  llaman a `reindexar()`; las altas (`add_node`/`add_link`) los mantienen y un clon (deshacer) los
  copia válidos. Si `nodes`/`links` se cambian a mano, el índice se sabe desfasado (cuenta y último
  id) y las búsquedas vuelven al recorrido lineal hasta la siguiente alta, baja o `reindexar()`.
  Reescribir en su sitio el id, el código o los extremos de un objeto exige `reindexar()` (F15-02
  cerrará los campos).

---

## 5. Cómo se comunican los objetos

**No se comunican.** Los objetos no tienen punteros entre sí:

```
Node ──(id)──┐
             ├── Link { from, to }        ← la única "referencia", validada al crear
Node ──(id)──┘

Topology::build(&project)              ← se deriva cada vez que hace falta
   ├── incident: nodo → [arcos]
   ├── degree:   nodo → nº de arcos
   ├── components: [ [nodos] ]
   └── flow_forward: arco → bool (por cotas)
```

- Un arco nace solo si sus dos extremos existen (`E_TOPO_ORPHAN_LINK`).
- Un nodo no se borra si tiene arcos (`E_STATE_REFERENCED`).
- Las válvulas, llaves y medidores son **dispositivos** (`attachable`): se dibujan sobre la red,
  no parten el arco. Partirlo obligaría a recalcular el modelo cada vez que se mueve una válvula,
  que es exactamente el tipo de acoplamiento que este visor evita.
- El sentido de flujo es un dato derivado de las cotas, no una propiedad editable.

**Ventaja para la revisión:** mover una válvula o borrar un pozo no puede corromper el modelo;
como mucho produce un diagnóstico de estructura.

---

## 6. Coordenadas y unidades

| Aspecto | Decisión | Motivo |
|---|---|---|
| CRS | UTM del proyecto (`EPSG:32717`/`32718`, hemisferio sur) | Ecuador continental es 17S/18S; 32617 es 17**N** y desplazaría el proyecto 10 000 km |
| Almacenamiento | `f64` en coordenadas absolutas + `origin` local | a 760 000 m, `f32` resuelve ~6 cm: el modelo temblaría |
| Render | resta del origen en `f64`, luego `f32` | precisión sub-centimétrica en pantalla |
| Longitudes | metros | |
| Diámetros | mm (entero) en el modelo, m al dibujar | coincide con el catálogo comercial |
| Cotas | metros | |
| Vertical | exageración configurable (`Camera::z_scale`, por defecto 2× en la app) | hace legible un relieve de 8 m en 600 m |

---

## 7. Render 3D

Rune es un framework de UI 2D; no hay pipeline 3D. La vista se construye con **geometría
proyectada + algoritmo del pintor**:

1. **Cámara orbital** (`view::Camera`): objetivo, azimut, elevación, distancia, FOV, `z_scale`.
2. **Proyección** en perspectiva con recorte del plano cercano (`project_segment`).
3. **Escena** (`view::build`): triángulos de terreno sombreados por normal + tinte por cota,
   tubos como segmentos con ancho proporcional al diámetro, pozos como cilindros (12 caras) o
   como rombo de tamaño fijo cuando miden menos de 3 px.
4. **Orden**: se ordena por profundidad descendente y se emite en **un solo `Shape`**; Rune
   dibuja en orden de inserción, así que el resultado es correcto sin búfer de profundidad.

**Límites y por qué se aceptan:**

| Límite | Efecto | Mitigación |
|---|---|---|
| Sin búfer de profundidad | Intersecciones reales entre sólidos pueden ordenarse mal | A la escala de una red (tubos finos, pozos pequeños) no se percibe; se documenta |
| Coste por primitiva | 5 000 primitivas ≈ 250 ms/frame en **debug** en esta máquina | Compilar en `--release`; terreno aligerado (`Terrain::thinned`); capas apagables |
| Sin iluminación avanzada | Sombreado plano | Sombreado por normal + tinte por cota: suficiente para leer el relieve |
| Texto dentro de la escena | Etiquetas costosas | El detalle va al CAD; en pantalla, el inspector |

**Regla de rendimiento:** la escena se reconstruye solo cuando cambian cámara, capas o selección.
No hay animación de fondo; una ventana quieta no dibuja.

---

## 8. La CLI como interfaz de IA

- **Un sobre** (`redes.envelope/1`) para todo, con 14 claves siempre presentes.
- `ok:false` ⇔ `meta.exit_code ≠ 0` (invariante con prueba).
- Diagnósticos con `code` estable, `hint` y `hint_command` ejecutable.
- `--dry-run` en toda escritura; `--pretty`; `--quiet`.
- **Determinismo**: mismo archivo de entrada + misma orden ⇒ mismos códigos y mismo orden de
  listado (orden de inserción, nunca `HashMap`).
- **Sin GPU**: `redes vista --salida x.svg` produce una vista 3D que un agente puede leer.
- Auto-descripción: `redes esquema` publica familias, atributos, subtipos, comandos, códigos y
  formatos de entrada y salida (dxf, geojson, glb, svg, csv de cantidades, csv y html del acta).

---

## 9. Decisiones (ADR) y qué se revirtió del consejo anterior

| # | Decisión | Estado |
|---|---|---|
| A1 | **No calcular.** Todo número es un `Dato` con fuente. | nueva (revierte hidráulica y tierras) |
| A2 | Documento **JSON** versionado (`redes.project/2`) en vez de SQLite (hoy `/3`, ver D10) | nueva; SQLite cuando el tamaño lo pida |
| A3 | Modelo por **filas tipadas + esquema de familia** en vez de columnar | simplificación de v0.1; el esquema sigue siendo la fuente |
| A4 | Dispositivos **no parten** el arco | revierte la propuesta del consejo |
| A5 | Aprobación **por objeto** con estado global bloqueado | nueva |
| A6 | Vista **3D** con algoritmo del pintor sobre `Shape` | nueva (el consejo planeaba 2D) |
| A7 | Un solo `Shape` por frame; terreno aligerado | nueva |
| A8 | La skill de ingesta es un **contrato JSON** documentado, no código embebido | nueva |
| A9 | Producto `Redes`, binario `redes`, archivo `*.redes`, variables `REDES_*` | cierra la discrepancia del consejo |
| A10 | Se conservan del consejo: familias como dato, topología derivada, `ElementId`, sobre JSON, pureza del núcleo | vigentes |

---

## 10. Riesgos abiertos

| # | Riesgo | Impacto | Mitigación |
|---|---|---|---|
| R1 | Rune es pre-release y no acepta PRs | Un bug del framework puede bloquear una versión | Revisión fijada (`78efd4a`), todo el uso de Rune confinado a `redes-app`, la CLI no depende de Rune |
| R2 | El algoritmo del pintor falla con geometría que se cruza | Artefactos visuales puntuales | Aceptado y documentado; el detalle fino se ve en CAD |
| R3 | Fidelidad de la ingesta desde DXF | Pérdida de objetos o atributos | Informe de leídos/descartados con motivo; ida y vuelta verificada; la skill declara el mapeo |
| R4 | Proyectos grandes (100k objetos) | Escena y memoria | Aligerado de terreno, capas, y la puerta abierta a teselado por regiones |
| R5 | DWG nativo | El cliente pidió DWG | Se entrega DXF (formato abierto de intercambio); DWG exige biblioteca propietaria (ODA) — decisión de compra, no técnica |
| R6 | Datos sin procedencia | Un número sin fuente no es auditable | `Dato` exige `fuente`; la ingesta la escribe o marca `desconocida` |
