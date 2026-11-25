---
name: redes-ingesta
description: Convierte datos de otra aplicación (Civil 3D/DXF, SWMM, EPANET, Excel/CSV, SIG/GeoJSON) al contrato `redes.plan/1` que consumen la CLI `redes` y el visor 3D. Úsala cuando haya que meter un modelo existente en Redes, o cuando un archivo no entre y haya que averiguar por qué. No la uses para calcular hidráulica, hidrología ni movimientos de tierra: Redes no calcula.
---

# Skill: ingesta de modelos en Redes

## Qué hace

Traduce un modelo ya diseñado en otra herramienta al archivo que Redes entiende, **sin
recalcular nada**. La salida es un JSON con el esquema `redes.plan/1`; después:

```powershell
redes importar plan --archivo plan.json
redes vista --salida vista.svg        # comprobar que se ve
redes objeto lista                    # comprobar que están todos
```

## Cuándo usarla

- Hay que ver en 3D una red que vive en Civil 3D, SWMM, EPANET, Excel o un SIG.
- Un DXF entra incompleto y hay que mapear capas o atributos.
- Hay que reimportar una versión corregida sin perder la revisión hecha.

## Cuándo NO usarla

- Para calcular caudales, presiones, volúmenes de excavación o cumplimiento normativo.
  Esos números **vienen de la app de origen** y se copian como `datos`.
- Para editar el diseño. Se corrige en la app de origen y se reimporta.

## Qué formato entra por script y cuál por la CLI

No todo pasa por el script: DXF, GeoJSON y terreno los lee la propia CLI, que entiende el formato
mejor que cualquier conversión intermedia. Usa la vía más corta.

| Lo que llega | Vía | Comando |
|---|---|---|
| SWMM o EPANET (`.inp`) | **script** `inp_to_plan.py` | `python scripts/inp_to_plan.py red.inp --tipo swmm --salida plan.json` y luego `redes importar plan --archivo plan.json` |
| Hoja de cálculo con pozos/tramos (CSV/TSV) | **script** `plan_desde_tabla.py` | `python scripts/plan_desde_tabla.py pozos.csv --tipo pozos --salida plan.json` |
| DXF de Civil 3D u otro CAD | **CLI directa** | `redes importar dxf --archivo red.dxf` |
| SIG (GeoJSON) con propiedades | **CLI directa** | `redes importar geojson --archivo red.geojson` |
| Nube de puntos del terreno (CSV E,N,Z) | **CLI directa** | `redes importar terreno --archivo terreno.csv` |

Reglas para elegir:

1. Si el archivo ya trae familia, código y atributos, pásalo por la **CLI**: no hay nada que mapear
   y una conversión intermedia solo puede perder datos.
2. Si el archivo es de una app que no conoce a Redes (SWMM, EPANET, una hoja de cálculo), usa el
   **script**: ahí es donde se decide qué familia y qué subtipo le toca a cada objeto.
3. Si el DXF viene de otra aplicación y las capas no se llaman como las de Redes, primero prueba la
   CLI y mira `descartados`; si el mapeo no sirve, escribe un `plan.json` a mano con
   `plan_desde_tabla.py` como plantilla.

## Tabla de decisión de familia y subtipo

El registro completo (46 familias, 315 subtipos) se consulta con `redes capacidades`. Para no
inventar claves:

```powershell
redes capacidades > capacidades.json      # todas las familias, subtipos y atributos
```

Si una clave del origen no existe, la importación la traduce si está entre las retiradas (por
ejemplo `agua.captacion:toma_rio` → `agua.bocatoma:barraje_fijo`) y lo informa en `renombrados`.
Lo que no se puede traducir se descarta y aparece en `descartados`: nunca se pierde en silencio.

## El contrato `redes.plan/1`

```json
{
  "schema": "redes.plan/1",
  "proyecto": {
    "nombre": "Alcantarillado La Esperanza",
    "crs": "EPSG:32717",
    "fuente_app": "Civil 3D",
    "fuente_archivo": "red-la-esperanza.dxf"
  },
  "terreno": [[760000.0, 9796000.0, 2512.34], [760010.0, 9796000.0, 2512.10]],
  "objetos": [
    {
      "tipo": "nodo",
      "codigo": "PZ-001",
      "familia": "saneamiento.pozo",
      "subtipo": "inspeccion",
      "e": 760060.0,
      "n": 9796090.0,
      "cota": 2510.92,
      "cota_terreno": 2512.52,
      "diametro": 1.2,
      "attrs": { "material": "hormigon" },
      "datos": {
        "caudal_diseno": { "valor": 12.0, "unidad": "L/s", "fuente": "SWMM" }
      }
    },
    {
      "tipo": "arco",
      "codigo": "CL-001",
      "familia": "saneamiento.tuberia",
      "subtipo": "colector",
      "desde": "PZ-001",
      "hasta": "PZ-002",
      "diametro_mm": 250,
      "vertices": [[760060.0, 9796090.0, 2510.92], [760125.0, 9796091.0, 2510.60]],
      "attrs": { "material": "PVC", "rugosidad_n": 0.013 },
      "datos": {
        "volumen_excavacion": { "valor": 118.4, "unidad": "m³", "fuente": "Civil 3D" }
      }
    }
  ]
}
```

