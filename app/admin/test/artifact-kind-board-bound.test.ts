// APP-169 second review, finding 2: this browser keeps at most 64 Cards. A
// Card the server has not confirmed must never be evicted by one it has: the
// confirmed ones can always be read again from the page. Here 64 local Cards
// have no saved row, one more is a saved Card, and the oldest local upload
// fails. (Its own process: `startBoard` runs once per page.)
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { heldKindFetch, install, KEY, page } from './fake-canvas-page.ts';

const APP = '00000000-0000-4000-8000-0000000016a1';
const id = (n: number) => `00000000-0000-4000-8000-0000000017${n.toString(16).padStart(2, '0')}`;
const LOCAL = Array.from({ length: 64 }, (_, n) => id(n));
const CONFIRMED = '00000000-0000-4000-8000-0000000018ff';

test('a server-confirmed Card never evicts a local Card that is not uploaded yet (APP-169)', async () => {
  const store = new Map<string, string>([[KEY, JSON.stringify(LOCAL.map((work) => `${APP}/${work}`))]]);
  const kinds: Record<string, 'card' | null> = Object.fromEntries(LOCAL.map((work) => [work, null]));
  kinds[CONFIRMED] = 'card';
  const held = heldKindFetch();
  install(page(APP, true, kinds), store, held.fetch_);
  const board = await import('../src/canvas/board.ts');
  board.startBoard(globalThis.document as unknown as Document);
  assert.equal(held.posts.length, 1, 'uploads go one at a time');
  assert.equal(held.posts[0]!.work, LOCAL[0], 'the oldest local Card is uploaded first');
  await held.answer(503);
  assert.equal(held.posts.length, 1, 'a failed upload stops this load; the rest wait for the next');
  const stored = new Set(JSON.parse(store.get(KEY) ?? '[]') as string[]);
  for (const work of LOCAL) assert.ok(stored.has(`${APP}/${work}`), `${work} is still in this browser`);
  assert.equal(stored.size, 64);
  assert.equal(board.isStandaloneWork(APP, CONFIRMED), true, 'the confirmed Card is still shown');
  assert.equal(board.isStandaloneWork(APP, LOCAL[0]!), true, 'the failed upload keeps its Card');
});
