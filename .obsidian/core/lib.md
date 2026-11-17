# lumen-core/src/lib.rs

`Core` struct. Shared state between UI thread and search workers.

## Holds
- `Arc<RwLock<Settings>>` — [[store]]
- `Mutex<History>` — frecency, command history
- apps list (`Arc<Vec<AppEntry>>`) — [[apps]]
- `FileIndex` — [[files]]
- `Mutex<ClipHistory>` — [[cliphistory]], saved to `clipboard.json`
- `Rates` — [[rates]]
- `Debouncer`s for history/clip writes — [[debounce]]

## Key methods
- `load()` — read settings, history, clips; start index.
- `update_settings(f)` — returns (old, new).
- `record_use`, `record_command` — frecency/history.
- `reindex()` — rebuild file index on background thread.
- `add_clip`, `touch_clip`, `pin_clip`, `delete_clip`, `clear_clips`.
- `search(raw, seq)` → `SearchResponse`. Routes by prefix:
  - `>`/`$` → [[terminal]]
  - `?` → [[web]] (+2000 score)
  - `;` → [[clips]]
  - `~/`, `/`, `~` → [[files]] browse
  - else: calc + apps + system + files + web + bare command. Caps per kind (File 6, Folder 4, App 6, Command 2, Web 3). Top hit on.
- `suggestions()` — frequent apps (8, padded with `STARTER_APPS`) + 4 recent files.
- `preview(item)` — app / file / clip / calc preview.
- `flush()` — force pending writes.

Query capped at 500 chars.
