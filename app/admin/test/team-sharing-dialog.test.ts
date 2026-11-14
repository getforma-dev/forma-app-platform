// APP-192 review (MINOR-2, MINOR-3, MINOR-7, NIT-2): the share dialog's
// organization section driven as a whole: which roles it offers, when the
// stop control shows, what an outage looks like, and the confirmation before
// anyone gets "Can review all entries". A small DOM double stands in for the
// browser; it is not a browser and renders nothing.
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { teamSection } from '../src/sharing/dialog.ts';

const APP = 'aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee';
const WORK = '11111111-2222-4333-8444-555555555555';
const ANA = '99999999-8888-4777-8666-555555555555';
const OWNER = 'cccccccc-dddd-4eee-8fff-000000000001';

type Listener = (event: { preventDefault(): void }) => void;
class El {
  readonly tagName: string;
  children: El[] = [];
  parent: El | null = null;
  attrs = new Map<string, string>();
  textContent = '';
  hidden = false;
  disabled = false;
  selected = false;
  listeners = new Map<string, Listener[]>();
  constructor(readonly doc: Doc, tag: string) { this.tagName = tag.toUpperCase(); }
  setAttribute(name: string, value: string) {
    this.attrs.set(name, value);
    if (name === 'selected') this.selected = true;
    if (name === 'disabled') this.disabled = true;
    if (name === 'hidden') this.hidden = true;
  }
  getAttribute(name: string) { return this.attrs.get(name) ?? null; }
  append(...nodes: El[]) { for (const node of nodes) { node.parent = this; this.children.push(node); } }
  replaceChildren(...nodes: El[]) { this.children = []; this.append(...nodes); }
  get isConnected(): boolean { return this === this.doc.body || (this.parent?.isConnected ?? false); }
  addEventListener(type: string, listener: Listener) { this.listeners.set(type, [...(this.listeners.get(type) ?? []), listener]); }
  fire(type: string) { for (const listener of this.listeners.get(type) ?? []) listener({ preventDefault() {} }); }
  get value(): string {
    if (this.tagName === 'SELECT') { const option = this.children.find(c => c.selected) ?? this.children[0]; return option?.value ?? ''; }
    return this.attrs.get('value') ?? '';
  }
  set value(wanted: string) { for (const option of this.children) option.selected = option.value === wanted; }
  get selectedOptions(): El[] { const value = this.value; const option = this.children.find(c => c.value === value); return option ? [option] : []; }
  all(): El[] { return [this, ...this.children.flatMap(child => child.all())]; }
  /** Shown: neither it nor an ancestor is hidden. */
  get shown(): boolean { return !this.hidden && (this.parent ? this.parent.shown : true); }
}
class Doc {
  body = new El(this, 'body');
  createElement(tag: string) { return new El(this, tag); }
}
const find = (root: El, predicate: (el: El) => boolean) => root.all().find(predicate);
const select = (root: El, name: string) => find(root, el => el.tagName === 'SELECT' && el.getAttribute('name') === name)!;
/** The visible button with this text, else any (to assert that it is hidden). */
const button = (root: El, text: string) => find(root, el => el.tagName === 'BUTTON' && el.textContent === text && el.shown)
  ?? find(root, el => el.tagName === 'BUTTON' && el.textContent === text);
const statusText = (root: El) => find(root, el => el.getAttribute('role') === 'status')!.textContent;
const flush = () => new Promise(resolve => setImmediate(resolve));

function answer(change: Record<string, unknown> = {}) {
  return {
    app_id: APP, work_id: WORK, roles: ['viewer', 'contributor', 'reviewer'], can_revoke_members: false,
    private_lists: true, owner_principal_id: OWNER, organization: { available: true, role: null },
    members: [{ principal_id: ANA, label: 'Ana Diaz', role: 'contributor' }], ...change,
  };
}
const colleagues = { members: [{ principal_id: OWNER, label: 'Owen Park' }, { principal_id: ANA, label: 'Ana Diaz' }] };
type Call = { url: string; method: string; body: unknown };
/** Answers GET .../share with `read()`, /v1/members with colleagues, and every POST with `post(body)`. */
function server(read: () => Response, post: (body: Record<string, unknown>) => Response = () => new Response('{}', { status: 500 })) {
  const calls: Call[] = [];
  globalThis.fetch = (async (url: string, init: RequestInit) => {
    const body = init.body ? JSON.parse(init.body as string) : null;
    calls.push({ url: String(url), method: init.method ?? 'GET', body });
    if (init.method === 'POST') return post(body);
    if (String(url) === '/v1/members') return new Response(JSON.stringify(colleagues), { status: 200 });
    return read();
  }) as typeof fetch;
  return calls;
}
const ok = (value: unknown) => new Response(JSON.stringify(value), { status: 200 });

