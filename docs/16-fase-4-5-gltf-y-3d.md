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

- **Una primitiva por familia** (material propio con `KHR_materials_unlit` y color por vértice):
  pozos, válvulas, accesorios, tuberías, terreno, calles y manzanas.
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
- El buffer de vértices se dimensiona a 200 000 vértices (≈ 7,2 MB); por encima se recorta.
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
