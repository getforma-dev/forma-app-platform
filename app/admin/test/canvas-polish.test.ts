// APP-112 / APP-116 polish: the pure parts (no browser): the plain-words
// router, the timing chip text, the token expiry line and plain form titles.
import assert from 'node:assert/strict';
import { test } from 'node:test';

test('plain sentences reach canvas commands, and only those (APP-112)', async () => {
  const { matchSentence } = await import('../src/canvas/sentences.ts');
  const restOf = (text: string) => {
    const found = matchSentence(text);
    return found ? (found.found[1] ?? '').trim() : null;
  };
  for (const [text, rest] of [
    ['unite the expense app', 'expense'], ['Unite expense', 'expense'], ['break apart the expense app', 'expense'],
    ['take a note: call the auditor Friday', 'call the auditor Friday'], ['show the weather in Austin', 'Austin'],
    ['open the calculator', ''], ['what time is it in Tokyo?', 'Tokyo'], ['new folder Q3 board pack', 'Q3 board pack'],
    ['arrange the canvas', ''], ['open the library', ''], ['show timings', 'show'],
  ] as const) {
    assert.equal(restOf(text), rest, text);
  }
  // Records, decisions and app requests stay on the action path and the composer.
  for (const text of [
    'I spent $86 on a client dinner at Nobu last night, Sarah approves my expenses',
    'approve the Nobu dinner', 'Review expenses over $200', 'build a weather tracker app',
    'I bought a note pad for $4', '/unite expense', 'Book me off Nov 24–26 for Thanksgiving, Sarah approves',
  ]) assert.equal(matchSentence(text), null, text);
});

test('timing chips read as real step times (APP-116)', async () => {
  const { chipText, formatMs, serverSteps } = await import('../src/canvas/timing.ts');
  assert.equal(formatMs(4.2), '4 ms');
  assert.equal(formatMs(812), '812 ms');
  assert.equal(formatMs(5840), '5.8 s');
  assert.equal(chipText([{ label: 'pick', ms: 4 }, { label: 'reshape', ms: 812 }]), 'pick 4 ms · reshape 812 ms');
  // The server's own number is shown when it differs from the page's by network time.
  assert.equal(chipText([{ label: 'pick', ms: 617, server: 556 }]), 'pick 617 ms (server 556 ms)');
  assert.equal(chipText([{ label: 'pick', ms: 558, server: 556 }]), 'pick 556 ms');
  assert.deepEqual(serverSteps({ context_ms: 200, generation_ms: 5800, admission_publish_ms: 50, total_ms: 6100 }).map((s) => s.label), ['context', 'model', 'admitted']);
  assert.equal(chipText(serverSteps({ generation_ms: 0 })), 'model not called (rehearsed)');
});

test('agent tokens say when they expire', async () => {
  const { expiresIn } = await import('../src/canvas/agents.ts');
  const now = Date.parse('2026-10-30T12:00:00Z');
  assert.equal(expiresIn('2026-11-29T12:00:00Z', now), 'expires in 30 days');
  assert.equal(expiresIn('2026-10-31T13:00:00Z', now), 'expires in 1 day');
  assert.equal(expiresIn('2026-10-30T20:00:00Z', now), 'expires today');
  assert.equal(expiresIn('2026-10-29T12:00:00Z', now), 'expired');
  assert.equal(expiresIn(null, now), '');
});

test('form cards name the record in plain words', async () => {
  const { formTitle } = await import('../src/canvas/render.ts');
  assert.equal(formTitle('New leave_request record'), 'New leave request');
  assert.equal(formTitle('Record decision'), 'Record decision');
});
