// APP-124 (start from your data): the empty canvas's third choice, "Drop
// files or upload", makes a folder holding the files; a folder with files
// offers Ask (the Ask tab scoped to the folder: `POST /v1/boxes/{box}/ask`),
// Make a card (its confirm first, then `POST /v1/boxes/{box}/apps
// {"one_card": true}`) and Make an app (the formation confirm, then the
// full formation); a folder answer is exactly `{text, references}`, each
// reference a file and row.
import assert from 'node:assert/strict';
import { test } from 'node:test';

const BOX = '00000000-0000-4000-8000-0000000b0c01';
const FILE = '00000000-0000-4000-8000-0000000f0c01';
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
  createElement(tag: string) { return new FakeNode(this, tag); }
  createElementNS(_ns: string, tag: string) { return new FakeNode(this, tag); }
  createTextNode(text: string) { const node = new FakeNode(this, '#text'); node.text = text; return node; }
  getElementById(id: string) { return this.ids.get(id) ?? null; }
  querySelector(selector: string) { return this.body.querySelector(selector); }
  querySelectorAll(selector: string) { return this.body.querySelectorAll(selector); }
  addEventListener() { /* no global keys here */ }
}

function freshDocument(): FakeDocument {
  const document_ = new FakeDocument();
  const dock = document_.createElement('div');
  dock.setAttribute('id', 'cv-confirm');
  document_.body.append(dock);
  (globalThis as { document?: unknown }).document = document_;
  return document_;
}

type Call = { url: string; method: string; body: Record<string, unknown> | null };
function fakeFetch(answer: (call: Call) => { status: number; body: unknown }): Call[] {
  const calls: Call[] = [];
  (globalThis as { fetch?: unknown }).fetch = async (url: string, init: { method?: string; body?: string } = {}) => {
    const call = { url, method: init.method ?? 'GET', body: init.body ? JSON.parse(init.body) as Record<string, unknown> : null };
    calls.push(call);
    const { status, body } = answer(call);
    return { ok: status < 300, status, json: async () => body };
  };
  return calls;
}


const button = (root: FakeNode, label: string) => root.all().find((node) => node.tagName === 'button' && node.textContent === label);
const settle = () => new Promise((resolve) => setTimeout(resolve, 10));
const contents = (owner: boolean, files: number) => ({
  box: { box_id: BOX, number: 1, name: 'Q3 board pack', kind: 'box', app_id: null, owner, file_count: files, created_at: '2026-10-31T00:00:00Z' },
  files: Array.from({ length: files }, (_, n) => ({
    file_id: FILE, name: n ? `notes-${n}.pdf` : 'q3_metrics.csv', kind: n ? 'pdf' : 'csv', media_type: 'text/csv', bytes: 900, sha256: 'sha256:x',
    created_at: '2026-10-31T00:00:00Z', content_url: `/v1/boxes/${BOX}/files/${FILE}/content`, data: null,
  })),
  apps: [],
}) as never;

test('the empty canvas offers "Drop files or upload": the picked files make a new folder (APP-124)', async () => {
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
  assert.equal(document_.activeElement, bar, 'Describe what you need puts the caret in the bar');
  pick.click();
  assert.equal(picked, 1, 'the button opens the file picker (wired once)');
  assert.equal(calls.length, 0, 'nothing is created before files are picked');
  input.files = [{ name: 'q3_metrics.csv', size: 900 }];
  input.click('change');
  await settle();
  const created = calls.filter((c) => c.method === 'POST' && c.url === '/v1/boxes');
  assert.deepEqual(created.map((c) => c.body), [{ name: 'Untitled folder' }], 'a new folder holds the files');
  assert.equal(input.value, '');
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
  assert.match(card.querySelector('.bx-card-preview')!.textContent, /“Q3 board pack”, with one card: the most useful view of q3_metrics\.csv/);
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
