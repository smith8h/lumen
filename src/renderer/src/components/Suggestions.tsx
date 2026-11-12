import type { ResultSection } from '@shared/types'
import { Icon } from './Icon'
import { ResultList } from './ResultList'
import { pointerMoved } from '../lib/pointer'

const TIPS = [
  { code: '=', text: 'calculate' },
  { code: '>', text: 'run a command' },
  { code: '?', text: 'search the web' },
  { code: ';', text: 'clipboard' },
  { code: '~/', text: 'browse files' }
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
          {/* At least six columns, so a short list doesn't stretch into giant tiles. */}
          <div className="grid" style={{ ['--cols' as string]: Math.max(6, apps.items.length) }}>
            {apps.items.map((item, i) => (
              <div
                key={item.id}
                className={`tile-app${i === selected ? ' selected' : ''}`}
                onMouseMove={(e) => pointerMoved(e) && i !== selected && onHover(i)}
                onClick={() => onRun(i)}
                role="button"
                aria-label={item.subtitle ? `${item.title}, ${item.subtitle}` : item.title}
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
      <div className="tips">
        <span className="lead-in">Start with</span>
        {TIPS.map((t) => (
          <button className="tip" key={t.code} onClick={() => onTip(t.code === '~/' ? '~/' : `${t.code} `)}>
            <code>{t.code}</code>
            {t.text}
          </button>
        ))}
      </div>
    </div>
  )
}
