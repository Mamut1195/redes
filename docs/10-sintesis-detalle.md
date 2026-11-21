# 10 — Síntesis: catálogo de piezas y motor gráfico

> Este documento cierra el consejo sobre detalle. Responde las dos preguntas del cliente:
> «¿qué familias y con qué detalle?» y «¿no sería mejor three.js?». Al final dice qué quedó
> implementado y qué sigue.

---

## 1. Lo que pidió el cliente

1. Familias de **todo** lo que hay en una red de agua potable y alcantarillado: pozos de
   distintos tipos, colectores, vertederos, válvulas, accesorios…
2. **Verlas dibujadas**, y que sean **paramétricas**.
3. Que «se vean como en la realidad» — forma real, no fotorrealismo.
4. Un consejo que defina el nivel de detalle necesario.
5. Una opinión sobre el motor: ¿three.js o el actual?

El consejo produjo cuatro documentos: [06-catalogo-familias](06-catalogo-familias.md) (2 127
líneas), [07-geometria-parametrica](07-geometria-parametrica.md) (1 025),
[08-motor-grafico](08-motor-grafico.md) (650) y [09-ux-detalle](09-ux-detalle.md) (972).

---

## 2. Catálogo: de 18 familias a 46 y 315 subtipos

El registro tenía 18 claves y 60 subtipos. El consejo propuso **29 familias y 216 subtipos**; la cadena de agua cruda (docs/20) añadió **17 familias y 99 subtipos netos** hasta **46 y 315**,
con una regla que evita explotar el catálogo:

> **El subtipo es la función; la variante de forma es un atributo.**

Así, «compuerta con husillo ascendente» es `compuerta` + `tipo_cuerpo`, y «codo a 22,5°» es
`codo` + `angulo`. Doce subtipos de válvula cubren 25 productos.

| Bloque | Familias | Subtipos |
|---|---|---|
| Saneamiento | 12 | 95 |
| Agua potable | 11 | 76 |
| Común (accesorios, tapas, escaleras, receptor) | 6 | 45 |
| **Total** | **29** | **216** |

Familias nuevas: `comun.accesorio`, `comun.tapa`, `comun.escalera`, `comun.cuerpo_receptor`,
`saneamiento.caja`, `saneamiento.arenero`, `saneamiento.trampa_grasa`, `agua.arqueta`,
`agua.hidrante`, `agua.dado_anclaje`, `agua.acometida`.

**Correcciones de dominio** (no solo ampliación): el hidrante sale de `agua.llave` y pasa a
familia propia; la acometida de agua potable sale de `saneamiento.acometida`; la caja de revisión
y el bombeo salen de `saneamiento.pozo`. Con alias de migración para no romper archivos.

**Trampa resuelta:** `dn_mm` (nominal), `od_mm` (exterior) e `id_mm` (interior) son campos
distintos. Un PVC de 200 mm exterior no es un DN 200 de agua potable; sin separarlos, el 3D
miente en el diámetro.

---

## 3. Geometría: cómo se hace «real» sin texturas

Cada pieza es un **ensamblaje de primitivas** (cilindro, tronco de cono, caja, tubo, anillo,
disco, prisma, barra) con parámetros derivados de los del objeto: `D`, `e`, `H`, `dn`, `angulo`.
No hay mallas externas ni texturas.

**Niveles de detalle por tamaño en pantalla** (criterio verificable, no un deseo):

| Tamaño | Qué se dibuja |
|---|---|
| < 3 px | símbolo de tamaño fijo (rombo) |
| 3–40 px | sólido simple: cuerpo + cono + tapa |
| > 40 px | completo: cono, marco, tapa, peldaños, bridas, volante |

Presupuesto por pieza: **118 caras / 260 triángulos** un pozo completo; **17/38** el simple.
Escena en órbita: ~3 750 caras → **≈11 ms**.

Reglas de forma que hacen que se reconozca:

- **Pozo**: solera + fuste + **cono de reducción** + marco + tapa + escalera. El cono es lo que
  distingue un pozo de un tubo parado; se dibuja en todos los LOD.
- **Pozo de caída**: tubo vertical de caída adosado al fuste.
- **Válvula**: cuerpo **horizontal** con dos bridas, vástago y volante (compuerta), palanca
  (mariposa), sin accionamiento (retención), campana (aire/purga), cámara de diafragma (RP).
