# 21 — Cifras medidas (y cómo reproducirlas)

> Este documento manda sobre cualquier número que aparezca en los informes anteriores
> (`docs/10` a `docs/17`). Aquellas páginas se conservan como historia del proyecto; varias de sus
> cifras se escribieron antes de que el código cambiara y ya no son ciertas.
>
> Medición: **2026-11-21**, binarios `--release`, `cargo test --workspace` en el mismo árbol.

## 1. Registro de familias

| Concepto | Cifra | Cómo se comprueba |
|---|---:|---|
| Familias | **46** | `redes capacidades` (campo `data.familias`) |
| Subtipos | **315** | suma de `subtipos` por familia |
| Atributos declarados | > 400 | `redes capacidades` |
| Prueba que lo fija | — | `capacidades_publica_el_registro_completo` (46 y 315 exactos) |

El registro se compone de 29 familias del consejo original (216 subtipos) más 17 familias de la
cadena de agua cruda con 95 subtipos, 17 subtipos nuevos en familias existentes y 13 retirados (sin
alias): `216 + 95 + 17 − 13 = 315`.

## 2. Salidas glTF

| Proyecto | Triángulos | Bytes | Primitivas | Dimensiones |
|---|---:|---:|---:|---|
| Ciudad de demostración | **31 710** | 3 439 708 | 20 | 900 × 696 × 71,8 m |
| Catálogo de piezas | **7 682** | 852 132 | 30 | 120 × 48 × 11,5 m |

Reproducir:

```powershell
redes -p demo/ciudad-san-bartolo.redes exportar gltf --salida out/ciudad.glb
redes biblioteca cantidades --glb out/ciudad.glb
```

Ojo: los bytes del `.glb` y los triángulos que reporta `biblioteca cantidades` incluyen el terreno
y los edificios cuando el proyecto los tiene. Las cifras de los informes antiguos («26 810
triángulos», «1,1 MB») corresponden a una versión anterior del exportador.

## 3. Demostraciones

| Proyecto | Objetos | Arcos | Red (m) | Familias distintas |
|---|---:|---:|---:|---:|
| `demo/ciudad-san-bartolo.redes` | 43 | 37 | — | 8 |
| `demo/cadena-completa.redes` | **90** | **72** | **961,4** | 31 |
| `demo/catalogo-piezas.redes` | 49 | 8 | — | 12 |
| `demo/plantilla.redes` | 3 | 2 | 50,0 | 2 |

Reproducir: `redes -p demo/cadena-completa.redes cantidades`.

## 4. Rendimiento

| Medida | Valor | Contexto |
|---|---:|---|
| `redes vista` de la ciudad | **31,5 ms** | release, proceso completo incluido |
| Reconstrucción de la ciudad (p95) | **34,5 ms** | `cargo test --test rendimiento`, perfil de depuración |
| Reconstrucción con corte (p95) | **33,9 ms** | ídem |
| Catálogo completo (p95) | **6,6 ms** | ídem |
| Pase 3D de 700 pozos | < 400 ms primer frame | `--gpu3d --snap`, incluido el arranque de wgpu |

En release la reconstrucción baja a ~9–12 ms. El «p95 247 ms → 2,5 ms» de `docs/16` no tiene banco
reproducible detrás: se retira como afirmación.

### 4.1 Índices del proyecto (F14-01)

Red sintética de `redes.plan/1` (`plan_sintetico` en `crates/redes-core/tests/indices.rs`), release,
2026-11-25. «Antes» es el recorrido lineal previo a F14-01 (cifras del informe de la rama); «después»,
medido al integrar con `cargo test --release -p redes-core --test indices -- --include-ignored --nocapture`.

| Medida | 50k antes | 50k después | 20k después |
|---|---:|---:|---:|
| Ingesta del plan | 91 s | **485 ms** | 194 ms |
| Escena (`build_opts`, mediana de 3) | 4,0 s | **49 ms** | 21 ms |
| Reimportar la misma entrega | 167 s | **912 ms** | 389 ms |
| Abrir el `.redes` (incluye `reindexar`) | 279 ms | **269 ms** | 103 ms |
| Búsquedas por código e id (3 por tramo) | — | 24 ms | 9,5 ms |

Abrir no empeora de forma medible: reconstruir el índice es lineal y queda dentro del ruido de la
deserialización.

### 4.2 Pase 3D sin regenerar ni copiar (F14-02, F14-03)

Ciudad (`demo/ciudad-san-bartolo.redes`), release, `--gpu3d`, 2026-11-25 (cifras de la rama).

| Medida | Antes | Después |
|---|---:|---:|
| Cambio de cámara (arrastrar, rueda, botón de cámara) | 6,03 ms | **0,008 ms** |
| Regeneraciones de malla en 30 cambios de cámara | 30 | **0** |
| Bytes copiados por frame con la escena quieta | 3,6 MB (0,46 ms con el mutex) | **0** |

La malla solo se regenera si cambia `ClaveMalla` (proyecto y revisión, capas, biblioteca); la
exageración vertical va en la matriz. Pruebas `mover_la_camara_en_3d_no_regenera_la_malla` y
`la_escena_quieta_no_se_vuelve_a_subir`.

### 4.3 Banco de 20k tramos, perfil y escena (F14-10, F14-09, F14-04, F14-07)

Red en peine de `tests/comun` (sin azar), release, Windows 11 de 16 hilos, 2026-11-27, con
`cargo test --release -p redes-core --test banco -- --include-ignored --nocapture` (escena: mediana
de 9 `build_opts`, vista encuadrada 900×600). Cada columna se midió al integrar ese paso en `main`.

