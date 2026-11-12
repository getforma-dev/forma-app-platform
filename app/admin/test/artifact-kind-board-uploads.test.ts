// APP-169 second review, finding 4: the first-load upload of local Cards goes
// one at a time, at most 16 per load, and only where nothing is saved
// (`if_absent`). A choice saved on another device after the page rendered
// wins over the upload. (Its own process: `startBoard` runs once per page.)
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { heldKindFetch, install, KEY, page } from './fake-canvas-page.ts';

const APP = '00000000-0000-4000-8000-0000000016a1';
const id = (n: number) => `00000000-0000-4000-8000-0000000019${n.toString(16).padStart(2, '0')}`;
const LOCAL = Array.from({ length: 20 }, (_, n) => id(n));

test('local Cards upload one at a time, at most 16 per load, and never replace a saved choice (APP-169)', async () => {
  const store = new Map<string, string>([[KEY, JSON.stringify(LOCAL.map((work) => `${APP}/${work}`))]]);
  const held = heldKindFetch();
  install(page(APP, true, Object.fromEntries(LOCAL.map((work) => [work, null]))), store, held.fetch_);
  const board = await import('../src/canvas/board.ts');
  board.startBoard(globalThis.document as unknown as Document);
  for (let n = 0; held.pending.length; n += 1) {
    // Another device saved LOCAL[2] as an App after this page rendered.
    await held.answer(200, held.pending[0]!.work === LOCAL[2] ? 'app' : undefined);
    assert.ok(n < 40, 'the uploads end');
  }
  assert.equal(held.posts.length, 16, 'at most 16 uploads per load');
  assert.equal(held.maxInFlight(), 1, 'one upload at a time');
  assert.deepEqual(held.posts.map((post) => post.work), LOCAL.slice(0, 16));
  for (const post of held.posts) assert.deepEqual([post.kind, post.ifAbsent], ['card', true]);
  assert.equal(board.isStandaloneWork(APP, LOCAL[2]!), false, 'the saved App wins over the upload');
  for (const work of LOCAL.filter((work) => work !== LOCAL[2])) assert.equal(board.isStandaloneWork(APP, work), true, work);
  const stored = new Set(JSON.parse(store.get(KEY) ?? '[]') as string[]);
  assert.equal(stored.has(`${APP}/${LOCAL[2]}`), false);
  for (const work of LOCAL.slice(16)) assert.ok(stored.has(`${APP}/${work}`), 'not uploaded yet: kept for the next load');
});
