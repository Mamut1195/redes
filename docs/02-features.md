# 02 — Plan de funcionalidades

> **Regla que gobierna todo el backlog:** ninguna funcionalidad calcula ingeniería. Si un ítem
> necesita un número que no viene del origen, se rechaza o se convierte en «mostrar el dato del
> origen». Las excepciones aparentes (longitud, área, volumen dibujado) son **geometría**, no
> diseño, y se marcan como derivadas de pantalla.

Prioridades: **MUST** (sin esto no hay producto) · **SHOULD** (duele si falta) · **COULD**
(deseable) · **WON'T** (decidido no hacer en v1). Esfuerzo: S ≤ 3 días · M ≤ 2 semanas ·
L ≤ 6 semanas · XL > 6 semanas.

---

## 1. Ingesta (ING)

| ID | Ítem | Valor | Aceptación | Prior. | Esf. |
|---|---|---|---|---|---|
| ING-01 | Importar plan JSON `redes.plan/1` | El contrato que produce la skill; cualquier fuente entra por aquí | · Nodos y arcos se crean con familia/tipo validados<br>· Códigos duplicados y extremos inexistentes se rechazan con motivo<br>· Informe con leídos/descartados | MUST | S |
| ING-02 | Importar DXF (puntos, líneas, polilíneas 3D) | El delineante ya tiene el modelo en CAD | · Lee POINT/LINE/POLYLINE/LWPOLYLINE/CIRCLE<br>· Mapea por capa (POZO, AGUA, ALC, EMIS…)<br>· Usa el CIRCLE como cota de tapa<br>· Ida y vuelta sin pérdida (44→44 objetos) | MUST | M |
| ING-03 | Importar terreno CSV/TXT (E,N,Z) | El levantamiento en el formato más común | · Ignora encabezado y líneas no numéricas<br>· Exige ≥ 3 puntos válidos<br>· Reporta descartes por línea | MUST | S |
| ING-04 | Importar GeoJSON | Puente con SIG y inventarios municipales | · Point y LineString con propiedades `familia`/`codigo`<br>· Extremos de línea resueltos por código o por proximidad | SHOULD | S |
| ING-05 | Perfiles de mapeo por app de origen | Que Civil 3D, SWMM y EPANET no exijan tocar código | · Un perfil declara capas, campos y unidades de la fuente<br>· Se elige con `--perfil`<br>· El perfil se guarda en el proyecto para auditar | SHOULD | M |
| ING-06 | Informe de importación con vista previa | Saber qué entró antes de aprobar nada | · Cuenta por familia y por capa<br>· Lista descartes con línea/entidad y motivo<br>· `--dry-run` no escribe | MUST | S |
| ING-07 | Importar SWMM `.inp` (nodos y conducciones) | La red de alcantarillado ya está modelada ahí | · Lee [JUNCTIONS], [CONDUITS], [OUTFALLS]<br>· Traduce diámetros y cotas<br>· Los caudales entran como `Dato`, no se recalculan | COULD | M |
| ING-08 | Importar EPANET `.inp` (nodos y tuberías) | Ídem para agua potable | · Lee [JUNCTIONS], [PIPES], [RESERVOIRS], [TANKS]<br>· Presiones y caudales como `Dato` | COULD | M |
| ING-09 | Importar IFC/Revit | BIM municipal | · Sólo si el cliente lo pide; requiere mapeo de familias | WON'T (v1) | XL |
| ING-10 | Reproceso: reimportar sobre un proyecto existente | Actualizar el modelo sin perder la revisión | · Empareja por código; conserva estado de revisión<br>· Informa altas, bajas y cambios | SHOULD | M |

## 2. Modelo y familias (MOD)

