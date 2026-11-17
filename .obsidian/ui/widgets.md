# lumen/src/ui/widgets.rs

Custom GTK widgets.

- `Glyph` — draws Lucide path via `gsk::Path` ([[glyph_paths]]). `new`, `with_stroke`, `set_name`.
- `Stage` — positions panel in full-screen window; emits `PanelRect` geometry. `PANEL_WIDTH` 780, `PANEL_RADIUS` 26.
- `AutoHeight` — animated-height container.
- `ListCanvas` — behind result lists, slides selection light (`Rect::lerp`, ease-out).
- Respects `gtk-enable-animations`.
