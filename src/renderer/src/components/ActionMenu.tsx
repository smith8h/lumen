import { ChevronRight } from 'lucide-react'
import type { SecondaryAction } from '@shared/types'
import { Keys } from './bits'
import { GLYPHS } from './Icon'
import { pointerMoved } from '../lib/pointer'

interface Props {
  title: string
  actions: SecondaryAction[]
  selected: number
  onHover(i: number): void
  onRun(i: number): void
}

export function ActionMenu({ title, actions, selected, onHover, onRun }: Props) {
  return (
    <div className="action-menu" onMouseDown={(e) => e.stopPropagation()}>
      <div className="menu-title">{title}</div>
      {actions.map((a, i) => {
        const G = a.glyph ? GLYPHS[a.glyph] : ChevronRight
        return (
          <div key={a.label + i} className={`item${i === selected ? ' selected' : ''}`} onMouseMove={(e) => pointerMoved(e) && onHover(i)} onClick={() => onRun(i)}>
            <G />
            <span className="label">{a.label}</span>
            {a.hint && <Keys combo={a.hint} />}
          </div>
        )
      })}
    </div>
  )
}
