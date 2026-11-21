# 06 — Catálogo de familias, subtipos y forma 3D

| Campo | Valor |
|---|---|
| Documento | `docs/consejo/06-catalogo-familias.md` |
| Autor | ingeniero sanitario — consejo de arquitectura |
| Versión | 1.0 (propuesta para decisión) |
| Fecha | 2026-11-18 |
| Estado | Catálogo cerrado en forma y parámetros; 8 decisiones abiertas (§7) |
| Audiencia | cliente, arquitectos de software, implementadores Rust/Rune, dibujantes CAD |
| Alcance | qué familias y subtipos existen en una red de agua potable y alcantarillado, qué parámetros definen su forma y qué se dibuja de cada uno |
| Fuera de alcance | hidráulica, hidrología, cómputos, excavación, render fotorrealista, cláusulas de norma |

**Cómo leer este documento.** El cliente pidió tres cosas en una: *todo* (catálogo exhaustivo), *dibujado* (que se vea) y *paramétrico* (que la forma salga de números, no de una malla guardada). El documento responde en ese orden:

- §2 dice **qué familias debe haber** y qué pasa con las 18 claves que existen hoy.
- §3 es el **catálogo**: para cada familia, todos los subtipos con su uso real en Ecuador y los parámetros que definen su forma.
- §4 fija el **contrato de parámetros**: nombres, unidades, bloques reutilizables, derivados y presupuesto por familia.
- §5 es el **catálogo de dimensiones normalizadas** (DN, diámetros de pozo, tapas, válvulas, accesorios).
- §6 decide **qué se dibuja y qué no**, con el nivel de detalle mínimo aceptable por familia.
- §7 son los **8 riesgos** que el cliente debe resolver; §8, el impacto concreto en el registro Rust.

**Regla dura de este documento.** No hay una sola fórmula de cálculo. Un parámetro entra aquí solo si cambia la *forma* de algo. El caudal, la presión, el volumen de excavación y el tirante son datos de la app de origen y viajan como atributos, no como geometría.

**Convención numérica.** Punto decimal en tablas y fórmulas (compatibilidad con el código); coma decimal en prosa. Unidades SI internas; el diámetro nominal se guarda como entero en milímetros.

## Contenido

0. Resumen: diez decisiones estructurantes
1. Convenciones, marcado y vocabulario
2. Inventario: lo que hay, lo que falta y el catálogo objetivo
3. Catálogo por familia (29 familias, 216 subtipos)
4. Contrato de parámetros
5. Catálogo de dimensiones normalizadas
6. Qué se dibuja y qué no
7. Riesgos y ambigüedades (8)
8. Impacto en el registro Rust
9. Anexos

---

## 0. Resumen: diez decisiones estructurantes

| # | Decisión | Consecuencia |
|---|---|---|
| D1 | El catálogo pasa de **18 claves / 60 subtipos** a **29 familias / 216 subtipos**. | Cobertura real del oficio. El cliente deja de encontrar "lo que no está". |
| D2 | **Subtipo = función. Variante de forma = atributo.** "Válvula de compuerta con husillo ascendente" es subtipo `compuerta` + atributo `tipo_cuerpo = husillo_ascendente`; "codo a 22.5°" es subtipo `codo` + atributo `angulo`. | Sin explosión combinatoria: 12 subtipos de válvula cubren 25 productos; 11 de accesorio cubren 60. El dibujo lee el atributo. |
| D3 | El catálogo es **dato**: una familia nueva es una entrada en `families.rs`, sin tocar el motor ni la UI. | Ya es así hoy (ADR-M1). Este documento no cambia la arquitectura: la llena. |
| D4 | Toda familia declara una lista de **partes dibujables** con nombre estable (`solera`, `cono`, `brocal`, `volante`, `brida`…). | El render itera partes; no hay un `match` gigante por familia. Añadir una familia no añade código de dibujo. |
| D5 | El nivel de detalle se decide por **tamaño en píxeles**, no por zoom ni por familia: LOD0 (<3 px), LOD1 (3–40 px), LOD2 (>40 px). | Un mismo pozo se dibuja como rombo, como cilindro o con cono, marco, tapa y peldaños según lo grande que se vea. Sin banderas por objeto. |
| D6 | Los **componentes** (tapa, escalera, accesorio, arqueta) son familias con `padre_id`, no atributos ni mallas embebidas. | El cliente puede seleccionar el marco, contar tapas D400 y exportarlas a DWG. Es lo que pidió al decir "todo, todo". |
| D7 | **Brocal, cono, banqueo y solera son partes del pozo, no familias.** | Se evitan cuatro familias de un solo objeto. La forma compuesta se declara una vez. |
| D8 | Se persiste **`dn_mm` + `od_mm` + `id_mm`** por separado; el dibujo usa OD/ID, la etiqueta usa DN. | Resuelve la trampa OD/DN del desagüe: un PVC de 200 OD no es un DN 200 de agua potable. Sin esto, el 3D miente en el diámetro. |
| D9 | "Como en la realidad" = **silueta reconocible + proporciones correctas + partes del oficio** (marco, tapa, peldaños, bridas, volante, cono). No texturas, no óxido, no letras en relieve, no roscas. | El presupuesto se gasta en lo que el ingeniero reconoce, no en lo que no se ve a 30 m. |
| D10 | Presupuesto por objeto: **≤ 1 primitiva en LOD0, ≤ 40 triángulos en LOD1, ≤ 400 en LOD2**. | Una ciudad de 5 000 pozos sigue a 60 fps. El catálogo declara el coste de cada familia (§6.3). |

**Lo que este documento *no* decide.** No decide colores de marca, ni si el agua se anima, ni si se dibujan edificios. Eso está en `04-rune-ui.md` y en la capa de estilo, que consume este catálogo.

---

## 1. Convenciones, marcado y vocabulario

### 1.1 Claves, prefijos y códigos

| Elemento | Regla | Ejemplo |
|---|---|---|
| Clave de familia | `dominio.familia`, dominio ∈ {`agua`, `saneamiento`, `comun`}, familia en singular, `snake_case`, sin acentos | `saneamiento.pozo`, `agua.valvula` |
| Clave de subtipo | `snake_case`, sin dominio, única **dentro** de su familia | `caida_interior`, `mariposa` |
| Prefijo de código | 2–3 letras mayúsculas, único en todo el registro (`claves_y_prefijos_unicos` lo verifica) | `PZ`, `VL`, `AX` |
| Código de objeto | `PREFIJO-NNN`, único por proyecto | `PZ-014` |
| Nombre visible | Etiqueta corta en español de Ecuador, sin abreviaturas inventadas | "Pozo de caída interior" |

**Regla de oro:** una clave no cambia de significado nunca. Si un subtipo se parte en dos, se conserva la clave original para el caso dominante y se crean claves nuevas para el resto, con alias en la skill de ingesta (§2.4).

### 1.2 Unidades

| Magnitud | Unidad persistida | Presentación | Nota |
|---|---|---|---|
| Longitud, cota, profundidad, diámetro de estructura | `m` | m (3 decimales) | Diámetros de pozo y cámara en metros, como se especifican en obra |
| Diámetro de tubería y de conexión | `mm` (entero) | mm | DN, OD e ID se guardan por separado (D8) |
| Ángulo | `°` | ° | Ángulo de codo, de ramal, de cruce, de disco |
| Área | `m²` | m² | Huella de tanque, área de infiltración |
| Volumen | `m³` | m³ | Solo para etiqueta; el visor no recalcula |
| Caudal | `L/s` | L/s | Atributo heredado; no interviene en la forma |
| Conteos | entero | — | N.º de entradas, peldaños, bocas, bridas, aros |

### 1.3 Marcado de obligatoriedad

| Marca | Significado | Efecto en el sistema |
|---|---|---|
| ● | **Obligatorio**: sin él la forma no se puede construir | El objeto no se crea (o se marca "incompleto" y se dibuja con la forma mínima) |
| ◐ | **Condicional**: obligatorio solo para ciertos subtipos | La regla vive en el `TypeDef` (`required`) |
| ○ | **Opcional**: mejora el dibujo, no lo habilita | Default del esquema; editable |
| D | **Derivado**: no se edita, se calcula de otros parámetros | `Access::Derived`; si llega del origen, se ignora |

Un objeto puede dibujarse con el mínimo obligatorio. Lo que no se acepta es dibujar *inventando*: si falta `altura_cono`, no se pone 0.60 por defecto — se usa el cono mínimo de la tabla de dimensiones (§5.2) y se marca el objeto como "forma incompleta" para revisión.

### 1.4 Vocabulario de partes

Una **parte** es un trozo de forma con nombre estable. Es la unidad de dibujo (D4) y la unidad de conteo de triángulos (D10). El vocabulario completo, con su primitiva, está en §6.2. Las partes que aparecen en §3 son: `solera`, `cuerpo`, `cono`, `brocal`, `marco`, `tapa`, `peldano`, `banqueo`, `entrada`, `salida`, `caida`, `pantalla`, `losa`, `fuste`, `cubierta`, `brida`, `cuerpo_valvula`, `bonete`, `vastago`, `volante`, `piston`, `disco`, `flotador`, `rejilla`, `muro`, `canaleta`, `deflector`, `empaque`, `apoyo`.

### 1.5 Estados y origen del dato

Se heredan de `01-domain.md` §4.1 y no se repiten aquí. Para el catálogo importa una sola cosa: **el estado no cambia la forma**. Un pozo `abandonado` se dibuja igual que uno `operativo`, con el color de estado que fije la capa de estilo. La única excepción acordada es `deleted`, que lo saca de escena.

---

## 2. Inventario: lo que hay, lo que falta y el catálogo objetivo

### 2.1 Lo que hay hoy (verificado en el código)

El registro `crates/redes-core/src/families.rs` declara **18 claves** y **60 subtipos**. La discrepancia con el encargo es real: el encargo habla de 17 familias y no menciona `agua.tuberia`, que sí existe (prefijo `TB`). Conviene corregir el número en la documentación del producto.

| # | Clave actual | Prefijo | Subtipos | Veredicto | Destino |
|---|---|---|---|---|---|
| 1 | `saneamiento.pozo` | PZ | 7 | Ampliar y limpiar | 12 subtipos; `bombeo` y `limpieza` salen a otras familias |
| 2 | `saneamiento.tuberia` | CL | 8 | Ampliar y corregir dominio | 10 subtipos; los tipos de agua potable (`conduccion`, `aduccion`, `distribucion`, `impulsion`) se retiran de aquí |
| 3 | `agua.tuberia` | TB | 4 | Ampliar | 8 subtipos |
| 4 | `saneamiento.descarga` | DS | 4 | Ampliar | 13 subtipos |
| 5 | `agua.valvula` | VL | 8 | Ampliar | 12 subtipos funcionales + variantes por atributo |
| 6 | `agua.llave` | LL | 4 | **Dividir** | 4 subtipos; `hidrante` pasa a `agua.hidrante` |
| 7 | `agua.tanque` | TQ | 3 | Ampliar | 8 subtipos |
| 8 | `agua.medidor` | MD | 2 | Ampliar | 7 subtipos |
| 9 | `saneamiento.acometida` | AC | 2 | **Dividir por dominio** | 5 subtipos de alcantarillado; el tipo `agua_potable` se va a `agua.acometida` |
| 10 | `saneamiento.camara` | CM | 3 | Ampliar | 11 subtipos |
| 11 | `saneamiento.sumidero` | SM | 2 | Ampliar | 8 subtipos |
| 12 | `saneamiento.canal` | CN | 2 | Ampliar | 9 subtipos |
| 13 | `saneamiento.sifon` | SF | 2 | Ampliar | 4 subtipos |
| 14 | `agua.captacion` | CP | 2 | Ampliar | 8 subtipos |
| 15 | `agua.planta` | PT | 2 | Ampliar | 11 subtipos |
| 16 | `saneamiento.bombeo` | EB | 1 | Ampliar | 5 subtipos |
| 17 | `comun.cruce` | CR | 2 | Ampliar | 10 subtipos |
| 18 | `comun.hito` | HT | 2 | Ampliar | 6 subtipos |
| | **Total** | | **60** | | **151** |

### 2.2 Lo que falta: 11 familias propuestas

El cliente pidió explícitamente accesorios de tubería, cajas de revisión, cámaras de inspección, sifones, areneros, trampas de grasa, medidores, bocas de visita, escaleras, brocales, marcos y tapas, válvulas de aire/purga e hidrantes. De esa lista, unas cosas ya existen como subtipo y otras no existen en absoluto.

| # | Clave propuesta | Prefijo | Qué resuelve | Por qué familia y no subtipo |
|---|---|---|---|---|
| 1 | `comun.accesorio` | AX | Codos, tees, cruces, reducciones, uniones, tapones, bridas, collarines, sillas | Pieza con forma propia, DN propio y ángulo propio; se cuenta y se dibuja suelta en el detalle de nudo |
| 2 | `comun.tapa` | TP | Marcos, tapas y rejillas (EN 124) | Es el objeto que el cliente ve en la calle; tiene clase de carga, material y despiece propio |
| 3 | `comun.escalera` | ES | Peldaños, escaleras de gato, marinas y con jaula | Se especifica aparte (material, separación, jaula) y se dibuja aparte |
| 4 | `comun.cuerpo_receptor` | RC | Río, quebrada, estero, mar, lago, canal, terreno | Una descarga sin receptor no se puede dibujar "como en la realidad": falta el agua que recibe |
| 5 | `saneamiento.caja` | CJ | Cajas de revisión de acometida (0.60 × 0.60, 0.60 × 0.90…) | Hoy está escondida como `pozo.limpieza`, y no es un pozo: no es visitable ni tiene cono |
| 6 | `saneamiento.arenero` | AR | Trampas de arena de patio, canal y prefabricadas | El arenero de sumidero es una *parte*; el arenero de patio es un *objeto* que se instala y se mantiene |
| 7 | `saneamiento.trampa_grasa` | TG | Trampas de grasa y separadores de aceites | Geometría de cámara múltiple, deflectores y tapa propia; no es un pozo ni una cámara de inspección |
| 8 | `agua.arqueta` | AQ | Arquetas de medidor, válvula, hidrante, purga y aire | Es el "pozo" del agua potable: sin ella, medidores y válvulas flotan en el aire del modelo |
| 9 | `agua.hidrante` | HD | Hidrantes de columna, de arqueta, bocas de riego y tipo tampón | Geometría muy distinta a una llave de banqueta; el cliente lo pidió como familia propia |
| 10 | `agua.dado_anclaje` | DA | Dados de anclaje en codos, tees y tapones | Volumen de hormigón con forma propia; aparece en todo plano de detalle de impulsión |
| 11 | `agua.acometida` | AA | Acometida de agua potable | Hoy vive en `saneamiento.acometida`, que es un error de dominio: una acometida de AP no es saneamiento |

**Componentes que se decidió *no* hacer familia** (y por qué):

| Candidato | Decisión | Razón |
|---|---|---|
| Brocal | Parte de `saneamiento.pozo` y `agua.arqueta` | Es un anillo de hormigón de 0.10–0.30 m; como familia obligaría a un objeto por cada pozo |
| Cono de reducción | Parte del pozo | Igual: no se instala suelto, se construye con el pozo |
| Banqueo / media caña | Parte del pozo | Es la solera conformada, no un objeto |
| Bomba, tablero, sifón de tanque | Diferido a v1.1 (§8.6) | El cliente no los pidió; se dibujan como volumen de la estación (§6.3) |
| Rejilla | Subtipo de `comun.tapa` | Comparte marco, clase de carga y material con la tapa |

### 2.3 Catálogo objetivo: 29 familias, 216 subtipos

| Dominio | Familias | Subtipos | Claves |
|---|---|---|---|
| Saneamiento | 12 | 95 | `pozo`, `tuberia`, `caja`, `camara`, `arenero`, `trampa_grasa`, `sumidero`, `canal`, `sifon`, `descarga`, `bombeo`, `acometida` |
| Agua potable | 11 | 76 | `tuberia`, `valvula`, `llave`, `hidrante`, `medidor`, `arqueta`, `tanque`, `captacion`, `planta`, `dado_anclaje`, `acometida` |
| Común | 6 | 45 | `accesorio`, `tapa`, `escalera`, `cruce`, `cuerpo_receptor`, `hito` |
| **Total** | **29** | **216** | |

El desglose por familia, con subtipos y parámetros, es §3. La suma cuadra con §2.1 y §2.2: 151 (existentes ampliadas) + 65 (11 familias nuevas) = 216.

### 2.4 Reglas de migración

Renombrar una clave rompe los archivos `*.redes` y los mapeos de la skill de ingesta. Por eso:

| Regla | Consecuencia |
|---|---|
| R1. La clave se conserva si el concepto se conserva | `saneamiento.pozo` sigue siendo `saneamiento.pozo` con 12 subtipos |
| R2. Un subtipo que cambia de familia se mueve con alias | `pozo/bombeo` → `bombeo/camara_humeda`; la ingesta acepta el nombre viejo y avisa |
| R3. Un subtipo que se parte conserva la clave en el caso dominante | `pozo/limpieza` → `caja/revision` (alias `pozo.limpieza` durante dos versiones) |
| R4. Nunca se reutiliza una clave con otro significado | `llave/hidrante` no se convierte en "válvula de hidrante"; se retira y se crea `hidrante/columna` |
| R5. La migración es de datos, no de esquema | Un proyecto viejo se abre, se migra en memoria y se marca `migrado_en`; no se reescribe el archivo sin confirmación |

**Alias obligatorios en la skill de ingesta** (tabla que debe vivir en `skills/redes-ingesta/references/mapeos.md`):

| Alias de origen | Clave nueva |
|---|---|
| `saneamiento.pozo:limpieza` | `saneamiento.caja:revision` |
| `saneamiento.pozo:bombeo` | `saneamiento.bombeo:camara_humeda` |
| `agua.llave:hidrante` | `agua.hidrante:columna` |
| `saneamiento.acometida:agua_potable` | `agua.acometida:domiciliaria` |
| `saneamiento.tuberia:conduccion` | `agua.tuberia:conduccion` |
| `saneamiento.tuberia:aduccion` | `agua.tuberia:aduccion` |
| `saneamiento.tuberia:distribucion` | `agua.tuberia:distribucion` |
| `saneamiento.tuberia:impulsion` | `saneamiento.tuberia:impulsion` (se queda: la impulsión de AL existe) |

---

## 3. Catálogo por familia

Cada familia trae: una línea de ficha, para qué sirve, la tabla de **subtipos** (con el uso real y los parámetros que cambian la forma) y la tabla de **parámetros** con unidad, rango típico y obligatoriedad. Los parámetros que se repiten en casi todas las familias no se repiten aquí: se declaran una vez como **bloques** en §4.2 y cada ficha dice qué bloques usa.

### 3.1 Saneamiento (12 familias, 95 subtipos)

#### 3.1.1 `saneamiento.pozo` — Pozo de inspección

`PZ` · punto con sólido derivado · nodo · 12 subtipos · partes: solera, cuerpo, cono, brocal, marco, tapa, peldaños, banqueo, caída

Da acceso, ventilación e inspección al colector y resuelve el cambio de dirección, pendiente o diámetro, la unión de aportes y la disipación de energía. Es la familia más numerosa de una red de alcantarillado y la que más se dibuja: si el pozo se ve bien, la red se ve bien.

**Subtipos**

