# lumen-core/src/debounce.rs

`Debouncer` — background thread runs a fn after quiet period.

- `new(name, delay, f)`, `poke()`, `flush()`.
- Batches writes of `history.json` and `clipboard.json` ([[lib]]).
