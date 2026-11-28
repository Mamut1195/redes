# 17 — Importar modelos de Blender (biblioteca de piezas)

> ⚠️ **Cifras corregidas.** Varios números de este documento se escribieron antes de que el
> código cambiara y ya no son ciertos. Las cifras vigentes, con cómo reproducirlas, están en
> [`21-cifras-medidas.md`](21-cifras-medidas.md). Este documento se conserva como historia.


> Qué hace falta para que una válvula, un pozo o una planta de tratamiento modelados en Blender
> sustituyan (o acompañen) a la pieza paramétrica. Qué ya está hecho y qué falta.

## 1. Decisión de formato: glTF 2.0 binario (`.glb`)

| Formato | Veredicto |
|---|---|
| **glTF 2.0 (.glb)** | **Sí.** Lo exporta Blender de serie, guarda jerarquía, materiales y colores por vértice, pesa poco y no tiene SDK propietario. Es el mismo formato que ya escribimos en `exportar gltf`. |
| OBJ + MTL | No. Sin jerarquía ni metadatos; los materiales se pierden con frecuencia. |
| FBX | No. SDK propietario de Autodesk y licencia incómoda para un producto propio. |
| STL | No. Solo triángulos: sin color, sin nombres, sin unidades. |
| USDZ | No por ahora. Excelente, pero pesado de integrar y sin ventaja para piezas. |

## 2. Lo que ya está hecho en el código (esta sesión)

- **Escritor** glTF: `crates/redes-core/src/gltf.rs` (`exportar_glb`), con una primitiva por familia,
  metros, origen restado, Y-arriba y `KHR_materials_unlit`. Verificado importándolo en Blender.
- **Lector y validador**: `crates/redes-core/src/biblioteca.rs` (`leer_glb`, `revisar`). Lee
  `POSITION`, `NORMAL`, `COLOR_0`, índices y la transformación de nodos (matriz o TRS con
  cuaternión), y comprueba escala, base en `z = 0`, centro en planta, presupuesto de triángulos y
  giro Y-arriba.
- **CLI**:
  ```powershell
  .\target\release\redes.exe biblioteca revisar --glb pieza.glb --familia agua.valvula
  ```
  Devuelve un sobre con `primitivas`, `triangulos`, `materiales`, `dimensiones_m`, `base_z`,
  `errores` y `avisos`. Exit ≠ 0 si hay errores (escala imposible, sin mallas).

Prueba de ida y vuelta en la suite: se exporta el catálogo, se vuelve a leer y coinciden
triángulos y caja envolvente (`biblioteca::tests::ida_y_vuelta_del_exportador`).

## 2.1 Datos maestros: el modelo no es solo una cáscara

El modelo **es apariencia**, pero también **es la fuente de las cantidades**: de su geometría salen
los volúmenes y las áreas que después se usan para medición y presupuesto. Para eso hacen falta tres
cosas, y las tres ya funcionan al importar:

### a) Medidas sacadas de la geometría (calculadas, no declaradas)

`redes biblioteca cantidades --glb pieza.glb --densidad 2.4` devuelve:

| Dato | Cómo se obtiene |
|---|---|
| **volumen (m³)** | teorema de la divergencia: suma de tetraedros desde el origen |
| **área (m²)** | suma de las áreas de los triángulos |
| **por material** | el mismo volumen separado por material (hormigón, acero…) |
| **cerrada** | cuenta de aristas que pertenecen a un solo triángulo |
| **peso (t)** | volumen × densidad pedida (2,4 hormigón armado; 7,85 acero) |

Si `cerrada` es `false`, el volumen **no es fiable** como cantidad de material: la malla tiene
agujeros. El comando lo dice con `aristas_abiertas` en vez de dar un número engañoso.

Ejemplo real medido en esta sesión (un `.glb` de 7 032 triángulos):

```
volumen_m3 8494.75 · area_m2 7515.83 · cerrada false · aristas_abiertas 2688
```

### b) Datos declarados en Blender (`extras` de glTF)

Blender escribe las **propiedades personalizadas** de cada objeto en el campo `extras` de glTF si
se exporta con `export_extras=True`. El lector ya las extrae y las guarda en el manifiesto, con
prefijo según de dónde vengan (`nodo.*`, `malla.*`, `asset.*`).

Convención recomendada para que sirvan a todo (visor, medición, DXF):

