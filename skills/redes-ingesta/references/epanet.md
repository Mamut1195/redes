# EPANET 2.2 (`.inp`) → `redes.plan/1`

Columnas según el manual de EPANET 2.2 (apéndice C, *Input File Format*). Antes de escribir el
plan, lee `plan.md`.

## 1. Cómo leer el archivo

- Mismo formato que SWMM: `[SECCION]`, `;` comentario, campos separados por espacios o
  tabuladores. `[OPTIONS]` suele ir **al final** del archivo: léelo antes de convertir nada.
- Un ID nombra un nodo o un enlace en todo el archivo; `[COORDINATES]`, `[VERTICES]`, `[TAGS]` y
  `[DEMANDS]` se cruzan por ID. `[END]` cierra el archivo.

## 2. Unidades (`[OPTIONS]`)

| Opción | Valores | Implica |
|---|---|---|
| `Units` | `CFS`, `GPM`, `MGD`, `IMGD`, `AFD` (US) · `LPS`, `LPM`, `MLD`, `CMH`, `CMD` (SI; `CMS` desde 2.3) | US: cotas y longitudes en **pies** (× 0,3048); diámetro de tubería y de válvula en **pulgadas** (× 25,4); diámetro de tanque en **pies**; presión en psi (× 0,7034 m = 0,3048/0,4333, la constante de EPANET); potencia en hp (× 0,7457 kW). SI: m, mm, m, m.c.a., kW. **Si falta, EPANET asume GPM** (dilo). Caudal a L/s: GPM × 0,0630902; CFS × 28,3168; MGD × 43,8126; IMGD × 52,6168; AFD × 14,2764; LPM ÷ 60; MLD × 11,5741; CMH ÷ 3,6; CMD ÷ 86,4 |
| `Headloss` | `H-W` (por defecto) · `D-W` · `C-M` | qué es la rugosidad de `[PIPES]`: C de Hazen-Williams; ε de Darcy-Weisbach (**milipies** en US, **mm** en SI); n de Chezy-Manning |
| `Pressure` | `PSI`, `KPA`, `METERS`… | unidad en que se **informan** presiones; si no es la por defecto, confirma la de las consignas de válvula con el usuario |

`[BACKDROP] UNITS` (`FEET`, `METERS`, `DEGREES`, `NONE`) es la unidad del mapa, independiente de
`Units`. `DEGREES` → lon/lat: pide reproyectar.

## 3. Secciones

| Sección | Columnas (índice desde 0) | Lectura |
|---|---|---|
| `[JUNCTIONS]` | `0 ID 1 Elev 2 [Demand] 3 [Pattern]` | `Elev` = cota del nudo con la que EPANET calcula presión (terreno o eje del tubo según el modelador: **pregunta**). Demanda base en `Units` |
| `[RESERVOIRS]` | `0 ID 1 Head 2 [Pattern]` | `Head` = **carga hidráulica** (nivel de agua): no es terreno, ni fondo, ni cota de la estructura |
| `[TANKS]` | `0 ID 1 Elev 2 InitLvl 3 MinLvl 4 MaxLvl 5 Diam 6 MinVol 7 [VolCurve] 8 [Overflow]` | `Elev` = fondo. Niveles medidos **desde el fondo**: nivel máx. = `row[4]`, **no** `row[5]`, que es el diámetro. Con `VolCurve` el diámetro no describe el tanque |
| `[PIPES]` | `0 ID 1 Node1 2 Node2 3 Length 4 Diam 5 Roughness 6 MinorLoss 7 [Status]` | `Status`: `Open`, `Closed`, `CV` (válvula de retención en el tramo) |
| `[PUMPS]` | `0 ID 1 Node1 2 Node2 3… palabras clave` | `HEAD curva`, `POWER valor`, `SPEED valor`, `PATTERN patrón` (en cualquier orden) |
| `[VALVES]` | `0 ID 1 Node1 2 Node2 3 Diam 4 Type 5 Setting 6 MinorLoss` | `Type`: `PRV`, `PSV`, `PBV` (presión), `FCV` (caudal), `TCV` (coeficiente), `GPV` (`Setting` = ID de curva); `PCV` desde 2.3 |
| `[DEMANDS]` | `0 Junction 1 Demand 2 [Pattern] ;Category` | **sustituye** a la demanda de `[JUNCTIONS]`; varias filas = varias categorías |
| `[EMITTERS]` | `Junction Coefficient` | aspersor, fuga o hidrante: **no** supongas hidrante; pregunta |
| `[STATUS]` | `ID Status/Setting` | estado inicial de enlaces; cita en el informe si cierra tramos |
| `[COORDINATES]` | `Node X Y` | planta; sin fila → sin nodo (no a (0,0)) |
| `[VERTICES]` | `Link X Y` | vértices intermedios en orden |
| `[TAGS]` | `NODE\|LINK ID Tag` | etiquetas del usuario: pistas del tipo de objeto |
| `[PATTERNS]`, `[CURVES]`, `[CONTROLS]`, `[RULES]`, `[ENERGY]`, `[QUALITY]`, `[SOURCES]`, `[REACTIONS]`, `[MIXING]`, `[TIMES]`, `[REPORT]`, `[LABELS]` | — | no son objetos; los comentarios de `[CURVES]` (`;PUMP: Bomba sumergible del pozo`) son pistas |

