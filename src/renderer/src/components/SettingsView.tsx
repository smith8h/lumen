import {
  ArrowLeft, Check, CircleCheck, ExternalLink, Folder, FolderX, Info, Keyboard, Monitor, Moon, Palette, Plus,
  RefreshCw, Search, Settings2, Sun, X, type LucideIcon
} from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
import type { AppInfo, SearchEngineId, Settings, ShortcutStatus, ThemeMode } from '@shared/types'
import { Keys, LumenMark } from './bits'
import { tilde } from '../lib/paths'

/** Curated accents, tuned to read well on both glass themes. The first is Lumen's default. */
const ACCENTS: { name: string; value: string }[] = [
  { name: 'Iris', value: '#7c6cff' },
  { name: 'Blue', value: '#3b82f6' },
  { name: 'Teal', value: '#14b8a6' },
  { name: 'Green', value: '#22c55e' },
  { name: 'Amber', value: '#f59e0b' },
  { name: 'Rose', value: '#f43f5e' }
]
const THEMES: { id: ThemeMode; name: string; icon: LucideIcon }[] = [
  { id: 'system', name: 'Auto', icon: Monitor },
  { id: 'light', name: 'Light', icon: Sun },
  { id: 'dark', name: 'Dark', icon: Moon }
]
const ENGINES: { id: SearchEngineId; name: string }[] = [
  { id: 'google', name: 'Google' },
  { id: 'duckduckgo', name: 'DuckDuckGo' },
  { id: 'brave', name: 'Brave Search' },
  { id: 'bing', name: 'Bing' },
  { id: 'startpage', name: 'Startpage' },
  { id: 'custom', name: 'Custom…' }
]
const TERMINALS = ['auto', 'ptyxis', 'gnome-terminal', 'kgx', 'konsole', 'kitty', 'alacritty', 'wezterm', 'ghostty', 'foot', 'tilix', 'xfce4-terminal', 'xterm']
const REPO_URL = 'https://github.com/smith8h/lumen'

const openUrl = (url: string) => window.lumen.execute({ action: { type: 'open-url', url } }).then((r) => r.ok && window.lumen.hide())

type PageId = 'general' | 'appearance' | 'search' | 'files' | 'about'
const PAGES: { id: PageId; name: string; icon: LucideIcon; lede: string }[] = [
  { id: 'general', name: 'General', icon: Settings2, lede: 'How Lumen starts and opens.' },
  { id: 'appearance', name: 'Appearance', icon: Palette, lede: 'Theme, colour and layout of the panel.' },
  { id: 'search', name: 'Search', icon: Search, lede: 'The web, calculator and conversions.' },
  { id: 'files', name: 'Files', icon: Folder, lede: 'Which files and folders Lumen can find.' },
  { id: 'about', name: 'About', icon: Info, lede: '' }
]

// ── Building blocks ───────────────────────────────────────────────────────

function Toggle({ on, onChange, label }: { on: boolean; onChange(v: boolean): void; label: string }) {
  return <button className={`toggle${on ? ' on' : ''}`} role="switch" aria-checked={on} aria-label={label} onClick={() => onChange(!on)} />
}

function Field({ title, hint, disabled, children }: { title: string; hint?: React.ReactNode; disabled?: boolean; children?: React.ReactNode }) {
  return (
    <div className={`field${disabled ? ' disabled' : ''}`}>
      <div className="label">
        <b>{title}</b>
        {hint && <small>{hint}</small>}
      </div>
      {children && <div className="control">{children}</div>}
    </div>
  )
}

function Switch({ title, hint, on, onChange, disabled }: { title: string; hint?: string; on: boolean; onChange(v: boolean): void; disabled?: boolean }) {
  return (
    <Field title={title} hint={hint} disabled={disabled}>
      <Toggle label={title} on={on} onChange={onChange} />
    </Field>
  )
}

