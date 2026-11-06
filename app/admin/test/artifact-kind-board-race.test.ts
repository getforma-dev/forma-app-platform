// APP-169 second review, finding 3: when the first-load upload of a local
// Card fails while the member has since chosen App, the board still sends
// the member's newest choice, and any message names that choice. (Its own
// process: `startBoard` runs once per page.)
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { Document_, heldKindFetch, install, KEY, page } from './fake-canvas-page.ts';

const APP = '00000000-0000-4000-8000-0000000016a1';
const FIRST = '00000000-0000-4000-8000-000000001a01';
const SECOND = '00000000-0000-4000-8000-000000001a02';

test('a failed upload never discards the member\'s newer choice (APP-169)', async () => {
  const store = new Map<string, string>([[KEY, JSON.stringify([`${APP}/${FIRST}`, `${APP}/${SECOND}`])]]);
  const held = heldKindFetch();
  install(page(APP, true, { [FIRST]: null, [SECOND]: null }), store, held.fetch_);
  const board = await import('../src/canvas/board.ts');
  board.startBoard(globalThis.document as unknown as Document);
  const activity = (globalThis.document as unknown as Document_).getElementById('cv-activity')!;
  assert.deepEqual(held.posts.map((post) => [post.work, post.kind, post.ifAbsent]), [[FIRST, 'card', true]]);

  // The member shows FIRST in Apps while its upload is in flight; the upload fails.
  const first = board.setStandaloneWork(APP, FIRST, false);
  assert.equal(held.posts.length, 1, 'the newer choice waits for the upload');
  await held.answer(503);
  assert.deepEqual(held.posts.map((post) => [post.work, post.kind, post.ifAbsent]).at(-1), [FIRST, 'app', false], 'the newer choice is still sent');
  await held.answer(200);
  assert.equal(await first, true);
  assert.equal(board.isStandaloneWork(APP, FIRST), false);
  assert.doesNotMatch(activity.textContent, /could not be saved/);

  // Same for SECOND, but the App save fails too: the message names the App choice.
  assert.deepEqual(held.posts.map((post) => [post.work, post.kind, post.ifAbsent]).at(-1), [SECOND, 'card', true]);
  const second = board.setStandaloneWork(APP, SECOND, false);
  await held.answer(503);
  assert.deepEqual(held.posts.map((post) => [post.work, post.kind]).at(-1), [SECOND, 'app']);
  await held.answer(503);
  assert.equal(await second, false);
  assert.equal(board.isStandaloneWork(APP, SECOND), true, 'nothing was saved: the local Card is shown again');
  assert.match(activity.textContent, /could not be saved as an App\. It stays a Card/);
  assert.ok(new Set(JSON.parse(store.get(KEY) ?? '[]') as string[]).has(`${APP}/${SECOND}`));
});
