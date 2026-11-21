# 02 — Arquitectura de sistemas

**Documento:** registro de decisiones (ADR) de arquitectura de software
**Autor:** Arquitecto de sistemas (consejo de arquitectura)
**Estado:** Propuesta para revisión del cliente — versión 1.0
**Fecha:** 2026-11-18
**Producto:** Rune Redes — app de escritorio (Rust + Rune UI) y CLI `redes`
**Alcance:** los 10 puntos del mandato del consejo. El *qué* y el *para quién* viven en `05-product.md`; este documento decide el *cómo*.
**Depende de:** `05-product.md` (personas, backlog, releases, criterios de aceptación). Cada decisión de aquí cita los IDs de backlog que desbloquea.
**No cubre:** el detalle del dominio hidráulico-sanitario (lo fija el ingeniero civil en `01-domain.md`); aquí solo se decide cómo se representa y se ejecuta.

---

## 0. Resumen ejecutivo

| # | Decisión | En una línea |
|---|---|---|
| 1 | **Workspace de 18 crates en 5 capas** | Cinco crates puros (`units`, `geom`, `crs`, `terrain`, `hydro`, `earth`, `core`, `topo`, `calc`) que la CLI y la UI comparten literalmente; la UI es un consumidor, no el centro. |
| 2 | **Modelo de objetos: arena tipada + columnas por esquema + IDs generacionales** | Ni OOP con `dyn`, ni ECS, ni enum gigante: un *arena* con IDs estables de 8 bytes, tablas columnares por familia descritas por esquema, y snapshots `Arc<Doc>` publicados. |
| 3 | **`FamilyDef` / `ElementType` / `Instance`, familias declaradas en manifiesto** | Una familia nueva = un manifiesto JSON (o un `const` en Rust, que produce el mismo JSON). Cero `match` nuevos en el núcleo. |
| 4 | **Capa de topología explícita: nodos = puntos de conexión, aristas = elementos lineales** | Los objetos **no se referencian entre sí**; publican puertos y la topología deriva nodos, aristas y cachés. La proximidad es un candidato; la conexión es un dato. |
| 5 | **f64 en CRS proyectado (UTM 17S/18S) + origen local para el render f32** | El documento guarda coordenadas absolutas en metros; la cámara resta el origen en f64 y recién entonces convierte a `f32`. Unidades explícitas por newtype (mm para diámetros, m para longitudes). |
| 6 | **Todo cambio es un `Command` con inverso; UI y CLI ejecutan el mismo camino** | `State` (efímero) ≠ `Document` (verdad). Eventos tipados que producen *efectos*, jamás mutaciones: eso rompe los ciclos. |
| 7 | **SQLite embebido como contenedor transaccional, no como modelo relacional** | Un `.redes` = un SQLite con WAL; el modelo son blobs binarios canónicos por familia en *chunks* de 1024 filas. Migraciones por `schema_version`. |
| 8 | **Registro en tiempo de ejecución + manifiestos de familia; sin scripting en v1** | `Plugin` con registro explícito (una lista, no un dios-`match`), contrato de cálculo publicado y suite de conformidad. WASM queda como puerta diseñada. |
| 9 | **Un solo hilo de UI; trabajadores por trabajos gruesos sobre snapshots inmutables** | La UI nunca toca disco ni calcula; los workers reciben `Arc<Doc>` y devuelven resultados + `Live<f32>` de progreso. |
| 10 | **Pruebas por fórmula, propiedad, doradas de archivo y snapshots headless de Rune** | Más dos pruebas de arquitectura que hacen cumplir las reglas de este documento automáticamente. |

**Los tres riesgos mayores** (§12): (R1) Rune es pre-release y su aptitud como lienzo 2D no está verificada; (R2) las familias declaradas en runtime pueden degradar la seguridad de tipos y el aislamiento; (R3) la topología y la geometría pueden producir números *silenciosamente* equivocados, que es exactamente la razón de compra del producto.

---

## 1. Workspace, crates y dirección de dependencias

### 1.1 Decisión

Workspace Cargo único, edición 2024, `resolver = "3"`, versión de Rust fijada en `rust-toolchain.toml`. Dieciocho crates en cinco capas. La regla es una sola: **las dependencias solo apuntan hacia abajo, y las capas 0–4 no saben que existe una UI, un archivo o un hilo.**

| # | Crate | Responsabilidad (una frase) | Capa | Puro | Depende de |
|---|---|---|---|---|---|
| 1 | `redes-units` | Newtypes de magnitud (`Meters`, `Millimeters`, `Slope`, `Elevation`, `FlowM3s`, `CubicMeters`, `Radians`) y sus conversiones; única puerta de unidades. | 0 | ✅ | — |
| 2 | `redes-geom` | Primitivas 2D/3D, tolerancias, intersecciones, estacionado sobre polilínea, áreas, índices espaciales (`rstar`). | 0 | ✅ | `units` |
| 3 | `redes-crs` | UTM ↔ geográficas (Transversa de Mercator, serie de Krüger), Helmert de 7 parámetros WGS84 ↔ PSAD56, registro EPSG, residuo del transformado. | 0 | ✅ | `units`, `geom` |
| 4 | `redes-terrain` | TIN con breaklines (CDT), muestreo de cota, curvas de nivel, pendientes, secciones del terreno, diferencia de superficies. | 1 | ✅ | `units`, `geom`, `crs` |
| 5 | `redes-hydro` | Manning con sección parcialmente llena, Hazen-Williams, Darcy-Weisbach (Colebrook), solver de red a presión (gradiente global), calado normal. | 1 | ✅ | `units`, `geom` |
| 6 | `redes-earth` | Sección tipo de zanja, volumen por áreas medias, volumen de pozo, balance de tierras, abundamiento, acarreo. | 1 | ✅ | `units`, `geom`, `terrain` |
| 7 | `redes-core` | **El corazón:** IDs, familias, esquemas de atributos, elementos, `Document`, codec canónico, validación estructural, rasgos-puerto (`AuditSink`, `Clock`). | 2 | ✅ | `units`, `geom`, `crs` |
| 8 | `redes-topo` | Grafo de topología: puertos, nodos, aristas, segmentos por accesorio, reconstrucción incremental, validación topológica. | 3 | ✅ | `core`, `geom` |
| 9 | `redes-calc` | Contratos de cálculo por familia, registro de cálculos, code packs, resultados con procedencia y `numeric_hash`, motor de validación normativa. | 4 | ✅ | `core`, `topo`, `hydro`, `earth`, `terrain` |
| 10 | `redes-cmd` | `Command`/`Txn`/`History`, deshacer-rehacer, propagación de eventos y efectos, claves naturales e idempotencia. | 4 | ✅ | `core`, `topo`, `calc` |
| 11 | `redes-store` | Contenedor `.redes` (SQLite), migraciones, autoguardado, bloqueo, integridad, respaldos, code packs en disco, bitácora. | 5 | ❌ IO | `core` |
| 12 | `redes-io` | Registro de formatos + rasgos `Importer`/`Exporter`; sub-crates por formato: `redes-io-dxf`, `-geojson`, `-csv`, `-landxml`, `-xlsx`, `-pdf`. | 6 | ❌ IO | `core`, `geom`, `terrain` |
| 13 | `redes-report` | Composición de documentos: memoria de cálculo, listados, perfil longitudinal, cajetín, lotes de planos. | 6 | ❌ IO | `calc`, `io`, `terrain` |
| 14 | `redes-api` | Superficie para agentes: manifiesto de capacidades autodescriptivo, sobre JSON versionado, códigos de error, servidor MCP. | 7 | ❌ | `cmd`, `calc`, `io`, `report` |
| 15 | `redes-cli` | Binario `redes`: parseo de argumentos, códigos de salida, modo lote. Solo presentación de `redes-api`. | 8 | ❌ | `api` |
| 16 | `redes-ui` | Vistas Rune (planta, perfil, inspector, tablas), cámara, picking, temas, i18n. **Único crate que conoce Rune.** | 8 | ❌ UI | `core`, `cmd`, `calc`, `store`, `io`, `report` |
| 17 | `redes-app` | Binario de escritorio: arranque, ventana, inyección de plugins, autoguardado, manejo de errores. | 9 | ❌ | `ui`, `store`, `plugins` |
| 18 | `redes-plugins` | **Raíz de composición:** la única lista donde se registran familias, cálculos, formatos e informes. Nadie depende de ella. | 9 | ❌ | todas las de datos |

**Dirección de dependencias (lista de aristas, sin ciclos):**

```
units ← geom ← crs ← terrain
units ← geom ← hydro
units ← geom ← terrain ← earth
       └──────── core ──┬── topo ──┐
                        │          ├── calc ──┐
                        │          │          ├── cmd ──┬── api ── cli
                        │          │          │         ├── ui
                        │          │          │         └── store
                        └──────────┴──────────┴── io ───┴── report
                                                        plugins → (todas)
```

Verificación en CI (§10.6): una prueba lee `cargo metadata`, construye el grafo y falla si aparece un ciclo o una arista prohibida (`core` → `store`, `calc` → `ui`, etc.). La regla no queda escrita en un documento: queda compilada.

### 1.2 Por qué esta forma y no otra

- **La CLI no es un cliente de la UI.** Comparten `core`+`cmd`+`calc`; el criterio de salida de v0.1 exige *diferencia 0* entre el volumen de la UI y el de la CLI para los mismos 5 tramos. Eso solo se garantiza si el número se calcula **una vez, en un solo crate** (`redes-calc`), y los dos frentes solo lo serializan.
- **`redes-topo` es un crate, no un módulo de `core`.** Es donde vivirán los errores más caros (ALC-07) y donde se necesita un ciclo de prueba propio (reconstrucción incremental vs. completa, §10.4). Aislarlo permite además calcular topología sin cargar el motor de cálculo.
- **`redes-hydro` y `redes-earth` no conocen el documento.** Reciben estructuras planas (`PipeInput`, `TrenchSection`) y devuelven números. Beneficio directo: se pueden validar contra EPANET (HID-01) y contra planillas (HID-02) en pruebas sin construir un proyecto.
- **`redes-crs` propio en vez de PROJ.** PROJ es una dependencia C de ~10 MB, con estado global y sin determinismo bit a bit garantizado entre versiones. Solo necesitamos Transversa de Mercator y Helmert, que son matemática cerrada y verificable contra los puntos de prueba publicados por EPSG. TOP-03 exige registrar el parámetro usado y el residuo: eso se hace mejor con código propio y auditable. En CI se compara contra `cs2cs` como prueba dorada, sin que PROJ sea dependencia de producción.
- **`redes-api` separado de `redes-cli`.** CLI-03 pide que el manifiesto se genere del registro en runtime, y CLI-06 pide un servidor MCP con el mismo manifiesto. Si el manifiesto vive en `redes-api`, la prueba de contrato "manifiesto MCP == `redes esquema`" es trivial y no puede divergir.

### 1.3 Alternativas consideradas

| Alternativa | Por qué se descarta |
|---|---|
| Un solo crate `redes` con módulos | El tiempo de compilación se dispara con 100k objetos y un motor de UI encima; y nada impide que un módulo de dominio importe `std::fs`, que es justo lo que hay que prohibir. |
| Dos crates (`redes-core`, `redes-ui`) | Obliga a que la CLI dependa del crate que contiene Rune, o a duplicar el dominio. Es el error clásico que hace que la CLI se vuelva un ciudadano de segunda. |
| `redes-hydro` dentro de `redes-calc` | Impide validar el solver contra EPANET sin construir un `Document`; y mezcla matemática pura con política de procedencia. |
| Un crate por formato desde el día 1 (`dxf`, `ifc`, `landxml`, …) | Sí, pero como **sub-crates de `redes-io`** y detrás de features: el binario de escritorio no debe arrastrar LandXML si nadie lo usa. |

### 1.4 Consecuencias

- **A favor:** la UI puede reescribirse entera sin tocar un cálculo; la CLI funciona aunque Rune no compile en la plataforma; cada fórmula tiene una suite de pruebas sin E/S.
- **En contra:** 18 `Cargo.toml` que mantener y *re-exports* que cuidar. Se mitiga con un `redes-core::prelude` y con la regla de que ningún crate exporta más de lo que necesita.
- **Regla de admisión:** un crate nuevo solo se justifica si (a) tiene una suite de pruebas propia, o (b) impone una prohibición de dependencia. `redes-report` cumple (a); `redes-plugins` cumple (b).

---

## 2. El modelo de objetos — la decisión central

### 2.1 Decisión

