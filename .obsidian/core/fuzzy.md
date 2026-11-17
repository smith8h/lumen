# lumen-core/src/fuzzy.rs

Fast launcher-style fuzzy scorer.

Priority: exact > prefix > word prefix > word initials (`vsc` → Visual Studio Code) > substring > subsequence.
Indexes are char positions (not bytes). camelCase counts as word boundary.

- `fuzzy_match(query, text) -> Option<Match{score, indexes}>`
- `match_fields(query, title, extra, extra_weight)` — title plus weighted extra fields (keywords, generic name).
- `lower(c)` — crate-visible lowercase helper.

Used by [[apps]], [[files]], [[system]], [[cliphistory]], [[terminal]].
