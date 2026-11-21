# 07 — Geometría paramétrica de los objetos de red

| Campo | Valor |
|---|---|
| Documento | `docs/consejo/07-geometria-parametrica.md` |
| Autor | `geometria` — consejo de arquitectura |
| Versión | 0.1 (borrador para decisión) |
| Fecha | 2026-11-18 |
| Estado | Propuesta técnica. 4 ADR abiertos (§11.1) y 6 riesgos declarados (§11.2) |
| Audiencia | implementadores Rust de `redes-core`, revisor de UI, cliente |
| Alcance | Sólidos, ensamblajes, LOD, orden del pintor, sombreado, colocación, caché y pruebas de la vista 3D |
| Fuera de alcance | Cámara y proyección (ya existen en `view.rs`), ingesta, cálculo hidráulico, exportación DXF/DWG |

**Cómo leer este documento.** §1 fija el punto de partida medido, no el supuesto. §2 define la
representación intermedia. §3 y §4 son el catálogo: primitivas y ensamblajes con sus parámetros
derivados. §5 pone el presupuesto y las reglas de LOD. §6–§8 resuelven los tres problemas del
renderer propio (orden, luz, colocación). §9–§10 cierran con caché y pruebas. §11 recoge lo que
hay que decidir y lo que puede salir mal.

**Convención numérica.** Punto decimal en fórmulas, tablas y nombres de parámetro. Coma en prosa.
Longitudes en metros, ángulos en radianes salvo que se diga «°». Los miles llevan espacio fino
solo en prosa; nunca en un valor que se compare o se persista.

**Contenido**

0. Resumen: las diez decisiones estructurantes
1. Punto de partida medido
2. Modelo de sólidos: la representación intermedia
3. Primitivas paramétricas
4. Ensamblajes por familia
5. Presupuesto de triángulos y reglas de LOD
6. Orden del pintor con sólidos
7. Sombreado y material sin texturas
8. Colocación, orientación y «floating»
9. Rendimiento: qué se cachea y con qué clave
10. Pruebas sin mirar la pantalla
11. Decisiones abiertas, riesgos y fuera de alcance

---

## 0. Resumen: las diez decisiones estructurantes

| # | Decisión | Consecuencia |
|---|---|---|
| G1 | La geometría se genera como **mallas de caras planas** (polígonos de 3 a 12 vértices con normal y material), en coordenadas locales del objeto y con el eje `+Z` hacia arriba. | Una sola representación sirve para pozo, válvula y tubo; el renderer sigue emitiendo `Prim::Poly`. |
| G2 | El ensamblaje se describe como **lista de primitivas con parámetros derivados de los atributos** del modelo (`D`, `H`, `DN`, `e`). Nunca hay medidas escritas a mano por familia. | Cambiar `diametro_interior` cambia el pozo entero, coherente. Se puede probar con invariantes. |
| G3 | Cada ensamblaje define **tres LOD** por tamaño en píxeles: símbolo (≤6 px), cilindro (≤25 px), completo (>25 px). El LOD es un parámetro de la función, no un estado global. | El mismo código sirve para la vista de ciudad y para el detalle de inspección. |
| G4 | **Backface culling por normal**: no se emite una cara que mira away de la cámara. | Un sólido cerrado baja ~45 % de primitivas y desaparecen la mitad de los errores de pintor. |
| G5 | Orden del pintor por **profundidad máxima de la cara** (no la media), con desempate por capa local (pared &lt; tapa). Las caras que superan ~60 px se **subdividen**. | Los artefactos se reducen a un conjunto acotado y declarado (§6.4). |
| G6 | Sombreado **sin texturas**: normal + luz direccional + ambiente, con el Lambert cuantizado en **bandas** y un término especular sólo para metal. | Hormigón y tapa metálica se distinguen de un vistazo, con 6 números por material. |
| G7 | La colocación es `(planta, cota de solera, giro de planta)`; el tubo se orienta con un **marco ortonormal** derivado de sus dos extremos. | No hay matrices 4×4 ni cuaterniones en el núcleo: tres vectores `f64` por arco. |
| G8 | El pozo **nunca flota**: la tapa se sube al máximo del terreno en su anillo y, si la ladera cae más de 0.25 m, se añade un **faldón** hasta 0.20 m por debajo del punto bajo. | Se elimina el defecto más visible en la demo (pozos colgando en la ladera). |
| G9 | Caché en dos niveles: **malla local** por clave paramétrica cuantizada, **malla en mundo** por `(ElementId, revision, lod)`. | Reconstruir en órbita es cero; sólo se proyecta y se ordena. |
| G10 | El presupuesto de sólidos **se paga con el terreno**, no con el frame: cada primitiva nueva se compensa aligerando el TIN durante la órbita. | Se puede multiplicar el detalle de la red sin perder 60 fps. |

---

## 1. Punto de partida medido

Todo lo de esta sección se midió sobre el repositorio, no se estimó. Comando y resultado:

```
redes proyecto ciudad --salida <tmp>/ciudad.redes     → 87 objetos
redes vista --project <tmp>/ciudad.redes --salida <tmp>/v.svg
  → primitivas: 8787   triangulos: 8746   lineas: 41
```

Descomposición contando los `<polygon>` del SVG por color:

| Grupo | Primitivas | % |
|---|---:|---:|
| Terreno (TIN) | 7 986 | 90.9 |
| Manzanas (muros + cubiertas) | 512 | 5.8 |
| Calles y aceras | 202 | 2.3 |
| Red de alcantarillado (pozos, cámaras) | 42 | 0.5 |
| Red de agua (hidrantes) | 4 | 0.0 |
| Líneas (ejes de tubería) | 41 | 0.5 |
| **Total** | **8 787** | **100** |

**Hallazgo 1 — las 8 700 primitivas no son pozos: son el terreno.** La red completa aporta 87
primitivas, el 1.0 %. Detallar los 35 pozos a 118 caras cada uno añade ~4 130 primitivas: la red
pasa de 87 a ~4 200 (×48) y la escena sólo crece un 47 %.

**Hallazgo 2 — el coste por primitiva.** Comparando mínimos de `redes vista` entre la ciudad
(8 787 primitivas, 11 ms) y el demo (4 646 primitivas, 7 ms) en release: 4 141 primitivas ≈ 4 ms,
es decir **≈1 µs por primitiva sólo en `view::build`** (proyección + orden + `expand`). El frame
declarado por el cliente, 25 ms en release sobre ~8 700 primitivas, implica **≈2.9 µs por
primitiva contando Rune** (triangulación por ear-clipping y dibujo). De ahí:

| Objetivo | Presupuesto de primitivas por frame |
|---|---:|
| 30 fps (33 ms) | ≈ 11 000 |
| **60 fps (16.7 ms)** | **≈ 5 800** |
| 120 fps (8.3 ms) | ≈ 2 900 |

**Hallazgo 3 — el terreno ya se aligera, pero tarde.** `crates/redes-app/src/main.rs:63` construye
`t.thinned((diag / 46.0).max(2.0))` ≈ celda de 25 m para la ciudad, pero `redes vista` usa el TIN
completo. El TIN crudo tiene 8 700 triángulos en 4 484 puntos; con celda de 25 m baja a ~2 000.
Ese es el presupuesto que se puede gastar en sólidos: **7 986 − 2 000 ≈ 6 000 primitivas libres**.

**Hallazgo 4 — Rune triangula cada `fill` en CPU.** `vendor/rune/crates/rune/src/shape.rs:339`
hace *ear clipping* con búsqueda de oreja O(n²) y, si el polígono no es simple, cae a un abanico
que dibuja mal. Reglas duras que salen de ahí y que este documento asume:

1. Ninguna cara con más de **12 vértices**.
2. Ninguna cara **auto-intersectada** ni con huecos: un anillo se emite como N quads, no como
   polígono con agujero.
3. Preferir **triángulos y quads convexos**: el ear-clipping de un quad es trivial.

---

## 2. Modelo de sólidos: la representación intermedia

### 2.1 Tipos

`crates/redes-core/src/solido.rs`, sin dependencias nuevas (el núcleo sólo usa `serde` hoy y no
se rompe esa regla):

