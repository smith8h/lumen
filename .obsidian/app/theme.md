# lumen/src/theme.rs

"Lit glass" tokens. GTK 4.14 lacks CSS vars/`color-mix()`, so mixing done here and filled into [[style-css]].

- `Rgba`, `rgba()`, `hex`, `alpha`, `css`, `mix(a, t, b)`.
- `dark()` / `light()` palettes.
- `stylesheet(dark, accent, shell_blur)` — final CSS.
- `TINTS` — glyph tile colors. `swatch_css`, `tile_css`.
- `resolve_dark(mode, system_dark)`.
