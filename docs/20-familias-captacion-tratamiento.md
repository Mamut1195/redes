# 20 — Familias de captación, conducción y tratamiento (cadena de agua cruda)

| Campo | Valor |
|---|---|
| Documento | `docs/20-familias-captacion-tratamiento.md` |
| Autor | ingeniero sanitario — arquitectura de información del catálogo |
| Versión | 1.0 (propuesta para decisión) |
| Fecha | 2026-11-21 |
| Estado | Especificación de catálogo. No implementa: deja el registro listo para implementar |
| Audiencia | implementadores de `redes-core`, mantenedores de la skill `redes-ingesta`, cliente |
| Alcance | familias y subtipos que faltan entre la captación y la planta, y en la salida del alcantarillado; atributos, capacidades, pieza paramétrica y alias de migración |
| Fuera de alcance | hidráulica, hidrología, dosificación, dimensionamiento, cumplimiento normativo, automatización |

**Cómo leer este documento.** La §1 dice qué hay hoy, con la clave exacta y el conteo verificado en `crates/redes-core/src/families.rs`. La §2 fija las convenciones con las que se propone lo nuevo. Las §3–§7 son el entregable en el orden pedido: **familias nuevas**, **huecos en familias existentes**, **atributos nuevos**, **qué no debe entrar** y **referencias**. La §8 son los anexos que hacen la propuesta ejecutable (alias de ingesta y checklist de pruebas).

**Regla dura.** El producto **no calcula**. Un número entra al catálogo solo si (a) cambia la forma de algo o (b) hay que verlo y compararlo en la revisión; en el segundo caso se marca **▣ dato** y el dibujo no depende de él. No se añade ni un atributo `Derived` hidráulico.

**Convención numérica.** Punto decimal en tablas y claves (compatibilidad con el código); coma en prosa. Unidades SI internas; diámetro nominal como entero en mm, como ya hace el registro.

**Mapa del entregable**

| Punto pedido | Sección |
|---|---|
| 1. Tabla FAMILIAS NUEVAS | §3.1 (tabla maestra) + §3.2–§3.8 (fichas) |
| 2. Tabla HUECOS EN FAMILIAS EXISTENTES | §4 |
| 3. Tabla ATRIBUTOS NUEVOS | §5 |
| 4. Qué NO debe entrar | §6 |
| 5. Referencias | §7 |

---

## 0. Resumen

### 0.1 Cifras de la propuesta

| Concepto | Hoy | Propuesta | Δ |
|---|---:|---:|---:|
| Familias | 29 | **46** | +17 |
| Subtipos | 216 | **315** | +99 netos |
| Subtipos nuevos en familias nuevas | — | **95** | |
| Subtipos nuevos en familias existentes | — | **17** | |
| Claves retiradas o movidas (con alias) | — | **13** | |
| Atributos nuevos en familias existentes | — | **62** | |
| Unidades nuevas en `Unit` | 13 | **21** | +8 |
| Primitivas nuevas en `parts.rs` | 11 | **13** | +2 |

Comprobación: `216 − 13 + 95 + 17 = 315`. El conteo se cierra con la aritmética del registro, no con una estimación. Los 95 subtipos de familias nuevas incluyen 13 claves renombradas que sustituyen a las retiradas; por eso el neto es +99 y no +112.

### 0.2 Las doce decisiones que hay que confirmar antes de implementar

| # | Decisión | Alternativa si se rechaza |
|---|---|---|
| D1 | `agua.bocatoma` es **familia propia** (polígono), no subtipo de `agua.captacion`. | Dejar `toma_rio`/`toma_quebrada` como están y añadir `barraje_fijo`/`barraje_movil` como subtipos de `agua.captacion`; se pierde el eje del azud. |
| D2 | El dominio se **parte por red** en cuatro casos nuevos (`agua.camara`, `agua.bombeo`, `agua.sedimentador`, `saneamiento.sedimentador`), como ya se parte `tuberia` y `acometida`. | Un atributo `red` en las familias existentes; contradice que hoy el dominio se codifica en la clave. |
| D3 | `agua.planta` se queda **solo con plantas completas**; las unidades (`cloracion`, `lecho_secado`, `espesador`) salen a familias propias. | Mantener las unidades como subtipos de planta: no se pueden dibujar dos filtros distintos en la misma planta. |
| D4 | Las unidades que sirven a AP y a AL van a `comun.*` (`desarenador`, `reja`, `compuerta`, `lecho_secado`, `aforador`, `caseta`). | Duplicarlas (`agua.desarenador` + `saneamiento.desarenador`): 6 familias de más y el mismo dibujo dos veces. |
| D5 | `comun.reja` (reja, criba, tamiz, colador) es familia distinta de `comun.tapa:rejilla_*` (rejilla de tapa). | Un solo nombre para dos objetos: la ingesta confundiría tapas con rejas de planta. |
| D6 | La compuerta de canal **no** es `agua.valvula`: es `comun.compuerta`, un objeto estructural medido en m² de hoja. | Meter `compuerta_canal` en `agua.valvula`: la válvula es un dispositivo a presión, la compuerta una estructura a superficie libre. |
| D7 | Los caudales, cargas, dosis y tiempos entran como **▣ dato editable**, no como `Derived`. | Marcarlos `Derived`: el motor no los puede llenar (no calcula) y el campo queda muerto. |
| D8 | Se añade `cota_clave_fin` al arco y el nodo conserva **una** cota (la de salida); la caída de cada llegada va en el arco entrante. | Un atributo por entrada (`cota_entrada_1..n`): explosión combinatoria y esquema que no valida. |
| D9 | `material` y `estado` pasan de `Text` libre a `Enum` con normalización en la ingesta. | Dejarlos `Text`: no se puede agrupar un listado de cantidades por material. |
| D10 | `saneamiento.bombeo` **pierde** `SOURCE` y `potencia_instalada` gana unidad (kW). | Dejar `SOURCE`: una estación de bombeo no es una fuente, es un escalón de carga. |
| D11 | La desinfección se modela como **dispositivo en línea** (`agua.desinfeccion`) más el tanque de contacto que ya existe (`agua.tanque:contacto`), y la caseta es un componente (`comun.caseta:cloracion`). | Una familia «cámara de cloración» con el tanque dentro: mezcla equipo con estructura. |
| D12 | `comun.caseta` entra al catálogo, pero **solo** para casetas que alojan proceso. | Dejar la arquitectura de planta fuera y que el revisor no pueda contar el laboratorio ni la caseta de dosificación. |

### 0.3 Dos premisas del encargo que no se sostienen

**`LINEAL` no existe.** El registro declara ocho capacidades y ninguna es lineal:

| Bit | Constante | Qué significa hoy en el registro |
|---|---|---|
| 1 | `HYDRAULIC_EDGE` | conduce o parte un arco |
| 2 | `HYDRAULIC_NODE` | tiene volumen y cota de agua |
| 3 | `EXCAVATABLE` | genera cubo de excavación |
| 4 | `STRUCTURAL` | se dibuja y se mide como sólido de hormigón |
| 5 | `ATTACHABLE` | puede colgar de un padre (`padre_id`) |
| 6 | `REPORTABLE` | entra en listados, etiquetas y cantidades |
| 7 | `CONSUMES_FLOW` | demanda caudal (llave, medidor, hidrante) |
| 8 | `SOURCE` | es fuente del modelo |

Lo «lineal» se expresa con `TopoRole::Link` + `GeomKind::Polyline`, que ya existen. **No se propone una capacidad nueva `LINEAL`**: sería redundante con dos campos que ya se validan.

**«CPE INEN 9» no es la norma de diseño de agua potable de Ecuador.** La serie verificable es **CPE INEN 5**: Parte 9-1 (dotaciones y caudales, ed. 1992) y Parte 9-2 (sistemas rurales, ed. 1997); la calidad la fija **NTE INEN 1108**. La referencia «INEN 9» del encargo no aparece en las fuentes consultadas para diseño de abastecimiento. En Ecuador, además, mandan las normas del prestador y los pliegos heredados del ex IEOS: son de **aplicación local**, no hay edición única nacional, y así se marca en las fichas.

### 0.4 Hallazgos del registro (verificados en el código, no supuestos)

| # | Hallazgo | Evidencia | Consecuencia |
|---|---|---|---|
| H1 | No existe la capacidad `LINEAL` | `families.rs:329-357` | §0.3 |
| H2 | `Unit` no tiene kW, mg/L, h, m³/h ni UNT | `families.rs:20-34` | `potencia_instalada` está en `Unit::None` (`families.rs:3353-3360`) |
| H3 | Hay claves de atributo **duplicadas** dentro de una misma familia | `agua.tuberia` declara `diametro_nominal` y `clase_presion` dos veces (`families.rs:1058` vs `1164`, `1086` vs `1172`); `saneamiento.tuberia` declara `diametro_nominal` y `material` dos veces (`946` vs `964`, `859` vs `988`) | `FamilyDef::attr()` devuelve la primera: la segunda es código muerto y contradice tipo y unidad |
| H4 | `agua.planta` mezcla escala planta y escala unidad | subtipos `cloracion`, `lecho_secado`, `espesador` (`families.rs:3307-3321`) | §3.5, §3.7, §4 |
| H5 | `saneamiento.camara` aloja cámaras de **conducción de agua potable** | subtipos `carga` y `quiebre` (`families.rs:2552-2561`) | §3.3 |
| H6 | `saneamiento.bombeo` declara `SOURCE` y es el único hogar de estaciones de impulsión de AP | `families.rs:3331-3335` | D10 |
| H7 | `agua.tuberia` no tiene cota de clave final | solo `cota_clave` (`families.rs:1115`) | no se puede revisar un perfil sin recalcular |
| H8 | `material()` y `estado()` son texto libre | `families.rs:403-413`, `450-460` | «PVC», «pvc» y «PVC-U» conviven |
| H9 | `agua.captacion` no tiene caudal disponible ni niveles de pozo | `families.rs:2992-3131` | no se puede revisar una captación real |
| H10 | `agua.tanque` no tiene cota de rebose ni de desagüe | `families.rs:1950-2081` | el rebose es lo que más se observa en inspección |
| H11 | No hay primitiva de plano inclinado ni de marco rectangular | `parts.rs:151-1095` (11 primitivas) | placas de sedimentador, taludes y marcos de compuerta no se pueden dibujar |
| H12 | El registro exige ≥ 4 subtipos por familia y prefijo único | tests `el_catalogo_tiene_la_escala_del_consejo`, `claves_y_prefijos_unicos` (`families.rs:4928-5012`) | restricción de diseño para las 17 familias nuevas |
| H13 | `agua.captacion` y `agua.planta` no declaran `EXCAVATABLE` aunque son estructuras enterradas | `families.rs:2991`, `3182` | no se emite excavación para ellas, y sí para un pozo de 1,2 m |
| H14 | Hay **6 atributos hidráulicos marcados `Derived`** que el motor nunca podrá llenar | `saneamiento.tuberia`: `caudal_diseno`, `tirante`, `velocidad`; `agua.tuberia`: `caudal_diseno`, `presion_servicio`, `perdida_carga` | contradicen la regla «el producto no calcula»: hay que decidir si se vuelven ▣ dato o se retiran |

H14 es el hallazgo que más afecta a este documento: si se añaden familias con caudales, cargas y dosis, la tentación es repetir el patrón `Derived`. **No se hace**: todos los números nuevos son ▣ dato.

---

## 1. Lo que ya existe en la cadena (claves exactas)

Solo las familias que tocan la cadena de agua cruda y su salida. El resto del registro (pozo, tubería de alcantarillado, caja, sumidero, canal, sifón, descarga, accesorio, tapa, escalera, hito, cuerpo receptor, cruce, dado de anclaje) no se repite aquí.

