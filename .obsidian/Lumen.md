# Lumen

Spotlight-style launcher for Linux. Rust, GTK 4 (4.14+), Wayland first. Alt+Space opens a glassy panel.

## Layout
- `crates/lumen-core` — toolkit-free logic. Unit tested with `cargo test`. See [[core/lib]].
- `crates/lumen` — GTK 4 binary. See [[app/main]].
- `gnome-extension/` — optional GNOME Shell companion. See [[gnome-extension]].
- `scripts/`, `packaging/`, `.github/workflows/` — see [[build-and-release]].

## Search prefixes
| Prefix | Mode | Module |
|---|---|---|
| none | apps, files, calc, system, web | [[core/search]] |
| `>` / `$` | terminal command | [[core/terminal]] |
| `?` | web search | [[core/web]] |
| `;` | clipboard history | [[core/clips]] |
| `~/`, `/` | path browsing | [[core/files]] |

## Core notes
[[core/lib]] · [[core/types]] · [[core/search]] · [[core/fuzzy]] · [[core/apps]] · [[core/files]] · [[core/calculator]] · [[core/units]] · [[core/rates]] · [[core/format]] · [[core/web]] · [[core/terminal]] · [[core/system]] · [[core/cliphistory]] · [[core/clips]] · [[core/store]] · [[core/shortcut]] · [[core/integration]] · [[core/env]] · [[core/debounce]] · [[core/constants]] · [[core/xdg]]

## App notes
[[app/main]] · [[app/app]] · [[app/dbus]] · [[app/desktop]] · [[app/clipboard]] · [[app/paste]] · [[app/theme]]

## UI notes
[[ui/mod]] · [[ui/widgets]] · [[ui/bits]] · [[ui/results]] · [[ui/preview]] · [[ui/suggestions]] · [[ui/menu]] · [[ui/output]] · [[ui/settings]] · [[ui/welcome]] · [[ui/glyph_paths]] · [[ui/style-css]]

## Other
[[gnome-extension]] · [[build-and-release]] · [[gitignore]]

## Data files (`~/.config/Lumen`)
`settings.json`, `history.json`, `clipboard.json`, `rates.json`. Override dir with `LUMEN_USER_DATA`.
