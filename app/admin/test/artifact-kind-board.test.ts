// APP-169 review: the first canvas load after the durable kinds shipped must
// not wipe a member's existing local Card choices (APP-168 "Make a card",
// "Keep data", "Show in Cards"). This drives the real `startBoard` over a
// small fake page: a library row with no server row keeps its local Card and
// uploads it once, an explicit server `app` row drops the local Card, a
// server `card` row is adopted, and a work the page did not list is kept.
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { Document_, install, KEY, page } from './fake-canvas-page.ts';

const APP = '00000000-0000-4000-8000-0000000016a1';
const LOCAL_ONLY = '00000000-0000-4000-8000-0000000016c1';
const SERVER_APP = '00000000-0000-4000-8000-0000000016c2';
const SERVER_CARD = '00000000-0000-4000-8000-0000000016c3';
const BOTH_CARD = '00000000-0000-4000-8000-0000000016c4';
const UNLISTED = '00000000-0000-4000-8000-0000000016c5';
const PLAIN = '00000000-0000-4000-8000-0000000016c6';

test('the first load keeps local Cards, uploads the ones the server lacks once, and drops only an explicit server App (APP-169)', async () => {
  const store = new Map<string, string>([[KEY, JSON.stringify([`${APP}/${LOCAL_ONLY}`, `${APP}/${SERVER_APP}`, `${APP}/${BOTH_CARD}`, `${APP}/${UNLISTED}`])]]);
  const posted: { url: string; body: string }[] = [];
  const document_ = page(APP, true, { [LOCAL_ONLY]: null, [SERVER_APP]: 'app', [SERVER_CARD]: 'card', [BOTH_CARD]: 'card', [PLAIN]: null });
  install(document_, store, async (url: string, init?: { method?: string; body?: string }) => {
    if (String(url).endsWith('/kind') && init?.method === 'POST') {
      posted.push({ url: String(url), body: init.body ?? '' });
      return { ok: true, status: 200, json: async () => JSON.parse(init.body ?? '{}') };
    }
    return { ok: false, status: 404, json: async () => ({}) };
  });
  const board = await import('../src/canvas/board.ts');
  board.startBoard(document_ as unknown as Document);
  await new Promise((resolve) => setImmediate(resolve));

  assert.equal(board.isStandaloneWork(APP, LOCAL_ONLY), true, 'a local Card with no server row is kept');
  assert.equal(board.isStandaloneWork(APP, SERVER_APP), false, 'an explicit server App replaces the local Card');
  assert.equal(board.isStandaloneWork(APP, SERVER_CARD), true, 'a server Card is adopted on a new browser');
  assert.equal(board.isStandaloneWork(APP, BOTH_CARD), true);
  assert.equal(board.isStandaloneWork(APP, UNLISTED), true, 'a work the page did not list is not dropped');
  assert.equal(board.isStandaloneWork(APP, PLAIN), false, 'no row and no local choice stays an App');
  const saved = new Set(JSON.parse(store.get(KEY) ?? '[]') as string[]);
  assert.deepEqual(saved, new Set([`${APP}/${LOCAL_ONLY}`, `${APP}/${SERVER_CARD}`, `${APP}/${BOTH_CARD}`, `${APP}/${UNLISTED}`]));
  assert.deepEqual(posted, [{ url: `/v1/library/${APP}/${LOCAL_ONLY}/kind`, body: JSON.stringify({ kind: 'card' }) }],
    'only the local Card the server has no row for is uploaded, once');
});

/** A `fetch` whose `/kind` answers wait until the test releases them, in order. */
function heldFetch(answer: (kind: string) => number) {
  const posted: string[] = [];
  const waiting: (() => void)[] = [];
  (globalThis as { fetch?: unknown }).fetch = async (_url: string, init?: { body?: string }) => {
    const kind = (JSON.parse(init?.body ?? '{}') as { kind: string }).kind;
    posted.push(kind);
    await new Promise<void>((resolve) => waiting.push(resolve));
    const status = answer(kind);
    return { ok: status < 300, status, json: async () => (status < 300 ? { kind } : { code: 'not_found' }) };
  };
  const release = async () => { waiting.shift()?.(); await new Promise((resolve) => setImmediate(resolve)); };
  return { posted, release, pending: () => waiting.length };
}
const activity = () => (document as unknown as Document_).getElementById('cv-activity')!.textContent;
const stored = () => new Set(JSON.parse((globalThis as unknown as { localStorage: Storage }).localStorage.getItem(KEY) ?? '[]') as string[]);

test('a later click is never overtaken: one write per work at a time, the newest choice last (APP-169)', async () => {
  const board = await import('../src/canvas/board.ts');
  const held = heldFetch(() => 200);
  const first = board.setStandaloneWork(APP, PLAIN, true);
  const second = board.setStandaloneWork(APP, PLAIN, false);
  assert.deepEqual(held.posted, ['card'], 'the second choice waits for the first answer');
  assert.equal(board.isStandaloneWork(APP, PLAIN), false, 'the newest choice shows at once');
  await held.release();
  assert.deepEqual(held.posted, ['card', 'app']);
  await held.release();
  assert.equal(await first, true);
  assert.equal(await second, true);
  assert.equal(board.isStandaloneWork(APP, PLAIN), false);
  assert.equal(stored().has(`${APP}/${PLAIN}`), false);
});

test('a refused save puts the last saved choice back and says so (APP-169)', async () => {
  const board = await import('../src/canvas/board.ts');
  const held = heldFetch(() => 404);
  const refused = board.setStandaloneWork(APP, PLAIN, true);
  assert.equal(board.isStandaloneWork(APP, PLAIN), true, 'shown at once');
  await held.release();
  assert.equal(await refused, false);
  assert.equal(board.isStandaloneWork(APP, PLAIN), false, 'the refused Card is taken back');
  assert.equal(stored().has(`${APP}/${PLAIN}`), false);
  assert.match(activity(), /could not be saved as a Card\. It stays an App/);

  // A saved Card, then a failed App: the Card the server kept is shown again.
  const mixed = heldFetch((kind) => (kind === 'card' ? 200 : 503));
  const saved = board.setStandaloneWork(APP, PLAIN, true);
  const failed = board.setStandaloneWork(APP, PLAIN, false);
  await mixed.release();
  await mixed.release();
  assert.equal(await saved, false, 'the queue ends on the failed write');
  assert.equal(await failed, false);
  assert.deepEqual(mixed.posted, ['card', 'app']);
  assert.equal(board.isStandaloneWork(APP, PLAIN), true);
  assert.equal(stored().has(`${APP}/${PLAIN}`), true);
  assert.match(activity(), /could not be saved as an App\. It stays a Card/);

  // A network failure is a refusal too.
  (globalThis as { fetch?: unknown }).fetch = async () => { throw new TypeError('offline'); };
  assert.equal(await board.setStandaloneWork(APP, PLAIN, false), false);
  assert.equal(board.isStandaloneWork(APP, PLAIN), true);
});
