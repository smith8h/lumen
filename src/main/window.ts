// The panel window. On Wayland apps can't choose where their window goes, so Lumen uses a
// transparent window covering the screen and draws the panel itself at Spotlight's position.
// A click on the transparent area (or focus moving elsewhere) closes it.
import { BrowserWindow, screen } from 'electron'
import { join } from 'node:path'
import { APP_NAME } from '@shared/constants'

let win: BrowserWindow | null = null
let ready: Promise<void> | null = null
let markReady: (() => void) | null = null
let shownAt = 0
let hideTimer: NodeJS.Timeout | undefined
let unloadTimer: NodeJS.Timeout | undefined
let iconPath: string | undefined
/** How long the panel may sit hidden before its page is unloaded to free memory (null = never). */
let unloadAfterMs: number | null = null
const isWayland = () => process.env.XDG_SESSION_TYPE === 'wayland' && !process.env.LUMEN_X11
const debug = !!process.env.LUMEN_DEBUG

export function createPanel(icon?: string): BrowserWindow {
  if (icon) iconPath = icon
  const display = screen.getPrimaryDisplay()
  ready = new Promise((resolve) => (markReady = resolve))
  win = new BrowserWindow({
    title: APP_NAME,
    width: display.size.width,
    height: display.size.height,
    x: display.bounds.x,
    y: display.bounds.y,
    show: false,
    frame: false,
    transparent: true,
    backgroundColor: '#00000000',
    hasShadow: false,
    resizable: false,
    movable: false,
    minimizable: false,
    maximizable: false,
    fullscreenable: false,
    skipTaskbar: true,
    alwaysOnTop: true,
    icon: iconPath,
    webPreferences: {
      preload: join(__dirname, '../preload/index.js'),
      sandbox: true,
      contextIsolation: true,
      nodeIntegration: false,
      spellcheck: false,
      // While hidden, let Chromium drop the page's graphics buffers (saves ~50 MB of GPU memory).
      backgroundThrottling: true
    }
  })
  win.setMenu(null)

  if (process.env.ELECTRON_RENDERER_URL) win.loadURL(process.env.ELECTRON_RENDERER_URL)
  else win.loadFile(join(__dirname, '../renderer/index.html'))

  win.on('blur', () => {
    // Ignore the focus shuffle that can happen right as the window maps.
    if (debug || Date.now() - shownAt < 200) return
    hidePanel(false)
  })
  win.on('close', (e) => {
    // Closing (e.g. Alt+F4) just hides; Lumen keeps running for the next Alt+Space.
    if (!quitting) {
      e.preventDefault()
      hidePanel(false)
    }
  })
  win.webContents.setWindowOpenHandler(() => ({ action: 'deny' }))
  win.webContents.on('will-navigate', (e) => e.preventDefault())
  if (debug) win.webContents.openDevTools({ mode: 'detach' })
  return win
}

let quitting = false
export const setQuitting = (): void => {
  quitting = true
}

export const panel = (): BrowserWindow | null => (win && !win.isDestroyed() ? win : null)
export const isPanelVisible = (): boolean => !!panel()?.isVisible()

/** The page calls this once its listeners are attached, so a show never goes unanswered. */
export function panelReady(sender: Electron.WebContents): void {
  if (sender === panel()?.webContents) markReady?.()
}

export function setUnloadAfter(ms: number | null): void {
  unloadAfterMs = ms
  clearTimeout(unloadTimer)
  if (ms !== null && win && !win.isVisible()) unloadTimer = setTimeout(unloadPanel, ms)
}

/** Close the page entirely; the next show rebuilds it. Frees the renderer and its GPU memory. */
function unloadPanel(): void {
  const w = panel()
  if (!w || w.isVisible()) return
  win = null
  ready = null
  w.destroy()
}

export async function showPanel(opts: { query?: string; view?: 'settings' | 'shortcut' } = {}): Promise<void> {
  clearTimeout(unloadTimer)
  const cold = !panel()
  const t0 = performance.now()
  if (cold) createPanel()
  await ready
  if (cold) console.log(`[panel] rebuilt in ${Math.round(performance.now() - t0)}ms`)
  const win = panel()
  if (!win) return
  clearTimeout(hideTimer)
  if (!isWayland()) {
    // X11 lets us pick the monitor under the mouse.
    const d = screen.getDisplayNearestPoint(screen.getCursorScreenPoint())
    win.setBounds(d.bounds)
  } else {
    const d = screen.getPrimaryDisplay()
    const [w, h] = win.getSize()
    if (w !== d.size.width || h !== d.size.height) win.setSize(d.size.width, d.size.height)
  }
  win.webContents.send('lumen:show', opts)
  shownAt = Date.now()
  win.show()
  win.focus()
  win.webContents.focus()
  if (!isWayland()) win.setAlwaysOnTop(true, 'screen-saver')
}

/** Hide the panel. `animated` gives the renderer a moment to play the closing animation. */
export function hidePanel(animated: boolean): void {
  const win = panel()
  if (!win || !win.isVisible()) return
  win.webContents.send('lumen:hide')
  clearTimeout(hideTimer)
  if (animated) hideTimer = setTimeout(() => panel()?.hide(), 120)
  else win.hide()
  clearTimeout(unloadTimer)
  if (unloadAfterMs !== null) unloadTimer = setTimeout(unloadPanel, unloadAfterMs)
}

export function togglePanel(opts: { query?: string; view?: 'settings' | 'shortcut' } = {}): void {
  const win = panel()
  if (win?.isVisible() && win.isFocused()) hidePanel(true)
  else void showPanel(opts)
}
