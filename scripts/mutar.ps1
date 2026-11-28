# Rompe el código a propósito y exige que la suite lo detecte.
#
# Uso:  powershell -NoProfile -File scripts/mutar.ps1
#       powershell -NoProfile -File scripts/mutar.ps1 -Solo etiquetas
#
# Restaura siempre desde una copia de seguridad local (no depende de git), incluso si el script
# falla a mitad: una mutación nunca debe quedarse en el árbol.
# Sale con código 1 si alguna mutación **sobrevive** (la suite pasa con el código roto).

param(
    [string]$Solo = ""
)

# Los stderr de cargo no deben abortar el script.
$ErrorActionPreference = "Continue"
$raiz = Split-Path -Parent $PSScriptRoot
Set-Location $raiz

$mutaciones = @(
    @{
        Nombre = "etiquetas"
        Archivo = "crates/redes-core/src/view.rs"
        Patron = 'if \(r_px > 9\.0 \|\| sel\)'
        Reemplazo = 'if false'
        Prueba = "las_etiquetas_aparecen_al_acercarse"
        Que = "dejar de generar etiquetas"
    },
    @{
        Nombre = "corte-tubos"
        Archivo = "crates/redes-core/src/view.rs"
        Patron = 'Some\(xc\) => cortar_tri\(verts, xc\),'
        Reemplazo = 'Some(_) => (Trozos::uno(verts), None),'
        Prueba = "el_corte_secciona_las_tuberias"
        Que = "dejar de seccionar las tuberias en el corte"
    },
    @{
        Nombre = "terreno-plano"
        Archivo = "crates/redes-core/src/view.rs"
        Patron = 'let depth = \(da \+ db \+ dc\) as f32 / 3\.0;'
        Reemplazo = 'let depth = 0.0f32; let _ = (da, db, dc);'
        Prueba = "el_terreno_queda_ordenado_de_lejos_a_cerca"
        Que = "dejar el terreno sin orden de profundidad"
    },
    @{
        Nombre = "base-previa"
        Archivo = "crates/redes-core/src/biblioteca.rs"
        Patron = 'base_previa: base,'
        Reemplazo = 'base_previa: base + 1.0,'
        Prueba = "normalizar_centra_y_apoya_la_base"
        Que = "informar una base falsa al normalizar"
    },
    @{
        Nombre = "filtro-familias"
        Archivo = "crates/redes-core/src/view.rs"
        Patron = '(?s)if !def\.can\(caps::HYDRAULIC_NODE\).*?continue;\s*\}'
        Reemplazo = 'if !def.can(caps::HYDRAULIC_NODE) && !def.can(caps::ATTACHABLE) { continue; }'
        Prueba = "el_render_dibuja_familias_no_hidraulicas"
        Que = "volver al filtro que ocultaba las familias no hidraulicas"
    },
    @{
        Nombre = "validar-contrapendiente"
        Archivo = "crates/redes-core/src/validar.rs"
        Patron = 'topo\.contrapendiente\.contains\(&l\.id\) \|\| b > a \+ 1e-9'
        Reemplazo = 'false'
        Destino = "validar"
        Prueba = "contrapendiente_dispara_cumple_y_no_evaluable"
        Que = "dejar de marcar la contrapendiente"
    },
    @{
        Nombre = "validar-supone-dato"
        Archivo = "crates/redes-core/src/validar.rs"
        Patron = '_ if exacto => cx\.cumple\(R\),'
        Reemplazo = '_ => cx.cumple(R),'
        Destino = "validar"
        Prueba = "recubrimiento_minimo_en_extremos"
        Que = "dar por bueno el recubrimiento cuando falta un dato (inventar)"
    },
    @{
        Nombre = "validar-agua-arriba"
        Archivo = "crates/redes-core/src/validar.rs"
        Patron = 'Some\(\(h, en\)\) if h < sep_v - 1e-9'
        Reemplazo = 'Some((h, en)) if h < -1e9'
        Destino = "validar"
        Prueba = "separacion_agua_alcantarillado"
        Que = "ignorar la separacion vertical entre agua y alcantarillado"
    },
    @{
        Nombre = "obs-estado-derivado"
        Archivo = "crates/redes-core/src/revision.rs"
        Patron = 'if obs\.iter\(\)\.any\(Observacion::abierta\) \{'
        Reemplazo = 'if false {'
        Prueba = "cerrar_la_ultima_abierta_deja_pendiente"
        Que = "dejar de derivar observado de las observaciones abiertas (F13-03)"
    },
    @{
        Nombre = "acta-sin-escapar"
        Archivo = "crates/redes-core/src/acta.rs"
        Patron = "'<' => o\.push_str\(""&lt;""\),"
        Reemplazo = "'<' => o.push('<'),"
        Destino = "acta"
        Prueba = "el_html_es_autocontenido_imprimible_y_escapa_el_texto"
        Que = "volcar el texto del usuario sin escapar en el acta HTML (F13-04)"
    },
    @{
        Nombre = "acta-autor-d2"
        Archivo = "crates/redes-core/src/acta.rs"
        Patron = '\.any\(\|o\| o\.autor\.is_some\(\) \|\| o\.autor_cierre\.is_some\(\)\)'
        Reemplazo = '.any(|_| true)'
        Destino = "acta"
        Prueba = "el_csv_abre_en_excel_con_tildes_y_una_fila_por_observacion"
        Que = "sacar columnas de autor aunque no se registro (D2, F13-04)"
    },
    @{
        Nombre = "reimportar-no-reinicia"
        Archivo = "crates/redes-core/src/reimportar.rs"
        Patron = 'if estado == EstadoObjeto::Aprobado && !solo_procedencia \{'
        Reemplazo = 'if false {'
        Destino = "reimportar"
        Prueba = "cambiar_la_cota_de_un_tramo_aprobado_lo_devuelve_a_pendiente"
        Que = "conservar aprobado un objeto cuyo diseno cambio al reimportar (F13-06)"
    },
    @{
        Nombre = "reimportar-pierde-observaciones"
        Archivo = "crates/redes-core/src/reimportar.rs"
        Patron = 'n\.observaciones = viejo\.observaciones\.clone\(\);'
        Reemplazo = 'n.observaciones = Vec::new();'
        Destino = "reimportar"
        Prueba = "reimportar_la_misma_entrega_conserva_toda_la_revision"
        Que = "perder las observaciones de los nodos al reimportar (F13-06)"
    },
    @{
        Nombre = "reimportar-bajas-silenciosas"
        Archivo = "crates/redes-core/src/reimportar.rs"
        Patron = '\(!aceptar_bajas && !codigos\.is_empty\(\)\)'
        Reemplazo = '(false)'
        Destino = "reimportar"
        Prueba = "las_bajas_con_revision_no_se_pierden_en_silencio"
        Que = "dar de baja en silencio objetos con revision (F13-06)"
    },
    @{
        # Prueba de la biblioteca (src/project_io.rs): sin `Destino`, el binario lib corre primero.
        Nombre = "proyecto-acepta-version-anterior"
        Archivo = "crates/redes-core/src/project_io.rs"
        Patron = 'if schema == vigente \{'
        Reemplazo = 'if schema.starts_with("redes.project/") {'
        Prueba = "rechaza_un_esquema_anterior_sin_migrar"
        Que = "volver a abrir un .redes de otra version sin migrar (F15-01, D10)"
    },
    @{
        Nombre = "indice-adyacencia"
        Archivo = "crates/redes-core/src/indice.rs"
        Patron = 'if l\.to != l\.from \{'
        Reemplazo = 'if false {'
        Destino = "indices"
        Prueba = "el_indice_coincide_con_el_recorrido_lineal_tras_cualquier_secuencia"
        Que = "olvidar el nodo final al indexar los tramos de cada nodo (F14-01)"
    },
    @{
        # Regresión de rendimiento, no de resultado: `node(id)` sigue siendo correcto pero recorre
        # todos los nodos, y la escena y la ingesta se vuelven cuadráticas. Solo el banco lo ve.
        Nombre = "banco-cuadratico"
        Archivo = "crates/redes-core/src/indice.rs"
        Patron = 'self\.indice\(\)\.map\(\|ix\| &ix\.nodo_por_id\)'
        Reemplazo = 'None'
        Destino = "banco"
        Prueba = "banco_20k_tramos_dentro_del_umbral"
        Que = "buscar nodos por id recorriendo el proyecto: escena cuadratica (F14-10)"
    },
    @{
        Nombre = "gpu3d-camara-regenera"
        Archivo = "crates/redes-app/src/main.rs"
        Patron = 'if self\.clave_malla != Some\(clave\) \{'
        Reemplazo = 'if true {'
        Paquete = "redes-app"
        Prueba = "mover_la_camara_en_3d_no_regenera_la_malla"
        Que = "regenerar la malla del pase 3D en cada movimiento de camara (F14-02)"
    },
    @{
        Nombre = "gpu3d-sube-cada-frame"
        Archivo = "crates/redes-app/src/gpu3d.rs"
        Patron = 'upload: \(self\.generation != uploaded\)\.then'
        Reemplazo = 'upload: true.then'
        Paquete = "redes-app"
        Prueba = "a_still_scene_is_not_uploaded_again"
        Que = "volver a subir la escena en cada frame con la escena quieta (F14-03)"
    },
    @{
        Nombre = "gpu3d-tope-fijo"
        Archivo = "crates/redes-app/src/gpu3d.rs"
        Patron = 'let budget = limit\.min\(scene3d::GPU_BUDGET\);'
        Reemplazo = 'let budget = scene3d::GPU_BUDGET;'
        Paquete = "redes-app"
        Prueba = "the_budget_respects_the_device_limit"
        Que = "ignorar max_buffer_size del dispositivo: panico con 5k pozos en una GPU de 256 MB (F14-05)"
    },
    @{
        Nombre = "escena3d-sin-compartir"
        Archivo = "crates/redes-core/src/scene3d.rs"
        Patron = '\.find\(\|c\| self\.signatures\[\*c as usize\] == self\.signature\)'
        Reemplazo = '.find(|_| false)'
        Prueba = "manholes_of_different_depth_share_prototypes"
        Que = "no compartir prototipos: cada pozo vuelve a copiar su malla (F14-05)"
    },
    @{
        Nombre = "escena3d-sin-escala-z"
        Archivo = "crates/redes-core/src/scene3d.rs"
        Patron = 'scale: \[1\.0, 1\.0, part\.scale_z\],'
        Reemplazo = 'scale: [1.0, 1.0, 1.0],'
        Prueba = "instancing_reproduces_the_exported_mesh"
        Que = "colocar el prototipo sin su escala en Z: fustes de otra altura (F14-05)"
    },
    @{
        Nombre = "escena3d-lod-fijo"
        Archivo = "crates/redes-core/src/scene3d.rs"
        Patron = 'if radius_px >= FULL_DETAIL_PX \{'
        Reemplazo = 'if true {'
        Prueba = "distant_objects_draw_as_boxes_and_near_ones_in_full"
        Que = "dibujar todo con el detalle completo, aunque ocupe un pixel (F14-05)"
    },
    @{
        Nombre = "escena3d-sin-recorte"
        Archivo = "crates/redes-core/src/scene3d.rs"
        Patron = 'outside\.iter\(\)\.any\(\|o\| \*o\)'
        Reemplazo = 'false'
        Prueba = "objects_outside_the_view_are_culled"
        Que = "dibujar lo que cae fuera de la camara (F14-05)"
    },
    @{
        Nombre = "escena3d-sin-tope"
        Archivo = "crates/redes-core/src/scene3d.rs"
        Patron = 'if instance_bytes \+ v \+ i > budget \{'
        Reemplazo = 'if false {'
        Prueba = "a_scene_over_budget_is_trimmed_and_warns"
        Que = "subir la escena entera aunque pase del tope de GPU, sin aviso (F14-05)"
    },
    @{
        Nombre = "glb-malla-por-nodo"
        Archivo = "crates/redes-core/src/gltf.rs"
        Patron = 'let index = match mesh_of\.get\(&key\) \{'
        Reemplazo = 'let index = match None::<&usize> {'
        Prueba = "equal_pieces_share_one_mesh_and_keep_every_triangle"
        Que = "copiar la malla de cada pozo en el .glb en vez de compartirla (F14-05)"
    },
    @{
        Nombre = "glb-largo-truncado"
        Archivo = "crates/redes-core/src/gltf.rs"
        Patron = 'u32::try_from\(total\)\.map_err\(\|_\| GlbTooLarge \{ bytes: total \}\)'
        Reemplazo = 'Ok::<u32, GlbTooLarge>(total as u32)'
        Prueba = "a_glb_over_4_gib_is_an_error"
        Que = "escribir un .glb de mas de 4 GB con el largo truncado a u32 (F14-05)"
    },
    @{
        Nombre = "cache-piezas-sin-acierto"
        Archivo = "crates/redes-core/src/mallas.rs"
        Patron = 'if let Some\(e\) = self\.mapa\.get_mut\(&clave\) \{'
        Reemplazo = 'if let Some(e) = None::<&mut Entrada> {'
        Prueba = "la_segunda_pasada_no_genera"
        Que = "regenerar todas las piezas en cada reconstruccion (F14-06)"
    },
    @{
        Nombre = "proxy-por-instancia"
        Archivo = "crates/redes-app/src/modelos.rs"
        Patron = 'if let Some\(m\) = self\.proxies\.get\(&clave\) \{'
        Reemplazo = 'if let Some(m) = None::<&Malla> {'
        Paquete = "redes-app"
        Prueba = "el_proxy_se_calcula_una_vez_por_pieza"
        Que = "recalcular el proxy por instancia (F14-06)"
    },
    @{
        Nombre = "recorte-de-mas"
        Archivo = "crates/redes-core/src/view.rs"
        Patron = 'x1: w \+ Self::MARGEN,'
        Reemplazo = 'x1: w * 0.5,'
        Prueba = "el_recorte_solo_quita_lo_que_no_se_ve"
        Que = "recortar media pantalla que si se ve (F14-07)"
    },
    @{
        Nombre = "recorte-apagado"
        Archivo = "crates/redes-core/src/view.rs"
        Patron = 'Pantalla::de\(&pr\)'
        Reemplazo = 'Pantalla::infinita()'
        Prueba = "el_recorte_solo_quita_lo_que_no_se_ve"
        Que = "ordenar tambien lo que cae fuera de pantalla (F14-07)"
    },
    @{
        Nombre = "esfera-pozo-corta"
        Archivo = "crates/redes-core/src/view.rs"
        Patron = 'h\.max\(0\.8\) \+ 0\.5 \+ dz_max'
        Reemplazo = '0.8'
        Prueba = "la_esfera_previa_del_pozo_envuelve_su_malla"
        Que = "descartar pozos que asoman por el borde (F14-07)"
    },
    @{
        Nombre = "orden-negativos"
        Archivo = "crates/redes-core/src/view.rs"
        Patron = '\(clave_de_orden\(d\) as u32\) \^ 0x8000_0000'
        Reemplazo = 'clave_de_orden(d) as u32'
        Prueba = "el_orden_del_pintor_es_el_estable_de_antes"
        Que = "ordenar mal las profundidades negativas (F14-04)"
    },
    @{
        Nombre = "bloques-desordenados"
        Archivo = "crates/redes-core/src/view.rs"
        Patron = '\.chunks\(tam\)'
        Reemplazo = '.chunks(tam).rev()'
        Prueba = "los_bloques_conservan_el_orden"
        Que = "juntar los bloques de hilos fuera de orden (F14-04)"
    },
    @{
        Nombre = "rejilla-sin-reparto"
        Archivo = "crates/redes-core/src/seleccion.rs"
        Patron = 'const CELDA: f32 = 32\.0;'
        Reemplazo = 'const CELDA: f32 = 1.0e6;'
        Destino = "escena"
        Prueba = "la_rejilla_elige_como_el_recorrido_lineal_mirando_poco"
        Que = "una sola celda: cada clic recorre toda la red (F14-07)"
    },
    @{
        Nombre = "rejilla-pierde-vecinos"
        Archivo = "crates/redes-core/src/seleccion.rs"
        Patron = '> r \+ media'
        Reemplazo = '> r'
        Prueba = "la_rejilla_elige_lo_mismo_que_el_recorrido_lineal"
        Que = "olvidar tramos en las celdas que rozan (F14-07)"
    },
    @{
        Nombre = "deshacer-copia-todo"
        Archivo = "crates/redes-core/src/undo.rs"
        Patron = 'if before != after \{'
        Reemplazo = 'if true {'
        Destino = "undo"
        Prueba = "twenty_steps_on_20k_links_weigh_a_tiny_fraction_of_copies"
        Que = "guardar cada objeto en cada paso, como las copias de antes (F14-08)"
    },
    @{
        Nombre = "deshacer-contadores"
        Archivo = "crates/redes-core/src/undo.rs"
        Patron = 'Side::Before => self\.before\.write\(p\),'
        Reemplazo = 'Side::Before => self.after.write(p),'
        Destino = "undo"
        Prueba = "undo_and_redo_restore_every_kind_of_edit_exactly"
        Que = "deshacer sin devolver la revision del proyecto (F14-08)"
    },
    @{
        Nombre = "deshacer-sin-reindexar"
        Archivo = "crates/redes-core/src/undo.rs"
        Patron = 'if self\.reindex \{'
        Reemplazo = 'if false {'
        Destino = "undo"
        Prueba = "undoing_an_added_node_keeps_the_indices_current"
        Que = "deshacer un alta sin rehacer los indices (F14-08)"
    },
    @{
        Nombre = "deshacer-sin-registrar"
        Archivo = "crates/redes-app/src/main.rs"
        Patron = 'self\.historial\.record\(&self\.project, &edited\);'
        Reemplazo = ''
        Paquete = "redes-app"
        Prueba = "undo_and_redo_buttons_restore_each_review_step"
        Que = "no llevar la revision de la app a la pila de deshacer (F14-08)"
    },
    @{
        Nombre = "clave-sin-receta"
        Archivo = "crates/redes-core/src/bake.rs"
        Patron = 'parts\.extend\(sources\.iter\(\)\.map\(\|s\| lf_only\(s\)\)\);'
        Reemplazo = 'let _ = sources;'
        Prueba = "the_key_follows_params_and_recipe_but_not_key_order_or_line_endings"
        Que = "una clave de horneado que no cambia al editar la receta: la cache serviria piezas viejas (F17-02)"
    },
    @{
        Nombre = "modelo-en-la-tapa"
        Archivo = "crates/redes-app/src/modelos.rs"
        Patron = 'at\.base_m as f32,'
        Reemplazo = 'm.nodo.z_rim as f32,'
        Paquete = "redes-app"
        Prueba = "a_piece_with_an_invert_anchor_sits_on_the_invert_minus_its_slab"
        Que = "apoyar el pozo horneado en la cota de tapa en vez de en la solera menos la losa (F17-03)"
    },
    @{
        Nombre = "pieza-doble-gpu"
        Archivo = "crates/redes-core/src/scene3d.rs"
        Patron = '!n\.deleted && !replaced\.contains\(&n\.id\)'
        Reemplazo = '!n.deleted'
        Prueba = "a_model_replaces_the_piece_of_its_object"
        Que = "dibujar en el pase 3D el modelo y ademas la pieza parametrica (F17-03)"
    },
    @{
        Nombre = "pieza-doble-pintor"
        Archivo = "crates/redes-core/src/view.rs"
        Patron = 'let model = models\.get\(&n\.id\);'
        Reemplazo = 'let model = None::<&ModelPiece>; let _ = models;'
        Prueba = "a_model_piece_is_drawn_instead_of_the_parametric_one"
        Que = "dibujar sin pase 3D la pieza parametrica en vez del modelo horneado (F17-03)"
    },
    @{
        Nombre = "extras-un-nodo"
        Archivo = "crates/redes-core/src/biblioteca.rs"
        Patron = 'let prefijo = format!\("nodo\.\{\}", nombre_o_indice\(nodo, indice\)\);'
        Reemplazo = 'let prefijo = "nodo".to_string();'
        Destino = "pozo_horneado"
        Prueba = "the_pilot_is_one_closed_solid_per_part_with_its_own_material"
        Que = "guardar los extras de un solo nodo: la tapa pisa el material del cuerpo (F17-03)"
    },
    @{
        Nombre = "ancla-inventada"
        Archivo = "crates/redes-core/src/bake.rs"
        Patron = '\.ok_or_else\(\|\| "el tramo no trae cota de solera en este extremo"\.to_string\(\)\)\?;'
        Reemplazo = '.unwrap_or(invert);'
        Prueba = "a_pipe_without_invert_has_no_anchor_and_says_why"
        Que = "inventar la cota de un tramo sin dato para ponerle ancla (F17-03)"
    },
    @{
        Nombre = "cota-sin-solera"
        Archivo = "crates/redes-core/src/bake.rs"
        Patron = 'let invert = node\.cota_solera\(\)\.ok_or_else\(\|\| COTA_SOLERA\.to_string\(\)\)\?;'
        Reemplazo = 'let invert = node.cota_solera().unwrap_or(0.0);'
        Destino = "captacion_horneada"
        Prueba = "a_level_without_the_object_invert_is_missing_not_filled_in"
        Que = "hornear una obra sin cota de solera como si estuviera a cota 0 (F17-06)"
    },
    @{
        Nombre = "topo-sin-quiebres"
        Archivo = "crates/redes-core/src/terrain.rs"
        Patron = '(?s)finite,\s*edges,'
        Reemplazo = 'finite, { let _ = edges; Vec::new() },'
        Prueba = "no_triangle_crosses_the_ravine_breakline"
        Que = "triangular sin las lineas de quiebre: el TIN rellena la quebrada (F17-11)"
    },
    @{
        Nombre = "topo-sin-recorte"
        Archivo = "crates/redes-core/src/terrain.rs"
        Patron = '!outside && !in_hole'
        Reemplazo = 'true || (!outside && !in_hole)'
        Prueba = "boundary_concavities_get_no_triangles"
        Que = "no recortar el TIN al contorno: triangulos en las concavidades (F17-11)"
    },
    @{
        Nombre = "topo-coincidentes"
        Archivo = "crates/redes-core/src/survey.rs"
        Patron = 'if \(f\.z - q\.z\)\.abs\(\) > SAME_HEIGHT_M \{'
        Reemplazo = 'if false {'
        Destino = "topografia"
        Prueba = "same_place_with_another_height_is_an_error_and_same_height_a_warning"
        Que = "elegir en silencio una de dos cotas para la misma planta (F17-11)"
    },
    @{
        Nombre = "datum-mezclado"
        Archivo = "crates/redes-core/src/ingest.rs"
        Patron = 'if datum\.is_declared\(\) && incoming\.is_declared\(\) && datum != incoming \{'
        Reemplazo = 'if false {'
        Destino = "topografia"
        Prueba = "a_survey_in_another_datum_than_the_network_is_e_datum"
        Que = "mezclar cotas de datums verticales distintos sin error (F17-12)"
    },
    @{
        Nombre = "modulo-fuera-de-clave"
        Archivo = "crates/redes-core/src/bake.rs"
        Patron = 'sources\.extend\(self\.module_bytes\.iter\(\)\.map\(Vec::as_slice\)\);'
        Reemplazo = 'let _ = &self.module_bytes;'
        Prueba = "a_module_the_recipe_imports_is_part_of_the_key"
        Que = "dejar plant_units.py fuera de la clave: tocarlo no rehornearia las unidades (F17-07)"
    },
    @{
        Nombre = "tubo-sin-recorte-en-ancla"
        Archivo = "crates/redes-core/src/pipe_layout.rs"
        Patron = '\.is_some_and\(\|\(_, code\)\| code == link\.code\)'
        Reemplazo = '.is_some_and(|_| false)'
        Prueba = "the_axis_stops_at_the_anchor_of_a_baked_node"
        Que = "dibujar el tubo hasta el borde de la pieza de respaldo y no hasta el ancla del pozo horneado (F17-04)"
    },
    @{
        Nombre = "tubo-campana-en-el-pozo"
        Archivo = "crates/redes-core/src/pipe_layout.rs"
        Patron = 'out\.push\(if k \+ 1 == count \{'
        Reemplazo = 'out.push(if false {'
        Prueba = "whole_segments_then_one_cut_barrel"
        Que = "terminar el tramo con una campana entera dentro del pozo en vez del corte (F17-04)"
    },
    @{
        Nombre = "tubo-sin-inclinar"
        Archivo = "crates/redes-core/src/scene3d.rs"
        Patron = 'let \(x, z\) = \(x \* cp - z \* sp, x \* sp \+ z \* cp\);'
        Reemplazo = 'let (x, z) = (x, z); let _ = (cp, sp);'
        Prueba = "baked_pipe_segments_are_instanced_on_their_sloped_axis"
        Que = "colocar los tramos de tubo horneados horizontales en un tramo con pendiente (F17-04)"
    },
    @{
        Nombre = "tubo-fuera-de-la-boca"
        Archivo = "crates/redes-core/src/pipe_layout.rs"
        Patron = '\.or_else\(\|\| by_name\.and_then\(\|by_name\| port_anchor_offset\(p, link, node, by_name\)\)\)'
        Reemplazo = ''
        Prueba = "a_pipe_ends_at_the_mouth_of_its_port_on_a_baked_fitting"
        Que = "llevar el tubo al centro del accesorio horneado en vez de a la boca: asoma por la esquina del codo (F17-05)"
    },
    @{
        Nombre = "accesorio-sin-girar"
        Archivo = "crates/redes-core/src/bake.rs"
        Patron = 'rotation_deg: frame\.rotation_deg,'
        Reemplazo = 'rotation_deg: by_object.rotation_deg,'
        Prueba = "a_port_framed_piece_turns_with_its_pipes_and_sits_on_the_pipe_invert"
        Que = "dejar el accesorio horneado sin girar con sus tubos (F17-05)"
    },
    @{
        Nombre = "tee-referencia-en-el-ramal"
        Archivo = "crates/redes-core/src/bake.rs"
        Patron = 'Some\(\(_, i, _\)\) => i,'
        Reemplazo = 'Some((_, _, j)) => j,'
        Prueba = "the_reference_of_a_tee_is_on_its_run_whatever_the_codes"
        Que = "empezar el marco de una tee por la salida en vez de por la entrada del tramo recto (F17-05)"
    },
    @{
        Nombre = "valvula-recta-con-angulo"
        Archivo = "crates/redes-core/src/bake.rs"
        Patron = 'None => 0\.0,'
        Reemplazo = 'None => drawn_angle(q.azimuth_deg, 1.0),'
        Prueba = "the_chain_demo_valves_bake_to_few_pieces"
        Que = "una pieza distinta por cada desvio del tubo en una valvula recta (F17-05)"
    },
    @{
        Nombre = "tubo-corto-en-accesorio"
        Archivo = "crates/redes-core/src/pipe_layout.rs"
        Patron = '"comun\.accesorio" \| "agua\.valvula" \| "agua\.hidrante" => 0\.0,'
        Reemplazo = ''
        Prueba = "pipes_run_into_unbaked_fittings_but_stop_at_chamber_walls"
        Que = "cortar el tubo a 0,75 m del accesorio: hueco entre el tubo y la pieza horneada (F17-05)"
    }
)

