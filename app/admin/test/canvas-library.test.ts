// APP-120: one canvas model. The Library has two sections, Apps (categories
// → apps → cards) and Widgets (Weather, Calculator, Clock, Note, Folder; the
// Folder row expands to the member's folders). Every row has an on-canvas
// dot, app rows read "n of m cards", and the pane re-renders from the
// board's one `canvas:changed` event. Cards are addressed `app.card` or by
// number. The address chip (`/expenses`) is one component wherever an app
// appears (Library row, app card header, the bar's hint, the confirm card),
// and clicking it puts "/expenses " in the bar.
import assert from 'node:assert/strict';
import { test } from 'node:test';

const EXPENSE_APP = '00000000-0000-4000-8000-000000001681';
const EXPENSE_WORK = '00000000-0000-4000-8000-000000001682';
const PTO_APP = '00000000-0000-4000-8000-000000001701';
const PTO_WORK = '00000000-0000-4000-8000-000000001702';
const COLLECTION = '00000000-0000-4000-8000-000000001683';

// --- A small fake DOM (no browser): enough for `el()` and the Library's rows ---

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
  replaceWith(node: FakeNode) { this.after(node); this.remove(); }
  remove() { this.parent?.detach(this); }
  addEventListener(type: string, listener: Listener) { this.listeners.set(type, [...(this.listeners.get(type) ?? []), listener]); }
  dispatchEvent(event: { type: string }) { this.click(event.type); return true; }
  click(type = 'click') {
    const event = { type, target: this, stopped: false, stopPropagation() { this.stopped = true; } };
    for (let node: FakeNode | null = this; node && !event.stopped; node = node.parent) for (const listener of node.listeners.get(type) ?? []) listener(event);
  }
  focus() { this.ownerDocument.activeElement = this; }
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
  listeners = new Map<string, ((event: unknown) => void)[]>();
  createElement(tag: string) { return new FakeNode(this, tag); }
  createTextNode(text: string) { const node = new FakeNode(this, '#text'); node.text = text; return node; }
  getElementById(id: string) { return this.ids.get(id) ?? null; }
  querySelectorAll(selector: string) { return [...this.ids.values()].flatMap((node) => node.all()).filter((node, at, list) => list.indexOf(node) === at && node.matches(selector)); }
  addEventListener(type: string, listener: (event: unknown) => void) { this.listeners.set(type, [...(this.listeners.get(type) ?? []), listener]); }
  dispatchEvent(event: { type: string }) { for (const listener of this.listeners.get(event.type) ?? []) listener(event); return true; }
}

const SERVER = {
  commands: [
    { slug: 'expenses', aliases: ['expense', 'claim'], title: 'Expenses', purpose: 'File and decide expenses', kind: 'library', app_id: EXPENSE_APP, record: true,
      hint: 'file a claim or approve one', examples: ['/expenses $86 at Nobu, client dinner, Sarah approves', '/expenses'] },
    { slug: 'time-off', aliases: ['pto', 'leave', 'vacation'], title: 'Time off', purpose: 'Book time off', kind: 'library', app_id: PTO_APP, record: true,
      hint: 'dates and reason', examples: ['/time-off Nov 24–26 for Thanksgiving'] },
  ],
};

async function setup() {
  const document_ = new FakeDocument();
  (globalThis as { document?: unknown }).document = document_;
  const input = document_.createElement('input');
  input.setAttribute('id', 'intent-input');
  const registry = await import('../src/canvas/command-registry.ts');
  const slash = await import('../src/canvas/slash.ts');
  const library = await import('../src/canvas/library.ts');
  registry.setAddresses(slash.parseCommands(SERVER).map(slash.serverAddress));
  const doc = document_ as unknown as Document;
  // Expenses is app 3 (as the server renders it, before the rename: `/expense`); Time off is app 4.
  const expensesRow = library.buildAppRow(doc, {
    app: EXPENSE_APP, work: EXPENSE_WORK, number: '3', title: 'Expenses', slug: 'expense',
    cards: [
      { id: `a-${EXPENSE_WORK}`, number: '3.0', title: 'App card', kind: 'app' },
      { id: `c-${EXPENSE_WORK}-by_status`, number: '3.1', title: 'Past claims by status', kind: 'number' },
      { id: `l-${EXPENSE_WORK}-${COLLECTION}`, number: '3.2', title: 'Expense ledger', kind: 'ledger' },
      { id: `f-${EXPENSE_WORK}-${COLLECTION}`, number: '3.3', title: 'New expense_claim record', kind: 'form' },
    ],
  });
  const ptoRow = library.buildAppRow(doc, {
    app: PTO_APP, work: PTO_WORK, number: '4', title: 'Time off', slug: 'time-off',
    cards: [{ id: `a-${PTO_WORK}`, number: '4.0', title: 'App card', kind: 'app' }, { id: `r-${PTO_WORK}-${COLLECTION}`, number: '4.1', title: 'Requests', kind: 'records' }],
  });
  const expenses = library.indexRow(expensesRow as unknown as HTMLElement)!;
  const pto = library.indexRow(ptoRow as unknown as HTMLElement)!;
  const list = registry.addressList();
  library.decorate(expenses, library.rowAddress(expenses, list));
  library.decorate(pto, library.rowAddress(pto, list));
  return { document_, doc, input, registry, slash, library, expenses, pto, expensesRow: expensesRow as unknown as FakeNode };
}

