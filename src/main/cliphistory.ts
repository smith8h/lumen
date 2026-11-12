// Clipboard history as plain data: dedupe, size cap, pinning and ranking.
// No Electron in here, so it can be unit-tested with `npm test`.
import { randomBytes } from 'node:crypto'
import { matchFields } from './fuzzy'

export interface ClipEntry {
  id: string
  text: string
  /** When this text was first copied. */
  firstAt: number
  /** When it was last copied or used. */
  lastAt: number
  pinned: boolean
}

/** Bigger copies (log dumps, whole files) are skipped. */
export const MAX_CLIP_BYTES = 100 * 1024
export const TITLE_CHARS = 80
/** How much of a clip the fuzzy matcher looks at beyond its first line. */
const SEARCH_CHARS = 2000

export function acceptable(text: unknown): text is string {
  return typeof text === 'string' && /\S/.test(text) && Buffer.byteLength(text, 'utf8') <= MAX_CLIP_BYTES
}

/** First non-blank line, whitespace collapsed, about 80 characters. */
export function clipTitle(text: string): string {
  const line = text.split('\n').find((l) => /\S/.test(l)) ?? ''
  const flat = line.replace(/\s+/g, ' ').trim()
  return flat.length > TITLE_CHARS ? flat.slice(0, TITLE_CHARS - 1).trimEnd() + '…' : flat
}

/** Small bonus so recent clips win ties: 0..60. */
export function recencyBoost(lastAt: number, now: number): number {
  const hours = (now - lastAt) / 3_600_000
  return hours < 1 ? 60 : hours < 24 ? 40 : hours < 24 * 7 ? 20 : 0
}

export interface RankedClip {
  entry: ClipEntry
  score: number
  /** Matched character indexes in clipTitle(entry.text). */
  matches: number[]
}

const newId = (): string => randomBytes(6).toString('hex')

export class ClipHistory {
  /** Newest first. */
  private entries: ClipEntry[] = []
  private max: number
  private makeId: () => string

  constructor(max = 200, makeId: () => string = newId) {
    this.max = max
    this.makeId = makeId
  }

  /** Replace everything, e.g. from disk. Bad or oversized entries are dropped. */
  load(raw: unknown): void {
    const list = Array.isArray(raw) ? raw : []
    const seen = new Set<string>()
    const ids = new Set<string>()
    this.entries = []
    for (const e of list as Partial<ClipEntry>[]) {
      if (!e || !acceptable(e.text) || seen.has(e.text)) continue
      const id = typeof e.id === 'string' && /^[\w-]{1,64}$/.test(e.id) && !ids.has(e.id) ? e.id : this.makeId()
      const lastAt = Number.isFinite(e.lastAt) ? Number(e.lastAt) : 0
      const firstAt = Number.isFinite(e.firstAt) ? Number(e.firstAt) : lastAt
      seen.add(e.text)
      ids.add(id)
      this.entries.push({ id, text: e.text, firstAt, lastAt, pinned: e.pinned === true })
    }
    this.entries.sort((a, b) => b.lastAt - a.lastAt)
    this.evict()
  }

  /** Record a copy. The same text moves to the top. Returns null when the text is skipped. */
  add(text: string, now = Date.now()): ClipEntry | null {
    if (!acceptable(text)) return null
    const i = this.entries.findIndex((e) => e.text === text)
    let entry: ClipEntry
    if (i >= 0) {
      entry = this.entries[i]
      this.entries.splice(i, 1)
      entry.lastAt = now
    } else {
      entry = { id: this.makeId(), text, firstAt: now, lastAt: now, pinned: false }
    }
    this.entries.unshift(entry)
    this.evict()
    return entry
  }

  setMax(max: number): void {
    this.max = Math.max(1, Math.floor(max))
    this.evict()
  }

  /** Drop the oldest unpinned clips beyond the cap. Pinned clips are never dropped and don't use up the cap. */
  private evict(): void {
    let over = this.entries.filter((e) => !e.pinned).length - this.max
    for (let i = this.entries.length - 1; i >= 0 && over > 0; i--) {
      if (this.entries[i].pinned) continue
      this.entries.splice(i, 1)
      over--
    }
  }

  get(id: string): ClipEntry | undefined {
    return this.entries.find((e) => e.id === id)
  }

  /** Mark a clip as just used, so it moves to the top. */
  touch(id: string, now = Date.now()): boolean {
    const i = this.entries.findIndex((e) => e.id === id)
    if (i < 0) return false
    const [entry] = this.entries.splice(i, 1)
    entry.lastAt = now
    this.entries.unshift(entry)
    return true
  }

  pin(id: string, pinned: boolean): boolean {
    const e = this.get(id)
    if (!e) return false
    e.pinned = pinned
    this.evict()
    return true
  }

  remove(id: string): boolean {
    const before = this.entries.length
    this.entries = this.entries.filter((e) => e.id !== id)
    return this.entries.length !== before
  }

  /** Forget everything except pinned clips. Returns how many were removed. */
  clear(): number {
    const before = this.entries.length
    this.entries = this.entries.filter((e) => e.pinned)
    return before - this.entries.length
  }

  get size(): number {
    return this.entries.length
  }

  all(): readonly ClipEntry[] {
    return this.entries
  }

  /** What `;` shows before typing: pinned first, then newest. */
  recent(limit: number): ClipEntry[] {
    return [...this.entries.filter((e) => e.pinned), ...this.entries.filter((e) => !e.pinned)].slice(0, limit)
  }

  /** Fuzzy search over the first line (highlighted) and the start of the text, with a small recency boost. */
  search(query: string, limit: number, now = Date.now()): RankedClip[] {
    const q = query.trim()
    if (!q) return this.recent(limit).map((entry) => ({ entry, score: 0, matches: [] }))
    const out: RankedClip[] = []
    for (const entry of this.entries) {
      const title = clipTitle(entry.text)
      const body = entry.text.length > SEARCH_CHARS ? entry.text.slice(0, SEARCH_CHARS) : entry.text
      const m = matchFields(q, title, [body.replace(/\s+/g, ' ')], 0.85)
      if (!m || m.score < 300) continue
      out.push({ entry, score: m.score + recencyBoost(entry.lastAt, now) + (entry.pinned ? 15 : 0), matches: m.indexes })
    }
    return out.sort((a, b) => b.score - a.score || b.entry.lastAt - a.entry.lastAt).slice(0, limit)
  }
}