| Familia (clave exacta) | Pref. | Rol / geometría | Capacidades | Subtipos | Qué cubre | Qué le falta para la cadena |
|---|---|---|---:|---|---|---|
| `agua.captacion` | CP | node / point | `HYDRAULIC_NODE·SOURCE·STRUCTURAL·REPORTABLE` | 8 | manantial de ladera y de fondo, pozo profundo y somero, galería filtrante, toma de río con presa, toma de quebrada, rejilla de fondo | toma directa sin azud, toma flotante, pozo radial; caudales y niveles; ver H13 |
| `agua.tuberia` | TB | link / polyline | `HYDRAULIC_EDGE·EXCAVATABLE·REPORTABLE` | 8 | conducción, aducción, distribución primaria y secundaria, ramal, impulsión, succión, línea de agua cruda | rebose, desagüe, conducción forzada, limpia; cota de clave final |
| `agua.valvula` | VL | device / point | `HYDRAULIC_EDGE·ATTACHABLE·REPORTABLE` | 12 | compuerta, mariposa, retención, aire, purga, PRV, sostenedora, reguladora de caudal, alivio, flotador, pie, altitud | guillotina y pinch (lodos); accionamiento y consigna |
| `agua.tanque` | TQ | node / polygon | `HYDRAULIC_NODE·STRUCTURAL·SOURCE·REPORTABLE` | 8 | apoyado cilíndrico y rectangular, elevado, enterrado, semienterrado, cisterna, contacto, agua cruda | tanque de succión, tanque elevado domiciliario; rebose y desagüe |
| `agua.medidor` | MD | device / point | `ATTACHABLE·REPORTABLE·CONSUMES_FLOW` | 7 | chorro múltiple, volumétrico, Woltman, electromagnético, ultrasónico, inserción, compuesto | Q máx/mín, pérdida de carga, clase metrológica |
| `agua.arqueta` | AQ | node / point | `HYDRAULIC_NODE·EXCAVATABLE·STRUCTURAL·REPORTABLE` | 6 | arqueta de medidor, válvula, hidrante, purga, aire, derivación | arqueta de muestreo |
| `agua.llave` | LL | device / point | `ATTACHABLE·REPORTABLE·CONSUMES_FLOW` | 4 | corporación, banqueta, pase, tapón | — |
| `agua.hidrante` | HI | device / point | `ATTACHABLE·REPORTABLE·CONSUMES_FLOW` | 4 | columna, arqueta, boca de riego, tipo tampón | — |
| `agua.acometida` | AM | link / polyline | `HYDRAULIC_EDGE·EXCAVATABLE·CONSUMES_FLOW·REPORTABLE` | 4 | domiciliaria, comercial, industrial, colectiva | — |
| `agua.dado_anclaje` | DA | support / point | `STRUCTURAL·REPORTABLE` | 4 | codo, tee, tapón, reducción | — |
| `agua.planta` | PT | node / polygon | `HYDRAULIC_NODE·STRUCTURAL·SOURCE·REPORTABLE` | 11 | PTAP convencional, compacta y de filtración directa; PTAR de lodos activados, de lagunas y UASB; laguna de estabilización; desalinizadora; espesador; lecho de secado; caseta de cloración | unidades de proceso; tipos de tratamiento (Fe/Mn, ablandamiento); ver H4 |
| `saneamiento.camara` | CM | node / point | `HYDRAULIC_NODE·EXCAVATABLE·STRUCTURAL·REPORTABLE` | 11 | desarenadora, reunión, **carga**, **quiebre**, válvulas, medición, reparto, alivio, inspección rectangular, cámara seca de bombeo, compensación | las de conducción AP están en el dominio equivocado (H5) |
| `saneamiento.bombeo` | EB | node / point | `HYDRAULIC_NODE·STRUCTURAL·EXCAVATABLE·REPORTABLE·SOURCE` | 5 | cámara húmeda, húmeda + seca, paquete compacto, pozo de eyección, estación de impulsión | versión AP del dominio; `SOURCE` de más |
| `saneamiento.canal` | CN | link / polyline | `HYDRAULIC_EDGE·EXCAVATABLE·REPORTABLE` | 9 | rectangular, trapezoidal, triangular, circular, cuneta V y U, coronación, descarga, box culvert | canal de by-pass |
| `comun.cruce` | CR | support / point | `REPORTABLE` | 10 | río, quebrada, vía, ferrocarril, otra red, eléctrica, telecom, gas, aéreo, subfluvial | hinca / perforación dirigida |
| `comun.hito` | HT | support / point | `REPORTABLE` | 6 | BM, punto de control, sondeo, calicata, **punto de aforo**, señalización | el aforo es un punto topográfico, no la estructura (ver §3.6) |

Ninguna de estas 15 familias se propone retirar. Tres pierden subtipos (con alias) y el resto solo gana.

---

## 2. Convenciones de la propuesta

### 2.1 Marcas

| Marca | Significado |
|---|---|
| ● | obligatorio: sin él la forma no se puede construir |
| ◐ | condicional: obligatorio solo en ciertos subtipos (lo declara el `TypeDef`) |
| ○ | opcional: mejora el dibujo o la revisión |
| ▣ | **dato**: número que se transporta y se muestra; **no** interviene en la geometría y **no** se calcula |
| D | derivado (solo los que ya existen: longitud, cota de terreno, profundidad) |
| P1 / P2 | prioridad: P1 = sin ella la cadena no se puede dibujar; P2 = completa el oficio |

### 2.2 Prefijos nuevos (verificados únicos contra los 29 actuales)

`BC` bocatoma · `CA` cámara de agua · `MZ` mezcla · `FL` floculador · `SD` sedimentador AP · `FI` filtro · `DI` desinfección · `BP` bombeo AP · `DZ` desarenador · `RJ` reja · `CG` compuerta · `AF` aforador · `LS` lecho de secado · `CS` caseta · `SE` sedimentador AL · `RX` reactor · `LG` laguna.

Ya usados y por tanto prohibidos: `PZ CL TB DS VL LL TQ MD AC CM SM CN SF CP PT EB AX TP ES HI AQ CJ AR TG RC DA AM CR HT`.

### 2.3 Reglas de migración

Se respetan las cinco reglas de `docs/consejo/06-catalogo-familias.md` §2.4. Las 12 claves que se retiran conservan alias en la skill de ingesta durante dos versiones (§8.1) y ningún archivo `.redes` se reescribe sin confirmación.

---

## 3. FAMILIAS NUEVAS

### 3.1 Tabla maestra

| # | clave | nombre | caps | subtipos | atributos (resumen) | pieza paramétrica (primitivas) | por qué |
|---|---|---|---|---:|---|---|---|
| 1 | `agua.bocatoma` | Bocatoma | `HYDRAULIC_NODE·SOURCE·STRUCTURAL·EXCAVATABLE·REPORTABLE` | 6 | largo y altura de barraje, cotas de cresta y umbral, ventana, compuertas, reja, canal de limpia | `prisma` (muros y alas) + `caja` (azud) + `tronco` (vertedero) + `barra` (rejilla) + `canal` (limpia) + `tubo` (conducto de toma) | la obra de toma en un cauce tiene eje, azud y huella; un punto de captación no los puede representar |
| 2 | `agua.camara` | Cámara de conducción | `HYDRAULIC_NODE·EXCAVATABLE·STRUCTURAL·REPORTABLE` | 8 | largo, ancho, altura útil, cotas de fondo, tapa y agua, cresta, caída, válvulas, by-pass | `caja`/`prisma` (muros y losa) + `tronco` (vertedero) + `anillo` (marco) + `barra` (escalera) + `barra_tapada` (tubería) | la cámara de carga y la de quiebre son de conducción de AP y hoy viven en `saneamiento.camara` |
| 3 | `agua.mezcla` | Mezcla rápida | `HYDRAULIC_NODE·STRUCTURAL·EXCAVATABLE·REPORTABLE` | 5 | largo, ancho, altura útil, número de cámaras, pantallas, difusor, dosis ▣, gradiente ▣ | `prisma` (muros) + `caja` (losa y tabiques) + `tronco` (vertedero) + `barra` (difusor) | la coagulación es la primera unidad de la PTAP y no existe en el registro |
| 4 | `agua.floculador` | Floculador | `HYDRAULIC_NODE·STRUCTURAL·EXCAVATABLE·REPORTABLE` | 5 | largo, ancho, altura útil, número de cámaras y pantallas, aletas, ángulo, tiempo ▣ | `prisma` + `caja` (pantallas y aletas) + `rampa` (pantalla inclinada) | el floculador define la planta de la PTAP y tiene forma propia (pantallas, aletas, hélice) |
| 5 | `agua.sedimentador` | Sedimentador (AP) | `HYDRAULIC_NODE·STRUCTURAL·EXCAVATABLE·REPORTABLE` | 5 | largo, ancho, altura útil, tirante, placas, vertedero, purga, tasa ▣ | `prisma` (muros) + `caja` (losa y canaletas) + `tronco` (tolva) + `rampa` (placas y lamela) | la decantación es unidad obligatoria de toda PTAP convencional |
| 6 | `agua.filtro` | Filtro | `HYDRAULIC_NODE·STRUCTURAL·EXCAVATABLE·REPORTABLE` | 5 | largo, ancho, altura de lecho y borde libre, celdas, lecho, granulometría ▣, tasa ▣, lavado | `caja` (celdas) + `prisma` (muros) + `tubo` (falso fondo) + `barra` (canaletas) | sin filtros no hay PTAP dibujable; hoy la filtración solo existe como adjetivo de la planta |
| 7 | `agua.desinfeccion` | Unidad de desinfección | `HYDRAULIC_EDGE·ATTACHABLE·REPORTABLE` | 6 | tipo, dosis ▣, concentración ▣, tanque de solución, bombas, potencia (kW), lámparas, tiempo ▣ | `caja` (gabinete) + `cilindro` (tanque) + `barra_tapada` (tubería) + `barra` (soporte) | es el único proceso obligatorio en todo sistema; hoy es un subtipo de planta |
| 8 | `agua.bombeo` | Estación de bombeo (AP) | `HYDRAULIC_NODE·STRUCTURAL·EXCAVATABLE·REPORTABLE` | 5 | largo, ancho, diámetro, profundidad, bombas, tipo, caudal ▣, AMT ▣, potencia (kW), niveles, válvulas | `tubo` + `tronco` + `anillo` (cámara) + `cilindro` (bombas) + `barra_tapada` (succión e impulsión) | la conducción por bombeo necesita su estación en el dominio del agua |
| 9 | `comun.desarenador` | Desarenador | `HYDRAULIC_NODE·STRUCTURAL·EXCAVATABLE·REPORTABLE` | 5 | largo, ancho, altura útil, tirante, naves, tolva, purga, limpieza, volumen de arenas ▣ | `prisma` (muros) + `caja` (losa) + `tronco` (tolva) + `canal` (limpia) + `barra` (rastrillo) | es unidad de AP y de AL; hoy solo existe como cámara de alcantarillado y como arenero de patio |
| 10 | `comun.reja` | Reja y criba | `ATTACHABLE·REPORTABLE` | 6 | ancho, altura, inclinación, abertura, número y diámetro de barras, limpieza, cota de umbral | `marco_rectangular` (marco) + `barra` (barras y rastrillo) | la reja se especifica y se cuenta aparte (abertura, mecanismo) y no es una tapa |
| 11 | `comun.compuerta` | Compuerta | `HYDRAULIC_EDGE·ATTACHABLE·REPORTABLE` | 5 | ancho, altura, cota de umbral, accionamiento, husillos, vástago, espesor y material de hoja | `marco_rectangular` + `caja` (hoja) + `barra` (husillo) + `anillo` (volante) | es estructura a superficie libre, no válvula a presión; aparece en toda bocatoma y PTAR |
| 12 | `comun.aforador` | Aforador | `HYDRAULIC_EDGE·STRUCTURAL·REPORTABLE` | 5 | garganta, ancho de canal, umbral, cresta, ángulo, caudales ▣ | `prisma` (muros) + `caja` (garganta) + `rampa` (transiciones) + `barra` (limnímetro) | medir caudal es requisito de operación y hoy solo hay un «punto de aforo» topográfico |
| 13 | `comun.lecho_secado` | Lecho de secado | `STRUCTURAL·EXCAVATABLE·REPORTABLE` | 4 | largo, ancho, altura de muro, celdas, espesor de arena y grava, dren, pendiente, cubierta | `caja` (celdas) + `prisma` (muros) + `barra` (drenes) + `tronco` (cubierta) | cierra el ciclo de lodos de AP y de AL y hoy es un subtipo de planta |
| 14 | `comun.caseta` | Caseta | `STRUCTURAL·REPORTABLE` | 7 | largo, ancho, altura, plantas, cubierta, material, cota de piso | `caja` (volumen) + `tronco` (cubierta) + `caja` (puerta) + `barra` (ventana) | la caseta de dosificación y el laboratorio se cuentan y se visitan; no son «decorado» |
| 15 | `saneamiento.sedimentador` | Sedimentador (AL) | `HYDRAULIC_NODE·STRUCTURAL·EXCAVATABLE·REPORTABLE` | 7 | largo, ancho, altura útil, tirante, vertedero, rasqueta, cámaras, volumen de digestión | `prisma` + `caja` + `tronco` (tolva) + `tubo` | primario, secundario, Imhoff y séptico son la salida obligatoria del alcantarillado |
| 16 | `saneamiento.reactor` | Reactor biológico | `HYDRAULIC_NODE·STRUCTURAL·EXCAVATABLE·REPORTABLE` | 5 | largo, ancho, altura útil, cámaras, aireación, difusores, sopladores, potencia (kW), carga ▣ | `prisma` + `caja` + `barra` (difusores) + `cilindro` (sopladores) | los lodos activados y el UASB son unidades, no «tipos de planta» |
| 17 | `saneamiento.laguna` | Laguna de estabilización | `HYDRAULIC_NODE·STRUCTURAL·EXCAVATABLE·REPORTABLE` | 6 | largo, ancho, profundidad, talud, corona, celdas, revestimiento, carga ▣ | `prisma` (taludes) + `caja` (fondo) + `canal` (entrada y salida) + `tronco` (vertedero) | es la tecnología más usada en Latinoamérica y hoy es un subtipo de `agua.planta` |