```rust
/// Material: índice a una tabla constante de color/rugosidad/brillo.
pub type Mat = u8;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Aabb { pub min: [f32; 3], pub max: [f32; 3] }

/// Una cara plana. `base`/`n` indexan `Malla::verts` sin indirección: la cara es un rango.
#[derive(Clone, Copy, Debug)]
pub struct Cara {
    pub base: u32,       // primer vértice en Malla::verts
    pub n: u8,           // 3..=12
    pub normal: [f32; 3],// unitaria, espacio local
    pub mat: Mat,
    pub flags: u8,       // CARA_TAPA | CARA_DOBLE | CARA_OCULTA
}

#[derive(Clone, Debug, Default)]
pub struct Malla {
    pub verts: Vec<[f32; 3]>, // locales, metros, +Z arriba, origen en el eje
    pub caras: Vec<Cara>,
    pub aabb: Aabb,
    pub tris: u32,            // (n-2) sumado; se valida contra el presupuesto
}

pub const CARA_TAPA: u8   = 1 << 0; // tapa/arriba: orden local posterior
pub const CARA_DOBLE: u8  = 1 << 1; // visible por ambas caras (chapa, prisma sin tapas)
pub const CARA_OCULTA: u8 = 1 << 2; // apoyo contra el terreno: no se emite nunca
```

**Por qué una lista de caras y no de triángulos.** Una pared de cilindro de 12 caras son 12
primitivas si se emite por quad y 24 si se emite por triángulo: el doble de `fill`, el doble de
ear-clipping, el doble de ordenación, y una costura de un píxel entre los dos triángulos de cada
quad (el mismo motivo por el que `view::build` ya llama a `expand()`). Se guardan **caras**, y
`Malla::tris` cuenta los triángulos que costará en GPU para poder presupuestar. La regla
`tris = Σ (n − 2)` es la que usan las tablas de §3–§5.

**Por qué vértices en `f32`.** `view::build` resta el origen del proyecto en `f64` y pasa a `f32`
(`crates/redes-core/src/view.rs:212`): a 760 000 m de coordenada absoluta, `f32` resuelve ~6 cm.
La malla local está a menos de 3 m del origen del objeto, así que `f32` es exacto de sobra y ahorra
la mitad de memoria. La transformación a mundo se hace **en `f64`** y se resta el origen antes de
proyectar.

### 2.2 Constructor

Un único acumulador evita duplicar la lógica de «añadir cara, actualizar AABB, contar triángulos»:

```rust
pub struct Constructor<'a> { m: &'a mut Malla, mat: Mat, pub caras_anillo: u8 }

impl<'a> Constructor<'a> {
    pub fn new(m: &'a mut Malla, mat: Mat, caras_anillo: u8) -> Self;
    pub fn con_material(&mut self, mat: Mat) -> Constructor<'_>;   // sub-constructor
    pub fn cara(&mut self, pts: &[[f32; 3]], flags: u8);
    pub fn cara_con_normal(&mut self, pts: &[[f32; 3]], n: [f32; 3], flags: u8);
    /// Anillo horizontal (tapa, brida, aro): `n` caras entre `r0` y `r1` a cota `z`.
    pub fn aro(&mut self, z: f32, r0: f32, r1: f32, n: [f32; 3], flags: u8);
    /// Pared vertical barrida: `n` caras de `z0` a `z1` con radio variable.
    pub fn barrido(&mut self, z0: f32, z1: f32, r0: f32, r1: f32, flags: u8);
    pub fn cerrar(&mut self) -> u32;  // recalcula aabb y tris, devuelve tris
}
```

`Constructor` normaliza la orientación: si el polígono queda con normal hacia adentro respecto al
eje local, lo invierte y avisa en `debug_assert!`. Esto es lo que hace que las pruebas de §10
puedan confiar en las normales.

### 2.3 Tabla de materiales (constante, no dato)

`const MATS: [Material; 10]`, con `Material { base: [f32; 3], bandas: u8, brillo: f32, trama: u8 }`.
Ver §7.3 para los valores. Los materiales son parte del render, no del documento `.redes`: no se
persisten ni se editan.

---

## 3. Primitivas paramétricas

Todas cuelgan de `Constructor`, reciben dimensiones **en metros ya derivadas** por el ensamblaje y
no consultan el modelo. `caras` es siempre el del anillo; `Tapas` dice qué discos se emiten.

```rust
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tapas { Ninguna, Inferior, Superior, Ambas }
```

### 3.1 Firmas

```rust
/// Cilindro recto de eje +Z, de `z0` a `z1`.
pub fn cilindro(c: &mut Constructor, z0: f32, z1: f32, r: f32, tapas: Tapas);

/// Cono truncado (o completo si `r1 == 0.0`): el cono de reducción de un pozo.
pub fn cono(c: &mut Constructor, z0: f32, z1: f32, r0: f32, r1: f32, tapas: Tapas);

/// Caja alineada, con giro opcional en planta (marco de tapa, cuerpo de medidor).
pub fn caja(c: &mut Constructor, centro: [f32; 3], dims: [f32; 3], giro_z: f32);

/// Cilindro hueco: pared exterior + pared interior + aro superior. Cuerpo de pozo.
pub fn tubo(c: &mut Constructor, z0: f32, z1: f32, r_int: f32, r_ext: f32);

/// Anillo plano (brida, aro de marco, borde de tapa). `n` = normal del plano.
pub fn anillo(c: &mut Constructor, z: f32, r_int: f32, r_ext: f32, n: [f32; 3]);

/// Disco plano (tapa, solera, boca de acometida).
pub fn disco(c: &mut Constructor, z: f32, r: f32, n: [f32; 3]);

/// Prisma extruido desde un polígono convexo en planta (cámaras, cajones, cuñas).
pub fn prisma(c: &mut Constructor, poligono: &[[f32; 2]], z0: f32, z1: f32, tapas: Tapas);

/// Esfera de pocos polos (cúpula de tanque, bomba, enrocado). `meridianos` × `franjas`.
pub fn esfera_baja(c: &mut Constructor, centro: [f32; 3], r: f32, meridianos: u8, franjas: u8);

/// Escalera de peldaños empotrados: barras horizontales cada `paso`, de `ancho` útil.
pub fn escalera(c: &mut Constructor, z0: f32, z1: f32, r_camara: f32, paso: f32, ancho: f32);

/// Brida: disco grueso con dos aros. `r_brida - r_tubo` es el ancho del aro.
pub fn brida(c: &mut Constructor, z: f32, r_tubo: f32, r_brida: f32, espesor: f32);

/// Volante de válvula: aro + radios + cubo. `radios` = 3 en LOD0, 0 = sólo aro.
pub fn volante(c: &mut Constructor, z: f32, r: f32, r_aro: f32, radios: u8);
```

### 3.2 Coste por primitiva

`c` = caras del anillo. Un quad cuesta 2 triángulos, una tapa de `c` vértices cuesta `c − 2`.

| Primitiva | Parámetros de coste | Caras | Triángulos |
|---|---|---:|---:|
| `cilindro` sin tapas | `c = 12` | 12 | 24 |
| `cilindro` con una tapa | `c = 12` | 13 | 34 |
| `cono` truncado sin tapas | `c = 12` | 12 | 24 |
| `caja` | — | 6 | 12 |
| `caja` sin tapas (barrote) | — | 4 | 8 |
| `tubo` | `c = 12` | 36 | 72 |
| `anillo` | `c = 12` | 12 | 24 |
| `disco` | `c = 12` | 1 | 10 |
| `prisma` de 4 lados, con tapas | — | 6 | 12 |
| `prisma` de 4 lados, sin tapas | — | 4 | 8 |
| `esfera_baja` | `m = 8`, `f = 3` | 24 | 32 |
| `escalera` de 5 peldaños | `c = 4` por barrote | 20 | 40 |
| `brida` | `c = 8` | 9 | 22 |
| `volante` con 3 radios | `c = 10` | 22 | 44 |

Los valores de LOD1 salen de bajar `c` a 8 y quitar tapas ocultas: `tubo` a 8 caras = 24 caras /
48 triángulos; `cilindro` a 8 = 8 caras / 16 triángulos; `disco` a 8 = 1 cara / 6 triángulos.

---

## 4. Ensamblajes por familia

### 4.1 Parámetros derivados comunes

