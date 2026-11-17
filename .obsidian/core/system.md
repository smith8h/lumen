# lumen-core/src/system.rs

Power, session, quick settings and Lumen commands.

- `commands()` — Lock, Log out, Suspend, Restart, Shut down, Dark/Light mode, Wi-Fi, Do Not Disturb, Settings, Empty Trash, plus Lumen ops.
- `system_results(q, history, now)` — fuzzy match.
- `run_system(op)` — loginctl / systemctl / gsettings / nmcli etc. GNOME-aware.
