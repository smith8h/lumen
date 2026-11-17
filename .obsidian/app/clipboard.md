# lumen/src/clipboard.rs

`ClipWatch` — clipboard capture. Signal + light poll (some compositors only notify focused app). Never logs contents.

- `apply(settings)` — start/stop by enabled/paused.
- `check`, `on_copy` → `core.add_clip`.
- `mark_secret()` — skip next copy (from extension, password managers).
- `write(text)` — set clipboard without recording.
