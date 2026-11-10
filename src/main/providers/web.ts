// URLs open directly in the default browser; anything else can be searched on the web.
import type { ResultItem } from '@shared/types'
import { SEARCH_ENGINES, WEB_KEYWORDS } from '@shared/constants'
import { getSettings } from '../store'

const TLD = /\.(com|org|net|io|dev|app|ai|co|me|edu|gov|info|xyz|tech|sh|gg|tv|so|ly|to|fm|uk|de|fr|iq|sa|ae|eg|tr|ru|in|jp|cn|br|ca|au|nl|it|es|ch|se|no|pl|us|eu|page|site|online|store|blog|cloud|design|news|wiki|[a-z]{2})(:\d+)?(\/|$|\?|#)/i

/** Returns a normalized URL if the query is clearly a web address. */
export function asUrl(query: string): string | null {
  const q = query.trim()
  if (!q || /\s/.test(q)) return null
  if (/^(https?|ftp):\/\/\S+$/i.test(q)) return q
  if (/^(localhost|127\.0\.0\.1|0\.0\.0\.0|\[::1\])(:\d+)?(\/\S*)?$/i.test(q)) return `http://${q}`
  if (/^\d{1,3}(\.\d{1,3}){3}(:\d+)?(\/\S*)?$/.test(q)) return `http://${q}`
  if (/^[\w-]+(\.[\w-]+)+(:\d+)?(\/\S*)?$/i.test(q) && TLD.test(q + '/') && !/^\d+(\.\d+)+$/.test(q)) return `https://${q}`
  return null
}

function engineUrl(): { name: string; url: string } {
  const s = getSettings()
  if (s.searchEngine === 'custom' && s.customSearchUrl.includes('%s')) {
    let host = 'the web'
    try {
      host = new URL(s.customSearchUrl.replace('%s', 'x')).hostname.replace(/^www\./, '')
    } catch {
      /* keep generic name */
    }
    return { name: host, url: s.customSearchUrl }
  }
  return SEARCH_ENGINES[s.searchEngine === 'custom' ? 'google' : s.searchEngine]
}

const fill = (template: string, q: string): string => template.replace('%s', encodeURIComponent(q))

export function webResults(query: string): ResultItem[] {
  const q = query.trim()
  if (!q) return []
  const out: ResultItem[] = []

  const url = asUrl(q)
  if (url) {
    out.push({
      id: `url:${url}`,
      kind: 'url',
      title: q.replace(/^https?:\/\//, ''),
      subtitle: 'Open in your browser',
      icon: { type: 'glyph', name: 'globe' },
      score: 2600,
      historyKey: `url:${url}`,
      action: { type: 'open-url', url },
      actions: [{ label: 'Copy link', action: { type: 'copy', text: url }, glyph: 'copy' }]
    })
  }

  // Keyword searches: "yt lofi", "gh electron", "wiki baghdad".
  const space = q.indexOf(' ')
  if (space > 0) {
    const kw = WEB_KEYWORDS[q.slice(0, space).toLowerCase()]
    const rest = q.slice(space + 1).trim()
    if (kw && rest) {
      out.push({
        id: `kw:${kw.name}:${rest}`,
        kind: 'web',
        title: `Search ${kw.name} for “${rest}”`,
        subtitle: new URL(kw.url.replace('%s', '')).hostname.replace(/^www\./, ''),
        icon: { type: 'glyph', name: 'search' },
        score: 2500,
        action: { type: 'open-url', url: fill(kw.url, rest) }
      })
    }
  }

  const engine = engineUrl()
  out.push({
    id: `web:${q}`,
    kind: 'web',
    title: `Search ${engine.name} for “${q}”`,
    subtitle: 'Opens in your default browser',
    icon: { type: 'glyph', name: 'search' },
    score: url ? 400 : 120,
    action: { type: 'open-url', url: fill(engine.url, q) },
    actions: Object.entries(WEB_KEYWORDS)
      .filter(([k]) => ['yt', 'gh', 'wiki', 'maps'].includes(k))
      .map(([, v]) => ({ label: `Search ${v.name}`, action: { type: 'open-url', url: fill(v.url, q) }, glyph: 'globe' as const }))
  })
  return out
}
