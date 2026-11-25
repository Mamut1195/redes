# 22 — Decisiones tomadas (y las que siguen abiertas)

Registro de lo que se decidió, cuándo y con qué consecuencia en el código. Si una decisión cambia,
se cambia aquí y en el mismo commit que el código.

| # | Fecha | Decisión | Consecuencia |
|---|---|---|---|
| D1 | 2026-11-21 | **Sin DWG.** DXF es suficiente para la ida y vuelta | F8-03 se cierra como «no». `exportar dxf` + `importar dxf` conservan familia, subtipo, código, atributos y datos (capa `REDES_INFO`). Si algún día se necesita DWG, hay que obtener licencia de ODA File Converter |
| D2 | 2026-11-21 | **La revisión no identifica a nadie.** | El journal guarda revisión, comando y fecha UTC. El usuario del sistema solo se anota si se arranca con `REDES_JOURNAL_USUARIO=1`. No hay firma ni identidad legal |
| D3 | 2026-11-21 | **La app no valora cantidades.** | `redes cantidades` emite cantidades con procedencia; los precios son de otra aplicación. La app es gratuita |
| D4 | 2026-11-21 | **Sin cálculo hidráulico, hidrológico ni de tierras.** | Todo número entra como `Dato{valor,unidad,fuente}`; lo que no venga de la app de origen se marca como discrepancia, nunca se calcula |
| D5 | 2026-11-25 | **La ingesta de formatos ajenos es una skill guiada, no adaptadores.** | La app solo importa `redes.plan/1` (validado, sin inventar datos), su propio DXF de ida y vuelta (capa `REDES_INFO`) y terreno. SWMM, EPANET, DXF/GeoJSON de terceros, Excel, etc. los adapta un agente siguiendo `skills/redes-ingesta` (formato, columnas, unidades, trampas, qué no inventar), que escribe el plan; la CLI lo valida. Se retiran `inp_to_plan.py`, `plan_desde_tabla.py` y la heurística de capas de `ingest.rs`. Los fixtures `.inp` y sus planes esperados quedan como ejemplos y pruebas del contrato |
| D6 | 2026-11-25 | **`redes validar` marca discrepancias geométricas; sus umbrales son del usuario.** (F13-01) | Los umbrales por defecto son una referencia típica, no una norma: pendiente sanitario 0,5–15 %, pluvial/canal y combinado 0,3–15 %; salto sin caída 0,75 m; recubrimiento 1,0 m alcantarillado y 0,8 m agua; profundidad de pozo 5,0 m; separación 3,0 m horizontal y 0,30 m vertical con el agua arriba; longitud máx(0,5 m; 1 %). Se cambian con `--reglas` y se guardan en `Project::reglas_validacion` (`--guardar`). Si falta un dato la regla es `no_evaluable` con `falta`; el DN no hace de DI ni se supone espesor. Única excepción, la **cota segura**: si aun sin lo que falta ya se incumple, se informa diciendo qué no se contó. Es un informe: `ok:true` aunque haya hallazgos |
| D7 | 2026-11-25 | **Observaciones con historial; el estado observado se deriva (F13-03).** | Cada objeto guarda una lista de observaciones (id, fecha UTC, revisión, texto, abierta/cerrada, cierre, ubicación opcional) que nunca se sobrescriben. Observado ⇔ alguna abierta; cerrar la última deja el objeto **pendiente**, no aprobado. Aprobar o reabrir con abiertas falla (`E_STATE_OBS_ABIERTAS`) salvo `aprobar --cerrar-todas "texto"`. Autor solo con `REDES_JOURNAL_USUARIO=1` (D2). El esquema sube a `redes.project/3`, sin conversión desde la `nota` única de los `/2` (no hay archivos en producción que conservar) |

## Abiertas

| # | Pregunta | Qué bloquea |
|---|---|---|
| A1 | ¿Qué es un **vertedero** en tus proyectos: punto de descarga, alivio, aliviadero o relleno sanitario? | Si es relleno, hace falta una familia nueva; si es alivio, se modela con `agua.camara:alivio`; si es descarga, ya está en `saneamiento.descarga` |
| A2 | ¿«Pozo» incluye el **pozo profundo de captación**? | Hoy conviven `agua.captacion:pozo_profundo` (captación) y `saneamiento.pozo` (inspección). Está cubierto, pero conviene confirmar que no se confunden en la ingesta |
| A3 | ¿El repositorio de GitHub debe ser **público**? | Con repo privado, Actions no arranca por el límite de facturación de la cuenta; en público, el CI corre gratis |
