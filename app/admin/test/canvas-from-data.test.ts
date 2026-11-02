// APP-124 (start from your data): "Drop files or upload" places spreadsheets
// as file cards, while other files make a folder; a folder with files
// offers Ask (the Ask tab scoped to the folder: `POST /v1/boxes/{box}/ask`),
// Make a card (its confirm first, then `POST /v1/boxes/{box}/apps
// {"one_card": true}`) and Make an app (the formation confirm, then the
// full formation); a folder answer is exactly `{text, references}`, each
// reference a file and row.
import assert from 'node:assert/strict';
import { test } from 'node:test';

const BOX = '00000000-0000-4000-8000-0000000b0c01';
const FILE = '00000000-0000-4000-8000-0000000f0c01';
const FILE2 = '00000000-0000-4000-8000-0000000f0c02';
(globalThis as { requestAnimationFrame?: unknown }).requestAnimationFrame = () => 0;

// --- A small fake DOM (no browser): enough for `el()`, the dock and the Library's rows ---
type Listener = (event: { type: string; target: FakeNode; stopPropagation(): void; preventDefault(): void }) => void;
class FakeNode {
  attributes = new Map<string, string>();
  dataset: Record<string, string> = {};
  children: FakeNode[] = [];
  listeners = new Map<string, Listener[]>();
  hidden = false;
  value = '';
  files: unknown[] = [];
  title = '';
  disabled = false;
  parent: FakeNode | null = null;
  text = '';
  classes = new Set<string>();
  classList = {
    add: (name: string) => this.classes.add(name),
    remove: (name: string) => this.classes.delete(name),
    contains: (name: string) => this.classes.has(name),
    toggle: (name: string, on?: boolean) => ((on ?? !this.classes.has(name)) ? this.classes.add(name) : this.classes.delete(name)),
  };
  constructor(public ownerDocument: FakeDocument, public tagName = 'div') {}
  get textContent(): string { return this.text + this.children.map((child) => child.textContent).join(''); }
  set textContent(value: string) { this.text = value; this.children = []; }
  get isConnected(): boolean { return this.parent !== null; }
  setAttribute(name: string, value: string) {
    if (name === 'hidden') this.hidden = true;
    else if (name.startsWith('data-')) this.dataset[name.slice(5).replace(/-([a-z])/g, (_m, c: string) => c.toUpperCase())] = value;
    else if (name === 'class') value.split(/\s+/).filter(Boolean).forEach((c) => this.classes.add(c));
    this.attributes.set(name, value);
    if (name === 'id') this.ownerDocument.ids.set(value, this);
  }
  getAttribute(name: string) {
    if (name.startsWith('data-')) return this.dataset[name.slice(5).replace(/-([a-z])/g, (_m, c: string) => c.toUpperCase())] ?? null;
    return this.attributes.get(name) ?? null;
  }
  removeAttribute(name: string) { this.attributes.delete(name); }
  private adopt(nodes: (FakeNode | string)[]): FakeNode[] {
    return nodes.map((node) => {
      const child = typeof node === 'string' ? this.ownerDocument.createTextNode(node) : node;
      child.parent?.detach(child);
      child.parent = this;
      return child;
    });
  }
  detach(child: FakeNode) { this.children = this.children.filter((node) => node !== child); child.parent = null; }
  append(...nodes: (FakeNode | string)[]) { this.children.push(...this.adopt(nodes)); }
  prepend(...nodes: (FakeNode | string)[]) { this.children.unshift(...this.adopt(nodes)); }
  replaceChildren(...nodes: FakeNode[]) { for (const child of [...this.children]) this.detach(child); this.append(...nodes); }
  private sibling(offset: number, nodes: (FakeNode | string)[]) {
    const parent = this.parent;
    if (!parent) return;
    const adopted = nodes.map((node) => (typeof node === 'string' ? this.ownerDocument.createTextNode(node) : node));
    for (const node of adopted) node.parent?.detach(node);
    const at = parent.children.indexOf(this) + offset;
    for (const node of adopted) node.parent = parent;
    parent.children.splice(at, 0, ...adopted);
  }
  before(...nodes: (FakeNode | string)[]) { this.sibling(0, nodes); }
  after(...nodes: (FakeNode | string)[]) { this.sibling(1, nodes); }
  replaceWith(node: FakeNode) { const parent = this.parent; if (!parent) return; const at = parent.children.indexOf(this); parent.detach(this); node.parent = parent; parent.children.splice(at, 0, node); }
  remove() { this.parent?.detach(this); }
  contains(node: unknown) { return this.all().includes(node as FakeNode); }
  addEventListener(type: string, listener: Listener) { this.listeners.set(type, [...(this.listeners.get(type) ?? []), listener]); }
  dispatchEvent(event: { type: string }) { this.click(event.type); return true; }
  click(type = 'click') {
    const event = { type, target: this, stopped: false, defaultPrevented: false, stopPropagation() { this.stopped = true; }, preventDefault() { this.defaultPrevented = true; } };
    for (let node: FakeNode | null = this; node && !event.stopped; node = node.parent) for (const listener of node.listeners.get(type) ?? []) listener(event);
  }
  focus() { this.ownerDocument.activeElement = this; }
  scrollIntoView() { /* nothing to scroll */ }
  setSelectionRange() { /* caret */ }
  all(): FakeNode[] { return [this, ...this.children.flatMap((child) => child.all())]; }
  matches(selector: string): boolean {
    const attr = selector.match(/^\[([a-z-]+)(?:="([^"]*)")?\]$/);
    if (attr) {
      const value = this.getAttribute(attr[1]!) ?? (this.attributes.has(attr[1]!) ? '' : null);
      return attr[2] === undefined ? value !== null : value === attr[2];
    }
    const compound = selector.match(/^\.([\w-]+)\[([a-z-]+)\]$/);
    if (compound) return this.classes.has(compound[1]!) && this.getAttribute(compound[2]!) !== null;
    if (selector.startsWith('.')) return this.classes.has(selector.slice(1));
    return this.tagName === selector;
  }
  querySelector<T = FakeNode>(selector: string): T | null { return (this.all().slice(1).find((node) => node.matches(selector)) ?? null) as T | null; }
  querySelectorAll(selector: string) { return this.all().slice(1).filter((node) => node.matches(selector)); }
}
class FakeDocument {
  ids = new Map<string, FakeNode>();
  activeElement: FakeNode | null = null;
  body = new FakeNode(this, 'body');
  created: FakeNode[] = [];
  createElement(tag: string) { const node = new FakeNode(this, tag); this.created.push(node); return node; }
  createElementNS(_ns: string, tag: string) { return new FakeNode(this, tag); }
  createTextNode(text: string) { const node = new FakeNode(this, '#text'); node.text = text; return node; }
  getElementById(id: string) { return this.ids.get(id) ?? null; }
  querySelector(selector: string) { return this.body.querySelector(selector); }
  querySelectorAll(selector: string) { return this.body.querySelectorAll(selector); }
  addEventListener() { /* no global keys here */ }
  removeEventListener() { /* no global keys here */ }
}

function freshDocument(): FakeDocument {
  const document_ = new FakeDocument();
  const dock = document_.createElement('div');
  dock.setAttribute('id', 'cv-confirm');
  document_.body.append(dock);
  (globalThis as { document?: unknown }).document = document_;
  return document_;
}

type Call = { url: string; method: string; body: Record<string, unknown> | null; key?: string };
type FetchAnswer = { status: number; body: unknown };
function fakeFetch(answer: (call: Call) => FetchAnswer | Promise<FetchAnswer>): Call[] {
  const calls: Call[] = [];
  (globalThis as { fetch?: unknown }).fetch = async (url: string, init: { method?: string; body?: string; headers?: Record<string, string> } = {}) => {
    const call = {
      url, method: init.method ?? 'GET', body: init.body ? JSON.parse(init.body) as Record<string, unknown> : null,
      ...(init.headers?.['idempotency-key'] ? { key: init.headers['idempotency-key'] } : {}),
    };
    calls.push(call);
    const { status, body } = await answer(call);
    return { ok: status < 300, status, json: async () => body };
  };
  return calls;
}


const button = (root: FakeNode, label: string) => root.all().find((node) => node.tagName === 'button' && node.textContent === label);
const settle = () => new Promise((resolve) => setTimeout(resolve, 10));

/** `uploadFile` uses XHR, which the fake `fetch` does not see. */
class FakeXHR {
  status = 0;
  responseText = '';
  upload = { addEventListener() { /* progress is optional */ } };
  private listeners = new Map<string, (() => void)[]>();
  open() { /* the URL is fixed by uploadFile */ }
  addEventListener(type: string, listener: () => void) {
    this.listeners.set(type, [...(this.listeners.get(type) ?? []), listener]);
  }
  send(form: FormData) {
    const sent = form.get('file');
    const name = sent instanceof File ? sent.name : 'q3_metrics.csv';
    const xlsx = name.toLowerCase().endsWith('.xlsx');
    this.status = 201;
    this.responseText = JSON.stringify({
      file: {
        file_id: xlsx ? FILE2 : FILE, name, kind: xlsx ? 'xlsx' : 'csv', media_type: xlsx ? 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet' : 'text/csv',
        bytes: 900, sha256: 'sha256:x', created_at: '2026-10-31T00:00:00Z', data: null,
      },
      preview: { format: xlsx ? 'xlsx' : 'csv', sheet: null, rows: 3, columns: [{ id: 'amount', header: 'Amount', type: 'decimal', nullable: false }], sample: [['1']] },
    });
    for (const listener of this.listeners.get('load') ?? []) listener();
  }
}
const contents = (owner: boolean, files: number) => ({
  box: { box_id: BOX, number: 1, name: 'Q3 board pack', kind: 'box', app_id: null, owner, file_count: files, created_at: '2026-10-31T00:00:00Z' },
  files: Array.from({ length: files }, (_, n) => ({
    file_id: FILE, name: n ? `notes-${n}.pdf` : 'q3_metrics.csv', kind: n ? 'pdf' : 'csv', media_type: 'text/csv', bytes: 900, sha256: 'sha256:x',
    created_at: '2026-10-31T00:00:00Z', content_url: `/v1/boxes/${BOX}/files/${FILE}/content`, data: null,
  })),
  apps: [],
}) as never;

test('the empty canvas offers "Drop files or upload": a spreadsheet becomes a file, other files make a folder (APP-124)', async () => {
  const document_ = freshDocument();
  const bar = document_.createElement('input');
  bar.setAttribute('id', 'intent-input');
  const describe = document_.createElement('button');
  describe.setAttribute('data-empty-describe', '');
  const pick = document_.createElement('button');
  pick.setAttribute('data-empty-upload-pick', '');
  const input = document_.createElement('input');
  input.setAttribute('data-empty-upload', '');
  let picked = 0;
  input.addEventListener('click', () => { picked += 1; });
  document_.body.append(bar, describe, pick, input);
  const calls = fakeFetch((call) => (call.method === 'POST' ? { status: 201, body: { box: { box_id: BOX, number: 1, name: 'Untitled folder', kind: 'box', owner: true, file_count: 0, created_at: '2026-10-31T00:00:00Z' } } } : { status: 200, body: { boxes: [] } }));
  const boxes = await import('../src/canvas/boxes.ts');
  boxes.startEmptyChoices(document_ as unknown as Document);
  boxes.startEmptyChoices(document_ as unknown as Document);
  describe.click();
  assert.equal(document_.activeElement, null, 'the upload module leaves creation to the APP-141 modal');
  pick.click();
  assert.equal(picked, 1, 'the button opens the file picker (wired once)');
  assert.equal(calls.length, 0, 'nothing is created before files are picked');
  (globalThis as { XMLHttpRequest?: unknown }).XMLHttpRequest = FakeXHR;
  input.files = [new File(['amount\n1'], 'q3_metrics.csv', { type: 'text/csv' })];
  input.click('change');
  await settle();
  assert.equal(calls.filter((c) => c.method === 'POST' && c.url === '/v1/boxes').length, 0, 'a spreadsheet does not become a folder or an app');
  assert.equal(calls.filter((c) => c.url === '/v1/boxes/uploads').length, 1, 'the file is kept in Uploads until the member chooses');
  const notice = document_.getElementById('bx-notice')!;
  assert.doesNotMatch(notice.textContent, /until you choose/);
  assert.equal(button(notice, 'Open original file'), undefined);
  assert.equal(button(notice, 'Make a card'), undefined);
  assert.equal(button(notice, 'Make an app'), undefined, 'creation belongs on the uploaded file card, not a second upload choice');
  const fileCard = document_.created.find(node => node.classes.has('bx-sheet'))!;
  assert.ok(fileCard, 'upload constructs the file itself as a card');
  assert.ok(button(fileCard, 'Make an app'), 'the file card owns its upgrade action');
  assert.equal(button(fileCard, 'Make a card'), undefined);
  notice.all().find((node) => node.getAttribute('aria-label') === 'Dismiss')!.click();
  assert.equal(notice.hidden, true, 'dismissing upload status does not undo the file');
  assert.equal(calls.filter((c) => c.url.includes('/apps')).length, 0, 'dismiss does not build an app or a card');
  assert.equal(calls.filter((c) => c.url.endsWith('/data')).length, 0, 'upload parses the original without creating a data work');
  input.files = [new File(['amount\n1'], 'notes.pdf', { type: 'application/pdf' })];
  input.click('change');
  await settle();
  const created = calls.filter((c) => c.method === 'POST' && c.url === '/v1/boxes');
  assert.deepEqual(created.map((c) => c.body), [{ name: 'Untitled folder' }], 'a file that is not a spreadsheet still makes a folder');
  assert.equal(input.value, '');
});

test('a spreadsheet dropped with another file stays a file, and the other file makes a folder', async () => {
  const document_ = freshDocument();
  const bar = document_.createElement('input');
  bar.setAttribute('id', 'intent-input');
  const pick = document_.createElement('button');
  pick.setAttribute('data-empty-upload-pick', '');
  const input = document_.createElement('input');
  input.setAttribute('data-empty-upload', '');
  document_.body.append(bar, pick, input);
  const calls = fakeFetch(() => ({ status: 201, body: { box: { box_id: BOX, number: 1, name: 'Untitled folder', kind: 'box', owner: true, file_count: 0, created_at: '2026-10-31T00:00:00Z' } } }));
  (globalThis as { XMLHttpRequest?: unknown }).XMLHttpRequest = FakeXHR;
  const boxes = await import('../src/canvas/boxes.ts');
  boxes.startEmptyChoices(document_ as unknown as Document);
  input.files = [
    new File(['amount\n1'], 'q3_metrics.csv', { type: 'text/csv' }),
    new File(['notes'], 'notes.pdf', { type: 'application/pdf' }),
  ];
  input.click('change');
  await settle();
  assert.equal(calls.filter((c) => c.url === '/v1/boxes/uploads').length, 1, 'the spreadsheet is kept in Uploads');
  assert.deepEqual(
    calls.filter((c) => c.method === 'POST' && c.url === '/v1/boxes').map((c) => c.body),
    [{ name: 'Untitled folder' }],
    'the other file still makes a folder',
  );
  const notice = document_.getElementById('bx-notice')!;
  assert.equal(button(notice, 'Open original file'), undefined, 'the file card opens its original on double-click');
  assert.ok(document_.created.some(node => node.classes.has('bx-sheet')));
  assert.doesNotMatch(notice.textContent, /until you choose/);
});

test('several spreadsheet uploads make individual file cards without starting a bulk app', async () => {
  const document_ = freshDocument();
  const calls = fakeFetch(() => ({ status: 201, body: { box: { box_id: BOX, number: 1, name: 'Uploads', kind: 'uploads', owner: true, file_count: 0, created_at: '2026-10-31T00:00:00Z' } } }));
  (globalThis as { XMLHttpRequest?: unknown }).XMLHttpRequest = FakeXHR;
  const boxes = await import('../src/canvas/boxes.ts');
  await boxes.newUntitled([
    new File(['amount\n1'], 'q3_metrics.csv', { type: 'text/csv' }),
    new File(['pk'], 'other.xlsx', { type: 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet' }),
  ]);
  const cards = document_.created.filter(node => node.classes.has('bx-sheet'));
  assert.deepEqual(cards.map(card => card.dataset.file), [FILE, FILE2]);
  for (const card of cards) {
    assert.ok(button(card, 'Make an app'));
    assert.equal(button(card, 'Make a card'), undefined);
  }
  const notice = document_.getElementById('bx-notice')!;
  assert.equal(button(notice, 'Make a card'), undefined);
  assert.equal(button(notice, 'Make an app'), undefined);
  assert.equal(calls.filter(call => call.url.endsWith('/apps') || call.url.endsWith('/data')).length, 0);
});

test('app result counts distinguish requested files from actually used tables and reject impossible success', async () => {
  const { appFileSummary } = await import('../src/canvas/data-sheet.ts');
  assert.equal(appFileSummary(4, 3), 'using 3 of 4 requested files');
  assert.equal(appFileSummary(4, 1), 'using 1 of 4 requested files');
  assert.equal(appFileSummary(2, 2), 'using 2 of 2 requested files');
  assert.equal(appFileSummary(1, 1), 'using 1 requested file');
  for (const [requested, used] of [[1, 0], [1, 2], [4, 4], [4, -1], [4, 1.5], [4, NaN], [0, 0], [21, 1]]) {
    assert.throws(() => appFileSummary(requested!, used!), /invalid_app_file_count/);
  }
});

const appSheetFile = {
  file_id: FILE, name: 'q3_metrics.csv', kind: 'csv' as const, media_type: 'text/csv', bytes: 20,
  sha256: 'sha256:x', created_at: '2026-10-31T00:00:00Z', content_url: `/v1/boxes/${BOX}/files/${FILE}/content`, data: null,
};
const appSheetPreview = {
  format: 'csv' as const, sheet: null, rows: 3,
  columns: [{ id: 'amount', header: 'Amount', type: 'integer' as const, nullable: false, scale: null }], sample: [['1']],
};
const APP = '00000000-0000-4000-8000-0000000a0c01';
const WORK = '00000000-0000-4000-8000-0000000a0c02';

test('the real app sheet reports the returned used count without changing requested file_ids', async () => {
  const document_ = freshDocument();
  const activity = document_.createElement('p');
  activity.setAttribute('id', 'cv-activity');
  document_.body.append(activity);
  const ids = [FILE, FILE2, '00000000-0000-4000-8000-0000000f0c03', '00000000-0000-4000-8000-0000000f0c04'];
  const calls = fakeFetch((call) => call.url.endsWith('/apps')
    ? { status: 201, body: { app_id: APP, work_id: WORK, title: 'Summary', files: 3 } }
    : { status: 200, body: {} });
  const { openDataSheet } = await import('../src/canvas/data-sheet.ts');
  let completed = 0;
  openDataSheet(BOX, appSheetFile, appSheetPreview, () => { completed += 1; }, false, ids);
  assert.match(document_.body.textContent, /4 files requested; apps use up to 3 readable tables/);
  assert.match(document_.body.textContent, /Other requested files use their saved types or inferred defaults/);
  button(document_.body, 'Make an app from this')!.click();
  button(document_.body, 'Build the app')!.click();
  await settle();
  assert.deepEqual(calls.filter((call) => call.url.endsWith('/apps')).map((call) => call.body), [{ purpose: 'an overview of this data', file_ids: ids }]);
  assert.deepEqual(calls.filter((call) => call.url.endsWith('/data')).map((call) => call.url), [`/v1/boxes/${BOX}/files/${FILE}/data`]);
  assert.equal(completed, 1);
  assert.match(activity.textContent, /generated using 3 of 4 requested files/);
  assert.doesNotMatch(activity.textContent, /generated from 4 files|includes all/);
});

test('a single requested file cannot announce success when unreadable or reported as unused', async () => {
  const { openDataSheet } = await import('../src/canvas/data-sheet.ts');
  for (const result of [
    { status: 422, body: { code: 'no_data_files' }, message: /no table \(CSV or XLSX\).*Nothing was placed/ },
    { status: 201, body: { app_id: APP, work_id: WORK, title: 'Summary', files: 0 }, message: /service is unavailable/ },
  ]) {
    const document_ = freshDocument();
    const activity = document_.createElement('p');
    activity.setAttribute('id', 'cv-activity');
    document_.body.append(activity);
    const calls = fakeFetch((call) => call.url.endsWith('/apps') ? result : { status: 200, body: {} });
    let completed = 0;
    openDataSheet(BOX, appSheetFile, appSheetPreview, () => { completed += 1; });
    button(document_.body, 'Make an app from this')!.click();
    button(document_.body, 'Build the app')!.click();
    await settle();
    assert.deepEqual(calls.filter((call) => call.url.endsWith('/apps')).map((call) => call.body), [{ purpose: 'an overview of this data', file_ids: [FILE] }]);
    assert.equal(completed, 0);
    assert.equal(activity.textContent, '');
    assert.match(document_.body.querySelector('.bx-status')!.textContent, result.message);
    assert.equal(button(document_.body, 'Build the app')!.disabled, false, 'failure stays actionable without a success announcement');
  }
});

test('a folder with files offers Ask, Make a card and Make an app; Make waits for its confirm (APP-124)', async () => {
  const document_ = freshDocument();
  const calls = fakeFetch((call) => (call.url.endsWith('/apps')
    ? { status: 422, body: { code: 'no_data_files' } }
    : { status: 200, body: { box: contents(true, 2), files: [], apps: [] } }));
  const boxes = await import('../src/canvas/boxes.ts');
  const asked: [string, string][] = [];
  boxes.setFolderAsk((boxId, name) => asked.push([boxId, name]));
  const { card } = boxes.buildBoxCard(contents(true, 2)) as unknown as { card: FakeNode };
  document_.body.append(card);
  const ask = button(card, 'Ask')!;
  const makeCard = button(card, 'Make a card')!;
  const makeApp = button(card, '✨ Make an app')!;
  assert.ok(ask && makeCard && makeApp, 'the three buttons');
  assert.deepEqual([ask.hidden, makeCard.hidden, makeApp.hidden], [false, false, false]);
  assert.ok(button(card, 'Brief me'), 'Brief me stays');
  // Ask: the Ask tab, scoped to this folder.
  ask.click();
  assert.deepEqual(asked, [[BOX, 'Q3 board pack']]);
  // Make a card: the confirm says what one card will be made; nothing is sent before its button.
  makeCard.click();
  const form = card.querySelector('form')!;
  assert.equal(form.hidden, false);
  assert.equal(form.dataset.making, 'card');
  assert.match(card.querySelector('.bx-card-preview')!.textContent, /standalone card, “Q3 board pack”, showing the most useful view of q3_metrics\.csv/);
  assert.equal(calls.filter((c) => c.method === 'POST').length, 0, 'no request before the confirm');
  form.click('submit');
  await settle();
  // Make an app: the formation prompt; its Build sends the full formation.
  makeApp.click();
  assert.equal(form.dataset.making, 'app');
  assert.equal(form.hidden, false);
  form.click('submit');
  await settle();
  const builds = calls.filter((c) => c.method === 'POST' && c.url === `/v1/boxes/${BOX}/apps`).map((c) => c.body);
  assert.deepEqual(builds, [{ one_card: true }, { purpose: 'the key metrics and explain them' }]);
  const { refusalText, BoxError } = await import('../src/canvas/box-api.ts');
  assert.match(refusalText(new BoxError('no_data_files', 422)), /no table \(CSV or XLSX\)/);
  assert.match(refusalText(new BoxError('data_copy_limit', 422)), /100,000 rows or 64 MiB\. No app was created/);
  assert.match(refusalText(new BoxError('data_copy_refused', 422)), /No app was created\. The unchanged original is still available to view or download/);
});

test('Make a card and Make an app send one idempotency key per confirm, reused when it is sent again (ADR-011 I4, APP-124 review)', async () => {
  const document_ = freshDocument();
  const calls = fakeFetch((call) => (call.url.endsWith('/apps')
    ? { status: 503, body: { code: 'unavailable' } }
    : { status: 200, body: { box: contents(true, 2), files: [], apps: [] } }));
  const boxes = await import('../src/canvas/boxes.ts');
  const { card } = boxes.buildBoxCard(contents(true, 2)) as unknown as { card: FakeNode };
  document_.body.append(card);
  const form = card.querySelector('form')!;
  const builds = () => calls.filter((c) => c.method === 'POST' && c.url === `/v1/boxes/${BOX}/apps`);
  button(card, 'Make a card')!.click();
  form.click('submit');
  await settle();
  assert.equal(form.hidden, false, 'a failed Make a card keeps its confirm open');
  form.click('submit');
  await settle();
  const [first, again] = builds();
  assert.match(first!.key ?? '', /^[0-9a-f]{32}$/, '128 random bits as 32 lowercase hex digits');
  assert.equal(again!.key, first!.key, 'sent again, the same key: the server answers the first result');
  // Make an app: its own key, kept when sent again; other words are another key.
  button(card, '✨ Make an app')!.click();
  form.click('submit');
  await settle();
  form.click('submit');
  await settle();
  const [, , app, appAgain] = builds();
  assert.deepEqual(app!.body, { purpose: 'the key metrics and explain them' });
  assert.match(app!.key ?? '', /^[0-9a-f]{32}$/);
  assert.notEqual(app!.key, first!.key, 'another confirm, another key');
  assert.equal(appAgain!.key, app!.key, 'Make an app sent again keeps its key');
  const purpose = card.all().find((node) => node.tagName === 'input' && (node.getAttribute('aria-label') ?? '').startsWith('What app'))!;
  purpose.value = 'a churn dashboard';
  form.click('submit');
  await settle();
  assert.deepEqual(builds()[4]!.body, { purpose: 'a churn dashboard' });
  assert.notEqual(builds()[4]!.key, app!.key, 'other words, another key');
  // Ask and the other routes carry no key.
  assert.ok(calls.filter((c) => !c.url.endsWith('/apps')).every((c) => c.key === undefined));
});

test('the Make buttons are the owner’s; a folder without files offers none of the three (APP-124)', async () => {
  freshDocument();
  fakeFetch(() => ({ status: 200, body: {} }));
  const boxes = await import('../src/canvas/boxes.ts');
  const shared = (boxes.buildBoxCard(contents(false, 1)) as unknown as { card: FakeNode }).card;
  assert.deepEqual(['Ask', 'Make a card', '✨ Make an app'].map((label) => button(shared, label)!.hidden), [false, true, true]);
  const empty = (boxes.buildBoxCard(contents(true, 0)) as unknown as { card: FakeNode }).card;
  assert.deepEqual(['Ask', 'Make a card', '✨ Make an app'].map((label) => button(empty, label)!.hidden), [true, true, true]);
  assert.equal(boxes.cardSummary(contents(true, 0)), 'This folder has no table (CSV or XLSX) to make a card from yet.');
});

test('Ask on a folder posts the question to the folder; its answer is exactly text and file rows (APP-124)', async () => {
  freshDocument();
  const answer = {
    text: 'q3_metrics.csv has 12 rows.',
    references: [{ ref: 'f1r1', file_id: FILE, file: 'q3_metrics.csv', sha256: 'sha256:x', sheet: null, row: 1, label: '2026-07-01 · North' }],
  };
  const calls = fakeFetch(() => ({ status: 200, body: answer }));
  const ask = await import('../src/canvas/ask.ts');
  const { folderAnswer } = await import('../src/canvas/box-api.ts');
  const scope = ask.folderScope(BOX, 'Q3 board pack')!;
  assert.equal(ask.scopeText(scope), 'Asking about 📁 Q3 board pack (its tables)');
  assert.equal(ask.folderScope('not-a-box', 'x'), null);
  await ask.ask('how many rows?', scope);
  assert.deepEqual(calls.map((c) => [c.method, c.url, c.body]), [['POST', `/v1/boxes/${BOX}/ask`, { question: 'how many rows?' }]]);
  // The closed shape: exactly {text, references}; a reference names its file and row.
  assert.deepEqual(folderAnswer(answer)?.references[0], { ...answer.references[0] });
  assert.equal(folderAnswer({ ...answer, chart: 'bar' }), null, 'any other key is not shown');
  assert.equal(folderAnswer({ text: 'x', references: [{ ref: 'f1r1', file_id: 'nope', file: 'a', row: 1 }] })?.references.length, 0);
  // A selected folder card scopes Ask to the folder.
  const document_ = freshDocument();
  const item = document_.createElement('div');
  const card = document_.createElement('article');
  card.setAttribute('class', 'cv-card bx-card');
  card.setAttribute('data-box', BOX);
  const title = document_.createElement('h2');
  title.setAttribute('class', 'bx-title');
  title.append('Q3 board pack');
  card.append(title);
  item.append(card);
  assert.deepEqual(ask.scopeOfItem(item as unknown as HTMLElement), scope);
});

test('folder uploads await real outcomes and count confirmed files, retaining partial failure details', async () => {
  const document_ = freshDocument();
  const pending: DeferredXHR[] = [];
  class DeferredXHR {
    status = 0;
    responseText = '';
    upload = { addEventListener() {} };
    listeners = new Map<string, (() => void)[]>();
    open() {}
    addEventListener(type: string, listener: () => void) { this.listeners.set(type, [...(this.listeners.get(type) ?? []), listener]); }
    send() { pending.push(this); }
    finish(status: number, body: unknown) {
      this.status = status;
      this.responseText = JSON.stringify(body);
      for (const listener of this.listeners.get('load') ?? []) listener();
    }
  }
  (globalThis as { XMLHttpRequest?: unknown }).XMLHttpRequest = DeferredXHR;
  fakeFetch(() => ({ status: 200, body: contents(true, 1) }));
  const { buildBoxCard } = await import('../src/canvas/boxes.ts');
  const built = buildBoxCard(contents(true, 0));
  const card = built.card as unknown as FakeNode;
  document_.body.append(card);
  let completed = false;
  const uploading = built.upload([
    new File(['first'], 'first.txt'), new File(['second'], 'second.txt'),
  ], 'land').then(result => { completed = true; return result; });
  await settle();
  assert.equal(completed, false, 'a pending upload must not produce a completion summary');
  assert.doesNotMatch(card.textContent, /files added|uploads confirmed/);
  pending[0]!.finish(201, { file: {
    file_id: FILE, name: 'first.txt', kind: 'text', bytes: 5, data: null,
  } });
  await settle();
  assert.equal(completed, false, 'the second upload is still pending');
  pending[1]!.finish(422, { code: 'unsupported_file' });
  assert.deepEqual(await uploading, { requested: 2, stored: 1 });
  assert.match(card.querySelector('.bx-card-status')!.textContent, /1 of 2 uploads confirmed/);
  assert.match(card.querySelector('.bx-progress')!.textContent, /second\.txt/);
  assert.doesNotMatch(card.textContent, /2 files added|Ask about them/);
});

test('oversize and full-folder uploads never report files added', async () => {
  freshDocument();
  const { buildBoxCard } = await import('../src/canvas/boxes.ts');
  fakeFetch(() => ({ status: 200, body: contents(true, 0) }));
  const oversize = new File(['x'], 'too-large.txt');
  Object.defineProperty(oversize, 'size', { value: 20 * 1024 * 1024 + 1 });
  const empty = buildBoxCard(contents(true, 0));
  assert.deepEqual(await empty.upload([oversize], 'land'), { requested: 1, stored: 0 });
  assert.match((empty.card as unknown as FakeNode).textContent, /too-large\.txt is over 20 MB and was not uploaded/);
  assert.match((empty.card as unknown as FakeNode).textContent, /0 of 1 uploads confirmed/);
  fakeFetch(() => ({ status: 200, body: contents(true, 20) }));
  const full = buildBoxCard(contents(true, 20));
  assert.deepEqual(await full.upload([new File(['a'], 'a.txt'), new File(['b'], 'b.txt')], 'land'), { requested: 2, stored: 0 });
  assert.match((full.card as unknown as FakeNode).textContent, /remaining files were not uploaded/);
  assert.match((full.card as unknown as FakeNode).textContent, /0 of 2 uploads confirmed/);
});

test('spreadsheet uploads keep partial and unknown outcomes visible without claiming nothing was stored', async () => {
  const document_ = freshDocument();
  fakeFetch(() => ({ status: 201, body: { box: {
    box_id: BOX, number: 1, name: 'Uploads', kind: 'uploads', owner: true, file_count: 0,
  } } }));
  class PartialXHR {
    status = 0;
    responseText = '';
    upload = { addEventListener() {} };
    listeners = new Map<string, (() => void)[]>();
    open() {}
    addEventListener(type: string, listener: () => void) { this.listeners.set(type, [...(this.listeners.get(type) ?? []), listener]); }
    send(form: FormData) {
      const file = form.get('file') as File;
      if (file.name === 'lost.csv') {
        for (const listener of this.listeners.get('error') ?? []) listener();
        return;
      }
      this.status = 201;
      this.responseText = JSON.stringify({ file: { file_id: FILE, name: file.name, kind: 'csv', bytes: file.size }, preview: { refused: 'unreadable' } });
      for (const listener of this.listeners.get('load') ?? []) listener();
    }
  }
  (globalThis as { XMLHttpRequest?: unknown }).XMLHttpRequest = PartialXHR;
  const { newUntitled } = await import('../src/canvas/boxes.ts');
  const partial = await newUntitled([new File(['bad syntax'], 'stored.csv'), new File(['a'], 'lost.csv')]);
  assert.match(partial, /stored\.csv is stored as a file/);
  assert.match(partial, /1 of 2 spreadsheet uploads confirmed/);
  assert.match(document_.getElementById('bx-notice')!.textContent, /lost\.csv/);
  assert.equal(button(document_.getElementById('bx-notice')!, 'Open original file'), undefined);
  assert.ok(document_.created.some(node => node.classes.has('bx-sheet') && node.dataset.file === FILE), 'a parser refusal leaves the original file card available');
  assert.equal(button(document_.getElementById('bx-notice')!, 'Make an app'), undefined);
  const unknown = await newUntitled([new File(['a'], 'lost.csv')]);
  assert.match(unknown, /0 of 1 spreadsheet uploads confirmed/);
  assert.doesNotMatch(unknown, /Nothing was stored/);
  assert.ok(button(document_.getElementById('bx-notice')!, 'Open Uploads'), 'unknown outcomes offer inspection before retry');
});

test('a file card owns Ask and Make an app, and Ask sends only that exact file', async () => {
  const document_ = freshDocument();
  const { fileCardFooter } = await import('../src/canvas/file-card.ts');
  const answer = {
    text: '<script>Amounts total 3.</script>',
    references: [{ ref: 'f1r1', file_id: FILE, file: 'q3_metrics.csv', sha256: 'sha256:x', sheet: null, row: 1, label: 'Amount 1' }],
  };
  const calls = fakeFetch(() => ({ status: 200, body: answer }));
  let built = 0;
  const footer = fileCardFooter(document_ as unknown as Document, BOX, appSheetFile, () => { built += 1; }) as unknown as FakeNode;
  document_.body.append(footer);
  const input = footer.querySelector('input')!;
  input.value = '  what is the total?  ';
  const form = footer.querySelector('form')!;
  form.click('submit'); form.click('submit');
  await settle();
  assert.deepEqual(calls.map(call => [call.url, call.body]), [[`/v1/boxes/${BOX}/ask`, { question: 'what is the total?', file_id: FILE }]]);
  assert.match(footer.textContent, /Amounts total 3/);
  assert.equal(footer.querySelector('script'), null, 'AI output is text, not executable markup');
  assert.match(footer.textContent, /Source rows \(1\)/);
  assert.equal(button(footer, 'Ask')!.disabled, false);
  assert.equal(button(footer, 'Make a card'), undefined);
  button(footer, 'Make an app')!.click();
  assert.equal(built, 1, 'the on-card action opens the upgrade path');
  const readOnly = fileCardFooter(document_ as unknown as Document, BOX, appSheetFile) as unknown as FakeNode;
  assert.equal(button(readOnly, 'Make an app'), undefined, 'read-only cards retain Ask without an owner action');
});

test('file Ask refuses references to a sibling file and retains an actionable input', async () => {
  const document_ = freshDocument();
  const { fileCardFooter } = await import('../src/canvas/file-card.ts');
  fakeFetch(() => ({ status: 200, body: {
    text: 'Sibling file secret', references: [{ref: 'f2r1', file_id: FILE2, file: 'other.xlsx', row: 1}],
  } }));
  const footer = fileCardFooter(document_ as unknown as Document, BOX, appSheetFile) as unknown as FakeNode;
  document_.body.append(footer);
  footer.querySelector('input')!.value = 'what is the total?';
  footer.querySelector('form')!.click('submit');
  await settle();
  assert.doesNotMatch(footer.textContent, /Sibling file secret/);
  assert.equal(button(footer, 'Ask')!.disabled, false);
  assert.equal(footer.querySelector('input')!.value, 'what is the total?');
});

test('the file-to-app sheet starts with a purpose and has no Keep as card or Add to app branch', async () => {
  const document_ = freshDocument();
  const calls = fakeFetch(() => ({ status: 200, body: {} }));
  const { openDataSheet } = await import('../src/canvas/data-sheet.ts');
  let placed = 0;
  openDataSheet(BOX, appSheetFile, appSheetPreview, undefined, false, undefined, {appOnly: true, placeApp: async () => { placed += 1; }});
  assert.match(document_.body.querySelector('.bx-modal-title')!.textContent, /Make an app · q3_metrics.csv/);
  assert.equal(document_.body.querySelector('.bx-extra')!.hidden, false);
  assert.deepEqual(document_.body.querySelector('.bx-modal-actions')!.querySelectorAll('button').map(node => node.textContent), ['Cancel', 'Build the app']);
  assert.equal(button(document_.body, 'Keep as card'), undefined);
  assert.equal(button(document_.body, 'Add to app…'), undefined);
  assert.equal(calls.length, 0);
  button(document_.body, 'Cancel')!.click();
  await settle();
  assert.equal(placed, 0);
  assert.equal(calls.length, 0, 'cancel keeps the source file without creating any data work or app');
  assert.equal(document_.body.querySelector('.bx-backdrop'), null);
});

test('file app retries reuse the exact intent/key and create no separate data card', async () => {
  const document_ = freshDocument();
  const { openDataSheet } = await import('../src/canvas/data-sheet.ts');
  let attempts = 0;
  const calls = fakeFetch(call => {
    if (!call.url.endsWith('/apps')) return {status: 200, body: {}};
    attempts++;
    return attempts === 1 ? {status: 503, body: {code: 'unavailable'}}
      : {status: 201, body: {app_id: APP, work_id: WORK, title: 'Metrics app', files: 1}};
  });
  const placed: unknown[] = [];
  let completed = 0;
  openDataSheet(BOX, appSheetFile, appSheetPreview, () => {completed++;}, false, undefined, {appOnly: true, placeApp: async card => {placed.push(card);}});
  const purpose = document_.body.querySelector('[aria-label="Purpose of the app"]')!;
  purpose.value = 'monthly metrics';
  document_.body.querySelector('[aria-label="Type of Amount"]')!.value = 'integer';
  const build = button(document_.body, 'Build the app')!;
  build.click(); build.click();
  await settle();
  assert.equal(completed, 0);
  assert.deepEqual(placed, []);
  assert.equal(build.disabled, false, 'failed creation remains retryable');
  assert.equal(purpose.disabled, true, 'a retry cannot silently change its original intent');
  purpose.value = 'a different app';
  build.click();
  await settle();
  const apps = calls.filter(call => call.url.endsWith('/apps'));
  assert.equal(apps.length, 2);
  assert.deepEqual(apps.map(call => call.body), Array.from({length: 2}, () => ({purpose: 'monthly metrics', file_ids: [FILE]})));
  assert.ok(apps[0]!.key);
  assert.equal(apps[0]!.key, apps[1]!.key);
  assert.deepEqual(calls.filter(call => call.url.endsWith('/data')).map(call => call.body), [{columns: [{id: 'amount', type: 'integer', nullable: false}], card: false}], 'registration is once and never creates another card');
  assert.deepEqual(placed, [{app_id: APP, work_id: WORK, title: 'Metrics app'}]);
  assert.equal(completed, 1);
});

test('a generated app waits for successful placement before replacing its file and retries placement without regeneration', async () => {
  const document_ = freshDocument();
  const { openDataSheet } = await import('../src/canvas/data-sheet.ts');
  const calls = fakeFetch(call => call.url.endsWith('/apps')
    ? {status: 201, body: {app_id: APP, work_id: WORK, title: 'Metrics app', files: 1}}
    : {status: 200, body: {}});
  const source = document_.createElement('article');
  source.setAttribute('class', 'bx-sheet');
  document_.body.append(source);
  let placement = 0, completed = 0;
  openDataSheet(BOX, appSheetFile, appSheetPreview, () => {completed++;}, false, undefined, {appOnly: true, placeApp: async () => {
    placement++;
    if (placement === 1) throw Error('placement unavailable');
    source.remove();
  }});
  const build = button(document_.body, 'Build the app')!;
  build.click();
  await settle();
  assert.equal(source.isConnected, true, 'a placement failure retains the original file card');
  assert.equal(completed, 0);
  assert.equal(build.disabled, false);
  build.click();
  await settle();
  assert.equal(source.isConnected, false, 'replacement belongs to the successful placement callback');
  assert.equal(completed, 1);
  assert.equal(calls.filter(call => call.url.endsWith('/apps')).length, 1, 'the created app is reused after placement failure');
  assert.equal(calls.filter(call => call.url.endsWith('/data')).length, 1);
});

test('closing a pending file app keeps its attempt locked until settlement and reopening reuses the saved app', async () => {
  const document_ = freshDocument();
  const { openDataSheet } = await import('../src/canvas/data-sheet.ts');
  type Attempt = import('../src/canvas/data-sheet.ts').DataSheetAppAttempt;
  const attempt: Attempt = {key: 'file-card-reopen-attempt', registered: false};
  let resolveApp!: (result: FetchAnswer) => void;
  const pending = new Promise<FetchAnswer>(resolve => {resolveApp = resolve;});
  const calls = fakeFetch(call => call.url.endsWith('/apps') ? pending : {status: 200, body: {}});
  let released = 0, placed = 0, completed = 0;
  const options = {
    appOnly: true, attempt,
    onRelease: () => {released++;},
    placeApp: async () => {placed++;},
  };
  openDataSheet(BOX, appSheetFile, appSheetPreview, () => {completed++;}, false, undefined, options);
  document_.body.querySelector('[aria-label="Purpose of the app"]')!.value = 'monthly metrics';
  document_.body.querySelector('[aria-label="Type of Amount"]')!.value = 'integer';
  button(document_.body, 'Build the app')!.click();
  await settle();
  assert.equal(calls.filter(call => call.url.endsWith('/apps')).length, 1);
  assert.equal(attempt.registered, true);
  document_.body.querySelector('[aria-label="Close (Esc)"]')!.click();
  assert.equal(document_.body.querySelector('.bx-backdrop'), null);
  assert.equal(released, 0, 'closing the modal does not release an unresolved creation attempt');
  resolveApp({status: 201, body: {app_id: APP, work_id: WORK, title: 'Metrics app', files: 1}});
  await settle();
  assert.equal(released, 1, 'the attempt unlocks once its pending request settles');
  assert.equal(placed, 0, 'closing before completion does not silently replace the source file');
  assert.equal(completed, 0);
  assert.equal(attempt.built?.app_id, APP, 'the generated result survives closing its dialog');
  openDataSheet(BOX, appSheetFile, appSheetPreview, () => {completed++;}, false, undefined, options);
  const purpose = document_.body.querySelector('[aria-label="Purpose of the app"]')!;
  assert.equal(purpose.value, 'monthly metrics');
  assert.equal(purpose.disabled, true);
  button(document_.body, 'Build the app')!.click();
  await settle();
  assert.equal(placed, 1);
  assert.equal(completed, 1);
  assert.equal(released, 2, 'successful replacement releases the reopened dialog once');
  assert.equal(calls.filter(call => call.url.endsWith('/apps')).length, 1, 'reopening places the saved app without another generation');
  assert.equal(calls.filter(call => call.url.endsWith('/data')).length, 1, 'reopening keeps the original data registration');
});

test('closing during app placement invalidates its callback before it can replace the file card', async () => {
  const document_ = freshDocument();
  const { openDataSheet } = await import('../src/canvas/data-sheet.ts');
  const calls = fakeFetch(call => call.url.endsWith('/apps')
    ? {status: 201, body: {app_id: APP, work_id: WORK, title: 'Metrics app', files: 1}}
    : {status: 200, body: {}});
  const source = document_.createElement('article');
  source.setAttribute('class', 'bx-sheet');
  document_.body.append(source);
  let resolvePlacement!: () => void;
  const pendingPlacement = new Promise<void>(resolve => {resolvePlacement = resolve;});
  let released = 0, placed = 0;
  const validity: boolean[] = [];
  openDataSheet(BOX, appSheetFile, appSheetPreview, undefined, false, undefined, {
    appOnly: true,
    onRelease: () => {released++;},
    placeApp: async (_card, isCurrent) => {
      validity.push(isCurrent());
      await pendingPlacement;
      validity.push(isCurrent());
      if (!isCurrent()) return false;
      placed++;
      source.remove();
      return true;
    },
  });
  button(document_.body, 'Build the app')!.click();
  await settle();
  assert.deepEqual(validity, [true], 'placement begins while its dialog is current');
  document_.body.querySelector('[aria-label="Close (Esc)"]')!.click();
  assert.equal(released, 0, 'closing during placement retains the attempt lock');
  assert.equal(source.isConnected, true);
  resolvePlacement();
  await settle();
  assert.deepEqual(validity, [true, false], 'the placement guard observes dismissal after its awaited work');
  assert.equal(placed, 0, 'dismissed placement cannot install the app');
  assert.equal(source.isConnected, true, 'the original file card stays on the canvas');
  assert.equal(released, 1, 'the lock releases after the guarded placement settles');
  assert.equal(calls.filter(call => call.url.endsWith('/apps')).length, 1);
  assert.equal(document_.body.querySelector('.bx-backdrop'), null);
});
