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

## F14 — Rendimiento con redes reales (P0/P1)

Objetivo: fluidez con 5k–50k tramos y 2k–20k pozos (hoy solo con cientos).

| ID | P | Ticket | Alcance | Aceptación | Días |
|---|---|---|---|---|---|
| F14-01 · [MAM-1105](https://linear.app/mamutsystems/issue/MAM-1105) | P0 | **Índices en `Project`** | `node()`/`link()` recorren todo (`model.rs:296-301`) y se llaman por tramo en `view.rs:666`; también `topology.rs:63`, `add_node/add_link` (`model.rs:343,378`), `next_code`, ajuste de nodos en ingesta (`ingest.rs:574,1056`). `HashMap` por id y código | construir la escena de 50k tramos < 100 ms en release; ingesta de 50k < 2 s | 1,5 |
| F14-02 · [MAM-1106](https://linear.app/mamutsystems/issue/MAM-1106) | P0 | **Cámara sin regenerar malla en 3D** — **hecho** | Hecho: la exageración vertical va en la matriz (`Geometria::fijar_camara`, `cam.matriz(aspecto, z_scale)`) y la malla se guarda en coordenadas locales con cota real; `rebuild()` en `--gpu3d` no llama a `build_opts` ni arma el `Shape`: solo `view::etiquetas` y la matriz; la malla se regenera solo si cambia `ClaveMalla` (proyecto y revisión, capas, biblioteca). Prueba `mover_la_camara_en_3d_no_regenera_la_malla`, mutación `gpu3d-camara-regenera`. Ciudad, release: 6,0 ms → 0,008 ms por cambio de cámara. Antes: `rebuild()` (`main.rs:227-277`) regenera todo por cada evento de ratón; pasar `z_scale` a la matriz (`view.rs:142`), regenerar solo si cambian proyecto o capas; no ejecutar `build_opts` ni construir el `Shape` en modo `--gpu3d` | arrastrar en 3D no llama a `triangulos_exportables` (contador en prueba) | 2 |
| F14-03 · [MAM-1107](https://linear.app/mamutsystems/issue/MAM-1107) | P0 | **Sin copia del búfer por frame** — **hecho** | Hecho: `Geometria::bytes` es `Arc<[u8]>` y el compositor toma con `Geometria::tomar(generación subida)` la matriz y, solo si hay generación nueva, el `Arc` (sin copiar bytes). Prueba `la_escena_quieta_no_se_vuelve_a_subir`, mutación `gpu3d-sube-cada-frame`. Ciudad: 3,6 MB copiados por frame (0,46 ms con el mutex) → 0. Antes: `gpu3d.rs:352` clona los bytes en cada frame con el mutex tomado; `Arc<[u8]>` o copiar solo si cambia la generación | 0 copias por frame con la escena quieta | 0,5 |
| F14-04 · [MAM-1108](https://linear.app/mamutsystems/issue/MAM-1108) | P1 | **Cámara precalculada y sin `Vec` por triángulo** | `basis()`/`eye()` por punto (`view.rs:170-206`); `pts`/`expand()` y `vec![tri]` por triángulo (`view.rs:733-961`) | construcción de escena ≥ 3× más rápida en el banco | 1 |
| F14-05 · [MAM-1109](https://linear.app/mamutsystems/issue/MAM-1109) | P1 | **LOD, recorte e instanciado en 3D** | `gltf.rs:75` usa siempre detalle completo; sin frustum culling; mallas sin índices copiadas por instancia (`gpu3d.rs:134-146`); el tope de 512 MB (`:318`) recorta en silencio | 20k pozos < 512 MB; superar el tope avisa en la interfaz | 4 |
| F14-06 · [MAM-1110](https://linear.app/mamutsystems/issue/MAM-1110) | P1 | **Caché de mallas paramétricas y proxies** | cachear por (familia, tipo, DN, LOD); no clonar la `Biblioteca` (`main.rs:386`) ni recalcular el proxy por instancia (`:410`) | segunda reconstrucción sin cambios no genera mallas | 1,5 |
| F14-07 · [MAM-1111](https://linear.app/mamutsystems/issue/MAM-1111) | P1 | **Rejilla espacial para red y picking** | recorte por pantalla antes de ordenar; ajuste de nodos y picking con rejilla | picking O(1) amortizado; objetos fuera de pantalla no se ordenan | 1 |
| F14-08 · [MAM-1112](https://linear.app/mamutsystems/issue/MAM-1112) | P2 | **Deshacer por diferencias** | `recordar()` clona el `Project` entero 20 veces (`main.rs:281-285`) | memoria del deshacer proporcional al cambio | 2 |
| F14-09 · [MAM-1113](https://linear.app/mamutsystems/issue/MAM-1113) | P1 | **Perfil de compilación** | `[profile.release] lto = "thin", codegen-units = 1`; `[profile.dev.package."*"] opt-level = 2` | binario release más rápido en el banco; dev usable | 0,25 |
| F14-10 · [MAM-1114](https://linear.app/mamutsystems/issue/MAM-1114) | P1 | **Banco con red sintética grande** | red de 20k/50k tramos generada; umbral en CI (amplía F9-03) | CI falla si la construcción de escena pasa del umbral | 1,5 |

**Total F14: 15,25 días.**

---

## F15 — Arquitectura y mantenibilidad (P1/P2)

| ID | P | Ticket | Alcance | Aceptación | Días |
|---|---|---|---|---|---|
| F15-01 · [MAM-1115](https://linear.app/mamutsystems/issue/MAM-1115) | P0 | **Migración de formato `.redes`** | hoy `load` deserializa directo (`project_io.rs:107`) y depende de `serde(default)`; pasar por `serde_json::Value` → `migrar_vN→vN+1` → `Project`. `redes.plan/*` acepta versiones futuras (`ingest.rs:124`): rechazarlas | fixtures v1 y v2 en `tests/fixtures` abren; plan `/2` → error `E_FMT_VERSION` | 2 |
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

**Total F15: 24 días.**

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
5. Si toca datos: compatibilidad con archivos anteriores comprobada.

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
