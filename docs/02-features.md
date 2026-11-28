# 02 — Funciones: qué hace Redes, qué no y qué falta

> Inventario vigente, comprobado contra el código y la CLI el **2026-11-28** (rama `main` en
> `4b12edb`, `redes esquema`, `--help` de cada comando, `crates/redes-app/src/*.rs`,
> `ingest.rs` y `terrain.rs`). Es lo primero que lee un usuario o un colaborador nuevo. Si el
> código y este documento discrepan, manda el código: se corrige aquí en el mismo commit.
> Sustituye al plan de funcionalidades del 2026-11-20 (IDs ING/MOD/VIS…, plan de releases v0.x).

Leyenda: ✅ listo · 🟡 parcial (funciona, con límites que se dicen) · 📋 planeado (ticket en
[docs/18](18-backlog.md) o «sin ticket»).

## 1. Qué es y para quién

Redes es un **visor y revisor 3D** de redes de agua potable y alcantarillado **ya diseñadas** en
otras aplicaciones (Civil 3D, SWMM, EPANET, Excel, SIG). Sirve a quien revisa un diseño: lo carga,
lo recorre en 3D, deja observaciones, lo aprueba y lo devuelve al CAD en DXF. Hay dos binarios:

- `redes-app`: el visor de escritorio (Rune, Windows 10/11 x64, DX12).
- `redes`: la CLI, con salida JSON `redes.envelope/1`, pensada para agentes.

No calcula, no verifica normativa y no edita el diseño (§3). Arquitectura: [docs/01](01-arquitectura.md).
Uso paso a paso: [docs/04](04-guia.md).

## 2. Lo que hace hoy

### 2.1 Ingesta

| Función | Estado | Cómo se usa | Referencia |
|---|---|---|---|
| Plan JSON `redes.plan/1` (nodos, arcos, terreno, `datos` con fuente) | ✅ | `redes importar plan --archivo plan.json [--dry-run]` | D5, F12-15, [skill plan](../skills/redes-ingesta/references/plan.md) |
| Contrato estricto del plan: campo desconocido → `E_FMT_CAMPO` con sugerencia; otra versión → `E_FMT_VERSION`; nada se rellena (lo ausente se ve como `—`) | ✅ | igual; `--dry-run` valida sin escribir | F12-03, F12-15, D10 |
| DXF exportado por Redes (capa `REDES_INFO`), ida y vuelta sin pérdida | ✅ | `redes importar dxf --archivo x.dxf` | D1, F8-01 |
| DXF de otra aplicación | ✅ por skill | se rechaza con `E_FMT_NOT_REDES`; un agente lo adapta a plan con la skill | D5 |
| SWMM `.inp`, EPANET `.inp`, DXF de Civil 3D, GeoJSON/SIG, tablas Excel/CSV | ✅ por skill | skill [`skills/redes-ingesta`](../skills/redes-ingesta/SKILL.md) (referencias por formato, sin scripts) → plan → `importar plan` | D5, [docs/03](03-skill-ingesta.md) |
| Reimportar una entrega nueva sin perder la revisión (empareja por código) | ✅ | `redes importar reimportar --archivo nuevo.json [--aceptar-bajas] [--tolerancia-pos 0.01]` | D9, F13-06 |
| Sistema de coordenadas | 🟡 | solo UTM sur WGS 84 (`EPSG:327NN`); otro CRS, coordenadas en grados o CRS distinto del proyecto → `E_CRS`; **no reproyecta** | F12-13 |
| Reimportar o importar desde la app | 📋 | hoy solo por CLI | F18-02 |

### 2.2 Topografía