Con los campos que ya existen en `Node` (`z_rim`, `z_terrain`, `z_invert`) y
`Node::diametro()` (`crates/redes-core/src/model.rs:149`):

| Símbolo | Fórmula | Uso |
|---|---|---|
| `D` | `n.diametro()` | diámetro interior del pozo/cámara |
| `e` | `attr("espesor_pared", 0.15)` | espesor de pared |
| `H` | `(n.z_rim − n.z_invert).max(0.30)` | altura total del pozo |
| `Hc` | `(0.30 * H).clamp(0.35, 0.90)` | altura del cono de reducción |
| `rim_r` | `0.30 + 0.05 * (D / 1.20)` | radio libre del marco |
| `t` | `0.12` | espesor de tapa y marco |
| `c` | `if lod == Completo { 12 } else { 8 }` | caras del anillo |

`H`, `D` y `e` son **los únicos** números que entran al ensamblaje desde el modelo; el resto son
relaciones geométricas fijas del documento. Así una prueba puede recorrer los 35 pozos de la demo
y comprobar el mismo invariante en todos.

### 4.2 F01 — Pozos (`saneamiento.pozo`)

**Pozo de inspección, LOD0.** Origen local en el eje, `z = 0` en la solera, `z = H` en la tapa.

| # | Pieza | Primitiva | Parámetros derivados | Caras | Tri |
|---|---|---|---|---:|---:|
| 1 | Solera | `disco` | `z = 0`, `r = D/2` | 1 | 10 |
| 2 | Banqueo (media caña) | 2 × `prisma` (3 lados, sin tapas) | `H_banq = 0.10`, ancho `D/2` | 6 | 12 |
| 3 | Pared exterior | `tubo` (paredes) | `r_ext = D/2 + e`, `z 0…H−Hc−t` | 12 | 24 |
| 4 | Pared interior | `tubo` (paredes) | `r_int = D/2`, mismo tramo | 12 | 24 |
| 5 | Aro cuerpo/cono | `anillo` | `z = H−Hc−t`, `D/2 … D/2+e` | 12 | 24 |
| 6 | Cono de reducción | `cono` | `r1 = D/2`, `r2 = rim_r`, `h = Hc` | 12 | 24 |
| 7 | Aro cono/marco | `anillo` | `z = H−t`, `rim_r … rim_r+0.06` | 12 | 24 |
| 8 | Marco | `anillo(8)` + `cilindro(8)` de asiento | `r = rim_r`, alto `t` | 16 | 32 |
| 9 | Tapa | `disco` + `anillo` | `z = H`, `r = rim_r−0.01` | 13 | 34 |
| 10 | Escalera | `escalera` | `paso = 0.30`, `ancho = 0.40`, 5 peldaños | 20 | 40 |
| 11 | Bocas de acometida | 2 × `disco` | `r = DN/2`, orientadas al arco | 2 | 12 |
| | **Total** | | | **118** | **260** |

> **`cono: r1 = D/2, r2 = 0.3, h = min(0.6, H*0.3)`.** El marco y la tapa son siempre de
> `rim_r ≈ 0.30–0.35 m`, independientemente de `D`: es lo que hace que un pozo de 1.50 m y uno de
> 0.90 m se lean distintos sin cambiar la tapa.

**Variantes** (delta sobre el pozo de inspección):

| Subtipo | Delta de ensamblaje | Δcaras | Δtri |
|---|---|---:|---:|
| `caida` | Tubo de caída vertical `cilindro(8)` junto a la pared + placa de impacto `caja` + aro de fondo; se anula la boca de entrada alta | +22 | +44 |
| `resalto` | Cuña de resalto `prisma(3)` en la solera + escalón `caja` | +10 | +22 |
| `ciego` | −1 boca de salida; + `disco` de grava filtrante a `z = 0.05` con material distinto | −1 | +4 |
| `bombeo` | + 2 bombas (`cilindro(8)` + `esfera_baja(6,2)`), + barandilla `escalera` de 4, + tubo de impulsión `cilindro(6)` | +52 | +110 |
| `limpieza` | Sustituye `tubo` circular por `prisma(4)` con paredes: 4 caras ext + 4 int + 4 aro + solera `prisma(4)` + losa + tapa `caja`; sin cono, sin escalera | 28 | 56 |
| `ventilacion` | Pozo de inspección sin bocas y con `cilindro(6)` de venteo sobre la tapa | +6 | +12 |

**LOD1 (≤25 px).** `cilindro(8)` macizo del fondo a `H−Hc` + `cono(8)` + `disco(8)` de tapa:
**17 caras / 38 triángulos**. **LOD2 (≤6 px).** Un rombo de 1 cara / 2 triángulos, exactamente el
que ya usa `view.rs:533` cuando `r_px < 3.0`; se mantiene y se sube el umbral a 6 px.

### 4.3 F02 — Tuberías (`saneamiento.tuberia`, `agua.tuberia`)

El eje sigue siendo una `Shape::line` por segmento en LOD1 (hoy es el 100 % del dibujo de tubos y
funciona). El LOD0 sólo aparece cuando el tubo supera 25 px de ancho en pantalla, es decir en
primer plano.

| Pieza | Primitiva | Parámetros derivados | Caras | Tri |
|---|---|---|---:|---:|
| Tramo de tubo | `cilindro(8)`, sin tapas | `r = D_ext/2`, `L = 6.00` | 8 | 16 |
| Campana (junta) | `cono(8)` | `r1 = D_ext/2`, `r2 = D_ext/2 + 0.06`, `h = 0.12` | 8 | 16 |
| Junta elástica | `anillo(8)` | 1 mm por delante del tubo | 1 | 6 |
| **Por tramo de 6 m** | | | **17** | **38** |

**Regla de juntas.** `n = floor(L / 6)` campanas, en `t_k = (k · 6) / L` desde el nodo de aguas
arriba; el resto hasta el nodo es el último tramo. Con `L ≤ 6.5 m` el arco es **un solo tramo sin
campana interior** (las campanas de los extremos las pone el pozo). Se dibujan como máximo **6
campanas** por arco; a partir de ahí el arco pasa a LOD1 aunque esté cerca, porque a esa distancia
el ojo ya no distingue la junta de una raya.

`D_ext = DN/1000 + 2·e_tubo`, con `e_tubo` del atributo `espesor_pared` (por defecto 0.008 m).

### 4.4 F04 — Válvulas (`agua.valvula`)

Cuerpo común: `cilindro(8)` con el eje del tubo (girado por el marco de §8.2), dos `brida(8)`, y
el accionamiento encima.

| Subtipo | Ensamblaje | Caras | Tri |
|---|---|---:|---:|
| `compuerta` | cuerpo 8 + 2 bridas 18 + bonete `cilindro(8)` 8 + `cono(8)` 8 + vástago `cilindro(6)` 6 + `volante` 22 + prensaestopas `anillo(8)` 8 | 78 | 164 |
| `mariposa` | cuerpo corto 8 + 2 bridas 18 + actuador `caja` 6 + palanca `caja` 6 | 38 | 84 |
| `retencion` | cuerpo 8 + 2 bridas 18 + tapa de registro `disco(8)`+`anillo(8)` 9 | 35 | 82 |
| `aire` | cuerpo `cilindro(8)` vertical 8 + cámara `esfera_baja(6,2)` 12 + 2 bocas `cilindro(6)` 12 + flotador `disco(6)` 1 | 33 | 56 |
| `purga` | cuerpo 8 + 2 bridas 18 + válvula pequeña `caja` 6 | 32 | 72 |
| `reduccion_presion` | cuerpo 8 + 2 bridas 18 + 2 diafragmas `cilindro(8)` 8 + piloto `caja` 6 + tubo de pilotaje `cilindro(4)` 4 | 44 | 96 |
| `reguladora` | igual que RP sin piloto | 34 | 76 |
| `alivio` | cuerpo 8 + 2 bridas 18 + resorte `cilindro(6)` 6 + capuchón `cono(6)` 6 | 38 | 84 |
| `pie` | cuerpo 8 + brida 9 + colador `caja` 6 + aletas `caja` ×2 12 | 35 | 74 |

**LOD1.** Sólo cuerpo `cilindro(8)` + un `disco(8)` de volante encima: **9 caras / 22 triángulos**
para todas las válvulas, con el color del material y una raya de brida implícita en el color.
**LOD2.** Un cuadrado de 1 cara.

