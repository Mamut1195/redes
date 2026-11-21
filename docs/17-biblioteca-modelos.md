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