| Función | Estado | Cómo se usa | Referencia |
|---|---|---|---|
| Puntos de terreno de un CSV/TXT | 🟡 | `redes importar terreno --archivo t.csv [--epsg 32717]`. Lee **las tres primeras columnas como E, N, Z**; separa por `,` `;` tabulador o espacio; salta líneas vacías, las que empiezan por `#` y una cabecera solo en la primera línea; descarta NaN e infinitos línea a línea; exige ≥ 3 puntos. Las demás columnas se ignoran | `ingest::import_terreno` |
| Terreno dentro del plan (`"terreno": [[E,N,Z], …]`) | ✅ | en el plan | [skill plan](../skills/redes-ingesta/references/plan.md) |
| TIN por Delaunay **sin restricciones** sobre la planta de los puntos | 🟡 | automático al importar (`delaunator`); descarta triángulos degenerados | `terrain::Terrain::build` |
| Cota y pendiente interpoladas en el TIN, solo dentro del casco convexo (fuera: sin cota, nunca extrapola) | ✅ | lo usan la vista, el perfil y `validar` | `Terrain::z_at`, `slope_at` |
| Terreno aligerado para dibujar (un punto por celda) | ✅ | automático en la app | `Terrain::thinned` |
| Curvas de nivel | 🟡 | se calculan (`Terrain::contours`), pero **ni se dibujan ni se exportan** | F18-08 |
| Terreno en el perfil longitudinal | ✅ | `redes perfil … --paso 10` muestrea la rasante entre pozos | F13-02 |
| Terreno en la salida | 🟡 | solo en glTF (`exportar gltf`, `--sin-terreno` lo quita); no en DXF ni en GeoJSON | F18-08 |
| Ejemplo sobre topografía real (MDT Copernicus GLO-30) | 📋 | `redes proyecto cadena` | F17-09, D17 |

Lo que falta para un levantamiento real está en §4.1.

### 2.3 Modelo y familias

| Función | Estado | Cómo se usa | Referencia |
|---|---|---|---|
| Registro de **46 familias / 315 subtipos** (15 de saneamiento, 19 de agua, 12 comunes), 854 entradas de atributo | ✅ | `redes esquema` (`data.familias`, `total_familias`) | [docs/20](20-familias-captacion-tratamiento.md), [docs/21](21-cifras-medidas.md) §1 |
| Todo número es `Dato{valor, unidad, fuente}` con procedencia | ✅ | inspector «Datos importados (no calculados)» | D4, [docs/01](01-arquitectura.md) §1 |
| Cotas separadas: terreno, tapa, solera (y clave) del nodo; soleras de inicio y fin del tramo | ✅ | en el plan | F12-01, F12-02 |
| Un único espacio de códigos para nodos y tramos vivos | ✅ | `E_STATE_DUPLICATE_NAME`; la ingesta descarta el repetido y lo informa | D11 |
| Topología para navegar y contar, con el sentido de flujo declarado (la contrapendiente es aviso, no se invierte) | ✅ | `perfil`, `validar` | F12-06 |
| Geometría paramétrica de respaldo para cada familia (`parts.rs`, `parts_cadena.rs`) | ✅ | se dibuja cuando el objeto no tiene modelo | D14 |
| Catálogo de familias como datos (no código) | 📋 | — | F15-03 |

### 2.4 Vista 3D (app)