| ID | Ítem | Valor | Aceptación | Prior. | Esf. |
|---|---|---|---|---|---|
| MOD-01 | Registro de familias como dato | Añadir una familia sin tocar el motor | · `families::build()` es el único lugar<br>· La UI y la CLI listan la familia nueva sin cambios<br>· Una familia inválida no rompe la app | MUST | M |
| MOD-02 | 17 familias con subtipos | Cubre agua potable y alcantarillado | · Cada familia declara atributos, unidades y subtipos<br>· Todo subtipo referencia atributos existentes (test) | MUST | M |
| MOD-03 | Las cuatro cotas | Evita el error clásico de confundir profundidad con cota | · terreno / tapa / clave / fondo<br>· El inspector muestra las tres primeras y el fondo si vino | MUST | S |
| MOD-04 | Identidad estable y códigos legibles | Referencias que sobreviven a la exportación | · `ElementId` generacional<br>· Código único por familia (`PZ-001`)<br>· Duplicado rechazado | MUST | S |
| MOD-05 | Datos de origen con procedencia | Que cada número tenga firma | · `Dato { valor, unidad, fuente }`<br>· El inspector muestra la fuente<br>· Un dato sin fuente se marca `desconocida` | MUST | S |
| MOD-06 | Topología derivada | Navegar y validar estructura sin acoplar objetos | · Incidencia, grado, componentes<br>· Arco huérfano rechazado al crear<br>· Nodo con arcos no se borra | MUST | M |
| MOD-07 | Estados de revisión por objeto | El flujo de aprobación es la razón de ser | · pendiente / aprobado / observado<br>· Conteo por estado en CLI y UI | MUST | S |
| MOD-08 | Atributos definidos por el usuario | Extender sin recompilar | · Un proyecto puede añadir atributos extra a una familia<br>· Se guardan y se muestran | COULD | M |
| MOD-09 | Validación estructural (no normativa) | Detectar modelos rotos antes de aprobar | · Arco sin extremos, código duplicado, longitud 0, nodo aislado<br>· Nunca opina sobre hidráulica | SHOULD | S |

## 3. Vista 3D (VIS)

| ID | Ítem | Valor | Aceptación | Prior. | Esf. |
|---|---|---|---|---|---|
| VIS-01 | Cámara orbital 3D | Ver la red como realmente está en el espacio | · Órbita, desplazamiento y acercamiento<br>· Encuadre automático del proyecto | MUST | M |
| VIS-02 | Terreno sombreado con relieve | El plano de planta no muestra la profundidad | · TIN triangulado y sombreado por normal<br>· Tinte por cota<br>· Exageración vertical configurable | MUST | M |
| VIS-03 | Tubos con diámetro visible | Se distingue un colector de una acometida | · Ancho proporcional al diámetro y a la distancia<br>· Color por red (alcantarillado / agua) | MUST | S |
| VIS-04 | Pozos como cilindros | Ver altura real del pozo | · Cilindro de tapa a solera<br>· Símbolo de tamaño fijo cuando mide < 3 px | MUST | M |
| VIS-05 | Capas conmutables | Revisar una red a la vez | · Terreno / alcantarillado / agua / nodos<br>· El cambio no recalcula nada del modelo | MUST | S |
| VIS-06 | Selección por clic con inspector | Identificar el objeto y leer su ficha | · Tolerancia en píxeles<br>· Inspector con geometría, estado, atributos y datos | MUST | M |
| VIS-07 | Vistas de cámara con nombre | Planta, isométrica, perfil con un clic | · Botones y `--scene` equivalente headless | SHOULD | S |
| VIS-08 | Resaltado de observados | Encontrar lo que falta corregir | · Color distinto y lista en el panel | MUST | S |
| VIS-09 | Etiquetas de código en pantalla | Nombrar sin abrir el inspector | · Etiqueta junto al objeto seleccionado y en hover<br>· Se apaga a baja escala para no ensuciar | SHOULD | M |
| VIS-10 | Corte vertical / sección | Ver la relación tubo-terreno en un punto | · Plano de corte movible<br>· Muestra terreno, tubo y pozo | COULD | L |
| VIS-11 | Aligerado automático de terreno | Mantener la interacción con levantamientos densos | · `Terrain::thinned` por tamaño de celda<br>· Se nota el cambio de malla, no el de relieve | MUST | S |
| VIS-12 | Modo "sólo red" | Revisar sin terreno | · Oculta el terreno y encuadra a los objetos | COULD | S |
| VIS-13 | Animación de recorrido | Presentar el proyecto | · Recorrido por la traza de un colector | WON'T (v1) | L |
| VIS-14 | Realidad aumentada / móvil | Ver en campo | · Fuera de alcance | WON'T (v1) | XL |

## 4. Revisión y aprobación (REV)

