import { spawn, spawnSync } from 'node:child_process'
import { accessSync, constants, readdirSync } from 'node:fs'
import { delimiter, join } from 'node:path'

/**
 * Environment for programs we launch. Strips variables that Electron, Chromium or the
 * AppImage runtime add, so launched apps behave exactly as if started from the desktop.
 */
export function cleanEnv(extra: Record<string, string> = {}): NodeJS.ProcessEnv {
  const env: NodeJS.ProcessEnv = { ...process.env }
  const appDir = env.APPDIR
  for (const key of Object.keys(env)) {
    if (/^(ELECTRON_|CHROME_|GOOGLE_|LUMEN_)/.test(key)) delete env[key]
  }
  for (const key of ['APPDIR', 'APPIMAGE', 'ARGV0', 'OWD', 'NO_AT_BRIDGE', 'GDK_BACKEND', 'ORIGINAL_XDG_CURRENT_DESKTOP']) delete env[key]
  // AppImage runtimes may prepend their own library and data paths.
  if (appDir) {
    for (const key of ['LD_LIBRARY_PATH', 'PATH', 'XDG_DATA_DIRS', 'PYTHONPATH', 'PERL5LIB', 'GSETTINGS_SCHEMA_DIR', 'QT_PLUGIN_PATH']) {
      const value = env[key]
      if (!value) continue
      const kept = value.split(delimiter).filter((p) => p && !p.startsWith(appDir))
      if (kept.length) env[key] = kept.join(delimiter)
      else delete env[key]
    }
  }
  if (process.env.ORIGINAL_XDG_CURRENT_DESKTOP) env.XDG_CURRENT_DESKTOP = process.env.ORIGINAL_XDG_CURRENT_DESKTOP
  return { ...env, ...extra }
}

/** Start a program fully detached from Lumen, so closing Lumen never closes it. */
export function spawnDetached(cmd: string, args: string[], opts: { cwd?: string; env?: NodeJS.ProcessEnv } = {}): Promise<void> {
  return new Promise((resolve, reject) => {
    try {
      const child = spawn(cmd, args, { cwd: opts.cwd, env: opts.env ?? cleanEnv(), detached: true, stdio: 'ignore' })
      child.once('error', reject)
      child.once('spawn', () => {
        child.unref()
        resolve()
      })
    } catch (err) {
      reject(err)
    }
  })
}

const whichCache = new Map<string, string | null>()

export function which(bin: string): string | null {
  if (whichCache.has(bin)) return whichCache.get(bin)!
  let found: string | null = null
  if (bin.includes('/')) {
    try {
      accessSync(bin, constants.X_OK)
      found = bin
    } catch {
      found = null
    }
  } else {
    for (const dir of (cleanEnv().PATH ?? '').split(delimiter)) {
      if (!dir) continue
      try {
        accessSync(join(dir, bin), constants.X_OK)
        found = join(dir, bin)
        break
      } catch {
        /* keep looking */
      }
    }
  }
  whichCache.set(bin, found)
  return found
}

let executables: Set<string> | null = null

/** Every program name on PATH — used to suggest "Run in terminal" for bare commands. */
export function pathExecutables(): Set<string> {
  if (executables) return executables
  executables = new Set()
  for (const dir of (cleanEnv().PATH ?? '').split(delimiter)) {
    try {
      for (const name of readdirSync(dir)) executables.add(name)
    } catch {
      /* missing PATH entry */
    }
  }
  return executables
}

export function run(cmd: string, args: string[], timeout = 3000): { ok: boolean; stdout: string; stderr: string } {
  const r = spawnSync(cmd, args, { env: cleanEnv(), encoding: 'utf8', timeout })
  return { ok: r.status === 0, stdout: (r.stdout ?? '').trim(), stderr: (r.stderr ?? '').trim() }
}

export const desktopName = (): string =>
  (process.env.ORIGINAL_XDG_CURRENT_DESKTOP || process.env.XDG_CURRENT_DESKTOP || 'unknown').toString()

export const isGnome = (): boolean => /gnome|unity|ubuntu/i.test(desktopName())
export const isKde = (): boolean => /kde|plasma/i.test(desktopName())

/** Path users and keybindings should run to start Lumen (AppImage, .deb wrapper or dev binary). */
export function launcherPath(): string {
  return process.env.APPIMAGE || process.env.LUMEN_LAUNCHER || process.execPath
}