async function open(read: () => Response, post?: (body: Record<string, unknown>) => Response) {
  const calls = server(read, post);
  const doc = new Doc();
  const team = teamSection(doc as unknown as Document, APP, WORK);
  const section = team.section as unknown as El;
  doc.body.append(section);
  await team.load();
  return { calls, section };
}

test('the access lists offer exactly the roles the server names, in plain words', async () => {
  const original = globalThis.fetch;
  try {
    const { section } = await open(() => ok(answer({ roles: ['viewer', 'contributor'] })));
    for (const name of ['role', 'everyone_role']) {
      const list = select(section, name);
      assert.deepEqual(list.children.map(o => o.value), ['viewer', 'contributor'], name);
      assert.deepEqual(list.children.map(o => o.textContent), ['Can view', 'Can add and edit entries'], name);
    }
    const all = await open(() => ok(answer()));
    assert.deepEqual(select(all.section, 'role').children.map(o => o.value), ['viewer', 'contributor', 'reviewer']);
    // People are listed by name with their access, never by id.
    const listed = all.section.all().map(el => el.textContent).filter(Boolean);
    assert.ok(listed.includes('Ana Diaz') && listed.includes('Can add and edit entries'));
    assert.ok(!listed.some(text => text.includes(ANA)));
  } finally { globalThis.fetch = original; }
});

test('the stop control shows only while something is shared with everyone', async () => {
  const original = globalThis.fetch;
  try {
    let live: string | null = null;
    const { section, calls } = await open(() => ok(answer({ organization: { available: true, role: live } })), body => {
      live = body.role === 'none' ? null : String(body.role);
      return ok({ app_id: APP, work_id: WORK, role: body.role });
    });
    const stop = () => button(section, 'Stop sharing with everyone')!;
    assert.equal(stop().shown, false, 'nothing is shared with everyone yet');
    button(section, 'Share with everyone')!.fire('click');
    await flush(); await flush();
    assert.deepEqual(calls.at(-2)?.body, { role: 'viewer' });
    assert.equal(stop().shown, true);
    stop().fire('click');
    await flush(); await flush();
    assert.deepEqual(calls.filter(c => c.method === 'POST').at(-1)?.body, { role: 'none' });
    assert.equal(stop().shown, false, 'stopped: the control goes away');
    assert.match(statusText(section), /Stopped sharing with everyone/);
  } finally { globalThis.fetch = original; }
});

test('a work shared person by person still offers to stop a live share with everyone', async () => {
  const original = globalThis.fetch;
  try {
    const { section } = await open(() => ok(answer({ organization: { available: false, role: 'viewer' } })));
    assert.equal(button(section, 'Stop sharing with everyone')!.shown, true);
    assert.equal(button(section, 'Share with everyone')?.shown ?? false, false);
    assert.equal(select(section, 'everyone_role').shown, false);
  } finally { globalThis.fetch = original; }
});

test('an outage shows the section with a plain message and a retry; a 404 hides it', async () => {
  const original = globalThis.fetch;
  try {
    let up = false;
    const { section } = await open(() => up ? ok(answer()) : new Response('{"code":"unavailable","status":503}', { status: 503 }));
    assert.equal(section.shown, true, 'an outage does not hide the section');
    assert.equal(statusText(section), 'Sharing is unavailable right now. Try again in a moment.');
    const retry = button(section, 'Try again')!;
    assert.equal(retry.shown, true);
    assert.equal(select(section, 'role').shown, false, 'nothing to act on yet');
    up = true;
    retry.fire('click');
    await flush(); await flush();
    assert.equal(select(section, 'role').shown, true);
    assert.equal(retry.shown, false);
    const refused = await open(() => new Response('{"code":"not_found","status":404}', { status: 404 }));
    assert.equal(refused.section.shown, false, 'no share authority: no section');
  } finally { globalThis.fetch = original; }
});

test('"Can review all entries" for everyone asks first, naming private entries; cancel sends nothing', async () => {
  const original = globalThis.fetch;
  try {
    let live: string | null = null;
    const { section, calls } = await open(() => ok(answer({ organization: { available: true, role: live } })), body => {
      live = body.role === 'none' ? null : String(body.role);
      return ok({ app_id: APP, work_id: WORK, role: body.role });
    });
    select(section, 'everyone_role').value = 'reviewer';
    button(section, 'Share with everyone')!.fire('click');
    await flush();
    assert.equal(calls.filter(c => c.method === 'POST').length, 0, 'nothing is sent before the confirmation');
    const asked = section.all().find(el => el.shown && /including private ones/.test(el.textContent));
    assert.ok(asked, 'the confirmation names the effect');
    assert.match(asked.textContent, /^Everyone in the organization will see all entries, including private ones/);
    button(section, 'Cancel')!.fire('click');
    await flush();
    assert.equal(calls.filter(c => c.method === 'POST').length, 0);
    assert.equal(section.all().some(el => el.shown && /including private ones/.test(el.textContent)), false);
    button(section, 'Share with everyone')!.fire('click');
    button(section, 'Let everyone review all entries')!.fire('click');
    await flush(); await flush();
    assert.deepEqual(calls.filter(c => c.method === 'POST').map(c => c.body), [{ role: 'reviewer' }]);
    // A lesser role for everyone needs no confirmation.
    select(section, 'everyone_role').value = 'viewer';
    button(section, 'Change access for everyone')!.fire('click');
    await flush(); await flush();
    assert.deepEqual(calls.filter(c => c.method === 'POST').at(-1)?.body, { role: 'viewer' });
  } finally { globalThis.fetch = original; }
});