test('/card expense.ledger, /card expenses.ledger and /card 3.2 name the same card; /card lists app.card rows with numbers (APP-120)', async () => {
  const { library, expenses } = await setup();
  const ledger = expenses.cards.find((card) => card.kind === 'ledger')!;
  assert.equal(ledger.slug, 'ledger');
  assert.equal(library.cardAddress(ledger), 'expenses.ledger');
  assert.deepEqual(expenses.cards.map((card) => card.slug), ['app', 'by-status', 'ledger', 'new']);
  for (const query of ['expense.ledger', 'expenses.ledger', 'claim.ledger', '3.2', '3.ledger', 'expenses.led']) {
    assert.equal(library.findCards(query)[0]?.id, ledger.id, query);
  }
  assert.equal(library.findCards('time-off.records')[0]?.number, '4.1');
  assert.equal(library.findCards('pto.records')[0]?.number, '4.1');
  // `/card ` suggestions: the app.card name with its number.
  const [first] = library.cardCompletions('expenses.l');
  assert.deepEqual(first, { badge: '3.2', title: 'expenses.ledger', detail: 'Expense ledger · Expenses', value: '/card expenses.ledger' });
  assert.ok(library.cardCompletions('').every((completion) => /^[a-z0-9-]+\.[a-z0-9-]+$/.test(completion.title)));
  assert.equal(library.cardCompletions('3.2', 'focus')[0]?.value, '/focus expenses.ledger');
  // `/app 3` and `/app expense` find the same app.
  assert.equal(library.findApps('3')[0], expenses);
  assert.equal(library.findApps('expense')[0], expenses);
  assert.equal(library.findApps('pto')[0]?.title, 'Time off');
});

test('Library → Widgets lists the five widgets; Folder expands to the member\'s folders; dots and "n of m cards" follow canvas:changed (APP-120)', async () => {
  const { doc, document_, library, expenses, expensesRow } = await setup();
  const board = await import('../src/canvas/board.ts');
  const section = library.buildWidgets(doc) as unknown as FakeNode;
  const titles = section.querySelectorAll('.cv-lib-app-title').map((node) => node.textContent);
  assert.deepEqual(titles, ['Weather', 'Calculator', 'Clock', 'Note', 'Folder']);
  assert.deepEqual(section.querySelectorAll('.cv-addr-chip').map((node) => node.textContent), ['/weather', '/calculator', '/clock', '/note', '/folder']);
  // The canvas, as the board would report it (the Library keeps no canvas state).
  const placed = new Set<string>();
  const snapshot: Parameters<typeof library.renderWidgets>[1] = {
    counts: { weather: 0, calculator: 0, clock: 0 },
    notes: [{ id: 'n1', title: 'Call the auditor', on: false }],
    folders: [{ id: 'b1', number: 1, title: 'Q3 board pack', detail: '3 files', on: false }, { id: 'b2', number: 2, title: 'Uploads', detail: '1 file', on: false }],
  };
  let renders = 0;
  library.watchCanvas(doc, () => {
    renders += 1;
    library.syncLibrary((card) => placed.has(card.id), () => snapshot);
    library.renderWidgets(section as unknown as HTMLElement, snapshot);
  });
  const settle = () => new Promise((resolve) => setTimeout(resolve, 0));
  const dot = (node: FakeNode | null) => node?.querySelector('[data-lib-dot]')?.getAttribute('data-on');
  const row = (kind: string) => section.querySelector(`[data-widget="${kind}"]`)!;
  const status = () => expensesRow.querySelector('[data-lib-on]')?.textContent;

  board.emitCanvasChanged(doc);
  await settle();
  assert.equal(renders, 1);
  assert.equal(status(), '0 of 4 cards');
  assert.equal(dot(expensesRow), 'false', 'hollow');
  assert.equal(expensesRow.querySelector('[data-lib-all]')?.textContent, 'Add');
  // The Folder row lists the member's folders (and "+ New folder").
  const folders = row('folder').querySelector('[data-widget-list="folder"]')!;
  assert.deepEqual(folders.querySelectorAll('.cv-lib-card-title').map((node) => node.textContent), ['Q3 board pack', 'Uploads', '+ New folder']);
  assert.equal(row('folder').querySelector('[data-lib-on]')?.textContent, '0 of 2 on canvas');
  assert.equal(row('weather').querySelector('[data-widget-toggle="weather"]')?.textContent, 'Add');

  // Add a card, a weather widget and a folder: one event re-renders every row.
  placed.add(expenses.cards[2]!.id);
  snapshot.counts.weather = 1;
  snapshot.folders[0]!.on = true;
  board.emitCanvasChanged(doc);
  board.emitCanvasChanged(doc);
  await settle();
  assert.equal(renders, 2, 'events in one turn render once');
  assert.equal(status(), '1 of 4 cards');
  assert.equal(dot(expensesRow), 'true', 'filled');
  assert.equal(expenses.cards[2]!.row.querySelector('[data-lib-action]')?.textContent, 'Remove');
  assert.equal(dot(row('weather')), 'true');
  assert.equal(row('weather').querySelector('[data-widget-toggle="weather"]')?.textContent, 'Remove');
  assert.equal(dot(row('clock')), 'false');
  assert.equal(dot(row('folder')), 'true');
  assert.equal(row('folder').querySelector('[data-lib-on]')?.textContent, '1 of 2 on canvas');
  assert.equal(folders.querySelector('[data-folder="b1"]')?.querySelector('[data-lib-action]')?.textContent, 'Remove');

  // Every card on: the app row's toggle is Remove. Then remove everything again.
  for (const card of expenses.cards) placed.add(card.id);
  board.emitCanvasChanged(doc);
  await settle();
  assert.equal(status(), '4 of 4 cards');
  assert.equal(expensesRow.querySelector('[data-lib-all]')?.textContent, 'Remove');
  placed.clear();
  snapshot.counts.weather = 0;
  snapshot.folders[0]!.on = false;
  board.emitCanvasChanged(doc);
  await settle();
  assert.equal(status(), '0 of 4 cards');
  assert.equal(dot(row('weather')), 'false');
  assert.equal(dot(row('folder')), 'false');
  assert.deepEqual(library.appStatus(1, 1), { dot: 'filled', label: '1 of 1 card', action: 'Remove' });
  assert.equal(document_.listeners.get('canvas:changed')?.length, 1);
});

