# lumen-core/src/terminal.rs

`> command` mode.

- `command_results(cmd, history)` — typed command first, then history matches.
- `bare_command_result(q)` — offer running a PATH executable when no strong app match.
- `needs_terminal(cmd)` — interactive programs (vim, htop, ssh…) go to terminal.
- `open_in_terminal(settings, cmd, cwd)` — `pick_terminal` from `TERMINAL_CHOICES` / `known_terminals`.
- `run_inline(cmd, emit)` — spawn shell, stream `RunEvent`s (stdout/stderr/exit). `Utf8Chunker` keeps UTF-8 intact.
- `kill_run(pid)` — kill process group.
- `user_shell`, `home_str`.

UI side: [[output]].
