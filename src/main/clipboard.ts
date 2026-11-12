// Clipboard history: captures text you copy, keeps it in clipboard.json (mode 0600) and
// serves it to the ";" search mode. Lives in the main process so it keeps working while
// the panel's page is unloaded. Clipboard contents are never logged.
import { clipboard } from 'electron'
import type { Preview, ResultItem, Settings } from '@shared/types'
import { ClipHistory, clipTitle, type ClipEntry } from './cliphistory'
import { getSettings, readJson, removeJson, writeJson } from './store'
import { primeClipboard } from './window'

const FILE = 'clipboard.json'
const SAVE_DELAY_MS = 1000
const POLL_MS = 700
/** A write Lumen made itself is ignored if the capture source reports it within this window. */
const SELF_WRITE_MS = 5000
const RESULT_LIMIT = 30
const PREVIEW_CHARS = 4000
/** A "that copy was a secret" report this close to a recorded copy refers to it. */
const SECRET_WINDOW_MS = 2000

// ── Capture ────────────────────────────────────────────────────────────────

export interface ClipCopy {
  text: string
  /** The copying app marked the text as a secret (when the source can tell). */
  secret: boolean
}

/** Where copies come from. The history doesn't care how they are noticed. */
export interface ClipSource {
  start(onCopy: (copy: ClipCopy) => void): void
  stop(): void
}

/**
 * Polls the system clipboard. On GNOME Wayland an unfocused (even windowless) Electron app
 * still sees other apps' copies this way, once it has mapped a window (see primeClipboard);
 * checked on GNOME 50 with Electron 45. Chromium only exposes the formats it knows, so this
 * source can't see password managers' x-kde-passwordManagerHint; the GNOME extension reports
 * those separately (markSecretCopy).
 */
class PollingSource implements ClipSource {
  private timer: NodeJS.Timeout | undefined
  private busy = false
  private last: string | null = null

  start(onCopy: (copy: ClipCopy) => void): void {
    if (this.timer) return
    this.timer = setInterval(() => void this.tick(onCopy), POLL_MS)
    void this.tick(onCopy)
  }

  stop(): void {
    clearInterval(this.timer)
    this.timer = undefined
    // Copies made while stopped (e.g. paused) are never recorded: the next start takes a fresh baseline.
    this.last = null
  }

  private async tick(onCopy: (copy: ClipCopy) => void): Promise<void> {
    if (this.busy) return
    this.busy = true
    try {
      const text = await clipboard.readText()
      if (!this.timer) return
      const first = this.last === null
      if (text === this.last) return
      this.last = text
      // On start, remember what's already there without recording it as a new copy.
      if (first) return
      onCopy({ text, secret: false })
    } catch {
      /* clipboard busy or owner gone; try again next tick */
    } finally {
      this.busy = false
    }
  }
}

// ── History ────────────────────────────────────────────────────────────────

const history = new ClipHistory()
let source: ClipSource = new PollingSource()
let capturing = false
let saveTimer: NodeJS.Timeout | undefined
let selfWrite: { text: string; at: number } | null = null
/** When the latest secret copy was reported, and the latest clip recorded (to undo it). */
let secretAt = 0
let lastAdded: { id: string; at: number } | null = null

const wanted = (s: Settings) => s.clipboardEnabled && !s.clipboardPaused

function onCopy({ text, secret }: ClipCopy): void {
  if (selfWrite && Date.now() - selfWrite.at < SELF_WRITE_MS && selfWrite.text === text) {
    selfWrite = null
    return
  }
  const s = getSettings()
  if (!wanted(s)) return
  if (s.clipboardIgnoreSecrets && (secret || Date.now() - secretAt < SECRET_WINDOW_MS)) return
  const entry = history.add(text)
  if (!entry) return
  lastAdded = { id: entry.id, at: Date.now() }
  scheduleSave()
}

/**
 * The GNOME extension saw a copy tagged as a secret by a password manager. It arrives either
 * just before the poll notices the copy (then that copy is skipped) or just after (then the
 * clip it produced is removed again).
 */
export function markSecretCopy(): void {
  if (!getSettings().clipboardIgnoreSecrets) return
  secretAt = Date.now()
  if (lastAdded && secretAt - lastAdded.at < SECRET_WINDOW_MS) {
    if (history.remove(lastAdded.id)) scheduleSave()
    lastAdded = null
  }
}

function scheduleSave(): void {
  if (!getSettings().clipboardPersist) return
  clearTimeout(saveTimer)
  saveTimer = setTimeout(saveNow, SAVE_DELAY_MS)
}

function saveNow(): void {
  clearTimeout(saveTimer)
  saveTimer = undefined
  if (!getSettings().clipboardPersist) return
  try {
    writeJson(FILE, { version: 1, entries: history.all() }, 0o600)
  } catch (err) {
    console.warn('[clipboard] could not save history:', err instanceof Error ? err.message : 'unknown error')
  }
}

/** Load saved history and start capturing if enabled. Call once at startup. */
export function initClipboard(): void {
  const s = getSettings()
  history.setMax(s.clipboardMax)
  if (s.clipboardPersist) history.load(readJson<{ entries?: unknown }>(FILE, {}).entries)
  applyClipboardSettings(s)
}

