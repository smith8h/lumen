// The panel window. On Wayland apps can't choose where their window goes, so Lumen uses a
// transparent window covering the screen and draws the panel itself at Spotlight's position.
// A click on the transparent area (or focus moving elsewhere) closes it.
import { BrowserWindow, screen } from 'electron'
import { join } from 'node:path'
import { APP_NAME } from '@shared/constants'

let win: BrowserWindow | null = null
let shownAt = 0
let hideTimer: NodeJS.Timeout | undefined
const isWayland = () => process.env.XDG_SESSION_TYPE === 'wayland' && !process.env.LUMEN_X11
const debug = !!process.env.LUMEN_DEBUG

export function createPanel(iconPath?: string): BrowserWindow {
  const display = screen.getPrimaryDisplay()
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
      backgroundThrottling: false
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

export const panel = (): BrowserWindow | null => win
export const isPanelVisible = (): boolean => !!win?.isVisible()

export function showPanel(opts: { query?: string; view?: 'settings' | 'shortcut' } = {}): void {
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
  if (!win || !win.isVisible()) return
  win.webContents.send('lumen:hide')
  clearTimeout(hideTimer)
  if (animated) hideTimer = setTimeout(() => win?.hide(), 120)
  else win.hide()
}

export function togglePanel(opts: { query?: string; view?: 'settings' | 'shortcut' } = {}): void {
  if (!win) return
  if (win.isVisible() && win.isFocused()) hidePanel(true)
  else showPanel(opts)
}
