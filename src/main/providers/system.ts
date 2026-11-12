// Power, session and quick-setting commands, plus Lumen's own commands.
import type { GlyphName, ResultItem, SystemOp } from '@shared/types'
import { APP_NAME } from '@shared/constants'
import { isGnome, run, spawnDetached, which } from '../env'
import { matchFields } from '../fuzzy'
import { frecency } from '../store'

interface SysCommand {
  id: string
  title: string
  subtitle: string
  keywords: string
  glyph: GlyphName
  op: SystemOp | 'lumen-settings' | 'lumen-quit' | 'lumen-reindex' | 'lumen-shortcut' | 'clip-clear'
  confirm?: string
}

const COMMANDS: SysCommand[] = [
  { id: 'lock', title: 'Lock Screen', subtitle: 'System', keywords: 'lock away', glyph: 'lock', op: 'lock' },
  { id: 'suspend', title: 'Sleep', subtitle: 'Suspend the computer', keywords: 'suspend sleep', glyph: 'moon', op: 'suspend' },
  { id: 'logout', title: 'Log Out', subtitle: 'End this session', keywords: 'logout log out sign out', glyph: 'logout', op: 'logout', confirm: 'Press Enter again to log out' },
  { id: 'restart', title: 'Restart', subtitle: 'Reboot the computer', keywords: 'restart reboot', glyph: 'restart', op: 'restart', confirm: 'Press Enter again to restart' },
  { id: 'shutdown', title: 'Shut Down', subtitle: 'Power off the computer', keywords: 'shutdown shut down power off poweroff', glyph: 'power', op: 'shutdown', confirm: 'Press Enter again to shut down' },
  { id: 'dark', title: 'Dark Mode', subtitle: 'Switch the desktop to dark style', keywords: 'dark mode theme night appearance', glyph: 'moon', op: 'dark-mode' },
  { id: 'light', title: 'Light Mode', subtitle: 'Switch the desktop to light style', keywords: 'light mode theme day appearance', glyph: 'sun', op: 'light-mode' },
  { id: 'wifi', title: 'Toggle Wi-Fi', subtitle: 'Turn wireless on or off', keywords: 'wifi wi-fi wireless network internet', glyph: 'wifi', op: 'wifi-toggle' },
  { id: 'dnd', title: 'Do Not Disturb', subtitle: 'Toggle notification banners', keywords: 'dnd do not disturb notifications focus silence', glyph: 'bell', op: 'dnd-toggle' },
  { id: 'trash', title: 'Empty Trash', subtitle: 'Permanently delete files in the Trash', keywords: 'empty trash bin delete', glyph: 'trash', op: 'empty-trash', confirm: 'Press Enter again to permanently delete everything in the Trash' },
  { id: 'settings', title: 'System Settings', subtitle: 'Open desktop settings', keywords: 'settings control center preferences', glyph: 'settings', op: 'settings' },
  { id: 'l-settings', title: `${APP_NAME} Settings`, subtitle: 'Theme, search engine, files, shortcut', keywords: 'lumen preferences config settings options', glyph: 'settings', op: 'lumen-settings' },
  { id: 'l-shortcut', title: `Set Up ${APP_NAME} Shortcut`, subtitle: 'Open Lumen with Alt+Space', keywords: 'lumen shortcut hotkey keybinding alt space', glyph: 'keyboard', op: 'lumen-shortcut' },
  { id: 'l-reindex', title: 'Rebuild File Index', subtitle: `Rescan your files for ${APP_NAME}`, keywords: 'lumen reindex refresh files index rescan', glyph: 'refresh', op: 'lumen-reindex' },
  { id: 'l-clip-clear', title: 'Clear Clipboard History', subtitle: 'Forget copied text (pinned clips stay)', keywords: 'clipboard history clear forget wipe copied clips', glyph: 'clipboard', op: 'clip-clear', confirm: 'Press Enter again to clear clipboard history' },
  { id: 'l-quit', title: `Quit ${APP_NAME}`, subtitle: 'Stop running in the background', keywords: 'lumen quit exit close', glyph: 'quit', op: 'lumen-quit' }
]

