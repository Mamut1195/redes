# Rompe el código a propósito y exige que la suite lo detecte.
#
# Uso:  pwsh -File scripts/mutar.ps1
#       pwsh -File scripts/mutar.ps1 -Solo etiquetas
#
# Requiere el árbol limpio: restaura cada mutación con `git checkout`.
# Sale con código 1 si alguna mutación **sobrevive** (la suite pasa con el código roto).

param(
    [string]$Solo = ""
)

$ErrorActionPreference = "Stop"
$raiz = Split-Path -Parent $PSScriptRoot
Set-Location $raiz

$estado = (git status --porcelain) -join ""
if ($estado.Trim() -ne "") {
    Write-Host "El árbol tiene cambios sin commitear; la mutación necesita partir de un estado limpio." -ForegroundColor Red
    exit 2
}

# Cada mutación: archivo, patrón (regex), reemplazo y prueba que DEBE fallar.
$mutaciones = @(
    @{
        Nombre = "etiquetas"
        Archivo = "crates/redes-core/src/view.rs"
        Patron = 'if r_px > 9\.0 \|\| sel \{'
        Reemplazo = 'if false {'
        Prueba = "las_etiquetas_aparecen_al_acercarse"
        Que = "dejar de generar etiquetas"
    },
    @{
        Nombre = "corte-tubos"
        Archivo = "crates/redes-core/src/view.rs"
        Patron = 'Some\(xc\) => cortar_tri\(verts, xc\),'
        Reemplazo = 'Some(_) => (vec![verts], None),'
        Prueba = "el_corte_secciona_las_tuberias"
        Que = "dejar de seccionar las tuberías en el corte"
    },
    @{
        Nombre = "filtro-familias"
        Archivo = "crates/redes-core/src/view.rs"
        Patron = '(?s)if !def\.can\(caps::HYDRAULIC_NODE\).*?continue;\s*\}'
        Reemplazo = 'if !def.can(caps::HYDRAULIC_NODE) && !def.can(caps::ATTACHABLE) { continue; }'
        Prueba = "el_render_dibuja_familias_no_hidraulicas"
        Que = "volver al filtro que ocultaba las familias no hidráulicas"
    }
)

$sobrevivieron = @()

foreach ($m in $mutaciones) {
    if ($Solo -ne "" -and $m.Nombre -ne $Solo) { continue }

    Write-Host ""
    Write-Host "== Mutación: $($m.Nombre) — $($m.Que)" -ForegroundColor Cyan

    $texto = Get-Content -Raw $m.Archivo
    $mutado = [regex]::Replace($texto, $m.Patron, $m.Reemplazo, 1)
    if ($mutado -eq $texto) {
        Write-Host "   No se pudo aplicar (el patrón ya no está). Revisar la mutación." -ForegroundColor Yellow
        $sobrevivieron += "$($m.Nombre) (no aplicable)"
        continue
    }
    try {
        Set-Content -Path $m.Archivo -Value $mutado -NoNewline
        $salida = & cargo test -p redes-core $m.Prueba 2>&1 | Out-String
        $paso = $salida -match "test result: ok"
        if ($paso) {
            Write-Host "   SOBREVIVIÓ: la suite pasa con el código roto ($($m.Prueba))" -ForegroundColor Red
            $sobrevivieron += $m.Nombre
        } else {
            Write-Host "   Detectada por $($m.Prueba)" -ForegroundColor Green
        }
    }
    finally {
        git checkout -- $m.Archivo
    }
}

Write-Host ""
if ($sobrevivieron.Count -gt 0) {
    Write-Host "Mutaciones que la suite NO detecta:" -ForegroundColor Red
    $sobrevivieron | ForEach-Object { Write-Host " - $_" -ForegroundColor Red }
    exit 1
}
Write-Host "Todas las mutaciones fueron detectadas." -ForegroundColor Green