### 3.1b Ficha de registro

| clave | prefijo | categoría | rol | geometría | prioridad | subtipos |
|---|---|---|---|---|---:|---:|
| `agua.bocatoma` | BC | agua | node | polygon | P1 | 6 |
| `agua.camara` | CA | agua | node | point | P1 | 8 |
| `agua.mezcla` | MZ | agua | node | polygon | P1 | 5 |
| `agua.floculador` | FL | agua | node | polygon | P1 | 5 |
| `agua.sedimentador` | SD | agua | node | polygon | P1 | 5 |
| `agua.filtro` | FI | agua | node | polygon | P1 | 5 |
| `agua.desinfeccion` | DI | agua | device | point | P1 | 6 |
| `agua.bombeo` | BP | agua | node | point | P1 | 5 |
| `comun.desarenador` | DZ | comun | node | polygon | P1 | 5 |
| `comun.reja` | RJ | comun | support | point | P1 | 6 |
| `comun.compuerta` | CG | comun | device | point | P1 | 5 |
| `comun.aforador` | AF | comun | device | point | P2 | 5 |
| `comun.lecho_secado` | LS | comun | support | polygon | P2 | 4 |
| `comun.caseta` | CS | comun | support | point | P2 | 7 |
| `saneamiento.sedimentador` | SE | saneamiento | node | polygon | P1 | 7 |
| `saneamiento.reactor` | RX | saneamiento | node | polygon | P1 | 5 |
| `saneamiento.laguna` | LG | saneamiento | node | polygon | P1 | 6 |

Nota de implementación: un `node` con `Polygon` ya existe (`agua.tanque`, `agua.planta`); no hace falta motor nuevo para las 10 familias poligonales (7 son puntuales). Para `agua.bocatoma` el eje del barraje se toma de la **primera arista** del polígono; si el visor no gira polígonos, se degrada a `point` + atributo `angulo_barraje` (°).

### 3.2 Etapa A — Captación

#### `agua.bocatoma` — Bocatoma
`BC` · polígono · nodo fuente · 6 subtipos · P1

Obra de toma en un cauce: encauza, eleva el nivel con un azud y deriva el caudal por una ventana con reja, dejando un canal de limpia para evacuar arenas y el excedente.

| subtipo | qué representa | parámetros distintivos |
|---|---|---|
| `barraje_fijo` | azud de cresta fija con toma lateral; el caso más común en quebradas de sierra | `largo_barraje`, `altura_barraje`, `cota_cresta`, `ancho_ventana` |
| `barraje_movil` | azud con compuertas de regulación (cresta variable) | `numero_compuertas`, `ancho_compuerta`, `altura_compuerta` |
| `bocatoma_lateral` | toma lateral sin azud, con umbral y muro de encauzamiento | `cota_umbral`, `longitud_alas`, `angulo_alas` |
| `bocatoma_frontal` | toma frontal con rejilla y muro de cabeza | `ancho_ventana`, `altura_ventana`, `ancho_reja`, `abertura_reja` |
| `toma_tirolesa` | rejilla en el lecho del cauce, con azud bajo (toma tirolés) | `longitud_reja`, `ancho_reja`, `inclinacion_reja` |
| `toma_sumergida` | torre de toma sumergida en embalse o lago | `altura_torre`, `numero_ventanas`, `cota_umbral` |

**Atributos.** `largo_barraje` (m ●) · `altura_barraje` (m ●) · `cota_cresta` (m ●) · `cota_umbral` (m ●) · `cota_fondo` (m ●) · `ancho_obra` (m ○) · `numero_naves` (int ○) · `ancho_ventana` (m ◐) · `altura_ventana` (m ◐) · `numero_compuertas` (int ◐) · `ancho_compuerta` (m ◐) · `altura_compuerta` (m ◐) · `longitud_alas` (m ○) · `angulo_alas` (° ○) · `espesor_muro` (m ○) · `longitud_reja` (m ◐) · `ancho_reja` (m ◐) · `abertura_reja` (mm ◐) · `inclinacion_reja` (° ◐) · `longitud_canal_limpia` (m ○) · `ancho_canal_limpia` (m ○) · `pendiente_canal` (m/m ○) · `altura_torre` (m ◐) · `numero_ventanas` (int ◐) · `material` (enum ○) · `caudal_diseno` (L/s ▣) · `caudal_concesionado` (L/s ▣) · `caudal_ecologico` (L/s ▣) · `estado` · `anio`.

**Pieza.** `prisma` para el muro de cabeza y las alas; `caja` para el cuerpo del azud; `tronco` para el vertedero redondeado; `barra` para las barras de reja; `canal` para el canal de limpia; `tubo`/`barra_tapada` para el conducto de toma. LOD1: dos cajas. LOD2: lo anterior más reja y alas.

**Por qué familia y no subtipo.** Un subtipo de `agua.captacion` no puede llevar eje de azud, longitud de barraje ni huella: la captación es un punto con cámara, la bocatoma es una obra transversal al cauce. Además la bocatoma aparece en el perfil del río, no en la planta de la red.

**Norma.** RNE OS.010 §4 y §4.1 (obras de toma en cursos superficiales); Resolución 0330 de 2017 (RAS), Título 2, art. 47 (caudal de diseño de bocatoma hasta 2·QMD y devolución del excedente a la fuente).

### 3.3 Etapa B — Conducción

#### `agua.camara` — Cámara de conducción
`CA` · punto · nodo · 8 subtipos · P1

Estructura visitable, enterrada o semienterrada, que resuelve una función hidráulica en la línea de conducción: romper presión, regular la entrada a un tramo a presión, alojar válvulas, medir, repartir o aliviar.

| subtipo | qué representa | parámetros distintivos |
|---|---|---|
| `carga` | cámara de carga: entrega el caudal a la conducción a presión con nivel controlado | `cota_agua`, `longitud_vertedero`, `cota_cresta` |
| `quiebre` | cámara de quiebre de presión en conducciones largas | `tipo_quiebre`, `altura_caida`, `cota_agua` |
| `valvulas` | aloja válvulas de gran diámetro (compuerta, aire, purga, macro) | `numero_valvulas`, `diametro_tuberia`, `diametro_bypass` |
| `medicion` | aloja macromedidor con by-pass | `diametro_tuberia`, `diametro_bypass`, `numero_valvulas` |
| `reparto` | divide el caudal en dos o más ramales | `numero_salidas`, `tipo_reparto` |
| `reunion` | junta dos o más conducciones antes de la planta o el tanque | `numero_llegadas`, `cota_fondo` |
| `alivio` | deriva excedentes por vertedero lateral | `longitud_vertedero`, `cota_cresta`, `tipo_vertedero` |
| `inspeccion` | registro visitable en impulsión o conducción a presión | `diametro_interior`, `numero_tapas`, `diametro_tapa` |

**Atributos.** `largo` (m ●) · `ancho` (m ●) · `altura_util` (m ●) · `cota_fondo` (m ●) · `cota_tapa` (m ●) · `diametro_interior` (m ◐) · `espesor_muro` (m ○) · `espesor_losa` (m ○) · `cota_agua` (m ◐) · `cota_cresta` (m ◐) · `longitud_vertedero` (m ◐) · `tipo_vertedero` (enum ◐) · `tipo_quiebre` (enum ◐) · `altura_caida` (m ◐) · `numero_valvulas` (int ◐) · `diametro_tuberia` (mm ◐) · `diametro_bypass` (mm ◐) · `numero_salidas` (int ◐) · `tipo_reparto` (enum ◐) · `numero_llegadas` (int ◐) · `numero_tapas` (int ○) · `diametro_tapa` (mm ○) · `material` (enum ○) · `estado` · `anio`.

**Pieza.** `caja`/`prisma` para muros y losa; `tubo` para el fuste circular; `tronco` para vertedero y embudo; `anillo` para marco de tapa; `barra` para escalera; `barra_tapada` para la tubería que la atraviesa. LOD1: una caja hueca.

**Por qué familia y no subtipo.** `agua.arqueta` es una caja ≤ 1,20 m sin acceso de personal, para un dispositivo; la cámara es visitable, tiene losa, tapa, escalera y una función hidráulica con nivel de agua. La diferencia es de clase, no de tamaño: la arqueta no tiene `cota_agua`.

**Norma.** RNE OS.010 §5 (conducción: cámaras de carga, de quiebre, válvulas de aire y de purga); RNE OS.040 (estaciones de bombeo, cámara de válvulas).

### 3.4 Etapa C — Pretratamiento

#### `comun.desarenador` — Desarenador
`DZ` · polígono · nodo · 5 subtipos · P1

Unidad de sedimentación de arenas y sólidos pesados antes de la conducción, de la planta o del bombeo; tiene tolva de purga y, según el tipo, naves alternas o aireación.

| subtipo | qué representa | parámetros distintivos |
|---|---|---|
| `longitudinal` | desarenador de flujo horizontal con tolva de arenas | `largo`, `ancho`, `tirante`, `diametro_purga` |
| `dos_naves` | dos naves alternas con compuertas, para limpiar sin detener el flujo | `numero_naves`, `numero_compuertas`, `tirante` |
| `vortex` | desarenador de vórtice, compacto, con agitador | `diametro_interior`, `profundidad`, `diametro_purga` |
| `aireado` | cámara de arenas aireada, con sopladores | `numero_sopladores`, `potencia_sopladores` (kW), `tirante` |
| `prefabricado` | equipo prefabricado de PRFV o PEAD | `diametro_interior`, `altura`, `diametro_entrada`, `diametro_salida` |

**Atributos.** `largo` (m ●) · `ancho` (m ●) · `altura_util` (m ●) · `tirante` (m ●) · `cota_fondo` (m ●) · `cota_agua` (m ●) · `cota_vertederos` (m ◐) · `numero_naves` (int ○) · `longitud_transicion` (m ○) · `diametro_purga` (mm ●) · `profundidad` (m ◐) · `tipo_limpieza` (enum ●) · `numero_compuertas` (int ◐) · `diametro_interior` (m ◐) · `altura` (m ◐) · `diametro_entrada` (mm ○) · `diametro_salida` (mm ○) · `numero_sopladores` (int ◐) · `potencia_sopladores` (kW ◐) · `volumen_arenas` (m³ ▣) · `caudal_diseno` (L/s ▣) · `carga_superficial` (m³/m²·d ▣) · `tiempo_retencion` (min ▣) · `material` (enum ○) · `estado`.

**Pieza.** `prisma` para muros y pantallas; `caja` para la losa de fondo; `tronco` invertido para la tolva; `canal` para el canal de limpia; `barra` para el rastrillo. LOD1: caja con un chaflán.

**Por qué familia.** Es unidad de proceso con tolva y purga, común a AP y AL. Ni `saneamiento.camara:desarenadora` (cámara de alcantarillado, sin tolva dibujable) ni `saneamiento.arenero` (trampa pequeña de patio) representan un desarenador de planta.

**Norma.** RNE OS.010 §4/§5 (desarenador antes de la conducción); RNE OS.020 (pretratamiento); RNE OS.090 §5 (desarenado en PTAR).

#### `comun.reja` — Reja y criba
`RJ` · punto · soporte adherible · 6 subtipos · P1

Elemento de retención de sólidos gruesos y finos a la entrada de una captación, de una planta o de un bombeo: marco, barras y mecanismo de limpieza.

| subtipo | qué representa | parámetros distintivos |
|---|---|---|
| `gruesos_manual` | reja de gruesos con limpieza manual de rastrillo | `abertura`, `numero_barras`, `inclinacion` |
| `gruesos_mecanica` | reja de gruesos con limpieza mecánica | `abertura`, `altura`, `tipo_limpieza` |
| `finos` | reja de finos (abertura ≤ 25 mm) | `abertura`, `numero_barras`, `espesor_barra` |
| `criba_estatica` | criba de barras curvas, sin partes móviles | `ancho`, `radio_curvatura`, `abertura` |
| `tamiz_rotativo` | tamiz rotativo o de tornillo con lavado | `diametro_tambor`, `abertura`, `potencia` (kW) |
| `colador` | colador o canastilla de succión de la captación | `diametro_colador`, `altura_colador`, `abertura` |

**Atributos.** `ancho` (m ●) · `altura` (m ●) · `inclinacion` (° ●) · `abertura` (mm ●) · `numero_barras` (int ○) · `diametro_barra` (mm ○) · `espesor_barra` (mm ○) · `tipo_limpieza` (enum ○) · `cota_umbral` (m ●) · `cota_agua` (m ○) · `radio_curvatura` (m ◐) · `diametro_tambor` (m ◐) · `potencia` (kW ◐) · `diametro_colador` (mm ◐) · `altura_colador` (m ◐) · `perdida_diseno` (m ▣) · `velocidad_paso` (m/s ▣) · `estado`.

