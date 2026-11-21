# 05 — Producto y Guía

**Documento:** definición de producto, alcance funcional y arquitectura documental
**Autor:** PM / Technical Writer (consejo de arquitectura)
**Estado:** Propuesta para revisión del cliente — versión 1.0
**Fecha:** 2026-11-18
**Producto:** Rune Redes — app de escritorio (Rust + Rune UI) y CLI para diseño de redes de agua potable y alcantarillado sanitario
**Idioma:** español (es-EC). El producto, la guía y los mensajes de error se entregan en español; el inglés queda como traducción opcional.

---

## 0. Cómo leer este documento

Este documento define **qué se construye, para quién, en qué orden y cómo se documenta**. No decide el motor hidráulico, el esquema de datos ni la arquitectura de crates: eso vive en los documentos hermanos del consejo (`01-*.md` a `04-*.md`). Cuando este documento necesita una decisión técnica, la marca como **[depende de 0X]** y la recoge en §8.

Cinco ideas que ordenan todo lo demás:

1. **El objeto es la unidad de trabajo, no el dibujo.** Pozos, tuberías, vertederos, válvulas y llaves son familias con atributos, reglas y cálculos propios. El plano es una *vista* del modelo, nunca la fuente de verdad.
2. **Todo lo que la UI hace, la CLI lo hace.** Sin excepciones y con una prueba automática que lo verifica (CLI-01). La CLI no es un extra: es la interfaz para agentes IA y para automatización municipal.
3. **Ningún número sin trazabilidad.** Cada resultado declara fórmula, parámetros, code pack y versión del proyecto. Si el code pack está incompleto, el producto **no afirma cumplimiento** (ver §5.6).
4. **La excavación es un producto, no un subproducto.** El cliente pidió cómputos: el volumen de zanja y pozo se calcula por secciones, con abundamiento y material excedente, y se compara contra campo.
5. **La guía se escribe con el producto y se prueba con él.** Cada ejemplo de la guía se ejecuta en CI; una captura desactualizada es un defecto, no una molestia.

### 0.1 Resumen ejecutivo

- **4 personas**, 8 recorridos de punta a punta (3 con variante de agente IA), **88 ítems de backlog** en 15 epics, priorizados MUST/SHOULD/COULD/WON'T.
- **v0.1 es un esqueleto caminante honesto**: importar topografía, 3 pozos, 2 colectores, excavación por tramo y perfil. Demuestra la cadena crítica completa en 20 minutos, aunque falte casi todo lo demás.
- **v1.0** cubre agua potable + alcantarillado + hidráulica validada contra code pack + cómputos + planos + CLI/MCP con paridad total.
- **9 documentos** forman la guía; el primero a escribir es la *Guía de inicio rápido*, y su sección 1 se entrega redactada en §5.10.
- **10 decisiones** quedan abiertas para el cliente (§8). Las tres primeras bloquean la validación normativa y deben cerrarse antes de v0.2.

### 0.2 Supuestos y dependencias

| # | Supuesto | Consecuencia si es falso |
|---|---|---|
| S1 | No existe código de producto todavía; el repositorio solo contiene `docs/` y un `vendor/rune` vacío. | El plan de releases (§4) asume construcción desde cero. |
| S2 | Rune UI puede alojar un lienzo 2D con pan/zoom, edición de vértices, entrada de teclado numérico y tablas virtualizadas. **[depende de 01]** No se pudo verificar: el clon vendorizado no tiene fuentes. | Sin lienzo propio, la vista de planta cae a SVG re-generado o a un panel de dibujo embebido; VIS-01 y AGU-01 pasan a riesgo alto y v0.1 se reduce a tablas + perfil. |
| S3 | Los levantamientos llegan en DXF (puntos acotados, curvas de nivel) o CSV de coordenadas; el datum puede ser WGS84 o PSAD56. | Si llegan en LandXML o en formato de estación total propietario, se agrega un importador (TOP-01 se vuelve L). |
| S4 | El cliente puede nombrar un ingeniero responsable que firme la parametrización normativa y las memorias. | Sin firma, la memoria de cálculo se entrega como borrador no apto para trámite. |
| S5 | La app debe funcionar **offline**; la licencia no depende de internet. | Cambia el diseño de licenciamiento y el de actualización de code packs. |
| S6 | Los municipios entregan simbología y cajetín propios, pero no una API. | Las plantillas de planos se configuran a mano (VIS-04, INF-04). |

### 0.3 Fuera de alcance de la v1 (explícito)

Modelado de transitorios hidráulicos (golpe de ariete), diseño estructural de obras civiles, presupuesto con base de precios propia, GIS completo, edición colaborativa en tiempo real, app móvil de campo, IFC/BIM, y cálculo de plantas de tratamiento. Se documentan como limitaciones en la guía (DOC-05) para que nadie las suponga incluidas.

---

## 1. Personas y trabajos por hacer

### 1.1 Vista comparada

| Persona | Objetivo principal | Métrica de éxito | 3 funcionalidades que más le importan |
|---|---|---|---|
| **P1. Ingeniera/o sanitaria/o de consultora** (diseñadora) | Producir un diseño defendible y entregable en días, no semanas | Diseño de 2 km con memoria y planos en ≤ 3 días hábiles | Perfil longitudinal automático (VIS-02) · Hidráulica validada contra norma (HID-01/03) · Memoria de cálculo reproducible (INF-01) |
| **P2. Técnico municipal / de la EMAP** | Revisar, comparar y aprobar proyectos con criterio uniforme | 80% de expedientes revisados en ≤ 1 día, con observaciones trazables | Validación contra code pack municipal (HID-03) · Auditoría de proyecto recibido (CLI-07, PER-03) · Listados y cómputos comparables (INF-03) |
| **P3. Jefe de obra / constructora** | Convertir el diseño en cantidades confiables y ejecutables | Error de volumen ≤ 5% vs medición en campo | Excavación por tramo y pozo (EXC-02/03) · Cómputos y presupuesto XLSX (EXC-05, INF-02) · Comparación calculado vs campo (EXC-07) |
| **P4. Agente IA (CLI/MCP)** | Ejecutar el flujo completo sin GUI, de forma determinista y verificable | ≥ 95% de comandos exitosos sin reintento humano | Manifiesto autodescriptivo (CLI-03) · Salida `--json` estable (CLI-02) · Errores estructurados con sugerencia (CLI-05) |

### 1.2 P1 — Ingeniera/o sanitaria/o de consultora

**Perfil.** 28–40 años, ingeniería civil o sanitaria, trabaja en una consultora de Quito, Cuenca o Guayaquil con 3–8 personas. Diseña 4–10 proyectos al año para municipios y para el sector inmobiliario. Domina AutoCAD y Excel; usa EPANET cuando el cliente lo exige y sabe que sus planillas tienen errores que nadie revisa.

**Trabajo por hacer.** *Cuando un municipio me contrata para diseñar una red, quiero llegar a un diseño consistente y defendible sin rehacer el perfil tres veces, para entregar a tiempo y que no me observen el expediente.*

**Flujo actual y dolor.** Levantamiento en Civil 3D o AutoCAD → dibuja planta a mano con bloques de pozo → arma el perfil longitudinal proyectando cotas una por una → vuelca longitudes y cotas a Excel → calcula pendientes y volúmenes en planillas encadenadas → si cambia una cota, **rehace el perfil y reconcilia el Excel a mano**. La memoria de cálculo se escribe al final copiando números del Excel, y con frecuencia no coincide con el plano. Dolor medido en el piloteo: 60–70% del tiempo se va en perfil y reconciliación; 20–30 h por km de red.

**Éxito.** Un cambio de cota en un pozo recalcula colector, perfil, volúmenes y memoria sin intervención manual; el expediente sale con cifras idénticas en plano, memoria y cómputo.

**Top 3.** VIS-02 (perfil generado del modelo) · HID-01 + HID-03 (hidráulica + validación normativa) · INF-01 (memoria de cálculo reproducible).

### 1.3 P2 — Técnico municipal / de la EMAP

**Perfil.** 35–55 años, revisa expedientes de terceros y planifica obra propia. Su herramienta principal es la norma municipal y la experiencia; su problema es que cada consultora entrega en formato distinto. Vive entre observaciones por inconsistencias y por criterios que no puede verificar.

**Trabajo por hacer.** *Cuando recibo un expediente, quiero verificar en una hora que cumple la ordenanza y que los números cuadran, para aprobar con sustento o devolver con observaciones precisas.*

**Flujo actual y dolor.** Recibe PDF + DWG + Excel; imprime, mide con escalímetro, compara cifras entre documentos, escribe observaciones en Word sin referencia al objeto. No hay registro de qué versión revisó. Cuando el consultor reenvía, vuelve a empezar.

**Éxito.** Abre el proyecto, corre la validación, obtiene una lista de incumplimientos con cláusula y objeto, y exporta un acta de revisión en 15 minutos; puede comparar la revisión 1 con la 2.

**Top 3.** HID-03 (validación con cláusula y severidad) · COL-04 + PER-03 (bitácora y auditoría de lo recibido) · INF-03 (listados y cómputos comparables entre proyectos).

### 1.4 P3 — Jefe de obra / constructora

**Perfil.** 40–60 años, residente de obra o gerente de una constructora pequeña. No diseña: **mide y ejecuta**. Su unidad de trabajo es el ítem de presupuesto y el viaje de volqueta.

**Trabajo por hacer.** *Cuando me entregan un diseño, quiero saber cuántos metros cúbicos de excavación, relleno y material excedente voy a pagar, para presupuestar sin quedarme corto ni pelear con el fiscalizador.*

**Flujo actual y dolor.** Recibe planos en PDF; recalcula volúmenes a mano en Excel con secciones dibujadas en AutoCAD; el abundamiento se aplica "como siempre" (1,25–1,30) sin respaldo; el fiscalizador mide distinto y aparecen adicionales. No hay forma de comparar lo calculado con lo medido.

**Éxito.** Exporta el cómputo por tramo y por pozo en XLSX con fórmulas visibles y criterios escritos; cuando llega la medición de campo, importa el CSV y ve el desvío por tramo con semáforo.

**Top 3.** EXC-02 + EXC-03 (volumen por secciones y por pozo) · EXC-04 (relleno, préstamo, excedente, abundamiento) · EXC-07 + INF-02 (comparación con campo y XLSX).

### 1.5 P4 — Agente IA (CLI/MCP)

**Perfil.** No es una persona: es un LLM operando por CLI o por MCP, encadenado a otros agentes (revisión, cotización, generación de informes). No ve la pantalla, no adivina, no tolera ambigüedad. Se equivoca si los parámetros no declaran unidad o si el error es un stacktrace.

**Trabajo por hacer.** *Cuando se me pide "diseña el colector de la calle X y dime si cumple", quiero descubrir las operaciones disponibles, ejecutarlas con parámetros tipados y obtener un resultado verificable con código de salida, para completar la tarea sin un humano mirando.*

**Flujo actual y dolor.** Sin CLI, el agente queda reducido a generar texto que un humano transcribe: lento y propenso a error. Con CLIs improvisadas, los comandos cambian sin aviso, los errores son texto libre y las unidades se adivinan (¿200 es mm o cm?).

**Éxito.** Descubre el manifiesto, ejecuta 12 comandos encadenados, obtiene `exit 0` con JSON validado por esquema, y si algo falla recibe `codigo` + `objeto` + `sugerencia` y puede corregir sin humano.

**Top 3.** CLI-03 (manifiesto en runtime, no lista copiada) · CLI-02 (JSON versionado con unidades en el nombre) · CLI-05 (errores con código y remedio).

---

## 2. Recorridos de punta a punta

Convenciones: los **objetos** van en `mono`; los **comandos** en bloque de código. Todos los comandos aceptan `--json`. El proyecto de ejemplo es siempre el mismo (§6.2) para que los recorridos se puedan probar.

### R1 — Crear proyecto e importar el levantamiento topográfico

**Actor:** P1 · **Duración objetivo:** 10 min · **Precondición:** DXF o CSV del levantamiento.

1. `redes proyecto nuevo --nombre "Alcantarillado La Esperanza" --datum WGS84 --utm 17S --unidades m` → crea el archivo `.redes` y fija el CRS del proyecto (objeto `Proyecto`).
2. El asistente pide población, dotación, aporte y periodo de diseño; valida rangos y guarda (objeto `ParametrosDiseno`).
3. `redes topo importar levantamiento.dxf --capa PUNTOS --capa CURVAS` → objetos `PuntoTopografico` y `CurvaNivel`.
4. Revisa el informe de importación: N leídos, N descartados **con motivo** (sin cota, duplicado, fuera de zona).
5. `redes topo superficie crear --metodo tin --resolucion 1.0` → objeto `Terreno` (MDT/TIN).
6. Verifica 3 puntos de control contra el plano: cota consultada vs cota de plano, tolerancia 0,05 m.
7. `redes modelo validar` → 0 errores críticos antes de dibujar nada.

