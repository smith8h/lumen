# lumen-core/src/shortcut.rs

Registers Alt+Space. Wayland blocks global grabs, so on GNOME adds a custom keybinding (gsettings) that calls Lumen over D-Bus (`gdbus call … Toggle`). GNOME's own Alt+Space (window menu) moved away, restored on remove.

- `status()`, `setup()`, `remove()` → `ShortcutStatus{configured, binding, desktop, command…}`.
- `refresh_command()` — update binding if launcher path changed.
- `toggle_command`, `shortcut_command`, `pretty_binding`.
- GVariant helpers: `gv_string`, `parse_strv`, `strv`.
- Conflict state saved to JSON for restore.

CLI: `lumen --setup-shortcut`, `--remove-shortcut` ([[main]]). UI: [[welcome]].
