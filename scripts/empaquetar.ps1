# Empaqueta Redes para Windows: binarios, demos, documentos y licencia en un zip portable.
#
#   powershell -File scripts/empaquetar.ps1              # compila en release y empaqueta
#   powershell -File scripts/empaquetar.ps1 -SinCompilar # usa lo que haya en target/release

param(
    [switch]$SinCompilar,
    [string]$Version = "0.1.0"
)

$ErrorActionPreference = "Continue"
$raiz = Split-Path -Parent $PSScriptRoot
Set-Location $raiz

if (-not $SinCompilar) {
    Write-Host "Compilando en release..." -ForegroundColor Cyan
    & cargo build --release -p redes-app -p redes-cli
    if ($LASTEXITCODE -ne 0) {
        Write-Host "La compilacion fallo; no se empaqueta." -ForegroundColor Red
        exit 1
    }
}

$destino = Join-Path $raiz ("out/paquete/redes-" + $Version + "-windows-x64")
if (Test-Path $destino) { Remove-Item $destino -Recurse -Force }
New-Item -ItemType Directory -Force $destino | Out-Null
New-Item -ItemType Directory -Force (Join-Path $destino "demo") | Out-Null
New-Item -ItemType Directory -Force (Join-Path $destino "docs") | Out-Null

foreach ($b in @("target/release/redes-app.exe", "target/release/redes.exe")) {
    if (Test-Path $b) {
        Copy-Item $b $destino -Force
    } else {
        Write-Host ("Falta " + $b + " (compila en release)") -ForegroundColor Red
        exit 1
    }
}

Get-ChildItem "demo/*.redes" -ErrorAction SilentlyContinue | ForEach-Object {
    Copy-Item $_.FullName (Join-Path $destino "demo") -Force
}

Copy-Item "README.md", "LICENSE" -Destination (Join-Path $destino "docs") -Force -ErrorAction SilentlyContinue
Copy-Item "docs/04-guia.md", "docs/18-backlog.md", "docs/20-familias-captacion-tratamiento.md" -Destination (Join-Path $destino "docs") -Force -ErrorAction SilentlyContinue

$ciudad = @'
@echo off
cd /d "%~dp0"
start "" redes-app.exe --abrir "demo\ciudad-san-bartolo.redes" --gpu3d
'@
Set-Content -Path (Join-Path $destino "Abrir ciudad.bat") -Value $ciudad -Encoding ascii

$cadena = @'
@echo off
cd /d "%~dp0"
start "" redes-app.exe --abrir "demo\cadena-completa.redes"
'@
Set-Content -Path (Join-Path $destino "Abrir cadena.bat") -Value $cadena -Encoding ascii

$marca = "redes " + $Version + " - " + (Get-Date -Format "yyyy-MM-dd")
Set-Content -Path (Join-Path $destino "VERSION.txt") -Value $marca -Encoding utf8

$zip = Join-Path $raiz ("out/paquete/redes-" + $Version + "-windows-x64.zip")
if (Test-Path $zip) { Remove-Item $zip -Force }
Compress-Archive -Path $destino -DestinationPath $zip -CompressionLevel Optimal

$mb = [math]::Round((Get-Item $zip).Length / 1MB, 1)
Write-Host ("Paquete: " + $zip + " (" + $mb + " MB)") -ForegroundColor Green