| # | Subtipo | Nombre técnico | Cuándo se usa | Parámetros distintivos |
|---|---|---|---|---|
| 1 | `inspeccion` | Pozo de inspección circular | Caso base: cambio de dirección, pendiente o diámetro; unión de hasta 4 colectores | `diametro_interior`, `numero_entradas`, `numero_salidas` |
| 2 | `inspeccion_rectangular` | Cámara de inspección rectangular | Aceras estrechas, profundidades grandes, varias entradas en muros distintos | `largo_interior`, `ancho_interior`, `numero_entradas` |
| 3 | `con_arenero` | Pozo con arenero (sedimentador) | Red pluvial con arrastre de arena; el pozo tiene un pozo de sedimentación bajo la solera | `profundidad_arenero`, `diametro_arenero` |
| 4 | `caida_exterior` | Pozo de caída exterior | Desnivel > 0.50 m que no se resuelve con pendiente; el tubo de caída va adosado por fuera | `altura_caida`, `diametro_caida`, `angulo_caida` |
| 5 | `caida_interior` | Pozo de caída interior | Igual que el anterior, pero el tubo va dentro del pozo con pantalla de impacto | `altura_caida`, `diametro_caida`, `longitud_pantalla` |
| 6 | `caida_escalonada` | Pozo de caída escalonada | Desniveles > 3.00 m; el agua baja por gradas o por varias caídas sucesivas | `altura_caida`, `numero_escalones`, `altura_escalon` |
| 7 | `resalto` | Pozo de resalto | Desnivel ≤ 0.50 m resuelto con un dado o resalto en la solera | `altura_resalto`, `longitud_resalto` |
| 8 | `empate` | Pozo de empate | Convergencia de colectores a distinta cota; el pozo ordena las llegadas antes de la salida | `numero_entradas`, `cota_entrada`, `altura_caida` |
| 9 | `disipador` | Pozo disipador de energía | Pendientes fuertes y emisarios de montaña; solera con deflector o dado | `tipo_disipador`, `altura_deflector`, `longitud_deflector` |
| 10 | `ciego` | Pozo ciego (absorbente) | Zona rural sin red; infiltra al terreno | `area_infiltracion`, `profundidad_efectiva`, `material_filtrante` |
| 11 | `ventilacion` | Pozo de ventilación | Tramos largos sin acceso; la tapa o el tubo ventilan el colector | `tipo_ventilacion`, `diametro_ventilacion`, `altura_ventilacion` |
| 12 | `limpieza` | Pozo de limpieza | Extremo de ramal o tramo corto; permite lavar con manguera, sin peldaños | `diametro_tapon`, `tipo_tapon` |

**Parámetros**

| Parámetro | Unidad | Rango típico | Obl. | Nota |
|---|---|---|---|---|
| `diametro_interior` | m | 0.90 / 1.20 / 1.50 | ● | Valor de catálogo (§5.2). Hasta 3.00 en cámaras |
| `tipo_seccion` | enum | circular, rectangular | ● | Default `circular` |
| `largo_interior` | m | 0.90–3.00 | ◐ | Solo rectangular |
| `ancho_interior` | m | 0.90–2.00 | ◐ | Solo rectangular |
| `espesor_pared` | m | 0.10–0.30 | ● | 0.15 es el más frecuente en hormigón |
| `espesor_solera` | m | 0.15–0.30 | ● | Incluye la cama de desplante |
| `cota_solera` | m | — | ● | Fondo interior; es la referencia de todo el pozo |
| `cota_tapa` | m | — | ● | Normalmente = cota de rasante |
| `profundidad_total` | m | 0.60–12.0 | D | De `cota_terreno` − `cota_solera` |
| `altura_cuerpo` | m | — | D | `profundidad_total` − `altura_cono` − `espesor_solera` |
| `altura_cono` | m | 0.45–0.90 | ◐ | Obligatorio si hay cono; se acota a §5.2 |
| `diametro_tapa` | m | 0.60 / 0.70 | ● | Fija el cono y el marco |
| `altura_brocal` | m | 0.10–0.30 | ○ | 0.20 en calzada |
| `ancho_brocal` | m | 0.15–0.30 | ○ | |
| `numero_entradas` | int | 0–6 | ● | 0 solo en ciego y limpieza |
| `diametro_entrada` | mm | 110–600 | ◐ | Por entrada; el máximo manda en el dibujo |
| `angulo_entrada` | ° | 0–360 | ○ | Orientación en planta de cada entrada |
| `numero_salidas` | int | 1–4 | ● | 0 solo en ciego |
| `diametro_salida` | mm | 110–600 | ◐ | |
| `ancho_banqueo` | m | 0.15–0.60 | ○ | Media caña; igual al DN de salida |
| `altura_banqueo` | m | 0.05–0.30 | ○ | |
| `pendiente_banqueo` | % | 0.5–10 | ○ | |
| `tipo_caida` | enum | exterior, interior, escalonada | ◐ | Solo subtipos de caída |
| `altura_caida` | m | 0.30–5.00 | ◐ | |
| `diametro_caida` | mm | 150–250 | ◐ | Nunca menor que el DN de salida |
| `longitud_pantalla` | m | 0.30–1.00 | ◐ | Caída interior |
| `numero_escalones` | int | 1–10 | ◐ | Caída escalonada |
| `altura_escalon` | m | 0.30–0.60 | ◐ | |
| `altura_resalto` | m | 0.10–0.50 | ◐ | Resalto |
| `longitud_resalto` | m | 0.30–1.50 | ◐ | |
| `profundidad_arenero` | m | 0.20–0.60 | ◐ | Pozo con arenero |
| `diametro_arenero` | m | 0.60–1.20 | ◐ | |
| `tipo_disipador` | enum | deflector, dado, escalonado | ◐ | Disipador |
| `altura_deflector` | m | 0.20–0.80 | ◐ | |
| `area_infiltracion` | m² | 1–20 | ◐ | Pozo ciego |
| `profundidad_efectiva` | m | 1.0–6.0 | ◐ | Altura de la zona filtrante |
| `material_filtrante` | enum | grava, arena, escollera | ○ | |
| `tipo_ventilacion` | enum | rejilla, tubo | ◐ | Ventilación |
| `diametro_ventilacion` | mm | 100–200 | ◐ | |
| `altura_ventilacion` | m | 0.30–2.50 | ◐ | Sobre rasante |
| `sistema_constructivo` | enum | fundido, prefabricado | ○ | Cambia el dibujo de las juntas |
| `tipo_tapon` | enum | ciego, bridado, roscado | ◐ | Pozo de limpieza |

**Bloques:** B-COTAS, B-MATERIAL, B-TAPA, B-ESCALERA, B-ESTADO (§4.2).

#### 3.1.2 `saneamiento.tuberia` — Tubería de alcantarillado

`CL` · polilínea 3D · arco · 10 subtipos · partes: tubo, junta, campana, revestimiento

Conduce agua residual, pluvial o combinada por gravedad, y también a presión en impulsiones y cruces forzados. La forma la definen tres cosas: la sección, el diámetro exterior y el eje. El resto es atributo.

**Subtipos**

| # | Subtipo | Nombre técnico | Cuándo se usa | Parámetros distintivos |
|---|---|---|---|---|
| 1 | `colector_sanitario` | Colector sanitario | Recoge aguas residuales domiciliarias | `dn_mm`, `pendiente`, `tipo_red = sanitario` |
| 2 | `colector_pluvial` | Colector pluvial | Recoge agua de lluvia de sumideros y bajantes | `dn_mm`, `pendiente`, `seccion` |
| 3 | `colector_combinado` | Colector combinado | Red antigua que lleva residual + lluvia | `dn_mm`, `pendiente`, `tipo_red = combinado` |
| 4 | `interceptor` | Interceptor | Recibe varios colectores; va por el borde del cauce o la vía principal | `dn_mm` ≥ 300, `seccion` |
| 5 | `emisario` | Emisario | Último tramo hasta el cuerpo receptor | `dn_mm`, `seccion` |
| 6 | `ramal` | Ramal / subcolector | Colector menor de 150–200 mm | `dn_mm`, `pendiente` |
| 7 | `impulsion` | Impulsión | Tramo a presión desde una estación de bombeo | `dn_mm`, `clase_presion`, `material` |
| 8 | `conduccion_forzada` | Conducción forzada | Tramo a presión dentro de una red por gravedad (cruce, sifón, descarga) | `dn_mm`, `clase_presion` |
| 9 | `rebose` | Rebose | Alivio de un pozo o cámara hacia un cauce | `dn_mm`, `pendiente` |
| 10 | `desague` | Desagüe | Vacía un tramo o un tanque; descarga libre | `dn_mm`, `pendiente`, `tipo_valvula` |

**Parámetros**

| Parámetro | Unidad | Rango típico | Obl. | Nota |
|---|---|---|---|---|
| `seccion` | enum | circular, ovoide, herradura, rectangular | ● | Default `circular` |
| `dn_mm` | mm | 75–600 (hasta 1 200) | ● | Entero; se dibuja con OD (D8) |
| `od_mm` | mm | — | ● | Exterior real del tubo |
| `id_mm` | mm | — | ● | Interior real; de él sale el hueco visible |
| `espesor_pared` | mm | 2.0–80 | ● | |
| `material` | enum | PVC, PEAD, HG, HD, hormigon, GRC, acero, mamposteria | ● | B-MATERIAL |
| `tipo_junta` | enum | espiga_campana, bridada, soldada, electrofusion, mecanica | ● | Define si se dibujan campanas cada 6 m |
| `rigidez_anular` | enum | SN2, SN4, SN8 | ◐ | Solo plásticos de alcantarillado |
| `clase_presion` | enum | PN6, PN10, PN16 | ◐ | Solo impulsión y conducción forzada |
| `pendiente` | m/m | 0.001–0.05 | ● | Del eje, no del terreno |
| `cota_clave_inicio` | m | — | ● | |
| `cota_clave_fin` | m | — | ● | |
| `longitud` | m | 0.5–5 000 | D | De la polilínea 3D |
| `ancho_base` | m | 0.40–3.00 | ◐ | Secciones no circulares |
| `altura_interior` | m | 0.40–3.00 | ◐ | Secciones no circulares |
| `numero_celdas` | int | 1–4 | ○ | Box culvert |
| `revestimiento` | enum | ninguno, hormigon, epoxico, PE | ○ | Cambia el color interior si hay corte |
| `tipo_red` | enum | sanitario, pluvial, combinado, industrial | ● | Atributo que el cliente usa para filtrar |

**Bloques:** B-COTAS, B-MATERIAL, B-ESTADO.

#### 3.1.3 `saneamiento.caja` — Caja de revisión

`CJ` · punto con sólido derivado · nodo · 8 subtipos · partes: solera, cuerpo, tapa, marco, entrada, salida, caída

Recibe la acometida y permite revisarla sin bajar. No es un pozo: no es visitable, no tiene cono y su tapa es cuadrada o rectangular. Hoy está escondida como `saneamiento.pozo:limpieza`, lo que produce pozos de 0.60 m con cono en los planos.

**Subtipos**

| # | Subtipo | Nombre técnico | Cuándo se usa | Parámetros distintivos |
|---|---|---|---|---|
| 1 | `revision` | Caja de revisión domiciliaria | Toda acometida de alcantarillado, en el límite del predio | `largo`, `ancho`, `profundidad` |
| 2 | `revision_con_peldano` | Caja de revisión con peldaño | Profundidad > 0.90 m; permite bajar a limpiar | `profundidad`, `numero_peldanos` |
| 3 | `paso` | Caja de paso | Cambio de dirección o unión de dos ramales de acometida | `numero_entradas`, `numero_salidas` |
| 4 | `sifonada` | Caja sifonada | Impide el paso de gases del colector al predio | `altura_sifon`, `diametro_sifon` |
| 5 | `empate` | Caja de empate | Varias acometidas confluyen antes del colector | `numero_entradas` |
| 6 | `inspeccion` | Caja de inspección | Acometidas industriales o colectivas; 0.80 × 0.80 o mayor | `largo`, `ancho` |
| 7 | `caida` | Caja con caída | Desnivel entre la acometida y el colector | `altura_caida`, `diametro_caida` |
| 8 | `prefabricada` | Caja prefabricada | Registro plástico de 0.40–0.60 m en patios y jardines | `diametro`, `altura` |

**Parámetros**

| Parámetro | Unidad | Rango típico | Obl. | Nota |
|---|---|---|---|---|
| `largo` | m | 0.60–1.20 | ● | Interior |
| `ancho` | m | 0.60–1.00 | ● | Interior |
| `profundidad` | m | 0.40–2.00 | ● | |
| `espesor_pared` | m | 0.10–0.20 | ● | |
| `espesor_solera` | m | 0.10–0.20 | ● | |
| `cota_tapa` | m | — | ● | |
| `cota_fondo` | m | — | ● | |
| `diametro_entrada` | mm | 100–200 | ● | |
| `diametro_salida` | mm | 100–200 | ● | |
| `numero_entradas` | int | 1–3 | ● | |
| `numero_salidas` | int | 1–2 | ● | |
| `altura_sifon` | m | 0.15–0.40 | ◐ | Caja sifonada |
| `diametro_sifon` | mm | 100–150 | ◐ | |
| `altura_caida` | m | 0.10–1.50 | ◐ | Caja con caída |
| `diametro_caida` | mm | 100–150 | ◐ | |
| `numero_peldanos` | int | 1–4 | ○ | |
| `diametro` | m | 0.40–0.60 | ◐ | Prefabricada |

**Bloques:** B-COTAS, B-MATERIAL, B-TAPA, B-ESTADO.

#### 3.1.4 `saneamiento.camara` — Cámara

`CM` · punto con sólido derivado · nodo · 11 subtipos · partes: solera, muro, losa, tapa, vano, deflector, compuerta

Estructura visitable o no, con función distinta de la inspección: separar arena, reunir caudales, romper presión, alojar válvulas o repartir. Se diferencia del pozo por la función y por la forma: rectangular, con losa y varias tapas.

**Subtipos**

| # | Subtipo | Nombre técnico | Cuándo se usa | Parámetros distintivos |
|---|---|---|---|---|
| 1 | `desarenadora` | Cámara desarenadora | Antes de una PTAR, un bombeo o un sifón; retiene arenas | `largo`, `ancho`, `tirante`, `numero_naves` |
| 2 | `reunion` | Cámara de reunión | Varios aportes se juntan antes de una planta o un bombeo | `numero_aportes`, `cota_solera` |
| 3 | `carga` | Cámara de carga | Regula la entrada a una conducción a presión | `cota_agua`, `longitud_vertedero` |
| 4 | `quiebre` | Cámara de quiebre de presión | Rompe la presión en conducciones largas | `tipo_quiebre`, `altura_caida` |
| 5 | `valvulas` | Cámara de válvulas | Aloja válvulas de gran diámetro enterradas | `largo`, `ancho`, `numero_valvulas` |
| 6 | `medicion` | Cámara de medición | Aloja un macromedidor con bypass | `diametro_tuberia`, `by_pass` |
| 7 | `reparto` | Cámara de reparto | Divide el caudal en dos o más ramales | `numero_salidas`, `tipo_reparto` |
| 8 | `alivio` | Cámara de alivio | Deriva excedentes a un alivio lateral | `longitud_vertedero`, `cota_cresta` |
| 9 | `inspeccion_rectangular` | Cámara de inspección | Registro grande no circular, en vías amplias o plantas | `largo`, `ancho`, `altura_util` |
| 10 | `bombeo_seco` | Cámara seca de bombeo | Aloja las bombas y el tablero, separada de la cámara húmeda | `largo`, `ancho`, `numero_bombas` |
| 11 | `compensacion` | Cámara de compensación | Amortigua variaciones de caudal antes de un proceso | `volumen_util`, `altura_util` |

**Parámetros**

| Parámetro | Unidad | Rango típico | Obl. | Nota |
|---|---|---|---|---|
| `largo` | m | 1.00–12.0 | ● | Interior |
| `ancho` | m | 1.00–8.00 | ● | Interior |
| `altura_util` | m | 0.80–6.00 | ● | Del fondo a la losa |
| `espesor_pared` | m | 0.15–0.40 | ● | |
| `espesor_losa` | m | 0.15–0.40 | ● | |
| `cota_solera` | m | — | ● | |
| `cota_losa` | m | — | ● | |
| `numero_tapas` | int | 1–6 | ● | El dibujo reparte tapas en la losa |
| `diametro_tapa` | m | 0.60 / 0.70 | ● | |
| `numero_naves` | int | 1–4 | ◐ | Desarenadora |
| `tirante` | m | 0.30–2.00 | ◐ | Desarenadora y compensación |
| `numero_aportes` | int | 2–6 | ◐ | Reunión |
| `numero_salidas` | int | 2–4 | ◐ | Reparto |
| `tipo_reparto` | enum | vertedero, compuerta, canal | ◐ | Reparto |
| `longitud_vertedero` | m | 0.30–5.00 | ◐ | Carga y alivio |
| `cota_cresta` | m | — | ◐ | Alivio |
| `tipo_quiebre` | enum | vertedero, chorro, deflector | ◐ | Quiebre |
| `altura_caida` | m | 0.30–3.00 | ◐ | Quiebre |
| `numero_valvulas` | int | 1–6 | ◐ | Cámara de válvulas |
| `numero_bombas` | int | 1–4 | ◐ | Cámara seca |
| `by_pass` | bool | — | ○ | Medición |

**Bloques:** B-COTAS, B-MATERIAL, B-TAPA, B-ESCALERA, B-ESTADO.

#### 3.1.5 `saneamiento.arenero` — Arenero

`AR` · punto con sólido derivado · nodo · 4 subtipos · partes: cuerpo, solera, rejilla, tapa, salida

Trampa de arena que no está bajo un sumidero: patio, canal, parqueadero o planta. Retiene sólidos pesados antes de que entren al colector. El arenero *del sumidero* es una parte del sumidero (§3.1.7), no esta familia.

**Subtipos**

| # | Subtipo | Nombre técnico | Cuándo se usa | Parámetros distintivos |
|---|---|---|---|---|
| 1 | `patio` | Arenero de patio | Patios y lavaderos donde llega arena y tierra | `largo`, `ancho`, `profundidad_util` |
| 2 | `canal` | Arenero de canal | Cabeza de canal de riego o de drenaje | `largo`, `ancho`, `tirante` |
| 3 | `prefabricado` | Arenero prefabricado | Solución plástica para vivienda unifamiliar | `diametro`, `altura` |
| 4 | `con_rejilla` | Arenero con rejilla | Parqueaderos y zonas de lavado de vehículos | `largo`, `ancho`, `tipo_rejilla`, `clase_carga` |

**Parámetros**

| Parámetro | Unidad | Rango típico | Obl. | Nota |
|---|---|---|---|---|
| `largo` | m | 0.40–2.00 | ● | |
| `ancho` | m | 0.40–1.50 | ● | |
| `profundidad_util` | m | 0.20–0.80 | ● | Volumen de arena retenida |
| `profundidad_total` | m | 0.40–1.50 | ● | |
| `diametro` | m | 0.30–0.60 | ◐ | Prefabricado |
| `altura` | m | 0.40–1.00 | ◐ | Prefabricado |
| `diametro_salida` | mm | 75–200 | ● | |
| `tipo_rejilla` | enum | barrotes, cuadricula | ◐ | Con rejilla |
| `clase_carga` | enum | A15, B125, C250 | ◐ | |

**Bloques:** B-COTAS, B-MATERIAL, B-TAPA, B-ESTADO.

#### 3.1.6 `saneamiento.trampa_grasa` — Trampa de grasa y separador

`TG` · punto con sólido derivado · nodo · 6 subtipos · partes: cuerpo, muro, deflector, tapa, entrada, salida

Retiene grasas y aceites de restaurantes, cocinas industriales y talleres antes de la descarga al colector. Se dibuja como una caja dividida en cámaras con deflectores, porque así se reconoce en campo.

**Subtipos**

| # | Subtipo | Nombre técnico | Cuándo se usa | Parámetros distintivos |
|---|---|---|---|---|
| 1 | `prefabricada_hdpe` | Trampa prefabricada de PEAD | Cocinas pequeñas y medianas; llega armada | `largo`, `ancho`, `altura`, `numero_camaras` |
| 2 | `prefabricada_prfv` | Trampa prefabricada de PRFV | Caudales medios y aguas con temperatura | `diametro`, `altura`, `numero_camaras` |
| 3 | `construida_2_camaras` | Trampa construida en sitio, 2 cámaras | El caso más común en Ecuador; hormigón fundido | `largo`, `ancho`, `tirante`, `numero_camaras` |
| 4 | `construida_3_camaras` | Trampa construida en sitio, 3 cámaras | Cocinas grandes; la tercera cámara es de sedimentación | `largo`, `ancho`, `tirante`, `numero_camaras` |
| 5 | `separador_grasas` | Separador de grasas industrial | Plantas de alimentos y camales | `largo`, `ancho`, `tirante`, `numero_deflectores` |
| 6 | `separador_aceites` | Separador de aceites e hidrocarburos | Talleres, gasolineras, lavadoras | `largo`, `ancho`, `altura`, `tipo_coalescedor` |

**Parámetros**

| Parámetro | Unidad | Rango típico | Obl. | Nota |
|---|---|---|---|---|
| `largo` | m | 0.60–6.00 | ● | |
| `ancho` | m | 0.40–3.00 | ● | |
| `altura` | m | 0.50–3.00 | ● | |
| `tirante` | m | 0.30–2.00 | ● | |
| `numero_camaras` | int | 1–3 | ● | Determina los muros interiores |
| `numero_deflectores` | int | 0–3 | ○ | |
| `diametro_entrada` | mm | 50–200 | ● | |
| `diametro_salida` | mm | 50–200 | ● | |
| `altura_salida` | m | — | ● | Cota del tubo de salida; define el tirante |
| `tipo_coalescedor` | enum | ninguno, placas, malla | ○ | Separador de aceites |
| `diametro` | m | 0.60–1.80 | ◐ | Prefabricada cilíndrica |

