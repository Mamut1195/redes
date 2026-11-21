# Abre el visor 3D con la ciudad de demostración.
#   derecha > "Ejecutar con PowerShell"
#   o:  pwsh -File ".\Abrir Redes (ciudad).ps1"
$ErrorActionPreference = 'Stop'
Set-Location -LiteralPath $PSScriptRoot

$exe = Join-Path $PSScriptRoot 'target\release\redes-app.exe'
if (-not (Test-Path $exe)) {
    Write-Host 'Compilando el visor por primera vez (varios minutos)...'
    cargo build --release -p redes-app
    if ($LASTEXITCODE -ne 0) { throw 'No se pudo compilar. ¿Está Rust instalado?' }
}

Start-Process -FilePath $exe -ArgumentList @('--abrir', (Join-Path $PSScriptRoot 'demo\ciudad-san-bartolo.redes'))
