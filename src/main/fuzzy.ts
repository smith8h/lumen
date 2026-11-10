// Small, fast fuzzy scorer tuned for launcher-style matching.
// Prefers, in order: exact > prefix > word-initials ("vsc" → "Visual Studio Code") > word prefix > substring > subsequence.

export interface Match {
  score: number
  indexes: number[]
}

const WORD_BREAK = /[\s\-_.()/[\]:]/

function isWordStart(text: string, i: number): boolean {
  if (i === 0) return true
  const prev = text[i - 1]
  const cur = text[i]
  if (WORD_BREAK.test(prev)) return true
  // camelCase boundary: "gnomeTerminal" → T
  return prev === prev.toLowerCase() && cur !== cur.toLowerCase()
}

function range(start: number, len: number): number[] {
  return Array.from({ length: len }, (_, k) => start + k)
}

/** Score `query` against `text`. Returns null when there is no match. Score is 0..1000. */
export function fuzzyMatch(query: string, text: string): Match | null {
  if (!query) return { score: 0, indexes: [] }
  const q = query.toLowerCase()
  const t = text.toLowerCase()
  if (q.length > t.length) return null

  if (t === q) return { score: 1000, indexes: range(0, q.length) }
  if (t.startsWith(q)) return { score: 900 - Math.min(t.length - q.length, 60), indexes: range(0, q.length) }

  // Word prefix: "code" in "Visual Studio Code".
  const qNoSpace = q.replace(/\s+/g, '')
  for (let i = 1; i < t.length; i++) {
    if (isWordStart(text, i) && t.startsWith(q, i)) {
      return { score: 820 - Math.min(i, 40) - Math.min(t.length - q.length, 40) / 2, indexes: range(i, q.length) }
    }
  }

  // Word initials: "vsc", "lo w" → LibreOffice Writer.
  const initials = initialsMatch(qNoSpace, text, t)
  if (initials) return { score: 780 - Math.min(t.length, 60) / 3, indexes: initials }

  const sub = t.indexOf(q)
  if (sub >= 0) return { score: 650 - Math.min(sub, 50) - Math.min(t.length - q.length, 50) / 2, indexes: range(sub, q.length) }

  return subsequence(qNoSpace, text, t)
}

function initialsMatch(q: string, text: string, t: string): number[] | null {
  const idx: number[] = []
  let qi = 0
  for (let i = 0; i < t.length && qi < q.length; i++) {
    if (isWordStart(text, i) && t[i] === q[qi]) {
      idx.push(i)
      qi++
    }
  }
  return qi === q.length && q.length >= 2 ? idx : null
}

function subsequence(q: string, text: string, t: string): Match | null {
  if (q.length < 2) return null
  const idx: number[] = []
  let qi = 0
  let score = 0
  let run = 0
  let last = -2
  for (let i = 0; i < t.length && qi < q.length; i++) {
    if (t[i] !== q[qi]) continue
    idx.push(i)
    run = i === last + 1 ? run + 1 : 1
    score += 10 + run * 6 + (isWordStart(text, i) ? 14 : 0)
    last = i
    qi++
  }
  if (qi < q.length) return null
  const spread = idx[idx.length - 1] - idx[0] + 1
  const density = q.length / spread
  // Very scattered matches are noise.
  if (density < 0.25 && q.length < 5) return null
  return { score: Math.min(560, 200 + score * density - Math.min(idx[0], 30)), indexes: idx }
}

/** Best match of the query against several fields; the first field is the title used for highlights. */
export function matchFields(query: string, title: string, extra: (string | undefined)[], extraWeight = 0.75): Match | null {
  const main = fuzzyMatch(query, title)
  let best: Match | null = main
  for (const field of extra) {
    if (!field) continue
    const m = fuzzyMatch(query, field)
    if (m && m.score * extraWeight > (best?.score ?? 0)) best = { score: m.score * extraWeight, indexes: main?.indexes ?? [] }
  }
  return best
}