**Bloques:** B-COTAS, B-MATERIAL, B-TAPA, B-ESTADO.

#### 3.1.7 `saneamiento.sumidero` — Sumidero

`SM` · punto con sólido derivado · nodo · 8 subtipos · partes: marco, rejilla, cuerpo, arenero, conexion, muro

Capta el agua de lluvia de la calzada y la entrega al colector pluvial. Es el objeto que el cliente ve en la calle, así que su marco y su rejilla son parte de la "realidad" que pide el encargo. La rejilla y su clase de carga viven en `comun.tapa` (§3.3.2); aquí se declara la boca.

**Subtipos**

| # | Subtipo | Nombre técnico | Cuándo se usa | Parámetros distintivos |
|---|---|---|---|---|
| 1 | `calzada` | Sumidero de calzada (boca de lobo) | Punto bajo de la calzada, contra el bordillo | `largo_util`, `ancho`, `tipo_rejilla` |
| 2 | `rejilla` | Sumidero de rejilla | Calzada o patio con rejilla horizontal | `largo_util`, `ancho`, `numero_barras` |
| 3 | `cuneta` | Sumidero de cuneta | Cuneta lateral con boca vertical | `largo_util`, `altura_boca` |
| 4 | `combinado` | Sumidero combinado | Boca + rejilla; zonas de lluvia intensa | `largo_util`, `ancho`, `altura_boca` |
| 5 | `acera` | Sumidero de acera | Acera con pendiente hacia la calzada | `largo_util`, `ancho` |
| 6 | `patio` | Sumidero de patio | Patios y cubiertas; rejilla pequeña | `largo_util`, `ancho`, `diametro_conexion` |
| 7 | `boca_tormenta` | Boca de tormenta | Entrega directa a un cauce o canal | `largo_util`, `altura_boca`, `muro_banqueta` |
| 8 | `tipo_ranura` | Sumidero de ranura | Vías con tráfico pesado y poca capacidad de rejilla | `longitud_ranura`, `ancho_ranura` |

**Parámetros**

| Parámetro | Unidad | Rango típico | Obl. | Nota |
|---|---|---|---|---|
| `largo_util` | m | 0.40–3.00 | ● | Dimensión mayor de la boca |
| `ancho` | m | 0.30–1.00 | ● | |
| `altura_boca` | m | 0.30–1.00 | ◐ | Boca vertical (cuneta, combinado, boca de tormenta) |
| `profundidad` | m | 0.60–2.50 | ● | De rasante a fondo |
| `espesor_pared` | m | 0.10–0.25 | ● | |
| `cota_rasante` | m | — | ● | |
| `cota_fondo` | m | — | ● | |
| `diametro_conexion` | mm | 100–400 | ● | Salida al colector |
| `tipo_rejilla` | enum | barrotes, cuadricula, combinada | ● | Remite a `comun.tapa` |
| `numero_barras` | int | 4–20 | ○ | |
| `separacion_barras` | mm | 15–40 | ○ | |
| `clase_carga` | enum | A15, B125, C250, D400 | ● | §5.3 |
| `con_arenero` | bool | — | ○ | El arenero es parte, no familia |
| `profundidad_arenero` | m | 0.20–0.60 | ◐ | Si `con_arenero` |
| `longitud_ranura` | m | 1.00–6.00 | ◐ | Tipo ranura |
| `ancho_ranura` | mm | 20–60 | ◐ | |
| `muro_banqueta` | bool | — | ○ | Boca de tormenta |

**Bloques:** B-COTAS, B-MATERIAL, B-ESTADO.

#### 3.1.8 `saneamiento.canal` — Canal

`CN` · polilínea con sección · arco · 9 subtipos · partes: solera, talud, muro, borde, junta, rejilla

Conduce a superficie libre: cunetas, canales de coronación, canales de descarga y box culvert. La forma la define la sección transversal; el eje, la polilínea.

**Subtipos**

| # | Subtipo | Nombre técnico | Cuándo se usa | Parámetros distintivos |
|---|---|---|---|---|
| 1 | `rectangular` | Canal rectangular | Conducciones revestidas de planta y descarga | `base`, `altura_muro` |
| 2 | `trapezoidal` | Canal trapezoidal | Canales de tierra o revestidos de gran caudal | `base`, `talud`, `altura_muro` |
| 3 | `triangular` | Canal triangular | Cunetas de drenaje vial | `base`, `talud` |
| 4 | `circular` | Canal circular (abierto) | Media caña o tubería abierta | `diametro` |
| 5 | `cuneta_v` | Cuneta en V | Borde de vía, sin revestimiento o con hormigón | `ancho_corona`, `profundidad` |
| 6 | `cuneta_u` | Cuneta en U | Cuneta revestida con bordillo | `ancho`, `altura_muro` |
| 7 | `coronacion` | Canal de coronación | Protege un talud o una plataforma de la escorrentía | `base`, `talud`, `longitud` |
| 8 | `descarga` | Canal de descarga | Lleva el agua de una estructura hasta el receptor | `base`, `pendiente` |
| 9 | `box_culvert` | Box culvert | Paso bajo vía o terraplén con sección cerrada | `numero_celdas`, `ancho_celda`, `altura_celda` |

**Parámetros**

| Parámetro | Unidad | Rango típico | Obl. | Nota |
|---|---|---|---|---|
| `seccion` | enum | rectangular, trapezoidal, triangular, circular | ● | Discriminante de forma |
| `base` | m | 0.20–20.0 | ● | Plantilla |
| `talud` | H:V | 0–3 | ◐ | Trapezoidal y triangular |
| `altura_muro` | m | 0.20–5.00 | ◐ | Rectangular, trapezoidal, cuneta U |
| `diametro` | m | 0.30–3.00 | ◐ | Circular |
| `ancho_corona` | m | 0.30–3.00 | ◐ | Cuneta V |
| `profundidad` | m | 0.10–1.00 | ◐ | Cuneta V |
| `borde_libre` | m | 0.10–1.00 | ● | |
| `espesor_revestimiento` | m | 0.05–0.40 | ○ | 0 = tierra |
| `revestimiento` | enum | tierra, hormigon, mamposteria, geosintetico | ● | B-MATERIAL |
| `numero_celdas` | int | 1–4 | ◐ | Box culvert |
| `ancho_celda` | m | 0.60–6.00 | ◐ | |
| `altura_celda` | m | 0.60–4.00 | ◐ | |
| `pendiente` | m/m | 0.0001–0.05 | ● | |
| `longitud` | m | 0.5–5 000 | D | |
| `tipo_junta` | enum | ninguna, junta_fria, sello | ○ | Cada 3–6 m |

**Bloques:** B-COTAS, B-MATERIAL, B-ESTADO.

#### 3.1.9 `saneamiento.sifon` — Sifón

`SF` · polilínea 3D · arco · 4 subtipos · partes: tubo, codo, camara, purga

Cruza un obstáculo pasando por debajo: río, quebrada, vía, otra conducción. La forma es un perfil en U con dos cámaras en los extremos.

**Subtipos**

| # | Subtipo | Nombre técnico | Cuándo se usa | Parámetros distintivos |
|---|---|---|---|---|
| 1 | `invertido` | Sifón invertido | Cruce de cauce con flujo a presión y velocidad de arrastre | `numero_conductos`, `diametro`, `cota_punto_bajo` |
| 2 | `normal` | Sifón normal | Cruce con depresión y lámina libre; poco usado en redes | `diametro`, `cota_punto_bajo` |
| 3 | `cruce` | Sifón de cruce | Caso particular de cruce con otra red o vía | `diametro`, `longitud` |
| 4 | `lavador` | Sifón lavador | Tramo que se vacía de golpe para arrastrar sedimentos | `altura_carga`, `diametro_camara` |

**Parámetros**

| Parámetro | Unidad | Rango típico | Obl. | Nota |
|---|---|---|---|---|
| `numero_conductos` | int | 1–4 | ● | 2 es lo habitual por redundancia |
| `diametro` | mm | 100–1 200 | ● | |
| `longitud` | m | 5–500 | ● | |
| `cota_entrada` | m | — | ● | |
| `cota_salida` | m | — | ● | |
| `cota_punto_bajo` | m | — | ● | |
| `largo_camara_entrada` | m | 1.00–4.00 | ● | |
| `ancho_camara_entrada` | m | 1.00–3.00 | ● | |
| `altura_camara_entrada` | m | 1.50–6.00 | ● | |
| `largo_camara_salida` | m | 1.00–4.00 | ● | |
| `ancho_camara_salida` | m | 1.00–3.00 | ● | |
| `altura_camara_salida` | m | 1.50–6.00 | ● | |
| `angulo_entrada` | ° | 0–180 | ○ | |
| `dispositivo_purga` | bool | — | ● | |
| `diametro_purga` | mm | 50–200 | ◐ | |
| `altura_carga` | m | 0.50–3.00 | ◐ | Sifón lavador |

**Bloques:** B-COTAS, B-MATERIAL, B-TAPA, B-ESTADO.

#### 3.1.10 `saneamiento.descarga` — Descarga, alivio y vertedero

`DS` · punto o línea · nodo de entrega · 13 subtipos · partes: muro, cresta, canaleta, difusor, tanque, enrocado, compuerta

Entrega el agua al cuerpo receptor de forma controlada y disipada. **Aviso de alcance:** "vertedero" designa en español al menos cuatro cosas (entrega final, alivio, aliviadero de cresta y relleno sanitario). Aquí se cubren las tres primeras; la cuarta no es hidráulica y queda fuera (§7, R1).

**Subtipos**

| # | Subtipo | Nombre técnico | Cuándo se usa | Parámetros distintivos |
|---|---|---|---|---|
| 1 | `emisario_muro` | Emisario en muro | Entrega contra un muro de contención en el cauce | `cota_entrega`, `diametro`, `altura_muro` |
| 2 | `emisario_talud` | Emisario en talud | Entrega sobre un talud; el agua baja por una canaleta | `cota_entrega`, `longitud_canaleta`, `talud` |
| 3 | `emisario_subacuatico` | Emisario subacuático | Descarga sumergida en río, estero o mar | `longitud_subacuatica`, `numero_orificios`, `diametro_orificio` |
| 4 | `alivio_lateral` | Alivio lateral | Descarga excedentes cuando el nivel supera la cresta | `longitud_vertedero`, `cota_cresta`, `tipo_cresta` |
| 5 | `alivio_camara` | Alivio con cámara | Alivio que nace en una cámara con compuerta o vertedero | `cota_cresta`, `tipo_control`, `diametro_salida` |
| 6 | `rebose_tanque` | Rebose de tanque | Controla el nivel máximo de un tanque | `cota_cresta`, `longitud_vertedero`, `tipo_rebose` |
| 7 | `vertedero_cresta` | Vertedero de cresta | Aliviadero de excedencias con lámina vertiente | `longitud_vertedero`, `tipo_cresta`, `altura_cresta` |
| 8 | `disipador_salto` | Disipador de salto | Caída vertical grande; el agua golpea una poza | `altura_caida`, `largo_poza`, `ancho_poza` |
| 9 | `disipador_tanque` | Tanque amortiguador | Alta energía; poza con dados o umbral | `largo_tanque`, `ancho_tanque`, `numero_dados` |
| 10 | `disipador_enrocado` | Enrocado de entrega | Entrega a un cauce con protección de escollera | `largo_enrocado`, `ancho_enrocado`, `espesor_enrocado` |
| 11 | `pozo_infiltracion` | Pozo de infiltración | Infiltra al terreno; sin cuerpo receptor superficial | `diametro`, `profundidad`, `area_infiltracion` |
| 12 | `descarga_emergencia` | Descarga de emergencia | By-pass de una estación de bombeo o de una PTAR | `diametro`, `cota_entrega`, `tipo_valvula` |
| 13 | `cso_reja` | Aliviadero con reja (CSO) | Sistemas combinados; retiene sólidos antes del alivio | `longitud_reja`, `abertura_reja`, `numero_barras` |

**Parámetros**

| Parámetro | Unidad | Rango típico | Obl. | Nota |
|---|---|---|---|---|
| `tipo_entrega` | enum | muro, talud, subacuatica, infiltracion | ● | |
| `cota_entrega` | m | — | ● | |
| `diametro` | mm | 100–1 500 | ● | |
| `cuerpo_receptor` | ref | — | ● | Referencia a `comun.cuerpo_receptor` |
| `longitud_vertedero` | m | 0.30–20.0 | ◐ | |
| `cota_cresta` | m | — | ◐ | |
| `tipo_cresta` | enum | delgada, ancha, redondeada | ◐ | |
| `altura_cresta` | m | 0.10–2.00 | ◐ | |
| `tipo_control` | enum | vertedero, compuerta, orificio | ◐ | Alivio con cámara |
| `tipo_rebose` | enum | libre, sifonico | ◐ | Rebose de tanque |
| `longitud_subacuatica` | m | 5–500 | ◐ | |
| `numero_orificios` | int | 1–40 | ◐ | Difusor |
| `diametro_orificio` | mm | 50–300 | ◐ | |
| `separacion_orificios` | m | 0.50–5.00 | ◐ | |
| `altura_caida` | m | 0.50–10.0 | ◐ | Disipador de salto |
| `largo_poza` | m | 1.00–10.0 | ◐ | |
| `ancho_poza` | m | 1.00–10.0 | ◐ | |
| `largo_tanque` | m | 1.00–12.0 | ◐ | Tanque amortiguador |
| `ancho_tanque` | m | 1.00–12.0 | ◐ | |
| `numero_dados` | int | 0–12 | ◐ | |
| `largo_enrocado` | m | 1.00–20.0 | ◐ | |
| `espesor_enrocado` | m | 0.30–1.20 | ◐ | |
| `longitud_reja` | m | 0.50–6.00 | ◐ | CSO |
| `abertura_reja` | mm | 6–25 | ◐ | |
| `altura_muro` | m | 0.50–5.00 | ◐ | Emisario en muro |

**Bloques:** B-COTAS, B-MATERIAL, B-ESTADO.

#### 3.1.11 `saneamiento.bombeo` — Estación de bombeo

`EB` · punto con sólido derivado · nodo · 5 subtipos · partes: camara, rejilla, tapa, tubo, caseta, bomba

Eleva el agua cuando la gravedad no alcanza. Se dibuja como un volumen visitable con reja, tapas y la tubería de impulsión saliendo; las bombas son un volumen interior que solo aparece en LOD2.

**Subtipos**

| # | Subtipo | Nombre técnico | Cuándo se usa | Parámetros distintivos |
|---|---|---|---|---|
| 1 | `camara_humeda` | Cámara húmeda | Pozo que recibe y bombea; el más común en alcantarillado | `diametro`, `profundidad`, `numero_bombas` |
| 2 | `camara_humeda_y_seca` | Cámara húmeda + cámara seca | Bombeo grande con sala de máquinas visitable | `largo`, `ancho`, `numero_bombas` |
| 3 | `paquete_compacto` | Estación compacta (paquete) | Solución prefabricada de PRFV/PEAD para caudales pequeños | `diametro`, `altura`, `numero_bombas` |
| 4 | `pozo_eyeccion` | Pozo de eyección | Vivienda o lote sin gravedad; eyección neumática | `diametro`, `altura`, `volumen_util` |
| 5 | `estacion_impulsion` | Estación de impulsión con cámara de válvulas | Impulsión de agua potable o de AL con válvulas de retención y compuerta | `largo`, `ancho`, `numero_valvulas` |

**Parámetros**

| Parámetro | Unidad | Rango típico | Obl. | Nota |
|---|---|---|---|---|
| `diametro` | m | 1.20–6.00 | ◐ | Cámaras cilíndricas |
| `largo` | m | 1.50–12.0 | ◐ | Cámaras rectangulares |
| `ancho` | m | 1.50–8.00 | ◐ | |
| `profundidad` | m | 1.50–10.0 | ● | |
| `espesor_pared` | m | 0.20–0.50 | ● | |
| `cota_tapa` | m | — | ● | |
| `cota_fondo` | m | — | ● | |
| `numero_bombas` | int | 1–6 | ● | |
| `tipo_bomba` | enum | sumergible, centrifuga_horizontal, vertical, eyector | ● | Cambia la silueta interior |
| `diametro_impulsion` | mm | 50–600 | ● | |
| `diametro_succion` | mm | 50–600 | ○ | |
| `cota_impulsion` | m | — | ● | |
| `con_reja` | bool | — | ● | Reja de gruesos a la entrada |
| `largo_reja` | m | 0.50–3.00 | ◐ | |
| `ancho_reja` | m | 0.30–2.00 | ◐ | |
| `numero_tapas` | int | 1–4 | ○ | |
| `largo_caseta` | m | 2.00–10.0 | ○ | Caseta de control |
| `ancho_caseta` | m | 2.00–6.00 | ○ | |
| `altura_caseta` | m | 2.20–4.00 | ○ | |
| `nivel_arranque` | m | — | ○ | Atributo de operación, no de forma |
| `nivel_parada` | m | — | ○ | |

**Bloques:** B-COTAS, B-MATERIAL, B-TAPA, B-ESCALERA, B-ESTADO.

#### 3.1.12 `saneamiento.acometida` — Acometida de alcantarillado

`AC` · polilínea 3D · arco terminal · 5 subtipos · partes: tubo, conexion

Conecta el predio al colector. Es un arco corto con una caja de revisión en el límite del predio. (La acometida de agua potable es otra familia: `agua.acometida`, §3.2.11.)

**Subtipos**

| # | Subtipo | Nombre técnico | Cuándo se usa | Parámetros distintivos |
|---|---|---|---|---|
| 1 | `domiciliaria` | Acometida domiciliaria | Vivienda unifamiliar; DN 100–150 | `dn_mm`, `pendiente`, `caja` |
| 2 | `pluvial` | Acometida pluvial | Patio o cubierta que descarga a la red pluvial | `dn_mm`, `pendiente` |
| 3 | `industrial` | Acometida industrial | Caudal y carga contaminante mayores; exige medición y tratamiento | `dn_mm`, `trampa_grasa` |
| 4 | `colectiva` | Acometida colectiva | Multifamiliar o conjunto; una sola conexión con caja de empate | `dn_mm`, `numero_usuarios`, `numero_entradas` |
| 5 | `comercial` | Acometida comercial | Locales y restaurantes; suele llevar trampa de grasa | `dn_mm`, `trampa_grasa` |

**Parámetros**

| Parámetro | Unidad | Rango típico | Obl. | Nota |
|---|---|---|---|---|
| `dn_mm` | mm | 100–200 | ● | |
| `material` | enum | PVC, PEAD, HG, hormigon | ● | |
| `longitud` | m | 2–60 | D | |
| `pendiente` | m/m | 0.01–0.05 | ● | Mínimo 1 %; 2 % recomendado |
| `cota_inicio` | m | — | ● | En el predio |
| `cota_fin` | m | — | ● | En el colector |
| `diametro_conexion` | mm | 100–400 | ● | Empalme al colector |
| `tipo_conexion` | enum | directa, silla, pozo | ● | Cambia el dibujo del empalme |
| `caja` | ref | — | ● | Referencia a `saneamiento.caja` |
| `numero_usuarios` | int | 1–200 | ○ | |
| `trampa_grasa` | ref | — | ◐ | Industrial y comercial |
| `profundidad_empalme` | m | 0.60–4.00 | ○ | |

**Bloques:** B-COTAS, B-MATERIAL, B-ESTADO.

### 3.2 Agua potable (11 familias, 76 subtipos)

#### 3.2.1 `agua.tuberia` — Tubería de agua potable

`TB` · polilínea 3D · arco · 8 subtipos · partes: tubo, junta, brida, revestimiento

Conduce agua a presión desde la captación hasta el medidor. La forma la definen DN, OD, clase de presión y el eje; el material cambia el color y el tipo de junta.

**Subtipos**

| # | Subtipo | Nombre técnico | Cuándo se usa | Parámetros distintivos |
|---|---|---|---|---|
| 1 | `conduccion` | Conducción | Captación → planta o tanque; puede ir a presión o en lámina libre | `dn_mm`, `clase_presion` |
| 2 | `aduccion` | Aducción | Tanque → red, o planta → tanque | `dn_mm` ≥ 100, `clase_presion` |
| 3 | `distribucion_primaria` | Red de distribución primaria | Malla principal; DN ≥ 100 | `dn_mm`, `clase_presion` |
| 4 | `distribucion_secundaria` | Red de distribución secundaria | Calles interiores; DN 50–100 | `dn_mm` |
| 5 | `ramal` | Ramal de distribución | Alimenta una manzana o un grupo de acometidas | `dn_mm` |
| 6 | `impulsion` | Impulsión | Bombeo → tanque o red | `dn_mm`, `clase_presion` |
| 7 | `succion` | Succión | Tanque o captación → bomba; tramo corto | `dn_mm`, `longitud` |
| 8 | `agua_cruda` | Línea de agua cruda | Captación → PTAP; material y revestimiento distintos | `dn_mm`, `revestimiento` |