**Artefactos:** `.redes` con topografía, informe de importación, metadatos de CRS.
**Éxito:** ningún punto huérfano; cota consultable en cualquier xy; el proyecto reabre idéntico.

> **Variante agente IA (R1-IA).** El agente recibe `{"levantamiento": "ruta.dxf", "crs": "EPSG:32717"}`. Ejecuta `redes topo importar --json`, lee `puntos_leidos`, `puntos_descartados[].motivo`; si hay descartes por "sin cota" > 5%, se detiene y reporta en vez de continuar con un MDT sesgado. Solo entonces crea el TIN y valida.

### R2 — Trazar la red de agua potable

**Actor:** P1 · **Duración objetivo:** 45 min para 1,5 km.

1. `redes red-agua crear --nombre "RP-1"` → objeto `RedAgua`.
2. En planta, clic sobre el MDT crea `Nodo` (empalme, reservorio, predio); *snap* a tubería existente para derivaciones.
3. `redes red-agua tramo crear --desde N-01 --hasta N-02 --material PVC --diametro_mm 110` → objeto `TuberiaAgua` con longitud calculada.
4. Asistente de dimensionamiento: propone diámetro comercial que cumple velocidad y pérdida; muestra alternativas y su Δcosto (AGU-03).
5. Inserta `Valvula`, `Hidrante`, `Llave` y `Medidor` desde el catálogo de familias (MOD-04).
6. `redes hidraulica calcular --red RP-1 --metodo hazen-williams` → caudales, velocidades, pérdidas, presiones por nodo.
7. `redes norma validar --red RP-1` → incumplimientos con cláusula y severidad.

**Artefactos:** red con trazabilidad nodo–tramo, cuadro de diámetros, resultados hidráulicos.
**Éxito:** toda la red conectada al menos a un punto de suministro; presiones dentro de rango o excepción justificada.

### R3 — Trazar colector con pozos y pendientes

**Actor:** P1 · **Duración objetivo:** 30 min para 800 m.

1. `redes alcantarillado red crear --nombre "Colector 1"`.
2. `redes alcantarillado pozo crear --nombre P-01 --cota_tapa 2810.45 --profundidad_m 2.10` → objeto `PozoRevision` (subtipo).
3. `redes alcantarillado colector crear --desde P-01 --hasta P-02 --diametro_mm 200 --pendiente 0.008` → objeto `Colector`; el sentido de flujo queda definido.
4. Repite por tramo; la app propone pendiente mínima según diámetro y code pack (ALC-06).
5. Pozo de caída donde la pendiente natural excede el límite: `redes alcantarillado pozo caida --pozo P-05 --altura_m 0.90` (ALC-03).
6. `redes alcantarillado validar` → pozos sin salida, ciclos, cotas inconsistentes, invertidas fuera de orden.
7. Vista de perfil en vivo para confirmar que la rasante no choca con el terreno.

**Artefactos:** red de alcantarillado con pozos tipificados y perfiles.
**Éxito:** 0 errores topológicos; 0 tramos con pendiente bajo el mínimo sin justificación registrada.

### R4 — Calcular hidráulica y validar contra norma

**Actor:** P1 (diseño) y P2 (revisión) · **Duración objetivo:** 10 min.

1. `redes hidraulica calcular --red "Colector 1" --metodo manning` → Q, v, Y/D, esfuerzo cortante por tramo.
2. `redes norma validar --code-pack EC-ALC-MUNICIPAL --json` → lista de `check` con `id`, `clausula`, `severidad`, `objeto`, `valor`, `limite`.
3. Revisa solo los críticos: la UI filtra y hace zoom al objeto del incumplimiento.
4. Corrige diámetro o pendiente; recalcula solo el tramo afectado y sus aguas abajo (recálculo incremental).
5. Reejecuta validación hasta 0 críticos. Cada excepción aceptada exige justificación escrita que irá a la memoria.
6. `redes norma informe --formato pdf` → veredicto con fecha, versión del proyecto y hash del code pack.

**Regla de producto:** si el code pack está `BLOCKED_BY_SOURCE` (falta edición, fuente o cláusulas), el producto **no emite veredicto de cumplimiento**: emite "no evaluable" con la lista de campos faltantes (§5.6).
**Éxito:** el veredicto es reproducible: mismo proyecto + mismo pack → mismo resultado y mismo hash.

### R5 — Calcular excavación y cómputos

**Actor:** P1 y P3 · **Duración objetivo:** 15 min.

1. `redes excavacion seccion --plantilla zanja-arena --talud 0.25 --cama_m 0.10 --proteccion_m 0.30` → objeto `SeccionTipo`.
2. `redes excavacion calcular --red "Colector 1" --secciones cada-20m --json` → volumen por tramo por áreas medias.
3. `redes excavacion pozos --json` → volumen de caja por pozo, con sobre-excavación.
4. `redes excavacion resumen` → excavación total, relleno compactado, cama de arena, material de préstamo, material excedente con factor de abundamiento.
5. Cierra el cuadre: `V_excavación = V_relleno + V_excedente` (tolerancia 0,1%).
6. `redes metrado exportar --formato xlsx --plantilla constructora` → cómputos con fórmulas visibles.
7. Si hay medición de campo: `redes excavacion comparar --campo medicion.csv` → error % por tramo y semáforo.

**Artefactos:** cómputo por tramo/pozo, resumen de movimientos de tierra, XLSX.
**Éxito:** 3 tramos de control reproducen el cálculo manual con error ≤ 0,5%; el cuadre cierra.

> **Variante agente IA (R5-IA).** El agente recibe "calcula la excavación del colector y dime si el volumen cuadra". Ejecuta `calcular --json`, verifica `cuadre.ok == true` y `cuadre.desviacion_pct <= 0.1`; si falla, **no** "arregla" números: devuelve `codigo: EXC_CUADRE` con los tramos que no cuadran y pide decisión.

### R6 — Generar perfil longitudinal y planos

**Actor:** P1 · **Duración objetivo:** 10 min para 40 planos.

1. `redes perfil generar --red "Colector 1" --escala_h 1:1000 --escala_v 1:100 --pdf`.
2. Ajusta escala vertical y separación de etiquetas; el perfil muestra terreno, clave, invert, profundidad, pendiente y diámetro por tramo.
3. `redes plano generar --red "Colector 1" --cajetin plantilla-municipal --lote por-tramo`.
4. Revisa el cajetín: proyecto, profesional, escala, fecha y número de plano se llenan del proyecto, no a mano.
5. `redes plano exportar --formato pdf --ruta salida/planos`.

**Éxito:** 40 planos en ≤ 5 min; el plano abre sin fuentes faltantes; las cotas del plano son las del modelo (±0).

### R7 — Exportar a DXF/GeoJSON y memoria de cálculo

**Actor:** P1 · **Duración objetivo:** 5 min.

1. `redes exportar dxf --ruta salida/red.dxf --capas por-familia` → DXF con capas, bloques y textos legibles.
2. Abre en AutoCAD/LibreCAD: 0 reparaciones, 0 entidades en capa 0.
3. `redes exportar geojson --ruta salida/red.geojson` → abre en QGIS con CRS correcto y atributos por familia.
4. `redes informe memoria --formato pdf --incluir-anexos` → memoria con datos, fórmulas, resultados, validación y trazabilidad.
5. `redes informe memoria --formato docx` para que el consultor edite texto no técnico.
6. Verifica que las cifras del PDF coinciden con las de la UI (mismo hash de resultados).

**Éxito:** el expediente es autoconsistente: plano, memoria y cómputo con las mismas cifras y la misma versión.

### R8 — Revisar un proyecto existente / auditar

**Actor:** P2 · **Duración objetivo:** 15 min.

1. `redes proyecto abrir expediente.redes --solo-lectura` → abre congelado, sin tocar el original.
2. `redes auditar --json` → informe de auditoría: versión de esquema, code pack usado, objetos huérfanos, cálculos desactualizados, excepciones sin justificar.
3. Compara la revisión recibida con la anterior: `redes proyecto comparar --con revision-1.redes` → diff por objeto (COL-03).
4. Corre la validación municipal: `redes norma validar --code-pack EC-ALC-MUNICIPAL`.
5. Genera acta de revisión: incumplimientos con cláusula, objeto, captura y acción sugerida.
6. Registra el acto en la bitácora: quién revisó, cuándo, qué versión (COL-04).

**Éxito:** el acta cita objetos concretos, no páginas; la revisión es reproducible por un tercero.

> **Variante agente IA (R8-IA).** El agente audita en lote: recorre 12 expedientes, ejecuta `auditar --json` y `norma validar --json`, y devuelve una tabla con `expediente`, `criticos`, `advertencias`, `no_evaluable_por_pack`. Si un expediente trae un code pack desconocido, lo marca como no evaluable en vez de inventar el criterio.

---

## 3. Backlog priorizado

### 3.1 Reglas del backlog

- **MUST**: sin esto no hay producto vendible; bloquea el release que lo contiene.
- **SHOULD**: duele si falta; se puede lanzar con una solución manual documentada.
- **COULD**: deseable; entra solo si el release tiene holgura.
- **WON'T (v1)**: decidido no hacerlo en v1; queda documentado como limitación.
- **Esfuerzo**: S ≤ 3 días · M ≤ 2 semanas · L ≤ 6 semanas · XL > 6 semanas (1 dev senior, incluye pruebas).
- **ID estable**: se usa en releases, guía, pruebas y en el manifiesto de la CLI. No se renombra.

### 3.2 Matriz de cobertura

| Epic | Prefijo | MUST | SHOULD | COULD | WON'T | Total |
|---|---|---|---|---|---|---|
| Proyecto | PRY | 3 | 2 | 1 | 0 | 6 |
| Topografía | TOP | 3 | 2 | 1 | 0 | 6 |
| Modelo de objetos | MOD | 4 | 2 | 1 | 1 | 8 |
| Red agua potable | AGU | 2 | 3 | 1 | 0 | 6 |
| Red alcantarillado | ALC | 5 | 1 | 1 | 0 | 7 |
| Hidráulica | HID | 3 | 1 | 2 | 1 | 7 |
| Excavación y cómputos | EXC | 4 | 2 | 2 | 0 | 8 |
| Visualización y planos | VIS | 4 | 1 | 1 | 0 | 6 |
| CLI / IA | CLI | 5 | 2 | 0 | 0 | 7 |
| Persistencia | PER | 3 | 1 | 1 | 0 | 5 |
| Colaboración | COL | 0 | 1 | 3 | 0 | 4 |
| Informes | INF | 3 | 1 | 0 | 0 | 4 |
| Extensibilidad | EXT | 1 | 2 | 2 | 0 | 5 |
| i18n | I18N | 2 | 1 | 1 | 0 | 4 |
| Documentación y ayuda | DOC | 4 | 1 | 0 | 0 | 5 |
| **Total** | | **46** | **23** | **17** | **2** | **88** |

### 3.3 Proyecto (PRY)

| ID | Ítem | Valor en una línea | Aceptación (verificable) | Prior. | Esf. |
|---|---|---|---|---|---|
| PRY-01 | Asistente de proyecto nuevo | Arranca bien desde el primer minuto, con datum y unidades correctos | · Crea `.redes` con nombre, datum, zona UTM, unidades y code pack<br>· Valida que datum y zona sean compatibles con un par de coordenadas de ejemplo<br>· El proyecto aparece en "Recientes" al cerrar y reabrir | MUST | S |
| PRY-02 | Abrir/guardar proyecto y compatibilidad de versiones | Un proyecto de hace un año sigue abriendo | · Round-trip guardar→abrir conserva el 100% de objetos (hash de modelo idéntico)<br>· Abre proyectos de esquema anterior aplicando migración y lo informa<br>· Si falta una familia, muestra qué plugin instalar y no corrompe el archivo | MUST | S |
| PRY-03 | Parámetros de diseño del proyecto | Población, dotación, aporte y periodo en un solo lugar, usados por todos los cálculos | · Formulario con rangos y unidades visibles; rechaza valores fuera de rango con mensaje que dice el rango<br>· HID y ALC consumen estos valores, no copias locales<br>· Los parámetros quedan impresos en la memoria de cálculo | MUST | S |
| PRY-04 | Escenarios y alternativas | Comparar "todo gravedad" vs "con estación de bombeo" sin duplicar archivos a mano | · Duplica un proyecto con nombre nuevo en ≤ 5 s<br>· Compara dos escenarios lado a lado: Δlongitud, Δvolumen, Δcosto<br>· Marca el escenario activo en UI y en `--json` | SHOULD | M |
| PRY-05 | Historial de cambios con deshacer | Se puede volver atrás sin miedo a romper el modelo | · 50 pasos de deshacer/rehacer por sesión<br>· Cada entrada registra objeto, hora y usuario<br>· El historial se exporta a CSV | SHOULD | M |
| PRY-06 | Plantillas de proyecto por municipio | Un proyecto nuevo nace ya configurado para la EMAP de turno | · La plantilla guarda code pack, simbología, cajetín y plantilla de informes<br>· Se aplica a un proyecto nuevo en un clic<br>· La plantilla declara su versión y avisa si el code pack cambió | COULD | M |

