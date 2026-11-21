# 12 — Revisión de arquitectura: fases 1–3

> Revisión del código, no de los documentos. Se leyó `redes-core` completo, la CLI y la app, se
> corrieron las pruebas y se midió con el binario. Los documentos 10 y 11 se usan solo como
> **afirmaciones a verificar**; cuando el código y el documento discrepan, se señala el par.

**Base de la revisión** (instantánea 2026-11-18 18:43 −05, sin git en el árbol):

| Archivo | Líneas | mtime | Nota |
|---|---|---|---|
| `families.rs` | 1 539 | 15:58 | generado; hay un `families.rs.bak` de 47 KB |
| `parts.rs` | 890 | 18:06 | |
| `view.rs` | 1 306 | 18:41 | se le añadió `pieza_para_exportar` **durante** esta revisión |
| `redes-app/src/main.rs` | 1 219 | 18:42 | ídem |
| `redes-cli/src/main.rs` | 777 | 18:36 | |
| `gltf.rs` | 320 | 18:39 | **fase 4 en curso por otro agente**; entra en el veredicto |

`cargo test -p redes-core` → 49 + 1 doctest en verde. `cargo build` → OK (sin warnings propios;
los 13 warnings son del `vendor/rune`). Las cifras de línea citadas valen para esta instantánea:
el árbol se está editando en paralelo.

---

## 0. Veredicto en una línea

**No**: las fases 1–3 no sostienen todavía un pase 3D con profundidad (fase 5) ni un glTF honesto
(fase 4), porque hay dos fallos de geometría que ningún z-búfer arregla —los tubos se generan en
`f32` con coordenadas UTM y colapsan, y 7 de las 29 familias no se dibujan jamás— y porque el
"corte" no corta tubos ni encuentra un interior que mostrar. Son 4–6 días de refactor localizado,
no un rediseño.

---

## 1. Corrección

### 1.1 Tuberías y canales en `f32` con coordenadas de mundo — **BLOQUEANTE**

`parts::tuberia` convierte a `f32` las coordenadas **absolutas** antes de restar el origen:

```rust
// parts.rs:826-827
let p0 = [a.x as f32, a.y as f32, a.z as f32];
let p1 = [b.x as f32, b.y as f32, b.z as f32];
```

El modelo vive en UTM 17S (`demo.rs:14` → E≈760 000, N≈9 796 000). En `f32` el paso mínimo en
N≈9,8·10⁶ es **1,0 m**, mayor que el radio de cualquier tubería. Comprobado:

```
N = 9796036  → [single](N + 0.125) = 9796036   (un solo valor para todo el anillo)
E = 760060   → paso 0,0625 m
```

Todo anillo de sección colapsa a una recta en el eje N: **el tubo se dibuja como una cinta plana
de espesor cero**, y las juntas (campanas) también. Es coherente con lo que muestran las capturas
del propio repo (`docs/img/modo-corte.png`, `vista-detalle.png`: tuberías como tablones). Ninguna
prueba lo detecta porque solo se comprueba `len() > 0` y una `bbox` en coordenadas locales
(`parts.rs:859-889`). Afecta igualmente al exportador: `gltf.rs:104` llama al mismo
`parts::tuberia` con las mismas coordenadas de mundo, así que **el `.glb` de la fase 4 nace con el
mismo defecto**.

Arreglo: `parts` nunca debe ver coordenadas de mundo. Pasar `P3` ya locales (restar `p.origin` en
`f64` y luego convertir), o `tuberia(a_local, b_local, …)`; y una prueba de regresión que exija que
la sección tenga ≥ 3 valores distintos en los dos ejes para un tubo en N≈9,8·10⁶.

Daño colateral en el mismo sitio: las campanas se construyen como cajas **alineadas a los ejes** y
desplazadas media caja:

```rust
// parts.rs:843-848
let caja_j = caja(d_ext*1.12, d_ext*1.12, d_ext*1.25).mover([
    centro[0] - d_ext*0.56, centro[1] - d_ext*0.56, centro[2] - d_ext*0.62,
]);
```

`caja` ya está centrada en planta (`parts.rs:141-143`), así que restar `0.56·d` la corre medio
ancho en X y en Y: la junta queda al lado del tubo, no sobre él; y en un tubo inclinado o diagonal
no sigue la dirección. Debe generarse en el marco del tubo (como hace `barra`).

Y `parts::canal` (nunca llamada, ver §4) ignora las cotas: calcula `az`/`bz` y las tira
(`parts.rs:813-814`), extruyendo de z=0 a z=alto en coordenadas de mundo. Es código muerto, pero
si alguien lo cablea dibuja los canales 2 500 m bajo el terreno.

### 1.2 Siete familias del catálogo no se dibujan nunca — **BLOQUEANTE**

`build_opts` filtra los nodos por capacidades:

