// Types shared by the main process, preload bridge and renderer.

export type ResultKind =
  | 'app'
  | 'file'
  | 'folder'
  | 'calc'
  | 'url'
  | 'web'
  | 'command'
  | 'system'
  | 'lumen'

/** How an icon should be drawn. `file` icons are served through the lumen-icon:// protocol. */
export type IconSpec =
  | { type: 'file'; path: string }
  | { type: 'data'; url: string }
  | { type: 'glyph'; name: GlyphName; tint?: string }

export type GlyphName =
  | 'calculator'
  | 'globe'
  | 'search'
  | 'terminal'
  | 'lock'
  | 'logout'
  | 'moon'
  | 'sun'
  | 'power'
  | 'restart'
  | 'settings'
  | 'folder'
  | 'file'
  | 'trash'
  | 'wifi'
  | 'bell'
  | 'refresh'
  | 'quit'
  | 'sparkles'
  | 'copy'
  | 'keyboard'

/** Everything the main process can do on behalf of a result. Validated in main before running. */
export type Action =
  | { type: 'launch-app'; desktopId: string; actionId?: string }
  | { type: 'open-path'; path: string }
  | { type: 'reveal-path'; path: string }
  | { type: 'open-terminal-at'; path: string }
  | { type: 'copy'; text: string }
  | { type: 'open-url'; url: string }
  | { type: 'run-command'; command: string; mode: 'terminal' | 'inline' }
  | { type: 'system'; op: SystemOp }
  | { type: 'lumen'; op: 'settings' | 'quit' | 'reindex' | 'shortcut' }

export type SystemOp =
  | 'lock'
  | 'logout'
  | 'suspend'
  | 'restart'
  | 'shutdown'
  | 'dark-mode'
  | 'light-mode'
  | 'wifi-toggle'
  | 'dnd-toggle'
  | 'settings'
  | 'empty-trash'

export interface SecondaryAction {
  label: string
  /** Keyboard hint rendered as keycaps, e.g. "Ctrl+Enter". */
  hint?: string
  action: Action
  glyph?: GlyphName
}

export interface ResultItem {
  id: string
  kind: ResultKind
  title: string
  subtitle?: string
  icon?: IconSpec
  /** Ranking score; higher is better. */
  score: number
  /** Indexes into `title` that matched the query, for highlighting. */
  matches?: number[]
  action: Action
  actions?: SecondaryAction[]
  /** Ask for a second Enter before running (power actions). */
  confirm?: string
  /** Key used for frecency ranking. */
  historyKey?: string
}

export type SectionId = 'top' | 'calc' | 'apps' | 'commands' | 'files' | 'folders' | 'web' | 'system'

export interface ResultSection {
  id: SectionId
  title: string
  items: ResultItem[]
}

export interface SearchResponse {
  seq: number
  query: string
  sections: ResultSection[]
  /** A hint about what mode the query triggered, shown as a chip in the search bar. */
  mode?: { label: string; glyph: GlyphName }
  tookMs: number
}

export type Preview =
  | { type: 'app'; name: string; comment?: string; icon?: IconSpec; exec?: string; categories?: string[]; path: string }
  | { type: 'image'; path: string; name: string; size: number; modified: number; width?: number; height?: number }
  | { type: 'text'; path: string; name: string; size: number; modified: number; content: string; language?: string }
  | { type: 'folder'; path: string; name: string; modified: number; entries: { name: string; dir: boolean }[]; total: number }
  | { type: 'file'; path: string; name: string; size: number; modified: number; mime?: string; icon?: IconSpec }
  | { type: 'calc'; expression: string; result: string; detail?: string }
  | { type: 'none' }

export interface ExecuteResult {
  ok: boolean
  error?: string
  /** Keep the panel open after running (e.g. inline commands, settings). */
  keepOpen?: boolean
  /** Inline command run id, for streaming output. */
  runId?: string
  /** Switch the panel to another view. */
  view?: 'settings' | 'shortcut'
  toast?: string
}

export interface CommandOutputEvent {
  runId: string
  chunk?: string
  stream?: 'stdout' | 'stderr'
  exitCode?: number | null
  done?: boolean
}

export type ThemeMode = 'system' | 'dark' | 'light'
export type SearchEngineId = 'google' | 'duckduckgo' | 'brave' | 'bing' | 'startpage' | 'custom'

export interface Settings {
  theme: ThemeMode
  accent: string
  /** 0..1 — how see-through the glass is. */
  glassOpacity: number
  searchEngine: SearchEngineId
  customSearchUrl: string
  terminal: string
  fileSearch: boolean
  fileRoots: string[]
  excludeDirs: string[]
  includeHidden: boolean
  launchAtLogin: boolean
  dimBackground: boolean
  showPreview: boolean
  currencyConversion: boolean
  /** Unload the panel's page after it has been hidden for a while. */
  freeMemoryWhenIdle: boolean
  firstRunDone: boolean
}

export interface ShortcutStatus {
  desktop: string
  supported: boolean
  configured: boolean
  binding: string
  command: string
  conflicts: string[]
}

export interface AppInfo {
  version: string
  platform: string
  session: string
  desktop: string
  indexedFiles: number
  indexedApps: number
  home: string
}

/** The panel's rectangle inside Lumen's window, in CSS pixels. */
export interface PanelRect {
  x: number
  y: number
  width: number
  height: number
  radius: number
}

export interface LumenApi {
  search(query: string, seq: number): Promise<SearchResponse>
  suggestions(): Promise<ResultSection[]>
  execute(item: Pick<ResultItem, 'action' | 'historyKey'>): Promise<ExecuteResult>
  preview(item: Pick<ResultItem, 'id' | 'kind' | 'action' | 'title' | 'subtitle'>): Promise<Preview>
  killCommand(runId: string): Promise<void>
  hide(): void
  /** Tell the main process the page is ready to be shown. */
  ready(): void
  getSettings(): Promise<Settings>
  setSettings(patch: Partial<Settings>): Promise<Settings>
  shortcutStatus(): Promise<ShortcutStatus>
  setupShortcut(): Promise<ShortcutStatus>
  removeShortcut(): Promise<ShortcutStatus>
  appInfo(): Promise<AppInfo>
  onShow(cb: (opts: { query?: string; view?: 'settings' | 'shortcut' }) => void): () => void
  onHide(cb: () => void): () => void
  onCommandOutput(cb: (e: CommandOutputEvent) => void): () => void
  onSettings(cb: (s: Settings) => void): () => void
  /** Tell the main process where the panel is (null when closed), for the GNOME extension's blur. */
  reportPanel(rect: PanelRect | null): void
  /** Whether the GNOME extension is drawing real blur behind the panel. */
  shellBlur(): Promise<boolean>
  onShellBlur(cb: (enabled: boolean) => void): () => void
}
