// Registers Alt+Space with the desktop. Wayland doesn't let apps grab global keys, so on
// GNOME we add a custom keyboard shortcut (the same thing Settings → Keyboard does) that
// calls Lumen over D-Bus. GNOME's own Alt+Space (window menu) is moved out of the way and
// restored if the shortcut is removed.
import { app, globalShortcut } from 'electron'
import { readFileSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'
import type { ShortcutStatus } from '@shared/types'
import { APP_NAME, DEFAULT_SHORTCUT } from '@shared/constants'
import { desktopName, isGnome, launcherPath, run, which } from './env'
import { toggleCommand } from './dbus'

const MEDIA_KEYS = 'org.gnome.settings-daemon.plugins.media-keys'
const KEY_PATH = '/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/lumen/'
const RELOCATABLE = `${MEDIA_KEYS}.custom-keybinding:${KEY_PATH}`
const CONFLICT_SCHEMAS = ['org.gnome.desktop.wm.keybindings', 'org.gnome.shell.keybindings', 'org.gnome.mutter.keybindings', MEDIA_KEYS]

const gvString = (s: string): string => `'${s.replace(/\\/g, '\\\\').replace(/'/g, "\\'")}'`
const parseStrv = (v: string): string[] => [...v.matchAll(/'((?:[^'\\]|\\.)*)'/g)].map((m) => m[1].replace(/\\(.)/g, '$1'))
const strv = (list: string[]): string => (list.length ? `[${list.map(gvString).join(', ')}]` : '@as []')
const same = (a: string, b: string) => a.toLowerCase().replace(/\s/g, '') === b.toLowerCase().replace(/\s/g, '')

/** The program (plus args) the shortcut should run. In development that's electron + app path. */
function launcherArgs(): string {
  return app.isPackaged ? launcherPath() : [process.execPath, app.getAppPath()].join('\u0000')
}

export function shortcutCommand(): string {
  return `sh -c ${gvShellQuote(toggleCommand(launcherArgs()))}`
}

// GNOME parses the command with g_shell_parse_argv, so wrap it in double quotes.
const gvShellQuote = (s: string): string => `"${s.replace(/(["\\$`])/g, '\\$1')}"`

const backupFile = () => join(app.getPath('userData'), 'shortcut-backup.json')

function findConflicts(binding: string): { schema: string; key: string; values: string[] }[] {
  const out: { schema: string; key: string; values: string[] }[] = []
  for (const schema of CONFLICT_SCHEMAS) {
    const r = run('gsettings', ['list-recursively', schema])
    if (!r.ok) continue
    for (const line of r.stdout.split('\n')) {
      const [, key, value] = /^\S+\s+(\S+)\s+(.*)$/.exec(line) ?? []
      if (!key || !value?.includes('<')) continue
      const values = value.startsWith('[') || value.startsWith('@as') ? parseStrv(value) : [value.replace(/^'|'$/g, '')]
      if (values.some((v) => same(v, binding))) out.push({ schema, key, values })
    }
  }
  return out
}

export function shortcutStatus(): ShortcutStatus {
  const desktop = desktopName()
  const base = { desktop, binding: DEFAULT_SHORTCUT, command: shortcutCommand() }
  if (!isGnome() || !which('gsettings')) {
    return { ...base, supported: false, configured: globalShortcut.isRegistered('Alt+Space'), conflicts: [] }
  }
  const list = parseStrv(run('gsettings', ['get', MEDIA_KEYS, 'custom-keybindings']).stdout)
  const binding = run('gsettings', ['get', RELOCATABLE, 'binding']).stdout.replace(/^'|'$/g, '')
  const configured = list.includes(KEY_PATH) && !!binding
  const conflicts = findConflicts(binding || DEFAULT_SHORTCUT)
    .filter((c) => !(c.schema === MEDIA_KEYS && c.key === 'custom-keybindings'))
    .map((c) => `${c.schema.split('.').pop()} › ${c.key}`)
  return { ...base, supported: true, configured, binding: binding || DEFAULT_SHORTCUT, conflicts }
}

export function setupShortcut(binding = DEFAULT_SHORTCUT): ShortcutStatus {
  if (!isGnome() || !which('gsettings')) {
    // X11 desktops (and Wayland ones with the GlobalShortcuts portal) via Electron.
    try {
      globalShortcut.register('Alt+Space', () => onGlobalShortcut?.())
    } catch {
      /* not supported here */
    }
    return shortcutStatus()
  }
  // Free the key from GNOME's own bindings, remembering what was there.
  let backup: { schema: string; key: string; values: string[] }[] = []
  try {
    backup = JSON.parse(readFileSync(backupFile(), 'utf8'))
  } catch {
    /* first setup */
  }
  for (const c of findConflicts(binding)) {
    if (c.schema === MEDIA_KEYS) continue
    if (!backup.some((b) => b.schema === c.schema && b.key === c.key)) backup.push(c)
    run('gsettings', ['set', c.schema, c.key, strv(c.values.filter((v) => !same(v, binding)))])
  }
  writeFileSync(backupFile(), JSON.stringify(backup, null, 2))

  const list = parseStrv(run('gsettings', ['get', MEDIA_KEYS, 'custom-keybindings']).stdout)
  if (!list.includes(KEY_PATH)) run('gsettings', ['set', MEDIA_KEYS, 'custom-keybindings', strv([...list, KEY_PATH])])
  run('gsettings', ['set', RELOCATABLE, 'name', gvString(APP_NAME)])
  run('gsettings', ['set', RELOCATABLE, 'command', gvString(shortcutCommand())])
  run('gsettings', ['set', RELOCATABLE, 'binding', gvString(binding)])
  return shortcutStatus()
}

export function removeShortcut(): ShortcutStatus {
  if (!isGnome()) {
    globalShortcut.unregister('Alt+Space')
    return shortcutStatus()
  }
  const list = parseStrv(run('gsettings', ['get', MEDIA_KEYS, 'custom-keybindings']).stdout)
  run('gsettings', ['set', MEDIA_KEYS, 'custom-keybindings', strv(list.filter((p) => p !== KEY_PATH))])
  run('gsettings', ['reset-recursively', RELOCATABLE])
  try {
    const backup = JSON.parse(readFileSync(backupFile(), 'utf8')) as { schema: string; key: string; values: string[] }[]
    for (const b of backup) run('gsettings', ['set', b.schema, b.key, strv(b.values)])
    writeFileSync(backupFile(), '[]')
  } catch {
    /* nothing to restore */
  }
  return shortcutStatus()
}

/** If the AppImage moved or Lumen was updated, point the existing shortcut at the new path. */
export function refreshShortcutCommand(): void {
  // Never let a development run repoint the user's real shortcut at itself.
  if (!app.isPackaged || !isGnome() || !which('gsettings')) return
  const s = shortcutStatus()
  if (!s.configured) return
  const current = run('gsettings', ['get', RELOCATABLE, 'command']).stdout
  if (current !== gvString(shortcutCommand())) run('gsettings', ['set', RELOCATABLE, 'command', gvString(shortcutCommand())])
}

let onGlobalShortcut: (() => void) | undefined

/** Non-GNOME desktops: try a regular global shortcut. */
export function registerFallbackShortcut(handler: () => void): void {
  onGlobalShortcut = handler
  if (isGnome()) return
  try {
    globalShortcut.register('Alt+Space', handler)
  } catch {
    /* compositor doesn't allow it; the user can bind `lumen --toggle` manually */
  }
}