/** An editable list of folder paths or names. */
function PathList({ items, onChange, placeholder, mono = true, icon: RowIcon }: { items: string[]; onChange(v: string[]): void; placeholder: string; mono?: boolean; icon: LucideIcon }) {
  const [draft, setDraft] = useState('')
  const add = () => {
    const v = draft.trim()
    if (v && !items.includes(v)) onChange([...items, v])
    setDraft('')
  }
  return (
    <div className="path-list">
      {items.length === 0 && <div className="none">Nothing here yet.</div>}
      {items.map((p) => (
        <div className="path-row" key={p} style={mono ? undefined : { fontFamily: 'inherit' }}>
          <RowIcon />
          <span title={p}>{tilde(p)}</span>
          <button onClick={() => onChange(items.filter((x) => x !== p))} aria-label={`Remove ${p}`}>
            <X size={14} />
          </button>
        </div>
      ))}
      <div className="path-add">
        <input
          className="text-input"
          placeholder={placeholder}
          value={draft}
          spellCheck={false}
          onChange={(e) => setDraft(e.target.value)}
          onKeyDown={(e) => {
            e.stopPropagation()
            if (e.key === 'Enter') add()
            if (e.key === 'Escape') (e.target as HTMLInputElement).blur()
          }}
        />
        <button className="btn small" onClick={add} disabled={!draft.trim()}>
          <Plus /> Add
        </button>
      </div>
    </div>
  )
}

// ── View ──────────────────────────────────────────────────────────────────

interface Props {
  settings: Settings
  onChange(patch: Partial<Settings>): void
  onBack(): void
  onShortcut(): void
  onToast(text: string): void
  /** The GNOME extension is blurring the background for real. */
  shellBlur?: boolean
}

