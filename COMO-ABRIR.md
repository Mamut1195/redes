# Cómo abrir el visor (demo de la ciudad)

Es una **aplicación de escritorio nativa**: abre una ventana, no una página web. No hay URL.

## Opción 1 — doble clic (lo más fácil)

En esta carpeta:

```
Abrir Redes (ciudad).bat
```

Abre la ventana **«Redes — visor 3D»** con la ciudad de demostración cargada.
Si el ejecutable no existe todavía, el `.bat` lo compila la primera vez (varios minutos).

También hay `Abrir Redes (ciudad).ps1` si prefieres PowerShell.

## Opción 2 — desde una terminal

```powershell
cd "C:\Users\Jonnathan\Mamut system\rune-alcantarillado-agua-potable"
.\target\release\redes-app.exe --abrir demo\ciudad-san-bartolo.redes
```

Sin `--abrir`, la app abre su demo interna (la red pequeña de La Esperanza).

## Opción 3 — compilar y abrir

```powershell
cargo build --release -p redes-app
.\target\release\redes-app.exe --abrir demo\ciudad-san-bartolo.redes
```

## Si el archivo de la ciudad no está

```powershell
.\target\release\redes.exe proyecto ciudad --salida demo\ciudad-san-bartolo.redes
```

## Qué hacer dentro

| Acción | Cómo |
|---|---|
| Girar la ciudad | Arrastrar con el botón izquierdo |
| Desplazar | Arrastrar con el botón derecho o el central |
| Acercar / alejar | Rueda del ratón |
| Encuadrar todo | Botón **Encuadrar** |
| Planta / isométrica | Botones **Planta** / **Isométrica** |
| Ver la red bajo la ciudad | Apagar la capa **Edificios** |
| Seleccionar un pozo o tubo | Clic sobre él |
| Revisar | **Aprobar** / **Observar** en el inspector |
| Guardar la revisión | Botón **Guardar** (aparece `Guardar *` si hay cambios) |

## Requisitos

- Windows 10/11 de 64 bits con una GPU con DirectX 12 (una integrada sirve).
- Nada que instalar: `redes-app.exe` es un binario autónomo (~22 MB).
- Para recompilar hace falta Rust 1.95+ con el toolchain MSVC.
