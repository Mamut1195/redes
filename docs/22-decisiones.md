# 22 — Decisiones tomadas (y las que siguen abiertas)

Registro de lo que se decidió, cuándo y con qué consecuencia en el código. Si una decisión cambia,
se cambia aquí y en el mismo commit que el código.

| # | Fecha | Decisión | Consecuencia |
|---|---|---|---|
| D1 | 2026-11-21 | **Sin DWG.** DXF es suficiente para la ida y vuelta | F8-03 se cierra como «no». `exportar dxf` + `importar dxf` conservan familia, subtipo, código, atributos y datos (capa `REDES_INFO`). Si algún día se necesita DWG, hay que obtener licencia de ODA File Converter |
| D2 | 2026-11-21 | **La revisión no identifica a nadie.** | El journal guarda revisión, comando y fecha UTC. El usuario del sistema solo se anota si se arranca con `REDES_JOURNAL_USUARIO=1`. No hay firma ni identidad legal |
| D3 | 2026-11-21 | **La app no valora cantidades.** | `redes cantidades` emite cantidades con procedencia; los precios son de otra aplicación. La app es gratuita |
| D4 | 2026-11-21 | **Sin cálculo hidráulico, hidrológico ni de tierras.** | Todo número entra como `Dato{valor,unidad,fuente}`; lo que no venga de la app de origen se marca como discrepancia, nunca se calcula |

## Abiertas

| # | Pregunta | Qué bloquea |
|---|---|---|
| A1 | ¿Qué es un **vertedero** en tus proyectos: punto de descarga, alivio, aliviadero o relleno sanitario? | Si es relleno, hace falta una familia nueva; si es alivio, se modela con `agua.camara:alivio`; si es descarga, ya está en `saneamiento.descarga` |
| A2 | ¿«Pozo» incluye el **pozo profundo de captación**? | Hoy conviven `agua.captacion:pozo_profundo` (captación) y `saneamiento.pozo` (inspección). Está cubierto, pero conviene confirmar que no se confunden en la ingesta |
| A3 | ¿El repositorio de GitHub debe ser **público**? | Con repo privado, Actions no arranca por el límite de facturación de la cuenta; en público, el CI corre gratis |
