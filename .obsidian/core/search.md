# lumen-core/src/search.rs

Ranks and groups results into sections.

- `sort_by_score(items)` — descending score.
- `group(items, with_top_hit)` — optional "Top Hit" section, then sections by `SectionId` order.
- `cap_kinds(items, caps)` — limit count per `ResultKind`.
- `section_of`, `title` — kind → section mapping/name.

Called by [[lib]] `search`. Tests: top hit + section order, caps.