### 3.4 Topografía (TOP)

| ID | Ítem | Valor en una línea | Aceptación (verificable) | Prior. | Esf. |
|---|---|---|---|---|---|
| TOP-01 | Importar levantamiento (DXF/CSV/TXT) | Pasar del plano topográfico al modelo sin digitar coordenadas | · Lee puntos con cota desde capa y desde texto/label, y curvas de nivel<br>· Informa leídos / descartados con motivo por punto<br>· 2 000 puntos importados en ≤ 20 s en equipo de referencia | MUST | M |
| TOP-02 | MDT/TIN a partir de puntos y curvas | Cotas consultables en cualquier punto del terreno | · Genera TIN y lo dibuja con sombreado<br>· Error vertical medio ≤ 0,05 m en los puntos de control<br>· Consulta de cota en cualquier xy, con aviso fuera del área del TIN | MUST | L |
| TOP-03 | CRS y datum (WGS84 / PSAD56, UTM 17S/18S) | Un proyecto no mezcla datums por accidente | · Soporta EPSG:32717, 32718, 24877, 24878<br>· Al transformar, registra el parámetro usado y el residuo<br>· Mezclar datums sin confirmación explícita es un error bloqueante | MUST | M |
| TOP-04 | Curvas de nivel, pendientes y sombreado | Leer el terreno de un vistazo para decidir el trazado | · Mapa de pendientes por rangos configurables<br>· Curvas maestras cada 1 m y secundarias cada 0,25 m, configurables<br>· Exporta curvas generadas a DXF | SHOULD | M |
| TOP-05 | Secciones transversales del terreno | Ver qué hay debajo de la zanja antes de excavar | · Sección en cualquier abscisa de un tramo<br>· N secciones equidistantes por tramo (N configurable)<br>· Exporta a CSV con distancia y cota | SHOULD | M |
| TOP-06 | Depuración de puntos atípicos | Evitar que un punto mal tomado arruine el MDT | · Detecta cotas fuera de ±3σ y las marca en el mapa<br>· Eliminar/restaurar punto no altera el TIN hasta confirmar<br>· Muestra cuántos puntos quedan y el rango de cotas | COULD | S |

### 3.5 Modelo de objetos (MOD)

| ID | Ítem | Valor en una línea | Aceptación (verificable) | Prior. | Esf. |
|---|---|---|---|---|---|
| MOD-01 | Registro de familias de objetos | Agregar pozos, válvulas o vertederos sin rehacer la app | · El registro expone por familia: nombre, esquema de atributos, unidades, icono y reglas<br>· La UI solo muestra familias activas en el proyecto<br>· Una familia inválida se rechaza con mensaje y no tumba la app | MUST | L |
| MOD-02 | Familia Pozo y subtipos | Pozos de revisión, caída, caja y sumidero con sus propios campos | · Cada subtipo tiene campos obligatorios propios (cota tapa, invert, diámetro, material, caída)<br>· Valida coherencia (invert ≤ tapa; caída ≥ 0)<br>· El subtipo se refleja en simbología y en listados | MUST | M |
| MOD-03 | Familia Tubería / Colector | Material, diámetro, rugosidad y clase en un solo objeto | · Calcula longitud y pendiente desde la geometría<br>· Cambiar diámetro recalcula hidráulica del tramo y de aguas abajo<br>· Se listan solo materiales y diámetros del catálogo activo | MUST | M |
| MOD-04 | Familias Válvula, Llave, Hidrante, Medidor | Los accesorios de agua potable dejan de ser bloques sueltos de CAD | · Insertables sobre tubería con snap y orientación<br>· Campos: tipo, diámetro, estado (abierta/cerrada), propietario<br>· Aparecen en listados, planos y cómputo de accesorios | SHOULD | M |
| MOD-05 | Familias Vertedero, Descarga y PTAR | El final del alcantarillado existe en el modelo, no solo en el plano | · Punto de descarga con cota, cuerpo receptor y caudal<br>· Valida que cada red tenga al menos una descarga o PTAR<br>· La descarga aparece en el perfil y en la memoria | SHOULD | M |
| MOD-06 | Conexiones domiciliarias y acometidas | Contar y cotizar las conexiones sin contarlas a mano | · N conexiones por tramo con cota, diámetro y lado<br>· Conteo por manzana y por tramo para cómputo<br>· Se dibujan y se listan con identificador único | COULD | L |
| MOD-07 | Validación estructural del modelo | Detecta el desorden antes de que se vuelva un número mal | · `redes modelo validar` detecta IDs duplicados, objetos huérfanos, referencias rotas y unidades incoherentes<br>· Mensaje por objeto con acción sugerida<br>· 0 errores en el proyecto de ejemplo | MUST | S |
| MOD-08 | Familias definidas por el usuario (sin recompilar) | Un municipio agrega "cámara de bombeo" sin esperar un release | · Se declara en manifiesto (atributos, unidades, reglas) y se carga en runtime<br>· Un error en la familia ajena queda aislado y reportado<br>· La familia nueva aparece en CLI y en UI sin cambios de código | WON'T | — |

> MOD-08 se difiere a v1.1 deliberadamente: la extensibilidad real (EXT-01/02) llega antes que las familias de terceros. Se documenta como limitación.

### 3.6 Red de agua potable (AGU)

| ID | Ítem | Valor en una línea | Aceptación (verificable) | Prior. | Esf. |
|---|---|---|---|---|---|
| AGU-01 | Trazado de red (nodos y tramos) | Dibujar la red como se dibuja en CAD, pero con datos | · Clic crea nodo con snap a MDT y a tubería; el tramo une dos nodos<br>· Longitud y pendiente se muestran al instante y se guardan<br>· 500 objetos con pan/zoom fluido (≥ 30 fps en equipo de referencia) | MUST | L |
| AGU-02 | Caudales de diseño | Qmd, Qmh y Qi calculados con fórmula visible | · Aplica dotación, población, coeficientes y aporte desde PRY-03<br>· Muestra fórmula, sustitución y resultado con unidades<br>· CLI y UI producen el mismo número para el mismo proyecto (mismo hash) | MUST | M |
| AGU-03 | Dimensionamiento asistido de diámetros | Elegir diámetro comercial sin iterar a mano | · Propone el diámetro comercial menor que cumple v y pérdida del pack<br>· Muestra al menos 2 alternativas con Δcosto y Δpérdida<br>· Registrar la elección deja rastro en la memoria | SHOULD | M |
| AGU-04 | Sectorización y válvulas de cierre | Saber qué queda sin agua al cerrar una válvula | · Sector por polígono o por conjunto de tramos<br>· Lista válvulas por sector y tramos afectados al cerrar una<br>· Simulación de cierre: qué nodos quedan sin servicio | COULD | M |
| AGU-05 | Presiones, velocidad y golpe de ariete | Verificar que la red no revienta ni deja sin presión | · Presiones por nodo vs rango del pack, con semáforo<br>· Alerta si la velocidad sale del rango<br>· Estima celeridad y sobrepresión por cierre (informativo) | SHOULD | L |
| AGU-06 | Trazado por lotes (CSV de nodos y tramos) | Cargar una red ya definida en tabla, sin dibujar | · Importa CSV con validación por fila y reporte de errores<br>· Crea 100 tramos en una sola operación<br>· Una fila mala no cancela las buenas: se informa y continúa | SHOULD | M |

### 3.7 Red de alcantarillado (ALC)

| ID | Ítem | Valor en una línea | Aceptación (verificable) | Prior. | Esf. |
|---|---|---|---|---|---|
| ALC-01 | Pozos con cotas de tapa, terreno e invert | El pozo es un objeto con física, no un bloque | · Cota tapa, cota terreno y profundidad; invert derivado de la pendiente<br>· Alerta si la profundidad excede el máximo del pack<br>· El pozo se dibuja en planta y en perfil con las mismas cotas | MUST | M |
| ALC-02 | Colector entre pozos con pendiente | Trazar el colector y ver la pendiente mientras se dibuja | · Pendiente por cotas o por porcentaje, con la otra derivada<br>· Sentido de flujo explícito y visible<br>· Cambiar la pendiente recalcula aguas abajo y avisa del impacto | MUST | L |
| ALC-03 | Pozos de caída y transiciones | Resolver los desniveles fuertes sin inventar detalle | · Caída con altura y tipo (externa/interna)<br>· Valida caída máxima del pack y avisa<br>· Se dibuja en perfil con la cota correcta de cada colector | SHOULD | M |
| ALC-04 | Acometidas y sumideros | Cerrar el modelo hasta la última conexión | · Acometida con cota, diámetro y lado; sumidero con rejilla y cota<br>· Conteo por tramo y por manzana<br>· Se listan y se dibujan con ID único | COULD | L |
| ALC-05 | Capacidad y relación de llenado (Y/D) | Saber si el tubo se llena antes de que lo diga el fiscalizador | · Calcula Q, v y Y/D por Manning con sección parcialmente llena<br>· Marca tramos fuera de rango del pack con motivo<br>· 0 falsos positivos en los 20 tramos del proyecto de ejemplo | MUST | M |
| ALC-06 | Pendientes mínimas y autolimpieza | Que el colector no se azolve a los dos años | · Regla configurable (v mínima, τ mínimo, pendiente mínima por diámetro)<br>· Lista de tramos incumplidos exportable a CSV<br>· Cada excepción exige justificación escrita | MUST | M |
| ALC-07 | Validación topológica de la red | Errores de conexión detectados antes del cálculo | · Detecta pozos sin salida, ciclos, cotas invertidas y tramos sin descarga<br>· Bloquea cómputo e informes si hay errores críticos<br>· Cada error lleva al objeto en un clic | MUST | S |

### 3.8 Hidráulica (HID)

| ID | Ítem | Valor en una línea | Aceptación (verificable) | Prior. | Esf. |
|---|---|---|---|---|---|
| HID-01 | Motor de agua a presión | Resultados que aguantan una revisión técnica | · Hazen-Williams y Darcy-Weisbach seleccionables<br>· Diferencia ≤ 1% frente a EPANET en 3 casos de prueba versionados<br>· Informa convergencia, iteraciones y residual | MUST | L |
| HID-02 | Motor de alcantarillado (Manning, flujo parcial) | El cálculo de siempre, pero reproducible | · Diferencia ≤ 0,5% frente a planilla de referencia en 10 tramos<br>· Casos de prueba con datos de entrada y salida esperada en el repo<br>· Un cambio de rugosidad se refleja en todos los tramos afectados | MUST | L |
| HID-03 | Validación contra code pack | "Cumple / no cumple" con cláusula y severidad, no con opinión | · Cada check tiene `id`, `clausula`, `severidad`, `objeto`, `valor`, `limite`<br>· Ejecutable en UI y CLI con el mismo resultado<br>· Si el pack está incompleto, el veredicto es "no evaluable" con campos faltantes | MUST | L |
| HID-04 | Memoria hidráulica | El revisor ve el cálculo, no solo el resultado | · PDF con datos, fórmulas, resultados y veredicto<br>· Declara versión del proyecto, del motor y hash del code pack<br>· Mismas cifras que la UI (verificado por hash) | SHOULD | M |
| HID-05 | Análisis de sensibilidad de demanda | Saber si el diseño aguanta el crecimiento | · Varía dotación y población ±20% en pasos configurables<br>· Tabla comparativa de presiones y velocidades<br>· Exportable a CSV | COULD | M |
| HID-06 | Bombeo y curva de bomba | Resolver el caso donde la gravedad no alcanza | · Curva de bomba por puntos y punto de operación<br>· Verifica NPSH y potencia estimada<br>· El resultado entra en la memoria | COULD | L |
| HID-07 | Transitorios y golpe de ariete | Cálculo dinámico completo | · No se implementa en v1; AGU-05 da una estimación informativa<br>· Se documenta como limitación con recomendación de software especializado | WON'T | — |

### 3.9 Excavación y cómputos (EXC)