**Parámetros**

| Parámetro | Unidad | Rango típico | Obl. | Nota |
|---|---|---|---|---|
| `dn_mm` | mm | 75–600 | ● | §5.1 |
| `od_mm` | mm | — | ● | |
| `id_mm` | mm | — | ● | |
| `espesor_pared` | mm | 2.0–20 | ● | |
| `material` | enum | PVC, PVC_O, PEAD, HD, acero, GRP, fibrocemento | ● | B-MATERIAL |
| `tipo_junta` | enum | espiga_campana, bridada, soldada, electrofusion, mecanica | ● | |
| `clase_presion` | enum | PN6, PN8, PN10, PN12.5, PN16, PN20 | ● | |
| `seccion` | enum | circular | ● | Fijo; se declara por simetría con AL |
| `longitud` | m | 0.5–10 000 | D | |
| `cota_clave_inicio` | m | — | ● | |
| `cota_clave_fin` | m | — | ● | |
| `pendiente` | m/m | −0.5–0.5 | ○ | |
| `cota_terreno` | m | — | ○ | |
| `profundidad` | m | 0.60–3.00 | ○ | Recubrimiento; se dibuja en el perfil |
| `tipo_soporte` | enum | cama_arena, cama_grava, dado_hormigon, colgado | ○ | |
| `revestimiento` | enum | ninguno, epoxico, PE, zinc | ○ | |
| `con_dado_anclaje` | bool | — | ○ | Enlaza con `agua.dado_anclaje` |

**Bloques:** B-COTAS, B-MATERIAL, B-ESTADO.

#### 3.2.2 `agua.valvula` — Válvula

`VL` · punto con sólido derivado · dispositivo de arco · 12 subtipos · partes: cuerpo, brida, bonete, vastago, volante, disco, piston, flotador, piloto

Aísla, controla, protege o alivia. Es la familia donde más se nota la decisión D2: **doce subtipos funcionales** y las variantes de fabricación como atributos (`tipo_cuerpo`, `tipo_bonete`, `tipo_accionamiento`). Así se dibuja un cuerpo con bridas, bonete, vástago y volante —que es lo que se reconoce— sin multiplicar tipos.

**Subtipos**

| # | Subtipo | Nombre técnico | Cuándo se usa | Parámetros distintivos |
|---|---|---|---|---|
| 1 | `compuerta` | Válvula de compuerta | Aislamiento todo/nada en distribución e impulsión | `tipo_cuerpo`, `altura_vastago` |
| 2 | `mariposa` | Válvula de mariposa | Aislamiento y control en DN grandes | `tipo_cuerpo`, `angulo_disco` |
| 3 | `retencion` | Válvula de retención | Impide el reverso en impulsiones | `tipo_clapeta`, `angulo_apertura` |
| 4 | `aire` | Válvula de aire | Admisión y expulsión en puntos altos | `diametro_orificio`, `numero_orificios` |
| 5 | `purga` | Válvula de purga | Vacía puntos bajos y arrastra sedimentos | `diametro_purga`, `tipo_purga` |
| 6 | `reduccion_presion` | Válvula reductora de presión (PRV) | Mantiene una presión máxima aguas abajo | `diametro_piloto`, `tipo_piloto` |
| 7 | `sostenedora` | Válvula sostenedora de presión | Garantiza presión mínima aguas arriba | `diametro_piloto` |
| 8 | `reguladora_caudal` | Válvula reguladora de caudal | Limita el caudal a un sector o a una derivación | `diametro_piloto` |
| 9 | `alivio` | Válvula de alivio / seguridad | Protege contra sobrepresión | `presion_tarado`, `diametro_orificio` |
| 10 | `flotador` | Válvula de flotador | Controla el llenado de un tanque | `diametro_flotador`, `longitud_brazo` |
| 11 | `pie` | Válvula de pie | Retención en la succión de una bomba | `diametro_colador`, `altura_colador` |
| 12 | `altitud` | Válvula de altitud | Controla el nivel de un tanque a distancia | `diametro_piloto`, `altura_torre` |

**Parámetros**

| Parámetro | Unidad | Rango típico | Obl. | Nota |
|---|---|---|---|---|
| `dn_mm` | mm | 25–1 200 | ● | |
| `tipo_conexion` | enum | bridada, roscada, espiga, ranurada, wafer | ● | |
| `tipo_cuerpo` | enum | husillo_ascendente, husillo_no_ascendente, wafer, lug, doble_brida, globo, angular | ● | Variante de fabricación |
| `tipo_bonete` | enum | estriado, liso, con_bypass | ○ | |
| `longitud_cara_a_cara` | mm | 150–1 200 | ● | §5.5; sin ella no hay encaje en el dibujo |
| `altura_total` | mm | 150–2 000 | ● | |
| `diametro_cuerpo` | mm | 60–1 500 | ● | |
| `diametro_volante` | mm | 100–600 | ○ | Sin volante si es motorizada |
| `altura_vastago` | mm | 100–1 200 | ◐ | Compuerta de husillo ascendente |
| `orientacion_vastago` | enum | vertical, horizontal, inclinado | ○ | |
| `numero_bridas` | int | 0–2 | ● | 0 si es espiga o ranurada |
| `diametro_brida` | mm | 100–1 500 | ◐ | |
| `espesor_brida` | mm | 15–60 | ◐ | |
| `tipo_accionamiento` | enum | manual, electrico, hidraulico, neumatico | ● | |
| `angulo_disco` | ° | 0–90 | ◐ | Mariposa; posición dibujada |
| `tipo_clapeta` | enum | simple, doble, muelle | ◐ | Retención |
| `angulo_apertura` | ° | 0–90 | ◐ | |
| `diametro_orificio` | mm | 3–50 | ◐ | Aire, alivio |
| `numero_orificios` | int | 1–4 | ◐ | |
| `diametro_purga` | mm | 25–150 | ◐ | |
| `tipo_purga` | enum | manual, automatica | ◐ | |
| `diametro_piloto` | mm | 6–50 | ◐ | PRV, sostenedora, altitud |
| `tipo_piloto` | enum | simple, doble, con_solenoide | ○ | |
| `diametro_flotador` | mm | 100–300 | ◐ | Flotador |
| `longitud_brazo` | mm | 200–1 000 | ◐ | |
| `diametro_colador` | mm | 100–600 | ◐ | Pie |
| `altura_colador` | mm | 100–400 | ◐ | |
| `altura_torre` | m | 0.50–3.00 | ◐ | Altitud |
| `posicion` | enum | abierta, cerrada, parcial | ○ | Se dibuja el disco en esa posición |

**Bloques:** B-COTAS, B-MATERIAL, B-ESTADO.

#### 3.2.3 `agua.llave` — Llave de acometida

`LL` · punto con sólido derivado · dispositivo · 4 subtipos · partes: collarin, cuerpo, vastago, volante

Dispositivo en la frontera red–usuario. No es una válvula de red: su función es cortar el paso a un predio. El hidrante salió de esta familia (§3.2.4) porque su geometría y su especificación no tienen nada que ver.

**Subtipos**

| # | Subtipo | Nombre técnico | Cuándo se usa | Parámetros distintivos |
|---|---|---|---|---|
| 1 | `corporacion` | Llave de corporación | Sobre la tubería de la red, con collarín de toma | `dn_red`, `dn_acometida`, `tipo_collarin` |
| 2 | `banqueta` | Llave de banqueta | Antes del medidor, en el límite del predio | `dn_mm`, `profundidad`, `tipo_cierre` |
| 3 | `pase` | Llave de paso domiciliaria | Corte interior, después del medidor | `dn_mm`, `tipo_cierre` |
| 4 | `tapon` | Tapón terminal | Cierra un ramal sin uso | `dn_mm`, `tipo_tapon` |

**Parámetros**

| Parámetro | Unidad | Rango típico | Obl. | Nota |
|---|---|---|---|---|
| `dn_mm` | mm | 13–100 | ● | 1/2"–4" |
| `dn_red` | mm | 50–400 | ◐ | Corporación |
| `dn_acometida` | mm | 13–50 | ◐ | |
| `tipo_collarin` | enum | silla, abrazadera, te_corporacion | ◐ | |
| `tipo_cierre` | enum | bola, compuerta, tapon | ● | Cambia la silueta |
| `profundidad` | m | 0.30–1.50 | ● | |
| `longitud_vastago` | mm | 100–800 | ○ | |
| `diametro_volante` | mm | 60–200 | ○ | |
| `tiene_caja` | bool | — | ○ | Caja o arqueta de protección |
| `tipo_tapon` | enum | ciego, bridado, roscado | ◐ | |

**Bloques:** B-COTAS, B-MATERIAL, B-ESTADO.

#### 3.2.4 `agua.hidrante` — Hidrante y boca de riego

`HD` · punto con sólido derivado · dispositivo de servicio · 4 subtipos · partes: cuerpo, boca, tapa, vastago, collarin

Toma de agua para bomberos o riego. Se dibuja entero porque es un objeto de calle: cuerpo, bocas, tapas y, si va enterrado, su arqueta.

**Subtipos**

| # | Subtipo | Nombre técnico | Cuándo se usa | Parámetros distintivos |
|---|---|---|---|---|
| 1 | `columna` | Hidrante de columna | Sobre acera o calzada, visible | `numero_bocas`, `altura_sobre_rasante` |
| 2 | `arqueta` | Hidrante de arqueta | Enterrado bajo tapa; no estorba en la acera | `largo_arqueta`, `ancho_arqueta`, `profundidad` |
| 3 | `boca_riego` | Boca de riego | Riego de parques y limpieza de calles | `dn_mm`, `altura_sobre_rasante` |
| 4 | `tipo_tapon` | Hidrante tipo tampón | Al ras del suelo, en calzadas estrechas | `diametro_tapa`, `profundidad` |

**Parámetros**

| Parámetro | Unidad | Rango típico | Obl. | Nota |
|---|---|---|---|---|
| `dn_mm` | mm | 65–150 | ● | |
| `numero_bocas` | int | 1–3 | ● | 2 × 65 + 1 × 100 es lo habitual |
| `diametro_bocas` | mm | 65–100 | ● | |
| `altura_sobre_rasante` | m | 0.50–1.20 | ◐ | Columna y boca de riego |
| `diametro_cuerpo` | mm | 100–250 | ● | |
| `altura_total` | mm | 500–1 500 | ● | |
| `tipo_tapa` | enum | roscada, bayoneta, ciega | ○ | |
| `diametro_tapa` | mm | 100–200 | ○ | |
| `valvula_auxiliar` | bool | — | ○ | |
| `sentido_salida` | enum | frontal, lateral, radial | ○ | |
| `largo_arqueta` | m | 0.60–2.00 | ◐ | Hidrante de arqueta |
| `ancho_arqueta` | m | 0.60–1.50 | ◐ | |
| `profundidad` | m | 0.60–1.80 | ◐ | |

**Bloques:** B-COTAS, B-MATERIAL, B-ESTADO.

#### 3.2.5 `agua.medidor` — Medidor

`MD` · punto con sólido derivado · dispositivo de arco · 7 subtipos · partes: cuerpo, esfera, tapa, conexion

Mide el caudal, para facturar o para controlar pérdidas. Su forma importa porque define el espacio de la arqueta y el detalle del nudo.

**Subtipos**

| # | Subtipo | Nombre técnico | Cuándo se usa | Parámetros distintivos |
|---|---|---|---|---|
| 1 | `chorro_multiple` | Medidor de chorro múltiple | Domiciliario, DN 15–25 | `dn_mm`, `longitud_total` |
| 2 | `volumetrico` | Medidor volumétrico | Caudales pequeños y muy variables; más preciso | `dn_mm`, `diametro_camara` |
| 3 | `woltman` | Medidor Woltman (turbina) | Macromedición DN 50–300 | `dn_mm`, `longitud_total` |
| 4 | `electromagnetico` | Medidor electromagnético | Conducciones y plantas; sin partes móviles | `dn_mm`, `longitud_total` |
| 5 | `ultrasonico` | Medidor ultrasónico | Macromedición y control de pérdidas | `dn_mm`, `longitud_total` |
| 6 | `insercion` | Medidor de inserción | Se instala en tubería existente sin cortar | `dn_tuberia`, `longitud_sonda` |
| 7 | `compuesto` | Medidor compuesto | Une chorro múltiple y Woltman para caudal muy variable | `dn_mm`, `dn_bypass` |

**Parámetros**

| Parámetro | Unidad | Rango típico | Obl. | Nota |
|---|---|---|---|---|
| `dn_mm` | mm | 15–400 | ● | |
| `longitud_total` | mm | 110–1 500 | ● | Distancia entre conexiones |
| `diametro_cuerpo` | mm | 50–500 | ● | |
| `altura_cuerpo` | mm | 80–500 | ● | |
| `tipo_conexion` | enum | roscada, bridada, ranurada | ● | |
| `orientacion` | enum | horizontal, vertical, inclinada | ○ | |
| `diametro_esfera` | mm | 40–150 | ○ | Cara visible del medidor |
| `numero_esferas` | int | 1–6 | ○ | |
| `dn_bypass` | mm | 15–100 | ◐ | Compuesto y medidores con bypass |
| `con_bypass` | bool | — | ○ | |
| `dn_tuberia` | mm | 100–2 000 | ◐ | Inserción |
| `longitud_sonda` | mm | 100–600 | ◐ | |
| `diametro_camara` | mm | 50–120 | ◐ | Volumétrico |

**Bloques:** B-COTAS, B-MATERIAL, B-ESTADO.

#### 3.2.6 `agua.arqueta` — Arqueta

`AQ` · punto con sólido derivado · nodo · 6 subtipos · partes: solera, muro, tapa, marco, tubo

Es el "pozo" del agua potable: aloja el medidor, la válvula o la purga bajo el nivel del suelo. Sin esta familia, medidores y válvulas quedan flotando en el modelo.

**Subtipos**

| # | Subtipo | Nombre técnico | Cuándo se usa | Parámetros distintivos |
|---|---|---|---|---|
| 1 | `medidor` | Arqueta de medidor | Toda acometida con medidor y tapa a la calle | `largo`, `ancho`, `profundidad` |
| 2 | `valvula` | Arqueta de válvula | Válvulas de red enterradas en calzada | `largo`, `ancho`, `numero_valvulas` |
| 3 | `hidrante` | Arqueta de hidrante | Hidrante enterrado (§3.2.4) | `largo`, `ancho` |
| 4 | `purga` | Arqueta de purga | Punto bajo con purga y desagüe | `largo`, `ancho`, `diametro_purga` |
| 5 | `aire` | Arqueta de aire | Punto alto con ventosa | `diametro`, `altura`, `diametro_ventosa` |
| 6 | `derivacion` | Arqueta de derivación | Reparto a dos o más ramales | `numero_salidas`, `diametro_tuberia` |

**Parámetros**

| Parámetro | Unidad | Rango típico | Obl. | Nota |
|---|---|---|---|---|
| `largo` | m | 0.40–2.50 | ● | |
| `ancho` | m | 0.40–1.50 | ● | |
| `profundidad` | m | 0.40–2.00 | ● | |
| `espesor_pared` | m | 0.10–0.25 | ● | |
| `espesor_solera` | m | 0.10–0.20 | ● | |
| `cota_tapa` | m | — | ● | |
| `cota_fondo` | m | — | ● | |
| `numero_tapas` | int | 1–2 | ○ | |
| `diametro_tuberia` | mm | 13–600 | ● | |
| `con_drenaje` | bool | — | ○ | Sumidero interior |
| `diametro` | m | 0.60–1.50 | ◐ | Arquetas cilíndricas |
| `altura` | m | 0.60–2.00 | ◐ | |
| `numero_valvulas` | int | 1–4 | ◐ | |
| `diametro_purga` | mm | 25–150 | ◐ | |
| `diametro_ventosa` | mm | 25–100 | ◐ | |

**Bloques:** B-COTAS, B-MATERIAL, B-TAPA, B-ESCALERA, B-ESTADO.

#### 3.2.7 `agua.tanque` — Tanque y reservorio

`TQ` · punto con polígono de huella · nodo con volumen · 8 subtipos · partes: fuste, cuerpo, cubierta, losa, escalera, baranda, respiradero, tubo

Almacena y regula. Se dibuja como un volumen con su cubierta y su escalera; el nivel de agua (NAMO/NAMI) es un plano horizontal que solo aparece en corte.

**Subtipos**

| # | Subtipo | Nombre técnico | Cuándo se usa | Parámetros distintivos |
|---|---|---|---|---|
| 1 | `apoyado_cilindrico` | Tanque apoyado cilíndrico | El más común en cabeceras municipales | `diametro`, `altura_util` |
| 2 | `apoyado_rectangular` | Tanque apoyado rectangular | Terrenos estrechos o volúmenes grandes | `largo`, `ancho`, `altura_util` |
| 3 | `elevado` | Tanque elevado (torre) | Presuriza una zona sin cerro | `altura_fuste`, `diametro_fuste` |
| 4 | `enterrado` | Tanque enterrado | Bajo parque o patio; sin impacto visual | `profundidad`, `tipo_cubierta` |
| 5 | `semienterrado` | Tanque semienterrado | Aprovecha un talud | `profundidad`, `altura_expuesta` |
| 6 | `cisterna` | Cisterna | Reserva de edificio o de planta | `largo`, `ancho`, `altura_util` |
| 7 | `contacto` | Tanque de contacto (cloración) | Retención para desinfección | `largo`, `numero_bafles`, `altura_util` |
| 8 | `crudo` | Tanque de agua cruda | Reserva antes de la PTAP | `diametro`, `altura_util` |

**Parámetros**

| Parámetro | Unidad | Rango típico | Obl. | Nota |
|---|---|---|---|---|
| `diametro` | m | 1.50–40.0 | ◐ | Cilíndrico |
| `largo` | m | 2.00–60.0 | ◐ | Rectangular |
| `ancho` | m | 2.00–40.0 | ◐ | |
| `altura_util` | m | 1.00–12.0 | ● | |
| `borde_libre` | m | 0.30–1.00 | ● | |
| `espesor_pared` | m | 0.15–0.60 | ● | |
| `espesor_losa` | m | 0.15–0.60 | ● | |
| `cota_fondo` | m | — | ● | |
| `cota_namo` | m | — | ● | Nivel máximo de operación |
| `cota_nami` | m | — | ● | Nivel mínimo de operación |
| `cota_rebose` | m | — | ● | |
| `tipo_cubierta` | enum | losa, cupula, conica, ninguna | ● | Cambia la silueta |
| `altura_fuste` | m | 3.00–30.0 | ◐ | Elevado |
| `diametro_fuste` | m | 1.00–8.00 | ◐ | |
| `numero_columnas` | int | 0–8 | ◐ | Elevado sobre columnas |
| `con_escalera` | bool | — | ○ | |
| `con_baranda` | bool | — | ○ | |
| `numero_respiraderos` | int | 0–6 | ○ | |
| `numero_bafles` | int | 0–10 | ◐ | Tanque de contacto |
| `altura_expuesta` | m | 0.50–4.00 | ◐ | Semienterrado |

**Bloques:** B-COTAS, B-MATERIAL, B-ESTADO.

#### 3.2.8 `agua.captacion` — Captación

`CP` · punto o polígono · nodo fuente · 8 subtipos · partes: cuerpo, rejilla, muro, caseta, brocal, tubo, cerco

Nodo fuente del sistema. Ocho subtipos que en obra no se parecen entre sí: una caja de manantial y una toma de río con presa son objetos distintos.

**Subtipos**

