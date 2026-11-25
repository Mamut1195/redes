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
        Reemplazo = 'Some(_) => (vec![verts], None),'
        Prueba = "el_corte_secciona_las_tuberias"
        Que = "dejar de seccionar las tuberias en el corte"
    },
    @{
        Nombre = "terreno-plano"
        Archivo = "crates/redes-core/src/view.rs"
        Patron = 'let depth = \(cam\.depth\(pa\) \+ cam\.depth\(pb\) \+ cam\.depth\(pc\)\) as f32 / 3\.0;'
        Reemplazo = 'let depth = 0.0f32; let _ = (pa, pb, pc);'
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
        Patron = 'subir: \(self\.generacion != subida\)\.then'
        Reemplazo = 'subir: true.then'
        Paquete = "redes-app"
        Prueba = "la_escena_quieta_no_se_vuelve_a_subir"
        Que = "volver a subir la malla en cada frame con la escena quieta (F14-03)"
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