| ID | Ítem | Valor en una línea | Aceptación (verificable) | Prior. | Esf. |
|---|---|---|---|---|---|
| EXC-01 | Sección tipo de zanja | Definir una vez cómo se excava y aplicarlo a toda la red | · Parámetros: ancho de fondo, talud por tipo de suelo, cama, protección, sobre-excavación<br>· Ancho calculado en función de diámetro y profundidad<br>· Vista de la sección con cotas y materiales | MUST | M |
| EXC-02 | Excavación por tramo (áreas medias) | Volumen de zanja que coincide con el de campo | · Volumen por tramo con estaciones (cada 20 m o configurable)<br>· Suma de tramos = total, sin diferencias de redondeo > 0,1%<br>· 3 tramos de control reproducen el cálculo manual con error ≤ 0,5% | MUST | L |
| EXC-03 | Excavación de pozos (caja) | El pozo también se excava y también se paga | · Volumen por pozo con sobre-excavación y talud<br>· Listado por pozo y total<br>· Un pozo más profundo cambia su volumen y el total | MUST | M |
| EXC-04 | Relleno, cama, préstamo, excedente y abundamiento | Cerrar el balance de tierras sin planillas paralelas | · Separa relleno compactado, cama de arena, material de préstamo y excedente<br>· Aplica factor de abundamiento editable y lo imprime en el informe<br>· `V_exc = V_relleno + V_excedente` con tolerancia 0,1% | MUST | M |
| EXC-05 | Cómputos de tubería, hormigón, acero y encofrado | Del volumen al ítem de presupuesto | · Metrado por ítem y por tramo/pozo<br>· Tubería agrupada por diámetro y material<br>· Exportable a XLSX con fórmulas visibles | SHOULD | M |
| EXC-06 | Transporte y distancia media (m³·km) | Costear el acarreo de material excedente | · Distancia media por ruta configurable (botadero)<br>· Volumen × distancia por tramo y total<br>· Se reporta junto al excedente | COULD | S |
| EXC-07 | Comparación calculado vs campo | Discutir con el fiscalizador con datos, no con criterio | · Importa CSV de medición de campo por tramo<br>· Error % por tramo con semáforo (> 5% rojo)<br>· Exporta el comparativo a XLSX | SHOULD | M |
| EXC-08 | Movimiento de tierras por diferencia de MDT | Volumen general de explanación, no solo zanja | · Volumen corte/relleno entre dos superficies<br>· Mapa de diferencias con escala de color<br>· Exportable a CSV | COULD | L |

### 3.10 Visualización y planos (VIS)

| ID | Ítem | Valor en una línea | Aceptación (verificable) | Prior. | Esf. |
|---|---|---|---|---|---|
| VIS-01 | Vista de planta con simbología normada | El plano se ve como el municipio espera verlo | · Capas, colores, grosores y etiquetas configurables<br>· Simbología por familia y subtipo, editable<br>· Etiquetas que no se superponen (separación automática) | MUST | L |
| VIS-02 | Perfil longitudinal | El dibujo que hoy consume el 60% del tiempo, generado | · Terreno, clave, invert, profundidad, pendiente y diámetro por tramo<br>· Escalas H y V independientes (1:1000 / 1:100 por defecto)<br>· Etiquetas de pozo y de progresiva legibles a la escala elegida | MUST | L |
| VIS-03 | Secciones transversales de zanja | Mostrar cómo se excava, no solo cuánto | · Sección por tramo con materiales y cotas<br>· Exportable a PDF y DXF<br>· Coherente con EXC-01 (mismos parámetros) | SHOULD | M |
| VIS-04 | Planos con cajetín y lotes | Entregar 40 planos sin armarlos uno por uno | · Cajetín con proyecto, profesional, escala, fecha y número de plano<br>· Lote por tramo o por sector, con índice de planos<br>· PDF con fuentes incrustadas; abre sin fuentes faltantes | MUST | L |
| VIS-05 | Exportar DXF | Que el plano siga siendo editable en CAD | · Capas por familia, bloques por subtipo, textos legibles<br>· Abre en AutoCAD y LibreCAD sin reparar; 0 entidades en capa 0<br>· El DXF exportado se reimporta al modelo sin pérdida de atributos clave | MUST | L |
| VIS-06 | Vista 3D del modelo | Entender la red en perspectiva para presentar | · Red y terreno en 3D con navegación<br>· Colores por diámetro o por pendiente<br>· Captura PNG para presentaciones | COULD | L |

### 3.11 CLI / IA (CLI)

| ID | Ítem | Valor en una línea | Aceptación (verificable) | Prior. | Esf. |
|---|---|---|---|---|---|
| CLI-01 | Paridad de capacidades UI ↔ CLI | Todo lo que se hace con el mouse se puede automatizar | · Matriz de capacidades con 100% de operaciones de creación, edición y cálculo cubiertas<br>· Prueba de CI que falla si una capacidad no tiene comando<br>· Cada comando de la matriz tiene al menos un ejemplo ejecutado en CI | MUST | XL |
| CLI-02 | Salida `--json` estable y versionada | Un agente puede parsear sin adivinar | · `--json` disponible en el 100% de los comandos<br>· Esquema publicado y validado con JSON Schema en CI<br>· Todo valor con unidad lleva la unidad en el nombre (`diametro_mm`, `longitud_m`) | MUST | L |
| CLI-03 | Manifiesto autodescriptivo (`redes esquema`) | El agente descubre la API sin leer la guía | · Devuelve comandos, parámetros, tipos, unidades, obligatoriedad, ejemplos y si es lectura o escritura<br>· Se genera del registro en runtime, no de una lista copiada a mano<br>· Un agente puede encadenar 12 comandos solo con el manifiesto (prueba de aceptación) | MUST | M |
| CLI-04 | `--dry-run` e idempotencia | Reintentar no duplica pozos | · `--dry-run` no escribe nada (verificado por hash del archivo)<br>· Repetir un comando de creación con la misma clave natural no duplica<br>· Los comandos declaran su clave natural en el manifiesto | MUST | M |
| CLI-05 | Códigos de salida y errores estructurados | El agente sabe qué hacer cuando algo falla | · Catálogo de códigos documentado (0 ok, 2 validación, 3 norma, 4 E/S, 5 topografía, 6 conflicto de versión)<br>· Error con `codigo`, `mensaje`, `objeto`, `sugerencia`; nunca stacktrace crudo<br>· Cada código tiene un ejemplo en la guía CLI | MUST | S |
| CLI-06 | Servidor MCP | Conectar el producto a agentes que no usan terminal | · Expone 12 tools con descripción para LLM, unidades declaradas y marca lectura/escritura<br>· Las tools de escritura exigen confirmación explícita del cliente MCP<br>· Prueba de contrato: el manifiesto MCP coincide con `redes esquema` | SHOULD | L |
| CLI-07 | Modo lote y CI | Diseñar y validar en pipeline, sin abrir la app | · Ejecuta un archivo de guion con N comandos y se detiene en el primer error crítico<br>· Sale con código ≠ 0 si la validación normativa falla<br>· Devuelve un resumen JSON con comandos ejecutados y duración | SHOULD | M |

### 3.12 Persistencia (PER)

| ID | Ítem | Valor en una línea | Aceptación (verificable) | Prior. | Esf. |
|---|---|---|---|---|---|
| PER-01 | Formato de proyecto único y versionado | Un archivo, no una carpeta de archivos sueltos | · Contenedor SQLite embebido con `schema_version` y migraciones automáticas<br>· 5 000 objetos en un archivo ≤ 50 MB y apertura ≤ 3 s<br>· El archivo es legible por la CLI sin la app abierta | MUST | L |
| PER-02 | Guardado automático y recuperación | Un corte de luz no cuesta una tarde | · Autoguardado cada 30 s y antes de cada cálculo largo<br>· Tras matar el proceso, la recuperación pierde ≤ 1 min de trabajo<br>· El aviso de recuperación dice qué se recuperó y desde cuándo | MUST | M |
| PER-03 | Integridad y respaldo | Saber que el archivo no está roto antes de enviarlo | · Verificación de checksum al abrir; si falla, ofrece el `.bak` más reciente<br>· Respaldo rotativo de 5 versiones<br>· `redes proyecto verificar --json` devuelve estado y fecha del último respaldo | SHOULD | S |
| PER-04 | GeoJSON de ida y vuelta | Compartir con QGIS y con el SIG municipal | · Coordenadas en el CRS del proyecto, declarado en el archivo<br>· Propiedades por familia (atributos y unidades)<br>· Abre en QGIS sin errores de geometría ni de CRS | MUST | M |
| PER-05 | Importar de otras herramientas (CSV/LandXML) | Traer proyectos existentes sin redibujar | · Importa CSV de pozos y tramos y LandXML de tuberías cuando exista<br>· Reporte de filas importadas y rechazadas<br>· Mapeo de columnas guardable como perfil | COULD | L |

### 3.13 Colaboración (COL)

| ID | Ítem | Valor en una línea | Aceptación (verificable) | Prior. | Esf. |
|---|---|---|---|---|---|
| COL-01 | Bloqueo de proyecto | Dos personas no se pisan el archivo | · Al abrir, avisa si el archivo está en uso y ofrece solo lectura<br>· El bloqueo se libera al cerrar o tras 15 min de inactividad<br>· El mensaje dice quién lo tiene y desde cuándo | COULD | M |
| COL-02 | Comentarios y marcas en el modelo | Observar sobre el objeto, no sobre una página | · Comentario anclado a un objeto con autor y fecha<br>· Se puede resolver y queda en el historial<br>· Aparece en el acta de revisión | COULD | M |
| COL-03 | Comparar revisiones | Ver qué cambió entre la v1 y la v2 del expediente | · Diff por objeto: agregado, modificado, eliminado<br>· Tabla de cambios con valores antes/después<br>· Exportable a PDF como anexo del acta | COULD | L |
| COL-04 | Bitácora de auditoría | Quién cambió qué, cuándo y con qué versión | · Registro inmutable de apertura, cálculo, validación, exportación y cambio de code pack<br>· Filtrable por usuario, fecha y objeto<br>· Exportable a CSV/PDF para el expediente municipal | SHOULD | M |

### 3.14 Informes (INF)

| ID | Ítem | Valor en una línea | Aceptación (verificable) | Prior. | Esf. |
|---|---|---|---|---|---|
| INF-01 | Memoria de cálculo (PDF/DOCX) | El documento que se firma, generado del modelo | · Incluye datos de entrada, fórmulas, resultados, validación y anexos<br>· Declara proyecto, versión, fecha, profesional y hash del code pack<br>· Regenerar la memoria del mismo proyecto produce el mismo hash de cifras | MUST | L |
| INF-02 | Cómputos y presupuesto (XLSX) | Del metrado al presupuesto sin reescribir | · Hojas por ítem con fórmulas visibles (no valores pegados)<br>· Totales cuadran con EXC (tolerancia 0,1%)<br>· Estructura de ítems configurable por plantilla | MUST | L |
| INF-03 | Listados técnicos | El anexo que el revisor siempre pide | · Listados de pozos, tramos, válvulas y descargas con columnas configurables<br>· Exportación CSV/XLSX en un clic<br>· Los listados respetan filtros activos de la vista | MUST | S |
| INF-04 | Plantillas de informe por municipio | Cada EMAP con su formato, sin programar | · Plantilla con portada, orden de secciones y cajetín propios<br>· Se aplica al generar sin editar el documento<br>· La plantilla declara su versión y avisa si el esquema cambió | SHOULD | M |

### 3.15 Extensibilidad (EXT)

| ID | Ítem | Valor en una línea | Aceptación (verificable) | Prior. | Esf. |
|---|---|---|---|---|---|
| EXT-01 | API de familias extensible | El producto crece por familias, no por parches | · Una familia se declara en manifiesto (atributos, unidades, reglas, icono) y se carga en runtime<br>· Un manifiesto inválido se rechaza con mensaje claro y no afecta al resto<br>· Cargar una familia nueva no requiere recompilar ni reiniciar la app | MUST | L |
| EXT-02 | Contrato de cálculo por familia | Una familia nueva puede calcular, no solo dibujar | · La familia declara su cálculo hidráulico y de excavación mediante el contrato publicado<br>· Suite de conformidad que la familia debe pasar para habilitarse<br>· Una familia sin cálculo válido se marca "solo dibujo" y no entra en cómputos | SHOULD | L |
| EXT-03 | SDK y ejemplo "familia vertedero" | Un desarrollador externo arranca en una tarde | · El ejemplo compila y pasa la suite de conformidad<br>· Documento de 2 páginas: del manifiesto a la app<br>· El ejemplo está versionado y se prueba en CI | SHOULD | M |
| EXT-04 | Plugins firmados y aislados | Un plugin de terceros no puede romper el proyecto | · Firma verificada al cargar; sin firma, se pide confirmación explícita<br>· Un fallo del plugin se contiene y se reporta sin cerrar la app<br>· El plugin declara qué capacidades usa | COULD | L |
| EXT-05 | Gestor de extensiones en la UI | Instalar y actualizar sin tocar archivos | · Lista instaladas con versión y estado<br>· Instala desde archivo y valida compatibilidad con la versión de la app<br>· Desinstalar deja el proyecto intacto | COULD | M |

### 3.16 i18n (I18N)

| ID | Ítem | Valor en una línea | Aceptación (verificable) | Prior. | Esf. |
|---|---|---|---|---|---|
| I18N-01 | UI en español (es-EC) | El producto habla el idioma del cliente | · 100% de cadenas externalizadas; prueba de CI que falla si hay texto incrustado<br>· Revisión por un hablante nativo con lista de correcciones cerrada<br>· Terminología consistente con el glosario (DOC-05) | MUST | M |
| I18N-02 | Formatos numéricos y de unidades locales | 1.234,56 se lee como mil doscientos, no como uno | · Entrada acepta coma y punto decimal según locale<br>· Exportaciones respetan el separador configurado<br>· Los números de la UI y de los informes coinciden | MUST | S |
| I18N-03 | Inglés (en-US) para UI y CLI | Un agente o consultor extranjero puede operar | · Traducción de UI y mensajes de CLI al 100% de las cadenas de usuario<br>· El manifiesto CLI expone descripciones en ambos idiomas<br>· Cambiar idioma no cambia los datos ni los números | COULD | M |
| I18N-04 | Glosario aplicado y verificado | Los mismos términos en toda la app y la guía | · Lint de terminología sobre cadenas y documentos<br>· El glosario ES/EN está enlazado desde la app<br>· 0 sinónimos prohibidos en la UI (p. ej. "tubería" vs "tubo" para el mismo objeto) | SHOULD | S |

