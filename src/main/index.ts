import { app, ipcMain, net, protocol, type IpcMainInvokeEvent } from 'electron'
import { writeFileSync } from 'node:fs'
import { homedir } from 'node:os'
import { pathToFileURL } from 'node:url'
import type { AppInfo, PanelRect, ResultItem, Settings } from '@shared/types'
import { APP_NAME } from '@shared/constants'
import { execute, killCommand, preview } from './actions'
import { applyClipboardSettings, clipCount, flushClipboard, initClipboard, markSecretCopy } from './clipboard'
import { emitPanelGeometry, startDbus } from './dbus'
import { desktopName, isGnome } from './env'
import { fileCount, initFiles, reindex } from './files'
import { canServe } from './icons'
import { iconFile, integrateAppImage, setAutostart } from './integration'
import { setPreviousWindow } from './paste'
import { appCount, initApps } from './providers/apps'
import { warmCalculator } from './providers/calculator'
import { search, suggestions } from './search'
import { refreshShortcutCommand, registerFallbackShortcut, removeShortcut, setupShortcut, shortcutStatus } from './shortcut'
import { DEFAULT_SETTINGS, getSettings, loadHistory, loadSettings, onSettingsChange, updateSettings } from './store'
import { createPanel, hidePanel, panel, panelReady, setQuitting, setUnloadAfter, showPanel, togglePanel } from './window'

// ── Before ready ───────────────────────────────────────────────────────────
app.setName(APP_NAME)
// Separate profile, e.g. for screenshots or testing: LUMEN_USER_DATA=/tmp/lumen-test
if (process.env.LUMEN_USER_DATA) app.setPath('userData', process.env.LUMEN_USER_DATA)
app.commandLine.appendSwitch('ozone-platform-hint', 'auto')
if (process.env.LUMEN_X11) app.commandLine.appendSwitch('ozone-platform', 'x11')
// Run Chromium's network service inside the main process: one process fewer (~20 MB).
app.commandLine.appendSwitch('enable-features', ['NetworkServiceInProcess2', ...(isGnome() ? [] : ['GlobalShortcutsPortal'])].join(','))
// Wayland app id / WM class, so docks match the window to lumen.desktop.
;(app as unknown as { setDesktopName?: (n: string) => void }).setDesktopName?.('lumen.desktop')

protocol.registerSchemesAsPrivileged([{ scheme: 'lumen-icon', privileges: { standard: true, secure: true, supportFetchAPI: true } }])

const startHidden = process.argv.includes('--hidden')
/** Hidden this long, the panel's page is unloaded; the next Alt+Space rebuilds it. */
const UNLOAD_AFTER_MS = Number(process.env.LUMEN_UNLOAD_MS) || 60_000

/** Command-line maintenance flags. Returns true when the flag means "don't show the panel". */
function handleCliFlags(argv: string[]): boolean {
  if (argv.includes('--setup-shortcut')) {
    const s = setupShortcut()
    updateSettings({ firstRunDone: true })
    console.log(s.configured ? `Shortcut ready: ${s.binding}` : `Couldn't set up the shortcut on ${s.desktop}. Bind this command yourself:\n${s.command}`)
    return true
  }
  if (argv.includes('--remove-shortcut')) {
    removeShortcut()
    console.log('Shortcut removed')
    return true
  }
  if (argv.includes('--quit')) {
    app.quit()
    return true
  }
  return false
}

if (!app.requestSingleInstanceLock()) {
  // Already running: the first instance gets our argv through 'second-instance'.
  app.exit(0)
}

app.on('second-instance', (_e, argv) => {
  if (handleCliFlags(argv) || argv.includes('--hidden')) return
  if (argv.includes('--toggle')) togglePanel()
  else showPanel()
})

app.on('window-all-closed', () => {
  /* stay resident */
})
app.on('before-quit', () => {
  setQuitting()
  flushClipboard()
})

// ── IPC ────────────────────────────────────────────────────────────────────
/** True while the GNOME extension draws real blur behind the panel. */
let shellBlur = false

function toPanelRect(v: unknown): PanelRect | null {
  if (!v || typeof v !== 'object') return null
  const r = v as Record<string, unknown>
  const num = (k: string) => (typeof r[k] === 'number' && Number.isFinite(r[k]) ? Math.round(Math.min(1e5, Math.max(-1e5, r[k] as number))) : NaN)
  const rect = { x: num('x'), y: num('y'), width: num('width'), height: num('height'), radius: num('radius') }
  if (Object.values(rect).some(Number.isNaN) || rect.width <= 0 || rect.height <= 0) return null
  return rect
}

function fromPanel(e: IpcMainInvokeEvent): boolean {
  return e.sender === panel()?.webContents
}

const SETTING_KEYS = Object.keys(DEFAULT_SETTINGS) as (keyof Settings)[]