| Función | Estado | Cómo se usa | Referencia |
|---|---|---|---|
| Cámara orbital | ✅ | botón izquierdo gira, derecho o central desplaza, rueda acerca | — |
| Botones de cámara | ✅ | barra: Encuadrar, Planta, Isométrica; panel «Cámaras»: Planta, Isométrica, Perfil (vista lateral rasante, no el perfil longitudinal) | — |
| Dos motores de dibujo: pintor 2D (por defecto) y pase 3D con profundidad, instancias, 3 LOD y recorte | ✅ | `redes-app --gpu3d` (o `REDES_GPU3D=1`) | F14-02…05, [docs/16](16-fase-4-5-gltf-y-3d.md) |
| Capas | 🟡 | chips Terreno, Alcantarillado, Agua potable, Edificios. La capa `nodos` existe en el núcleo (`view::Layers::nodos`) pero **no tiene chip** | [docs/11](11-modos-inspeccion.md) |
| Selección por clic con rejilla espacial | ✅ | clic sin arrastrar | F14-07 |
| Inspector de nodo | 🟡 | familia, subtipo, estado, E/N, cotas, altura, datos con fuente y atributos; **solo los 10 primeros atributos**; muestra «diámetro 1,20 m» aunque el objeto no traiga diámetro (§4.6) | D12 |
| Inspector de tramo | 🟡 | DN, longitud **en planta**, cotas inicial y final, datos; sin atributos | D12 |
| Inspector sin selección: proyecto, CRS, estado, objetos, procedencia (`fuentes`) | ✅ | — | — |
| Solo lectura: no hay controles para editar el diseño | ✅ | prueba `el_inspector_no_edita_parametros_del_diseno` | D12 |
| Etiquetas de código con carriles anti-solape (hasta 28 en pantalla) | ✅ | automáticas | F10-03 |
| Foco sobre el objeto (encuadra la caja del modelo si lo tiene) | ✅ | botón Foco; Salir vuelve a la vista general | F6-07, [docs/11](11-modos-inspeccion.md) |
| Corte vertical con poché | 🟡 | botón Corte; Shift + rueda lo desplaza. Un solo plano, perpendicular al eje E local, por el nodo seleccionado; no corta a lo largo de un tramo | F10-02 |
| Rayos X (lo enterrado translúcido) | ✅ | botón Rayos X o `--rayos-x` | F10-04 |
| Explosión de la pieza enfocada | ✅ | botón Explosión; Shift + rueda la abre | [docs/11](11-modos-inspeccion.md) |
| Perfil longitudinal en panel | ✅ | botón Perfil en el inspector de un tramo | F13-02 |
| Leyenda por forma y color con conteos | ✅ | panel izquierdo | — |
| Modelos `.glb` de biblioteca en su nodo; proxy decimado sin GPU | ✅ | `--gpu3d` para la malla real; sin él, silueta y aviso | F6-01…05 |
| Exageración vertical | 🟡 | fija (×2,0; ×2,6 con contexto urbano; ×1 en foco); sin control en la UI | F18-04 |
| Abrir proyecto | 🟡 | `redes-app --abrir x.redes` (o `-p`, una ruta `.redes` suelta, `REDES_PROJECT`); sin diálogo de abrir. Sin nada, abre el demo; si lo pedido falla, abre vacío y lo dice | — |
| Capturas y árbol sin ventana | ✅ | `--snap out.png --scene <escena>`, `--tree arbol.json`, `--size`, `--scale` (Rune). 16 escenas: `iso`, `planta`, `perfil`, `detalle`, `ciudad`, `calle`, `catalogo`, `vitrina`, `pozos`, `pieza`, `foco`, `corte`, `explosion`, `ptap`, `ptar`, `validar` | [docs/00](00-investigacion-rune.md) |
| Vista SVG sin GPU para agentes | ✅ | `redes vista --salida v.svg [--azimut --elevacion --distancia --ancho --alto]`; sin capas, corte ni foco | — |
| Buscar por código, filtrar por estado o familia | 📋 | no existen en la app (la CLI sí filtra: §2.5) | §4.2 |

### 2.5 Revisión

| Función | Estado | Cómo se usa | Referencia |
|---|---|---|---|
| Estados pendiente / aprobado / observado (observado ⇔ alguna observación abierta) | ✅ | panel «Revisión» con conteos | D7 |
| Aprobar y observar un objeto | ✅ | inspector (Aprobar, Observar) o `redes objeto aprobar/observar --codigo X` | D7, F12-07 |
| Observaciones con historial: añadir, cerrar, listar | ✅ | inspector (Añadir observación, Cerrar) o `objeto observar`, `objeto cerrar-observacion`, `objeto observaciones` | D7, F13-03 |
| Reabrir un aprobado | 🟡 | solo CLI: `objeto reabrir` | D7 |
| Listar por estado o familia | ✅ | `redes objeto lista [--estado pendiente] [--familia saneamiento.pozo]` | — |
| Aprobar el proyecto (exige 0 pendientes y 0 observados) | 🟡 | solo CLI: `redes proyecto aprobar` (`E_STATE_PENDING`, `E_STATE_OBSERVED`) | F12-07 |
| Deshacer y rehacer 20 pasos (revisión y modelo asignado) | ✅ | botones Deshacer, Rehacer | F14-08 |
| Guardar atómico (temporal + renombrar) | ✅ | botón Guardar (el `*` indica cambios) | F15-09 (sigue en el hilo de UI) |
| Comparar dos entregas por código (diseño y revisión) | ✅ | `redes proyecto diff --a a.redes --b b.redes [--solo diseno\|revision] [--salida d.csv]` | F13-05 |
| Acta de revisión CSV (Excel) o HTML imprimible | 🟡 | solo CLI: `redes acta --salida acta.html` | F13-04, D2 |
| Journal con revisión, comando y fecha UTC; sin identidad salvo `REDES_JOURNAL_USUARIO=1` | ✅ | automático | D2 |

### 2.6 Validación geométrica

