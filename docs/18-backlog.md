# 18 — Backlog: fases y tickets de lo que falta

> Todo lo que queda, en tickets con criterio de aceptación verificable y esfuerzo. Ordenado por
> fases; dentro de cada fase, por dependencia. Estimaciones en días de una persona.

## Estado de partida (lo que ya está)

Fases 1–5 cerradas: catálogo 29 familias / 216 subtipos, foco/corte/explosión, etiquetas, ficha
paramétrica, glTF de salida, pase 3D con profundidad, lector `.glb`, validador, importador con
normalización, manifiesto y medición de volumen/área.

Lo que sigue se agrupa en **seis fases**. La ruta corta a valor para el usuario es
**F6 → F7 → F8**.

---

## F6 — Modelos propios en el visor — **F6-01, F6-02, F6-03, F6-04 y F6-07 hechos**

> Un objeto con `modelo` se dibuja con su `.glb` en el pase 3D, con caché por pieza, política de
> modelos pesados (aviso si no hay `--gpu3d`) y **foco sobre la caja del modelo** (`F6-07`).
> Quedan F6-05 (proxy decimado), F6-06 (selector en el inspector) y F6-08 (anclas de conexión).


Objetivo: que un modelo importado se vea en su pozo, válvula o planta, con su cota y su giro.