```rust
// view.rs:644-646
if !def.can(caps::HYDRAULIC_NODE) && !def.can(caps::ATTACHABLE) { continue; }
```

Las familias cuyo `caps` no incluye ninguna de las dos **desaparecen del 3D**, aunque tengan
geometría escrita y estén en el catálogo de promoción:

| Familia | `caps` en `families.rs` | Geometría que existe y no se usa |
|---|---|---|
| `comun.accesorio` | `1088` (solo EDGE+REPORTABLE) | `parts::accesorio` (564) |
| `comun.tapa` | `1129` | `pieza_local` (1028-1033) |
| `comun.escalera` | `1163` | `pieza_local` (1034-1045) |
| `comun.cuerpo_receptor` | `1338` | `parts::cuerpo_receptor` (727) |
| `agua.dado_anclaje` | `1366` | `pieza_local` (1052) |
| `comun.cruce` | `1417` | `parts::cruce` (747) |
| `comun.hito` | `1449` | `parts::hito` (719) |

Son 7 de 29 (24 %). La prueba que debería cubrirlo no pasa por el filtro: `todas_las_familias_dibujan`
(`view.rs:1128`) llama `pieza_local` directamente, así que **la suite verde certifica algo que el
render no hace**. El catálogo (`demo_catalog.rs:139-221`) coloca esas piezas y el usuario ve huecos;
la captura `docs/img/catalogo-general.png` los tiene.

Arreglo: derivar "dibujable" de la geometría declarada (`geom == Point`, `topo != Link`) o añadir
`caps::DRAWABLE`; y reescribir la prueba contra `build_opts`, contando primitivas por familia.

### 1.3 El corte no corta tubos y el pozo no tiene interior — **BLOQUEANTE** (fase 2)

`cortar_tri` se aplica **solo** dentro del bucle de nodos (`view.rs:719-722`). Los arcos se
convierten en primitivas directamente (`view.rs:630-637`) sin pasar por el recorte. El documento
([11-modos-inspeccion](11-modos-inspeccion.md) §2) afirma: *«El recorte es por triángulo
(Sutherland-Hodgman en el semiplano), así que también corta tubos»* — falso. La captura
`docs/img/modo-corte.png` lo muestra: el pozo está cortado y las dos tuberías lo atraviesan enteras.

Peor: el pozo que se corta **es macizo**. El fuste es `cilindro` (`parts.rs:405-409`) y el cono
`tronco` (`parts.rs:413-417`), sin pared interior. No hay hueco, ni `espesor_pared`, ni banqueo, ni
escalera visible "por dentro". El mismo documento promete *«ver el interior real del pozo: espesor
de pared, escalera, banqueo, tubos entrando»*: hoy no hay interior que ver. El atributo
`espesor_pared` (`families.rs:447`) es decorativo.

Arreglo, en este orden: (1) recortar también los arcos; (2) fuste como `tubo(r, r − espesor_pared)`
para que el corte tenga sentido; (3) cara de sección rellena (ya está en "lo que sigue", doc 11 §9).

### 1.4 Explosión: correcta en el reparto, mal conectada al resto

`view.rs:699-711` reparte por índice de parte y escala con la altura de la pieza; funciona
(`la_explosion_separa_las_partes_del_objeto_enfocado`). Dos grietas:

- Solo explota el **nodo** enfocado (`opts.foco == Some(n.id)`); los arcos no tienen partes y no
  explotan nunca.
- `alternar_explosion` (app `main.rs:219-233`) activa `explosion` aunque el `foco` quede en `None`
  si la selección es un arco (`node_by_code` → `None`): el aviso dice "Explosión activa" y no se
  mueve nada.
- `Opciones.explosion` se documenta `0..2.5` (`view.rs:196`) pero no se acota en el motor; solo la
  rueda lo acota en la app (`main.rs:462`).

### 1.5 `families.rs` generado: atributos duplicados y unidades incoherentes — **IMPORTANTE**

El generador emite el bloque estándar (`dn()`, `material()`, `estado()`…) **y además** los
atributos del catálogo, sin deduplicar. Resultado medido sobre el registro real:

| Familia | Duplicado | Líneas |
|---|---|---|
| `saneamiento.tuberia` | `diametro_nominal`, `material` | 507+532, 510+535 |
| `agua.tuberia` | `diametro_nominal` | 560+581 |
| `agua.llave` | `diametro_nominal` | 715+722 |
| `agua.medidor` | `diametro_nominal` | 781+785 |
| `saneamiento.acometida` | `diametro_nominal` | 811+817 |
| `agua.hidrante` | `diametro_nominal` | 1191+1201 |
| `agua.dado_anclaje` | `diametro_nominal` | 1368+1371 |
| `agua.acometida` | `diametro_nominal` | 1392+1397 |

