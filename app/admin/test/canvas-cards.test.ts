// APP-123: Ask and Shape after the demo. **Make it a card** previews a new
// card over an Ask answer's records and adds nothing without a click; a
// card made by you is stored on the server (`POST /v1/cards`) and judged
// again when it comes back; `@` mentions scope Ask and the scope chip is
// removable (removing it asks about the canvas); `/forma <words>` is a plain
// sentence; a folder's name (`/q3 <words>`, `/folder:q3 <words>`) runs its
// formation, verbs and apps first; a pinned answer keeps its text and
// receipts and never runs again.
import assert from 'node:assert/strict';
import { test } from 'node:test';

const APP = '00000000-0000-4000-8000-000000001681';
const WORK = '00000000-0000-4000-8000-000000001684';
const COLLECTION = '00000000-0000-4000-8000-000000001683';
const PUBLICATION = '00000000-0000-4000-8000-0000000016a1';
const BOX = '00000000-0000-4000-8000-0000000b0c01';
const id = (n: number) => `00000000-0000-4000-8000-00000000c00${n}`;

// --- A small fake DOM (no browser): enough for `el()`, the dock and the Library's rows ---
type Listener = (event: { type: string; target: FakeNode; stopPropagation(): void }) => void;
class FakeNode {
  attributes = new Map<string, string>();
  dataset: Record<string, string> = {};
  children: FakeNode[] = [];
  listeners = new Map<string, Listener[]>();
  hidden = false;
  value = '';
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
    const event = { type, target: this, stopped: false, stopPropagation() { this.stopped = true; } };
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

const money = (cents: string) => ({ type: 'money', minor_units: cents, currency: 'USD', minor_unit_exponent: 2 });
const record = (n: number, merchant: string, cents: string, status: string) => ({
  record_id: id(n), collection_id: COLLECTION, schema_id: 'expense_claim', version: 1, commit_sequence: n,
  committed_at: '2026-10-28T12:00:00Z', creator: { principal_id: 'p', label: 'Diego Ramos' }, updater: { principal_id: 'p', label: 'Diego Ramos' },
  values: [
    { field_id: 'amount', value: money(cents), display: `$${(Number(cents) / 100).toFixed(2)}` },
    { field_id: 'merchant', value: { type: 'text', value: merchant }, display: merchant },
    { field_id: 'status', value: { type: 'enum', value: status }, display: status },
  ],
});
const LISTING = {
  app_id: APP, work_id: WORK, publication_id: PUBLICATION, definition_digest: `sha256:${'a'.repeat(64)}`, work_version: 4, can_write: true,
  collections: [{
    collection_id: COLLECTION, schema_id: 'expense_claim', schema_version: 1, commands: [],
    fields: [
      { id: 'amount', type: 'money', nullable: false, writable: true },
      { id: 'merchant', type: 'text', nullable: false, writable: true },
      { id: 'status', type: 'enum', nullable: false, writable: true, values: ['submitted', 'approved', 'rejected'] },
    ],
  }],
  records: [record(1, 'Nobu', '8600', 'submitted'), record(2, 'Delta', '41200', 'approved'), record(3, 'Figma', '1500', 'rejected')],
  truncated: false,
};
const receipt = (n: number, reference: string) => ({
  kind: 'record', reference, app_id: APP, work_id: WORK, collection_id: COLLECTION, record_id: id(n), version: 1, commit_sequence: n,
  committed_at: '2026-10-28T12:00:00Z', publication_id: PUBLICATION, definition_digest: `sha256:${'a'.repeat(64)}`, work_version: 4,
});
/** Ask's answer to "where are the red invoices?": two records, each with its storage receipt. */
const ANSWER = {
  question: 'where are the red invoices?', text: 'Two need attention: Nobu ($86.00, submitted) and Figma ($15.00, rejected).',
  scope: { kind: 'card' as const, address: 'expenses.ledger' },
  references: [
    { ref: 'r1', app_id: APP, work_id: WORK, record_id: id(1), address: 'expenses', label: 'Nobu · $86.00 · submitted' },
    { ref: 'r3', app_id: APP, work_id: WORK, record_id: id(3), address: 'expenses', label: 'Figma · $15.00 · rejected' },
  ],
  receipts: [receipt(1, 'r1'), receipt(3, 'r3')],
};
const button = (root: FakeNode, label: string) => root.all().find((node) => node.tagName === 'button' && node.textContent === label);
const settle = () => new Promise((resolve) => setTimeout(resolve, 10));

test('Make it a card opens a Shape preview of the answer’s records; nothing is added without a click (APP-123)', async () => {
  const document_ = freshDocument();
  const stored: Record<string, unknown>[] = [];
  const calls = fakeFetch((call) => {
    if (call.method === 'GET' && call.url === `/v1/work/${APP}/${WORK}/records`) return { status: 200, body: LISTING };
    if (call.method === 'POST' && call.url === '/v1/cards') {
      stored.push(call.body!);
      return { status: 200, body: { saved: true, card: { card_id: call.body!.card_id, kind: 'card', app_id: APP, work_id: WORK, source: 'records', address: 'expenses.ledger',
        title: call.body!.title, spec: { from: { collection: COLLECTION }, view: call.body!.spec }, everyone: false, mine: true, can_share: true, created_at: '2026-10-31T12:00:00Z' } } };
    }
    return { status: 404, body: { code: 'not_found' } };
  });
  const ask = await import('../src/canvas/ask.ts');
  const shape = await import('../src/canvas/shape.ts');
  const { tableFromRecords, evaluate, cardView, chips } = await import('../src/canvas/talk-view.ts');
  // The answer's references are the record set: one list, two records.
  const set = ask.recordSet(ANSWER);
  assert.ok(typeof set !== 'string');
  if (typeof set === 'string') return;
  assert.deepEqual(set, { app: APP, work: WORK, collection: COLLECTION, records: [id(1), id(3)], address: 'expenses.ledger' });
  const view = { records: set.records };
  // The preview shows only those records, each with its receipt.
  const shown = evaluate(tableFromRecords(LISTING as never, COLLECTION), cardView(view, { stored: true })!);
  assert.ok(shown.kind === 'rows');
  if (shown.kind === 'rows') assert.deepEqual(shown.rows.map(({ row }) => [row.text.merchant, row.receipt?.record_id]), [['Nobu', id(1)], ['Figma', id(3)]]);
  assert.deepEqual(chips(view), ['The 2 records of an answer']);
  // Make it a card → a preview and its dock; Cancel adds nothing.
  await shape.previewNewCard({ ...set, title: ask.titleOf(ANSWER.question), view });
  const dock = document_.getElementById('cv-confirm')!;
  assert.ok(button(dock, 'Add as new card') && button(dock, 'Cancel'), 'the preview asks first');
  assert.deepEqual(calls.map((c) => `${c.method} ${c.url}`), [`GET /v1/work/${APP}/${WORK}/records`], 'only the records were read');
  button(dock, 'Cancel')!.click();
  await settle();
  assert.equal(stored.length, 0, 'Cancel stores nothing');
  // Again, then Add as new card: one stored card, the answer's record set over its records card.
  await shape.previewNewCard({ ...set, title: ask.titleOf(ANSWER.question), view });
  button(document_.getElementById('cv-confirm')!, 'Add as new card')!.click();
  await settle();
  assert.equal(stored.length, 1, 'stored on the click');
  const body = stored[0]!;
  assert.match(String(body.card_id), /^[0-9a-f-]{36}$/);
  assert.deepEqual({ ...body, card_id: '' }, {
    card_id: '', kind: 'card', app_id: APP, work_id: WORK, card: 'records', address: 'expenses.ledger', title: 'where are the red invoices?',
    collection: COLLECTION, spec: { records: [id(1), id(3)] },
  });
  // An answer over two lists is not one card; an answer with no records neither.
  assert.equal(typeof ask.recordSet({ ...ANSWER, receipts: [receipt(1, 'r1'), { ...receipt(3, 'r3'), collection_id: WORK }] }), 'string');
  assert.equal(typeof ask.recordSet({ ...ANSWER, references: [], receipts: [] }), 'string');
});

test('a stored card is judged again: the record set only on a stored card, its source closed (APP-123)', async () => {
  const { cardView } = await import('../src/canvas/talk-view.ts');
  const cards = await import('../src/canvas/cards.ts');
  assert.equal(cardView({ records: [id(1)] }), null, 'Shape’s own specs never carry a record set');
  assert.deepEqual(cardView({ records: [id(1)] }, { stored: true }), { records: [id(1)] });
  assert.equal(cardView({ records: ['r1'] }, { stored: true }), null);
  assert.equal(cardView({ records: [id(1), id(1)] }, { stored: true }), null);
  assert.equal(cardView({ presentation: 'number', receipts: false }, { stored: true }), null, 'receipts are never dropped');
  const card = {
    card_id: '00000000-0000-4000-8000-0000000c4a01', kind: 'card', app_id: APP, work_id: WORK, source: 'ledger', address: 'expenses.ledger', title: 'Red invoices',
    spec: { from: { collection: COLLECTION }, view: { filters: [{ field: 'status', op: 'in', value: ['submitted', 'rejected'] }] } },
    everyone: false, mine: true, can_share: true, created_at: '2026-10-31T12:00:00Z',
  };
  const stored = cards.storedCard(card)!;
  assert.ok(stored);
  assert.deepEqual(cards.madeView(stored)?.from, { collection: COLLECTION });
  assert.equal(cards.madeLine(stored), 'Made by you from /expenses.ledger · only you see it');
  assert.equal(cards.madeLine({ ...stored, mine: false, everyone: true }), 'From /expenses.ledger · added for everyone by the app’s owner');
  for (const bad of [{ ...card, card_id: '1' }, { ...card, kind: 'note' }, { ...card, address: '<b>' }, { ...card, title: '' }]) assert.equal(cards.storedCard(bad), null);
  for (const spec of [{ from: { collection: COLLECTION }, view: { css: 'x' } }, { from: { sql: 'x' }, view: { palette: 'green' } }, { view: { palette: 'green' } },
    { from: { view: 'by_status' }, view: { records: [id(1)] } }]) {
    assert.equal(cards.madeView({ ...stored, spec }), null, JSON.stringify(spec));
  }
  // A pinned answer: exactly what Ask answered, judged again.
  const request = cards.pinRequest('00000000-0000-4000-8000-0000000c4a02', ANSWER);
  assert.deepEqual(Object.keys(request).sort(), ['card_id', 'kind', 'question', 'receipts', 'references', 'scope', 'text']);
  const pin = cards.storedCard({ ...card, kind: 'answer', app_id: APP, work_id: WORK, source: '', spec: { question: ANSWER.question, text: ANSWER.text, scope: ANSWER.scope, references: ANSWER.references, receipts: ANSWER.receipts } })!;
  assert.deepEqual(cards.pinnedAnswer(pin)?.receipts, ANSWER.receipts, 'its receipts are kept');
  assert.equal(cards.madeView(pin), null, 'an answer is not a view');
});

test('a pinned answer is read-only: its text and receipts, labeled with the question and time; it never runs again (APP-123)', async () => {
  const document_ = freshDocument();
  const calls = fakeFetch(() => ({ status: 404, body: {} }));
  const ask = await import('../src/canvas/ask.ts');
  const cards = await import('../src/canvas/cards.ts');
  const pin = cards.storedCard({ card_id: '00000000-0000-4000-8000-0000000c4a03', kind: 'answer', app_id: APP, work_id: WORK, source: '', address: 'expenses.ledger',
    title: ANSWER.question, spec: { question: ANSWER.question, text: ANSWER.text, scope: ANSWER.scope, references: ANSWER.references, receipts: ANSWER.receipts },
    everyone: false, mine: true, can_share: false, created_at: '2026-10-31T12:00:00Z' })!;
  // No canvas host here: placing it renders nothing and calls nothing.
  assert.equal(ask.placePinnedAnswer(pin), true);
  assert.equal(calls.length, 0, 'a pinned answer never asks again');
  assert.equal(document_.ids.size, 1);
});

/** The Library's Expenses rows (its ledger is `expenses.ledger`, 3.2), as the canvas indexes them. */
async function library() {
  const document_ = freshDocument();
  const registry = await import('../src/canvas/command-registry.ts');
  const slash = await import('../src/canvas/slash.ts');
  const library = await import('../src/canvas/library.ts');
  registry.setAddresses(slash.parseCommands({ commands: [
    { slug: 'expenses', aliases: ['expense'], title: 'Expenses', purpose: 'File and decide expenses', kind: 'library', app_id: APP, record: true },
  ] }).map(slash.serverAddress));
  const row = library.buildAppRow(document_ as unknown as Document, {
    app: APP, work: WORK, number: '3', title: 'Expenses', slug: 'expenses',
    cards: [
      { id: `a-${WORK}`, number: '3.0', title: 'App card', kind: 'app' },
      { id: `c-${WORK}-total`, number: '3.1', title: 'Total', kind: 'number' },
      { id: `l-${WORK}-${COLLECTION}`, number: '3.2', title: 'Expense ledger', kind: 'ledger' },
    ],
  });
  const indexed = library.indexRow(row as unknown as HTMLElement)!;
  library.decorate(indexed, library.rowAddress(indexed, registry.addressList()));
  return { document_, registry };
}

test('@expenses.ledger scopes an Ask; removing the chip scopes it to the canvas (APP-123)', async () => {
  await library();
  const { mentionsOf } = await import('../src/canvas/talk-view.ts');
  assert.deepEqual(mentionsOf('@expenses.total why is it high?'), { mentions: ['expenses.total'], question: 'why is it high?' });
  assert.deepEqual(mentionsOf('why is @app:expenses so high'), { mentions: ['app:expenses'], question: 'why is so high' });
  assert.deepEqual(mentionsOf('mail joe@example.com about it'), { mentions: [], question: 'mail joe@example.com about it' });
  const bodies: Record<string, unknown>[] = [];
  fakeFetch((call) => {
    bodies.push(call.body ?? {});
    return { status: 200, body: { text: 'Two are late.', receipts: [], references: [] } };
  });
  const ask = await import('../src/canvas/ask.ts');
  await ask.ask('? @expenses.ledger which are late?');
  assert.equal(bodies.length, 1);
  assert.deepEqual(bodies[0], { question: 'which are late?', scope: { kind: 'card', address: 'expenses.ledger', works: [{ app_id: APP, work_id: WORK, address: 'expenses', collection: COLLECTION }] } });
  assert.equal(ask.scopeText(ask.askScope()!), 'Asking about /expenses.ledger', 'the chip shows the mention');
  // The namespaced app form: every record of the app.
  await ask.ask('@app:expenses how many');
  assert.deepEqual(bodies[1]?.scope, { kind: 'app', address: 'expenses', works: [{ app_id: APP, work_id: WORK, address: 'expenses' }] });
  // A mention that names nothing asks nothing.
  await ask.ask('@nothing-here why');
  await ask.ask('@expenses @app:expenses why');
  assert.equal(bodies.length, 2, 'no request for an unknown or a second mention');
  // × on the chip: the canvas (nothing is on this canvas, so nothing is asked).
  ask.clearScope();
  assert.equal(ask.askScope()?.kind, 'canvas');
  await ask.ask('how many');
  assert.equal(bodies.length, 2);
});

test('/forma <words> is exactly a plain sentence (APP-123)', async () => {
  freshDocument();
  const calls = fakeFetch(() => ({ status: 503, body: { code: 'unavailable' } }));
  const talk = await import('../src/canvas/talk.ts');
  const actions = await import('../src/canvas/actions.ts');
  const said: string[] = [];
  await actions.submitUtterance('track client dinners over $200', (m) => said.push(m));
  await talk.FORMA_COMMAND.run('track client dinners over $200', (m) => said.push(m));
  assert.equal(calls.length, 2);
  assert.deepEqual(calls[1], calls[0], 'the same request');
  assert.equal(said[said.length - 1], said[said.length / 2 - 1], 'the same outcome');
  await talk.FORMA_COMMAND.run('', (m) => said.push(m));
  assert.equal(calls.length, 2, 'no words, no request');
});

test('/q3 <words> and /folder:q3 <words> run formation; verbs and apps answer first (APP-123)', async () => {
  const { registry } = await library();
  const boxes = await import('../src/canvas/boxes.ts');
  const box = (name: string, number: number, boxId = BOX) => ({ box_id: boxId, name, number, kind: 'folder', owner: true, file_count: 2 }) as never;
  const list = [box('Q3', 1), box('Focus', 2, '00000000-0000-4000-8000-0000000b0c02'), box('Expenses', 3, '00000000-0000-4000-8000-0000000b0c03'), box('Q3 board pack', 4, '00000000-0000-4000-8000-0000000b0c04')];
  registry.setFolderResolver((name) => boxes.folderRunnable(name, list));
  assert.equal(boxes.folderSlug('Q3 board pack'), 'q3-board-pack');
  assert.equal(registry.findCommand('q3')?.title, 'Formation from Q3');
  assert.equal(registry.findCommand('folder:q3')?.title, 'Formation from Q3');
  assert.equal(registry.findCommand('q3-board-pack')?.title, 'Formation from Q3 board pack');
  assert.equal(registry.findCommand('folder:4')?.title, 'Formation from Q3 board pack');
  // A verb wins over a folder named like one (reached as /folder:focus); an app over a folder.
  assert.equal(registry.findCommand('focus')?.title, 'Formation from Focus', 'no /focus verb registered yet: the folder answers');
  registry.registerCommands([{ slug: 'focus', title: 'Focus', purpose: 'Focus the selection', run: () => undefined }]);
  assert.equal(registry.findCommand('focus')?.title, 'Focus');
  assert.equal(registry.findCommand('folder:focus')?.title, 'Formation from Focus');
  assert.equal(registry.findCommand('expenses')?.title, 'Expenses');
  // Running it builds from the folder's files: the formation request names the folder and the words.
  const calls = fakeFetch(() => ({ status: 422, body: { code: 'could_not_build' } }));
  const said: string[] = [];
  await registry.findCommand('q3')!.run('formation build a metrics app', (m) => said.push(m));
  await registry.findCommand('folder:q3')!.run('build a metrics app', (m) => said.push(m));
  const builds = calls.filter((c) => c.method === 'POST' && c.url === `/v1/boxes/${BOX}/apps`);
  assert.deepEqual(builds.map((c) => c.body), [{ purpose: 'build a metrics app' }, { purpose: 'build a metrics app' }]);
});
