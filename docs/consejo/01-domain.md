# 01 — Especificación de dominio

## Redes de agua potable y alcantarillado sanitario (Ecuador)

| Campo | Valor |
|---|---|
| Documento | `docs/council/01-domain.md` |
| Autor | `civil-domain` — consejo de arquitectura |
| Versión | 0.1 (borrador para decisión) |
| Fecha | 2026-11-18 |
| Estado | Propuesta técnica. Contiene 9 decisiones abiertas (§B) y 11 incertidumbres normativas declaradas (§6.5) |
| Audiencia | arquitectos de software, implementadores Rust, revisor hidráulico, cliente |
| Alcance | Dominio de ingeniería civil: objetos, atributos, hidráulica, normativa, excavación, topografía, validación |
| Fuera de alcance | Arquitectura de software, UI, persistencia, empaquetado (otros documentos del consejo) |

**Cómo leer este documento.** Las secciones 2–4 definen *qué existe* (taxonomía y atributos). La sección 5 define *cómo se calcula*. La 6 define *contra qué se valida*. La 7 y la 8 definen los dos insumos que el cliente pidió explícitamente: volumen de excavación y superficie topográfica. La 9 es la lista de reglas ejecutables que alimenta el backlog. La 11 traduce todo a decisiones de modelado en Rust y la 12 a features.

**Convención numérica.** En fórmulas y tablas se usa el **punto decimal** (compatibilidad con el código y con los tests). En prosa se usa coma cuando no hay ambigüedad. Los miles se separan con espacio fino o coma según contexto; ningún valor persistido lleva separador de miles.

**Contenido**

0. Resumen: diez decisiones estructurantes
1. Convenciones, unidades y glosario
2. Modelo conceptual (grafo, cuatro cotas, identidad)
3. Taxonomía de familias F01–F17
4. Diccionario de atributos y catálogos
5. Hidráulica (agua potable y alcantarillado)
6. Marco normativo y contrato del code pack
7. Excavación y movimiento de tierras
8. Topografía y superficie
9. Reglas de validación (68 reglas)
10. Fuera de alcance v1
11. Implicaciones para el modelo Rust
12. Backlog derivado (épicas E1–E7)
Anexos A–D: tablas, ADR pendientes, referencias, vectores de prueba

---

## 0. Resumen: las diez decisiones estructurantes

| # | Decisión | Consecuencia |
|---|---|---|
| D1 | El modelo canónico es un **grafo tipado nodo–arco**, no una colección de "dibujos". Las familias son un **catálogo de tipos** con comportamiento asociado. | Toda la hidráulica, la excavación y la validación se calculan sobre el grafo. Añadir una familia futura = añadir un tipo + sus reglas, sin tocar el motor. |
| D2 | Cada objeto tiene **cuatro cotas** distintas: terreno natural, rasante, clave (invert) y fondo de excavación. | Evita el error clásico de confundir "profundidad" con "cota". Habilita recubrimiento, excavación y perfiles correctos. |
| D3 | Una tubería física = **un arco entre dos nodos**. Las válvulas y medidores **parten** el arco. Existe una agrupación visual "tramo continuo" que no es una entidad de cálculo. | Coherencia entre topología y cálculo; sin arcos con elementos "colgando". |
| D4 | **"Llave" ≠ "válvula"**. Se propone que *válvula* = dispositivo en la red; *llave* = dispositivo en la frontera red–usuario (acometida, hidrante). Requiere confirmación del cliente (§3.5, ADR-01). | Si se confirma, dos familias con reglas distintas. Si no, se colapsan en una con atributo `rol`. |
| D5 | Los valores normativos **no se codifican**: viven en un *code pack* versionado y firmado (§6.3). | Cambiar de municipio o de edición de norma no requiere recompilar. Cada resultado cita la versión del pack usada. |
| D6 | La topografía es una **superficie versionada** y pueden coexistir varias capas (terreno, rasante, roca, nivel freático). | Los objetos referencian `(superficie_id, versión)`. Un cambio de superficie invalida volúmenes y se puede auditar. |
| D7 | La excavación es un **cálculo derivado**, nunca un dato editable, con caché por hash de insumos. | Recalcular es barato; nunca hay volumen "a mano" inconsistente con la geometría. |
| D8 | Dos motores hidráulicos separados: **presión** (Hazen-Williams / Darcy-Weisbach + Todini) y **gravedad** (Manning + flujo gradualmente variado + remanso). | Un solo motor para ambos es fuente segura de errores; la interfaz los unifica. |
| D9 | Unidades **SI internas** (m, s, m³/s, Pa, N); presentación en unidades de oficio (L/s, mm, m/km, %, mca). | Sin ambigüedad en fórmulas; la conversión ocurre solo en la frontera de UI/CLI. |
| D10 | La validación es un **conjunto de reglas declarativas** con id, severidad y trazabilidad a code pack. | Testeable, auditable, extensible; el informe de cumplimiento se genera de las reglas, no de la UI. |

---

## 1. Convenciones, unidades y glosario

### 1.1 Magnitudes y unidades

| Magnitud | Unidad interna | Unidad de presentación | Precisión de presentación |
|---|---|---|---|
| Longitud, cota, profundidad | m | m | 0.001 m (1 mm) |
| Coordenada plana (E, N) | m | m | 0.001 m |
| Diámetro nominal | — (entero, mm) | mm | entero |
| Diámetro exterior/interior | m | mm | 0.1 mm |
| Área | m² | m² | 0.01 |
| Volumen | m³ | m³ | 0.01 |
| Caudal | m³/s | L/s | 0.01 L/s |
| Velocidad | m/s | m/s | 0.01 |
| Pendiente | m/m | % o ‰ | 0.01 % |
| Rugosidad absoluta `k` | m | mm | 0.01 mm |
| Presión / carga | m (columna de agua) | mca | 0.01 |
| Presión (absoluta) | Pa | kPa o bar | 0.1 |
| Tensión tractriz | Pa | Pa | 0.01 |
| Ángulo, azimut | rad | grados sexagesimales | 1″ |
| Tiempo | s | s / min / h | 1 s |

**Reglas de conversión obligatorias en la frontera:** `1 mca = 9 806.65 Pa` (a 4 °C) — se usa `9 810 Pa` para consistencia con `γ = 9 810 N/m³` en toda la aplicación. `1 L/s = 0.001 m³/s`. `1 % = 0.01 m/m`. `1 ‰ = 0.001 m/m`.

**Constantes físicas adoptadas:** `g = 9.81 m/s²`; `ρ = 1 000 kg/m³`; `γ = 9 810 N/m³`; `ν = 1.004 × 10⁻⁶ m²/s` (agua a 20 °C) con variación por temperatura tabulada; `K_agua = 2.07 × 10⁹ Pa` (módulo de compresibilidad). Estas constantes son **parámetros del motor**, no literales dispersos.

### 1.2 Símbolos

| Símbolo | Significado | Unidad |
|---|---|---|
| `Q` | caudal | m³/s |
| `V` | velocidad media | m/s |
| `D` | diámetro interior | m |
| `D_n` / `DN` | diámetro nominal | mm |
| `L` | longitud | m |
| `S` | pendiente (sen θ ≈ tan θ para pendientes de redes) | m/m |
| `J` | pérdida unitaria | m/m |
| `h_f` | pérdida por fricción | m |
| `h_m` | pérdida local | m |
| `R` | radio hidráulico (`A/P`) | m |
| `n` | coeficiente de Manning | s/m^(1/3) |
| `C` | coeficiente de Hazen-Williams | — |
| `f` | factor de fricción de Darcy | — |
| `k` | rugosidad absoluta equivalente | m |
| `Re` | número de Reynolds | — |
| `y` | tirante de agua | m |
| `θ_c` | ángulo central del segmento circular mojado | rad |
| `τ` | tensión tractriz media | Pa |
| `a` | celeridad de onda de presión | m/s |
| `Z` | cota | m |
| `e` | espesor de pared | m |

### 1.3 Glosario español (Ecuador) → inglés técnico

| Término | Equivalente | Nota |
|---|---|---|
| Pozo (de inspección/revisión) | manhole | En Ecuador "pozo" también es pozo de agua; en este dominio, salvo subtipo explícito, es cámara de registro. |
| Cámara | chamber | Estructura visitable o no, con función específica. |
| Colector | collector / sewer | Conduce aguas residuales por gravedad. |
| Emisario | outfall / outfall sewer | Tramo final hasta el cuerpo receptor. |
| Interceptor | interceptor | Colector que capta varios colectores. |
| Vertedero | overflow / spillway / outfall structure | **Ambigüedad crítica**, ver §3.3. |
| Llave | valve / service stop / hydrant | **Ambigüedad crítica**, ver §3.5. |
| Acometida | service connection | Agua o alcantarillado. |
| Cota clave | invert elevation | Fondo interior de la tubería. |
| Cota de rasante | finished grade elevation | Superficie de acabado (calzada/vereda). |
| Cota de terreno | existing ground elevation | Levantamiento. |
| Plantilla | invert (fondo de pozo) | |
| Tirante | flow depth | |
| Brocal / marco y tapa | frame and cover | |
| Dado de anclaje | thrust block | |
| Entibado | shoring / trench box | |
| Cama de arena | bedding | |
| Relleno | backfill | |
| Esponjamiento | bulking factor | |
| Talud | side slope | |
| Berma | bench | |
| Cuneta | gutter | |
| Sumidero | gully / catch basin inlet | |
| Hito / BM | benchmark | |
| Captación | intake / source works | |
| Impulsión | force main | |
| Sifón invertido | inverted siphon | |
| Rebose | overflow | |

---

## 2. Modelo conceptual

### 2.1 Primitivas geométricas

| Primitiva | Uso | Requisitos |
|---|---|---|
| `Punto` | pozos, válvulas, medidores, hidrantes, sumideros, hitos | (E, N) obligatorio; Z derivada de superficie o explícita |
| `Polilínea 3D` | tuberías, canales, sifones, cruces, acometidas | Vértices en planta; Z por cota clave o por superficie; longitud 3D calculada |
| `Polígono` | tanques, plantas, áreas de captación | Anillo exterior; área y perímetro calculados; puede tener huecos |
| `Arco` | vínculo hidráulico entre dos nodos | Nodo inicio ≠ nodo fin; sentido aguas abajo declarado |
| `Superficie TIN` | terreno, rasante, roca, freático | Triángulos con orientación consistente; ver §8 |
| `Línea de rotura` | quiebres del terreno, bordes de vía, cauces | Restricción del TIN |

### 2.2 Grafo tipado nodo–arco

El grafo es la **única fuente de verdad topológica**. Reglas:

1. Todo arco referencia exactamente dos nodos existentes del mismo modelo y de la misma red (AP o AL). No hay arcos colgantes.
2. Un nodo puede pertenecer a una sola red. Los cruces entre AP y AL son objetos de familia *Cruce*, sin conectividad hidráulica.
3. Las válvulas, medidores y demás dispositivos **en línea** se modelan como un nodo de dispositivo que parte el arco en dos: `nodo_A — arco_1 — nodo_válvula — arco_2 — nodo_B`. La UI puede presentarlos como una sola tubería con un accesorio, pero el cálculo ve dos arcos.
4. El sentido de flujo en gravedad es **impuesto** por cotas (aguas abajo = cota clave menor). En presión es **calculado** y puede revertirse.
5. Los nodos tienen un `rol_hidraulico`: `fuente` (captación, tanque, PTAP), `consumo` (demanda), `transito` (unión), `descarga` (emisario, alivio), `dispositivo` (válvula, medidor), `almacenamiento` (tanque).
6. Toda componente conexa debe alcanzar al menos una `fuente` y, en AP, al menos un `consumo`; si no, es un error bloqueante (VAL-GEN-004).

### 2.3 Las cuatro cotas

Este es el punto donde más se equivocan las herramientas existentes. Definiciones obligatorias:

| Cota | Símbolo | Definición | Origen |
|---|---|---|---|
| Terreno natural | `Z_T` | Superficie existente antes de excavar | Levantamiento / TIN `terreno` |
| Rasante | `Z_R` | Superficie de acabado proyectada (calzada, vereda, terreno conformado) | TIN `rasante` o igual a `Z_T` si no hay proyecto vial |
| Clave | `Z_C` | Fondo **interior** de la tubería (invert) | Diseño hidráulico |
| Fondo de excavación | `Z_F` | Fondo de la zanja | Derivada: `Z_C − e − h_cama` |

Derivadas:

- Profundidad de zanja: `H = Z_T − Z_F`
- Recubrimiento (cover): `C_r = Z_R − (Z_C + D)` (a la clave exterior)
- Profundidad de pozo: `H_p = Z_T − Z_solera`
- Desnivel de pozo: `ΔZ = Z_C,entrada − Z_C,salida`

**Regla dura:** `Z_R` nunca se usa para calcular excavación; `Z_T` nunca se usa para verificar recubrimiento. Si el proyecto no tiene rasante, `Z_R = Z_T` explícitamente y el sistema lo marca como supuesto (no como dato).

### 2.4 Identidad, versionado y trazabilidad

- `id` interno: UUID v7. `codigo`: legible y único por modelo, con patrón por familia (`PZ-001`, `CL-014`, `VAL-003`).
- Toda entidad guarda: `creado_por`, `creado_en`, `modificado_en`, `revision`, `origen_dato` (`levantamiento`, `proyecto`, `importado`, `estimado`).
- Todo resultado de cálculo guarda: `hash_insumos`, `version_code_pack`, `version_superficie`, `motor` y `version_motor`.
- Ningún resultado se persiste como verdad: se persiste como *evidencia* con su hash de insumos.

---

## 3. Taxonomía de familias

### 3.0 Tabla maestra

| # | Familia | Rol topológico | Geometría | Red | Subtipos |
|---|---|---|---|---|---|
| F01 | Pozo | nodo | punto (+ sólido) | AL, AP (cámaras) | inspección, caída, resalto, ciego, bombeo, limpieza |
| F02 | Tubería | arco | polilínea 3D | AP, AL | conducción, aducción, distribución, colector, emisario, interceptor, impulsión, rebose/desagüe |
| F03 | Descarga / vertedero | nodo o arco corto | punto o línea | AL, AP | emisario final, alivio, rebose de tanque, disipador |
| F04 | Válvula | dispositivo de arco | punto | AP, AL (impulsión) | compuerta, mariposa, retención, aire, purga, reducción de presión, reguladora, alivio, flotador, pie |
| F05 | Llave | dispositivo de acometida | punto | AP, AL | corporación, banqueta, hidrante, tapón terminal |
| F06 | Tanque / reservorio | nodo con volumen | punto + polígono | AP | apoyado, elevado, enterrado, cisterna, contacto, regulación |
| F07 | Planta | nodo de proceso | polígono | AP, AL | PTAP, PTAR, laguna, desalinizadora, espesador |
| F08 | Medidor | dispositivo de arco | punto | AP | domiciliario, macromedidor, electromagnético, ultrasónico |
| F09 | Cámara | nodo | punto o polígono | AP, AL | desarenadora, reunión, carga, quiebre, válvulas, medición |
| F10 | Sifón | arco especial | polilínea 3D | AL, AP | invertido, normal, de cruce |
| F11 | Cruce | arco corto / atributo | punto o línea | AP, AL | río, vía, otra red, aéreo, subfluvial |
| F12 | Acometida | arco terminal | polilínea | AP, AL | agua potable, alcantarillado, pluvial |
| F13 | Sumidero | nodo de captación | punto | AL (pluvial) | calzada, rejilla, cuneta, arenero |
| F14 | Canal | arco abierto | polilínea + sección | AP, AL | rectangular, trapezoidal, triangular, circular, cuneta, coronación |
| F15 | Captación | nodo fuente | punto o polígono | AP | manantial, pozo profundo, galería filtrante, toma de río |
| F16 | Estación de bombeo | nodo compuesto | punto o polígono | AP, AL | cámara húmeda + grupo de bombas + accesorios |
| F17 | Elementos de apoyo | sin rol hidráulico | punto, línea, polígono | — | hitos, BM, sondeos, tapones, señalización, áreas de obra |

**Nota de diseño:** F16 se modela como **nodo compuesto** que agrupa una F01 (cámara de bombeo) y una o más bombas. No se duplica la cámara: la estación es una vista sobre la cámara más el equipamiento.

---

### 3.1 F01 — Pozos

**Propósito.** Permitir acceso, ventilación, inspección y mantenimiento; cambiar dirección, diámetro o pendiente; recibir aportes; disipar energía.

**Representación.** Nodo puntual. Sólido de referencia: cilindro (interior) o prisma rectangular, con brocal, cono de reducción, cuerpo y solera. La excavación se calcula como prisma independiente (§7.6).

**Subtipos**

| Subtipo | Cuándo se usa | Atributos distintivos |
|---|---|---|
| Pozo de inspección | Cambio de dirección, pendiente, diámetro o unión de colectores | `diametro_interior`, `numero_entradas`, `numero_salidas` |
| Pozo de caída | Desnivel que no se puede resolver con pendiente (típicamente > 0.50 m) | `altura_caida`, `tipo_caida` (exterior/interior), `diametro_caida` |
| Pozo de resalto | Desnivel pequeño (≤ 0.50 m) resuelto con resalto en la solera | `altura_resalto`, `longitud_resalto` |
| Pozo ciego | Sin salida; infiltración al terreno (rural, sin red) | `area_infiltracion`, `tasa_infiltracion`, `nivel_freatico` |
| Cámara de bombeo | Recibe y bombea; volumen de retención | `volumen_util`, `niveles`, `bombas[]` |
| Pozo de limpieza / caja de revisión | Acometidas y tramos cortos | `diametro`, `tipo_tapa` |
| Pozo de ventilación | Tramos largos sin acceso | `area_ventilacion` |

**Conectividad**

- AL: ≥ 1 entrada y ≥ 1 salida (excepto ciego y de limpieza, que admiten 1/0). Las entradas pueden ser varias; se permite entrada a distinta cota con caída.
- AP: solo en función de cámara de válvulas o de medición; no es nodo de consumo.
- Un pozo **no** puede conectar dos redes distintas. El cruce AP/AL se modela como F11.

