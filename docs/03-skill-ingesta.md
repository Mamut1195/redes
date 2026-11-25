# 03 — La skill de ingesta

> La skill vive en `skills/redes-ingesta/`: `SKILL.md` (flujo y reglas) y `references/` (contrato
> del plan y una referencia por formato). Este documento explica **por qué** existe y **cómo**
> encaja en el producto. Para el procedimiento, abrir `SKILL.md`.

---

## 1. El problema que resuelve

Redes no diseña: recibe. Cada aplicación exporta a su manera:

| Origen | Lo que da | Lo que le falta |
|---|---|---|
| Civil 3D | DXF con capas, bloques con atributos, polilíneas 3D; mejor aún LandXML o informes | Cotas y diámetros suelen estar solo en etiquetas; los objetos AECC no salen en DXF |
| SWMM | `.inp` con topología, soleras, desfases, secciones y planta (`[COORDINATES]`, `[VERTICES]`) | Terreno cuando `MaxDepth = 0`; forma constructiva de pozos y descargas; sistema si no hay `[DWF]` ni subcuencas |
| EPANET | `.inp` con topología, cotas de nudo, diámetros y planta (`[COORDINATES]`, `[VERTICES]`) | Terreno (la cota del nudo es hidráulica); qué es cada `RESERVOIR`; si un tanque es apoyado o elevado; si una tubería es conducción o distribución |
| Excel | Cuadros de tramos y pozos con cotas | Planta (a veces); qué columna es clave y cuál solera |
| SIG | GeoJSON/Shapefile con planta y atributos | CRS proyectado (a menudo viene en lon/lat); cotas fiables |

## 2. La decisión: una skill guiada, no adaptadores (D5)

```
   SWMM, EPANET, DXF/LandXML ajeno,     agente + skill        CLI
   GeoJSON/Shapefile, Excel/CSV   ──►   (lee, entiende,  ──►  redes importar plan
                                         pregunta, mapea)     (valida, informa)
                                              │                     │
                                              ▼                     ▼
                                        redes.plan/1           documento *.redes

   DXF escrito por Redes (REDES_INFO) ──► redes importar dxf
   Terreno E,N,Z (CSV)                ──► redes importar terreno
```

- **Un solo formato de entrada** para orígenes ajenos: `redes.plan/1`.
- **No hay un conversor por programa.** Los scripts `inp_to_plan.py` y `plan_desde_tabla.py`
  y la heurística de capas de `ingest.rs` se retiran: cada origen real trae variantes que un
  mapeo fijo resuelve adivinando. Un agente que **lee** el archivo con la referencia del formato
  delante puede detectar la ambigüedad y **preguntar**.
- La skill es **conocimiento**, no código: columnas, unidades, qué significa cada cota, trampas
  conocidas y qué no se inventa. Un formato nuevo es una referencia nueva, no una versión nueva
  del binario.
- La CLI es la que **valida**: familia y subtipo existentes, códigos únicos, extremos existentes,
  esquema conocido. El agente corrige según sus diagnósticos.

**Consecuencias buenas:** un origen nuevo no exige recompilar; las decisiones dudosas (sistema,
subtipo, CRS, DN o DI) las toma el usuario y quedan en el informe; los ejemplos trabajados
muestran línea a línea cómo se lee un `.inp`.

**Consecuencias malas (asumidas):** la conversión depende de un agente que siga las reglas; no es
determinista como un script. Se mitiga con reglas duras en `SKILL.md`, con los ejemplos de
`crates/redes-core/tests/fixtures/` (su importación está probada en `tests/ingesta.rs`) y con la
validación de la CLI.

## 3. Qué garantiza el importador del plan

