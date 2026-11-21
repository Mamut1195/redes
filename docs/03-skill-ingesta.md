# 03 — La skill de ingesta

> La skill vive en `skills/redes-ingesta/` (`SKILL.md` + `references/mapeos.md` +
> `scripts/inp_to_plan.py`). Este documento explica **por qué** existe y **cómo** encaja en el
> producto. Para el procedimiento paso a paso, abrir `SKILL.md`.

---

## 1. El problema que resuelve

Redes no diseña: recibe. Pero cada aplicación exporta a su manera:

| Origen | Lo que da | Lo que le falta |
|---|---|---|
| Civil 3D | DXF con capas, bloques, *proxies* AECC | Nada, si el delineante explota los *proxies* |
| SWMM | `.inp` con topología y cotas | Planta (no guarda coordenadas) |
| EPANET | `.inp` con topología y cotas | Planta |
| Excel | Inventario tabulado | Geometría 3D |
| SIG | GeoJSON con planta | Cotas y jerarquía de familias |

Sin una capa de conversión, cada proyecto acaba con un script distinto, escrito a mano, que
nadie documenta y que se rompe en la siguiente entrega. La skill es esa capa, pero **escrita una
vez, versionada y verificable**.

## 2. La decisión: un contrato, no un conversor monolítico

```
   DXF ────────────────►┐
   SWMM ──(script)─────►│
   EPANET ─(script)────►├──► redes.plan/1  ──► CLI `redes importar plan`
   Excel ──(plantilla)─►│        (JSON)            │
   GeoJSON ────────────►┘                          ▼
                                            documento `*.redes`
```

- **Un solo formato de entrada** (`redes.plan/1`) para todos los orígenes.
- La conversión de cada origen vive **fuera del binario**: un script, una plantilla o una
  persona con un LLM, pero siempre produciendo el mismo contrato.
- El contrato se valida al importar: familia conocida, subtipo existente, códigos únicos,
  extremos existentes, coordenadas presentes.

**Consecuencias buenas:**

- Un origen nuevo no exige recompilar nada.
- Un agente puede escribir el JSON directamente, sin ejecutar código nuestro.
- Los errores son del archivo, no del motor: el importador dice qué objeto y por qué.
- El contrato se puede versionar (`redes.plan/2`) sin tocar el modelo.

**Consecuencia mala (asumida):** el usuario necesita un paso intermedio. Se mitiga con los
atajos ya implementados (`importar dxf|geojson|terreno`) y con el script de SWMM/EPANET.

## 3. Qué garantiza el importador

| Garantía | Cómo |
|---|---|
| No inventa datos | Un campo ausente queda ausente; el inspector muestra `—` |
| No calcula | Los números del origen se copian a `Dato { valor, unidad, fuente }` |
| No pierde objetos en silencio | Informe con `nodos`, `arcos`, `descartados` y el motivo de cada descarte |
| No duplica | Código único por proyecto; conflicto → `E_STATE_DUPLICATE_NAME` |
| No acepta modelos rotos | Arco sin extremos → `E_TOPO_ORPHAN_LINK`; mismo nodo en ambos extremos → `E_TOPO_SELF_LOOP` |
| Deja rastro | `fuentes[]` con app, archivo, formato y fecha |

## 4. Los tres niveles de la skill

1. **Nivel 1 — atajo.** El origen ya está en un formato que el binario entiende:
   `redes importar dxf --archivo x.dxf`. Cero código.
2. **Nivel 2 — script.** El origen es texto estructurado (`.inp`, CSV raro, export de Excel):
   se usa `scripts/inp_to_plan.py` o se adapta en 30 líneas.
3. **Nivel 3 — agente.** Un LLM lee el archivo, aplica `references/mapeos.md` y escribe el JSON.
   Es el camino para formatos que aparecen una sola vez (un inventario en PDF, un DWG con
   bloques raros).

Los tres producen el mismo contrato y se validan igual. Eso es lo importante.

## 5. Cómo se prueba

| Prueba | Qué verifica |
|---|---|
| `cargo test -p redes-core` | El importador de plan rechaza duplicados y huérfanos; el DXF de ida y vuelta conserva los 44 objetos |
| `python skills/…/inp_to_plan.py` + `redes importar plan` | Un `.inp` de SWMM se convierte y entra sin descartes |
| `redes exportar dxf` → `redes importar dxf` | La geometría sobrevive el viaje de vuelta |
| `redes vista --salida x.svg` | Lo que entró se ve (una imagen que un agente puede mirar) |

## 6. Lo que la skill **no** hará nunca

- Rellenar caudales, presiones o volúmenes «faltantes».
- Convertir un modelo hidráulico en un diseño.
- Decidir si un diámetro cumple norma.
- Inventar coordenadas: si el origen no trae planta, se piden.

Si un proyecto no se puede representar sin calcular algo, es que el origen no traía el dato y
hay que volver a la aplicación que lo produce.