### 3.17 Documentación y ayuda (DOC)

| ID | Ítem | Valor en una línea | Aceptación (verificable) | Prior. | Esf. |
|---|---|---|---|---|---|
| DOC-01 | Guía de inicio rápido (15 min) | El usuario llega a su primera red sin ayuda | · 3 usuarios nuevos completan el recorrido en ≤ 15 min cronometrados<br>· ≤ 8 páginas; un paso por pantalla, sin texto decorativo<br>· 0 preguntas al soporte durante la prueba | MUST | M |
| DOC-02 | Manual de usuario por tarea | Resuelve la tarea concreta sin leer 200 páginas | · Cubre el 100% de los ítems MUST con una página por tarea<br>· Cada tarea incluye "si algo falla" con 2 causas típicas<br>· Cada captura se regenera con la versión vigente (CI de capturas) | MUST | L |
| DOC-03 | Referencia de cálculos con ejemplos resueltos | El revisor puede verificar el número a mano | · Cada fórmula con sustitución numérica y resultado<br>· Un revisor externo reproduce 3 ejemplos sin ayuda<br>· Cada ejemplo declara unidades y tolerancia | MUST | L |
| DOC-04 | Guía de la CLI para agentes | Un agente opera el producto leyendo una sola página | · Cada comando con ejemplo copiable y salida esperada<br>· CI ejecuta todos los ejemplos y falla si uno deja de funcionar<br>· Incluye recetas de 3 flujos completos y catálogo de errores | MUST | M |
| DOC-05 | Referencia de familias, FAQ y glosario | La respuesta está donde se busca | · Una ficha por familia con atributos, unidades y reglas<br>· FAQ con las 20 preguntas reales del piloto<br>· Glosario ES/EN con definición y sinónimos prohibidos | SHOULD | M |

---

## 4. Plan de releases

Regla de oro: **cada release se demuestra con un recorrido completo, no con una lista de features.** Si la demo necesita explicación, el release no está listo.

### 4.1 v0.1 — Esqueleto caminante (semanas 1–8)

**Qué entra:** PRY-01, PRY-02, PRY-03 · TOP-01, TOP-02, TOP-03 · MOD-01, MOD-02, MOD-03, MOD-07 · ALC-01, ALC-02 · EXC-01, EXC-02 · VIS-02 (solo perfil PDF) · CLI-01 (6 comandos), CLI-02, CLI-04 · PER-01 · I18N-01, I18N-02 · DOC-01 (borrador).

**Demo (20 min, en vivo, sin trampas):** importar un DXF de 300 puntos → validar modelo → crear 6 pozos y 5 colectores con pendiente → calcular excavación por áreas medias → generar el perfil longitudinal en PDF → reabrir el proyecto y comprobar que todo está igual.

**Criterio de salida:** la cadena topografía → pozos → colector → excavación → perfil se ejecuta dos veces seguidas con el mismo resultado; el archivo reabre sin pérdida; la CLI produce el mismo volumen que la UI para los 5 tramos (diferencia 0).

**Lo que todavía no se puede prometer:** agua potable, planos de planta, hidráulica, DXF de salida, memoria de cálculo. La guía lo dice explícitamente en la primera página.

### 4.2 v0.2 — Diseño de alcantarillado completo (semanas 9–18)

**Qué entra:** ALC-03, ALC-05, ALC-06, ALC-07 · HID-02, HID-03 · EXC-03, EXC-04, EXC-05 · VIS-01, VIS-04, VIS-05 · CLI-03, CLI-05 · PER-02, PER-04 · INF-01, INF-03 · DOC-02 (primer bloque).

**Demo (25 min):** proyecto de 800 m con 12 pozos y pozo de caída → validación normativa con 3 incumplimientos provocados a propósito → corrección en vivo y recálculo aguas abajo → cómputo con abundamiento y cuadre de tierras → planos PDF con cajetín → DXF que se abre en AutoCAD sin reparar → memoria de cálculo con las mismas cifras.

**Criterio de salida:** validar contra el code pack municipal piloto produce un veredicto reproducible (mismo hash en 2 ejecuciones y en CLI); DXF abre sin reparaciones en 2 CAD distintos; el cuadre de tierras cierra con tolerancia 0,1%.

### 4.3 v0.3 — Agua potable, extensibilidad y agentes (semanas 19–30)

**Qué entra:** AGU-01…AGU-06 · HID-01, HID-04 · EXC-06, EXC-07 · VIS-03 · CLI-06, CLI-07 · PER-03 · COL-04 · EXT-01, EXT-02, EXT-03 · INF-02, INF-04 · I18N-04 · DOC-03, DOC-04, DOC-05.

**Demo (30 min):** red de agua potable de 1,5 km con diámetros dimensionados y presiones validadas → simultáneamente, un agente IA conectado por MCP ejecuta la auditoría de un expediente y devuelve la tabla de incumplimientos → se instala una familia nueva (vertedero) sin recompilar y se usa en el modelo.

**Criterio de salida:** un agente completa R8-IA sin intervención humana y con `exit 0`; instalar la familia de ejemplo no requiere reiniciar; el 100% de los comandos del manifiesto tiene ejemplo ejecutado en CI.

### 4.4 v1.0 — Producto vendible (semanas 31–40)

**Qué entra:** lo que queda de SHOULD (PRY-04, PRY-05, TOP-04, TOP-05, MOD-04, MOD-05, AGU-05, ALC-04, HID-04, INF-04, EXT-04/05 si hay holgura) + endurecimiento: rendimiento con 5 000 objetos, instalador MSI firmado, licenciamiento offline, manual completo, guía de arquitectura, y el paquete de migración de proyectos piloto.

**Demo (40 min, con el cliente):** un proyecto real del cliente, de principio a fin, con su code pack y su cajetín, ejecutado por el ingeniero del cliente —no por el equipo— mientras el equipo solo observa.

**Criterio de salida (Definition of Done de v1.0):** 3 proyectos reales entregados con la app; ≥ 80% del tutorial completado sin ayuda; 0 defectos críticos abiertos; guía completa publicada y versionada; CLI con paridad verificada por CI.

### 4.5 Trazabilidad release ↔ riesgo

| Release | Riesgo principal que ataca | Si falla, qué se pierde |
|---|---|---|
| v0.1 | Que el modelo de objetos y la excavación no sean creíbles | Se pierde la razón de compra: el cómputo |
| v0.2 | Que la validación normativa no sea defendible | Se pierde al técnico municipal (P2) |
| v0.3 | Que la CLI no sea usable por agentes | Se pierde la diferenciación y el caso de automatización |
| v1.0 | Que el cliente no pueda entregar un expediente real | Se pierde la venta |

---

## 5. La guía: arquitectura documental

### 5.1 Principios editoriales

1. **La guía es parte del producto.** Se versiona con la app (`Guía v1.0.3`), vive en el repositorio (`/docs/guia`) y se publica en HTML y PDF. Cada página declara a qué versión aplica.
2. **Una tarea, una página.** El manual no se lee de corrido: se busca "cómo trazo un pozo de caída" y se encuentra una página que resuelve eso.
3. **Los ejemplos se ejecutan.** Todo bloque de CLI de la guía corre en CI. Si un comando cambia, la guía rompe el build antes de mentirle al usuario.
4. **Las capturas se regeneran.** CI compara capturas contra la app vigente; una captura con un botón que ya no existe es un defecto con ticket.
5. **Los números se pueden verificar a mano.** La referencia de cálculos muestra la sustitución, no solo la fórmula, con las mismas cifras que produce la app.
6. **Español técnico, no traducción.** Se dice "pozo de revisión", "colector", "rasante", "cota de invert", "metrado". Nunca "manhole", "pipe" ni "layout".
7. **Tres niveles de profundidad por página:** qué hace (1 frase) → cómo se hace (pasos) → por qué funciona así (enlace a la referencia de cálculos o a la arquitectura).
8. **Toda página termina en "si algo falla"** con las dos causas más frecuentes y el mensaje exacto que verá el usuario.

### 5.2 Mapa de la documentación

| # | Documento | Responde a | Cuándo se usa | Extensión |
|---|---|---|---|---|
| 1 | Guía de inicio rápido | "¿Cómo empiezo?" | Primer día, 15 min | 8 págs |
| 2 | Manual de usuario por tarea | "¿Cómo hago X?" | Trabajo diario | 60–80 págs |
| 3 | Referencia de familias de objetos | "¿Qué es un pozo de caída y qué campos tiene?" | Diseño y revisión | 30 págs |
| 4 | Referencia de cálculos | "¿De dónde sale este número?" | Revisión técnica y auditoría | 40 págs |
| 5 | Guía de la CLI para agentes | "¿Cómo automatizo esto?" | Integración y agentes IA | 25 págs |
| 6 | Guía de normas y parámetros (code packs) | "¿Qué norma aplica y qué parámetros usa?" | Configuración municipal | 20 págs |
| 7 | Arquitectura para desarrolladores | "¿Cómo extiendo o mantengo esto?" | Desarrollo y plugins | 35 págs |
| 8 | FAQ y solución de problemas | "Algo no funciona" | Soporte | 15 págs |
| 9 | Glosario ES/EN | "¿Qué significa este término?" | Transversal | 10 págs |

**Rutas de lectura sugeridas:** P1 → 1, 2, 3, 4. P2 → 1, 6, 4, 8. P3 → 1, 2 (secciones de cómputo), 4. P4 → 5, 3, 8. Desarrollador → 7, 3, 6.

### 5.3 Documento 1 — Guía de inicio rápido (15 minutos)

**Propósito.** Llevar a un ingeniero que nunca vio la app hasta un proyecto con topografía, tres pozos, dos colectores, un volumen de excavación y un perfil en PDF, en una sola sesión y sin ayuda.
**Audiencia.** P1, P2 y P3 en su primer día. Requisito: saber leer un plano topográfico.
**Contrato de la guía.** 7 pasos, 1 objetivo por paso, ≤ 8 páginas, sin teoría. Si el usuario se atasca, la guía falló (DOC-01).

**Tabla de contenido (2 niveles):**

1. Antes de empezar
   1.1 Requisitos, instalación y licencia
   1.2 Qué necesitas del levantamiento
   1.3 Convenciones de esta guía
2. Tu primer proyecto en 7 pasos
   2.1 Paso 1 — Crear el proyecto y fijar el datum
   2.2 Paso 2 — Importar el levantamiento y revisar el informe
   2.3 Paso 3 — Crear el terreno (MDT)
   2.4 Paso 4 — Trazar tres pozos y dos colectores
   2.5 Paso 5 — Calcular la excavación
   2.6 Paso 6 — Generar el perfil longitudinal
   2.7 Paso 7 — Exportar y reabrir
3. Verificación: ¿lo hice bien?
   3.1 Lista de comprobación final
   3.2 Los tres errores más comunes del primer día
4. Si algo falla
   4.1 La importación descarta puntos
   4.2 El perfil sale vacío o en blanco
   4.3 No encuentro un comando
5. Siguientes pasos
   5.1 Del ejemplo a tu proyecto real
   5.2 Aprender haciendo: el proyecto tutorial
   5.3 A dónde ir ahora (mapa de la guía)

**Las 3 páginas más importantes:** "2.2 Importar el levantamiento y revisar el informe" (ahí se pierde el 70% de los primerizos) · "2.5 Calcular la excavación" (el momento en que el producto demuestra su valor) · "4.1 La importación descarta puntos" (la duda que genera más tickets).

### 5.4 Documento 2 — Manual de usuario por tarea

**Propósito.** Ser la referencia operativa del día a día: cada tarea con pasos, capturas, resultado esperado y resolución de problemas.
**Audiencia.** P1, P2, P3 con el producto ya instalado. Se consulta por búsqueda, no por lectura lineal.

**Tabla de contenido (2 niveles):**

1. El espacio de trabajo
   1.1 Ventana, paneles y vistas
   1.2 Navegación, zoom y selección
   1.3 Atajos de teclado
   1.4 Preferencias y unidades
2. Proyectos
   2.1 Crear, abrir y guardar
   2.2 Parámetros de diseño
   2.3 Escenarios y comparación
   2.4 Plantillas de municipio
3. Topografía
   3.1 Importar DXF, CSV y TXT
   3.2 Crear y editar el MDT
   3.3 Sistema de coordenadas y datum
   3.4 Depurar puntos atípicos
   3.5 Secciones del terreno
