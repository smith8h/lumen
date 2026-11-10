import { homedir } from 'node:os'
import { join } from 'node:path'

/** XDG data directories, most specific first, including Flatpak and Snap exports. */
export function dataDirs(): string[] {
  const home = homedir()
  const dataHome = process.env.XDG_DATA_HOME || join(home, '.local/share')
  const sys = (process.env.XDG_DATA_DIRS || '/usr/local/share:/usr/share')
    .split(':')
    .filter((d) => d && !(process.env.APPDIR && d.startsWith(process.env.APPDIR)))
  const extra = [
    join(dataHome, 'flatpak/exports/share'),
    '/var/lib/flatpak/exports/share',
    '/var/lib/snapd/desktop',
    '/usr/local/share',
    '/usr/share'
  ]
  return [...new Set([dataHome, ...sys, ...extra])]
}

export const configHome = (): string => process.env.XDG_CONFIG_HOME || join(homedir(), '.config')

/** Locale names to try for localized keys, e.g. Name[ar_IQ] → Name[ar] → Name. */
export function localeKeys(): string[] {
  const raw = (process.env.LC_MESSAGES || process.env.LC_ALL || process.env.LANG || '').split('.')[0]
  if (!raw || raw === 'C' || raw === 'POSIX') return []
  const lang = raw.split('_')[0]
  return raw === lang ? [lang] : [raw, lang]
}