**Reglas de vecindad**

- Distancia máxima entre pozos: parámetro (`dist_max_pozos`, típico 100–120 m; en Ecuador es frecuente 100 m). No es regla hidráulica sino de mantenimiento.
- Todo cambio de dirección > 45°, cambio de diámetro o de pendiente exige pozo.
- Un pozo no puede recibir más de `n_max_conexiones` (parámetro, típico 4–6) sin justificación de diseño.

---

### 3.2 F02 — Tuberías

**Propósito.** Conducir agua (AP) o agua residual (AL) entre nodos.

**Representación.** Arco con polilínea 3D. La cota clave se define en los extremos; entre ellos, la tubería es recta en perfil (pendiente constante) salvo que se declare un vértice hidráulico intermedio, en cuyo caso se parte el arco.

**Subtipos**

| Subtipo | Red | Función | Régimen | Notas |
|---|---|---|---|---|
| Conducción | AP | Fuente → planta / tanque | Presión o lámina libre | Puede ser canal o tubería |
| Aducción | AP | Tanque → red (o planta → tanque) | Presión | Caudal de diseño = QMD |
| Red de distribución | AP | Malla urbana | Presión | Primaria (≥ 100 mm) y secundaria |
| Impulsión | AP, AL | Bombeo → descarga | Presión | Requiere válvula de retención y aire |
| Colector | AL | Recoge y conduce aguas residuales | Lámina libre | DN mín. 200 mm típico |
| Interceptor | AL | Recibe varios colectores | Lámina libre | DN ≥ 300–400 mm |
| Emisario | AL | Último tramo hasta el receptor | Lámina libre o presión | Puede terminar en F03 |
| Rebose / desagüe | AP, AL | Evacuar excedentes o vaciar un tanque | Lámina libre | Descarga libre; no es conducción de servicio |
| Ramal / subcolector | AL | Colector menor | Lámina libre | DN 150–200 mm |

**Conectividad**

- AP: nodo–nodo; admite mallas (ciclos). Un arco no puede tener más de un diámetro ni un cambio de material.
- AL: nodo–nodo con sentido aguas abajo obligatorio. En AL **no se admiten ciclos** (VAL-AL-008).
- Una tubería no puede terminar en el aire salvo que sea rebose o descarga declarada.

**Atributos de cálculo obligatorios:** `diametro_nominal`, `diametro_interior`, `material`, `rugosidad_n` o `coef_C`, `longitud`, `pendiente` o `cotas extremas`, `clase_presion`.

---

### 3.3 F03 — Descargas y vertederos

**Propósito.** Entregar el agua al cuerpo receptor de forma controlada y disipada.

**Ambigüedad declarada (ADR-02).** En español técnico "vertedero" designa al menos cuatro cosas distintas:

1. **Estructura de entrega / emisario final** (outfall): el punto donde el sistema descarga a un río, quebrada, mar o canal.
2. **Alivio / rebose** (overflow, CSO en sistemas combinados): estructura que descarga excedentes cuando el nivel supera una cota.
3. **Vertedero de excedencias de un tanque o presa** (spillway): lámina vertiente con fórmula de cresta.
4. **Relleno sanitario / botadero** (landfill): uso no hidráulico, fuera de alcance.

La propuesta es una sola familia F03 con subtipo explícito y una relación `descarga_a` hacia un objeto *Cuerpo receptor* (entidad externa con nombre, tipo, cota de nivel de agua y uso). El consejo debe confirmar que el cliente se refería a (1) y (2), no a (4).

**Subtipos**

| Subtipo | Función | Atributos |
|---|---|---|
| Emisario final | Entrega del caudal tratado | `cota_descarga`, `nivel_receptor`, `tipo_difusor`, `longitud_subacuatica` |
| Alivio / rebose lateral | Descarga de excedentes | `cota_cresta`, `longitud_vertedero`, `caudal_maximo`, `frecuencia_alivio` |
| Rebose de tanque | Control de nivel máximo | `cota_NAMO`, `longitud_vertedero`, `capacidad` |
| Disipador / estructura de entrega | Disipar energía | `tipo` (salto, tanque, enrocado), `dimensiones` |
| Pozo de infiltración | Infiltración al terreno | `area`, `tasa` |

**Conectividad.** Entrada obligatoria (1). Salida: solo hacia *Cuerpo receptor* (externo) o hacia otra F03 en cascada. Nunca hacia un nodo de la red.

**Hidráulica asociada.** La capacidad de un vertedero de cresta se calcula con la fórmula de vertedero (§5.2.10): `Q = C_w · L_e · H^(3/2)`, con `C_w ≈ 1.84` para cresta delgada (Francis, SI) y `≈ 1.70` para cresta ancha. Estos coeficientes son parámetros del code pack.

---

### 3.4 F04 — Válvulas

**Propósito.** Aislar, controlar, proteger o aliviar. Es un **dispositivo de arco**: parte la tubería y añade una pérdida local y, en algunos casos, una condición de borde (presión fija aguas abajo).

**Subtipos**

| Subtipo | Función | Condición de borde | `K` típico (abierta) |
|---|---|---|---|
| Compuerta | Aislamiento todo/nada | Ninguna (abierta) / cierre | 0.2 |
| Mariposa | Aislamiento y control | Ninguna / pérdida | 0.3–0.6 |
| Retención (check) | Impide reverso | Flujo unidireccional | 2.5–3.5 |
| Aire (admisión/expulsión) | Evita cavitación y transitorios | Ninguna en régimen | — |
| Purga / desagüe | Vaciar tramos bajos | Descarga libre | 1.0–2.0 |
| Reductora de presión (PRV) | Mantiene presión aguas abajo | `H_salida = H_set` | — |
| Sostenedora / reguladora | Mantiene presión aguas arriba | `H_entrada ≥ H_set` | — |
| Alivio / seguridad | Protege contra sobrepresión | Abre si `H > H_set` | — |
| Flotador | Control de nivel en tanque | `H = H_NAMO` | — |
| Pie (foot valve) | Retención en succión | Unidireccional | 2.0–3.0 |

**Conectividad.** Siempre 1 entrada y 1 salida en la misma red. Una válvula no puede quedar en el extremo de un arco sin nodo.

**Atributos clave:** `diametro_nominal`, `clase_presion`, `tipo_accionamiento` (manual, eléctrico, hidráulico, neumático), `posicion` (abierta/cerrada/parcial), `ajuste` (presión o caudal de consigna), `perdida_local_K`, `sentido_permitido`, `anio`, `estado`.

**Regla de modelado:** toda válvula de cierre se modela como abierta para el cálculo de demanda máxima, salvo que el diseñador declare un escenario de operación (válvula cerrada) — soportado por *escenarios* (F-15).

---

### 3.5 F05 — Llaves (ambigüedad que el consejo debe resolver)

**Problema.** El cliente pidió "familia de válvulas" **y** "familia de llaves" por separado. En el español de Ecuador "llave" cubre al menos cuatro referentes:

| Interpretación | Qué es | ¿Pertenece a este dominio? |
|---|---|---|
| (a) Llave de paso / llave de maniobra | Válvula de corte, generalmente domiciliaria | Sí, pero es una válvula → duplicaría F04 |
| (b) Llave de corporación / banqueta | Elementos de la acometida: collarín, llave de corporación, medidor, llave de banqueta | Sí; es la **frontera red–usuario** |
| (c) Llave / grifo | Artefacto sanitario (grifería de lavabo, ducha, inodoro) | No; es instalación interior, no red |
| (d) Llave de incendio | Hidrante | Sí; es un dispositivo de red con demanda específica |

**Propuesta (ADR-01).** Definir:

> **Válvula (F04)** = dispositivo instalado **dentro** de la red, cuyo fallo afecta la operación del sistema y que aparece en el cálculo hidráulico como elemento en línea.
>
> **Llave (F05)** = dispositivo instalado en la **frontera entre la red y el usuario** (acometida) o de **servicio contra incendio**, que no forma parte de la malla de distribución y se calcula como demanda o como punto de conexión.

**Subtipos propuestos para F05**

| Subtipo | Descripción | Modelo hidráulico |
|---|---|---|
| Llave de corporación | Collarín + llave sobre la tubería de la red | No es elemento de malla; marca el nodo de acometida |
| Llave de banqueta | Corte antes del medidor, en el límite del predio | Ídem |
| Hidrante | Boca de incendio (DN 65–100) | Nodo de demanda con caudal de incendio |
| Tapón terminal | Cierre de un ramal sin uso | Nodo sin salida; no aporta caudal |
| Válvula de acometida con medidor | Conjunto integrado | Se descompone en F05 + F08 |

**Si el cliente confirma que "llave" = grifería**, la familia sale del alcance del diseño de redes y se declara fuera de v1 (§10), porque pertenece al proyecto de instalaciones interiores del edificio.

**Conectividad.** F05 siempre en el extremo de una F12 (acometida) o sobre ella. Nunca entre dos nodos de la malla.

---

### 3.6 F06 — Tanques y reservorios

**Propósito.** Almacenar, regular presión y garantizar reserva (incluida la de incendio).

**Representación.** Nodo con volumen; polígono de huella; **curva área–cota** o geometría paramétrica (cilíndrico, rectangular) para calcular volumen entre niveles.

**Subtipos:** apoyado, elevado (torre), enterrado, cisterna, tanque de contacto (cloración), tanque de regulación, tanque de almacenamiento de agua cruda.

**Atributos clave:** `volumen_util` (m³), `cota_fondo`, `cota_NAMO` (nivel máximo de operación), `cota_NAMI` (nivel mínimo de operación), `cota_NAE` (nivel de alarma/emergencia), `borde_libre`, `diametro` o `dimensiones`, `material`, `tipo_entrada` (flotador/compuerta), `cota_rebose`, `cota_desague`, `tiempo_retencion`, `tasa_renovacion`.

**Cálculo de volumen de regulación (orientativo, parametrizable):** `V_reg = k_reg · Q_MD · 86 400/1000`, con `k_reg` típico 0.20–0.30 (fracción del caudal máximo diario). El método exacto (curva de demanda vs. curva de entrada) es v1.1; en v1 se acepta `k_reg` del code pack.

**Conectividad.** Entradas: 1 o más (aducción, impulsión). Salidas: 1 o más (red). Un tanque puede ser fuente y destino a la vez en distintas horas; para el cálculo de demanda máxima es **fuente de carga fija** `H = Z_NAMI` (caso crítico) o `Z_NAMO` (caso de presión máxima). Ambos escenarios son obligatorios (VAL-AP-011).

---

### 3.7 F07 — Plantas de tratamiento

**Propósito.** Representar la instalación como nodo de proceso con capacidad y calidad de salida. **No** se diseña el tren de tratamiento en v1.

**Subtipos:** PTAP (potabilizadora), PTAR (aguas residuales), laguna de estabilización, desalinizadora, espesador/lecho de secado.

**Atributos clave:** `caudal_diseno` (L/s), `caudal_punta`, `tren_proceso` (texto tipificado), `calidad_entrada` y `calidad_salida` (turbiedad, DBO5, DQO, SST, coliformes), `eficiencia_remocion`, `cota_hidraulica`, `area`, `punto_descarga` (→ F03), `generacion_lodos`.

**Conectividad.** Entrada y salida obligatorias; puede tener rebose de emergencia (→ F03) y retorno de lodos.

---

### 3.8 F08 — Medidores y macromedidores

**Propósito.** Medir caudal: facturación (domiciliario) y control de pérdidas (macromedición por sector/DMA).

**Subtipos:** domiciliario (ISO 4064, DN 15–25), macromedidor (Woltman/turbina, DN 50–300), electromagnético, ultrasónico, medidor de inserción.

**Atributos clave:** `diametro_nominal`, `Q_nominal`, `Q_max`, `Q_min`, `clase_metrologica`, `perdida_carga_Qnom` (m), `tipo_lectura` (local, remota, pulsos), `fecha_instalacion`, `ultima_lectura`, `numero_serie`.

**Hidráulica.** Se modela como pérdida local `h_m = K_m · V²/(2g)` o, si el fabricante da curva, `h_m = a·Q^b`. Para macromedidores en conducciones de gravedad, la pérdida puede ser significativa y **no debe ignorarse**.

**Conectividad.** En línea, sobre F12 (domiciliario) o sobre F02/F16 (macromedidor). Nunca en paralelo sin derivación (by-pass), que debe modelarse como arco alternativo si existe.

---

### 3.9 F09 — Cámaras

**Propósito.** Estructuras visitables o no, con función específica distinta de la inspección de alcantarillado.

| Subtipo | Función | Atributos distintivos |
|---|---|---|
| Cámara desarenadora | Retener arena y sólidos pesados | `longitud`, `ancho`, `tirante`, `velocidad_diseno`, `volumen_arenas` |
| Cámara de reunión | Unir varios caudales antes de una planta o bombeo | `numero_aportes`, `cota_solera` |
| Cámara de carga | Regular entrada a conducción a presión | `cota_agua`, `vertedero_excedencias` |
| Cámara de quiebre de presión | Romper presión en conducciones largas | `cota_libre`, `tipo_quiebre` |
| Cámara de válvulas | Alojar válvulas de gran diámetro | `dimensiones`, `acceso`, `drenaje` |
| Cámara de medición | Alojar macromedidor | `diametro_tuberia`, `by_pass` |
| Cámara de inspección a presión | Registro en impulsiones | `diametro`, `tapa` |

**Representación.** Punto con dimensiones, o polígono si es grande. Se diferencia de F01 por función: si la función principal es inspeccionar un colector, es F01.

---

### 3.10 F10 — Sifones

**Propósito.** Cruzar un obstáculo (río, quebrada, vía, otra conducción) pasando por debajo, manteniendo el flujo.

**Subtipos y diferencias**

| Subtipo | Funcionamiento | Criterio |
|---|---|---|
| Sifón invertido | Flujo a presión, sin aire, entre dos cámaras | AL y AP; velocidad ≥ 0.90 m/s para arrastre |
| Sifón normal | Flujo en lámina libre con depresión | Poco usado en redes; común en instalaciones |
| Sifón de cruce | Caso particular de cruce con depresión | Se modela como F10 + F11 |

**Atributos clave:** `diametro_nominal`, `numero_conductos` (2 para redundancia), `longitud`, `material`, `cota_entrada`, `cota_salida`, `cota_punto_bajo`, `perdida_total`, `velocidad_minima`, `dispositivo_purga`.

**Hidráulica.** Se calcula a presión (Darcy-Weisbach o Hazen-Williams) con suma de pérdidas locales de entrada, codos y salida. Dos verificaciones obligatorias:

1. **Carga disponible:** `ΔH = H_agua,entrada − H_agua,salida ≥ 1.10·(h_f + h_m)`. Si no se cumple, el sifón no transporta el caudal de diseño y hay que subir el nivel de entrada o aumentar el diámetro.
2. **Presión positiva en el punto bajo:** la línea de energía en el punto bajo debe quedar por encima de la clave del tubo más un margen: `HGL_bajo ≥ Z_C,bajo + D + 0.30 m`, con `HGL_bajo = HGL_entrada − h_f(entrada→punto bajo)`. Si no se cumple, se forman bolsas de aire y hay riesgo de vacío; se corrige con válvula de admisión/expulsión de aire o cambiando el trazado.

**Conectividad.** 2 nodos, misma red. No admite derivaciones.

---

### 3.11 F11 — Cruces

**Propósito.** Registrar y cuantificar interferencias y obras especiales.

**Subtipos:** cruce de río/quebrada, cruce de vía (ferroviaria o carretera), cruce con otra red (AP, AL, eléctrica, telecom, gas), cruce aéreo (puente, pasarela), cruce subfluvial.

**Atributos clave:** `tipo`, `obstaculo` (referencia), `longitud_cruce`, `angulo_cruce`, `profundidad_minima`, `proteccion` (dado de hormigón, encamisado, camisa de acero), `separacion_minima_otras_redes`, `permiso_requerido`, `entidad_autorizante`.

**Modelo.** Es un **atributo de arco** más un objeto de registro: el cruce no cambia la topología, pero sí el costo y las restricciones de excavación. Se materializa como objeto F11 asociado a un `arco_id` y a una distancia de inicio sobre el arco.

**Criterio típico (parametrizable):** separación vertical mínima entre alcantarillado y agua potable `≥ 0.30 m` libre, y horizontal `≥ 1.00 m`; si no se cumple, encamisado. Los valores exactos dependen de la norma local y del prestador: **no se fijan aquí**.

---

### 3.12 F12 — Acometidas domiciliarias

**Propósito.** Conectar el predio a la red.

| Subtipo | Recorrido | DN típico | Pendiente típica |
|---|---|---|---|
| Agua potable | Red → llave de corporación → medidor → predio | 13–25 mm (½"–1") | N/A (presión) |
| Alcantarillado | Predio → caja de revisión → colector | 100–150 mm | ≥ 1 % (recomendado 2 %) |
| Pluvial | Predio/patio → sumidero o colector | 100–150 mm | ≥ 0.5 % |

**Atributos clave:** `tipo`, `diametro_nominal`, `material`, `longitud`, `pendiente`, `cota_inicio`, `cota_fin`, `caja_revision`, `medidor_id`, `numero_predio`, `propietario`, `estado`.

**Conectividad.** Agua: nodo de red → punto de predio (entidad externa `Predio`). Alcantarillado: punto de predio → nodo de red (pozo o colector). La acometida **no** forma parte de la malla de distribución; se calcula como demanda puntual.

**Modelo de datos:** `Predio` es una entidad externa mínima (`id`, `clave_catastral`, `direccion`, `uso`, `numero_habitantes`, `area_construida`), porque sin ella no se puede repartir la demanda ni facturar.

---

### 3.13 F13 — Sumideros

**Propósito.** Captar agua de lluvia en calzada y entregarla a la red pluvial.

**Subtipos:** sumidero de calzada (boca de lobo), sumidero de rejilla, sumidero de cuneta, arenero.

**Atributos clave:** `tipo`, `longitud_util`, `ancho`, `rejilla` (tipo y clase de carga), `cota_rasante`, `cota_fondo`, `diametro_conexion`, `area_drenada`, `coeficiente_escurrimiento`.

