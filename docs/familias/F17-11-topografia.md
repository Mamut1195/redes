# F17-11 + F17-12 — Topografía de levantamiento, CRS y datum vertical (MAM-1154, MAM-1156)

Informe de la rama para que el orquestador lo consolide en docs/02, 18, 21 y 23 (este agente no los
edita). Decisión de origen: D18.

## 1. Contrato `redes.topografia/1`

Documentado en `skills/redes-ingesta/references/topografia.md` §1; código en
`crates/redes-core/src/survey.rs` (`SurveyFile`, `ElevationGrid`) y `terrain.rs` (`SurveyPoint`,
`PointClass`, `VerticalDatum`).

- Raíz: `schema` (obligatorio, exacto), `nombre`, `crs` (`EPSG:326NN`/`327NN`), `datum_vertical`
  (obligatorio: `egm2008` | `egm96` | `elipsoidal` | `no_declarado`), `fuente`, `puntos`,
  `quiebres`, `contorno`, `huecos`, `malla`. Campos desconocidos → `E_FMT_CAMPO`.
- Punto: `numero` (texto, único), `e`, `n`, `z`, `descripcion` (original), `clase`
  (`terreno` | `quiebre` | `contorno` | `excluido`).
- Quiebre: `descripcion` + `puntos` (números en orden, ≥ 2, solo `quiebre`/`contorno`).
- Contorno: lista de números (cierre implícito); huecos: `descripcion` + `puntos` (≥ 3).
- Malla: `e_centro_noroeste`, `n_centro_noroeste`, `celda_m`, `filas`, `columnas`, `sin_dato`,
  `valores` (fila a fila desde el norte), `fuente`.
- Errores nuevos: `E_TOPO_PUNTO`, `E_TOPO_COINCIDENTES`, `E_TOPO_REFERENCIA`, `E_TOPO_GEOMETRIA`,
  `E_TOPO_MALLA`, `E_TOPO_VACIA` (código 7) y `E_DATUM` (código 1); en `redes esquema`.

Reglas de la app (nada se reclasifica ni se inventa):

- Entran al TIN `terreno`, `quiebre`, `contorno` y los nodos de malla; los `excluido` se guardan
  con número y descripción pero no tocan la superficie.
- Misma planta (±1 mm) y misma cota: entra una vez con aviso. Distinta cota: `E_TOPO_COINCIDENTES`.
- Quiebres = aristas obligadas. Dos que se cruzan fuera de un punto común (o con el contorno) →
  `E_TOPO_GEOMETRIA` con los tramos (lo detecta el propio CDT, sin O(n²)).
- Contorno y huecos con todos sus vértices en el TIN: sus lados son restricciones y el recorte es
  exacto. Con algún vértice `excluido` (esquinas de casa): se recorta todo triángulo que pise el
  polígono, el hueco queda algo más grande y nunca se interpola dentro.
- Sin contorno: casco convexo, con aviso.
- Malla: un nodo no entra si hay un punto o un quiebre del levantamiento a menos de una celda, ni
  fuera del contorno ni en un hueco (el levantamiento manda donde existe). Tope 1,5 M nodos.
- `z_at` fuera del contorno o en un hueco → `None`.

Proyecto: `redes.project/4` (D10, sin migración; demos regenerados). `Terrain` añade
`datum_vertical`, `constraints`, `boundary`, `holes` y `levantamiento` (puntos, quiebres, contorno,
huecos, uso de la malla). `Project` añade `datum_vertical` (de la red, lo declara el plan).

## 2. `importar terreno`

**Se retira** (regla sin-legacy: nada en producción). Leía las tres primeras columnas como
E, N, Z: una fila PNEZD `1,9796005,760005,2503.7,TN` entraba como E = 1, N = 9 796 005,
Z = 760 005, sin aviso. Ahora el único camino es `redes importar topografia --archivo x.json`
(con `--dry-run`); el comando viejo no existe (clap lo rechaza, código ≠ 0) y un PNEZD crudo
pasado a `importar topografia` da `E_FMT_PARSE`. Pruebas: `a_raw_pnezd_file_cannot_be_misread_as_terrain`
(core) y `the_old_terrain_import_no_longer_exists` (CLI). Se borra `tests/fixtures/terreno.csv`
y sus pruebas en `tests/ingesta.rs`. El `terreno: [[E,N,Z]]` del plan sigue (es JSON explícito).

## 3. Triangulación restringida: `spade`

