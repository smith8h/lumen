import { CircleCheck, CircleAlert, Search, Settings2, TriangleAlert } from 'lucide-react'
import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react'
import type { Preview, ResultItem, ResultSection, SearchResponse, SecondaryAction, Settings } from '@shared/types'
import { ActionMenu } from './components/ActionMenu'
import { Keys, LumenMark } from './components/bits'
import { GLYPHS } from './components/Icon'
import { OutputView, type RunState } from './components/OutputView'
import { PreviewPane } from './components/PreviewPane'
import { ResultList } from './components/ResultList'
import { SettingsView } from './components/SettingsView'
import { Suggestions } from './components/Suggestions'
import { WelcomeView } from './components/WelcomeView'
import { setHome } from './lib/paths'
import { resetPointer } from './lib/pointer'

type View = 'search' | 'settings' | 'shortcut'
const api = window.lumen

/** How much of the glass tint to keep when the GNOME extension blurs the background for real. */
const SHELL_BLUR_TINT = 0.7

function useTheme(settings: Settings | null, shellBlur: boolean): void {
  const [systemDark, setSystemDark] = useState(() => matchMedia('(prefers-color-scheme: dark)').matches)
  useEffect(() => {
    const mq = matchMedia('(prefers-color-scheme: dark)')
    const fn = () => setSystemDark(mq.matches)
    mq.addEventListener('change', fn)
    return () => mq.removeEventListener('change', fn)
  }, [])
  useLayoutEffect(() => {
    if (!settings) return
    const root = document.documentElement
    root.dataset.theme = settings.theme === 'system' ? (systemDark ? 'dark' : 'light') : settings.theme
    root.style.setProperty('--accent', settings.accent)
    root.style.setProperty('--glass-alpha', String(settings.glassOpacity * (shellBlur ? SHELL_BLUR_TINT : 1)))
    root.toggleAttribute('data-shell-blur', shellBlur)
  }, [settings, systemDark, shellBlur])
}

/** Whether the GNOME extension is drawing real blur behind the panel. */
function useShellBlur(): boolean {
  const [on, setOn] = useState(false)
  useEffect(() => {
    api.shellBlur().then(setOn)
    return api.onShellBlur(setOn)
  }, [])
  return on
}

/**
 * Reports the panel's rectangle to the main process while it's open, so the GNOME extension
 * can blur exactly the area behind it. Follows every size change, including animations.
 */
function usePanelGeometry(ref: React.RefObject<HTMLElement | null>, open: boolean): void {
  useLayoutEffect(() => {
    const el = ref.current
    if (!el || !open) {
      api.reportPanel(null)
      return
    }
    let frame = 0
    const report = () => {
      cancelAnimationFrame(frame)
      frame = requestAnimationFrame(() => {
        api.reportPanel({
          x: el.offsetLeft,
          y: el.offsetTop,
          width: el.offsetWidth,
          height: el.offsetHeight,
          radius: parseFloat(getComputedStyle(el).borderTopLeftRadius) || 0
        })
      })
    }
    const ro = new ResizeObserver(report)
    ro.observe(el)
    window.addEventListener('resize', report)
    report()
    return () => {
      ro.disconnect()
      window.removeEventListener('resize', report)
      cancelAnimationFrame(frame)
      api.reportPanel(null)
    }
  }, [ref, open])
}

/** Animates the panel body's height to whatever its content needs. */
function useAutoHeight<T extends HTMLElement>(): [React.RefObject<T | null>, number | undefined] {
  const ref = useRef<T>(null)
  const [height, setHeight] = useState<number>()
  useLayoutEffect(() => {
    const el = ref.current
    if (!el) return
    const ro = new ResizeObserver(() => setHeight(el.offsetHeight))
    ro.observe(el)
    setHeight(el.offsetHeight)
    return () => ro.disconnect()
  }, [])
  return [ref, height]
}

