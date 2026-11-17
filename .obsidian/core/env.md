# lumen-core/src/env.rs

Launch programs like the desktop would.

- `clean_env()` — strip Lumen/dev vars (`dropped_env_names`).
- `which`, `path_executables` (cached set).
- `run`, `run_timeout` → `RunOutput`.
- `spawn_detached(cmd, args, cwd)` — clean env, `setsid`, reaper thread (no zombies).
- `desktop_name`, `is_gnome`, `is_wayland`, `session_type`, `launcher_path`, `shell_quote`.