`spade 2.15` (MIT/Apache, Rust puro, predicados robustos; deps `robust`, `smallvec`, `hashbrown`,
`num-traits`) sustituye a `delaunator`. Motivo: trae CDT con carga masiva
(`try_bulk_load_cdt`) que informa de restricciones en conflicto en vez de abortar, parte una
restricción que pasa por un vértice y descarta duplicados. Implementar CDT propio sería más código
y más riesgo que la dependencia. `Terrain::build` (demos, plan) usa la misma CDT sin restricciones:
la diagonal elegida en rejillas cocirculares cambia respecto a `delaunator` (los demos regenerados
lo reflejan), las pruebas existentes pasan igual.

`Terrain::thinned` (copia de dibujo) conserva los vértices de restricción y vuelve a triangular
con las mismas restricciones y recortes: antes de esto el aligerado volvía a rellenar la quebrada.

### Cifras (Windows 11, 16 hilos, con otros 4 agentes compilando: CPU al 100 %)

`cargo test --release -p redes-core --test topografia survey_ -- --include-ignored --nocapture`.
Levantamiento sintético: n puntos al azar en 1 km², 20 quiebres de 200 tomas, contorno de 400
vértices, 50 casas como huecos (esquinas excluidas). Tiempo de `import_survey` completo (leer
JSON, validar, CDT, recorte, índice):

| Puntos | Release | Depuración | Triángulos |
|---|---|---|---|
| 10 000 | 146 ms | 496 ms | — |
| 40 000 | 328 ms | 1 508 ms | 65 972 |
| 100 000 | **801 ms** | — | 152 788 |

Crecimiento 10k→40k ×2,2–3,0 (prueba `survey_import_grows_linearly` exige < ×10).

Banco F14 (`tests/banco.rs`): no usa terreno (el plan sintético no trae `terreno`), así que este
cambio no toca su camino. En depuración falló una vez por la carga de la máquina (escena 2 579 ms
frente a 600 ms; 5k→20k ×5,2): ver §7 para la cifra en release.

## 4. Skill

- Nuevo `skills/redes-ingesta/references/topografia.md`: contrato, qué valida la app, flujo,
  lectura de PNEZD/PENZD/NEZ/ENZ (cómo distinguir E y N por magnitud y cuándo preguntar: en el
  hemisferio norte cerca del ecuador no se puede), DXF (POINT, INSERT con atributos, polilíneas
  3D), LandXML (CgPoints N E Z, Breaklines, Boundaries outer/hide, Definition), MDT en rejilla
  (Copernicus, georreferencia de la celda NO, NoData), **tabla de códigos típicos** aprobada como
  valores ajustables, reglas de encadenado (consecutivos, `BV1`/`BV2`, `INI`/`FIN`, número de
  cadena, cierre), contorno y huecos, CRS y datum, qué preguntar, ejemplo trabajado e informe.
- `SKILL.md`: descripción y «Cuándo usarla» incluyen la topografía; tabla de referencias enruta a
  `topografia.md`; se quita el atajo `importar terreno`; preguntar el datum vertical.
- `references/plan.md`: CRS norte/sur, nuevo `proyecto.datum_vertical`, se corrige «el TIN se
  etiqueta siempre como 32717».
- Fixtures: `crates/redes-core/tests/fixtures/topografia_quebrada.pnezd.txt` (137 puntos: quebrada
  con fondo cada 24 m y márgenes cada 12 m, dos bordes de vía, contorno con muesca, casa,
  árbol, poste, BM, pozo existente), su traducción `topografia_quebrada.topografia.json`, y
  `topografia_civil3d.landxml.xml` + `topografia_civil3d.topografia.json`.
- docs/03 y docs/04 actualizados (diagrama, garantías, pruebas, CRS/datum, problemas frecuentes).

## 5. F17-12 CRS y datum

- `ingest::is_wgs84_utm` acepta 32601–32660 y 32701–32760 (antes solo sur). Mensajes y pista de
  `E_CRS` actualizados. 32717 sigue siendo el valor por defecto de `proyecto nuevo --crs` y de los
  demos (es su zona real); no queda ningún lugar que lo suponga para datos importados.
- Datum: `VerticalDatum` en `Terrain` y en `Project`. `check_vertical_datum` compara lo que entra
  con el de la red y el del terreno: dos declarados distintos → `E_DATUM` (sin tocar el proyecto);
  uno sin declarar frente a uno declarado → aviso. Reimportar conserva el datum.