| Función | Estado | Cómo se usa | Referencia |
|---|---|---|---|
| 10 reglas sobre las cotas importadas: `contrapendiente`, `pendiente`, `salto_pozo`, `recubrimiento`, `profundidad_pozo`, `separacion`, `dn_aguas_abajo`, `tramo_huerfano`, `nodo_aislado`, `longitud` | ✅ | `redes validar [--solo pendiente,recubrimiento] [--reglas r.json] [--guardar]`; es un informe (`ok:true` con hallazgos) | D6, F13-01 |
| Umbrales del usuario; los de fábrica son referencia, no norma; falta un dato → `no_evaluable` | ✅ | `--reglas`, `Project::reglas_validacion` | D6 |
| Panel Validación con navegación a cada hallazgo | ✅ | botón Validar en el panel izquierdo; escena `--scene validar` | F13-01 |
| Tapa o rasante declaradas frente al TIN; objetos fuera del terreno | 📋 | no hay regla | F18-07 |

### 2.7 Salida

| Función | Estado | Cómo se usa | Referencia |
|---|---|---|---|
| DXF: capa por familia con color, metros, bloques con atributos, polilíneas 3D, capa `OBSERVACIONES`, capa `REDES_INFO` (apagada) para la vuelta | 🟡 | `redes exportar dxf --salida x.dxf [--solo-aprobados]`. Sin abrir aún en AutoCAD/BricsCAD reales | D1, F8-01, F8-02, F12-08 |
| GeoJSON con atributos y datos como propiedades | ✅ | `redes exportar geojson --salida x.geojson` (sin `--solo-aprobados`) | F8-05 |
| glTF binario `.glb` (piezas iguales comparten malla; > 4 GB → `E_IO_TOO_LARGE`) | ✅ | `redes exportar gltf --salida x.glb [--sin-terreno]` | F14-05, [docs/16](16-fase-4-5-gltf-y-3d.md) |
| Cantidades con procedencia: objetos, longitudes por DN y material, volúmenes, áreas, pesos, discrepancias declarado/medido | ✅ | `redes cantidades [--grupo …] [--csv c.csv] [--tolerancia 10]` | D3, F7 |
| Perfil longitudinal SVG y JSON | ✅ | `redes perfil --desde P1 --hasta P9 --salida p.svg` o `--tramos T1,T2` | F13-02 |
| Exportar desde la app | 📋 | no hay botón: todo sale por CLI | F18-02 |

### 2.8 Biblioteca de piezas

| Función | Estado | Cómo se usa | Referencia |
|---|---|---|---|
| Revisar un `.glb` (mallas, triángulos, medidas, avisos) | ✅ | `redes biblioteca revisar --glb x.glb` | [docs/17](17-biblioteca-modelos.md) |
| Importar y normalizar (base en 0, centrado, escala) con licencia y autor | ✅ | `redes biblioteca importar --glb x.glb --familia … [--licencia --autor]` | F11-05 |
| Medir un `.glb` (volumen, área, materiales) | ✅ | `redes biblioteca cantidades --glb x.glb` | F7 |
| Listar y aplicar a una familia (escribe `modelo` y datos con fuente `modelo:<id>`, sin pisar otra fuente) | ✅ | `biblioteca lista`, `biblioteca aplicar --modelo … --familia …`; en la app, «Modelos de la biblioteca» en el inspector | F6-06, F7-01 |
| Anclas de conexión (la tubería nace en el puerto del modelo) | ✅ | automático al aplicar | F6-08 |
| Hornear piezas paramétricas con Blender sin ventana, en lote y con caché por clave; un dato que falta no se rellena (pieza de respaldo y `W_PARAMETRO_FALTANTE`) | ✅ | `redes biblioteca hornear --receta tools/blender/build_manhole.receta.json [--parametros …] [--sin-blend] [--blender …]` | D15, D16, F17-02 |
| Recetas disponibles | 🟡 | solo el pozo (`tools/blender/build_manhole.*`), en piloto | F17-03 |
| Tubería, accesorios, captación, planta de tratamiento | 📋 | — | F17-04…F17-07 |
| Cadena completa dibujada solo con piezas de Blender; biblioteca horneada en el paquete | 📋 | — | F17-08, F17-10, D17 |

### 2.9 CLI para agentes

