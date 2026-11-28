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
| **Anclas como *empties*** con nombre y `DN` en propiedad personalizada (`entrada__<tramo>`, `salida__<tramo>`: una por tubo) | conectividad de la red |
| *Empty* **`solera`** en la solera de la pieza (si la tiene) | el visor apoya ese punto en la cota de solera del objeto; sin él, la base va a la cota de tapa (§12.4) |
| **Colores en el material** (`baseColorFactor`) si no hay `COLOR_0` | el visor los toma tal cual |
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

Huecos vistos (no se arreglan aquí; pasan a F17-02/F17-03; cerrados en F17-03, §12.5):

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

Las preguntas que quedaban (hornear bajo demanda o reconstruir en el núcleo, cuántas piezas por
proceso, Blender en el CI) las resuelve F17-02 (§11, D16).

## 11. F17-02: cómo llegan los parámetros a la geometría

Evaluación del 28/11/2026 (decisión **D16** en docs/22). Pregunta: cómo pasan los datos de un
objeto del proyecto (diámetro, cotas, espesor, material…) a la pieza de Blender que lo dibuja, sin
que la app dependa de Blender para abrir y sin inventar ningún número.

### 11.1 Las tres vías

| | A. Hornear bajo demanda con caché | B. Reconstruir la receta en el núcleo | C. Plantilla con regiones estirables |
|---|---|---|---|
| Cómo | La CLI resuelve los parámetros de cada objeto, calcula una clave y solo llama a Blender (un proceso para todas) con las piezas que no están en la caché | La receta se escribe en un formato propio que leen Blender **y** Rust; el núcleo genera la malla | Blender hornea una plantilla por familia con regiones con nombre (el fuste del pozo, el cuerpo del tubo) y el núcleo las estira según el parámetro |
| Blender en ejecución | No: la app solo lee `.glb`; Blender solo hace falta para hornear lo que falta | No | No |
| Fidelidad | Total: es lo que dibuja Blender | Solo lo que el formato propio sepa expresar | Exacta solo en tramos prismáticos (estirar en Z un fuste recto); falla con diámetros (estirar un anillo engorda la pared), conos, número de segmentos, ángulos de accesorios |
| Mantenimiento | Una receta por familia | **Dos implementaciones** por familia (Blender y Rust) que pueden divergir, más un intérprete de un subconjunto de Geometry Nodes (cientos de tipos de nodo; booleanas y biselados exigen una biblioteca de geometría) | Una receta más, por familia, la lista de regiones y qué parámetro estira cada una (lógica repartida) |
| Encaje con D14 | Directo | Contradice D14: es volver a `parts.rs` con otro nombre | Compatible, como optimización |
| Coste | Prototipo hecho (este ticket) | Semanas, y otra vez por cada familia nueva | ~2 días por familia prismática |

**B se descarta**: duplicar cada receta en Rust es exactamente lo que D14 quiere dejar atrás, y
para comprobar que las dos coinciden haría falta Blender en el CI de todos modos. **C** queda como
optimización posible para parámetros longitudinales (profundidad del pozo, largo del tubo) si el
número de piezas distintas pesa en redes reales (§11.5); hoy no hace falta.

### 11.2 Prototipo: `redes biblioteca hornear`

- **Contrato de la receta** `tools/blender/build_manhole.receta.json` (`redes.receta/1`), junto a su
  guion `build_manhole.py` (mismo nombre) y al conductor común `tools/blender/redes_bake.py`. Dice de
  dónde sale cada parámetro:
  - `"desde": "atributo"`: un atributo del objeto (`diametro_interior`, `espesor_pared`, `material`);
  - `"desde": "tapa_menos_solera"`: cota de tapa menos cota de solera, las dos del origen;
  - `"desde": "forma"`: un detalle de dibujo fijado por la receta (altura del cono, diámetro de la
    boca, espesor y material de la tapa, segmentos). Es parte de la receta versionada, entra en la
    clave y en el manifiesto (`horneado.*`), y **nunca** se convierte en `Dato` ni alimenta cantidades.
  Una prueba comprueba que las unidades de la receta coinciden con las del registro de familias.
- **Núcleo** (`redes_core::bake`): resuelve los parámetros de un objeto, calcula la clave y arma la
  entrada del manifiesto. No ejecuta Blender.
