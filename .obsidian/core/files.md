# lumen-core/src/files.rs

File search.

- `FileIndex` — in-memory name index built on background thread. `build(opts)`, `search(q, limit)`, `count()`.
- `IndexOptions` — roots, excludes, include hidden (from [[store]] settings).
- `browse(query, frecency)` — `~/Doc…` path browsing.
- `recent_files(limit)` — reads `recently-used.xbel` (`percent_decode` for URIs).
- `file_result`, `file_preview` — text snippet, folder listing, image, size/mtime.
- Helpers: `extension`, `is_image`, `basename`, `dirname`.