- **Tubería**: tubo con **juntas cada 6 m** y recorte en la pared del pozo (no lo atraviesa).
- **Descarga**: muro de cabeza + alas inclinadas + losa disipadora.
- **Hidrante**: cuerpo + bonete + tres bocas.

---

## 4. Motor gráfico: la respuesta a three.js

### Qué hay hoy, medido en esta máquina

| Dato | Valor |
|---|---|
| Coste de construir la escena | **0,39 µs por primitiva** (5 328 prims → 2,1 ms) |
| Reparto en la ciudad | terreno 0,57 ms · **contexto urbano 1,51 ms (65 %)** · **red 0,02 ms (1 %)** |
| Órbita real | 2,4 ms p50 (demo) · 33 ms con 45 mil primitivas |
| Frame frío | 20 ms release · 223 ms debug |
| Techo estimado a 60 fps | ~5 800 primitivas/frame con el renderer actual |

**El dato que ordena todo:** la red es el **1 %** del coste. Detallar pozos y válvulas es barato;
lo caro es el terreno y la ciudad. Por eso «más detalle» no exige cambiar de motor.

### Las opciones, comparadas

| Opción | Esfuerzo | Qué gana | Qué pierde | Veredicto |
|---|---|---|---|---|
| **A. Mejorar el renderer propio** (LOD, presupuesto, culling, sombreado) | 2 semanas | 2–4× más objetos; cero dependencias nuevas | Sin búfer de profundidad ni materiales | **Sí, ahora** |
| **B. Pase 3D real con `wgpu` dentro de Rune** | 3–5 semanas | Z-buffer, materiales, instancing, 100k+ triángulos | API pre-release (`Compositor`), riesgo de romperse al actualizar Rune | **Sí, cuando el catálogo lo pida** |
| **C. three.js en webview o app web** | 4–8 semanas | Materiales y sombras de un motor maduro | Segundo runtime (WebView2: 1,7 GB en esta máquina), segundo lenguaje, IPC con el núcleo, se pierde la app nativa y la paridad CLI↔UI | **No** |
| **D. Exportar glTF/GLB** | 3–5 días | Se abre en Blender, three.js, Windows 3D Viewer; el cliente ve el modelo donde quiera | No es la app; es un entregable | **Sí, ya** |
| **E. Motor Rust (bevy/rend3)** | 3–6 meses | 3D completo | Se rehace el 80 % del producto | No |

### Por qué three.js no entra (y no es por rendimiento)

three.js **no degradaría** el rendimiento: para 50 mil triángulos va sobrado. El problema es
otro:

1. **Segundo runtime.** Habría que embeber un navegador (WebView2) o publicar una web aparte.
   Eso rompe «un solo .exe», añade 1,7 GB de dependencia en esta máquina y mete un segundo
   lenguaje (JS) y un canal IPC para hablar con el núcleo en Rust.
2. **Se pierde la paridad CLI ↔ app** que hoy es automática: la CLI y la app comparten
   `redes-core`; con una vista web habría dos caminos para el mismo dato.
3. **El problema que resolvería no es el que tenemos.** Falta detalle en las piezas, no
   materiales PBR. Un pozo se reconoce por su cono y su tapa, no por su rugosidad.

**Dónde sí sirve three.js:** como **visor externo del `.glb`**. Exportamos y el cliente lo abre
en Blender, en three.js o en el visor de Windows. Eso es la opción D, y es complementaria.

### Cuándo se justificaría el pase 3D real (opción B)

Rune expone `Compositor` con `Frame::device/queue/target` y slots de efecto: **un pase con
búfer de profundidad es API pública, no un hack** (documentado en
[08-motor-grafico](08-motor-grafico.md) §2). Se justifica cuando:

- el catálogo pase de ~200 piezas visibles a la vez, o
- haga falta **corte** real (sección del pozo) y **explosión**, que el pintor no puede hacer bien, o
- el cliente pida materiales y sombras.

Antes de comprometerlo hay que prototipar un día: verificar que `EffectPhase::Backdrop` abre un
pase con profundidad sobre el target de la ventana.

---

## 5. Qué quedó implementado en esta pasada