**Arena tipada con IDs generacionales + tablas columnares por familia descritas por esquema + snapshots `Arc<Doc>` con copia al escribir por familia.**

Concretamente:

1. **Un solo espacio de IDs** para todo elemento, de 8 bytes, generacional: `ElementId { slot: u32, gen: u32 }`. Es la identidad que usan el undo, el archivo, la CLI y los agentes. Nunca se reutiliza un `slot` con la misma `gen`.
2. **Un `slot` no guarda el elemento**: guarda `family`, `type`, `GeomId`, `flags` y el índice de fila dentro de la tabla de su familia. El elemento *es* su fila.
3. **Los atributos viven en tablas columnares por familia**, una columna por atributo del esquema, tipada (`Vec<f64>`, `Vec<u32>`, bitset, `Vec<Arc<str>>`…). No hay `HashMap<String, Value>` por objeto.
4. **El documento se publica como `Arc<Doc>`**; las tablas de cada familia son `Arc` internamente, de modo que publicar un snapshot es clonar punteros y editar una familia clona solo esa familia.
5. **Nada de `Rc`/`RefCell` entre objetos.** Las referencias son `ElementId` y se validan.

```rust
// redes-core/src/id.rs
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[derive(serde::Serialize, serde::Deserialize)]
pub struct ElementId { pub slot: u32, pub gen: u32 }

impl ElementId {
    pub const fn new(slot: u32, gen: u32) -> Self { Self { slot, gen } }
    /// Texto estable para la CLI y para los agentes: "0004:0001".
    pub fn short(self) -> String { format!("{:04x}:{:04x}", self.slot, self.gen) }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct FamilyId(u16);      // handle de runtime; la identidad persistida es la clave textual
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct GeomId(u32);
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct RowIdx(u32);        // índice denso dentro de la tabla de una familia
```

```rust
// redes-core/src/doc.rs  (forma; los detalles de esquema van en §3)
pub struct Doc {
    pub meta: ProjectMeta,           // CRS, origen local, datum, unidades, tolerancias
    pub families: FamilyRegistry,    // FamilyId ↔ clave textual, esquemas
    pub slots: SlotArena,            // Vec<Slot> + free-list + generaciones
    pub tables: FamilyTables,        // FamilyId → Arc<FamilyTable>
    pub geoms: GeomStore,            // GeomId → Arc<Geom>
    pub topo: Topology,              // derivado, con epoch (§4)
    pub revision: Revision,          // u64, sube en cada transacción confirmada
}

impl Doc {
    pub fn get(&self, id: ElementId) -> Option<ElementRef<'_>>;
    pub fn iter_family(&self, f: FamilyId) -> impl Iterator<Item = (ElementId, ElementRef<'_>)>;
    pub fn count(&self, f: FamilyId) -> usize;
}
```

### 2.2 Comparación con las cuatro alternativas del mandato

| Criterio | (a) OOP + `dyn Trait` por familia | (b) ECS (`bevy_ecs`/`hecs`) | (c) **Arena tipada + esquema + generacionales** | (d) `enum` + structs |
|---|---|---|---|---|
| Extensión por terceros **en runtime** | ❌ exige código compilado y enlazado; el `Box<dyn>` no aporta nada que un registro de datos no dé | ❌ un componente es un tipo Rust: no existe "componente declarado en JSON" | ✅ una familia es **datos** (`FamilyDef`); el comportamiento se compone de banderas y del contrato de cálculo | ❌ cada variante nueva toca el núcleo |
| IDs estables para undo/persistencia/IA | ⚠️ depende de punteros o de un mapa aparte | ⚠️ `Entity` es generacional y estable, pero **el orden no lo es** | ✅ 8 bytes, generacional, serializable, ordenable; el mismo ID en UI, archivo y CLI | ✅ si se añade una arena (y entonces ya es (c)) |
| Consulta sobre 100k objetos | ⚠️ salto por puntero, sin localidad | ✅ el mejor del cuadro para *filtros por componente* | ✅ un recorrido de columna es memoria contigua; los filtros de dominio son por familia + rango de IDs | ⚠️ enum de tamaño variable; recorrer mezcla tamaños |
| Ergonomía con UI retenida que re-ejecuta vistas | ❌ `Rc<RefCell<..>>` → pánico en runtime en mitad del frame | ⚠️ `World` en el `State` y queries en cada vista: presta el mundo entero | ✅ la vista recibe `&State` con un `Arc<Doc>` inmutable: cero préstamos mutables en el frame | ✅ barato de leer, pero inmutable solo si se copia |
| Serialización | ❌ `typetag`/`erased-serde`, un hook por tipo | ❌ requiere reflexión sobre componentes | ✅ el esquema **es** el códec: mismas columnas, mismo JSON/binario, misma validación | ✅ `serde` derivado, pero el sobre crece con cada variante |
| Añadir familia sin tocar el núcleo | ⚠️ nuevo `impl` + entrada de registro (compilado) | ❌ | ✅ manifiesto + registro; la UI, el IO y el validador lo leen del esquema | ❌ `match` exhaustivo en N sitios |

**Veredicto:** (c). El ECS es un motor de almacenamiento y consulta excelente, pero no es un modelo de documento: no da orden canónico, ni esquema serializable, ni familias declarativas, y arrastra un planificador que una app de UI no necesita. El OOP con `dyn` reproduce el problema que este producto quiere evitar (objetos que se referencian y se rompen) y no habilita nada en runtime. El `enum` es correcto para conjuntos *cerrados* — y por eso se usa, pero solo donde el conjunto es genuinamente cerrado: **la geometría**.

> **Regla de estilo del consejo:** cerrado donde el dominio es cerrado (geometría: `Point | Polyline | Polygon | TinFace | Mesh`); abierto donde el dominio es abierto (familias de elementos).

### 2.3 Invariantes que impone

| # | Invariante | Cómo se hace cumplir |
|---|---|---|
| I1 | Ningún puntero ni dirección es identidad. `ElementId` es la única identidad, y `(slot, gen)` no se reutiliza. | `SlotArena` incrementa `gen` al liberar; prueba de reutilización. |
| I2 | Todo el estado del proyecto vive en el `Doc`. Selección, índices, cachés y topología son **derivados y desechables**. | Revisión: nada fuera de `Doc` se serializa. |
| I3 | Los atributos se escriben solo por `AttrStore::set`, que valida contra el esquema. | Campos privados + `pub(crate)`; la prueba de arquitectura prohíbe `pub` en las columnas. |
| I4 | El esquema de una familia es inmutable mientras el documento vive. Evolucionar = subir `version` y migrar. | `FamilyDef` en `Arc`; `version` en el manifiesto. |
| I5 | Toda mutación pasa por `Doc::apply(&dyn Command)`. No hay setters públicos. | `&mut Doc` solo se obtiene dentro de `Command::apply`. |
| I6 | El orden de iteración es determinista: por `ElementId` o por orden de inserción explícito, **nunca** por `HashMap`. | `BTreeMap`/`IndexMap` + lint; prohibido `HashMap` en rutas que producen salida. |
| I7 | Ninguna referencia entre elementos; solo `ElementId` con validación de existencia. | `doc.validate()` (MOD-07) reporta referencias rotas. |
| I8 | La geometría se guarda una vez, en coordenadas de mundo, en `GeomStore`; los elementos la referencian por `GeomId`. | `GeomStore` con `Arc<Geom>`, compartible al copiar. |
| I9 | Un snapshot publicado (`Arc<Doc>`) es inmutable para sus lectores y consistente respecto de un `revision`. | Copia al escribir por familia; nunca `&mut` a través del `Arc`. |
| I10 | La identidad persistida de una familia es su **clave textual**, no su `FamilyId` numérico (que depende del orden de registro). | El manifiesto guarda `family_key ↔ id` y se remapea al abrir. |

### 2.4 Consecuencias

- **A favor:** la vista de Rune recibe `&State`, y `State` contiene `doc: Arc<Doc>` — el compilador garantiza que el frame no muta nada. Los 100k objetos no necesitan `unsafe`, ni `RefCell`, ni `RwLock`.
- **A favor:** undo, persistencia y CLI comparten los mismos IDs y el mismo orden; el `numeric_hash` de INF-01 es reproducible.
- **En contra:** escribir "un atributo de un objeto" es más verboso (`doc.attr_mut(id, DIAMETRO)?`) que `pozo.diametro = 200.0`. Se compensa con *builders* de comando (`CreatePozo::new(..).diametro_mm(200)`) y con un inspector genérico.
- **En contra:** las columnas son heterogéneas, así que el código genérico trabaja sobre `Value` en la frontera (inspector, JSON). Esa frontera está **confinada a dos módulos** y es la única con despacho por tipo.

---

## 3. El concepto de *familia*

### 3.1 Decisión: tres niveles `Family` / `ElementType` / `Instance`

| Nivel | Qué es | Dónde vive | Identidad | Ejemplo |
|---|---|---|---|---|
| **Familia** (`FamilyDef`) | El contrato: geometría, puertos, atributos, capacidades, icono, i18n, versión. | Manifiesto (JSON) o `const` en Rust que produce el mismo JSON. | clave textual `dominio.familia` | `saneamiento.pozo` |
| **Tipo de elemento** (`ElementType`) | Un subtipo de catálogo con valores por defecto, restricciones propias y símbolo. Es **dato**, no código. | Dentro del manifiesto de la familia, o en un code pack. | clave `familia.tipo` | `saneamiento.pozo.caida` |
| **Instancia** (`Instance`) | Un objeto colocado: ID + tipo + geometría + fila de atributos. | `Doc` | `ElementId` | el pozo `0004:0001` de la calle X |

Esta separación es la que pide MOD-02 (pozos de revisión / caída / caja / sumidero con campos obligatorios propios) y MOD-04 (válvula / llave / hidrante / medidor) sin duplicar esquemas: **una familia, N tipos**.

### 3.2 El descriptor de esquema

```rust
// redes-core/src/family.rs
pub struct FamilyDef {
    pub key: FamilyKey,            // "saneamiento.tuberia" — identidad persistida
    pub version: u16,              // sube cuando cambia el esquema; dispara migración
    pub label: i18n::Key,          // "family.saneamiento.tuberia"
    pub category: Category,        // Agua | Saneamiento | Topografia | Comun
    pub geom: GeomKind,            // Point | Polyline | Polygon
    pub ports: PortTemplate,       // Ends | Fixed(&'static [PortDecl]) | EdgeWithAttachments
    pub caps: Caps,                // banderas de capacidad (bitflags)
    pub attrs: Vec<AttrDef>,       // orden estable = orden del archivo
    pub types: Vec<ElementTypeDef>,// subtipos de catálogo
    pub icon: IconKey,
    pub symbol: SymbolKey,         // simbología VIS-01
    pub calc: Option<CalcContract>,// EXT-02: qué cálculo declara y con qué parámetros
}

bitflags::bitflags! {
    #[derive(Clone, Copy, PartialEq, Eq)]
    pub struct Caps: u32 {
        const HYDRAULIC_EDGE = 1 << 0;   // participa como arista en la red hidráulica
        const HYDRAULIC_NODE = 1 << 1;   // participa como nodo (aporta cota/carga)
        const EXCAVATABLE    = 1 << 2;   // genera volumen de excavación
        const SURFACE        = 1 << 3;   // es una superficie (TIN)
        const STRUCTURAL     = 1 << 4;   // ocupa volumen de obra civil (pozo, tanque)
        const ATTACHABLE     = 1 << 5;   // se inserta sobre una arista (válvula, acometida)
        const REPORTABLE     = 1 << 6;   // aparece en listados y memoria
        const CONSUMES_FLOW  = 1 << 7;   // demanda (acometida, hidrante)
    }
}

pub struct AttrDef {
    pub key: AttrKeyName,          // "diametro_interior" — sin unidad en la clave interna
    pub label: i18n::Key,          // "attr.diametro_interior"
    pub unit: Unit,                // Mm | M | Slope | M3s | M3 | Mca | None
    pub ty: AttrType,              // F64 | I32 | Bool | Text | Enum(&'static [EnumOpt]) | Ref(RefFilter) | Point3
    pub default: Value,
    pub required: bool,
    pub constraints: Vec<Constraint>, // Min | Max | Step | OneOf | Monotonic | Cross(CrossRule) | Regex
    pub group: GroupKey,           // pestaña del inspector
    pub access: Access,            // Editable | ReadOnly | Derived
    pub norm_ref: Option<ClauseRef>, // "CPE INEN 5:2019 §6.3.2"
    pub doc: i18n::Key,            // ayuda emergente
}

pub struct ElementTypeDef {
    pub key: TypeKey,              // "caida"
    pub label: i18n::Key,
    pub symbol: SymbolKey,
    pub required: Vec<AttrKeyName>,        // campos obligatorios propios (MOD-02)
    pub presets: Vec<(AttrKeyName, Value)>,// valores por defecto del subtipo
    pub extra_constraints: Vec<(AttrKeyName, Constraint)>,
}
```