export function SettingsView({ settings: s, onChange, onBack, onShortcut, onToast, shellBlur }: Props) {
  const [page, setPage] = useState<PageId>('general')
  const [info, setInfo] = useState<AppInfo | null>(null)
  const [shortcut, setShortcut] = useState<ShortcutStatus | null>(null)
  const nav = useRef<HTMLElement>(null)
  const scroller = useRef<HTMLDivElement>(null)

  useEffect(() => {
    window.lumen.appInfo().then(setInfo)
    window.lumen.shortcutStatus().then(setShortcut)
  }, [])

  useEffect(() => {
    scroller.current?.scrollTo({ top: 0 })
  }, [page])

  // ↑/↓ (or Ctrl+Tab) move between pages unless a text field or dropdown has focus.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const t = e.target as HTMLElement
      if (t.closest('input, select, textarea')) return
      const i = PAGES.findIndex((p) => p.id === page)
      let d = 0
      if (e.key === 'ArrowDown' || (e.ctrlKey && e.key === 'Tab' && !e.shiftKey)) d = 1
      else if (e.key === 'ArrowUp' || (e.ctrlKey && e.key === 'Tab' && e.shiftKey)) d = -1
      if (!d) return
      e.preventDefault()
      const next = PAGES[(i + d + PAGES.length) % PAGES.length]
      setPage(next.id)
      nav.current?.querySelector<HTMLButtonElement>(`[data-page="${next.id}"]`)?.focus()
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [page])

  const binding = shortcut?.binding.replace('<Alt>', 'Alt+').replace('<Super>', 'Super+').replace('<Control>', 'Ctrl+').replace('space', 'Space')
  const current = PAGES.find((p) => p.id === page)!

  const reindex = async () => {
    const res = await window.lumen.execute({ action: { type: 'lumen', op: 'reindex' } })
    onToast(res.ok ? 'Rebuilding file index…' : (res.error ?? 'Could not rebuild the index'))
  }

  return (
    <div className="settings">
      <nav className="settings-nav" ref={nav} aria-label="Settings sections">
        <header>
          <button className="icon-btn" onClick={onBack} aria-label="Back to search" title="Back (Esc)">
            <ArrowLeft size={18} />
          </button>
          <h1>Settings</h1>
        </header>
        {PAGES.map((p) => (
          <button key={p.id} data-page={p.id} className={`nav-item${page === p.id ? ' on' : ''}`} aria-current={page === p.id ? 'page' : undefined} onClick={() => setPage(p.id)}>
            <p.icon /> {p.name}
          </button>
        ))}
        <div className="nav-foot">
          <Keys combo="esc" /> to go back
        </div>
      </nav>

      <div className="scroll" ref={scroller} style={{ flex: 1, minWidth: 0 }}>
        <div className="settings-page" key={page}>
          {page !== 'about' && (
            <>
              <h2>{current.name}</h2>
              <p className="lede">{current.lede}</p>
            </>
          )}

          {page === 'general' && (
            <>
              <h3>Shortcut</h3>
              <div className="group">
                <Field
                  title="Open Lumen"
                  hint={shortcut?.configured ? `Press ${binding} anywhere to open Lumen.` : shortcut?.supported === false ? `Bind a key to “${shortcut.command}” in your desktop’s keyboard settings.` : 'Not set up yet.'}
                >
                  {shortcut?.configured ? (
                    <>
                      <Keys combo={binding ?? 'Alt+Space'} />
                      <button className="btn small ghost" onClick={async () => setShortcut(await window.lumen.removeShortcut())}>
                        Remove
                      </button>
                    </>
                  ) : (
                    <button className="btn small primary" onClick={onShortcut}>
                      <Keyboard /> Set up
                    </button>
                  )}
                </Field>
              </div>

              <h3>Startup</h3>
              <div className="group">
                <Switch title="Launch at login" hint="Start quietly in the background so the shortcut opens instantly." on={s.launchAtLogin} onChange={(v) => onChange({ launchAtLogin: v })} />
                <Switch
                  title="Free memory when idle"
                  hint="Unload the panel after a minute hidden. Opening it again takes about 0.1s longer."
                  on={s.freeMemoryWhenIdle}
                  onChange={(v) => onChange({ freeMemoryWhenIdle: v })}
                />
              </div>

              <h3>Terminal</h3>
              <div className="group">
                <Field title="Terminal app" hint="Used for “Run in terminal” and terminal apps.">
                  <select className="select" value={s.terminal} onChange={(e) => onChange({ terminal: e.target.value })}>
                    {TERMINALS.map((t) => (
                      <option key={t} value={t}>
                        {t === 'auto' ? 'Automatic' : t}
                      </option>
                    ))}
                  </select>
                </Field>
              </div>
            </>
          )}

          {page === 'appearance' && (
            <>
              <h3>Look</h3>
              <div className="group">
                <Field title="Theme" hint={s.theme === 'system' ? 'Follows your desktop’s light or dark style.' : undefined}>
                  <div className="segmented" role="radiogroup" aria-label="Theme">
                    {THEMES.map((t) => (
                      <button key={t.id} role="radio" aria-checked={s.theme === t.id} className={s.theme === t.id ? 'on' : ''} onClick={() => onChange({ theme: t.id })}>
                        <t.icon /> {t.name}
                      </button>
                    ))}
                  </div>
                </Field>
                <Field title="Accent colour" hint={ACCENTS.find((a) => a.value === s.accent)?.name ?? 'Custom'}>
                  <div className="swatches" role="radiogroup" aria-label="Accent colour">
                    {ACCENTS.map((a) => (
                      <button
                        key={a.value}
                        role="radio"
                        aria-checked={s.accent === a.value}
                        className={`swatch${s.accent === a.value ? ' on' : ''}`}
                        style={{ ['--c' as string]: a.value }}
                        onClick={() => onChange({ accent: a.value })}
                        aria-label={a.name}
                        title={a.name}
                      >
                        <Check strokeWidth={3} />
                      </button>
                    ))}
                  </div>
                </Field>
                <Field
                  title="Background blur"
                  hint={shellBlur ? 'Real blur is on, drawn by the Lumen Glass GNOME extension.' : 'Frosted tint. Install the Lumen Glass GNOME extension for real blur.'}
                >
                  {shellBlur ? (
                    <span className="badge success">
                      <CircleCheck /> Active
                    </span>
                  ) : (
                    <button className="btn small ghost" onClick={() => openUrl(`${REPO_URL}#gnome-extension-for-real-blur-optional`)}>
                      Learn more <ExternalLink />
                    </button>
                  )}
                </Field>
              </div>

              <h3>Layout</h3>
              <div className="group">
                <Switch title="Preview pane" hint="Show details for the selected result beside the list." on={s.showPreview} onChange={(v) => onChange({ showPreview: v })} />
              </div>
            </>
          )}

          {page === 'search' && (
            <>
              <h3>Web</h3>
              <div className="group">
                <Field title="Search engine" hint="Used when a query isn’t an app, file or link.">
                  <select className="select" value={s.searchEngine} onChange={(e) => onChange({ searchEngine: e.target.value as SearchEngineId })}>
                    {ENGINES.map((e) => (
                      <option key={e.id} value={e.id}>
                        {e.name}
                      </option>
                    ))}
                  </select>
                </Field>
                {s.searchEngine === 'custom' && (
                  <Field title="Custom URL" hint="Put %s where the search text goes.">
                    <input
                      className="text-input"
                      style={{ width: 260 }}
                      defaultValue={s.customSearchUrl}
                      spellCheck={false}
                      onKeyDown={(e) => e.stopPropagation()}
                      onBlur={(e) => onChange({ customSearchUrl: e.target.value })}
                    />
                  </Field>
                )}
              </div>

              <h3>Calculator</h3>
              <div className="group">
                <Switch
                  title="Currency conversion"
                  hint="“100 usd to eur”. Downloads exchange rates once a day."
                  on={s.currencyConversion}
                  onChange={(v) => onChange({ currencyConversion: v })}
                />
              </div>
            </>
          )}

          {page === 'files' && (
            <>
              <h3>Index</h3>
              <div className="group">
                <Switch
                  title="Search files and folders"
                  hint={info ? `${info.indexedFiles.toLocaleString()} items indexed.` : undefined}
                  on={s.fileSearch}
                  onChange={(v) => onChange({ fileSearch: v })}
                />
                <Switch title="Include hidden files" hint="Dotfiles and folders like ~/.config." on={s.includeHidden} onChange={(v) => onChange({ includeHidden: v })} disabled={!s.fileSearch} />
                <Field title="Rebuild index" hint="Re-scan now instead of waiting for changes to be noticed." disabled={!s.fileSearch}>
                  <button className="btn small" onClick={reindex}>
                    <RefreshCw /> Rebuild
                  </button>
                </Field>
              </div>

              <h3>Folders to search</h3>
              <div className="group" style={s.fileSearch ? undefined : { opacity: 0.45, pointerEvents: 'none' }}>
                <PathList items={s.fileRoots} onChange={(v) => onChange({ fileRoots: v })} placeholder="/path/to/folder" icon={Folder} />
              </div>

              <h3>Skipped folders</h3>
              <div className="group" style={s.fileSearch ? undefined : { opacity: 0.45, pointerEvents: 'none' }}>
                <PathList items={s.excludeDirs} onChange={(v) => onChange({ excludeDirs: v })} placeholder="Folder name or path, e.g. node_modules" icon={FolderX} />
              </div>
            </>
          )}

          {page === 'about' && (
            <>
              <div className="about">
                <LumenMark size={64} className="mark" />
                <h2>Lumen</h2>
                <p>{info ? `Version ${info.version}` : ' '}</p>
              </div>
              <div className="group stats">
                <div>
                  <b>{info ? info.indexedApps.toLocaleString() : '–'}</b>
                  <small>Apps</small>
                </div>
                <div>
                  <b>{info ? info.indexedFiles.toLocaleString() : '–'}</b>
                  <small>Files indexed</small>
                </div>
                <div>
                  <b>{info ? info.session : '–'}</b>
                  <small>{info?.desktop.replace(/^ubuntu:/i, '') || 'Desktop'}</small>
                </div>
              </div>
              <div className="group">
                <Field title="Source code and issues" hint="github.com/smith8h/lumen">
                  <button className="btn small ghost" onClick={() => openUrl(REPO_URL)}>
                    Open <ExternalLink />
                  </button>
                </Field>
              </div>
            </>
          )}
        </div>
      </div>
    </div>
  )
}