| Función | Estado | Cómo se usa | Referencia |
|---|---|---|---|
| Sobre único `redes.envelope/1` con 14 claves siempre presentes; `ok:false` ⇔ salida ≠ 0 | ✅ | toda la CLI; pruebas de contrato con el binario real | `envelope.rs`, F9-01 |
| Introspección: 36 comandos (con el alias `capacidades`), 29 códigos `E_*` con su código de salida, familias, formatos | ✅ | `redes esquema` | — |
| Opciones globales `-p/--project` (o `REDES_PROJECT`, o búsqueda ascendente), `--pretty`, `--dry-run`, `--quiet` | ✅ | todas las órdenes | — |
| Errores tipados (hoy códigos dentro de `String`) | 📋 | — | F15-06 |
| Fachada única `redes_core::api` y canal `Comando → aplicar → Evento` | 📋 | — | F15-13, F15-14 |
| `--pretty` automático en terminal | 📋 | — | F16-12 |

### 2.10 Proyecto y persistencia

| Función | Estado | Cómo se usa | Referencia |
|---|---|---|---|
| Archivo `*.redes` JSON `redes.project/3`; solo se abre la versión vigente (`E_PROJ_SCHEMA`), sin migraciones hasta v1 | ✅ | `redes proyecto nuevo --salida x.redes`, `proyecto info` | D10, F15-01 |
| Proyectos de demostración | ✅ | `redes proyecto demo\|ciudad\|plantilla\|cadena\|catalogo\|catalogo-total --salida …` | F11-04 |
| Paquete portable (zip con binarios, demos, docs y lanzadores) | ✅ | `powershell -File scripts/empaquetar.ps1` | F11-03 |
| Guardar fuera del hilo de UI | 📋 | — | F15-09 |
| Instalador MSI/winget firmado | 📋 | — | F16-06 |

### 2.11 Rendimiento

| Función | Estado | Cómo se usa | Referencia |
|---|---|---|---|
| Índices por id, código y nodo → tramos | ✅ | automático | F14-01 |
| Red de 20k tramos fluida (escena, apertura, reimportación) con banco y umbrales en el CI | ✅ | `cargo test --release -p redes-core --test banco -- --include-ignored` | F14-04…F14-10 |
| Cifras | ✅ | §5 | [docs/21](21-cifras-medidas.md) |

## 3. Lo que NO hace, por decisión

| No hace | Por qué | Decisión |
|---|---|---|
| Cálculo hidráulico, hidrológico, de caudales, presiones ni movimiento de tierras | Dos motores dan dos verdades; el número lo firma quien lo calculó en su aplicación. Todo número entra como `Dato` con fuente | D4, [docs/01](01-arquitectura.md) §1 |
| Verificar normativa o «cumplimiento» | `validar` marca discrepancias geométricas con umbrales del usuario; no es una norma | D6 |
| Editar el diseño (DN, cotas, trazado, parámetros) | Se corrige en la aplicación de origen y se reimporta; la app solo escribe revisión y el modelo 3D asignado | D12, D9 |
| Inventar o rellenar datos | Lo ausente se muestra `—`; lo que no cuadra es discrepancia, no corrección | D4, F12-03, D16 |
| Leer o escribir DWG | DXF basta para la ida y vuelta; DWG exige licencia ODA | D1 |
| Adaptadores por programa (SWMM, EPANET, Civil 3D, SIG, Excel) en el código | Los adapta un agente con la skill guiada, que escribe el plan; la CLI lo valida | D5 |
| Valorar cantidades o presupuestar | Emite cantidades con procedencia; los precios son de otra aplicación | D3 |
| Identificar o firmar al revisor | El journal guarda revisión, comando y fecha; el usuario solo con `REDES_JOURNAL_USUARIO=1` | D2 |
| Abrir `.redes` de versiones anteriores | No hay nada en producción; se regenera reimportando la entrega. Migraciones al congelar v1 | D10 |
| Reproyectar coordenadas | Solo UTM sur WGS 84; la entrada llega ya proyectada | F12-13 |
| Ejecutar Blender desde la app | La app abre sin Blender; hornear es de la CLI | D16 |
| Producir piezas con un MCP de Blender | Las piezas salen de scripts versionados con `bpy` sin ventana | D15 |
| Edición colaborativa en tiempo real | Un archivo, un revisor | [docs/01](01-arquitectura.md) |

