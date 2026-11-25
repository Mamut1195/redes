---
name: redes-ingesta
description: Convierte una red de agua o alcantarillado diseñada en otra aplicación (EPA SWMM o EPANET .inp, DXF de Civil 3D/AutoCAD, LandXML, GeoJSON/Shapefile de un SIG, Excel/CSV de diseño o informes de Civil 3D) en un plan `redes.plan/1` para la CLI `redes` y el visor 3D. Úsala cuando haya que meter un modelo existente en Redes o averiguar por qué un archivo no entra. El agente lee el origen, lo mapea con las referencias de formato, escribe el plan y lo valida con la CLI. No calcula hidráulica, hidrología ni movimiento de tierras.
---

# Ingesta guiada en Redes

Tú (el agente) haces la conversión. No hay adaptadores por programa: lees el archivo de origen,
entiendes su formato con la referencia correspondiente, escribes un `redes.plan/1` y la CLI lo
valida (decisión D5, `docs/22-decisiones.md`).

## Cuándo usarla

- Hay que ver en 3D una red que vive en SWMM, EPANET, Civil 3D, un SIG o una hoja de cálculo.
- Un archivo de otra aplicación no entra, o entra incompleto, y hay que saber por qué.
- Hay que reimportar una versión corregida del origen.

No la uses para calcular caudales, presiones, volúmenes ni cumplimiento normativo, ni para
corregir el diseño: eso es de la aplicación de origen.

**Sin skill:** un DXF escrito por Redes (capa `REDES_INFO`) entra con `redes importar dxf`, y los
puntos de terreno `E,N,Z` con `redes importar terreno --archivo t.csv --epsg 32717`.

## Flujo

1. **Identifica el formato** por la extensión y las primeras líneas (`[TITLE]`/`[JUNCTIONS]` con
   `[CONDUITS]` → SWMM; con `[PIPES]` → EPANET; `0\nSECTION` → DXF; `"FeatureCollection"` →
   GeoJSON; `<LandXML` → LandXML).
2. **Lee la referencia** de ese formato (tabla de abajo) y `references/plan.md` (el contrato).
   Lee el archivo de origen entero, sección por sección; si es grande, pártelo con un script de
   usar y tirar que **solo lea**. El mapeo lo decides tú con la referencia.
3. **Consulta el registro** de familias: `redes esquema --familia <clave> --pretty`. Usa solo
   familias, subtipos y atributos que existan.
4. **Pregunta lo ambiguo antes de escribir** (ver «Preguntar»). Agrupa las preguntas en un solo
   mensaje con tu propuesta para cada una.
5. **Escribe el plan** (`plan.json`) y, a la vez, la lista de ausentes y descartes.
6. **Valida** en un proyecto de prueba nuevo (`redes proyecto nuevo --nombre prueba --salida
   out\prueba.redes`): primero `redes -p out\prueba.redes --dry-run importar plan --archivo
   plan.json` y luego sin `--dry-run`. El `--dry-run` valida e importa en una copia en memoria
   y da el mismo informe que la importación real, sin escribir (`plan.md` §6).
7. **Corrige según los diagnósticos** (`detalle_descartes`, `detalle_avisos`, `renombrados`,
   `E_FMT_CAMPO` con el campo válido sugerido, `E_FMT_PARSE`) y repite
   hasta que lo único descartado sea lo que tú decidiste descartar.
8. **Importa** en el proyecto real del usuario (proyecto nuevo o sin esos códigos: los repetidos se
   descartan).
9. **Revisa:** `redes -p <proyecto> objeto lista` (cuenta nodos y arcos frente al origen) y
   `redes -p <proyecto> vista --salida out\vista.svg` (míralo: saltos de cota, nodos sueltos,
   tramos que cruzan la ciudad = CRS o unidades mal).
10. **Entrega el informe** al usuario (ver «Informe»).

## Referencias