### 3.3 Seguridad de tipos sin `match` gigante

El acceso a atributos es genérico sobre un marcador de tipo. La clave **solo se puede obtener del registro**, y obtenerla verifica el tipo una vez:

```rust
// redes-core/src/attr.rs
pub trait AttrKind: 'static {
    const TAG: TypeTag;
    type Repr: Copy + PartialEq + serde::Serialize + serde::de::DeserializeOwned;
}

pub struct AttrKey<K: AttrKind> { id: AttrId, name: &'static str, _k: PhantomData<fn() -> K> }

impl FamilyRegistry {
    /// Falla si el atributo no existe o si su tipo no es K. Es la única forma de obtener una clave.
    pub fn key<K: AttrKind>(&self, fam: FamilyKey, name: &str) -> Result<AttrKey<K>, SchemaError>;
}

// El documento expone acceso tipado:
impl Doc {
    pub fn get<K: AttrKind>(&self, id: ElementId, k: AttrKey<K>) -> Option<K::Repr>;
    pub fn set<K: AttrKind>(&mut self, id: ElementId, k: AttrKey<K>, v: K::Repr) -> Result<(), Reject>;
}

// En el núcleo, claves constantes con tipo conocido en compilación:
pub mod saneamiento {
    use super::*;
    pub const DIAMETRO_INTERIOR: AttrKey<attr::Mm> = AttrKey::const_new(AttrId(3), "diametro_interior");
    pub const PENDIENTE:        AttrKey<attr::Slope> = AttrKey::const_new(AttrId(5), "pendiente");
}
```

Dos niveles de garantía, deliberados:

- **Compilación** para las familias del producto: `saneamiento::DIAMETRO_INTERIOR` es una constante tipada; `doc.get(id, DIAMETRO_INTERIOR)` devuelve `Millimeters`.
- **Runtime verificado** para familias de terceros: `registry.key::<Mm>(fam, "diametro_interior")` se resuelve al cargar el manifiesto y falla con un mensaje claro si el manifiesto miente. A partir de ahí el acceso sigue siendo tipado.

### 3.4 Agregar una familia en UN solo lugar

```rust
// redes-families/src/saneamiento/pozo.rs
pub const FAMILY: &str = include_str!("../../manifests/saneamiento.pozo.json");

// El mismo archivo, en JSON, es lo que carga un tercero en runtime (EXT-01):
// {
//   "key": "saneamiento.pozo", "version": 2, "label": "family.saneamiento.pozo",
//   "geom": "point", "ports": { "template": "fixed", "decls": [
//       { "name": "salida", "kind": "flow" } ] },
//   "caps": ["hydraulic_node", "excavatable", "structural", "reportable"],
//   "attrs": [
//     { "key": "cota_tapa", "unit": "m", "ty": "f64", "required": true,
//       "constraints": [ { "min": 0.0 } ], "norm_ref": "CPE INEN 5:2019 §6.2" },
//     { "key": "profundidad", "unit": "m", "ty": "f64", "access": "derived" }
//   ],
//   "types": [
//     { "key": "revision", "label": "type.pozo.revision", "symbol": "pozo_rev" },
//     { "key": "caida", "label": "type.pozo.caida", "symbol": "pozo_caida",
//       "required": ["altura_caida"], "presets": [["altura_caida", 0.6]] }
//   ]
// }
```

Lo que **no** hay que tocar al añadir una familia:

| Consumidor | Cómo se entera | Por qué no hay `match` |
|---|---|---|
| Inspector de la UI | itera `def.attrs` y dibuja el widget según `AttrDef.ty` | el widget depende del **tipo de atributo**, no de la familia |
| Serialización | itera las columnas del esquema | el códec es el esquema |
| Validador | itera `constraints` | las restricciones son datos |
| Importadores/exportadores | despachan por `GeomKind` + `Caps` + claves de atributo | el formato se decide por forma, no por identidad |
| Motor de cálculo | `CalcContract` declarado en el manifiesto | el cálculo se elige por contrato, no por familia |
| Simbología y planos | `symbol` / `icon` del tipo | VIS-01 es data-driven |
| CLI y manifiesto para agentes | itera el registro | CLI-03 se cumple por construcción |

**La única excepción admisible** es una familia que necesita comportamiento genuinamente nuevo (p. ej. una válvula que modela una ley de cierre): se registra un `FamilyBehavior` opcional junto al manifiesto. El núcleo no cambia; solo se añade un `impl`. Ese es el "una familia = un manifiesto + opcionalmente un `impl`", y no un punto de extensión por cada consumidor.

### 3.5 Por qué el manifiesto es la fuente, también para las familias propias

Los tres manifiestos propios (pozo, tubería, vertedero, válvula, llave, tanque, acometida) se escriben **en el mismo formato** que usaría un tercero y se cargan por el mismo camino. Consecuencia: EXT-01 se prueba todos los días desde la semana 1 en vez de descubrirse roto en la semana 20, y la suite de conformidad de EXT-03 corre contra familias propias.

### 3.6 Consecuencias

- **A favor:** MOD-01, MOD-02, MOD-04, EXT-01, EXT-02 y CLI-03 se resuelven con el mismo mecanismo.
- **En contra:** los manifiestos son más verbosos que código Rust y no tienen ayuda del compilador. Mitigación: el manifiesto se **valida contra un JSON Schema propio** al cargar (con mensajes por campo) y las familias propias tienen una prueba que compara el manifiesto contra su `const` tipado, de modo que un error de tipeo en un atributo no llega a producción.
- **En contra:** renombrar un atributo rompe proyectos. Mitigación: tabla de alias por versión de familia (`aliases: {"diametro": "diametro_interior"}`) aplicada en la migración.
- **Riesgo R2** (ver §12): un manifiesto malicioso o inválido. Se rechaza la familia completa, se informa, y el resto del proyecto sigue vivo (EXT-01).

---

## 4. Comunicación entre objetos: la capa de topología

### 4.1 Decisión

**Los objetos no se hablan. Publican puertos y leen la topología.**

Tres estructuras, y ninguna referencia entre elementos:

| Estructura | Qué representa | Identidad | Persistida |
|---|---|---|---|
| **Puerto** (`Port`) | Un punto de conexión *de un elemento*: posición, cota de invert, diámetro, tipo. | `(ElementId, PortId)` | sí, dentro del elemento |
| **Nodo** (`TopoNode`) | Una **clase de coincidencia**: el conjunto de puertos que están unidos físicamente. | `NodeId` | sí (la unión es dato, no deducción) |
| **Arista** (`TopoEdge`) | Trayecto de flujo entre dos nodos, recortado de un elemento lineal entre dos puntos de attachment. | `EdgeId` | derivada, con epoch |

```rust
// redes-core/src/element.rs
pub struct PortId(u16);

pub struct Port {
    pub id: PortId,
    pub name: PortName,        // "inicio" | "fin" | "entrada" | "salida" | "alivio" | "rebose"
    pub at: Point2,            // mundo, metros
    pub invert: Elevation,     // cota de solera del puerto, m
    pub diameter: Option<Millimeters>,
    pub kind: PortKind,        // Flow | Structural | Free
}

pub enum PortTemplate {
    /// Tuberías y colectores: exactamente dos puertos, en los extremos de la polilínea.
    Ends,
    /// Pozos, tanques, válvulas: puertos declarados con nombre y desplazamiento local.
    Fixed(&'static [PortDecl]),
    /// Elementos lineales que admiten inserción de accesorios (válvulas, acometidas).
    EdgeWithAttachments,
}
```

```rust
// redes-topo/src/lib.rs
pub struct Topology {
    pub epoch: Epoch,                  // sube cuando cambia algo estructural
    nodes: Vec<TopoNode>,
    edges: Vec<TopoEdge>,
    node_of_port: BTreeMap<(ElementId, PortId), NodeId>,
    incident: BTreeMap<NodeId, SmallVec<[EdgeId; 6]>>,
    attachments: BTreeMap<ElementId, SmallVec<[Attachment; 4]>>,  // accesorios sobre un elemento
}

pub struct TopoNode {
    pub id: NodeId,
    pub at: Point2,
    pub cover: Option<Elevation>,      // cota de tapa, si hay estructura
    pub members: SmallVec<[PortRef; 6]>, // (ElementId, PortId)
    pub min_invert: Elevation,         // caché derivada
    pub max_invert: Elevation,         // caché derivada
    pub component: ComponentId,        // componente conexa (union-find)
}

pub struct TopoEdge {
    pub id: EdgeId,
    pub element: ElementId,
    pub from: NodeId,
    pub to: NodeId,
    pub s0: f64, pub s1: f64,          // estaciones dentro del elemento (m)
    pub len_2d: Meters,
    pub slope: Slope,                  // derivada de los invert de sus dos puertos
}

pub struct Attachment { pub element: ElementId, pub on: ElementId, pub station: f64 }
```

### 4.2 Cómo se responden las cuatro preguntas del cliente

| Pregunta | Respuesta arquitectónica | Código |
|---|---|---|
| **¿Cómo sabe una tubería los invert de sus extremos?** | No los pregunta: **los tiene**. Cada puerto guarda su `invert`. Lo que el nodo aporta es la *coherencia*: si dos puertos unidos difieren más que `tol.invert`, o hay un elemento de caída explícito, o el nodo reporta un error de validación (ALC-07: "cotas invertidas"). La pendiente de la arista se deriva **solo de sus dos puertos**, nunca del vecino. | `edge.slope = (p_out.invert - p_in.invert) / len_2d` |
| **¿Cómo sabe un pozo qué tuberías le llegan?** | `topo.incident(node_of(pozo))` devuelve `&[EdgeId]`; cada arista sabe su elemento y su estación. El pozo **no almacena** la lista: es una caché derivada. | `for e in topo.incident(n) { let el = topo.edge(e).element; }` |
| **¿Cómo parte una válvula una tubería?** | **No la parte.** La válvula es un elemento `ATTACHABLE` con `Attachment { element, on: tuberia, station }`. La topología emite **dos aristas** sobre el mismo elemento (`s0..s` y `s..s1`). El registro persistido de la tubería no cambia: sigue siendo una tubería. | ver §4.3 |
| **¿Cómo lee la excavación el terreno y la tubería?** | Recibe `&Doc` (terreno + elemento) y trabaja sobre la **arista**, no sobre el vecindario. No consulta otros objetos; los cruces se resuelven con un barrido determinista sobre el índice espacial (§4.6). | `earth::trench_volume(&doc, edge_id, &section)` |

### 4.3 Decisión fina: partir el elemento vs. proyectar el accesorio

| | (A) Partir la tubería al insertar la válvula | (B) **Proyectar el accesorio sobre la tubería** |
|---|---|---|
| Identidad del objeto | se destruye y se crean dos | se conserva |
| Idempotencia para agentes | mala: reintentar duplica o fragmenta | buena: `AttachValve` repetido es no-op (misma clave natural) |
| Reimportar/actualizar desde CAD | el plano trae una tubería, el modelo tiene dos | round-trip directo (VIS-05) |
| Cómputo | natural por elemento | requiere agrupar segmentos por elemento para el metrado |
| Complejidad de la topología | baja | media (estaciones, orden, desempate) |

**Se elige (B).** El costo se paga en la topología (que ya es derivada y reconstruible) y se gana en lo que el producto promete: identidad estable (I1), idempotencia (CLI-04) y round-trip de DXF (VIS-05). La partición física se hace **solo en la frontera de exportación** (DXF/LandXML piden entidades separadas), donde es barata y desechable.

Reglas de estación: `station ∈ [0, L]`; dos accesorios en la misma estación se ordenan por `ElementId` (determinista); un accesorio a menos de `tol.snap` de un extremo se imputa al nodo del extremo en vez de crear un segmento de longitud cero.

### 4.4 Puertos: cuántos y de qué tipo

