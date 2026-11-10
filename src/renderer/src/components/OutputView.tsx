import { Copy, RotateCcw, SquareTerminal, Square } from 'lucide-react'
import { useEffect, useRef } from 'react'
import { Keys } from './bits'

export interface RunState {
  runId: string
  command: string
  chunks: { text: string; err: boolean }[]
  done: boolean
  exitCode: number | null
  startedAt: number
  endedAt?: number
}

// eslint-disable-next-line no-control-regex
const ANSI = /\x1b\[[0-9;?]*[ -/]*[@-~]|\x1b\][^\x07]*\x07|\r(?!\n)/g

interface Props {
  run: RunState
  onStop(): void
  onRerun(): void
  onTerminal(): void
  onCopy(text: string): void
}

export function OutputView({ run, onStop, onRerun, onTerminal, onCopy }: Props) {
  const pre = useRef<HTMLPreElement>(null)
  const stick = useRef(true)

  useEffect(() => {
    const el = pre.current
    if (el && stick.current) el.scrollTop = el.scrollHeight
  }, [run.chunks.length])

  const text = run.chunks.map((c) => c.text.replace(ANSI, '')).join('')
  const secs = (((run.endedAt ?? Date.now()) - run.startedAt) / 1000).toFixed(1)
  const status = !run.done ? (
    <span className="status">
      <span className="spinner" /> Running
    </span>
  ) : run.exitCode === 0 ? (
    <span className="status ok">Done · {secs}s</span>
  ) : (
    <span className="status fail">Exit {run.exitCode ?? 'killed'} · {secs}s</span>
  )

  return (
    <div className="output fade-in">
      <header>
        <span className="cmd">
          <span style={{ color: 'var(--accent-strong)' }}>❯</span> {run.command}
        </span>
        {status}
      </header>
      <pre
        ref={pre}
        onScroll={(e) => {
          const el = e.currentTarget
          stick.current = el.scrollHeight - el.scrollTop - el.clientHeight < 24
        }}
      >
        {run.chunks.length === 0 && !run.done && <span className="dim">Waiting for output…</span>}
        {run.chunks.map((c, i) => (
          <span key={i} className={c.err ? 'err' : undefined}>
            {c.text.replace(ANSI, '')}
          </span>
        ))}
        {run.done && run.chunks.length === 0 && <span className="dim">(no output)</span>}
      </pre>
      <div style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
        {!run.done ? (
          <button className="btn small" onClick={onStop}>
            <Square /> Stop <Keys combo="Ctrl+C" />
          </button>
        ) : (
          <button className="btn small" onClick={onRerun}>
            <RotateCcw /> Run again <Keys combo="Enter" />
          </button>
        )}
        <button className="btn small" onClick={() => onCopy(text)} disabled={!text}>
          <Copy /> Copy output
        </button>
        <button className="btn small ghost" onClick={onTerminal}>
          <SquareTerminal /> Open in terminal <Keys combo="Ctrl+Enter" />
        </button>
      </div>
    </div>
  )
}