| # | Subtipo | Nombre técnico | Cuándo se usa | Parámetros distintivos |
|---|---|---|---|---|
| 1 | `manantial_ladera` | Manantial de ladera | Nace en un talud; caja con vertedero y tapa | `largo`, `ancho`, `altura` |
| 2 | `manantial_fondo` | Manantial de fondo | Nace en el suelo; caja enterrada con rejilla | `diametro`, `profundidad` |
| 3 | `pozo_profundo` | Pozo profundo | Acuífero profundo; entubado y con sello sanitario | `diametro_entubado`, `profundidad` |
| 4 | `pozo_somero` | Pozo somero / freático | Napa alta, poca profundidad | `diametro_entubado`, `profundidad` |
| 5 | `galeria_filtrante` | Galería filtrante | Caudal difuso en un aluvial; tubos drenantes en zanja | `longitud_galeria`, `diametro_galeria` |
| 6 | `toma_rio` | Toma de río con presa | Río con caudal permanente; presa de derivación | `ancho_presa`, `altura_presa` |
| 7 | `toma_quebrada` | Toma de quebrada | Quebrada estacional; bocatoma de fondo | `ancho_bocatoma`, `altura_bocatoma` |
| 8 | `rejilla_fondo` | Rejilla de fondo | Capta bajo el lecho con rejilla sumergida | `longitud_rejilla`, `ancho_rejilla` |

**Parámetros**

| Parámetro | Unidad | Rango típico | Obl. | Nota |
|---|---|---|---|---|
| `largo` | m | 0.60–10.0 | ◐ | Caja de captación |
| `ancho` | m | 0.60–8.00 | ◐ | |
| `altura` | m | 0.60–6.00 | ◐ | |
| `diametro` | m | 0.60–3.00 | ◐ | Caja cilíndrica |
| `profundidad` | m | 1.00–300 | ◐ | Pozo |
| `diametro_entubado` | mm | 100–600 | ◐ | Pozo |
| `diametro_perforacion` | mm | 150–1 000 | ◐ | Pozo |
| `altura_brocal` | m | 0.20–1.00 | ○ | Pozo |
| `sello_sanitario` | bool | — | ● | Pozo: impide contaminación por el anular |
| `longitud_galeria` | m | 10–200 | ◐ | Galería filtrante |
| `diametro_galeria` | mm | 100–400 | ◐ | |
| `ancho_presa` | m | 2.00–60.0 | ◐ | Toma de río |
| `altura_presa` | m | 0.50–5.00 | ◐ | |
| `ancho_bocatoma` | m | 1.00–10.0 | ◐ | Toma de quebrada |
| `altura_bocatoma` | m | 0.50–3.00 | ◐ | |
| `longitud_rejilla` | m | 0.50–5.00 | ◐ | Rejilla de fondo |
| `ancho_rejilla` | m | 0.50–5.00 | ◐ | |
| `largo_caseta` | m | 2.00–8.00 | ○ | Caseta de protección |
| `ancho_caseta` | m | 2.00–6.00 | ○ | |
| `altura_caseta` | m | 2.20–3.50 | ○ | |

**Bloques:** B-COTAS, B-MATERIAL, B-TAPA, B-ESTADO.

#### 3.2.9 `agua.planta` — Planta de tratamiento

`PT` · polígono de huella · nodo de proceso · 11 subtipos · partes: nave, tanque, caseta, tubo, cerco

Se dibuja como **volumetría**, no como proceso: la huella, las naves y los tanques. El tren de tratamiento es un atributo de texto; el visor no simula el proceso.

**Subtipos**

| # | Subtipo | Nombre técnico | Cuándo se usa | Parámetros distintivos |
|---|---|---|---|---|
| 1 | `ptap_convencional` | PTAP convencional | Mezcla, floculación, sedimentación, filtración y desinfección | `numero_unidades`, `largo`, `ancho` |
| 2 | `ptap_compacta` | PTAP compacta | Caudales pequeños; módulo prefabricado | `largo`, `ancho`, `altura` |
| 3 | `ptap_filtracion_directa` | PTAP de filtración directa | Agua de baja turbiedad | `numero_unidades`, `largo` |
| 4 | `ptar_lodos_activos` | PTAR de lodos activados | Tratamiento secundario convencional | `numero_unidades`, `diametro_tanque` |
| 5 | `ptar_laguna` | PTAR de lagunas aireadas | Clima cálido, terreno disponible | `numero_lagunas`, `largo_laguna` |
| 6 | `ptar_uasb` | PTAR con reactor UASB | Alta carga orgánica; poco volumen | `numero_unidades`, `diametro_tanque` |
| 7 | `laguna_estabilizacion` | Laguna de estabilización | Poblaciones pequeñas; bajo costo | `numero_lagunas`, `largo_laguna` |
| 8 | `desalinizadora` | Planta desalinizadora | Costa con agua salobre o de mar | `numero_unidades`, `largo`, `ancho` |
| 9 | `espesador` | Espesador de lodos | Reduce el volumen de lodos antes del secado | `diametro_tanque`, `altura` |
| 10 | `lecho_secado` | Lecho de secado | Secado de lodos por gravedad | `largo`, `ancho`, `numero_celdas` |
| 11 | `cloracion` | Caseta de cloración | Desinfección final, con o sin planta | `largo`, `ancho`, `altura` |

**Parámetros**

| Parámetro | Unidad | Rango típico | Obl. | Nota |
|---|---|---|---|---|
| `largo` | m | 3.00–200 | ◐ | Huella rectangular |
| `ancho` | m | 3.00–150 | ◐ | |
| `altura` | m | 2.00–12.0 | ◐ | Naves y módulos |
| `numero_unidades` | int | 1–12 | ● | Se dibujan en fila o en batería |
| `forma_unidad` | enum | rectangular, circular, cuadrada | ● | Cambia el volumen de cada unidad |
| `diametro_tanque` | m | 3.00–40.0 | ◐ | Unidades circulares |
| `numero_lagunas` | int | 1–8 | ◐ | Lagunas |
| `largo_laguna` | m | 10–200 | ◐ | |
| `ancho_laguna` | m | 10–100 | ◐ | |
| `tirante` | m | 0.50–4.00 | ○ | |
| `numero_celdas` | int | 1–12 | ◐ | Lecho de secado |
| `area` | m² | 20–50 000 | D | De la huella |
| `con_cerco` | bool | — | ○ | Perímetro de protección |

**Bloques:** B-COTAS, B-MATERIAL, B-ESTADO.

#### 3.2.10 `agua.dado_anclaje` — Dado de anclaje

`DA` · punto con sólido derivado · apoyo · 4 subtipos · partes: bloque, anclaje

Bloque de hormigón que absorbe el empuje en un cambio de dirección o en un terminal. Se dibuja como un prisma pegado a la pieza; es lo que aparece en todo plano de detalle de impulsión.

**Subtipos**

| # | Subtipo | Nombre técnico | Cuándo se usa | Parámetros distintivos |
|---|---|---|---|---|
| 1 | `codo` | Dado en codo | Cambio de dirección en impulsión | `dn_mm`, `angulo`, `largo` |
| 2 | `tee` | Dado en tee | Derivación a presión | `dn_mm`, `largo` |
| 3 | `tapon` | Dado en tapón | Extremo cerrado de un ramal a presión | `dn_mm`, `largo` |
| 4 | `reduccion` | Dado en reducción | Cambio de diámetro a presión | `dn_mm`, `dn_salida` |

**Parámetros**

| Parámetro | Unidad | Rango típico | Obl. | Nota |
|---|---|---|---|---|
| `dn_mm` | mm | 50–1 200 | ● | Tubería que ancla |
| `dn_salida` | mm | 50–1 200 | ◐ | Reducción |
| `angulo` | ° | 11.25–90 | ◐ | Codo y tee |
| `largo` | mm | 300–2 000 | ● | Dimensión en el sentido de la tubería |
| `ancho` | mm | 300–2 000 | ● | |
| `alto` | mm | 300–2 000 | ● | |
| `recubrimiento` | mm | 50–150 | ○ | |
| `tipo_hormigon` | enum | simple, armado | ○ | |

**Bloques:** B-COTAS, B-ESTADO.

#### 3.2.11 `agua.acometida` — Acometida de agua potable

`AA` · polilínea 3D · arco terminal · 4 subtipos · partes: tubo, collarin, cuerpo_valvula, conexion

Conecta la red con el predio. Es un arco corto con tres objetos en serie: corporación, medidor y llave de paso. Vive en el dominio `agua`, no en `saneamiento`.

**Subtipos**

| # | Subtipo | Nombre técnico | Cuándo se usa | Parámetros distintivos |
|---|---|---|---|---|
| 1 | `domiciliaria` | Acometida domiciliaria | Vivienda unifamiliar; DN 13–25 | `dn_mm`, `medidor`, `arqueta` |
| 2 | `comercial` | Acometida comercial | Locales y restaurantes | `dn_mm` |
| 3 | `industrial` | Acometida industrial | Caudal mayor; DN 50–100 | `dn_mm`, `dn_medidor` |
| 4 | `colectiva` | Acometida colectiva | Multifamiliar o conjunto con medidor único | `dn_mm`, `numero_usuarios` |

**Parámetros**

| Parámetro | Unidad | Rango típico | Obl. | Nota |
|---|---|---|---|---|
| `dn_mm` | mm | 13–100 | ● | |
| `material` | enum | PE, PVC, HG, cobre | ● | |
| `longitud` | m | 2–60 | D | |
| `cota_inicio` | m | — | ● | En la red |
| `cota_fin` | m | — | ● | En el predio |
| `diametro_conexion` | mm | 50–400 | ● | Empalme a la red |
| `tipo_conexion` | enum | collarín, tee, silla | ● | |
| `medidor` | ref | — | ● | Referencia a `agua.medidor` |
| `arqueta` | ref | — | ● | Referencia a `agua.arqueta` |
| `numero_usuarios` | int | 1–200 | ○ | |
| `dn_medidor` | mm | 15–100 | ◐ | Industrial |

**Bloques:** B-COTAS, B-MATERIAL, B-ESTADO.

### 3.3 Común (6 familias, 45 subtipos)

Familias que sirven a las dos redes. Se separan en `comun` para no duplicar catálogo: un codo de PVC existe igual en agua y en alcantarillado; lo que cambia es el DN, la presión y el color.

#### 3.3.1 `comun.accesorio` — Accesorio de tubería

`AX` · punto con sólido derivado · componente de arco · 11 subtipos · partes: cuerpo, campana, brida, conexion

Pieza que cambia dirección, diámetro o ramifica. El cliente lo pidió por nombre y es lo que hace que un nudo se vea como un nudo y no como tres líneas que se cruzan. **El ángulo no es subtipo: es parámetro** (D2); así once subtipos cubren más de sesenta productos comerciales.

**Subtipos**

| # | Subtipo | Nombre técnico | Cuándo se usa | Parámetros distintivos |
|---|---|---|---|---|
| 1 | `codo` | Codo | Cambio de dirección en el nudo | `angulo`, `radio_curvatura` |
| 2 | `curva` | Curva de gran radio | Cambio suave en conducciones enterradas | `angulo`, `radio_curvatura` |
| 3 | `tee` | Tee | Derivación a 90° | `dn_ramal`, `angulo_ramal` |
| 4 | `cruz` | Cruz | Cuatro ramales en un punto | `dn_ramal`, `numero_ramales` |
| 5 | `reduccion` | Reducción | Cambio de diámetro aguas abajo | `dn_salida` |
| 6 | `ampliacion` | Ampliación | Cambio de diámetro aguas arriba | `dn_salida` |
| 7 | `union` | Unión (dresser / mecánica) | Empalme desmontable entre dos tubos | `longitud_cara_a_cara` |
| 8 | `tapon` | Tapón | Cierre definitivo o provisional de un extremo | `tipo_tapon` |
| 9 | `brida` | Brida | Unión embridada suelta | `diametro_brida`, `numero_pernos` |
| 10 | `collarin` | Collarín de toma | Acometida sobre tubería en servicio | `dn_red`, `dn_derivacion` |
| 11 | `silla` | Silla de derivación | Derivación con refuerzo en tubería plástica | `dn_red`, `dn_derivacion` |

**Parámetros**

| Parámetro | Unidad | Rango típico | Obl. | Nota |
|---|---|---|---|---|
| `dn_principal` | mm | 13–1 200 | ● | |
| `dn_ramal` | mm | 13–600 | ◐ | Tee, cruz, collarín, silla |
| `dn_salida` | mm | 13–1 200 | ◐ | Reducción, ampliación |
| `angulo` | ° | 11.25 / 22.5 / 45 / 90 | ◐ | Codo y curva; el catálogo comercial usa estos cuatro |
| `angulo_ramal` | ° | 45 / 60 / 90 | ◐ | Tee y silla |
| `radio_curvatura` | mm | 50–3 000 | ○ | Codo corto, medio o largo |
| `tipo_conexion` | enum | espiga, campana, bridada, roscada, electrofusion, ranurada | ● | |
| `largo_espiga` | mm | 30–300 | ○ | |
| `longitud_cara_a_cara` | mm | 100–1 000 | ◐ | Unión y brida |
| `presion_nominal` | enum | PN6, PN10, PN16, PN25 | ◐ | Solo agua potable |
| `numero_pernos` | int | 4–24 | ◐ | Brida |
| `diametro_brida` | mm | 50–1 500 | ◐ | |
| `numero_ramales` | int | 1–4 | ◐ | Cruz |
| `dn_red` | mm | 50–600 | ◐ | Collarín, silla |
| `dn_derivacion` | mm | 13–100 | ◐ | |
| `tipo_tapon` | enum | ciego, bridado, roscado, expansible | ◐ | |

**Bloques:** B-MATERIAL, B-JUNTA, B-CONEXION, B-ESTADO.

#### 3.3.2 `comun.tapa` — Tapa, marco y rejilla

`TP` · punto con sólido derivado · componente de nodo · 7 subtipos · partes: tapa, marco, rejilla, placa

Es lo único que el ciudadano ve del alcantarillado y lo que el cliente reconocerá primero en el visor. Por eso es familia y no atributo: se selecciona, se cuenta y se exporta a DWG con su clase de carga.

**Subtipos**

| # | Subtipo | Nombre técnico | Cuándo se usa | Parámetros distintivos |
|---|---|---|---|---|
| 1 | `ciega_circular` | Tapa ciega circular | Pozos y arquetas sin ventilación | `diametro`, `clase_carga` |
| 2 | `ventilada` | Tapa ventilada | Pozos de ventilación y cámaras con gases | `diametro`, `numero_aberturas` |
| 3 | `hermetica` | Tapa hermética | Cámaras con riesgo de olores o inundación | `diametro`, `tipo_sello` |
| 4 | `cuadrada` | Tapa cuadrada | Cajas de revisión y arquetas | `lado`, `clase_carga` |
| 5 | `rectangular` | Tapa rectangular | Cámaras grandes, dos o más hojas | `lado_mayor`, `lado_menor` |
| 6 | `rejilla_barrotes` | Rejilla de barrotes | Sumideros de calzada | `largo`, `ancho`, `numero_barras` |
| 7 | `rejilla_cuadricula` | Rejilla de cuadrícula | Sumideros de patio y parqueadero | `lado`, `abertura` |

**Parámetros**

| Parámetro | Unidad | Rango típico | Obl. | Nota |
|---|---|---|---|---|
| `forma` | enum | circular, cuadrada, rectangular | ● | |
| `diametro` | mm | 400–900 | ◐ | 600 y 700 son los de catálogo (§5.3) |
| `lado` | mm | 300–900 | ◐ | |
| `lado_mayor` | mm | 400–1 500 | ◐ | |
| `lado_menor` | mm | 300–900 | ◐ | |
| `espesor` | mm | 20–100 | ● | |
| `clase_carga` | enum | A15, B125, C250, D400, E600, F900 | ● | EN 124 (§5.3) |
| `material` | enum | fundicion, hormigon, polipropileno, acero | ● | |
| `tiene_marco` | bool | — | ● | |
| `ancho_marco` | mm | 30–150 | ◐ | |
| `altura_marco` | mm | 30–200 | ◐ | Es el brocal cuando el pozo lo tiene |
| `numero_bisagras` | int | 0–2 | ○ | |
| `numero_aberturas` | int | 0–40 | ◐ | Ventilada |
| `tipo_sello` | enum | ninguna, goma, laberinto | ◐ | Hermética |
| `numero_barras` | int | 4–30 | ◐ | Rejilla |
| `abertura` | mm | 10–60 | ◐ | |
| `inscripcion` | texto | — | ○ | "ALCANTARILLADO", "AGUA POTABLE"; se dibuja como etiqueta, no como relieve |

**Bloques:** B-ESTADO.

#### 3.3.3 `comun.escalera` — Escalera y peldaños

`ES` · punto o línea · componente de nodo · 4 subtipos · partes: peldano, larguero, jaula, anclaje

Permite bajar al pozo o a la cámara. Se especifica aparte porque tiene su propio material, su separación y su norma de seguridad; y se dibuja aparte porque es lo que da escala al pozo en el visor.

**Subtipos**

| # | Subtipo | Nombre técnico | Cuándo se usa | Parámetros distintivos |
|---|---|---|---|---|
| 1 | `peldano` | Peldaño individual | Pozos de poca profundidad; peldaños embebidos | `separacion_vertical`, `ancho` |
| 2 | `gato` | Escalera de gato | Pozos profundos; dos largueros y peldaños | `longitud`, `numero_peldanos` |
| 3 | `marina` | Escalera marina | Cámaras y tanques; un larguero central | `longitud`, `ancho` |
| 4 | `con_jaula` | Escalera con jaula | Profundidad > 3 m; jaula de protección | `longitud`, `numero_aros`, `diametro_jaula` |

**Parámetros**

| Parámetro | Unidad | Rango típico | Obl. | Nota |
|---|---|---|---|---|
| `tipo` | enum | peldano, gato, marina, con_jaula | ● | |
| `ancho` | mm | 300–500 | ● | |
| `separacion_vertical` | mm | 250–350 | ● | 300 es lo normal |
| `diametro_barra` | mm | 16–30 | ● | |
| `longitud` | m | 0.60–12.0 | ◐ | |
| `numero_peldanos` | int | 1–40 | D | De la longitud y la separación |
| `diametro_jaula` | mm | 600–800 | ◐ | |
| `numero_aros` | int | 0–20 | ◐ | |
| `material` | enum | acero_galvanizado, hierro_forjado, polipropileno | ● | |

**Bloques:** B-ESTADO.

#### 3.3.4 `comun.cruce` — Cruce e interferencia

`CR` · punto o línea · apoyo · 10 subtipos · partes: camisa, apoyo, bloque, senal

Registra la interferencia y la obra especial. No cambia la topología del arco, pero sí el dibujo: una camisa, un apoyo aéreo o un dado.

**Subtipos**

| # | Subtipo | Nombre técnico | Cuándo se usa | Parámetros distintivos |
|---|---|---|---|---|
| 1 | `rio` | Cruce de río | La red pasa bajo un cauce | `longitud_cruce`, `profundidad_minima` |
| 2 | `quebrada` | Cruce de quebrada | Cauce estacional | `longitud_cruce` |
| 3 | `via` | Cruce de vía | Calzada o carretera | `longitud_cruce`, `tipo_proteccion` |
| 4 | `ferrocarril` | Cruce de ferrocarril | Vía férrea; exige encamisado | `longitud_cruce`, `diametro_camisa` |
| 5 | `otra_red` | Cruce con otra red | Agua, alcantarillado, eléctrica, telecom, gas | `separacion_vertical` |
| 6 | `electrica` | Cruce con ducto eléctrico | Riesgo de corrosión y de inducción | `separacion_vertical` |
| 7 | `telecom` | Cruce con telecomunicaciones | | `separacion_vertical` |
| 8 | `gas` | Cruce con gasoducto | Exige separación y protección | `separacion_vertical` |
| 9 | `aereo` | Cruce aéreo | Puente, pasarela o apoyo sobre pilas | `numero_apoyos`, `luz_apoyo` |
| 10 | `subfluvial` | Cruce subfluvial | Bajo el lecho de un río caudaloso | `longitud_cruce`, `profundidad_minima` |

**Parámetros**

| Parámetro | Unidad | Rango típico | Obl. | Nota |
|---|---|---|---|---|
| `longitud_cruce` | m | 2–500 | ● | |
| `angulo_cruce` | ° | 30–90 | ○ | |
| `profundidad_minima` | m | 0.60–5.00 | ● | |
| `separacion_vertical` | m | 0.10–1.00 | ◐ | Cruce con otra red |
| `tipo_proteccion` | enum | ninguna, camisa, dado, losa, enrocado | ● | |
| `diametro_camisa` | mm | 100–1 500 | ◐ | |
| `espesor_camisa` | mm | 3–20 | ○ | |
| `numero_apoyos` | int | 0–20 | ◐ | Aéreo |
| `luz_apoyo` | m | 2.00–30.0 | ◐ | |
| `tipo_apoyo` | enum | pila, mensula, colgante, flotante | ◐ | |

**Bloques:** B-COTAS, B-MATERIAL, B-ESTADO.

#### 3.3.5 `comun.cuerpo_receptor` — Cuerpo receptor