**Hidráulica.** Capacidad de captación por fórmula de orificio/vertedero según geometría; en v1 se permite `Q_captado = min(Q_aportado, Q_capacidad)` con `Q_capacidad` tabulado por tipo. El diseño pluvial completo está fuera de v1 (§10), pero el objeto sí existe para no bloquearlo después.

**Conectividad.** 1 salida hacia F01 o F02 pluvial; 0 entradas (recibe por superficie).

---

### 3.14 F14 — Canales

**Propósito.** Conducir a superficie libre: cunetas, canales de coronación, canales de descarga, canales de planta.

**Subtipos:** rectangular, trapezoidal, triangular, circular (como canal), cuneta, canal de coronación, canal de descarga.

**Atributos clave:** `forma`, `base` (m), `talud_z` (H:V), `tirante_normal`, `tirante_critico`, `rugosidad_n`, `pendiente`, `longitud`, `revestimiento` (tierra, hormigón, mampostería, geosintético), `borde_libre`.

**Hidráulica.** Manning con `A` y `P` según sección (§5.2). El cálculo de tirante normal requiere resolver la ecuación implícita; el de tirante crítico, `Q²·T/(g·A³) = 1`.

**Conectividad.** Nodo–nodo; puede terminar en F03 (descarga) o en F01 (entrega a colector). No admite presión.

---

### 3.15 F15 — Captación

**Propósito.** Nodo fuente del sistema de agua potable.

| Subtipo | Atributos distintivos |
|---|---|
| Manantial | `caudal_aforado`, `caudal_minimo`, `tipo_captacion` (ladera, fondo), `cota` |
| Pozo profundo | `profundidad`, `nivel_estatico`, `nivel_dinamico`, `caudal_especifico`, `diametro_entubado` |
| Galería filtrante | `longitud`, `diametro`, `caudal_unitario` |
| Toma de río | `caudal_ecologico`, `periodo_retorno`, `obra_toma`, `desarenador` |

**Atributos comunes:** `caudal_disponible` (L/s), `caudal_concesionado` (L/s), `cota`, `calidad` (turbiedad, color, pH, coliformes), `fuente` (nombre del cuerpo de agua), `permiso_aprovechamiento`.

**Conectividad.** Salida obligatoria hacia F02 o F07. Es el nodo de carga fija o de caudal fijo del modelo.

**Nota normativa:** el caudal ecológico y el caudal concesionado son restricciones legales, no hidráulicas; el motor debe poder imponer `Q ≤ Q_concesionado` como cota superior.

---

### 3.16 F16 — Estación de bombeo

**Propósito.** Elevar el agua cuando la gravedad no alcanza.

**Composición:** cámara húmeda (F01 subtipo bombeo) + `n` bombas + válvulas de retención y compuerta + medidor + tablero.

**Atributos clave:** `caudal_bombeo` (L/s), `altura_manometrica` (m), `numero_bombas` (n+1 redundancia típica), `potencia_unitaria` (kW), `eficiencia`, `curva_bomba` (H–Q, η–Q, NPSHr), `niveles` (arranque, parada, alarma, rebose), `volumen_util`, `tiempo_retencion`, `tipo_control` (nivel, caudal, presión).

**Hidráulica.** `H_b = H_estática + h_f,impulsión + h_m`; punto de operación en la intersección de la curva de bomba con la curva del sistema. Potencia: `P = ρ·g·Q·H/(1000·η)` (kW). NPSH disponible: `NPSH_d = (P_atm − P_v)/(ρg) + Z_succión − h_f,succión`.

**Conectividad.** Entrada (gravedad) y salida (impulsión a presión). No puede bombear directamente a una red de distribución sin tanque, salvo diseño justificado.

---

### 3.17 F17 — Elementos de apoyo

**Propósito.** Sostener el levantamiento, la construcción y la documentación sin intervenir en la hidráulica.

Subtipos: hito/BMD, punto de sondeo (SPT, calicata), punto de aforo, tapón, señalización, área de instalación de faena, límite de predio, servidumbre.

**Regla:** estos objetos nunca tienen conectividad hidráulica; si el usuario los conecta, es un error de tipo (el compilador lo impide, §11).

---

## 4. Diccionario de atributos

### 4.1 Atributos comunes a toda entidad

| Atributo (`id`) | Tipo | Unidad | Rango | Oblig. | Notas |
|---|---|---|---|---|---|
| `id` | UUID v7 | — | — | sí | Inmutable |
| `codigo` | texto | — | patrón por familia | sí | Único por modelo |
| `familia` | enum F01…F17 | — | — | sí | Discriminante |
| `subtipo` | enum por familia | — | — | sí | |
| `red` | enum {AP, AL, PLUVIAL} | — | — | sí | |
| `nombre` | texto | — | ≤ 120 car. | no | Etiqueta de usuario |
| `geometria` | punto/polilínea/polígono | m | — | sí | En CRS del modelo |
| `superficie_id` | ref | — | — | sí (si Z derivada) | §8.6 |
| `z_terreno` | decimal | m | −10 … 5 000 | cond. | Derivada o explícita |
| `z_rasante` | decimal | m | −10 … 5 000 | cond. | Si no hay rasante, = `z_terreno` |
| `propietario` | enum/texto | — | — | sí | Prestador, municipio, comuna, privado |
| `operador` | texto | — | — | no | Quién opera |
| `estado` | enum | — | — | sí | §4.3 |
| `anio_instalacion` | entero | año | 1900 … año+5 | no | |
| `vida_util` | entero | años | 10 … 100 | no | |
| `material` | enum | — | — | cond. | §4.3 |
| `origen_dato` | enum | — | — | sí | levantamiento / proyecto / importado / estimado |
| `fecha_levantamiento` | fecha | — | — | cond. | |
| `exactitud_xy` | decimal | m | 0.001 … 10 | no | |
| `exactitud_z` | decimal | m | 0.001 … 1 | no | |
| `observaciones` | texto | — | — | no | |
| `revision` | entero | — | ≥ 1 | sí | |
| `creado_en` / `modificado_en` | timestamp | — | — | sí | UTC |

### 4.2 Atributos por familia

#### F01 Pozo

| Atributo | Tipo | Unidad | Rango típico | Oblig. |
|---|---|---|---|---|
| `diametro_interior` | decimal | m | 0.90 / 1.20 / 1.50 | sí |
| `tipo_seccion` | enum {circular, rectangular} | — | — | sí |
| `material` | enum {hormigon, ladrillo, PVC, HDPE, GRP} | — | — | sí |
| `espesor_pared` | decimal | m | 0.10 … 0.30 | sí |
| `cota_solera` | decimal | m | — | sí |
| `cota_terreno` | decimal | m | — | sí |
| `profundidad` | decimal | m | 0.60 … 12.0 | derivada |
| `numero_entradas` | entero | — | 0 … 6 | sí |
| `numero_salidas` | entero | — | 0 … 4 | sí |
| `altura_caida` | decimal | m | 0 … 5.0 | cond. (subtipo caída) |
| `tipo_caida` | enum {exterior, interior, resalto} | — | — | cond. |
| `tipo_tapa` | enum {peatonal, vehicular_ligera, vehicular_pesada} | — | — | sí |
| `clase_tapa` | enum {A15, B125, C250, D400, E600, F900} | — | — | sí |
| `peldaños` | booleano | — | — | no |
| `ventilacion` | booleano | — | — | no |
| `volumen_util` | decimal | m³ | — | cond. (bombeo) |
| `nivel_arranque` / `nivel_parada` / `nivel_alarma` | decimal | m | — | cond. (bombeo) |

#### F02 Tubería

| Atributo | Tipo | Unidad | Rango típico | Oblig. |
|---|---|---|---|---|
| `diametro_nominal` | entero | mm | 25 … 2 000 | sí |
| `diametro_exterior` | decimal | m | — | sí |
| `diametro_interior` | decimal | m | — | sí |
| `espesor_pared` | decimal | m | — | sí |
| `material` | enum | — | — | sí |
| `rugosidad_n` | decimal | s/m^(1/3) | 0.009 … 0.015 | cond. (AL) |
| `coef_C` | entero | — | 100 … 150 | cond. (AP) |
| `rugosidad_k` | decimal | m | 0.000 001 5 … 0.003 | cond. (DW) |
| `longitud` | decimal | m | 0.5 … 5 000 | derivada |
| `pendiente` | decimal | m/m | 0.0005 … 0.20 | cond. (AL) |
| `cota_clave_inicio` / `cota_clave_fin` | decimal | m | — | sí |
| `clase_presion` | enum {PN6, PN8, PN10, PN12.5, PN16, PN20, RDE21, RDE26, RDE32.5, RDE41} | — | — | cond. (AP) |
| `presion_trabajo` | decimal | mca | 0 … 160 | cond. (AP) |
| `tipo_junta` | enum {espiga_campana, bridada, soldada, electrofusion, mecánica} | — | — | sí |
| `tipo_soporte` | enum {cama_arena, cama_grava, dado_hormigon, colgado} | — | — | no |
| `recubrimiento` | decimal | m | 0.60 … 3.0 | derivada |
| `anio` | entero | año | — | no |
| `numero_tramos` | entero | — | — | no |

#### F03 Descarga / vertedero

| Atributo | Tipo | Unidad | Rango típico | Oblig. |
|---|---|---|---|---|
| `tipo_descarga` | enum | — | — | sí |
| `cota_cresta` | decimal | m | — | cond. |
| `longitud_vertedero` | decimal | m | 0.3 … 20 | cond. |
| `coef_vertedero` | decimal | — | 1.70 … 1.84 | cond. |
| `cota_descarga` | decimal | m | — | sí |
| `nivel_receptor` | decimal | m | — | sí |
| `cuerpo_receptor` | ref externa | — | — | sí |
| `caudal_maximo` | decimal | m³/s | — | cond. |
| `frecuencia_alivio` | decimal | veces/año | 0 … 50 | no |
| `tipo_difusor` | enum {ninguno, simple, multiplo} | — | — | no |
| `longitud_subacuatica` | decimal | m | 0 … 500 | no |

#### F04 Válvula

| Atributo | Tipo | Unidad | Rango típico | Oblig. |
|---|---|---|---|---|
| `diametro_nominal` | entero | mm | 25 … 1 200 | sí |
| `tipo_valvula` | enum | — | — | sí |
| `clase_presion` | enum | — | — | sí |
| `tipo_accionamiento` | enum {manual, electrico, hidraulico, neumatico} | — | — | sí |
| `posicion` | enum {abierta, cerrada, parcial} | — | — | sí |
| `apertura` | decimal | % | 0 … 100 | cond. |
| `perdida_K` | decimal | — | 0 … 10 | sí |
| `consigna_presion` | decimal | mca | 5 … 100 | cond. (PRV/PSV) |
| `consigna_caudal` | decimal | L/s | — | cond. |
| `sentido_unico` | booleano | — | — | sí (check) |
| `anio` | entero | año | — | no |
| `estado` | enum | — | — | sí |

#### F05 Llave / hidrante

| Atributo | Tipo | Unidad | Rango típico | Oblig. |
|---|---|---|---|---|
| `tipo_llave` | enum | — | — | sí |
| `diametro_nominal` | entero | mm | 13 … 100 | sí |
| `caudal_incendio` | decimal | L/s | 5 … 30 | cond. (hidrante) |
| `presion_minima_incendio` | decimal | mca | 10 … 20 | cond. |
| `tiempo_autonomia` | entero | min | 60 … 180 | cond. |
| `radio_cobertura` | decimal | m | 50 … 150 | cond. (hidrante) |
| `bocas` | entero | — | 1 … 3 | cond. |
| `tipo_tapa` | enum | — | — | no |
| `ubicacion` | enum {vereda, calzada, predio} | — | — | sí |

#### F06 Tanque

| Atributo | Tipo | Unidad | Rango típico | Oblig. |
|---|---|---|---|---|
| `tipo_tanque` | enum | — | — | sí |
| `volumen_util` | decimal | m³ | 5 … 50 000 | sí |
| `volumen_total` | decimal | m³ | — | sí |
| `cota_fondo` | decimal | m | — | sí |
| `cota_NAMO` / `cota_NAMI` / `cota_NAE` | decimal | m | — | sí |
| `borde_libre` | decimal | m | 0.30 … 1.00 | sí |
| `diametro` o `largo`/`ancho` | decimal | m | — | sí |
| `material` | enum | — | — | sí |
| `tipo_entrada` | enum {flotador, compuerta, vertedero} | — | — | sí |
| `cota_rebose` | decimal | m | — | sí |
| `cota_desague` | decimal | m | — | no |
| `sistema_cloracion` | enum {ninguno, hipoclorito, gas, UV} | — | — | no |
| `tiempo_retencion` | decimal | h | 2 … 48 | derivada |

#### F07 Planta

| Atributo | Tipo | Unidad | Rango típico | Oblig. |
|---|---|---|---|---|
| `tipo_planta` | enum | — | — | sí |
| `caudal_diseno` | decimal | L/s | 1 … 5 000 | sí |
| `caudal_punta` | decimal | L/s | — | sí |
| `tren_proceso` | texto tipificado | — | — | sí |
| `dbo5_entrada` / `dbo5_salida` | decimal | mg/L | 100 … 600 / 10 … 80 | cond. (PTAR) |
| `sst_salida` | decimal | mg/L | 10 … 100 | cond. |
| `turbiedad_salida` | decimal | UNT | 0.5 … 5 | cond. (PTAP) |
| `eficiencia_remocion` | decimal | % | 40 … 99 | derivada |
| `cota_hidraulica` | decimal | m | — | sí |
| `area` | decimal | m² | — | sí |
| `punto_descarga` | ref F03 | — | — | sí (PTAR) |
| `generacion_lodos` | decimal | kg/d | — | no |

#### F08 Medidor

| Atributo | Tipo | Unidad | Rango típico | Oblig. |
|---|---|---|---|---|
| `tipo_medidor` | enum | — | — | sí |
| `diametro_nominal` | entero | mm | 15 … 400 | sí |
| `Q_max` / `Q_nominal` / `Q_min` | decimal | m³/h | — | sí |
| `clase_metrologica` | enum {A, B, C} | — | — | no |
| `perdida_carga_Qnom` | decimal | m | 0.5 … 10 | sí |
| `tipo_lectura` | enum {local, remota, pulsos} | — | — | sí |
| `numero_serie` | texto | — | — | sí |
| `fecha_instalacion` | fecha | — | — | sí |
| `by_pass` | booleano | — | — | no |

#### F10 Sifón

| Atributo | Tipo | Unidad | Rango típico | Oblig. |
|---|---|---|---|---|
| `tipo_sifon` | enum | — | — | sí |
| `diametro_nominal` | entero | mm | 100 … 1 200 | sí |
| `numero_conductos` | entero | — | 1 … 4 | sí |
| `longitud_total` | decimal | m | 5 … 500 | sí |
| `cota_entrada` / `cota_salida` / `cota_punto_bajo` | decimal | m | — | sí |
| `velocidad_minima` | decimal | m/s | 0.90 … 1.20 | derivada |
| `dispositivo_purga` | booleano | — | — | sí |

#### F12 Acometida

| Atributo | Tipo | Unidad | Rango típico | Oblig. |
|---|---|---|---|---|
| `tipo_acometida` | enum | — | — | sí |
| `diametro_nominal` | entero | mm | 13 … 150 | sí |
| `material` | enum {PE, PVC, hierro_galvanizado, cobre} | — | — | sí |
| `longitud` | decimal | m | 2 … 100 | sí |
| `pendiente` | decimal | m/m | 0.005 … 0.05 | cond. (AL) |
| `cota_inicio` / `cota_fin` | decimal | m | — | sí |
| `caja_revision` | booleano | — | — | cond. (AL) |
| `medidor_id` | ref F08 | — | — | cond. |
| `predio_id` | ref | — | — | sí |
| `uso_predio` | enum {residencial, comercial, industrial, oficial, mixto} | — | — | sí |
| `numero_habitantes` | entero | hab | 1 … 50 | cond. |

#### F13 Sumidero

| Atributo | Tipo | Unidad | Rango típico | Oblig. |
|---|---|---|---|---|
| `tipo_sumidero` | enum | — | — | sí |
| `longitud_util` | decimal | m | 0.6 … 3.0 | sí |
| `ancho` | decimal | m | 0.4 … 1.0 | sí |
| `tipo_rejilla` | enum {barrotes, cuadricula, combinada} | — | — | sí |
| `clase_carga` | enum {A15, B125, C250, D400} | — | — | sí |
| `cota_rasante` / `cota_fondo` | decimal | m | — | sí |
| `diametro_conexion` | entero | mm | 150 … 400 | sí |
| `area_drenada` | decimal | m² | 50 … 5 000 | no |
| `coef_escurrimiento` | decimal | — | 0.50 … 0.95 | cond. |

#### F14 Canal

| Atributo | Tipo | Unidad | Rango típico | Oblig. |
|---|---|---|---|---|
| `forma` | enum {rectangular, trapezoidal, triangular, circular} | — | — | sí |
| `base` | decimal | m | 0.20 … 20 | cond. |
| `talud_z` | decimal | H:V | 0 … 3 | cond. |
| `rugosidad_n` | decimal | s/m^(1/3) | 0.012 … 0.035 | sí |
| `pendiente` | decimal | m/m | 0.000 1 … 0.05 | sí |
| `tirante_normal` | decimal | m | — | derivada |
| `tirante_critico` | decimal | m | — | derivada |
| `revestimiento` | enum {tierra, hormigon, mamposteria, geosintetico} | — | — | sí |
| `borde_libre` | decimal | m | 0.10 … 1.00 | sí |

#### F15 Captación

| Atributo | Tipo | Unidad | Rango típico | Oblig. |
|---|---|---|---|---|
| `tipo_captacion` | enum | — | — | sí |
| `caudal_disponible` | decimal | L/s | 0.5 … 5 000 | sí |
| `caudal_concesionado` | decimal | L/s | — | sí |
| `caudal_ecologico` | decimal | L/s | — | cond. |
| `cota` | decimal | m | — | sí |
| `nivel_estatico` / `nivel_dinamico` | decimal | m | — | cond. (pozo) |
| `profundidad` | decimal | m | 10 … 300 | cond. (pozo) |
| `calidad_turbiedad` | decimal | UNT | 0 … 5 000 | no |
| `permiso_aprovechamiento` | texto | — | — | no |

