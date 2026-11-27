# vendor/rune — copia fijada, no un fork

`vendor/rune` es el **árbol de trabajo de Rune en el commit `78efd4a`** (rama `dev`), copiado dentro
del repositorio. No es un fork: no hay cambios propios en su código.

## Por qué está aquí y no como dependencia de crates.io o de git

1. **Rune no tiene versión estable.** La API que usamos —`Compositor`, `El::effect`, `Shape`,
   las banderas de *tooling*— vive en `dev` y cambia entre commits. Un `rune = "0.x"` no existe
   con esa superficie, y un `git = "…"` flotaría con cada `cargo update`.
2. **Rune trae dependencias parcheadas** en su propio `vendor/` (`wgpu-hal`, `winit`,
   `parley_engine`, `accesskit_windows`, `accesskit_macos`). Compilar Rune contra las versiones de
   crates.io falla o mezcla dos `wgpu-hal`. Por eso nuestro `Cargo.toml` repite la tabla
   `[patch.crates-io]` apuntando a `vendor/rune/vendor/*`: es lo que hace que el grafo de
   dependencias sea el mismo que el de Rune.
3. **Reproducibilidad y compilación sin red.** El árbol versionado compila igual en cualquier
   máquina y el CI no descarga Rune. Es el mismo binario que se probó.

## Coste

- 99 MB y ~3 400 archivos dentro de Git (unos 63 MB empaquetados). Alarga el clon.
- Actualizar Rune es manual: traer un checkout nuevo en otra revisión, repetir la tabla de
  parches, compilar y ejecutar la suite.

## Reglas

- **No se formatea ni se edita** `vendor/rune` (`cargo fmt --all` lo alcanza: usar
  `cargo fmt -p redes-core -p redes-cli -p redes-app`).
- No hay parches propios sobre Rune. Lo único que se «parchea» son las dependencias que Rune ya
  trae corregidas en `vendor/rune/vendor/*` (`wgpu-hal`, `winit`, `parley_engine`,
  `accesskit_windows`, `accesskit_macos`): la tabla `[patch.crates-io]` del `Cargo.toml` raíz
  apunta a esas carpetas y hay que mantenerla igual que la de Rune.
- Si algún día hay que cambiar Rune, **no se toca en su sitio**: se cambia upstream (o en un fork)
  y se trae la revisión nueva a `vendor/rune` (`scripts/traer-rune.ps1` deja el checkout en
  `out/rune-upstream` para comparar), anotando aquí el commit y el motivo.
- Actualizar de revisión = una tarea con ticket propio: cambiar el commit, compilar, ejecutar la
  suite y revisar a mano los modos (foco, corte, explosión, `--gpu3d`).
