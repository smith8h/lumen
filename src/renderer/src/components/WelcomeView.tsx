import { Check, Copy, Keyboard } from 'lucide-react'
import { useEffect, useState } from 'react'
import type { ShortcutStatus } from '@shared/types'
import { LumenMark } from './bits'

interface Props {
  onDone(): void
  onCopy(text: string): void
}

/** First-run screen: explains Alt+Space and sets the shortcut up with one click. */
export function WelcomeView({ onDone, onCopy }: Props) {
  const [status, setStatus] = useState<ShortcutStatus | null>(null)
  const [busy, setBusy] = useState(false)

  useEffect(() => {
    window.lumen.shortcutStatus().then(setStatus)
  }, [])

  const setup = async () => {
    setBusy(true)
    setStatus(await window.lumen.setupShortcut())
    setBusy(false)
  }

  return (
    <div className="welcome">
      <LumenMark size={72} className="mark" />
      <h1>Welcome to Lumen</h1>
      <p>Apps, files, calculations, the web and your terminal — one keystroke away.</p>
      <div className="bigkeys">
        <span className="kbd">Alt</span>
        <span style={{ color: 'var(--text-3)' }}>+</span>
        <span className="kbd space">Space</span>
      </div>

      {status?.configured ? (
        <>
          <span className="status-line">
            <Check size={16} /> Shortcut ready — press Alt+Space anywhere
          </span>
          <div className="buttons">
            <button className="btn primary" onClick={onDone} autoFocus>
              Start searching
            </button>
          </div>
        </>
      ) : status?.supported ? (
        <>
          <p className="note">
            Lumen adds a keyboard shortcut to {status.desktop.replace(/^ubuntu:/i, '')}
            {status.conflicts.length ? ' and moves GNOME’s own Alt+Space (the window menu) out of the way' : ''}. You can undo this
            anytime in Settings.
          </p>
          <div className="buttons">
            <button className="btn primary" onClick={setup} disabled={busy} autoFocus>
              <Keyboard /> Enable Alt+Space
            </button>
            <button className="btn ghost" onClick={onDone}>
              Not now
            </button>
          </div>
        </>
      ) : status ? (
        <>
          <p className="note">
            Add a custom shortcut in your desktop’s keyboard settings that runs this command, bound to Alt+Space:
          </p>
          <div className="manual">
            <code>{status.command}</code>
            <button className="btn small" onClick={() => onCopy(status.command)}>
              <Copy /> Copy
            </button>
          </div>
          <div className="buttons">
            <button className="btn primary" onClick={onDone} autoFocus>
              Start searching
            </button>
          </div>
        </>
      ) : null}
    </div>
  )
}