- **Clave** = BLAKE3 (32 hex) de `redes_bake.py` + guion de la receta + JSON canónico de los
  parámetros (claves ordenadas). Los finales de línea se normalizan (CRLF y LF dan la misma clave).
  La versión de Blender **no** entra: la caché tiene que poder leerse sin Blender; las recetas se
  fijan a Blender 5.1. La profundidad derivada se redondea al micrómetro, porque `2491,63 − 2490,0`
  en `f64` es `1,6300000000001091` y daba otra clave y otros bytes que un `1,63` escrito a mano.
- **Caché** en la carpeta de la biblioteca del proyecto: `biblioteca/horneado/<clave>.glb`, junto a
  `biblioteca.json`. Viaja con el proyecto: otro equipo sin Blender ve las mismas piezas. Cada pieza
  se escribe aparte (`<clave>.tmp.glb`) y se renombra al terminar Blender: la caché nunca tiene
  archivos a medias; si Blender falla, se borran todas las del lote.
- **Lote**: todas las piezas que faltan van a **un** proceso de Blender (`{"jobs": [...]}`); el
  conductor valida todos los juegos antes de hornear el primero y vacía la escena entre pieza y pieza
  (`read_factory_settings(use_empty=True)`).
- **`.blend` de revisión**: junto a cada `<clave>.glb` se guarda `<clave>.blend` (sin comprimir,
  `save_as_mainfile` **después** de exportar el glTF, para que no cambie sus bytes), salvo con
  `--sin-blend`. Al abrirlo en Blender se ve cómo se construyó la pieza: la escena lleva
  `redes_receta`, `redes_parametros` (el JSON) y un `redes_<parámetro>` por cada uno; cada objeto,
  sus `redes_*`; la tapa conserva su modificador de Geometry Nodes con las entradas a la vista
  (radio, espesor, elevación, segmentos) para tocarlas, y el texto `redes_receta.txt` explica cómo
  rehornear el cuerpo, que es geometría explícita. El `.blend` **no entra en el determinismo** (Blender
  guarda marcas de tiempo): no forma parte de la clave ni de las comparaciones byte a byte; si falta,
  la pieza se rehornea (el `.glb` sale idéntico). Pesa ~585 kB por pieza frente a ~37 kB del `.glb`.
  Muestra: los tres juegos de F17-01 (con la forma de la receta) en `out/f17-02/revision/`
  (`pozo_a_DI1200_H2500`, `pozo_b_DI1500_H4000`, `pozo_c_DI900_H1600`, `.glb` y `.blend`), hechos con
  `redes biblioteca hornear --receta tools/blender/build_manhole.receta.json --parametros out/f17-02/revision/juegos.json --carpeta out/f17-02/revision`.
- **Con proyecto** (`redes -p x.redes biblioteca hornear --receta tools/blender/build_manhole.receta.json`):
  hornea los objetos de la familia y subtipo de la receta, anota una pieza por clave en el manifiesto
  (con todos los parámetros en `extras` `horneado.*`), apunta `modelo` de cada objeto a su pieza,
  quita del manifiesto las piezas de esa receta que ya no usa nadie y guarda el proyecto. Sin
  proyecto, `--parametros juegos.json` hornea juegos dados a mano (solo los parámetros de dato).

### 11.3 Cuando falta un parámetro (D16)

- Un parámetro de dato que el objeto no trae (atributo ausente o vacío, o la cota en `sin_dato`)
  **no se rellena**: el objeto no se hornea, el sobre lo lista en `sin_hornear` con `faltan` y avisa
  `W_PARAMETRO_FALTANTE`; si el objeto tenía una pieza horneada de esa receta, se le quita (sería una
  forma que los datos ya no respaldan). El visor sigue con la pieza de respaldo de `parts.rs` y el
  inspector muestra `—` en el dato.
- Con `--parametros`, un juego incompleto o con un parámetro de forma falla entero:
  `E_PARAMETRO_FALTANTE`.
- El conductor de Blender no tiene valores por defecto (el prototipo de F17-01 sí los tenía y se
  quitaron): un juego incompleto sale con `missing params: [...]` y código 1.