| ID | Ítem | Valor | Aceptación | Prior. | Esf. |
|---|---|---|---|---|---|
| REV-01 | Aprobar / observar / reabrir un objeto | El flujo de trabajo real de una revisión | · Comandos `objeto aprobar/observar/reabrir`<br>· La UI refleja el estado al instante | MUST | S |
| REV-02 | Nota de observación | Decir qué corregir | · `--nota` se guarda con el objeto<br>· Se muestra en el inspector | MUST | S |
| REV-03 | Aprobación global bloqueada por pendientes | No aprobar por accidente | · `proyecto aprobar` falla con `E_STATE_PENDING` y dice cuántos faltan | MUST | S |
| REV-04 | Filtro y lista de pendientes | Trabajar en orden | · `objeto lista --estado pendiente`<br>· Filtro por familia | MUST | S |
| REV-05 | Historial de revisiones | Auditar quién aprobó qué | · Journal con revisión y comando<br>· Exportable a CSV | SHOULD | M |
| REV-06 | Comentarios por objeto con autor y fecha | Revisión en equipo | · Requiere identidad de usuario | COULD | M |
| REV-07 | Firma / sello de aprobación | Valor legal | · Hash del documento + usuario + fecha<br>· Se estampa en el informe | COULD | M |
| REV-08 | Comparar dos revisiones | Ver qué cambió entre entregas | · `proyecto diff a.redes b.redes`<br>· Altas, bajas y cambios por objeto | SHOULD | L |

## 5. Salida CAD y SIG (EXP)

| ID | Ítem | Valor | Aceptación | Prior. | Esf. |
|---|---|---|---|---|---|
| EXP-01 | Exportar DXF con capas por familia | Es lo que el delineante necesita para el DWG | · Polilíneas 3D para arcos, POINT+CIRCLE para nodos<br>· Capas `PZ_POZO`, `CL_TUBERÍA`…<br>· Ida y vuelta verificada | MUST | M |
| EXP-02 | Exportar GeoJSON con datos de origen | Inventario municipal y SIG | · Punto y línea con propiedades y `Dato`s<br>· CRS declarado | SHOULD | S |
| EXP-03 | Exportar sólo lo aprobado | Entregar sin arrastrar lo observado | · `--solo-aprobados`<br>· Informe de lo omitido | SHOULD | S |
| EXP-04 | Exportar DXF por red (agua / alcantarillado) | Entregas separadas | · Filtro por familia o red | COULD | S |
| EXP-05 | DWG nativo | Lo pidió el cliente | · Requiere biblioteca ODA (licencia)<br>· Se entrega DXF y el CAD hace DWG | WON'T (v1) | XL |
| EXP-06 | PDF del plano | Presentación | · Se resuelve en CAD/ Illustrator desde el DXF/SVG | COULD | M |
| EXP-07 | Exportar vista 3D a SVG | Compartir sin abrir la app | · `redes vista --salida x.svg` (ya existe)<br>· Misma escena que la app | MUST | S |

## 6. CLI e IA (CLI)

| ID | Ítem | Valor | Aceptación | Prior. | Esf. |
|---|---|---|---|---|---|
| CLI-01 | Sobre único `redes.envelope/1` | Un parser para el agente | · 14 claves siempre presentes<br>· `ok:false` ⇔ salida ≠ 0 (test) | MUST | S |
| CLI-02 | `esquema` autodescriptivo | Descubrir sin documentación | · Familias, atributos, subtipos, comandos, códigos<br>· Filtro por familia | MUST | S |
| CLI-03 | Diagnósticos con `hint` y `hint_command` | Errores que enseñan el siguiente paso | · Todo error trae código estable y pista<br>· La pista es un comando ejecutable | MUST | S |
| CLI-04 | `--dry-run` en toda escritura | Probar sin romper | · No escribe y devuelve el plan | MUST | S |
| CLI-05 | Salida estable y determinista | Diferencias y pruebas doradas | · Orden de inserción, sin `HashMap` en salidas<br>· Mismos códigos en cada corrida | MUST | S |
| CLI-06 | Resolución de proyecto | Menos fricción | · `--project`, `$REDES_PROJECT`, búsqueda ascendente<br>· Error con pista si no hay | MUST | S |
| CLI-07 | `vista` sin GPU | Que un agente «vea» el modelo | · SVG de la misma escena 3D<br>· Devuelve conteo de primitivas y cámara | MUST | S |
| CLI-08 | Modo lote (`plan run`) | Automatizar una importación completa | · Lista de comandos atómica con rollback | SHOULD | M |
| CLI-09 | Servidor MCP | Integración con asistentes | · Tools generadas del registro<br>· Perfiles lectura/escritura | COULD | M |
| CLI-10 | `--explain` de la ingesta | Saber por qué un objeto no entró | · Traza del mapeo capa→familia→objeto | SHOULD | S |