> El `volante` es la pieza que más se apaga: 44 de los 164 triángulos de una compuerta están en el
> volante. A 25–40 px nadie lo ve; a partir de 60 px es lo que hace legible una válvula.

### 4.5 F05 — Hidrante (`agua.llave`, `ty = hidrante`)

| Pieza | Primitiva | Parámetros | Caras | Tri |
|---|---|---|---:|---:|
| Cuerpo | `cilindro(8)` | `r = 0.075`, `h = 0.75` sobre rasante | 8 | 16 |
| Brida base | `brida(8)` | `r = 0.11` | 9 | 22 |
| Bocas (2) | `cilindro(6)` horizontales | `r = 0.030`, `l = 0.09` | 12 | 24 |
| Bonete + tuerca | `cono(6)` + `caja` | `r1 = 0.075`, `r2 = 0.05` | 8 | 16 |
| Vástago | `cilindro(6)` | `r = 0.012` | 6 | 12 |
| **Total** | | | **43** | **90** |

**LOD1:** `cilindro(8)` + `disco(8)` = 9 caras / 22 triángulos. **LOD2:** cuadrado.

### 4.6 F06 — Tanque (`agua.tanque`)

| Pieza | Primitiva | Parámetros | Caras | Tri |
|---|---|---|---:|---:|
| Fuste | `tubo(12)` | `r = D/2`, `e = 0.25`, `z 0…H_fuste` | 36 | 72 |
| Losa de fondo | `cilindro(12)`, tapa superior | `r = D/2 + 0.30` | 13 | 34 |
| Cubierta | `cono(12)` | `r1 = D/2+0.30`, `r2 = 0.30`, `h = 0.20·D` | 12 | 24 |
| Aro de cubierta | `anillo(12)` | `rim` de la cubierta | 12 | 24 |
| Escalera exterior | `escalera` 10 peldaños | `paso = 0.30` | 40 | 80 |
| Respiradero | `cilindro(6)` + `cono(6)` | `r = 0.08` | 12 | 24 |
| **Total** | | | **125** | **258** |

**LOD1:** `cilindro(12)` con tapa + `cono(12)`: 25 caras / 58 triángulos. **LOD2:** un disco.

### 4.7 F03 — Descarga / vertedero (`saneamiento.descarga`)

| Pieza | Primitiva | Caras | Tri |
|---|---|---:|---:|
| Muro de entrega | `caja` | 6 | 12 |
| Losa de fondo | `prisma(4)` | 6 | 12 |
| Aletas | 2 × `caja` | 12 | 24 |
| Enrocado | 6 × `caja` girada (no esferas: 12 tri cada una contra 32) | 36 | 72 |
| **Total** | | **60** | **120** |

El enrocado es el único sitio donde una `caja` girada sustituye a una `esfera_baja` a propósito:
6 cajas (72 triángulos) hacen el mismo papel visual que 6 esferas (192) y ordenan mejor.

### 4.8 F08 — Medidor (`agua.medidor`)

| Pieza | Primitiva | Caras | Tri |
|---|---|---:|---:|
| Cuerpo | `cilindro(8)` sobre el eje | 8 | 16 |
| Bridas (2) | `brida(8)` | 18 | 44 |
| Esfera/carátula | `disco(8)` + `anillo(8)` | 9 | 22 |
| Tapa de registro | `caja` | 6 | 12 |
| **Total** | | **41** | **94** |

### 4.9 F09 — Cámara (`saneamiento.camara`)

| Pieza | Primitiva | Caras | Tri |
|---|---|---:|---:|
| Paredes (hueco) | 4 `caja` ext + 4 int + aro de 4 | 12 | 24 |
| Solera | `prisma(4)` | 6 | 12 |
| Losa con hueco | 4 `caja` | 4 | 8 |
| Marco y tapa | 2 `caja` | 12 | 24 |
| Escalera | `escalera` 5 | 20 | 40 |
| Bocas (2) | `disco(8)` | 2 | 12 |
| **Total** | | **56** | **120** |

Se prefieren cajas a cilindros porque la cámara real es rectangular y porque cuatro quads son 8
triángulos contra 24 de un anillo de 12.

### 4.10 F13 — Sumidero (`saneamiento.sumidero`)

| Pieza | Primitiva | Caras | Tri |
|---|---|---:|---:|
| Cajón | 4 `caja` + solera | 10 | 20 |
| Marco | 4 `caja` | 4 | 8 |
| Rejilla | 5 barrotes `caja` sin tapas | 20 | 40 |
| Sifón | `cilindro(6)` | 6 | 12 |
| **Total** | | **40** | **80** |

La rejilla (20 caras) es el 50 % del sumidero; se apaga la segunda, detrás de los barrotes y el
marco.

---

## 5. Presupuesto de triángulos y reglas de LOD

### 5.1 Presupuesto por objeto

Cifras objetivo, **verificadas por prueba** (§10.1), no medidas. Formato `caras / triángulos`.

| Familia | LOD2 (≤6 px) | LOD1 (≤25 px) | LOD0 (>25 px) |
|---|---:|---:|---:|
| Pozo de inspección | 1 / 2 | 17 / 38 | 118 / 260 |
| Pozo de caída | 1 / 2 | 21 / 50 | 140 / 304 |
| Pozo de resalto | 1 / 2 | 17 / 38 | 128 / 282 |
| Pozo ciego | 1 / 2 | 16 / 36 | 117 / 264 |
| Pozo de bombeo | 1 / 2 | 26 / 60 | 170 / 370 |
| Pozo de limpieza | 1 / 2 | 7 / 14 | 28 / 56 |
| Tubería (por tramo de 6 m) | línea | línea | 17 / 38 |
| Válvula compuerta | 1 / 2 | 9 / 22 | 78 / 164 |
| Válvula mariposa | 1 / 2 | 9 / 22 | 38 / 84 |
| Válvula retención | 1 / 2 | 9 / 22 | 35 / 82 |
| Válvula de aire | 1 / 2 | 9 / 22 | 33 / 56 |
| Válvula de purga | 1 / 2 | 9 / 22 | 32 / 72 |
| Válvula RP | 1 / 2 | 9 / 22 | 44 / 96 |
| Hidrante | 1 / 2 | 9 / 22 | 43 / 90 |
| Tanque | 1 / 2 | 25 / 58 | 125 / 258 |
| Descarga / vertedero | 1 / 2 | 18 / 36 | 60 / 120 |
| Medidor | 1 / 2 | 9 / 22 | 41 / 94 |
| Cámara | 1 / 2 | 17 / 34 | 56 / 120 |
| Sumidero | 1 / 2 | 13 / 26 | 40 / 80 |

### 5.2 Presupuesto de escena

**Modo órbita** (60 fps, ventana 1200×760, ~200 objetos visibles, ciudad de demostración):

| Grupo | Cantidad | LOD / mezcla | Caras | Triángulos |
|---|---:|---|---:|---:|
| Terreno aligerado (celda 25 m) | 1 | — | 2 000 | 2 000 |
| Calles + aceras | 98 | — | 202 | 404 |
| Manzanas (muros fusionados sin trocear) | 108 | — | 432 | 864 |
| Pozos | 35 | 3×LOD0 + 10×LOD1 + 22×LOD2 | 546 | 1 204 |
| Cámaras | 7 | 1×LOD0 + 6×LOD1 | 158 | 324 |
| Tanque | 1 | LOD0 | 125 | 258 |
| Hidrantes | 3 | 1×LOD0 + 2×LOD2 | 45 | 94 |
| Tuberías | 41 arcos (≈150 segmentos) | 6 tramos LOD0 + 144 líneas | 246 | 228 |
| **Total** | | | **≈ 3 750** | **≈ 5 400** |

A 2.9 µs por primitiva: **≈10.9 ms**, dentro de los 16.7 ms con margen para Rune.

**Modo reposo** (imagen quieta, se permite un frame caro): presupuesto **12 000** primitivas, con
el terreno a celda 12 m y todos los objetos visibles a su LOD por píxeles. Como el visor no anima
en reposo (`docs/01-arquitectura.md:219`), 25–30 ms de un frame único es aceptable; lo que no es
aceptable es que ese coste ocurra **durante** el arrastre.

