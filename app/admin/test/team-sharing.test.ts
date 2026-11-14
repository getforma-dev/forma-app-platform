// APP-192: the share dialog's organization section: roles in plain words,
// one request shape per action, the server's answer read strictly, and
// plain copy for every refusal.
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import {
  ROLE_WORDS, ShareError, everyoneRequest, memberRevokeRequest, memberShareRequest, offeredRoles,
  readTeamSharing, revokeMemberShare, shareWithEveryone, shareWithMember, teamPath, teamRefusal, teamSharing,
  type TeamSharing,
} from '../src/sharing/api.ts';

const APP = 'aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee';
const WORK = '11111111-2222-4333-8444-555555555555';
const ANA = '99999999-8888-4777-8666-555555555555';
const answer = {
  app_id: APP, work_id: WORK, roles: ['viewer', 'contributor', 'reviewer'], can_revoke_members: false,
  private_lists: true, organization: { available: true, role: null },
  members: [{ principal_id: ANA, label: 'Ana Diaz', role: 'contributor' }],
};

test('each role reads in plain words, weakest first, true for shared and private lists', () => {
  // W4's rule: in a shared list a contributor may change anyone's entries;
  // only a private list keeps them to their own (own_rows). So the words
  // claim neither "their own" nor "all".
  assert.deepEqual(ROLE_WORDS, {
    viewer: 'Can view',
    contributor: 'Can add and edit entries',
    reviewer: 'Can review all entries',
  });
  for (const words of Object.values(ROLE_WORDS)) assert.doesNotMatch(words, /record|permission|reviewer|contributor/i);
});

test('a person share names the grantee and the role, nothing else', () => {
  for (const role of ['viewer', 'contributor', 'reviewer'] as const) {
    const request = memberShareRequest(APP, WORK, ANA, role);
    assert.ok(request);
    assert.equal(request.url, `/v1/work/${APP}/${WORK}/share`);
    assert.deepEqual(JSON.parse(request.body), { grantee_principal_id: ANA, role });
    assert.deepEqual(Object.keys(JSON.parse(request.body)), ['grantee_principal_id', 'role']);
    assert.doesNotMatch(request.body, /record_write/);
  }
  assert.equal(memberShareRequest(APP, WORK, ANA, 'owner' as never), null);
  assert.equal(memberShareRequest(APP, WORK, ANA, 'none' as never), null);
  assert.equal(memberShareRequest(APP.toUpperCase(), WORK, ANA, 'viewer'), null);
  assert.equal(memberShareRequest(APP, '../x', ANA, 'viewer'), null);
  assert.equal(memberShareRequest(APP, WORK, 'Ana Diaz', 'viewer'), null);
});

test('everyone in the organization takes a role or none, on its own route', () => {
  for (const role of ['viewer', 'contributor', 'reviewer', 'none'] as const) {
    const request = everyoneRequest(APP, WORK, role);
    assert.ok(request);
    assert.equal(request.url, `/v1/work/${APP}/${WORK}/share/organization`);
    assert.deepEqual(JSON.parse(request.body), { role });
  }
  assert.equal(everyoneRequest(APP, WORK, 'everyone' as never), null);
  assert.equal(everyoneRequest(APP, 'card-1', 'viewer'), null);
});

// The person route's `none` (W4's revoke_work_share); the dialog shows the
// control only when the server says this member may use it (the owner).
test('removing one person is one request through one function', () => {
  const request = memberRevokeRequest(APP, WORK, ANA);
  assert.ok(request);
  assert.equal(request.url, `/v1/work/${APP}/${WORK}/share`);
  assert.deepEqual(JSON.parse(request.body), { grantee_principal_id: ANA, role: 'none' });
  assert.equal(memberRevokeRequest(APP, WORK, 'nobody'), null);
});

test('the dialog reads the server answer strictly', () => {
  assert.equal(teamPath(APP, WORK), `/v1/work/${APP}/${WORK}/share`);
  assert.throws(() => teamPath(APP, '../work'));
  const read = teamSharing(answer, APP, WORK);
  assert.ok(read);
  assert.deepEqual(read.members, answer.members);
  assert.equal(read.canRevokeMembers, false);
  assert.equal(read.privateLists, true);
  for (const change of [
    { app_id: WORK }, { roles: ['viewer', 'owner'] }, { roles: [] }, { roles: ['contributor'] },
    { organization: { available: true, role: 'none' } }, { organization: { available: 'yes', role: null } },
    { members: [{ principal_id: 'x', label: 'Ana', role: 'viewer' }] },
    { members: [{ principal_id: ANA, label: '', role: 'viewer' }] },
    { members: [{ principal_id: ANA, label: 'Ana', role: 'admin' }] },
    { can_revoke_members: 'false' },
    { private_lists: 'yes' }, { private_lists: undefined },
  ]) {
    assert.equal(teamSharing({ ...answer, ...change }, APP, WORK), null, JSON.stringify(change));
  }
});

test('reviewer is offered only when the server says this sharer may grant it', () => {
  const with_ = teamSharing(answer, APP, WORK) as TeamSharing;
  assert.deepEqual(offeredRoles(with_), ['viewer', 'contributor', 'reviewer']);
  const without = teamSharing({ ...answer, roles: ['viewer', 'contributor'] }, APP, WORK) as TeamSharing;
  assert.deepEqual(offeredRoles(without), ['viewer', 'contributor']);
  const viewer = teamSharing({ ...answer, roles: ['viewer'] }, APP, WORK) as TeamSharing;
  assert.deepEqual(offeredRoles(viewer), ['viewer']);
});