#### F16 Estación de bombeo

| Atributo | Tipo | Unidad | Rango típico | Oblig. |
|---|---|---|---|---|
| `caudal_bombeo` | decimal | L/s | 1 … 2 000 | sí |
| `altura_manometrica` | decimal | m | 5 … 300 | sí |
| `numero_bombas` | entero | — | 2 … 6 | sí |
| `potencia_unitaria` | decimal | kW | 0.5 … 2 000 | sí |
| `eficiencia` | decimal | % | 50 … 85 | sí |
| `curva_HQ` | puntos | (L/s, m) | ≥ 3 puntos | sí |
| `curva_eficiencia` | puntos | (L/s, %) | ≥ 3 puntos | no |
| `npsh_requerido` | decimal | m | 1 … 10 | no |
| `tipo_control` | enum {nivel, caudal, presion, manual} | — | — | sí |
| `volumen_util` | decimal | m³ | — | sí |
| `tiempo_retencion` | decimal | min | 5 … 30 | derivada |

### 4.3 Catálogos controlados

**Materiales de tubería (valores de diseño por defecto, a confirmar en code pack):**

| Material | `n` (AL) | `C` (AP) | `k` (mm) | `E` (GPa) | Uso |
|---|---|---|---|---|---|
| PVC-U | 0.010 | 140–150 | 0.0015–0.007 | 2.8–3.0 | AP y AL |
| PVC-O | 0.009 | 145–150 | 0.0015 | 3.0–3.5 | AP |
| PEAD PE100 | 0.009–0.010 | 140–150 | 0.0015–0.007 | 0.8–1.0 | AP y AL |
| Hierro dúctil | 0.012–0.013 | 130–140 | 0.10–0.26 | 170 | AP, impulsión |
| Acero | 0.012–0.013 | 100–130 | 0.045–0.30 | 200–210 | AP, impulsión |
| Hormigón simple | 0.013–0.015 | — | 0.30–3.0 | 25–30 | AL |
| Hormigón armado | 0.013–0.015 | — | 0.30–3.0 | 30–35 | AL |
| GRP / PRFV | 0.010–0.011 | 140–150 | 0.01–0.05 | 20–30 | AL, AP |
| Fibrocemento | 0.012–0.014 | 130–140 | 0.05–0.15 | 15–20 | AP (en desuso) |
| Mampostería | 0.015–0.017 | — | — | — | AL |
| HF (corrugado) | 0.022–0.025 | — | 0.5–3.0 | 200 | AL pluvial |

**Estados:** `proyectado`, `en_construccion`, `operativo`, `fuera_de_servicio`, `abandonado`, `en_rehabilitacion`.

**Clase de tapa (EN 124):** A15 (peatonal), B125 (vereda/estacionamiento), C250 (calzada con tráfico pesado), D400 (carretera), E600, F900 (aeropuertos).

---

## 5. Hidráulica

Esta sección fija **qué debe calcular la aplicación y con qué ecuación exacta**. Todo coeficiente empírico (`n`, `C`, `k`, `K`, `C_w`) es parámetro del code pack, nunca una constante compilada.

### 5.1 Agua potable

#### 5.1.1 Caudales de diseño

**Población de proyecto.** El período de diseño `T` es parámetro (típico 20 años en redes, 25 en obras de captación). Métodos admisibles:

- Aritmético: `P_n = P_0 + (ΔP/Δt)·n`
- Geométrico: `P_n = P_0·(1 + r)^n` (el más usado en Ecuador; `r` = tasa de crecimiento anual, típico 1.5 %–3.5 %)
- Exponencial: `P_n = P_0·e^(r·n)`
- Logístico: `P_n = P_sat/(1 + ((P_sat/P_0) − 1)·e^(−r·n))`

La aplicación debe permitir **comparar los cuatro** y elegir uno, guardando la serie usada.

**Dotación.** `dot` en L/(hab·día). Valores de referencia (a confirmar por code pack municipal):

| Contexto | Dotación típica | Observación |
|---|---|---|
| Urbano consolidado, clima frío | 130–180 | Con micromedición |
| Urbano, clima cálido (Costa) | 180–250 | Sin micromedición puede superar 250 |
| Rural con conexión domiciliaria | 80–120 | Incluye pérdidas |
| Rural con piletas públicas | 40–60 | |
| Variables no domésticas | + 10 %–25 % | Comercial, industrial, oficial |

**Caudales.**

`Q_med [L/s] = dot · P / 86 400`

`Q_MD = K_1 · Q_med` — caudal máximo diario, `K_1` típico **1.20–1.50** (1.30 frecuente).
`Q_MH = K_1 · K_2 · Q_med` — caudal máximo horario, `K_2` típico **1.50–2.50** (1.50–2.00 frecuente).

**Verificar la consistencia:** `K_2` debe ser mayor que 1; si el code pack trae `K_2` como factor adicional sobre `Q_MD`, el motor debe declararlo explícitamente para no duplicar el `K_1`.

**Caudal de incendio.** `Q_inc` (parámetro, típico 10–15 L/s para 2 bocas simultáneas, duración 1–2 h). Se superpone al `Q_MH` solo en el escenario "demanda máxima + incendio". **No se suma** al caudal medio.

**Pérdidas de agua.** `Q_producido = Q_facturado/(1 − p_perdidas)`, con `p_perdidas` típico 20 %–35 % (parámetro; en Ecuador es habitual 25 %–40 % y debe declararse).

**Caudal de diseño por tramo.** En red de distribución: `Q_tramo = Q_MH · (habitantes aguas abajo / habitantes totales)` o por área tributaria. Se admite asignación por densidad (`hab/ha`) cuando no hay catastro.

#### 5.1.2 Pérdidas por fricción

**a) Hazen-Williams (SI, forma principal).**

`h_f = 10.674 · L · Q^1.852 / (C^1.852 · D^4.871)`

con `L` y `h_f` en m, `Q` en m³/s, `D` en m, `C` adimensional.

Forma con unidades de oficio (`Q` en L/s, `D` en mm), pérdida unitaria:

`J [m/m] = 1.207 × 10^10 · Q^1.852 / (C^1.852 · D^4.871)`

y `J [m/km] = 1 000 · J [m/m]`.

Velocidad implícita: `V = 0.849 · C · R^0.63 · S^0.54` (SI, `R` en m); para tubo lleno `R = D/4`.

Rango de validez declarado: `D ≥ 50 mm`, `V ≤ 3 m/s`, agua a temperatura ambiente. Fuera de ese rango el error crece; la app debe **advertir**, no bloquear.

**b) Darcy-Weisbach (SI).**

`h_f = f · (L/D) · V²/(2g)`

`Re = V·D/ν`

Régimen laminar (`Re < 2 000`): `f = 64/Re`.

Régimen turbulento: **Colebrook-White** (implícita)

`1/√f = −2·log10( k/(3.71·D) + 2.51/(Re·√f) )`

Alternativas explícitas, obligatorias en el motor por estabilidad numérica:

- **Swamee-Jain:** `f = 0.25 / [ log10( k/(3.7·D) + 5.74/Re^0.9 ) ]²`  (3×10³ < Re < 5×10⁸; 10⁻⁶ < k/D < 10⁻²)
- **Haaland:** `1/√f = −1.8·log10[ (k/(3.7·D))^1.11 + 6.9/Re ]`

**Regla de motor:** Colebrook se resuelve por iteración de punto fijo o Newton con tolerancia `|Δf| ≤ 1×10⁻⁶` y máximo 30 iteraciones; si no converge, se usa Swamee-Jain y se registra en el informe.

**c) Elección del método.** El code pack declara `metodo_friccion_AP ∈ {HW, DW}`. Si es `HW`, el atributo `coef_C` es obligatorio; si es `DW`, lo es `rugosidad_k`. El motor no debe mezclar ambos en la misma red sin advertencia.

#### 5.1.3 Pérdidas locales

`h_m = Σ K_i · V²/(2g)`

| Elemento | `K` típico |
|---|---|
| Entrada desde tanque (boca de campana) | 0.50 |
| Entrada con arista viva | 0.80–1.00 |
| Salida a tanque | 1.00 |
| Codo 90° radio corto | 0.90 |
| Codo 90° radio largo | 0.30 |
| Codo 45° | 0.20 |
| Tee, paso directo | 0.30 |
| Tee, paso lateral | 1.50 |
| Válvula de compuerta abierta | 0.20 |
| Válvula de mariposa abierta | 0.30–0.60 |
| Válvula de retención | 2.50–3.50 |
| Reducción gradual | 0.05–0.20 |
| Ampliación gradual | 0.20–0.50 |
| Medidor domiciliario | 2.00–5.00 |
| Rejilla / colador | 1.00–5.00 |

Método equivalente: `L_eq = K·D/f` (solo válido en régimen turbulento rugoso).

**Regla:** en redes de distribución con tramos > 100 m, `h_m` puede despreciarse (< 5 %); en impulsiones, sifones y cámaras de bombeo **es obligatorio** incluirlo. La app debe calcularlo siempre y marcar su peso relativo.

#### 5.1.4 Resolución de la red

**Hardy Cross (mallas).** Para cada lazo:

`ΔQ = − Σ h_f,i / ( n · Σ |h_f,i| / Q_i )`

con `h_f = r·Q·|Q|^(n−1)`, `n = 1.852` (HW) o `n = 2` (DW con `f` constante). Se reparte `ΔQ` con signo en todas las tuberías del lazo y se itera hasta `|ΔQ| < 0.1 L/s` y cierre de lazo `|Σh_f| < 0.001 m`.

Limitación: converge mal en redes con muchas mallas y con tanques a distinta cota. **No se usará como motor principal.**

**Todini / Global Gradient Algorithm (motor principal).** Sistema:

`A11·Q + A12·H = −A10·H_0`
`A21·Q = q`

donde `A11` es diagonal con `r_i·|Q_i|^(n_i−1)`, `A12` es la matriz de incidencia nodo–arco, `A21 = A12ᵀ`, `A10` acopla nodos de carga fija (tanques), `H_0` son sus cargas y `q` son las demandas nodales. Se resuelve por Newton acoplado; el sistema lineal resultante es simétrico y definido positivo si la red es conexa y tiene al menos un nodo de carga fija.

Criterios de convergencia (parámetros): `Σ|Q_i^(k+1) − Q_i^k| / Σ|Q_i^k| < 1×10⁻⁶`, balance de masa nodal `|ΣQ − q| < 0.01 L/s`, máximo 100 iteraciones.

**Casos de borde obligatorios:** nodo aislado, red sin fuente, válvula cerrada que desconecta un sector, tanque que se vacía (carga por debajo del fondo), demanda negativa (no permitida), arco con `Q = 0` (linealización con `Q_min = 1×10⁻⁶ m³/s`).

#### 5.1.5 Diámetro económico

Criterio primario, por velocidad económica:

`D = √( 4·Q / (π·v_eco) )`, con `v_eco` típico **1.0–1.5 m/s** (parámetro).

Criterio de selección práctica (Bresse y variantes): `D ≈ K_b·√Q`, con `Q` en m³/s, `D` en m y `K_b` **entre 1.0 y 1.5** según autor y régimen de operación. **No se fija un valor único**: `K_b` es parámetro del code pack y la app debe mostrar el rango, no un número.

Optimización económica real (v1.1): minimizar `C_total = C_tuberia(D)·L + C_energia(Q,H,t) + C_mantenimiento`, sujeto a `v_min ≤ v ≤ v_max` y `P_min ≤ P ≤ P_max`. En v1 se ofrece el diámetro comercial inmediatamente superior que cumple velocidad y presión.

#### 5.1.6 Verificaciones de presión y velocidad

| Verificación | Expresión | Valor típico (parámetro) |
|---|---|---|
| Presión mínima en servicio | `P = H_GL − Z_nodo ≥ P_min` | 10 mca (algunos prestadores 15) |
| Presión máxima estática | `P_est = Z_tanque,NAMO − Z_nodo ≤ P_max` | 70 mca (rango 50–70) |
| Presión máxima en transitorio | `P_trans ≤ PN_tuberia` | 1.0 × PN, con margen 1.3 sobre estática |
| Velocidad mínima | `v ≥ v_min` | 0.60 m/s |
| Velocidad máxima | `v ≤ v_max` | 3.00 m/s (5.00 en transitorio) |
| Velocidad en impulsión | `0.80 ≤ v ≤ 2.50` | Evita sedimentación y golpe |

Nota: la presión mínima se evalúa en el **nodo de consumo**, con la cota del terreno (no la de la tubería). Si el usuario está en un edificio de varios pisos, la presión requerida en la red es `P_min + 3.5 m` por piso adicional (parámetro).

#### 5.1.7 Golpe de ariete

**Celeridad de onda.**

`a = 1425 / √( 1 + K·D/(E·e) )`

con `K = 2.07×10⁹ Pa` (agua), `E` módulo elástico de la pared (Pa), `e` espesor (m), `D` diámetro interior (m). Alternativa exacta: `a = √(K/ρ) / √(1 + K·D/(E·e))`.

Valores típicos de `a`: PVC 300–500 m/s; PEAD 250–400; hierro dúctil 1 000–1 200; acero 1 000–1 200; hormigón 900–1 100. **Calcular siempre con la fórmula**, no tabular.

**Sobrepresión de Joukowsky (cierre rápido).**

`ΔH = a·ΔV / g`

**Tiempo crítico.** `t_c = 2L/a`. Si el tiempo de cierre `t_cierre < t_c`, el cierre es rápido y aplica Joukowsky completo. Si `t_cierre ≥ t_c`, se usa la fórmula de Michaud (cierre lento):

`ΔH = 2·L·V / (g·t_cierre)`

**Verificación.** `H_max = H_estática + ΔH ≤ PN` y `H_min = H_estática − ΔH ≥ −10 mca` (presión absoluta > presión de vapor, margen de 2 m). Si falla: aumentar clase de presión, instalar tanque de oscilación, volante de inercia, válvula de alivio o aire.

**Ejemplo numérico verificado.** PVC DN 200, `e = 3.9 mm`, `E = 2.8 GPa`, `V = 1.5 m/s`, cierre instantáneo:

`a = 1425/√(1 + (2.07×10⁹ × 0.200)/(2.8×10⁹ × 0.0039)) = 1425/√(1 + 37.9) = 228 m/s`
`ΔH = 228 × 1.5 / 9.81 = 34.9 m`

Es decir, un cierre instantáneo en PVC DN 200 a 1.5 m/s añade casi 35 m de carga: **una tubería PN 6 no lo resiste**. Este ejemplo debe ser un test de regresión del motor.

#### 5.1.8 Bombeo

- Curva de bomba: `H_b(Q) = H_0 − a_b·Q²` (o spline sobre los puntos del fabricante).
- Curva del sistema: `H_s(Q) = H_estática + (h_f + h_m)(Q)`; para régimen turbulento `H_s ≈ H_est + r·Q^1.852`.
- Punto de operación: `H_b(Q) = H_s(Q)` (intersección).
- Potencia: `P_eje [kW] = ρ·g·Q·H_b / (1 000·η)`; `P_motor = P_eje · f_s`, con `f_s` 1.10–1.20.
- Energía: `E [kWh/año] = P_eje · h_operación · 365`.
- NPSH: `NPSH_d = (P_atm − P_v)/(ρ·g) + Z_succión − h_f,succión`; debe cumplir `NPSH_d ≥ NPSH_r + 0.5 m`.

### 5.2 Alcantarillado sanitario

#### 5.2.1 Caudales de diseño

`Q_agua_residual = c_r · Q_agua_potable`, con coeficiente de retorno `c_r` típico **0.70–0.85** (0.80 frecuente).

Aportes:

- **Infiltración:** `Q_inf = q_inf · A` (L/s por ha) o fracción del caudal medio; `q_inf` típico **0.05–0.20 L/s/ha**, dependiendo de napa y estado de juntas. También se admite `Q_inf = 0.10·Q_med`.
- **Conexiones erradas / pluvial:** solo en sistema combinado o mixto.
- **Industrial:** caudal medido o estimado por tipo de industria.

**Factor de punta.** Harmon:

`M = 1 + 14/(4 + √P_k)`

con `P_k` = población en **miles** de habitantes. Para `P = 10 000` hab: `M = 1 + 14/(4 + 3.162) = 2.98`. Para `P = 1 000`: `M = 1 + 14/5 = 3.80`. Para `P = 100 000`: `M = 1.78`.

Alternativa normativa frecuente: `M` constante entre 1.5 y 2.5 (parámetro). **El code pack decide**; la app debe registrar cuál se usó.

`Q_diseño = M·Q_med + Q_inf + Q_ind`

**Caudal mínimo de diseño.** Todo tramo debe poder transportar al menos `Q_min` (típico 1.5 L/s, parámetro) para verificar arrastre.

#### 5.2.2 Manning

`V = (1/n)·R^(2/3)·S^(1/2)`
`Q = (1/n)·A·R^(2/3)·S^(1/2)`

con `R = A/P` en m, `S` en m/m, `n` en s/m^(1/3). Válido para flujo uniforme; en alcantarillado se acepta como aproximación de diseño tramo a tramo.

Sección circular parcialmente llena (`y` = tirante, `D` = diámetro interior):

`θ_c = 2·arccos(1 − 2y/D)`  (radianes, `0 < y/D < 1`)

`A = (D²/8)·(θ_c − sen θ_c)`
`P = θ_c·D/2`
`T = D·sen(θ_c/2)`  (ancho superficial)

#### 5.2.3 Capacidad a tubo lleno

Para `y = D`: `A_o = πD²/4`, `R_o = D/4`, luego

`Q_o = (1/n)·(πD²/4)·(D/4)^(2/3)·S^(1/2) = (0.3117/n)·D^(8/3)·S^(1/2)`
`V_o = (1/n)·(D/4)^(2/3)·S^(1/2)`