| Familia | Plantilla | Puertos |
|---|---|---|
| Tubería / colector | `Ends` | `inicio` (flujo, entrada), `fin` (flujo, salida) |
| Pozo | `Fixed` | uno por cada acometida, más `salida`; el número crece con el proyecto → puertos **dinámicos** por instancia |
| Válvula / llave | `Fixed` | `entrada`, `salida` (mismo invert a ambos lados salvo pérdida local) |
| Vertedero / descarga | `Fixed` | `entrada`; el puerto es *libre* (descarga a cuerpo receptor) |
| Tanque | `Fixed` | `entrada`, `salida`, `rebose`, `drenaje` |
| Acometida | `Fixed` | `conexion` (sobre la tubería), `descarga` (a predio) |
| TIN / superficie | ninguno | no participa del grafo |

**Puertos dinámicos:** el pozo es el caso interesante. No se le asignan N puertos al crear: se le añaden cuando una tubería se conecta (`Connect` crea el puerto en el pozo). El esquema declara `max_ports` y una `PortPolicy` (`AnyAzimuth | FixedSlots`), y la validación avisa si un pozo supera el máximo del code pack. Esto evita el error clásico de reservar 4 puertos y quedarse corto en el pozo de un cruce.

### 4.5 Semántica de conexión y *snap*

La regla que sostiene todo el modelo: **la proximidad es un candidato; la conexión es un dato.**

| Gesto | Qué hace | Qué persiste |
|---|---|---|
| Captura a nodo existente | el puerto se une al `NodeId` | miembro nuevo en el nodo |
| Captura a puerto libre | crea un nodo nuevo con ambos puertos | `NodeId` nuevo |
| Captura a segmento de arista | crea un `Attachment` en la estación | accesorio + segmento |
| Captura al TIN | solo ajusta la cota (`cover`/`invert` sugerido) | nada estructural |
| Sin captura | puerto libre | nada |

- Tolerancias por proyecto: `pos_snap = 0.05 m`, `pos_equal = 1e-6 m`, `invert_equal = 0.001 m`.
- La captura durante la importación **no muta en silencio**: produce un informe ("3 tuberías sin conectar, 1 pozo huérfano") que el usuario o el agente revisan (TOP-01 pide exactamente eso: leídos/descartados con motivo).
- `Connect`/`Disconnect` son comandos con inverso. Nunca hay conexión implícita por cercanía en el momento de calcular: si dos puertos están a 1 mm y no hay nodo, la validación lo reporta como "puerto sin conectar", no lo une. Un cálculo que "adivina" conexiones es un cálculo que no se puede auditar.

### 4.6 Reconstrucción de la topología

**Incremental por defecto, completa como red de seguridad.**

```
Command::apply
   └─ deja en EditCtx: touched_elements, touched_geometry, structural: bool
        └─ (fin de la transacción) TopoBuilder::update(&mut Topology, &Doc, &TouchSet)
             1. invalidar puertos y aristas de los elementos tocados
             2. candidatos de unión: índice espacial (rstar) en la ventana de los puertos tocados
             3. reconciliar nodos: union-find sobre coincidencias; nodo nuevo si no hay socio
             4. recomputar aristas: recortar cada elemento lineal en sus estaciones de attachment
             5. recalcular cachés afectadas: incident, min/max invert, componentes conexas
             6. `epoch += 1` y marcar resultados de cálculo obsoletos (§6.5)
```

| Situación | Estrategia | Coste esperado |
|---|---|---|
| Mover un pozo | incremental, 1 nodo + sus aristas | O(grado del nodo) |
| Cambiar el diámetro de 50 tuberías | incremental, sin tocar nodos | O(50) |
| Insertar una válvula | incremental, 1 attachment + 1 arista partida | O(1) |
| Importar un DXF de 2 000 puntos | completa, en worker, con progreso | O(n log n) |
| Abrir un proyecto | completa al cargar (o desde el blob derivado si el `epoch` coincide) | O(n log n) |
| Duda / `--strict` | completa + comparación con la incremental (debug) | 2× |

**Regla de oro:** la topología es *derivable por completo del documento*. Se puede tirar y reconstruir sin pérdida, salvo los `NodeId` (que se persisten para que los agentes y el undo los direccionen). Corrección: en compilación `debug` y en `--strict`, tras cada transacción se compara `topology_incremental == topology_full` (§10.4). Un fallo de reconstrucción incremental se detecta en CI, no en el expediente del cliente.

### 4.7 Conflictos de préstamo y ciclos

| Problema | Solución |
|---|---|
| `&Element` retenido mientras se muta el `Doc` | La API de topología **nunca devuelve `&Element`**: devuelve `ElementId`, `Copy`, o `Vec<ElementId>`. `Doc::get(id)` revalida `slot`/`gen` en cada llamada. |
| Dos estructuras que se prestan mutuamente | `Topology` es dueña de sus datos y **no presta del `Doc`**: guarda IDs y copias de posición/cota. Se reconstruye con `&Doc` y se consulta sin él. |
| Ciclos de *referencia* en el modelo | Prohibidos por diseño (I7): la contención es un árbol (`parent: Option<ElementId>`) con profundidad máxima validada; no hay referencias entre pares. |
| Ciclos de *flujo* (redes en lazo) | Permitidos y necesarios (AGU-01, agua potable). El solver de gradiente global los resuelve; la validación ALC-07 solo los prohíbe donde la norma lo exige (alcantarillado por gravedad). |
| Recursión infinita en traversals | Todo recorrido usa conjunto de visitados y profundidad máxima; ninguna función pública de topología es recursiva sin cota. |
| Reentrada por eventos | Los suscriptores no mutan (`&Doc`), solo emiten `Effect` (§6.6). |

---

## 5. Coordenadas y geometría

### 5.1 Decisión numérica

| Aspecto | Decisión |
|---|---|
| Tipo | `f64` en todo el dominio. `f32` **solo** en la frontera de render. |
| CRS | Proyectado, UTM. Ecuador: **EPSG:32717 / 32718** (WGS84 17S/18S) y **24877 / 24878** (PSAD56 17S/18S), según TOP-03. |
| Almacenamiento | Easting/Northing **absolutos** en metros dentro del `Doc` (round-trip de exportación sin drift). |
| Estabilidad | `ProjectOrigin { easting, northing }` en la metadata; toda la aritmética de cámara y toda conversión a `f32` restan el origen **en f64 primero**. |
| Datum vertical | Cota **ortométrica** en metros, con `VerticalDatum` declarado (`Local | EGM2008 | IGM-Quito`) y `geoid_offset_m` editable. Se guarda el datum, no se adivina. |
| Unidades | Ver §5.2. Sin excepciones. |
| Tolerancias | Objeto `Tolerance` en la metadata del proyecto (§5.3). |
| Precisión de despliegue | Se muestra según configuración; se **almacena** siempre `f64` en m (y mm enteros para diámetros comerciales). |

**Por qué el origen local es obligatorio y no opcional:** Rune trabaja en puntos `f32`. Una coordenada UTM de ~500 000 m en `f32` tiene una resolución de ~0.03 m; el modelo tiembla y las líneas se cruzan visualmente. La conversión correcta es `((world_f64 - origin_f64) * zoom) as f32`, y el origen debe **recentrarse** cuando la cámara se aleja más de ~10 km ("floating origin"), de modo que la magnitud local se mantenga por debajo de ~10⁴ m y la resolución de `f32` sea ≤ 1 mm.

```rust
// redes-crs/src/project.rs
pub struct ProjectOrigin { pub easting: f64, pub northing: f64 }

impl ProjectOrigin {
    /// Mundo → espacio local (f64) → pantalla (f32). Única ruta permitida hacia el renderer.
    pub fn to_screen(&self, p: Point2, cam: &Camera) -> [f32; 2] {
        let lx = (p.x - self.easting) * cam.zoom + cam.pan.x;
        let ly = (p.y - self.northing) * cam.zoom + cam.pan.y;
        [lx as f32, ly as f32]
    }
}
```

### 5.2 Unidades en la frontera (tabla normativa)

| Magnitud | Unidad interna | Tipo | Sufijo en JSON (CLI-02) | Ejemplo |
|---|---|---|---|---|
| Diámetro **nominal** (designación comercial: 110, 200, 315) | **milímetro** | `NominalMm` | `_mm` | `diametro_nominal_mm: 200` |
| Diámetro **geométrico** (interior/exterior de pozo, tanque, cámara) | **metro** | `Meters` | `_m` | `diametro_interior_m: 0.196` |
| Longitud, coordenada, cota | **metro** | `Meters` / `Elevation` | `_m` | `longitud_m: 42.318` |
| Pendiente | **m/m** (adimensional) | `Slope` | `_m_m` | `pendiente_m_m: 0.0035` |
| Caudal | **m³/s** | `FlowM3s` | `_m3s` | `caudal_m3s: 0.0125` |
| Volumen | **m³** | `CubicMeters` | `_m3` | `volumen_m3: 118.402` |
| Presión | **mca** (columna de agua) | `MetersOfWater` | `_mca` | `presion_mca: 18.24` |
| Ángulo | radián interno, grado en UI/JSON | `Radians` / `Degrees` | `_deg` | `angulo_deg: 45.0` |
| Rugosidad Manning | s·m^-1/3 | `ManningN` | `_n` | `coeficiente_n: 0.013` |

Reglas duras, verificadas por prueba:

1. Ningún `f64` desnudo cruza una frontera de módulo: se usa el newtype. La prueba de arquitectura prohíbe `f64` en firmas `pub` de `redes-hydro` y `redes-earth`.
2. La clave interna del atributo **no lleva unidad** (`diametro_interior`); el sufijo se añade **al serializar**, desde `AttrDef.unit`. Así el JSON cumple CLI-02 sin que el modelo tenga nombres distintos por formato.
3. Un atributo con `unit: None` nunca se serializa con sufijo.
4. **Dos regímenes de diámetro, nunca mezclados.** El diámetro *nominal* es una designación comercial y va en **mm enteros** (200, no 0.2). El diámetro *geométrico* (interior de un pozo de 1,20 m, diámetro exterior de un tanque) va en **metros**. Son newtypes distintos (`NominalMm` y `Meters`), de modo que el compilador impide la confusión. Esto concuerda con `01-domain.md` §4: `diametro_nominal` en mm (25…2 000) y `diametro_interior` de pozo en m (0,90 / 1,20 / 1,50). Un atributo que declare la unidad equivocada es un error de validación (MOD-07, "unidades incoherentes"), no una coerción silenciosa: confundir 200 mm con 200 m es un error de 1 000× que no se ve en pantalla.

### 5.3 Tolerancias

```rust
pub struct Tolerance {
    pub pos_snap: f64,      // 0.05 m — captura
    pub pos_equal: f64,     // 1e-6 m — igualdad geométrica
    pub invert_equal: f64,  // 0.001 m — coherencia de cotas en un nodo
    pub slope_min: f64,     // 0.001 m/m — pendiente mínima constructiva
    pub angular: f64,       // 1e-9 rad — colinealidad
    pub area: f64,          // 1e-6 m² — polígonos degenerados
}
```

- Regla: **ningún `==` sobre `f64`** en código de dominio. Todas las comparaciones pasan por `redes_geom::approx::{eq, le, ge}` con tolerancia **escala-consciente**: se comparan longitudes en el sistema local, no en UTM absoluto.
- En la geometría robusta (orientación, incircle) se usan predicados exactos (`robust`, vía `geo`), no épsilon: un TIN construido con épsilon produce triángulos invertidos.
- Las tolerancias son **datos del proyecto**, se guardan en el archivo y se imprimen en la memoria (INF-01), porque un volumen calculado con otra tolerancia no es el mismo volumen.

### 5.4 Crate de geometría: elección

| Crate | Veredicto | Motivo |
|---|---|---|
| `geo` + `geo-types` | ✅ **sí**, para geometría plana 2D | Algoritmos maduros (intersección, buffer, área, punto en polígono), predicados robustos vía `robust`, `f64`. Es el "kit de cocina" del plano. |
| `spade` | ✅ **sí**, para el TIN | Es el único del ecosistema con **Delaunay restringido (CDT)**: sin breaklines no se pueden modelar quiebres, zanjas ni bordes de plataforma. TOP-02 lo necesita. |
| `rstar` | ✅ **sí**, índice espacial | R-tree para picking, captura, cruces y ventanas de consulta. |
| `earcutr` | ✅ sí, triangulación de relleno | Para pintar polígonos en el renderer (sombreado del TIN, secciones). |
| `kurbo` | ❌ no | Orientado a curvas y aplanado de UI; 2D; no aporta sobre `geo` para el dominio y añade un segundo vocabulario de tipos. |
| `parry2d` | ❌ no | Es un motor de colisión (física), con `nalgebra` de arrastre y foco `f32`. No necesitamos CCD ni resolución de contactos. |
| Todo propio | ❌ no | Reimplementar Delaunay restringido y predicados robustos es un proyecto en sí mismo y un riesgo de corrección inaceptable. |

