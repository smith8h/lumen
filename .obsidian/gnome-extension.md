# gnome-extension/

Optional GNOME Shell companion "Lumen Glass" (`lumen@lumenlauncher.github.io`, shell 50). Lumen works without it.

## extension.js
- Watches Lumen bus name. Listens `PanelGeometry` → places blur.
- Keeps Lumen window out of dash, dock, Alt+Tab. Forces keyboard focus.
- Sets `ShellBlur` property.
- Tracks focus → `PreviousWindow(wm_class, is_x11)` for paste ([[app/paste]]).
- Clipboard owner change → `ClipboardSecret` when password-manager mime seen. Never reads text.

## glass.js
- `GlassActor` — GPU blur: copy behind pixels, downscale, gaussian (`SIGMA` 56), shader clips to rounded rect, `SATURATION` 1.6. `MARGIN` 96 px.

## metadata.json
uuid, name, shell-version, version-name.

Protocol: [[app/dbus]]. Pack: `scripts/pack-extension.sh`. Test: `scripts/extension-nested.sh`.