**Tabla de referencia (n = 0.013, calculada y verificada):**

| DN (mm) | `Q_o` a S = 0.5 % (L/s) | `V_o` (m/s) | `Q_o` a S = 1.0 % (L/s) | `V_o` (m/s) |
|---|---|---|---|---|
| 200 | 23.19 | 0.738 | 32.80 | 1.044 |
| 250 | 42.05 | 0.857 | 59.47 | 1.211 |
| 300 | 68.38 | 0.967 | 96.70 | 1.368 |
| 350 | 103.14 | 1.072 | 145.87 | 1.516 |
| 400 | 147.26 | 1.172 | 208.26 | 1.657 |
| 500 | 267.00 | 1.360 | 377.60 | 1.923 |
| 600 | 434.17 | 1.536 | 614.01 | 2.172 |

#### 5.2.4 Flujo parcial: relaciones `q/Q` y `v/V`

`A/A_o = (θ_c − sen θ_c)/(2π)`
`R/R_o = 1 − sen θ_c/θ_c`
`q/Q_o = (A/A_o)·(R/R_o)^(2/3)`
`v/V_o = (R/R_o)^(2/3)`

**Tabla normalizada (calculada con las fórmulas anteriores, no copiada de un libro):**

| `y/D` | `θ_c` (rad) | `A/A_o` | `q/Q_o` | `v/V_o` |
|---|---|---|---|---|
| 0.05 | 0.9021 | 0.0187 | 0.0048 | 0.257 |
| 0.10 | 1.2870 | 0.0520 | 0.0209 | 0.401 |
| 0.15 | 1.5908 | 0.0941 | 0.0486 | 0.517 |
| 0.20 | 1.8546 | 0.1424 | 0.0876 | 0.615 |
| 0.25 | 2.0944 | 0.1955 | 0.1370 | 0.701 |
| 0.30 | 2.3186 | 0.2523 | 0.1958 | 0.776 |
| 0.35 | 2.5322 | 0.3119 | 0.2629 | 0.843 |
| 0.40 | 2.7389 | 0.3735 | 0.3370 | 0.902 |
| 0.45 | 2.9413 | 0.4364 | 0.4165 | 0.954 |
| 0.50 | 3.1416 | 0.5000 | 0.5000 | 1.000 |
| 0.55 | 3.3419 | 0.5636 | 0.5857 | 1.039 |
| 0.60 | 3.5443 | 0.6265 | 0.6718 | 1.072 |
| 0.65 | 3.7510 | 0.6881 | 0.7564 | 1.099 |
| 0.70 | 3.9646 | 0.7477 | 0.8372 | 1.120 |
| 0.75 | 4.1888 | 0.8045 | 0.9119 | 1.134 |
| 0.80 | 4.4286 | 0.8576 | 0.9775 | 1.140 |
| 0.81 | 4.4791 | 0.8677 | 0.9892 | 1.140 |
| 0.85 | 4.6924 | 0.9059 | 1.0304 | 1.137 |
| 0.90 | 4.9962 | 0.9480 | 1.0658 | 1.124 |
| 0.94 | 5.2933 | 0.9755 | **1.0757** | 1.103 |
| 0.95 | 5.3811 | 0.9813 | 1.0745 | 1.095 |
| 0.98 | 5.7156 | 0.9952 | 1.0567 | 1.062 |

**Hechos que el motor debe conocer:** el caudal máximo **no** ocurre a tubo lleno sino a `y/D ≈ 0.94` (`q/Q_o = 1.076`); la velocidad máxima ocurre a `y/D ≈ 0.81` (`v/V_o = 1.14`). Por eso "diseñar a tubo lleno" es conservador en capacidad pero **no** en velocidad.

#### 5.2.5 Tensión tractriz

`τ = γ·R·S = 9 810·R·S`  [Pa]

Criterios típicos (parámetros):

| Red | `τ_min` | Observación |
|---|---|---|
| Alcantarillado sanitario | 1.0–1.5 Pa | Arrastre de sólidos orgánicos |
| Alcantarillado pluvial | 3.0–4.0 Pa | Arrastre de arena |
| Colector con alta carga de grasas | 2.0 Pa | Requiere limpieza frecuente |

Pendiente mínima derivada: `S_min = τ_min/(9 810·R)`. Para DN 200 a tubo lleno (`R = 0.05 m`) y `τ_min = 1.0 Pa`: `S_min = 0.00204` (0.20 %). **Esto demuestra por qué la regla "0.5 % mínimo" es más restrictiva que la tensión tractriz** y por qué ambas deben coexistir: la primera es constructiva, la segunda es de autolimpieza.

#### 5.2.6 Criterios de velocidad, pendiente y llenado

| Criterio | Expresión | Valor típico |
|---|---|---|
| Velocidad mínima | `v ≥ v_min` | 0.60 m/s (sanitario); 0.75 m/s recomendado |
| Velocidad máxima | `v ≤ v_max` | 5.0 m/s (hormigón); 3.0–4.0 m/s si hay sólidos abrasivos |
| Llenado máximo de diseño | `y/D ≤ 0.85` | 0.75 en colectores principales |
| Llenado máximo hidráulico | `y/D ≤ 0.94` | Por encima, flujo inestable |
| Pendiente mínima | `S ≥ S_min` | 0.5 % para `DN ≤ 200`; 0.3 % para `DN > 200` (parámetro) |
| Pendiente máxima | `S ≤ S_max` | 10 %–15 % (requiere disipación) |
| Froude | `Fr = V/√(g·D_h) ≤ 0.80` | `D_h = A/T`; supercrítico exige análisis |

#### 5.2.7 Régimen crítico y perfil

`Fr = V/√(g·D_h)`, con `D_h = A/T`.

- `Fr < 1`: subcrítico (régimen normal en alcantarillado).
- `Fr = 1`: crítico; el tirante crítico resuelve `Q²·T/(g·A³) = 1`.
- `Fr > 1`: supercrítico; se produce en pendientes fuertes y en caídas. Requiere verificar resalto y no debe usarse en tramos con cambio de pendiente sin análisis.

**Perfil hidráulico (remanso).** Ecuación de energía entre secciones 1 y 2:

`Z_1 + y_1 + α·V_1²/(2g) = Z_2 + y_2 + α·V_2²/(2g) + h_f + h_m`

con `α ≈ 1.0`, `h_f = L·(J_1 + J_2)/2` (pendiente de fricción media, Manning) y `h_m` por cambio de dirección en pozo. Se integra por pasos estándar desde aguas abajo hacia aguas arriba (remanso) cuando el control está aguas abajo (descarga, pozo de caída), y en sentido contrario cuando el control es crítico.

**Regla de la app:** para cada tramo se calcula (a) capacidad a tubo lleno, (b) tirante normal al caudal de diseño, (c) verificación de remanso si el tramo es el último antes de una descarga o si hay un pozo de caída aguas abajo. Sin (c) el diseño puede fallar por inundación.

#### 5.2.8 Pérdidas y caídas en pozos

- Cambio de dirección: `h_m = K_d·V²/(2g)`, `K_d` 0.1–1.0 según ángulo (0.1 para 0–15°, 1.0 para > 90°).
- Caída: la energía se disipa; no se recupera aguas abajo. El pozo de caída debe garantizar que el chorro no erosione la solera: usar `h_caida ≤ 0.5 m` con resalto, o tubería de caída con disipador.
- Cota clave de salida: `Z_C,salida = Z_C,entrada − h_f,tramo − h_m` (si no hay caída). Si hay caída: `Z_C,salida = Z_C,entrada − ΔZ_caida`.

#### 5.2.9 Ejemplo numérico verificado (test de regresión)

Datos: `Q_diseño = 15.0 L/s`, `DN 200 mm`, `n = 0.013`, `S = 0.005 m/m`.

1. `Q_o = 23.19 L/s` (tabla §5.2.3); `q/Q_o = 15.0/23.19 = 0.647`.
2. De la tabla §5.2.4, interpolando entre `y/D = 0.55` (0.5857) y `0.60` (0.6718): `y/D ≈ 0.586`.
3. `y = 0.117 m`; `v/V_o ≈ 1.062`; `v = 1.062 × 0.738 = 0.784 m/s`.
4. `R/R_o = (v/V_o)^(3/2) = 1.062^1.5 = 1.096`; `R = 0.0548 m`.
5. `τ = 9 810 × 0.0548 × 0.005 = 2.69 Pa`.

Resultado: cumple `v ≥ 0.60`, `y/D ≤ 0.85`, `τ ≥ 1.0`. Tramo válido.

Contraejemplo: el mismo caudal con `S = 0.002` da `Q_o = 14.66 L/s < 15.0 L/s`: **el tubo trabajaría a presión**. La app debe reportar "capacidad insuficiente", no un `y/D > 1`.

#### 5.2.10 Vertederos y estructuras de descarga

Vertedero de cresta:

`Q = C_w · L_e · H^(3/2)`

con `Q` en m³/s, `L_e` longitud efectiva (m), `H` carga sobre la cresta (m), `C_w ≈ 1.84` (cresta delgada, SI) y `≈ 1.70` (cresta ancha). Con contracción lateral: `L_e = L − 0.2·H`. Sumergencia: aplicar corrección según `H_2/H_1` (tabla del code pack, no aproximar).

#### 5.2.11 Canales

Sección trapezoidal: `A = b·y + z·y²`, `P = b + 2y·√(1 + z²)`, `T = b + 2zy`.
Sección rectangular: `A = b·y`, `P = b + 2y`, `T = b`.

Tirante normal: resolver `Q = (1/n)·A·(A/P)^(2/3)·S^(1/2)` por bisección o Newton (la función es monótona en `y`).
Tirante crítico: resolver `Q²·T/(g·A³) = 1`.
Borde libre: `f_b = 0.10 + 0.05·y` (m) como mínimo práctico, o el que fije el code pack.

---

## 6. Marco normativo

### 6.1 Jerarquía aplicable en Ecuador

| Nivel | Instrumento | Qué aporta al diseño |
|---|---|---|
| Constitucional | Constitución de 2008 (art. 12, art. 264, art. 314) | El agua como derecho; competencia municipal exclusiva sobre agua potable, alcantarillado y saneamiento; prohibición de privatización del servicio |
| Legal | Ley Orgánica de Recursos Hídricos, Usos y Aprovechamiento del Agua (LORHUAA, 2014) | Concesión, caudal ecológico, autoridad única del agua, regulación |
| Reglamentario | Reglamento General de la LORHUAA | Procedimientos de autorización y control |
| Técnico nacional | CPE INEN 5 (Instalaciones Sanitarias) y normas IEOS | Criterios de diseño de redes, dotaciones, coeficientes, velocidades |
| Técnico de calidad | NTE INEN 1108 (agua potable — requisitos) | Parámetros de calidad del agua entregada |
| Local | Ordenanzas municipales y manuales del prestador (EPMAPS, ETAPA EP, Interagua, EMAAP-Q, etc.) | Valores numéricos concretos exigidos en cada cantón |
| Internacional (referencia) | AWWA, EN, ISO | Métodos, materiales, ensayos, gestión |

**Conclusión operativa:** no existe en Ecuador un código nacional único, vigente y completo que fije todos los valores hidráulicos. En la práctica manda la **ordenanza municipal + manual del prestador**, y el CPE INEN 5 / IEOS se usan como referencia de método. Por eso el diseño de la app debe apoyarse en un **code pack por jurisdicción** y no en "la norma ecuatoriana" en abstracto.

### 6.2 Normas ecuatorianas y su uso

| Identificador | Qué regula | Uso previsto en la app | Confianza |
|---|---|---|---|
| CPE INEN 5 — Instalaciones Sanitarias | Diseño de instalaciones de agua potable y alcantarillado | Fuente de criterios base (dotación, coeficientes, velocidades, recubrimientos) | **Media**: existe y es la referencia clásica; la numeración de subpartes y su vigencia deben verificarse contra el ejemplar físico |
| Normas IEOS (Instituto Ecuatoriano de Obras Sanitarias) | Diseño de sistemas de agua potable y alcantarillado, urbanos y rurales | Criterios de dotación, período de diseño, coeficientes de punta, velocidades | **Media**: ampliamente citadas en proyectos ecuatorianos; edición exacta a confirmar |
| NTE INEN 1108 | Agua potable. Requisitos | Validación de calidad (no hidráulica) | **Alta** |
| LORHUAA (2014) y su Reglamento | Concesión, caudal ecológico, autoridad | Restricciones legales de caudal y permisos | **Alta** (existencia); **media** en detalles procedimentales |
| NEC (2015) | Estructuras, sismo, geotecnia, cargas | Solo para estructuras de soporte (tanques, cámaras, anclajes) | **Alta** en que existe y es estructural; **media** en que no contiene criterios hidráulicos de redes |
| Ordenanzas municipales / manuales de prestador | Valores numéricos exigidos localmente | Code pack municipal | **Alta** en su existencia; el contenido varía por cantón |
| Regulación de ARCA / autoridad del agua | Tarifas, calidad de servicio, control | Reportes regulatorios (v2) | **Media** |

**Advertencia explícita:** este documento **no cita números de cláusula** de CPE INEN 5 ni de las normas IEOS, porque no se dispone del ejemplar oficial verificado. Cualquier cláusula que el code pack declare debe provenir de la fuente, con edición y hash (§6.4). Un número de cláusula inventado es un defecto grave del pack, no un detalle.

### 6.3 Referencias internacionales

| Familia | Documento | Para qué se usa |
|---|---|---|
| Diseño de redes AP | AWWA M32 — Distribution Network Analysis | Métodos de análisis de mallas, calibración, escenarios |
| Materiales AP | AWWA M11 (acero), M23 (PVC), M41 (hierro dúctil), M55 (PEAD) | Diseño y instalación de tuberías |
| Acometidas y medición | AWWA M22 — Sizing Water Service Lines and Meters | Dimensionamiento de acometidas |
| Tubería PVC a presión | AWWA C900 / C905 | Clases y dimensionamiento |
| Válvulas | AWWA C509 (compuerta resilente), C504 (mariposa), C512 (aire), C508 (retención) | Especificación de válvulas |
| Hidrantes | AWWA C502 — Dry-Barrel Fire Hydrants | Especificación de hidrantes |
| Alcantarillado — gestión | EN 752 | Gestión y diseño de sistemas de desagüe exterior |
| Alcantarillado — construcción | EN 1610 | Construcción y ensayo de tuberías; referencia para ancho de zanja y relleno |
| Alcantarillado — materiales | EN 1916 / EN 1917 (hormigón), EN 1401 (PVC-U), EN 12666 (PP) | Especificación de tubos y pozos |
| Sumideros y tapas | EN 124 | Clases de carga de tapas y rejillas |
| Agua — sistemas exteriores | EN 805 | Requisitos de sistemas y componentes de agua fuera de edificios |
| Tuberías AP | EN 545 (hierro dúctil), EN 12201 (PE) | Especificación |
| Tuberías AP | ISO 2531 (hierro dúctil), ISO 1452 (PVC-U) | Especificación |
| Medición | ISO 4064 | Medidores de agua potable |
| Servicio al usuario | ISO 24510 | Evaluación y mejora del servicio |
| Gestión de alcantarillado | ISO 24511 | Gestión de servicios de saneamiento |
| Gestión de agua potable | ISO 24512 | Gestión de servicios de agua potable |
| Activos | ISO 55000 | Gestión de activos (v2) |
| Buenas prácticas | ASCE/WEF MOP 37; Metcalf & Eddy; CONAGUA (MAPAS); RAS 2000 (Colombia) | Contrastar valores cuando la norma local no fija uno |

**Uso correcto:** una referencia internacional **no sustituye** la norma local. Se usa para (a) métodos de cálculo, (b) especificaciones de material, (c) valores por defecto cuando el code pack local no fija ninguno, siempre marcados como "valor de referencia internacional, no normativo localmente".

### 6.4 Contrato del code pack

Un *code pack* es el registro de procedencia de una norma y de los parámetros que la app carga. **No contiene texto de la norma**: contiene identificadores, valores y trazabilidad.

| Campo | Obligatorio | Ejemplo |
|---|---|---|
| `jurisdiccion` | sí | `EC-PICHINCHA-RUCM` |
| `standard` | sí | `CPE INEN 5` |
| `edicion` | sí | `1992` (a verificar) |
| `fuente` + procedencia | sí | Ruta/URL del ejemplar, quién lo emitió |
| `alcance` | sí | "Diseño de redes de agua potable y alcantarillado" |
| `clausulas_usadas` | sí | Lista de identificadores exactos, tomados del ejemplar |
| `operaciones_soportadas` | sí | `validar_pendiente_minima`, `calcular_dotacion` |
| `exclusiones` | sí | Lo que el pack **no** cubre |
| `unidades_y_convenciones` | sí | "pendiente en m/m; presión en mca" |
| `errata` / `enmiendas` | si existen | |
| `hash_fuente` | sí | SHA-256 del ejemplar digitalizado |
| `casos_de_validacion` | sí | ≥ 1 caso con resultado esperado |
| `estado` | sí | `ACTIVE` / `BLOCKED_BY_SOURCE` |

**Regla de oro:** si falta un campo obligatorio, el pack queda `BLOCKED_BY_SOURCE` y la app **no puede emitir un veredicto de cumplimiento**; solo puede calcular mecánica y marcar los chequeos como "no verificables". Esto debe ser visible en el informe y en la CLI.

**Regla de oro 2:** cada resultado numérico guarda `hash_code_pack` y `version`. Cambiar un parámetro invalida todos los veredictos dependientes.

### 6.5 Registro de incertidumbre normativa

