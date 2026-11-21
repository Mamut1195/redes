# Trae el checkout de Rune en la revisión fijada, sin tocar el proyecto.
#
#   powershell -File scripts/traer-rune.ps1                 # clona en vendor/rune-si-falta
#   powershell -File scripts/traer-rune.ps1 -Destino C:\tmp\rune
#
# Uso típico: comparar con upstream, revisar un cambio del framework o preparar una actualización.
# El proyecto compila contra `vendor/rune` tal como está; este script no lo sustituye solo.

param(
    [string]$Destino = "",
    [string]$Remoto = "https://github.com/anomalyco/rune.git",
    [string]$Revision = "78efd4a7876b5478dd72033ce6fd2b2f13ab9f9a"
)

$ErrorActionPreference = "Continue"
$raiz = Split-Path -Parent $PSScriptRoot
Set-Location $raiz

if ($Destino -eq "") {
    $Destino = Join-Path $raiz "out/rune-upstream"
}

if (Test-Path (Join-Path $Destino ".git")) {
    Write-Host ("Ya hay un checkout en " + $Destino) -ForegroundColor Yellow
} else {
    Write-Host ("Clonando Rune en " + $Revision + " -> " + $Destino) -ForegroundColor Cyan
    New-Item -ItemType Directory -Force $Destino | Out-Null
    & git clone --filter=blob:none $Remoto $Destino
    if ($LASTEXITCODE -ne 0) {
        Write-Host "No se pudo clonar. Comprueba la red y el remoto." -ForegroundColor Red
        exit 1
    }
}

Push-Location $Destino
& git fetch --all --tags
& git checkout $Revision
$ok = $LASTEXITCODE
Pop-Location

if ($ok -ne 0) {
    Write-Host "No se pudo cambiar a la revision fijada." -ForegroundColor Red
    exit 1
}

Write-Host ""
Write-Host "Listo. Para comparar con lo que compila el proyecto:" -ForegroundColor Green
Write-Host ("  git -C """ + $Destino + """ status")
Write-Host ("  git -C """ + $Destino + """ log --oneline -1")
Write-Host ""
Write-Host "Para actualizar de verdad: cambia las dos copias (vendor/rune y este checkout),"
Write-Host "repite la tabla [patch.crates-io] del Cargo.toml, compila y pasa la suite."