**Pieza.** `marco_rectangular` (primitiva nueva) para el marco; `barra` para las barras y el rastrillo. LOD1: tres barras y un marco.

**Por qué familia y no atributo.** La abertura, el número de barras, la inclinación y el mecanismo de limpieza definen la pieza y su costo; se especifica en el listado de obras metálicas. No se usa `comun.tapa:rejilla_*`, que es una **tapa** con clase de carga, no una reja de canal.

**Norma.** RNE OS.020 (rejas a la entrada de planta); RNE OS.090 §5 (rejas y desarenado en PTAR).

#### `comun.compuerta` — Compuerta
`CG` · punto · dispositivo de arco · 5 subtipos · P1

Órgano de cierre y regulación en un canal o en un orificio: hoja, marco, husillo y accionamiento. No confundir con la válvula, que trabaja a presión en tubería.

| subtipo | qué representa | parámetros distintivos |
|---|---|---|
| `mural` | compuerta mural de deslizamiento en muro de canal | `ancho`, `altura`, `cota_umbral`, `altura_vastago` |
| `tablachos` | ataguía de tablachos para cerrar un canal en mantenimiento | `numero_tablachos`, `altura_tablacho` |
| `reguladora` | compuerta de regulación de nivel (tipo vertedero) | `cota_cresta`, `ancho`, `tipo_accionamiento` |
| `basculante` | compuerta basculante o clapeta de alivio | `ancho`, `altura`, `angulo_apertura` |
| `de_fondo` | compuerta de fondo para purga de arenas y lodos | `diametro`, `cota_umbral`, `tipo_accionamiento` |

**Atributos.** `ancho` (m ●) · `altura` (m ●) · `cota_umbral` (m ●) · `cota_agua` (m ○) · `tipo_accionamiento` (enum ●) · `numero_husillos` (int ○) · `altura_vastago` (m ○) · `espesor_hoja` (mm ○) · `material_hoja` (enum ○) · `numero_tablachos` (int ◐) · `altura_tablacho` (m ◐) · `cota_cresta` (m ◐) · `angulo_apertura` (° ◐) · `diametro` (mm ◐) · `estado`.

**Pieza.** `marco_rectangular` + `caja` (hoja) + `barra` (husillo) + `anillo` (volante). LOD1: una caja vertical sobre el umbral.

**Por qué familia.** Una compuerta no tiene coeficiente de pérdida local ni clase de presión: tiene ancho de hoja, umbral y accionamiento, y se mide en m² de obra metálica. Meterla en `agua.valvula` obligaría a que una válvula tuviera `cota_umbral`, que en una tubería a presión no significa nada.

#### `comun.aforador` — Aforador
`AF` · punto · dispositivo de arco · 5 subtipos · P2

Estructura de medición de caudal a superficie libre: resalto o garganta calibrada con escala, aguas arriba de la planta o en un canal de derivación.

| subtipo | qué representa | parámetros distintivos |
|---|---|---|
| `parshall` | canaleta Parshall con garganta | `ancho_garganta`, `longitud_garganta`, `altura_umbral` |
| `vertedero_rectangular` | vertedero rectangular de pared delgada | `longitud_cresta`, `cota_cresta` |
| `vertedero_triangular` | vertedero triangular en V | `angulo_vertedero`, `cota_cresta` |
| `vertedero_trapezoidal` | vertedero trapezoidal (Cipolletti) | `longitud_cresta`, `talud` |
| `vertedero_cresta_ancha` | vertedero de cresta ancha (sección crítica) | `longitud_cresta`, `ancho_cresta`, `altura_umbral` |

**Atributos.** `ancho_canal` (m ●) · `cota_umbral` (m ●) · `cota_cresta` (m ◐) · `ancho_garganta` (m ◐) · `longitud_garganta` (m ◐) · `longitud_cresta` (m ◐) · `ancho_cresta` (m ◐) · `angulo_vertedero` (° ◐) · `talud` (H:V ◐) · `altura_umbral` (m ○) · `caudal_max` (L/s ▣) · `caudal_min` (L/s ▣) · `estado`.

**Pieza.** `prisma` para muros; `caja` para la garganta; `rampa` (primitiva nueva) para transiciones; `barra` para el limnímetro. LOD1: un estrechamiento.

**Por qué familia.** `comun.hito:aforo` es un punto topográfico de sección conocida (se usa para medir con molinete); el aforador es una estructura de hormigón que se construye, se cuenta y se dibuja.

### 3.5 Etapa D — Tratamiento

#### `agua.mezcla` — Mezcla rápida (coagulación)
`MZ` · polígono · nodo · 5 subtipos · P1

Primera unidad de la PTAP: dispersa el coagulante en el agua cruda en segundos, por resalto, vertedero, agitador, mezclador estático o inyección.

| subtipo | qué representa | parámetros distintivos |
|---|---|---|
| `resalto_hidraulico` | mezcla por resalto en un canal con cambio de pendiente | `altura_caida`, `ancho`, `longitud_resalto` |
| `vertedero_lateral` | mezcla por caída sobre vertedero lateral | `longitud_vertedero`, `altura_caida` |
| `mecanica` | mezcla mecánica con agitador de hélice | `diametro_agitador`, `potencia` (kW), `altura_util` |
| `mezclador_estatico` | mezclador estático en línea (elementos helicoidales) | `diametro_tuberia`, `numero_elementos`, `longitud` |
| `inyeccion` | inyección con bomba dosificadora y difusor en tubería | `diametro_tuberia`, `diametro_difusor`, `numero_puntos` |

**Atributos.** `largo` (m ●) · `ancho` (m ●) · `altura_util` (m ●) · `cota_fondo` (m ●) · `cota_agua` (m ○) · `numero_camaras` (int ○) · `ancho_pantalla` (m ◐) · `numero_pantallas` (int ◐) · `separacion_pantallas` (m ◐) · `altura_caida` (m ◐) · `longitud_resalto` (m ◐) · `longitud_vertedero` (m ◐) · `diametro_agitador` (m ◐) · `potencia` (kW ◐) · `diametro_tuberia` (mm ◐) · `numero_elementos` (int ◐) · `longitud` (m ◐) · `diametro_difusor` (mm ◐) · `numero_puntos` (int ◐) · `diametro_entrada` (mm ○) · `diametro_salida` (mm ○) · `caudal_diseno` (L/s ▣) · `gradiente_diseno` (s⁻¹ ▣) · `tiempo_mezcla` (s ▣) · `dosis_coagulante` (mg/L ▣) · `material` (enum ○) · `estado`.

**Pieza.** `prisma` (muros) + `caja` (losa y tabiques) + `tronco` (vertedero) + `barra` (difusor y eje del agitador) + `cilindro` (motor). LOD1: caja con un tabique.

**Por qué familia.** Es unidad con volumen, cota de agua y tabiques; el caudal y el gradiente son datos, no geometría. Se separa de `agua.floculador` porque la forma y el tiempo son otros (segundos contra minutos) y en obra son estructuras distintas.

**Norma.** RNE OS.020 (criterios de diseño de unidades; mezcla rápida antes de la floculación).

#### `agua.floculador` — Floculador
`FL` · polígono · nodo · 5 subtipos · P1

Unidad de agitación lenta que forma el flóculo: cámaras en serie con pantallas, aletas, canal helicoidal o paletas mecánicas.

| subtipo | qué representa | parámetros distintivos |
|---|---|---|
| `pantallas_horizontales` | floculador hidráulico de pantallas horizontales | `numero_camaras`, `numero_pantallas`, `separacion_pantallas` |
| `pantallas_verticales` | floculador de pantallas verticales (tipo Cochrane) | `numero_camaras`, `ancho_pantalla`, `separacion_pantallas` |
| `aletas` | floculador de aletas | `numero_aletas`, `angulo_aletas`, `separacion_aletas` |
| `helicoidal` | floculador helicoidal (canal en espiral) | `diametro_helice`, `numero_giros`, `pendiente_canal` |
| `mecanico` | floculador mecánico de paletas | `numero_paletas`, `potencia` (kW), `diametro_paletas` |

**Atributos.** `largo` (m ●) · `ancho` (m ●) · `altura_util` (m ●) · `cota_fondo` (m ●) · `cota_agua` (m ○) · `numero_camaras` (int ●) · `numero_pantallas` (int ◐) · `separacion_pantallas` (m ◐) · `ancho_pantalla` (m ◐) · `numero_aletas` (int ◐) · `angulo_aletas` (° ◐) · `separacion_aletas` (m ◐) · `diametro_helice` (m ◐) · `numero_giros` (int ◐) · `pendiente_canal` (m/m ◐) · `numero_paletas` (int ◐) · `diametro_paletas` (m ◐) · `potencia` (kW ◐) · `diametro_entrada` (mm ○) · `diametro_salida` (mm ○) · `caudal_diseno` (L/s ▣) · `gradiente_diseno` (s⁻¹ ▣) · `tiempo_retencion` (min ▣) · `material` (enum ○) · `estado`.

**Pieza.** `prisma` (muros) + `caja` (pantallas y aletas) + `rampa` (pantalla inclinada) + `canal` (helicoidal). LOD1: caja con tres tabiques.

**Norma.** RNE OS.020 §5.9 (floculación y sus tipologías, §5.9.3).

#### `agua.sedimentador` — Sedimentador de agua potable
`SD` · polígono · nodo · 5 subtipos · P1

Unidad de decantación que separa el flóculo: flujo horizontal con rasquetas, alta tasa con placas, manto de lodos, módulos tubulares o decantador de contacto.

| subtipo | qué representa | parámetros distintivos |
|---|---|---|
| `convencional` | sedimentador de flujo horizontal con rasquetas y tolva | `largo`, `ancho`, `tirante`, `tipo_rasqueta` |
| `alta_tasa_placas` | decantador de alta tasa con placas inclinadas | `numero_placas`, `separacion_placas`, `angulo_placas` |
| `manto_lodos` | clarificador de manto de lodos | `tirante`, `diametro_interior`, `cota_cresta_vertedero` |
| `lamela_tubular` | módulos tubulares o lamelas | `numero_modulos`, `angulo_placas`, `separacion_placas` |
| `decantador_contacto` | decantador de contacto (recirculación de lodos) | `diametro_interior`, `cota_agua`, `numero_camaras` |

**Atributos.** `largo` (m ●) · `ancho` (m ●) · `altura_util` (m ●) · `tirante` (m ●) · `cota_fondo` (m ●) · `cota_agua` (m ●) · `cota_cresta_vertedero` (m ◐) · `longitud_vertedero` (m ◐) · `numero_unidades` (int ○) · `numero_placas` (int ◐) · `separacion_placas` (m ◐) · `angulo_placas` (° ◐) · `numero_modulos` (int ◐) · `diametro_interior` (m ◐) · `numero_camaras` (int ◐) · `tipo_rasqueta` (enum ◐) · `diametro_entrada` (mm ○) · `diametro_salida` (mm ○) · `diametro_purga` (mm ●) · `caudal_diseno` (L/s ▣) · `carga_superficial` (m³/m²·d ▣) · `tiempo_retencion` (min ▣) · `material` (enum ○) · `estado`.

**Pieza.** `prisma` (muros) + `caja` (losa y canaletas de salida) + `tronco` (tolva) + `rampa` (placas y lamela) + `barra` (rasqueta). LOD1: caja con una tolva.

**Norma.** RNE OS.020 §5.10 (decantación; criterios en §5.10.2.2); AWWA/ASCE, *Water Treatment Plant Design* (capítulo de sedimentación).

#### `agua.filtro` — Filtro
`FI` · polígono · nodo · 5 subtipos · P1

Unidad terminal de remoción de partículas: lecho granular con lavado, o membrana. Define la planta de la PTAP y es lo primero que se reconoce en una visita.

| subtipo | qué representa | parámetros distintivos |
|---|---|---|
| `rapido_gravedad` | filtro rápido de arena por gravedad con lavado inverso | `numero_celdas`, `altura_lecho`, `tipo_lecho` |
| `rapido_presion` | filtro rápido a presión, envasado | `diametro`, `altura_lecho`, `diametro_entrada` |
| `lento_arena` | filtro lento de arena, de gran superficie | `largo`, `ancho`, `altura_lecho` |
| `carbon_activado` | filtro de carbón activado granular (GAC) | `altura_lecho`, `tipo_lecho`, `numero_celdas` |
| `membrana_uf` | bloque de ultrafiltración (membranas sumergidas o presurizadas) | `numero_modulos`, `area_membrana`, `diametro_tuberia` |