| # | Afirmación | Confianza | Qué hacer antes de v1 |
|---|---|---|---|
| I-01 | CPE INEN 5 es la norma ecuatoriana de instalaciones sanitarias y cubre diseño de redes | Media | Obtener ejemplar; verificar año, partes y vigencia |
| I-02 | Las normas IEOS siguen siendo aceptadas como referencia municipal | Media | Confirmar con 2–3 municipios objetivo |
| I-03 | La NEC 2015 no fija criterios hidráulicos de redes | Media-alta | Verificar índice de volúmenes NEC |
| I-04 | Dotación urbana típica 160–220 L/hab/d en Ecuador | Media | Contrastar con el manual del prestador objetivo |
| I-05 | `K_1 = 1.30`, `K_2 = 1.50–2.00` | Media | Ídem |
| I-06 | Presión mínima de servicio 10 mca y máxima 70 mca | Media | Ídem |
| I-07 | Pendiente mínima de colector 0.5 % para DN ≤ 200 | Media | Ídem |
| I-08 | Velocidad máxima 5 m/s en hormigón, 3–4 en PVC | Media | Ídem |
| I-09 | Coeficiente de retorno 0.80 | Media-alta | Ídem |
| I-10 | Ancho de zanja = `D_ext + 0.60 m` como práctica local | Media | Contrastar con APU y EN 1610 |
| I-11 | La autoridad del agua es ARCA y el ente rector ha cambiado de nombre/adscripción | Media | Confirmar denominación vigente a la fecha del pack |

**Cómo se resuelve:** cada fila se convierte en una tarea del backlog (§12, F-54) y en un campo del pack. Mientras no se resuelva, la app usa el valor marcado como "referencia" y lo muestra como advertencia, no como cumplimiento.

---

## 7. Excavación y movimiento de tierras

La excavación es un **requisito de primera clase**: debe ser exacta, auditable y separable por tipo de obra, porque alimenta el presupuesto y el plan de obra.

### 7.1 Definiciones y principio de cálculo

- `Z_T` cota de terreno natural (§2.3).
- `Z_C` cota clave (invert).
- `Z_F = Z_C − e − h_cama` cota de fondo de excavación.
- `H = Z_T − Z_F` profundidad de excavación.
- `b` ancho de zanja en el fondo.
- `z` talud horizontal por unidad vertical (H:V). `z = 0` ⇒ paredes verticales.

**Principio:** el volumen se calcula **por tramos entre secciones transversales**, con área constante en cada sección. Nunca se usa un "ancho medio" ni una "profundidad media" global del proyecto.

### 7.2 Ancho de zanja

`b = D_ext + 2·c`

con `c` = espacio de trabajo libre a cada lado. Valores típicos (parámetros): `c = 0.30 m` general; `c = 0.40 m` en roca o con entibado; `b_min = 0.60 m` por razones constructivas (acceso de personal y compactación).

| DN | `D_ext` PVC/PE (m) | `b` PVC (m) | `D_ext` hormigón (m) | `b` hormigón (m) |
|---|---|---|---|---|
| 110 | 0.110 | 0.70 | — | — |
| 160 | 0.160 | 0.75 | — | — |
| 200 | 0.225 | 0.85 | 0.250 | 0.85 |
| 250 | 0.280 | 0.90 | 0.310 | 0.90 |
| 300 | 0.315 | 0.90 | 0.360 | 0.95 |
| 400 | 0.400 | 1.00 | 0.460 | 1.05 |
| 500 | 0.500 | 1.10 | 0.580 | 1.20 |
| 600 | 0.630 | 1.25 | 0.700 | 1.30 |

**Nota:** los valores de `D_ext` son referenciales por serie/fabricante; la app debe tomarlos del catálogo del material, no de esta tabla. La tabla sirve para dimensionar y para el *default* cuando no hay catálogo. La columna `b` está redondeada a múltiplos de 0.05 m; el cálculo siempre usa `b = D_ext + 2c` con el `D_ext` del catálogo.

**Sobreancho (overbreak).** En roca o con entibado, el ancho real excavado excede el de proyecto: `b_real = b + 2·s_ob`, con `s_ob = 0.10–0.20 m` (parámetro). Debe declararse si el volumen se computa con `b` o con `b_real`; **recomendación: computar con `b_real` y reportar ambos**.

### 7.3 Profundidad

`H = Z_T − Z_C + e + h_cama`

- `e` = espesor de pared del tubo.
- `h_cama` = espesor de cama de arena/grava (típico **0.10 m**; **0.15 m** en roca).

Profundidad de proyecto (recubrimiento) se verifica con `Z_R`, no con `Z_T` (§2.3).

**Casos especiales:**

- Terreno en pendiente transversal fuerte: la sección no es horizontal; se calcula el área real con el perfil transversal o se subdivide el tramo.
- Cruce de vía: puede requerir `H` mayor (recubrimiento mínimo 1.20 m bajo calzada, parámetro) y encamisado.
- Nivel freático por encima de `Z_F`: la excavación es "bajo agua" y cambia el rendimiento y el tipo de obra (agotamiento, entibado). Debe existir una capa `freático` (§8).

### 7.4 Taludes, bermas y entibado

| Tipo de suelo | Talud `z` (H:V) | Ángulo aprox. | Observación |
|---|---|---|---|
| Roca sana | 0.10–0.25 | 80–85° | Puede exigir voladura |
| Roca fracturada | 0.25–0.50 | 63–76° | |
| Grava densa / suelo granular compacto | 0.75–1.00 | 45° | |
| Arena suelta | 1.50 | 34° | |
| Limo | 1.00–1.50 | 34–45° | |
| Arcilla firme | 0.75 | 53° | |
| Arcilla blanda / limo saturado | 1.50 | 34° | |
| Relleno antrópico | 1.50–2.00 | 27–34° | Requiere entibado |

**Reglas de entibado (parámetros, no norma):**

- `H > 1.50 m` en suelo no cohesivo o con estructuras a menos de `2·H` ⇒ entibado obligatorio.
- `H > 3.00 m` ⇒ entibado diseñado (no tabulado) y plan de seguridad.
- Bermas: cada **3.00 m** de profundidad, ancho **0.60–1.00 m**; reducen el volumen pero lo hacen más costoso. La app debe ofrecer "con bermas / sin bermas".
- Con bermas, el área se calcula por franjas verticales: `A = Σ (b·h_i + z·h_i²)` por franja.

**Área de la sección transversal:**

- Paredes verticales (`z = 0`): `A = b·H`
- Trapezoidal: `A = b·H + z·H²`
- Con bermas: suma de franjas.

### 7.5 Cama, relleno y materiales

| Componente | Espesor típico | Material | ¿Importado? |
|---|---|---|---|
| Cama (bedding) | 0.10 m (0.15 en roca) | Arena o grava fina compactada | Sí |
| Relleno lateral (haunching) | hasta el eje del tubo | Material seleccionado | Sí / parcial |
| Relleno sobre clave | 0.30 m mínimo | Material seleccionado, sin piedras > 20 mm | Sí |
| Relleno final | hasta `Z_R` | Material de excavación compactado al 95 % PM | No |
| Relleno bajo calzada | 0.30 m bajo subbase | Material granular | Sí |

**Regla:** el relleno importado (cama + seleccionado) se mide en m³ y se paga aparte del volumen excavado. El motor debe producir **tres volúmenes distintos**: excavación, relleno importado y relleno con material propio.

### 7.6 Volúmenes

**a) Zanja entre secciones (áreas medias).**

`V = Σ_i L_i · (A_i + A_{i+1})/2`

Es exacto cuando el área varía linealmente con la longitud, que es el caso normal (profundidad lineal). Se aplica entre pozos y en cada cambio de diámetro, material o suelo.

**b) Prismoidal (cuando el área no varía linealmente).**

`V = (L/6)·(A_1 + 4·A_m + A_2)`

Donde `A_m` es el área en la sección media **calculada con la profundidad media real**, no como promedio de áreas. Se usa en terrenos con quiebre, en cambios de talud y en capas de suelo. La diferencia con (a) suele ser < 2 %; la app debe ofrecer el método y registrar cuál usó.

**c) Volumen por capas (suelo / roca).** Si existe una cota de techo rocoso `Z_roca`:

- Espesor de suelo: `H_s = min(H, Z_T − max(Z_roca, Z_F))`
- Espesor de roca: `H_r = max(0, Z_roca − Z_F)`
- `V_suelo = Σ L·(A_s,i + A_s,i+1)/2`, `V_roca = Σ L·(A_r,i + A_r,i+1)/2`
- `V_total = V_suelo + V_roca`; se reportan separados porque el precio unitario difiere.

**d) Pozo (prisma, no zanja).**

`A_exc = (π/4)·(D_int + 2·e + 2·s)²` para pozo circular
`A_exc = (a + 2s)·(b + 2s)` para pozo rectangular

`V_pozo = A_exc · H_p`, con `H_p = Z_T − Z_solera` y `s = 0.30 m` (espacio de trabajo).

Si `H_p > 1.50 m`, se añade talud por franjas (igual que la zanja). El volumen del pozo **nunca** se suma al de la zanja del tramo adyacente: se mide y se paga por separado (`m³ pozo` vs `m³ zanja`).

**e) Volumen ocupado por el tubo** (para descontar del relleno):

`V_tubo = (π/4)·D_ext²·L`

**f) Balance de tierras.**

Tres estados de volumen, que nunca se suman entre sí:

- **Banco (in situ):** `V_exc`, el que se mide y se paga como excavación.
- **Suelto:** `V_suelto = V_exc · f_e`; es el que se transporta y se dispone.
- **Compactado:** `V_relleno`, el que se exige en obra.

Cálculo:

- `V_relleno` = `V_exc − V_tubo − V_cama` para una zanja; `V_exc,pozo − V_estructura` para un pozo. No se mezclan.
- `V_suelto_necesario = V_relleno / f_c` (volumen suelto que hay que colocar para lograr el relleno compactado).
- `V_sobrante = V_suelto − V_suelto_necesario`. Si `> 0`, va a disposición final; si `< 0`, falta material y se pide préstamo por `|V_sobrante|`.
- `f_e` = factor de esponjamiento: **1.20–1.35** en suelo, **1.40–1.60** en roca.
- `f_c` = factor de compactación: **0.85–0.90** (suelto → compactado).

**Error frecuente que la app debe impedir:** comparar `V_exc` (banco) con `V_relleno` (compactado) y concluir que "sobra tierra". El balance solo es válido en el mismo estado.

**g) Excavación adicional por caída o por cámara.** Cualquier estructura enterrada (cámara, tanque, sifón) usa el mismo método prismático con su propia geometría; no se asimila a zanja.

### 7.7 Clasificación de suelos

| Criterio | Clases | Uso |
|---|---|---|
| Excavabilidad (para precio unitario) | Tierra, material suelto, roca suelta, roca fija, material saturado, relleno antrópico | Define el APU y el rendimiento |
| SUCS (geotecnia) | GW, GP, GM, GC, SW, SP, SM, SC, ML, CL, CH, OL, OH, PT | Define talud, entibado, capacidad portante |
| Compactación | 90 %, 95 %, 98 % Proctor modificado | Especificación de relleno |
| Agresividad | Baja, media, alta (sulfatos, pH) | Selección de material de tubo y de cama |

**Regla:** el modelo debe permitir asignar un `tipo_suelo` por **tramo** y por **profundidad** (estratigrafía), no solo por proyecto. Sin eso, el volumen por tipo de material es ficticio.

### 7.8 Ejemplo numérico verificado

**Zanja.** DN 200 PVC (`D_ext = 0.225 m`), `L = 100 m`, `H_1 = 2.00 m`, `H_2 = 2.50 m`, `b = 0.85 m`, paredes verticales (`z = 0`):

- `A_1 = 0.85 × 2.00 = 1.700 m²`
- `A_2 = 0.85 × 2.50 = 2.125 m²`
- `V = 100 × (1.700 + 2.125)/2 = 191.25 m³`

Con talud `z = 0.25` en toda la altura:

- `A_1 = 0.85×2.00 + 0.25×4.00 = 2.700 m²`
- `A_2 = 0.85×2.50 + 0.25×6.25 = 3.688 m²`
- `V = 100 × (2.700 + 3.688)/2 = 319.38 m³`

Diferencia: **+67 %**. Es la razón por la que el talud no puede ser un supuesto oculto: debe ser un dato con trazabilidad al estudio geotécnico.

**Pozo.** `D_int = 1.20 m`, `e = 0.15 m`, `s = 0.30 m`, `H_p = 3.00 m`:

- `D_exc = 1.20 + 0.30 + 0.60 = 2.10 m`
- `A_exc = π/4 × 2.10² = 3.464 m²`
- `V_pozo = 3.464 × 3.00 = 10.39 m³`

### 7.9 Integración con el modelo

Cada arco produce un registro `VolumenExcavacion` con:

| Campo | Contenido |
|---|---|
| `arco_id` | Referencia al tramo |
| `metodo` | `areas_medias` / `prismoidal` |
| `segmentos[]` | `L`, `H_1`, `H_2`, `b`, `z`, `A_1`, `A_2`, `V` |
| `capas` | `suelo`, `roca`, `saturado` con su `V` |
| `v_cama`, `v_relleno_importado`, `v_relleno_propio`, `v_tubo`, `v_sobrante` | m³ |
| `hash_insumos` | Superficie + cotas + diámetros + suelos + parámetros |
| `version_code_pack` | |

**Reglas de integración:**

1. El volumen es **derivado**. Si el usuario edita la cota clave o el diámetro, el volumen se recalcula; no hay editable manual.
2. Si la superficie cambia de versión, todos los volúmenes quedan "desactualizados" hasta recálculo; el estado se muestra.
3. Los totales se agrupan por: red, familia, diámetro, rango de profundidad (< 1.5 m, 1.5–3 m, 3–5 m, > 5 m), tipo de suelo y tipo de obra (zanja / pozo / estructura).
4. El volumen de excavación **no** incluye demolición de pavimento, reposición, ni transporte; son ítems separados (v1.1) con su propia geometría.

---

## 8. Topografía y superficie de levantamiento

### 8.1 Puntos

Cada punto del levantamiento:

| Campo | Tipo | Unidad | Obligatorio | Nota |
|---|---|---|---|---|
| `id` | UUID | — | sí | |
| `este` | decimal | m | sí | En el CRS del proyecto |
| `norte` | decimal | m | sí | |
| `cota` | decimal | m | sí | Ortométrica |
| `codigo` | texto | — | sí | `TER`, `BOR`, `EJE`, `TAPA`, `POSTE`, `AGUA`, `ALC` |
| `descripcion` | texto | — | no | |
| `precision_xy` | decimal | m | no | |
| `precision_z` | decimal | m | no | |
| `fecha` | fecha | — | no | |
| `fuente` | enum | — | sí | `gnss_rtk`, `estacion_total`, `nivelacion`, `digitalizacion`, `importado` |
| `importado_de` | texto | — | no | Archivo, equipo, operador |

**Requisitos mínimos:** ≥ 4 puntos por hectárea en terreno urbano plano; más en terreno quebrado; puntos en todos los cambios de pendiente, bordes de calzada, tapas de pozo existentes (con cota de tapa y, si se puede, de fondo), cauces y líneas de agua.

### 8.2 Líneas de rotura y TIN

- **Líneas de rotura obligatorias:** bordes de calzada, cunetas, cauces, pie y corona de taludes, muros, bordes de terraza. Sin ellas, el TIN "redondea" los quiebres y la excavación sale mal.
- **TIN:** triangulación de Delaunay **con restricciones** (constrained Delaunay) para que las líneas de rotura sean aristas. Algoritmo recomendado: inserción incremental con flip legal sobre aristas no restringidas; alternativa: divide y vencerás.
- **Orientación:** todos los triángulos en sentido antihorario; normal hacia arriba.
- **Prohibiciones:** triángulos con área < 0.01 m²; aristas que crucen una línea de rotura; huecos triangulados como si fueran terreno (los huecos son huecos).
- **Cotas de tapa vs. cota de terreno:** una tapa de pozo existente es un punto con dos cotas (tapa y fondo, si se midió). El TIN usa la tapa; el fondo es dato del pozo.

**Control de calidad del TIN**

| Chequeo | Umbral (parámetro) | Acción |
|---|---|---|
| Ángulo mínimo de triángulo | ≥ 10° | Advertencia |
| Relación de aspecto (circunradio/inradio) | ≤ 5 | Advertencia |
| Longitud máxima de arista | ≤ 3 × mediana | Advertencia |
| Puntos duplicados (misma E,N, distinto Z) | 1 mm | Error |
| Puntos casi colineales | área < 0.01 m² | Error |
| Cota atípica (respecto de la mediana de vecinos) | desviación > 3·MAD | Advertencia |
| Cobertura de objetos | 100 % dentro de la envolvente convexa | Error si hay extrapolación |

### 8.3 Sistemas de referencia (CRS)

**Ecuador continental usa principalmente UTM zona 17 S.** El ejemplo `EPSG:32617` de la solicitud corresponde a **UTM 17 N**, que solo cubre la parte continental al **norte del ecuador** (Esmeraldas, Carchi, Imbabura, norte de Pichincha). Usarlo para Quito, Guayaquil o Cuenca no produce un error visible de magnitud: produce un desplazamiento de aproximadamente **10 000 000 m en el norte**, porque el hemisferio invierte el falso norte. El síntoma es que el proyecto "salta" a otra coordenada sin que nadie lo note hasta que se superpone con cartografía.

| CRS | EPSG | Uso | Estado |
|---|---|---|---|
| WGS 84 / UTM zone 17S | 32717 | Mayor parte del Ecuador continental (Costa y Sierra) | **Recomendado por defecto** |
| WGS 84 / UTM zone 18S | 32718 | Oriente / Amazonía | Soporte obligatorio |
| WGS 84 / UTM zone 17N | 32617 | Norte del país (sobre el ecuador) | Soporte obligatorio |
| WGS 84 / UTM zone 15S / 16S | 32715 / 32716 | Galápagos | Soporte opcional v1 |
| PSAD56 (geográfico) | 4248 | Cartografía histórica IGM | Solo lectura/importación |
| PSAD56 / UTM 17S | a verificar en EPSG | Cartografía histórica proyectada | Solo lectura |
| SIRGAS 2000 (geográfico) | 4674 | Marco moderno | Solo lectura |
| WGS 84 (geográfico) | 4326 | Intercambio (GeoJSON, GPS) | Intercambio |

**Reglas:**

