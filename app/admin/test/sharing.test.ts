import assert from 'node:assert/strict';
import { test } from 'node:test';
import { canvasPath, createShare, isShareKey, ShareError, shareSummary, sharesPath, type ShareDraft } from '../src/sharing/api.ts';
import { readFileSync } from 'node:fs';
import { DEMO_NOTE_TEXT, SAMPLE_NOTE_TEXT, demoNoteHidden, sampleNoteHidden, sharedJson, syncDemoNote, syncSampleNote } from '../src/shared/document.ts';
import { Document_ } from './fake-canvas-page.ts';
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

test('the Sample data note shows only for an available read of a sample work (APP-098 DS-84)', () => {
  assert.equal(sampleNoteHidden(null), true, 'hidden while access is unverified');
  assert.equal(sampleNoteHidden({ available: true, sample_data: true }), false);
  assert.equal(sampleNoteHidden({ available: true, sample_data: false }), true);
  assert.equal(sampleNoteHidden({ available: true }), true, 'an older response without the flag');
  assert.equal(sampleNoteHidden({ available: false, sample_data: true }), true);
});

test('the Sample data note is created once, hidden while unverified and shown again (APP-098 DS-84 review)', () => {
  const document_ = new Document_();
  const host = document_.createElement('header');
  const note = () => host.querySelectorAll('[data-sample-data]');
  syncSampleNote(host as unknown as HTMLElement, null);
  assert.equal(note().length, 0, 'nothing to hide, nothing created');
  syncSampleNote(host as unknown as HTMLElement, { available: true, sample_data: false });
  assert.equal(note().length, 0, 'a work without sample data gets no note');
  // A page that became available in place (an invitation accepted in another tab).
  syncSampleNote(host as unknown as HTMLElement, { available: true, sample_data: true });
  assert.equal(note().length, 1);
  assert.equal(note()[0]!.hidden, false);
  assert.equal(note()[0]!.querySelector('.badge')?.textContent, 'Sample data');
  assert.ok(note()[0]!.all().some((n) => n.textContent === ` ${SAMPLE_NOTE_TEXT}`));
  syncSampleNote(host as unknown as HTMLElement, null);
  assert.equal(note()[0]!.hidden, true, 'hidden while access is re-checked');
  syncSampleNote(host as unknown as HTMLElement, { available: true, sample_data: true });
  assert.equal(note().length, 1, 'never a second note');
  assert.equal(note()[0]!.hidden, false);
  // The server-rendered note and the client's say the same thing.
  assert.ok(readFileSync(new URL('../src/shared/app.tsx', import.meta.url), 'utf8').includes(SAMPLE_NOTE_TEXT));
});

test('the Synthetic data note follows the same rule on the guest page and its connected card (APP-098 DS-40)', () => {
  assert.equal(demoNoteHidden(null), true, 'hidden while access is unverified');
  assert.equal(demoNoteHidden({ available: true, demo_data: true }), false);
  assert.equal(demoNoteHidden({ available: true, demo_data: false }), true);
  assert.equal(demoNoteHidden({ available: true }), true, 'an older response without the flag');
  assert.equal(demoNoteHidden({ available: false, demo_data: true }), true);
  const document_ = new Document_();
  const host = document_.createElement('header');
  const demo = () => host.querySelectorAll('[data-demo-data]');
  syncDemoNote(host as unknown as HTMLElement, null);
  assert.equal(demo().length, 0, 'nothing to hide, nothing created');
  // A page or card that became available in place, in a demo organization.
  syncDemoNote(host as unknown as HTMLElement, { available: true, demo_data: true });
  assert.equal(demo().length, 1);
  assert.equal(demo()[0]!.hidden, false);
  assert.equal(demo()[0]!.querySelector('.badge')?.textContent, 'Synthetic data');
  assert.ok(demo()[0]!.all().some((n) => n.textContent === ` ${DEMO_NOTE_TEXT}`));
  syncDemoNote(host as unknown as HTMLElement, null);
  assert.equal(demo()[0]!.hidden, true, 'hidden while access is re-checked');
  syncDemoNote(host as unknown as HTMLElement, { available: true, demo_data: true });
  assert.equal(demo().length, 1, 'never a second note');
  assert.equal(demo()[0]!.hidden, false);
  // The two notes are separate elements; one never stands in for the other.
  syncSampleNote(host as unknown as HTMLElement, { available: true, sample_data: true });
  assert.equal(host.querySelectorAll('[data-sample-data]').length, 1);
  assert.equal(demo().length, 1);
  // Both server-rendered pages say the same words as the client's note.
  for (const page of ['../src/shared/app.tsx', '../src/work/app.tsx']) {
    assert.ok(readFileSync(new URL(page, import.meta.url), 'utf8').includes(DEMO_NOTE_TEXT), page);
  }
});