## 7. Persistencia y proyecto (PER)

| ID | Ítem | Valor | Aceptación | Prior. | Esf. |
|---|---|---|---|---|---|
| PER-01 | Archivo `*.redes` JSON versionado | Legible, diffeable, inspeccionable por un agente | · `schema: redes.project/N`<br>· Guardado atómico (temporal + rename) | MUST | S |
| PER-02 | Migración de esquema | Abrir proyectos viejos | · Al abrir una versión anterior, migra y avisa<br>· Un esquema futuro falla con mensaje claro | SHOULD | M |
| PER-03 | Autoguardado y recuperación | No perder una revisión | · Guarda cada N cambios en un trabajador<br>· Al reabrir ofrece recuperar | SHOULD | M |
| PER-04 | Bloqueo de archivo | Dos personas no pisan el mismo proyecto | · Aviso si el archivo cambió en disco | COULD | M |
| PER-05 | SQLite para proyectos grandes | > 50 MB o guardado incremental | · Migración transparente desde JSON | COULD | L |
| PER-06 | Empaquetado con el proyecto | Llevar la evidencia con el modelo | · Los DXF/CSV originales se adjuntan al archivo | SHOULD | M |

## 8. Extensibilidad (EXT)

| ID | Ítem | Valor | Aceptación | Prior. | Esf. |
|---|---|---|---|---|---|
| EXT-01 | Familia nueva en un solo lugar | Crecer sin tocar el motor | · Añadir `FamilyDef` basta<br>· Test: la familia aparece en CLI, UI e inspector | MUST | M |
| EXT-02 | Perfiles de ingesta como dato | Cada app de origen, su mapeo | · JSON de perfil versionado<br>· Sin recompilar | SHOULD | M |
| EXT-03 | Exportadores enchufables | Nuevos formatos | · Trait `Exporter` + registro | COULD | M |
| EXT-04 | Scripting del usuario | Automatizaciones locales | · Se prefiere la CLI a un lenguaje embebido | WON'T (v1) | L |
| EXT-05 | API estable para plugins | Ecosistema | · Sólo cuando haya un tercero real | WON'T (v1) | XL |

## 9. Idiomas y accesibilidad (I18N)

| ID | Ítem | Valor | Aceptación | Prior. | Esf. |
|---|---|---|---|---|---|
| I18N-01 | Interfaz y mensajes en español (es-EC) | El usuario y su norma | · Toda etiqueta y mensaje en español<br>· Códigos de error en inglés estable | MUST | S |
| I18N-02 | Glosario ES/EN | Coherencia con CAD y normas | · Términos del dominio con su equivalente | SHOULD | S |
| I18N-03 | Inglés para la CLI | Agentes y colegas | · `--lang en` para mensajes | COULD | M |
| I18N-04 | Accesibilidad (roles, foco, contraste) | Uso institucional | · Roles y etiquetas en la UI<br>· Navegación por teclado | SHOULD | M |

## 10. Documentación y ayuda (DOC)

| ID | Ítem | Valor | Aceptación | Prior. | Esf. |
|---|---|---|---|---|---|
| DOC-01 | Guía de inicio (15 min) | Que alguien vea su red en 3D hoy | · Importar → ver → aprobar → exportar<br>· Comandos copiables y verificados | MUST | S |
| DOC-02 | Referencia de la CLI | Todo comando con ejemplo | · Generada del `esquema` cuando sea posible | MUST | S |
| DOC-03 | Referencia de familias y atributos | Saber qué significa cada campo | · Una ficha por familia con unidades | MUST | M |
| DOC-04 | Guía de la skill de ingesta | Convertir desde cada app | · Un caso por app de origen<br>· Tabla de mapeo y validación | MUST | M |
| DOC-05 | Limitaciones explícitas | Evitar expectativas falsas | · «No calcula», «DXF no DWG», «sin búsqueda de conflictos» | MUST | S |
| DOC-06 | Arquitectura para desarrolladores | Mantener el proyecto | · Este documento + el de Rune | SHOULD | S |
| DOC-07 | Vídeo de 3 minutos | Demostración | · Del DXF al modelo aprobado | COULD | S |

