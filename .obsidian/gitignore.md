# Git ignore rules

Updated 2026-11-17.

## Ignored
- Rust build: `target/`, `release/`, `dist/`, `*.rs.bk`
- Packages: `.deb`, `.rpm`, `.AppImage`, `.flatpak`, archives, compiled gschemas
- Logs/temp/editor swap, OS junk
- IDEs: `.vscode/`, `.idea/`
- AI tooling: `.claude/`, `.codebuddy/`, `.github/instructions/`, `.windsurfrules`, `.mcp.json`, `CLAUDE.md`, `.code-review-graph/`

## Notes
- AI tooling files untracked via `git rm --cached`; still on disk.
- `.obsidian/` notes stay tracked.
