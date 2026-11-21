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
- Si algún día hay que cambiar Rune, **no se toca en su sitio**: se crea un fork, se mantiene el
  parche en `vendor/parches/*.patch` con un script que lo aplique, y se documenta aquí el motivo,
  el commit base y cómo revertirlo.
- Actualizar de revisión = una tarea con ticket propio: cambiar el commit, compilar, ejecutar la
  suite y revisar a mano los modos (foco, corte, explosión, `--gpu3d`).