test('the address chip: Library row, app card header, bar hint and confirm card; a click fills the bar (APP-120)', async () => {
  const { doc, document_, input, expensesRow } = await setup();
  const chips = await import('../src/canvas/address-chip.ts');
  const { commandName } = await import('../src/canvas/actions.ts');
  // (1) The Library row: the canonical address in grey next to the title, and "also /expense, /claim".
  const chip = expensesRow.querySelector('.cv-addr-chip')!;
  assert.equal(chip.textContent, '/expenses');
  assert.equal(chip.tagName, 'button');
  assert.equal(expensesRow.querySelector('.cv-addr-also')?.textContent, 'also /expense, /claim');
  assert.equal(expensesRow.querySelector('.cv-lib-slug'), chip, 'the server\'s plain /expense became the chip');
  assert.equal(expensesRow.querySelector('.cv-lib-cardname')?.textContent, 'expenses.app');
  chip.click();
  assert.equal(input.value, '/expenses ');
  assert.equal(document_.activeElement, input);
  // (2) The app card's header on the canvas.
  input.value = '';
  const card = document_.createElement('article');
  card.setAttribute('class', 'cv-card cv-app');
  card.dataset.app = EXPENSE_APP;
  card.dataset.work = EXPENSE_WORK;
  const title = document_.createElement('h2');
  title.textContent = 'Expense claims';
  card.append(title);
  chips.decorateAppCard(card as unknown as HTMLElement);
  chips.decorateAppCard(card as unknown as HTMLElement);
  const headChips = card.querySelectorAll('.cv-addr-chip');
  assert.equal(headChips.length, 1, 'idempotent');
  assert.equal(card.children[1], headChips[0], 'right after the title');
  assert.equal(headChips[0]!.textContent, '/expenses');
  headChips[0]!.click();
  assert.equal(input.value, '/expenses ');
  // (3) The bar's hint while typing an address.
  const line = chips.addressLine(doc, { slug: 'expenses', title: 'Expenses', hint: 'file a claim or approve one' }) as unknown as FakeNode;
  assert.equal(line.textContent, '/expenses — Expenses · file a claim or approve one');
  assert.equal(line.querySelector('.cv-addr-chip')?.tagName, 'code', 'the plain chip: the bar already has it');
  // (4) The confirm card: the governed command by name, and "via /expenses".
  assert.equal(chips.addressFor(EXPENSE_APP, EXPENSE_WORK)?.slug, 'expenses');
  const values = [{ field_id: 'amount', label: 'Amount', display: '$86.00', value: 8600 }];
  assert.equal(commandName({ action: 'create', command_id: 'submit', schema_id: 'expense_claim', values }), 'Submit claim');
  assert.equal(commandName({ action: 'decide', command_id: 'decide', schema_id: 'expense_claim', values: [{ field_id: 'status', label: 'Status', display: 'approved', value: { type: 'enum', value: 'approved' } }] }), 'Approve claim');
  assert.equal(commandName({ action: 'decide', command_id: 'decide', schema_id: 'access_request', values: [{ field_id: 'status', label: 'Status', display: 'denied', value: 'denied' }] }), 'Deny request');
});