| Propiedad en Blender | Para qué |
|---|---|
| `redes_familia`, `redes_subtipo` | mapeo automático al catálogo |
| `redes_dn_mm` | diámetro nominal |
| `redes_diametro_interior_m`, `redes_altura_m` | cotas principales de la pieza |
| `redes_material` | material estructural (hormigón, acero, PVC) |
| `redes_volumen_m3` | volumen **declarado** por el proyectista (se compara con el medido) |
| `redes_codigo` | código de la pieza en el proyecto (`PZ-014`) |

### c) Contraste declarado vs medido

Al importar se guardan **las dos cosas**: lo declarado en `extras` y lo medido de la malla. La app
puede avisar si no cuadran, por ejemplo un `redes_volumen_m3` de 1,2 cuando la geometría mide 3,4.
Ese contraste es lo que evita que un modelo "bonito" infle un presupuesto.

### d) La ficha maestra (manifiesto)

El manifiesto guarda, por pieza: familia, subtipo, DN, archivo, escala, triángulos, dimensiones,
**volumen**, **área**, **cerrada**, **extras** y bytes. Es la ficha que sirve al visor y a la
medición a la vez, sin volver a abrir el `.glb`.

> Regla: la geometría manda en las **cantidades** (volumen, área, peso); los parámetros mandan en
> las **cotas de red** (tapa, clave, pendiente) y en el **estado de revisión**. Si los dos
> discrepan, se avisa; no se elige uno en silencio.

## 3. Cómo entra un modelo hoy (importación guiada)

Como los modelos los va a poner **el usuario**, no se le puede exigir una convención de nombres ni
que prepare el archivo: el importador lo normaliza y lo anota.

```powershell
# 1) revisar (opcional, pero recomendable)
.\target\release\redes.exe biblioteca revisar --glb valvula.glb --familia agua.valvula

# 2) importar: normaliza y anota en biblioteca.json
.\target\release\redes.exe biblioteca importar `
    --glb valvula.glb --familia agua.valvula --subtipo compuerta --dn 200 `
    --carpeta .\mi-proyecto

# 3) ver qué hay en la biblioteca
.\target\release\redes.exe biblioteca lista --carpeta .\mi-proyecto
```

Lo que hace `importar`, automáticamente:

| Ajuste | Detalle |
|---|---|
| **Y-arriba → Z-arriba** | aplica la rotación del nodo raíz y convierte al marco de la app |
| **Centra en planta** | resta el centro de la caja envolvente en X e Y |
| **Apoya la base en z = 0** | resta la cota mínima |
| **Escala** | `--escala` (1.0 por defecto); el validador avisa si el tamaño no cuadra con la familia |
| **Color** | usa `COLOR_0` si existe; si no, el color de la familia |
| **Manifiesto** | escribe `biblioteca.json` con id, familia, subtipo, DN, archivo, escala, triángulos, medidas, bytes |
| **Aviso de peso** | si pasa de 60 000 triángulos, avisa de que use `--gpu3d` |

Resultado medido con un `.glb` de ejemplo de 7 032 triángulos: base `z = 0`, centro en planta
`(0, 0)`, manifiesto escrito y relectura sin errores.

## 4. Lo que falta para que se vea en el visor

1. **Referencia del objeto al modelo**: un `modelo: Option<String>` en `Node` (o un valor por
   defecto por familia+subtipo) que apunte a una entrada del manifiesto. Sin esto, la app no sabe
   qué modelo dibujar en cada pozo.
2. **Colocación e instanciación**: cargar el `.glb` una vez, cachear la malla y dibujar N
   instancias en las posiciones de los nodos (cota y giro incluidos).
3. **Camino de dibujo**: el renderer 2D (pintor) **no aguanta** mallas de decenas de miles de
   triángulos; el pase 3D (`--gpu3d`) sí. O se exige `--gpu3d` con modelos, o se genera un
   **proxy decimado** (≈300 triángulos) para el 2D.
   Hoy (F14-06): el manifiesto se comparte en un `Arc` (no se clona en cada reconstrucción) y el
   proxy se calcula una vez por pieza y paso de rejilla (`modelos::Cache::proxy`), no por
   instancia ni por reconstrucción.
4. **Anclas de conexión**: puntos con nombre y diámetro, para que la tubería sepa dónde llega.
5. **Regla de convivencia** (decidida con el cliente): el modelo cambia la apariencia; los datos
   siguen saliendo de los parámetros (ver §6).