function sanitize(patch: Partial<Settings>): Partial<Settings> {
  const out: Partial<Settings> = {}
  for (const k of SETTING_KEYS) {
    if (!(k in patch)) continue
    const v = patch[k]
    if (typeof v !== typeof DEFAULT_SETTINGS[k]) continue
    if (Array.isArray(DEFAULT_SETTINGS[k]) && !(Array.isArray(v) && v.every((x) => typeof x === 'string'))) continue
    ;(out as Record<string, unknown>)[k] = v
  }
  if (out.accent && !/^#[0-9a-f]{6}$/i.test(out.accent)) delete out.accent
  if (out.clipboardMax !== undefined) {
    if (Number.isFinite(out.clipboardMax)) out.clipboardMax = Math.round(Math.min(5000, Math.max(10, out.clipboardMax)))
    else delete out.clipboardMax
  }
  return out
}

function registerIpc(): void {
  const handle = <A extends unknown[], R>(channel: string, fn: (e: IpcMainInvokeEvent, ...args: A) => R) =>
    ipcMain.handle(channel, (e, ...args) => {
      if (!fromPanel(e)) throw new Error('Unauthorized sender')
      return fn(e, ...(args as A))
    })

  handle('search', (_e, query: string, seq: number) => search(String(query).slice(0, 500), Number(seq)))
  handle('suggestions', () => suggestions())
  handle('execute', (e, item: Pick<ResultItem, 'action' | 'historyKey'>) => execute(item, e.sender))
  handle('preview', (_e, item: Pick<ResultItem, 'id' | 'kind' | 'action' | 'title' | 'subtitle'>) => preview(item))
  handle('kill', (_e, runId: string) => killCommand(runId))
  handle('settings:get', () => getSettings())
  handle('settings:set', (_e, patch: Partial<Settings>) => updateSettings(sanitize(patch)))
  handle('shortcut:status', () => shortcutStatus())
  handle('shortcut:setup', () => setupShortcut())
  handle('shortcut:remove', () => removeShortcut())
  handle(
    'app:info',
    (): AppInfo => ({
      version: app.getVersion(),
      platform: process.platform,
      session: process.env.XDG_SESSION_TYPE ?? 'unknown',
      desktop: desktopName(),
      indexedFiles: fileCount(),
      indexedApps: appCount(),
      clipboardItems: clipCount(),
      home: homedir()
    })
  )
  handle('shell-blur:get', () => shellBlur)
  ipcMain.on('panel:geometry', (e, rect: unknown) => {
    if (e.sender === panel()?.webContents) emitPanelGeometry(toPanelRect(rect))
  })
  ipcMain.on('renderer:ready', (e) => panelReady(e.sender))
  ipcMain.on('hide', (e) => {
    if (e.sender === panel()?.webContents) hidePanel(true)
  })
}

// ── Startup ────────────────────────────────────────────────────────────────
app.whenReady().then(async () => {
  loadSettings()
  loadHistory()
  initClipboard()

  protocol.handle('lumen-icon', (req) => {
    const path = decodeURIComponent(new URL(req.url).pathname)
    if (!canServe(path)) return new Response('Not found', { status: 404 })
    return net.fetch(pathToFileURL(path).toString())
  })

  initApps()
  initFiles()
  warmCalculator()
  registerIpc()

  let lastFileSettings = JSON.stringify([getSettings().fileRoots, getSettings().excludeDirs, getSettings().includeHidden, getSettings().fileSearch])
  let lastAutostart = getSettings().launchAtLogin
  const applyUnload = () => setUnloadAfter(getSettings().freeMemoryWhenIdle ? UNLOAD_AFTER_MS : null)
  onSettingsChange((s) => {
    applyUnload()
    applyClipboardSettings(s)
    panel()?.webContents.send('lumen:settings', s)
    const fileSettings = JSON.stringify([s.fileRoots, s.excludeDirs, s.includeHidden, s.fileSearch])
    if (fileSettings !== lastFileSettings) {
      lastFileSettings = fileSettings
      reindex()
    }
    if (s.launchAtLogin !== lastAutostart) {
      lastAutostart = s.launchAtLogin
      setAutostart(s.launchAtLogin)
    }
  })

  createPanel(iconFile())
  applyUnload()
  await startDbus({
    toggle: () => togglePanel(),
    show: (query) => showPanel({ query }),
    hide: () => hidePanel(true),
    previousWindow: (wmClass, x11) => setPreviousWindow(wmClass, x11),
    clipboardSecret: () => markSecretCopy(),
    shellBlurChanged: (enabled) => {
      shellBlur = enabled
      panel()?.webContents.send('lumen:shell-blur', enabled)
    },
    capture: app.isPackaged
      ? undefined
      : async (path) => {
          const img = await panel()!.webContents.capturePage()
          writeFileSync(path, img.toPNG())
        },
    key: app.isPackaged
      ? undefined
      : (combo) => {
          const parts = combo.split('+')
          const keyCode = parts.pop()!
          const modifiers = parts.map((m) => m.toLowerCase()) as ('ctrl' | 'shift' | 'alt')[]
          const wc = panel()!.webContents
          wc.sendInputEvent({ type: 'keyDown', keyCode, modifiers })
          if (keyCode.length === 1) wc.sendInputEvent({ type: 'char', keyCode, modifiers })
          wc.sendInputEvent({ type: 'keyUp', keyCode, modifiers })
        }
  })
  registerFallbackShortcut(() => togglePanel())
  refreshShortcutCommand()
  integrateAppImage()
  setAutostart(getSettings().launchAtLogin)

  if (handleCliFlags(process.argv)) return
  const first = !getSettings().firstRunDone
  if (first || !startHidden) {
    // Wait for the renderer so the first frame is already styled.
    const show = () => showPanel(first ? { view: 'shortcut' } : {})
    const wc = panel()?.webContents
    if (wc?.isLoading()) wc.once('did-finish-load', show)
    else show()
  }
})
