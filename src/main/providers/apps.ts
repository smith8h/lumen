// Discovers installed applications from .desktop files (freedesktop Desktop Entry spec)
// and launches them the same way the desktop does.
import { existsSync, readdirSync, readFileSync, watch, type FSWatcher } from 'node:fs'
import { homedir } from 'node:os'
import { join } from 'node:path'
import type { Preview, ResultItem } from '@shared/types'
import { cleanEnv, desktopName, spawnDetached, which } from '../env'
import { matchFields } from '../fuzzy'
import { allowServe, releaseThemeIndexes, resolveIcon } from '../icons'
import { frecency } from '../store'
import { dataDirs, localeKeys } from '../xdg'
import { openInTerminal } from './terminal'

export interface DesktopAction {
  id: string
  name: string
  exec: string
  icon?: string
}

export interface DesktopApp {
  id: string
  path: string
  name: string
  genericName?: string
  comment?: string
  keywords: string[]
  categories: string[]
  exec: string
  icon?: string
  iconPath: string | null
  terminal: boolean
  workDir?: string
  actions: DesktopAction[]
}

type Group = Record<string, string>

function parseIni(text: string): Map<string, Group> {
  const groups = new Map<string, Group>()
  let current: Group | null = null
  for (const raw of text.split('\n')) {
    const line = raw.trim()
    if (!line || line.startsWith('#')) continue
    if (line.startsWith('[') && line.endsWith(']')) {
      current = {}
      groups.set(line.slice(1, -1), current)
      continue
    }
    const eq = line.indexOf('=')
    if (eq < 0 || !current) continue
    current[line.slice(0, eq).trim()] = line.slice(eq + 1).trim()
  }
  return groups
}

function unescape(value: string): string {
  return value.replace(/\\([sntr\\;])/g, (_, c: string) => ({ s: ' ', n: '\n', t: '\t', r: '\r', '\\': '\\', ';': ';' })[c] ?? c)
}

function localized(group: Group, key: string): string | undefined {
  for (const loc of localeKeys()) {
    const v = group[`${key}[${loc}]`]
    if (v) return unescape(v)
  }
  return group[key] ? unescape(group[key]) : undefined
}

const list = (v?: string): string[] => (v ? v.split(';').map((s) => s.trim()).filter(Boolean) : [])

function currentDesktops(): string[] {
  return desktopName().split(':').map((d) => d.toLowerCase())
}

function parseDesktopFile(path: string, id: string): DesktopApp | null {
  let text: string
  try {
    text = readFileSync(path, 'utf8')
  } catch {
    return null
  }
  const groups = parseIni(text)
  const e = groups.get('Desktop Entry')
  if (!e || e.Type !== 'Application' || !e.Exec) return null
  if (e.NoDisplay === 'true' || e.Hidden === 'true') return null
  const desktops = currentDesktops()
  const only = list(e.OnlyShowIn).map((s) => s.toLowerCase())
  if (only.length && !only.some((d) => desktops.includes(d))) return null
  const not = list(e.NotShowIn).map((s) => s.toLowerCase())
  if (not.some((d) => desktops.includes(d))) return null
  if (e.TryExec && !which(unescape(e.TryExec))) return null

  const name = localized(e, 'Name')
  if (!name) return null
  const actions: DesktopAction[] = []
  for (const actionId of list(e.Actions)) {
    const g = groups.get(`Desktop Action ${actionId}`)
    const actionName = g && localized(g, 'Name')
    if (g?.Exec && actionName) actions.push({ id: actionId, name: actionName, exec: g.Exec, icon: g.Icon })
  }
  const icon = e.Icon ? unescape(e.Icon) : undefined
  const iconPath = resolveIcon(icon)
  return {
    id,
    path,
    name,
    genericName: localized(e, 'GenericName'),
    comment: localized(e, 'Comment'),
    keywords: list(localized(e, 'Keywords')),
    categories: list(e.Categories),
    exec: e.Exec,
    icon,
    iconPath: iconPath ? allowServe(iconPath) : null,
    terminal: e.Terminal === 'true',
    workDir: e.Path || undefined,
    actions
  }
}

let apps = new Map<string, DesktopApp>()
const watchers: FSWatcher[] = []
let rescanTimer: NodeJS.Timeout | undefined

function scan(): void {
  const found = new Map<string, DesktopApp>()
  const seen = new Set<string>()
  const walk = (dir: string, prefix: string): void => {
    let entries
    try {
      entries = readdirSync(dir, { withFileTypes: true })
    } catch {
      return
    }
    for (const ent of entries) {
      const full = join(dir, ent.name)
      if (ent.isDirectory()) {
        walk(full, `${prefix}${ent.name}-`)
      } else if (ent.name.endsWith('.desktop')) {
        const id = prefix + ent.name
        // Earlier data dirs win (e.g. ~/.local overrides /usr/share).
        if (seen.has(id)) continue
        seen.add(id)
        const app = parseDesktopFile(full, id)
        if (app) found.set(id, app)
      }
    }
  }
  for (const d of dataDirs()) walk(join(d, 'applications'), '')
  apps = found
  releaseThemeIndexes()
}

