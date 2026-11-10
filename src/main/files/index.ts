// Main-process side of file search: owns the worker, turns hits into results,
// browses paths typed directly ("~/Doc…") and builds previews.
import { app, nativeImage } from 'electron'
import { existsSync, openSync, readdirSync, readSync, closeSync, statSync } from 'node:fs'
import { homedir } from 'node:os'
import { basename, dirname, extname, join } from 'node:path'
import { Worker } from 'node:worker_threads'
import type { IconSpec, Preview, ResultItem } from '@shared/types'
import { fuzzyMatch } from '../fuzzy'
import { allowServe } from '../icons'
import { frecency, getSettings } from '../store'
import type { FileHit, WorkerRequest, WorkerResponse } from './worker'

let worker: Worker | null = null
let indexed = 0
let nextId = 1
const pending = new Map<number, (hits: FileHit[]) => void>()
let reindexTimer: NodeJS.Timeout | undefined

export function initFiles(): void {
  worker = new Worker(join(__dirname, 'files-worker.js'))
  worker.on('message', (msg: WorkerResponse) => {
    if (msg.type === 'indexed') {
      indexed = msg.count
      console.log(`[files] indexed ${msg.count} entries in ${msg.ms}ms`)
    } else if (msg.type === 'results') {
      pending.get(msg.id)?.(msg.hits)
      pending.delete(msg.id)
    }
  })
  worker.on('error', (err) => console.error('[files] worker error', err))
  reindex()
  reindexTimer = setInterval(reindex, 15 * 60_000)
  reindexTimer.unref()
}

export function reindex(): void {
  const s = getSettings()
  if (!worker || !s.fileSearch) return
  const roots = s.fileRoots.map((r) => r.replace(/^~(?=\/|$)/, homedir())).filter((r) => existsSync(r))
  worker.postMessage({ type: 'index', options: { roots, excludeDirs: s.excludeDirs, includeHidden: s.includeHidden } } satisfies WorkerRequest)
}

export const fileCount = (): number => indexed

function queryWorker(query: string, limit: number): Promise<FileHit[]> {
  if (!worker || !getSettings().fileSearch) return Promise.resolve([])
  const id = nextId++
  return new Promise((resolve) => {
    pending.set(id, resolve)
    worker!.postMessage({ type: 'search', id, query, limit } satisfies WorkerRequest)
    setTimeout(() => {
      if (pending.delete(id)) resolve([])
    }, 400)
  })
}

// ── Icons ──────────────────────────────────────────────────────────────────

const IMAGE_EXT = new Set(['.png', '.jpg', '.jpeg', '.gif', '.webp', '.svg', '.bmp', '.ico', '.avif'])
const iconCache = new Map<string, string>()

async function fileIcon(path: string, dir: boolean): Promise<IconSpec> {
  const ext = dir ? '<dir>' : extname(path).toLowerCase() || '<none>'
  if (!dir && IMAGE_EXT.has(ext)) {
    try {
      if (statSync(path).size < 25 * 1024 * 1024) return { type: 'file', path: allowServe(path) }
    } catch {
      /* fall through to generic icon */
    }
  }
  const cached = iconCache.get(ext)
  if (cached) return { type: 'data', url: cached }
  try {
    const img = await app.getFileIcon(path, { size: 'large' })
    if (!img.isEmpty()) {
      const url = img.toDataURL()
      iconCache.set(ext, url)
      return { type: 'data', url }
    }
  } catch {
    /* use glyph */
  }
  return { type: 'glyph', name: dir ? 'folder' : 'file' }
}

const tildify = (p: string): string => (p.startsWith(homedir()) ? '~' + p.slice(homedir().length) : p)

async function toResult(path: string, dir: boolean, score: number, indexes: number[]): Promise<ResultItem> {
  const key = `file:${path}`
  return {
    id: key,
    kind: dir ? 'folder' : 'file',
    title: basename(path),
    subtitle: tildify(dirname(path)),
    icon: await fileIcon(path, dir),
    score: score + frecency(key),
    matches: indexes,
    historyKey: key,
    action: { type: 'open-path', path },
    actions: [
      { label: dir ? 'Open folder' : 'Open', hint: 'Enter', action: { type: 'open-path', path } },
      { label: 'Show in Files', hint: 'Ctrl+Enter', action: { type: 'reveal-path', path }, glyph: 'folder' },
      { label: 'Copy path', hint: 'Ctrl+C', action: { type: 'copy', text: path }, glyph: 'copy' },
      { label: 'Open terminal here', action: { type: 'open-terminal-at', path: dir ? path : dirname(path) }, glyph: 'terminal' }
    ]
  }
}

