# 16 — Fase 4 y fase 5: glTF y pase 3D con profundidad

> ⚠️ **Cifras corregidas.** Varios números de este documento se escribieron antes de que el
> código cambiara y ya no son ciertos. Las cifras vigentes, con cómo reproducirlas, están en
> [`21-cifras-medidas.md`](21-cifras-medidas.md). Este documento se conserva como historia.


## Fase 4 — `exportar gltf`

```powershell
.\target\release\redes.exe -p out\ciudad.redes exportar gltf --salida out\ciudad.glb
.\target\release\redes.exe -p out\ciudad.redes exportar gltf --salida out\sin-terreno.glb --sin-terreno
```

Qué contiene el `.glb`:

- La malla `redes` del nodo raíz lleva **una primitiva por familia** de la geometría fija
  (tuberías, terreno, calles y manzanas). Cada familia tiene su material con
  `KHR_materials_unlit` y color por vértice.
- **Un nodo hijo por pieza** (pozos, válvulas, accesorios…), con el código del objeto como nombre
  y su posición como `translation`. Las piezas iguales (misma `ClavePieza`) **comparten una
  malla** (F14-05). Antes cada pozo copiaba su malla entera, y 20k tramos daban un `.glb` de
  1,1 GB en 26 s con 4 GB de RAM (`docs/21` §4.4).
- El binario se escribe de una vez, sin búferes intermedios: primero se calcula la disposición y
  luego se vuelca cada vértice. Si el archivo pasara de 4 GB (los largos del GLB son `u32`),
  `exportar_glb` devuelve `GlbTooLarge` y la CLI responde `E_IO_TOO_LARGE`, en vez de escribir un
  largo truncado.
- **Metros**, origen restado (coordenadas locales) para no perder precisión en `float32`.
- Nodo raíz con rotación de **Z‑arriba a Y‑arriba** (convención glTF).
- Sin extensiones obligatorias: se abre en Blender, three.js, Windows 3D Viewer, Babylon.

**Verificación hecha:**

| Comprobación | Resultado |
|---|---|
| Contenedor GLB (magic, versión, largo, chunk JSON/BIN) | válido, largo declarado = real |
| Accessors y bufferViews | `count × 12` bytes por accessor, offsets dentro del BIN |
| Importación real en **Blender 5.1 headless** | `REDES_MESH 2 REDES_TRIS 26816` para la ciudad |
| Catálogo | 30 primitivas, 6 696 triángulos |
| Ciudad con contexto urbano | 20 primitivas, 26 810 triángulos (incluye 10 calles y 108 manzanas) |
| `--sin-terreno` | reduce la malla al modelo |

Pruebas en `crates/redes-core/src/gltf.rs`: cabecera y JSON, coordenadas en metros dentro del
modelo, tamaño razonable, calles y cubiertas presentes, y **persistencia**: guardar y volver a
abrir un proyecto conserva el entorno y su TIN.

## Fase 5 — pase 3D con z‑búfer

`--gpu3d` (o `REDES_GPU3D=1`) activa un compositor propio dentro de Rune:

1. Se sube la malla (posiciones, normales y color) a un `vertex buffer` y la matriz
   vista‑proyección a un `uniform`.
2. Se abre **un pase propio de wgpu con textura de profundidad** sobre el mismo `target` de la
   ventana, con `LoadOp::Clear`.
3. Se dibuja la interfaz de Rune encima, reabriendo el pase con `LoadOp::Load`
   (`Renderer::draw_range_with`).
4. El canvas y el contenedor raíz se pintan **transparentes** para que el 3D se vea a través.

Sin el flag, todo sigue por el renderer 2D (algoritmo del pintor): la app funciona igual.

**Cámara y malla separadas (F14-02, F14-03).** La malla se guarda en coordenadas locales (origen
restado en `f64` antes de bajar a `f32`) **con la cota real**; la exageración vertical va en la
matriz (`Geometry::set_camera` → `Camera::matriz(aspecto, z_scale)`, alrededor de la cota del
objetivo). Por eso orbitar, desplazar, acercar o cambiar `z_scale` solo recalcula la matriz, el plan de dibujo y las
etiquetas (`view::etiquetas`, mismas reglas que la escena del pintor); en `--gpu3d` no se ejecuta
`build_opts` ni se arma el `Shape`. La escena (`scene3d::Scene`) se regenera solo cuando
cambia su clave: proyecto (`Arc` y `revision`), capas terreno/agua/alcantarillado/edificios o la
llegada de la biblioteca. La escena va en un `Arc`: el compositor toma la matriz y el plan cada frame y
la escena solo si la generación es nueva (`Geometry::take`), sin copiar la malla con el mutex
tomado. En la ciudad (release): 6,0 ms → 0,008 ms por cambio de cámara y 3,6 MB copiados por
frame → 0. La barra de estado cuenta, en `--gpu3d`, los triángulos que dibuja la GPU.

**Caché de mallas de pieza (F14-06).** La malla paramétrica de un nodo sale de
`redes_core::mallas::CachePiezas`, con clave `ClavePieza` = familia, subtipo, atributos (DN,
diámetros, ángulos…), bits de `z_rim − z_invert`, falta de altura y nivel de detalle; la posición no
entra (la malla es local). La caché vive en el estado de la app entre reconstrucciones: apagar una
capa o editar un objeto regenera la malla de la escena, pero solo genera las piezas cuya clave
cambió (`CachePiezas::generadas`, prueba `la_segunda_reconstruccion_no_genera_piezas`). Cada
pasada poda lo que no usó, así que no crece con las ediciones. `triangulos_exportables` (glTF)
usa una caché efímera: dos nodos iguales comparten la malla también al exportar.

