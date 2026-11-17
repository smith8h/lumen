# lumen/src/desktop.rs

GIO side.

- `load_apps()` → `Vec<AppEntry>` ([[apps]]).
- `launch_context`, `launch_app(app, action, settings)` — terminal apps via configured terminal.
- `open_uri`, `open_path`, `open_url`, `reveal_path` (FileManager1 D-Bus, fallback open dir).