**Atributos.** `largo` (m ●) · `ancho` (m ●) · `altura_total` (m ●) · `altura_lecho` (m ●) · `altura_borde_libre` (m ●) · `cota_fondo` (m ●) · `cota_agua` (m ○) · `cota_canaleta_lavado` (m ○) · `numero_celdas` (int ●) · `tipo_lecho` (enum ●) · `espesor_lecho` (m ○) · `diametro` (m ◐) · `numero_modulos` (int ◐) · `area_membrana` (m² ◐) · `diametro_tuberia` (mm ◐) · `diametro_entrada` (mm ○) · `diametro_salida` (mm ○) · `diametro_lavado` (mm ○) · `caudal_diseno` (L/s ▣) · `tasa_filtracion` (m³/m²·h ▣) · `granulometria` (mm ▣) · `material` (enum ○) · `estado`.

**Pieza.** `caja` (celdas) + `prisma` (muros y canaletas) + `tubo` (falso fondo) + `barra` (canaletas de lavado). LOD1: caja con el nivel de lecho. LOD2: falso fondo y canaletas.

**Norma.** RNE OS.020 §5.11 (filtración; §5.11.2.3 filtros rápidos convencionales de un material, §5.11.2.4 filtros rápidos de lechos mixtos y múltiples).

#### `agua.desinfeccion` — Unidad de desinfección
`DI` · punto · dispositivo de arco · 6 subtipos · P1

Equipo que dosifica el desinfectante o irradia el agua: hipoclorito, cloro gas, electrocloración, UV u ozono. El tanque de contacto es `agua.tanque:contacto` y la caseta es `comun.caseta:cloracion`.

| subtipo | qué representa | parámetros distintivos |
|---|---|---|
| `hipoclorito_sodio` | dosificación de hipoclorito de sodio en solución | `capacidad_tanque`, `numero_bombas`, `caudal_dosificacion` |
| `hipoclorito_calcio` | dosificación de hipoclorito de calcio (pastillas o granulado) | `capacidad_tanque`, `numero_bombas` |
| `cloro_gas` | cloración con cloro gas (cilindros y clorador) | `numero_cilindros`, `capacidad_cilindro`, `caudal_dosificacion` |
| `electrocloracion` | generación in situ a partir de sal | `capacidad_tanque`, `potencia` (kW), `consumo_sal` |
| `uv` | reactor ultravioleta en línea | `numero_lampas`, `potencia` (kW), `diametro_tuberia` |
| `ozono` | generador de ozono con cámara de contacto | `potencia` (kW), `numero_camaras`, `diametro_tuberia` |

**Atributos.** `tipo_desinfectante` (enum ●) · `diametro_conexion` (mm ●) · `capacidad_tanque` (m³ ○) · `numero_bombas` (int ○) · `numero_cilindros` (int ◐) · `capacidad_cilindro` (kg ◐) · `numero_lampas` (int ◐) · `numero_camaras` (int ◐) · `potencia` (kW ○) · `consumo_sal` (kg/d ◐) · `diametro_tuberia` (mm ◐) · `dosis_diseno` (mg/L ▣) · `concentracion_solucion` (% ▣) · `caudal_dosificacion` (L/h ▣) · `tiempo_contacto` (min ▣) · `estado`.

**Pieza.** `caja` (gabinete y caseta) + `cilindro` (tanque de solución y cilindros de gas) + `barra_tapada` (tubería) + `barra` (soportes). LOD1: caja y cilindro.

**Por qué dispositivo y no cámara.** El equipo se instala sobre la tubería; el volumen de contacto se dibuja con `agua.tanque:contacto` (que ya tiene `numero_bafles`). Así no se duplica el tanque y el tiempo de contacto se declara como dato, no se calcula.

**Norma.** RNE OS.020 (desinfección); 40 CFR 141.70–141.75 (Surface Water Treatment Rule) y 40 CFR 141.400 (Ground Water Rule) para la obligación de desinfectar; EPA 815-R-06-007 para UV.

### 3.6 Etapa E — Almacenamiento, bombeo y servicios

#### `agua.bombeo` — Estación de bombeo de agua potable
`BP` · punto · nodo · 5 subtipos · P1

Escalón de elevación de agua cruda o tratada: cámara de succión, bombas, válvulas de retención y compuerta, y la impulsión que sale.

| subtipo | qué representa | parámetros distintivos |
|---|---|---|
| `camara_succion` | cámara o tanque de succión de bombas | `volumen_util`, `diametro_succion`, `cota_nivel_min` |
| `camara_humeda` | cámara húmeda con bombas sumergibles | `diametro_interior`, `profundidad`, `numero_bombas` |
| `camara_seca` | cámara seca con bombas de eje horizontal | `largo`, `ancho`, `numero_bombas` |
| `booster_en_linea` | estación de refuerzo en línea, sin cámara | `diametro_succion`, `diametro_impulsion`, `numero_bombas` |
| `paquete_compacto` | equipo compacto prefabricado (PRFV/PEAD) | `diametro_interior`, `altura`, `numero_bombas` |

**Atributos.** `diametro_interior` (m ◐) · `largo` (m ◐) · `ancho` (m ◐) · `altura` (m ◐) · `profundidad` (m ●) · `altura_util` (m ○) · `cota_fondo` (m ●) · `cota_tapa` (m ○) · `cota_nivel_min` (m ◐) · `numero_bombas` (int ●) · `tipo_bomba` (enum ●) · `numero_valvulas` (int ○) · `diametro_succion` (mm ○) · `diametro_impulsion` (mm ●) · `nivel_arranque` (m ○) · `nivel_parada` (m ○) · `volumen_util` (m³ ○) · `caudal_bombeo` (L/s ▣) · `altura_manometrica` (m ▣) · `potencia_unitaria` (kW ▣) · `rendimiento` (% ▣) · `material` (enum ○) · `estado`.

**Pieza.** `tubo` + `tronco` + `anillo` (cámara) + `caja` (losa y caseta) + `cilindro` (bombas) + `barra_tapada` (succión e impulsión) + `barra` (escalera). LOD1: cilindro enterrado con tapa.

**Por qué familia.** Es la versión de agua potable de `saneamiento.bombeo`, con el mismo criterio con que el registro ya separa `agua.tuberia` de `saneamiento.tuberia`. La estación de agua cruda y la de agua tratada no son saneamiento, y su equipamiento (succión, impulsión, AMT) es otro.

**Norma.** RNE OS.040 (estaciones de bombeo de agua para consumo humano); RAS, Título 2 / capítulo B.8 (número y capacidad de bombas).

#### `comun.lecho_secado` — Lecho de secado
`LS` · polígono · soporte · 4 subtipos · P2

Batería de celdas con drenes donde se deshidrata el lodo de los sedimentadores, de AP o de AL, antes de su disposición.

| subtipo | qué representa | parámetros distintivos |
|---|---|---|
| `arena_grava` | lecho de arena y grava con drenes enterrados | `espesor_arena`, `espesor_grava`, `diametro_dren` |
| `pavimentado` | lecho pavimentado con pendiente y drenaje | `pendiente_fondo`, `numero_celdas` |
| `cubierto` | lecho cubierto (invernadero) para clima lluvioso | `altura_muro`, `tipo_cubierta` |
| `mecanizado` | lecho con equipo de volteo o carga mecanizada | `largo`, `ancho`, `altura_muro` |

**Atributos.** `largo` (m ●) · `ancho` (m ●) · `altura_muro` (m ●) · `numero_celdas` (int ●) · `espesor_arena` (m ◐) · `espesor_grava` (m ◐) · `diametro_dren` (mm ◐) · `pendiente_fondo` (m/m ●) · `cota_fondo` (m ●) · `cota_rasante` (m ○) · `tipo_cubierta` (enum ◐) · `material` (enum ○) · `estado`.

**Pieza.** `caja` (celdas) + `prisma` (muros) + `barra` (drenes) + `tronco` (cubierta). LOD1: una caja baja subdividida.

**Por qué familia.** Es una batería de celdas que se cuenta por m² y se mantiene; hoy vive como subtipo de `agua.planta` y como tal no se puede poner dos baterías en la misma planta ni una en una PTAR.

**Norma.** RNE OS.090 §5.9 (tratamiento de lodos; lagunas de lodos y secado).

#### `comun.caseta` — Caseta
`CS` · punto · soporte · 7 subtipos · P2

Edificación de planta que aloja un proceso o un servicio: dosificación, cloración, válvulas, operación, laboratorio, almacén o vigilancia. Se cuenta por m² construidos.

| subtipo | qué representa | parámetros distintivos |
|---|---|---|
| `dosificacion` | caseta de dosificación de coagulante u otro insumo | `largo`, `ancho`, `altura` |
| `cloracion` | caseta de cloración (almacén y equipo) | `largo`, `ancho`, `altura` |
| `valvulas` | caseta sobre cámara de válvulas | `largo`, `ancho`, `cota_piso` |
| `operacion` | caseta de operación y control | `largo`, `ancho`, `numero_plantas` |
| `laboratorio` | laboratorio de control de calidad | `largo`, `ancho`, `altura` |
| `almacen` | almacén de insumos y repuestos | `largo`, `ancho`, `altura` |
| `guardiania` | caseta de vigilancia y control de acceso | `largo`, `ancho`, `altura` |

**Atributos.** `largo` (m ●) · `ancho` (m ●) · `altura` (m ●) · `numero_plantas` (int ○) · `tipo_cubierta` (enum ○) · `cota_piso` (m ●) · `material` (enum ○) · `estado`.

**Pieza.** `caja` (volumen) + `tronco` (cubierta) + `caja` (puerta) + `barra` (ventana). LOD1: una caja.

**Por qué familia.** Sin ella no se puede contar el área construida de la planta ni ubicar el punto de dosificación, que es lo que se revisa en una inspección. Se limita a casetas de proceso: la arquitectura administrativa y el urbanismo de planta quedan fuera (§6).

#### `comun.aforador` y `comun.reja` y `comun.compuerta`
Ya descritos en §3.4. Se listan aquí porque aparecen en las tres etapas (captación, conducción y planta).

### 3.7 Etapa F — Salida del alcantarillado

#### `saneamiento.sedimentador` — Sedimentador de aguas residuales
`SE` · polígono · nodo · 7 subtipos · P1

Unidad de separación por gravedad del alcantarillado: primario, secundario, tanque Imhoff, séptico, espesador o lamelar.

| subtipo | qué representa | parámetros distintivos |
|---|---|---|
| `primario` | sedimentador primario con rasquetas y tolva | `largo`, `ancho`, `tirante`, `tipo_rasqueta` |
| `secundario` | sedimentador secundario tras lodos activados | `diametro_interior`, `tirante`, `cota_cresta` |
| `imhoff` | tanque Imhoff: sedimentación arriba y digestión abajo | `numero_camaras`, `volumen_digestion`, `altura_util` |
| `septico` | tanque séptico de una o dos cámaras | `largo`, `ancho`, `profundidad` |
| `septico_con_filtro` | séptico con filtro anaerobio de salida | `largo`, `ancho`, `altura_filtro` |
| `espesador` | espesador de lodos por gravedad | `diametro_interior`, `profundidad`, `tipo_rasqueta` |
| `lamela` | sedimentador lamelar en PTAR | `numero_modulos`, `angulo_placas`, `separacion_placas` |

**Atributos.** `largo` (m ●) · `ancho` (m ●) · `altura_util` (m ●) · `tirante` (m ●) · `cota_fondo` (m ●) · `cota_agua` (m ●) · `cota_cresta` (m ◐) · `longitud_vertedero` (m ◐) · `numero_unidades` (int ○) · `numero_camaras` (int ◐) · `volumen_digestion` (m³ ◐) · `altura_filtro` (m ◐) · `diametro_interior` (m ◐) · `profundidad` (m ◐) · `numero_modulos` (int ◐) · `angulo_placas` (° ◐) · `separacion_placas` (m ◐) · `tipo_rasqueta` (enum ◐) · `diametro_entrada` (mm ○) · `diametro_salida` (mm ○) · `diametro_purga` (mm ●) · `caudal_diseno` (L/s ▣) · `carga_superficial` (m³/m²·d ▣) · `tiempo_retencion` (min ▣) · `material` (enum ○) · `estado`.

**Pieza.** `prisma` (muros) + `caja` (losa) + `tronco` (tolva) + `caja` (pantalla deflectora del Imhoff) + `tubo` (colector de lodos). LOD1: caja con tolva.

**Por qué familia.** La PTAR se dibuja por unidades, no por «tipo de planta». El Imhoff y el séptico tienen cámara de digestión: la pieza lo muestra y el revisor lo cuenta.

**Norma.** RNE OS.090 §5 (tratamiento primario y tanque Imhoff; sedimentador secundario §5.5.3.2; espesamiento §5.9).

#### `saneamiento.reactor` — Reactor biológico
`RX` · polígono · nodo · 5 subtipos · P1

Unidad de tratamiento biológico: lodos activados, aireación extendida, UASB, SBR o lecho móvil, con su aireación y sus difusores.