- En el proyecto de demostración (`redes proyecto demo`) **ningún** pozo se hornea: ninguno declara
  `espesor_pared`. Es el comportamiento esperado, y la pregunta para el usuario (§11.6).

### 11.4 Sin Blender instalado

- **La app abre siempre**: nunca ejecuta Blender; dibuja las piezas de la caché y, para el resto, la
  geometría de `parts.rs`. Si falta el archivo de la caché, `modelos::Cache` lo da por fallido y se
  ve la paramétrica.
- **La CLI**: si todo está en la caché, `hornear` funciona sin Blender (`blender: null`). Si falta
  alguna pieza, falla con `E_BLENDER_NO_ENCONTRADO` y **no toca** ni el proyecto ni el manifiesto.
  Blender se busca en `--blender`, `$REDES_BLENDER`, la instalación estándar más reciente
  (`%ProgramFiles%\Blender Foundation\Blender *\blender.exe`) y el `PATH`.

### 11.5 Medido

Windows 11, Blender 5.1.0, CLI en release, 28/11/2026. Pozos distintos (diámetro y profundidad
distintos en cada uno), 764 triángulos y ~37 kB por pieza.

| Caso | Tiempo |
|---|---:|
| Arranque de Blender (`--python-expr "pass"`) | 1,50 s |
| 1 pieza, un proceso | 2,04–2,24 s |
| 10 piezas en 10 procesos (como F17-01) | 20,8 s (2,08 s/pieza) |
| 10 piezas en **un** proceso | **2,51 s** |
| 100 piezas en **un** proceso | **7,18–7,22 s** (0,072 s/pieza; ~0,053 s por pieza añadida) |
| `hornear` con proyecto, 1 / 10 / 100 pozos sin caché (`--sin-blend`) | 2,28 / 2,55 / 7,35 s |
| ídem, 10 / 100 pozos **con** `.blend` de revisión (por defecto) | 2,60 / 9,03 s |
| `hornear` con proyecto, 1 / 10 / 100 pozos **todos en caché** (sin Blender) | **26 / 31 / 86 ms** |

- Hornear 100 piezas en lote es ~29 veces más rápido que en 100 procesos (7,2 s frente a ~208 s).
- **Determinismo**: el `.glb` de un juego horneado en un lote de 1, 10 o 100 es idéntico byte a byte
  al de un horneado suelto, y el que escribe la CLI es idéntico al horneado a mano con el mismo JSON.
- Proyección a una red grande (sin medir): 10 000 pozos todos distintos ≈ 9 min la primera vez y
  ~370 MB de caché; después, milisegundos. Si eso pesa, la vía C (estirar el fuste en Z, como ya hace
  `scene3d` con los prototipos de F14-05) reduce las piezas a una por diámetro, espesor y material.

### 11.6 Recomendación y lo que queda

**Vía A**: hornear bajo demanda desde la CLI, en lote, con caché por clave en la carpeta del
proyecto. La app nunca depende de Blender; un dato que falta no se inventa: deja la pieza de respaldo.

**CI**: no instala Blender. Corre las pruebas de contrato con una caché sembrada (sin Blender) y la
de Blender se salta con aviso; comprueba las recetas contra el registro y las claves. Cuando F17-03
versione el `.glb` de la pieza piloto, el CI comprobará que su nombre es la clave que da la receta
actual y que la malla es válida (cerrada, triángulos), sin rehornear. Un trabajo manual opcional con
Blender podría rehornear y comparar bytes más adelante.

Para el usuario:

1. Qué parámetros del pozo son **dato** (vienen del origen; si faltan, no se hornea) y cuáles son
   **forma** de la receta. Hoy: dato = diámetro interior, profundidad (tapa − solera), espesor de
   pared y material; forma = cono, boca, tapa y segmentos. Con esa partición el demo no hornea ningún
   pozo (no declara espesor).
2. Si una red grande justifica la vía C para la profundidad (§11.5).

Para F17-03 (no hacen falta para este prototipo; los parámetros del horneado ya quedan en `horneado.*`).
Los cuatro primeros quedaron resueltos en F17-03 (§12); la limpieza de la caché sigue abierta:

- `biblioteca cantidades` guarda los `extras` de un solo nodo: el `nodo.redes_material` de la tapa
  pisa al del cuerpo (claves sin nombre de nodo).
