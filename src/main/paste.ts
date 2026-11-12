// Paste a clip into the app that was focused before Lumen opened.
//
// Order matters: put the text on the clipboard while Lumen still has focus (Wayland only
// lets the focused app set it), hide the panel so focus goes back, wait for that to settle,
// then press Ctrl+V with ydotool (Wayland, any window) or xdotool (X11 / XWayland windows).
// The text itself never touches a command line. If nothing can press keys, the text is
// simply left on the clipboard with a hint to press Ctrl+V.
import { spawn } from 'node:child_process'
import { existsSync } from 'node:fs'
import { Notification } from 'electron'
import type { ExecuteResult } from '@shared/types'
import { APP_NAME } from '@shared/constants'
import { cleanEnv, which } from './env'
import { writeClipboard } from './clipboard'
import { KNOWN_TERMINALS } from './providers/terminal'
import { hidePanel, lastShownAt } from './window'

/** Panel close animation (120 ms) plus time for the compositor to hand focus back. */
const PASTE_SETTLE_MS = 240
const INJECT_TIMEOUT_MS = 2000
const FALLBACK_TOAST = 'Copied — press Ctrl+V'

type Backend = 'ydotool' | 'xdotool'

const isWayland = () => process.env.XDG_SESSION_TYPE === 'wayland'

function ydotoolSocket(): string | null {
  const env = cleanEnv()
  const path = env.YDOTOOL_SOCKET || (env.XDG_RUNTIME_DIR ? `${env.XDG_RUNTIME_DIR}/.ydotool_socket` : '')
  return path && existsSync(path) ? path : null
}

let cached: Backend | null = null

/** The best way to press keys in other apps right now (null = none). Only a found backend is cached. */
function backendFor(x11Target: boolean): Backend | null {
  if (cached === 'ydotool' || (cached === 'xdotool' && !isWayland())) return cached
  if (which('ydotool') && ydotoolSocket()) return (cached = 'ydotool')
  if (!isWayland() && which('xdotool')) return (cached = 'xdotool')
  // Without ydotool on Wayland, xdotool can still reach an XWayland window.
  if (isWayland() && x11Target && which('xdotool')) return 'xdotool'
  return null
}

// Linux input event codes for ydotool.
const KEY_CTRL = 29
const KEY_SHIFT = 42
const KEY_V = 47

function argsFor(backend: Backend, terminal: boolean): string[] {
  if (backend === 'xdotool') return ['key', '--clearmodifiers', terminal ? 'ctrl+shift+v' : 'ctrl+v']
  const down = terminal ? [KEY_CTRL, KEY_SHIFT, KEY_V] : [KEY_CTRL, KEY_V]
  return ['key', ...down.map((k) => `${k}:1`), ...[...down].reverse().map((k) => `${k}:0`)]
}

function inject(backend: Backend, terminal: boolean): Promise<boolean> {
  return new Promise((resolve) => {
    try {
      const child = spawn(backend, argsFor(backend, terminal), { env: cleanEnv(), stdio: 'ignore' })
      const timer = setTimeout(() => {
        child.kill('SIGKILL')
        resolve(false)
      }, INJECT_TIMEOUT_MS)
      child.once('error', () => {
        clearTimeout(timer)
        resolve(false)
      })
      child.once('exit', (code) => {
        clearTimeout(timer)
        resolve(code === 0)
      })
    } catch {
      resolve(false)
    }
  })
}

// ── The window Lumen was opened over (reported by the GNOME extension) ─────

let previous: { wmClass: string; x11: boolean; at: number } | null = null

/** The GNOME extension tells us which window had focus before Lumen's panel took it. */
export function setPreviousWindow(wmClass: string, x11: boolean): void {
  previous = { wmClass: String(wmClass).slice(0, 256), x11: !!x11, at: Date.now() }
}

/** Only trust a report made since the panel was last shown. */
function previousWindow(): { wmClass: string; x11: boolean } | null {
  return previous && previous.at >= lastShownAt() - 1000 ? previous : null
}

const TERMINAL_CLASSES = new Set([...KNOWN_TERMINALS, 'console', 'terminal', 'gnome-terminal-server', 'terminator', 'tilda', 'guake', 'yakuake', 'rio', 'contour', 'blackbox', 'terminology'])

/** "org.gnome.Ptyxis" → ptyxis, "com.mitchellh.ghostty" → ghostty, "org.gnome.Console" → console. */
export function isTerminalClass(wmClass: string): boolean {
  const c = wmClass.toLowerCase()
  return TERMINAL_CLASSES.has(c) || TERMINAL_CLASSES.has(c.split('.').pop() ?? '')
}

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms))

/** Copy `text`, return focus to the previous window and paste it there. Never throws. */
export async function pasteText(text: string, opts: { terminal?: boolean } = {}): Promise<ExecuteResult> {
  try {
    await writeClipboard(text)
  } catch {
    return { ok: false, error: 'Could not write to the clipboard' }
  }
  const prev = previousWindow()
  const backend = backendFor(!!prev?.x11)
  // Nothing can press Ctrl+V for us: leave it on the clipboard; the renderer shows the hint, then hides.
  if (!backend) return { ok: true, toast: FALLBACK_TOAST }

  const terminal = opts.terminal ?? (prev ? isTerminalClass(prev.wmClass) : false)
  hidePanel(true)
  await sleep(PASTE_SETTLE_MS)
  const ok = await inject(backend, terminal)
  if (!ok) {
    if (backend === cached) cached = null
    console.warn(`[paste] ${backend} failed; leaving the text on the clipboard`)
    if (Notification.isSupported()) new Notification({ title: APP_NAME, body: FALLBACK_TOAST, silent: true }).show()
  }
  // The panel is already on its way out; tell the renderer not to do anything else.
  return { ok: true, keepOpen: true }
}