| subtipo | qué representa | parámetros distintivos |
|---|---|---|
| `lodos_activos` | reactor aerobio de lodos activados con difusores | `numero_camaras`, `numero_difusores`, `potencia_aireacion` |
| `aireacion_extendida` | zanja de oxidación de aireación extendida | `largo`, `ancho`, `numero_aireadores` |
| `uasb` | reactor anaerobio de flujo ascendente (RAFA) | `altura_util`, `numero_camaras`, `diametro_salida_gas` |
| `sbr` | reactor secuencial por lotes | `numero_camaras`, `tirante`, `numero_decantadores` |
| `mbbr` | reactor de lecho móvil con relleno | `volumen_relleno`, `numero_difusores`, `tirante` |

**Atributos.** `largo` (m ●) · `ancho` (m ●) · `altura_util` (m ●) · `tirante` (m ●) · `cota_fondo` (m ●) · `cota_agua` (m ●) · `volumen_util` (m³ ○) · `numero_camaras` (int ●) · `tipo_aireacion` (enum ●) · `numero_difusores` (int ◐) · `numero_aireadores` (int ◐) · `numero_sopladores` (int ◐) · `potencia_aireacion` (kW ◐) · `volumen_relleno` (m³ ◐) · `numero_decantadores` (int ◐) · `diametro_entrada` (mm ○) · `diametro_salida` (mm ○) · `diametro_salida_gas` (mm ◐) · `caudal_diseno` (L/s ▣) · `carga_organica` (kg DBO/d ▣) · `tiempo_retencion` (h ▣) · `material` (enum ○) · `estado`.

**Pieza.** `prisma` (muros) + `caja` (losa) + `barra` (difusores y barandilla) + `cilindro` (sopladores y aireadores). LOD1: caja subdividida.

**Norma.** RNE OS.090 §5.5.3 (lodos activados; sedimentador secundario y zanjas de oxidación) y §5.6.3 (tratamiento anaerobio de flujo ascendente); Metcalf & Eddy, *Wastewater Engineering*.

#### `saneamiento.laguna` — Laguna de estabilización
`LG` · polígono · nodo · 6 subtipos · P1

Estanque de tierra con taludes y estructuras de entrada y salida donde el agua residual se trata por acción bacteriana y algal; puede ser facultativa, aerobia, anaerobia, aireada, de maduración o de lodos.

| subtipo | qué representa | parámetros distintivos |
|---|---|---|
| `facultativa` | laguna facultativa (zona aerobia y anaerobia) | `profundidad`, `talud`, `carga_organica` |
| `aerobia` | laguna aerobia de poca profundidad | `profundidad`, `ancho_corona` |
| `anaerobia` | laguna anaerobia profunda | `profundidad`, `talud`, `cubierta` |
| `aireada` | laguna aireada con sopladores o flotadores | `numero_aireadores`, `potencia` (kW) |
| `maduracion` | laguna de maduración (pulido y remoción de patógenos) | `profundidad`, `numero_celdas` |
| `lodos` | laguna de lodos (almacenamiento y secado) | `profundidad`, `numero_celdas` |

**Atributos.** `largo` (m ●) · `ancho` (m ●) · `profundidad` (m ●) · `cota_fondo` (m ●) · `cota_agua` (m ●) · `talud` (H:V ●) · `ancho_corona` (m ●) · `numero_celdas` (int ○) · `revestimiento` (enum ○) · `cubierta` (bool ◐) · `numero_aireadores` (int ◐) · `potencia` (kW ◐) · `diametro_entrada` (mm ○) · `diametro_salida` (mm ○) · `caudal_diseno` (L/s ▣) · `carga_organica` (kg DBO/ha·d ▣) · `tiempo_retencion` (d ▣) · `estado`.

**Pieza.** `prisma` (taludes) + `caja` (fondo) + `canal` (entrada y salida) + `tronco` (vertedero de salida). LOD1: un prisma bajo.

**Por qué familia.** Es la tecnología más difundida en la región y su forma (taludes, corona, celdas) no cabe en un «tipo de planta». Además hoy está en el dominio equivocado: `agua.planta:laguna_estabilizacion`.

**Norma.** RNE OS.090 §5.5.2 (lagunas y remoción de patógenos); EPA 625/1-83-015 (*Design Manual: Municipal Wastewater Stabilization Ponds*).

### 3.8 Etapa G — Captación subterránea: lo que se queda y lo que se añade

`agua.captacion` no se parte: sigue siendo la captación de manantial y de pozo. Pierde tres subtipos hacia `agua.bocatoma` (§4) y gana tres en §4. Su ficha de atributos se completa en §5.

---

## 4. HUECOS EN FAMILIAS EXISTENTES

### 4.1 Subtipos que faltan

| familia | subtipo que falta | atributos | por qué |
|---|---|---|---|
| `agua.captacion` | `toma_directa` | `cota_umbral`, `ancho_ventana`, `altura_ventana`, `ancho_reja`, `abertura_reja` | RNE OS.010 §4.1: la toma directa sin azud (rejilla y cámara) es distinta de la toma con presa, que hoy es el único caso cubierto |
| `agua.captacion` | `toma_flotante` | `diametro_succion`, `longitud_flexible`, `volumen_flotador` | captación en embalse o lago; práctica local en represas andinas y en la costa |
| `agua.captacion` | `pozo_radial` | `longitud_drenes`, `numero_drenes`, `diametro_dren` | galería horizontal radial; poco frecuente, pero se dibuja y se cuenta |
| `agua.tuberia` | `rebose` | `diametro_nominal`, `cota_cresta`, `longitud_vertedero` | todo reservorio tiene rebose y hoy solo existe en `saneamiento.tuberia:rebose` |
| `agua.tuberia` | `desague` | `diametro_nominal`, `tipo_valvula` | vaciado de tanque y de conducción; hoy solo en saneamiento |
| `agua.tuberia` | `conduccion_forzada` | `clase_presion`, `cota_clave`, `cota_clave_fin` | tramo a presión dentro de una conducción por gravedad: cruce, sifón, paso bajo cauce |
| `agua.tuberia` | `limpia` | `diametro_nominal`, `pendiente` | tubería de limpia de bocatoma y desarenador (OS.010 §4) |
| `agua.tanque` | `succion` | `volumen_util`, `diametro_succion`, `cota_nivel_min` | tanque de succión de la estación de bombeo (OS.040) |
| `agua.tanque` | `elevado_domiciliario` | `volumen_util`, `diametro`, `altura` | tanque de techo; aparece en toda acometida rural y no es un tanque elevado de torre |
| `agua.valvula` | `guillotina` | `diametro_nominal`, `tipo_accionamiento` | cierre de líneas de lodos y de purga de desarenadores |
| `agua.valvula` | `pinch` | `diametro_nominal`, `material_cuerpo` | lodos y dosificación; real en PTAP y PTAR |
| `agua.arqueta` | `muestreo` | `diametro`, `altura`, `llave_muestreo` (bool) | toma de muestras de calidad en planta y en red |
| `agua.planta` | `ptap_remocion_hierro_manganeso` | `numero_unidades`, `largo`, `ancho` | agua subterránea con Fe/Mn: oxidación y filtración; común en costa y sierra |
| `agua.planta` | `ptap_ablandamiento` | `numero_unidades`, `diametro_tanque` | dureza alta (cal-soda o resinas) |
| `agua.planta` | `ptar_primario` | `numero_unidades`, `largo`, `ancho` | planta que solo hace primario (sedimentación y desinfección); existe en localidades pequeñas |
| `comun.cruce` | `hinca` | `longitud_cruce`, `diametro_camisa`, `profundidad_minima` | cruce sin zanja (perforación dirigida o hinca) en vías y ferrocarril |
| `saneamiento.canal` | `bypass` | `base`, `pendiente`, `longitud` | canal de by-pass de planta: derivación de emergencia que OS.090 exige |

Total: **17 subtipos nuevos** en familias existentes (`agua.captacion` 3, `agua.tuberia` 4, `agua.tanque` 2, `agua.valvula` 2, `agua.arqueta` 1, `agua.planta` 3, `comun.cruce` 1, `saneamiento.canal` 1).

### 4.2 Subtipos que se retiran (con alias, nunca en silencio)

| clave retirada | destino | motivo |
|---|---|---|
| `agua.captacion:toma_rio` | `agua.bocatoma:barraje_fijo` | la toma con presa es una bocatoma; el dominio y la forma cambian |
| `agua.captacion:toma_quebrada` | `agua.bocatoma:bocatoma_lateral` | ídem |
| `agua.captacion:rejilla_fondo` | `agua.bocatoma:toma_tirolesa` | la rejilla en el lecho es una toma tirolesa, no una captación de manantial |
| `saneamiento.camara:carga` | `agua.camara:carga` | cámara de conducción de agua potable en dominio de saneamiento (H5) |
| `saneamiento.camara:quiebre` | `agua.camara:quiebre` | ídem |
| `saneamiento.camara:valvulas` | `agua.camara:valvulas` | ídem |
| `saneamiento.camara:medicion` | `agua.camara:medicion` | ídem |
| `saneamiento.camara:reparto` | `agua.camara:reparto` | ídem |
| `saneamiento.camara:alivio` | `agua.camara:alivio` | ídem |
| `agua.planta:cloracion` | `comun.caseta:cloracion` + `agua.desinfeccion:hipoclorito_sodio` | una caseta no es una planta, y el equipo es otro objeto |
| `agua.planta:lecho_secado` | `comun.lecho_secado:arena_grava` | unidad, no planta |
| `agua.planta:espesador` | `saneamiento.sedimentador:espesador` | unidad de AL, no de AP |
| `agua.planta:laguna_estabilizacion` | `saneamiento.laguna:facultativa` | una laguna es una unidad, no una planta; hoy está además en el dominio del agua |

Las cuatro variantes `ptap_*` y las tres `ptar_*` de `agua.planta` **no** se retiran: se conservan como «planta completa» esquemática para cuando el origen solo trae la instalación entera. La unidad suelta, cuando existe, manda (§3.5–§3.7).

Efecto en los conteos: `saneamiento.camara` 11 → 5 (sigue cumpliendo el mínimo de 4); `agua.planta` 11 → 10; `agua.captacion` 8 → 5 + 3 = 8. Retiros totales: 13.

### 4.3 Huecos que **no** son de subtipo

| familia | hueco | qué falta | por qué |
|---|---|---|---|
| `saneamiento.pozo` | sin subtipos nuevos | atributos: `diametro_tapa`, `tipo_tapa`, `sistema_constructivo`, `cota_rasante` | los 12 subtipos cubren el oficio; lo que falta es poder dibujar la tapa y saber si es fundido o prefabricado |
| `agua.captacion` | sin subtipos nuevos (aparte de §4.1) | capacidades: `EXCAVATABLE` | es una estructura enterrada y no emite excavación (H13) |
| `agua.planta` | sin subtipos nuevos (aparte de §4.1) | capacidades: `EXCAVATABLE`; atributo `tipo_tratamiento` | ídem; y el «tipo de tratamiento» de OS.020 es una clasificación del agua, no una forma |
| `saneamiento.bombeo` | sin subtipos nuevos | quitar `SOURCE`; `potencia_unitaria` en kW | H6 y H2 |

---

## 5. ATRIBUTOS NUEVOS

### 5.1 Atributos en familias existentes