`RC` · línea o polígono · referencia externa · 7 subtipos · partes: cauce, margen, lamina

El río, estero o mar que recibe la descarga. Sin él, un emisario termina en el vacío y el visor no puede mostrar "cómo se ve en la realidad" una descarga. No tiene conectividad hidráulica: es una referencia.

**Subtipos**

| # | Subtipo | Nombre técnico | Cuándo se usa | Parámetros distintivos |
|---|---|---|---|---|
| 1 | `rio` | Río | Cuerpo receptor principal | `ancho`, `cota_agua` |
| 2 | `quebrada` | Quebrada | Cauce de invierno | `ancho`, `cota_agua` |
| 3 | `estero` | Estero | Cauce de marea en la Costa | `ancho`, `cota_agua`, `amplitud_marea` |
| 4 | `mar` | Mar | Descarga costera o subacuática | `cota_agua`, `longitud_costa` |
| 5 | `lago` | Lago o laguna | Cuerpo léntico | `ancho`, `cota_agua` |
| 6 | `canal` | Canal | Canal de riego o de drenaje | `ancho`, `cota_agua` |
| 7 | `terreno` | Terreno | Infiltración o descarga a campo abierto | `area` |

**Parámetros**

| Parámetro | Unidad | Rango típico | Obl. | Nota |
|---|---|---|---|---|
| `ancho` | m | 0.50–5 000 | ● | Ancho medio del cauce |
| `cota_agua` | m | — | ● | Nivel de referencia para la descarga |
| `cota_fondo` | m | — | ○ | |
| `direccion_flujo` | ° | 0–360 | ○ | Para dibujar la lámina |
| `longitud_tramo` | m | 10–20 000 | ○ | |
| `tipo_margen` | enum | natural, muro, escollera, playa | ○ | |
| `amplitud_marea` | m | 0–6 | ◐ | Estero y mar |
| `longitud_costa` | m | 10–20 000 | ◐ | Mar |
| `area` | m² | 10–1 000 000 | ◐ | Terreno |

**Bloques:** B-ESTADO.

#### 3.3.6 `comun.hito` — Hito, BM y punto de control

`HT` · punto · apoyo · 6 subtipos · partes: fuste, placa, senal

Sostiene el levantamiento y la construcción. No interviene en la hidráulica ni en la forma de la red, pero sin hitos el visor no tiene referencias y el replanteo no se puede leer.

**Subtipos**

| # | Subtipo | Nombre técnico | Cuándo se usa | Parámetros distintivos |
|---|---|---|---|---|
| 1 | `bm` | Benchmark | Cota de referencia del proyecto | `altura`, `lado_placa` |
| 2 | `punto_control` | Punto de control | Vértice de la poligonal | `altura` |
| 3 | `sondeo` | Punto de sondeo | Ensayo de suelo (SPT) | `profundidad` |
| 4 | `calicata` | Calicata | Inspección superficial del terreno | `largo`, `ancho`, `profundidad` |
| 5 | `aforo` | Punto de aforo | Medición de caudal en un cauce | `ancho_seccion` |
| 6 | `senalizacion` | Señalización de obra | Balizas, vallas, tapas provisionales | `altura`, `ancho` |

**Parámetros**

| Parámetro | Unidad | Rango típico | Obl. | Nota |
|---|---|---|---|---|
| `altura` | m | 0.20–2.00 | ◐ | Sobre el terreno |
| `diametro` | mm | 20–200 | ○ | Fuste |
| `lado_placa` | mm | 50–300 | ◐ | BM |
| `profundidad` | m | 0.50–30.0 | ◐ | Sondeo y calicata |
| `largo` | m | 0.50–3.00 | ◐ | Calicata |
| `ancho` | m | 0.50–2.00 | ◐ | Calicata |
| `ancho_seccion` | m | 1.00–50.0 | ◐ | Aforo |
| `tipo_placa` | enum | metalica, hormigon, pintada | ○ | |

**Bloques:** B-COTAS, B-ESTADO.

---

## 4. Contrato de parámetros

Esta sección es la que convierte 29 fichas en un modelo implementable. Si un parámetro no cumple estas reglas, no entra al catálogo.

### 4.1 Reglas de nombre

| Regla | Ejemplo correcto | Ejemplo incorrecto | Por qué |
|---|---|---|---|
| `snake_case`, sin acentos ni ñ | `diametro_interior` | `diámetroInterior` | Sobrevive a JSON, CSV, Rune y SQL |
| La unidad va en el nombre solo si es ambigua | `dn_mm`, `altura_m` | `diametro` (¿m o mm?) | Evita el error de 1 000× |
| Prefijo por familia semántica | `cota_*`, `altura_*`, `longitud_*`, `espesor_*`, `numero_*`, `tipo_*`, `diametro_*`, `ancho_*`, `largo_*` | `h1`, `d1`, `x` | Se ordenan solos en la tabla y en el inspector |
| Sin redundancia con la familia | en `agua.valvula`: `dn_mm` | `dn_valvula_mm` | El contexto ya lo da |
| El subtipo no se repite en el parámetro | `tipo_cuerpo` | `tipo_cuerpo_compuerta` | Un atributo, un nombre |
| Booleano en positivo | `con_escalera` | `sin_escalera` | Evita la doble negación |
| Un parámetro, un significado | `diametro_tapa` | `diametro` (¿pozo? ¿tapa? ¿tubo?) | Ambigüedad = dibujo equivocado |

### 4.2 Bloques reutilizables

Ocho bloques que se declaran una vez y se referencian por nombre. Si un bloque cambia, cambia en un solo sitio.

**B-COTAS** — la referencia vertical de todo objeto enterrado

| Parámetro | Unidad | Rango típico | Obl. | Nota |
|---|---|---|---|---|
| `cota_terreno` | m | — | ● | Del TIN o medida |
| `cota_rasante` | m | — | ○ | Si no hay proyecto vial, igual a terreno |
| `cota_referencia` | m | — | ● | Tapa en nodos, clave en arcos |
| `cota_fondo` | m | — | ● | Solera o fondo de zanja |
| `profundidad` | m | 0.30–15.0 | D | Terreno − fondo |

**B-MATERIAL** — cambia el color y el despiece

| Parámetro | Unidad | Rango típico | Obl. | Nota |
|---|---|---|---|---|
| `material` | enum | §4.3 | ● | |
| `tipo_hormigon` | enum | simple, armado | ○ | |
| `recubrimiento` | mm | 0–150 | ○ | |
| `acabado` | enum | liso, enlucido, visto | ○ | Solo afecta al sombreado |
| `color` | texto | — | ○ | Solo si el cliente impone color de norma |

**B-TAPA** — el remate superior

| Parámetro | Unidad | Rango típico | Obl. | Nota |
|---|---|---|---|---|
| `tipo_tapa` | enum | §4.3 | ● | |
| `diametro_tapa` | m | 0.60 / 0.70 | ● | |
| `clase_carga` | enum | A15, B125, C250, D400, E600, F900 | ● | EN 124 |
| `tiene_marco` | bool | — | ○ | |
| `altura_marco` | mm | 30–200 | ○ | |
| `ancho_marco` | mm | 30–150 | ○ | |

**B-ESCALERA** — acceso y escala visual

| Parámetro | Unidad | Rango típico | Obl. | Nota |
|---|---|---|---|---|
| `con_escalera` | bool | — | ○ | |
| `tipo_escalera` | enum | §4.3 | ○ | |
| `separacion_peldanos` | mm | 250–350 | ○ | |
| `ancho_escalera` | mm | 300–500 | ○ | |

**B-ESTADO** — lo que no cambia la forma

| Parámetro | Unidad | Rango típico | Obl. | Nota |
|---|---|---|---|---|
| `estado` | enum | §4.3 | ● | Color de estado |
| `anio` | int | 1900–2030 | ○ | |
| `origen_dato` | enum | levantamiento, proyecto, importado, estimado | ● | |
| `observaciones` | texto | — | ○ | |

**B-CONEXION** — cómo se empalma

| Parámetro | Unidad | Rango típico | Obl. | Nota |
|---|---|---|---|---|
| `dn_conexion` | mm | 13–1 500 | ● | |
| `tipo_conexion` | enum | §4.3 | ● | |
| `angulo_conexion` | ° | 0–360 | ○ | |

**B-JUNTA** — el despiece del tubo

| Parámetro | Unidad | Rango típico | Obl. | Nota |
|---|---|---|---|---|
| `tipo_junta` | enum | §4.3 | ● | |
| `longitud_tramo` | m | 3–12 | ○ | 6 m es lo normal en PVC y HG |
| `numero_juntas` | int | 0–2 000 | D | Longitud / longitud de tramo |

**B-ENTRADA** — las llegadas de un nodo

| Parámetro | Unidad | Rango típico | Obl. | Nota |
|---|---|---|---|---|
| `numero_entradas` | int | 0–6 | ● | |
| `diametro_entrada` | mm | 75–600 | ◐ | |
| `angulo_entrada` | ° | 0–360 | ○ | |

### 4.3 Enums controlados

Un enum es la única forma de que "hormigón armado" no conviva con "H.A." y "concreto reforzado" en el mismo proyecto.

| Enum | Valores admitidos |
|---|---|
| `tipo_red` | `sanitario`, `pluvial`, `combinado`, `industrial` |
| `tipo_seccion` | `circular`, `rectangular`, `ovoide`, `herradura` |
| `seccion` (canal) | `rectangular`, `trapezoidal`, `triangular`, `circular` |
| `material` | `PVC`, `PVC_O`, `PEAD`, `HG`, `HD`, `hormigon`, `hormigon_armado`, `GRC`, `acero`, `GRP`, `fibrocemento`, `mamposteria`, `fundicion`, `polipropileno`, `ladrillo` |
| `tipo_junta` | `espiga_campana`, `bridada`, `soldada`, `electrofusion`, `mecanica`, `ranurada`, `ninguna` |
| `tipo_conexion` | `espiga`, `campana`, `bridada`, `roscada`, `electrofusion`, `ranurada`, `collarín`, `tee`, `silla`, `directa` |
| `clase_presion` | `PN6`, `PN8`, `PN10`, `PN12.5`, `PN16`, `PN20`, `PN25` |
| `rigidez_anular` | `SN2`, `SN4`, `SN8` |
| `clase_carga` | `A15`, `B125`, `C250`, `D400`, `E600`, `F900` |
| `tipo_tapa` | `ciega_circular`, `ventilada`, `hermetica`, `cuadrada`, `rectangular`, `rejilla_barrotes`, `rejilla_cuadricula` |
| `tipo_escalera` | `peldano`, `gato`, `marina`, `con_jaula` |
| `tipo_cuerpo` (válvula) | `husillo_ascendente`, `husillo_no_ascendente`, `wafer`, `lug`, `doble_brida`, `globo`, `angular` |
| `tipo_accionamiento` | `manual`, `electrico`, `hidraulico`, `neumatico` |
| `tipo_caida` | `exterior`, `interior`, `escalonada` |
| `tipo_disipador` | `deflector`, `dado`, `escalonado` |
| `tipo_quiebre` | `vertedero`, `chorro`, `deflector` |
| `tipo_cresta` | `delgada`, `ancha`, `redondeada` |
| `tipo_reparto` | `vertedero`, `compuerta`, `canal` |
| `tipo_proteccion` | `ninguna`, `camisa`, `dado`, `losa`, `enrocado` |
| `tipo_apoyo` | `pila`, `mensula`, `colgante`, `flotante` |
| `tipo_cubierta` | `losa`, `cupula`, `conica`, `ninguna` |
| `forma_unidad` | `rectangular`, `circular`, `cuadrada` |
| `tipo_rejilla` | `barrotes`, `cuadricula`, `combinada` |
| `tipo_bomba` | `sumergible`, `centrifuga_horizontal`, `vertical`, `eyector` |
| `estado` | `proyectado`, `en_construccion`, `operativo`, `fuera_de_servicio`, `abandonado`, `en_rehabilitacion` |
| `origen_dato` | `levantamiento`, `proyecto`, `importado`, `estimado` |

**Regla:** un valor nuevo en un enum obliga a revisar el dibujo de esa familia. Si el valor no cambia la forma (por ejemplo `material = GRP`), basta con el color; si la cambia (por ejemplo `tipo_cubierta = cupula`), hay que declarar la parte y su primitiva en §6.

### 4.4 Parámetros derivados

Se calculan, no se editan. Si llegan del archivo de origen, se descartan y se marcan como "recalculado".

| Derivado | De qué sale | Se usa para |
|---|---|---|
| `profundidad` | cota de terreno − cota de fondo | Altura del sólido |
| `altura_cuerpo` | profundidad − altura de cono − espesor de solera | Cilindro del pozo |
| `altura_cono` | (diámetro interior − diámetro de tapa) / 2, acotada a §5.2 | Cono de reducción |
| `diametro_exterior` | diámetro interior + 2 × espesor de pared | Sombra y excavación |
| `numero_peldanos` | longitud de escalera / separación | Dibujo de peldaños |
| `numero_juntas` | longitud / longitud de tramo | Dibujo de campanas |
| `longitud` | polilínea 3D | Etiqueta y despiece |
| `area` | polígono de huella | Etiqueta de tanque y planta |
| `numero_tapas` | área de losa / área de tapa, redondeado hacia arriba | Reparto de tapas en cámaras |

**Límite:** el visor no recalcula caudales, presiones, tirantes ni volúmenes de excavación. Esos números vienen de la app de origen y se muestran como dato con su fuente.

### 4.5 Núcleo de forma por familia

El mínimo con el que ya se puede construir un sólido reconocible. Es el contrato que debe cumplir el importador: si falta un parámetro del núcleo, el objeto entra en la lista de "forma incompleta" en vez de dibujarse mal.

| Familia | Núcleo mínimo | Lo que habilita | Solo mejora |
|---|---|---|---|
| `saneamiento.pozo` | `diametro_interior`, `profundidad`, `diametro_tapa` | Cilindro + cono + tapa | cono, brocal, peldaños, banqueo |
| `saneamiento.tuberia` | `dn_mm`, `od_mm`, eje | Tubo de diámetro correcto | junta, revestimiento |
| `saneamiento.caja` | `largo`, `ancho`, `profundidad` | Caja con tapa | sifón, peldaños |
| `saneamiento.camara` | `largo`, `ancho`, `altura_util` | Volumen rectangular | tapas, vanos, deflectores |
| `saneamiento.arenero` | `largo`, `ancho`, `profundidad_total` | Caja con rejilla | rejilla, clase de carga |
| `saneamiento.trampa_grasa` | `largo`, `ancho`, `altura`, `numero_camaras` | Caja con muros interiores | deflectores |
| `saneamiento.sumidero` | `largo_util`, `ancho`, `profundidad` | Boca con rejilla | barras, arenero |
| `saneamiento.canal` | `seccion`, `base`, `altura_muro` | Prisma de sección | talud, juntas |
| `saneamiento.sifon` | `diametro`, `cota_punto_bajo`, `longitud` | Perfil en U | cámaras, purga |
| `saneamiento.descarga` | `cota_entrega`, `diametro` | Boca de entrega | vertedero, difusor, disipador |
| `saneamiento.bombeo` | `diametro` o `largo`/`ancho`, `profundidad` | Cámara con tapas | reja, caseta, bombas |
| `saneamiento.acometida` | `dn_mm`, eje | Tubo de acometida | caja, empalme |
| `agua.tuberia` | `dn_mm`, `od_mm`, eje | Tubo a presión | clase, junta, soporte |
| `agua.valvula` | `dn_mm`, `longitud_cara_a_cara`, `altura_total` | Cuerpo con bridas y vástago | volante, piloto, disco |
| `agua.llave` | `dn_mm`, `tipo_cierre` | Cuerpo con vástago | collarín, caja |
| `agua.hidrante` | `dn_mm`, `numero_bocas`, `altura_sobre_rasante` | Columna con bocas | tapas, arqueta |
| `agua.medidor` | `dn_mm`, `longitud_total` | Cuerpo con esferas | bypass, cámara |
| `agua.arqueta` | `largo`, `ancho`, `profundidad` | Caja con tapa | drenaje, tapas múltiples |
| `agua.tanque` | `diametro` o `largo`/`ancho`, `altura_util` | Volumen con cubierta | fuste, escalera, baranda |
| `agua.captacion` | `largo`/`diametro`, `altura` o `profundidad` | Caja o pozo | rejilla, presa, caseta |
| `agua.planta` | huella + `numero_unidades` | Volumetría | naves, tanques, cerco |
| `agua.dado_anclaje` | `dn_mm`, `largo`, `ancho`, `alto` | Prisma de hormigón | ángulo, refuerzo |
| `agua.acometida` | `dn_mm`, eje | Tubo de acometida | medidor, arqueta |
| `comun.accesorio` | `dn_principal`, `tipo_conexion` | Cuerpo con extremos | ángulo, radio, bridas |
| `comun.tapa` | `forma`, `clase_carga`, dimensión | Disco o placa | marco, bisagras, rejilla |
| `comun.escalera` | `tipo`, `longitud`, `separacion_vertical` | Largueros y peldaños | jaula |
| `comun.cruce` | `longitud_cruce`, `tipo_proteccion` | Camisa o apoyo | ángulo, separación |
| `comun.cuerpo_receptor` | `ancho`, `cota_agua` | Lámina de agua | márgenes, marea |
| `comun.hito` | `altura` | Poste con placa | tipo de placa |

### 4.6 Validación cruzada del catálogo

Reglas que no dependen de la norma sino de la coherencia geométrica. Se evalúan al editar y al importar; severidad `error` bloquea el dibujo, `aviso` lo permite con marca.

| Id | Regla | Severidad |
|---|---|---|
| CAT-01 | `diametro_tapa` < `diametro_interior` | error |
| CAT-02 | `altura_cono` ≤ `profundidad` − `espesor_solera` | error |
| CAT-03 | `dn_caida` ≥ `dn_salida` del pozo | aviso |
| CAT-04 | `numero_entradas` + `numero_salidas` ≥ 1 salvo pozo ciego | error |
| CAT-05 | `cota_tapa` ≥ `cota_solera` | error |
| CAT-06 | `espesor_pared` > 0 y < `diametro_interior` | error |
| CAT-07 | `longitud_cara_a_cara` presente en toda válvula ≥ DN 50 | error |
| CAT-08 | `clase_carga` de la tapa ≥ clase del entorno declarado | aviso |
| CAT-09 | `altura_util` de cámara ≥ `tirante` | error |
| CAT-10 | `numero_camaras` de trampa de grasa ≥ 1 y coincide con los muros dibujados | error |
| CAT-11 | `dn_mm` de accesorio existe en la serie §5.1 | aviso |
| CAT-12 | `tipo_conexion` compatible con el `material` (p. ej. electrofusión solo en PE) | aviso |
| CAT-13 | `angulo` de codo en {11.25, 22.5, 45, 90} | aviso |
| CAT-14 | Todo objeto tiene `cota_referencia` y `cota_fondo`, o ambas derivables del terreno | error |

---

## 5. Catálogo de dimensiones normalizadas

Tablas de referencia para que el dibujo use medidas de obra y no números inventados. **Los valores son de catálogo comercial y de práctica ecuatoriana**; el valor exacto de un proyecto lo fija el fabricante o el pliego. Cuando un parámetro no coincide con la tabla, el objeto se dibuja con su valor y se marca `fuera_de_catalogo` (aviso, no error).

### 5.1 Diámetros nominales de tubería (75–600 mm)

Serie métrica de uso corriente. El OD de PVC y PE coincide con el DN; el de hierro dúctil es mayor.

| DN (mm) | OD PVC/PE (mm) | OD HD (mm) | Espesor típico (mm) | Uso típico |
|---|---|---|---|---|
| 75 | 75 | — | 2.3–4.5 | Ramal de agua potable, acometida |
| 90 | 90 | — | 2.8–5.4 | Ramal de agua potable |
| 110 | 110 | — | 3.2–6.6 | Red secundaria AP; acometida de alcantarillado |
| 125 | 125 | — | 3.7–7.4 | Red secundaria AP |
| 160 | 160 | — | 4.0–9.5 | Red AP; ramal de alcantarillado |
| 200 | 200 | 222 | 4.9–11.9 | Red primaria AP; colector mínimo de alcantarillado |
| 250 | 250 | 274 | 6.2–14.8 | Red primaria AP; colector |
| 315 | 315 | 326 | 7.7–18.7 | Conducción AP; interceptor |
| 355 | 355 | — | 8.7–21.1 | Conducción AP; interceptor |
| 400 | 400 | 429 | 9.8–23.7 | Interceptor, emisario |
| 500 | 500 | 532 | 12.3–29.7 | Emisario |
| 600 | 600 | 635 | 14.7–35.6 | Emisario; conducción AP grande |

