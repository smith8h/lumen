# lumen-core/src/rates.rs

Exchange rates. Free keyless endpoint via `ureq` (rustls). Only base currency sent.

- Cache `rates.json`, 12 h TTL. Download on background thread.
- `Rates::get()` — cached data or triggers fetch.
- `Rates::offline()`, `with_rates(..)` — for tests.