1. Un proyecto tiene **un CRS de trabajo** declarado; todos los objetos se guardan en él.
2. La importación de otra fuente exige transformación explícita, registrada (`desde_crs`, `hacia_crs`, `método`, `parámetros`).
3. **Datum vertical:** las cotas deben ser ortométricas respecto del datum vertical oficial (nivel medio del mar; el mareógrafo de referencia debe confirmarse en el code pack). Si el levantamiento es GNSS puro, hay que aplicar la ondulación del geoide; la app debe exigir declarar `tipo_cota ∈ {ortometrica, elipsoidal}` y **rechazar** mezclas.
4. Tolerancia de transformación: error horizontal ≤ 0.05 m y vertical ≤ 0.02 m; si se supera, advertencia.

### 8.4 Interpolación de la cota en un punto

Dentro de un triángulo de vértices `A, B, C`, la superficie es el plano:

`Z(x,y) = Z_A + u·(Z_B − Z_A) + v·(Z_C − Z_A)`

donde `(u, v)` son las coordenadas baricéntricas de `(x,y)` respecto de `A` y los vectores `B−A`, `C−A`:

`u = ((x−x_A)(y_C−y_A) − (y−y_A)(x_C−x_A)) / det`
`v = ((y−y_A)(x_B−x_A) − (x−x_A)(y_B−y_A)) / det`

con `det = (x_B−x_A)(y_C−y_A) − (y_B−y_A)(x_C−x_A)`.

**Reglas:** `u ≥ 0`, `v ≥ 0`, `u+v ≤ 1` con tolerancia `1×10⁻⁹`. Fuera del TIN: extrapolación solo si el usuario lo autoriza, y el objeto se marca `z_extrapolada = true`. La búsqueda del triángulo debe ser `O(log n)` con índice espacial (R-tree o walking).

**Precisión esperada:** la cota interpolada no es mejor que la del levantamiento. Si `exactitud_z` del punto es 0.03 m, la cota interpolada tiene esa incertidumbre; la app debe propagarla al volumen (ver §8.7).

### 8.5 Curvas de nivel

- Generación por intersección de cada triángulo con el plano `Z = c`: se obtiene un segmento por triángulo; se unen segmentos que comparten arista.
- Intervalo equidistante (0.25, 0.50, 1.00 m) o variable; curvas maestras cada 5 intervalos.
- Suavizado **solo para dibujo**; el cálculo de volumen usa el TIN, nunca las curvas.
- Etiquetado: cota en el punto medio del segmento, orientación legible.

### 8.6 Capas de superficie y vinculación con los objetos

Un proyecto puede tener varias superficies:

| Capa | Contenido | Uso |
|---|---|---|
| `terreno` | Levantamiento | Excavación, cotas de terreno |
| `rasante` | Proyecto vial / urbanización | Recubrimiento, cotas de acabado |
| `roca` | Techo del estrato rocoso | Clasificación de excavación |
| `freático` | Nivel freático | Excavación bajo agua, entibado |
| `diseño` | Superficie modificada por el proyecto | Movimiento de tierras global |

**Vinculación:**

- Cada objeto guarda `superficie_id` + `version_superficie` + `hash_superficie`.
- La cota se **deriva** al crear o al mover el objeto, y se congela; si la superficie cambia, el objeto se marca "cota desactualizada" y ofrece recalcular.
- El usuario puede fijar una cota manual (`z_manual`), pero debe declarar `motivo` (p. ej. "cota de proyecto impuesta"). La app muestra el ícono de override en todo informe.
- Un objeto no puede referenciar dos superficies para la misma cota. Si se necesita (p. ej., terreno y rasante), son dos atributos distintos.

### 8.7 Propagación de incertidumbre al volumen

El volumen de un tramo depende de `H_1` y `H_2`, que dependen de cotas interpoladas. Propagación de primer orden:

`σ_V ≈ L·(b + 2z·H̄)·σ_H` (paredes trapezoidales, `H̄` media)

Con `σ_H ≈ 0.05 m` (levantamiento + interpolación), un tramo de 100 m con `b = 0.85` y `H̄ = 2.25 m` tiene `σ_V ≈ 100 × 0.85 × 0.05 = 4.25 m³` sobre 191 m³ (< 2.5 %). La app debe mostrar este orden de magnitud cuando el volumen se use para presupuesto.

---

## 9. Reglas de validación

### 9.1 Formato de una regla

Toda regla es declarativa y testeable:

`id` — `ámbito` — `predicado` — `severidad` — `parámetro` / `referencia` — `mensaje`

**Severidades:**

| Severidad | Efecto |
|---|---|
| `BLOQUEANTE` | Impide emitir el informe de diseño y el listado de volúmenes como "válido" |
| `ERROR` | El diseño se marca "no conforme"; se puede exportar con la marca |
| `ADVERTENCIA` | Se exporta; requiere justificación del diseñador |
| `INFO` | Solo informativo |

**Regla de emisión:** un diseño es "conforme" solo si no hay `BLOQUEANTE` ni `ERROR`, el code pack está `ACTIVE` y todas las reglas tienen parámetro resuelto. Si falta un parámetro, la regla queda `NO_VERIFICABLE` y el informe no puede decir "conforme".

### 9.2 Catálogo de reglas

#### Agua potable

| ID | Regla verificable | Severidad | Parámetro |
|---|---|---|---|
| VAL-AP-001 | En el escenario de demanda máxima, `H_GL − Z_nodo ≥ P_min` en todo nodo de consumo | BLOQUEANTE | `p_min_servicio` = 10 mca |
| VAL-AP-002 | `Z_tanque,NAMO − Z_nodo ≤ P_max` (presión estática) en todo nodo | BLOQUEANTE | `p_max_estatica` = 70 mca |
| VAL-AP-003 | `0.60 ≤ v ≤ 3.00 m/s` en todo arco en operación normal | ADVERTENCIA | `v_min`, `v_max` |
| VAL-AP-004 | `v ≤ 5.00 m/s` en el escenario transitorio | ERROR | `v_max_transitorio` |
| VAL-AP-005 | `P_max_transitorio ≤ PN` del tubo | BLOQUEANTE | `factor_pn` = 1.0 |
| VAL-AP-006 | Todo nodo de consumo es alcanzable desde al menos una fuente | BLOQUEANTE | — |
| VAL-AP-007 | `DN ≥ 50 mm` en red de distribución (63 mm recomendado) | ERROR | `dn_min_red` |
| VAL-AP-008 | `DN ≥ 13 mm` en acometida | ERROR | `dn_min_acometida` |
| VAL-AP-009 | Recubrimiento `Z_R − (Z_C + D) ≥ 0.80 m`; `≥ 1.20 m` bajo calzada | BLOQUEANTE | `rec_min`, `rec_min_via` |
| VAL-AP-010 | Tramo sin válvula de seccionamiento ≤ 500 m | ADVERTENCIA | `l_max_sin_valvula` |
| VAL-AP-011 | Se calculan y reportan los dos escenarios de tanque (`NAMI` y `NAMO`) | BLOQUEANTE | — |
| VAL-AP-012 | `DN` no aumenta aguas abajo en un ramal, salvo justificación registrada | INFO | — |
| VAL-AP-013 | `J ≤ 0.02 m/m` (20 m/km) en red de distribución, salvo justificación | ADVERTENCIA | `j_max_red` |
| VAL-AP-014 | Con `Q_inc` simultáneo, la presión en el hidrante más desfavorable `≥ 10 mca` | ERROR | `p_min_incendio` |
| VAL-AP-015 | Ningún nodo queda con un único camino de suministro si es sector crítico | INFO | — |
| VAL-AP-016 | `S` declarada coincide con `(Z_C,inicio − Z_C,fin)/L` dentro de ±0.0005 | ERROR | `tol_pendiente` |
| VAL-AP-017 | `PN ≥ P_max_estática + ΔH_joukowsky` | BLOQUEANTE | — |
| VAL-AP-018 | Pérdida local total en impulsión ≤ 15 % de `h_f`; si no, revisar diseño | INFO | — |

#### Alcantarillado

| ID | Regla verificable | Severidad | Parámetro |
|---|---|---|---|
| VAL-AL-001 | `S ≥ 0.5 %` para `DN ≤ 200`; `S ≥ 0.3 %` para `DN > 200` | BLOQUEANTE | `s_min_200`, `s_min_mayor` |
| VAL-AL-002 | `y/D ≤ 0.85` al caudal de diseño | BLOQUEANTE | `llenado_max` |
| VAL-AL-003 | `v ≥ 0.60 m/s` al caudal de diseño (0.75 recomendado) | ERROR | `v_min` |
| VAL-AL-004 | `v ≤ 5.00 m/s` (3.00–4.00 en PVC con abrasivos) | ERROR | `v_max` |
| VAL-AL-005 | `τ = 9 810·R·S ≥ 1.0 Pa` | ERROR | `tau_min` |
| VAL-AL-006 | `DN ≥ 200 mm` en colector; `DN ≥ 150 mm` en acometida | ERROR | `dn_min_colector` |
| VAL-AL-007 | `Z_C` decrece estrictamente aguas abajo en cada tramo | BLOQUEANTE | — |
| VAL-AL-008 | El grafo de AL no contiene ciclos | BLOQUEANTE | — |
| VAL-AL-009 | Distancia entre pozos consecutivos ≤ 120 m | ADVERTENCIA | `dist_max_pozos` |
| VAL-AL-010 | Cambio de dirección > 45° implica pozo | ERROR | `ang_max_sin_pozo` |
| VAL-AL-011 | Cambio de diámetro o de material implica pozo | ERROR | — |
| VAL-AL-012 | Profundidad de pozo ≤ 6.00 m; si mayor, marcar "diseño especial" | ADVERTENCIA | `h_max_pozo` |
| VAL-AL-013 | Si `ΔZ` en pozo > 0.50 m, el subtipo debe ser pozo de caída | ERROR | `dz_max_sin_caida` |
| VAL-AL-014 | `Z_C,salida ≤ Z_C,entrada` en todo pozo | BLOQUEANTE | — |
| VAL-AL-015 | `Fr ≤ 0.80` en el tramo, o análisis de resalto adjunto | ADVERTENCIA | `fr_max` |
| VAL-AL-016 | Recubrimiento `≥ 0.80 m`; `≥ 1.20 m` bajo calzada | BLOQUEANTE | `rec_min` |
| VAL-AL-017 | `Q_diseño ≤ Q_o` (capacidad a tubo lleno) | ERROR | — |
| VAL-AL-018 | Conexiones en un pozo ≤ 6 | ADVERTENCIA | `n_max_conexiones` |
| VAL-AL-019 | `S ≤ 15 %`; si mayor, exige estructura de disipación | ADVERTENCIA | `s_max` |
| VAL-AL-020 | El emisario descarga sobre el nivel de agua del receptor o con difusor | ERROR | — |
| VAL-AL-021 | Ningún colector de cota menor recibe a otro de cota mayor sin caída | BLOQUEANTE | — |
| VAL-AL-022 | `Q_diseño ≥ Q_min` (1.5 L/s) para verificar arrastre | INFO | `q_min` |

#### Excavación

| ID | Regla verificable | Severidad | Parámetro |
|---|---|---|---|
| VAL-EX-001 | `b ≥ 0.60 m` y `b ≥ D_ext + 0.60 m` | ERROR | `c_lateral` |
| VAL-EX-002 | `H ≥ 0.60 m` (profundidad mínima constructiva) | ERROR | `h_min` |
| VAL-EX-003 | `H > 1.50 m` en suelo no cohesivo exige entibado o talud declarado | BLOQUEANTE | `h_entibado` |
| VAL-EX-004 | `z ≤ z_max(tipo_suelo)` | ERROR | tabla §7.4 |
| VAL-EX-005 | `H > 3.00 m` exige bermas o entibado diseñado | ADVERTENCIA | `h_berma` |
| VAL-EX-006 | `h_cama ≥ 0.10 m` (`≥ 0.15` en roca) | ERROR | `h_cama` |
| VAL-EX-007 | Si `Z_F < Z_freático`, marcar "excavación bajo agua" y requerir agotamiento | ADVERTENCIA | — |
| VAL-EX-008 | Volumen de pozo se reporta separado del de zanja | ERROR | — |
| VAL-EX-009 | `V_exc = V_relleno + V_tubo + V_sobrante` dentro de ±0.5 % | ERROR | `tol_balance` |
| VAL-EX-010 | Todo tramo tiene `tipo_suelo` asignado | ADVERTENCIA | — |

#### Topografía

| ID | Regla verificable | Severidad | Parámetro |
|---|---|---|---|
| VAL-TOP-001 | Todo objeto está dentro de la envolvente del TIN (o marcado como extrapolado) | ERROR | — |
| VAL-TOP-002 | No hay puntos duplicados (misma E,N, distinto Z) | ERROR | `tol_xy` = 0.001 m |
| VAL-TOP-003 | No hay triángulos con área < 0.01 m² ni ángulo < 5° | ERROR | — |
| VAL-TOP-004 | CRS del proyecto declarado y único; importaciones con transformación registrada | BLOQUEANTE | — |
| VAL-TOP-005 | `tipo_cota` homogéneo (no mezclar ortométrica y elipsoidal) | BLOQUEANTE | — |
| VAL-TOP-006 | Las líneas de rotura son aristas del TIN | ERROR | — |
| VAL-TOP-007 | `exactitud_z` declarada para todo punto usado en excavación | ADVERTENCIA | — |
| VAL-TOP-008 | Densidad mínima de puntos por hectárea | ADVERTENCIA | `dens_min` |

#### Generales

| ID | Regla verificable | Severidad | Parámetro |
|---|---|---|---|
| VAL-GEN-001 | `id` y `codigo` únicos en el modelo | BLOQUEANTE | — |
| VAL-GEN-002 | No hay arcos colgantes (extremo sin nodo) | BLOQUEANTE | — |
| VAL-GEN-003 | No hay nodos huérfanos sin arcos | ERROR | — |
| VAL-GEN-004 | Toda componente conexa alcanza una fuente y (en AP) un consumo | BLOQUEANTE | — |
| VAL-GEN-005 | Magnitudes dentro de rango físico (D > 0, e < D/2, 0 < S < 1) | BLOQUEANTE | — |
| VAL-GEN-006 | Para emitir "conforme", el code pack está `ACTIVE` | BLOQUEANTE | — |
| VAL-GEN-007 | Ningún objeto AP comparte nodo con AL | BLOQUEANTE | — |
| VAL-GEN-008 | `Z_C,inicio − Z_C,fin` coherente con la longitud y la pendiente | ERROR | `tol_pendiente` |
| VAL-GEN-009 | Año de instalación ≤ año actual + 5; fechas coherentes | ADVERTENCIA | — |
| VAL-GEN-010 | Todo objeto tiene `estado` y `propietario` | ERROR | — |

### 9.3 Ejemplo de aplicación de una regla

`VAL-AL-002` sobre el ejemplo de §5.2.9: `y/D = 0.586 ≤ 0.85` ⇒ cumple. Si el caudal fuera 30 L/s con `Q_o = 23.19 L/s`, la regla no se evalúa como "0.85" sino que dispara `VAL-AL-017` ("capacidad insuficiente") y la app no debe reportar un tirante. **Orden de evaluación:** capacidad → llenado → velocidad → tractriz → pendiente. Evaluar en otro orden produce mensajes engañosos.

---

## 10. Fuera de alcance para v1

| # | Excluido | Por qué | Qué se deja preparado |
|---|---|---|---|
| 1 | Diseño hidrológico e hidráulico pluvial completo | Requiere IDF, hietogramas, método racional, tránsito; es otro producto | Objetos F13 (sumideros) y F14 (canales) ya existen; el motor pluvial es v2 |
| 2 | Transitorios completos (MOC, separación de columna, cavitación) | Costo de implementación alto; el 90 % de los casos se cubre con Joukowsky/Michaud | Interfaz `AnalisisTransitorio` con implementación `Joukowsky` y margen para `MOC` |
| 3 | Calidad del agua (cloro residual, edad, trazadores) | No es requisito del cliente; requiere simulación extendida | Atributos de calidad ya existen en F07; motor en v2 |
| 4 | Diseño estructural de tuberías, pozos, tanques y anclajes | Es cálculo estructural, no de redes | Familia F17 y atributos geométricos permiten exportar a un motor estructural |
| 5 | Diseño de procesos de PTAP/PTAR (coagulación, lodos, aireación) | Es ingeniería de procesos; el nodo F07 solo representa capacidad | F07 con `tren_proceso` y calidad de salida |
| 6 | Selección y optimización de bombas, eficiencia energética | Requiere catálogo de fabricantes y optimización | F16 con curva H–Q y punto de operación ya en v1 |
| 7 | Costos, APU, presupuesto, cronograma | Depende de precios locales y del formato del cliente | Volúmenes y longitudes ya salen separados por ítem |
| 8 | Planos, CAD, DXF, IFC, impresión | Es un producto de dibujo; Rune no es un CAD | Modelo con geometría completa exportable |
| 9 | GIS completo, LiDAR, nubes de puntos masivas | Otro dominio de software | Importación de puntos y TIN ya cubiertos |
| 10 | Gestión de activos, inspección CCTV, ISO 55000 | Es operación, no diseño | Atributos `estado`, `año`, `vida_util` ya existen |
| 11 | Multiusuario, colaboración en tiempo real, nube | La app es de escritorio | Modelo con `id` UUID y versionado permite sincronización futura |
| 12 | Normativa de países distintos de Ecuador | El cliente opera en Ecuador | Code pack por jurisdicción permite añadir otros |
| 13 | Instalaciones interiores y grifería ("llaves" como grifos) | No es red; es proyecto de edificio | Si el cliente confirma esa acepción, se declara y se excluye (ADR-01) |
| 14 | Modelado 3D/BIM y render | No aporta al cálculo | Sólidos simples ya previstos para pozos y tanques |

**Criterio de exclusión:** se excluye lo que (a) no es necesario para calcular volúmenes y verificar cumplimiento, o (b) requiere datos que el cliente no tiene. Todo lo excluido debe tener una "puerta" en el modelo para no bloquear la v2.

---

## 11. Implicaciones para el modelo de dominio en Rust

