# 23 — Estado y pendientes (traspaso)

Documento de traspaso: qué está hecho, qué falta y por dónde seguir. Las cifras vigentes están en
[`21-cifras-medidas.md`](21-cifras-medidas.md); las decisiones, en [`22-decisiones.md`](22-decisiones.md).

## Lo que está hecho y verificado

| Bloque | Estado |
|---|---|
| Registro | **46 familias / 315 subtipos**, incluidas las 17 de la cadena de agua cruda |
| Cadena completa | `redes proyecto cadena`: 90 objetos, 72 tramos, de la bocatoma (2530 m) a la descarga (2498 m) |
| Modelos | importación, validación, medidas, caché, dibujo en el pase 3D, política de pesados, foco sobre el modelo, **selector en el inspector** y **anclas de conexión** (se leen del `.glb` y la tubería nace en el puerto, no en el centro) |
| Cantidades | `redes cantidades`: unidades, longitudes por DN/material, volúmenes, pesos, discrepancias y CSV |
| CAD | ida y vuelta DXF sin pérdida (46 familias y 7 arcos vuelven idénticos), **bloques con atributos** por familia y GeoJSON con atributos |
| Calidad | **216 pruebas** (núcleo 126 + 13 validar + 31 ingesta + 3 cotas + 2 rendimiento, app 17, CLI 16 + 3 + 4, doctest 1), arnés de 8 mutaciones, CI en GitHub **en verde** |
| Revisor (F13) | `redes validar` geométrico con panel Validación (F13-01, D6), `redes perfil` con botón Perfil en el inspector (F13-02), `redes proyecto diff` (F13-05) |
| Producto | plantilla, licencias de modelos, journal con fecha, paquete portable y **release 0.1.0 publicada** |
| Documentación | 23 documentos; los informes antiguos llevan aviso de cifras corregidas |

## Lo que falta (por orden de valor)

Revisión para v1 del 25/11/2026 (nota 5,5/10): el plan está en `docs/18-backlog.md`, fases **F12–F16**,
y en Linear (MAM-1085…MAM-1139). Se trabaja **por fases en orden**; no se abre una fase sin cerrar la anterior.

| Fase | Estado |
|---|---|
| F12 — Corrección de dominio | **cerrada** (F12-01…F12-15). Ingesta por skill guiada (D5) |
| F13 — Herramientas del revisor | **en curso**: hechas F13-01 (`redes validar`), F13-02 (perfil longitudinal) y F13-05 (`proyecto diff`); pendientes F13-03 (observaciones con historial, en curso), F13-04 (acta) y F13-06 (reimportar sin perder revisión) |
| F14 — Rendimiento con redes reales | pendiente (índices, cámara 3D sin regenerar, LOD/instanciado) |
| F15 — Arquitectura | pendiente (migración de formato P0, agregado, catálogo como datos, errores tipados) |
| F16 — Release v1 | pendiente (F16-01 ya cerrada por F12-09) |

Decisiones abiertas del usuario: A1 (vertedero), A2 (pozo profundo), sumideros de la topología
(`is_sink` en `topology.rs`: descarga, cuerpo receptor, reactor, laguna, sedimentador).
Sin validar en CAD real: el DXF de F12-08 (no hubo `ezdxf`); abrir una demo exportada en AutoCAD/BricsCAD.

## Cómo retomar

```powershell
cargo test --workspace                       # 216 pruebas
powershell -File scripts/mutar.ps1           # 8 mutaciones
powershell -File scripts/empaquetar.ps1      # zip portable
gh run list --limit 3                        # CI
```

Para el proxy (1): `modelos::proxy(malla, paso)` devuelve la malla decimada; el punto de enganche es
el pase del pintor donde hoy se cae a la pieza paramétrica.

## Deuda conocida

- `redes-cli/src/main.rs` concentra todos los comandos; si crece, toca partirlo por dominio.
- El `journal` no guarda identidad (decisión D2); si algún día se pide, es un campo opcional más.
- El `.glb` del catálogo y de la ciudad incluyen el terreno: las cifras de triángulos de `docs/21`
  lo tienen en cuenta.
