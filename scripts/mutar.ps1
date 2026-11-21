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
        $salida = (& cargo test -p redes-core $m.Prueba 2>&1 | Out-String)
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
