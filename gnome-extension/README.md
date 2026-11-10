# Lumen Glass: GNOME Shell extension

An optional companion for [Lumen](../README.md) on GNOME Shell 50. Install, build and test steps are in the main README under *GNOME extension for real blur*.

| File | What it does |
| --- | --- |
| `extension.js` | Watches Lumen on D-Bus, finds its window, hides it from the window list, focuses it and attaches the glass. |
| `glass.js` | `LumenGlassActor`: the blurred, rounded glass. Uses a custom `Clutter.Effect` that builds the blur from paint nodes and masks it with a GLSL rounded rectangle. |
| `metadata.json` | Extension metadata. |

## D-Bus contract

Lumen owns `io.github.lumenlauncher.Lumen` on the session bus, object `/io/github/lumenlauncher/Lumen`, interface `io.github.lumenlauncher.Lumen`.

| Member | Type | Direction | Meaning |
| --- | --- | --- | --- |
| `PanelGeometry(x, y, w, h, radius)` | signal `(iiiii)` | Lumen → extension | The panel's rectangle in CSS pixels, relative to Lumen's window. All zeros means the panel is closed. Sent on every size change, including during animations. |
| `ShellBlur` | property `b`, read/write | extension → Lumen | `true` while the extension draws real blur. Lumen then lowers its own tint, and re-sends `PanelGeometry` whenever it's set to `true`. |

Lumen's window is the one whose PID owns the bus name (`GetConnectionUnixProcessID`). If that lookup isn't available, the extension falls back to the `lumen` app id with the title `Lumen`.

## Why not `Shell.BlurEffect`?

GNOME's `Shell.BlurEffect` in `BACKGROUND` mode blurs a plain rectangle. Wrapping it in a mask effect doesn't work: the mask renders into an offscreen buffer, so the blur would only see that empty buffer instead of the screen. Stacking several clipped blurs to approximate the corners blurs the overlaps twice. So `glass.js` builds the same pipeline from Clutter paint nodes (`BlitNode` → `LayerNode` → `BlurNode`) and draws the result through a shader that applies an anti-aliased rounded-rectangle mask and saturation. That costs one blur per frame.

On a partial redraw, the pixels outside the redraw clip are last frame's, including Lumen's own panel. The effect reuses its last blur for that frame and queues a full repaint of its area for the next one, so stale pixels never get blurred into the glass.
