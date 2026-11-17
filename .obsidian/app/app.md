# lumen/src/app.rs

Controller. Owns `Core`, panel window, desktop integrations. Runs actions.

- `View` — Search, Settings, Shortcut.
- `ShowOpts{query, view}`, `ExecResult` (done / toast / err).
- `startup` — load apps, clipboard watch, theme, color-scheme watch, autostart, shortcut refresh.
- `show`, `hide(animated)`, `toggle`, `focus_lost`.
- `schedule_unload` / `unload_panel` — free widgets after idle (`LUMEN_UNLOAD_MS`, default 60 s) when `freeMemoryWhenIdle`.
- `apply_theme`, `is_shell_blur`, `update_settings` (side effects: autostart, clipboard, reindex).
- `execute(action, history_key)` — dispatch every `Action` ([[types]]).
- `paste(text, terminal)` — uses [[paste]].
- `BusHandler` — implements [[dbus]] `Handler`: toggle, show_query, hide, quit, previous_window, clipboard_secret, shell blur, capture (screenshot, testing), key (synthetic keypress).