4. Modelo de objetos
   4.1 Insertar pozos y subtipos
   4.2 Trazar tuberías y colectores
   4.3 Válvulas, llaves, hidrantes y medidores
   4.4 Vertederos, descargas y PTAR
   4.5 Mover, copiar y eliminar objetos
   4.6 Validar el modelo
5. Red de agua potable
   5.1 Trazar la red
   5.2 Caudales de diseño
   5.3 Dimensionar diámetros
   5.4 Sectorizar y válvulas de cierre
   5.5 Verificar presiones y velocidades
6. Red de alcantarillado
   6.1 Pozos y cotas
   6.2 Colectores y pendientes
   6.3 Pozos de caída
   6.4 Acometidas y sumideros
   6.5 Relación de llenado y capacidad
   6.6 Pendientes mínimas y excepciones
7. Cálculo y validación
   7.1 Ejecutar el cálculo hidráulico
   7.2 Interpretar los resultados
   7.3 Validar contra el code pack
   7.4 Justificar una excepción
   7.5 Recalcular tras un cambio
8. Excavación y cómputos
   8.1 Definir la sección tipo de zanja
   8.2 Calcular excavación por tramo
   8.3 Calcular excavación de pozos
   8.4 Relleno, préstamo y excedente
   8.5 Cómputos de materiales
   8.6 Comparar con medición de campo
9. Planos y perfiles
   9.1 Vista de planta y simbología
   9.2 Perfil longitudinal
   9.3 Secciones transversales
   9.4 Cajetín y lotes de planos
   9.5 Exportar DXF, PDF y GeoJSON
10. Informes
    10.1 Memoria de cálculo
    10.2 Cómputos y presupuesto
    10.3 Listados técnicos
    10.4 Plantillas de informe
11. Revisión y auditoría
    11.1 Abrir un expediente en solo lectura
    11.2 Auditar un proyecto recibido
    11.3 Comparar revisiones
    11.4 Generar el acta de revisión
12. Solución de problemas por tarea
    12.1 Síntomas y causas frecuentes
    12.2 Cómo reportar un defecto con datos útiles

**Las 3 páginas más importantes:** "6.2 Colectores y pendientes" (el corazón del diseño de alcantarillado) · "8.2 Calcular excavación por tramo" (el entregable que se paga) · "7.3 Validar contra el code pack" (donde el producto se vuelve defendible).

### 5.5 Documento 3 — Referencia de familias de objetos

**Propósito.** Definir cada familia y subtipo con sus atributos, unidades, reglas de validación, representación gráfica y relación con el cómputo. Es el contrato entre el modelo y el usuario.
**Audiencia.** P1 y P2 (consulta), desarrolladores de plugins (contrato), agentes IA (vía `redes esquema`).

**Tabla de contenido (2 niveles):**

1. El modelo de objetos
   1.1 Objeto, familia, subtipo y atributo
   1.2 Identificadores y claves naturales
   1.3 Unidades y convenciones
   1.4 Herencia y atributos comunes
2. Familias de topografía
   2.1 Punto topográfico
   2.2 Curva de nivel
   2.3 Terreno (MDT/TIN)
3. Familias de nodo
   3.1 Pozo de revisión
   3.2 Pozo de caída
   3.3 Caja de revisión y sumidero
   3.4 Nodo de empalme y de predio
   3.5 Reservorio y tanque
4. Familias de conducción
   4.1 Tubería de agua potable
   4.2 Colector de alcantarillado
   4.3 Canal y cuneta
   4.4 Emisario y descarga
5. Familias de accesorio
   5.1 Válvula de compuerta y de aire
   5.2 Llave de paso y medidor
   5.3 Hidrante
   5.4 Conexión domiciliaria
   5.5 Estación de bombeo
6. Familias de obra civil
   6.1 Cámara de bombeo
   6.2 Vertedero y aliviadero
   6.3 Planta de tratamiento
7. Reglas transversales
   7.1 Reglas de coherencia geométrica
   7.2 Reglas de coherencia hidráulica
   7.3 Reglas de cómputo
   7.4 Cómo se reporta una violación
8. Extender el catálogo
   8.1 Anatomía de un manifiesto de familia
   8.2 Contrato de cálculo
   8.3 Conformidad y pruebas

**Las 3 páginas más importantes:** "3.1 Pozo de revisión" (el objeto más usado del producto) · "4.2 Colector de alcantarillado" (donde viven pendiente, rugosidad y cómputo) · "8.1 Anatomía de un manifiesto de familia" (la puerta de la extensibilidad).

### 5.6 Documento 4 — Referencia de cálculos

**Propósito.** Que cualquier ingeniero o revisor pueda reproducir cada número del producto, a mano y con las mismas unidades. Es el documento que se defiende en una auditoría.
**Audiencia.** P1 y P2, revisores externos, peritos, desarrolladores del motor.

**Tabla de contenido (2 niveles):**

1. Convenciones de cálculo
   1.1 Unidades, símbolos y redondeo
   1.2 Tolerancias declaradas por cálculo
   1.3 Reproducibilidad y hash de resultados
2. Hidráulica de agua potable
   2.1 Población, dotación y caudales de diseño
   2.2 Coeficientes de variación horaria
   2.3 Hazen-Williams
   2.4 Darcy-Weisbach y pérdidas locales
   2.5 Presiones, velocidad y línea piezométrica
   2.6 Estimación de golpe de ariete
3. Hidráulica de alcantarillado
   3.1 Caudal de aporte y de infiltración
   3.2 Manning y sección parcialmente llena
   3.3 Relación de llenado Y/D y capacidad
   3.4 Velocidad mínima y esfuerzo cortante
   3.5 Pendientes mínimas y máximas
   3.6 Pozos de caída y pérdidas
4. Excavación y movimiento de tierras
   4.1 Sección tipo de zanja y ancho de fondo
   4.2 Taludes por tipo de suelo
   4.3 Volumen por áreas medias
   4.4 Volumen de caja de pozo
   4.5 Relleno, cama, protección y sobre-excavación
   4.6 Abundamiento y factor de esponjamiento
   4.7 Balance de tierras y material excedente
   4.8 Transporte y distancia media
5. Cómputos y materiales
   5.1 Tubería por diámetro y material
   5.2 Hormigón, acero y encofrado
   5.3 Accesorios y conexiones
6. Validaciones normativas
   6.1 Cómo se construye un check
   6.2 Severidad y veredicto
   6.3 Code packs: estado ACTIVE y BLOCKED_BY_SOURCE
   6.4 Del check a la cláusula
7. Ejemplos resueltos
   7.1 Colector de 200 mm: de la pendiente al Y/D
   7.2 Tramo de zanja de 3,5 m: del terreno al volumen
   7.3 Pozo de 3,2 m: caja, relleno y excedente
   7.4 Red de agua de 12 nodos: caudales y presión
8. Verificación independiente
   8.1 Cómo comparar contra EPANET
   8.2 Cómo comparar contra planilla propia
   8.3 Registro de casos de prueba

**Las 3 páginas más importantes:** "4.3 Volumen por áreas medias" (el número que se paga) · "3.3 Relación de llenado Y/D y capacidad" (el que define si el diseño sirve) · "7.2 Tramo de zanja de 3,5 m" (el ejemplo que el revisor reproduce a mano).

**Regla de contenido:** cada fórmula lleva su sustitución numérica con unidades, el resultado, y una nota de "si tu número no coincide, revisa esto".

### 5.7 Documento 5 — Guía de la CLI para agentes

**Propósito.** Que un agente IA (o un ingeniero con scripts) ejecute el flujo completo sin GUI, con resultados deterministas y errores accionables. Es también la guía para integrar por MCP.
**Audiencia.** P4 (agentes), desarrolladores de integración, equipos de automatización municipal.

**Tabla de contenido (2 niveles):**

1. El contrato de la CLI
   1.1 Instalación y autenticación local
   1.2 Modelo de comandos y convenciones
   1.3 Regla de oro: paridad con la UI
   1.4 Determinismo y versión
2. Descubrimiento
   2.1 `redes esquema` y el manifiesto
   2.2 Lectura del manifiesto por un LLM
   2.3 Unidades en los nombres de parámetros
   2.4 Qué comandos escriben y cuáles solo leen
3. Comandos por dominio
   3.1 Proyecto
   3.2 Topografía
   3.3 Agua potable
   3.4 Alcantarillado
   3.5 Hidráulica y validación
   3.6 Excavación y cómputos
   3.7 Planos e informes
   3.8 Auditoría
4. Salida y errores
   4.1 Esquema JSON y versionado
   4.2 Catálogo de códigos de salida
   4.3 Errores con objeto y sugerencia
   4.4 Reintentos, idempotencia y `--dry-run`
5. Patrones de uso
   5.1 Receta: proyecto desde cero en 12 comandos
   5.2 Receta: auditar 12 expedientes en lote
   5.3 Receta: recalcular tras un cambio de cota
   5.4 Receta: generar planos en CI
6. Integración por MCP
   6.1 Levantar el servidor
   6.2 Tools disponibles y su contrato
   6.3 Confirmación de escrituras
   6.4 Límites y buenas prácticas
7. Límites y seguridad
   7.1 Qué no puede hacer un agente
   7.2 Bloqueo de archivo y concurrencia
   7.3 Registro de auditoría de las acciones del agente

**Las 3 páginas más importantes:** "2.1 `redes esquema` y el manifiesto" (sin esto el agente adivina) · "5.1 Receta: proyecto desde cero en 12 comandos" (el flujo completo en una página) · "4.2 Catálogo de códigos de salida" (lo que permite al agente corregirse solo).

**Criterio de calidad:** la guía CLI se prueba con un agente real que no tiene otro contexto: si necesita preguntar algo que no está en la página, la página está incompleta.

### 5.8 Documento 6 — Guía de normas y parámetros (code packs)

**Propósito.** Explicar cómo el producto aplica una norma: qué es un code pack, qué contiene, cómo se activa, cómo se audita y qué pasa cuando está incompleto. Es la guía que protege al cliente en una revisión municipal.
**Audiencia.** P2 (parametrización municipal), P1 (consulta), responsables de calidad, auditores.

**Tabla de contenido (2 niveles):**

1. Qué es un code pack
   1.1 Norma, edición y jurisdicción
   1.2 Pack, parámetro, check y cláusula
   1.3 Qué NO es un code pack
2. Anatomía de un pack
   2.1 Campos obligatorios del registro
   2.2 Fuente autorizada y procedencia
   2.3 Alcance y exclusiones
   2.4 Identificadores de cláusula usados
   2.5 Unidades, convenciones y erratas
   2.6 Versión, hash y casos de validación
3. Estado del pack
   3.1 ACTIVE: cuándo se puede afirmar cumplimiento
   3.2 BLOCKED_BY_SOURCE: qué falta y qué se bloquea
   3.3 MECHANICS_ONLY: cálculo sin veredicto
   3.4 Cómo se muestra el estado en la app y en la CLI
4. Packs incluidos y packs del cliente
   4.1 Qué trae el producto de fábrica
   4.2 Cómo se parametriza el pack municipal
   4.3 Quién firma la parametrización
   4.4 Actualizar un pack sin romper proyectos
5. Parámetros típicos y su efecto
   5.1 Dotación y coeficientes
   5.2 Velocidades y pendientes
   5.3 Profundidades y recubrimientos
   5.4 Taludes y abundamiento
   5.5 Separaciones mínimas y cruces
6. Trazabilidad y auditoría
   6.1 Del veredicto a la cláusula
   6.2 Qué queda escrito en la memoria
   6.3 Cómo se audita un proyecto con pack ajeno
   6.4 Errores frecuentes de parametrización

**Las 3 páginas más importantes:** "2.1 Campos obligatorios del registro" (el contrato de calidad) · "3.1 ACTIVE: cuándo se puede afirmar cumplimiento" (la regla que evita afirmaciones falsas) · "4.2 Cómo se parametriza el pack municipal" (la tarea real del técnico).

**Regla de producto asociada:** un pack con campos faltantes se muestra como `BLOCKED_BY_SOURCE`, con la lista de lo que falta; en ese estado el producto calcula (`MECHANICS_ONLY`) pero **no emite veredicto de cumplimiento**. Nunca se rellena un campo desde la memoria del modelo ni desde "lo que suele usarse".

### 5.9 Documento 7 — Arquitectura para desarrolladores

**Propósito.** Permitir que un tercero mantenga, extienda o audite el producto sin leer todo el código: capas, contratos, decisiones y cómo se prueba.
**Audiencia.** Desarrolladores del cliente, autores de plugins, futuros mantenedores.

**Tabla de contenido (2 niveles):**

1. Visión y principios
   1.1 Modelo primero, vistas después
   1.2 Núcleo determinista y sin UI
   1.3 Un solo contrato para UI, CLI y MCP
2. Estructura del código
   2.1 Crates y módulos
   2.2 Dependencias permitidas y prohibidas
   2.3 Dónde vive cada responsabilidad