export function initApps(): void {
  scan()
  // Pick up installs/uninstalls without a restart.
  for (const d of dataDirs()) {
    const dir = join(d, 'applications')
    if (!existsSync(dir)) continue
    try {
      watchers.push(
        watch(dir, () => {
          clearTimeout(rescanTimer)
          rescanTimer = setTimeout(scan, 1500)
        })
      )
    } catch {
      /* inotify limit reached — the periodic rescan still covers it */
    }
  }
  setInterval(scan, 10 * 60_000).unref()
}

export const appCount = (): number => apps.size
export const getApp = (id: string): DesktopApp | undefined => apps.get(id)

function toResult(app: DesktopApp, score: number, matches: number[]): ResultItem {
  return {
    id: `app:${app.id}`,
    kind: 'app',
    title: app.name,
    subtitle: app.comment || app.genericName,
    icon: app.iconPath ? { type: 'file', path: app.iconPath } : { type: 'glyph', name: 'sparkles' },
    score,
    matches,
    historyKey: `app:${app.id}`,
    action: { type: 'launch-app', desktopId: app.id },
    actions: app.actions.map((a) => ({ label: a.name, action: { type: 'launch-app', desktopId: app.id, actionId: a.id } }))
  }
}

export function searchApps(query: string, limit = 8): ResultItem[] {
  const out: ResultItem[] = []
  for (const app of apps.values()) {
    const m = matchFields(query, app.name, [app.genericName, app.keywords.join(' '), app.id.replace(/\.desktop$/, '')], 0.72)
    if (!m || m.score < 200) continue
    out.push(toResult(app, m.score + 150 + frecency(`app:${app.id}`), m.indexes))
  }
  return out.sort((a, b) => b.score - a.score).slice(0, limit)
}

export function frequentApps(limit: number): ResultItem[] {
  return [...apps.values()]
    .map((a) => ({ a, f: frecency(`app:${a.id}`) }))
    .filter((x) => x.f > 0)
    .sort((x, y) => y.f - x.f)
    .slice(0, limit)
    .map(({ a, f }) => toResult(a, f, []))
}

/** Split an Exec line into argv per the spec, expanding the field codes we support. */
export function parseExec(exec: string, app: DesktopApp): string[] {
  const args: string[] = []
  let cur = ''
  let inQuote = false
  let has = false
  for (let i = 0; i < exec.length; i++) {
    const c = exec[i]
    if (inQuote) {
      if (c === '\\' && i + 1 < exec.length) {
        cur += exec[++i]
      } else if (c === '"') {
        inQuote = false
      } else cur += c
    } else if (c === '"') {
      inQuote = true
      has = true
    } else if (c === ' ' || c === '\t') {
      if (has || cur) args.push(cur)
      cur = ''
      has = false
    } else cur += c
  }
  if (has || cur) args.push(cur)

  const out: string[] = []
  for (const arg of args) {
    if (arg === '%i') {
      if (app.icon) out.push('--icon', app.icon)
      continue
    }
    if (/^%[fFuUdDnNvm]$/.test(arg)) continue
    const expanded = arg.replace(/%([%ck])/g, (_, c: string) => (c === '%' ? '%' : c === 'c' ? app.name : app.path)).replace(/%[fFuUdDnNvm]/g, '')
    out.push(expanded)
  }
  return out
}

export async function launchApp(desktopId: string, actionId?: string): Promise<void> {
  const app = apps.get(desktopId)
  if (!app) throw new Error('App no longer installed')

  // `gio launch` handles DBus activation, startup notification and Path= exactly like GNOME.
  if (!actionId && !app.terminal && which('gio')) {
    await spawnDetached('gio', ['launch', app.path])
    return
  }
  const action = actionId ? app.actions.find((a) => a.id === actionId) : undefined
  const argv = parseExec(action?.exec ?? app.exec, app)
  if (!argv.length) throw new Error('Invalid Exec line')
  const cwd = app.workDir && existsSync(app.workDir) ? app.workDir : homedir()
  if (app.terminal) {
    await openInTerminal(argv.map(shellQuote).join(' '), cwd)
    return
  }
  await spawnDetached(argv[0], argv.slice(1), { cwd, env: cleanEnv() })
}

const shellQuote = (s: string): string => (/^[\w@%+=:,./-]+$/.test(s) ? s : `'${s.replace(/'/g, `'\\''`)}'`)

export function appPreview(desktopId: string): Preview {
  const app = apps.get(desktopId)
  if (!app) return { type: 'none' }
  return {
    type: 'app',
    name: app.name,
    comment: app.comment || app.genericName,
    icon: app.iconPath ? { type: 'file', path: app.iconPath } : undefined,
    exec: app.exec.replace(/\s+%[a-zA-Z]/g, ''),
    categories: app.categories.filter((c) => !/^(GTK|GNOME|Qt|KDE|X-)/.test(c)).slice(0, 4),
    path: app.path
  }
}