Lo propio se reserva a lo que ningún crate hace: **sólidos de zanja** (prismas entre secciones, con talud y cama), **estacionado** sobre polilínea, y la **aritmética vertical** (invert, tapa, profundidad, caída).

### 5.5 Consecuencias

- **A favor:** exportar en coordenadas absolutas es trivial y sin drift; el render no tiembla; las tolerancias son auditables y viajan en el archivo.
- **A favor:** `redes-crs` sin PROJ mantiene el instalador pequeño y el resultado determinista; el residuo del Helmert se registra y se imprime (TOP-03).
- **En contra:** hay que implementar y probar Transversa de Mercator. Mitigación: puntos de prueba EPSG + comparación dorada contra `cs2cs` en CI; si en el futuro se necesita más de un CRS exótico, la puerta es un `dyn CrsTransform` en `redes-crs` sin tocar a los consumidores.
- **En contra:** el "floating origin" complica la cámara. Se encapsula en `Camera` y nadie más lo ve.

---

## 6. Estado, deshacer/rehacer y eventos

### 6.1 Decisión: comando con inverso, y un solo camino de mutación

```rust
// redes-cmd/src/lib.rs
pub trait Command: Send + Sync + std::fmt::Debug {
    fn name(&self) -> &'static str;                       // "element.create" — estable, va al manifiesto
    fn natural_key(&self, doc: &Doc) -> Option<NaturalKey>; // CLI-04: clave natural para idempotencia
    fn validate(&self, doc: &Doc) -> Result<(), Reject>;    // sin mutar: base de --dry-run
    fn apply(&self, doc: &mut Doc, cx: &mut EditCtx<'_>) -> Result<Inverse, Reject>;
    fn encode(&self) -> CommandEnvelope;                    // JSON versionado para CLI/MCP
}

pub struct Inverse(Box<dyn Command>);   // el inverso es, a su vez, un comando

pub struct Txn {
    pub id: TxnId,
    pub label: i18n::Key,
    pub actor: Actor,                  // humano | agente | importador
    pub commands: Vec<Box<dyn Command>>,
    pub expected_revision: Option<Revision>, // concurrencia optimista (CLI-05, código 6)
}
```

Ciclo de vida de una transacción:

```
validate (todo) → begin → apply(c1) → apply(c2) … → topo.update → publish(Arc<Doc>) → eventos → efectos → commit
                                                                                              │
                                   historial ← [inversos en orden inverso, con metadatos] ────┘
```

- **Deshacer** aplica los inversos en orden inverso. **Rehacer** reaplica. (PRY-05: 50 pasos por sesión, con objeto, hora y usuario en cada entrada, exportable a CSV.)
- **El historial de sesión es una *vista* sobre un journal de revisiones persistente** (§7.1). Deshacer **no borra historia**: añade una revisión inversa, de modo que la auditoría del expediente (COL-04), `--expect-revision` (CLI-05) y el Ctrl+Z de la UI cuentan exactamente la misma historia. Los 50 pasos de PRY-05 son la ventana de la UI, no el límite del archivo; `project gc --keep-revisions 50` decide cuánto se conserva.
- **Por qué inversos y no snapshots:** un snapshot de 100k objetos por edición es inviable en memoria y en tiempo. Los inversos son O(cambio). El costo es que **todo comando debe saber deshacerse**: un `Delete` guarda los registros eliminados completos en su inverso (payload acotado por la profundidad del historial); un `Create` guarda los IDs asignados.
- **Fusión de comandos** (coalescing): arrastrar un vértice produce 200 `MoveVertex`; se fusionan en uno por `(elemento, sesión de arrastre)` para que un Ctrl+Z deshaga el arrastre completo.

### 6.2 `State` (UI) vs `Document`

| | `Document` | `State` |
|---|---|---|
| Qué contiene | el modelo: familias, elementos, geometría, topología, revisiones | lo efímero: selección, cámara, herramienta activa, panel abierto, previsualización |
| Quién lo muta | solo `Command::apply` | solo los manejadores de la vista (`&mut State`) |
| Se persiste | sí | no (salvo preferencias, aparte) |
| Se comparte con workers | sí, como `Arc<Doc>` | no |
| En Rune | `state.doc: Arc<Doc>` | el resto de `State` |

```rust
// redes-ui/src/state.rs
pub struct State {
    pub doc: Arc<Doc>,             // verdad publicada, inmutable en el frame
    pub history: History,          // deshacer/rehacer + etiquetas
    pub sel: Selection,            // IDs seleccionados (no referencias)
    pub view: PlanView,            // cámara, LOD, capas visibles
    pub tool: Tool,                // Select | DrawPipe | PlaceManhole | …
    pub preview: Option<Preview>,  // arrastre en curso: NO toca el Doc hasta confirmar
    pub jobs: Jobs,                // trabajos en curso + Live<f32> de progreso
    pub ui: UiState,               // paneles, diálogos, foco, inspector
}

impl State {
    /// Punto ÚNICO de mutación del modelo desde la UI.
    pub fn commit(&mut self, txn: Txn) -> Result<Outcome, Reject> {
        let mut doc = (*self.doc).clone_for_write();   // copia al escribir por familia
        let outcome = doc.apply_txn(txn)?;
        self.history.push(&outcome);
        self.doc = Arc::new(doc);                      // publicación barata
        self.drain_events_and_effects();
        Ok(outcome)
    }
}
```

Esto encaja con Rune sin fricción: la vista es `fn view(s: &State, cx: &mut Cx<State>) -> El`, los manejadores corren **entre** frames con `&mut State`, y la vista se vuelve a ejecutar después. La arquitectura no pelea con el framework: lo usa como está diseñado.

**Previsualización vs. mutación.** Arrastrar un vértice con 100k objetos no puede publicar un `Arc<Doc>` nuevo por frame. Mientras se arrastra, `State.preview` guarda el delta y la vista lo dibuja como superposición; al soltar se emite **una** transacción. Es el patrón de toda app CAD y es también lo que hace que el deshacer sea legible ("Mover pozo" y no 200 movimientos).

### 6.3 La CLI reutiliza exactamente los mismos comandos

```rust
// redes-api/src/lib.rs
pub struct Request {
    pub command: String,            // "element.create" — del manifiesto
    pub args: serde_json::Value,
    pub dry_run: bool,              // CLI-04
    pub expected_revision: Option<u64>,
    pub idem: Option<NaturalKey>,   // CLI-04
    pub project: ProjectPath,
}

pub struct Response {
    pub ok: bool,
    pub command: &'static str,
    pub revision_before: u64,
    pub revision_after: u64,
    pub created: Vec<IdRef>,        // {"ref": "t1", "id": "0004:0001"}
    pub numeric_hash: Option<Hash>, // INF-01 / HID-04
    pub diagnostics: Vec<Diagnostic>,
    pub exit_code: i32,             // 0,2,3,4,5,6 (CLI-05)
}
```

- `redes <comando> --json` construye el mismo `Box<dyn Command>` que la UI y lo ejecuta contra el mismo `Doc` y el mismo `CalcEngine`. La única diferencia es la serialización.
- `redes esquema` (CLI-03) se genera **iterando el registro de comandos**, no de una lista escrita a mano; la prueba de CI falla si un comando registrado no aparece o si un comando del manifiesto no tiene ejemplo ejecutado (CLI-01).
- `redes mcp` (CLI-06) expone el **mismo** manifiesto como tools; la prueba de contrato compara ambos.

### 6.4 Determinismo e idempotencia para agentes

| Requisito | Mecanismo |
|---|---|
| Misma entrada → mismo resultado | Sin tiempo, sin azar, sin orden de `HashMap`, sin paralelismo dentro de un cálculo. Un `Clock` inyectable (rasgo-puerto en `core`) es la única fuente de tiempo. |
| Mismos números en UI y CLI | Un solo `CalcEngine`; el resultado lleva `numeric_hash` sobre la codificación canónica de los `f64` (decimal de ida y vuelta más corto) y el orden de acumulación es fijo (por `ElementId`, luego por estación). |
| Reintentar no duplica | `natural_key` declarada por el comando (p. ej. `(familia, nombre, xy redondeado a tol)`); un índice de claves naturales aplicadas por sesión-devuelve `NoOp` en vez de crear. |
| No escribir nada en `--dry-run` | `validate()` corre la transacción completa sobre una copia (o sobre un `Doc` clonado estructuralmente) y se descarta; la prueba de CLI-04 compara el hash del archivo antes y después. |
| No pisar trabajo ajeno | `expected_revision`; si no coincide → `exit 6` con `codigo: "conflicto_version"` y la revisión actual. |
| Reproducibilidad del cómputo | Toda suma de volúmenes usa **suma compensada (Neumaier)** con orden fijo, para que Σtramos = total dentro de 0,1% (EXC-02) sin depender del orden de recorrido. |

### 6.5 Eventos: tipados, con *efectos*, sin ciclos

```rust
pub enum DocEvent {
    ElementAdded { id: ElementId, family: FamilyId },
    ElementRemoved { id: ElementId },
    AttrChanged { id: ElementId, key: AttrId },
    GeometryChanged { id: ElementId },
    TopologyRebuilt { epoch: Epoch, dirty: DirtyScope },
    CalcStale { scope: Scope },
    CalcFinished { scope: Scope, numeric_hash: Hash },
    SelectionChanged,
    RevisionChanged { from: Revision, to: Revision },
}

pub trait Subscriber: Send {
    /// Recibe solo lectura. NO puede mutar el documento: devuelve efectos.
    fn on_event(&mut self, ev: &DocEvent, doc: &Doc) -> SmallVec<[Effect; 4]>;
}

pub enum Effect {
    RebuildTopo(TouchSet),
    SolveHydraulics(Scope),
    RecomputeVolumes(Scope),
    InvalidateRender(LayerMask),
    Autosave,
    Notify(Notification),
}
```

**El diseño que rompe los ciclos** son tres reglas:

1. Los suscriptores reciben `&Doc`, nunca `&mut Doc`. No hay forma de que un evento mute el modelo.
2. Un suscriptor **no llama a otro suscriptor**: devuelve `Effect`, y el planificador los aplica en una segunda fase con orden fijo (topología → cálculo → render → persistencia).
3. La fase de efectos tiene un presupuesto de rondas (máx. 4). Si se agota, se fuerza una reconstrucción completa y se registra el incidente. Nunca hay oscilación infinita.

Es el mismo patrón que Rune usa internamente para sus máquinas de estado (`step(state, event) -> Effects`): una decisión consistente con el anfitrión, no una capa extra.

### 6.6 Consecuencias

- **A favor:** deshacer es exacto; la CLI y la UI no pueden divergir; los agentes obtienen idempotencia y concurrencia optimista gratis.
- **En contra:** escribir un comando nuevo cuesta más que un setter (validate + apply + inverse + encode + natural_key). Mitigación: macro `command!` para los triviales y un *checklist* de PR; la suite de propiedades (§10.3) verifica automáticamente `apply(inverso(apply(c))) == identidad` para todo comando registrado, que es el error caro.
- **En contra:** las transacciones largas (importar 100k) deben ser una sola transacción; el historial guarda su inverso (que puede ser grande). Se acepta: un import es raro y su inverso es "borrar lo creado por IDs".

---

## 7. Persistencia

### 7.1 Decisión

**Un archivo `.redes`, que es una base SQLite embebida (`rusqlite` con `bundled`), en modo WAL, usada como *contenedor transaccional* — no como modelo relacional de atributos.**

Lo que cambia respecto de la intuición "SQLite = tablas por familia": **los atributos no son columnas SQL**. Son blobs binarios canónicos, en *chunks* de filas.

| Tabla | Contenido | Por qué |
|---|---|---|
| `meta(key, value)` | `format_version`, `schema_version`, CRS, datum, origen, unidades, tolerancias, code pack activo, `numeric_hash` de los últimos cálculos | lo que hay que leer sin cargar el modelo |
| `family(key, version, manifest_json, hash)` | manifiestos de familia (propios y de terceros) | el proyecto es autodescriptivo: sin el manifiesto no se puede interpretar el blob |
| `family_chunk(family_key, chunk_idx, rows, bytes)` | **el modelo**: 1024 filas por chunk, columnares, codificadas en binario canónico | granularidad de escritura ~100 KB; sin DDL dinámico; sin EAV |
| `geometry(id, kind, bytes)` | polilíneas/puntos/polígonos, con `Arc` compartible en memoria | separa topología de representación |
| `topology_cache(epoch, bytes)` | topología derivada y su epoch | abrir un proyecto grande sin recalcular |
| `revision(rev, parent_rev, txn, actor, ts, label, inverse_bytes)` | **journal de revisiones append-only**: cada transacción confirmada, con su inverso | es la fuente de verdad de `--expect-revision`, del deshacer (§6.1) y de la auditoría (COL-04). Deshacer añade, no borra. |
| `code_pack(id, version, jurisdiction, bytes, signature)` | normas y parámetros | trazabilidad HID-03 / INF-01 |
| `audit(ts, actor, action, target, detail)` | bitácora inmutable | COL-04 |
| `attachment(name, bytes)` | DXF fuente, imágenes, plantillas | el proyecto viaja completo |