test('every refusal has plain copy that names no other member', () => {
  const cases: Array<[number, string, RegExp]> = [
    [422, 'reviewer_not_allowed', /review all entries/],
    [422, 'contributor_not_allowed', /add and edit/],
    [422, 'organization_share_unavailable', /^This app uses data sources or folder files, so it can be shared one person at a time only\.$/],
    [404, 'not_found', /refused/],
    [400, 'malformed_share', /refresh/i],
    [403, 'cross_site', /reload/i],
    [429, 'too_many_requests', /wait/i],
    [503, 'unavailable', /right now/],
  ];
  for (const [status, code, pattern] of cases) {
    const message = teamRefusal(status, code);
    assert.match(message, pattern, code);
    assert.doesNotMatch(message, /record_|principal|grantee|_/, code);
  }
});

test('the client sends same-origin JSON, reads a 404 as no share authority, and shows the code\'s copy', async () => {
  const original = globalThis.fetch;
  const calls: Array<{ url: string; init: RequestInit }> = [];
  try {
    globalThis.fetch = async (url, init) => { calls.push({ url: String(url), init: init! }); return new Response(JSON.stringify(answer), { status: 200 }); };
    const read = await readTeamSharing(APP, WORK);
    assert.equal(read?.members[0]?.label, 'Ana Diaz');
    assert.equal(calls[0]?.init.method, 'GET');
    assert.equal(calls[0]?.init.credentials, 'same-origin');
    assert.equal(calls[0]?.init.cache, 'no-store');

    globalThis.fetch = async () => new Response('{"code":"not_found","status":404}', { status: 404 });
    assert.equal(await readTeamSharing(APP, WORK), null);

    globalThis.fetch = async (url, init) => { calls.push({ url: String(url), init: init! }); return new Response('{"code":"reviewer_not_allowed","status":422}', { status: 422 }); };
    await assert.rejects(shareWithMember(APP, WORK, ANA, 'reviewer'), (error: unknown) => error instanceof ShareError && /review all entries/.test(error.message));
    const sent = calls.at(-1)!;
    assert.equal(sent.url, `/v1/work/${APP}/${WORK}/share`);
    assert.equal(sent.init.method, 'POST');
    assert.equal((sent.init.headers as Record<string, string>)['content-type'], 'application/json');
    assert.deepEqual(JSON.parse(sent.init.body as string), { grantee_principal_id: ANA, role: 'reviewer' });

    globalThis.fetch = async (url, init) => { calls.push({ url: String(url), init: init! }); return new Response(JSON.stringify({ app_id: APP, work_id: WORK, role: 'none' }), { status: 200 }); };
    assert.equal(await shareWithEveryone(APP, WORK, 'none'), 'none');
    assert.deepEqual(JSON.parse(calls.at(-1)!.init.body as string), { role: 'none' });
    globalThis.fetch = async () => new Response(JSON.stringify({ app_id: APP, work_id: WORK, role: 'viewer' }), { status: 200 });
    await assert.rejects(shareWithEveryone(APP, WORK, 'contributor'), ShareError, 'an answer for another role is not success');

    globalThis.fetch = async () => { throw new TypeError('lost'); };
    await assert.rejects(revokeMemberShare(APP, WORK, ANA), (error: unknown) => error instanceof ShareError && error.uncertain);
  } finally {
    globalThis.fetch = original;
  }
});

test('the dialog offers the organization section in the existing style, with no other wording', () => {
  const source = readFileSync(new URL('../src/sharing/dialog.ts', import.meta.url), 'utf8');
  for (const copy of ['People in your organization', 'Everyone in the organization', 'Stop sharing with everyone', 'Shared with']) {
    assert.ok(source.includes(copy), copy);
  }
  assert.doesNotMatch(source, /student|teacher|classroom|homework|\bgrades?\b|pupil/i);
  assert.doesNotMatch(source, /principal_id\b.*textContent|textContent.*principal_id/);
});

test('a hidden control in the section stays hidden whatever its button class', () => {
  // The smoke's first run showed "Stop sharing with everyone" while nothing
  // was shared: `.share-danger { display: inline-block }` outranked `hidden`.
  const css = readFileSync(new URL('../src/styles/sharing.css', import.meta.url), 'utf8');
  assert.match(css, /\.share-team \[hidden\][^{]*\{ display: none; \}/);
});

test('a share notice is shown only for an app this canvas does not have yet', async () => {
  const { sharedAppNotice } = await import('../src/canvas/live.ts');
  assert.equal(sharedAppNotice(true), null);
  const notice = sharedAppNotice(false);
  assert.equal(notice, 'An app was shared with you. Reload the page to find it in My apps.');
  assert.doesNotMatch(notice ?? '', /private link/i);
});

test('a removal is confirmed only by an answer naming the person and none', async () => {
  const original = globalThis.fetch;
  try {
    globalThis.fetch = async () => new Response(JSON.stringify({ app_id: APP, work_id: WORK, grantee_principal_id: ANA, role: 'none' }), { status: 200 });
    await revokeMemberShare(APP, WORK, ANA);
    for (const body of [{ app_id: APP, work_id: WORK, grantee_principal_id: ANA, role: 'viewer' }, { app_id: APP, work_id: WORK, role: 'none' }, {}]) {
      globalThis.fetch = async () => new Response(JSON.stringify(body), { status: 200 });
      await assert.rejects(revokeMemberShare(APP, WORK, ANA), (error: unknown) => error instanceof ShareError && error.uncertain, JSON.stringify(body));
    }
    globalThis.fetch = async () => new Response('{"code":"not_found","status":404}', { status: 404 });
    await assert.rejects(revokeMemberShare(APP, WORK, ANA), (error: unknown) => error instanceof ShareError && /refused/.test(error.message));
  } finally { globalThis.fetch = original; }
});
