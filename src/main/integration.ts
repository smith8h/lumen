// Desktop integration: start at login, and give AppImage users a menu entry + icon.
import { app } from 'electron'
import { copyFileSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { homedir } from 'node:os'
import { join } from 'node:path'
import { APP_NAME } from '@shared/constants'
import { launcherPath, run, which } from './env'
import { configHome } from './xdg'

const execQuote = (s: string): string => (/[\s"'\\$`]/.test(s) ? `"${s.replace(/(["\\$`])/g, '\\$1')}"` : s)
const autostartFile = () => join(configHome(), 'autostart', 'lumen.desktop')

export function iconFile(): string {
  return app.isPackaged ? join(process.resourcesPath, 'icon.png') : join(app.getAppPath(), 'resources/icon.png')
}

export function setAutostart(enabled: boolean): void {
  // A dev build can't be started without the project folder, so only packaged builds autostart.
  if (!app.isPackaged) return
  const file = autostartFile()
  if (!enabled) {
    rmSync(file, { force: true })
    return
  }
  mkdirSync(join(configHome(), 'autostart'), { recursive: true })
  writeFileSync(
    file,
    [
      '[Desktop Entry]',
      'Type=Application',
      `Name=${APP_NAME}`,
      'Comment=Spotlight-style launcher',
      `Exec=${execQuote(launcherPath())} --hidden`,
      'Icon=lumen',
      'Terminal=false',
      'NoDisplay=true',
      'X-GNOME-Autostart-enabled=true',
      'X-GNOME-Autostart-Delay=2',
      ''
    ].join('\n')
  )
}

/** AppImages aren't installed, so add Lumen to the app menu ourselves (and keep the path current). */
export function integrateAppImage(): void {
  const appImage = process.env.APPIMAGE
  if (!appImage) return
  const share = process.env.XDG_DATA_HOME || join(homedir(), '.local/share')
  const iconDir = join(share, 'icons/hicolor/512x512/apps')
  const desktopFile = join(share, 'applications/lumen.desktop')
  const content = [
    '[Desktop Entry]',
    'Type=Application',
    `Name=${APP_NAME}`,
    'GenericName=Launcher',
    'Comment=Search apps, files and the web; run commands and calculations',
    `Exec=${execQuote(appImage)} %U`,
    'Icon=lumen',
    'Terminal=false',
    'Categories=Utility;',
    'Keywords=launcher;spotlight;search;run;',
    'StartupWMClass=lumen',
    `X-AppImage-Version=${app.getVersion()}`,
    ''
  ].join('\n')
  try {
    if (existsSync(desktopFile) && readFileSync(desktopFile, 'utf8') === content) return
    mkdirSync(iconDir, { recursive: true })
    mkdirSync(join(share, 'applications'), { recursive: true })
    copyFileSync(iconFile(), join(iconDir, 'lumen.png'))
    writeFileSync(desktopFile, content)
    if (which('update-desktop-database')) run('update-desktop-database', [join(share, 'applications')])
  } catch (err) {
    console.warn('[integration] could not add menu entry', err)
  }
}