**Contraste con lo que costaría el detalle ingenuo.** Los 46 nodos + 150 segmentos a LOD0 sin
LOD por píxeles: `46 × ~120 + 150 × 17 ≈ 8 070` primitivas de red; con terreno aligerado y ciudad
(≈2 640) se va a ~10 700 primitivas → **≈31 ms**, es decir 32 fps. De ahí la regla G10.

### 5.3 Regla de LOD por tamaño en píxeles

```rust
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Lod { Simbolo = 0, Cilindro = 1, Completo = 2 }

/// `r_px` = radio proyectado en puntos; se mide una vez por objeto y frame.
pub fn lod_de(r_px: f32) -> Lod {
    if r_px < 6.0 { Lod::Simbolo } else if r_px < 25.0 { Lod::Cilindro } else { Lod::Completo }
}
```

Los umbrales son los del encargo y se justifican solos: por debajo de 6 px un pozo son 12 px de
ancho, donde una tapa y un cono son el mismo píxel; entre 6 y 25 px se distingue la silueta
(cuerpo + cono + tapa) pero no una brida; por encima de 25 px el objeto ya ocupa 50 px y empiezan a
verse escalera, marco y volante.

Además del tamaño hay dos topes duros, porque el tamaño no basta cuando hay 30 objetos grandes:

| Tope | Regla |
|---|---|
| Presupuesto de frame | Al alcanzar 80 % del presupuesto de §5.2, los objetos más lejanos bajan un LOD (de `Completo` a `Cilindro`, de `Cilindro` a `Simbolo`). |
| Objetos por LOD0 | Máximo 8 objetos a `Completo` por frame; el resto, un LOD menos. |

### 5.4 Qué se apaga primero

Orden estricto. Cada paso se aplica sólo si hace falta presupuesto, y **nunca** se apaga una tapa
ni el cuerpo: sin ellos el objeto deja de leerse.

| Prioridad | Se apaga | Ahorro (caras) |
|---:|---|---:|
| 1 | Escalera | 20 |
| 2 | Bridas de válvula (2 × 9) | 18 |
| 3 | Volante con radios → `disco` | 21 |
| 4 | Banqueo | 6 |
| 5 | Campanas de tubo (cada una) | 8 |
| 6 | Marco → una sola `caja` | 10 |
| 7 | Enrocado / rejilla (de 6 a 3 piezas) | 36 → 18 |
| 8 | Caras del anillo 12 → 8 → 5 | 33 % y 37 % |
| 9 | Cono → cilindro del mismo diámetro | 0 (se hace por velocidad, no por presupuesto) |
| 10 | Terreno: celda 25 → 35 → 50 m | 2 000 → 1 000 → 500 |
| 11 | Manzanas: muros → sólo cubiertas | 432 → 108 |

Los pasos 1–8 son por objeto y no cambian la silueta. Los pasos 10–11 son de escena y son los que
de verdad mueven la aguja (Hallazgo 3).

---

## 6. Orden del pintor con sólidos

### 6.1 Lo que ya hace `view.rs` y se conserva

`view::build` ordena de lejos a cerca con `sort_far_first` (`view.rs:613`) usando la profundidad
media de la primitiva, y emite en **grupos** con orden fijo: terreno → calles → red → ciudad
(`view.rs:599-607`). La red va sobre el terreno (modo rayos X) y bajo los edificios. Eso no se
toca: es una decisión de producto. Lo que cambia es **cómo se calcula la clave dentro del grupo de
red** y qué se hace con las caras grandes.

### 6.2 Clave de orden

```rust
/// Clave del pintor para una cara ya proyectada.
#[derive(Clone, Copy)]
pub struct Clave {
    pub prof_max: f32, // máximo de las profundidades de sus vértices
    pub capa: u8,      // 0 = pared/cuerpo, 1 = tapa/arriba, 2 = boca/encima
    pub prof_med: f32, // desempate fino
}
```

Se ordena **descendente por `prof_max`** y, en empate, descendente por `capa` y luego por
`prof_med`. Tres decisiones detrás de eso:

1. **Profundidad máxima, no media.** Una cara que abarca 3 m de fondo tiene una media que la
   coloca en el medio del sólido y hace que se dibuje *después* de una tapa que está más lejos: es
   el artefacto clásico del pintor. El máximo la manda al fondo, y la tapa, más cercana, gana.
2. **Capa local como desempate.** Dentro de un mismo objeto, la tapa y el marco deben pintarse
   siempre después de las paredes, aunque compartan profundidad al micrómetro. Un entero de 2 bits
   resuelve lo que de otro modo es *z-fighting* determinista.
3. **`prof_med` sólo desempata.** No decide el orden visible, así que un cambio de cámara no
   reordena dos caras coplanares de forma errática.

### 6.3 Subdivisión de caras grandes

Una cara mayor que ~60 px de lado se parte en el espacio de mundo **antes** de proyectar, para que
su clave de profundidad sea representativa. Regla:

| Elemento | Tamaño máximo de cara | Trozos |
|---|---|---|
| Pared de pozo (H ≈ 3 m) | 1.0 m de alto | `ceil(H / 1.0)` = 3 |
| Pared de cámara / tanque | 1.5 m | `ceil(H / 1.5)` |
| Muro de manzana | 12 m (ya existe: `TROZO` en `view.rs:371`) | se conserva |
| Tramo de tubo | 6 m (el propio tramo) | ya subdividido por juntas |
| Tapa / disco | no se subdivide | es plana: su profundidad es una |

La subdivisión cuesta primitivas: una pared de pozo de 3 m pasa de 12 a 36 caras. Por eso se aplica
**sólo en LOD0** y sólo si el objeto supera 25 px. A 25 px, 3 trozos de pared miden 8 px cada uno:
el artefacto que se evita vale más que las 24 primitivas que cuesta.

### 6.4 Artefactos aceptables

| Artefacto | Cuándo aparece | Decisión |
|---|---|---|
| La boca de la tubería queda tapada por la pared del pozo | Siempre (el tubo entra en el pozo) | **Aceptado.** Se mitiga adelantando la boca 1 mm hacia el eje del tubo. |
| Un tubo que cruza a otro se corta en el cruce | Cruces reales de red | **Aceptado**, ya ocurre hoy y está documentado (`docs/01-arquitectura.md:259`). |
| Brida contra cuerpo de válvula: parpadeo de un píxel | Caras coplanares | **Inaceptable.** Se separa 2 mm en el ensamblaje; lo verifica una prueba. |
| La escalera de un pozo cerrado no se ve | Tapa dibujada encima | **Aceptado** (es lo real). Se muestra el interior sólo con el pozo «destapado»: elevación > 55° **y** seleccionado; entonces se omite la tapa y se dibujan escalera y banqueo. |
| El faldón de un pozo en ladera asoma sobre el terreno | Ladera fuerte | **Aceptado** si asoma &lt; 0.1 m; por encima, se sube el pozo (§8.3). |
| Costura de 1 px entre caras contiguas | Antialiasing por primitiva | **Inaceptable.** Se reutiliza `expand(&pts, 0.5)` de `view.rs:626` por cara. |
| Sombra: no hay | — | **Aceptado.** No hay sombras proyectadas; el contacto se sugiere con el faldón y con una banda oscura en la base (§7.4). |

### 6.5 Lo que no se intenta

No se ordena por objeto y luego por cara dentro del objeto (dos pasadas). Con sólidos que se
interpenetran —tubo dentro de pozo, vástago dentro de bonete— el orden por objeto es *peor* que el
orden por cara, porque fuerza a que todo un objeto quede delante de otro cuando sólo una pieza lo
está. Una sola lista, una sola ordenación, `sort_unstable_by` sobre `Clave`. Esto también mantiene
el coste de ordenación en `O(n log n)` sobre ~3 800 elementos: ≈0.15 ms.

---

## 7. Sombreado y material sin texturas

### 7.1 La función

```rust
/// Sombreado por normal. `n` = normal en mundo, `vista` = dirección cámara→punto.
pub fn sombrear(n: [f32; 3], vista: [f32; 3], mat: Mat, semilla: u32) -> [f32; 3];
```