3. Modelo de datos
   3.1 Familias, atributos y unidades
   3.2 Geometría y CRS
   3.3 Esquema persistido y migraciones
4. Motor de cálculo
   4.1 Entradas, salidas y determinismo
   4.2 Hidráulica
   4.3 Excavación y cómputos
   4.4 Validación y code packs
   4.5 Pruebas de referencia y tolerancias
5. Interfaz Rune
   5.1 Composición de vistas y estado
   5.2 Lienzo 2D: dibujo, snapping y selección
   5.3 Rendimiento con miles de objetos
   5.4 Accesibilidad y atajos
6. CLI y MCP
   6.1 Registro de comandos como fuente única
   6.2 Generación del manifiesto
   6.3 Serialización y versionado del JSON
7. Extensibilidad
   7.1 Manifiesto de familia
   7.2 Contrato de cálculo
   7.3 Aislamiento y fallos
8. Construcción y entrega
   8.1 Compilar, probar y empaquetar
   8.2 CI: pruebas, capturas y ejemplos de la guía
   8.3 Versionado y compatibilidad de archivos

**Las 3 páginas más importantes:** "1.3 Un solo contrato para UI, CLI y MCP" (la decisión que hace posible la paridad) · "4.1 Entradas, salidas y determinismo" (la base de la confianza en los números) · "7.1 Manifiesto de familia" (el mecanismo de crecimiento del producto).

### 5.10 Documento 8 — FAQ y solución de problemas

**Propósito.** Resolver en menos de dos minutos el 80% de las consultas de soporte, con el mensaje exacto que ve el usuario y la acción concreta.
**Audiencia.** Todos, especialmente en la primera semana.

**Tabla de contenido (2 niveles):**

1. Cómo usar esta guía
   1.1 Buscar por mensaje de error
   1.2 Buscar por síntoma
2. Instalación y licencia
   2.1 No abre o se cierra al inicio
   2.2 Licencia y modo offline
   2.3 Actualización y compatibilidad de proyectos
3. Topografía
   3.1 La importación descarta puntos
   3.2 El terreno sale invertido o con picos
   3.3 El proyecto está en otro datum
4. Modelo y dibujo
   4.1 No puedo unir dos tramos
   4.2 El pozo quedó sin salida
   4.3 Los objetos desaparecieron de la vista
5. Cálculo
   5.1 El cálculo no converge
   5.2 El Y/D sale mayor que 1
   5.3 El volumen no cuadra con mi Excel
6. Validación normativa
   6.1 Todo sale "no evaluable"
   6.2 No estoy de acuerdo con un check
   6.3 El pack municipal no está
7. Planos e informes
   7.1 El PDF sale en blanco
   7.2 El DXF abre con capas vacías
   7.3 Las cifras del informe no coinciden
8. CLI y agentes
   8.1 El comando devuelve error de validación
   8.2 El JSON cambió entre versiones
   8.3 El agente repite operaciones
9. Rendimiento y archivos
   9.1 El proyecto va lento
   9.2 El archivo se corrompió
   9.3 Cómo reportar un defecto

**Las 3 páginas más importantes:** "3.1 La importación descarta puntos" · "5.3 El volumen no cuadra con mi Excel" · "9.3 Cómo reportar un defecto" (convierte un ticket inútil en uno reproducible).

### 5.11 Documento 9 — Glosario ES/EN

**Propósito.** Un solo significado por término, en la app, la guía y la CLI. Evita el clásico "¿invert o cota de fondo?" que arruina una revisión.
**Audiencia.** Todos; obligatorio para traductores y para quien escriba cadenas de UI.

**Tabla de contenido (2 niveles):**

1. Cómo usar el glosario
   1.1 Término preferido, sinónimos y prohibidos
   1.2 Relación con la interfaz y con la CLI
2. Términos de topografía
   2.1 Levantamiento, punto, cota, progresiva
   2.2 Curva de nivel, MDT, TIN, pendiente
   2.3 Datum, zona UTM, EPSG
3. Términos de hidráulica
   3.1 Caudal, dotación, aporte, infiltración
   3.2 Pérdida, presión, línea piezométrica
   3.3 Y/D, esfuerzo cortante, capacidad
4. Términos de alcantarillado y agua potable
   4.1 Pozo de revisión, pozo de caída, caja, sumidero
   4.2 Colector, emisario, acometida, conexión
   4.3 Válvula, llave, hidrante, medidor, vertedero
5. Términos de construcción y cómputo
   5.1 Excavación, relleno, cama, protección
   5.2 Abundamiento, esponjamiento, préstamo, excedente
   5.3 Metrado, cómputo, ítem, APU
6. Términos de software
   6.1 Proyecto, escenario, familia, atributo, check
   6.2 Code pack, estado, severidad, veredicto
   6.3 Manifiesto, comando, tool, idempotencia
7. Siglas y normas
   7.1 Siglas de instituciones y normas
   7.2 Abreviaturas de planos
   7.3 Equivalencias ES/EN

**Las 3 páginas más importantes:** "4.1 Pozo de revisión, pozo de caída, caja, sumidero" (donde más se confunde) · "5.2 Abundamiento, esponjamiento, préstamo, excedente" (donde se pierde dinero) · "6.2 Code pack, estado, severidad, veredicto" (donde se pierde credibilidad).

### 5.12 El primer documento a escribir: Guía de inicio rápido

**Por qué este primero.** Es el único documento que se puede escribir y probar **antes** de que exista el producto completo: se redacta contra el esqueleto caminante de v0.1, se prueba con 3 usuarios reales y se corrige. Además fija el tono, el formato de paso y el estilo de captura que heredarán los otros ocho documentos. Escribir la referencia de cálculos primero sería escribir teoría sin producto que la respalde.

**Regla de escritura:** cada paso se escribe como *acción → resultado visible → qué hacer si no aparece*. Nunca "el sistema permite".

#### Sección 1 redactada — "Antes de empezar"

> **1. Antes de empezar**
>
> Esta guía te lleva de cero a tu primer proyecto con topografía, pozos, colectores, volumen de excavación y perfil longitudinal en **15 minutos**. No explica teoría ni hidráulica: para eso están la *Referencia de cálculos* y el *Manual de usuario*. Aquí solo se hace.
>
> Vas a necesitar tres cosas: la aplicación instalada, un levantamiento topográfico en DXF o CSV, y quince minutos sin interrupciones.
>
> **1.1 Requisitos, instalación y licencia**
>
> | Requisito | Mínimo | Recomendado |
> |---|---|---|
> | Sistema operativo | Windows 10 de 64 bits | Windows 11 de 64 bits |
> | Memoria RAM | 8 GB | 16 GB |
> | Espacio en disco | 500 MB libres | 2 GB libres (para proyectos y planos) |
> | Pantalla | 1366 × 768 | 1920 × 1080 o superior |
> | Datos | Ninguno: la app funciona sin conexión | — |
>
> Instala ejecutando `RuneRedes-Setup-x.y.z.msi` y aceptando el directorio por defecto. La primera vez que abras la app se te pedirá tu nombre y tu institución: eso se usa para firmar la bitácora de auditoría y el cajetín de los planos, así que escríbelo como quieres que aparezca en el expediente.
>
> La licencia se activa una sola vez. Si trabajas en una red sin internet, pide a tu administrador el archivo de activación offline; la app no vuelve a pedir conexión.
>
> Para comprobar que quedó bien instalada, abre una terminal y escribe:
>
> ```bash
> redes --version
> ```
>
> Debe responder con la versión, por ejemplo `redes 0.1.0`. Si el comando no existe, cierra y vuelve a abrir la terminal (el instalador agrega la ruta al PATH, pero la terminal abierta conserva el anterior).
>
> **1.2 Qué necesitas del levantamiento**
>
> La app no inventa el terreno: lo lee de tu levantamiento. Antes de empezar, revisa que tu archivo tenga esto:
>
> - **Puntos acotados** con coordenadas X, Y y cota Z. En DXF, en una capa identificable (por ejemplo `PUNTOS`) y con la cota como texto o como atributo del bloque.
> - **Curvas de nivel** si tu levantamiento las tiene, en su propia capa. Si no las tienes, la app genera el terreno solo con los puntos: funciona, pero la superficie es menos fiel entre puntos.
> - **El datum y la zona UTM** con los que se levantó. En Ecuador lo habitual es WGS84 o PSAD56, zona 17S o 18S. Si no lo sabes, pregúntalo antes de importar: cambiarlo después obliga a reproyectar y es la causa número uno de proyectos con coordenadas incoherentes.
> - **Las cotas referidas al nivel del mar**, no a un punto local arbitrario. Si tu levantamiento usa una cota local (por ejemplo "BM = 100,00"), anótalo: la app lo admite, pero debes registrar la referencia para que el perfil y la memoria sean interpretables.
>
> No necesitas limpiar el dibujo: la app ignora capas que no le pides y te informa cuántos puntos descartó y por qué. Lo que sí conviene es traer **un archivo por levantamiento**, no el plano completo de la ciudad.
>
> **1.3 Convenciones de esta guía**
>
> Cada paso tiene la misma estructura, para que puedas seguirlo sin releer:
>
> - **Qué vas a ver.** El resultado en pantalla al terminar el paso.
> - **Cómo se hace.** Los clics o el comando, numerados.
> - **Si algo falla.** Las dos causas más frecuentes y qué hacer, con el mensaje exacto que verás.
>
> Convenciones de escritura: los nombres de botones, campos y paneles van en **negrita**; los objetos del modelo van en `mono` (`PozoRevision`, `Colector`); los comandos de terminal van en bloque de código con el prefijo `redes`. Las unidades siempre se escriben: `m`, `mm`, `l/s`, `m³`, `%`. Los números usan el formato ecuatoriano: **1.234,56** (punto para miles, coma para decimales).
>
> Los ejemplos usan el proyecto **La Esperanza** (§6.2), que viene incluido con la app: puedes abrirlo y comparar tu resultado con el esperado en cualquier momento. Si tu resultado no coincide, no sigas: revisa "3. Verificación: ¿lo hice bien?".

---

## 6. Onboarding dentro de la app

### 6.1 Primera ejecución: cinco pantallas, ningún manual

| # | Pantalla | Objetivo | Contenido | Salida |
|---|---|---|---|---|
| 1 | Bienvenida | Que sepa qué hace el producto en una frase | "Diseña redes de agua y alcantarillado sobre tu levantamiento, con cómputo de excavación." Dos botones: **Abrir el proyecto de ejemplo** / **Crear mi proyecto** | Elección de ruta |
| 2 | Perfil de uso | Adaptar la UI al rol | 3 opciones: *Diseño* (P1), *Revisión municipal* (P2), *Cómputos y obra* (P3). Cambia paneles visibles y valores por defecto | Paneles configurados |
| 3 | Unidades y formato | Evitar el desastre de unidades | Confirma `m`, `mm`, `l/s`, `m³`, separador decimal `,`, y datum por defecto | Preferencias guardadas |
| 4 | Code pack | Saber con qué norma se va a validar | Lista de packs disponibles con su estado (`ACTIVE` / `BLOCKED_BY_SOURCE`) y qué implica cada uno | Pack activo o "decidir después" |
| 5 | Primer paso | Arrancar con una acción concreta | **Importar mi levantamiento** / **Ver el tutorial de 25 minutos** / **Explorar el proyecto de ejemplo** | Proyecto abierto |

Reglas: se puede saltar todo (botón **Omitir** visible, no escondido); lo omitido se recupera desde **Ayuda → Primeros pasos**; ninguna pantalla pide datos que la app pueda inferir; la pantalla 4 explica en una línea qué significa no tener pack (se calcula, no se certifica).

### 6.2 Proyecto de ejemplo: "La Esperanza"

Un proyecto completo, incluido con la app, que sirve de referencia, de campo de pruebas y de material del tutorial.

| Atributo | Valor |
|---|---|
| Contenido | 2,4 km de colector · 38 pozos (2 de caída) · 1 red de agua de 900 m · 1 reservorio · 3 descargas |
| Topografía | 1 240 puntos acotados, curvas cada 0,5 m, datum WGS84 / UTM 17S |
| Code pack | `EC-ALC-DEMO` marcado explícitamente como **demostración, no normativo** |
| Resultados esperados | Volumen de excavación 1 847,3 m³ · 12 tramos con observación · 0 errores topológicos |
| Para qué sirve | Comparar resultados, probar la CLI sin riesgo, aprender el flujo sin datos propios |

El pack del ejemplo se llama "DEMO" a propósito: ningún usuario debe confundir un ejemplo con la norma de su municipio.

### 6.3 Tooltips y ayuda contextual

- **Regla de los 140 caracteres:** un tooltip explica *qué hace* y *qué unidad usa*, en una línea. Si necesita más, es un enlace a la guía, no un tooltip.
- **Cobertura obligatoria:** el 100% de los campos con unidad tiene tooltip con unidad y rango válido; el 100% de los botones de cálculo tiene tooltip con el método que aplica.
- **Dos niveles:** básico (siempre visible al pasar el cursor) y avanzado (icono `?` que abre la página exacta del manual, con ancla a la sección).
- **Prohibido:** tooltips que repiten la etiqueta ("Diámetro: el diámetro") o que dicen "consulte la documentación" sin enlace.

