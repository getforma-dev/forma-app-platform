// APP-195: a refresh that a live hint triggers starts after a uniform random
// delay of 0–3 s; the page that committed reads at once; a read that fails
// with a 503 or a network error is tried once more after 1–3 s before the page
// shows it failed; refreshes the member starts are unchanged; a hidden page
// schedules nothing and reads once when it is shown. The clock, the random
// source and the page's visibility are fakes, so every delay here is exact.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { hintDelay, hintSchedule, retryDelay, retryOnce, type Fetch, type Timing } from '../src/canvas/live.ts';
import { documentRead, documentRefresher, type RefreshMode } from '../src/work/document.ts';

const flush = () => new Promise<void>((resolve) => setImmediate(resolve));

/** A clock that moves only when the test advances it, and a scripted random source. */
function fakeTiming(randoms: number[] = []) {
  let now = 0;
  let next = 1;
  const timers = new Map<number, { at: number; run: () => void }>();
  const draws: number[] = [];
  let hidden = false;
  const shown = new Set<() => void>();
  const timing: Timing = {
    random: () => {
      const value = randoms.shift();
      if (value === undefined) throw new Error('an unexpected random draw');
      draws.push(value);
      return value;
    },
    setTimeout: (run, ms) => {
      const id = next++;
      timers.set(id, { at: now + ms, run });
      return id;
    },
    clearTimeout: (id) => { timers.delete(id as number); },
    hidden: () => hidden,
    onVisible: (listener) => {
      shown.add(listener);
      return () => { shown.delete(listener); };
    },
  };
  /** The page is hidden, or shown again (which calls the visibility listeners). */
  const hide = () => { hidden = true; };
  const show = () => {
    hidden = false;
    for (const listener of [...shown]) listener();
  };
  /** Runs every timer due within `ms`, in order, letting promises settle between them. */
  const advance = async (ms: number) => {
    const until = now + ms;
    await flush();
    for (;;) {
      const due = [...timers.entries()].filter(([, timer]) => timer.at <= until).sort((a, b) => a[1].at - b[1].at)[0];
      if (!due) break;
      timers.delete(due[0]);
      now = due[1].at;
      due[1].run();
      await flush();
    }
    now = until;
    await flush();
  };
  return { timing, advance, timers, draws, now: () => now, hide, show, listeners: () => shown.size };
}

/** A deterministic uniform source (a 32-bit LCG). */
function seeded(seed: number): () => number {
  let state = seed >>> 0;
  return () => {
    state = (Math.imul(state, 1664525) + 1013904223) >>> 0;
    return state / 2 ** 32;
  };
}

const answer = (status: number) => new Response(status === 204 ? null : '{}', { status });

/** A scripted fetch: each call takes the next step (a status, or an error to throw). */
function scripted(steps: (number | Error)[]) {
  const calls: number[] = [];
  let clock = () => 0;
  const request: Fetch = async (_input, init) => {
    calls.push(clock());
    if (init?.signal?.aborted) throw init.signal.reason;
    const step = steps.shift();
    if (step === undefined) throw new Error('an unexpected request');
    if (step instanceof Error) throw step;
    return answer(step);
  };
  return { request, calls, at: (now: () => number) => { clock = now; } };
}

test('a hinted refresh starts after a uniform random delay of 0–3 s', async () => {
  assert.equal(hintDelay({ ...fakeTiming([0]).timing }), 0);
  assert.equal(hintDelay(fakeTiming([0.5]).timing), 1500);
  assert.equal(hintDelay(fakeTiming([0.99999]).timing), 2999);
  // A source outside [0, 1) still gives a delay inside the window.
  assert.equal(hintDelay(fakeTiming([1]).timing), 2999);
  assert.equal(hintDelay(fakeTiming([-1]).timing), 0);
  assert.equal(hintDelay(fakeTiming([Number.NaN]).timing), 0);
  const random = seeded(195);
  const delays = Array.from({ length: 2000 }, () => hintDelay({ ...fakeTiming().timing, random }));
  assert.ok(delays.every((delay) => Number.isInteger(delay) && delay >= 0 && delay < 3000));
  assert.ok(Math.min(...delays) < 50 && Math.max(...delays) > 2950, 'the whole window is used');
  const thirds = [0, 1, 2].map((third) => delays.filter((delay) => Math.floor(delay / 1000) === third).length);
  assert.ok(thirds.every((count) => count > 600 && count < 730), `uniform: ${thirds}`);

  const clock = fakeTiming([0.5, 0.25]);
  const hints = hintSchedule(clock.timing);
  const runs: number[] = [];
  hints.hint('work-a', () => runs.push(clock.now()));
  assert.deepEqual(runs, [], 'never at once');
  // Hints for the same key while one waits join it: one read, one draw.
  hints.hint('work-a', () => runs.push(-1));
  await clock.advance(1499);
  assert.deepEqual(runs, []);
  await clock.advance(1);
  assert.deepEqual(runs, [1500]);
  assert.deepEqual(clock.draws, [0.5]);
  // After it ran, the next hint waits again.
  hints.hint('work-a', () => runs.push(clock.now()));
  await clock.advance(3000);
  assert.deepEqual(runs, [1500, 1500 + 750]);
  assert.equal(hints.waiting('work-a'), false);

  const two = fakeTiming([0.1, 0.9]);
  const each = hintSchedule(two.timing);
  const order: string[] = [];
  each.hint('work-a', () => order.push('a'));
  each.hint('work-b', () => order.push('b'));
  await two.advance(3000);
  assert.deepEqual(order, ['a', 'b'], 'each key waits on its own');
});

