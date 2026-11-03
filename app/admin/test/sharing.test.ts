import assert from 'node:assert/strict';
import { test } from 'node:test';
import { canvasPath, createShare, isShareKey, ShareError, shareSummary, sharesPath, type ShareDraft } from '../src/sharing/api.ts';
import { sharedJson } from '../src/shared/document.ts';
import { connectedEntries } from '../src/shared/canvas-controller.ts';

const ORG = 'aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee';
const SHARE = '11111111-2222-4333-8444-555555555555';
const OTHER = '33333333-4444-4555-8666-777777777777';
const KEY = `${ORG}.${SHARE}`;
const summary = { share_id: KEY, version: 2, status: 'active', recipient_email: 'guest@example.test', allow_add_to_canvas: false, expires_at: '2026-11-08T12:00:00Z', href: `/s/${KEY}` };

test('share locators cannot become external URLs, paths or another share response', () => {
  assert.equal(isShareKey(KEY), true);
  for (const key of [SHARE, `${KEY}.extra`, `../${KEY}`, `${ORG}/${SHARE}`, 'https://example.test']) {
    assert.equal(isShareKey(key), false);
    assert.throws(() => canvasPath(key));
  }
  assert.equal(canvasPath(KEY), `/v1/shared/${KEY}/canvas`);
  assert.throws(() => sharesPath(ORG, '../work'));
  assert.ok(shareSummary(summary));
  for (const change of [{ href: 'https://example.test' }, { version: 0 }, { expires_at: 'invalid' }, { allow_add_to_canvas: 'true' }, { share_id: OTHER }]) {
    assert.equal(shareSummary({ ...summary, ...change }), null);
  }
});

test('linked inventory requires explicit canvas capability and rejects duplicate or substituted references', () => {
  const app = { id: KEY, title: 'My app', href: `/s/${KEY}`, owner_label: 'Owner', can_add_to_canvas: true };
  assert.deepEqual(connectedEntries({ apps: [app] }), [app]);
  for (const apps of [[{ ...app, can_add_to_canvas: false }], [{ ...app, href: `/s/${ORG}.${OTHER}` }], [app, app]]) {
    assert.throws(() => connectedEntries({ apps }), /verify/);
  }
});

test('live props bind the exact shared key and retain read-only capability booleans', () => {
  const value = { share_id: KEY, title: 'App', available: true, can_add_to_canvas: false, added_to_canvas: false,
    can_accept: false, needs_sign_in: false, message: '', views: [], record_cards: [] };
  assert.equal(sharedJson(value, KEY).can_add_to_canvas, false);
  assert.throws(() => sharedJson(value, `${ORG}.${OTHER}`));
  assert.throws(() => sharedJson({ ...value, can_add_to_canvas: 'true' }, KEY));
});

test('uncertain creation retries keep the same idempotency body and never claim unverified success', async () => {
  const original = globalThis.fetch;
  const calls: RequestInit[] = [];
  const draft: ShareDraft = { recipient_email: 'guest@example.test', allow_add_to_canvas: false, expires_in_days: 7, idempotency_key: SHARE };
  try {
    globalThis.fetch = async (_url, init) => { calls.push(init!); throw new TypeError('lost response'); };
    await assert.rejects(createShare(sharesPath(ORG, OTHER), draft), error => error instanceof ShareError && error.uncertain);
    globalThis.fetch = async (_url, init) => { calls.push(init!); return new Response(JSON.stringify({ share: summary }), { status: 201 }); };
    assert.equal((await createShare(sharesPath(ORG, OTHER), draft)).share_id, KEY);
    assert.equal(calls[0]?.body, calls[1]?.body);
    assert.equal(calls[1]?.credentials, 'same-origin');
    assert.equal(calls[1]?.cache, 'no-store');
    assert.equal(JSON.parse(calls[1]?.body as string).allow_add_to_canvas, false);
    globalThis.fetch = async () => new Response('{}', { status: 200 });
    await assert.rejects(createShare(sharesPath(ORG, OTHER), draft), error => error instanceof ShareError && error.uncertain);
    globalThis.fetch = async () => new Response('{}', { status: 409 });
    await assert.rejects(createShare(sharesPath(ORG, OTHER), draft), error => error instanceof ShareError && !error.uncertain);
  } finally { globalThis.fetch = original; }
});