### 6.4 Estados vacíos

| Pantalla | Mensaje | Acción ofrecida |
|---|---|---|
| Proyecto sin topografía | "Todavía no hay terreno. Importa tu levantamiento para empezar a dibujar." | **Importar levantamiento** · **Usar el ejemplo** |
| Red sin objetos | "Esta red está vacía. Crea el primer pozo o importa una red desde CSV." | **Crear pozo** · **Importar CSV** |
| Sin cálculo ejecutado | "Aún no hay resultados. Calcula la red para ver caudales y presiones." | **Calcular ahora** |
| Cómputo sin sección tipo | "Para calcular la excavación necesitas definir cómo se excava." | **Definir sección tipo** (abre el formulario con valores sugeridos) |
| Validación sin code pack | "No hay norma activa: se puede calcular, pero no certificar cumplimiento." | **Elegir code pack** · **Ver qué implica** |
| Listados sin resultados | "Ningún objeto cumple el filtro actual." | **Limpiar filtros** |

Regla: un estado vacío nunca es un callejón; siempre ofrece la acción que lo resuelve y, cuando existe, la opción de ver el ejemplo.

### 6.5 Mensajes de error escritos para humanos

| Situación | Mal (lo que hace el software normal) | Bien (lo que haremos) |
|---|---|---|
| Punto sin cota | `Error: parse failed at line 412` | "El punto 412 de la capa `PUNTOS` no tiene cota. Se importará el resto; corrige ese punto o exclúyelo de la capa." |
| Cota incoherente | `ValidationException: invert > cover` | "El pozo P-07 tiene el fondo (2.812,10 m) por encima de la tapa (2.811,90 m). Revisa la cota de invert o la profundidad." |
| Pendiente insuficiente | `Check failed: ALC-06` | "El tramo P-03 → P-04 tiene pendiente 0,4% y el mínimo para 200 mm es 0,5%. Sube la pendiente o registra una justificación." |
| Pack incompleto | `CodePackError(3)` | "El pack `EC-ALC-MUNICIPAL` no declara la edición de la norma. Sin ese dato no se puede afirmar cumplimiento: el cálculo se ejecuta, el veredicto no se emite." |
| Comando CLI inválido | `unexpected argument '--diametro'` | "Falta la unidad en el parámetro. Usa `--diametro_mm 200` (no `--diametro`). Ejecuta `redes esquema` para ver los parámetros de este comando." |
| Archivo bloqueado | `EBUSY` | "El proyecto está abierto por `mgarcia` desde las 09:14. Puedes abrirlo en solo lectura o esperar a que lo cierre." |

Reglas de redacción: (1) qué pasó, en la primera frase, sin códigos; (2) sobre qué objeto; (3) qué hacer, en imperativo; (4) el código técnico solo al final, copiable, para soporte. Nunca "error inesperado", nunca "contacte al administrador" sin decir qué mirar.

### 6.6 Tutorial "aprender haciendo"

Un proyecto guiado de 25–35 minutos, dentro de la app, que valida cada paso automáticamente y no deja avanzar con el modelo roto.

| Paso | Tarea | Validación automática | Tiempo |
|---|---|---|---|
| 1 | Crear el proyecto "Mi primer colector" con datum 17S | Existe el proyecto y el CRS es correcto | 2 min |
| 2 | Importar `la-esperanza.dxf` y revisar el informe | ≥ 1 200 puntos importados y 0 descartes por "sin cota" | 4 min |
| 3 | Crear el terreno y consultar la cota en un punto dado | Cota consultada coincide con la esperada ± 0,05 m | 3 min |
| 4 | Crear 4 pozos con cotas dadas y unirlos con 3 colectores | 4 pozos, 3 colectores, 0 errores topológicos | 6 min |
| 5 | Calcular Y/D y corregir un tramo que no cumple | 0 tramos con observación al finalizar | 5 min |
| 6 | Definir la sección tipo y calcular excavación | Volumen total dentro de ± 1% del esperado | 5 min |
| 7 | Generar el perfil y exportar el DXF | PDF y DXF generados; el DXF reimporta sin pérdida | 4 min |

Al terminar: insignia **"Primer colector diseñado"**, el proyecto queda guardado como ejemplo propio, y se ofrece el siguiente nivel (tutorial de agua potable). Si el usuario abandona, el tutorial retoma donde quedó; el progreso se guarda en el proyecto, no en la nube.

**Métrica del tutorial:** ≥ 80% de los usuarios que lo empiezan lo terminan sin abrir la guía ni pedir soporte (M6).

---

## 7. Métricas de éxito

Se miden con telemetría local opt-in (agregada, sin datos del proyecto) y con los pilotos. Sin telemetría, la métrica se mide en el piloto con observación directa.

| # | Métrica | Definición / fórmula | Objetivo | Instrumentación | Frecuencia |
|---|---|---|---|---|---|
| M1 | Tiempo hasta la primera red | Minutos desde el primer lanzamiento hasta el primer proyecto con ≥ 3 pozos y ≥ 2 colectores guardado | ≤ 30 min (capacitado) · ≤ 45 min (novato con tutorial) | Evento local `primera_red_completada` con marca de tiempo de sesión | Por release y por piloto |
| M2 | Diseños que pasan validación al primer envío | Proyectos validados sin observaciones críticas / proyectos validados | ≥ 70% | Contador de `norma validar` con 0 críticos por proyecto | Mensual |
| M3 | Error de excavación vs campo | `abs(V_calc − V_campo) / V_campo` por tramo medido | ≤ 5% en ≥ 80% de tramos; mediana ≤ 3% | EXC-07 con CSV de campo | Por obra |
| M4 | Uso de la CLI por agentes | Proyectos creados o modificados solo por CLI / proyectos activos; y comandos exitosos sin reintento | ≥ 30% de proyectos en pilotos con agentes · ≥ 95% de comandos exitosos | Log de comandos (código de salida) | Mensual |
| M5 | Retrabajo de edición | Horas de edición por km de red (incluye rehacer perfil y reconciliar) | ≤ 4 h/km (línea base actual: 20–30 h/km) | Tiempo activo por proyecto / longitud de red | Por proyecto |
| M6 | Finalización del tutorial | Tutoriales terminados / tutoriales iniciados | ≥ 80% | Evento `tutorial_completado` | Por release |
| M7 | Autosuficiencia en importación | Importaciones resueltas sin ticket / importaciones totales | ≥ 90% | Tickets etiquetados "topografía" | Mensual |
| M8 | Reproducibilidad de la memoria | Memorias regeneradas con hash de cifras idéntico / memorias regeneradas | 100% | `informe memoria --verificar-hash` | Por release |
| M9 | Tiempo de planos | Minutos para generar 40 planos de un proyecto de 2,4 km | ≤ 5 min | Cronómetro del comando `plano generar` | Por release |
| M10 | Satisfacción y soporte | NPS del piloto; tickets por proyecto nuevo | NPS ≥ 8/10 · ≤ 1 ticket/proyecto | Encuesta + conteo de tickets | Por piloto |

**Métrica de salud del producto (no del usuario):** porcentaje de ítems del backlog con criterio de aceptación automatizado. Objetivo: ≥ 70% al llegar a v1.0. Un backlog con criterios que nadie ejecuta es una lista de deseos.

**Contra-métricas (para no engañarnos):** número de validaciones ejecutadas (puede subir sin mejorar nada), líneas de código, número de features entregadas. No se reportan como éxito.

---

## 8. Riesgos y preguntas abiertas para el cliente

Estas no son dudas del equipo: son **decisiones que el cliente debe tomar**. Cada una bloquea o encarece una parte del plan.

| # | Decisión requerida | Opciones | Por qué urge | Plazo sugerido |
|---|---|---|---|---|
| D1 | **¿Qué norma y qué edición validamos primero?** Una sola como pack de referencia para v0.2 | (a) Normativa nacional de agua potable y alcantarillado vigente · (b) ordenanza de una EMAP piloto · (c) ambas, con la nacional como base y la municipal como especialización | Sin jurisdicción, edición y fuente autorizada no hay pack `ACTIVE`; sin pack no hay veredicto de cumplimiento (HID-03) y la memoria no es defendible | Antes de v0.2 |
| D2 | **¿Los levantamientos vienen en WGS84 o en PSAD56?** Y ¿las cotas son al nivel del mar o locales? | (a) WGS84 UTM 17S/18S · (b) PSAD56 UTM 17S/18S · (c) mixto, con transformación declarada | Cambia TOP-03, la reproyección y todos los informes; un datum mal asumido invalida el expediente completo | Antes de v0.1 |
| D3 | **¿La hidráulica de v1 es solo régimen permanente?** Es decir: sin transitorios ni golpe de ariete | (a) Sí, permanente + estimación informativa de sobrepresión · (b) incluir transitorios en v1 (agrega ~6 semanas y un solver nuevo) | Hoy HID-07 está en WON'T; si el cliente lo necesita para el trámite, cambia el alcance y la fecha | Antes de v0.2 |
| D4 | **¿Qué formato de entrega exige el municipio?** Planos, cajetín, simbología y firma | (a) PDF + DXF con cajetín propio · (b) además DWG nativo (requiere librería comercial) · (c) firma electrónica del profesional | Define VIS-04/VIS-05 y si hace falta una dependencia de pago; la firma electrónica puede ser un requisito legal | Antes de v0.3 |
| D5 | **¿Entregamos presupuesto con precios o solo metrados?** | (a) Solo metrados y el cliente pone precios · (b) APU y base de precios incluida · (c) integración con la base de precios existente del cliente | (b) es un producto aparte (base de precios, actualización, jurisdicción); (a) es lo que asume INF-02 | Antes de v1.0 |
| D6 | **¿Se necesita trabajo simultáneo de varias personas en el mismo proyecto?** | (a) No: bloqueo de archivo (COL-01) · (b) Sí: edición concurrente (servidor, CRDT, +8–12 semanas) | Cambia la arquitectura de persistencia (PER-01) y el modelo de colaboración; hacerlo después es caro | Antes de v0.3 |
| D7 | **¿Se permiten plugins de terceros en v1?** | (a) Solo familias internas y SDK documentado · (b) plugins firmados de terceros (EXT-04) · (c) mercado de extensiones | (b) exige firma, sandbox y política de soporte; sin decisión, EXT-04/05 quedan fuera | Antes de v1.0 |
| D8 | **¿Hay que migrar proyectos históricos?** ¿Cuántos y en qué formato? | (a) No · (b) Sí, N proyectos en Excel/CAD (agrega importadores y validación de datos sucios) · (c) solo los del piloto | Si hay 200 proyectos en Excel, aparece un producto de migración que nadie presupuestó | Antes de v0.3 |
| D9 | **¿Cómo se licencia y se despliega?** | (a) MSI por usuario, licencia offline · (b) licencia flotante en red municipal · (c) servidor interno con clientes delgados | Define el instalador, la activación y el soporte; (c) contradice el requisito offline si no se diseña bien | Antes de v1.0 |
| D10 | **¿Quién firma la memoria y con qué responsabilidad?** | (a) El ingeniero del cliente, con su firma y sello · (b) la consultora · (c) el municipio revisa y firma | Determina qué texto de descargo lleva la memoria, qué se registra en la bitácora y qué se puede afirmar en el veredicto normativo | Antes de v1.0 |

**Riesgo de producto que no depende del cliente (vigilar):** si el lienzo 2D de Rune no alcanza para edición de planta con snapping y miles de objetos (supuesto S2), la vista de planta se degrada y con ella AGU-01, ALC-02 y VIS-01. Mitigación: prototipo de lienzo en la semana 1–2 de v0.1, antes de comprometer fecha. Es la única dependencia técnica que puede mover el plan de releases.

---

## Anexo A — Definition of Done (por ítem)

Un ítem está terminado cuando: (1) cumple sus criterios de aceptación con al menos una prueba automatizada o un guion reproducible; (2) tiene comando de CLI si es una capacidad (CLI-01); (3) aparece en la guía en la página que le corresponde; (4) los mensajes de error siguen la regla de §6.5; (5) está en el manifiesto si es familia o comando; (6) la documentación declara a qué versión aplica.

## Anexo B — Trazabilidad personas → recorridos → releases

| Persona | Recorridos que usa | Release en que queda cubierta |
|---|---|---|
| P1 Consultora | R1, R2, R3, R4, R5, R6, R7 | v0.1 (parcial) → v0.3 (completa) |
| P2 Municipal | R1, R4, R8 | v0.2 (validación) → v0.3 (auditoría y actas) |
| P3 Constructora | R5, R7 | v0.2 (cómputos) → v0.3 (comparación con campo) |
| P4 Agente IA | R1-IA, R5-IA, R8-IA, R7 | v0.1 (6 comandos) → v0.3 (MCP y paridad) |

