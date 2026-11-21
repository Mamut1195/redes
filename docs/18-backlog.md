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

## F6 — Modelos propios en el visor (apariencia)

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

## F7 — Datos y cantidades (medición)

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

## F8 — Intercambio con CAD (ida y vuelta)

Objetivo: que lo que el CAD devuelve no pierda familia, código, atributos ni datos.

| ID | Ticket | Alcance | Aceptación | Días |
|---|---|---|---|---|
| F8-01 | **Arreglar el round-trip DXF** | exportar e importar conserva `family`, `ty`, `code`, `attrs`, `datos` | prueba: exportar 44 objetos, borrar el `.redes`, importar el DXF → 44 objetos con los mismos datos | 2 |
| F8-02 | **Bloques con atributos** | cada familia como bloque con atributos (código, DN, cotas) para que el CAD los devuelva | un bloque editado en CAD vuelve con los atributos actualizados | 2 |
| F8-03 | **Importar DWG** | conversión vía ODA File Converter (licencia) o servicio; decisión previa | un `.dwg` real se abre y conserva capas y bloques | 2 + licencia |
| F8-04 | **Fixture real** | archivo del cliente para calibrar capas y bloques | importa sin descartes y con informe de mapeo | 1 |
| F8-05 | **GeoJSON/Shapefile con atributos** | exportar atributos y estilos | QGIS abre el archivo con la tabla completa | 1 |

**Total F8: 8 días** (+ coste de la licencia ODA si se quiere DWG).

---

## F9 — Calidad técnica (deuda con red)

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

## F10 — Acabado visual (que “se vea como en la realidad”)

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
- El resto de las fases (F6-05…F11) se crean cuando el sprint 1 esté en marcha, para no llenar el
  tablero de tickets que aún no se van a tocar.
