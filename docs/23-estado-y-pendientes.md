# 23 — Estado y pendientes (traspaso)

Documento de traspaso: qué está hecho, qué falta y por dónde seguir. Las cifras vigentes están en
[`21-cifras-medidas.md`](21-cifras-medidas.md); las decisiones, en [`22-decisiones.md`](22-decisiones.md).

## Lo que está hecho y verificado

| Bloque | Estado |
|---|---|
| Registro | **46 familias / 315 subtipos**, incluidas las 17 de la cadena de agua cruda |
| Cadena completa | `redes proyecto cadena`: 90 objetos, 72 tramos, de la bocatoma (2530 m) a la descarga (2498 m) |
| Modelos | importación, validación, medidas, caché, dibujo en el pase 3D, política de pesados, foco sobre el modelo, **selector en el inspector** y **anclas de conexión** (se leen del `.glb` y la tubería nace en el puerto, no en el centro) |
| Cantidades | `redes cantidades`: unidades, longitudes por DN/material, volúmenes, pesos, discrepancias y CSV |
| CAD | ida y vuelta DXF sin pérdida (46 familias y 7 arcos vuelven idénticos), **bloques con atributos** por familia y GeoJSON con atributos |
| Calidad | **275 pruebas** (núcleo 150 + 13 validar + 5 acta + 8 reimportar + 32 ingesta + 3 cotas + 2 rendimiento + 3 índices + 1 banco + 2 escena, app 25, CLI 19 + 3 + 4 + 2 acta + 2 reimportar, doctest 1), arnés de 29 mutaciones, CI en GitHub **en verde** |
| Formato (F15-01, D10) | solo se abre la versión vigente: `redes.project/3` (otra versión → `E_PROJ_SCHEMA`, con pista para regenerar desde la entrega) y `redes.plan/1` (otra → `E_FMT_VERSION`); sin migraciones antes de v1; demos de `demo/` regeneradas a `/3` |
| Revisor (F13) | `redes validar` geométrico con panel Validación (F13-01, D6), `redes perfil` con botón Perfil en el inspector (F13-02), observaciones con historial (`objeto observar/observaciones/cerrar-observacion`, panel en el inspector, `redes.project/3`, F13-03, D7; sin conversión de `.redes` anteriores), `redes proyecto diff` (F13-05, compara estado y cada observación por id), `redes acta` en CSV (Excel, BOM y «;») o HTML imprimible (F13-04, sin autor si no se registró, D2), `redes importar reimportar` (F13-06, D9: empareja por código, conserva estados y observaciones, reinicia a pendiente el aprobado cuyo diseño cambió y exige `--aceptar-bajas` para dar de baja objetos con revisión) |
| Índices (F14-01) | `Project` indexa por id, por código y nodo → tramos (`indice.rs`, no se serializa; se rehace al abrir y al reimportar); 50k tramos: ingesta 91 s → 485 ms, escena 4,0 s → 49 ms, reimportar 167 s → 912 ms (`docs/21` §4.1). Un código dado de baja se puede reutilizar: la unicidad de códigos al abrir solo cuenta objetos vivos (los ids, todos). **D11**: nodos y tramos vivos comparten un único espacio de códigos; `add_node`/`add_link` rechazan el choque (`E_STATE_DUPLICATE_NAME`) y la ingesta lo descarta con aviso. La app busca por código con el índice (`node_by_code_mut`) |
| Pase 3D (F14-02, F14-03) | La cámara no regenera la malla (`ClaveMalla`, exageración vertical en la matriz, `rebuild()` sin `build_opts` en `--gpu3d`) y el búfer no se copia por frame (`Arc<[u8]>`, `Geometria::tomar`). Ciudad: cambio de cámara 6,03 ms → 0,008 ms; 30 → 0 regeneraciones en 30 cambios; 3,6 MB → 0 copiados por frame (`docs/21` §4.2). La escena `--snap pozos` vuelve a mostrar el pozo (miraba a un punto fijo de un demo anterior; lienzo vacío con y sin `--gpu3d`) |
| Rendimiento P1 (F14-04, F14-06, F14-07, F14-09, F14-10) | Caché de mallas por pieza y `Biblioteca` en `Arc` (F14-06); banco de 20k tramos con umbral en el CI (paso «Banco», release) y crecimiento 5k→20k (F14-10); perfil release con LTO fina y dependencias en `opt-level 2` en dev (F14-09); escena con cámara precalculada, sin `Vec` por triángulo y en hilos (F14-04); recorte por pantalla y `seleccion::RejillaSeleccion` para el clic (F14-07). 20k tramos, release: escena 30 → 7 ms, ingesta 392 → 182 ms, reimportar 697 → 349 ms; de cerca 444 → 5 ms (`docs/21` §4.3) |
| Pase 3D sin pánico en GPUs de 256 MB (F14-05, parte P0) | El tope del búfer de vértices sale de `device.limits().max_buffer_size` (`gpu3d::tope_bufer`, sin pasar de 512 MB) y el recorte avisa en la barra de estado. Red sintética de 5k tramos (~10k objetos, 285 MB de vértices): `--gpu3d --snap` sale con 0 (antes, pánico «Buffer size 301989888 > 268435456») |
| Inspector de solo lectura (D12) | Se retiró «Parámetros (en vivo)» (`State::ajustar`, journal `ui.parametro`, que inventaba DN 200): la app no edita el diseño |
| Producto | plantilla, licencias de modelos, journal con fecha, paquete portable y **release 0.1.0 publicada** |
| Documentación | 23 documentos; los informes antiguos llevan aviso de cifras corregidas |