| Garantía | Cómo |
|---|---|
| No calcula | Los números del origen se copian a `Dato { valor, unidad, fuente }` |
| No pierde objetos en silencio | Informe con `nodos`, `arcos`, `detalle_descartes` (motivo de cada uno) y `renombrados` |
| No duplica | Código repetido → descarte con motivo |
| No acepta modelos rotos | Extremo inexistente → descarte; mismo nodo en ambos extremos → `E_TOPO_SELF_LOOP` |
| Deja rastro | `fuentes[]` con app, archivo, formato y fecha |

Pendiente (F12-03): hoy `import_plan` todavía rellena huecos (terreno = solera, solera =
terreno − 1,5, DN 200, subtipo por defecto). Por eso la skill obliga a listar los ausentes en el
informe. Pendiente también: que `--dry-run` valide el plan sin escribir (hoy solo comprueba el
archivo); la skill valida importando en un proyecto de prueba.

### Cotas: solera y clave no son lo mismo (F12-01, F12-02)

| Cota | Qué es | En el plan |
|---|---|---|
| Solera (invert) | Fondo interior del tubo o del pozo | nodo: `cota` → `z_invert`; tramo: `cota_solera_inicio`/`cota_solera_fin` y Z de `vertices` |
| Clave | Punto más alto del interior del tubo: clave = solera + DI | tramo: `attrs.cota_clave` (inicio) y `attrs.cota_clave_fin` |
| Rasante / tapa | Superficie / boca del pozo | `cota_terreno` (→ `z_terrain` y `z_rim`); tapa distinta: `attrs.cota_tapa` |
| Recubrimiento | rasante − (clave + espesor) | solo si el origen lo declara |

Una clave nunca se copia como solera; solera = clave − DI solo con el DI declarado.

## 4. Estructura de la skill

| Archivo | Contenido |
|---|---|
| `SKILL.md` | cuándo usarla, flujo (identificar → leer referencia → plan → `--dry-run` → corregir → importar → informe), reglas duras, qué preguntar |
| `references/plan.md` | contrato `redes.plan/1` derivado de `ingest.rs`, trampas del contrato, cotas, cómo consultar el registro (`redes esquema --familia`) y validar |
| `references/swmm.md` | SWMM 5: secciones, columnas, `FLOW_UNITS`, `LINK_OFFSETS`, `XSECTIONS`, mapeo, ejemplos y trampas |
| `references/epanet.md` | EPANET 2.2: secciones, `Units`, `Headloss`, `TANKS`, `RESERVOIRS`, válvulas, mapeo, ejemplos y trampas |
| `references/dxf-civil3d.md` | DXF de terceros: códigos de grupo, `INSERT`+`ATTRIB`, polilíneas 3D, capas NCS y en español, ajuste de extremos; LandXML |
| `references/geojson-sig.md` | GeoJSON/Shapefile: CRS, campos típicos, extremos |
| `references/tablas.md` | Excel/CSV e informes de Civil 3D: columnas, clave → solera |

## 5. Cómo se prueba

| Prueba | Qué verifica |
|---|---|
| `cargo test -p redes-core --test ingesta` | Los planes de ejemplo de cuatro `.inp` (SWMM CFS y CMS, EPANET GPM y LPS) entran sin descartes y con las cotas, diámetros y familias esperadas |
| `cargo test -p redes-core --test cotas_tramo` | La solera declarada del tramo manda y el tubo se dibuja sobre ella |
| `redes importar plan` en un proyecto de prueba | Los descartes y renombrados del plan concreto |
| `redes vista --salida x.svg` | Lo que entró se ve (una imagen que un agente puede mirar) |

## 6. Lo que la skill **no** hará nunca

- Rellenar caudales, presiones, volúmenes, terrenos o coordenadas «faltantes».
- Convertir un modelo hidráulico en un diseño, ni decidir si un diámetro cumple norma.
- Elegir por el usuario lo que el origen no dice (sistema, CRS, subtipo): lo pregunta.

Si un proyecto no se puede representar sin calcular algo, es que el origen no traía el dato y
hay que volver a la aplicación que lo produce.
