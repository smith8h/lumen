# lumen/src/dbus.rs

D-Bus interface on app name/path. Shortcut toggles in ms.

## Methods
Toggle, Show(query), Hide, Quit, PreviousWindow(wm_class, is_x11), ClipboardSecret, Capture(path), Key(combo).

## Signal / property
- `PanelGeometry(x, y, w, h, radius)` — all zeros = closed. For [[gnome-extension]] blur.
- `ShellBlur` (rw bool) — extension draws real blur; Lumen thins tint.

- `Handler` trait, `set_handler`, `register`, `unregister`, `emit_geometry`, `call_running(method)`.
