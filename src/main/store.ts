import { app } from 'electron'
import { chmodSync, existsSync, mkdirSync, readFileSync, renameSync, rmSync, writeFileSync } from 'node:fs'
import { homedir } from 'node:os'
import { join } from 'node:path'
import type { Settings } from '@shared/types'

const dir = () => app.getPath('userData')

export function readJson<T>(file: string, fallback: T): T {
  try {
    return JSON.parse(readFileSync(join(dir(), file), 'utf8')) as T
  } catch {
    return fallback
  }
}

/** Write atomically so a crash never leaves half a file behind. `mode` restricts who can read it. */
export function writeJson(file: string, data: unknown, mode?: number): void {
  if (!existsSync(dir())) mkdirSync(dir(), { recursive: true })
  const target = join(dir(), file)
  writeFileSync(target + '.tmp', JSON.stringify(data, null, 2), mode === undefined ? undefined : { mode })
  // `mode` only applies when the file is created; a leftover .tmp keeps its old permissions.
  if (mode !== undefined) chmodSync(target + '.tmp', mode)
  renameSync(target + '.tmp', target)
}

export function removeJson(file: string): void {
  rmSync(join(dir(), file), { force: true })
}

export const DEFAULT_SETTINGS: Settings = {
  theme: 'system',
  accent: '#7c6cff',
  searchEngine: 'google',
  customSearchUrl: 'https://example.com/search?q=%s',
  terminal: 'auto',
  fileSearch: true,
  fileRoots: [homedir()],
  excludeDirs: [
    'node_modules', '.git', '.cache', '.npm', '.cargo', '.rustup', '.gradle', '.m2', '.pub-cache',
    '.venv', 'venv', '__pycache__', '.next', '.nuxt', '.dart_tool', 'build', 'target', 'snap', '.var',
    'Trash', '.local/share/Trash', 'Android/Sdk', '.android', '.sdks', '.vscode-server', '.wine',
    '*.asar.unpacked', 'site-packages', 'dist-packages', '.flatpak-builder'
  ],
  includeHidden: false,
  launchAtLogin: true,
  showPreview: true,
  currencyConversion: true,
  freeMemoryWhenIdle: true,
  clipboardEnabled: true,
  clipboardPersist: true,
  clipboardMax: 200,
  clipboardIgnoreSecrets: true,
  clipboardPaused: false,
  firstRunDone: false
}

let settings: Settings = { ...DEFAULT_SETTINGS }
const listeners = new Set<(s: Settings) => void>()

export function loadSettings(): Settings {
  const saved = readJson<Record<string, unknown>>('settings.json', {})
  // Keep only keys Lumen still knows, so retired options (e.g. glassOpacity) drop out on the next save.
  const known = Object.fromEntries(Object.entries(saved).filter(([k]) => k in DEFAULT_SETTINGS)) as Partial<Settings>
  settings = { ...DEFAULT_SETTINGS, ...known }
  return settings
}

export const getSettings = (): Settings => settings

export function updateSettings(patch: Partial<Settings>): Settings {
  settings = { ...settings, ...patch }
  writeJson('settings.json', settings)
  for (const l of listeners) l(settings)
  return settings
}

export function onSettingsChange(fn: (s: Settings) => void): void {
  listeners.add(fn)
}

// ── Frecency: things you open often and recently rank higher ──────────────

interface HistoryEntry {
  count: number
  last: number
}

let history: Record<string, HistoryEntry> = {}
let commandHistory: string[] = []
let saveTimer: NodeJS.Timeout | undefined

export function loadHistory(): void {
  const data = readJson<{ items?: Record<string, HistoryEntry>; commands?: string[] }>('history.json', {})
  history = data.items ?? {}
  commandHistory = data.commands ?? []
}

function scheduleSave(): void {
  clearTimeout(saveTimer)
  saveTimer = setTimeout(() => writeJson('history.json', { items: history, commands: commandHistory }), 500)
}

export function recordUse(key: string): void {
  const e = history[key] ?? { count: 0, last: 0 }
  e.count += 1
  e.last = Date.now()
  history[key] = e
  // Keep the file small.
  const keys = Object.keys(history)
  if (keys.length > 2000) {
    keys.sort((a, b) => history[a].last - history[b].last)
    for (const k of keys.slice(0, keys.length - 1500)) delete history[k]
  }
  scheduleSave()
}

/** 0..~180 bonus based on how often and how recently something was used. */
export function frecency(key: string | undefined): number {
  if (!key) return 0
  const e = history[key]
  if (!e) return 0
  const ageDays = (Date.now() - e.last) / 86_400_000
  const recency = ageDays < 1 ? 1 : ageDays < 7 ? 0.7 : ageDays < 30 ? 0.45 : 0.2
  return Math.min(180, Math.log2(e.count + 1) * 45 * recency)
}

export function topHistory(prefix: string, limit: number): string[] {
  return Object.entries(history)
    .filter(([k]) => k.startsWith(prefix))
    .sort((a, b) => frecency(b[0]) - frecency(a[0]))
    .slice(0, limit)
    .map(([k]) => k.slice(prefix.length))
}

export function recordCommand(cmd: string): void {
  commandHistory = [cmd, ...commandHistory.filter((c) => c !== cmd)].slice(0, 100)
  scheduleSave()
}

export const getCommandHistory = (): string[] => commandHistory