Por qué el *chunk* y no una fila por elemento: una fila por elemento pierde la localidad de columna que hace rápido recorrer una familia entera, y añade ~40 bytes de overhead por fila; un blob por familia no permite guardar solo lo que cambió. El chunk de 1024 filas es el punto donde ambas cosas se cumplen: recorrer una familia es leer chunks secuenciales, y editar un objeto toca un chunk.

### 7.2 Alternativas consideradas

| Alternativa | Veredicto | Motivo |
|---|---|---|
| **SQLite, modelo como blobs por chunk** | ✅ **elegida** | Un archivo, transaccional, con WAL (lectores concurrentes), guardado incremental por página, `schema_version` con migraciones, `integrity_check`, y la CLI puede leer el proyecto **mientras la app está abierta** (PER-01, COL-01). |
| SQLite, una tabla por familia con columnas | ❌ | Exige DDL dinámico al añadir familias (familia de terceros en runtime = `ALTER TABLE` en un archivo de usuario), o EAV, que es lento e ilegible. Contradice EXT-01. |
| SQLite, EAV (`element_attr(element, key, value)`) | ❌ | 100k × 20 atributos = 2M filas con `TEXT`; consultas lentas, tipos perdidos, sin validación. |
| Un JSON único | ❌ | 100k objetos → 50–200 MB que se reescriben en cada autoguardado; sin transaccionalidad; un corte de luz corrompe el archivo (PER-02). |
| ZIP de JSON + binarios | ❌ | Reescribir el ZIP completo en cada guardado; sin bloqueo fino; sin lectura parcial; la CLI tendría que descomprimir todo. |
| Archivo propio con *memory-mapped* + journal | ❌ | Un año de trabajo en un formato binario propio sin herramientas de recuperación. SQLite ya resolvió eso. |

**Nota de conciliación con `05-product.md` (PER-01):** el documento de producto pide "contenedor SQLite embebido con `schema_version` y migraciones automáticas". Se mantiene tal cual. Lo que este ADR precisa es **qué hay dentro**: no un esquema relacional por familia, sino blobs canónicos por chunk. Eso es lo que permite que EXT-01 (familias en runtime) y PER-01 (SQLite) sean compatibles.

### 7.3 Versionado y migraciones

```rust
pub struct Manifest {
    pub format_version: u32,                 // del contenedor; se rechaza si > CURRENT
    pub schema_version: u32,                 // del modelo
    pub families: BTreeMap<FamilyKey, u16>,  // versión por familia
    pub crs: CrsId, pub datum: VerticalDatum, pub origin: ProjectOrigin,
    pub tolerance: Tolerance,
    pub code_pack: Option<PackRef>,
}
```

- Migración en cadena: `Migration { from: u32, to: u32, apply: fn(&mut MigrationCtx) -> Result<(), MigrateError> }`, una por paso, registradas en orden y probadas con un archivo de fixture por versión.
- **Nunca se abre en silencio un archivo más nuevo**: `format_version > CURRENT` → `exit 4` con mensaje accionable ("actualiza la aplicación"). Un archivo viejo se migra, se informa qué se migró y se guarda un `.bak` previo (PRY-02, PER-03).
- Renombrar un atributo = alias en la migración de la familia, no edición del blob.
- Si falta una familia (proyecto de otra instalación): se abre en modo **solo lectura parcial**, se dice qué plugin instalar y **no se destruye nada** (PRY-02).

### 7.4 Autoguardado, bloqueo y recuperación

| Necesidad | Decisión |
|---|---|
| Autoguardado (PER-02) | Cada 30 s **y** antes de todo cálculo largo: una transacción SQLite que escribe solo los chunks sucios. Se ejecuta en un worker con un `Arc<Doc>` inmutable; nunca en el frame. |
| Pérdida máxima | ≤ 30 s de trabajo. Con WAL, un corte de luz deja la base consistente; a lo sumo se pierde la última transacción no confirmada. |
| Bloqueo (COL-01) | Tabla `lock(pid, host, user, since, heartbeat)` en modo `EXCLUSIVE` para escritura; el `heartbeat` se renueva cada 30 s; a los 15 min sin latido el bloqueo es reclamable. La CLI abre en solo lectura si el bloqueo está vivo. |
| Integridad (PER-03) | `PRAGMA integrity_check` al abrir + checksum de los blobs por chunk; si falla, se ofrece el `.bak` rotativo (5 versiones). `redes proyecto verificar --json` devuelve el estado. |
| Respaldos | Rotativos de 5, por copia del archivo con SQLite en modo *backup API* (consistente en caliente). |

### 7.5 El "code pack"

```rust
pub struct CodePack {
    pub id: PackId,                  // "EC-PICHINCHA-EMAAP-2024"
    pub jurisdiction: Jurisdiction,  // país / ciudad / EMAP
    pub standard: StandardRef,       // "CPE INEN 5", "RAS 2000", "NEC-11"
    pub edition: Edition,            // "2019" / "revisión 3"
    pub source: SourceRef,           // dónde está el documento y su hash
    pub clauses: Vec<Clause>,        // id, texto citable, severidad
    pub params: BTreeMap<ParamKey, ParamValue>,
    pub defaults: BTreeMap<(FamilyKey, TypeKey), Vec<(AttrKeyName, Value)>>,
    pub checks: Vec<CheckDef>,       // qué se valida, con qué límite y qué cláusula
    pub signature: Option<Signature>,
}
```

- El code pack es **dato versionado, no código**. Se carga en el proyecto (`meta.code_pack`) y se puede cambiar; cada resultado de cálculo registra `PackRef { id, version, hash }`.
- HID-03 exige que cada veredicto lleve `id`, `clausula`, `severidad`, `objeto`, `valor`, `limite`; y que si el pack está incompleto el veredicto sea **"no evaluable"** con los campos faltantes. Eso se modela en `CheckResult`, no en texto libre.
- Un code pack se puede firmar (EXT-04) y su `hash` entra en el `numeric_hash` de la memoria: cambiar el pack cambia el hash, que es exactamente lo que un revisor quiere ver.

### 7.6 Consecuencias

- **A favor:** cumplir PER-01/02/03/04/05 sin inventar formato; la CLI lee el proyecto abierto; el guardado es incremental y barato; el archivo es un solo objeto que se copia y se envía.
- **En contra:** el archivo no es *diffable* con `git`. Mitigación: `redes proyecto exportar --json` produce una vista canónica y ordenada, apta para diff y para auditoría; y el `numeric_hash` detecta cambios semánticos.
- **En contra:** se pierde el target `wasm32` para persistencia (SQLite C). Se acepta: el producto es de escritorio. `ProjectStore` es un rasgo, así que un futuro backend web no requiere tocar el dominio; solo un `impl`.
- **Regla que se mantiene de Rune:** `redes-ui` **no** depende de `redes-store` para nada que ocurra en un frame; y ninguna API de persistencia recibe `PathBuf` en su firma pública — recibe un `ProjectHandle`. Así el día que se quiera una variante web, el dominio no se toca.

---

## 8. Mecanismo de extensibilidad

### 8.1 Decisión

**Registro explícito en tiempo de ejecución, alimentado por manifiestos de datos y por plugins compilados. Sin capa de scripting en v1.**

```rust
// redes-plugins/src/lib.rs — la raíz de composición, el único lugar que conoce a todos
pub fn register_all(reg: &mut Registry) {
    redes_families::register(reg);          // pozos, tuberías, válvulas, llaves, tanques, acometidas, vertederos
    redes_calc::register_default(reg);      // manning, hazen-williams, darcy, excavación, balance de tierras
    redes_io::register_default(reg);        // dxf, geojson, csv, landxml, xlsx, pdf
    redes_report::register(reg);            // memoria, listados, perfil, planos
}

pub struct Registry {
    families: FamilyRegistry,
    calcs: CalcRegistry,        // Box<dyn Calculation>
    importers: ImporterRegistry,// Box<dyn Importer>
    exporters: ExporterRegistry,// Box<dyn Exporter>
    reports: ReportRegistry,
    behaviors: BehaviorRegistry,// Box<dyn FamilyBehavior> — la excepción, no la regla
}
```

| Mecanismo | Se usa para | Por qué |
|---|---|---|
| **Manifiesto de datos** (JSON) | familias, tipos, atributos, restricciones, símbolos, contrato de cálculo | es lo único que permite EXT-01 (cargar sin recompilar ni reiniciar) y MOD-08 en el futuro |
| **`Box<dyn Trait>` en el registro** | cálculos, importadores, exportadores, informes | número abierto, heterogéneo, elegido en runtime; el despacho ocurre una vez por operación, no por objeto |
| **Lista explícita de plugins** | componer el registro al arrancar | determinista (orden fijo), greppable, funciona en todas las plataformas, sin magia del enlazador |
| **Features de Cargo** | adelgazar el binario (sin LandXML, sin XLSX) | el instalador no arrastra lo que nadie usa |
| **`catch_unwind` + capacidades declaradas** | contener un plugin defectuoso (EXT-04) | un `panic` en un plugin no debe cerrar la app ni corromper el proyecto |
| **WASM (futuro)** | aislamiento real y scripting de usuario | la puerta está diseñada: los rasgos cruzan solo datos planos |

### 8.2 Por qué registro explícito y no `inventory`

`inventory` (auto-registro por sección del enlazador) es tentador, pero: (a) el orden de registro queda indefinido y este sistema necesita orden determinista para que el manifiesto y los hashes sean estables; (b) no funciona igual en todas las plataformas/objetos de enlace; (c) hace invisible qué está cargado. Una lista de cuatro líneas en `redes-plugins` es más honesta y no es un dios-`match`: es una lista de plugins, y añadir uno *es* un acto de composición.

### 8.3 El contrato de cálculo (EXT-02) y su suite de conformidad (EXT-03)

```rust
pub trait Calculation: Send + Sync {
    fn id(&self) -> CalcId;                          // "saneamiento.manning"
    fn inputs(&self) -> &'static [InputSpec];        // (familia, atributo, unidad) que necesita
    fn outputs(&self) -> &'static [OutputSpec];
    fn run(&self, ctx: &CalcCtx<'_>) -> Result<CalcOutput, CalcError>;
    fn provenance(&self, pack: Option<&PackRef>) -> Provenance;
}
```

- Una familia declara en su manifiesto `calc: { hydraulic: "saneamiento.manning", earthwork: "comun.zanja_areas_medias" }`. Si no lo declara, la familia es **"solo dibujo"** y no entra en cómputos (EXT-02 lo exige explícitamente).
- La **suite de conformidad** es una batería de casos (entradas canónicas + salidas esperadas + tolerancia) que un cálculo debe pasar para habilitarse. Corre contra los cálculos propios en CI; es el mismo arnés que usaría un tercero (EXT-03 pide que el ejemplo "familia vertedero" la pase).
- Un cálculo que falla la suite se registra como no disponible y el veredicto de la validación pasa a "no evaluable", no a "cumple".

### 8.4 Familias definidas por el usuario

Hoy: **manifiesto declarado por un tercero (o por el propio equipo), cargado en runtime** — EXT-01, v0.3.
Después (v1.1, MOD-08): **autoría desde la UI**. Gracias a §3, esa funcionalidad es *un editor de manifiestos*, no una reingeniería: el modelo ya la soporta. Es exactamente la razón de haber elegido (c) en §2.

### 8.5 Por qué no scripting en v1

| Opción | Veredicto |
|---|---|
| Lua/Rhai embebido | ❌ duplica el modelo de datos en un lenguaje sin tipos, mata el determinismo y el `numeric_hash`, y abre un agujero de seguridad en un producto que se firma y se entrega a municipios. |
| WASM (wasmtime) | ⏳ diseñado, no implementado. Es la respuesta correcta si EXT-04 sube de prioridad: aislamiento real, límites de memoria, determinismo configurable. |
| CLI + JSON como "scripting" | ✅ **sí, hoy**: el agente es el intérprete. CLI-01/03/07 ya dan el 100% de las capacidades en un lenguaje con tipos verificables por esquema. |