**Series que no son DN.** El alcantarillado plástico se especifica por **OD** y no por DN: 110, 160, 200, 250, 315, 400, 500, 630. Un "PVC 200" de desagüe tiene 200 mm exteriores y un DN equivalente cercano a 200, pero su interior es menor. Por eso el catálogo guarda `dn_mm`, `od_mm` e `id_mm` por separado (D8). El hormigón y el GRC se especifican por **diámetro interior** y clase de tubo.

### 5.2 Diámetros de pozo y cámara

| Elemento | Dimensión interior | Espesor de pared | Altura de cono | Tapa | Escalera | Uso |
|---|---|---|---|---|---|---|
| Pozo tipo A | 0.90 m | 0.15 m | 0.45 m | 0.60 m | Opcional | Profundidad ≤ 2.50 m; colectores ≤ DN 300 |
| Pozo tipo B | 1.20 m | 0.15–0.20 m | 0.60 m | 0.60 / 0.70 m | Sí | Caso general |
| Pozo tipo C | 1.50 m | 0.20 m | 0.90 m | 0.70 m | Sí | Colectores ≥ DN 400; pozos de caída |
| Pozo grande | 1.80 / 2.10 / 2.40 / 3.00 m | 0.20–0.30 m | Sin cono (losa) | Múltiples | Sí | Cámaras de inspección |
| Caja de revisión | 0.60 × 0.60 m | 0.10–0.15 m | Sin cono | 0.60 × 0.60 | No | Acometida domiciliaria |
| Caja con peldaño | 0.60 × 0.90 m | 0.15 m | Sin cono | 0.60 × 0.90 | 2–3 peldaños | Profundidad > 0.90 m |
| Caja de inspección | 0.80 × 0.80 m | 0.15 m | Sin cono | 0.80 × 0.80 | No | Acometida industrial |
| Cámara rectangular | 1.20 × 1.20 a 4.00 × 3.00 m | 0.20–0.40 m | Sin cono (losa) | 1–6 tapas | Sí | Desarenador, reunión, válvulas |

**Regla de cono:** el cono baja del diámetro interior al diámetro de tapa. Para 1.20 → 0.60 m, el desnivel típico es 0.60 m; para 1.50 → 0.70 m, 0.90 m. Si el resultado de la geometría no coincide con la tabla, manda el parámetro del objeto y se avisa (`CAT-02`).

### 5.3 Tapas, marcos y rejillas

| Elemento | Dimensión nominal | Clase de carga | Uso |
|---|---|---|---|
| Tapa circular | 600 mm | B125 / C250 | Pozos en acera y calzada |
| Tapa circular | 700 mm | C250 / D400 | Pozos grandes, calzada con tráfico pesado |
| Tapa circular | 450 / 500 mm | A15 / B125 | Arquetas de medidor, cámaras pequeñas |
| Tapa cuadrada | 600 × 600 mm | B125 / C250 | Cajas de revisión |
| Tapa cuadrada | 700 × 700 mm | C250 | Cajas y arquetas |
| Tapa rectangular | 600 × 900 mm | C250 | Cajas con peldaño |
| Tapa rectangular | 1 000 × 600 mm | D400 | Cámaras con dos hojas |
| Rejilla de calzada | 400 × 600 mm | C250 / D400 | Sumidero de boca de lobo |
| Rejilla de cuneta | 300 × 500 mm | B125 / C250 | Sumidero de cuneta |
| Rejilla de patio | 300 × 300 mm | A15 | Patios y cubiertas |

**Clases (EN 124) y su lectura práctica:** A15 = zonas peatonales y veredas; B125 = aceras, estacionamientos; C250 = calzadas con tráfico pesado; D400 = carreteras; E600 = áreas industriales y aeropuertos; F900 = aeropuertos de carga. El visor dibuja el diámetro y la clase como texto de etiqueta; la clase no cambia la forma, cambia el espesor (20–100 mm).

### 5.4 Escaleras y peldaños

| Parámetro | Valor de catálogo | Nota |
|---|---|---|
| Separación vertical entre peldaños | 0.30 m | 0.25–0.35 admitido |
| Ancho del peldaño | 0.40–0.50 m | 0.30 en pozos de 0.90 m |
| Diámetro de la barra | 20–25 mm | Acero galvanizado o polipropileno |
| Primer peldaño bajo la tapa | 0.30 m | Evita el golpe al abrir |
| Altura desde la que se exige jaula | 3.00 m | Práctica de seguridad; confirmar con el pliego (§7, R6) |
| Diámetro de la jaula | 0.70 m | Aros cada 0.60–0.90 m |

### 5.5 Válvulas: longitudes y alturas de referencia

Rangos para que la válvula encaje en el nudo y se vea proporcionada. **No son cotas de fabricación**: el valor exacto lo fija el fabricante y la serie de cara a cara.

| DN (mm) | Longitud cara a cara (mm) | Altura total (mm) | Diámetro de volante (mm) |
|---|---|---|---|
| 50 | 180–230 | 250–400 | 150–200 |
| 80 | 200–280 | 300–500 | 200–250 |
| 100 | 230–350 | 350–600 | 250–300 |
| 150 | 280–450 | 450–800 | 300–400 |
| 200 | 330–550 | 550–1 000 | 350–450 |
| 300 | 400–700 | 700–1 300 | 400–600 |
| 400 | 500–850 | 900–1 600 | 500–700 |
| 500 | 600–1 000 | 1 000–1 900 | 600–800 |
| 600 | 700–1 200 | 1 200–2 200 | 700–900 |

**Mariposa:** la longitud es la mitad que la de compuerta para el mismo DN (cuerpo wafer o lug). **Retención:** longitud cercana a la de compuerta, altura menor. **Aire y purga:** cuerpo pequeño (1/4 a 1/2 del DN de la tubería) y altura dominada por la ventosa.

### 5.6 Accesorios: ángulos, radios y longitudes

| Parámetro | Valores de catálogo | Nota |
|---|---|---|
| Ángulo de codo | 11.25°, 22.5°, 45°, 90° | Son los cuatro ángulos comerciales |
| Radio de codo | Corto ≈ 1.0 D; medio ≈ 1.5 D; largo ≈ 2.0 D | D = diámetro exterior |
| Longitud de tee | 1.5–2.0 D por cada ramal | |
| Longitud de reducción | 1.0–2.0 D | |
| Ángulo de tee o silla | 45°, 60°, 90° | 45° es la "yee" |
| Longitud de unión dresser | 100–300 mm | Según DN |
| Brida: número de pernos | 4 (≤ DN 80), 8 (DN 100–200), 12 (DN 250–400), 16 (DN 500–600) | Referencia visual |

### 5.7 Arquetas, cajas y cámaras pequeñas

| Elemento | Planta | Profundidad típica | Tapa |
|---|---|---|---|
| Arqueta de medidor | 0.60 × 0.40 m | 0.40–0.90 m | 0.45 / 0.60 m |
| Arqueta de válvula | 0.80 × 0.80 m | 0.80–1.60 m | 0.60 × 0.60 m |
| Arqueta de hidrante | 0.80 × 0.80 m | 0.80–1.50 m | 0.60 × 0.60 m |
| Arqueta de purga | 0.60 × 0.60 m | 0.60–1.50 m | 0.60 × 0.60 m |
| Caja de revisión | 0.60 × 0.60 m | 0.40–1.20 m | 0.60 × 0.60 m |
| Caja con peldaño | 0.60 × 0.90 m | 0.90–2.00 m | 0.60 × 0.90 m |
| Registro prefabricado | Ø 0.40 / 0.60 m | 0.40–1.00 m | Ø 0.40 / 0.60 m |
| Trampa de grasa pequeña | 0.60 × 0.40 m | 0.50–0.80 m | 0.60 × 0.40 m |

### 5.8 Hidrantes y medidores

| Elemento | Dimensión | Nota |
|---|---|---|
| Hidrante de columna | DN 65/80/100; altura 0.70–1.00 m sobre rasante | 2 bocas de 65 + 1 de 100 |
| Hidrante de arqueta | DN 65/80; arqueta 0.80 × 0.80 m | Tapa a ras del piso |
| Boca de riego | DN 50/65; altura 0.30–0.60 m | |
| Medidor domiciliario DN 15 | L 110–165 mm | Rosca 1/2" |
| Medidor domiciliario DN 20 | L 190 mm | Rosca 3/4" |
| Medidor domiciliario DN 25 | L 260 mm | Rosca 1" |
| Macromedidor DN 50–300 | L 200–500 mm según tipo | Woltman, electromagnético |
| Medidor electromagnético DN 400–600 | L 500–900 mm | Con bridas |

### 5.9 Canales y secciones no circulares

| Sección | Rango de dimensiones | Nota |
|---|---|---|
| Rectangular | b 0.30–3.00 m; h 0.30–2.50 m | |
| Trapezoidal | b 0.50–5.00 m; talud H:V 0.5–2.0 | |
| Triangular (cuneta V) | corona 0.30–1.20 m; profundidad 0.10–0.50 m | |
| Cuneta U | ancho 0.30–0.80 m; altura 0.20–0.60 m | |
| Box culvert | 1–3 celdas de 1.00 × 1.00 a 4.00 × 3.00 m | |
| Ovoide (alcantarillado) | altura ≈ 1.5 × ancho; 0.60 × 0.90 a 1.20 × 1.80 m | Para caudales variables |
| Herradura | 1.00 × 1.20 a 3.00 × 3.00 m | Colectores grandes |

### 5.10 Materiales y su aspecto

El cliente pidió que "se vean como en la realidad". Sin texturas, la realidad se comunica con el color y el brillo. Esta tabla es la traducción a la capa de estilo.

| Material | Color base | Brillo | Se usa en |
|---|---|---|---|
| PVC (agua) | Gris claro azulado | Medio | AP |
| PVC (desagüe) | Gris medio o naranja | Bajo | AL |
| PEAD | Negro mate | Bajo | AP y AL |
| Hierro dúctil | Negro con óxido | Medio | Impulsión, AP |
| Hierro galvanizado | Gris metálico | Alto | Acometidas |
| Hormigón armado | Gris claro | Bajo | Pozos, cámaras, tubería |
| Hormigón simple | Gris medio | Bajo | Cajas, dados |
| Acero | Gris oscuro | Alto | Impulsión, camisas |
| GRP / PRFV | Verde oliva o beige | Bajo | AL, cámaras |
| Fibrocemento | Gris azulado | Bajo | AP antigua |
| Mampostería | Ladrillo | Bajo | Pozos antiguos, canales |
| Fundición (tapas) | Negro grafito | Medio | Tapas y rejillas |

### 5.11 Umbral de representación

| Tamaño aparente en pantalla | LOD | Qué se dibuja | Coste |
|---|---|---|---|
| Menos de 3 px | LOD0 | Símbolo de tamaño fijo (rombo, círculo, cuadrado) | 1 primitiva |
| 3–40 px | LOD1 | Sólido de 8–12 caras, sin partes finas | ≤ 40 triángulos |
| Más de 40 px | LOD2 | Sólido + partes del oficio (cono, marco, tapa, peldaños, bridas, volante) | ≤ 400 triángulos |

El umbral se evalúa **por objeto y por fotograma**, con histéresis para que no parpadee al girar la cámara: se sube de LOD a 45 px y se baja a 35 px. La histéresis es un parámetro de la capa de estilo, no del catálogo.

---

## 6. Qué se dibuja y qué no

### 6.1 La regla

Todo objeto se dibuja con **partes** (§6.2) y cada parte tiene una **primitiva**. El render no conoce familias: recibe una lista de partes con su primitiva, su transformación y su color. Eso es lo que permite añadir una familia sin tocar el dibujo (D3, D4).

Tres modos de vista, un solo modelo:

| Modo | Qué muestra | Cuándo | Coste |
|---|---|---|---|
| Sólido | Solo lo que está sobre el terreno: tapas, marcos, hidrantes, casetas, tanques, canales | Vista de calle, presentación al cliente | Bajo: se descartan las partes enterradas |
| Rayos X | La red completa a través del terreno, con transparencia | Trabajo diario, revisión | Medio: es el modo actual del visor |
| Corte | Un plano de sección que muestra el interior: pozo hueco, tubo, niveles de agua | Explicar una caída, un sifón, un tanque | Alto: exige recortar las partes contra el plano |

El catálogo declara las partes **una vez**; cada modo decide cuáles pinta. Un pozo en modo sólido es un círculo con su marco; en rayos X es un cilindro; en corte es un hueco con solera, cono y banqueo.

### 6.2 Vocabulario de partes

| Parte | Definición | Primitiva | Familias que la usan |
|---|---|---|---|
| `solera` | Fondo interior del pozo o cámara | Losa | pozo, caja, camara, arenero, trampa_grasa, bombeo |
| `cuerpo` | Volumen principal vertical | Cilindro o prisma | pozo, caja, camara, tanque, arqueta, trampa_grasa |
| `cono` | Transición del cuerpo al brocal | Cono truncado | pozo |
| `brocal` | Anillo de hormigón bajo el marco | Anillo | pozo, arqueta |
| `marco` | Aro metálico que recibe la tapa | Anillo | pozo, caja, arqueta, sumidero |
| `tapa` | Placa que cierra | Disco o placa | pozo, caja, arqueta, sumidero |
| `peldano` | Barra de ascenso | Barra | pozo, camara, arqueta |
| `banqueo` | Media caña de la solera | Prisma curvo | pozo |
| `entrada` / `salida` | Tubo que acomete | Tubo corto | pozo, caja, camara |
| `conexion` | Tubo de empalme al colector | Tubo corto | sumidero, caja, arqueta, arenero |
| `caida` | Tubo vertical de caída | Tubo | pozo |
| `pantalla` | Placa que rompe el chorro | Placa | pozo |
| `tubo` | Cuerpo de la tubería | Tubo | tuberia, sifon, descarga, acometida, cruce |
| `revestimiento` | Capa interior o exterior del tubo | Tubo (casco) | tuberia, sifon |
| `losa` | Tapa estructural de cámara | Losa | camara, tanque, planta |
| `muro` | Pared de cámara o canal | Prisma | camara, canal, descarga, trampa_grasa |
| `fuste` | Cuerpo de la torre de un tanque | Cilindro | tanque, hito |
| `cubierta` | Techo del tanque | Losa, cúpula o cono | tanque |
| `brida` | Disco de unión embridada | Anillo | valvula, accesorio, medidor, hidrante |
| `cuerpo_valvula` | Cuerpo del dispositivo | Prisma o cilindro | valvula, llave, medidor |
| `bonete` | Tapa superior de la válvula | Cilindro | valvula |
| `vastago` | Husillo de accionamiento | Barra | valvula, llave |
| `volante` | Rueda de maniobra | Aro con radios | valvula, llave |
| `disco` | Obturador de mariposa | Disco | valvula |
| `piston` | Obturador de compuerta | Placa | valvula |
| `flotador` | Boya de control de nivel | Esfera | valvula |
| `piloto` | Circuito de control de una PRV | Tubo fino | valvula |
| `rejilla` | Placa con barras | Placa con huecos | sumidero, arenero, tapa, descarga |
| `deflector` | Placa que cambia el flujo | Placa | trampa_grasa, pozo |
| `canaleta` | Conducto de entrega | Prisma curvo | descarga |
| `cresta` | Umbral vertiente | Prisma | descarga |
| `compuerta` | Obturador de alivio o cámara | Placa | descarga, camara |
| `difusor` | Tubo con orificios de descarga | Tubo | descarga |
| `enrocado` | Escollera de protección | Prisma | descarga |
| `apoyo` | Pila o ménsula de un cruce aéreo | Prisma | cruce |
| `camisa` | Tubo de protección de un cruce | Tubo | cruce |
| `anclaje` | Fijación de escalera, camisa o apoyo | Prisma | escalera, cruce, dado_anclaje |
| `bloque` | Dado de anclaje | Prisma | dado_anclaje |
| `escalera` | Conjunto de peldaños con largueros | Barra + prisma | pozo, camara, tanque |
| `larguero` | Larguero de escalera | Barra | escalera |
| `jaula` | Aros de protección de escalera | Aro | escalera |
| `baranda` | Baranda perimetral | Barra | tanque, camara, bombeo |
| `respiradero` | Tubo de venteo de tanque | Tubo | tanque |
| `placa` | Placa de identificación | Placa | hito, medidor |
| `senal` | Señalización de obra | Placa | hito, cruce |
| `caseta` | Volumen de protección o control | Prisma | captacion, bombeo, planta |
| `bomba` | Cuerpo de bomba | Cilindro | bombeo |
| `nave` | Edificio de proceso | Prisma | planta |
| `tanque` | Unidad de proceso | Cilindro o prisma | planta, tanque |
| `cerco` | Perímetro de protección | Prisma delgado | planta, captacion |
| `cauce` | Lámina de agua del receptor | Placa | cuerpo_receptor |
| `lamina` | Superficie de agua | Placa | cuerpo_receptor, tanque |
| `margen` | Borde del cauce | Prisma | cuerpo_receptor |
| `junta` | Junta entre dos tubos | Anillo | tuberia, canal |
| `campana` | Extremo acampanado del tubo | Anillo | tuberia, accesorio |
| `codo` | Curva del trazado | Prisma curvo | sifon |
| `camara` | Volumen de cámara de entrada o salida | Prisma | sifon, bombeo |
| `arenero` | Receso de sedimentación bajo la solera | Prisma hueco | pozo, sumidero |
| `vano` | Abertura en un muro o losa | Prisma hueco | camara |
| `talud` | Plano inclinado del canal | Prisma | canal |
| `borde` | Labio de borde libre | Prisma | canal |
| `collarin` | Abrazadera de toma | Anillo | llave, hidrante, acometida |
| `boca` | Salida de hidrante | Cilindro | hidrante |
| `esfera` | Carátula del medidor | Disco | medidor |
| `purga` | Boquilla de vaciado | Tubo corto | sifon |

En las fichas de §3 las partes se escriben en plural y en lenguaje natural ("peldaños", "bridas", "caída"); los nombres canónicos, los que usa el código, son los de esta tabla en singular. Son la misma lista: la ficha resume y la tabla contrata.

Un render que no sepa dibujar una parte debe fallar en la prueba, no en la pantalla: si una familia declara `cono` y no hay primitiva de cono, el catálogo no compila. Eso es lo que evita que una familia nueva aparezca "sin dibujo".

### 6.3 Nivel de detalle mínimo aceptable por familia

La columna **mínimo** es el criterio de aceptación: si el objeto se ve más grande que 40 px y le falta una de esas partes, el dibujo no se acepta como "se ve como en la realidad".

| Familia | Mínimo (LOD2) | Añade en LOD2 | Nunca |
|---|---|---|---|
| `saneamiento.pozo` | Cilindro + cono + marco y tapa | Escalera, banqueo, entradas, caída | Rosca de la tapa, bisagras, relieve de letras |
| `saneamiento.tuberia` | Tubo de OD correcto + eje | Campanas cada 6 m, junta | Rosca, refuerzo interno, textura |
| `saneamiento.caja` | Caja + tapa + entrada y salida | Peldaños, sifón | Bisagras, sello |
| `saneamiento.camara` | Volumen + losa + tapas | Vanos, deflector, compuerta, escalera | Armadura, juntas de construcción |
| `saneamiento.arenero` | Caja + rejilla + salida | Rejilla detallada | Barro, arena acumulada |
| `saneamiento.trampa_grasa` | Caja + muros interiores + tapa | Deflectores, coalescedor | Grasa acumulada, olor |
| `saneamiento.sumidero` | Boca + rejilla + marco | Barras, arenero, muro banqueta | Hojas y basura |
| `saneamiento.canal` | Prisma de sección + solera | Talud, juntas, borde | Fisuras, vegetación |
| `saneamiento.sifon` | Perfil en U + dos cámaras | Purga, codos | Aire, vacío |
| `saneamiento.descarga` | Boca de entrega + muro o talud | Vertedero, difusor, disipador, enrocado | Espuma, salpicadura |
| `saneamiento.bombeo` | Cámara + tapas + impulsión | Reja, bombas, caseta, baranda | Motor, cableado, tablero detallado |
| `saneamiento.acometida` | Tubo + caja | Empalme, pendiente | Zanja |
| `agua.tuberia` | Tubo de OD y PN correctos | Bridas, junta, dado de anclaje | Rosca, revestimiento interno |
| `agua.valvula` | Cuerpo + 2 bridas + vástago | Volante, bonete, piloto, disco | Tuercas, placa de identificación, rosca |
| `agua.llave` | Cuerpo + vástago | Volante, collarín | Roscas |
| `agua.hidrante` | Columna + bocas + tapas | Arqueta, válvula auxiliar, collarín | Cadenas, pintura gastada |
| `agua.medidor` | Cuerpo + esferas + conexiones | Bypass, tapa | Reloj interno, cristal |
| `agua.arqueta` | Caja + tapa | Drenaje, tapas múltiples, peldaños | Bisagras |
| `agua.tanque` | Volumen + cubierta | Fuste, escalera, baranda, respiraderos | Impermeabilización, fisuras |
| `agua.captacion` | Caja o pozo + rejilla | Presa, caseta, sello sanitario, brocal | Bomba sumergible detallada |
| `agua.planta` | Huella + volúmenes de unidades | Naves, tanques, cerco, caseta | Tubería de proceso, equipos |
| `agua.dado_anclaje` | Prisma de hormigón | Refuerzo, recubrimiento | Armadura |
| `agua.acometida` | Tubo + medidor + arqueta | Collarín, llave | Zanja |
| `comun.accesorio` | Cuerpo + extremos | Bridas, ramal, radio | Rosca, sello |
| `comun.tapa` | Placa + marco + clase | Bisagras, aberturas, barrotes | Relieve de inscripción, óxido |
| `comun.escalera` | Largueros + peldaños | Jaula, anclajes | Soldadura |
| `comun.cruce` | Camisa o apoyo | Ángulo, protección | Permiso, señalización |
| `comun.cuerpo_receptor` | Lámina de agua + márgenes | Marea, dirección de flujo | Oleaje, vegetación |
| `comun.hito` | Poste + placa | Tipo de placa | Pintura |

