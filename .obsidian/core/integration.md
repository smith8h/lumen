# lumen-core/src/integration.rs

Start at login.

- `set_autostart(enabled)` — writes/removes `~/.config/autostart/lumen.desktop` (`--hidden`).
- `is_dev_build()` — exe path contains `/target/`.