**Consecuencia:** la extensibilidad de v1 es de *datos* (familias) y de *plugins compilados* (cálculos, formatos). Es el mínimo que no exige reescritura: si mañana entra WASM, el `Calculation` ya recibe datos planos y devuelve datos planos, que es la forma de una frontera FFI.

---

## 9. Hilos y presupuesto de rendimiento

### 9.1 Decisión: un hilo de UI, trabajadores por trabajos gruesos

| Hilo | Hace | Nunca hace |
|---|---|---|
| **UI (principal)** | construir la vista, layout, entrada, dibujar, aplicar comandos y publicar `Arc<Doc>` | leer/escribir archivos, construir el TIN, resolver hidráulica, calcular volúmenes, importar/exportar |
| **Workers (2–4)** | TIN, hidráulica, volúmenes, importación, exportación, informes, autoguardado | tocar `State`, mutar el `Doc`, dibujar |
| **Render (Rune)** | GPU, atlas de texto, presentación | — |

```rust
pub enum Job {
    BuildTin   { surface: ElementId, points: Arc<[Point3]>, breaklines: Arc<[Polyline]> },
    Solve      { scope: Scope, pack: PackRef, snapshot: Arc<Doc> },
    Volumes    { scope: Scope, section: SectionId, snapshot: Arc<Doc> },
    Import     { format: FormatId, source: SourceRef },
    Export     { format: FormatId, scope: Scope, dest: DestRef },
    Report     { kind: ReportKind, scope: Scope },
    Autosave   { snapshot: Arc<Doc>, rev: Revision },
}

pub struct JobResult {
    pub kind: JobKind,
    pub rev: Revision,          // revisión del snapshot usado
    pub topo_epoch: Epoch,      // época de la topología usada
    pub payload: ResultPayload, // resultado nuevo, o un Command a aplicar
}
```

**Reglas de paso de datos (las que evitan condiciones de carrera sin candados en el frame):**

1. El worker recibe `Arc<Doc>` **inmutable**: consistencia de snapshot gratis, sin `RwLock`, sin `Mutex` en el camino de la UI.
2. El worker **no muta nada**. Devuelve un `JobResult`; si el resultado debe entrar al modelo (p. ej. un TIN construido), lo hace como un `Command` (`CommitTin`) que la UI aplica en un manejador. Así el undo cubre también lo que produjo un worker.
3. Si `result.rev != state.doc.revision`, el resultado está obsoleto y se descarta (o se re-lanza). Es la única regla de coherencia necesaria.
4. El progreso se comunica con `Live<f32>` (handle atómico *latest-wins* de Rune): el worker escribe, la UI lee en el frame sin bloquear y **sin re-ejecutar la vista**. Para el texto de fase se usa un `Arc<AtomicU8>`.
5. Cancelación con `Arc<AtomicBool>`; el worker comprueba en los puntos de control y devuelve `Cancelled`.

### 9.2 La regla dura: cero E/S en el bucle de frames

Rune lo exige explícitamente por una razón concreta en Windows: cada apertura y cada renombrado pasan por filtros antivirus y pueden fallar con *sharing violation*. Se eleva a invariante del proyecto:

> **Ninguna operación de archivo, de red, de base de datos o de sincronización puede ejecutarse en el hilo de UI durante un frame, una entrada o una animación.**

Se hace cumplir de tres formas:

1. **Prueba de arquitectura** (§10.6): un `IoGuard` (bandera en *thread-local*) se activa alrededor de cada frame en compilación `debug` y `test`; el `ProjectStore` y los importadores hacen `panic!` si se los toca con el guard activo. La prueba conduce un frame completo y falla si alguien abre un archivo.
2. **Dependencia**: `redes-ui` no depende de `redes-store` para el camino de frame (solo para acciones explícitas de "guardar", que emiten un `Job::Autosave`).
3. **Revisión**: ninguna función `pub` de `redes-ui` recibe un `PathBuf` ni un `File`.

**Reglas de Rune que se respetan al pie de la letra** (están en su documentación y no son opcionales): trabajo lento desde `on_ready`/manejador, nunca en el primer frame; `poll` devuelve `true` solo si cambió algo visible; un frame inactivo no dibuja; `Tx::instant()` para lo que llega tarde; `Live<T>` para valores que cambian sin re-ejecutar la vista; `virtual_rows` y `memo` para tablas; presupuesto de caché acotado en bytes.

### 9.3 Presupuesto

| Escenario | Objetivo | Dónde se mide |
|---|---|---|
| Frame con proyecto de 10k objetos, vista de planta | ≤ 8,3 ms (120 Hz) con vista estática; ≤ 16,6 ms durante pan/zoom | `criterion` + `--profile perf` de Rune |
| Frame con 100k objetos, vista de planta | ≤ 16,6 ms; pan/zoom sin re-ejecutar la vista (`Live`) | ídem |
| Abrir proyecto de 5 000 objetos (PER-01) | ≤ 3 s | test de integración |
| Importar DXF de 2 000 puntos (TOP-01) | ≤ 20 s con progreso | test E2E |
| Importar 100k objetos | ≤ 5 s | bench |
| Reconstrucción completa de topología, 100k | ≤ 300 ms | bench |
| Reconstrucción incremental típica | ≤ 2 ms | bench |
| Hidráulica, 10k tuberías (HID-01) | ≤ 1 s | bench + comparación EPANET |
| Volúmenes, 100k con secciones cada 20 m | ≤ 2 s | bench |
| Regenerar la memoria de cálculo | ≤ 3 s | bench |

**Cómo se dibuja 100k en Rune sin 100k `El`:** el plano no se construye con un elemento por objeto. Se dibuja en **pocas `canvas`/`El::shape` por baldosa y por nivel de detalle**, con geometría ya teselada en espacio local y `f32`, producida por un worker y cacheada por `(epoch de topología, cámara, LOD)`. El *picking* no usa el hit-testing de Rune: se hace analíticamente en `f64` contra el índice espacial, sobre el punto local que entrega el manejador del canvas. Consecuencias: el número de elementos de Rune es proporcional a las baldosas visibles, no a los objetos; y la superficie de dibujo va `.inert()` para que Rune no recorra su interior buscando blancos de clic.

**Techo de segmentos por frame** (~20k) con decimación por debajo de 1 px de separación; las capas estáticas (grilla, curvas de nivel, contorno) usan `.cache()` con `.cache_key()`.

### 9.4 Consecuencias

- **A favor:** la UI no se congela nunca por un cálculo; el progreso es visible; cancelar es posible; el resultado de un worker entra al modelo con undo.
- **A favor:** sin `async`, sin runtime de tareas. Hilos + canales + `Live`. Menos superficie, más determinismo, más fácil de depurar.
- **En contra:** hay que partir los cálculos en trabajos con puntos de control; una operación nueva que no sea un `Job` es una violación de arquitectura. Se acepta: es la disciplina que hace que la app se sienta rápida.
- **En contra:** duplicar memoria durante un cálculo (el `Arc` es compartido, pero los resultados parciales no). Se acota: los jobs declaran su presupuesto y se rechazan si exceden.

---

## 10. Estrategia de pruebas

### 10.1 Pirámide

| Nivel | Qué prueba | Herramienta | Dónde |
|---|---|---|---|
| Fórmula | cada ecuación contra un caso resuelto a mano (Manning, HW, DW, áreas medias, Helmert, TM) | `#[test]` con `approx` | `hydro`, `earth`, `crs`, `geom` |
| Propiedad | invariantes geométricas y de dominio | `proptest` | `geom`, `terrain`, `earth`, `topo` |
| Dorada de archivo | round-trip de DXF/GeoJSON/LandXML/XLSX | `insta` + fixtures | `io-*`, `report` |
| Snapshot de UI | la vista se dibuja y su árbol es estable | headless de Rune | `ui` |
| Extremo a extremo | la CLI ejecuta flujos completos y devuelve JSON válido | proceso real | `cli` |
| Arquitectura | las reglas de este documento | `cargo metadata` + grep dirigido | workspace |

### 10.2 Fórmulas

- Cada fórmula lleva **tres** casos: nominal, borde (división por cero, sección llena, pendiente nula) y fuera de rango (debe devolver `CalcError`, no `NaN`).
- Prohibido `NaN`/`inf` en una salida: todo resultado se valida con `is_finite()` antes de publicarse. Un `NaN` en un volumen es un expediente devuelto.
- Los casos de HID-01/HID-02 se versionan con entrada y salida esperada en el repo (diferencia ≤ 1% vs EPANET, ≤ 0,5% vs planilla de referencia), y se ejecutan en CI.

### 10.3 Propiedades (las que atrapan lo caro)

| Propiedad | Crate |
|---|---|
| `apply(inverso(apply(c))) == identidad` para **todo** comando registrado (generado por la lista del registro) | `cmd` |
| `station_of(point_at(s)) ≈ s` en una polilínea | `geom` |
| Interpolar el TIN en un vértice devuelve exactamente su cota | `terrain` |
| El volumen es monótono en profundidad y en ancho de zanja | `earth` |
| `Σ volúmenes por tramo == volumen total` (tolerancia 1e-9 relativa, con suma compensada) | `earth` |
| Un nodo nunca tiene dos puertos del mismo elemento con el mismo nombre | `topo` |
| La topología incremental es idéntica a la completa tras una secuencia aleatoria de ediciones | `topo` |
| Guardar → abrir conserva el `hash` del modelo (PRY-02) | `store` |

### 10.4 Topología: prueba diferencial

```rust
#[test]
fn incremental_igual_a_completa() {
    let mut doc = fixtures::proyecto_mediano();
    for op in generador_de_ediciones(doc.revision(), 500) {
        doc.apply(op).unwrap();
        let incremental = doc.topo.clone();
        let completa = Topology::rebuild(&doc);
        assert_eq!(incremental.nodes_canonicos(), completa.nodes_canonicos());
        assert_eq!(incremental.edges_canonicas(), completa.edges_canonicas());
    }
}
```

Esta es la prueba más importante del proyecto después de las de volumen: un error de reconstrucción incremental produce números plausibles y equivocados, que es el peor modo de falla posible.

### 10.5 Snapshot de UI con Rune headless

Rune permite ejecutar cualquier app sin ventana: render a PNG, volcado del árbol de elementos y entrada guionada sobre reloj virtual. De ahí el esquema:

- **Golden PNG** por escena (planta vacía, 3 pozos y 2 colectores, perfil, inspector, diálogo de error), con tolerancia perceptual y no igualdad de bytes.
- **Golden del árbol** (`--tree` en JSON) normalizado (IDs, tiempos) y comparado con `insta`: detecta cambios de estructura y de accesibilidad que el PNG no muestra.
- **Guiones de entrada** sobre el reloj virtual: deterministas, sin esperas reales. Es la única forma de probar interacción sin volverse frágil.
- Las escenas usan el **proyecto de ejemplo único** que define `05-product.md` §6.2, para que guía, pruebas y capturas hablen de lo mismo (y una captura vieja sea un defecto, no una molestia).

### 10.6 Pruebas de arquitectura (hacen cumplir este documento)

| Prueba | Qué falla si… |
|---|---|
| Grafo de dependencias | aparece un ciclo, o `core` depende de `store`, o un crate puro depende de `ui`/`store`/`io` |
| Pureza | un crate marcado puro usa `std::fs`, `std::net`, `std::process` o `PathBuf` |
| E/S en frame | el `IoGuard` detecta acceso a disco durante un frame (debug y test) |
| Paridad UI ↔ CLI | un comando registrado no aparece en `redes esquema`, o una capacidad del manifiesto no tiene comando (CLI-01) |
| Contrato MCP | el manifiesto de `redes mcp` difiere de `redes esquema` (CLI-06) |
| Unidades | una firma `pub` de `hydro`/`earth` usa `f64` desnudo en vez de un newtype |
| i18n | hay texto literal en una vista en vez de una clave (I18N-01) |
| Determinismo | dos ejecuciones del mismo cómputo dan `numeric_hash` distinto |

### 10.7 E2E de la CLI

