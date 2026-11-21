# 11 — Modos de inspección y capa de lectura (fases 2 y 3)

> ⚠️ **Cifras corregidas.** Varios números de este documento se escribieron antes de que el
> código cambiara y ya no son ciertos. Las cifras vigentes, con cómo reproducirlas, están en
> [`21-cifras-medidas.md`](21-cifras-medidas.md). Este documento se conserva como historia.


> Lo que se implementó después del catálogo: ver la pieza por dentro, separarla, leer sus datos y
> cambiarlos en vivo. Todo dentro del visor, sin otra aplicación.

![Modos](img/modos.gif)

## 1. Foco

`Foco` (botón) lleva la cámara a la pieza seleccionada:

- encuadre por tamaño de la pieza: `distancia = 3,4 × max(diámetro, altura)`, entre 2,5 y 60 m;
- **`z_scale = 1`** dentro del foco: se dibuja a escala real, sin la exageración vertical de la
  vista general (allí un pozo de 4 m se ve como uno de 10);
- conserva acimut/elevación de la vista general, así el usuario no se desorienta.

`Salir` vuelve al encuadre general y restaura `z_scale` (2,0 o 2,6 con ciudad).

## 2. Corte

`Corte` activa un **plano vertical** por el centro del objeto seleccionado. No es transparencia:
se recorta la geometría contra el semiplano y se dibuja la **arista de corte** en amarillo.

- `Shift + rueda` desplaza el plano (2 cm por punto de rueda).
- Sirve para ver el interior real del pozo: espesor de pared, escalera, banqueo, tubos entrando.
- El recorte es por triángulo (Sutherland-Hodgman en el semiplano), así que también corta tubos.

## 3. Explosión

`Explosión` separa la pieza enfocada **por partes nombradas**, no por bandas de altura:

- Pozo: `solera`, `fuste`, `cono`, `marco`, `tapa`, `escalera` y el accesorio del subtipo
  (tubo de caída, resalto, bombas, cubeta de arenero, dado disipador).
- Válvula: `cuerpo`, `bridas`, `accionamiento` (volante, palanca, campana o diafragma).

El reparto es proporcional al número de partes: la pila se abre ~0,85 de la altura de la pieza,
así que con 6 partes la tapa queda a media altura y nada se sale de cuadro. `Shift + rueda` abre o
cierra el factor (0–2,5). Si el objeto no está despiezado, se muestra como una sola parte.

La lista de partes sale de `pozo_partes()` y `valvula_partes()` en `redes-core::parts`, y `pozo()`
/ `valvula()` las unen para el dibujo normal: la misma fuente para las dos vistas.

## 4. Etiquetas

Cada objeto cuya pieza ya se distingue en pantalla (radio proyectado > 9 px) o que está
seleccionado recibe una etiqueta con `código · subtipo`.

- Presupuesto: **24 etiquetas** como máximo, las más cercanas; la del objeto seleccionado se
  dibuja al final para que nada la tape.
- Se dibujan como texto real (capa superpuesta al canvas), no como parte de la malla: se leen a
  cualquier zoom y no engordan el triángulo.

## 5. Leyenda accesible

La leyenda del panel izquierdo usa **forma + texto**, no solo color:

| Glifo | Significado |
|---|---|
| ● | Alcantarillado (n objetos) |
| ◆ | Agua potable (n objetos) |
| ◇ | Aprobado |
| ○ | Pendiente |
| ✕ | Observado |

Los conteos salen del proyecto en vivo; al aprobar u observar, la leyenda cambia.

## 6. Ficha paramétrica en vivo

En el inspector, el bloque **Parámetros (en vivo)** tiene `− DN`, `+ DN`, `− Ø`, `+ Ø`,
`− prof.`, `+ prof.`:

- `DN` ajusta `diametro_nominal` (25 mm por clic, 25–1200).
- `Ø` ajusta `diametro_interior` (0,1 m por clic, 0,4–4,0) y **se ve cambiar la pieza**.
- `prof.` mueve la clave (`z_invert`, 0,25 m por clic) con tope en la tapa menos 0,6 m.

Cada cambio: apila un paso de deshacer, marca el documento sucio y reconstruye la escena. La
profundidad y el diámetro son los mismos parámetros que generan la malla, así que la pieza y el
inspector nunca se contradicen.

## 7. Deshacer

`Deshacer` recupera el proyecto anterior de una pila de **20 pasos**. Cubre la edición
paramétrica; la revisión (aprobar/observar) se guarda en el archivo, no en la pila.

## 8. Verificación

Pruebas automáticas que sostienen estos modos:

| Prueba | Qué comprueba |
|---|---|
| `el_corte_parte_un_triangulo_en_dos` | el recorte devuelve 2 triángulos y una arista |
| `el_corte_descarta_lo_que_queda_entero_al_otro_lado` | nada sobrevive al otro lado del plano |
| `el_corte_reduce_la_geometria_dibujada` | la escena cortada tiene menos triángulos |
| `la_explosion_separa_las_partes_del_objeto_enfocado` | mismos triángulos, geometría desplazada |
| `las_etiquetas_son_pocas_y_tienen_texto` | ≤ 28 etiquetas, texto y posición finitos |
| `el_boton_foco_acerca_la_camara` | distancia < 20 m y `z_scale = 1` |
| `el_boton_corte_activa_y_desactiva_el_plano` | ida y vuelta del plano |
| `el_boton_explosion_separa_la_pieza` | factor de explosión > 0 |
| `los_parametros_se_editan_en_vivo_y_se_desHacen` | +25 mm de DN, sucio, deshacer revierte |
| `salir_del_modo_vuelve_a_la_vista_general` | cámara y opciones limpias |

Escenas headless para revisar sin tocar el ratón: `--scene foco`, `--scene corte`,
`--scene explosion`.

## 9. Lo que sigue

1. Corte con **cara de sección** rellena (hoy es la arista; falta el relleno tramado).
2. Etiquetas con **seis carriles** y detección de solapes (hoy es una lista sin colisión).
3. Ficha editable para **todos** los parámetros, no solo tres.