**Por qué importa** (medido por la revisión de pruebas, release, p95 por frame):

| Escena | 2D (pintor) | 3D (`--gpu3d`) |
|---|---|---|
| `ciudad` | 33,9 ms | **2,3 ms** |
| `corte` | 247,0 ms | **2,5 ms** |

El corte es el caso que justifica el pase: recortar cientos de triángulos y ordenarlos por el
pintor cuesta ~250 ms; con z‑búfer la GPU lo hace en 2,5 ms y **el orden es correcto por
construcción** (adiós a los artefactos del pintor).

**Límites conocidos:**

- El resaltado de selección y los colores de revisión (aprobado/observado) siguen siendo del
  camino 2D; en `--gpu3d` el modelo se dibuja con su color de material.
- El modo corte en 3D **no** recorta (el z‑búfer no necesita recorte para ocluir): el corte
  sigue siendo del camino 2D. Son dos herramientas distintas, no dos versiones del mismo pase.
- **Instancias, LOD y recorte (F14-05).** La escena del pase es `redes_core::scene3d::Scene`:
  - **Prototipos compartidos.** La malla de cada pieza (de `CachePiezas`) se parte por sus primitivas
    (`Mesh::part_breaks`, que anota `Mesh::extend`). Cada parte se lleva a forma canónica (esquina
    mínima en el origen y alto 1) y se busca entre los prototipos ya registrados, con firma
    cuantizada a 0,1 mm en planta y 10⁻⁴ del alto en Z. Un fuste de 2 m y otro de 3 m son el mismo
    prototipo con otra escala en Z, y los peldaños de una escalera, uno solo. Cada pieza es una
    lista de instancias: desplazamiento, escala y giro, 32 bytes. El desplazamiento se suma en `f64`
    (base del nodo − origen + esquina de la parte) y solo después baja a `f32` (docs/15).
  - **Tres LOD por objeto:** completo, simple y caja envolvente con el color medio. Se elige por
    radio en pantalla: ≥ 14 pt completo, ≥ 3 pt simple, el resto caja. El umbral de 14 pt es el
    mismo que usa el pintor.
  - **Recorte por tronco de visión:** cada objeto lleva su caja. La geometría fija (tuberías,
    calles, manzanas y terreno) va en lotes indexados en una rejilla de 16 × 16, cada uno con su
    caja.
  - **Plan por cámara.** `scene3d::plan` recorta, elige el LOD y agrupa las instancias por
    prototipo. Lo calcula `Geometry::set_camera`: mover la cámara solo rehace el plan y el búfer de
    instancias; vértices e índices se suben una vez por generación.
  - **Shader.** Aplica la instancia (`escala → giro en Z → desplazamiento`) y lleva la normal por la
    escala inversa. Cada llamada enlaza el búfer de instancias en su primera instancia, sin
    `first_instance` (no todos los backends lo admiten).
- **Tope con aviso.** La escena entera (vértices + índices + búfer de instancias del peor caso) no
  pasa de `GPU_BUDGET` = 512 MB ni de `max_buffer_size` del dispositivo (`gpu3d::device_budget`).
  El compositor baja el tope al del dispositivo y regenera la escena. Si no cabe, se suben los
  primeros prototipos y lotes que caben y la barra de estado avisa «La escena 3D pide … MB de GPU y
  el tope es … MB: no se dibujan …». El aviso sale de la escena, así que también aparece en una
  captura `--snap`. Antes el tope era 512 MB fijo y una GPU de 256 MB entraba en pánico con ~5k
  pozos. Con instancias, 20k tramos piden 22,9 MB en vez de 1 110 MB (ver docs/21 §4.4).
- **Pruebas** (`scene3d`): `manholes_of_different_depth_share_prototypes`,
  `instancing_reproduces_the_exported_mesh` (las instancias en LOD completo dan los mismos
  triángulos que el glTF, a 1 mm), `distant_objects_draw_as_boxes_and_near_ones_in_full`,
  `objects_outside_the_view_are_culled` y `a_scene_over_budget_is_trimmed_and_warns`. En la app:
  `the_budget_respects_the_device_limit`. El banco (`tests/banco.rs`) tiene una fase «malla 3D»
  con tope de bytes por objeto.
- La app pide `wgpu ~30.0.1`, la misma versión que usa Rune fijado en `vendor/rune`.

Pruebas: las de la fase 1‑3 más `el_proyecto_guardado_conserva_el_entorno`; el pase 3D se verificó
con capturas headless (`--gpu3d --snap`) de `ciudad`, `vitrina`, `pozos` y `pieza`.

## Estado final de las cinco fases

| Fase | Estado |
|---|---|
| 1 — catálogo 29 familias / 216 subtipos con dibujo | cerrada |
| 2 — foco, corte, explosión | cerrada |
| 3 — etiquetas, leyenda, ficha paramétrica, deshacer | cerrada |
| 4 — glTF verificado | cerrada |
| 5 — pase 3D con profundidad (opt‑in) | cerrada |
| 6 — acabado visual (tapa legible, poché, carriles) | pendiente |
| — — DXF ida y vuelta, contrato del CLI | pendiente |