| familia | atributo | unidad | tipo | grupo | obl. | por qué |
|---|---|---|---|---|---|---|
| `agua.captacion` | `caudal_disponible` | L/s | Num | Hidraulico | ● | el caudal aforado es el dato que define si la fuente alcanza; hoy solo hay «concesionado» |
| `agua.captacion` | `caudal_ecologico` | L/s | Num | Hidraulico | ○ | restricción legal de la fuente (OS.010 §3) |
| `agua.captacion` | `nivel_estatico` | m | Num | Hidraulico | ◐ | pozo: nivel sin bombeo |
| `agua.captacion` | `nivel_dinamico` | m | Num | Hidraulico | ◐ | pozo: nivel con bombeo; define la profundidad de la bomba |
| `agua.captacion` | `cota_agua` | m | Num | Geometrico | ○ | cota de la lámina en la cámara de captación |
| `agua.captacion` | `cota_umbral` | m | Num | Geometrico | ◐ | umbral de la ventana de toma |
| `agua.captacion` | `tipo_bomba` | — | Enum | General | ◐ | pozo con bomba: sumergible, vertical, eyector |
| `agua.captacion` | `potencia_bomba` | kW | Num | Operacion | ◐ | equipo instalado en el pozo |
| `agua.captacion` | `diametro_succion` | mm | Num | Hidraulico | ◐ | salida del pozo a la conducción |
| `agua.captacion` | `numero_unidades` | — | Int | General | ○ | captaciones dobles o de reserva |
| `agua.captacion` | `calidad_turbiedad` | UNT | Num | Operacion | ○ | dato de calidad que decide el tren de tratamiento |
| `agua.tuberia` | `cota_clave_fin` | m | Num | Geometrico | ● | sin ella no se puede revisar un perfil sin recalcular (H7) |
| `agua.tuberia` | `regimen` | — | Enum | Hidraulico | ● | gravedad, bombeo, lámina libre: es atributo, no subtipo |
| `agua.tuberia` | `tipo_junta` | — | Enum | Material | ○ | espiga-campana, bridada, soldada, electrofusión |
| `agua.tuberia` | `recubrimiento` | m | Num | Geometrico | D | derivado de cotas: rasante − (solera + DI + espesor); no es la profundidad (tapa − solera) |
| `agua.tuberia` | `numero_tramos` | — | Int | General | ○ | conteo de tramos entre cámaras |
| `agua.tanque` | `cota_rebose` | m | Num | Hidraulico | ● | el rebose es lo que más se revisa en una inspección (H10) |
| `agua.tanque` | `cota_desague` | m | Num | Hidraulico | ○ | vaciado |
| `agua.tanque` | `borde_libre` | m | Num | Geometrico | ● | separa NAMO de la losa |
| `agua.tanque` | `tipo_entrada` | — | Enum | Hidraulico | ○ | flotador, compuerta, vertedero |
| `agua.tanque` | `volumen_total` | m³ | Num | Hidraulico | ○ | el útil ya existe; el total es el que se construye |
| `agua.tanque` | `diametro_entrada` | mm | Num | Hidraulico | ○ | para dibujar la acometida al tanque |
| `agua.tanque` | `diametro_salida` | mm | Num | Hidraulico | ○ | ídem |
| `agua.valvula` | `tipo_accionamiento` | — | Enum | Operacion | ● | manual, eléctrico, hidráulico, neumático |
| `agua.valvula` | `consigna_presion` | mca | Num | Hidraulico | ◐ | PRV, sostenedora, alivio |
| `agua.valvula` | `consigna_caudal` | L/s | Num | Hidraulico | ◐ | reguladora de caudal |
| `agua.valvula` | `rango_tarado` | mca | Num | Hidraulico | ◐ | alivio y seguridad |
| `agua.valvula` | `diametro_bypass` | mm | Num | General | ○ | by-pass de la válvula |
| `agua.medidor` | `q_max` | m³/h | Num | Hidraulico | ● | ISO 4064: sin Q máx/mín no se revisa un medidor |
| `agua.medidor` | `q_min` | m³/h | Num | Hidraulico | ● | ídem |
| `agua.medidor` | `perdida_carga_qnom` | mca | Num | Hidraulico | ○ | pérdida a caudal nominal |
| `agua.medidor` | `clase_metrologica` | — | Enum | General | ○ | A, B, C |
| `agua.medidor` | `tipo_lectura` | — | Enum | Operacion | ○ | local, remota, pulsos |
| `agua.medidor` | `numero_serie` | — | Text | Operacion | ○ | trazabilidad |
| `saneamiento.pozo` | `diametro_tapa` | mm | Num | Geometrico | ● | la tapa define el cono y el marco |
| `saneamiento.pozo` | `tipo_tapa` | — | Enum | General | ○ | peatonal, vehicular ligera, vehicular pesada |
| `saneamiento.pozo` | `sistema_constructivo` | — | Enum | Material | ○ | fundido en sitio, prefabricado |
| `saneamiento.pozo` | `cota_rasante` | m | Num | Geometrico | ○ | la rasante no siempre es la tapa |
| `saneamiento.camara` | `numero_tapas` | — | Int | General | ● | el dibujo reparte tapas en la losa |
| `saneamiento.camara` | `diametro_tapa` | mm | Num | Geometrico | ○ | ídem |
| `saneamiento.camara` | `cota_tapa` | m | Num | Geometrico | ● | falta hoy, y sin ella la cámara no se coloca |
| `saneamiento.camara` | `espesor_pared` | m | Num | Estructural | ○ | ídem |
| `saneamiento.camara` | `espesor_losa` | m | Num | Estructural | ○ | ídem |
| `saneamiento.camara` | `tipo_vertedero` | — | Enum | Hidraulico | ◐ | delgada, ancha |
| `saneamiento.tuberia` | `cota_clave_fin` | m | Num | Geometrico | ● | mismo motivo que en `agua.tuberia` |
| `saneamiento.tuberia` | `tipo_junta` | — | Enum | Material | ○ | define si se dibujan campanas |
| `saneamiento.tuberia` | `rigidez_anular` | — | Enum | Material | ◐ | SN2, SN4, SN8 en plásticos |
| `saneamiento.bombeo` | `potencia_unitaria` | kW | Num | Operacion | ● | sustituye `potencia_instalada`, que hoy no tiene unidad (H2) |
| `saneamiento.bombeo` | `tipo_bomba` | — | Enum | General | ● | sumergible, centrífuga horizontal, vertical, eyector |
| `saneamiento.bombeo` | `nivel_arranque` | m | Num | Operacion | ○ | operación, no forma |
| `saneamiento.bombeo` | `nivel_parada` | m | Num | Operacion | ○ | ídem |
| `saneamiento.bombeo` | `tiempo_retencion` | min | Num | Hidraulico | ○ | dato de diseño |
| `agua.planta` | `tipo_tratamiento` | — | Enum | General | ● | clasificación de OS.020: I, II-A, II-B, II-C, III |
| `agua.planta` | `caudal_punta` | L/s | Num | Hidraulico | ○ | el de diseño ya existe; la punta es otro dato |
| `agua.planta` | `tren_proceso` | — | Text | General | ○ | secuencia de unidades en texto («desarenador–mezcla–floculador–filtro–cloro») |
| `agua.planta` | `cota_hidraulica` | m | Num | Geometrico | ● | cota de la lámina en el punto de entrega |
| `agua.planta` | `area` | m² | Num | Geometrico | ○ | huella para el listado |
| `agua.planta` | `punto_descarga` | — | Text | General | ◐ | referencia al cuerpo receptor (PTAR) |
| `comun.cruce` | `angulo_cruce` | ° | Num | Geometrico | ○ | interferencia real, no solo longitud |
| `comun.cruce` | `permiso_requerido` | — | Text | Operacion | ○ | quién autoriza el cruce |
| `comun.cruce` | `entidad_autorizante` | — | Text | Operacion | ○ | ídem |
| `comun.cuerpo_receptor` | `uso` | — | Enum | General | ○ | abastecimiento, riego, recreación, vertido: cambia el criterio de revisión |

### 5.2 Unidades que faltan en `Unit`

| variante | símbolo | `key` | por qué |
|---|---|---|---|
| `Kw` | kW | `kw` | potencia de bombas, agitadores, sopladores y UV; hoy `potencia_instalada` no tiene unidad |
| `M3h` | m³/h | `m3h` | caudal de medidores (Q máx/mín) y de dosificación |
| `M3d` | m³/d | `m3d` | caudal de diseño de plantas y lagunas |
| `MgL` | mg/L | `mgl` | dosis de coagulante, cloro y ozono; concentración de lodos |
| `H` | h | `h` | tiempo de retención de reactores y lagunas |
| `Min` | min | `min` | tiempo de retención de floculadores y sedimentadores |
| `Unt` | UNT | `unt` | turbiedad, el parámetro que decide el tren de tratamiento |
| `S1` | s⁻¹ | `s1` | gradiente de velocidad, que hoy se declararía sin unidad |

### 5.3 Enums que faltan (hoy son `Text` libre)

| atributo | dónde | valores propuestos | motivo |
|---|---|---|---|
| `material` | 6 familias | PVC-U, PVC-O, PEAD, HD, HF, HG, acero, hormigón simple, hormigón armado, GRP/PRFV, fibrocemento, mampostería | H8: hoy «PVC», «pvc» y «PVC-U» conviven y no se puede agrupar el listado de cantidades |
| `estado` | 12 familias | proyectado, en_construccion, operativo, fuera_de_servicio, abandonado, en_rehabilitacion | ídem; los valores ya están en `docs/consejo/01-domain.md` §4.3 |
| `tipo_bomba` | captación, bombeo | sumergible, centrifuga_horizontal, vertical, eyector | cambia la silueta interior |
| `tipo_accionamiento` | válvula, compuerta | manual, electrico, hidraulico, neumatico | se especifica y se revisa |
| `regimen` | tubería | gravedad, bombeo, lamina_libre | separa conducción por gravedad de impulsión sin crear subtipos |
| `tipo_junta` | tubería | espiga_campana, bridada, soldada, electrofusion, mecanica | define el dibujo de la junta |
| `sistema_constructivo` | pozo, cámara | fundido, prefabricado | cambia el dibujo de las juntas |
| `tipo_tapa` | pozo | peatonal, vehicular_ligera, vehicular_pesada | con `clase_carga` de `comun.tapa` |
| `clase_metrologica` | medidor | A, B, C | ISO 4064 |
| `tipo_lectura` | medidor | local, remota, pulsos | operación |
| `uso` | cuerpo receptor | abastecimiento, riego, recreacion, vertido | criterio de revisión |

**Advertencia de migración.** Convertir `material` y `estado` de `Text` a `Enum` **rompe** los archivos existentes si no se normaliza antes. El orden obligatorio es: (1) añadir el enum; (2) normalizar en la ingesta con tabla de alias; (3) marcar el proyecto como migrado. Nunca al revés.

### 5.4 Primitivas que faltan en `parts.rs`

| primitiva | firma propuesta | para qué familia |
|---|---|---|
| `rampa` | `rampa(poly: &[[f32;2]], z0: f32, z1: f32, m: Material) -> Mesh` | placas y lamela de sedimentador, taludes de laguna, transiciones de aforador |
| `marco_rectangular` | `marco_rectangular(ancho: f32, alto: f32, espesor: f32, m: Material) -> Mesh` | marco de reja y de compuerta (hoy solo hay `anillo` circular) |

Las 11 primitivas actuales (`caja`, `cilindro`, `tronco`, `disco`, `anillo`, `tubo`, `barra`, `barra_tapada`, `prisma`, `canal`, `tuberia`) cubren el resto. `Mesh::girar` solo gira alrededor de Z, así que una placa inclinada **no** se puede componer con lo que hay (H11).

### 5.5 Ensamblajes que hay que escribir

`parts.rs` tiene 17 funciones de pieza para 29 familias (varias familias comparten pieza). Las 17 familias nuevas necesitan **14 ensamblajes** porque tres reutilizan pieza existente: `comun.compuerta` ≈ `agua.valvula` sin cuerpo; `comun.caseta` ≈ `planta` simplificada; `comun.lecho_secado` ≈ `caja` subdividida. Los otros 14 son código nuevo, no datos.

---

## 6. Qué NO debe entrar

| Candidato | Por qué parece de la cadena | Por qué no entra | Dónde vive |
|---|---|---|---|
| Caudal de diseño calculado (QMD, QMH, K1, K2) | toda ficha de captación lo pide | es hidrología y demanda: el visor no lo deriva | atributo ▣ con `fuente` |
| Volumen de regulación del reservorio | se discute en toda memoria | es cálculo | dato ▣ |
| Área de filtración, carga superficial, gradiente, tiempo de retención | parametrizan cada unidad | son resultados de diseño; el registro solo los transporta | atributo ▣ |
| Dosis de coagulante o de cloro (prueba de jarras) | define la caseta de dosificación | es cálculo de laboratorio | atributo ▣ |
| Punto de operación de la bomba (intersección H–Q) | se habla de «curva de bomba» | la curva es dato del fabricante; el punto de operación es cálculo | la curva no entra; caudal y AMT como ▣ |
| Golpe de ariete, transitorios, NPSH | aparecen en toda conducción a presión | cálculo | fuera |
| Volumen de excavación de la unidad | el visor ya emite cantidades | es derivado del motor, no un objeto | derivado existente |
| Sensores y analizadores en línea (turbiedad, pH, cloro residual, nivel radar) | están en la caseta de dosificación | son instrumentación de proceso, no geometría | a lo sumo un texto `instrumentacion` en la unidad |
| PLC, SCADA, tablero, telemetría, datalogger | se dibujan en los planos de planta | automatización, no red | fuera |
| Lazo de control de dosificación | se confunde con el dosificador | el equipo entra; el control no | equipo = `agua.desinfeccion` |
| Cerco perimétrico, jardines, vías internas, estacionamiento, oficinas | están en el plano de planta | arquitectura y urbanismo, no red | fuera (solo casetas de proceso, D12) |
| Grifería y artefactos sanitarios, instalación interior | «llave» también es grifería | es instalación interior del edificio (ADR-01, doc 01 §3.5) | fuera |
| Transformador, grupo electrógeno, medidor de energía | servicios de la planta | eléctrico | solo como `comun.caseta:subestacion` si se quiere contar el volumen |
| Relleno sanitario / botadero («vertedero», 4.ª acepción) | la palabra engaña | no es hidráulica | fuera (doc 06, R1) |
| Canal de riego, obras agrícolas, drenaje parcelario | se dibujan con `saneamiento.canal` | otra disciplina y otro cliente | no se crea familia |
| Modelos 3D de fabricante (bomba, válvula, filtro) | «que se vea como en la realidad» | es la biblioteca de modelos, no el catálogo | F6 del backlog |
| Verificación de cumplimiento (dosis mínima, tiempo de contacto, velocidad) | toda norma fija umbrales | el visor no dictamina; solo compara declarado con medido | F7-02 (reconciliación), no catálogo |
| Población, dotación, coeficientes de variación | alimentan el caudal | hidrología y demanda | fuera |
| Trenes de ósmosis inversa y recuperación de energía | `agua.planta:desalinizadora` ya existe | la planta entra como tipo; el tren de membranas es otro producto | fuera de v1 |
| Un `Access::Derived` hidráulico nuevo | el registro ya tiene 6 (H14) | contradice la regla del producto | no se añade ninguno |