| ID | Ticket | Alcance | Aceptación | Días |
|---|---|---|---|---|
| F6-01 · [MAM-1066](https://linear.app/mamutsystems/issue/MAM-1066) | **Campo `modelo` en el modelo de datos** | `Node.modelo: Option<String>` y `Project.biblioteca: Option<Biblioteca>`; serde con default para abrir archivos viejos | guardar y abrir conserva el campo; prueba de compatibilidad con un `.redes` anterior | 1 |
| F6-02 · [MAM-1067](https://linear.app/mamutsystems/issue/MAM-1067) | **Caché de mallas** | cargar cada `.glb` una vez, decodificar a `Vec<Vertice>` y reutilizar por `(id, lod)` | dos nodos con el mismo modelo usan una sola carga (contador en prueba) | 2 |
| F6-03 · [MAM-1068](https://linear.app/mamutsystems/issue/MAM-1068) | **Colocación en el pase 3D** | dibujar la malla del modelo en la posición del nodo, con su cota y giro; instancias | captura headless: el modelo aparece dónde y cómo debe | 2 |
| F6-04 · [MAM-1069](https://linear.app/mamutsystems/issue/MAM-1069) | **Política de dibujo** | si hay modelos: exigir `--gpu3d` o usar proxy decimado; aviso claro | con modelos y sin `--gpu3d`, la app avisa y no se arrastra | 2 |
| F6-05 | **Proxy decimado** | simplificar a ≤ 400 triángulos para el renderer 2D | la misma pieza se ve en 2D y en 3D sin saltos | 2 |
| F6-06 | **Selector en el inspector** | elegir modelo entre los de la biblioteca para esa familia/DN; el más reciente por defecto | el cambio se ve en vivo y queda en el journal | 1 |
| F6-07 | **Foco sobre el modelo** | encuadrar usando la caja del modelo, no la paramétrica | Foco encuadra la pieza importada completa | 1 |
| F6-08 | **Anclas de conexión** | leer `empties` `entrada`/`salida`/`cielo` y usarlas para recortar tuberías | la tubería nace y muere en el ancla, no en el centro del nodo | 2 |

**Total F6: 13 días.** Sin F6-01 y F6-03 no se ve nada; F6-02 y F6-04 son lo que evita que la app
se arrastre.

---

## F7 — Datos y cantidades (medición) — **cerrada**

> Implementado el 21/11/2026 (commit `65db0fe`): `redes cantidades` con unidades por familia,
> longitud por familia/DN/material, volumen y área medidos, peso por densidad, discrepancias
> declarado/medido con tolerancia y export CSV (BOM y `;`, Excel en español). **Sin valoración**:
> la app emite cantidades, no precios (F7-07 cerrado como «no»).


Objetivo: que el modelo alimente el listado de cantidades con procedencia, y que un descuadre se
vea.

| ID | Ticket | Alcance | Aceptación | Días |
|---|---|---|---|---|
| F7-01 · [MAM-1070](https://linear.app/mamutsystems/issue/MAM-1070) | **Volcar medidas al objeto** | al importar, escribir `datos: { volumen_m3, area_m2, peso_t }` con `fuente: "modelo:<id>"` | el inspector muestra la procedencia; `objeto lista --pretty` la incluye | 1 |
| F7-02 | **Reconciliación declarado/medido** | comparar `extras` (`redes_volumen_m3`) con lo medido; tolerancia por familia; aviso | con un modelo que declara 1,2 y mide 3,4, aparece aviso y no se pierde ninguno de los dos | 2 |
| F7-03 | **Listado de cantidades** | comando `redes cantidades --por familia` con volumen, área, longitud por DN y material | JSON y CSV con totales; cuadra con la suma de los objetos | 2 |
| F7-04 | **Longitudes de tubería** | longitud por tramo, por DN y material, descontando la parte dentro del pozo | suma de longitudes cuadra con el modelo | 1 |
| F7-05 | **Densidades y pesos** | tabla material → densidad; peso automático | hormigón 2,4 · acero 7,85 · PVC 1,4 (configurable) | 1 |
| F7-06 | **Exportar a Excel/CSV** | hoja de cantidades para el presupuesto | abre en Excel sin avisos; columnas estables | 1 |
| F7-07 | **Decisión: ¿precios?** | definir si la app valora cantidades o solo las emite | decisión escrita; si no, el ticket se cierra como “no” | 0,5 |

**Total F7: 8,5 días.** F7-01..03 es el mínimo que hace útil la medición.

---

## F8 — Intercambio con CAD (ida y vuelta) — **F8-01 y F8-05 hechos**

> `exportar dxf` escribe una capa `REDES_INFO` con familia, subtipo, código, cotas, trazado,
> atributos y datos; al importar se reconstruye el proyecto desde ella (heurística por capas solo
> para archivos ajenos). GeoJSON incluye ya los atributos de la familia. Ida y vuelta verificada con
> el catálogo completo: 46 familias y 7 arcos vuelven idénticos. **F8-02** (bloques) queda como
> mejora cosmética para CAD; **F8-03** (DWG) sigue bloqueado por la licencia.


Objetivo: que lo que el CAD devuelve no pierda familia, código, atributos ni datos.

| ID | Ticket | Alcance | Aceptación | Días |
|---|---|---|---|---|
| F8-01 | **Arreglar el round-trip DXF** | exportar e importar conserva `family`, `ty`, `code`, `attrs`, `datos` | prueba: exportar 44 objetos, borrar el `.redes`, importar el DXF → 44 objetos con los mismos datos | 2 |
| F8-02 | **Bloques con atributos** | cada familia como bloque con atributos (código, DN, cotas) para que el CAD los devuelva | un bloque editado en CAD vuelve con los atributos actualizados | 2 |
| F8-03 | **Importar DWG** (cerrado como «no»: ver `22-decisiones.md`) | conversión vía ODA File Converter (licencia) o servicio; decisión previa | un `.dwg` real se abre y conserva capas y bloques | 2 + licencia |
| F8-04 | **Fixture real** | archivo del cliente para calibrar capas y bloques | importa sin descartes y con informe de mapeo | 1 |
| F8-05 | **GeoJSON/Shapefile con atributos** | exportar atributos y estilos | QGIS abre el archivo con la tabla completa | 1 |

**Total F8: 8 días** (+ coste de la licencia ODA si se quiere DWG).

---

## F9 — Calidad técnica — **F9-01…F9-04 hechos**

> Contrato del CLI con 11 pruebas sobre el binario real (sobre, códigos de salida, `--dry-run`,
> `--pretty`, `--quiet`), fixtures de los cuatro importadores (DXF ajeno, GeoJSON municipal,
> terreno CSV con fila mala y plan de la skill) y presupuesto de frame medido: ciudad p95 34,5 ms,
> corte 33,9 ms, catálogo 6,6 ms en depuración. **F9-05** completado salvo el remote de GitHub.


| ID | Ticket | Alcance | Aceptación | Días |
|---|---|---|---|---|
| F9-01 | **Pruebas de contrato del CLI** | sobre, `error.code`, exit codes, `--dry-run` | 0 pruebas → ≥ 12; ninguna toca la red | 2 |
| F9-02 | **Fixtures de ingesta** | DXF, GeoJSON, terreno, `.inp` en `tests/fixtures` | cada importador tiene su prueba con fixture | 2 |
| F9-03 | **Presupuesto de frame en CI** | p95 por escena como umbral | `corte` < 50 ms y `ciudad` < 40 ms en modo 2D | 1 |
| F9-04 | **Persistencia** | guardar/abrir conserva entorno, TIN, attrs, datos, fuentes | prueba de ida y vuelta completa | 1 |
| F9-05 | **Git + CI** | `git init`, `.gitignore`, workflow fmt/clippy/test/release | el repo no tiene `target/`; CI verde en un push | 1 |
| F9-06 | **Telemetría y LOD** | prims · ms · fps en la barra; degradación automática | al pasar el techo, baja el detalle y lo dice | 2 |

**Total F9: 9 días.** F9-05 es lo que permite trabajar en equipo sin perder cambios.

---

## F10 — Acabado visual — **cerrada (F10-01…F10-06)**

> Tapa y marco legibles: marco con canto de 7 cm, tapa a ras de la calle con grabado y bisagra, y
> escalera que muere en el arranque del cono (ya no lo atraviesa). Interruptor de **rayos X**
> (`--rayos-x` y botón) que dibuja solera, fuste y escalera translúcidos. Poché en la sección (trama
> de rayitas sobre la cara de corte), etiquetas en 7 carriles con línea guía y sin solapes, y
> paleta con más contraste entre hormigón, hierro y PVC.
 (que “se vea como en la realidad”)

| ID | Ticket | Alcance | Aceptación | Días |
|---|---|---|---|---|
| F10-01 | **Tapa y marco legibles** | tapa con espesor, resalte y bisagra; marco con sección | a 40 px se distingue tapa de aro | 1 |
| F10-02 | **Poché en la sección** | rellenar la cara de corte con trama | el corte se lee como sección, no como alambre | 1,5 |
| F10-03 | **Etiquetas con carriles** | 6 carriles, anti-solape, línea guía | 0 solapes > 2 px en una escena de 200 objetos | 2 |
| F10-04 | **Interruptor de rayos X** | ver/no ver lo enterrado | el botón cambia el modo y lo indica | 1 |
| F10-05 | **Materiales separados** | hormigón/hierro/PVC por tono y brillo | se distinguen sin seleccionar | 1 |
| F10-06 | **Escalera dentro del cono** | corregir los 18 cm de intersección | sin intersección en la malla | 0,5 |

**Total F10: 7 días.**

---

## F11 — Producto — **F11-03 (paquete), F11-04 (plantilla) y F11-05 (licencias) hechos**

> `scripts/empaquetar.ps1` construye un zip portable de 10,9 MB con los dos binarios, las demos,
> los documentos, lanzadores `.bat` y `VERSION.txt`; probado ejecutando `redes --version` y
> `proyecto info` desde el paquete. `redes proyecto plantilla` genera un lienzo con terreno y tres
> pozos. `biblioteca importar --licencia --autor` guarda la procedencia y `biblioteca lista` avisa
> de las piezas que no la declaran. **F11-02** (firma de la revisión) sigue pendiente de decisión.

## F11 — Producto y decisiones abiertas

| ID | Ticket | Qué decide | Días |
|---|---|---|---|
| F11-01 | **Semántica de “vertedero” y “pozo”** | si «vertedero» es emisario, alivio, aliviadero o relleno; si «pozo» incluye el profundo de captación | 0,5 |
| F11-02 | **Quién aprueba** | identidad y firma de la revisión (hoy no hay) | 2 |
| F11-03 | **Instalador** | MSI o zip portable + actualización | 2 |
| F11-04 | **Proyecto y biblioteca de ejemplo** | plantilla con piezas de muestra | 1 |
| F11-05 | **Licencias de modelos de terceros** | campo obligatorio en el manifiesto y aviso | 0,5 |

**Total F11: 6 días.**

---

# Revisión para v1 (25/11/2026)

> Revisión independiente en cuatro frentes: arquitectura (6,5/10), dominio (5/10), rendimiento
> (4/10) y calidad/release (7/10). **Nota global para una v1: 5,5/10.** Lista como demo o piloto
> interno; no lista para aprobar redes reales. Lo que falta está en F12–F16. Los **P0** (F12-01…08,
> F13-01, F14-01…03, F15-01, F16-01, F16-02) son bloqueantes para v1.
> Prioridad: **P0** bloquea v1 · **P1** importante para v1 · **P2** después de v1.

## F12 — Corrección de dominio (P0) — **cerrada (25/11/2026)**

Objetivo: un revisor nunca ve una cota, un diámetro o un sentido de flujo que no venga del origen o
que esté mal interpretado.

> **Cambio de enfoque (D5, 25/11/2026):** la ingesta de formatos ajenos es una **skill guiada**, no
> adaptadores. F12-04 y F12-05 pasan a ser referencias de formato en la skill (columnas, unidades,
> offsets, trampas) con los fixtures `.inp` como ejemplos; F12-09 retira la heurística de capas de
> `ingest.rs` y la lleva a la skill; F12-13 valida el CRS que declara el plan. F12-03 queda en Rust:
> `import_plan` no rellena nada que el plan no traiga.
>
> **Cerrada.** Las 15 hechas. Resumen:
> - Clave y solera separadas; el tramo lleva sus soleras (`cota_solera_inicio/fin`) y el tubo se dibuja sobre ellas.
> - Nada se inventa: lo ausente queda en `sin_dato`, se ve como `—` o gris, y no se exporta como dato.
> - La skill de ingesta es guiada (D5): SKILL.md + referencias SWMM, EPANET, DXF Civil 3D, SIG y tablas; sin scripts. La app solo importa plan, su DXF (`REDES_INFO`) y terreno.
> - El plan se valida entero: `--dry-run` real, `E_FMT_CAMPO` con sugerencia, `E_CRS`, DI y secciones, vértices 2D, dispositivos conectados con `desde`/`hasta`, `cota_eje`; `esquema` lista los valores admitidos.
> - Contrapendiente como aviso sin invertir el sentido; aprobar exige cero observados; DXF para el dibujante (capas con color, metros, rótulos, observaciones); pozo con cono excéntrico; juntas por material; longitud 3D.
> - Pruebas: 125 → 170+ en verde.

| ID | P | Ticket | Alcance | Aceptación | Días |
|---|---|---|---|---|---|
| F12-01 · [MAM-1085](https://linear.app/mamutsystems/issue/MAM-1085) | P0 | **Clave ≠ solera** | `families.rs:443` rotula `cota_clave` como «(invert)»; `export.rs:284` escribe `cota_clave` con `z_invert`; `plan_desde_tabla.py` convierte clave en `cota` (solera). Separar `cota_solera` y `cota_clave` en todo el flujo | un Excel con cotas de clave entra con solera = clave − DI; el GeoJSON exporta `cota_solera` | 1,5 |
| F12-02 · [MAM-1086](https://linear.app/mamutsystems/issue/MAM-1086) | P0 | **Cota explícita del tramo** | `Link` con `cota_solera_inicio/fin` declaradas (hoy `model.rs:172` dice «eje o clave», `ingest.rs:233` toma la solera del pozo y `view.rs:710` centra el tubo en ella). Dibujar el tubo con el fondo en la solera; DN ≠ diámetro exterior | prueba: un tramo con solera 100,00 y DI 0,30 dibuja el eje en 100,15 + espesor | 2 |
| F12-03 · [MAM-1087](https://linear.app/mamutsystems/issue/MAM-1087) | P0 | **No inventar datos (D4)** | quitar los valores por defecto silenciosos: terreno = solera / solera = terreno − 1,5 (`ingest.rs:158-162`), tapa = terreno (`:183`), DN 200 (`:254`), nodos DXF con cota 0 (`:586`), SWMM terreno = solera + 1,5 y tanques con volumen 100 en `inp_to_plan.py` | dato ausente → `—` en la vista y entrada en el informe de descartes/avisos; prueba por cada caso | 2 |
| F12-04 · [MAM-1088](https://linear.app/mamutsystems/issue/MAM-1088) | P0 | **Lector SWMM correcto** | `[CONDUITS]` col. 6–7 son InOffset/OutOffset (no diámetros); leer `FLOW_UNITS`, `XSECTIONS.shape`, `OUTFALLS`, `STORAGE`, `PUMPS`, `WEIRS`, `ORIFICES`, `VERTICES`; distinguir pluvial y sanitario | fixture SWMM real (unidades en CFS y CMS) en `tests/fixtures`; saltos en pozo conservados | 3 |
| F12-05 · [MAM-1089](https://linear.app/mamutsystems/issue/MAM-1089) | P0 | **Lector EPANET correcto** | `TANKS` nivel máx. es `row[4]` (hoy `row[5]` = diámetro); la carga del `RESERVOIR` no es cota de terreno; `JUNCTION` no es cámara de válvulas; leer `VALVES`, `PUMPS`, `HEADLOSS`, unidades, `VERTICES`. Corregir docs/03:23-24 (ambos traen planta) | fixture EPANET real (SI y US) con prueba | 2,5 |
| F12-06 · [MAM-1090](https://linear.app/mamutsystems/issue/MAM-1090) | P0 | **Sentido de flujo declarado** | `topology.rs:40-44` invierte el sentido según cotas: usar el sentido declarado y marcar la contrapendiente como discrepancia; no aplicar en redes a presión; `stranded` (`:84-91`) debe aceptar descarga y PTAR como sumideros | una contrapendiente aparece como aviso y no cambia el sentido; red de gravedad sin bombeo no sale como aislada | 1,5 |
| F12-07 · [MAM-1091](https://linear.app/mamutsystems/issue/MAM-1091) | P0 | **Aprobar exige cero observaciones abiertas** | `redes-cli/src/main.rs:594` solo bloquea pendientes; el journal no anota el objeto aprobado | `proyecto aprobar` con un objeto observado → `ok:false`; journal registra código del objeto | 0,5 |
| F12-08 · [MAM-1092](https://linear.app/mamutsystems/issue/MAM-1092) | P0 | **DXF exportado útil para el dibujante** | tabla LAYER con colores; `$INSUNITS` y `$DWGCODEPAGE` (las capas llevan tildes); radio del bloque = DI/2 (hoy dibuja el doble, `export.rs:46,80`); línea del pozo de tapa a solera, hacia abajo (`:79-82`); etiquetas de tramo (DN, material, pendiente, longitud) y de pozo (cotas); notas en capa `OBSERVACIONES` (`:213-240`) | abre en AutoCAD/BricsCAD sin avisos; prueba que lee el DXF con un lector independiente | 3 |
| F12-09 · [MAM-1093](https://linear.app/mamutsystems/issue/MAM-1093) | P1 | **Importar DXF de Civil 3D** | leer `INSERT` (`ingest.rs:348`); `"BARRaje"` nunca coincide en mayúsculas (`:883`); `AP`/`HIDR` no deben crear nodos con familia de tramo; `CAMARA` ≠ pozo sanitario; capas NCS en inglés (`C-WATR-*`, `C-SSWR-*`, `C-STRM-*`) | fixture de Civil 3D con bloques: estructuras como nodos, sin familias de tramo en nodos | 2 |
| F12-10 · [MAM-1094](https://linear.app/mamutsystems/issue/MAM-1094) | P1 | **Familia tubería coherente** | `material` y `diametro_nominal` duplicados (`families.rs:868/991`, `887/1016`); `seccion` como lista de formas; DI/DE/SDR/PN; espesor por DN y material (no 8 mm fijo); juntas por material (PVC 6 m, hormigón 1–2,5 m) | prueba de que ningún atributo se repite en ninguna familia | 1,5 |
| F12-11 · [MAM-1095](https://linear.app/mamutsystems/issue/MAM-1095) | P1 | **Pozo constructivo** | cono excéntrico de 0,6–0,9 m; escalera bajo la boca (hoy a 0,48 m del eje con boca de 0,36 m, `parts.rs:533,573`) | captura `--snap`: la escalera cae dentro de la boca | 1 |
| F12-12 · [MAM-1096](https://linear.app/mamutsystems/issue/MAM-1096) | P1 | **Recubrimiento bien definido** | `demo_city.rs:568` llama recubrimiento a tapa − solera (es profundidad); recubrimiento = rasante − (clave + espesor) | nombre y fórmula corregidos donde aparezca | 0,5 |
| F12-13 · [MAM-1097](https://linear.app/mamutsystems/issue/MAM-1097) | P1 | **Sistema de coordenadas controlado** | EPSG:32717 fijo (`ingest.rs:67`): admitir 17S/18S; comparar `Terrain.epsg` con `Project.crs`; detectar GeoJSON en lon/lat | GeoJSON en grados → error `E_CRS` legible; terreno en otra zona → aviso | 1,5 |
| F12-14 · [MAM-1098](https://linear.app/mamutsystems/issue/MAM-1098) | P2 | **Longitud 3D en cantidades** | `cantidades.rs:92` mide en planta; añadir longitud inclinada | conducción con 20 % de pendiente reporta ambas longitudes | 0,5 |
| F12-15 · [MAM-1140](https://linear.app/mamutsystems/issue/MAM-1140) | P0 | **Contrato del plan sólido** (surge de D5) | `--dry-run` que valida; campos desconocidos = error con sugerencia; unidades coherentes; DI y secciones no circulares; vértices sin Z; dispositivos conectados; `cota_eje`; `esquema` con valores de listas; ida y vuelta DXF de soleras y enlaces | cada punto con prueba; fixtures SWMM/EPANET importan | 2 |

**Total F12: 25 días.**

---

## F13 — Herramientas del revisor (P0/P1) — cerrada

Objetivo: que el ingeniero revisor encuentre los problemas el día 1, sin cálculo hidráulico.

| ID | P | Ticket | Alcance | Aceptación | Días |
|---|---|---|---|---|---|
| F13-01 · [MAM-1099](https://linear.app/mamutsystems/issue/MAM-1099) | P0 | **`redes validar` geométrico** (hecho: `redes validar`, panel Validación; umbrales en D6) | reglas sobre cotas importadas: contrapendiente, pendiente fuera de rango, salto en pozo sin subtipo de caída, recubrimiento mínimo, profundidad máxima, separación agua/alcantarillado (≥ 3 m horizontal, ≥ 0,30 m vertical, agua arriba), DN que baja aguas abajo, tramos huérfanos y nodos aislados, longitud declarada ≠ medida. Umbrales configurables. Panel en la app | cada regla tiene prueba con fixture; el resultado sale en el sobre JSON y como lista navegable en la app | 4 |
| F13-02 · [MAM-1100](https://linear.app/mamutsystems/issue/MAM-1100) | P1 | **Perfil longitudinal** (hecho: `redes perfil` SVG/JSON y botón Perfil en el inspector del tramo) | por colector: rasante, solera, clave y pozos; SVG desde la CLI y vista en la app | perfil de la demo coincide con las cotas del archivo | 4 |
| F13-03 · [MAM-1101](https://linear.app/mamutsystems/issue/MAM-1101) | P1 | **Observaciones con historial** — **hecho** | varias por objeto, fecha, estado abierta/cerrada, ubicación; sustituye a la `nota` única (antes se sobrescribía; sin conversión de archivos anteriores). Sin autor salvo que cambie D2. Hecho: `redes_core::revision`, `objeto observar/observaciones/cerrar-observacion/aprobar --cerrar-todas/reabrir`, historial en el inspector, esquema `redes.project/3` (D7) | observar dos veces conserva ambas; cerrar la última deja pendiente; aprobar con abiertas falla | 3 |
| F13-04 · [MAM-1102](https://linear.app/mamutsystems/issue/MAM-1102) | P1 | **Acta de revisión** — **hecho** | exportar observaciones y estados a CSV/HTML imprimible. Hecho: `redes_core::acta`, `redes acta --formato csv\|html --salida ruta` (CSV con BOM y `;`; HTML autocontenido con `@media print`); autor solo si está en los datos (D2) | abre en Excel y en el navegador | 1,5 |
| F13-05 · [MAM-1103](https://linear.app/mamutsystems/issue/MAM-1103) | P2 | **Comparar entregas (REV-08)** (hecho: `redes proyecto diff` con CSV) | `proyecto diff a.redes b.redes` | altas, bajas y cambios por objeto | 3 |
| F13-06 · [MAM-1104](https://linear.app/mamutsystems/issue/MAM-1104) | P2 | **Reimportar sin perder revisión (ING-10)** — **hecho** | emparejar por código y conservar estados y observaciones. Hecho: `redes_core::reimportar`, `redes importar reimportar --archivo [--formato] [--aceptar-bajas] [--tolerancia-pos]`, informe de altas/bajas/cambiados/reiniciados en el sobre, `--dry-run` en memoria, un `commit` en el journal (D9) | reimportar el mismo plan conserva el 100 % de la revisión | 3 |

**Total F13: 18,5 días.**

---

## F14 — Rendimiento con redes reales (P0/P1) — **cerrada (27/11/2026)**

> **Cerrada.** Los diez tickets hechos (F14-01…F14-10). F14-04 se acepta con ×2,7 a 20k en vez del
> ×3 estricto (decisión del usuario). Siguiente fase: **F15**.

Objetivo: fluidez con 5k–50k tramos y 2k–20k pozos (hoy solo con cientos).

| ID | P | Ticket | Alcance | Aceptación | Días |
|---|---|---|---|---|---|
| F14-01 · [MAM-1105](https://linear.app/mamutsystems/issue/MAM-1105) | P0 | **Índices en `Project`** — **hecho** | Hecho: `redes_core::indice` (por id, por código, nodo → tramos, cuentas por familia; borrados incluidos, se filtra al leer; no se serializa, `reindexar()` al abrir y al reimportar). 50k tramos en release: escena 4,0 s → 49 ms, ingesta 91 s → 485 ms, reimportar 167 s → 912 ms (`docs/21` §4.1). Prueba de coherencia índice = recorrido lineal (`tests/indices.rs`). Antes: `node()`/`link()` recorren todo (`model.rs:296-301`) y se llaman por tramo en `view.rs:666`; también `topology.rs:63`, `add_node/add_link` (`model.rs:343,378`), `next_code`, ajuste de nodos en ingesta (`ingest.rs:574,1056`). `HashMap` por id y código | construir la escena de 50k tramos < 100 ms en release; ingesta de 50k < 2 s | 1,5 |
| F14-02 · [MAM-1106](https://linear.app/mamutsystems/issue/MAM-1106) | P0 | **Cámara sin regenerar malla en 3D** — **hecho** | Hecho: la exageración vertical va en la matriz (`Geometria::fijar_camara`, `cam.matriz(aspecto, z_scale)`) y la malla se guarda en coordenadas locales con cota real; `rebuild()` en `--gpu3d` no llama a `build_opts` ni arma el `Shape`: solo `view::etiquetas` y la matriz; la malla se regenera solo si cambia `ClaveMalla` (proyecto y revisión, capas, biblioteca). Prueba `mover_la_camara_en_3d_no_regenera_la_malla`, mutación `gpu3d-camara-regenera`. Ciudad, release: 6,0 ms → 0,008 ms por cambio de cámara. Antes: `rebuild()` (`main.rs:227-277`) regenera todo por cada evento de ratón; pasar `z_scale` a la matriz (`view.rs:142`), regenerar solo si cambian proyecto o capas; no ejecutar `build_opts` ni construir el `Shape` en modo `--gpu3d` | arrastrar en 3D no llama a `triangulos_exportables` (contador en prueba) | 2 |
| F14-03 · [MAM-1107](https://linear.app/mamutsystems/issue/MAM-1107) | P0 | **Sin copia del búfer por frame** — **hecho** | Hecho: `Geometria::bytes` es `Arc<[u8]>` y el compositor toma con `Geometria::tomar(generación subida)` la matriz y, solo si hay generación nueva, el `Arc` (sin copiar bytes). Prueba `la_escena_quieta_no_se_vuelve_a_subir`, mutación `gpu3d-sube-cada-frame`. Ciudad: 3,6 MB copiados por frame (0,46 ms con el mutex) → 0. Antes: `gpu3d.rs:352` clona los bytes en cada frame con el mutex tomado; `Arc<[u8]>` o copiar solo si cambia la generación | 0 copias por frame con la escena quieta | 0,5 |
| F14-04 · [MAM-1108](https://linear.app/mamutsystems/issue/MAM-1108) | P1 | **Cámara precalculada y sin `Vec` por triángulo** — **hecho** | Hecho: `Camera::proyector()` (base, ojo y focal una vez por escena; proyección y profundidad juntas), `expand` y el corte (`Trozos`) en la pila, un búfer de eje para todos los tramos, tramos y nodos repartidos en bloques entre hilos (`en_bloques`, desde 5 000 objetos, orden conservado) y el pintor ordena claves enteras (`sort_far_first`, estable como antes). Pruebas `el_orden_del_pintor_es_el_estable_de_antes`, `los_bloques_conservan_el_orden`; mutaciones `orden-negativos`, `bloques-desordenados`. Banco release (mismo perfil F14-09): escena 20k encuadrada 18,8 → 7,0 ms (×2,7; ×4,3 frente a antes de F14-09), 50k 49,9 → 13,4 ms (×3,7); `tests/escena.rs`, 20k de cerca 444 → 5,4 ms. Antes: `basis()`/`eye()` por punto (`view.rs:170-206`); `pts`/`expand()` y `vec![tri]` por triángulo (`view.rs:733-961`) | construcción de escena ≥ 3× más rápida en el banco. **El usuario lo acepta como hecho** (27/11/2026) con ×2,7 a 20k (×3,7 a 50k, ×4,3 contando el perfil) en lugar del ×3 estricto | 1 |
| F14-05 · [MAM-1109](https://linear.app/mamutsystems/issue/MAM-1109) | P1 | **LOD, recorte e instanciado en 3D** — **hecho** | Hecho (27/11/2026): `redes_core::scene3d`. Las piezas se parten por primitivas (`Mesh::part_breaks`) en prototipos indexados y compartidos (forma canónica: esquina en el origen y alto 1). Cada objeto tiene tres LOD (completo, simple y caja) que se eligen por radio en pantalla, y la geometría fija va en lotes de 16×16. El recorte es por tronco de visión con cajas, y el plan por cámara (`scene3d::plan`) agrupa las instancias por prototipo. `gpu3d.rs` usa búferes de vértices, índices e instancias con su shader. El tope de la escena es `min(512 MB, max_buffer_size)` y avisa en la barra de estado. Con 20k tramos: 1 110 MB → **22,9 MB**, primer frame 2,75 s → 135 ms, RAM 2,9 GB → 319 MB (`docs/21` §4.4). Pruebas `manholes_of_different_depth_share_prototypes`, `instancing_reproduces_the_exported_mesh`, `distant_objects_draw_as_boxes_and_near_ones_in_full`, `objects_outside_the_view_are_culled`, `a_scene_over_budget_is_trimmed_and_warns` y `the_budget_respects_the_device_limit`. El banco tiene una fase «malla 3D» con tope de bytes por objeto. Mutaciones `escena3d-*` y `gpu3d-tope-fijo`. El `.glb` comparte la malla de las piezas iguales y se escribe sin copias intermedias: 20k tramos pasan de 1,1 GB y 26 s a 26 MB y 3,6 s; pasar de 4 GB es `E_IO_TOO_LARGE` (mutaciones `glb-*`) | 20k pozos < 512 MB; superar el tope avisa en la interfaz | 4 |
| F14-06 · [MAM-1110](https://linear.app/mamutsystems/issue/MAM-1110) | P1 | **Caché de mallas paramétricas y proxies** — **hecho** | Hecho: `redes_core::mallas::CachePiezas` con clave familia, subtipo, atributos (DN incluido), bits de la altura tapa − solera, falta de altura y LOD; vive en el estado de la app y poda lo no usado en cada pasada; `gltf::triangulos_con_cache`. La `Biblioteca` va en `Arc` y el proxy se calcula una vez por pieza (`modelos::Cache::proxy`). Pruebas `la_segunda_reconstruccion_no_genera_piezas`, `el_proxy_se_calcula_una_vez_por_pieza`; mutaciones `cache-piezas-sin-acierto`, `proxy-por-instancia`. 20k pozos, release: segunda malla de la escena 1 163 ms → 483 ms (0 piezas generadas). Antes: cachear por (familia, tipo, DN, LOD); no clonar la `Biblioteca` (`main.rs:386`) ni recalcular el proxy por instancia (`:410`) | segunda reconstrucción sin cambios no genera mallas | 1,5 |
| F14-07 · [MAM-1111](https://linear.app/mamutsystems/issue/MAM-1111) | P1 | **Rejilla espacial para red y picking** — **hecho** | Hecho: recorte por pantalla en `build_opts` (`Pantalla`: por caja de la primitiva proyectada, o conservador por esfera de pieza —la de un pozo sin teselarlo— y cápsula de tramo); `Scene::descartadas` cuenta lo que no entra al ordenamiento. Selección con `redes_core::seleccion::RejillaSeleccion` (celdas de 32 pt, se arma una vez por cámara; la app la invalida en `rebuild`), mismo resultado que el recorrido lineal. Pruebas `el_recorte_solo_quita_lo_que_no_se_ve`, `la_esfera_previa_del_pozo_envuelve_su_malla`, `la_rejilla_elige_lo_mismo_que_el_recorrido_lineal`, `de_cerca_lo_que_no_se_ve_no_se_ordena`, `la_rejilla_elige_como_el_recorrido_lineal_mirando_poco` (20k: un clic mira < 10 % encuadrada y < 50 objetos de cerca); mutaciones `recorte-de-mas`, `recorte-apagado`, `esfera-pozo-corta`, `rejilla-sin-reparto`, `rejilla-pierde-vecinos`. Antes: recorte por pantalla antes de ordenar; ajuste de nodos y picking con rejilla | picking O(1) amortizado; objetos fuera de pantalla no se ordenan | 1 |
| F14-08 · [MAM-1112](https://linear.app/mamutsystems/issue/MAM-1112) | P2 | **Deshacer por diferencias** — **hecho** | Hecho: `redes_core::undo::History` (20 pasos) guarda por paso los nodos, tramos y entradas del journal que cambiaron, antes y después, más `revision`, `estado` y `next_slot`; rehace los índices si cambian altas, códigos o extremos. La app registra con `State::commit_edit` (observar, cerrar, aprobar, modelo con sus anclas), deshace en su sitio (`Arc::make_mut`; la revisión cambia, así que `ClaveMalla` regenera) y gana **Rehacer**. Red sintética de 20k tramos, 20 pasos: ~270 MB (20 copias, JSON) → **14,9 kB** (`docs/21` §4.5). Pruebas `tests/undo.rs` (6: deshacer/rehacer exactos por tipo de operación, anclas, índices tras un alta, pila acotada, memoria 20k) y `undo_and_redo_buttons_restore_each_review_step`; mutaciones `deshacer-copia-todo`, `deshacer-contadores`, `deshacer-sin-reindexar`, `deshacer-sin-registrar`. Antes: `recordar()` clona el `Project` entero 20 veces (`main.rs:281-285`) | memoria del deshacer proporcional al cambio | 2 |
| F14-09 · [MAM-1113](https://linear.app/mamutsystems/issue/MAM-1113) | P1 | **Perfil de compilación** — **hecho** | Hecho en el `Cargo.toml` raíz. Banco 20k tramos, release, sin → con perfil: ingesta 392 → 184 ms, escena 30 → 19 ms, abrir 217 → 121 ms, reimportar 697 → 335 ms (50k: escena 78 → 50 ms). Depuración con dependencias en opt-level 2: escena 20k 93 ms. Antes: `[profile.release] lto = "thin", codegen-units = 1`; `[profile.dev.package."*"] opt-level = 2` | binario release más rápido en el banco; dev usable | 0,25 |
| F14-10 · [MAM-1114](https://linear.app/mamutsystems/issue/MAM-1114) | P1 | **Banco con red sintética grande** — **hecho** | Hecho: `crates/redes-core/tests/banco.rs` con la red en peine de `tests/comun` (compartida con `tests/indices.rs`, sin azar: reproducible). 20k tramos: umbral absoluto por fase (~5× lo medido en local; release: ingesta 1 s, escena 100 ms, abrir 600 ms, reimportar 2 s; depuración: 5 s / 600 ms / 6 s / 8 s) y crecimiento de la escena de 5k a 20k < ×8 (lineal ≈ ×4,5, cuadrático ≈ ×15, independiente de la máquina). Paso «Banco» del CI en release; 50k `#[ignore]`, a mano. Mutación `banco-cuadratico` (`node(id)` sin índice: escena 20 → 260 ms en release). Antes: red de 20k/50k tramos generada; umbral en CI (amplía F9-03) | CI falla si la construcción de escena pasa del umbral | 1,5 |

**Total F14: 15,25 días.**

Fuera de fase (27/11/2026): se retiró el panel «Parámetros (en vivo)» del inspector (D12, la app no
edita el diseño). Cualquier ítem que pida editar parámetros desde la app queda descartado.

---

## Orden de fases vigente (27/11/2026)

Un solo frente abierto a la vez; no se abre una fase sin cerrar la anterior.

1. ~~F14 — Rendimiento~~ (cerrada).
2. **F17 — Familias paramétricas en Blender** (D14). Va **antes** de F15: los IDs de F15/F16 se conservan porque ya están en Linear.
3. **F20 — Familias de referencia** (D20): cada familia contrastada con la bibliografía técnica y con lo que se construye; ficha de cuándo se usa y cuándo no, y recetas mejoradas.
4. **F21 — Topografía**: el terreno se ve y se usa como el real (sombreado, curvas, ortofoto, densidad, nubes grandes, obras visibles), sin inventar cota (D21).
5. **F15 — Arquitectura y mantenibilidad**, ampliada con la fachada única y el canal de comandos (ver F15-13…F15-15).
6. **F18 — Revisión completa en la app y salidas que faltan** (huecos del inventario docs/02 §4), sobre la fachada de F15.
7. **F19 — Entregables: escena de Blender, IFC para BIM (Bonsai) y planos de planta y perfil en DXF** (D19).
8. **F16 — Release v1.**

Frentes transversales, revisados al cerrar cada ticket:

| Frente | Qué se exige |
|---|---|
| Ordenar el código | sin código muerto; código que se explica solo y en inglés (D13) |
| Rendimiento | el banco del CI no empeora; cifras nuevas en docs/21 |
| Codegraph y Archify | `codegraph sync .` tras cada commit; diagrama de arquitectura y análisis de acoplamiento rehechos al cerrar cada fase (`out/auditoria/`) |
| Guía de agentes | `CLAUDE.md` y `AGENTS.md` al día con cada cambio de reglas o comandos |
| Flujos deterministas | cada flujo (ingesta, revisión, exportación, cierre de ticket, integración de subagentes) son pasos fijos y verificables (comandos, scripts, CI), no criterio del agente |
| Inventario de features | docs/02 y este backlog dicen qué está listo, parcial o planeado; todo hueco de docs/02 §4 tiene ticket (F17-11…13, F18) o queda «fuera de v1» |

---

## F17 — Familias paramétricas en Blender (D14) — **en curso**

Blender es la **única fuente visual** de las piezas; la geometría de `parts.rs`/`parts_cadena.rs`
queda como respaldo cuando falta modelo. Los datos (DN, cotas, estado) siguen saliendo del modelo de
datos (docs/17 §7). Cada familia es **paramétrica en todo** lo que la define (diámetros, alturas,
espesores, material, longitudes, ángulos, juntas, tapas), no un catálogo de variantes fijas. Las
modelan agentes con Blender 5.1 sin ventana; el usuario aprueba cada pieza por capturas.

| ID | P | Ticket | Alcance | Aceptación | Días |
|---|---|---|---|---|---|
| F17-01 | P0 | ~~**Evaluar MCP de Blender frente a `bpy` sin ventana**~~ **hecho** (D15): `bpy` sin ventana con scripts en `tools/blender/`; prototipo de pozo a ~2,0 s por pieza y `.glb` idénticos byte a byte (docs/17 §10) | fiabilidad con Geometry Nodes, uso sin intervención por subagentes, reproducible en CI | informe con recomendación en docs/17 y decisión en docs/22 | 0,5 |
| F17-02 | P0 | ~~**Cómo llegan los parámetros a la geometría**~~ **hecho** (D16): `redes biblioteca hornear`, en lote y con caché `biblioteca/horneado/<clave>.glb`; 100 pozos en 7,2 s, en caché 86 ms sin Blender; un dato que falta deja la pieza de respaldo (docs/17 §11) | hornear bajo demanda (CLI → Blender → caché `.glb` por parámetros) o exportar la receta de la familia y reconstruirla en el núcleo; la app no depende de Blender en ejecución salvo decisión explícita | decisión D# con prototipo medido | 1,5 |
| F17-03 | P0 | ~~**Pieza piloto: pozo de inspección paramétrico**~~ **hecho, a la espera de que el usuario apruebe las capturas** (docs/17 §12): receta con cuerpo, cono excéntrico o concéntrico, escalones, marco, tapa, losa y banqueta; anclas por tubo desde las soleras de los tramos (`sin_ancla` si faltan); el modelo se apoya en solera − losa y sustituye a la paramétrica con y sin `--gpu3d`; `extras` por nodo; piloto versionado con prueba de contrato sin Blender | cuerpo, cono, escalones, tapa, base y espesores como entradas; anclas por entrada/salida; manifiesto; partición dato/forma aprobada por el usuario; colocar el modelo en la solera y en lugar de la paramétrica; huecos de `extras` de docs/17 §11.6 | capturas iso, corte y foco aprobadas por el usuario; prueba de contrato del `.glb` | 2 |
| F17-04 | P1 | **Tubería con campana y espiga, y juntas** | DN, espesor, material y longitud | ídem | 1,5 |
| F17-05 | P1 | **Accesorios**: codos, tees, reducciones, válvulas, hidrantes | ángulos, DN de cada boca, material | ídem | 3 |
| F17-06 | P1 | **Captación**: bocatoma, desarenador, conducción | dimensiones de obra como entradas | ídem | 3 |
| F17-07 | P1 | **Planta de tratamiento**: unidades de la cadena (docs/20) | por unidad, dimensiones y espesores de muro | ídem | 4 |
| F17-08 | P0 | **Demo de la cadena completa solo con piezas de Blender** | `redes proyecto cadena` dibujado con la biblioteca; recorrido de capturas (iso, cortes verticales, perfil, foco en la planta) | el usuario aprueba el recorrido; banco sin regresión | 2 |
| F17-09 | P0 | **Ejemplo con topografía real** (D17) | recorte del MDT Copernicus GLO-30 (zona andina, con atribución) como terreno de `redes proyecto cadena`: quebrada encajada, lomas, vaguada y cruce de río; trazado coherente con el relieve de la captación a la planta y a la distribución; todas las piezas horneadas | perfiles y cortes verticales legibles; el usuario aprueba el recorrido | 2 |
| F17-10 | P1 | **Biblioteca horneada en el paquete** (D17) | juego de medidas comerciales por familia horneado con las recetas, manifiesto y recetas dentro de `scripts/empaquetar.ps1`; el CI comprueba que cada `.glb` versionado corresponde a la clave de su receta | el paquete portable abre la demo con piezas de Blender sin Blender instalado | 1 |
| F17-11 | P0 | **Topografía de levantamiento** (D18), antes de F17-09 | contrato `redes.topografia/1` (puntos con número, E, N, Z, descripción y clase; líneas de quiebre; contorno y huecos; malla opcional) validado por `redes importar topografia`; TIN con restricciones (Delaunay restringido) que respeta quiebres y contorno; referencia de topografía en `skills/redes-ingesta` para PNEZD, DXF y LandXML de Civil 3D con tabla de códigos típica ajustable; fixtures de ejemplo | un levantamiento con quebrada no cruza el cauce con triángulos; el agente traduce un PNEZD real siguiendo la skill y la CLI lo valida | 3 |
| F17-12 | P0 | **CRS y datum vertical** (antes de F17-09) | UTM norte y sur (`326NN`/`327NN`); el datum vertical se declara y se guarda con el terreno (ortométrico EGM2008/EGM96 o elipsoidal) y se muestra; mezclar datums falla con error legible; corregir `skills/redes-ingesta/references/plan.md` («siempre 32717») | terreno Copernicus etiquetado EGM2008; plan y terreno con datums distintos → error | 1 |
| F17-13 | P1 | ~~**Calidad del terreno**~~ → F21-04 | informe al importar: puntos duplicados (misma E,N con distinta Z), densidad, triángulos largos y espigas; el aligerado de la vista conserva picos y quiebres (hoy `thinned` guarda el primer punto de la celda) | fixture con duplicados y espiga → avisos con coordenadas; la vista aligerada no pierde la cima | 1,5 |

**Total F17: 26 días.**

---

## F20 — Familias de referencia: lo que recomiendan los expertos y lo que se construye (D20) (P1)

Tras F17 (las familias ya existen y se hornean), cada familia se contrasta con la bibliografía técnica y
con lo que de verdad se construye, y se mejora su receta y su ficha. La ficha sirve a la vez al usuario
(qué es, cuándo se usa y cuándo no), al agente de ingesta (para elegir familia y subtipo al traducir una
entrega, D5) y a quien modela en Blender (proporciones y detalles reales). Es **orientación documentada,
no verificación**: la app sigue sin comprobar normativa ni calcular (D4); lo que la ficha diga nunca se
convierte en una regla que corrija el diseño.

Ficha por familia y subtipo (`docs/familias/fichas/<familia>.md`): qué es; cuándo se usa y cuándo no
(situaciones típicas, alternativas); partes y materiales reales; rangos habituales de dimensiones como
referencia, nunca como valor por defecto; cómo se construye (procedimiento, juntas, cimentación);
datos que debe traer el diseño; **tabla de parámetros** —todas las medidas son paramétricas, y la tabla
dice de cada una si es dato del diseño (viene del plan), si se adapta al sitio (la cambia quien diseña
según terreno, caudal o acceso) o si es forma de catálogo que normalmente no se toca (y qué pasa si se
cambia)—; fuentes citadas (normas y guías de la región —p. ej. INEN/CPE INEN en
Ecuador, RAS en Colombia, OS.070/OS.100 en Perú, CONAGUA-MAPAS en México, guías OPS/CEPIS— y manuales de
fabricantes); y qué cambia en la receta de Blender.

| ID | P | Ticket | Alcance | Aceptación | Días |
|---|---|---|---|---|---|
| F20-01 | P1 | **Método y plantilla de la ficha** | plantilla, criterios para elegir y citar fuentes, cómo se enlaza la ficha con el registro de familias (`families*.rs`), la skill de ingesta y la receta | plantilla aprobada por el usuario con una ficha de ejemplo (pozo de inspección) | 1 |
| F20-02 | P1 | **Alcantarillado** | pozos de inspección y de caída, cámaras, sumideros, conexiones domiciliarias, descargas | fichas aprobadas; recetas mejoradas con capturas aprobadas | 3 |
| F20-03 | P1 | **Tuberías y juntas por material** | PVC, PEAD, hormigón, hierro dúctil, acero, PRFV: campana y espiga, unión mecánica, termofusión, bridas; longitudes comerciales | ídem | 2 |
| F20-04 | P1 | **Accesorios y válvulas** | codos, tees, reducciones, cruces, tapones; válvulas de compuerta, mariposa, retención, aire, purga, reductoras; hidrantes; cajas de válvulas | ídem | 3 |
| F20-05 | P1 | **Captación y conducción** | todos los subtipos de bocatoma, desarenador y cámaras (rompe presión, carga, reunión), pasos elevados y anclajes | ídem | 3 |
| F20-06 | P1 | **Planta de potabilización (PTAP)** | unidades de la cadena (docs/20) con lo que se construye de verdad en plantas pequeñas y medianas | ídem | 3 |
| F20-07 | P1 | **Planta de aguas residuales (PTAR)** | rejas, desarenadores, tanques sépticos e Imhoff, UASB, lagunas, filtros, lechos de secado | ídem | 3 |
| F20-08 | P2 | **Reservorios y bombeo** | tanques apoyados y elevados, estaciones de bombeo, cámaras de válvulas | ídem | 2 |
| F20-09 | P1 | **Fichas en la skill y en la app** | la skill de ingesta usa las fichas para proponer familia y subtipo (y preguntar si duda); el inspector enlaza la ficha de cada objeto | el agente elige bien el subtipo en los fixtures; botón «Ficha» en el inspector | 1,5 |

La misma tabla de parámetros se ve en el `.blend` de revisión (propiedades agrupadas en «dato», «se
adapta al sitio» y «forma») y en el inspector, para que cualquiera sepa qué puede cambiar y qué no.

**Total F20: 21,5 días.** Cada ficha la aprueba el usuario antes de cambiar la receta.

---

## F21 — Topografía: que el terreno se vea y se use como el real (D21) (P1)

F17-11/12 dejaron el contrato y el TIN restringido. Esta fase hace que el terreno **se vea** como el
sitio (hoy sale facetado y plano de color) y que aguante levantamientos pobres y nubes enormes. La
calidad del resultado depende de cuántos puntos traiga el levantamiento: la app lo muestra y lo dice,
pero **no inventa cota** (D4): todo suavizado es solo de presentación y `z_at`, el perfil y `validar`
siguen leyendo el TIN tal cual.

| ID | P | Ticket | Alcance | Aceptación | Días |
|---|---|---|---|---|---|
| F21-01 | P1 | **Sombreado del terreno** | normales por vértice respetando las líneas de quiebre (arista viva en cunetas y taludes), luz rasante tipo *hillshade*, rampa hipsométrica configurable; en la vista CPU y en `--gpu3d` | capturas de la quebrada y de la cadena aprobadas por el usuario | 2 |
| F21-02 | P1 | **Curvas de nivel en la vista** | maestras e intermedias con intervalo elegible y etiquetas de cota; respetan quiebres y contorno (antes parte de F18-08) | captura en planta con curvas legibles | 1,5 |
| F21-03 | P1 | **Ortofoto sobre el terreno** | imagen georreferenciada (GeoTIFF RGB o JPG + world file) proyectada sobre el TIN; la traduce el agente con la skill si viene en otro formato | captura con ortofoto alineada a la red | 2 |
| F21-04 | P1 | **Calidad y densidad del levantamiento** (absorbe F17-13) | informe al importar: duplicados, triángulos largos, espigas, densidad por zona; en la vista, capa que marca las zonas con pocos puntos (triángulos más largos que un umbral del usuario) sin rellenarlas | fixture escaso → zonas marcadas y aviso con coordenadas | 2 |
| F21-05 | P2 | **Suavizado solo de presentación** | subdivisión de la malla mostrada para levantamientos escasos, que no toca `z_at` ni las salidas; se activa y se rotula como «vista suavizada» | prueba: `z_at` y el perfil no cambian con el suavizado activo | 1,5 |
| F21-06 | P2 | **Nubes y rejillas grandes** | LAS/LAZ (clase 2, suelo) y rejillas de millones de celdas con niveles de detalle por teselas; aligerado que conserva quiebres y picos | 5 M puntos abren en tiempo aceptable (cifra en docs/21) | 3 |
| F21-07 | P2 | **Varias superficies** (antes F18-06) | terreno existente y proyectado; añadir puntos sin reemplazar; elegir cuál usan la vista, el perfil y `validar` | proyecto con dos superficies; perfil con ambas líneas | 3 |
| F21-08 | P2 | **Curvas de nivel como entrada** | la skill convierte polilíneas con cota (DXF de Civil 3D) en puntos y quiebres de `redes.topografia/1`, preguntando el intervalo y lo dudoso | fixture DXF de curvas → TIN sin terrazas falsas | 1,5 |
| F21-09 | P1 | **Terreno alrededor de las obras** | recorte visual del terreno en zanjas, pozos y estructuras enterradas (excavación de presentación o terreno translúcido local), para que la bocatoma, las cámaras y la planta no queden tapadas por la ladera | capturas de la captación y de la planta con las obras visibles | 2 |

**Total F21: 18,5 días.** F17-13 y F18-06 pasan a F21-04 y F21-07; las curvas de F18-08 a F21-02.

---

## F15 — Arquitectura y mantenibilidad (P1/P2)

| ID | P | Ticket | Alcance | Aceptación | Días |
|---|---|---|---|---|---|
| F15-01 · [MAM-1115](https://linear.app/mamutsystems/issue/MAM-1115) | P0 | **Solo la versión vigente del `.redes` (sin migraciones hasta v1)** — **hecho** | replanteado el 25/11/2026 (D10): no hay nada en producción, no se mantiene compatibilidad. `project_io::load` mira `schema` antes que los campos y abre **solo** `redes.project/3`; otra versión (anterior, futura, ajena o ausente) → `E_PROJ_SCHEMA` (código 4; se funde `E_PROJ_SCHEMA_TOO_NEW`) con la versión que trae, la que entiende y la pista de regenerar con `redes importar`. Se retira la corrección `cota_clave`→`cota_solera` al abrir. `redes.plan/*` solo `/1`; otra versión → `E_FMT_VERSION` (código 7, en `redes esquema`). Los `.redes` de `demo/` y `docs/demo/` regenerados con `redes proyecto cadena\|catalogo\|ciudad\|plantilla`. Mutación `proyecto-acepta-version-anterior` en `scripts/mutar.ps1`. Migraciones (`migrar_vN→vN+1`) al congelar v1 | un `.redes` `/2` da `E_PROJ_SCHEMA` y no abre; un plan `/2` da `E_FMT_VERSION` (contrato con el binario real); los `.redes` de `demo/` abren | 1 |
| F15-02 · [MAM-1116](https://linear.app/mamutsystems/issue/MAM-1116) | P1 | **Encapsular el agregado `Project`** | `nodes`/`links` y campos de `Node` son `pub` (`model.rs:249-250`); ~80 filtros manuales de `!deleted`. Campos privados, iteradores `vivos()`, alta solo por `add_*` | ningún `.deleted` fuera de `model.rs` | 3 |
| F15-03 · [MAM-1117](https://linear.app/mamutsystems/issue/MAM-1117) | P1 | **Catálogo de familias como datos** | `families.rs` (8.553 líneas, `build()` ≈ 7.950) + `families_extra.rs` que lo parchea: pasar a JSON/TOML con `include_str!` y validación en prueba | el catálogo se carga y valida; diffs legibles | 4 |
| F15-04 · [MAM-1118](https://linear.app/mamutsystems/issue/MAM-1118) | P1 | **Sin `match` por cadena de familia** | claves de familia literales en `view.rs` (36), `ingest.rs` (20), `parts_cadena.rs` (17), `biblioteca.rs` (11), app (6); `view.rs:1209/1280` repiten pozo → entero mágico. `enum TipoPozo` y tabla familia → constructor | añadir una familia geométrica toca un solo sitio | 3 |
| F15-05 · [MAM-1119](https://linear.app/mamutsystems/issue/MAM-1119) | P1 | **Partir funciones gigantes** | `view::build_opts` (671 líneas, `view.rs:357`), `view()` de la app (656, `main.rs:711`), `run()` CLI (357, `redes-cli/src/main.rs:329`, comandos en módulos), `import_dxf` (242), `pozo_partes` (174) | ninguna función > 150 líneas | 3 |
| F15-06 · [MAM-1120](https://linear.app/mamutsystems/issue/MAM-1120) | P1 | **Errores tipados** | 27 `Result<_, String>` con códigos en el texto; `E_IO_NOT_FOUND` para cualquier lectura. `enum RedesError` con `code()` e `impl Error` | la CLI mapea por `match`, no parseando texto | 2 |
| F15-07 · [MAM-1121](https://linear.app/mamutsystems/issue/MAM-1121) | P2 | **Demos detrás de `feature = "demos"`** | `demo*` (≈ 2.550 líneas y la mayoría de `unwrap`) en la API pública de core | core compila sin demos | 1 |
| F15-08 · [MAM-1122](https://linear.app/mamutsystems/issue/MAM-1122) | P2 | **Core sin entorno ni E/S directa** | `model.rs:511-518` lee `REDES_JOURNAL_USUARIO`/`USERNAME`; adaptadores reciben `Read`/`&[u8]` | identidad inyectada; pruebas sin disco | 1,5 |
| F15-09 · [MAM-1123](https://linear.app/mamutsystems/issue/MAM-1123) | P1 | **Guardar fuera del hilo de UI** | `State::save` (`redes-app/src/main.rs:216`) es síncrono, contra la regla de `project_io.rs:20` | guardar un proyecto grande no congela la ventana | 1 |
| F15-10 · [MAM-1124](https://linear.app/mamutsystems/issue/MAM-1124) | P1 | **Quitar `panic` en producción** | `geom.lock().unwrap()` (`gpu3d.rs:351`), `ingest.rs:228`, ~38 `unwrap()` en core fuera de pruebas | 0 `unwrap` fuera de pruebas y demos (clippy `unwrap_used` en core) | 1 |
| F15-11 · [MAM-1125](https://linear.app/mamutsystems/issue/MAM-1125) | P1 | **Documentos desactualizados** | `docs/01` §4.2 «17 familias» (son 46); `redes-core/Cargo.toml` dice «hidráulica y movimiento de tierras» (contra D4); `docs/03` dice que SWMM/EPANET no traen planta; `docs/18` y `docs/23` desfasados sobre F6-05; decidir si el peso por densidad (`cantidades.rs:24`) cabe en D4 | docs coherentes entre sí y con el código | 0,5 |
| F15-12 · [MAM-1126](https://linear.app/mamutsystems/issue/MAM-1126) | P2 | **Aislar Rune** | capa fina `ui_port`; `gpu3d.rs` acoplado a internos; registrar el commit de origen en `docs/00` | actualizar Rune toca un solo módulo | 2 |

| F15-13 | P1 | **Fachada única `redes_core::api`** (auditoría del 27/11/2026) | hoy CLI y app entran a 21 módulos por ~80 rutas y el `prelude` no se usa. Reexportar desde `api`, migrar CLI y app, cerrar el resto a `pub(crate)` | prueba en CI que falla si algo fuera de core importa `redes_core::` sin pasar por `api` | 2 |
| F15-14 | P1 | **Un solo canal de cambios: `Comando` → `aplicar` → `Evento`** | toda mutación del proyecto pasa por `api::aplicar(&mut Project, Command)`; las lecturas por `api::consultar`; el journal guarda el comando completo; la CLI deriva el Envelope del resultado | `Project` no se muta fuera de `servicios`; deshacer y journal salen del comando | 4 |
| F15-15 | P1 | **Capas sin ciclos** | ciclos `model` ↔ `revision`/`validar`/`project_io`/`indice` y `families` ↔ `families_extra`: sacar `SCHEMA_VERSION`, `ReglasValidacion` y `Observacion` a una capa `base` | Codegraph y Archify sin ciclos entre capas | 2 |

**Total F15: 32 días.**

---

## F18 — Revisión completa en la app y salidas que faltan (P1/P2)

Huecos del inventario verificado (docs/02 §4). Va tras F15 para construirse sobre la fachada `api`, salvo
F18-07 y F18-12, que pueden adelantarse si bloquean una revisión real.

| ID | P | Ticket | Alcance | Aceptación | Días |
|---|---|---|---|---|---|
| F18-01 | P1 | **Buscar y filtrar en la app** | buscar por código (encuadra y selecciona); filtrar por estado de revisión y por familia; chip de la capa `nodos` | captura con filtro «observados» y búsqueda por código | 2 |
| F18-02 | P1 | **Acciones de revisión desde la app** | botones de acta, reimportar, exportar (DXF, GeoJSON, glTF, CSV), reabrir y aprobar el proyecto; mismos códigos `E_*` y journal que la CLI | cada botón produce el mismo resultado que su comando | 3 |
| F18-03 | P1 | **Inspector completo** | todos los atributos del nodo (hoy 10) y los del tramo, con fuente; longitud 3D del tramo (hoy en planta) | árbol `--tree` con todos los atributos de un pozo y de un tramo | 1 |
| F18-04 | P1 | **Corte orientable y exageración vertical** | corte a lo largo de un tramo o con orientación elegida (hoy fijo en E local); control de exageración vertical en la UI | capturas de corte por un colector y con ×1/×5 | 2 |
| F18-05 | P2 | **Teclado y capturas con selección** | flechas para girar (o retirar el aviso «Flechas para girar»); `--snap` con un objeto seleccionado para capturar el inspector | prueba de la escena con selección; aviso coherente con lo que hace la app | 1 |
| F18-06 | P2 | ~~**Varias superficies**~~ → F21-07 | terreno existente y proyectado; añadir puntos a una superficie sin reemplazarla; elegir cuál usan la vista, el perfil y `validar` | proyecto con dos superficies; perfil con ambas líneas | 3 |
| F18-07 | P1 | **Reglas de `validar` contra el terreno** | tapa o rasante declaradas frente al TIN (diferencia sobre umbral del usuario, D6) y objetos fuera del terreno; se informa, no se corrige | fixture con tapa 0,5 m bajo el TIN → hallazgo; objeto fuera → hallazgo | 1 |
| F18-08 | P1 | **Salidas que faltan** | `--solo-aprobados` en GeoJSON; DXF por red (agua o alcantarillado); curvas de nivel en la vista y en el DXF; terreno en DXF y GeoJSON | pruebas de contrato por cada salida | 2 |
| F18-09 | P1 | **DXF abierto en AutoCAD/BricsCAD** | abrir un DXF exportado en un CAD real y dejar constancia (capturas, versión) | constancia en docs/16 o docs/04; lo que falle, ticket | 0,5 |
| F18-10 | P2 | **Reimportar conserva las anclas de biblioteca** | `importar reimportar` reaplica las anclas de `biblioteca aplicar` a los objetos que siguen | prueba: reimportar no pierde el modelo anclado | 1 |
| F18-11 | P2 | **Ayuda de la CLI coherente** | `proyecto aprobar --help` dice que bloquea pendientes **y observados**; repasar el resto de ayudas contra el código | prueba que compara la ayuda con los códigos de `esquema` | 0,5 |
| F18-12 · [MAM-1155](https://linear.app/mamutsystems/issue/MAM-1155) | P0 | ~~**No inventar el diámetro del pozo**~~ **hecho en F17-03**: `Node::inner_diameter_m` (dato o nada) y `Node::drawing_diameter_m` con la constante `FALLBACK_DRAWING_DIAMETER_M` solo para dibujar; el inspector muestra `—` | pozo sin `diametro_interior` → `—` | 0,5 |

Fuera de v1, sin ticket: nubes LAS/LAZ, Revit nativo (el IFC de F19-02 lo cubre), servidor MCP, modo lote, autoguardado, atributos
definidos por el usuario. LandXML y curvas de nivel de Civil 3D entran por la skill de topografía (F17-11).

**Total F18: 17,5 días.**

---

## F19 — Entregables: escena de Blender, IFC para BIM y planos de planta y perfil (D19) (P1)

Un proyecto revisado sale listo para seguir trabajando en otras herramientas, sin rehacer nada: una
escena de Blender para visualización, un IFC para metodología BIM en Bonsai (antes BlenderBIM) y
dibujos de planta y perfil en DXF para que en AutoCAD se armen los planos. Todo sale de los datos
declarados y de las piezas horneadas (D14–D16); nada se calcula ni se corrige al exportar.

| ID | P | Ticket | Alcance | Aceptación | Días |
|---|---|---|---|---|---|
| F19-01 | P1 | **Escena de Blender del proyecto** | `redes exportar blender --salida x.blend`: Blender sin ventana arma el terreno, las piezas horneadas colocadas (respaldo si falta), materiales por material declarado, colecciones por red, familia y estado de revisión, cámaras iso/planta/foco y luz; propiedades `redes_*` con los datos y su fuente en cada objeto; coordenadas locales con el origen guardado (CLAUDE.md, f64) | abrir el `.blend` en Blender 5.1 y renderizar la cadena sin tocar nada; captura aprobada por el usuario | 2,5 |
| F19-02 | P1 | **Exportar IFC 4.3 para Bonsai** | `redes exportar ifc --salida x.ifc` con IfcOpenShell (en Blender sin ventana o Python aparte, como D15): `IfcProject` georreferenciado (`IfcMapConversion` + `IfcProjectedCRS` con el EPSG y el datum vertical, F17-12); tubos `IfcPipeSegment`, pozos y cámaras `IfcDistributionChamberElement`, accesorios `IfcPipeFitting`, válvulas `IfcValve`, tanques `IfcTank`, unidades de planta con su clase IFC (tabla familia → clase IFC en docs/16); sistemas `IfcDistributionSystem` (agua, alcantarillado); terreno como `IfcGeographicElement` con malla; geometría de las piezas horneadas; `Pset_Redes` con cada `Dato` (valor, unidad, fuente), código, estado de revisión y observaciones | el IFC valida con IfcOpenShell y abre en Bonsai con clases, sistemas, propiedades y georreferencia; prueba de contrato sin Blender | 4 |
| F19-03 | P2 | **IFC preparado para 4D** | agrupar por etapa constructiva si el plan la declara (atributo `etapa`, `IfcGroup`/`IfcZone`), para que el cronograma (`IfcWorkSchedule`/`IfcTask`) se arme en Bonsai; la app no planifica ni inventa etapas | proyecto con dos etapas → dos grupos en Bonsai | 1 |
| F19-04 | P1 | **Planta 2D en DXF para planos** | `redes exportar planta --salida planta.dxf`: dibujo en planta (Z=0) con capas por red, familia, DN y estado; símbolos de pozo, válvula, hidrante y unidades de planta como bloques; etiquetas de código, DN, material, longitud y pendiente declaradas (texto con estilo propio, sin solapes graves); curvas de nivel (F18-08) y bordes del levantamiento; sin cajetín: los planos se arman en AutoCAD | abre en AutoCAD/BricsCAD (F18-09) con capas y bloques limpios; prueba de contrato de capas y textos | 3 |
| F19-05 | P1 | **Perfiles longitudinales en DXF** | `redes exportar perfiles --colector <código>\|--todos --salida perfiles.dxf`: por colector, línea de terreno (TIN), tubo con solera y clave, pozos con tapa y fondo, escalas horizontal y vertical elegibles, retícula de cotas y guitarra (abscisa, cota terreno, cota solera, profundidad, DN, longitud, pendiente) con los datos declarados; lo que falte sale `—`; un perfil por colector, separados en el espacio modelo | perfil de la cadena abierto en AutoCAD; los valores de la guitarra coinciden con `redes perfil --json` | 3 |

**Total F19: 13,5 días.** Depende de F17 (piezas horneadas y topografía real) y de F18-08 (curvas de
nivel). Va antes de F16 (release v1).

---

## F16 — Release v1 (P0/P1)

| ID | P | Ticket | Alcance | Aceptación | Días |
|---|---|---|---|---|---|
| F16-01 · [MAM-1127](https://linear.app/mamutsystems/issue/MAM-1127) | P0 | **DXF ilegible es error** (hecho por F12-09) | `importar dxf` con basura devuelve `ok:true`, 0 objetos y sube la revisión | `ok:false`, código `E_FMT_PARSE`, revisión sin cambios | 0,5 |
| F16-02 · [MAM-1128](https://linear.app/mamutsystems/issue/MAM-1128) | P0 | **README con la sintaxis real** | `importar` exige `--archivo`; el README pone la ruta suelta | todos los comandos del README se ejecutan (prueba que los extrae) | 0,5 |
| F16-03 · [MAM-1129](https://linear.app/mamutsystems/issue/MAM-1129) | P1 | **Fixtures reales y verificación externa** | EPANET, SWMM y DXF de Civil 3D reales; pruebas de ida y vuelta con nombre explícito; validar DXF y glTF con un lector externo | `cargo test` cubre los cinco formatos con archivos reales | 2 |
| F16-04 · [MAM-1130](https://linear.app/mamutsystems/issue/MAM-1130) | P1 | **Cobertura de módulos críticos** | `ingest.rs` (3 pruebas en 1.246 líneas), `export.rs` (3), `families_extra.rs` y `parts_cadena.rs` (0), CLI (solo contrato) | cobertura medida (`cargo llvm-cov`) ≥ 60 % en `ingest` y `export` | 2 |
| F16-05 · [MAM-1131](https://linear.app/mamutsystems/issue/MAM-1131) | P1 | **CI de publicación** | disparo por etiqueta `v*`, ejecutar `empaquetar.ps1`, subir artefactos y crear GitHub Release | etiquetar `v0.2.0` publica el zip | 1 |
| F16-06 · [MAM-1132](https://linear.app/mamutsystems/issue/MAM-1132) | P1 | **Instalador** (continúa F11-03) | MSI o winget; firma de código | instala y desinstala limpio en Windows 11 | 2 |
| F16-07 · [MAM-1133](https://linear.app/mamutsystems/issue/MAM-1133) | P1 | **Icono y recursos de Windows** | `.ico`, `.rc` y `build.rs` con versión y nombre | el `.exe` muestra icono y versión en Propiedades | 0,5 |
| F16-08 · [MAM-1134](https://linear.app/mamutsystems/issue/MAM-1134) | P1 | **CHANGELOG** | `CHANGELOG.md` desde 0.1.0 | entrada por versión publicada | 0,25 |
| F16-09 · [MAM-1135](https://linear.app/mamutsystems/issue/MAM-1135) | P2 | **Licencias y vulnerabilidades** | `cargo deny` / `cargo audit` en CI | CI falla con licencia incompatible o aviso de seguridad | 0,5 |
| F16-10 · [MAM-1136](https://linear.app/mamutsystems/issue/MAM-1136) | P1 | **Mensajes de error para personas** | `E_FMT_PARSE` dice «Verifique la ruta» con archivos que existen pero están dañados; mensajes técnicos de serde | pista acorde al error; texto en español claro | 0,5 |
| F16-11 · [MAM-1137](https://linear.app/mamutsystems/issue/MAM-1137) | P2 | **Avisos de compilación y clippy** | ~20 avisos propios (16 de asignación fuera de inicializador, patrones inalcanzables, campos `symbols`/`depth` sin leer, variables `field`/`nb`/`nink`) | `cargo clippy` sin avisos propios; subir el nivel en CI | 0,5 |
| F16-12 · [MAM-1138](https://linear.app/mamutsystems/issue/MAM-1138) | P2 | **Salida legible en terminal** | la CLI imprime JSON en una línea; `--pretty` automático si la salida es una terminal | salida legible sin flags en terminal; sin cambios en tuberías | 0,5 |
| F16-13 · [MAM-1139](https://linear.app/mamutsystems/issue/MAM-1139) | P2 | **Codificación de los `.md` en Windows** | mojibake en PowerShell 5.1 | se ven bien en la consola de Windows | 0,25 |

**Total F16: 11 días.**

---

**Total revisión v1: 91,75 días.** Ruta mínima a v1 (P0): F12-01…08, F13-01, F14-01…03, F15-01,
F16-01, F16-02 ≈ **27 días** (≈ 5,5 semanas a una persona).

---

## Orden recomendado (tres sprints)

| Sprint | Contenido | Resultado |
|---|---|---|
| **1 (2 semanas)** | F6-01, F6-02, F6-03, F6-04 + F7-01 — **ya creados en Linear** como MAM-1066…MAM-1070, proyecto *Redes — Visor 3D* | se **ven** los modelos y sus medidas llegan al objeto |
| **2 (2 semanas)** | F6-05…08 + F7-02, F7-03, F7-04 | modelos con anclas, cantidades por familia y aviso de descuadre |
| **3 (2 semanas)** | F8-01, F8-02 + F9-01, F9-05 | el CAD devuelve datos y el proyecto entra en Git/CI |
| Después | F9-02..06, F10, F11 | robustez, acabado y producto |

**Total estimado: 51,5 días** (≈ 11 semanas a una persona; 4–5 semanas con dos).

---

## Definición de terminado (para todos los tickets)

1. Prueba automática que falla sin el cambio.
2. `cargo test --workspace` y `cargo build --release` en verde.
3. Documento actualizado (`docs/`), no solo el código.
4. Si toca la interfaz: captura headless con `--snap` en el ticket.
5. Si toca datos: antes de v1 (D10), si cambia el esquema se sube `SCHEMA_VERSION` y se regeneran
   los `.redes` de `demo/`; desde v1, migración y fixture de la versión anterior.

## Riesgos que cambian la lista

- **Sin `git` no hay tickets trazables**: F9-05 debería adelantarse si entra más gente.
- **DWG** depende de una licencia (ODA); si no hay, F8-03 se cierra como “no” y se queda DXF.
- **Modelos de usuario de mala calidad** (mallas abiertas, escala en mm): el validador ya bloquea
  lo grave; F7-02 evita que un descuadre pase inadvertido.

---

## Seguimiento en Linear

- Proyecto: **Redes — Visor 3D de redes sanitarias (Rust + Rune)** (`P-MAM-15`).
- Etiqueta: `redes`.
- Sprint 1 creado: **MAM-1066 … MAM-1070** (F6-01, F6-02, F6-03, F6-04, F7-01).
- Revisión v1 (25/11/2026): **MAM-1085 … MAM-1139** (F12-01…F16-13, 55 tickets) con milestone por fase y prioridad P0=Urgent, P1=High, P2=Medium.
- El resto de las fases (F6-05…F11) se crean cuando el sprint 1 esté en marcha, para no llenar el
  tablero de tickets que aún no se van a tocar.