```
L1 = normalize([-0.45, -0.60, 0.66])   // la misma del terreno: una sola luz en toda la escena
L2 = normalize([ 0.55,  0.35, 0.20])   // relleno, opuesto y bajo
dif = max(0, n·L1)
rie = max(0, n·L2)
H   = normalize(L1 + (-vista))
esp = smoothstep(0.86, 0.97, n·H) * brillo
s   = 0.36 + 0.58 * banda(dif, bandas) + 0.16 * rie + esp
color = base * s * fog + fondo * (1 - fog)
```

- `banda(x, k) = floor(x * k) / (k - 1)` cuantiza el Lambert en `k` escalones. Es lo que da
  «rugosidad» sin textura: el hormigón tiene 4 bandas suaves y el metal 3 duras.
- `fog(depth, distance)` es exactamente el de `view.rs:639`; **no** se reimplementa, se importa.
  Un sólido y el terreno tienen que apagarse al mismo ritmo o el pozo «flota» en la distancia.
- `semilla` es el hash FNV-1a de `(ElementId, índice de cara)`, ya disponible en el código
  (`model.rs:429`). Se usa para las manchas de §7.4. Determinista: la misma cámara da la misma
  imagen.

### 7.2 Por qué bandas y no Lambert continuo

Con Lambert continuo, dos caras contiguas de un cilindro de 12 caras difieren ~5 % de brillo y el
cilindro se lee como un tambor facetado. Cuantizando a 4 bandas, las caras vecinas caen en la misma
banda y el cilindro se lee como un cilindro **con bandas de luz**, que es como se ve el hormigón
real bajo sol duro. Además el número de colores distintos baja de ~200 por objeto a ~6, lo que
facilita el batching en Rune.

### 7.3 Tabla de materiales

| `Mat` | Nombre | `base` | `bandas` | `brillo` | `trama` |
|---|---|---|---:|---:|---|
| 0 | Hormigón visto | (0.62, 0.60, 0.55) | 4 | 0.00 | manchas |
| 1 | Hormigón húmedo | (0.42, 0.43, 0.41) | 4 | 0.05 | manchas |
| 2 | Hierro fundido (tapa) | (0.26, 0.27, 0.29) | 3 | 0.55 | sectores |
| 3 | Acero galvanizado | (0.62, 0.64, 0.66) | 3 | 0.70 | ninguno |
| 4 | Acero pintado (válvula) | (0.24, 0.34, 0.48) | 3 | 0.35 | ninguno |
| 5 | PVC / PE | (0.72, 0.72, 0.70) | 2 | 0.25 | ninguno |
| 6 | Ladrillo | (0.55, 0.36, 0.28) | 4 | 0.00 | hiladas |
| 7 | Grava / filtro | (0.46, 0.44, 0.38) | 3 | 0.00 | motas |
| 8 | Agua (sólo superficie de tanque) | (0.16, 0.34, 0.42) | 2 | 0.80 | ninguno |
| 9 | Terreno del faldón | (0.38, 0.40, 0.28) | 4 | 0.00 | manchas |

`base` es el color sin luz y ya incluye el tinte de familia, así que no hay una segunda tabla de
«color por familia»: el material **es** el color. Un pozo es `Mat 0`; su tapa, `Mat 2`; su marco,
`Mat 0`. Una válvula es `Mat 4` con volante `Mat 2`.

### 7.4 Tapa metálica contra hormigón

La diferencia tiene que leerse a 30 px, no a 300. Cuatro recursos, todos sin textura:

| | Tapa metálica (`Mat 2`) | Marco / cuerpo de hormigón (`Mat 0`) |
|---|---|---|
| Base | gris azulado oscuro, 0.26–0.29 | gris cálido claro, 0.55–0.62 |
| Bandas | 3 (contraste duro, 0.58 de amplitud) | 4 (contraste suave, 0.40) |
| Especular | sí, `smoothstep(0.86, 0.97)`, +0.55 | no |
| Trama | 8 sectores alternos con ±6 % de luz (bajorrelieve) y 2 anillos concéntricos | mancha por hash de cara, ±3 % de luz |
| Borde | aro exterior 0.34 explícito | sin borde |

Los 8 sectores de la tapa son 8 quads entre el cubo central y el borde, con dos tonos alternos:
**8 caras / 16 triángulos** y es lo que hace que la tapa no parezca un disco de plástico. Los 2
anillos se consiguen con 2 `anillo` de 12 caras, sólo en LOD0.

> **Sin texturas, con 4 números.** No hay coordenadas UV ni atlas: el material son `base`,
> `bandas`, `brillo` y `trama`. Añadir un material nuevo es añadir una fila a `MATS`, no tocar el
> ensamblaje.

---

## 8. Colocación, orientación y «floating»

### 8.1 Colocación

```rust
#[derive(Clone, Copy, Debug)]
pub struct Colocacion { pub origen: [f64; 3], pub giro_z: f32 }

impl Colocacion {
    /// Lleva un vértice local (m) a mundo (m, sin restar el origen del proyecto).
    pub fn a_mundo(&self, p: [f32; 3]) -> [f64; 3];
}

/// Origen en la solera; giro tomado del primer arco no borrado del nodo.
pub fn colocacion_de_nodo(n: &Node, p: &Project) -> Colocacion;
```

- `origen = (n.pos.x, n.pos.y, n.z_invert)`. La solera es el cero local porque es el datum del
  pozo: `z_rim − z_invert = H` es su altura, y `H` ya sale de un campo del modelo.
- `giro_z = atan2(dy, dx)` del arco saliente (o entrante si no hay saliente) en planta. Sin arcos,
  `giro_z = 0`. Así las bocas de acometida caen donde cae la tubería sin buscar el ángulo.
- No hay escala. Las primitivas se generan con dimensiones reales; la exageración vertical de la
  cámara (`Camera::z_scale`) se aplica al proyectar, en `to_local` (`view.rs:212`), no aquí.

> **Aviso de `z_scale`.** Con `z_scale = 2.6` (el valor de la app para la ciudad,
> `crates/redes-app/src/main.rs:68`) un cilindro deja de ser un cilindro en pantalla: se estira
> 2.6× en vertical. Es aceptable y ya pasa hoy, pero significa que **no se puede juzgar la
> proporción de un pozo en pantalla**; para eso está el inspector con las cotas.

### 8.2 Orientar un tubo entre dos puntos

```rust
/// Marco ortonormal con +Z a lo largo del eje. `e` = eje, `lado` y `arriba` completan.
pub fn marco_tubo(a: [f64; 3], b: [f64; 3]) -> ([f64; 3], [f64; 3], [f64; 3]);
```

```
e     = normalize(b - a)
ref   = if |e.z| > 0.9 { [1,0,0] } else { [0,0,1] }   // evita el paralelismo con Z
lado  = normalize(cross(e, ref))
arriba= cross(lado, e)
```

`ref` cambia a `[1,0,0]` cuando el tubo es casi vertical: con `ref = [0,0,1]` el producto vectorial
se anula y el marco explota (el mismo problema que ya resuelve `Camera::basis` en `view.rs:94`).
Para un tubo vertical se usa `e` como eje y `lado`/`arriba` en el plano horizontal.

El tubo se construye siempre con el eje `+Z` local y se transforma con la matriz cuyas columnas son
`(lado, arriba, e)`. Una sola función, `colocar_tramo(&Malla, a, b) -> Malla`, sirve para el tubo,
para la campana y para la válvula en línea.

### 8.3 Evitar el «floating» en ladera

El defecto actual: `z_rim` viene del modelo y en una ladera el terreno, dentro del diámetro del
pozo, cae 0.3–0.6 m. El cilindro se apoya en `z_invert` y su tapa queda a `z_rim`, así que en el
lado de abajo aparece un hueco de aire entre la tapa y el suelo.

```rust
/// Cota de tapa corregida y faldón, muestreando el terreno en el anillo del pozo.
pub fn asentar(n: &Node, t: &Terrain, r: f64) -> Asiento { /* ... */ }

pub struct Asiento {
    pub z_sup: f64,     // cota de tapa definitiva
    pub z_inf: f64,     // cota del punto bajo del anillo
    pub faldon: Option<f64>, // altura del faldón si hace falta
}
```

Regla, con 8 muestras del TIN sobre el anillo de radio `r + 0.40`:

