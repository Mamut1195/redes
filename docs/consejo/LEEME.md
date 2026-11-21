# Documentos del consejo — alcance anterior

Estos cinco documentos se produjeron cuando el encargo era **diseñar** redes (trazado,
hidráulica, excavación) y no solo verlas. El cliente cambió el alcance a mitad de camino:

> «esta app es únicamente para visualización 3D […] aquí no se va a hacer cálculos hidráulicos,
> hidrológicos, ni nada. La idea es que se traigan los datos desde otras apps, y con una skill
> se convierta los datos de las otras apps en datos que esta app o el CLI de esta app puedan
> usar. […] una vez que esté aprobado, se le pase a DWG para que haga el detallado ahí.»

Por eso el alcance vigente es el de `docs/01-arquitectura.md`, `docs/02-features.md` y
`docs/03-skill-ingesta.md`. Lo que sigue siendo útil de estos documentos:

| Documento | Qué conservar | Qué quedó fuera |
|---|---|---|
| `01-domain.md` | Taxonomía de las 17 familias, atributos con unidades, las cuatro cotas (terreno, rasante, clave, fondo), glosario ES/EN, reglas de nomenclatura | Toda la hidráulica (§5), la normativa (§6), el movimiento de tierras (§7) y las 68 reglas de validación (§9) |
| `02-systems.md` | Capas y dirección de dependencias, familias como **dato** y no como código, topología derivada, `ElementId`, snapshots inmutables, presupuesto de hilos | El modelo columnar por familia (v0.1 usa filas tipadas), SQLite (v0.1 usa JSON), deshacer por inverso |
| `03-cli-ai.md` | El sobre JSON, la taxonomía de códigos, `--dry-run`, `--explain`, `hint_command`, la idea de manifiesto generado del registro | Las 127 acciones, la parte de cálculo y el modo lote atómico |
| `04-rune-ui.md` | Composición de la ventana, la regla «los elementos son el recurso escaso», pan/zoom con `Live`, pruebas headless | El lienzo 2D de planta y el detallado (ahora es una vista 3D) |
| `05-product.md` | Personas, recorridos, formato de backlog, plan de guía, decisiones abiertas para el cliente | Todo lo que dependía de calcular (hidráulica, cómputos) |

La diferencia de nombre también quedó resuelta: el producto es **Redes**, el binario `redes`,
el archivo `*.redes` y las variables `REDES_*`.