Consecuencias reales: `FamilyDef::attr` devuelve **el primero** (`families.rs:371-373`), así que el
segundo es código muerto que nadie ve; `redes esquema` **publica la clave dos veces** (verificado
en el JSON: `saneamiento.tuberia` → 24 atributos con `diametro_nominal` y `material` repetidos), de
modo que un agente que genere formularios desde el esquema produce campos duplicados; y la prueba
`claves_y_prefijos_unicos` (`families.rs:1479`) solo comprueba unicidad **entre** familias, nunca
dentro de una.

Unidades incoherentes dentro de la misma familia (el 3D usa metros, el inspector muestra la unidad
declarada):

- `comun.cuerpo_receptor.amplitud_marea` → `Unit::M2` (`families.rs:1345`); una amplitud de marea
  es una longitud.
- `agua.valvula.presion_tarado` → `Unit::None` (`683`), mientras `presion_nominal` es `Mca` (`668`).
- `agua.llave.dn_red` / `dn_acometida` → `None` (`719-720`), aunque son diámetros; `agua.medidor`
  `dn_tuberia`/`dn_bypass` (`788`, `790`) y `comun.accesorio` `dn_ramal`/`dn_salida` (`1097`,
  `1100`) igual.
- `saneamiento.acometida.pendiente` → `None` (`818`) mientras en `saneamiento.tuberia` es `Mperm`
  (`521`): la misma magnitud con dos unidades.
- `saneamiento.descarga.espesor_enrocado` → `None` (`632`) siendo un espesor en metros.
- `saneamiento.camara.tirante` → `None` (`847`) siendo una altura de agua.
- Dentro de una misma familia conviven mm y m para geometría: `agua.tanque.diametro` en `Mm` (`750`)
  y `altura_util` en `M` (`751`); `agua.captacion.diametro` `Mm` (`993`) y `largo` `M` (`990`).

Además el generador no está en el árbol: no hay script (`*.py/*.ps1/*.js`) que produzca
`families.rs`; solo el `.bak`. El archivo de 91 KB es, a efectos prácticos, **editado a mano**.

### 1.6 App: estado, deshacer y edición paramétrica

- **La edición no pasa por el journal.** `ajustar` modifica `z_invert`/`diametro_*` y reconstruye,
  pero no llama a `Project::commit` (app `main.rs:247-272`), mientras `set_estado` sí (`main.rs:119`).
  Se rompe el invariante de `model.rs:336-339`: `revision` y `journal` dejan de describir el
  documento, y un archivo guardado tras editar tres veces sigue con la revisión anterior.
- **`recordar` clona el proyecto entero en cada clic** (`main.rs:156-161`), y se llama antes de
  saber si hay algo que cambiar (`main.rs:249`). Con la selección en un arco, `ajustar` no encuentra
  nodo, no cambia nada, marca `dirty` y apila un paso no-op. 20 pasos × proyecto completo (el demo
  ciudad son 1,1 MB de JSON) = ~20 MB y creciendo.
- **`deshacer` no revierte la revisión** (`main.rs:235-244`): aprobar/observar no entra en la pila
  (decisión explícita, doc 11 §7, de acuerdo), pero tampoco se restaura `sel` ni `opts.foco`.
- **Panic latente**: `(z_invert + delta).clamp(z_terrain - 8.0, z_rim - 0.6)` (`main.rs:254-255`).
  Si la tapa está a más de 7,4 m bajo el terreno (o `z_rim < z_terrain − 7,4`), `min > max` y
  `f64::clamp` **entra en pánico**. Un archivo importado con `z_rim == z_invert` y 10 m de terreno
  encima basta.
- **Etiquetas invertidas**: `− prof.` suma +0,25 a la clave (profundiza) y `+ prof.` resta
  (reduce profundidad) — `main.rs:648-649` contra `main.rs:253-257`. El texto dice lo contrario de
  lo que hace.
- **`+ Ø` escribe `diametro_interior` en familias que no lo declaran** (`main.rs:258-262`): en una
  válvula crea un atributo fuera de esquema que el inspector no muestra (recorre `def.attrs`,
  `main.rs:662`) y que el dibujo no usa (la válvula se mide por `diametro_nominal`). El usuario ve
  "cambió" y no cambia nada.
- **Guardado no atómico**: `State::save` usa `std::fs::write` dentro de un hilo suelto
  (`main.rs:127-136`) en vez de `project_io::save`, que sí es atómico (temporal + `sync_all` +
  rename, `project_io.rs:22-37`). `dirty = false` se pone antes de saber si escribió, dos clics
  seguidos pueden truncar el archivo y el fallo solo va a `eprintln!`.