---

## 7. Referencias

### 7.1 Normas

| # | Referencia | Uso en este documento |
|---|---|---|
| N1 | Perú, MVCS. **Reglamento Nacional de Edificaciones, Norma OS.010 «Captación y conducción de agua para consumo humano»**, D.S. 011-2006-VIVIENDA. §3 fuentes, §4 captación, §4.1 aguas superficiales, §5 conducción. [Texto](http://www.digesa.minsa.gob.pe/NormasLegales/Normas/OS.010.pdf) | bocatoma, toma directa, desarenador, cámaras de carga y quiebre, válvulas de aire y purga, tubería de limpia |
| N2 | Perú, MVCS. **RNE, Norma OS.020 «Plantas de tratamiento de agua para consumo humano»**, D.S. 011-2006-VIVIENDA, modificada por D.S. 024-2009-VIVIENDA. §5 criterios de unidades: §5.9 floculación (§5.9.3), §5.10 decantación (§5.10.2.2), §5.11 filtración (§5.11.2.3 y §5.11.2.4). [Texto](https://prod1.seace.gob.pe/SeaceWeb-PRO/SdescargarArchivoAlfresco?fileCode=b9d56934-894e-407b-9eaa-ac534a2c8e9f) | mezcla, floculador, sedimentador, filtro, desinfección, reja, tipos de tratamiento |
| N3 | Perú, MVCS. **RNE, Norma OS.030 «Almacenamiento de agua para consumo humano»** y **OS.040 «Estaciones de bombeo de agua para consumo humano»** | tanque de succión, rebose, caseta de válvulas, estación de bombeo AP |
| N4 | Perú, MVCS. **RNE, Norma OS.090 «Plantas de tratamiento de aguas residuales»**. §5.5.2 lagunas (§5.5.2.5 y §5.5.2.6), §5.5.3 lodos activados (§5.5.3.2 sedimentador secundario, §5.5.3.3 zanjas de oxidación), §5.6.3 tratamiento anaerobio de flujo ascendente, §5.9 tratamiento de lodos (§5.9.3 lagunas de lodos). [Texto](http://www.digesa.minsa.gob.pe/NormasLegales/Normas/OS.090.pdf) | sedimentador AL, reactor, laguna, lecho de secado, reja y desarenado, Imhoff |
| N5 | Colombia, MVCT. **Resolución 0330 de 2017** (Reglamento Técnico del Sector Agua Potable y Saneamiento Básico), Título 2 (acueducto). Art. 47 y parágrafo 1: caudal de diseño de bocatoma hasta 2·QMD con devolución del excedente. Capítulo B.8: número y capacidad de bombas (numeración heredada del RAS 2000). | bocatoma, estación de bombeo |
| N6 | Ecuador, INEN. **CPE INEN 5, Parte 9-1** (diseño de sistemas de abastecimiento de agua potable, ed. 1992) y **Parte 9-2** (sistemas rurales, ed. 1997); **NTE INEN 1108** (agua potable, requisitos). Verificadas por referencia secundaria; la edición vigente debe confirmarse en el catálogo del INEN. | práctica ecuatoriana de dotación, caudales y calidad |
| N7 | Estados Unidos. **40 CFR 141.70–141.75** (Surface Water Treatment Rule) y **40 CFR 141.400** (Ground Water Rule). | obligación de filtrar y desinfectar |
| N8 | **EN 124** (tapas y marcos; clases A15 a F900). Ya citada en `docs/consejo/06-catalogo-familias.md` §5.3. | clase de carga de tapas y rejillas |

### 7.2 Manuales y libros

| # | Referencia | Uso |
|---|---|---|
| M1 | U.S. EPA. **Design Manual: Municipal Wastewater Stabilization Ponds**, EPA 625/1-83-015, Cincinnati, 1983. [Registro](https://www.osti.gov/biblio/5177415) | tipologías y forma de lagunas facultativas, aerobias, anaerobias y de maduración |
| M2 | U.S. EPA. **Ultraviolet Disinfection Guidance Manual for the Final Long Term 2 Enhanced Surface Water Treatment Rule**, EPA 815-R-06-007, noviembre 2006. [Texto](https://www.epa.gov/system/files/documents/2022-10/ultraviolet-disinfection-guidance-manual-2006.pdf) | reactor UV: lámparas, potencia, validación |
| M3 | AWWA. **M51 Air Valves: Air-Release, Air/Vacuum, and Combination**, 2.ª ed., 2016. [Ficha](https://webstore.ansi.org/standards/awwa/awwam512016) | válvulas de aire y de purga en conducciones |
| M4 | AWWA. **M42 Steel Water-Storage Tanks**. | reservorios: rebose, ventilación, borde libre |
| M5 | AWWA. **M6 Water Meters — Selection, Installation, Testing, and Maintenance**. | Q máx/mín y clase metrológica del medidor |
| M6 | AWWA/ASCE. **Water Treatment Plant Design**, 5.ª ed., McGraw-Hill, 2012. | sedimentación, filtración, desinfección: criterios de forma |
| M7 | Metcalf & Eddy / Tchobanoglous et al. **Wastewater Engineering: Treatment and Resource Recovery**, 5.ª ed., McGraw-Hill, 2014. | tanque Imhoff, lodos activados, UASB, espesadores |

### 7.3 Documentos internos del producto

| # | Referencia | Uso |
|---|---|---|
| I1 | `docs/consejo/06-catalogo-familias.md` | convención de claves, prefijos, reglas de migración, D2 (subtipo = función), D6 (componentes con padre), D7 (partes del pozo) |
| I2 | `docs/consejo/01-domain.md` | cuatro cotas, catálogos de materiales y estados, ambigüedad «llave/válvula» (ADR-01), «vertedero» (ADR-02) |
| I3 | `crates/redes-core/src/families.rs` | registro verificado a la fecha de este documento (29 familias, 216 subtipos) |
| I4 | `crates/redes-core/src/parts.rs` | primitivas disponibles y ensamblajes existentes |
| I5 | `skills/redes-ingesta/references/mapeos.md` | mapeos de DXF, SWMM y EPANET que hay que ampliar (§8.1) |

### 7.4 Nota sobre práctica local

Tres prácticas de este documento **no son normativas** y se marcan como locales: (a) la separación entre arqueta y cámara por acceso de personal (§3.3) es convención de dibujo, no de norma; (b) el número de naves del desarenador y el tipo de reja dependen del prestador; (c) en Ecuador la norma nacional (N6) se superpone con los pliegos del prestador y del ex IEOS, sin edición única. Cuando un atributo dependa de eso, se deja como texto o enum abierto y se documenta en la ficha del proyecto.

---

## 8. Anexos

### 8.1 Alias obligatorios para la skill `redes-ingesta`

| Alias de origen | Clave nueva |
|---|---|
| `agua.captacion:toma_rio` | `agua.bocatoma:barraje_fijo` |
| `agua.captacion:toma_quebrada` | `agua.bocatoma:bocatoma_lateral` |
| `agua.captacion:rejilla_fondo` | `agua.bocatoma:toma_tirolesa` |
| `saneamiento.camara:carga` | `agua.camara:carga` |
| `saneamiento.camara:quiebre` | `agua.camara:quiebre` |
| `saneamiento.camara:valvulas` | `agua.camara:valvulas` |
| `saneamiento.camara:medicion` | `agua.camara:medicion` |
| `saneamiento.camara:reparto` | `agua.camara:reparto` |
| `saneamiento.camara:alivio` | `agua.camara:alivio` |
| `agua.planta:cloracion` | `comun.caseta:cloracion` |
| `agua.planta:lecho_secado` | `comun.lecho_secado:arena_grava` |
| `agua.planta:espesador` | `saneamiento.sedimentador:espesador` |
| `agua.planta:laguna_estabilizacion` | `saneamiento.laguna:facultativa` |
| `agua.planta:ptar_lodos_activos` | `saneamiento.reactor:lodos_activos` (preferencia, no retiro) |
| `agua.planta:ptar_laguna` | `saneamiento.laguna:aireada` (preferencia, no retiro) |
| `agua.planta:ptar_uasb` | `saneamiento.reactor:uasb` (preferencia, no retiro) |
| `saneamiento.bombeo:estacion_impulsion` | `agua.bombeo:camara_seca` (si la red es AP) |

Las tres filas marcadas «preferencia» no retiran nada: `agua.planta:ptar_*` se conserva como planta completa esquemática, y el alias solo se aplica cuando el origen trae la **unidad** suelta. Los trece alias obligatorios son los trece primeros.

Mapeos de origen que cambian con estas familias:

| Origen | Antes | Ahora |
|---|---|---|
| EPANET `[PUMPS]` | `agua.tuberia:impulsion` con nota | nodo `agua.bombeo` + arcos `agua.tuberia:impulsion` |
| EPANET `[RESERVOIRS]` | `agua.tanque` | `agua.tanque` o `agua.captacion` si es fuente superficial |
| SWMM `[STORAGE]` | `saneamiento.camara:reunion` | `saneamiento.camara:reunion`, `saneamiento.laguna:*` o `saneamiento.reactor:*` según el uso declarado |
| DXF capa `REJA`, `REJILLA` | `comun.tapa:rejilla_barrotes` | `comun.reja:gruesos_manual` (la rejilla de tapa sigue siendo `comun.tapa`) |
| DXF capa `COMPUERTA` | `agua.valvula:compuerta` | `comun.compuerta:mural` |
| DXF capa `FILTRO` | `agua.planta:ptap_convencional` | `agua.filtro:rapido_gravedad` |
| DXF capa `DESARENADOR` | `saneamiento.camara:desarenadora` | `comun.desarenador:longitudinal` |

### 8.2 Checklist de implementación (lo que el registro ya verifica)

Antes de dar por buena una familia nueva, estas cinco pruebas del registro tienen que pasar sin tocarlas:

| Prueba (`families.rs`) | Qué exige | Cómo se cumple aquí |
|---|---|---|
| `claves_y_prefijos_unicos` | clave y prefijo únicos; toda familia declara subtipos | §2.2: 17 prefijos verificados contra los 29 actuales |
| `todo_subtipo_declara_atributos_existentes` | cada `required` de un `TypeDef` existe como `AttrDef` de su familia | las fichas de §3 listan los atributos de cada subtipo antes de declararlos |
| `los_defaults_respetan_su_tipo` | `Num` con `Value::Num`, `Int` con `Value::Int`, `Enum` con `Value::Text` | los ▣ son `Num` con default numérico o sin default |
| `el_catalogo_tiene_la_escala_del_consejo` | ≥ 4 subtipos por familia, etiquetas sin backtick y no vacías | ninguna familia nueva baja de 4 (`comun.lecho_secado` tiene 4) |
| `hay_familias_para_las_dos_redes` | ≥ 5 familias de agua y ≥ 6 de saneamiento | agua 19, saneamiento 15, común 12 |

Y tres comprobaciones que **no** existen hoy y convendría añadir:

1. Ninguna familia declara dos veces la misma clave de atributo (H3).
2. Ningún atributo hidráulico es `Access::Derived` (H14).
3. Toda familia con `STRUCTURAL` declara `EXCAVATABLE` o justifica por qué no (H13).

### 8.3 Conteo final

| Bloque | Familias | Subtipos |
|---|---:|---:|
| Registro actual | 29 | 216 |
| Nuevas familias (§3) | +17 | +95 |
| Subtipos nuevos en familias existentes (§4.1) | 0 | +17 |
| Subtipos retirados con alias (§4.2) | 0 | −13 |
| **Total propuesto** | **46** | **315** |

Comprobación: `29 + 17 = 46` familias y `216 + 95 + 17 − 13 = 315` subtipos.

Por dominio: agua 19 familias, saneamiento 15, común 12. Por prioridad: P1 = 14 familias nuevas (79 subtipos), P2 = 3 familias nuevas (16 subtipos).

---

*Este documento es el único artefacto nuevo de la tarea. No se modificó ningún archivo de `crates/` ni ningún otro de `docs/`.*
