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
cadena de agua cruda con 95 subtipos, 17 subtipos nuevos en familias existentes y 13 retirados con
alias: `216 + 95 + 17 − 13 = 315`.

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

## 5. Pruebas

| Suite | Pruebas |
|---|---:|
| Núcleo (`redes-core`, unitarias) | **135** |
| Validación geométrica (`redes-core/tests/validar.rs`) | **13** |
| Acta de revisión (`redes-core/tests/acta.rs`) | **5** |
| Reimportar (`redes-core/tests/reimportar.rs`) | **8** |
| Ingesta con fixtures (`tests/ingesta.rs`) | **31** |
| Cotas de tramo (`tests/cotas_tramo.rs`) | **3** |
| Presupuesto de frame (`tests/rendimiento.rs`) | **2** |
| Interfaz (`redes-app`) | **18** |
| Contrato del CLI (`tests/contrato.rs`) | **17** |
| Perfil del CLI (`tests/perfil.rs`) | **3** |
| Validar del CLI (`tests/validar.rs`) | **4** |
| Acta del CLI (`tests/acta.rs`) | **2** |
| Reimportar del CLI (`tests/reimportar.rs`) | **2** |
| Doctest | **1** |
| **Total** | **244** |

Más el arnés de mutación: `powershell -File scripts/mutar.ps1` rompe catorce comportamientos (cinco
de vista, tres de `redes validar`, el estado derivado de las observaciones y dos del acta: escapar el
HTML y no sacar columnas de autor sin registrarlo, D2; y tres de reimportar: reiniciar el aprobado
cuyo diseño cambió, conservar las observaciones y no dar de baja en silencio) y exige que la suite falle.

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