## 4. Mapeo a familias de Redes

| Origen | Familia:subtipo | Campos |
|---|---|---|
| `JUNCTION` | **no es una cámara** ni un pozo. Proponer `comun.accesorio` según los enlaces que llegan (1 `tapon`, 2 `union` o `reduccion` si cambian los diámetros, 3 `tee`, 4 `cruz`) y **confirmar** con el usuario; 0 o > 4 enlaces → listar | `cota_eje = Elev` si el modelador lo puso en el eje del tubo (lo habitual), o `cota_terreno = Elev` si es el terreno (pregunta, §3). **Nunca** `cota`: no es solera. `attrs.tipo_accesorio` = subtipo; `datos.demanda_base` (L/s), o `demanda_base_1`, `_2`… por categoría |
| `RESERVOIR` | sin familia propia: **pregunta** qué es (`agua.captacion:<subtipo>`, `agua.bocatoma`, `agua.tanque`…). Sin respuesta, no entra | `datos.carga_hidraulica = Head`. **Sin `cota`** |
| `TANK` | `agua.tanque:<pregunta>` (EPANET no distingue apoyado, elevado ni enterrado) | `cota = attrs.cota_piso = Elev`; `cota_nivel_min = Elev + MinLvl`; `cota_nivel_max = Elev + MaxLvl`; `attrs.diametro` en **mm** si no hay `VolCurve`; `datos.nivel_inicial`. **Sin `volumen_util`** |
| `PIPE` | `agua.tuberia:<pregunta>` (`conduccion`, `aduccion`, `impulsion`, `distribucion_primaria`…); EPANET no lo dice. Proponer por posición (aguas abajo de una bomba hasta el tanque: `impulsion`) y confirmar | `diametro_mm` / `diametro_interior_mm` según §5 (pulgadas × 25,4); `datos.longitud_origen`; `H-W` → `attrs.coef_hazen`; `D-W` → `attrs.rugosidad_abs` (mm); `C-M` → `datos.rugosidad_n`; `CV` → `attrs.tipo_valvula = "retencion"` |
| `PUMP` | `agua.bombeo:booster_en_linea` (o la estación que diga el usuario), en la planta de `Node1` | la cota de `Node1` en el mismo campo (`cota_eje`) si la tiene; `attrs.potencia_unitaria` (kW) si hay `POWER`; `desde = Node1`, `hasta = Node2`. Sin arco |
| `VALVE` `PRV` | `agua.valvula:reduccion_presion`, en `Node1` | `cota_eje` de `Node1`, `desde = Node1`, `hasta = Node2`, `attrs.tipo_valvula = "reduccion_presion"`, `diametro_nominal` (mm), `consigna_presion` (m) |
| `VALVE` `PSV` | `agua.valvula:sostenedora` | `consigna_presion` |
| `VALVE` `FCV` | `agua.valvula:reguladora_caudal` | `tipo_valvula = "reguladora"`, `consigna_caudal` (L/s) |
| `VALVE` `TCV`, `PBV`, `GPV`, `PCV` | sin equivalente directo: pregunta (una `TCV` suele ser una compuerta o mariposa estrangulada) | — |

**Dispositivos de enlace.** Bombas y válvulas son enlaces en EPANET y nodos en Redes: se ubican
en `Node1`, no se unen con arcos y declaran `desde = Node1` y `hasta = Node2` (`plan.md` §2.1):
la topología los ve conectados. Si uno de esos nodos no entra en el plan (p. ej. un `RESERVOIR`
sin respuesta), ese lado queda sin unir y el informe lo dice.

**Nudos.** Una familia de nudo genérico no hace falta: `comun.accesorio` con `tapon`, `union`,
`reduccion`, `tee` o `cruz` según los enlaces cubre los nudos de EPANET (no se añade un subtipo
«nudo», que se dibujaría como tapón). Su cota va en `cota_eje`: el nodo queda sin solera y se
dibuja plano a esa cota; los tubos sin soleras salen como línea sin dato, que es lo que EPANET
da.

**Vértices.** EPANET no da Z: escribe `[VERTICES]` como `[E,N]` (`plan.md` §2.2). Sin soleras en
los extremos el tramo queda sin cotas de dato; no las repartas tú.

## 5. Diámetro nominal frente a interior

