@echo off
rem Abre el visor 3D de Redes con la ciudad de demostracion.
rem Doble clic en este archivo. Si el ejecutable no existe, lo compila la primera vez.
setlocal
cd /d "%~dp0"

set EXE=target\release\redes-app.exe

if not exist "%EXE%" (
  echo Compilando el visor por primera vez. Puede tardar varios minutos...
  cargo build --release -p redes-app
  if errorlevel 1 (
    echo.
    echo No se pudo compilar. Compruebe que Rust esta instalado ^(rustup^).
    pause
    exit /b 1
  )
)

start "" "%EXE%" --abrir "demo\ciudad-san-bartolo.redes"
endlocal