| Origen | Lee |
|---|---|
| Contrato, cotas (solera, clave, rasante, tapa, recubrimiento), validación | `references/plan.md` |
| EPA SWMM 5 `.inp` | `references/swmm.md` |
| EPANET 2.2 `.inp` | `references/epanet.md` |
| DXF de Civil 3D / AutoCAD, LandXML | `references/dxf-civil3d.md` |
| GeoJSON, Shapefile, GeoPackage | `references/geojson-sig.md` |
| Excel / CSV de diseño, informes de Civil 3D | `references/tablas.md` |

Ejemplos trabajados (origen y plan, línea a línea en `swmm.md` §6 y `epanet.md` §6):
`crates/redes-core/tests/fixtures/{swmm_si_cms,swmm_us_cfs,epanet_si_lps,epanet_us_gpm}.inp` y
sus `.plan.json`.

## Reglas duras

1. **Nunca inventes un dato.** Lo que el origen no trae queda **ausente** en el plan (no pongas
   la clave) y se lista en el informe. Nada de «valores razonables»: ni terreno = solera + 1,5,
   ni DN 200, ni volumen de tanque, ni coordenadas (0,0), ni Z = 0.
2. **Conserva la procedencia.** `proyecto.fuente_app` y `fuente_archivo` siempre; cada número en
   `datos` lleva `"fuente": "<App> (<archivo>)"`. No mezcles en un plan datos de dos orígenes
   sin decirlo en la `fuente`.
3. **Todo a SI**: m, mm (diámetros de tubo), L/s, m.c.a., kW. Lee la unidad del origen
   (`FLOW_UNITS`, `Units`, `$INSUNITS`, fila de unidades de la hoja); si no la declara, pregunta.
   Las coordenadas no se convierten: se exige el CRS correcto.
4. **Solera ≠ clave.** solera = clave − DI solo con DI declarado; nunca al revés
   (`plan.md` §4).
5. **Pregunta en vez de adivinar** (ver abajo). Si el usuario no sabe, elige la opción más
   conservadora y dilo en el informe como supuesto suyo, no tuyo.
6. **No calcules hidráulica** ni nada derivado: volúmenes, pendientes, caudales, presiones,
   recubrimientos. Las conversiones de unidad y de referencia (clave → solera con DI) no son
   cálculo; dilas igualmente. La Z de los vértices no la repartas tú: escríbelos `[E,N]` y Redes
   la interpola y la marca como derivada (`plan.md` §2.2).
7. **No pierdas objetos en silencio.** Cada objeto del origen acaba en el plan o en la lista de
   descartes con su motivo. Cuenta: objetos del origen = plan + descartados.
8. **Solo campos del contrato.** El importador ignora campos desconocidos sin avisar.

## Preguntar (no adivinar)

- **CRS** si el origen no lo declara (Ecuador: `EPSG:32717` 17S o `EPSG:32718` 18S).
- **Sistema**: pluvial, sanitario o combinado, si el archivo no lo deja claro.
- **Subtipo por defecto** de cada grupo: tipo de tubería en EPANET (conducción, distribución,
  impulsión), de tanque (apoyado, elevado…), de descarga, de válvula sin equivalente.
- **Qué es** un `RESERVOIR`, una `JUNCTION` de SWMM sin profundidad, una capa `CAMARA`, un
  `[EMITTERS]`.
- **Unidades dudosas**: diámetro `8` (¿pulgadas?), pendiente `1,5` (¿%?), `CT` (¿terreno o tapa?).
- **Si el diámetro del origen es DN o interior.**
- **Si se importa lo existente**, lo proyectado o ambos.

## Informe al usuario

Al terminar, en un mensaje corto:

- origen, CRS y unidades leídas; supuestos confirmados por el usuario;
- conteo: objetos del origen, nodos y arcos importados, descartados;
- **descartados** con motivo (del plan y de la CLI);
- **ausentes**: qué dato falta en qué objetos (p. ej. «terreno ausente en J3, MH3, PJ1»). Hasta
  que el importador deje de rellenar huecos, advierte que el visor puede mostrar un valor
  provisional donde dice ausente (`plan.md` §3);
- conversiones hechas (US → SI, clave → solera); vértices sin Z (`[E,N]`);
- la vista SVG generada.
