# CLAUDE.md — Redes (visor 3D de redes de agua potable y alcantarillado)

## Qué es
- App de escritorio Rust + Rune (`redes-app`) y CLI `redes` (sobre JSON `redes.envelope/1`, pensada para agentes) para **ver, revisar y aprobar en 3D** redes ya diseñadas en otras apps (Civil 3D, EPANET, SWMM, Excel, SIG) y devolverlas al CAD en DXF.
- Entrada: `redes.plan/1` (lo escribe un agente con la skill `skills/redes-ingesta`, D5), el DXF propio de ida y vuelta (capa `REDES_INFO`) y terreno. Salida: DXF, GeoJSON, glTF, CSV de cantidades.
- Windows 10/11 x64 con DX12. Idioma del producto, del código y de los docs: español.

## Límites duros (no los cruces ni "ayudando")
- NO calcula hidráulica, hidrología, movimiento de tierras ni presupuesto. NO verifica normativa. NO edita el diseño.
- Todo número es `Dato{valor,unidad,fuente}` con procedencia. Si falta, se muestra `—`; nunca se inventa. Lo que no cuadra se marca como discrepancia, no se corrige.
- Sin DWG (D1: solo DXF). La revisión no identifica a nadie (D2). La app no valora cantidades (D3).
- Antes de proponer una función, revisa docs/01-arquitectura.md §1 y docs/22-decisiones.md.

## Mapa del repo
- `crates/redes-core`: motor headless, SIN UI y SIN cálculo de ingeniería.
  - `model` (objetos, `Dato`), `families`/`families_extra`/`parts`/`parts_cadena` (registro de familias y subtipos + geometría paramétrica), `topology`, `terrain` (TIN), `geom`, `view` (vista, pintor, corte, etiquetas), `ingest` (plan/DXF/GeoJSON/terreno), `export` (DXF, GeoJSON), `gltf`, `biblioteca` (modelos .glb), `cantidades`, `project_io` (`*.redes`), `envelope`, `city`, `demo*` (proyectos de demostración).
- `crates/redes-cli`: binario `redes`. `src/main.rs` concentra todos los comandos (clap); `src/svg.rs` genera la vista SVG.
- `crates/redes-app`: visor Rune. `main.rs` (ventana, inspector, revisión), `gpu3d.rs` (pase 3D con profundidad, `--gpu3d`), `modelos.rs` (caché de mallas, proxy).
- `skills/redes-ingesta/`: skill guiada (D5): `SKILL.md` + `references/` por formato (plan, SWMM, EPANET, DXF Civil 3D, SIG, tablas). Sin scripts: NO añadas adaptadores por programa ni en Python ni en Rust; mejora las referencias y el contrato del plan.
- `docs/`: documentación numerada `NN-nombre.md`. `docs/consejo/` es contexto histórico (el alcance con cálculos quedó descartado), NO el alcance vigente.
- `scripts/`: `mutar.ps1`, `empaquetar.ps1`, `traer-rune.ps1`.
- `vendor/rune`: copia fijada de Rune. `demo/`: proyectos de ejemplo. `out/`: salida generada (ignorada).

## Comandos (PowerShell; los mismos que usa el CI)
- Formato: `cargo fmt -p redes-core -p redes-cli -p redes-app --check` (NUNCA `cargo fmt --all`: alcanza vendor/rune).
- Clippy: `cargo clippy -p redes-core -p redes-cli -p redes-app --all-targets -- -D clippy::correctness`
- Pruebas: `cargo test --workspace`
- Release: `cargo build --release -p redes-app -p redes-cli`
- Mutación: `powershell -NoProfile -File scripts/mutar.ps1`. Debe salir con 0: si sobrevive una mutación, la suite está floja.
- Paquete portable: `powershell -File scripts/empaquetar.ps1` (`-SinCompilar` para no recompilar).
- Proyecto demo: `cargo run -p redes-cli -- proyecto demo --salida out\la-esperanza.redes` (también `ciudad`, `catalogo`, `cadena`).
- Abrir la app: `$env:REDES_PROJECT = "out\x.redes"; cargo run -p redes-app --release` (añade `--gpu3d` para el pase con profundidad).
- Verificar la UI sin ventana: `cargo run -p redes-app -- --snap out\vista.png --scene iso`; `--tree out\arbol.json`.
- CLI: `redes esquema` lista los comandos y los códigos de error. Opciones globales: `-p/--project`, `--pretty`, `--dry-run`, `--quiet`.
- Variables de entorno: `REDES_PROJECT`, `REDES_GPU`, `REDES_INFO`, `REDES_JOURNAL_USUARIO`.