Decisiones abiertas: A1 (vertedero), A2 (pozo profundo), A3 (repo público) en [docs/22](22-decisiones.md).

## 4. Lo que falta

### 4.1 Topografía para un levantamiento real (F17-11, F17-12, F17-13, F18-06, F18-08)

Hoy `importar terreno` toma las tres primeras columnas como E, N, Z y arma un Delaunay sin
restricciones (`ingest.rs` `import_terreno`, `terrain.rs` `Terrain::build`). `SurfacePoint`
solo guarda `{e, n, z}`. Para un levantamiento de campo falta:

| Falta | Qué pasa hoy |
|---|---|
| Formatos de columnas PNEZD, PENZD, NEZ, ENZ con número de punto y descripción; orden elegible o leído de la cabecera | Un PNEZD **se lee mal sin aviso**: el número de punto entra como E, el norte como N y el este como Z. `rechazar_lonlat` no lo detecta porque N > 90. Un NEZ cambia E por N en silencio |
| Número y descripción (código) del punto en el modelo | Se descartan: no se guardan, no se muestran, no salen |
| Puntos clasificados por código: terreno frente a no terreno (árboles, postes, edificaciones, tapas existentes, cercas) | Todo punto entra al TIN y deforma la superficie |
| Líneas de quiebre como restricciones (bordes de vía, cunetas, ejes, pie y corona de talud, quebradas) | El TIN no es restringido: redondea los quiebres. Haría falta un Delaunay restringido (CDT) |
| Polígono de borde y de exclusión (huecos: edificios, lagunas) | El TIN cubre el casco convexo: triángulos largos sobre las concavidades dan cotas falsas que la vista, el perfil y `validar` usan |
| MDT en rejilla: GeoTIFF y ESRI ASCII (`.asc`) | No hay lector. F17-09 (Copernicus GLO-30) tendrá que convertir la rejilla a puntos fuera de la app |
| Curvas de nivel como entrada (polilíneas con cota en DXF), TIN de Civil 3D (LandXML), nubes LAS/LAZ | No hay lector |
| CRS y datum | Solo UTM sur WGS 84 (`327NN`), sin reproyección ni hemisferio norte; sin datum vertical (ortométrico/EGM2008 frente a elipsoidal): Copernicus va en EGM2008 y nada lo anota |
| Más de una superficie (existente y proyectada) y añadir puntos a la existente | Importar reemplaza el terreno entero |
| Puntos duplicados (misma E, N con distinta Z) e informe de calidad (densidad, triángulos largos, espigas) | No se detectan ni se informan |
| Curvas de nivel en la vista y en el DXF; terreno en DXF y GeoJSON | Se calculan y no se usan; el terreno solo sale en glTF |
| Aligerado que respete quiebres y picos | `thinned` guarda el primer punto de cada celda: puede perder picos en la vista |
| Referencia de topografía en la skill de ingesta | No hay `references/topografia.md`; el plan solo admite `terreno: [[E,N,Z]]` |

Con D5, el mapeo de columnas (PNEZD, NEZ, descripciones) puede ir por la skill, pero el plan
necesita campos nuevos (código del punto, quiebres, bordes) y el núcleo un TIN restringido. El
TIN es geometría del levantamiento, no cálculo de ingeniería: no cruza D4 mientras no derive
volúmenes ni diseño.

### 4.2 App

| Falta | Ticket |
|---|---|
| Buscar un objeto por código | F18-01 |
| Filtrar por estado de revisión y por familia | F18-01 |
| Chip de la capa `nodos` (existe en `view::Layers`, no en la app) | F18-01 |
| Botones de acta, reimportar, exportar, reabrir y aprobar el proyecto | F18-02 |
| Todos los atributos en el inspector de nodo (hoy 10) y atributos en el de tramo | F18-03 |
| Longitud 3D en el inspector de tramo (hoy en planta; `cantidades` ya da la 3D) | F18-03 |
| Corte a lo largo de un tramo u orientable | F18-04 |
| Control de exageración vertical | F18-04 |
| Seleccionar un objeto en una escena `--snap` (capturar el inspector) | F18-05 |
| Guardar fuera del hilo de UI | F15-09 |

### 4.3 Validación y salida

