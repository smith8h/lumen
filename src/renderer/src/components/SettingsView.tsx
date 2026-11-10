import { ArrowLeft, Check, Keyboard, Plus, X } from 'lucide-react'
import { useEffect, useState } from 'react'
import type { AppInfo, SearchEngineId, Settings, ShortcutStatus, ThemeMode } from '@shared/types'
import { Keys } from './bits'
import { tilde } from '../lib/paths'

const ACCENTS = ['#7c6cff', '#3b82f6', '#06b6d4', '#10b981', '#f59e0b', '#f43f5e', '#ec4899', '#a3a3a3']
const ENGINES: { id: SearchEngineId; name: string }[] = [
  { id: 'google', name: 'Google' },
  { id: 'duckduckgo', name: 'DuckDuckGo' },
  { id: 'brave', name: 'Brave Search' },
  { id: 'bing', name: 'Bing' },
  { id: 'startpage', name: 'Startpage' },
  { id: 'custom', name: 'Custom…' }
]
const TERMINALS = ['auto', 'ptyxis', 'gnome-terminal', 'kgx', 'konsole', 'kitty', 'alacritty', 'wezterm', 'ghostty', 'foot', 'tilix', 'xfce4-terminal', 'xterm']

function Toggle({ on, onChange, label }: { on: boolean; onChange(v: boolean): void; label: string }) {
  return <button className={`toggle${on ? ' on' : ''}`} role="switch" aria-checked={on} aria-label={label} onClick={() => onChange(!on)} />
}

function Field({ title, hint, children }: { title: string; hint?: string; children: React.ReactNode }) {
  return (
    <div className="field">
      <div className="label">
        <b>{title}</b>
        {hint && <small>{hint}</small>}
      </div>
      {children}
    </div>
  )
}

interface Props {
  settings: Settings
  onChange(patch: Partial<Settings>): void
  onBack(): void
  onShortcut(): void
}

