import type { ResultSection } from '@shared/types'
import { Icon } from './Icon'
import { ResultList } from './ResultList'
import { pointerMoved } from '../lib/pointer'

const TIPS = [
  { code: '>', text: 'Run a command' },
  { code: '=', text: 'Calculate' },
  { code: '?', text: 'Search the web' },
  { code: '~/', text: 'Browse files' }
]

interface Props {
  sections: ResultSection[]
  selected: number
  onHover(i: number): void
  onRun(i: number): void
  onTip(prefix: string): void
}

/** What you see before typing: your most used apps, recent files and a few tips. */
export function Suggestions({ sections, selected, onHover, onRun, onTip }: Props) {
  const [apps, ...rest] = sections[0]?.id === 'apps' ? sections : [undefined, ...sections]
  const offset = apps?.items.length ?? 0
  return (
    <div className="suggest">
      {apps && (
        <>
          <div className="section-title">{apps.title}</div>
          <div className="grid">
            {apps.items.map((item, i) => (
              <div
                key={item.id}
                className={`tile-app${i === selected ? ' selected' : ''}`}
                onMouseMove={(e) => pointerMoved(e) && i !== selected && onHover(i)}
                onClick={() => onRun(i)}
                title={item.subtitle}
              >
                <Icon spec={item.icon} />
                <span>{item.title}</span>
              </div>
            ))}
          </div>
        </>
      )}
      {rest.length > 0 && (
        <div style={{ margin: '0 -8px' }}>
          <ResultList
            sections={rest as ResultSection[]}
            selected={selected - offset}
            onHover={(i) => onHover(i + offset)}
            onRun={(i) => onRun(i + offset)}
          />
        </div>
      )}
      <div className="section-title">Tips</div>
      <div className="tips">
        {TIPS.map((t) => (
          <div className="tip" key={t.code} onClick={() => onTip(t.code === '~/' ? '~/' : `${t.code} `)}>
            <code>{t.code}</code>
            {t.text}
          </div>
        ))}
      </div>
    </div>
  )
}