## Convenciones
- Comandos de la CLI, mensajes y docs en español. Los identificadores en español que ya existen se migran a inglés poco a poco (ver «Idioma del código»).
- Commits en español, en una línea y descriptivos, con el código de fase entre paréntesis cuando aplique (`Proxy de malla por rejilla (F6-05)`). Los de cierre de sesión empiezan con `Traspaso:`.
- Definición de terminado (docs/18):
  - una prueba que falle sin el cambio;
  - test y build release en verde;
  - doc actualizado en el mismo commit;
  - captura `--snap` si toca la UI.
- Decisiones en `docs/22-decisiones.md` (D# decididas, A# abiertas): se editan en el mismo commit que el código.
- Estado y traspaso en `docs/23-estado-y-pendientes.md`; backlog por fase en `docs/18-backlog.md`; cifras vigentes en `docs/21-cifras-medidas.md`. Actualízalos al cerrar trabajo.
- Idioma del código: **inglés** para el código nuevo (identificadores, comentarios, nombres de pruebas), porque el repo es MIT y público. La interfaz, los mensajes, los comandos de la CLI, los códigos `E_*` y los docs siguen en español (contrato con el usuario). Migración gradual: al tocar un módulo, renombra en inglés lo que toques; no hagas renombrados masivos fuera de F15.
- Código que se explica solo: nombres que cuenten la historia (funciones cortas con verbo, tipos con el concepto del dominio) en vez de comentarios que repiten lo que hace el código. Un comentario solo dice el *porqué* que el código no puede decir (una decisión D#, un bug ya corregido, un límite externo). Si necesitas comentar un bloque, extráelo a una función con ese nombre. Al tocar un archivo, borra los comentarios que sobran.
- Contrato de la CLI: `ok:false` ⇔ código de salida ≠ 0. Las pruebas de contrato usan el binario real.

## Reglas técnicas
- Coordenadas UTM en `f64` absoluto con `origin` local. El render resta el origen en `f64` y solo después baja a `f32`. Construir geometría en `f32` con UTM rompió los tubos (docs/15): no lo repitas.
- No reintroducir bugs ya corregidos: NaN al guardar, guardado no atómico, pánico del TIN.
- `vendor/rune`: NO editar ni formatear. Los cambios a Rune van como `vendor/parches/*.patch` documentados en `vendor/LEEME.md`. Mantén la tabla `[patch.crates-io]` del `Cargo.toml` raíz.
- `vendor/rune` está fuera del workspace: no lo metas en comandos de cargo del producto.
- Edition 2024. El CI solo exige clippy `correctness`, pero no añadas avisos nuevos.

## Herramientas para agentes (Codegraph y Archify)
- **Codegraph** indexa `crates/` (sin `vendor/`) en `.codegraph/` (ignorado por git). Si falta: `codegraph init .`; tras cambios grandes: `codegraph sync .`.
  - Úsalo ANTES de grep+Read para entender código: herramienta MCP `codegraph_explore` con `projectPath` = raíz del repo, o CLI `codegraph explore|callers|callees|impact <símbolo> .`.
  - Antes de cambiar una función pública de `redes-core`, mira su radio de impacto con `codegraph impact <símbolo> .`.
- **Archify** (skill `archify`): diagramas de arquitectura/flujo respaldados por el código y "Code Analysis" de acoplamiento. Salidas en `out/auditoria/` (ignorado). Usa `archify-pr` para explicar ramas o PRs con cambios de arquitectura.
- Subagentes: dales en el prompt estas dos herramientas y la ruta del repo; sus worktrees viven en `.claude/worktrees/` y se borran al fusionar.

## Antes de cambiar algo, mira
- Alcance: docs/01-arquitectura.md, docs/22-decisiones.md. Qué falta: docs/23 y docs/18.
- Uso de la CLI y la app: docs/04-guia.md. Ingesta: docs/03-skill-ingesta.md.
- Familias: docs/20-familias-captacion-tratamiento.md (muy grande: busca con grep, no lo leas entero) y `families*.rs`.
- Modelos .glb: docs/17. 3D/glTF: docs/16. Modos de inspección: docs/11. Rune: docs/00 y `vendor/LEEME.md`.