## 5. Contrato para quien modela en Blender

Checklist que debe cumplir un `.glb` para entrar sin retoques:

| Regla | Por qué |
|---|---|
| **Unidades en metros** y transformaciones aplicadas (`Ctrl+A`) | el visor no adivina la escala |
| **Origen en el centro de la base** (`z = 0` apoya) | la pieza se coloca por su base, como la paramétrica |
| **+Z arriba en Blender** (exportación por defecto a +Y) | coincide con nuestro modelo; el lector ya detecta el giro |
| **Una pieza por objeto** (o una colección por pieza) | una malla = una primitiva en el manifiesto |
| **Nombre con convención**: `familia__subtipo__DN200` | mapeo automático al catálogo |
| **Materiales con nombre** (`hormigon`, `hierro`, `pvc`, `laton`…) | se reutiliza la paleta de la app |
| **≤ 60 000 triángulos** por pieza (≤ 300 000 una planta) | presupuesto del visor |
| **Sin cámaras, luces ni planos de referencia** | solo peso |
| **Triangulado** y sin modificadores sin aplicar | el lector no evalúa modificadores |
| **Anclas como *empties*** con nombre y `DN` en propiedad personalizada | conectividad de la red |
| **Licencia anotada** en el manifiesto | trazabilidad legal |

Receta mínima en Blender (Scripting) para dejar un objeto listo:

```python
import bpy
obj = bpy.context.active_object
bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)
obj.name = "agua.valvula__compuerta__DN200"
bpy.ops.export_scene.gltf(
    filepath="valvula_compuerta_dn200.glb",
    export_format="GLB",
    use_selection=True,
    export_apply=True,
    export_yup=True,
    export_materials="EXPORT",
)
```

## 6. Manifiesto

```json
{
  "schema": "redes.biblioteca/1",
  "piezas": [
    {
      "id": "valvula_compuerta_dn200",
      "familia": "agua.valvula",
      "subtipo": "compuerta",
      "dn_mm": 200,
      "archivo": "piezas/valvula_compuerta_dn200.glb",
      "escala": 1.0,
      "lod": { "alto": "valvula_compuerta_dn200.glb", "bajo": "valvula_compuerta_dn200_lod1.glb" },
      "anclas": [
        { "nombre": "entrada", "pos": [0, -0.19, 0.1], "dn_mm": 200, "eje": [0, 1, 0] },
        { "nombre": "salida", "pos": [0, 0.19, 0.1], "dn_mm": 200, "eje": [0, 1, 0] }
      ],
      "licencia": "CC-BY-4.0",
      "autor": "…",
      "sha256": "…"
    }
  ]
}
```

## 7. Regla de convivencia (decidida)

Una pieza tiene **dos fuentes posibles**: parámetros (la paramétrica) y modelo (la de Blender).

- **El modelo cambia la apariencia, nunca los datos.** La cota de tapa, la clave, el DN y el estado
  siguen saliendo del modelo de datos; el `.glb` solo sustituye cómo se dibuja.
- Si hay modelo para esa combinación (`familia`, `subtipo`, `DN`), se usa; si no, se usa la
  paramétrica. La app avisa en el inspector qué fuente está usando.
- Un modelo **nunca** modifica el volumen de excavación ni la exportación DXF: eso sale de los
  parámetros.

## 8. Plan y esfuerzo

| Paso | Qué | Esfuerzo |
|---|---|---|
| Hecho | lector `.glb` + validador + `biblioteca revisar` | 1 día |
| Hecho | normalización (marco, centro, base, escala) + manifiesto + `biblioteca importar/lista` | 1 día |
| 1 | Campo `modelo` en el nodo y en el proyecto | 1–2 días |
| 2 | Colocación e instanciación en el pase 3D + caché por `(modelo, lod)` | 2–3 días |
| 3 | Proxy decimado para el renderer 2D (o exigir `--gpu3d`) | 2 días |
| 4 | Anclas y conexión automática de tuberías | 2 días |
| 5 | Inspector: origen del dibujo (modelo/paramétrica) y encuadre al modelo | 1 día |

Total realista: **8–12 días** para la integración completa, con el lector ya hecho.

## 9. Riesgos

