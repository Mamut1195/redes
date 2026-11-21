# 23 — Estado y pendientes (traspaso)

Documento de traspaso: qué está hecho, qué falta y por dónde seguir. Las cifras vigentes están en
[`21-cifras-medidas.md`](21-cifras-medidas.md); las decisiones, en [`22-decisiones.md`](22-decisiones.md).

## Lo que está hecho y verificado

| Bloque | Estado |
|---|---|
| Registro | **46 familias / 315 subtipos**, incluidas las 17 de la cadena de agua cruda |
| Cadena completa | `redes proyecto cadena`: 90 objetos, 72 tramos, de la bocatoma (2530 m) a la descarga (2498 m) |
| Modelos | importación, validación, medidas, caché, dibujo en el pase 3D, política de pesados, foco sobre el modelo, **selector en el inspector** y **lectura de anclas** |
| Cantidades | `redes cantidades`: unidades, longitudes por DN/material, volúmenes, pesos, discrepancias y CSV |
| CAD | ida y vuelta DXF sin pérdida (46 familias y 7 arcos vuelven idénticos) y GeoJSON con atributos |
| Calidad | 89 + 14 + 11 + 5 + 2 + 1 pruebas, arnés de 5 mutaciones, CI en GitHub **en verde** |
| Producto | plantilla, licencias de modelos, journal con fecha, paquete portable y **release 0.1.0 publicada** |
| Documentación | 23 documentos; los informes antiguos llevan aviso de cifras corregidas |

## Lo que falta (por orden de valor)

| # | Tarea | Dónde | Notas |
|---|---|---|---|
| 1 | **F6-08 segunda mitad**: que la tubería nazca en el ancla del modelo | `view.rs` (dibujo de arcos) o `ingest`/`biblioteca` (ajuste al aplicar) | Las anclas ya se leen y se guardan en `Entrada.anclas`; falta usarlas para mover el extremo del arco |
| 2 | **F6-05 proxy decimado** | `modelos.rs` | Hoy, sin `--gpu3d` se dibuja la paramétrica y se avisa; el proxy permitiría ver una silueta del modelo |
| 3 | **F8-02 bloques DXF con atributos** | `export.rs` | Comodidad para CAD: hoy los datos viajan en la capa `REDES_INFO` (TEXT) y la ida y vuelta funciona |
| 4 | **«Vertedero» y «pozo profundo»** | `docs/22-decisiones.md` | Decisión de dominio del usuario; afecta a mapeos de la skill, no al motor |
| 5 | Avisos de clippy de estilo | varios | Correctness en cero; quedan ~9 de estilo propios y los de `vendor/` |

## Cómo retomar

```powershell
cargo test --workspace                       # 122 pruebas
powershell -File scripts/mutar.ps1           # 5 mutaciones
powershell -File scripts/empaquetar.ps1      # zip portable
gh run list --limit 3                        # CI
```

Para 1: el punto de partida es `Entrada.anclas` (biblioteca) y el bucle de arcos en `view.rs`, donde
`link.path.first()` y `.last()` se proyectan tal cual. La transformación del ancla es la misma que
la de la malla: local (x, y, z) + posición del nodo, con la base en `z_rim`.

## Deuda conocida

- `redes-cli/src/main.rs` concentra todos los comandos; si crece, toca partirlo por dominio.
- El `journal` no guarda identidad (decisión D2); si algún día se pide, es un campo opcional más.
- El `.glb` del catálogo y de la ciudad incluyen el terreno: las cifras de triángulos de `docs/21`
  lo tienen en cuenta.
