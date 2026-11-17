# lumen/src/ui/mod.rs

`Panel` — search bar, results, preview, footer, action menu, toasts; plus settings, welcome, output views.

- `BODY_MAX` 420 px body height.
- `primary_action(item)` — label + hint per kind.
- `new`, `wire` — signals, key controller.
- `on_show`, `on_hide`, `on_settings`.
- `run_search` — off-thread `core.search`, `seq` drops stale replies.
- `render_body`, `render_chrome`, `render_footer`.
- `select`, `update_selection`, `schedule_preview`.
- `perform`, `secondary` — run actions via [[app]].
- `copy_text`, `open_url_and_hide`, `flash` (toast).
- Menu: `open_menu`, `close_menu` ([[menu]]).
- Runs: `start_run`, `stop_run`, `rerun`, `run_in_terminal`, `output_text` ([[output]]).
- `press(combo)` — synthetic keys (D-Bus `Key`).
- `handle_key` — arrows, Enter, Tab, Ctrl+K, Esc, Ctrl+, etc.