## Lo que falta (por orden de valor)

Revisión para v1 del 25/11/2026 (nota 5,5/10): el plan está en `docs/18-backlog.md`, fases **F12–F16**,
y en Linear (MAM-1085…MAM-1139). Se trabaja **por fases en orden**; no se abre una fase sin cerrar la anterior.

| Fase | Estado |
|---|---|
| F12 — Corrección de dominio | **cerrada** (F12-01…F12-15). Ingesta por skill guiada (D5) |
| F13 — Herramientas del revisor | **cerrada** (F13-01…F13-06): `redes validar`, perfil longitudinal, observaciones con historial, `redes acta`, `proyecto diff` y `importar reimportar` |
| F14 — Rendimiento con redes reales | **en curso**: **P0 cerrados** — F14-01 (índices en `Project`, MAM-1105), F14-02 (cámara 3D sin regenerar malla, MAM-1106) y F14-03 (sin copia del búfer por frame, MAM-1107). **P1 hechos**: F14-04, F14-06, F14-07, F14-09, F14-10. Siguen F14-05 (LOD, recorte e instanciado en el pase 3D; ya hecha la parte P0 del tope por dispositivo; hay un borrador sin integrar en `wip/f14-05`, ver abajo) y F14-08 (deshacer por diferencias, P2) |
| F15 — Arquitectura | pendiente; **F15-01 hecha por adelantado** (solo la versión vigente del `.redes`, D10, MAM-1115). Falta agregado, catálogo como datos y errores tipados |
| F16 — Release v1 | pendiente (F16-01 ya cerrada por F12-09) |

Decisiones abiertas del usuario: A1 (vertedero), A2 (pozo profundo), sumideros de la topología
(`is_sink` en `topology.rs`: descarga, cuerpo receptor, reactor, laguna, sedimentador).
Sin validar en CAD real: el DXF de F12-08 (no hubo `ezdxf`); abrir una demo exportada en AutoCAD/BricsCAD.

Pendientes menores conocidos de F13 (no bloquean F14):

- `redes acta` e `importar reimportar` solo están en la CLI: no tienen botón en la app.
- Reimportar no reaplica las anclas de `biblioteca aplicar`: tras reimportar hay que volver a aplicar el modelo.
- Cada reimportación añade una entrada a `fuentes` aunque la entrega sea idéntica.
- La escena `--snap` no permite seleccionar un tramo (no se puede capturar el inspector con un objeto elegido).

Pendientes conocidos de F14:

- En la cadena completa (`redes proyecto cadena`), el último tramo de alcantarillado hacia la descarga
  queda fuera de la superficie del terreno. Sin revisar si es el dato del demo o el dibujo.
- `ClaveMalla` supone que todo cambio del proyecto sube `revision` (o cambia el `Arc` del proyecto).
  Un cambio que no pase por `commit` dejaría la malla del pase 3D desfasada hasta el siguiente.
- F14-04 pedía escena ≥ 3×: con el mismo perfil da ×2,7 a 20k encuadrada y ×3,7 a 50k (×4,3 a 20k contando
  F14-09). Se dio por hecho; si se quiere el ×3 estricto a 20k, lo que queda es el teselado de tubos y pozos.
- La rejilla de selección (F14-07) se rehace al primer clic tras cada `rebuild`; con 20k encuadrados una
  celda junta hasta ~3 000 objetos (filas lejanas apiñadas). Es O(1) amortizado por cámara, no por densidad.
- Borrador de F14-05 sin integrar: `escena3d.rs` (rescatado de un worktree, ~900 líneas): prototipos
  indexados por trozos canónicos, tres LOD (completo, simple, caja), recorte por tronco en lotes de 16×16,
  `planificar` por cámara y aviso al pasar el tope. Una sola prueba; no está en `lib.rs` ni conectado
  a `gpu3d.rs` (faltan búferes de instancias y el shader).
- El aviso «Pase 3D recortado…» lo fija el compositor al dibujar, así que aparece en el frame
  siguiente: no sale en una captura `--snap` de un solo frame (sí en la consola). Con la red de 5k el
  pase dibuja ~90 % de los vértices; el arreglo de fondo sigue siendo F14-05 (LOD e instancias).
- `gpu3d.rs`: el campo `Recursos::depth` da el aviso `dead_code` (ya estaba antes de F14-02; solo se
  guarda para mantener viva la textura de profundidad).

## Cómo retomar

```powershell
cargo test --workspace                       # 275 pruebas
powershell -File scripts/mutar.ps1           # 29 mutaciones
powershell -File scripts/empaquetar.ps1      # zip portable
gh run list --limit 3                        # CI
```

Para el proxy (1): `modelos::proxy(malla, paso)` devuelve la malla decimada; el punto de enganche es
el pase del pintor donde hoy se cae a la pieza paramétrica.

## Deuda conocida

- `redes-cli/src/main.rs` concentra todos los comandos; si crece, toca partirlo por dominio.
- El índice de `Project` detecta altas y vectores cambiados de tamaño, pero no reescribir un objeto
  en su sitio (cambiarle id, código o extremos): quien lo haga debe llamar a `reindexar()` hasta que
  F15-02 cierre los campos públicos.
- El `journal` no guarda identidad (decisión D2); si algún día se pide, es un campo opcional más.
- El `.glb` del catálogo y de la ciudad incluyen el terreno: las cifras de triángulos de `docs/21`
  lo tienen en cuenta.
