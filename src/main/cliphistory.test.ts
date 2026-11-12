// Unit tests for the clipboard history logic. Run with `npm test`.
import assert from 'node:assert/strict'
import { test } from 'node:test'
import { acceptable, ClipHistory, clipTitle, MAX_CLIP_BYTES, recencyBoost } from './cliphistory'

const ids = () => {
  let n = 0
  return () => `id${++n}`
}

test('skips empty, whitespace-only and oversized text', () => {
  const h = new ClipHistory(10, ids())
  assert.equal(h.add(''), null)
  assert.equal(h.add('  \n\t '), null)
  assert.equal(h.add('x'.repeat(MAX_CLIP_BYTES + 1)), null)
  assert.ok(h.add('x'.repeat(MAX_CLIP_BYTES)))
  assert.equal(h.size, 1)
  // The limit is in bytes, not characters.
  assert.equal(acceptable('é'.repeat(MAX_CLIP_BYTES / 2 + 1)), false)
})

test('the same text moves to the top instead of duplicating', () => {
  const h = new ClipHistory(10, ids())
  const a = h.add('alpha', 1)!
  h.add('beta', 2)
  const again = h.add('alpha', 3)!
  assert.equal(again.id, a.id)
  assert.equal(again.firstAt, 1)
  assert.equal(again.lastAt, 3)
  assert.deepEqual(h.all().map((e) => e.text), ['alpha', 'beta'])
})

test('caps the history and drops the oldest first', () => {
  const h = new ClipHistory(3, ids())
  for (const [i, t] of ['a', 'b', 'c', 'd', 'e'].entries()) h.add(t, i)
  assert.deepEqual(h.all().map((e) => e.text), ['e', 'd', 'c'])
  h.setMax(2)
  assert.deepEqual(h.all().map((e) => e.text), ['e', 'd'])
})

test('pinned clips are never evicted', () => {
  const h = new ClipHistory(3, ids())
  const old = h.add('old', 0)!
  h.pin(old.id, true)
  for (const [i, t] of ['b', 'c', 'd', 'e'].entries()) h.add(t, i + 1)
  // The cap counts unpinned clips only, so a full pin list never blocks new copies.
  assert.deepEqual(h.all().map((e) => e.text), ['e', 'd', 'c', 'old'])
  const h2 = new ClipHistory(1, ids())
  h2.pin(h2.add('p1', 0)!.id, true)
  h2.pin(h2.add('p2', 1)!.id, true)
  h2.add('loose', 2)
  h2.add('newer', 3)
  assert.deepEqual(h2.all().map((e) => e.text), ['newer', 'p2', 'p1'])
  // Unpinning makes a clip evictable again.
  h2.pin(h2.all()[2].id, false)
  assert.deepEqual(h2.all().map((e) => e.text), ['newer', 'p2'])
})

test('clear keeps pinned clips; remove deletes one', () => {
  const h = new ClipHistory(10, ids())
  const keep = h.add('keep', 0)!
  h.pin(keep.id, true)
  const gone = h.add('gone', 1)!
  h.add('also gone', 2)
  assert.equal(h.remove(gone.id), true)
  assert.equal(h.remove('nope'), false)
  assert.equal(h.clear(), 1)
  assert.deepEqual(h.all().map((e) => e.text), ['keep'])
})

test('recent() lists pinned first, then newest', () => {
  const h = new ClipHistory(10, ids())
  const p = h.add('pinned', 0)!
  h.pin(p.id, true)
  h.add('one', 1)
  h.add('two', 2)
  assert.deepEqual(h.recent(10).map((e) => e.text), ['pinned', 'two', 'one'])
  assert.deepEqual(h.recent(2).map((e) => e.text), ['pinned', 'two'])
})

test('touch moves a clip to the top', () => {
  const h = new ClipHistory(10, ids())
  const a = h.add('a', 0)!
  h.add('b', 1)
  assert.equal(h.touch(a.id, 5), true)
  assert.deepEqual(h.all().map((e) => e.text), ['a', 'b'])
})

test('search ranks better matches first and boosts recent clips', () => {
  const now = 10 * 86_400_000
  const h = new ClipHistory(10, ids())
  h.add('git push origin main', now - 9 * 86_400_000)
  h.add('unrelated note', now - 1000)
  h.add('a long paragraph that mentions git somewhere in the middle', now - 2 * 86_400_000)
  const r = h.search('git', 10, now)
  assert.equal(r[0].entry.text, 'git push origin main')
  assert.deepEqual(r[0].matches, [0, 1, 2])
  assert.ok(!r.some((x) => x.entry.text === 'unrelated note'))

  // Same match quality: the more recent one wins.
  const h2 = new ClipHistory(10, ids())
  h2.add('token abc', now - 30 * 86_400_000)
  h2.add('token xyz', now - 60_000)
  assert.equal(h2.search('token', 10, now)[0].entry.text, 'token xyz')
  assert.ok(recencyBoost(now - 60_000, now) > recencyBoost(now - 30 * 86_400_000, now))
})

test('search also finds text beyond the first line', () => {
  const h = new ClipHistory(10, ids())
  h.add('header line\nsecond line has the needle')
  assert.equal(h.search('needle', 10).length, 1)
})

test('clipTitle uses the first non-blank line, collapsed and shortened', () => {
  assert.equal(clipTitle('\n\n   hello    world  \nsecond'), 'hello world')
  const long = clipTitle('x'.repeat(200))
  assert.equal(long.length, 80)
  assert.ok(long.endsWith('…'))
})

test('load drops bad entries and duplicates, and enforces the cap', () => {
  const h = new ClipHistory(1, ids())
  h.load([
    { id: 'a', text: 'one', firstAt: 1, lastAt: 1, pinned: false },
    { id: 'b', text: 'two', firstAt: 2, lastAt: 2, pinned: true },
    { id: 'c', text: 'one', firstAt: 3, lastAt: 3, pinned: false },
    { id: 'd', text: '   ', lastAt: 4 },
    null,
    { id: '../../etc', text: 'three', lastAt: 5 }
  ])
  assert.deepEqual(h.all().map((e) => e.text), ['three', 'two'])
  assert.notEqual(h.all()[0].id, '../../etc')
  h.load('garbage')
  assert.equal(h.size, 0)
})