function primaryAction(item: ResultItem): SecondaryAction {
  const labels: Record<ResultItem['kind'], string> = {
    app: 'Open',
    file: 'Open',
    folder: 'Open folder',
    calc: 'Copy result',
    url: 'Open link',
    web: 'Search',
    command: item.action.type === 'run-command' && item.action.mode === 'inline' ? 'Run and show output' : 'Run in terminal',
    system: 'Run',
    lumen: 'Open'
  }
  return { label: labels[item.kind], hint: 'Enter', action: item.action }
}

export function App() {
  const [settings, setSettings] = useState<Settings | null>(null)
  const [open, setOpen] = useState(false)
  const [view, setView] = useState<View>('search')
  const [query, setQuery] = useState('')
  const [response, setResponse] = useState<SearchResponse | null>(null)
  const [suggested, setSuggested] = useState<ResultSection[]>([])
  const [selected, setSelected] = useState(0)
  const [preview, setPreview] = useState<{ id: string; data: Preview } | null>(null)
  const [menu, setMenu] = useState<{ selected: number } | null>(null)
  const [toast, setToast] = useState<{ text: string; error?: boolean; id: number } | null>(null)
  const [confirmId, setConfirmId] = useState<string | null>(null)
  const [run, setRun] = useState<RunState | null>(null)
  const input = useRef<HTMLInputElement>(null)
  const seq = useRef(0)
  const previewCache = useRef(new Map<string, Preview>())
  const [bodyRef, bodyHeight] = useAutoHeight<HTMLDivElement>()
  const panelRef = useRef<HTMLDivElement>(null)
  const shellBlur = useShellBlur()

  useTheme(settings, shellBlur)
  usePanelGeometry(panelRef, open && !!settings)

  // ── Wiring to the main process ──────────────────────────────────────────
  useEffect(() => {
    api.appInfo().then((i) => setHome(i.home))
    api.getSettings().then(setSettings)
    api.suggestions().then(setSuggested)
    const offs = [
      api.onSettings(setSettings),
      api.onShow((opts) => {
        setOpen(true)
        resetPointer()
        setMenu(null)
        setConfirmId(null)
        previewCache.current.clear()
        api.suggestions().then(setSuggested)
        // Alt+Space always lands on search, like Spotlight.
        setView(opts.view ?? 'search')
        setRun((r) => (r && r.done ? null : r))
        if (typeof opts.query === 'string') setQuery(opts.query)
        requestAnimationFrame(() => {
          input.current?.focus()
          input.current?.select()
        })
      }),
      api.onHide(() => {
        setOpen(false)
        setMenu(null)
        setToast(null)
      }),
      api.onCommandOutput((e) => {
        setRun((r) => {
          if (!r || r.runId !== e.runId) return r
          if (e.done) return { ...r, done: true, exitCode: e.exitCode ?? null, endedAt: Date.now() }
          return { ...r, chunks: [...r.chunks, { text: e.chunk ?? '', err: e.stream === 'stderr' }] }
        })
      })
    ]
    return () => offs.forEach((off) => off())
  }, [])

  // Tell the main process we can be shown (after a cold start, nothing is visible until now).
  const loaded = settings !== null
  useEffect(() => {
    if (loaded) api.ready()
  }, [loaded])

  // ── Search ──────────────────────────────────────────────────────────────
  useEffect(() => {
    const id = ++seq.current
    setConfirmId(null)
    setMenu(null)
    if (!query.trim()) {
      setResponse(null)
      setSelected(0)
      return
    }
    api.search(query, id).then((r) => {
      if (r.seq !== seq.current) return
      setResponse(r)
      setSelected(0)
    })
  }, [query])

  const sections = query.trim() ? (response?.sections ?? []) : suggested
  const items = useMemo(() => sections.flatMap((s) => s.items), [sections])
  const current: ResultItem | undefined = items[selected]

  // ── Preview of the selected result ──────────────────────────────────────
  const showPreview = !!settings?.showPreview && !!query.trim() && items.length > 0 && view === 'search' && !run
  useEffect(() => {
    if (!showPreview || !current) return
    const cached = previewCache.current.get(current.id)
    if (cached) {
      setPreview({ id: current.id, data: cached })
      return
    }
    const t = setTimeout(() => {
      api.preview(current).then((data) => {
        previewCache.current.set(current.id, data)
        setPreview({ id: current.id, data })
      })
    }, 40)
    return () => clearTimeout(t)
  }, [current, showPreview])

  // ── Actions ─────────────────────────────────────────────────────────────
  const flash = useCallback((text: string, error = false) => setToast({ text, error, id: Date.now() }), [])
  useEffect(() => {
    if (!toast) return
    const t = setTimeout(() => setToast(null), toast.error ? 2600 : 1400)
    return () => clearTimeout(t)
  }, [toast])

  const perform = useCallback(
    async (item: ResultItem, action = item.action) => {
      if (item.confirm && action === item.action && confirmId !== item.id) {
        setConfirmId(item.id)
        return
      }
      setConfirmId(null)
      setMenu(null)
      const res = await api.execute({ action, historyKey: item.historyKey })
      if (!res.ok) {
        flash(res.error ?? 'Something went wrong', true)
        return
      }
      if (res.view) {
        setView(res.view)
        return
      }
      if (res.runId && action.type === 'run-command') {
        setRun({ runId: res.runId, command: action.command, chunks: [], done: false, exitCode: null, startedAt: Date.now() })
        return
      }
      if (res.toast) {
        flash(res.toast)
        if (!res.keepOpen) setTimeout(() => api.hide(), 650)
        return
      }
      if (!res.keepOpen) api.hide()
    },
    [confirmId, flash]
  )

  const copy = useCallback(
    async (text: string) => {
      await api.execute({ action: { type: 'copy', text } })
      flash('Copied to clipboard')
    },
    [flash]
  )

  const menuActions = useMemo(() => (current ? [primaryAction(current), ...(current.actions ?? [])] : []), [current])

  const secondary = useCallback(() => {
    if (!current) return
    const alt = current.actions?.find((a) => a.hint === 'Ctrl+Enter')
    if (alt) perform(current, alt.action)
  }, [current, perform])

  const updateSettings = useCallback(async (patch: Partial<Settings>) => {
    setSettings((s) => (s ? { ...s, ...patch } : s))
    setSettings(await api.setSettings(patch))
  }, [])

  const finishWelcome = useCallback(() => {
    updateSettings({ firstRunDone: true })
    setView('search')
    requestAnimationFrame(() => input.current?.focus())
  }, [updateSettings])

  // ── Keyboard ────────────────────────────────────────────────────────────
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const ctrl = e.ctrlKey || e.metaKey
      if (view === 'settings') {
        if (e.key === 'Escape') {
          e.preventDefault()
          setView('search')
          requestAnimationFrame(() => input.current?.focus())
        }
        return
      }
      if (view === 'shortcut') {
        if (e.key === 'Escape') {
          e.preventDefault()
          finishWelcome()
        }
        return
      }
      if (run) {
        if (e.key === 'Escape') {
          e.preventDefault()
          if (!run.done) api.killCommand(run.runId)
          setRun(null)
          requestAnimationFrame(() => input.current?.focus())
        } else if (ctrl && e.key === 'c' && !run.done && !getSelection()?.toString()) {
          e.preventDefault()
          api.killCommand(run.runId)
        } else if (e.key === 'Enter' && ctrl) {
          e.preventDefault()
          api.execute({ action: { type: 'run-command', command: run.command, mode: 'terminal' } }).then(() => api.hide())
        } else if (e.key === 'Enter' && run.done) {
          e.preventDefault()
          const fake: ResultItem = { id: 'rerun', kind: 'command', title: run.command, score: 0, action: { type: 'run-command', command: run.command, mode: 'inline' } }
          perform(fake)
        }
        return
      }
      if (menu) {
        if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
          e.preventDefault()
          const d = e.key === 'ArrowDown' ? 1 : -1
          setMenu({ selected: (menu.selected + d + menuActions.length) % menuActions.length })
        } else if (e.key === 'Enter') {
          e.preventDefault()
          if (current) perform(current, menuActions[menu.selected].action)
        } else if (e.key === 'Escape' || (ctrl && e.key === 'k') || e.key === 'Tab') {
          e.preventDefault()
          setMenu(null)
        }
        return
      }

      const goTo = (index: number) => {
        e.preventDefault()
        setConfirmId(null)
        setSelected(Math.max(0, Math.min(items.length - 1, index)))
      }
      const move = (d: number) => goTo(selected + d)
      // Before typing, the first section is a row of app tiles: ←/→ move inside it, ↑/↓ jump in and out.
      const gridCount = !query.trim() && suggested[0]?.id === 'apps' ? suggested[0].items.length : 0
      const inGrid = selected < gridCount
      if (e.key === 'ArrowDown' || (ctrl && (e.key === 'n' || e.key === 'j'))) goTo(inGrid ? (gridCount < items.length ? gridCount : selected) : selected + 1)
      else if (e.key === 'ArrowUp' || (ctrl && e.key === 'p')) goTo(inGrid ? selected : selected === gridCount ? 0 : selected - 1)
      else if (inGrid && (e.key === 'ArrowRight' || e.key === 'ArrowLeft')) move(e.key === 'ArrowRight' ? 1 : -1)
      else if (e.key === 'PageDown') move(5)
      else if (e.key === 'PageUp') move(-5)
      else if (e.key === 'Enter') {
        e.preventDefault()
        if (!current) return
        if (ctrl) secondary()
        else perform(current)
      } else if ((ctrl && e.key === 'k') || (e.key === 'Tab' && current)) {
        e.preventDefault()
        if (menuActions.length > 1) setMenu({ selected: 0 })
      } else if (ctrl && e.key === ',') {
        e.preventDefault()
        setView('settings')
      } else if (e.key === 'Escape') {
        e.preventDefault()
        if (confirmId) setConfirmId(null)
        else if (query) setQuery('')
        else api.hide()
      }
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [view, run, menu, menuActions, items, selected, current, query, suggested, confirmId, perform, secondary, finishWelcome])

  if (!settings) return null

  // ── Render ──────────────────────────────────────────────────────────────
  const hasResults = items.length > 0
  const mode = query.trim() ? response?.mode : undefined
  const ModeGlyph = mode ? GLYPHS[mode.glyph] : null
  const confirmText = current && confirmId === current.id ? current.confirm : null

  let body: React.ReactNode
  if (view === 'settings') {
    body = <SettingsView settings={settings} onChange={updateSettings} onBack={() => setView('search')} onShortcut={() => setView('shortcut')} shellBlur={shellBlur} />
  } else if (view === 'shortcut') {
    body = <WelcomeView onDone={finishWelcome} onCopy={copy} />
  } else if (run) {
    body = (
      <OutputView
        run={run}
        onStop={() => api.killCommand(run.runId)}
        onRerun={() => perform({ id: 'rerun', kind: 'command', title: run.command, score: 0, action: { type: 'run-command', command: run.command, mode: 'inline' } })}
        onTerminal={() => api.execute({ action: { type: 'run-command', command: run.command, mode: 'terminal' } }).then(() => api.hide())}
        onCopy={copy}
      />
    )
  } else if (!query.trim()) {
    body = (
      <Suggestions
        sections={suggested}
        selected={selected}
        onHover={setSelected}
        onRun={(i) => items[i] && perform(items[i])}
        onTip={(prefix) => {
          setQuery(prefix)
          input.current?.focus()
        }}
      />
    )
  } else if (hasResults) {
    body = (
      <div className={`split${showPreview ? ' with-preview' : ''}`}>
        <ResultList sections={sections} selected={selected} onHover={setSelected} onRun={(i) => items[i] && perform(items[i])} />
        {showPreview && current && <PreviewPane item={current} preview={preview?.id === current.id ? preview.data : null} />}
      </div>
    )
  } else if (response) {
    body = (
      <div className="empty">
        <Search />
        No results for “{query.trim()}”
      </div>
    )
  }

  const hints: [string, string][] =
    view !== 'search' || run
      ? []
      : !current
        ? [['esc', 'Close']]
        : current.kind === 'file' || current.kind === 'folder'
          ? [['Enter', 'Open'], ['Ctrl+Enter', 'Show in Files'], ['Ctrl+K', 'Actions']]
          : current.kind === 'command'
            ? [['Enter', 'Run'], ['Ctrl+K', 'Actions']]
            : current.kind === 'calc'
              ? [['Enter', 'Copy']]
              : menuActions.length > 1
                ? [['Enter', primaryAction(current).label], ['Ctrl+K', 'Actions']]
                : [['Enter', primaryAction(current).label]]

  const searchVisible = view === 'search'
  return (
    <div
      className={`stage${open ? ' open' : ''}`}
      onMouseDown={(e) => {
        if (e.target === e.currentTarget || (e.target as HTMLElement).classList.contains('backdrop')) api.hide()
      }}
    >
      <div className={`backdrop${settings.dimBackground ? ' on' : ''}`} />
      <div className="panel" ref={panelRef} onMouseDown={() => menu && setMenu(null)}>
        {searchVisible && (
          <>
            <div className="searchbar">
              <div className={`lead${mode ? ' active' : ''}`}>{ModeGlyph ? <ModeGlyph size={22} /> : <Search size={22} />}</div>
              <input
                ref={input}
                value={query}
                autoFocus
                spellCheck={false}
                placeholder="Search apps, files and the web…"
                onChange={(e) => setQuery(e.target.value)}
              />
              {mode && (
                <span className="chip" key={mode.label}>
                  {ModeGlyph && <ModeGlyph size={13} />} {mode.label}
                </span>
              )}
              <button className="icon-btn" onClick={() => setView('settings')} aria-label="Settings" title="Settings (Ctrl+,)">
                <Settings2 size={18} />
              </button>
            </div>
            <div className={`divider${body ? '' : ' hidden'}`} />
          </>
        )}

        <div className="body" style={{ height: bodyHeight }}>
          <div ref={bodyRef}>{body}</div>
        </div>

        {searchVisible && (body || current) && (
          <div className="footer">
            <span className="brand">
              {confirmText ? (
                <span className="confirm-bar">
                  <TriangleAlert size={15} /> {confirmText}
                </span>
              ) : (
                <>
                  <LumenMark size={16} /> Lumen
                  {query.trim() && response && !run && (
                    <span className="status">
                      · {items.length} result{items.length === 1 ? '' : 's'} in {response.tookMs}ms
                    </span>
                  )}
                </>
              )}
            </span>
            {hints.map(([k, label]) => (
              <span className="hint" key={k}>
                {label} <Keys combo={k} />
              </span>
            ))}
          </div>
        )}

        {menu && current && (
          <ActionMenu
            title={current.title}
            actions={menuActions}
            selected={menu.selected}
            onHover={(i) => setMenu({ selected: i })}
            onRun={(i) => perform(current, menuActions[i].action)}
          />
        )}

        {toast && (
          <div className={`toast${toast.error ? ' error' : ''}`} key={toast.id}>
            {toast.error ? <CircleAlert /> : <CircleCheck />} {toast.text}
          </div>
        )}
      </div>
    </div>
  )
}