1. **Dos fuentes de la verdad** si un modelo trae sus propias medidas: mitigado por §6 (el modelo
   no aporta datos, solo forma).
2. **Peso**: una planta de tratamiento con 300 000 triángulos por instancia repetida se dispara;
   hay que instanciar (no copiar) y usar LOD.
3. **Escala silenciosa**: un `.glb` en milímetros entra 1000× más grande; el validador lo detecta
   y debe ser bloqueante, no un aviso.
4. **Licencias**: sin campo de licencia en el manifiesto no se puede distribuir el producto.
5. **El pintor no perdona**: si se permite un modelo pesado sin `--gpu3d`, el visor se arrastra;
   hay que decidirlo explícitamente y avisar.

## 10. F17-01: MCP de Blender frente a bpy sin ventana

Evaluación del 28/11/2026 (decisión **D15** en docs/22). Pregunta: cómo deben modelar los agentes
las familias paramétricas de D14 (pozo, tubería, accesorios, captación, planta) con Blender 5.1.
Criterios de docs/18: fiabilidad con Geometry Nodes, uso sin intervención por subagentes y salida
reproducible en el CI.

### 10.1 Las dos vías

| | MCP de Blender | `bpy` sin ventana |
|---|---|---|
| Cómo funciona | Un complemento dentro de Blender abre un socket TCP (por defecto `localhost:9876`) y un servidor MCP aparte, lanzado por el cliente por stdio, le reenvía órdenes | `blender -b --factory-startup --python-exit-code 1 --python script.py -- params.json`: un proceso por pieza, sin ventana, que termina al exportar |
| Qué hace falta | Blender abierto con el complemento instalado y activo, el servidor MCP (Python 3.10+ y `uv` en el comunitario) y registrarlo en el cliente del agente | Solo `blender.exe` y el script del repo |
| Sin ventana | Comunitario (`ahujasid/blender-mcp`, ahora `mcp-for-blender`): no; su guía supone la interfaz. Oficial (Blender Lab, `projects.blender.org/lab/blender_mcp`): tiene herramientas `*_for_cli` que lanzan un Blender en segundo plano, pero entonces es `bpy` sin ventana con un intermediario | Sí, es su modo normal |
| Seguridad | Los dos exponen `execute_blender_code` (Python arbitrario) por un socket **sin autenticación**; el comunitario además envía telemetría anónima por defecto (`DISABLE_TELEMETRY`) y trae un «modo seguro» opcional | Ejecuta solo el script versionado; sin puertos abiertos |
| Estado de la sesión | El agente modifica una escena viva: lo que hizo antes (él o el usuario) queda dentro y cambia el resultado | `--factory-startup` y `read_factory_settings(use_empty=True)`: cada horneado empieza de cero |
| Reproducible | No por sí mismo: el código que manda el agente no queda en el repo | Sí: la receta es el script más el JSON de parámetros, los dos versionables |
| Madurez | Oficial: v1.0.x (abril–septiembre de 2026), experimental de Blender Lab, pensado para consultar la API, depurar y hacer capturas. Comunitario: muy usado, sin versión estable declarada; avisa de tiempos de espera y de fallos en la primera conexión | Es la línea de órdenes de Blender de siempre |

No se instaló ningún MCP: no aporta nada que el horneado sin ventana no haga y exigiría tocar la
configuración del cliente. La comparación sale de la documentación de los dos proyectos.

### 10.2 Prototipo medido

`tools/blender/build_manhole.py` hornea un pozo de inspección con todo como parámetro (diámetro
interior, profundidad, espesor de muro y losa, altura del cono, diámetro de la boca, espesor de la
tapa, material del cuerpo y de la tapa, segmentos). El cuerpo y el cono son un sólido de revolución
con vértices y caras explícitos; la tapa es un modificador de **Geometry Nodes** (Mesh Cylinder,
Transform, Triangulate, Set Material) cuyas entradas se fijan desde el JSON, y el exportador glTF lo
aplica (`export_apply`). Cada objeto lleva sus `redes_*` en `extras`.

```powershell
blender -b --factory-startup --python-exit-code 1 `
    --python tools\blender\build_manhole.py -- out\f17-01\params.json