/** Follow settings changes: start or stop polling, resize, write or delete the file. */
export function applyClipboardSettings(s: Settings): void {
  history.setMax(s.clipboardMax)
  if (wanted(s) && !capturing) {
    capturing = true
    primeClipboard()
    source.start(onCopy)
  } else if (!wanted(s) && capturing) {
    capturing = false
    source.stop()
  }
  if (s.clipboardPersist) scheduleSave()
  else {
    clearTimeout(saveTimer)
    removeJson(FILE)
  }
}

/** Write any pending changes now (on quit). */
export function flushClipboard(): void {
  if (saveTimer) saveNow()
}

/** For tests or a future extension-based source. */
export function setClipSource(next: ClipSource): void {
  if (capturing) source.stop()
  source = next
  if (capturing) source.start(onCopy)
}

/** Put text on the clipboard without recording it as a new copy. */
export async function writeClipboard(text: string): Promise<void> {
  selfWrite = { text, at: Date.now() }
  await clipboard.writeText(text)
}

export const clipCount = (): number => history.size
export const getClip = (id: string): ClipEntry | undefined => history.get(String(id))

export function touchClip(id: string): void {
  if (history.touch(String(id))) scheduleSave()
}

export function pinClip(id: string, pinned: boolean): boolean {
  const ok = history.pin(String(id), !!pinned)
  if (ok) scheduleSave()
  return ok
}

export function deleteClip(id: string): boolean {
  const ok = history.remove(String(id))
  if (ok) scheduleSave()
  return ok
}

export function clearClips(): number {
  const n = history.clear()
  scheduleSave()
  return n
}

// ── Search results and preview ─────────────────────────────────────────────

function ago(ms: number, now = Date.now()): string {
  const s = Math.max(0, Math.round((now - ms) / 1000))
  if (s < 45) return 'Just now'
  const m = Math.round(s / 60)
  if (m < 60) return `${m} min ago`
  const h = Math.round(m / 60)
  if (h < 24) return `${h} h ago`
  const d = Math.round(h / 24)
  if (d < 30) return d === 1 ? 'Yesterday' : `${d} days ago`
  return new Date(ms).toLocaleDateString([], { year: 'numeric', month: 'short', day: 'numeric' })
}

const chars = (n: number) => `${n.toLocaleString()} char${n === 1 ? '' : 's'}`

function clipItem(e: ClipEntry, score: number, matches: number[]): ResultItem {
  const pinAction = { type: 'clip-pin', id: e.id, pinned: !e.pinned } as const
  return {
    id: `clip:${e.id}`,
    kind: 'clip',
    title: clipTitle(e.text),
    subtitle: `${e.pinned ? 'Pinned · ' : ''}${ago(e.lastAt)} · ${chars(e.text.length)}`,
    icon: { type: 'glyph', name: e.pinned ? 'pin' : 'clipboard' },
    score,
    matches,
    action: { type: 'clip-paste', id: e.id },
    actions: [
      { label: 'Copy to clipboard', hint: 'Ctrl+Enter', action: { type: 'clip-copy', id: e.id }, glyph: 'copy' },
      { label: 'Paste into terminal (Ctrl+Shift+V)', action: { type: 'clip-paste', id: e.id, terminal: true }, glyph: 'terminal' },
      { label: e.pinned ? 'Unpin' : 'Pin', hint: 'Ctrl+Shift+P', action: pinAction, glyph: 'pin' },
      { label: 'Delete', hint: 'Ctrl+D', action: { type: 'clip-delete', id: e.id }, glyph: 'trash' }
    ]
  }
}

/** Results for ";query". Never part of normal searches. */
export function clipResults(query: string): ResultItem[] {
  const s = getSettings()
  if (!s.clipboardEnabled) {
    return [
      {
        id: 'clip:off',
        kind: 'lumen',
        title: 'Clipboard history is off',
        subtitle: 'Turn it on in Settings → Clipboard',
        icon: { type: 'glyph', name: 'clipboard' },
        score: 1000,
        action: { type: 'lumen', op: 'settings' }
      }
    ]
  }
  const ranked = history.search(query, RESULT_LIMIT)
  if (!ranked.length && !query.trim()) {
    return [
      {
        id: 'clip:empty',
        kind: 'lumen',
        title: 'Nothing copied yet',
        subtitle: s.clipboardPaused ? 'Recording is paused in Settings → Clipboard' : 'Text you copy in any app shows up here',
        icon: { type: 'glyph', name: 'clipboard' },
        score: 1000,
        action: { type: 'lumen', op: 'settings' }
      }
    ]
  }
  // Keep the list order (pinned, then newest) when there's no query.
  return ranked.map((r, i) => clipItem(r.entry, query.trim() ? 1000 + r.score : 2000 - i, r.matches))
}

export function clipPreview(id: string): Preview {
  const e = history.get(String(id))
  if (!e) return { type: 'none' }
  return {
    type: 'clip',
    text: e.text.length > PREVIEW_CHARS ? e.text.slice(0, PREVIEW_CHARS) : e.text,
    truncated: e.text.length > PREVIEW_CHARS,
    length: e.text.length,
    lines: e.text.split('\n').length,
    firstAt: e.firstAt,
    lastAt: e.lastAt,
    pinned: e.pinned
  }
}
