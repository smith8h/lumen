# lumen-core/src/clips.rs

`;` mode results and clip previews.

- `clip_results(history, q, settings, now)` — ranked clips, notices when disabled/paused/empty.
- `clip_preview(history, id)` — full text, age, size.
- Actions: paste (`ClipPaste`), copy, pin, delete, clear.

Data: [[cliphistory]].