- **Carga sin validación**: `load` solo comprueba el prefijo del esquema (`project_io.rs:40-47`).
  No valida `next_slot > max(slot)` ni ids únicos: un archivo editado a mano puede producir
  `ElementId` repetidos y `node(id)` devolverá el primero en silencio (`model.rs:261-266`). Tampoco
  hay compactación: los `deleted` se guardan para siempre y `next_slot` solo crece.
- **El viewport se queda viejo al redimensionar**: `on_layout` solo re-encuadra la primera vez
  (`main.rs:417-421`); `Camera::viewport` no se actualiza, y como `project` centra con
  `viewport/2` (`view.rs:111`), la escena queda desplazada hasta el siguiente clic.
- **`pick` miente con los arcos**: selecciona por la recta entre el primer y el último vértice
  (`main.rs:281-296`), no por el trazado; un colector en L se selecciona desde fuera de su
  geometría. Y no respeta capas apagadas.

### 1.7 CLI, ingesta y export

- **`--dry-run` miente en importación**: `dry_import` solo mira `metadata().len()`
  (`cli main.rs:610-618`). Verificado: `redes importar dxf --archivo no-existe.dxf --dry-run`
  devuelve `ok: true, bytes: 0, exit 0`. El `--dry-run` global se anuncia como "valida y reporta
  sin escribir" (`cli main.rs:36-38`).
- **`exportar gltf` reporta `"primitivas": 0`** (`cli main.rs:739`), un número inventado.
- **DXF**: la tabla `LAYER` se declara con `70\n0` — **cero capas** — mientras las entidades
  referencian capas (`export.rs:24` vs `export.rs:37`). El `BTreeMap layers` que se llena en
  `export.rs:32` y `60` no se escribe nunca. AutoCAD las crea al vuelo, un lector R12 estricto no.
  Además el `CIRCLE` usa `node.diametro()` con su default de 1,2 m para **toda** familia
  (`export.rs:41`) y se sitúa en `z_rim` mientras el `POINT` está en `z_invert` (`export.rs:37`
  vs `45`): dos datums en el mismo objeto.
- **La polilínea DXF no lleva diámetro**: el delineante recibe el eje y tiene que volver a teclear
  todos los DN, justo lo que el doc 10 §1 dice evitar.
- **Ingesta**: `default_subtype` (`ingest.rs:239-252`) mapea `agua.llave → "columna"`, subtipo que
  no existe en `agua.llave` (sus subtipos son `corporacion`, `banqueta`, `pase`, `tapon`). Todo
  `agua.llave` importado por DXF/GeoJSON sin subtipo **se descarta** con `subtipo 'columna' no
  existe`. El fallback `_ => "inspeccion"` (`ingest.rs:249`) mete `inspeccion` en familias que no
  lo tienen (p. ej. `comun.tapa`), con el mismo resultado.
- **Ingesta O(n²)**: cada nodo sin código llama `next_code` (`model.rs:275-292`), que cuenta y
  recorre todos los nodos y arcos; `add_node` vuelve a recorrer para comprobar duplicados. Un plan
  de 20 000 nodos son ~10⁸ comparaciones de cadenas. Se nota a partir de ~5 000 objetos.

---

## 2. Consistencia: ¿el 3D usa los mismos parámetros que muestra el inspector?

**No en general.** El inspector enseña los atributos del esquema (`app main.rs:644-676`), el 3D usa
un subconjunto y, en varios casos, otro parámetro distinto:

| Familia | El inspector muestra | El 3D dibuja | Línea |
|---|---|---|---|
| `saneamiento.caja`, `agua.arqueta`, `saneamiento.arenero`, `saneamiento.trampa_grasa` | `largo` × `ancho` (0,6×0,6 / 0,8×1,2 / 1,0×1,5 m) | **caja fija de 0,9 × 0,9 m** | `view.rs:1047-1051` |
| `agua.tanque` | `diametro` (mm), `altura_util` | diámetro = √(V/3) y altura acotada a 2–8 m | `view.rs:1058-1067` |
| `agua.planta` | `caudal_diseno` (L/s) | se pasa como **diámetro en metros** | `view.rs:1053` |
| `comun.cuerpo_receptor`, `comun.cruce` | `ancho`, `longitud_cruce` | constantes 8×14 y 8 m | `view.rs:1056-1057` |
| `saneamiento.pozo` (fuste) | `espesor_pared` = 0,15 m | cilindro macizo, sin pared | `parts.rs:405-409` |
| Cualquier familia | "altura" = terreno − clave (`model.rs:146-148`) | altura dibujada = tapa − clave | `view.rs:949` |
| `saneamiento.canal` (rectangular, 0,6 m de plantilla) | `seccion`, `ancho_plantilla` | **tubo circular** de `d_mm` | `view.rs:605` |
| `saneamiento.pozo` `inspeccion_rectangular` | `largo_interior`, `ancho_interior` | pozo circular genérico | `view.rs:929-953` |
| `saneamiento.bombeo` | `numero_bombas` | pozo de inspección sin bombas | `view.rs:929-953` |