Sin escribir código, estas son las decisiones que el implementador debe respetar.

### 11.1 Tipos de magnitud (newtypes)

| Concepto | Tipo | Invariante en construcción |
|---|---|---|
| Longitud / cota | `Metro(f64)` | finito, `-500 ≤ z ≤ 10 000` |
| Diámetro nominal | `DnMilimetros(u16)` | > 0, pertenece al catálogo o se marca "no comercial" |
| Diámetro interior/exterior | `Metro(f64)` | `0 < D_int < D_ext` |
| Caudal | `Caudal(f64)` | ≥ 0, en m³/s |
| Velocidad | `Velocidad(f64)` | ≥ 0, en m/s |
| Pendiente | `Pendiente(f64)` | `0 ≤ s < 1` (una pendiente negativa es un error de captura, no un valor) |
| Presión | `Presion(f64)` | ≥ −10 mca, en mca |
| Rugosidad | `Rugosidad(f64)` | > 0 |
| Ángulo | `Radianes(f64)` | normalizado a `[0, 2π)` |
| Año | `Anio(u16)` | 1900 … actual+5 |

**Regla:** la API pública nunca recibe `f64` suelto para una magnitud física. El constructor valida y devuelve `Result`. Esto elimina la clase de error "pasé mm donde iban metros".

### 11.2 Enumeraciones cerradas

- `Familia` (F01…F17) y un `Subtipo` por familia. Los subtipos son `enum`, no `String`: permiten `match` exhaustivo y serialización estable.
- `Material`, `EstadoElemento`, `TipoSuelo`, `TipoJunta`, `ClasePresion`, `Severidad`, `MetodoFriccion`, `RolHidraulico`.
- Los catálogos extensibles (materiales, diámetros comerciales) son **datos**, no enums: se cargan de un archivo versionado. Un enum obligaría a recompilar para añadir un diámetro.

### 11.3 Representación del elemento

Un `enum` cuyas variantes son las 17 familias, cada una envolviendo su estructura propia, con métodos comunes (`familia()`, `geometria()`, `cotas()`). Ventajas: `match` exhaustivo, serialización simple, sin *downcasting*. Desventajas: el `enum` crece con cada familia; se mitiga con módulos por familia.

**No** usar `Box<dyn Trait>` como representación primaria: complica la serialización, la comparación y el versionado. Los *traits* se usan para comportamiento (`CalculaHidraulica`, `AportaVolumen`), no para almacenamiento.

### 11.4 Grafo

- Almacenamiento por índices estables (`slotmap` o `Vec` + generación), no por punteros.
- `Nodo { id, punto, rol, elemento_id }`, `Arco { id, nodo_inicio, nodo_fin, elemento_id, sentido }`.
- Invariantes verificados al construir y al editar: sin arcos colgantes, sin nodos huérfanos, sin cruces AP/AL.
- **Determinismo:** iterar siempre sobre estructuras ordenadas (`BTreeMap` o `Vec` ordenado por id). Un `HashMap` produce resultados distintos entre ejecuciones y rompe la reproducibilidad de los informes.

### 11.5 Validación y errores

- `Hallazgo { regla_id, severidad, elemento_id, mensaje, valor_observado, valor_limite, code_pack_hash }`.
- `validar(modelo, code_pack) -> Vec<Hallazgo>` puro, sin IO, testeable.
- Errores de dominio con `thiserror`; sin `panic!` en rutas de usuario. Un `panic` en cálculo hidráulico debe ser un `Result`.
- Tolerancias explícitas: `EPS_LONGITUD = 1e-6 m`, `EPS_CAUDAL = 1e-9 m³/s`, `EPS_PENDIENTE = 1e-9`.

### 11.6 Separación en capas (crates)

| Capa | Responsabilidad | Depende de |
|---|---|---|
| `dominio` | Tipos, invariantes, grafo, reglas declarativas | nada |
| `hidraulica` | HW, DW, Manning, GGA, transitorio | `dominio` |
| `excavacion` | Zanja, pozo, capas, balance | `dominio` |
| `topografia` | Puntos, TIN, interpolación, CRS | `dominio` |
| `normativo` | Code pack, carga, verificación | `dominio` |
| `persistencia` | Serialización, migraciones | `dominio` |
| `ui` / `cli` | Rune / terminal | todas |

La capa `dominio` no debe depender de Rune ni de IO. Esto permite testear el 80 % del valor sin abrir una ventana.

---

## 12. Backlog derivado

### Épica E1 — Topografía

| ID | Feature | Depende de | Aceptación |
|---|---|---|---|
| F-01 | Alta, edición e importación de puntos (CSV, TXT, DXF de puntos) | — | VAL-TOP-002, VAL-TOP-007 |
| F-02 | TIN con constrained Delaunay y líneas de rotura | F-01 | VAL-TOP-003, VAL-TOP-006 |
| F-03 | Curvas de nivel y etiquetado | F-02 | Verificación visual + no altera el TIN |
| F-04 | CRS: selección, transformación y registro | F-01 | VAL-TOP-004, VAL-TOP-005 |
| F-05 | Capas de superficie (terreno, rasante, roca, freático) | F-02 | Un objeto lee la capa correcta |
| F-06 | Interpolación de cota con índice espacial | F-02 | Precisión ≤ 1 mm en el triángulo; `O(log n)` |
| F-07 | Control de calidad del levantamiento (duplicados, outliers, densidad) | F-01 | VAL-TOP-002, VAL-TOP-008 |

### Épica E2 — Modelo de objetos

| ID | Feature | Depende de | Aceptación |
|---|---|---|---|
| F-10 | Catálogo de familias y subtipos (F01–F17) | — | Serialización estable; match exhaustivo |
| F-11 | Grafo nodo–arco con invariantes | F-10 | VAL-GEN-002 … 004 |
| F-12 | Atributos por familia y validación de tipos | F-10 | §4 completo |
| F-13 | Catálogos de materiales, diámetros, tapas, estados | F-10 | Carga desde archivo versionado |
| F-14 | Edición con deshacer/rehacer y auditoría | F-11 | Toda edición deja `revision` |
| F-15 | Escenarios (válvula cerrada, demanda pico, incendio) | F-11 | Conmutables y reportables |

### Épica E3 — Hidráulica de agua potable

| ID | Feature | Depende de | Aceptación |
|---|---|---|---|
| F-20 | Cálculo de caudales de diseño (población, dotación, K1, K2, incendio) | F-12 | Cuatro métodos de proyección comparables |
| F-21 | Motor de fricción HW y DW (Colebrook + Swamee-Jain + Haaland) | F-20 | Vectores de §D |
| F-22 | Resolución de red (Todini/GGA) con convergencia reportada | F-21 | Malla de prueba con solución conocida |
| F-23 | Pérdidas locales y su peso relativo | F-21 | Tabla §5.1.3 |
| F-24 | Verificación de presiones y velocidades, dos escenarios de tanque | F-22 | VAL-AP-001, 002, 011 |
| F-25 | Golpe de ariete (celeridad, Joukowsky, Michaud) | F-21 | Vector §5.1.7 |
| F-26 | Bombeo: curva, punto de operación, potencia, NPSH | F-22 | Curva de prueba |
| F-27 | Diámetro económico y selección de diámetro comercial | F-22 | Rango de `v_eco` y `K_b` |

### Épica E4 — Hidráulica de alcantarillado

| ID | Feature | Depende de | Aceptación |
|---|---|---|---|
| F-30 | Caudales de aporte, infiltración y factor de punta (Harmon) | F-12 | Curva M(P) verificada |
| F-31 | Manning, tubo lleno y flujo parcial con tabla `q/Q`, `v/V` | F-30 | Vectores §5.2.3 y §5.2.4 |
| F-32 | Tensión tractriz y pendiente mínima derivada | F-31 | Vector §5.2.5 |
| F-33 | Perfil hidráulico y remanso (paso estándar) | F-31 | Caso con control aguas abajo |
| F-34 | Pozos: caídas, pérdidas por dirección, cota de salida | F-33 | VAL-AL-013, 014 |
| F-35 | Verificación completa de reglas AL | F-31…F-34 | VAL-AL-001 … 022 |

### Épica E5 — Excavación

| ID | Feature | Depende de | Aceptación |
|---|---|---|---|
| F-40 | Zanja: ancho por diámetro, profundidad por cotas, talud por suelo | F-05, F-12 | Vector §7.8 (191.25 y 319.38 m³) |
| F-41 | Pozo como prisma con sobreancho y talud por franjas | F-40 | Vector §7.8 (10.39 m³) |
| F-42 | Estratigrafía: suelo, roca, saturado; volumen por capa | F-40 | Capas suman el total |
| F-43 | Balance de tierras: cama, relleno, tubo, esponjamiento, sobrante | F-40 | VAL-EX-009 |
| F-44 | Reportes por diámetro, profundidad, suelo y tipo de obra | F-43 | Totales cuadran con el detalle |
| F-45 | Incertidumbre del volumen por precisión del levantamiento | F-06, F-43 | §8.7 implementado |

### Épica E6 — Normativo

| ID | Feature | Depende de | Aceptación |
|---|---|---|---|
| F-50 | Esquema y validador de code pack (§6.4) | — | Pack incompleto ⇒ `BLOCKED_BY_SOURCE` |
| F-51 | Carga de packs por jurisdicción y selección | F-50 | Dos packs coexisten |
| F-52 | Motor de reglas declarativas con severidad | F-12, F-50 | Las 68 reglas de §9.2 ejecutables |
| F-53 | Informe de cumplimiento con trazabilidad | F-52 | Cada hallazgo cita regla + pack + hash |
| F-54 | Resolución de las 11 incertidumbres de §6.5 | F-51 | Cada una con fuente y fecha |

### Épica E7 — Interfaz (Rune) y CLI

| ID | Feature | Depende de | Aceptación |
|---|---|---|---|
| F-60 | Vista de planta con objetos y cotas | F-11 | Selección y edición |
| F-61 | Vista de perfil longitudinal (terreno, clave, rasante) | F-60 | Cota y pendiente por tramo |
| F-62 | Panel de atributos por familia | F-12 | Campos obligatorios bloquean guardado |
| F-63 | Panel de validación con navegación al objeto | F-52 | Clic lleva al elemento |
| F-64 | Reporte de volúmenes exportable (CSV) | F-44 | Cuadra con la UI |
| F-70 | CLI para agentes: `validar`, `calcular`, `volumenes`, `norma` | F-52, F-44 | Salida estable (JSON) y códigos de salida |

---

## Anexo A — Tablas de referencia rápida

### A.1 Diámetros comerciales (mm)

Agua potable: 20, 25, 32, 40, 50, 63, 75, 90, 110, 160, 200, 250, 315, 355, 400, 450, 500, 630, 710, 800.
Alcantarillado: 110, 160, 200, 250, 315, 355, 400, 450, 500, 600, 710, 800, 900, 1 000, 1 200.

### A.2 Coeficientes de Manning por material (referencia; el code pack manda)

| Material | `n` mín | `n` típico | `n` máx |
|---|---|---|---|
| PVC-U liso | 0.009 | 0.010 | 0.011 |
| PEAD | 0.009 | 0.010 | 0.011 |
| GRP | 0.010 | 0.011 | 0.012 |
| Hormigón in situ, terminación buena | 0.012 | 0.013 | 0.014 |
| Hormigón prefabricado | 0.012 | 0.013 | 0.015 |
| Hormigón con sedimentos | 0.013 | 0.015 | 0.016 |
| Mampostería de ladrillo | 0.014 | 0.015 | 0.017 |
| Metal corrugado | 0.021 | 0.024 | 0.026 |
| Tierra (canal) | 0.022 | 0.025 | 0.030 |
| Roca (canal) | 0.030 | 0.035 | 0.040 |

### A.3 Coeficiente `C` de Hazen-Williams

| Material | `C` (nuevo) | `C` (20 años) |
|---|---|---|
| PVC / PEAD | 150 | 140 |
| Hierro dúctil con revestimiento | 140 | 130 |
| Acero con revestimiento | 140 | 120 |
| Acero sin revestimiento | 120 | 90 |
| Hormigón | 130 | 120 |
| Fibrocemento | 140 | 130 |

### A.4 Rugosidad absoluta `k` (mm)

| Material | `k` |
|---|---|
| PVC, PEAD, GRP | 0.0015–0.007 |
| Hierro dúctil nuevo | 0.10–0.15 |
| Hierro dúctil usado | 0.20–0.30 |
| Acero nuevo | 0.045–0.10 |
| Acero con incrustación | 0.30–1.50 |
| Hormigón | 0.30–3.00 |

---

## Anexo B — Decisiones abiertas (ADR pendientes)

| ADR | Decisión | Opciones | Recomendación | Impacto si no se resuelve |
|---|---|---|---|---|
| ADR-01 | ¿Qué es una "llave"? | (a) válvula domiciliaria, (b) conjunto de acometida, (c) grifería, (d) hidrante | (b) + (d); F04 = red, F05 = frontera | Se duplica la familia válvulas o se modela mal la acometida |
| ADR-02 | ¿Qué es un "vertedero"? | (1) emisario/descarga, (2) alivio, (3) aliviadero de tanque, (4) relleno sanitario | (1)+(2)+(3) en F03; (4) fuera de alcance | Familia mal definida y reglas de descarga inaplicables |
| ADR-03 | Fuente del code pack inicial | CPE INEN 5, IEOS, manual de un prestador | Manual del prestador + CPE/IEOS como método | No hay valores defendibles |
| ADR-04 | Método de fricción por defecto en AP | HW o DW | HW para diseño, DW para verificación e impulsiones | Resultados inconsistentes entre módulos |
| ADR-05 | Motor de red AP | Hardy Cross o Todini | Todini (GGA) | Convergencia pobre en redes grandes |
| ADR-06 | Superficies múltiples en v1 | Solo terreno / terreno + rasante + roca | Terreno + rasante + roca | Excavación y recubrimiento incorrectos |
| ADR-07 | Unidad de presentación de pendiente | % o m/m o ‰ | % en UI, m/m en datos | Errores de 100× |
| ADR-08 | ¿Alcantarillado pluvial en v1? | Sí / no | No (solo objetos) | Se subestima el alcance |
| ADR-09 | Jurisdicción inicial del pack | Un cantón / varios | Uno piloto + estructura multi-pack | Reglas contradictorias |

---

## Anexo C — Referencias

**Ecuador**

- Constitución de la República del Ecuador (2008), arts. 12, 264, 314.
- Ley Orgánica de Recursos Hídricos, Usos y Aprovechamiento del Agua (LORHUAA), 2014, y su Reglamento General.
- CPE INEN 5 — Instalaciones Sanitarias (Código Ecuatoriano de la Construcción). *Edición y vigencia a verificar.*
- Normas del IEOS para diseño de sistemas de agua potable y alcantarillado. *Edición a verificar.*
- NTE INEN 1108 — Agua potable. Requisitos.
- Ordenanzas municipales y manuales de diseño de los prestadores (EPMAPS, ETAPA EP, Interagua, EMAAP-Q y otros).

**Internacional**

- AWWA M32, M22, M11, M23, M41, M55; AWWA C900, C905, C502, C504, C508, C509, C512.
- EN 752, EN 805, EN 124, EN 545, EN 1401, EN 1610, EN 1916, EN 1917, EN 12666, EN 12201.
- ISO 2531, ISO 1452, ISO 4064, ISO 24510, ISO 24511, ISO 24512, ISO 55000.
- ASCE/WEF MOP 37; Metcalf & Eddy, *Wastewater Engineering*; CONAGUA, *MAPAS*; RAS 2000 (Colombia).

**Nota:** las referencias internacionales se citan como método o especificación, no como norma aplicable en Ecuador.

---

## Anexo D — Vectores de prueba (regresión)

Estos valores se calcularon con las fórmulas de este documento y son la línea base de los tests. Tolerancia: ±0.5 % salvo indicación.

| # | Caso | Entrada | Resultado esperado |
|---|---|---|---|
| T-01 | Manning, tubo lleno | DN 200, `n = 0.013`, `S = 0.005` | `Q_o = 23.19 L/s`, `V_o = 0.738 m/s`, `τ = 2.45 Pa` |
| T-02 | Flujo parcial | `y/D = 0.5` | `q/Q_o = 0.500`, `v/V_o = 1.000`, `θ_c = π` |
| T-03 | Flujo parcial, máximo | `y/D = 0.94` | `q/Q_o = 1.0757` |
| T-04 | Flujo parcial, velocidad máxima | `y/D = 0.81` | `v/V_o = 1.1400` |
| T-05 | Tirante de diseño | `Q = 15 L/s`, DN 200, `n = 0.013`, `S = 0.005` | `y/D = 0.586` (±0.005), `v = 0.784 m/s`, `τ = 2.69 Pa` |
| T-06 | Capacidad insuficiente | `Q = 15 L/s`, DN 200, `S = 0.002` | `Q_o = 14.66 L/s` ⇒ error de capacidad |
| T-07 | Hazen-Williams | `Q = 10 L/s`, DN 100, `C = 140` | `J = 0.0166 m/m` (16.56 m/km), `V = 1.27 m/s` |
| T-08 | Celeridad y golpe de ariete | PVC DN 200, `e = 3.9 mm`, `E = 2.8 GPa`, `ΔV = 1.5 m/s` | `a = 228 m/s`, `ΔH = 34.9 m` |
| T-09 | Excavación en zanja, paredes verticales | `L = 100 m`, `b = 0.85`, `H_1 = 2.00`, `H_2 = 2.50` | `V = 191.25 m³` |
| T-10 | Excavación en zanja, talud `z = 0.25` | ídem | `V = 319.38 m³` |
| T-11 | Excavación de pozo | `D_int = 1.20`, `e = 0.15`, `s = 0.30`, `H = 3.00` | `V = 10.39 m³` |
| T-12 | Manning, acometida | DN 110, `n = 0.011`, `S = 0.01` | `Q_o = 7.87 L/s`, `V = 0.83 m/s` |

**Uso:** cada vector debe existir como test unitario del crate correspondiente y en la CLI (`calcular --caso T-05`), para que un agente pueda verificar el motor sin abrir la UI.
