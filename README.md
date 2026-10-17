# Proxy Manager

Aplicación de terminal (TUI) escrita en Rust con [ratatui](https://ratatui.rs/) para configurar el proxy del sistema en la configuración del shell (`~/.bashrc`, `~/.zshrc` o fish) y en `/etc/environment`.

Es una reescritura en Rust de la versión original en Go/Fyne, conservando su funcionamiento y sus archivos de configuración.

## Funcionalidad

- Formulario simplificado: **Proxy URL** (ej. `http://usuario:contraseña@host:3128`) y **No Proxy**.
- Habilitar / deshabilitar el proxy en los dos archivos a la vez (todas las variables: `http_proxy`, `https_proxy`, `no_proxy` y sus variantes en mayúsculas).
- Asegura al iniciar que las variables de proxy existan en los archivos (añade stubs comentados si faltan).
- Historial de configuraciones guardadas: cargar, borrar y guardar.
- `/etc/environment` se escribe con permisos de administrador vía `pkexec` (se pide la contraseña al momento).
- Ajustes persistentes: el ancho del panel izquierdo se guarda entre ejecuciones.

## Uso

```bash
cargo build --release
cp target/release/proxy-manager .
./proxy-manager
```

También puedes ejecutarla directamente con `cargo run --release`.

## Teclas

| Tecla             | Acción                                      |
| ----------------- | ------------------------------------------- |
| `Tab` / `Shift+Tab` | Cambiar el foco entre paneles y campos    |
| `↑` `↓` / `k` `j` | Mover la selección en el historial          |
| `Enter`           | Cargar la configuración seleccionada        |
| `Del` / `d`       | Borrar la configuración seleccionada        |
| `F2`              | Guardar la configuración en el historial    |
| `F3`              | Recargar historial, estado y formulario     |
| `F5`              | Habilitar / deshabilitar el proxy           |
| `Ctrl+←` / `Ctrl+→` | Ajustar el ancho del panel izquierdo      |
| `F1` / `?`        | Ayuda                                       |
| `Ctrl+Q` / `Ctrl+C` | Salir                                     |

## Archivos

- `~/.bashrc` (o `~/.zshrc` / fish según `$SHELL`) y `/etc/environment`: variables de proxy.
- `~/.proxy-manager-history.json`: historial de configuraciones (mismo formato que la versión original).
- `~/.proxy-manager-settings.json`: ajustes de la interfaz (mismo formato que la versión original).

## Requisitos

- Rust 1.88+ (edición 2024)
- `pkexec` (parte de PolicyKit) para escritura en `/etc/environment`
- Una terminal (la aplicación se niega a arrancar sin TTY)