test('the page that committed reads at once, and its read replaces a waiting hint', async () => {
  const clock = fakeTiming([0.8]);
  const hints = hintSchedule(clock.timing);
  const runs: string[] = [];
  hints.hint('document', () => runs.push('hint'));
  hints.now('document', () => runs.push('committed'));
  assert.deepEqual(runs, ['committed'], 'no delay');
  await clock.advance(5000);
  assert.deepEqual(runs, ['committed'], 'a read that started after the hint covers it');
  assert.deepEqual(clock.draws, [0.8], 'the committed read draws no delay');

  const page = fakeTiming([0.6]);
  const modes: [RefreshMode, number][] = [];
  const refresher = documentRefresher((mode) => modes.push([mode, page.now()]), page.timing);
  refresher.hint();
  refresher.committed();
  await page.advance(5000);
  assert.deepEqual(modes, [['live', 0]]);
  refresher.dispose();
});

test('a failed read is tried once more after a random 1–3 s, and the second answer is final', async () => {
  assert.equal(retryDelay(fakeTiming([0]).timing), 1000);
  assert.equal(retryDelay(fakeTiming([0.5]).timing), 2000);
  assert.equal(retryDelay(fakeTiming([0.99999]).timing), 2999);
  assert.equal(retryDelay(fakeTiming([1]).timing), 2999);

  // 503, then 200: the page sees the 200, 2 s later; nothing in between.
  {
    const clock = fakeTiming([0.5]);
    const server = scripted([503, 200]);
    server.at(clock.now);
    let status = 0;
    void retryOnce(clock.timing, server.request)('/w/a/b').then((response) => { status = response.status; });
    await clock.advance(1999);
    assert.equal(status, 0, 'the first failure is not shown');
    await clock.advance(1);
    assert.equal(status, 200);
    assert.deepEqual(server.calls, [0, 2000]);
  }
  // Two 503s: the second is the answer; there is no third read.
  {
    const clock = fakeTiming([0]);
    const server = scripted([503, 503]);
    let status = 0;
    void retryOnce(clock.timing, server.request)('/w/a/b').then((response) => { status = response.status; });
    await clock.advance(10000);
    assert.equal(status, 503);
    assert.equal(server.calls.length, 2);
  }
  // A network error is retried the same way; a second one is the page's.
  {
    const clock = fakeTiming([0.25, 0.75]);
    const once = scripted([new TypeError('network'), 200]);
    let status = 0;
    void retryOnce(clock.timing, once.request)('/v1/x').then((response) => { status = response.status; });
    await clock.advance(1500);
    assert.equal(status, 200);
    const twice = scripted([new TypeError('first'), new TypeError('second')]);
    let failure: unknown = null;
    void retryOnce(clock.timing, twice.request)('/v1/x').catch((error) => { failure = error; });
    await clock.advance(3000);
    assert.equal((failure as Error | null)?.message, 'second');
    assert.equal(twice.calls.length, 2);
  }
  // Only a 503 or a network error is retried, and an aborted read never is.
  for (const status of [200, 204, 404, 409, 429, 500]) {
    const clock = fakeTiming();
    const server = scripted([status]);
    let seen = 0;
    void retryOnce(clock.timing, server.request)('/v1/x').then((response) => { seen = response.status; });
    await clock.advance(5000);
    assert.equal(seen, status);
    assert.equal(server.calls.length, 1, `no retry after ${status}`);
    assert.deepEqual(clock.draws, [], 'no delay drawn');
  }
  {
    const clock = fakeTiming([0.5]);
    const server = scripted([503]);
    const stop = new AbortController();
    let failure: unknown = null;
    void retryOnce(clock.timing, server.request)('/v1/x', { signal: stop.signal }).catch((error) => { failure = error; });
    await clock.advance(500);
    stop.abort(new Error('page left'));
    await clock.advance(5000);
    assert.equal((failure as Error | null)?.message, 'page left');
    assert.equal(server.calls.length, 1, 'no read after the page stopped');
    assert.equal(clock.timers.size, 0, 'the wait is cancelled');
  }
});

