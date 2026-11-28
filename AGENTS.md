# AGENTS.md

Guía para agentes de código (Codex, Cursor, Claude Code y otros) que trabajan en este repo.

**La guía completa y vigente es [`CLAUDE.md`](CLAUDE.md).** Léela entera antes de tocar nada: alcance,
límites duros, mapa del repo, comandos del CI, convenciones y reglas técnicas. Este archivo solo
resume lo imprescindible y no la sustituye; si se contradicen, manda `CLAUDE.md`.

## Lo imprescindible

- Redes es un visor y revisor 3D de redes de agua y alcantarillado. **No calcula, no verifica
  normativa y no edita el diseño** (docs/01 §1, docs/22 D4 y D12).
- Código nuevo en inglés y que se explique solo; interfaz, CLI y docs en español (D13).
- Las piezas se dibujan con familias paramétricas de Blender (D14). Se hornean sin ventana con un
  script versionado en `tools/blender/`, no por un MCP (D15):
  `blender -b --factory-startup --python-exit-code 1 --python tools/blender/<script>.py -- <params.json>`.
  En un proyecto: `redes biblioteca hornear --receta tools/blender/X.receta.json` (lote, caché por clave;
  un dato que falta no se rellena, D16).
- Orden de fases y frentes transversales: `docs/18-backlog.md` §Orden de fases vigente. Estado y
  traspaso: `docs/23-estado-y-pendientes.md`. Decisiones: `docs/22-decisiones.md`.
- Antes de leer código, usa **Codegraph** (`codegraph explore|callers|impact <símbolo> .`; si falta el
  índice, `codegraph init .`). Para diagramas y acoplamiento, **Archify** (salidas en `out/auditoria/`).
- Un ticket está terminado cuando tiene una prueba que falla sin el cambio, `cargo test --workspace`,
  clippy, fmt, el build release y `scripts/mutar.ps1` en verde, y los docs actualizados en el mismo commit.
- Nunca `cargo fmt --all` ni editar `vendor/rune`.
