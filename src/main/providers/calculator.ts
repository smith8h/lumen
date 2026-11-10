// Calculator, unit and currency conversion powered by mathjs.
//   2^10 · sqrt(2) · 15% of 80 · 5 km to mi · 30 c to f · 100 usd to iqd
import { app } from 'electron'
import { readFileSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'
import type { MathJsInstance } from 'mathjs'
import type { ResultItem } from '@shared/types'
import { getSettings } from '../store'

let math: MathJsInstance | null = null
let limitedEvaluate: ((expr: string) => unknown) | null = null

function getMath(): MathJsInstance {
  if (math) return math
  // Loaded on the first calculation, not at startup: mathjs is large (~10 MB of heap).
  // eslint-disable-next-line @typescript-eslint/no-require-imports
  const { all, create } = require('mathjs') as typeof import('mathjs')
  math = create(all, { number: 'number', precision: 64 })
  // Keep a reference first; the import below replaces the public versions (mathjs security guide).
  limitedEvaluate = math.evaluate
  // Lock the parser down: no defining units, importing or meta-evaluation from user input.
  const blocked = (name: string) => () => {
    throw new Error(`${name} is disabled`)
  }
  math.import(
    Object.fromEntries(
      ['import', 'createUnit', 'evaluate', 'parse', 'compile', 'simplify', 'derivative', 'resolve', 'reviver', 'chain', 'help'].map((n) => [n, blocked(n)])
    ),
    { override: true }
  )
  return math
}

function safeEval(expr: string): unknown {
  getMath()
  return limitedEvaluate!(expr)
}

const UNIT_ALIASES: [RegExp, string][] = [
  [/°\s*c\b|\bcelsius\b/gi, 'degC'],
  [/°\s*f\b|\bfahrenheit\b/gi, 'degF'],
  [/\bkelvin\b/gi, 'K'],
  [/\bmph\b/gi, 'mi/h'],
  [/\bkph\b|\bkmh\b|\bkm\/h\b/gi, 'km/h'],
  [/\blbs\b/gi, 'lb'],
  [/\bounces?\b/gi, 'oz']
]

function normalize(input: string): string {
  let s = input.trim()
  s = s.replace(/[×✕]/g, '*').replace(/÷/g, '/').replace(/−/g, '-').replace(/,(?=\d{3}\b)/g, '')
  // 3x4 → 3*4
  s = s.replace(/(\d)\s*x\s*(?=\d)/gi, '$1*')
  // 15% of 80 → 15% * 80
  s = s.replace(/%\s*of\s+/gi, '% * ')
  // "30 c to f" — bare c/f only when converting temperatures.
  s = s.replace(/^([\d.\s+-]+)\s*c\s+(to|in)\s+f$/i, '$1 degC to degF').replace(/^([\d.\s+-]+)\s*f\s+(to|in)\s+c$/i, '$1 degF to degC')
  for (const [re, unit] of UNIT_ALIASES) s = s.replace(re, unit)
  return s
}

function looksLikeMath(q: string): boolean {
  if (!/\d|\b(pi|e|tau|phi|sqrt|sin|cos|tan|log|ln|abs)\b/i.test(q)) return false
  // A plain number or a version like 1.2.3 isn't a calculation.
  if (/^[\d.\s]+$/.test(q) && !/\d\s+\d/.test(q)) return false
  return !/^[a-z]+$/i.test(q)
}

function fmt(value: unknown): string | null {
  const m = getMath()
  if (typeof value === 'number') {
    if (!Number.isFinite(value)) return null
    if (Number.isInteger(value) && Math.abs(value) < 1e21) return value.toLocaleString('en-US')
    const s = m.format(value, { precision: 12, lowerExp: -9, upperExp: 21 })
    const [int, frac] = s.split('.')
    return /e/.test(s) ? s : Number(int).toLocaleString('en-US') + (frac ? '.' + frac : '')
  }
  if (m.isUnit(value) || m.isComplex(value) || m.isBigNumber(value) || m.isFraction(value)) return m.format(value, { precision: 10 })
  if (typeof value === 'string' && /^0[xbo]/.test(value)) return value
  if (m.isMatrix(value) || Array.isArray(value)) return m.format(value, { precision: 8 })
  if (typeof value === 'boolean') return String(value)
  return null
}

const plain = (s: string): string => s.replace(/,/g, '')

function calcItem(expression: string, result: string, subtitle: string, score = 3000): ResultItem {
  return {
    id: `calc:${expression}`,
    kind: 'calc',
    title: result,
    subtitle,
    icon: { type: 'glyph', name: 'calculator' },
    score,
    action: { type: 'copy', text: plain(result) },
    actions: [
      { label: 'Copy result', hint: 'Enter', action: { type: 'copy', text: plain(result) }, glyph: 'copy' },
      { label: 'Copy expression', action: { type: 'copy', text: `${expression} = ${result}` }, glyph: 'copy' }
    ]
  }
}

// ── Currency ───────────────────────────────────────────────────────────────

interface Rates {
  base: string
  rates: Record<string, number>
  fetched: number
}

let rates: Rates | null = null
let ratesPromise: Promise<Rates | null> | null = null
const ratesFile = () => join(app.getPath('userData'), 'rates.json')

async function getRates(): Promise<Rates | null> {
  const fresh = (r: Rates | null) => r && Date.now() - r.fetched < 12 * 3600_000
  if (fresh(rates)) return rates
  if (!rates) {
    try {
      rates = JSON.parse(readFileSync(ratesFile(), 'utf8')) as Rates
      if (fresh(rates)) return rates
    } catch {
      /* no cache yet */
    }
  }
  ratesPromise ??= (async () => {
    try {
      // Free, keyless endpoint. Only the base currency is sent — nothing about the user.
      const res = await fetch('https://open.er-api.com/v6/latest/USD', { signal: AbortSignal.timeout(3500) })
      const json = (await res.json()) as { result?: string; rates?: Record<string, number> }
      if (json.result === 'success' && json.rates) {
        rates = { base: 'USD', rates: json.rates, fetched: Date.now() }
        writeFileSync(ratesFile(), JSON.stringify(rates))
      }
    } catch {
      /* offline — keep stale rates if any */
    } finally {
      ratesPromise = null
    }
    return rates
  })()
  return ratesPromise
}

const CURRENCY_WORDS: Record<string, string> = {
  dollar: 'USD', dollars: 'USD', $: 'USD', euro: 'EUR', euros: 'EUR', '€': 'EUR', pound: 'GBP', pounds: 'GBP', '£': 'GBP',
  yen: 'JPY', '¥': 'JPY', dinar: 'IQD', dinars: 'IQD', riyal: 'SAR', lira: 'TRY', rupee: 'INR', rupees: 'INR', dirham: 'AED'
}

const currencyCode = (s: string): string => CURRENCY_WORDS[s.toLowerCase()] ?? s.toUpperCase()

async function currency(q: string): Promise<ResultItem | null> {
  const m = /^([$€£¥]?)\s*([\d.,]+)\s*([a-z$€£¥]{1,8})?\s+(?:to|in|as|=)\s+([a-z$€£¥]{1,8})$/i.exec(q.trim())
  if (!m) return null
  const from = currencyCode(m[1] || m[3] || '')
  const to = currencyCode(m[4])
  if (!/^[A-Z]{3}$/.test(from) || !/^[A-Z]{3}$/.test(to)) return null
  const r = await getRates()
  if (!r || !r.rates[from] || !r.rates[to]) return null
  const amount = Number(m[2].replace(/,/g, ''))
  if (!Number.isFinite(amount)) return null
  const value = (amount / r.rates[from]) * r.rates[to]
  const digits = value >= 100 ? 2 : value >= 1 ? 4 : 6
  const result = `${value.toLocaleString('en-US', { maximumFractionDigits: digits })} ${to}`
  const when = new Date(r.fetched).toLocaleString([], { dateStyle: 'medium', timeStyle: 'short' })
  return calcItem(q, result, `${amount.toLocaleString('en-US')} ${from} → ${to} · rates from ${when}`)
}

export async function calculate(query: string): Promise<ResultItem[]> {
  const q = query.trim().replace(/^=\s*/, '')
  if (q.length < 2) return []
  if (getSettings().currencyConversion) {
    const c = await currency(q)
    if (c) return [c]
  }
  if (!looksLikeMath(q) && !query.trim().startsWith('=')) return []
  const expr = normalize(q)
  try {
    const value = safeEval(expr)
    if (typeof value === 'function' || value === undefined) return []
    const result = fmt(value)
    if (result === null || plain(result) === q.replace(/\s/g, '')) return []
    return [calcItem(q, result, `${q} =`)]
  } catch {
    return []
  }
}

export function warmCalculator(): void {
  if (getSettings().currencyConversion) void getRates()
}
