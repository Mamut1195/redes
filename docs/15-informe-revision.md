# 15 — Revisión del consejo (fases 1‑3) y respuesta

> ⚠️ **Cifras corregidas.** Varios números de este documento se escribieron antes de que el
> código cambiara y ya no son ciertos. Las cifras vigentes, con cómo reproducirlas, están en
> [`21-cifras-medidas.md`](21-cifras-medidas.md). Este documento se conserva como historia.


> Tres revisiones independientes sobre el código y los artefactos de las fases 1‑3, y qué se
> corrigió antes de seguir. Informes completos:
> [12-revision-arquitectura](consejo/12-revision-arquitectura.md),
> [13-revision-visual](consejo/13-revision-visual.md),
> [14-revision-pruebas](consejo/14-revision-pruebas.md).

## Lo que encontró el consejo

| # | Hallazgo | Gravedad | Estado |
|---|---|---|---|
| 1 | **Precisión**: las tuberías se construían en `f32` con coordenadas UTM (N≈9,8 M). El paso de `f32` allí es **1 m**, así que la sección del tubo colapsaba y todo tubo se dibujaba como cinta plana. El `.glb` heredaba el defecto. | bloqueante | **corregido**: los tubos se generan en coordenadas locales y solo después se exagera la z |
| 2 | **Siete familias nunca se dibujaban**: el filtro del render pedía `HYDRAULIC_NODE`/`ATTACHABLE` y `comun.accesorio`, `tapa`, `escalera`, `cuerpo_receptor`, `dado_anclaje`, `cruce`, `hito` solo tienen `REPORTABLE`/`STRUCTURAL`. La prueba de cobertura llamaba a la pieza directamente, así que **la suite verde certificaba algo que el render no hacía**. | bloqueante | **corregido**: el filtro admite cualquier familia con geometría; la cobertura ahora pasa por el mismo camino que el render |
| 3 | **El corte no cortaba tubos** y el fuste del pozo era un cilindro macizo: el documento 11 afirmaba "espesor de pared, escalera, banqueo" y no era cierto. | bloqueante | **corregido**: el corte recorta también los tubos y el fuste es un **tubo hueco** con 12 cm de pared |
| 4 | **Índice del TIN perdido al cargar**: `TriIndex` es `#[serde(skip)]`, así que un proyecto abierto de disco tenía el TIN pero `z_at()` devolvía `None` para todo punto. Los edificios y las calles no se dibujaban ni se exportaban. | bloqueante | **corregido**: `Terrain::reindexar()` al cargar, con prueba de ida y vuelta |
| 5 | **Atributos duplicados** en el catálogo generado (`diametro_nominal` ×2 en 8 familias, `material` ×2) y `redes esquema` publicaba la clave repetida. | importante | **corregido**: deduplicados; queda un generador que no está en el árbol (el catálogo vive ya en `families.rs`) |
| 6 | **Edición paramétrica frágil**: `ajustar` no entraba en el journal, podía hacer panic si el clamp quedaba invertido y los botones de profundidad estaban al revés. | importante | **corregido**: `commit("ui.parametro")`, clamp con límites ordenados y signo correcto |
| 7 | **DXF no es ida y vuelta**: al reimportar, los 46 nodos acaban como `saneamiento.pozo`, se regeneran códigos y se pierden `attrs`/`datos`. | importante | **abierto** (no es de fases 4‑5) |
| 8 | **CLI sin pruebas de contrato** y `exportar gltf` informaba `"primitivas": 0` en duro. | menor | **corregido** el contador; faltan las pruebas de contrato |

## Lo que la revisión visual dejó claro

- Las piezas enterradas se dibujan enteras sobre el terreno (**rayos X a propósito**, para revisar),
  y eso hace que el catálogo parezca una maqueta de tambores sobre césped. Es una decisión de
  producto, no un fallo: en modo revisión hay que ver lo enterrado.
- **La tapa no se lee**: es un disco 6 cm dentro de un cono y el marco no tiene espesor. Es el
  arreglo visual de mayor impacto pendiente.
- El **corte no tiene poché** (la sección es una línea, no un relleno tramado).
- Etiquetas sin carriles: se cortan en los bordes.

Estos cuatro son mejora visual de fase 6 y están anotados, no corregidos.

## Números de la revisión de pruebas

| Métrica | Valor medido |
|---|---|
| Suite | 57 verdes (49 núcleo + 7 app + 1 doctest), 25 s |
| Catálogo | 29 familias / 216 subtipos exactos; el demo cubre 29/29 en 49 objetos |
| Tapas vs TIN | máximo 0,187 m, media 0,046 (el criterio era ±0,35) |
| Recubrimiento | 1,200–6,000 m |
| Pendiente mínima | 1,245 % (criterio ≥0,4 %) |
| Frame 2D release (p95) | 0,7–1,3 ms en reposo; **33,9 ms** en `ciudad`, **247 ms** en `corte` |
| Frame 3D (`--gpu3d`, p95) | **2,3 ms** en `ciudad`, **2,5 ms** en `corte` |

El pase 3D con profundidad es, sobre todo, lo que arregla el corte: 247 ms → 2,5 ms.

## Veredicto del consejo y qué se hizo

El consejo dijo: **no pasar a fase 5 sin antes arreglar la precisión, el filtro de familias y el
corte**. Se arreglaron los tres (más el índice del TIN, que bloqueaba el export), y con eso se
pudo cerrar la fase 4 (glTF) y la fase 5 (pase 3D con profundidad).

Queda abierto, por orden de impacto:

1. Ida y vuelta DXF (pierde familia y atributos).
2. Pruebas de contrato del CLI.
3. Tapa y marco legibles; poché en la sección.
4. Etiquetas con carriles y detección de solape.