/** "~/Documents/rep" or "/etc/hos" → entries of that folder matching the last part. */
async function browse(query: string): Promise<ResultItem[]> {
  const expanded = query.replace(/^~(?=\/|$)/, homedir())
  const isDirQuery = expanded.endsWith('/')
  const folder = isDirQuery ? expanded : dirname(expanded)
  const part = isDirQuery ? '' : basename(expanded)
  let entries
  try {
    entries = readdirSync(folder, { withFileTypes: true })
  } catch {
    return []
  }
  const hits: { path: string; dir: boolean; score: number; idx: number[] }[] = []
  for (const e of entries) {
    if (!part.startsWith('.') && e.name.startsWith('.')) continue
    const m = part ? fuzzyMatch(part, e.name) : { score: 500, indexes: [] }
    if (!m || m.score < 300) continue
    hits.push({ path: join(folder, e.name), dir: e.isDirectory(), score: m.score + (e.isDirectory() ? 20 : 0), idx: m.indexes })
  }
  hits.sort((a, b) => b.score - a.score || a.path.localeCompare(b.path))
  const results = await Promise.all(hits.slice(0, 12).map((h) => toResult(h.path, h.dir, 1800 + h.score / 10, h.idx)))
  if (!part && existsSync(folder)) {
    results.unshift({ ...(await toResult(folder.replace(/\/$/, '') || '/', true, 2400, [])), subtitle: 'Open this folder' })
  }
  return results
}

export async function searchFiles(query: string, limit = 10): Promise<ResultItem[]> {
  const q = query.trim()
  if (/^(~\/|\/|~$)/.test(q)) return browse(q)
  if (q.length < 2) return []
  const hits = await queryWorker(q, limit)
  return Promise.all(hits.map((h) => toResult(h.path, h.dir, h.score * 0.88, h.indexes)))
}

export async function recentFiles(limit: number): Promise<ResultItem[]> {
  const items: ResultItem[] = []
  try {
    const { readFileSync } = await import('node:fs')
    const xml = readFileSync(join(homedir(), '.local/share/recently-used.xbel'), 'utf8')
    const entries = [...xml.matchAll(/<bookmark href="file:\/\/([^"]+)"[^>]*?modified="([^"]+)"/g)]
      .map((m) => ({ path: decodeURIComponent(m[1]), t: Date.parse(m[2]) }))
      .sort((a, b) => b.t - a.t)
    const seen = new Set<string>()
    for (const e of entries) {
      if (seen.has(e.path) || !existsSync(e.path)) continue
      seen.add(e.path)
      const dir = statSync(e.path).isDirectory()
      items.push(await toResult(e.path, dir, 0, []))
      if (items.length >= limit) break
    }
  } catch {
    /* no recent files */
  }
  return items
}

// ── Previews ───────────────────────────────────────────────────────────────

const TEXT_EXT = new Set([
  '.txt', '.md', '.markdown', '.json', '.js', '.mjs', '.cjs', '.ts', '.tsx', '.jsx', '.py', '.rs', '.go', '.c', '.h', '.cpp', '.hpp',
  '.java', '.kt', '.dart', '.rb', '.php', '.sh', '.bash', '.zsh', '.fish', '.yml', '.yaml', '.toml', '.ini', '.conf', '.cfg', '.xml',
  '.html', '.css', '.scss', '.sql', '.csv', '.log', '.env', '.lua', '.swift', '.vue', '.svelte', '.gradle', '.properties', '.desktop', '.service'
])

function looksBinary(buf: Buffer): boolean {
  const n = Math.min(buf.length, 1024)
  for (let i = 0; i < n; i++) if (buf[i] === 0) return true
  return false
}

export async function filePreview(path: string): Promise<Preview> {
  let st
  try {
    st = statSync(path)
  } catch {
    return { type: 'none' }
  }
  const name = basename(path) || path
  if (st.isDirectory()) {
    let entries: { name: string; dir: boolean }[] = []
    try {
      entries = readdirSync(path, { withFileTypes: true })
        .filter((e) => !e.name.startsWith('.'))
        .map((e) => ({ name: e.name, dir: e.isDirectory() }))
        .sort((a, b) => Number(b.dir) - Number(a.dir) || a.name.localeCompare(b.name))
    } catch {
      /* unreadable folder */
    }
    return { type: 'folder', path, name, modified: st.mtimeMs, entries: entries.slice(0, 14), total: entries.length }
  }
  const ext = extname(path).toLowerCase()
  if (IMAGE_EXT.has(ext) && st.size < 25 * 1024 * 1024) {
    let width: number | undefined
    let height: number | undefined
    if (ext !== '.svg') {
      const img = nativeImage.createFromPath(path)
      if (!img.isEmpty()) ({ width, height } = img.getSize())
    }
    return { type: 'image', path: allowServe(path), name, size: st.size, modified: st.mtimeMs, width, height }
  }
  if (st.size < 4 * 1024 * 1024 && (TEXT_EXT.has(ext) || !ext)) {
    const buf = Buffer.alloc(Math.min(st.size, 6000))
    const fd = openSync(path, 'r')
    try {
      readSync(fd, buf, 0, buf.length, 0)
    } finally {
      closeSync(fd)
    }
    if (!looksBinary(buf)) return { type: 'text', path, name, size: st.size, modified: st.mtimeMs, content: buf.toString('utf8'), language: ext.slice(1) }
  }
  return { type: 'file', path, name, size: st.size, modified: st.mtimeMs, icon: await fileIcon(path, false) }
}
