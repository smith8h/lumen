import {
  BellOff, Calculator, CircleX, Clipboard, ClipboardPaste, Copy, File, Folder, Globe, Keyboard, Lock, LogOut, Moon, Power,
  Pin, RefreshCw, RotateCcw, Search, Settings, Sparkles, SquareTerminal, Sun, Trash2, Wifi, type LucideIcon
} from 'lucide-react'
import { useState } from 'react'
import type { GlyphName, IconSpec } from '@shared/types'

export const GLYPHS: Record<GlyphName, LucideIcon> = {
  calculator: Calculator,
  globe: Globe,
  search: Search,
  terminal: SquareTerminal,
  lock: Lock,
  logout: LogOut,
  moon: Moon,
  sun: Sun,
  power: Power,
  restart: RotateCcw,
  settings: Settings,
  folder: Folder,
  file: File,
  trash: Trash2,
  wifi: Wifi,
  bell: BellOff,
  refresh: RefreshCw,
  quit: CircleX,
  sparkles: Sparkles,
  copy: Copy,
  keyboard: Keyboard,
  clipboard: Clipboard,
  pin: Pin,
  paste: ClipboardPaste
}

/** macOS-style colourful tiles for built-in result types. */
const TINTS: Partial<Record<GlyphName, [string, string]>> = {
  calculator: ['#ffb547', '#ff7a1a'],
  globe: ['#5ac8fa', '#2a7fff'],
  search: ['#6ea8ff', '#3b5bff'],
  terminal: ['#4b5563', '#111827'],
  lock: ['#8b9bb4', '#4a5568'],
  logout: ['#f78ca0', '#e0457b'],
  moon: ['#8f7bff', '#4b3fd6'],
  sun: ['#ffd76a', '#ff9f1c'],
  power: ['#ff7a7a', '#e0313f'],
  restart: ['#7ee0b5', '#16a37a'],
  settings: ['#a0a7b8', '#5c6478'],
  folder: ['#7cc4ff', '#3a8dde'],
  file: ['#c3cad8', '#8a93a6'],
  trash: ['#9aa5b8', '#5a6477'],
  wifi: ['#59d8ff', '#1e9bd7'],
  bell: ['#c28dff', '#8b48e8'],
  refresh: ['#71e1c6', '#1aa889'],
  quit: ['#ff9a9a', '#d94848'],
  sparkles: ['#b69cff', '#7c6cff'],
  copy: ['#a0a7b8', '#5c6478'],
  keyboard: ['#b69cff', '#7c6cff'],
  clipboard: ['#ffd27a', '#e8912d'],
  pin: ['#ff9eb5', '#e0457b'],
  paste: ['#ffd27a', '#e8912d']
}

export function GlyphTile({ name }: { name: GlyphName }) {
  const G = GLYPHS[name] ?? Sparkles
  const [t1, t2] = TINTS[name] ?? ['#b69cff', '#7c6cff']
  return (
    <div className="tile" style={{ ['--t1' as string]: t1, ['--t2' as string]: t2 }}>
      <G strokeWidth={2.1} />
    </div>
  )
}

export const iconUrl = (path: string): string => `lumen-icon://icon${encodeURI(path).replace(/[?#]/g, encodeURIComponent)}`

export function Icon({ spec, fallback = 'sparkles', thumb = false }: { spec?: IconSpec; fallback?: GlyphName; thumb?: boolean }) {
  const [broken, setBroken] = useState(false)
  let content
  if (spec?.type === 'file' && !broken) {
    content = <img src={iconUrl(spec.path)} className={thumb ? 'thumb' : undefined} onError={() => setBroken(true)} draggable={false} alt="" />
  } else if (spec?.type === 'data' && !broken) {
    content = <img src={spec.url} onError={() => setBroken(true)} draggable={false} alt="" />
  } else {
    content = <GlyphTile name={spec?.type === 'glyph' ? spec.name : fallback} />
  }
  return <div className="icon">{content}</div>
}