| Caso | Condición | Acción |
|---|---|---|
| Plano | `z_max − z_min ≤ 0.25` | `z_sup = max(z_rim, z_max)`. Sin faldón. |
| Ladera | `0.25 < z_max − z_min ≤ 1.20` | `z_sup = z_max` y **faldón** `cilindro(12)` de `r+0.15` desde `z_sup` hasta `z_min − 0.20`, material 9. Coste +12 caras. |
| Enterrado | `z_sup − z_rim > 1.20` | **Brocal**: `cilindro(12)` hasta `z_sup` con `r + e`, material 0. Coste +13 caras. |
| Fuera del TIN | `z_at` devuelve `None` en más de 2 muestras | `z_sup = z_rim`, sin faldón, y el objeto se marca «sin terreno» para el informe. Nunca se dibuja a cota 0 (la regla ya existe en `view.rs:279`). |

El faldón es la pieza que resuelve el problema y por eso no se apaga nunca en el orden de §5.4:
cuesta 12 caras, menos que los dos peldaños de escalera que se apagan antes que él (8 caras), y
elimina el artefacto más visible de la demo.

**Mismo problema, otras familias.** Tanque: base en `min(cotas) − 1.0`, cubierta en
`max(cotas) + altura` (la regla que ya usa `view.rs:382` para las manzanas; se reutiliza). Cámara
y sumidero: idéntico a pozo. Hidrante y válvula: **no** se asientan con faldón (son piezas sobre
rasante); se anclan con una `brida` de base que se hunde 0.05 m bajo el terreno, suficiente para
que no se vea el borde.

---

## 9. Rendimiento: qué se cachea y con qué clave

### 9.1 Dos cachés

```rust
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct ClaveMalla {
    pub fam: u16,        // hash de `Node::family` / `Link::family`
    pub ty: u16,         // hash de `Node::ty`
    pub lod: u8,
    /// Parámetros cuantizados: cm para longitudes, grados para ángulos.
    pub q: [u16; 6],
}

pub struct CacheMallas {
    mapa: HashMap<ClaveMalla, Arc<Malla>>,
    orden: VecDeque<ClaveMalla>,   // LRU
    tope: usize,                   // 256 entradas
}

impl CacheMallas {
    pub fn obtener(&mut self, k: ClaveMalla, generar: impl FnOnce() -> Malla) -> Arc<Malla>;
}
```

| Caché | Clave | Invalida cuando | Tamaño |
|---|---|---|---|
| **Malla local** | `ClaveMalla` (familia + subtipo + LOD + parámetros cuantizados) | Nunca: es función pura de la clave | 256 mallas, LRU |
| **Malla en mundo** | `(ElementId, Project::revision, Lod, asiento)` | Cambia el objeto, el documento o el terreno | 1 entrada por objeto, ~200 |

**Cuantización.** `q_cm(x) = (x * 100.0).round() as u16` para longitudes y `q_deg(a) = (a.to_degrees().round() as u16) % 360`
para el giro. Un pozo de `D = 1.20` y otro de `1.201` comparten malla: 1 cm de diferencia no se ve
a 25 px y evita fabricar una malla nueva por cada pozo ligeramente distinto. En la demo hay 35
pozos con 3 diámetros distintos (0.90, 1.10, 1.30): **3 mallas, no 35**.

**Por qué dos niveles.** La malla local no depende de la cámara ni del terreno: se construye una
vez por combinación y se reutiliza entre proyectos con la misma clave (útil al abrir la misma
ciudad dos veces). La malla en mundo sí depende del asiento (que muestrea el TIN) y de la
revisión: si el usuario aprueba un objeto, sólo se invalida ése.

### 9.2 Cuánto cuesta reconstruir

Coste estimado de `Malla` en release, a ~0.2 µs por cara (trigonometría del anillo + empuje de
vértices + AABB):

| Ensamblaje | Caras | Coste |
|---|---:|---:|
| Pozo LOD0 | 118 | ≈25 µs |
| Pozo LOD1 | 17 | ≈4 µs |
| Válvula compuerta LOD0 | 78 | ≈18 µs |
| Tramo de tubo LOD0 | 17 | ≈4 µs |
| Cámara LOD0 | 56 | ≈12 µs |

Un cambio de diámetro que invalide **todos** los pozos cuesta 35 × 25 µs ≈ 0.9 ms: irrelevante. Lo
que **no** es irrelevante es reconstruir todo cada frame en órbita: 200 objetos × ~20 µs ≈ 4 ms, un
cuarto del presupuesto de 16.7 ms. Por eso la caché no es una optimización opcional: es lo que
permite el LOD0 selectivo.

### 9.3 Reparto del frame a 60 fps

| Etapa | Coste objetivo | Nota |
|---|---:|---|
| Consulta de caché (200 objetos) | 0.1 ms | *hash* de 6 `u16` + `HashMap` |
| Transformar + proyectar (~3 800 caras × 4 vértices) | 1.2 ms | ~15 ns por vértice |
| Backface culling | 0.1 ms | un producto escalar por cara |
| Ordenar (`sort_unstable_by`, 3 800) | 0.15 ms | ~40 ns por comparación |
| Emitir `Shape` (3 800 `fill`) | 3.8 ms | ≈1 µs por primitiva (§1) |
| Rune: ear-clipping + GPU | ≤8.0 ms | medido por el cliente: 25 ms / 8 700 |
| **Total** | **≤13.3 ms** | deja 3.4 ms de margen |

### 9.4 Reglas de la órbita

1. **Durante `Phase::Move`** (`crates/redes-app/src/main.rs:283`): `Lod::Presupuesto(3 500)` y
   terreno a celda 35 m. El LOD por píxeles sigue mandando, pero el presupuesto recorta.
2. **Al soltar (`Phase::Up`)**: una reconstrucción a calidad plena (§5.2, modo reposo) y se deja de
   pedir frames. El usuario ve el detalle cuando deja de moverse, que es cuando lo mira.
3. **`tx.live_only()` ya está**: no se reconstruye el árbol de UI, sólo el `Shape`.
4. **Nada de trabajo nuevo en el frame**: ni `fs`, ni parseo, ni `Vec::with_capacity` grandes. Las
   mallas salen de la caché como `Arc<Malla>`; el único `Vec` que crece por frame es el de
   primitivas, con capacidad reservada a `presupuesto + 10 %`.
5. **Sin hilos en órbita.** El pool de 2 hilos sólo se usa al abrir un proyecto o al cambiar el
   LOD global de golpe (>200 mallas nuevas). En órbita todo es *hit* de caché y el coste está en
   proyectar, que es inherentemente secuencial por el orden del pintor.

---

## 10. Pruebas sin mirar la pantalla

El criterio: **toda propiedad geométrica que el ojo verificaría se convierte en un número**. Estas
pruebas viven en `crates/redes-core/src/solido.rs` (`#[cfg(test)]`) y no necesitan GPU ni ventana.

### 10.1 Presupuesto y conteo

```rust
#[test] fn conteo_por_familia_y_lod();
```

Tabla dirigida por datos: para cada `(familia, subtipo, lod)` de §5.1, construir con parámetros
canónicos y afirmar `malla.caras.len()` y `malla.tris` dentro de ±10 %. Es la prueba que impide que
un ensamblaje se infle sin que nadie lo note: el día que alguien añada tres bridas a la compuerta,
falla con «78 → 96 caras, presupuesto 78».

### 10.2 Caja envolvente

```rust
#[test] fn aabb_coincide_con_los_parametros();
```

Para un pozo de `D = 1.20`, `e = 0.15`, `H = 3.00`: `aabb.min == [-(D/2+e), -(D/2+e), 0.0]`,
`aabb.max == [(D/2+e), (D/2+e), H]`, tolerancia 1 mm. Detecta el error de signo, el olvido de `e`
y el origen mal puesto (el fallo más caro: un pozo dibujado con el cero en la tapa).

### 10.3 Invariantes de forma

```rust
#[test] fn la_tapa_esta_en_la_cota_de_rim();
#[test] fn la_solera_esta_en_z_cero();
#[test] fn el_solido_es_cerrado_y_orientado();
#[test] fn el_volumen_coincide_con_el_analitico();
```

- **Tapa:** existe una cara con `flags & CARA_TAPA != 0`, normal `≈ [0,0,1]` y todos sus vértices a
  `z == H` (±0.5 mm). Es literalmente la invariante que pidió el encargo.
