import { ArrowDown, ArrowUp, CornerDownLeft } from 'lucide-react'
import { Fragment, useId } from 'react'

const KEY_LABELS: Record<string, React.ReactNode> = {
  enter: <CornerDownLeft />,
  up: <ArrowUp />,
  down: <ArrowDown />,
  ctrl: 'Ctrl',
  alt: 'Alt',
  shift: 'Shift',
  esc: 'esc',
  tab: 'Tab',
  space: 'Space'
}

/** Renders "Ctrl+Enter" as keycaps. */
export function Keys({ combo }: { combo: string }) {
  return (
    <span className="keys">
      {combo.split('+').map((k, i) => (
        <span className="kbd" key={i}>
          {KEY_LABELS[k.toLowerCase()] ?? k}
        </span>
      ))}
    </span>
  )
}

/** Title with matched characters emphasised. */
export function Highlight({ text, indexes }: { text: string; indexes?: number[] }) {
  if (!indexes?.length) return <>{text}</>
  const set = new Set(indexes)
  const parts: { s: string; m: boolean }[] = []
  for (let i = 0; i < text.length; i++) {
    const m = set.has(i)
    const last = parts[parts.length - 1]
    if (last && last.m === m) last.s += text[i]
    else parts.push({ s: text[i], m })
  }
  return (
    <>
      {parts.map((p, i) => (p.m ? <mark key={i}>{p.s}</mark> : <Fragment key={i}>{p.s}</Fragment>))}
    </>
  )
}

/** The Lumen mark: a glass lens with a four-point glint. */
export function LumenMark({ size = 20, className }: { size?: number; className?: string }) {
  const id = useId().replace(/:/g, '')
  return (
    <svg width={size} height={size} viewBox="0 0 64 64" className={className} aria-hidden>
      <defs>
        <linearGradient id={`${id}g`} x1="8" y1="6" x2="58" y2="60" gradientUnits="userSpaceOnUse">
          <stop stopColor="#9d8cff" />
          <stop offset="0.55" stopColor="#6c5cff" />
          <stop offset="1" stopColor="#ff6fb5" />
        </linearGradient>
        <radialGradient id={`${id}s`} cx="0" cy="0" r="1" gradientTransform="translate(22 16) rotate(50) scale(40)" gradientUnits="userSpaceOnUse">
          <stop stopColor="white" stopOpacity="0.55" />
          <stop offset="1" stopColor="white" stopOpacity="0" />
        </radialGradient>
      </defs>
      <rect x="4" y="4" width="56" height="56" rx="17" fill={`url(#${id}g)`} />
      <rect x="4" y="4" width="56" height="56" rx="17" fill={`url(#${id}s)`} />
      <rect x="4.5" y="4.5" width="55" height="55" rx="16.5" fill="none" stroke="white" strokeOpacity="0.35" />
      <path d="M32 15c1.4 9.2 7.8 15.6 17 17-9.2 1.4-15.6 7.8-17 17-1.4-9.2-7.8-15.6-17-17 9.2-1.4 15.6-7.8 17-17Z" fill="white" />
      <circle cx="45" cy="19" r="2.6" fill="white" fillOpacity="0.85" />
    </svg>
  )
}