`Diam` de EPANET es el diámetro **hidráulico** (interior) que escribió el modelador, pero muchos
escriben el DN comercial (160 para un PVC de 160 mm). Pregunta cuál es. Si es interior y el
usuario da el DN, `diametro_mm` = DN y `diametro_interior_mm` = `Diam`; si no, `diametro_mm` =
`Diam` y dilo. Los ejemplos de §6 escriben `Diam` como DN porque son valores comerciales (160,
110, 90).

## 6. Ejemplos trabajados (`crates/redes-core/tests/fixtures/`)

### `epanet_si_lps.inp` → `epanet_si_lps.plan.json` (LPS, H-W)

| Línea del `.inp` | Objeto del plan |
|---|---|
| 106 `Units LPS`, 107 `Headloss H-W` | m, mm, L/s; rugosidad → `coef_hazen` |
| 8 `N2 2512.50 1.20 P1` + T3, T4, T5 | `N2` `comun.accesorio:tee`, `cota_eje 2512.5`, `demanda_base 1.2` L/s |
| 7 `N1` + T2 (160) y T3 (110) | `N1` `reduccion` |
| 16 `CAPT 2478.00` | `CAPT` `agua.captacion:pozo_profundo` (respuesta del usuario; la curva `CB1` dice «bomba sumergible del pozo»), `datos.carga_hidraulica 2478`, **sin `cota`** |
| 20 `TQ1 2530.00 2.50 0.30 3.20 12.00` | `TQ1` `agua.tanque:apoyado_cilindrico`, `cota_nivel_min 2530.3`, `cota_nivel_max 2533.2`, `diametro 12000`, `nivel_inicial 2.5`; sin volumen |
| 24 `T1 NB TQ1 850 160 140` + 137-138 vértices | `T1` `agua.tuberia:impulsion` (aguas abajo de `B1`, confirmado), DN 160, 4 vértices `[E,N]` (sin soleras: línea sin dato) |
| 29 `T6 … CV` | `T6` con `attrs.tipo_valvula "retencion"` |
| 33 `B1 CAPT NB HEAD CB1` | `B1` `agua.bombeo:booster_en_linea` en la planta de `CAPT`, `desde CAPT`, `hasta NB`, sin cota (el reservorio no tiene): se descarta y se lista |
| 37 `PRV1 N4 N5 90 PRV 25` | `PRV1` `agua.valvula:reduccion_presion` en `N4`, `cota_eje 2505.2`, `desde N4`, `hasta N5`, `consigna_presion 25.0` |

### `epanet_us_gpm.inp` → `epanet_us_gpm.plan.json` (GPM, D-W)

| Línea del `.inp` | Objeto del plan |
|---|---|
| 100 `Units GPM`, 101 `Headloss D-W` | pies, pulgadas, psi, hp; ε en milipies |
| 7 `J20 690.00 150` | `J20` `cota_eje 210.312`, `demanda_base 9.4635` L/s |
| 9 `J40 640.00 80` + 41-42 `[DEMANDS]` 50 y 30 | `J40` `demanda_base_1 3.1545`, `demanda_base_2 1.8927` (la de 80 se sustituye) |
| 14 `LAKE 620.00` | **no está en el plan**: el usuario no dijo qué era; listado |
| 18 `TWR 780 15 5 30 40` | `TWR` `cota 237.744`, `cota_nivel_max 246.888` (780+30 ft), `diametro 12192` (40 ft) |
| 23 `102 J10 J20 2000 8 0.85` + 126 vértice | `102` DN 203 (8 in), `rugosidad_abs 0.2591` mm, `longitud_origen 609.6` |
| 30 `PU1 LAKE J10 POWER 25` | `PU1` `agua.bombeo:booster_en_linea` en la planta de `LAKE`, `potencia_unitaria 18.6425` kW |
| 34 `V1 J30 J50 6 PRV 40` | `V1` en `J30`, `desde J30`, `hasta J50`, `diametro_nominal 152`, `consigna_presion 28.1375` m |
| 35 `V2 J20 J50 4 FCV 100` | `V2` `reguladora_caudal`, `desde J20`, `hasta J50`, `consigna_caudal 6.309` L/s |

## 7. Trampas conocidas

- `row[5]` de `[TANKS]` como nivel máximo (es el diámetro).
- La carga de un `RESERVOIR` como cota de terreno o de fondo.
- `JUNCTION` → cámara de válvulas: un nudo no es una estructura.
- Diámetro de tanque en pies tratado como pulgadas, o el de tubería como pies.
- Olvidar que `[DEMANDS]` sustituye a la demanda de `[JUNCTIONS]` (no se suman).
- `Closed` en `[PIPES]` o `[STATUS]`: el tramo existe; el estado es de la simulación.
- Volumen de tanque «calculado» con diámetro × altura: es cálculo; no se escribe.