- **Solera:** existe una cara con normal `≈ [0,0,-1]` a `z == 0`.
- **Cerrado:** contar aristas; cada arista dirigida debe aparecer exactamente una vez y su inversa
  exactamente una vez. `aristas_libres(malla) == 0` para todo sólido marcado cerrado. Un tubo
  hueco es cerrado; un prisma sin tapas, no, y se declara.
- **Volumen firmado:** `Σ (1/6)·a·(b×c)` sobre los triángulos, comparado con `π/4·D²·H` (±3 % por
  el poligonado). Esta prueba es la que detecta caras invertidas y caras faltantes a la vez, sin
  mirar la pantalla: un sólido con una cara al revés da un volumen menor.

### 10.4 Golden hash

```rust
#[test] fn la_malla_canonica_no_cambia();
```

FNV-1a sobre los bytes de `verts` de 4 mallas canónicas (pozo LOD0, válvula compuerta, tramo de
tubo, cámara). Si el hash cambia, alguien tocó la geometría: la prueba no dice si está mal, dice
«esto cambió, decídelo». Se actualiza a mano con el cambio en el mismo commit.

### 10.5 LOD y colocación

```rust
#[test] fn lod_por_pixeles_en_los_bordes();   // 5.9→Simbolo, 6.0→Cilindro, 24.9→Cilindro, 25.0→Completo
#[test] fn el_marco_del_tubo_es_ortonormal(); // |e|=|lado|=|arriba|=1, productos ≈ 0, para tubos horizontales, verticales y a 45°
#[test] fn el_asiento_no_deja_hueco();        // para un TIN sintético en ladera 20 %, z_inf del faldón < terreno en las 8 muestras
#[test] fn no_hay_piezas_coplanares();        // toda brida separa ≥ 2 mm su cara de la del cuerpo vecino (§6.4)
```

`no_hay_piezas_coplanares` recorre los pares de caras de un mismo ensamblaje y falla si dos
normales son casi paralelas y sus planos distan menos de 2 mm: es la prueba que respalda lo que
§6.4 promete y evita el parpadeo de un píxel sin abrir la ventana.

### 10.6 Escena

```rust
#[test] fn la_ciudad_cabe_en_el_presupuesto_de_orbita();
```

Para la ciudad de demostración (`demo_city::ciudad()`), con la cámara de `Camera::fit` y 8 azimuts
de 45°, afirmar `scene.prims ≤ 6 000` y `scene.tri_count ≤ 12 000`. Es la prueba que convierte el
presupuesto de §5.2 en un contrato ejecutable.

```rust
#[test] fn el_orden_del_pintor_es_monotono();  // depths no crecientes, con la nueva Clave
#[test] fn ninguna_cara_tiene_mas_de_12_vertices();
#[test] fn ninguna_cara_es_auto_intersectada(); // área = Σ triángulos del abanico, para quads y anillos
```

### 10.7 Regresión por CLI

`redes vista` ya devuelve `primitivas`, `triangulos` y `lineas` en el sobre
(`crates/redes-cli/src/main.rs:310`). Basta una prueba que ejecute la vista de la ciudad y afirme
que las tres cifras están bajo presupuesto: es la misma comprobación que §10.6 pero sobre el
camino real, y sirve de centinela en CI sin GPU.

---

## 11. Decisiones abiertas, riesgos y fuera de alcance

### 11.1 ADR abiertos

| # | Decisión | Opciones | Recomendación |
|---|---|---|---|
| GA-1 | ¿La geometría vive en `redes-core` o en la app? | (a) núcleo, (b) `redes-app` | **(a)**: el exportador SVG de la CLI debe ver los mismos sólidos, y el núcleo no depende de Rune. |
| GA-2 | ¿Se persiste la malla o se regenera? | (a) regenerar siempre, (b) cachear en el `.redes` | **(a)**: la malla es función de los atributos; persistirla crea una segunda verdad. |
| GA-3 | ¿El detalle se apaga por presupuesto automático o por preferencia del usuario? | (a) automático, (b) con control | **(a)** con el tope de §5.3, y un chip «Detalle» que fuerza `Completo` mientras se inspecciona un objeto. |
| GA-4 | ¿Se dibuja el interior de los pozos? | (a) nunca, (b) al destapar | **(b)** con la regla de §6.4: elevación > 55° y seleccionado. Es barato y es lo que el revisor quiere ver. |

### 11.2 Riesgos

| # | Riesgo | Impacto | Mitigación |
|---|---|---|---|
| GR-1 | El pintor falla en intersecciones reales (tubo dentro de pozo) | Artefacto puntual | Backface culling + clave por profundidad máxima + subdivisión; el resto se declara aceptable (§6.4) |
| GR-2 | Un presupuesto de 5 800 primitivas es frágil si Rune cambia el coste por `fill` | 60 fps en riesgo | El presupuesto es un parámetro de `Lod::Presupuesto`, no una constante; se recalibra con una medición |
| GR-3 | `z_scale` deforma los sólidos | Proporción falsa en pantalla | Ya documentado; el inspector muestra cotas reales. No se corrige en geometría |
| GR-4 | Cuantizar a 1 cm colapsa pozos que el usuario distingue | Dos pozos de 1.20 y 1.21 idénticos | Aceptado: 1 cm a 25 px es 0.4 px. Si molesta, bajar a 5 mm duplica las mallas |
| GR-5 | El faldón tapa una boca de acometida en ladera | Lectura incorrecta de la conexión | El faldón se dibuja **antes** que la boca (capa 0 vs 2) y la boca se adelanta 1 mm |
| GR-6 | La malla local en caché crece sin control con proyectos grandes | Memoria | LRU de 256 entradas; una malla LOD0 pesa ~8 KB (472 vértices × 12 B + 118 caras × 20 B) → 256 × 8 KB ≈ 2 MB |

### 11.3 Fuera de alcance

- Sombras proyectadas y oclusión ambiental: el renderer no tiene búfer de profundidad.
- Texturas, UV y atlas: se sustituyen por materiales + bandas (§7).
- Teselado adaptativo por objeto (subdivisión de curvas): un cilindro de 12 caras es suficiente a
  la escala del visor.
- Exportación a DXF/DWG de los sólidos: el detallado se hace en CAD, como pidió el cliente.
- Animación (válvulas que giran, agua que corre): el visor es de revisión, no de operación.

---

## Anexo A — Dónde se toca el código

| Archivo | Cambio |
|---|---|
| `crates/redes-core/src/solido.rs` | **Nuevo.** §2–§3 y §9: `Malla`, `Cara`, `Constructor`, primitivas, `Lod`, `CacheMallas` |
| `crates/redes-core/src/ensamblaje.rs` | **Nuevo.** §4 y §8: `ensamblar`, `colocacion_de_nodo`, `marco_tubo`, `asentar` |
| `crates/redes-core/src/view.rs` | Sustituye el bloque de cilindro de 12 caras (`view.rs:550-593`) por la emisión de `Malla`; `sort_far_first` pasa a ordenar por `Clave` |
| `crates/redes-core/src/lib.rs` | `pub mod solido; pub mod ensamblaje;` |
| `crates/redes-app/src/main.rs` | `rebuild()` con `Lod::Presupuesto` durante el arrastre (§9.4) |
| `crates/redes-cli/src/main.rs` | Sin cambios: `vista` ya reporta `primitivas`/`triangulos` |

## Anexo B — Correspondencia con las familias del registro

Las claves de `crates/redes-core/src/families.rs` que tienen ensamblaje propio: `saneamiento.pozo`
(inspeccion, caida, resalto, ciego, bombeo, limpieza, ventilacion), `saneamiento.tuberia` y
`agua.tuberia` (8 y 4 subtipos, mismo ensamblaje), `saneamiento.descarga`,
`agua.valvula` (8 subtipos), `agua.llave` (sólo `hidrante`; `corporacion`, `banqueta` y `tapon` son
símbolos de 1 cara), `agua.tanque`, `agua.medidor`, `saneamiento.camara`, `saneamiento.sumidero`.
`saneamiento.canal` y `saneamiento.sifon` usan el ensamblaje de tubería con sección constante.
`comun.cruce` y `comun.hito` no tienen sólido: son símbolos.