| Medida (20k tramos) | F14-10 | + perfil F14-09 | + escena F14-04/07 | Umbral CI |
|---|---:|---:|---:|---:|
| Ingesta | 392 ms | 184 ms | 182 ms | 1 000 ms |
| Escena | 30,1 ms | 18,8 ms | **7,0 ms** | 100 ms |
| Abrir | 217 ms | 121 ms | 115 ms | 600 ms |
| Reimportar | 697 ms | 335 ms | 349 ms | 2 000 ms |
| Búsquedas | 14,9 ms | 8,7 ms | 8,1 ms | — |
| Escena 50k | 77,5 ms | 49,9 ms | **13,4 ms** | — |

F14-04 pedía la escena ≥ 3× más rápida: con el mismo perfil, ×2,7 a 20k y ×3,7 a 50k (×4,3 a 20k
frente a antes de F14-09). Depuración con dependencias en `opt-level 2`: escena 20k 93 ms.

`tests/escena.rs` (`banco_escena`, ignorado, mínimo de 15, release), antes → después de F14-04/07:

| Cámara | Antes | Después | Primitivas |
|---|---:|---:|---:|
| Red 20k encuadrada | 17,3 ms | **6,5 ms** | 40 299 |
| Red 20k de cerca | 444 ms | **5,4 ms** | 3 054 → 493 |
| Ciudad encuadrada | 3,8 ms | **2,1 ms** | 12 009 |
| Ciudad de cerca | 3,9 ms | **1,3 ms** | 12 494 → 541 |

De cerca, el recorte por pantalla deja fuera lo que no se ve antes de teselarlo y ordenarlo. La
selección por rejilla: con 20k encuadrados un clic mira < 10 % de los 40 299 objetos (las filas
lejanas se apiñan); de cerca, < 50.

## 5. Pruebas

| Suite | Pruebas |
|---|---:|
| Núcleo (`redes-core`, unitarias) | **150** |
| Validación geométrica (`redes-core/tests/validar.rs`) | **13** |
| Acta de revisión (`redes-core/tests/acta.rs`) | **5** |
| Reimportar (`redes-core/tests/reimportar.rs`) | **8** |
| Ingesta con fixtures (`tests/ingesta.rs`) | **32** |
| Cotas de tramo (`tests/cotas_tramo.rs`) | **3** |
| Presupuesto de frame (`tests/rendimiento.rs`) | **2** |
| Índices del proyecto (`tests/indices.rs`) | **3** |
| Banco 20k (`tests/banco.rs`, + 1 de 50k ignorado) | **1** |
| Escena y selección (`tests/escena.rs`, + 1 banco ignorado) | **2** |
| Interfaz (`redes-app`) | **24** |
| Contrato del CLI (`tests/contrato.rs`) | **19** |
| Perfil del CLI (`tests/perfil.rs`) | **3** |
| Validar del CLI (`tests/validar.rs`) | **4** |
| Acta del CLI (`tests/acta.rs`) | **2** |
| Reimportar del CLI (`tests/reimportar.rs`) | **2** |
| Doctest | **1** |
| **Total** | **274** |

Más el arnés de mutación: `powershell -File scripts/mutar.ps1` rompe veintiocho comportamientos (cinco
de vista, tres de `redes validar`, el estado derivado de las observaciones y dos del acta: escapar el
HTML y no sacar columnas de autor sin registrarlo, D2; y tres de reimportar: reiniciar el aprobado
cuyo diseño cambió, conservar las observaciones y no dar de baja en silencio; y abrir un `.redes` de otra versión sin
migrar, F15-01, D10; y olvidar el nodo final en el índice de tramos por nodo, F14-01; y dos del pase 3D: regenerar la malla
al mover la cámara, F14-02, y volver a subirla en cada frame, F14-03; una escena cuadrática en el banco, F14-10;
dos de la caché de piezas y proxies, F14-06; y siete de la escena y la selección, F14-04/07: recortar de más,
no recortar, esfera de pozo corta, orden de negativos, bloques de hilos desordenados, rejilla de una celda y
rejilla que pierde tramos) y exige que la suite falle.

## 6. Qué cifras de los informes anteriores son falsas

| Documento | Dice | Medido |
|---|---|---|
| `docs/15` | «57 pruebas (49+7+1), 25 s» | 118 pruebas, ~2 s |
| `docs/10` | «vitrina 27 de 29 familias» | catálogo total con **46** familias |
| `docs/16` | «`.glb` ciudad 1,1 MB, 20 primitivas, 26 810 triángulos» | 3,44 MB, 20 primitivas, **31 710** |
| `docs/16` | «pozo completo 260 triángulos» | 332 completos / 176 simples (medido en `parts`) |
| `docs/16` | «p95 247 ms → 2,5 ms» | sin banco; ver §4 |
| `docs/15` | «la cobertura pasa por el mismo camino que el render» | hoy sí: `cada_familia_del_registro_dibuja` usa el render real |

## 7. Cómo rehacer esta tabla

1. `cargo build --release -p redes-app -p redes-cli`.
2. Los comandos de §2 y §3.
3. `cargo test --workspace` para el conteo de pruebas.
4. `cargo test -p redes-core --test rendimiento -- --nocapture` para los p95.

Si una cifra de este documento y el código discrepan, manda el código: actualizar aquí en el mismo
commit que cambie el comportamiento.
