import assert from 'node:assert/strict';
import { test } from 'node:test';
import { countdownText, matchingBatch, remainingSeconds, tickCountdowns } from '../src/record-components/controller.ts';

const component = { id: 'total', state: 'ready', filters: [], rows: [], columns: [], groups: [], has_receipt: true, message: 'All records' };
test('a dashboard replacement requires the current filter generation and the whole exact component set', () => {
  const ids = new Set(['total', 'trend']);
  const batch = { filter_epoch: 3, components: [component, { ...component, id: 'trend' }] };
  assert.equal(matchingBatch(batch, 3, ids), true);
  assert.equal(matchingBatch(batch, 4, ids), false, 'late older filter result');
  assert.equal(matchingBatch({ ...batch, components: [component] }, 3, ids), false, 'never replace only some linked metrics');
  assert.equal(matchingBatch({ ...batch, components: [component, component] }, 3, ids), false);
  assert.equal(matchingBatch({ ...batch, components: [component, { ...component, id: 'foreign' }] }, 3, ids), false);
});

// ADR-020 C1 (APP-204): the countdown counts from the server's instant plus the
// monotonic time since the response, never from the browser's wall clock.
test('the countdown text is m:ss, h:mm:ss from an hour, and Closed at zero', () => {
  assert.equal(countdownText(25), '0:25');
  assert.equal(countdownText(600), '10:00');
  assert.equal(countdownText(3599), '59:59');
  assert.equal(countdownText(3600), '1:00:00');
  assert.equal(countdownText(0), 'Closed');
  assert.equal(countdownText(-3), 'Closed');
});
test('remaining time is the target minus the server instant and the elapsed monotonic time', () => {
  assert.equal(remainingSeconds('2026-11-29T15:05:30Z', '2026-11-29T15:05:05Z', 0), 25);
  assert.equal(remainingSeconds('2026-11-29T15:05:30Z', '2026-11-29T15:05:05Z', 10_400), 15, 'a partial second still shows');
  assert.equal(remainingSeconds('2026-11-29T15:05:30Z', '2026-11-29T15:05:05Z', 25_000), 0);
  assert.equal(remainingSeconds('2027-01-01T00:00:20Z', '2026-12-31T23:59:50Z', 0), 30, 'across a year');
  assert.equal(remainingSeconds('', '2026-11-29T15:05:05Z', 0), null);
  assert.equal(remainingSeconds('2026-11-29T15:05:30Z', 'not a time', 0), null);
  assert.equal(remainingSeconds('2026-11-29T15:05:30Z', '2026-11-29T15:05:05Z', -1), null);
});
test('a browser clock an hour off changes nothing', () => {
  const realNow = Date.now;
  try {
    const before = remainingSeconds('2026-11-29T15:05:30Z', '2026-11-29T15:05:05Z', 2_000);
    Date.now = () => realNow() + 3_600_000;
    assert.equal(remainingSeconds('2026-11-29T15:05:30Z', '2026-11-29T15:05:05Z', 2_000), before);
    // The ticker reads only the timer's data and the monotonic clock it is given.
    const timer = { dataset: { countdown: 'open', countdownTarget: '2026-11-29T15:05:30Z', serverTime: '2026-11-29T15:05:05Z' } as Record<string, string>, textContent: '0:25' };
    const seen = new WeakMap<object, number>();
    tickCountdowns([timer], seen, 1_000);
    assert.equal(timer.textContent, '0:25', 'first sight anchors at the given instant');
    tickCountdowns([timer], seen, 6_500);
    assert.equal(timer.textContent, '0:20');
    tickCountdowns([timer], seen, 40_000);
    assert.equal(timer.textContent, 'Closed');
    const closed = { dataset: { countdown: 'closed', countdownTarget: '2026-11-29T15:05:30Z', serverTime: '2026-11-29T15:05:05Z' } as Record<string, string>, textContent: 'Closed' };
    tickCountdowns([closed], seen, 2_000);
    assert.equal(closed.textContent, 'Closed', 'a closed countdown never restarts');
  } finally {
    Date.now = realNow;
  }
});