$sobrevivieron = @()

foreach ($m in $mutaciones) {
    if ($Solo -ne "" -and $m.Nombre -ne $Solo) { continue }

    Write-Host ""
    Write-Host "== Mutacion: $($m.Nombre) - $($m.Que)" -ForegroundColor Cyan

    $original = Get-Content -Raw $m.Archivo
    $respaldo = "$($m.Archivo).mutacion.bak"
    Set-Content -Path $respaldo -Value $original -NoNewline

    $mutado = [regex]::Replace($original, $m.Patron, $m.Reemplazo, 1)
    if ($mutado -eq $original) {
        Write-Host "   No se pudo aplicar (el patron ya no esta)." -ForegroundColor Yellow
        Remove-Item $respaldo -Force -ErrorAction SilentlyContinue
        $sobrevivieron += "$($m.Nombre) (no aplicable)"
        continue
    }

    try {
        Set-Content -Path $m.Archivo -Value $mutado -NoNewline
        # Con `Destino` se corre solo ese binario de pruebas (tests/<Destino>.rs): si no, los
        # binarios anteriores imprimen "test result: ok. 0 passed" y la mutacion pareceria viva.
        $destino = @()
        if ($m.Destino) { $destino = @("--test", $m.Destino) }
        # `Paquete`: crate cuyas pruebas corren (por defecto redes-core). La app solo tiene pruebas
        # unitarias en su binario, así que no necesita `Destino`.
        $paquete = "redes-core"
        if ($m.Paquete) { $paquete = $m.Paquete }
        $salida = (& cargo test -p $paquete @destino $m.Prueba 2>&1 | Out-String)
        if ($salida -match "test result: ok") {
            Write-Host "   SOBREVIVIO: la suite pasa con el codigo roto ($($m.Prueba))" -ForegroundColor Red
            $sobrevivieron += $m.Nombre
        }
        else {
            Write-Host "   Detectada por $($m.Prueba)" -ForegroundColor Green
        }
    }
    finally {
        # Siempre: si el árbol se queda mutado, el siguiente commit sería una bomba.
        Copy-Item $respaldo $m.Archivo -Force
        Remove-Item $respaldo -Force -ErrorAction SilentlyContinue
    }
}

Write-Host ""
if ($sobrevivieron.Count -gt 0) {
    Write-Host "Mutaciones que la suite NO detecta:" -ForegroundColor Red
    $sobrevivieron | ForEach-Object { Write-Host " - $_" -ForegroundColor Red }
    exit 1
}
Write-Host "Todas las mutaciones fueron detectadas." -ForegroundColor Green
# El último `cargo test` de una mutación termina con 101 (fallo esperado). Sin este `exit 0`, el
# proceso hereda ese código y el job se marca en rojo aunque todo haya ido bien.
exit 0