### 6.4 Lo que nunca se dibuja

| Elemento | Por qué no | Qué se hace en su lugar |
|---|---|---|
| Roscas y tuercas | Miles de triángulos para algo invisible a 1 m | Cilindro liso; si hay que documentarlo, va al despiece DWG |
| Letras en relieve de la tapa | Es textura, no forma | Etiqueta de texto en LOD2 o atributo `inscripcion` |
| Armadura del hormigón | No se ve, no cambia la silueta | Atributo `tipo_hormigon` |
| Juntas de construcción y fisuras | Es estado, no forma | Atributo `estado` y color |
| Óxido, manchas, desgaste | Es material envejecido | Color de material (§5.10) |
| Agua en movimiento, salpicaduras | Es animación, no catálogo | Plano de nivel en modo corte |
| Arena, grasa, lodos acumulados | Es operación | Atributo de mantenimiento si el cliente lo pide |
| Césped, árboles, vehículos | Es contexto, no red | Capa de entorno urbano, ya existente |
| Sombra arrojada | El render actual no la tiene | Sombreado por cara (§6.7) |
| Transparencia del terreno | Rompe el orden del pintor | Modo rayos X por capas, ya implementado |

### 6.5 Etiquetas y simbología

| Elemento | LOD0 | LOD1 | LOD2 |
|---|---|---|---|
| Código del objeto (`PZ-014`) | No | Solo si está seleccionado | Sí, si la capa de etiquetas está activa |
| Cota de tapa / solera | No | No | Sí, en el inspector |
| Diámetro | No | No | Sí, junto al objeto |
| Símbolo | Rombo (AL) o círculo (AP), tamaño fijo | — | — |
| Color de estado | Sí (relleno del símbolo) | Sí | Sí, en el marco |

**Regla de legibilidad:** en una vista general no se dibujan más de 40 etiquetas a la vez. La capa de etiquetas prioriza por selección, luego por estado distinto de `operativo`, luego por cercanía a la cámara.

### 6.6 Presupuesto y rendimiento

| Escenario | Objetos | Triángulos LOD1 | Triángulos LOD2 | Fotogramas objetivo |
|---|---|---|---|---|
| Barrio | 200 | 8 000 | 80 000 | 60 fps |
| Ciudad media | 5 000 | 200 000 | 2 000 000 | 45–60 fps |
| Ciudad grande | 50 000 | 2 000 000 | — | 30 fps; LOD2 solo en el 1 % visible |

La regla que hace esto posible: **el LOD2 es la excepción, no la norma**. Se aplica al objeto seleccionado, al que está a menos de 30 m de la cámara y a los que el cliente marque en una lista de "objetos de presentación". Todo lo demás vive en LOD1.

---

## 7. Riesgos y ambigüedades

Ocho puntos que el cliente debe resolver. Los cuatro primeros bloquean trabajo; los cuatro últimos se pueden decidir en paralelo.

| # | Ambigüedad | Por qué importa | Opciones | Recomendación | Bloquea |
|---|---|---|---|---|---|
| R1 | **"Vertedero"** designa cuatro cosas: entrega final al cauce, alivio de excedentes, aliviadero de cresta y relleno sanitario | Si el cliente quería el relleno sanitario, es otra familia y otro catálogo (no hidráulico) | (a) las tres hidráulicas; (b) las tres + relleno sanitario; (c) solo entrega final | (a). El relleno sanitario no pertenece a una red de agua | Sí: §3.1.10 y §3.3.5 |
| R2 | **"Llave" vs "válvula" vs "hidrante"**: el registro actual mete el hidrante dentro de `agua.llave` | Si el hidrante es subtipo, no se puede seleccionar, contar ni dibujar como objeto propio | (a) familia propia (propuesta); (b) subtipo de llave; (c) subtipo de válvula | (a). Geometría, norma y mantenimiento son distintos | Sí: §3.2.3 y §3.2.4 |
| R3 | **"Pozo"**: el cliente puede estar pensando en pozos de agua (profundos) o en pozos de inspección de alcantarillado | Son familias distintas, con forma y parámetros distintos; confundirlas produce un modelo inservible | (a) `agua.captacion:pozo_profundo` para agua y `saneamiento.pozo` para alcantarillado (propuesta); (b) una sola familia con atributo de red | (a), y que la UI nunca muestre "Pozo" a secas: siempre "Pozo de inspección" o "Pozo profundo" | Sí: §3.1.1 y §3.2.8 |
| R4 | **"Como en la realidad"** no tiene criterio de aceptación | Sin criterio, el trabajo no termina nunca: siempre falta un tornillo | (a) silueta + partes del oficio (D9); (b) fotorrealismo con texturas; (c) malla detallada de fabricante | (a). Es lo único compatible con 5 000 objetos a 60 fps y con el render actual (polígonos sombreados, sin texturas) | Sí: §6.3 |
| R5 | **Componentes**: ¿tapa, escalera y accesorio son objetos o dibujo? | Si son objetos, hay que modelarlos, contarlos y exportarlos; si son dibujo, se pierden al exportar a DWG | (a) familias con `padre_id` (propuesta); (b) atributos del padre; (c) partes generadas por el render | (a) para tapa y accesorio (se especifican y se compran); (c) para escalera (nunca se especifica suelta) | No, pero cambia el modelo |
| R6 | **De dónde salen las dimensiones normalizadas**: municipio, prestador o fabricante | La tabla de §5 es de práctica; un municipio puede exigir 1.00 m de pozo o tapa de 600 en calzada | (a) tabla de este documento como default y code pack por municipio; (b) catálogo del prestador; (c) catálogo comercial de un fabricante | (a). El valor del proyecto siempre gana; la tabla solo propone | No |
| R7 | **OD vs DN**: el alcantarillado plástico se especifica por diámetro exterior y el agua por DN; el archivo de origen suele traer solo "200" | Un mismo número puede significar dos tubos distintos; el 3D mentiría en el diámetro | (a) guardar `dn_mm` + `od_mm` + `id_mm` (D8); (b) un solo campo y una tabla de conversión por material | (a), y que la ingesta exija el material para interpretar el número | Sí: §4.4 y §5.1 |
| R8 | **Vista por defecto**: el cliente quiere ver la red "como en la realidad", pero la red está enterrada | En modo sólido no se ve nada; en rayos X se ve todo pero no es realista | (a) rayos X por defecto (como hoy) y sólido a demanda; (b) sólido por defecto; (c) las dos mitades en pantalla | (a). El modo sólido se reserva para la presentación y para el recorrido de calle | No |

**Riesgo residual aceptado.** El catálogo no cubre equipamiento electromecánico (bombas, tableros, sopladores, dosificadores) ni obra civil de plantas más allá de la volumetría. Si el cliente lo pide, es una extensión de catálogo, no un cambio de arquitectura.

---

## 8. Impacto en el registro Rust

### 8.1 Lo que ya sirve y no se toca

| Pieza existente | Estado | Comentario |
|---|---|---|
| `FamilyDef { key, code_prefix, label, category, topo, geom, caps, attrs, types }` | Sirve | Es exactamente el contenedor que este catálogo necesita |
| `TypeDef { key, label, required, presets }` | Sirve | Los `required` son los parámetros obligatorios de §3 |
| `AttrDef { key, label, unit, ty, default, required, constraints, group, access }` | Sirve | `constraints` cubre los rangos; `access = Derived` cubre §4.4 |
| `caps::{HYDRAULIC_NODE, HYDRAULIC_EDGE, EXCAVATABLE, STRUCTURAL, ATTACHABLE, REPORTABLE, CONSUMES_FLOW, SOURCE}` | Sirve | Las familias nuevas usan `ATTACHABLE` y `STRUCTURAL` |
| Tests `claves_y_prefijos_unicos`, `todo_subtipo_declara_atributos_existentes`, `los_defaults_respetan_su_tipo`, `hay_familias_para_las_dos_redes` | Sirven | El catálogo crece sin romperlos |

### 8.2 Cambios de esquema propuestos

Cuatro cambios pequeños que evitan meter el dibujo en el `match` de la vista:

| Cambio | Forma propuesta | Por qué |
|---|---|---|
| Grupo de atributos de forma | Añadir `Group::Forma` a `Group` (hoy: `General`, `Geometrico`, `Hidraulico`, `Estructural`, `Material`, `Operacion`) | El inspector debe separar "cómo se ve" de "qué resiste" |
| Partes dibujables | `ViewDef { lod_max: u8, partes: &'static [ParteDef] }` y `ParteDef { parte: Parte, primitiva: Primitiva, desde: u8 }` en `FamilyDef` | El render itera partes (D4); una familia sin partes no compila |
| Capacidad de componente | Bits `caps::COMPONENT` y `caps::ATTACHED` | Tapa, escalera y accesorio no son nodos hidráulicos ni estructurales |
| Enlace al padre | `FamilyDef.parent_family: Option<&'static str>` | Declara que `comun.tapa` se cuelga de `saneamiento.pozo`, `saneamiento.caja` y `agua.arqueta` |

**Lo que no se cambia:** `Unit` ya tiene `M`, `Mm`, `M2`, `M3`, `Deg`, `Percent`, `Mperm`; `AttrType` ya tiene `Num`, `Int`, `Bool`, `Text`, `Enum`. Las referencias (`caja`, `medidor`, `cuerpo_receptor`) se guardan como `Text` con validación en el validador, no como un tipo nuevo en v1.

### 8.3 Las 11 familias nuevas en forma de registro

| Clave | Prefijo | `topo` | `geom` | `caps` | `parent_family` |
|---|---|---|---|---|---|
| `comun.accesorio` | AX | `Device` | `Point` | `ATTACHABLE`, `COMPONENT`, `REPORTABLE` | arco (`Link`) |
| `comun.tapa` | TP | `Support` | `Point` | `COMPONENT`, `REPORTABLE` | pozo, caja, arqueta, sumidero |
| `comun.escalera` | ES | `Support` | `Point` | `COMPONENT` | pozo, camara, arqueta |
| `comun.cuerpo_receptor` | RC | `Support` | `Polyline` | `REPORTABLE` | — |
| `saneamiento.caja` | CJ | `Node` | `Point` | `HYDRAULIC_NODE`, `EXCAVATABLE`, `STRUCTURAL`, `REPORTABLE` | — |
| `saneamiento.arenero` | AR | `Node` | `Point` | `HYDRAULIC_NODE`, `EXCAVATABLE`, `STRUCTURAL` | — |
| `saneamiento.trampa_grasa` | TG | `Node` | `Point` | `HYDRAULIC_NODE`, `EXCAVATABLE`, `STRUCTURAL` | — |
| `agua.arqueta` | AQ | `Node` | `Point` | `HYDRAULIC_NODE`, `EXCAVATABLE`, `STRUCTURAL` | — |
| `agua.hidrante` | HD | `Device` | `Point` | `ATTACHABLE`, `CONSUMES_FLOW`, `REPORTABLE` | — |
| `agua.dado_anclaje` | DA | `Support` | `Point` | `STRUCTURAL` | arco (`Link`) |
| `agua.acometida` | AA | `Link` | `Polyline` | `HYDRAULIC_EDGE`, `EXCAVATABLE`, `CONSUMES_FLOW` | — |

### 8.4 Pruebas que hay que añadir

| Prueba | Qué verifica | Coste |
|---|---|---|
| `toda_familia_declara_partes` | Ninguna familia queda sin `ViewDef` | Trivial |
| `toda_parte_tiene_primitiva` | No hay partes huérfanas ni primitivas inventadas | Trivial |
| `todo_subtipo_declara_un_obligatorio` | Un subtipo sin parámetros obligatorios es un subtipo mal definido | Trivial |
| `rangos_coherentes` | `min < default < max` en todo atributo numérico | Trivial |
| `presupuesto_de_triangulos` | LOD1 ≤ 40 y LOD2 ≤ 400 por familia | Bajo |
| `enums_sin_sinonimos` | No conviven "H.A." y "hormigon_armado" | Bajo |
| `alias_de_migracion_resuelven` | Cada alias de §2.4 apunta a una clave existente | Bajo |

### 8.5 Lo que queda fuera de v1

| Elemento | Por qué se difiere |
|---|---|
| Bombas, tableros y equipos electromecánicos | El cliente no los pidió; se dibujan como volumen dentro de la estación |
| Tren de proceso de PTAP/PTAR | Requiere catálogo de equipos y no cambia la red |
| Señalización, cerco, caseta detallada | Contexto, no red |
| Mobiliario urbano, árboles, vehículos | Capa de entorno |
| Texturas y materiales PBR | Fuera del alcance del render actual |

---

## 9. Anexos

### A. Normas citadas (solo identificador)

No se cita ninguna cláusula. El identificador sirve para que el lector sepa de qué norma habla el catálogo; los valores concretos viven en el *code pack* versionado (decisión D5 de `01-domain.md`).

| Identificador | Materia | Dónde se usa |
|---|---|---|
| EN 124 | Tapas y rejillas de calzada; clases A15–F900 | §5.3 |
| EN 558 | Longitud cara a cara de válvulas | §5.5 |
| EN 1074 | Válvulas para abastecimiento de agua | §3.2.2 |
| EN 1092 | Bridas | §3.3.1 |
| EN 1917 | Pozos y cámaras prefabricadas de hormigón | §3.1.1 |
| ISO 2531 | Tubos de fundición dúctil | §5.1 |
| ISO 1452 | PVC-U a presión | §5.1 |
| ISO 4427 | Polietileno a presión | §5.1 |
| ISO 4064 | Medidores de agua | §3.2.5 |
| ISO 7005 | Bridas metálicas | §3.3.1 |
| ASTM C478 | Pozos prefabricados de hormigón | §3.1.1 |
| ASTM C443 | Juntas de goma para pozos | §3.1.1 |
| ASTM D3034 | Tubo de PVC para alcantarillado | §5.1 |
| AWWA C500 | Válvulas de compuerta | §3.2.2 |
| AWWA C502 | Hidrantes de barril seco | §3.2.4 |
| AWWA C503 | Hidrantes de barril húmedo | §3.2.4 |
| AWWA C504 | Válvulas de mariposa | §3.2.2 |
| AWWA C508 | Válvulas de retención | §3.2.2 |
| AWWA C509 | Válvulas de compuerta de asiento elástico | §3.2.2 |
| AWWA C512 | Válvulas de aire | §3.2.2 |
| ACI 350 | Estructuras de contención de líquidos | §3.2.7 |

**Normativa ecuatoriana.** Las ordenanzas municipales y los pliegos del prestador fijan diámetros mínimos, distancias entre pozos, clases de tapa y detalles tipo. No se les asigna identificador aquí porque varían por cantón; entran al *code pack* como `pliego_municipal`. Las NTE INEN aplicables se incorporarán con su identificador cuando el code pack se cierre.

### B. Glosario y sinónimos

| Término | Sinónimos | Qué es en este catálogo |
|---|---|---|
| Pozo de inspección | Boca de visita, pozo de visita, buzón, registro | `saneamiento.pozo` |
| Caja de revisión | Registro domiciliario, caja de registro | `saneamiento.caja` |
| Cámara | Cámara de inspección, cámara de reunión | `saneamiento.camara` |
| Arqueta | Caja de medidor, caja de válvula | `agua.arqueta` |
| Sumidero | Boca de lobo, boca de tormenta, rejilla de calzada | `saneamiento.sumidero` |
| Arenero | Trampa de arena, desarenador pequeño | `saneamiento.arenero` |
| Trampa de grasa | Separador de grasas, trampa de aceites | `saneamiento.trampa_grasa` |
| Vertedero | Emisario final, alivio, aliviadero, rebose | `saneamiento.descarga` |
| Llave | Llave de paso, llave de corporación, llave de banqueta | `agua.llave` |
| Hidrante | Boca de incendio, grifo contra incendio | `agua.hidrante` |
| Boca de riego | Grifo de riego | `agua.hidrante:boca_riego` |
| Ventosa | Válvula de aire, purgador de aire | `agua.valvula:aire` |
| Purga | Válvula de desagüe, purgador | `agua.valvula:purga` |
| Macromedidor | Medidor de caudal, contador | `agua.medidor` |
| Brocal | Anillo de brocal | Parte del pozo, no familia |
| Marco y tapa | Tapa de registro | `comun.tapa` |
| Media caña | Banqueo, canaleta de solera | Parte del pozo |
| Dado de anclaje | Bloque de anclaje, dado de empuje | `agua.dado_anclaje` |
| Sifón invertido | Sifón, cruce deprimido | `saneamiento.sifon` |
| Fuste | Cuerpo de torre | Parte del tanque |
| Cámara húmeda | Pozo de bombeo | `saneamiento.bombeo:camara_humeda` |
| Cámara seca | Sala de máquinas | `saneamiento.camara:bombeo_seco` |

### C. Checklist de aceptación para la demostración

Diez comprobaciones que el cliente puede hacer en la demo sin conocer el código:

1. Un pozo de 1.20 m visto de cerca muestra cilindro, cono, marco, tapa y peldaños.
2. El mismo pozo visto de lejos se convierte en un rombo del color de su estado.
3. Un pozo de caída interior muestra el tubo de caída y la pantalla.
4. Una válvula de compuerta muestra cuerpo, dos bridas, bonete, vástago y volante.
5. Una válvula de mariposa se distingue de la de compuerta por el cuerpo y el disco.
6. Un hidrante tiene columna, dos bocas laterales, una frontal y tapas.
7. Un sumidero de calzada muestra marco, rejilla y la boca contra el bordillo.
8. Un tramo de tubería cambia de diámetro visible al pasar de DN 110 a DN 315.
9. Un tanque elevado se ve con fuste, cubierta y escalera.
10. Al seleccionar cualquier objeto, el inspector muestra sus parámetros con unidad y su estado.

### D. Cambios respecto del catálogo anterior

| Antes | Ahora | Motivo |
|---|---|---|
| 18 claves, 60 subtipos | 29 familias, 216 subtipos | Cobertura del oficio |
| `hidrante` dentro de `agua.llave` | `agua.hidrante` (4 subtipos) | Geometría y especificación propias |
| `agua_potable` dentro de `saneamiento.acometida` | `agua.acometida` (4 subtipos) | Dominio equivocado |
| `limpieza` dentro de `saneamiento.pozo` | `saneamiento.caja` (8 subtipos) | No es un pozo: no es visitable ni tiene cono |
| `bombeo` dentro de `saneamiento.pozo` | `saneamiento.bombeo` (5 subtipos) | La estación no es un pozo |
| Tipos de agua potable en `saneamiento.tuberia` | Solo en `agua.tuberia` | Un arco, una red, un dominio |
| Sin accesorios, tapas, escaleras, arquetas | 4 familias nuevas | Lo que el cliente pidió ver |
| Sin dimensiones de referencia | §5 con 11 tablas | Para que el dibujo use medidas de obra |

---

*Fin del documento. Las decisiones abiertas de §7 se cierran con el cliente; ninguna bloquea la implementación de §3, §4 y §5, que ya están en forma de registro.*