- `biblioteca importar` deja `extras` vacío en el manifiesto (el `.glb` normalizado no los conserva).
- El visor coloca el modelo con su base en la **cota de tapa** (`modelos::instancias`) y además dibuja
  la paramétrica: un pozo horneado (base = fondo de la losa) tiene que apoyarse en la solera menos el
  espesor de la losa y sustituir a la paramétrica.
- Las piezas que ya no usa nadie salen del manifiesto, pero sus `.glb` y `.blend` siguen en la caché
  (falta una limpieza). Con `.blend` la caché pesa ~16 veces más: decidir si el `.blend` va siempre o
  solo bajo petición en redes grandes.

## 12. F17-03: pozo de inspección piloto, colocado en la red

Hecho el 28/11/2026 con Blender 5.1.0. La receta `tools/blender/build_manhole.py` dibuja el pozo
completo y la pieza sustituye a la de `parts.rs` en el visor, con y sin `--gpu3d`.

### 12.1 Qué es dato y qué es forma (D17)

| Parámetro de la receta | Desde | Valor |
|---|---|---|
| `inner_diameter_m` | atributo `diametro_interior` | dato |
| `depth_m` | `tapa_menos_solera` | dato |
| `wall_thickness_m` | atributo `espesor_pared` (o un `Dato` `espesor_pared` en m, con su fuente) | dato |
| `body_material` | atributo `material` | dato |
| `connections` | `conexiones`: los tramos que llegan al pozo | dato (ver §12.3) |
| `base_thickness_m`, `base_overhang_m` | forma | losa de 0,20 m que sobresale 0,10 m |
| `cone_type`, `cone_height_m`, `min_barrel_height_m` | forma | cono excéntrico de 0,60 m; fuste mínimo 0,30 m |
| `opening_diameter_m` | forma | boca de 0,60 m |
| `frame_height_m`, `frame_width_m`, `frame_seat_m` | forma | marco de 0,10 × 0,08 m con asiento de 0,03 m |
| `cover_thickness_m`, `cover_clearance_m`, `cover_material` | forma | tapa de fundición de 0,05 m, holgura 5 mm |
| `step_first_height_m`, `step_spacing_m`, `step_top_clearance_m` | forma | primer escalón a 0,40 m sobre la solera, cada 0,30 m, hasta 0,25 m bajo el marco |
| `step_width_m`, `step_projection_m`, `step_embed_m`, `step_bar_m`, `step_material` | forma | escalón en U de 0,30 m, a 0,15 m de la pared, empotrado 0,05 m, barra de 25 mm, polipropileno |
| `segments` | forma | 48 |

Todos los espesores son entradas. La forma va en el contrato versionado y entra en la clave; nunca es
`Dato` ni alimenta cantidades del proyecto.

### 12.2 Partes y marco de la pieza

- Marco: Z arriba, metros, origen en el centro de la **cara inferior de la losa**. La solera (fondo del
  canal, cara superior de la losa) queda en `z = base_thickness_m` y la tapa en `z = base_thickness_m + depth_m`.
- **Cuerpo** (`cuerpo`): losa, fuste y cono en un solo sólido de anillos apilados con vértices y caras
  explícitos (`loft`). Excéntrico: la boca se desplaza hacia −X y esa pared sube vertical hasta el marco
  (como la paramétrica). Si el pozo es tan somero que el cono no cabe sobre el fuste mínimo, el cono
  cede altura; si le queda menos que el espesor de pared, se cierra con una **losa superior plana** del
  espesor de pared. Es una regla de forma (para aprobar).
- **Marco** (`marco`): anillo en L (collar más asiento de la tapa), explícito.
- **Tapa** (`tapa`): modificador de Geometry Nodes (radio, espesor, elevación de la cara superior,
  desplazamiento en X, segmentos): se edita en el `.blend` de revisión.
- **Escalones** (`escalones`): en U (huella y dos patas empotradas), cajas explícitas, en la pared −X;
  en un cono concéntrico siguen la pared inclinada.
