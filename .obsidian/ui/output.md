# lumen/src/ui/output.rs

Inline command output (`> ls -la`).

- `RunState` — pid, running, exit code.
- `strip_ansi`.
- `OutputView` — `append(text, stderr)` (autoscroll if at bottom), `refresh(run)`, `has_selection`.

Process: [[terminal]] `run_inline`.
