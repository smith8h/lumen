# lumen-core/src/store.rs

Settings, history, JSON files in `~/.config/Lumen` (same dir as old Electron version).

- `config_dir`, `read_json`, `write_json(file, data, mode)` (atomic, optional perms), `remove_json`, `file_path`, `now_ms`.
- `ThemeMode`, `SearchEngine` (Google, DuckDuckGo, Brave, Bing, Startpage, Custom).
- `Settings` (camelCase JSON): theme, accent, searchEngine, customSearchUrl, terminal, fileSearch, fileRoots, excludeDirs, includeHidden, launchAtLogin, showPreview, currencyConversion, freeMemoryWhenIdle, clipboard{Enabled,Persist,Max,IgnoreSecrets,Paused}, firstRunDone.
  - `from_map` — bad values fall back per field. `sanitized`, `save`.
- `DEFAULT_EXCLUDES` — node_modules, .git, .cache, .npm, .cargo, .rustup, .gradle…
- `History` — `record_use`, `frecency`, `record_command`.
- `tilde`, `expand_tilde`, `exists`.