## 11. Colaboración (COL)

| ID | Ítem | Valor | Aceptación | Prior. | Esf. |
|---|---|---|---|---|---|
| COL-01 | Comentarios y menciones | Revisión distribuida | · Requiere usuarios | COULD | L |
| COL-02 | Edición simultánea | Varios revisores | · Fuera de alcance: un archivo, un revisor | WON'T (v1) | XL |
| COL-03 | Exportar acta de revisión | Dejar constancia | · PDF/CSV con aprobados, observados y notas | SHOULD | M |

---

## 12. Plan de releases

### v0.1 — Esqueleto caminante (lo que ya está)

Importar plan/DXF/GeoJSON/CSV · 17 familias · vista 3D con capas y selección · revisión por
objeto · exportar DXF/GeoJSON/SVG · CLI con sobre JSON · app Rune con inspector · demo.

**Demo de aceptación:** DXF de Civil 3D → 44 objetos en 3D → aprobar todo → DXF de vuelta con
los mismos 44 objetos y capas por familia.

### v0.2 — Ingesta real y revisión cómoda (4–6 semanas)

ING-05 perfiles, ING-06 informe con vista previa, ING-10 reproceso, REV-04/05, VIS-07/08/09,
EXP-03, PER-02/03, DOC-02/03/04.

**Demo:** tres orígenes distintos (DXF, plan JSON, GeoJSON) entran con un perfil cada uno; una
revisión de 200 objetos se completa y se exporta sólo lo aprobado.

### v0.3 — Escala y automatización (6–8 semanas)

VIS-11 fino, VIS-10 sección, ING-07 SWMM, ING-08 EPANET, CLI-08 lote, REV-08 diff, PER-05
SQLite, COL-03 acta.

**Demo:** proyecto de 50 000 objetos navegable; lote de importación + aprobación + exportación
sin intervención.

### v1.0 — Producto vendible

Perfiles certificados por app, acta de revisión firmada, instalador Windows, guía completa,
accesibilidad, i18n. DWG sigue fuera (se entrega DXF).

---

## 12.1 Estado de implementación (verificado en v0.1)

| Área | Hecho | Parcial | Pendiente |
|---|---|---|---|
| ING | 01, 02, 03, 04, 06 | — | 05, 07, 08, 09, 10 |
| MOD | 01, 02, 03, 04, 05, 06, 07 | 09 (validación estructural básica) | 08 |
| VIS | 01, 02, 03, 04, 05, 06, 08, 11 | 07 (escenas headless, no selector en UI) | 09, 10, 12, 13, 14 |
| REV | 01, 02, 03, 04 | 05 (journal en el archivo, sin exportar) | 06, 07, 08 |
| EXP | 01, 02, 03, 07 | — | 04, 05, 06 |
| CLI | 01, 02, 03, 04, 05, 06, 07 | — | 08, 09, 10 |
| PER | 01 | — | 02–06 |
| EXT | 01 | — | 02–05 |
| I18N | 01, 02 | — | 03, 04 |
| DOC | 01, 02, 03, 04, 05, 06 | — | 07 |
| COL | — | — | 01, 03 |

Pruebas que sostienen lo marcado como hecho: `cargo test` (31 pruebas de `redes-core` + 2 de
interacción de la app con `Runtime`), ida y vuelta DXF (44 objetos → 44 objetos), conversión de
un `.inp` de SWMM, y capturas headless de las cuatro escenas.

## 13. Lo que este producto **no** hará (por contrato)
1. Calcular hidráulica, hidrología, caudales, presiones o volúmenes de excavación.
2. Verificar cumplimiento normativo.
3. Editar el diseño (se corrige en la app de origen y se reimporta).
4. Producir DWG nativo.
5. Presupuestar.
6. Editar colaborativamente en tiempo real.

Si el cliente pide cualquiera de estas cosas, es un producto distinto con otro alcance, otro
equipo y otro presupuesto — y probablemente dos motores de cálculo que discrepan.
