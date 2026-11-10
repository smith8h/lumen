import { File, Folder } from 'lucide-react'
import type { Preview, ResultItem } from '@shared/types'
import { Keys } from './bits'
import { Icon, iconUrl } from './Icon'
import { tilde } from '../lib/paths'

const size = (n: number): string => {
  if (n < 1024) return `${n} B`
  const units = ['KB', 'MB', 'GB', 'TB']
  let v = n / 1024
  let i = 0
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024
    i++
  }
  return `${v.toFixed(v < 10 ? 1 : 0)} ${units[i]}`
}

const when = (ms: number): string => {
  const d = new Date(ms)
  const diff = Date.now() - ms
  if (diff < 60_000) return 'Just now'
  if (diff < 3600_000) return `${Math.round(diff / 60_000)} min ago`
  if (diff < 86_400_000 && d.getDate() === new Date().getDate()) return `Today, ${d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}`
  return d.toLocaleDateString([], { year: 'numeric', month: 'short', day: 'numeric' })
}


function Meta({ rows }: { rows: [string, string | undefined][] }) {
  return (
    <dl className="meta">
      {rows
        .filter(([, v]) => v)
        .map(([k, v]) => (
          <div key={k} style={{ display: 'contents' }}>
            <dt>{k}</dt>
            <dd title={v}>{v}</dd>
          </div>
        ))}
    </dl>
  )
}

export function PreviewPane({ item, preview }: { item: ResultItem; preview: Preview | null }) {
  const p = preview
  const key = item.id

  if (!p || p.type === 'none') {
    return (
      <div className="preview" key={key}>
        <div className="hero">
          <div className="big-icon">
            <Icon spec={item.icon} />
          </div>
          <h3>{item.title}</h3>
          {item.subtitle && <p>{item.subtitle}</p>}
        </div>
        {item.kind === 'command' && (
          <Meta
            rows={[
              ['Enter', item.action.type === 'run-command' && item.action.mode === 'inline' ? 'Run & show output' : 'Run in terminal'],
              ['Ctrl+K', 'More actions']
            ]}
          />
        )}
        {(item.kind === 'web' || item.kind === 'url') && item.action.type === 'open-url' && (
          <Meta rows={[['Opens', new URL(item.action.url).hostname]]} />
        )}
      </div>
    )
  }

  switch (p.type) {
    case 'app':
      return (
        <div className="preview" key={key}>
          <div className="hero">
            <div className="big-icon">{p.icon?.type === 'file' ? <img src={iconUrl(p.icon.path)} alt="" /> : <Icon spec={p.icon} />}</div>
            <h3>{p.name}</h3>
            {p.comment && <p>{p.comment}</p>}
            {!!p.categories?.length && (
              <div className="chips">
                {p.categories.map((c) => (
                  <span key={c}>{c.replace(/([a-z])([A-Z])/g, '$1 $2')}</span>
                ))}
              </div>
            )}
          </div>
          {!!item.actions?.length && (
            <div className="actions-list">
              {item.actions.slice(0, 3).map((a) => (
                <div key={a.label} className="btn small block ghost" style={{ pointerEvents: 'none' }}>
                  {a.label}
                </div>
              ))}
            </div>
          )}
          <Meta rows={[['Command', tilde(p.exec ?? '')], ['Entry', tilde(p.path)]]} />
        </div>
      )
    case 'image':
      return (
        <div className="preview" key={key}>
          <div className="image">
            <img src={iconUrl(p.path)} alt="" draggable={false} />
          </div>
          <Meta
            rows={[
              ['Name', p.name],
              ['Dimensions', p.width ? `${p.width} × ${p.height}` : undefined],
              ['Size', size(p.size)],
              ['Modified', when(p.modified)]
            ]}
          />
        </div>
      )
    case 'text':
      return (
        <div className="preview" key={key}>
          <pre className="code">{p.content.split('\n').slice(0, 40).join('\n')}</pre>
          <Meta rows={[['Name', p.name], ['Size', size(p.size)], ['Modified', when(p.modified)]]} />
        </div>
      )
    case 'folder':
      return (
        <div className="preview" key={key}>
          <div className="hero" style={{ paddingTop: 0, flexDirection: 'row', textAlign: 'left' }}>
            <div className="big-icon" style={{ width: 44, height: 44 }}>
              <Icon spec={item.icon} fallback="folder" />
            </div>
            <div style={{ minWidth: 0 }}>
              <h3>{p.name}</h3>
              <p>{p.total === 0 ? 'Empty folder' : `${p.total} item${p.total === 1 ? '' : 's'}`}</p>
            </div>
          </div>
          <div className="entries">
            {p.entries.map((e) => (
              <div key={e.name}>
                {e.dir ? <Folder /> : <File />}
                {e.name}
              </div>
            ))}
          </div>
          <Meta rows={[['Where', tilde(p.path)], ['Modified', when(p.modified)]]} />
        </div>
      )
    case 'file':
      return (
        <div className="preview" key={key}>
          <div className="hero">
            <div className="big-icon">
              <Icon spec={p.icon ?? item.icon} fallback="file" />
            </div>
            <h3>{p.name}</h3>
          </div>
          <Meta rows={[['Where', tilde(p.path)], ['Size', size(p.size)], ['Modified', when(p.modified)]]} />
        </div>
      )
    case 'calc':
      return (
        <div className="preview" key={key}>
          <div className="hero" style={{ paddingTop: 34, gap: 14 }}>
            <p style={{ fontFamily: 'var(--font-mono)' }}>{p.expression}</p>
            <div className="calc-result">{p.result}</div>
            <p style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
              <Keys combo="Enter" /> to copy
            </p>
          </div>
          {p.detail && p.detail !== `${p.expression} =` && <p style={{ textAlign: 'center', fontSize: 12, color: 'var(--text-3)', marginTop: 'auto' }}>{p.detail}</p>}
        </div>
      )
  }
}
