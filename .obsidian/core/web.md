# lumen-core/src/web.rs

URLs and web search.

- `as_url(query)` — detect URL / domain, add scheme.
- `web_results(q, settings)` — open URL, keyword engines (`WEB_KEYWORDS` in [[constants]]), default engine search.
- `engine`, `engine_label` — chosen engine or custom template (`%s`).
- `encode_component`, `host_of`.