Dos mecanismos explican casi todo: (a) `pieza_local` decide por **familia** y no por subtipo, así
que los subtipos solo cambian el dibujo del pozo (y por una cadena de `if` sobre `ty`, `view.rs:930-946`);
(b) el LOD y las etiquetas se calculan con `n.diametro()` (`view.rs:661`), que devuelve
`diametro_interior` **o el default 1,2 m para cualquier familia** (`model.rs:149-151`). Una válvula
DN200 tiene radio real 0,12 m y se mide como 0,6 m: entra en LOD 2 y recibe etiqueta mucho antes de
distinguirse, y el umbral de símbolo (<3 px) casi nunca se alcanza.

**El 3D tampoco respeta las cotas reales por defecto**: la app arranca con `z_scale = 2.0`
(2,6 con ciudad) — `app main.rs:82` — sin indicarlo en ninguna parte de la interfaz; el panel dice
"altura 2,20 m" y el pozo mide 4,40 m en pantalla. `Foco` lo pone a 1,0 (`main.rs:180`) y `Salir`
lo devuelve a 2,0 (`main.rs:193`). La CLI, en cambio, dibuja a 1,0 (`cli main.rs:315-323`), de modo
que **la vista del agente y la del usuario no coinciden**, contra lo que afirma `svg.rs:4-5`.