| Pieza | Estado |
|---|---|
| Registro | **46 familias y 315 subtipos**: 29 del consejo + 17 de la cadena de agua cruda |
| Primitivas | cilindro, tronco, caja, tubo, anillo, disco, prisma, barra, barra con tapas |
| Pozos | inspección, caída, resalto, ciego, bombeo, limpieza (cono, marco, tapa, escalera) |
| Válvulas | compuerta (volante), mariposa (palanca), retención, aire/purga, RP |
| Tuberías | tubo con juntas y recorte en el pozo; línea cuando está lejos |
| Otros | hidrante, tanque, descarga, cámara, sumidero, medidor, arqueta, caja, arenero, trampa, tapa, escalera, dado, accesorios (codo, tee, cruz, reducción, unión, tapón) |
| LOD | por tamaño en pantalla (símbolo / simple / completo) + presupuesto de triángulos |
| Demo | `redes proyecto catalogo` con una muestra de cada familia |

**Fase 1 cerrada** con evidencia:

- `redes esquema` publica **46 familias / 315 subtipos**; la prueba `el_catalogo_tiene_la_escala_del_consejo` falla si baja de 29/216.
- La prueba `todas_las_familias_dibujan` recorre el registro y exige que **cada familia** produzca
  geometría (pieza de nodo o tubo), así que una familia nueva sin dibujo rompe la suite.
- `redes proyecto catalogo-total` arma una muestra de **las 46 familias** en una rejilla, y la prueba `cada_familia_del_registro_dibuja` falla si alguna no dibuja
  (las dos restantes son variantes de arco sin nodo propio).

**Fase 2 y 3 cerradas** (ver [11-modos-inspeccion](11-modos-inspeccion.md)):

| Modo | Estado | Prueba |
|---|---|---|
| Foco (cámara + escala real) | hecho | `el_boton_foco_acerca_la_camara` |
| Corte vertical con arista marcada | hecho | `el_corte_parte_un_triangulo_en_dos`, `el_corte_reduce_la_geometria_dibujada` |
| Explosión por partes nombradas | hecho | `la_explosion_separa_las_partes_del_objeto_enfocado` |
| Etiquetas de código y subtipo | hecho (presupuesto 24) | `las_etiquetas_son_pocas_y_tienen_texto` |
| Leyenda por forma y texto | hecho | (UI) |
| Ficha paramétrica en vivo + deshacer | hecho | `los_parametros_se_editan_en_vivo_y_se_deshacen` |

**Fase 4 y 5 cerradas** (ver [16-fase-4-5-gltf-y-3d](16-fase-4-5-gltf-y-3d.md)):

| Entrega | Estado | Verificación |
|---|---|---|
| `exportar gltf` (.glb con una primitiva por familia, terreno, calles y manzanas) | hecho | estructura GLB + importación en Blender 5.1 headless |
| Pase 3D con búfer de profundidad (`--gpu3d`) | hecho | capturas headless; p95 247 ms → 2,5 ms en corte |

Pendiente: relleno tramado de la sección, carriles de etiquetas con detección de solape, tapa y
marco legibles, y la ida y vuelta DXF.

---

## 6. Plan para llegar al detalle completo

| Fase | Qué | Esfuerzo |
|---|---|---|
| 1 | Completar el catálogo de piezas: 216 subtipos con sus partes (las 65 partes dibujables del consejo) | 4–6 semanas |
| 2 | Corte vertical, explosión y foco (modos del consejo de UX) | 3 semanas |
| 3 | Etiquetas, leyenda accesible y ficha paramétrica editable en vivo | 2 semanas |
| 4 | `exportar gltf` verificado con `gltf-validator` y Blender headless | 1 semana |
| 5 | Pase 3D con profundidad (opción B) si el catálogo lo pide | 3–5 semanas |

**Criterio de aceptación de «se ve como en la realidad»** (medible, del consejo):

1. A 40 px de tamaño, un pozo muestra cono, marco, tapa y peldaños; una válvula muestra bridas,
   vástago y volante.
2. La tapa está a `z_rim` y la solera a `z_invert` (±1 mm).
3. Ninguna pieza se dibuja a cota 0 por falta de datos: se dibuja un marcador y se avisa.
4. Una escena con 200 piezas visibles se mantiene sobre 30 fps al orbitar.