function toItem(c: SysCommand, score: number, matches: number[]): ResultItem {
  if (c.op === 'clip-clear') {
    return { id: `sys:${c.id}`, kind: 'lumen', title: c.title, subtitle: c.subtitle, icon: { type: 'glyph', name: c.glyph }, score, matches, confirm: c.confirm, action: { type: 'clip-clear' } }
  }
  const lumenOp = c.op.startsWith('lumen-') ? (c.op.slice(6) as 'settings' | 'quit' | 'reindex' | 'shortcut') : null
  return {
    id: `sys:${c.id}`,
    kind: lumenOp ? 'lumen' : 'system',
    title: c.title,
    subtitle: c.subtitle,
    icon: { type: 'glyph', name: c.glyph },
    score,
    matches,
    confirm: c.confirm,
    historyKey: `sys:${c.id}`,
    action: lumenOp ? { type: 'lumen', op: lumenOp } : { type: 'system', op: c.op as SystemOp }
  }
}

export function systemResults(query: string): ResultItem[] {
  const out: ResultItem[] = []
  for (const c of COMMANDS) {
    const m = matchFields(query, c.title, [c.keywords], 0.85)
    if (!m || m.score < 520) continue
    out.push(toItem(c, m.score * 0.92 + frecency(`sys:${c.id}`), m.indexes))
  }
  return out.sort((a, b) => b.score - a.score).slice(0, 4)
}

const gsettings = (schema: string, key: string, value?: string) =>
  value === undefined ? run('gsettings', ['get', schema, key]) : run('gsettings', ['set', schema, key, value])

export async function runSystem(op: SystemOp): Promise<string | undefined> {
  const gnome = isGnome() && which('gnome-session-quit')
  switch (op) {
    case 'lock':
      if (!run('loginctl', ['lock-session']).ok) await spawnDetached('xdg-screensaver', ['lock'])
      return
    case 'suspend':
      await spawnDetached('systemctl', ['suspend'])
      return
    case 'logout':
      if (gnome) await spawnDetached('gnome-session-quit', ['--logout', '--no-prompt'])
      else await spawnDetached('loginctl', ['terminate-session', process.env.XDG_SESSION_ID ?? 'self'])
      return
    case 'restart':
      await spawnDetached('systemctl', ['reboot'])
      return
    case 'shutdown':
      await spawnDetached('systemctl', ['poweroff'])
      return
    case 'dark-mode':
      gsettings('org.gnome.desktop.interface', 'color-scheme', 'prefer-dark')
      return 'Dark mode on'
    case 'light-mode':
      gsettings('org.gnome.desktop.interface', 'color-scheme', 'default')
      return 'Light mode on'
    case 'wifi-toggle': {
      const state = run('nmcli', ['radio', 'wifi'])
      if (!state.ok) throw new Error('NetworkManager (nmcli) not available')
      const next = state.stdout === 'enabled' ? 'off' : 'on'
      run('nmcli', ['radio', 'wifi', next])
      return `Wi-Fi ${next}`
    }
    case 'dnd-toggle': {
      const cur = gsettings('org.gnome.desktop.notifications', 'show-banners')
      const next = cur.stdout === 'true' ? 'false' : 'true'
      gsettings('org.gnome.desktop.notifications', 'show-banners', next)
      return next === 'false' ? 'Do Not Disturb on' : 'Do Not Disturb off'
    }
    case 'empty-trash':
      if (!run('gio', ['trash', '--empty']).ok) throw new Error('Could not empty the Trash')
      return 'Trash emptied'
    case 'settings':
      for (const bin of ['gnome-control-center', 'systemsettings', 'xfce4-settings-manager', 'cinnamon-settings']) {
        if (which(bin)) {
          await spawnDetached(bin, [])
          return
        }
      }
      throw new Error('No settings app found')
  }
}