test('a person given "Can review all entries" is confirmed by name first', async () => {
  const original = globalThis.fetch;
  try {
    const { section, calls } = await open(() => ok(answer()), body => ok({ ...body, app_id: APP, work_id: WORK }));
    select(section, 'grantee').value = ANA;
    select(section, 'role').value = 'reviewer';
    find(section, el => el.tagName === 'FORM')!.fire('submit');
    await flush();
    assert.equal(calls.filter(c => c.method === 'POST').length, 0);
    assert.ok(section.all().some(el => el.shown && el.textContent.startsWith('Ana Diaz will see all entries, including private ones')));
    button(section, 'Yes, share')!.fire('click');
    await flush(); await flush();
    assert.deepEqual(calls.filter(c => c.method === 'POST').map(c => c.body), [{ grantee_principal_id: ANA, role: 'reviewer' }]);
  } finally { globalThis.fetch = original; }
});

test('an app with private lists says what that means for each role', async () => {
  const original = globalThis.fetch;
  try {
    const note = (section: El) => section.all().find(el => el.shown && el.textContent.startsWith('This app has private lists'));
    const withPrivate = await open(() => ok(answer()));
    assert.ok(note(withPrivate.section));
    const shared = await open(() => ok(answer({ private_lists: false })));
    assert.equal(note(shared.section), undefined);
  } finally { globalThis.fetch = original; }
});

test('"Remove access" shows only for the owner, removes the person, and says so', async () => {
  const original = globalThis.fetch;
  try {
    const remove = (section: El) => find(section, el => el.tagName === 'BUTTON' && el.textContent === 'Remove access');
    const sharer = await open(() => ok(answer({ can_revoke_members: false })));
    assert.equal(remove(sharer.section), undefined, 'no control for a sharer who is not the owner');
    let listed = [{ principal_id: ANA, label: 'Ana Diaz', role: 'reviewer' }];
    const { section, calls } = await open(() => ok(answer({ can_revoke_members: true, members: listed })), body => {
      listed = [];
      return ok({ ...body, app_id: APP, work_id: WORK });
    });
    const button_ = remove(section)!;
    assert.equal(button_.getAttribute('aria-label'), 'Remove access for Ana Diaz');
    button_.fire('click');
    await flush(); await flush();
    assert.deepEqual(calls.filter(c => c.method === 'POST').map(c => c.body), [{ grantee_principal_id: ANA, role: 'none' }]);
    assert.equal(statusText(section), 'Ana Diaz no longer has access.');
    assert.equal(remove(section), undefined, 'the list no longer shows her');
    assert.ok(section.all().some(el => el.textContent === 'Not shared with anyone in your organization yet.'));
  } finally { globalThis.fetch = original; }
});

test('a lower access for a person says they now have it', async () => {
  const original = globalThis.fetch;
  try {
    let role = 'reviewer';
    const { section } = await open(() => ok(answer({ members: [{ principal_id: ANA, label: 'Ana Diaz', role }] })), body => {
      role = String(body.role);
      return ok({ ...body, app_id: APP, work_id: WORK });
    });
    select(section, 'grantee').value = ANA;
    select(section, 'role').value = 'viewer';
    find(section, el => el.tagName === 'FORM')!.fire('submit');
    await flush(); await flush();
    assert.equal(statusText(section), 'Ana Diaz now has “Can view”.');
    assert.ok(section.all().some(el => el.textContent === 'Can view' && el.tagName === 'P'));
  } finally { globalThis.fetch = original; }
});

test('the person picker leaves out the app owner (the caller is never in /v1/members)', async () => {
  const original = globalThis.fetch;
  try {
    const { section } = await open(() => ok(answer()));
    const picker = select(section, 'grantee');
    assert.deepEqual(picker.children.map(o => o.value), ['', ANA]);
    assert.ok(!picker.children.some(o => o.textContent === 'Owen Park'));
    // An app without a known owner leaves everyone listed in the picker.
    const unknown = await open(() => ok(answer({ owner_principal_id: null })));
    assert.deepEqual(select(unknown.section, 'grantee').children.map(o => o.value), ['', OWNER, ANA]);
  } finally { globalThis.fetch = original; }
});