| Falta | Ticket |
|---|---|
| Regla de tapa o rasante declaradas frente al TIN, y de objetos fuera del terreno | F18-07 |
| `--solo-aprobados` en GeoJSON; DXF por red (agua o alcantarillado) | F18-08 |
| Abrir un DXF exportado en AutoCAD/BricsCAD y dejar constancia | F18-09 |
| Reimportar reaplica las anclas de `biblioteca aplicar` | F18-10 |

### 4.4 Piezas, arquitectura y release

- F17: pozo piloto (F17-03), tubería (F17-04), accesorios (F17-05), captación (F17-06), planta
  (F17-07), cadena solo con piezas de Blender (F17-08), topografía real (F17-09), biblioteca horneada
  en el paquete (F17-10), topografía de levantamiento (F17-11), CRS y datum vertical (F17-12), calidad
  del terreno (F17-13).
- F18: buscar y filtrar (F18-01), acciones de revisión en la app (F18-02), inspector completo (F18-03),
  corte orientable y exageración (F18-04), teclado y `--snap` con selección (F18-05), varias superficies
  (F18-06), `validar` contra el terreno (F18-07), salidas que faltan (F18-08), DXF en un CAD real
  (F18-09), reimportar con anclas (F18-10), ayuda de la CLI (F18-11), diámetro inventado (F18-12).
- F15: agregado `Project` encapsulado (F15-02), catálogo como datos (F15-03), sin `match` por familia
  (F15-04), funciones gigantes (F15-05), errores tipados (F15-06), demos tras `feature` (F15-07),
  núcleo sin E/S (F15-08), sin `panic` (F15-10), fachada `api` y canal de comandos (F15-13…15).
- F16: fixtures reales (F16-03), cobertura (F16-04), CI de publicación (F16-05), instalador
  (F16-06), icono (F16-07), CHANGELOG (F16-08), licencias (F16-09), mensajes para personas
  (F16-10), avisos de compilación (F16-11), `--pretty` automático (F16-12).
- Fuera de v1, sin ticket: IFC/Revit, servidor MCP, modo lote, autoguardado, atributos definidos por
  el usuario.

### 4.5 Decisiones pendientes que cambian funciones

A1 y A2 ([docs/22](22-decisiones.md)); sumideros de la topología (`is_sink`); identidad de la
revisión (F11-02, hoy cerrada por D2).

### 4.6 Contradicciones encontradas al verificar

- (F18-12, MAM-1155) El inspector muestra «diámetro 1,20 m» cuando el nodo no trae `diametro_interior`
  (`Node::diametro`, `model.rs`, `unwrap_or(1.20)`): es un dato inventado en pantalla, contra D4 y
  F12-03. Debería mostrarse `—`.
- (F18-05) El aviso del foco dice «Flechas para girar», pero la app no atiende el teclado.
- (F17-12) `skills/redes-ingesta/references/plan.md` dice que el TIN del plan «se etiqueta siempre como
  32717»; el código usa el EPSG del plan.
- (corregido en d8fb19b) `CLAUDE.md` citaba la variable `REDES_GPU`; la app lee `REDES_GPU3D=1` (`gpu3d::activo`). También
  lista GeoJSON entre lo que importa `ingest`, que desde D5 no lo lee.
- (F18-11) `redes proyecto aprobar --help` dice que exige «que no queden objetos pendientes»; también exige
  que no queden observados (`E_STATE_OBSERVED`).

## 5. Cifras clave

De [docs/21](21-cifras-medidas.md), que manda sobre cualquier cifra:

| Medida | Valor |
|---|---:|
| Familias / subtipos | 46 / 315 |
| Cadena completa (`redes proyecto cadena`) | 90 objetos, 72 tramos, 961,4 m |
| 20k tramos, release: ingesta / escena / abrir / reimportar | 182 ms / 7,0 ms / 115 ms / 349 ms |
| 50k tramos: escena | 13,4 ms |
| Pase 3D, 20k tramos: GPU / primer frame / RAM pico | 22,9 MB / 135 ms / 319 MB |
| glTF de 20k tramos | 26,2 MB en 3,6 s |
| Cambio de cámara en 3D (ciudad) | 0,008 ms, sin regenerar la malla |
| Deshacer, 20 pasos con 20k tramos | 14,9 kB (antes 270 MB) |
| Horneado: 100 pozos en un proceso / todo en caché | 7,2 s / 86 ms sin Blender |
| Pruebas / mutaciones | 299 / 41 |