test('refreshes the member starts are unchanged: at once, the plain read, no retry', async () => {
  const page = fakeTiming([0.9]);
  const modes: [RefreshMode, number][] = [];
  const refresher = documentRefresher((mode) => modes.push([mode, page.now()]), page.timing);
  refresher.member();
  assert.deepEqual(modes, [['member', 0]], 'no delay');
  assert.deepEqual(page.draws, [], 'no delay drawn');
  // A member's refresh leaves a waiting hint as it was.
  refresher.hint();
  refresher.member();
  await page.advance(3000);
  assert.deepEqual(modes, [['member', 0], ['member', 0], ['live', 2700]]);
  refresher.dispose();

  const clock = fakeTiming();
  const server = scripted([503]);
  const plain = documentRead('member', clock.timing, server.request);
  assert.equal(plain, server.request, 'the member reads with the page fetch itself');
  assert.equal((await plain('/w/a/b')).status, 503);
  assert.equal(server.calls.length, 1);
  assert.deepEqual(clock.draws, []);
  const hinted = scripted([503, 200]);
  const live = fakeTiming([0]);
  let status = 0;
  void documentRead('live', live.timing, hinted.request)('/w/a/b').then((response) => { status = response.status; });
  await live.advance(1000);
  assert.equal(status, 200, 'a hinted or committed read retries');
});

test('a disposed page runs no waiting hint', async () => {
  const page = fakeTiming([0.1]);
  const modes: RefreshMode[] = [];
  const refresher = documentRefresher((mode) => modes.push(mode), page.timing);
  refresher.hint();
  refresher.dispose();
  refresher.hint();
  refresher.committed();
  await page.advance(5000);
  assert.deepEqual(modes, []);
  assert.equal(page.timers.size, 0);
});

test('a hidden page schedules nothing for a hint and reads once when it is shown', async () => {
  // The canvas: no refresh of its own on visibility, so the schedule resumes.
  const canvas = fakeTiming([0.5]);
  const hints = hintSchedule(canvas.timing, { resumeWhenShown: true });
  const runs: string[] = [];
  canvas.hide();
  hints.hint('work-a', () => runs.push('a'));
  hints.hint('work-a', () => runs.push('a again'));
  hints.hint('work-b', () => runs.push('b'));
  await canvas.advance(10000);
  assert.deepEqual(runs, [], 'nothing is read while hidden');
  assert.equal(canvas.timers.size, 0, 'nothing is scheduled');
  assert.deepEqual(canvas.draws, [], 'no delay drawn');
  assert.equal(hints.stale('work-a'), true);
  canvas.show();
  assert.deepEqual(runs, ['a again', 'b'], 'each stale key reads once, at once');
  assert.equal(hints.stale('work-a'), false);
  assert.equal(canvas.listeners(), 0, 'the visibility listener is released');
  canvas.show();
  await canvas.advance(10000);
  assert.deepEqual(runs, ['a again', 'b'], 'once only');
  // Shown, a hint waits 0–3 s again.
  hints.hint('work-a', () => runs.push('visible'));
  await canvas.advance(1499);
  assert.deepEqual(runs, ['a again', 'b']);
  await canvas.advance(1);
  assert.deepEqual(runs, ['a again', 'b', 'visible']);

  // The work document: its own visibility refresh covers the stale hint.
  const page = fakeTiming();
  const modes: [RefreshMode, number][] = [];
  const refresher = documentRefresher((mode) => modes.push([mode, page.now()]), page.timing);
  page.hide();
  refresher.hint();
  refresher.hint();
  await page.advance(10000);
  assert.deepEqual(modes, [], 'a hidden page schedules nothing');
  assert.equal(page.timers.size, 0);
  assert.deepEqual(page.draws, []);
  assert.equal(refresher.stale(), true, 'it is marked stale');
  assert.equal(page.listeners(), 0, 'the document does not subscribe: its own handler reads');
  page.show();
  refresher.shown();
  assert.deepEqual(modes, [['member', 10000]], 'one read on becoming visible');
  assert.equal(refresher.stale(), false);
  await page.advance(10000);
  assert.deepEqual(modes, [['member', 10000]], 'and no other');
  refresher.dispose();
});

test('the committer on a visible page is unchanged by the visibility rule', async () => {
  const page = fakeTiming([0.6]);
  const modes: [RefreshMode, number][] = [];
  const refresher = documentRefresher((mode) => modes.push([mode, page.now()]), page.timing);
  refresher.committed();
  assert.deepEqual(modes, [['live', 0]], 'at once');
  assert.deepEqual(page.draws, [], 'no delay drawn');
  refresher.hint();
  refresher.committed();
  await page.advance(5000);
  assert.deepEqual(modes, [['live', 0], ['live', 0]], 'and it still replaces a waiting hint');
  assert.equal(refresher.stale(), false);
  refresher.dispose();
});