- Cada comando del manifiesto tiene **al menos un ejemplo ejecutado en CI** (CLI-01). El ejemplo es literalmente el de la guía (DOC-04): si la guía miente, CI falla.
- Toda respuesta se valida contra su **JSON Schema** publicado (CLI-02).
- Se verifica: `--dry-run` no cambia el hash del archivo; repetir una creación con la misma clave natural no duplica; `expected_revision` obsoleto devuelve `exit 6`; los errores traen `codigo`/`mensaje`/`objeto`/`sugerencia` y nunca un *stacktrace* (CLI-05).
- **Paridad numérica:** un test corre el mismo cálculo por CLI y por el motor embebido de la UI y exige `numeric_hash` idéntico (criterio de salida de v0.1: diferencia 0).

### 10.8 Consecuencias

- **A favor:** los errores caros (undo que no deshace, incremental que diverge, UI que escribe en disco, CLI que se desvía de la UI) se detectan en CI.
- **En contra:** el `IoGuard` y las pruebas de arquitectura son código de prueba con acceso a detalles internos. Se confinan a `tests/` y a `#[cfg(debug_assertions)]`, sin costo en release.
- **En contra:** los snapshots de UI se vuelven ruidosos si el diseño cambia. Mitigación: tolerancia perceptual, escenas pequeñas y `insta` con revisión explícita.

---

## 11. Invariantes globales (lista de verificación para PR)

1. Ninguna mutación del modelo fuera de un `Command`.
2. Ningún `ElementId` reutilizado; ninguna identidad por puntero.
3. Ningún `HashMap` en una ruta que produzca salida ordenada o un hash.
4. Ninguna referencia entre elementos: solo IDs, validados.
5. Ninguna familia nueva toca el núcleo: manifiesto (+ opcionalmente un `impl` de `FamilyBehavior`).
6. Ningún cálculo en el hilo de UI; ninguna E/S en un frame.
7. Ningún `f64` desnudo en una frontera de módulo: newtype de unidad.
8. Ningún `==` sobre `f64`: tolerancia explícita del proyecto.
9. Todo resultado numérico lleva procedencia (`revision`, `topo_epoch`, `pack`, `numeric_hash`).
10. Todo comando nuevo trae su inverso, su `natural_key`, su esquema JSON y su prueba de ida y vuelta.
11. Toda cadena visible es una clave i18n.
12. La topología es siempre reconstruible desde el documento.

---

## 12. Los tres riesgos mayores

### R1 — Rune es pre-release y su aptitud como lienzo 2D no está verificada

**Por qué es grave.** El producto depende de que Rune aloje un lienzo con pan/zoom, edición de vértices, entrada numérica y tablas virtualizadas; `05-product.md` lo marca como supuesto **S2 no verificado** (el clon vendorizado no tiene fuentes). Además Rune está fijado a un *rev* de `dev`, no acepta PRs y su API se mueve. Si el lienzo no es viable, VIS-01 y AGU-01 pasan a riesgo alto y la demo de v0.1 se reduce a tablas y perfil.

**Mitigación.**

1. **Spike de 5 días en la semana 1, con criterio GO/NO-GO escrito**: dibujar 10k segmentos en un `canvas`, pan/zoom con `Live` a 60 fps, picking en f64 sobre clic, texto con atlas a 1:1000, y un PNG headless. Si falla, se decide en esa semana, no en la 20.
2. **Aislamiento total:** solo `redes-ui` conoce Rune. `State` no contiene tipos de Rune; la cámara y el picking son nuestros. Reescribir la capa de vistas es costoso, no catastrófico.
3. **El producto funciona sin la UI.** La CLI cubre el 100% de las capacidades (CLI-01). Si Rune se atasca, se entrega CLI + informes y la UI se retrasa, pero el producto no muere.
4. **Rev fijado y actualización como tarea planificada**, con la suite de snapshots headless como red de seguridad para detectar regresiones visuales al subir de rev.
5. **Plan B escrito:** si `El::shape`/`canvas` no rinde, la superficie del plano cae a un `NativeSurface` propio (o a un raster de baldosas) manteniendo el mismo `PlanRenderer` detrás de un rasgo.

### R2 — Las familias declaradas en runtime erosionan la seguridad de tipos y el aislamiento

**Por qué es grave.** EXT-01 exige cargar familias sin recompilar; eso significa que un manifiesto —posiblemente escrito por un tercero o por un agente— define atributos, unidades y reglas. Un manifiesto inválido puede corromper el proyecto, y una familia mal declarada puede producir números sin sentido (un diámetro en metros donde se esperan mm). EXT-04 añade plugins de terceros con firma y contención.

**Mitigación.**

1. **Validación de manifiesto contra JSON Schema propio**, con mensajes por campo, antes de activar nada. Una familia inválida se rechaza completa y **no afecta al resto** (EXT-01), con el proyecto abierto en solo lectura parcial si hacía falta.
2. **Unidades en el esquema, no en el nombre**: la unidad es un campo `AttrDef.unit` con tipo, y el acceso pasa por el newtype. Un manifiesto que declara `unit: "mm"` y un valor de 0,2 se lee como 0,2 mm, no como 0,2 m; la ambigüedad se elimina en el origen.
3. **Aislamiento de fallos:** `catch_unwind` en las fronteras de plugin, capacidades declaradas, y un plugin que falla queda deshabilitado con diagnóstico, sin cerrar la app ni bloquear el guardado.
4. **Suite de conformidad (EXT-03)** obligatoria para habilitar el cálculo de una familia; sin ella, la familia es "solo dibujo" y no entra en cómputos.
5. **Prueba de contrato del manifiesto propio**: los manifiestos de nuestras familias se comparan contra su `const` tipado, de modo que un error de tipeo no llega a producción.

### R3 — Topología y geometría producen números *plausibles* y equivocados

**Por qué es grave.** La razón de compra es el cómputo (EXC). Un volumen mal calculado no se ve: se ve en la obra, o en la observación del fiscalizador. Los tres modos de falla silenciosa son: (a) la reconstrucción incremental de topología diverge de la completa; (b) una conexión que "se adivina" por cercanía une dos pozos que no debían unirse; (c) un invert incoherente produce una pendiente negativa que el cálculo absorbe sin quejarse.

**Mitigación.**

1. **La topología es derivada y reconstruible**; en `debug` y `--strict` se compara incremental contra completa tras cada transacción (§10.4). Un fallo se ve en CI, no en el expediente.
2. **La conexión es un dato, nunca una deducción.** La proximidad genera un candidato de captura; si no hay `Connect`, la validación reporta "puerto sin conectar" y **bloquea el cómputo** (ALC-07 exige exactamente eso: bloquear cómputo e informes con errores críticos).
3. **Validación estructural y topológica como compuerta** (MOD-07, ALC-07): pozos sin salida, ciclos no permitidos, cotas invertidas, tramos sin descarga, diámetros incoherentes. Con severidad y con salto al objeto.
4. **Procedencia completa y `numeric_hash`** en cada resultado: revisión del documento, época de topología, versión del motor, hash del code pack, tolerancias. Un número sin procedencia no se imprime en la memoria (INF-01).
5. **Casos dorados contra referencias externas**: EPANET para agua a presión, planilla del piloto para alcantarillado, cálculo manual para 3 tramos de excavación. Si el número se mueve, se mueve contra una referencia, no contra una corazonada.

**Riesgos secundarios (vigilados, no principales):** el formato de chunks y sus migraciones para siempre (mitigado con fixtures por versión y `.bak`); la curva de aprendizaje de Rune para el equipo (mitigada con el spike y con `examples/`); el rendimiento de la memoria de cálculo en PDF con muchas tablas (mitigado generando en worker).

---

## 13. Anexo: orden de implementación

| Hito (según `05-product.md` §4) | Qué se construye de este ADR | Prueba que lo cierra |
|---|---|---|
| **v0.1** (sem. 1–8) | `units`, `geom`, `crs`, `terrain`, `earth`, `core`, `topo` mínimo, `cmd` mínimo, `store` (SQLite + migración 0), `io-dxf`/`-csv` de entrada, `report` (perfil PDF), `api`+`cli` con 6 comandos, `ui` con tablas + perfil | cadena topografía→pozos→colector→excavación→perfil dos veces con el mismo resultado; CLI y UI con volumen idéntico; archivo reabre sin pérdida |
| **v0.2** (9–18) | `hydro`, `calc` con code packs, `topo` completo + validación, `io-dxf` de salida, `report` (memoria, planos), `ui` con planta (VIS-01), `api` con manifiesto completo | veredicto reproducible con el mismo hash en 2 ejecuciones y en CLI; DXF abre sin reparar en 2 CAD; cuadre de tierras a 0,1% |
| **v0.3** (19–30) | `hydro` a presión (GGA), familias en manifiesto + suite de conformidad, `api` con MCP y modo lote, `cmd` con idempotencia completa, `store` con bloqueo y respaldos | agente completa el flujo sin humano; familia nueva instalada sin reiniciar; 100% del manifiesto con ejemplo en CI |
| **v1.0** (31–40) | endurecimiento: 5 000 objetos, presupuesto de frame, `IoGuard` en CI, snapshots de UI, instalador, migración de pilotos | 3 proyectos reales entregados; 0 defectos críticos |

**El spike de Rune (R1) es la primera tarea de la semana 1 y bloquea `ui`, no el resto de la cadena.** La CLI puede avanzar en paralelo porque no depende de Rune para nada.

---

## 14. Concordancias y divergencias con los documentos hermanos

Este documento se escribió contra `01-domain.md`, `03-cli-ai.md` y `05-product.md`. Lo que sigue es el estado de cada punto de contacto, para que el consejo no publique dos verdades.

| # | Punto | Estado | Detalle |
|---|---|---|---|
| C1 | Unidades de diámetro | ✅ **Resuelto aquí** | Dos regímenes (nominal en mm, geométrico en m) con newtypes distintos, §5.2 regla 4. Concuerda con `01-domain.md` §4. |
| C2 | Journal de revisiones y deshacer | ✅ **Resuelto aquí** | `03-cli-ai.md` lo hace persistente y append-only; se adopta en §6.1 y §7.1. Deshacer añade una revisión inversa, no borra historia. Los 50 pasos de PRY-05 pasan a ser la ventana de la UI. |
| C3 | "Un solo archivo" vs. directorio lateral | ✅ **Resuelto aquí** | El modelo canónico vive íntegro en el `.redes` (PER-01). Un sidecar opcional solo aloja journal caliente, caché y claves de idempotencia, y es **borrable sin pérdida**: si se elimina, el proyecto sigue siendo válido (se pierde como mucho el historial fino). |
| C4 | `Action` de `03-cli-ai.md` vs. `Command` de este ADR | ✅ **Concuerdan** | `mutates`, `destructive`, `idempotent`, `dry_run`, `input_schema`, `output_schema`, `examples`, `error_codes` son metadatos del mismo registro; `redes-api` los publica a partir de `Command::name/natural_key/validate/encode` y de los esquemas generados. No hay dos mecanismos, hay dos nombres. |
| C5 | Comandos de descubrimiento sin abrir el proyecto | ✅ **Concuerda** | El manifiesto se genera del registro en memoria (CLI-03), así que `redes esquema` responde sin tocar el archivo. Es una prueba de arquitectura (§10.6). |
| C6 | **Prefijo del binario y extensión del proyecto** | ⚠️ **Abierto — decide el lead** | `05-product.md` y este documento dicen `redes` / `.redes`; `03-cli-ai.md` dice `mamut` / `project.mamut` / `mamut.toml` / `MAMUT_*`. Debe quedar **uno solo**, porque aparece en cada ejemplo de la guía (DOC-04) y en el manifiesto. Recomendación: `redes`, por coherencia con el nombre del producto ("Rune Redes") y con el prefijo de los 18 crates. Si se elige `mamut`, el cambio es mecánico (prefijo y extensión), no toca ninguna decisión de arquitectura. |
| C7 | **Idioma de los identificadores de acción** | ⚠️ **Abierto — decide el lead** | `03-cli-ai.md` usa acciones en inglés (`manhole.create`); `01-domain.md` y `05-product.md` usan el vocabulario en español (`diametro_interior`, `altura_caida`) y I18N-04 prohíbe sinónimos. Recomendación: **una sola lengua para los identificadores** (no para las etiquetas, que ya son claves i18n) y, si se mezcla, una tabla de alias publicada en el manifiesto. Mi voto: español para familias y atributos (vienen del dominio y del glosario) e inglés solo si se decide que la API es un producto aparte. Lo que no es aceptable es que un agente tenga que adivinar si el pozo es `pozo` o `manhole`. |
| C8 | Nombres de crate | ✅ **Concuerdan** | El prefijo `redes-` es el mismo que el del producto; si C6 cambia el prefijo, los crates se renombran sin tocar dependencias lógicas. |