### Campos

| Campo | Obligatorio | Regla |
|---|---|---|
| `schema` | sí | `redes.plan/1` |
| `proyecto.crs` | sí | EPSG del proyecto; Ecuador continental: `32717` (17S) o `32718` (18S). **`32617` es 17N y desplaza 10 000 km** |
| `terreno[]` | no | Puntos `[E, N, Z]`; producen el TIN |
| `objetos[].tipo` | sí | `nodo` o `arco` |
| `familia` | sí | Clave de `redes esquema` (p. ej. `saneamiento.pozo`) |
| `subtipo` | no | Si falta se usa el primero de la familia |
| `codigo` | no | Si falta se asigna `PZ-001`, `CL-001`… |
| `e`, `n` | sí (nodo) | Coordenadas del proyecto en metros |
| `cota` | no | Solera del nodo (fondo interior), nunca la clave; si falta y hay TIN se usa `z_terreno − 1.5` |
| `cota_terreno` | no | Si falta, se interpola del TIN |
| `diametro` | no | Metros (interior del pozo) |
| `diametro_mm` | no | Milímetros (tubería), por defecto 200 |
| `vertices` | no | Si falta, el arco es recto entre las cotas de sus nodos |
| `attrs` | no | Atributos de la familia con su clave exacta |
| `datos` | no | Números del origen: `{valor, unidad, fuente}`. **Nunca se recalculan** |

## Procedimiento

1. **Identificar la fuente.** ¿DXF? ¿Texto de SWMM/EPANET? ¿Excel? ¿GeoJSON?
2. **Extraer la geometría**: planta (E, N) y cotas. Sin planta no hay 3D.
3. **Mapear familias y subtipos** con la tabla de `references/mapeos.md`. Ante la duda,
   la familia genérica (`saneamiento.pozo`, `saneamiento.tuberia`) y se anota.
4. **Normalizar unidades**: longitudes a metros, diámetros a mm, caudales a L/s, cotas a metros.
   Convertir es aritmética de unidades, no cálculo de ingeniería.
5. **Copiar los resultados del origen** a `datos` con su `fuente`. Si el origen no trae un
   número, **no se inventa**: se omite.
6. **Emitir el plan** y validarlo:
   ```powershell
   python skills/redes-ingesta/scripts/inp_to_plan.py --tipo swmm red.inp --salida plan.json --coords coords.csv
   redes importar plan --archivo plan.json --dry-run
   redes importar plan --archivo plan.json
   ```
7. **Verificar**: `redes proyecto info` (conteos), `redes objeto lista` (códigos), y
   `redes vista --salida vista.svg` (que se vea).
8. **Informar**: cuántos entraron, cuántos se descartaron y por qué.

## Atajos ya implementados

| Fuente | Camino |
|---|---|
| DXF (puntos, líneas, polilíneas 3D, círculos) | `redes importar dxf --archivo x.dxf --tolerancia 1.0` (mapeo por nombre de capa) |
| GeoJSON | `redes importar geojson --archivo x.geojson` (propiedades `familia`, `codigo`, `desde`, `hasta`) |
| Puntos de terreno | `redes importar terreno --archivo puntos.csv --epsg 32717` |
| SWMM / EPANET `.inp` | `scripts/inp_to_plan.py` → `redes importar plan` |
| Excel / CSV de inventario | exportar a CSV `codigo,E,N,Z,tipo,diametro` y mapear con un plan JSON |

## Errores típicos y qué hacer

| Síntoma | Causa | Arreglo |
|---|---|---|
| `E_TOPO_ORPHAN_LINK` | El arco referencia un código que no está en `objetos` | Añadir el nodo o corregir `desde`/`hasta` |
| `E_STATE_DUPLICATE_NAME` | Dos objetos con el mismo `codigo` | Renombrar; los códigos son únicos en el proyecto |
| Todo apilado en el origen | El `.inp` no trae coordenadas | Pasar `--coords codigo,E,N` |
| El proyecto aparece a 10 000 km | CRS del hemisferio equivocado | Usar `32717`/`32718`, nunca `32617` |
| El pozo no tiene altura | Sólo llegó la solera, sin tapa | Añadir `cota_terreno` o traer el TIN |
| El inspector muestra `—` en un dato | El origen no traía ese número | Es correcto: no se inventa |

## Reglas de oro

1. **No calcular.** Copiar, convertir unidades, ordenar. Nunca derivar un número de diseño.
2. **Procedencia obligatoria.** Todo `dato` lleva `fuente`; si se desconoce, `"desconocida"`.
3. **Códigos estables.** El mismo objeto conserva su código entre importaciones.
4. **Descartes con motivo.** Lo que no se entiende se informa, no se adivina.
5. **Un plan, un origen.** Varias fuentes se importan en pasos separados; el proyecto acumula
   `fuentes`.
