import { useLayoutEffect, useRef, useState } from 'react'
import type { ResultItem, ResultSection } from '@shared/types'
import { Highlight, Keys } from './bits'
import { Icon } from './Icon'
import { pointerMoved } from '../lib/pointer'

const KIND_LABEL: Record<ResultItem['kind'], string> = {
  app: 'Application',
  file: 'File',
  folder: 'Folder',
  calc: 'Copy',
  url: 'Open',
  web: 'Search',
  command: 'Run',
  system: 'Run',
  lumen: 'Open'
}

interface Props {
  sections: ResultSection[]
  selected: number
  onHover(index: number): void
  onRun(index: number): void
}

export function ResultList({ sections, selected, onHover, onRun }: Props) {
  const scroller = useRef<HTMLDivElement>(null)
  const rows = useRef<(HTMLDivElement | null)[]>([])
  const [bar, setBar] = useState<{ y: number; h: number } | null>(null)

  useLayoutEffect(() => {
    const el = rows.current[selected]
    if (!el) {
      setBar(null)
      return
    }
    setBar({ y: el.offsetTop, h: el.offsetHeight })
    const s = scroller.current
    if (!s) return
    const top = el.offsetTop - 30
    const bottom = el.offsetTop + el.offsetHeight + 8
    if (top < s.scrollTop) s.scrollTop = Math.max(0, top)
    else if (bottom > s.scrollTop + s.clientHeight) s.scrollTop = bottom - s.clientHeight
  }, [selected, sections])

  let flat = 0
  rows.current = []
  return (
    <div className="results" ref={scroller}>
      {bar && <div className="selection" style={{ transform: `translateY(${bar.y}px)`, height: bar.h }} />}
      {sections.map((section) => (
        <div key={section.id}>
          <div className="section-title">{section.title}</div>
          {section.items.map((item) => {
            const index = flat++
            const isSel = index === selected
            const top = section.id === 'top'
            return (
              <div
                key={item.id}
                ref={(el) => {
                  rows.current[index] = el
                }}
                className={`row ${item.kind}${top ? ' top' : ''}${isSel ? ' selected' : ''}`}
                onMouseMove={(e) => pointerMoved(e) && index !== selected && onHover(index)}
                onClick={() => onRun(index)}
              >
                <Icon spec={item.icon} thumb={item.icon?.type === 'file' && item.kind === 'file'} />
                <div className="text">
                  <div className="title">
                    <Highlight text={item.title} indexes={item.matches} />
                  </div>
                  {item.subtitle && <div className="subtitle">{item.subtitle}</div>}
                </div>
                <div className="trail">
                  {item.confirm ? null : <span className="kind">{KIND_LABEL[item.kind]}</span>}
                  <Keys combo="Enter" />
                </div>
              </div>
            )
          })}
        </div>
      ))}
    </div>
  )
}
