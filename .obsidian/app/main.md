# lumen/src/main.rs

Entry point. Subclasses `gtk::Application` (`LumenApplication`) to register [[dbus]] on the app's bus name/path.

## CLI flags
- `--help`, `--version`
- `--setup-shortcut`, `--remove-shortcut` — [[shortcut]], no GUI.
- `--quit` — D-Bus call to running instance.
- `--toggle`, `--hidden` — forwarded to running instance.

## Flow
- `startup` → hold app (stays resident), `App::startup`.
- `command_line`: remote → toggle/show. First run → shortcut view. `--hidden` → prime clipboard only.
- `shutdown` → `core.flush()`.