- **Banqueta** (`banqueta`): sobre la losa, partida por un canal recto en la dirección de la primera
  salida con DN; el canal es tan ancho como ese DN y la banqueta sube medio DN (altura de media caña).
  Sin salida con DN no hay canal que dibujar y no hay banqueta. Simplificación: el fondo del canal es
  plano (no semicircular) y las entradas no tienen canal propio.
- Cada parte es un sólido cerrado y lleva su material con color (`baseColorFactor`): `hormigon`,
  `fundicion`, `polipropileno`. El lector toma ese color cuando el `.glb` no trae `COLOR_0`.

### 12.3 Anclas por entrada y salida

- `bake::node_connections` saca de cada tramo que toca el pozo: nombre `salida__<código>` (el tramo
  empieza en el pozo) o `entrada__<código>` (termina en él), acimut desde el pozo hacia el siguiente
  vértice del trazado (grados desde +X en sentido antihorario, menos el `giro` del pozo si lo declara),
  la solera del tramo en ese extremo menos la solera del pozo, y el DN si es dato.
- La solera del tramo es la declarada (`cota_solera_inicio`/`cota_solera_fin`) o la del trazado si es
  dato. **Sin ella no hay ancla**: el sobre de `hornear` la lista en `sin_ancla` (`codigo`, `tramo`,
  `motivo`) y avisa `W_SIN_ANCLA`; un pozo sin tramos sale con «sin tramos conectados». El pozo se
  hornea igual: faltar un ancla no es faltar un dato del pozo.
- En el `.glb`, cada ancla es un *empty* con ese nombre en la cara exterior del fuste, a la cota de la
  solera del tramo, con `redes_tramo`, `redes_azimut_deg`, `redes_sobre_solera_m`, `redes_eje` y
  `redes_dn_mm`. El *empty* `solera` marca la solera del pozo.
- `biblioteca::anclas_de_glb` ahora lee las anclas por **nombre completo** (tres entradas dan tres
  anclas, antes una), en el marco de la malla y aplicando las transformaciones de los padres (antes
  restaba el centro en planta a la altura). `importar` las normaliza igual que los vértices
  (`anclas_normalizadas`). `anclar_conexiones` usa primero el ancla con el código del tramo.

### 12.4 Colocación en el visor

- `biblioteca::cota_base_modelo`: una pieza con ancla `solera` se coloca con su `z = 0` en
  **solera − altura del ancla** (la solera del objeto menos la losa); una sin ella, como antes, con la
  base en la cota de tapa. Lo usan el pase 3D, el pintor, el foco y `anclar_conexiones`.
- **Sustituye** a la paramétrica: en `--gpu3d`, `PlacedModel::replaces` quita la pieza de `parts.rs`
  del objeto; sin `--gpu3d`, `view::build_with_models` dibuja el modelo en su lugar por el mismo camino
  que las piezas (corte, tinte de selección, recorte y niebla). Antes el pintor dibujaba la paramétrica
  y además el proxy encima. El pintor usa la malla entera hasta 20 000 triángulos y el proxy por encima
  (aviso en la barra de estado). Sin modelo o sin `.glb`, se ve la paramétrica.
- La resta del origen sigue en `f64` antes de bajar a `f32` (docs/15).
- Corregido de paso: la app volvía a girar los ejes de una malla que el lector ya da en Z arriba, así
  que toda pieza de Blender se habría visto tumbada; ahora cada triángulo lleva su normal (antes
  `[0, 0, 1]`, sin sombreado en el pase 3D).

### 12.5 Huecos de §11.6 cerrados

- `biblioteca cantidades` (y el manifiesto) guardan los `extras` **por nodo**:
  `nodo.<nombre del nodo>.<clave>` (y `malla.<nombre>.<clave>`). El material de la tapa ya no pisa al del
  cuerpo.
- `biblioteca importar` lee los `extras` y las anclas del `.glb` de origen (el normalizado es una sola
  malla sin ellos).
- `dn_mm` es nulo en las piezas de pozo: el pozo no tiene DN; su medida principal es
  `horneado.inner_diameter_m` (y el DN de cada tubo va en su ancla).
- Inspector (MAM-1155): sin `diametro_interior`, el diámetro se muestra `—`; la pieza de respaldo se
  dibuja con la constante de dibujo `FALLBACK_DRAWING_DIAMETER_M` (1,20 m), que nunca se presenta como
  dato (`Node::inner_diameter_m` frente a `Node::drawing_diameter_m`).

