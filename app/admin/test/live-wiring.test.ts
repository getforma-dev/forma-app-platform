// APP-195 review m4 and m6: the browser wiring of the live-refresh timing.
// `browserTiming` reads the real page's visibility and listens for the real
// `visibilitychange` event; the canvas board sends a live hint through the
// 0–3 s schedule (and defers it on a hidden page); the work document treats
// coming back `online` like a hint. Node's mock timers and a stubbed
// Math.random make every delay exact; the page is an EventTarget.
import { test, mock } from 'node:test';
import assert from 'node:assert/strict';

const APP = '00000000-0000-4000-8000-000000000001';
const WORK = '00000000-0000-4000-8000-000000000002';

type FakePage = EventTarget & { visibilityState: 'visible' | 'hidden' };
function fakePage(visibilityState: 'visible' | 'hidden' = 'visible'): FakePage {
  return Object.assign(new EventTarget(), { visibilityState });
}
function show(page: FakePage) {
  page.visibilityState = 'visible';
  page.dispatchEvent(new Event('visibilitychange'));
}

test('browserTiming reads the page visibility and listens for visibilitychange', async () => {
  const page = fakePage('hidden');
  const before = (globalThis as { document?: unknown }).document;
  (globalThis as { document?: unknown }).document = page;
  try {
    const { browserTiming } = await import('../src/canvas/live.ts');
    assert.equal(browserTiming.hidden?.(), true);
    page.visibilityState = 'visible';
    assert.equal(browserTiming.hidden?.(), false);
    let calls = 0;
    const stop = browserTiming.onVisible!(() => { calls += 1; });
    page.visibilityState = 'hidden';
    page.dispatchEvent(new Event('visibilitychange'));
    assert.equal(calls, 0, 'not while hidden');
    show(page);
    assert.equal(calls, 1, 'once when shown');
    page.dispatchEvent(new Event('visibilitychanged'));
    assert.equal(calls, 1, 'only the real event name');
    stop();
    show(page);
    assert.equal(calls, 1, 'not after unsubscribing');
  } finally {
    (globalThis as { document?: unknown }).document = before;
  }
});

test('the board sends a live hint through the 0–3 s schedule, and a hidden page reads once when shown', async () => {
  const page = fakePage();
  Object.assign(globalThis, { document: page });
  mock.timers.enable({ apis: ['setTimeout'] });
  const random = Math.random;
  Math.random = () => 0.5;
  try {
    const board = await import('../src/canvas/board.ts');
    const seen: string[] = [];
    board.onLiveEvent((event) => seen.push(event.kind));
    const committed = { kind: 'record.committed' as const, app_id: APP, work_id: WORK };
    board.onLive(committed);
    board.onLive(committed);
    assert.deepEqual(seen, [], 'never at once');
    mock.timers.tick(1499);
    assert.deepEqual(seen, [], 'not before the drawn delay');
    mock.timers.tick(1);
    assert.deepEqual(seen, ['record.committed'], 'after 1.5 s, once: the second hint joined the first');
    // A lagged stream re-reads after its own wait too.
    board.onLive({ kind: 'stream.lagged' });
    assert.deepEqual(seen, ['record.committed']);
    mock.timers.tick(1500);
    assert.deepEqual(seen, ['record.committed', 'stream.lagged']);
    // A hidden page schedules nothing and reads once when it is shown.
    page.visibilityState = 'hidden';
    board.onLive(committed);
    board.onLive(committed);
    mock.timers.tick(10000);
    assert.equal(seen.length, 2, 'nothing while hidden');
    show(page);
    assert.deepEqual(seen, ['record.committed', 'stream.lagged', 'record.committed'], 'once, at once, when shown');
    show(page);
    mock.timers.tick(10000);
    assert.equal(seen.length, 3, 'and only once');
  } finally {
    Math.random = random;
    mock.timers.reset();
  }
});

test('the work document: shown again and Refresh read at once; coming back online waits like a hint', async () => {
  const { listenForRefreshes } = await import('../src/work/document.ts');
  const page = fakePage();
  const network = new EventTarget();
  const calls: string[] = [];
  const refresher = {
    shown: () => calls.push('shown'),
    member: () => calls.push('member'),
    hint: () => calls.push('hint'),
  };
  const lifecycle = new AbortController();
  const check = listenForRefreshes(page, network, refresher, lifecycle.signal);
  network.dispatchEvent(new Event('online'));
  assert.deepEqual(calls, ['hint'], 'online: like a live hint (0–3 s, one retry)');
  page.dispatchEvent(new Event('forma:document-refresh'));
  assert.deepEqual(calls, ['hint', 'member'], 'the Refresh button: the member read, at once');
  show(page);
  assert.deepEqual(calls, ['hint', 'member', 'shown'], 'shown again: the page refresh, which covers a stale hint');
  page.visibilityState = 'hidden';
  page.dispatchEvent(new Event('visibilitychange'));
  check();
  assert.equal(calls.length, 3, 'nothing while hidden, including the 30 s check');
  page.visibilityState = 'visible';
  check();
  assert.deepEqual(calls.at(-1), 'shown', 'the 30 s check while visible');
  lifecycle.abort();
  network.dispatchEvent(new Event('online'));
  page.dispatchEvent(new Event('forma:document-refresh'));
  show(page);
  assert.equal(calls.length, 4, 'nothing after the page stopped');
});
