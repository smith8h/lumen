// Resolves freedesktop icon names ("org.gnome.Nautilus", "firefox") to image files,
// following the user's icon theme and its Inherits= chain, then hicolor and pixmaps.
import { existsSync, readdirSync, readFileSync } from 'node:fs'
import { homedir } from 'node:os'
import { extname, join } from 'node:path'
import { run } from './env'
import { dataDirs } from './xdg'

interface Candidate {
  path: string
  rank: number
}

const EXT = new Set(['.svg', '.png', '.xpm'])
const themeIndexes = new Map<string, Map<string, Candidate>>()
let themeChain: string[] | null = null

function iconBaseDirs(): string[] {
  return [join(homedir(), '.icons'), ...dataDirs().map((d) => join(d, 'icons'))]
}

function readInherits(theme: string): string[] {
  for (const base of iconBaseDirs()) {
    const index = join(base, theme, 'index.theme')
    if (!existsSync(index)) continue
    const line = readFileSync(index, 'utf8').split('\n').find((l) => l.startsWith('Inherits='))
    return line ? line.slice(9).split(',').map((s) => s.trim()).filter(Boolean) : []
  }
  return []
}

function currentTheme(): string {
  const r = run('gsettings', ['get', 'org.gnome.desktop.interface', 'icon-theme'])
  if (r.ok && r.stdout) return r.stdout.replace(/^'|'$/g, '')
  return 'hicolor'
}

function chain(): string[] {
  if (themeChain) return themeChain
  const out: string[] = []
  const visit = (t: string): void => {
    if (out.includes(t) || out.length > 12) return
    out.push(t)
    for (const parent of readInherits(t)) visit(parent)
  }
  visit(currentTheme())
  for (const fallback of ['Adwaita', 'hicolor']) if (!out.includes(fallback)) out.push(fallback)
  themeChain = out
  return out
}

/** Bigger raster sizes and scalable SVGs rank higher; "apps" contexts beat others. */
function rankFor(dirPath: string, ext: string): number {
  let rank = 0
  if (ext === '.svg') rank += 1000
  const size = /(\d+)x\d+/.exec(dirPath)
  if (size) rank += Math.min(Number(size[1]), 512)
  if (/scalable/.test(dirPath)) rank += 600
  if (/@2x/.test(dirPath)) rank += 50
  if (/symbolic/.test(dirPath)) rank -= 2000
  if (/\/apps\b/.test(dirPath)) rank += 40
  if (ext === '.xpm') rank -= 500
  return rank
}

function indexTheme(theme: string): Map<string, Candidate> {
  const cached = themeIndexes.get(theme)
  if (cached) return cached
  const map = new Map<string, Candidate>()
  const walk = (dir: string, depth: number): void => {
    if (depth > 4) return
    let entries
    try {
      entries = readdirSync(dir, { withFileTypes: true })
    } catch {
      return
    }
    for (const e of entries) {
      const full = join(dir, e.name)
      if (e.isDirectory() || (e.isSymbolicLink() && !extname(e.name))) {
        walk(full, depth + 1)
        continue
      }
      const ext = extname(e.name)
      if (!EXT.has(ext)) continue
      const name = e.name.slice(0, -ext.length)
      const rank = rankFor(dir, ext)
      const prev = map.get(name)
      if (!prev || rank > prev.rank) map.set(name, { path: full, rank })
    }
  }
  for (const base of iconBaseDirs()) walk(join(base, theme), 0)
  themeIndexes.set(theme, map)
  return map
}

const resolved = new Map<string, string | null>()

export function resolveIcon(name: string | undefined): string | null {
  if (!name) return null
  if (resolved.has(name)) return resolved.get(name)!
  let found: string | null = null
  if (name.startsWith('/')) {
    found = existsSync(name) ? name : null
  } else {
    const bare = EXT.has(extname(name)) ? name.slice(0, -extname(name).length) : name
    for (const theme of chain()) {
      const hit = indexTheme(theme).get(bare)
      if (hit) {
        found = hit.path
        break
      }
    }
    if (!found) {
      for (const d of dataDirs()) {
        for (const ext of ['.svg', '.png', '.xpm']) {
          const p = join(d, 'pixmaps', bare + ext)
          if (existsSync(p)) {
            found = p
            break
          }
        }
        if (found) break
      }
    }
  }
  resolved.set(name, found)
  return found
}

/** Files we're willing to serve over lumen-icon:// — only things we resolved ourselves. */
const servable = new Set<string>()

export function allowServe(path: string): string {
  servable.add(path)
  return path
}

export const canServe = (path: string): boolean => servable.has(path)

export function resetIconCache(): void {
  themeChain = null
  themeIndexes.clear()
  resolved.clear()
}