export function SettingsView({ settings: s, onChange, onBack, onShortcut }: Props) {
  const [info, setInfo] = useState<AppInfo | null>(null)
  const [shortcut, setShortcut] = useState<ShortcutStatus | null>(null)
  const [newRoot, setNewRoot] = useState('')

  useEffect(() => {
    window.lumen.appInfo().then(setInfo)
    window.lumen.shortcutStatus().then(setShortcut)
  }, [])

  const addRoot = () => {
    const p = newRoot.trim()
    if (p && !s.fileRoots.includes(p)) onChange({ fileRoots: [...s.fileRoots, p] })
    setNewRoot('')
  }

  return (
    <>
      <div className="view-header">
        <button className="icon-btn" onClick={onBack} aria-label="Back">
          <ArrowLeft size={18} />
        </button>
        <h1>Settings</h1>
        <Keys combo="esc" />
      </div>
      <div className="scroll">
        <div className="settings">
          <h2>Appearance</h2>
          <div className="group">
            <Field title="Theme">
              <div className="segmented">
                {(['system', 'dark', 'light'] as ThemeMode[]).map((t) => (
                  <button key={t} className={s.theme === t ? 'on' : ''} onClick={() => onChange({ theme: t })}>
                    {t[0].toUpperCase() + t.slice(1)}
                  </button>
                ))}
              </div>
            </Field>
            <Field title="Accent colour">
              <div className="swatches">
                {ACCENTS.map((c) => (
                  <button
                    key={c}
                    className={`swatch${s.accent === c ? ' on' : ''}`}
                    style={{ ['--c' as string]: c }}
                    onClick={() => onChange({ accent: c })}
                    aria-label={c}
                  />
                ))}
              </div>
            </Field>
            <Field title="Glass opacity" hint="Lower is more see-through.">
              <input
                type="range"
                min={0.3}
                max={1}
                step={0.01}
                value={s.glassOpacity}
                onChange={(e) => onChange({ glassOpacity: Number(e.target.value) })}
              />
            </Field>
            <Field title="Dim the desktop" hint="Darken everything behind the panel while it's open.">
              <Toggle label="Dim the desktop" on={s.dimBackground} onChange={(v) => onChange({ dimBackground: v })} />
            </Field>
            <Field title="Preview pane" hint="Show details for the selected result.">
              <Toggle label="Preview pane" on={s.showPreview} onChange={(v) => onChange({ showPreview: v })} />
            </Field>
          </div>

          <h2>Search</h2>
          <div className="group">
            <Field title="Web search engine">
              <select className="select" value={s.searchEngine} onChange={(e) => onChange({ searchEngine: e.target.value as SearchEngineId })}>
                {ENGINES.map((e) => (
                  <option key={e.id} value={e.id}>
                    {e.name}
                  </option>
                ))}
              </select>
            </Field>
            {s.searchEngine === 'custom' && (
              <Field title="Custom search URL" hint="Use %s where the search text goes.">
                <input
                  className="text-input"
                  style={{ width: 260 }}
                  defaultValue={s.customSearchUrl}
                  onBlur={(e) => onChange({ customSearchUrl: e.target.value })}
                />
              </Field>
            )}
            <Field title="Currency conversion" hint="“100 usd to iqd” — fetches daily exchange rates.">
              <Toggle label="Currency conversion" on={s.currencyConversion} onChange={(v) => onChange({ currencyConversion: v })} />
            </Field>
            <Field title="Search files" hint={info ? `${info.indexedFiles.toLocaleString()} files and folders indexed · ${info.indexedApps} apps` : undefined}>
              <Toggle label="Search files" on={s.fileSearch} onChange={(v) => onChange({ fileSearch: v })} />
            </Field>
            {s.fileSearch && (
              <>
                <Field title="Include hidden files" hint="Dotfiles and folders like ~/.config.">
                  <Toggle label="Include hidden files" on={s.includeHidden} onChange={(v) => onChange({ includeHidden: v })} />
                </Field>
                <Field title="Folders to search">
                  <input
                    className="text-input"
                    placeholder="/path/to/folder"
                    value={newRoot}
                    onChange={(e) => setNewRoot(e.target.value)}
                    onKeyDown={(e) => {
                      e.stopPropagation()
                      if (e.key === 'Enter') addRoot()
                    }}
                  />
                  <button className="btn small" onClick={addRoot} aria-label="Add folder">
                    <Plus />
                  </button>
                </Field>
                <div className="paths">
                  {s.fileRoots.map((r) => (
                    <span className="path-chip" key={r}>
                      {tilde(r)}
                      <button onClick={() => onChange({ fileRoots: s.fileRoots.filter((x) => x !== r) })} aria-label={`Remove ${r}`}>
                        <X size={12} />
                      </button>
                    </span>
                  ))}
                </div>
              </>
            )}
          </div>

          <h2>General</h2>
          <div className="group">
            <Field
              title="Keyboard shortcut"
              hint={shortcut?.configured ? `Lumen opens with ${shortcut.binding.replace('<Alt>', 'Alt+').replace('space', 'Space')}` : 'Not set up yet'}
            >
              {shortcut?.configured ? (
                <span style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
                  <Check size={16} color="var(--success)" />
                  <button className="btn small" onClick={async () => setShortcut(await window.lumen.removeShortcut())}>
                    Remove
                  </button>
                </span>
              ) : (
                <button className="btn small primary" onClick={onShortcut}>
                  <Keyboard /> Set up
                </button>
              )}
            </Field>
            <Field title="Launch at login" hint="Start quietly in the background so Alt+Space is instant.">
              <Toggle label="Launch at login" on={s.launchAtLogin} onChange={(v) => onChange({ launchAtLogin: v })} />
            </Field>
            <Field title="Terminal" hint="Used for “Run in terminal” and terminal apps.">
              <select className="select" value={s.terminal} onChange={(e) => onChange({ terminal: e.target.value })}>
                {TERMINALS.map((t) => (
                  <option key={t} value={t}>
                    {t === 'auto' ? 'Automatic' : t}
                  </option>
                ))}
              </select>
            </Field>
          </div>

          {info && (
            <p style={{ margin: '18px 2px 0', fontSize: 12, color: 'var(--text-3)', lineHeight: 1.6 }}>
              Lumen {info.version} · {info.desktop} on {info.session}
            </p>
          )}
        </div>
      </div>
    </>
  )
}
