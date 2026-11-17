# lumen-core/src/cliphistory.rs

Clipboard history as plain data.

- `ClipEntry` — id, text, title, created/last used, pinned.
- Limits: `MAX_CLIP_BYTES` 100 KB, `TITLE_CHARS` 80. `acceptable(text)` skips blank/oversized.
- `ClipHistory`: `load`, `add` (dedupe → moves to top), `set_max`, `evict` (pinned never dropped, not counted in cap), `touch`, `pin`, `remove`, `clear`, `recent` (pinned first), `search` (fuzzy + `recency_boost`).
- `random_id` via `getrandom`.

Wrapped by [[lib]]. Results: [[clips]]. Capture: [[clipboard]].