- Se muestra en `proyecto info` (`terreno.datum_vertical`, `datum_vertical_red`,
  `puntos_levantamiento`, `quiebres`, `contorno`, `huecos`) y en el inspector de la app
  (panel Proyecto: «datum de la red», «datum del terreno», «puntos de terreno»).

## 6. Pruebas y mutación

- `terrain.rs` (unitarias, 7 nuevas): ningún triángulo cruza la quebrada (y sin restricción sí,
  para probar que la prueba muerde); la copia aligerada conserva la quebrada; restricciones
  cruzadas se informan; las concavidades del contorno no tienen triángulos (150 de 200); hueco con
  esquinas fuera del TIN sin cota dentro; claves del datum.
- `tests/topografia.rs` (core, 25 + 1 ignorada): fixture PNEZD traducida, cota del fondo de la
  quebrada, sin cota en muesca/casa/fuera, guardar y abrir, PNEZD crudo, LandXML, versión,
  campos, clase, datum obligatorio, no finitos, números repetidos, coincidentes, referencias,
  quiebres cruzados, contorno que se corta, hueco fuera, aviso sin contorno, malla sola, malla y
  levantamiento, malla incoherente, hemisferio norte, otra zona y grados, `E_DATUM` en ambos
  sentidos, aviso sin declarar, crecimiento lineal, 100k (ignorada).
- `crates/redes-cli/tests/topografia.rs` (5, binario real): `--dry-run` + importación + info,
  `importar terreno` ya no existe, `E_DATUM` con código 1, proyecto 32618, `esquema` lista los
  códigos.
- `tests/ingesta.rs`: 18N entra; 32661/3857/4326 dan `E_CRS`.
- `scripts/mutar.ps1`: 4 mutaciones nuevas — `topo-sin-quiebres` (CDT sin restricciones),
  `topo-sin-recorte` (sin recorte por contorno), `topo-coincidentes` (elegir una cota en
  silencio), `datum-mezclado` (no comprobar el datum).

## 7. Resultados de esta rama

- `cargo test --workspace`: 346 pruebas, 0 fallos (con 2 hilos de compilación, tras liberar disco).
- Banco F14 en release (`cargo test --release -p redes-core --test banco`): pasa; escena 20k
  7,3 ms, ingesta 267,5 ms, abrir 188,5 ms, reimportar 634,7 ms, malla 3D 95,4 ms (sin regresión:
  este cambio no toca el camino del banco).
- `scripts/mutar.ps1`: 50/50 mutaciones detectadas, incluidas las 4 nuevas; salida 0.
- clippy `-D clippy::correctness`: ningún aviso nuevo en los archivos tocados; `cargo fmt --check`
  en verde; build release de `redes-app` y `redes-cli` en verde.
- Capturas en `out/f17-11/` (ignorado por git): `quebrada-iso-cpu.png`, `quebrada-iso-gpu3d.png`,
  `quebrada-planta-cpu.png`, `quebrada-planta-gpu3d.png`. Proyecto: `out/f17-11/quebrada.redes`
  (137 puntos, 129 vértices de TIN, 244 triángulos). Se ve la quebrada como un surco continuo, la
  muesca del contorno sin triángulos y el hueco de la casa (algo mayor que la casa: recorte por
  solape con esquinas excluidas); el inspector muestra los dos datums y los 137 puntos.

## 8. Decisiones para el usuario

1. **Datums admitidos.** Solo `egm2008`, `egm96`, `elipsoidal`, `no_declarado`. Un levantamiento
   de estación total enlazado a la red de nivelación nacional (IGM, mareógrafo de La Libertad) no
   es ninguno de esos: hoy entra como `no_declarado`. ¿Añadir `nacional` (o `local` con texto)?
2. **Línea de quiebre que cita un punto `terreno`** se rechaza (`E_TOPO_REFERENCIA`): obliga a la
   traducción a marcarlo `quiebre`. ¿Conforme o se relaja?
3. **Huecos con esquinas excluidas** recortan por solape (hueco algo mayor que la casa). La
   alternativa exacta exige que las esquinas sean `contorno` con su cota al pie. La skill sigue la
   tabla aprobada (`EDI` → excluido + hueco).
4. **Malla**: entra completa salvo junto al levantamiento (a menos de una celda); no se diezma más.
   Para mallas enormes hay tope de 1,5 M nodos y se pide recortar.
5. **Curvas de nivel como entrada** (polilíneas 2D con elevación) siguen sin camino: la skill pide
   preguntar antes de convertirlas en puntos.
