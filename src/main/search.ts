// Runs every provider for a query, ranks the results and groups them into sections.
import type { GlyphName, ResultItem, ResultSection, SearchResponse, SectionId } from '@shared/types'
import { SEARCH_ENGINES } from '@shared/constants'
import { frequentApps, getApp, searchApps } from './providers/apps'
import { calculate } from './providers/calculator'
import { clipResults } from './clipboard'
import { bareCommandResult, commandResults } from './providers/terminal'
import { systemResults } from './providers/system'
import { asUrl, webResults } from './providers/web'
import { recentFiles, searchFiles } from './files'
import { getSettings } from './store'

const TITLES: Record<SectionId, string> = {
  top: 'Top Hit',
  calc: 'Calculator',
  clipboard: 'Clipboard',
  apps: 'Applications',
  commands: 'Commands',
  files: 'Files',
  folders: 'Folders',
  web: 'Web',
  system: 'System'
}

const ORDER: SectionId[] = ['clipboard', 'calc', 'top', 'apps', 'system', 'folders', 'files', 'commands', 'web']

function sectionOf(item: ResultItem): SectionId {
  switch (item.kind) {
    case 'app':
      return 'apps'
    case 'file':
      return 'files'
    case 'folder':
      return 'folders'
    case 'calc':
      return 'calc'
    case 'clip':
      return 'clipboard'
    case 'url':
    case 'web':
      return 'web'
    case 'command':
      return 'commands'
    default:
      return 'system'
  }
}

function group(items: ResultItem[], withTopHit: boolean): ResultSection[] {
  const sorted = [...items].sort((a, b) => b.score - a.score)
  const buckets = new Map<SectionId, ResultItem[]>()
  let top: ResultItem | undefined
  if (withTopHit) {
    top = sorted.find((i) => i.kind !== 'calc' && i.kind !== 'web' && i.score >= 640)
  }
  for (const item of sorted) {
    if (item === top) continue
    const id = sectionOf(item)
    const list = buckets.get(id) ?? []
    list.push(item)
    buckets.set(id, list)
  }
  if (top) buckets.set('top', [top])
  return ORDER.filter((id) => buckets.get(id)?.length).map((id) => ({ id, title: TITLES[id], items: buckets.get(id)! }))
}

function engineName(): string {
  const s = getSettings()
  return s.searchEngine === 'custom' ? 'Web' : SEARCH_ENGINES[s.searchEngine].name
}

export async function search(query: string, seq: number): Promise<SearchResponse> {
  const started = performance.now()
  const raw = query
  const q = raw.trim()
  let mode: SearchResponse['mode']
  let items: ResultItem[] = []
  let topHit = true

  if (/^[>$]/.test(q)) {
    mode = { label: 'Terminal', glyph: 'terminal' }
    items = commandResults(q.replace(/^[>$]\s*/, ''))
    topHit = false
  } else if (q.startsWith('?')) {
    mode = { label: engineName(), glyph: 'globe' }
    items = webResults(q.slice(1)).map((i) => ({ ...i, score: i.score + 2000 }))
    topHit = false
  } else if (q.startsWith(';')) {
    // Clipboard history only ever shows up behind its prefix (privacy, and less noise).
    mode = { label: getSettings().clipboardPaused ? 'Clipboard · paused' : 'Clipboard', glyph: 'clipboard' }
    items = clipResults(q.slice(1).trim())
    topHit = false
  } else if (/^(~\/|\/|~$)/.test(q)) {
    mode = { label: 'Files', glyph: 'folder' as GlyphName }
    items = await searchFiles(q, 12)
    topHit = false
  } else if (q) {
    const isUrl = !!asUrl(q)
    const [calc, files] = await Promise.all([calculate(q), isUrl ? Promise.resolve([]) : searchFiles(q, 10)])
    const apps = searchApps(q, 7)
    const system = systemResults(q)
    const web = webResults(q)
    items = [...calc, ...apps, ...system, ...files, ...web]
    if (calc.length) mode = { label: 'Calculator', glyph: 'calculator' }
    else if (isUrl) mode = { label: 'Open URL', glyph: 'globe' }
    const strongApp = apps.some((a) => a.score > 700)
    const bare = !strongApp && !calc.length ? bareCommandResult(q) : null
    if (bare) items.push(bare)
    // Keep sections tidy: cap files and folders separately.
    items = capKinds(items, { file: 6, folder: 4, app: 6, command: 2, web: 3 })
  }

  return { seq, query: raw, sections: group(items, topHit), mode, tookMs: Math.round(performance.now() - started) }
}

function capKinds(items: ResultItem[], caps: Partial<Record<ResultItem['kind'], number>>): ResultItem[] {
  const counts = new Map<string, number>()
  return [...items]
    .sort((a, b) => b.score - a.score)
    .filter((i) => {
      const n = (counts.get(i.kind) ?? 0) + 1
      counts.set(i.kind, n)
      return n <= (caps[i.kind] ?? 99) + (i.kind === 'app' || i.kind === 'file' ? 1 : 0)
    })
}

const STARTER_APPS = [
  'org.gnome.Nautilus.desktop', 'brave-browser.desktop', 'brave_brave.desktop', 'firefox.desktop', 'firefox_firefox.desktop',
  'google-chrome.desktop', 'code.desktop', 'org.gnome.Ptyxis.desktop', 'org.gnome.Terminal.desktop', 'org.gnome.Settings.desktop',
  'org.gnome.TextEditor.desktop', 'org.gnome.Calculator.desktop', 'telegram-desktop.desktop', 'spotify.desktop', 'org.gnome.Software.desktop'
]

export async function suggestions(): Promise<ResultSection[]> {
  const apps = frequentApps(8)
  const ids = new Set(apps.map((a) => a.id))
  for (const id of STARTER_APPS) {
    if (apps.length >= 8) break
    const app = getApp(id)
    if (app && !ids.has(`app:${id}`)) {
      const [item] = searchApps(app.name, 3).filter((r) => r.id === `app:${id}`)
      if (item) {
        apps.push({ ...item, matches: [] })
        ids.add(item.id)
      }
    }
  }
  const sections: ResultSection[] = []
  if (apps.length) sections.push({ id: 'apps', title: 'Suggestions', items: apps })
  const recent = await recentFiles(4)
  if (recent.length) sections.push({ id: 'files', title: 'Recent Files', items: recent })
  return sections
}
