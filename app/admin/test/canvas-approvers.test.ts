// APP-206 (ADR-020 §8): the canvas's approver step after a filed record.
// Only an approver the server judged `shareable` gets the automatic share
// request; a reviewer is reached with no request and a note; anyone else,
// and any standing the server did not send, gets a plain note and no
// request. The "share was refused" message never appears for a reviewer.
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { reviewerNote, shareWith, unreachableNote, type Draft } from '../src/canvas/actions.ts';

const APP = '00000000-0000-4000-8000-000000000a10';
const WORK = '00000000-0000-4000-8000-000000000c11';
const OLIVE = '00000000-0000-4000-8000-000000000c01';
const RITA = '00000000-0000-4000-8000-000000000c02';
const NIA = '00000000-0000-4000-8000-000000000c04';

type Approver = NonNullable<Draft['approvers']>[number];
function draft(approvers: Approver[], approver: Approver | null = approvers[0] ?? null): Draft {
  return {
    action: 'create', app_id: APP, work_id: WORK, title: 'Claims and notes', placed: false, command_id: 'submit',
    record_id: '00000000-0000-4000-8000-000000000c14', collection_id: '00000000-0000-4000-8000-000000000c12', schema_id: 'claim',
    values: [], command_request: { idempotency_key: '0123456789abcdef0123456789abcdef' }, approver, target: null,
    confirm_token: 't', parse_ms: 1, approvers,
  };
}

type Call = { url: string; method: string; body: unknown };
async function withFetch<T>(answer: (call: Call) => Response | Promise<Response>, run: (calls: Call[]) => Promise<T>): Promise<T> {
  const original = globalThis.fetch;
  const calls: Call[] = [];
  globalThis.fetch = (async (url: string, init?: RequestInit) => {
    const call = { url: String(url), method: String(init?.method ?? 'GET'), body: init?.body ? JSON.parse(String(init.body)) : null };
    calls.push(call);
    return answer(call);
  }) as typeof fetch;
  try { return await run(calls); } finally { globalThis.fetch = original; }
}
const ok = () => new Response('{}', { status: 200 });
const refused = () => new Response('{"code":"not_found","status":404}', { status: 404 });

test('a reviewer approver is reached with no share request and no refusal wording', async () => {
  await withFetch(refused, async (calls) => {
    const step = await shareWith(draft([{ principal_id: OLIVE, label: 'Olive Park', standing: 'reviewer' }]));
    assert.equal(calls.length, 0, 'no request for a reviewer');
    assert.equal(step.sent, 0);
    assert.equal(step.note, 'Filed for Olive Park\'s review.');
    assert.equal(step.note, reviewerNote('Olive Park'));
    assert.doesNotMatch(step.note, /refused|could not|share/i);
  });
});

test('an unreachable approver, and any standing the server did not send, get a plain note and no request', async () => {
  await withFetch(ok, async (calls) => {
    for (const standing of ['unreachable', undefined, 'everyone', 'Shareable'] as const) {
      const step = await shareWith(draft([{ principal_id: NIA, label: 'Nia Brooks', standing: standing as never }]));
      assert.equal(step.note, 'Nia Brooks is not a reviewer of this app; the record is filed and visible to its reviewers.', String(standing));
      assert.equal(step.note, unreachableNote('Nia Brooks'));
      assert.equal(step.sent, 0);
    }
    assert.equal(calls.length, 0, 'fail closed: only the exact `shareable` ever shares');
  });
});

test('a shareable approver goes through /share/add as a contributor, with today\'s three outcomes', async () => {
  const shareable = draft([{ principal_id: NIA, label: 'Nia Brooks', standing: 'shareable' }]);
  await withFetch(ok, async (calls) => {
    const step = await shareWith(shareable);
    assert.deepEqual(calls, [{ url: `/v1/work/${APP}/${WORK}/share/add`, method: 'POST', body: { grantee_principal_id: NIA, role: 'contributor' } }]);
    assert.deepEqual(step, { note: 'Shared with Nia Brooks for approval.', sent: 1 });
  });
  await withFetch(refused, async () => {
    assert.equal((await shareWith(shareable)).note, 'The share with Nia Brooks was refused; the record is filed.');
  });
  await withFetch(() => { throw new Error('offline'); }, async () => {
    assert.equal((await shareWith(shareable)).note, 'The share with Nia Brooks could not be sent; the record is filed.');
  });
});

test('several approvers: one request for the shareable one only, notes in order', async () => {
  await withFetch(ok, async (calls) => {
    const step = await shareWith(draft([
      { principal_id: OLIVE, label: 'Olive Park', standing: 'reviewer' },
      { principal_id: RITA, label: 'Rita Okafor', standing: 'shareable' },
      { principal_id: NIA, label: 'Nia Brooks', standing: 'unreachable' },
    ]));
    assert.deepEqual(calls.map((c) => (c.body as { grantee_principal_id: string }).grantee_principal_id), [RITA]);
    assert.equal(step.sent, 1);
    assert.equal(step.note, [reviewerNote('Olive Park'), 'Shared with Rita Okafor for approval.', unreachableNote('Nia Brooks')].join(' '));
  });
});

test('a draft with only `approver` (no list) follows its standing too; no approver, no note', async () => {
  await withFetch(ok, async (calls) => {
    const single = { principal_id: OLIVE, label: 'Olive Park', standing: 'reviewer' as const };
    assert.deepEqual(await shareWith({ ...draft([], single), approvers: undefined }), { note: reviewerNote('Olive Park'), sent: 0 });
    assert.deepEqual(await shareWith(draft([], null)), { note: '', sent: 0 });
    assert.equal(calls.length, 0);
  });
});
