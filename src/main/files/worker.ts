// File index running in a worker thread: walks the configured folders, keeps a compact
// in-memory table of names, and answers fuzzy queries in a few milliseconds.
import { readdirSync, readFileSync } from 'node:fs'
import { homedir } from 'node:os'
import { join, relative, sep } from 'node:path'
import { parentPort } from 'node:worker_threads'
import { fuzzyMatch } from '../fuzzy'

export interface IndexOptions {
  roots: string[]
  excludeDirs: string[]
  includeHidden: boolean
}

export interface FileHit {
  path: string
  name: string
  dir: boolean
  score: number
  indexes: number[]
}

export type WorkerRequest =
  | { type: 'index'; options: IndexOptions }
  | { type: 'search'; id: number; query: string; limit: number }

export type WorkerResponse =
  | { type: 'indexed'; count: number; ms: number }
  | { type: 'results'; id: number; hits: FileHit[] }

const MAX_ENTRIES = 500_000
const MAX_DEPTH = 14

let names: string[] = []
let lower: string[] = []
let parent = new Int32Array(0)
let depth = new Uint8Array(0)
let isDir = new Uint8Array(0)
/** Absolute path of each root, indexed by negative parent ids (-1 → roots[0]). */
let rootPaths: string[] = []
let recent = new Set<string>()

function loadRecent(): Set<string> {
  const set = new Set<string>()
  try {
    const xml = readFileSync(join(homedir(), '.local/share/recently-used.xbel'), 'utf8')
    for (const m of xml.matchAll(/href="file:\/\/([^"]+)"/g)) set.add(decodeURIComponent(m[1]))
  } catch {
    /* no recent files list */
  }
  return set
}

function buildIndex(opts: IndexOptions): number {
  const n: string[] = []
  const p: number[] = []
  const d: number[] = []
  const dirFlags: number[] = []
  const excludeNames = new Set(opts.excludeDirs.filter((e) => !e.includes('/') && !e.startsWith('*')))
  const excludeSuffixes = opts.excludeDirs.filter((e) => e.startsWith('*')).map((e) => e.slice(1))
  const excludePaths = opts.excludeDirs.filter((e) => e.includes('/'))
  rootPaths = [...new Set(opts.roots)]

  // Breadth-first, so a huge deep folder can't starve shallow, more useful files.
  const queue: { abs: string; id: number; depth: number; root: string }[] = rootPaths.map((abs, i) => ({ abs, id: -(i + 1), depth: 0, root: abs }))
  for (let qi = 0; qi < queue.length && n.length < MAX_ENTRIES; qi++) {
    const { abs, id, depth: lvl, root } = queue[qi]
    let entries
    try {
      entries = readdirSync(abs, { withFileTypes: true })
    } catch {
      continue
    }
    for (const e of entries) {
      if (n.length >= MAX_ENTRIES) break
      const name = e.name
      if (!opts.includeHidden && name.startsWith('.')) continue
      const dir = e.isDirectory()
      if (!dir && !e.isFile()) continue
      if (dir && (excludeNames.has(name) || excludeSuffixes.some((s) => name.endsWith(s)))) continue
      const full = abs + sep + name
      if (dir && excludePaths.length) {
        const rel = relative(root, full)
        if (excludePaths.some((x) => rel === x || rel.startsWith(x + sep))) continue
      }
      const idx = n.length
      n.push(name)
      p.push(id)
      d.push(lvl + 1)
      dirFlags.push(dir ? 1 : 0)
      if (dir && lvl + 1 < MAX_DEPTH) queue.push({ abs: full, id: idx, depth: lvl + 1, root })
    }
  }
  names = n
  lower = n.map((s) => s.toLowerCase())
  parent = Int32Array.from(p)
  depth = Uint8Array.from(d)
  isDir = Uint8Array.from(dirFlags)
  recent = loadRecent()
  return n.length
}

function pathOf(i: number): string {
  const parts: string[] = []
  let cur = i
  while (cur >= 0) {
    parts.push(names[cur])
    cur = parent[cur]
  }
  return rootPaths[-cur - 1] + sep + parts.reverse().join(sep)
}

const HOME_FOLDERS = new Set(['Desktop', 'Documents', 'Downloads', 'Music', 'Pictures', 'Videos', 'Projects', 'Public', 'Templates'])

function search(query: string, limit: number): FileHit[] {
  const q = query.trim().toLowerCase()
  if (!q) return []
  const scored: { i: number; score: number; indexes: number[] }[] = []
  const started = performance.now()

  const consider = (i: number, base: number, indexes: number[]): void => {
    let score = base - depth[i] * 14
    if (isDir[i]) score += 25
    if (depth[i] === 1 && HOME_FOLDERS.has(names[i])) score += 120
    if (names[i].startsWith('.')) score -= 80
    scored.push({ i, score, indexes })
  }

  // Pass 1: substring hits (fast path covers most queries).
  for (let i = 0; i < lower.length; i++) {
    if (lower[i].includes(q)) {
      const m = fuzzyMatch(q, names[i])
      if (m) consider(i, m.score, m.indexes)
    }
  }
  // Pass 2: initials / subsequence, time-boxed so typing stays instant.
  if (scored.length < limit * 3 && q.length >= 3 && !q.includes(' ')) {
    const first = q[0]
    const seen = new Set(scored.map((s) => s.i))
    for (let i = 0; i < lower.length; i++) {
      if ((i & 4095) === 0 && performance.now() - started > 45) break
      if (seen.has(i) || !lower[i].includes(first)) continue
      const m = fuzzyMatch(q, names[i])
      if (m && m.score >= 300) consider(i, m.score - 60, m.indexes)
    }
  }

  scored.sort((a, b) => b.score - a.score)
  const top = scored.slice(0, limit * 4).map((s) => {
    const path = pathOf(s.i)
    return { path, name: names[s.i], dir: isDir[s.i] === 1, score: s.score + (recent.has(path) ? 160 : 0), indexes: s.indexes }
  })
  return top.sort((a, b) => b.score - a.score).slice(0, limit)
}

parentPort?.on('message', (msg: WorkerRequest) => {
  if (msg.type === 'index') {
    const t = performance.now()
    const count = buildIndex(msg.options)
    parentPort!.postMessage({ type: 'indexed', count, ms: Math.round(performance.now() - t) } satisfies WorkerResponse)
  } else if (msg.type === 'search') {
    parentPort!.postMessage({ type: 'results', id: msg.id, hits: search(msg.query, msg.limit) } satisfies WorkerResponse)
  }
})