### 12.6 Demo y pruebas

- `redes proyecto cadena`: los 18 pozos declaran `espesor_pared` = 0,15 m como `Dato` con fuente
  «demo: dato de ejemplo, no de un diseño» y `material` = `hormigon`, así que se hornean todos. Los
  12 pozos de la zona de agua no tienen tramos (sin anclas, avisado); los 6 del colector llevan su
  entrada y su salida.
- Pieza piloto versionada: `tools/blender/piloto/piloto.json` (tres tramos: dos entradas DN 200 y 160 y
  una salida DN 250) y su `<clave>.glb` (1 940 triángulos, 93 kB, cerrada, 1,7 × 1,7 × 2,6 m).
  `crates/redes-core/tests/pozo_horneado.rs` comprueba **sin Blender** que el nombre es la clave de la
  receta actual (si se toca la receta hay que rehornear el piloto; el mensaje dice cómo), las cinco
  partes, malla cerrada con volumen positivo por material, `extras` por nodo, base en 0 y tapa en
  losa + profundidad, y un ancla por tubo a su cota. Con Blender, `tests/hornear.rs` lo rehornea y exige
  los mismos bytes; sin Blender se salta con aviso.
- Capturas y `.blend` de revisión para aprobar: `out/f17-03/` (§12.7).

### 12.7 Para aprobar y medido

Capturas (`out/f17-03/`, carpeta ignorada; se rehacen con los mismos pasos):

```powershell
redes proyecto cadena --salida out\f17-03\cadena\cadena.redes
redes -p out\f17-03\cadena\cadena.redes biblioteca hornear --receta tools\blender\build_manhole.receta.json
$env:REDES_PROJECT = "out\f17-03\cadena\cadena.redes"
redes-app --snap out\f17-03\iso.png --scene iso            # y --gpu3d
redes-app --snap out\f17-03\foco.png --scene foco          # y --gpu3d
redes-app --snap out\f17-03\corte.png --scene corte        # corte vertical (solo pintor)
redes-app --snap out\f17-03\pozo-modelo.png --scene pozo-modelo   # y --gpu3d
```

- Las escenas `foco`, `corte` y `explosion` eligen ahora el primer pozo con modelo y al menos dos
  tramos (PZ-302 en la cadena); la nueva `pozo-modelo` lo encuadra sin terreno ni selección, para ver
  la pieza con sus materiales también en `--gpu3d` (con terreno, el pase 3D solo deja ver la tapa).
- El corte vertical solo existe en el pintor: el pase 3D no corta (como antes de F17-03).
- `.blend` de revisión de cuatro pozos distintos en `out/f17-03/revision/`: PZ-201 (sin tramos: sin
  anclas ni banqueta), PZ-301 (solo salida), PZ-302 y PZ-306 (entrada y salida).

Medido (Windows 11, Blender 5.1.0, CLI en release): la cadena tiene 18 pozos que dan **7 piezas
distintas** (los 12 sin tramos comparten una; cada pozo con tubos es la suya, porque sus anclas
son parte de la clave), horneadas en un proceso en **2,49 s** con `.blend`. Pieza con dos tubos:
1 868 triángulos; sin tubos: 1 676; piloto con tres: 1 940 (≈ 93 kB). El proyecto entero con modelos:
6 134 triángulos en el pintor en la escena `foco`.

Para el usuario: aprobar la forma de §12.1 (sobre todo el cono excéntrico hacia −X, la losa superior
plana de los pozos someros, la banqueta de medio DN con fondo plano y que la banqueta solo exista
con una salida con DN) y las capturas.

## 13. Familias de F17-04…F17-07 y topografía

Los informes de cada familia horneada están en `docs/familias/`: tubería ([F17-04](familias/F17-04-tuberia.md)), accesorios ([F17-05](familias/F17-05-accesorios.md)), captación ([F17-06](familias/F17-06-captacion.md)), planta ([F17-07](familias/F17-07-planta.md)) y topografía ([F17-11](familias/F17-11-topografia.md)). Cada uno trae la partición dato/forma, los pilotos versionados en `tools/blender/piloto/`, las cifras y las decisiones pendientes.
