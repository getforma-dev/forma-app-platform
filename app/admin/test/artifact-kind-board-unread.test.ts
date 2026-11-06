// APP-169 review: when the canvas could not read the member's saved Card or
// App choices, the page carries no `data-artifact-kinds` signal. The board
// then keeps this browser's choices exactly and sends nothing, even where a
// row says `app` or carries no saved choice. (Its own process: `startBoard`
// runs once per page.)
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { install, KEY, page } from './fake-canvas-page.ts';

const APP = '00000000-0000-4000-8000-0000000016a1';
const LOCAL = '00000000-0000-4000-8000-0000000016d1';
const SAYS_APP = '00000000-0000-4000-8000-0000000016d2';
const SAYS_CARD = '00000000-0000-4000-8000-0000000016d3';

test('without the page signal the local Cards stay and nothing is uploaded (APP-169)', async () => {
  const local = [`${APP}/${LOCAL}`, `${APP}/${SAYS_APP}`];
  const store = new Map<string, string>([[KEY, JSON.stringify(local)]]);
  const posted: string[] = [];
  const document_ = page(APP, false, { [LOCAL]: null, [SAYS_APP]: 'app', [SAYS_CARD]: 'card' });
  install(document_, store, async (url: string, init?: { method?: string }) => {
    if (init?.method === 'POST') posted.push(String(url));
    return { ok: false, status: 404, json: async () => ({}) };
  });
  const board = await import('../src/canvas/board.ts');
  board.startBoard(document_ as unknown as Document);
  await new Promise((resolve) => setImmediate(resolve));
  assert.equal(board.isStandaloneWork(APP, LOCAL), true);
  assert.equal(board.isStandaloneWork(APP, SAYS_APP), true, 'an unread page cannot take a Card away');
  assert.equal(board.isStandaloneWork(APP, SAYS_CARD), false, 'an unread page adds nothing either');
  assert.deepEqual(new Set(JSON.parse(store.get(KEY) ?? '[]') as string[]), new Set(local));
  assert.deepEqual(posted, []);
});