# params.json: {"inner_diameter_m":1.5,"depth_m":4.0,"wall_thickness_m":0.2, ..., "output":"...\\pozo.glb"}
```

Medido en Windows 11 con Blender 5.1.0, tres horneados por juego de parámetros:

| Juego | DI / prof. / espesor (m) | Segmentos | Triángulos | Bytes | Volumen medido (m³) | Cerrada | 3 horneados idénticos | Tiempo por horneado |
|---|---|---:|---:|---:|---:|---|---|---:|
| a | 1,2 / 2,5 / 0,15 | 48 | 764 | 37 164 | 1,7022 | sí | **sí** | 2,04–2,16 s |
| b | 1,5 / 4,0 / 0,20 | 64 | 1 020 | 49 272 | 4,4937 | sí | **sí** | 2,02–2,03 s |
| c | 0,9 / 1,6 / 0,10 | 32 | 508 | 25 464 | 0,5689 | sí | **sí** | 2,02–2,06 s |

- **Tiempo**: ~2,0 s por pieza, de los que ~1,55 s son el arranque de Blender
  (`--python-expr "pass"`) y el resto construir y exportar (el exportador tarda 0,05 s). Hornear
  muchas piezas en un mismo proceso amortiza el arranque; eso es de F17-02.
- **Determinismo**: los `.glb` de los mismos parámetros salen idénticos byte a byte. La primera
  versión, con `bmesh.ops.spin` + `remove_doubles` + `triangulate`, **no** lo era: mismos
  vértices y mismos triángulos, pero en otro orden en cada ejecución (cambiaba el búfer de índices
  del cuerpo). Se sustituyó por listas explícitas de vértices y caras (`from_pydata`). La parte de
  Geometry Nodes salió idéntica desde el principio. Regla para F17: en la receta, nada de
  operadores `bmesh` que fusionan o reordenan; vértices y caras explícitos o Geometry Nodes.
- **Errores**: sin `--python-exit-code 1`, una excepción de Python deja a Blender salir con código
  **0** y sin `.glb` (pasó con un tipo de árbol de nodos mal escrito). Con la opción sale con 1.
  Los parámetros inválidos (`sys.exit` con mensaje) salen con 1 en los dos casos.
- **Comprobación del volumen** (juego b): a mano, losa 0,567 + fuste 3,204 + cono 0,654 =
  4,425 m³ con círculos perfectos; con 64 segmentos el polígono da ≈ 4,418, y la CLI mide 4,4178
  de hormigón más 0,0759 de tapa (π·0,55²·0,08 = 0,0760).

### 10.3 Entra en la biblioteca sin cambios

- `redes biblioteca revisar --glb pozo_b_1.glb --familia saneamiento.pozo`: 2 primitivas,
  1 020 triángulos, materiales `hormigon_armado` y `fundicion`, base en `z = 0`, dimensiones
  1,9 × 1,9 × 4,08 m, 0 errores y 0 avisos.
- `redes biblioteca cantidades`: `cerrada: true`, 0 aristas abiertas, volumen por material y los
  `extras` `nodo.redes_*`.
- `redes biblioteca importar` + `lista`: se importa y se anota en el manifiesto sin errores.

Huecos vistos (no se arreglan aquí; pasan a F17-02/F17-03):

- `cantidades` guarda los `extras` de **un** nodo (el cuerpo); los de la tapa se pierden.
- `importar` deja `extras` vacío en el manifiesto (el `.glb` normalizado no los conserva).
- `dn_mm` queda nulo: el pozo no tiene DN; su cota principal es el diámetro interior.

### 10.4 Recomendación

**`bpy` sin ventana con scripts versionados en `tools/blender/`** como única vía para hornear
piezas, en local y en el CI. Es la única que cumple los tres criterios: Geometry Nodes funciona
igual sin ventana, un subagente lo lanza con una orden y lee el código de salida, y la misma
receta con los mismos parámetros da los mismos bytes.

El MCP oficial de Blender Lab puede servir, **opcional y fuera del flujo**, al usuario o a un
agente para consultar la API de Python o mirar una escena mientras escribe una receta; nunca para
producir piezas, porque lo que hace no queda en el repo ni se puede repetir. El comunitario se
descarta (sin modo sin ventana, telemetría por defecto, socket sin autenticación).

Siguen abiertas en F17-02: si la app hornea bajo demanda (CLI → Blender → caché por parámetros) o
reconstruye la receta en el núcleo; cuántas piezas por proceso; y si el CI instala Blender para
hornear o solo comprueba los `.glb` ya horneados (hoy el CI no tiene Blender).
