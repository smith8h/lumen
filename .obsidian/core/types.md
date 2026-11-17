# lumen-core/src/types.rs

Data shared by providers and UI.

- `ResultKind`: App, File, Folder, Calc, Url, Web, Command, System, Lumen, Clip. Drives section, primary label, footer hints.
- `Glyph`: built-in Lucide icons (Calculator, Globe, Terminal, Lock, Power, Clipboard, Pin, Paste…). Paths in [[glyph_paths]].
- `IconSpec`: `Named`, `File`, `FileType{path,dir}`, `Glyph`.
- `RunMode`: Terminal, Inline.
- `SystemOp`: Lock, Logout, Suspend, Restart, Shutdown, DarkMode, LightMode, WifiToggle, DndToggle, Settings, EmptyTrash.
- `LumenOp`: Settings, Quit, Reindex, Shortcut.
- `Action`: LaunchApp, OpenPath, RevealPath, OpenTerminalAt, Copy, OpenUrl, RunCommand, System, Lumen, ClipPaste{id,terminal}, ClipCopy, ClipPin, ClipDelete, ClipClear. Executed by [[app]] `execute`.
- `SecondaryAction` — builder: `new`, `hint`, `glyph`.
- `ResultItem` — id, kind, title, subtitle, icon, score, match indexes, action, secondary actions.
- `SectionId`, `ResultSection`, `SearchMode`, `SearchResponse{seq, query, sections, mode, took_ms}`.
- `FolderEntry`, `Preview`: App, Image, Text, Folder, File, Calc, Clip, None.
