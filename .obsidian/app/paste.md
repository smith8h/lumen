# lumen/src/paste.rs

Paste clip into previously focused app.

Order: set clipboard while focused → hide panel → wait → press Ctrl+V (Ctrl+Shift+V for terminals).

- `Backend` — ydotool (Wayland) or xdotool (X11/XWayland). `backend_for(x11_target)`, `forget_backend`.
- `inject(backend, terminal)`, `is_terminal_class(wm_class)`.
- Text never on command line.
