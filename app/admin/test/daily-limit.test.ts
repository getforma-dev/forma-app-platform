// D10: every model surface shows the same plain words for 429 `daily_limit`.
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { requestAsk, type RecordScope } from '../src/ask/client.ts';
import { DAILY_LIMIT_MESSAGE } from '../src/canvas/daily-limit.ts';

const app = '00000000-0000-4000-8000-000000001001', work = '00000000-0000-4000-8000-000000001004';
const scope: RecordScope = { kind: 'app', address: 'books', works: [{ app_id: app, work_id: work, address: 'books' }] };
const capped = () => new Response(JSON.stringify({ code: 'daily_limit', status: 429 }), { status: 429, headers: { 'retry-after': '3600' } });
const windowed = () => new Response(JSON.stringify({ code: 'too_many_requests', status: 429 }), { status: 429, headers: { 'retry-after': '120' } });

test('the copy is the plain sentence of the plan', () => {
  assert.equal(DAILY_LIMIT_MESSAGE, "Today's AI limit for this workspace is reached. Building apps and Ask pause until midnight UTC; your apps and entries keep working.");
});

test('compose tells the daily cap apart from the member window', async () => {
  const { composeIntent, messageFor, ComposeError } = await import('../src/canvas/compose.ts');
  const previous = globalThis.fetch;
  try {
    globalThis.fetch = async () => capped();
    const error = await composeIntent('track book loans').catch((caught: unknown) => caught);
    assert.ok(error instanceof ComposeError);
    assert.equal(error.code, 'daily_limit');
    assert.equal(messageFor(error), `${DAILY_LIMIT_MESSAGE} Nothing was placed.`);
    globalThis.fetch = async () => windowed();
    const window = await composeIntent('track book loans').catch((caught: unknown) => caught);
    assert.ok(window instanceof ComposeError);
    assert.equal(window.code, 'too_many');
    assert.ok(messageFor(window).includes('about 2 minutes'));
  } finally { globalThis.fetch = previous; }
});

test('ask, shape and folders show the daily cap sentence', async () => {
  const previous = globalThis.fetch;
  try {
    globalThis.fetch = async () => capped();
    assert.equal(await requestAsk('How many?', scope), DAILY_LIMIT_MESSAGE);
  } finally { globalThis.fetch = previous; }
  const { shapeRefusal } = await import('../src/canvas/shape.ts');
  assert.equal(shapeRefusal('daily_limit', false), DAILY_LIMIT_MESSAGE);
  assert.equal(shapeRefusal('daily_limit', true), DAILY_LIMIT_MESSAGE);
  const { BoxError, refusalText } = await import('../src/canvas/box-api.ts');
  assert.equal(refusalText(new BoxError('daily_limit', 429)), DAILY_LIMIT_MESSAGE);
});