Y las cotas del dato no se comprueban: `pozo_partes` fuerza `h = (rim − invert).max(0.8)`
(`parts.rs:391`), así que un pozo con la clave por encima de la tapa se dibuja como una columna de
0,8 m **hacia arriba**, sin diagnóstico. Tampoco se cumple el criterio 3 del doc 10 §6 ("nunca a
cota 0, se dibuja un marcador y se avisa"): si el punto cae fuera del TIN, `z_de` devuelve `None` y
la calle o la manzana **desaparece en silencio** (`view.rs:368-380`). Peor, `z_de` consulta primero
el terreno **aligerado** de pantalla (`view.rs:369`), que es el que se generó con `thinned`
(`app main.rs:77-80`), no el TIN real: las bases de los edificios se apoyan en una malla gruesa.

---

## 3. Rendimiento

**Lo que cuesta un frame hoy.** La escena se construye en cada evento (`rebuild`, `app main.rs:138-148`)
y se **vuelve a copiar entera** en cada frame dentro de `view()`:

```rust
// app main.rs:400-411
for p in &s.scene.prims {
    Prim::Poly { pts, .. } => { shape = shape.fill(pts.clone(), color(*fill)); }
    Prim::Line { a, b, w, color: c, .. } => { shape = shape.line(...); }
}
```

Es decir: N `Vec` clonados + N comandos de Rune **por frame**, más 28 etiquetas que se recrean como
elementos. No hay caché de `Shape` ni invalidación.

**Lo que cuesta reconstruir.** `rebuild` se llama en cada `Phase::Move` del arrastre
(`app main.rs:443`); cada llamada:

1. re-proyecta los ~4 000 triángulos del terreno aligerado;
2. **regenera la malla de cada nodo visible** — no hay caché por `(familia, subtipo, parámetros)`;
   un pozo LOD 2 son ~284 triángulos (`parts.rs:389-494`), una válvula ~200;
3. hace `p.node(from)` y `p.node(to)` por arco, que son búsquedas lineales (`view.rs:577-578` →
   `model.rs:261-266`): **O(nodos × arcos)** por reconstrucción;
4. ordena con el algoritmo del pintor, O(n log n) con comparador de `f32` (`view.rs:816-822`);
5. asigna varios `Vec` por primitiva (`expand`, `view.rs:829-840`).

**Medido** (debug, `redes vista`, 1200×760): demo ciudad = 84 objetos, 4 484 puntos de TIN,
8 700 triángulos → **8 784 primitivas**, 58–74 ms por vista, SVG de 643 KB. El catálogo: 2 177
primitivas, 29 ms.

**El techo y qué se rompe primero.** El terreno está acotado por construcción (`thinned` con celda
`diag/46`, `app main.rs:77-80`): nunca pasa de ~4 000 triángulos. El crecimiento patológico son los
**nodos**, porque cada uno regenera su malla y añade cientos de primitivas. Orden de rotura:

1. **~1 500–3 000 nodos visibles** con LOD 2: la reconstrucción por arrastre deja de ir a 60 fps y
   el `Shape` por frame se vuelve el coste dominante (el propio doc 10 estima ~5 800 primitivas/frame
   como techo del renderer actual; una red urbana real lo pasa con 1 000 pozos).
2. El **orden del pintor** empieza a fallar antes que el rendimiento: con piezas grandes y
   solapadas, el `depth` medio por triángulo produce saltos de orden visibles (y con el relleno de
   sección de la fase 2 sería peor).
3. El **presupuesto de triángulos no existe de verdad**: `if tri_count > 90_000 { lod = 1 }`
   (`view.rs:692`) solo baja un nivel, no descarta nodos, y se evalúa **antes** de contar los nodos
   del propio bucle (solo lleva el terreno).

Arreglo barato con más retorno: cachear la malla por `(family, ty, params redondeados, lod)`;
indexar nodos por `ElementId` (`HashMap` en `Project`, o pasar el índice al `build`); no reconstruir
en `Phase::Move` (reconstruir al soltar, o mover solo la cámara); reutilizar el `Shape` cuando
`scene` no cambia; y ordenar por objeto (no por triángulo) con clave de profundidad de la pieza.

---

## 4. Mantenimiento: lo frágil y lo que falta probar

- **Sin control de versiones.** No hay `.git`: no hay diff, ni vuelta atrás, ni forma de saber qué
  cambió entre la fase 3 y la 4. Con un archivo generado de 91 KB y otro de 50 KB editándose a mano,
  es el mayor riesgo del proyecto, por encima de cualquier bug.
- **El "generado" no es reproducible.** No existe el script que produce `families.rs`; queda un
  `.bak`. Cualquier arreglo del catálogo se hará a mano y el próximo "regenerar" lo pisará.
- **Dependencia de la API pre-release de Rune**: `rune = { path = "../../vendor/rune/crates/rune" }`
  con `AppSpec`, `Shape::fill`, `tx.live_only()`, `rune::extra_args()`, `Runtime` (solo en tests).
  Está vendorizado y fijado, pero el `Cargo.toml` del workspace avisa de que los `[patch]` de Rune
  hay que repetirlos en cada crate que lo consuma (hoy solo `redes-app`); añadir un segundo crate
  con UI duplicaría el patch. Actualizar Rune es un evento de proyecto, no un `cargo update`.
- **Código muerto que parece vivo** (engaña en la revisión): `Topology` completo (213 líneas con
  `stranded`/`isolated`, útil para revisar) no lo usa nadie fuera de sus pruebas; `parts::canal`
  (nunca llamada); `Mesh::girar` (nunca llamada); `caps_of`/`is_viewable` en la app
  (`main.rs:1079-1087`, `#[allow(dead_code)]`, y `is_viewable` **duplica** el filtro que causa
  §1.2); `State.cursor` (se escribe, no se lee); las ramas `saneamiento.bombeo` de `pieza_local`
  (`view.rs:1076-1082`, inalcanzables porque `pieza_partes` las captura antes, `view.rs:929`).
- **Métricas que mienten**: `line_count` se incrementa al añadir **triángulos** de tubería
  (`view.rs:635`), así que la barra de estado y el sobre de la CLI informan "317 líneas" en una
  escena con 10 tubos; la prueba `la_camara_encuadra_el_proyecto` pasa por ese contador
  (`view.rs:1106`).
- **Niebla dependiente del zoom**: `fog(depth, cam.distance)` (`view.rs:842-845`) hace que el color
  de un objeto cambie al acercarse, y nunca pasa de 55 % hacia el fondo. No es un fallo de
  corrección, pero rompe la idea de "el mismo objeto se ve igual".

**Pruebas que faltan** (cada una habría cazado un hallazgo de este informe):

1. Precisión: una tubería en N≈9,8·10⁶ debe tener sección con ≥3 valores distintos en X e Y (§1.1).
2. Cobertura de render: contar primitivas **por familia** a través de `build_opts` (§1.2).
3. Corte sobre arcos: con `corte_x` activo, las primitivas de arco deben reducirse (§1.3).
4. Cota de tapa: `hi[2] == z_rim` con tolerancia 1 mm, no 0,1 m (§1.5 de la lista Top 10).
5. Corte con el plano fuera de la pieza y con el plano coincidente con un vértice (el cálculo de
   `t` en `view.rs:896` es correcto pero retorcido; conviene fijarlo con casos límite).
6. Esquema: claves de atributo únicas por familia, y unidad no vacía para todo atributo numérico
   cuyo nombre empiece por `diametro_`, `longitud_`, `altura_`, `cota_`, `profundidad_`, `ancho_`.
7. Persistencia: cargar un archivo con `next_slot` inconsistente o ids duplicados debe fallar.
8. Ingesta: cada `default_subtype` debe existir en su familia (hoy falla, §1.7).
9. CLI: `--dry-run` con ruta inexistente debe devolver error.
10. Golden image: `--scene corte` y `--scene explosion` comparadas contra una referencia; hoy no hay
    ninguna prueba visual y los PNG del doc no se regeneran.

---

## 5. Top 10 hallazgos

| # | Sev | Dónde | Qué | Impacto | Arreglo |
|---|---|---|---|---|---|
| 1 | **BLOQUEANTE** | `parts.rs:819-829` (+`gltf.rs:104`) | Tuberías y juntas se generan en `f32` con coordenadas UTM; en N≈9,8·10⁶ el paso es 1 m y la sección colapsa | Todo tubo se ve como cinta plana; el `.glb` hereda el defecto | Construir en local (`origin` restado en `f64`) y trasladar después; prueba de precisión |
| 2 | **BLOQUEANTE** | `view.rs:644` | Filtro por `HYDRAULIC_NODE`/`ATTACHABLE` deja fuera 7 familias con geometría escrita | 24 % del catálogo invisible; la vitrina y el doc 10 mienten | Filtrar por geometría declarada o `caps::DRAWABLE`; prueba contra `build_opts` |
| 3 | **BLOQUEANTE** | `view.rs:719-722`, `parts.rs:405-417` | El corte solo recorta nodos; el pozo es macizo (sin pared ni hueco) | El modo Corte no muestra interior; doc 11 §2 falso | Recortar arcos; fuste como `tubo`; cara de sección |
| 4 | **IMPORTANTE** | `families.rs:507+532`, `560+581`, `715+722`, `781+785`, `811+817`, `1191+1201`, `1368+1371`, `1392+1397`, `510+535` | Atributos duplicados; `attr()` devuelve el primero; `redes esquema` publica claves repetidas | El segundo atributo es muerto; formularios generados desde el esquema se duplican | Deduplicar en el generador + prueba de unicidad por familia |
| 5 | **IMPORTANTE** | `parts.rs:391`, `421-429` | La tapa queda 50 mm sobre `z_rim`; `max(0.8)` inventa pozos de 0,8 m | Criterio ±1 mm del doc 10 incumplido; datos incoherentes se tapan | Cerrar el ensamblaje en `rim` exacto; diagnosticar en vez de acotar |
| 6 | **IMPORTANTE** | `app main.rs:247-272`, `156-161` | `ajustar` no hace `commit`; `recordar` clona el proyecto entero por clic, incluso sin cambios | `revision`/`journal` divergen; 20 copias completas del modelo | `commit` por edición; copiar solo si hay cambio (o diff) |
| 7 | **IMPORTANTE** | `app main.rs:254-255`, `648-649` | `clamp(min,max)` con `min>max` entra en pánico; `−/+ prof.` invertidos; `+ Ø` escribe atributo fuera de esquema | Crash con datos importados; el botón hace lo contrario de lo que dice | `min/max` calculados con `min()`; corregir signos; validar el atributo contra el esquema |
| 8 | **IMPORTANTE** | `app main.rs:127-136`, `project_io.rs:40-47` | Guardado no atómico en hilo suelto; `dirty=false` antes de escribir; carga sin validar ids | Archivo truncado o corrupción silenciosa de `ElementId` | Usar `project_io::save`; validar `next_slot`/ids al cargar |
| 9 | **IMPORTANTE** | `view.rs:661`, `691-694` | LOD y etiquetas se deciden con `n.diametro()` (default 1,2 m para toda familia) | Piezas diminutas con etiqueta y LOD caro; umbral 14 px ≠ 40 px del doc | Usar el radio real de la pieza generada; budget sobre primitivas, no sobre `tri_count` |
| 10 | **IMPORTANTE** | `app main.rs:443`, `400-411`; `view.rs:577-578` | Rebuild completo en cada movimiento del ratón, sin caché de mallas y con búsqueda lineal por arco | ~1 500–3 000 nodos dejan de ir a 60 fps; el `Shape` se reconstruye cada frame | Caché por `(familia, ty, params, lod)`; índice por id; no reconstruir en `Phase::Move` |

**Menores** (no entran en el Top 10, pero están): `line_count` cuenta triángulos (`view.rs:635`);
fog dependiente del zoom (`view.rs:842`); `z_scale` 2,0/2,6 sin indicación y CLI a 1,0
(`app main.rs:82`, `cli main.rs:315`); `z_de` prefiere el TIN aligerado y desaparece en silencio
fuera del TIN (`view.rs:368-380`); tabla `LAYER` vacía y datum mixto en el DXF (`export.rs:24,37,45`);
`--dry-run` de importación devuelve `ok` con archivo inexistente (`cli main.rs:610`); `"primitivas": 0`
en gltf (`cli main.rs:739`); `default_subtype("agua.llave") = "columna"` inexistente
(`ingest.rs:247`); canales dibujados como tubo circular y `parts::canal` muerta (`view.rs:605`,
`parts.rs:779`); bombas del pozo de bombeo nunca dibujadas (`view.rs:929`); `Topology` sin cablear;
DXF sin diámetros; `next_code` O(n) por inserción (`model.rs:275-292`); sin git.

---

## 6. Veredicto: ¿se puede pasar a fase 4 (glTF) y 5 (pase 3D con profundidad)?

**A la fase 5, no.** Un z-búfer no arregla nada de esto: no devuelve el volumen a tubos colapsados
por `f32`, no dibuja familias que el bucle de nodos filtra, no crea el interior del pozo ni corta
los arcos. La fase 5 cambiaría **cómo se ordena y se ocluye** lo que ya se genera; los tres
bloqueantes están en **qué se genera**. Además, el pase con profundidad solo se justifica cuando el
pintor falla por orden (piezas grandes solapadas, relleno de sección) — y ese escenario aún no
existe porque el corte es incompleto.

**A la fase 4, tampoco sin condición.** Ya hay `gltf.rs` en el árbol (otro agente, 18:39) y
**reutiliza `parts::tuberia` en coordenadas de mundo** (`gltf.rs:104`), de modo que el `.glb`
saldrá con los tubos planos y con las 7 familias ausentes si no se corrige antes. El glTF no puede
declararse entregable mientras el hallazgo 1 esté abierto: es el mismo error, exportado a un
formato que el cliente va a abrir en Blender.

**Refactor previo mínimo** (estimación: 4–6 días, sin tocar la arquitectura):

1. `parts` en marco local + traslación en `f64` (hallazgo 1, incluido glTF). *1–2 días.*
2. Filtro de dibujabilidad por geometría declarada y prueba por familia (hallazgo 2). *1 día.*
3. Corte sobre arcos + fuste como tubo con `espesor_pared` (hallazgo 3). *1–2 días.*
4. Deduplicar `families.rs` y congelar el generador en el repo (hallazgo 4). *medio día.*
5. Tapa exactamente en `rim` y diagnóstico de cotas incoherentes (hallazgo 5). *medio día.*

Los hallazgos 6–10 (estado, guardado, LOD, rendimiento) pueden ir en paralelo y **no** bloquean las
fases 4–5, pero el 8 (guardado no atómico) debería entrar antes de que alguien revise un proyecto
real, porque el modo de fallo es pérdida de datos.

---

## Anexo A — Evidencia ejecutada

```
cargo test -p redes-core --offline            → 49 passed; 0 failed  (+1 doctest)
cargo build --offline                         → Finished dev profile (solo warnings de vendor/rune)

redes esquema                                 → total_familias 29, subtipos 216
                                                saneamiento.tuberia: 24 atributos,
                                                "diametro_nominal" x2, "material" x2
redes -p catalogo.redes importar dxf \
      --archivo no-existe.dxf --dry-run       → ok:true, bytes:0, exit 0
redes proyecto ciudad --salida ciudad.redes   → proyecto info: 84 objetos (43 nodos, 41 arcos),
                                                4 484 puntos de TIN, 8 700 triángulos
redes -p ciudad.redes vista --salida v.svg    → 8 784 primitivas, 8 743 triángulos, 41 líneas,
                                                58–74 ms, 643 KB de SVG

[single](9796036.0 + 0.125) = 9796036   → el anillo de un tubo colapsa (paso f32 = 1,0 m)
```

Capturas del propio repo usadas como evidencia: `docs/img/modo-corte.png` (pozo cortado, tuberías
sin cortar, cuerpo macizo), `docs/img/catalogo-general.png` (vitrina sin las 7 familias),
`docs/img/vista-detalle.png` (barra de estado con `line_count` inflado).

## Anexo B — Contradicciones documento ↔ código

| Documento dice | El código hace |
|---|---|
| "el recorte también corta tubos" (11 §2) | solo nodos (`view.rs:719`) |
| "se ve el interior real: espesor de pared, escalera, banqueo" (11 §2) | el fuste es macizo (`parts.rs:405`) |
| "la profundidad y el diámetro son los mismos parámetros que generan la malla" (11 §6) | largo/ancho, `diametro_tapa`, `diametro` del tanque y `seccion` del canal se ignoran (§2) |
| "conserva acimut/elevación de la vista general" (11 §1) | `enfocar` fija −48°/16° (`app main.rs:178-179`) |
| "LOD: 3–40 px simple, >40 px completo" (10 §3) | umbral en 14 px y por radio (`view.rs:691`) |
| "la tapa está a z_rim ±1 mm" (10 §6) | tapa a `rim + 50 mm`, prueba con ±100 mm (`parts.rs:428`, `870`) |
| "ninguna pieza a cota 0: se dibuja un marcador y se avisa" (10 §6) | fuera del TIN no se dibuja nada (`view.rs:368-380`) |
| "27 de las 29 familias en 49 objetos" (10 §5) | 22 familias visibles; 7 nunca se dibujan (§1.2) |
| "un agente pide una imagen y lo que ve coincide con el usuario" (`svg.rs:4`) | CLI a `z_scale` 1,0, app a 2,0/2,6 |
