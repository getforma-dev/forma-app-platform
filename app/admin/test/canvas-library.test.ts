// Whole-work Library placement, with metadata retained for naming and Ask. The address chip (`/expenses`) is one component wherever an app
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
  disabled = false;
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

test('metadata addresses stay stable for naming and Ask, but Apps expose no /card placement candidates', async () => {
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
  assert.deepEqual(library.cardCompletions('expenses.l'), [], 'app components are not placeable');
  assert.deepEqual(library.findStandaloneWorks('3.2'), []);
  assert.equal(library.cardCompletions('expenses', 'focus')[0]?.value, '/focus expenses', 'Focus offers a whole app');
  // `/app 3` and `/app expense` find the same app.
  assert.equal(library.findApps('3')[0], expenses);
  assert.equal(library.findApps('expense')[0], expenses);
  assert.equal(library.findApps('pto')[0]?.title, 'Time off');
});

test('Library → Widgets lists the five widgets; Folder expands to the member\'s folders; whole-work view status follows canvas:changed (APP-120)', async () => {
  const { doc, document_, library, expenses, expensesRow } = await setup();
  const board = await import('../src/canvas/board.ts');
  const section = library.buildWidgets(doc) as unknown as FakeNode;
  const titles = section.querySelectorAll('.cv-lib-app-title').map((node) => node.textContent);
  assert.deepEqual(titles, ['Weather', 'Calculator', 'Clock', 'Note', 'Folder']);
  assert.deepEqual(section.querySelectorAll('.cv-addr-chip').map((node) => node.textContent), ['/weather', '/calculator', '/clock', '/note', '/folder']);
  // The canvas, as the board would report it (the Library keeps no canvas state).
  const placed = new Map<string, { mode: 'face' | 'stack' | 'open' }>();
  const snapshot: Parameters<typeof library.renderWidgets>[1] = {
    counts: { weather: 0, calculator: 0, clock: 0 },
    notes: [{ id: 'n1', title: 'Call the auditor', on: false }],
    folders: [{ id: 'b1', number: 1, title: 'Q3 board pack', detail: '3 files', on: false }, { id: 'b2', number: 2, title: 'Uploads', detail: '1 file', on: false }],
  };
  let renders = 0;
  library.watchCanvas(doc, () => {
    renders += 1;
    library.syncLibrary((work) => placed.get(work) ?? null, () => snapshot);
    library.renderWidgets(section as unknown as HTMLElement, snapshot);
  });
  const settle = () => new Promise((resolve) => setTimeout(resolve, 0));
  const dot = (node: FakeNode | null) => node?.querySelector('[data-lib-dot]')?.getAttribute('data-on');
  const row = (kind: string) => section.querySelector(`[data-widget="${kind}"]`)!;
  const status = () => expensesRow.querySelector('[data-lib-on]')?.textContent;

  board.emitCanvasChanged(doc);
  await settle();
  assert.equal(renders, 1);
  assert.equal(status(), '');
  assert.equal(dot(expensesRow), 'false', 'hollow');
  assert.equal(expensesRow.querySelector('[data-lib-all]')?.textContent, 'Add');
  // The Folder row lists the member's folders (and "+ New folder").
  const folders = row('folder').querySelector('[data-widget-list="folder"]')!;
  assert.deepEqual(folders.querySelectorAll('.cv-lib-card-title').map((node) => node.textContent), ['Q3 board pack', 'Uploads', '+ New folder']);
  assert.equal(row('folder').querySelector('[data-lib-on]')?.textContent, '0 of 2 on canvas');
  assert.equal(row('weather').querySelector('[data-widget-toggle="weather"]')?.textContent, 'Add');
  assert.equal(row('note').querySelector('[data-lib-on]')?.textContent, '0 of 1 on canvas');
  assert.equal(library.widgetStatus('note', { ...snapshot, notes: [] }).label, '', 'empty notes need no 0 of 0 count');

  // Add a card, a weather widget and a folder: one event re-renders every row.
  placed.set(expenses.work, { mode: 'face' });
  snapshot.counts.weather = 1;
  snapshot.folders[0]!.on = true;
  board.emitCanvasChanged(doc);
  board.emitCanvasChanged(doc);
  await settle();
  assert.equal(renders, 2, 'events in one turn render once');
  assert.equal(status(), 'Card view');
  assert.equal(dot(expensesRow), 'true', 'filled');
  assert.equal(expensesRow.querySelector('[data-lib-remove]')?.hidden, false);
  assert.equal(dot(row('weather')), 'true');
  assert.equal(row('weather').querySelector('[data-widget-toggle="weather"]')?.textContent, 'Remove');
  assert.equal(dot(row('clock')), 'false');
  assert.equal(dot(row('folder')), 'true');
  assert.equal(row('folder').querySelector('[data-lib-on]')?.textContent, '1 of 2 on canvas');
  assert.equal(folders.querySelector('[data-folder="b1"]')?.querySelector('[data-lib-action]')?.textContent, 'Remove');

  // Switching the same work changes its mode, never a partial count.
  placed.set(expenses.work, { mode: 'stack' });
  board.emitCanvasChanged(doc);
  await settle();
  assert.equal(status(), 'Browse view');
  assert.equal(expensesRow.querySelector('[data-lib-all]')?.textContent, 'Add');
  placed.clear();
  snapshot.counts.weather = 0;
  snapshot.folders[0]!.on = false;
  board.emitCanvasChanged(doc);
  await settle();
  assert.equal(status(), '');
  assert.equal(dot(row('weather')), 'false');
  assert.equal(dot(row('folder')), 'false');
  assert.deepEqual(library.appStatus('open'), { dot: 'filled', label: 'Open view' });
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

// --- APP-120 gaps, fixed with APP-121 ---

test('the app card on the canvas gets its /expenses chip and its display name even when it was placed without data-app (APP-120 gap)', async () => {
  const { doc, document_, library } = await setup();
  const board = document_.createElement('div');
  board.setAttribute('id', 'cv-board');
  const card = document_.createElement('article');
  card.setAttribute('class', 'cv-card cv-app');
  card.setAttribute('data-card', `a-${EXPENSE_WORK}`);
  const title = document_.createElement('h2');
  title.textContent = 'Expense claims';
  card.append(title);
  board.append(card);
  assert.equal(card.dataset.app, undefined, 'as the library\'s placement left it before the fix');
  library.labelCanvasCards(doc);
  assert.equal(card.dataset.app, EXPENSE_APP);
  const chips = card.querySelectorAll('.cv-addr-chip');
  assert.equal(chips.length, 1);
  assert.equal(chips[0]!.textContent, '/expenses');
  assert.equal(title.textContent, 'Expenses', 'the display name, not the stored definition title');
  library.labelCanvasCards(doc);
  assert.equal(card.querySelectorAll('.cv-addr-chip').length, 1, 'idempotent');
});

// --- APP-121: naming ---

const LABELED = {
  commands: [
    { slug: 'team-spend', aliases: ['expenses', 'expense', 'claim', 'exp'], title: 'Team spend', purpose: 'Our team spend', kind: 'library', app_id: EXPENSE_APP, record: true,
      hint: 'file a claim or approve one', examples: ['/team-spend $86 at Nobu', '/team-spend'],
      description: 'Our team spend', kept: [], mine: ['exp'], can_label: true,
      cards: [{ card: 'ledger', name: 'Q3 spend', description: 'The Q3 ledger', my_name: null, mine: ['q3'] }] },
    { slug: 'time-off', aliases: ['pto', 'leave', 'vacation'], title: 'Time off', purpose: 'Book time off', kind: 'library', app_id: PTO_APP, record: true,
      hint: 'dates and reason', examples: ['/time-off Nov 24–26'], can_label: false, cards: [] },
  ],
};

async function labeled() {
  const context = await setup();
  const { registry, slash, library, expenses, pto } = context;
  registry.setAddresses(slash.parseCommands(LABELED).map(slash.serverAddress));
  const list = registry.addressList();
  // Every indexed row (earlier tests' rows stay in the module), as `applyAddresses` does.
  for (const app of library.libraryApps()) library.decorate(app, library.rowAddress(app, list));
  library.applyLabels(list);
  void expenses; void pto;
  return context;
}

test('labels: the renamed app answers to /team-spend and its old names, a member\'s /exp and /q3 resolve, and names show on rows and canvas cards (APP-121)', async () => {
  const { doc, document_, registry, library, expenses, expensesRow } = await labeled();
  for (const name of ['team-spend', 'expenses', 'expense', 'exp']) assert.equal(registry.findCommand(name)?.slug, 'team-spend', name);
  // Library: the app's and the card's labels; the card's address follows the app's.
  assert.equal(expensesRow.querySelector('.cv-lib-app-title')?.textContent, 'Team spend');
  const ledger = expenses.cards.find((card) => card.slug === 'ledger')!;
  assert.equal(ledger.title, 'Q3 spend');
  assert.equal(ledger.base, 'Expense ledger');
  assert.equal(ledger.row.querySelector('.cv-lib-card-title')?.textContent, 'Q3 spend');
  assert.equal(library.cardAddress(ledger), 'team-spend.ledger');
  assert.equal(library.findCards('expenses.ledger')[0]?.id, ledger.id, 'the old address still finds the card');
  assert.ok(expensesRow.querySelector('[data-lib-edit]'), 'a ✎ on the row');
  // `/q3`: the member's own card shortcut runs `/card team-spend.ledger`.
  const q3 = registry.findCommand('q3')!;
  assert.equal(q3.title, 'Q3 spend');
  assert.match(q3.purpose, /team-spend\.ledger/);
  // Canvas cards: the ledger card's header reads its label, the app card the app's name.
  const board = document_.createElement('div');
  board.setAttribute('id', 'cv-board-2');
  const card = document_.createElement('article');
  card.setAttribute('class', 'cv-card');
  card.setAttribute('data-card', ledger.id);
  const head = document_.createElement('header');
  const h3 = document_.createElement('h3');
  h3.textContent = 'Expense ledger';
  head.append(h3);
  card.append(head);
  board.append(card);
  library.labelCanvasCards(doc);
  assert.equal(h3.textContent, 'Q3 spend');
  assert.equal(h3.getAttribute('title'), 'The Q3 ledger');
  assert.equal(card.getAttribute('aria-label'), 'Q3 spend');
});

test('/rename and /alias post the right label; a non-owner renames a card for themselves only and cannot rename the app (APP-121)', async () => {
  const { registry, slash } = await labeled();
  const naming = await import('../src/canvas/naming.ts');
  const library = await import('../src/canvas/library.ts');
  registry.registerCommands([library.CARD_COMMAND, naming.RENAME_COMMAND, naming.ALIAS_COMMAND]);
  assert.deepEqual(naming.parseRename('expenses Team spend /Team-Spend'), { target: 'expenses', name: 'Team spend', address: 'team-spend' });
  assert.deepEqual(naming.parseRename('expenses.total Q3 spend'), { target: 'expenses.total', name: 'Q3 spend', address: null });
  assert.equal(naming.findTarget('expenses')?.card, null);
  assert.equal(naming.findTarget('expenses.ledger')?.card?.slug, 'ledger');
  assert.equal(naming.findTarget('q3')?.card?.slug, 'ledger');
  assert.equal(naming.findTarget('3.0')?.card, null, 'the app card names its app');
  const posted: { url: string; body: unknown }[] = [];
  (globalThis as { fetch?: unknown }).fetch = async (url: string, init?: { body?: string }) => {
    if (url === slash.COMMANDS_URL) return { ok: true, status: 200, json: async () => LABELED };
    posted.push({ url, body: JSON.parse(init?.body ?? 'null') });
    return { ok: true, status: 200, json: async () => ({ saved: true }) };
  };
  const reports: string[] = [];
  const report = (message: string) => reports.push(message);
  // The owner: the app for everyone, with a new address; a card for everyone.
  await registry.findCommand('rename')!.run('expenses Team spend /team-spend', report);
  await registry.findCommand('rename')!.run('expenses.ledger Q3 spend', report);
  // A shortcut of their own, then taken back.
  await registry.findCommand('alias')!.run('mine expenses', report);
  await registry.findCommand('alias')!.run('exp', report);
  assert.deepEqual(posted, [
    { url: `/v1/apps/${EXPENSE_APP}/labels`, body: { scope: 'organization', name: 'Team spend', address: 'team-spend' } },
    { url: `/v1/apps/${EXPENSE_APP}/labels`, body: { scope: 'organization', card: 'ledger', name: 'Q3 spend' } },
    { url: `/v1/apps/${EXPENSE_APP}/labels`, body: { scope: 'personal', card: '', aliases: ['exp', 'mine'] } },
    { url: `/v1/apps/${EXPENSE_APP}/labels`, body: { scope: 'personal', card: '', aliases: [] } },
  ]);
  assert.match(reports[0]!, /Team spend at \/team-spend/);
  assert.match(reports[1]!, /for everyone\. Its data and receipts are unchanged/);
  // Time off: this member is no owner. The app is refused without a request; a card becomes their own name.
  posted.length = 0;
  await registry.findCommand('rename')!.run('pto Leave', report);
  assert.equal(posted.length, 0);
  assert.match(reports.at(-1)!, /Only the owner of Time off or an admin/);
  await registry.findCommand('rename')!.run('pto.records My requests', report);
  assert.deepEqual(posted, [{ url: `/v1/apps/${PTO_APP}/labels`, body: { scope: 'personal', card: 'records', name: 'My requests' } }]);
  assert.match(reports.at(-1)!, /for you/);
  // What a refusal says.
  assert.match(naming.labelMessage(422, 'verb_name', '/card'), /^\/card is a command’s name/);
  assert.match(naming.labelMessage(422, 'name_taken', '/team-spend'), /already taken/);
  assert.match(naming.labelMessage(404, '', '/team-spend'), /Only its owner or an admin/);
  assert.match(naming.labelMessage(422, 'not_shared', '/my-tool'), /share it/);
  assert.match(naming.labelMessage(422, 'too_many_labels', '/q4'), /100 names/);
});

test('the ✎ editor sends only what the member changed; an untouched save sends nothing (APP-121 review)', async () => {
  const naming = await import('../src/canvas/naming.ts');
  const before = { name: 'Expenses', address: '/expenses', description: '', myName: '', mine: '/exp' };
  // Saved as opened: no request, so no label and no audit event.
  assert.deepEqual(naming.editorRequests(before, { ...before }, null, true), []);
  assert.deepEqual(naming.editorRequests(before, { ...before, name: ' Expenses ' }, null, true), []);
  // Only the changed fields, organization first, then the member's own.
  assert.deepEqual(naming.editorRequests(before, { ...before, address: '/Team-Spend' }, null, true),
    [{ scope: 'organization', address: 'team-spend' }]);
  assert.deepEqual(naming.editorRequests(before, { ...before, name: 'Team spend', mine: '/exp, /ts' }, null, true),
    [{ scope: 'organization', name: 'Team spend' }, { scope: 'personal', card: '', aliases: ['exp', 'ts'] }]);
  assert.deepEqual(naming.editorRequests(before, { ...before, description: 'Our spend' }, null, true),
    [{ scope: 'organization', description: 'Our spend' }]);
  // A card: no address; a non-owner's own name for it.
  const card = { ...before, name: '', mine: '' };
  assert.deepEqual(naming.editorRequests(card, { ...card, name: 'Q3 spend' }, 'totals', true),
    [{ scope: 'organization', card: 'totals', name: 'Q3 spend' }]);
  assert.deepEqual(naming.editorRequests(card, { ...card, myName: 'Mine' }, 'totals', false),
    [{ scope: 'personal', card: 'totals', name: 'Mine' }]);
  assert.deepEqual(naming.editorRequests(card, { ...card, name: 'ignored' }, 'totals', false), [], 'not the owner: the organization fields are not sent');
});

// APP-122 review: one resolver for Shape and Ask. `/shape` takes every form
// of an address (APP-121's namespaced ones included); `/ask [address]
// <question>` scopes to the app or card its first word names exactly, else
// the whole line is the question (selection, else canvas).
test('/shape and /ask resolve every form of an address through one resolver; a non-name first word is part of the question (APP-122)', async () => {
  const { registry, slash, library, expenses } = await setup();
  const shape = await import('../src/canvas/shape.ts');
  const ask = await import('../src/canvas/ask.ts');
  const ledger = expenses.cards.find((card) => card.kind === 'ledger')!;
  // (Earlier tests renamed Expenses and indexed its row again; the old address is kept, and every form still reaches it.)
  const app = shape.resolveTarget('expenses');
  assert.ok(app.kind === 'app' && app.app.work === expenses.work);
  const name = app.address;
  const resolved = shape.resolveTarget('expenses.ledger');
  const cardName = resolved.kind === 'card' ? library.cardAddress(resolved.card) : '';
  assert.ok(cardName.endsWith('.ledger'));
  assert.deepEqual(shape.resolveTarget('app:expenses'), app);
  assert.deepEqual(shape.resolveTarget('/app:Expenses'), app);
  for (const form of ['expenses.ledger', 'card:expenses.ledger', '3.2', 'card:3.2', 'expense.ledger']) {
    const card = shape.resolveTarget(form);
    assert.ok(card.kind === 'card' && card.card.id === ledger.id && card.address === cardName, form);
  }
  assert.equal(shape.resolveTarget('folder:q3').kind, 'none');
  assert.equal(shape.resolveTarget('widget:spreadsheet').kind, 'none');
  assert.equal(shape.resolveTarget('app:nothing').kind, 'none');
  // `/ask <app> …` scopes to the app, `/ask <app.card> …` to the card and its collection.
  const byApp = ask.askLine('expenses why did spend jump in March?');
  assert.deepEqual(byApp.scope, { kind: 'app', address: name, works: [{ app_id: EXPENSE_APP, work_id: EXPENSE_WORK, address: byApp.scope?.works[0]?.address ?? '' }] });
  assert.equal(byApp.question, 'why did spend jump in March?');
  assert.deepEqual(ask.askLine('app:expenses why did spend jump in March?'), byApp, 'the full form is the same');
  assert.deepEqual(ask.askLine('expense why did spend jump in March?').scope, byApp.scope, 'an alias too');
  const byCard = ask.askLine('expenses.ledger which claims are overdue?');
  assert.deepEqual(byCard.scope, { kind: 'card', address: cardName, works: [{ app_id: EXPENSE_APP, work_id: EXPENSE_WORK, address: byApp.scope?.works[0]?.address ?? '', collection: COLLECTION }] });
  assert.equal(byCard.question, 'which claims are overdue?');
  assert.deepEqual(ask.askLine('card:expenses.ledger which claims are overdue?'), byCard);
  assert.deepEqual(ask.askLine('3.2 which claims are overdue?').scope, byCard.scope);
  // Not a name: the whole line is the question (selection, else canvas). Verbs, prefixes and titles never scope.
  for (const line of ['why is this high?', 'card why is this high?', 'notes from last week', 'time off is high', 'expenses.led which?', 'exp is high']) {
    assert.deepEqual(ask.askLine(line), { scope: null, question: line }, line);
  }
  assert.equal(ask.askLine('widget:calculator what is it')?.message, 'Select a weather, clock or calculator widget to ask about its controls and settings.');
  assert.equal(ask.askLine('folder:q3 what is in it').scope, null);
  // A personal shortcut names its app for its member.
  registry.setAddresses(slash.parseCommands({ commands: [{ ...SERVER.commands[0]!, aliases: ['expense', 'claim', 'e'], mine: ['e'] }, SERVER.commands[1]!] }).map(slash.serverAddress));
  library.decorate(expenses, library.rowAddress(expenses, registry.addressList()));
  const mine = ask.askLine('e how many claims');
  assert.ok(mine.scope?.kind === 'app' && mine.scope.works[0]?.work_id === EXPENSE_WORK && mine.question === 'how many claims');
  assert.ok(shape.resolveTarget('e').kind === 'app');
});

test('a library row keeps the canvas size apart from the pane label (APP-104)', async () => {
  const { doc, library } = await setup();
  const board = await import('../src/canvas/board.ts');
  assert.deepEqual(board.cardFootprint('summary'), { width: 400, height: 250 });
  assert.deepEqual(board.cardFootprint('timeline'), { width: 620, height: 340 });
  assert.deepEqual(board.cardFootprint('app'), { width: 340, height: 300 });
  assert.deepEqual(board.cardFootprint('form'), { width: 360, height: 300 });
  assert.deepEqual(board.cardFootprint('records'), { width: 680, height: 260 });
  assert.deepEqual(board.cardFootprint('ledger'), { width: 520, height: 280 });
  // Pane labels, and canvas kinds the board does not size, stay the fallback.
  for (const kind of ['number', 'chart', 'table', 'status']) {
    assert.deepEqual(board.cardFootprint(kind), { width: 440, height: 300 }, kind);
  }
  const row = library.buildAppRow(doc, {
    app: EXPENSE_APP, work: '00000000-0000-4000-8000-000000001690', number: '9', title: 'Sized', slug: '',
    cards: [
      { id: 'c-sized-spend', number: '9.1', title: 'Spend by vendor', kind: 'number', canvasKind: 'summary' },
      { id: 'c-sized-when', number: '9.2', title: 'When', kind: 'chart', canvasKind: 'timeline' },
    ],
  }) as unknown as FakeNode;
  const summaryRow = row.querySelectorAll('.cv-lib-card')[0]!;
  const timelineRow = row.querySelectorAll('.cv-lib-card')[1]!;
  assert.equal(summaryRow.dataset.canvasKind, 'summary');
  assert.equal(summaryRow.querySelector('.cv-lib-kind')?.textContent, 'number');
  assert.equal(timelineRow.dataset.canvasKind, 'timeline');
  const indexed = library.indexRow(row as unknown as HTMLElement)!;
  assert.equal(indexed.cards[0]?.kind, 'number');
  assert.equal(indexed.cards[0]?.canvasKind, 'summary');
  assert.equal(indexed.cards[1]?.kind, 'chart');
  assert.equal(indexed.cards[1]?.canvasKind, 'timeline');
  assert.deepEqual(board.cardFootprint(indexed.cards[0]!.canvasKind), { width: 400, height: 250 });
  assert.deepEqual(board.cardFootprint(indexed.cards[1]!.canvasKind), { width: 620, height: 340 });
});


test('App rows expose whole-work actions with inert component metadata and no expander', async () => {
  const { expensesRow } = await setup();
  assert.equal(expensesRow.querySelector('[data-lib-expand]'), null);
  assert.equal(expensesRow.querySelector('[data-lib-metadata]')?.hidden, true);
  assert.equal(expensesRow.querySelector('[data-lib-metadata]')?.getAttribute('aria-hidden'), 'true');
  assert.equal(expensesRow.querySelector('[data-lib-all]')?.textContent, 'Add');
  assert.equal(expensesRow.querySelector('[data-lib-stack]')?.textContent, 'Browse');
  assert.equal(expensesRow.querySelector('[data-lib-open-view]')?.textContent, 'Open');
  assert.equal(expensesRow.querySelector('[data-lib-classify]')?.textContent, 'Show in Cards');
  assert.match(expensesRow.querySelector('[data-lib-classify]')?.getAttribute('title') ?? '', /on your account/);
  assert.equal(expensesRow.dataset.artifactKind, undefined, 'only the server page says what the member saved');
  for (const button of expensesRow.querySelectorAll('[data-lib-toggle]')) assert.equal(button.getAttribute('tabindex'), '-1');
});

test('explicit standalone classification admits one whole work and is reversible; legacy child commands cannot place App parts', async () => {
  const { library, expenses, pto } = await setup();
  const board = await import('../src/canvas/board.ts');
  const reports: string[] = [];
  let requests = 0;
  const posted: { url: string; body: string; method?: string; credentials?: string }[] = [];
  (globalThis as { fetch?: unknown }).fetch = async (url: string, init?: { method?: string; body?: string; credentials?: string }) => {
    if (String(url).endsWith('/kind')) {
      posted.push({ url: String(url), body: init?.body ?? '', method: init?.method, credentials: init?.credentials });
      return { ok: true, status: 200, json: async () => ({ kind: JSON.parse(init?.body ?? '{}').kind }) };
    }
    requests += 1;
    throw Error('Unexpected placement request');
  };
  assert.equal(await board.setStandaloneWork(expenses.app, expenses.work, false), true);
  assert.equal(library.findStandaloneWorks('expenses.ledger').length, 0);
  await library.CARD_COMMAND.run('expenses.ledger', message => reports.push(message));
  assert.match(reports.at(-1)!, /No card matches/);
  assert.equal(requests, 0, 'refusal is before any group load or placement');
  assert.equal(await board.setStandaloneWork(expenses.app, expenses.work, true), true);
  assert.equal(posted.at(-1)?.url, `/v1/library/${expenses.app}/${expenses.work}/kind`);
  assert.equal(posted.at(-1)?.method, 'POST');
  assert.equal(posted.at(-1)?.credentials, 'same-origin');
  assert.equal(posted.at(-1)?.body, JSON.stringify({ kind: 'card' }));
  assert.equal(expenses.cards.length, 4, 'multi-component work, not a card-count heuristic');
  for (const query of ['expenses', 'expense.ledger', '3.2']) {
    const found = library.findStandaloneWorks(query);
    assert.equal(found.length, 1, query);
    assert.equal(found[0]?.work, expenses.work);
    assert.equal(found[0]?.cards.length, 4, 'every component stays with the work');
  }
  assert.equal(library.findStandaloneWorks('pto.records').length, 0);
  assert.equal(board.isStandaloneWork(pto.app, pto.work), false);
  const shown = library.findStandaloneWorks('expenses')[0]!;
  assert.equal(library.cardCompletions('expenses')[0]?.value, `/card ${shown.slug}`);
  assert.equal(library.cardCompletions('expenses')[0]?.detail, `/${shown.slug} · standalone card`);
  assert.equal(await board.setStandaloneWork(expenses.app, expenses.work, false), true);
  assert.equal(posted.at(-1)?.body, JSON.stringify({ kind: 'app' }));
  assert.equal(requests, 0, 'classification is not a placement request');
  assert.deepEqual(library.cardCompletions('expenses'), []);
  assert.ok(library.findCards('expenses.ledger').length, 'naming and Ask metadata survives category changes');
});

test('uploaded file Cards remain in inventory after closing and route placement by exact box/file identity', async () => {
  const { document_, library } = await setup();
  const section = document_.createElement('section');
  const entries = [
    {boxId: EXPENSE_APP, fileId: EXPENSE_WORK, title: 'Checking.csv', detail: 'CSV · Uploads', on: true},
    {boxId: PTO_APP, fileId: EXPENSE_WORK, title: '<img src=x>.xlsx', detail: 'Excel · Audit folder', on: false},
  ];
  const calls: string[][] = [];
  const render = () => library.renderFileCards(section as unknown as HTMLElement, entries, toggle);
  const toggle = async (box: string, file: string) => {
    calls.push([box, file]);
    const entry = entries.find(entry => entry.boxId === box && entry.fileId === file)!;
    entry.on = !entry.on;
    render();
    return entry.on ? 'File card added.' : 'File card removed; the file stays.';
  };
  render();
  const firstKey = `${EXPENSE_APP}/${EXPENSE_WORK}`;
  const secondKey = `${PTO_APP}/${EXPENSE_WORK}`;
  const first = () => section.querySelector(`[data-file-card-toggle="${firstKey}"]`)!;
  const rows = () => section.querySelectorAll('.cv-lib-file-card');
  assert.deepEqual(rows().map(row => row.querySelector('.cv-lib-app-title')?.textContent), entries.map(entry => entry.title));
  assert.equal(section.querySelector('img'), null, 'file names remain text');
  assert.match(rows()[1]!.dataset.search!, /audit folder/);
  assert.equal(first().textContent, 'Remove');
  assert.equal(first().getAttribute('aria-pressed'), 'true');
  first().click();
  await new Promise(resolve => setTimeout(resolve, 0));
  assert.equal(rows().length, 2, 'closing changes placement without removing the inventory row');
  assert.equal(first().textContent, 'Add');
  assert.equal(first().getAttribute('aria-pressed'), 'false');
  first().click();
  await new Promise(resolve => setTimeout(resolve, 0));
  section.querySelector(`[data-file-card-toggle="${secondKey}"]`)!.click();
  await new Promise(resolve => setTimeout(resolve, 0));
  assert.deepEqual(calls, [[EXPENSE_APP, EXPENSE_WORK], [EXPENSE_APP, EXPENSE_WORK], [PTO_APP, EXPENSE_WORK]], 'same file ID in another authorized folder keeps its own reference');
  assert.equal(first().textContent, 'Remove');
});

test('file card placement guards repeated clicks across rerenders and re-enables the current row', async () => {
  const { document_, library } = await setup();
  const section = document_.createElement('section');
  const entries = [{boxId: EXPENSE_APP, fileId: EXPENSE_WORK, title: 'Checking.csv', detail: 'CSV · Uploads', on: false}];
  let complete!: (message: string) => void;
  let calls = 0;
  const toggle = () => {calls++; return new Promise<string>(resolve => {complete = resolve;});};
  const render = () => library.renderFileCards(section as unknown as HTMLElement, entries, toggle);
  const button = () => section.querySelector('[data-file-card-toggle]')!;
  render();
  const original = button(); original.focus();
  render();
  assert.equal(button(), original, 'unchanged canvas notifications retain the focused Library control');
  original.click(); original.click();
  assert.equal(calls, 1);
  assert.equal(original.disabled, true);
  entries[0]!.on = true; render();
  assert.notEqual(button(), original);
  assert.equal(button().disabled, true);
  button().click();
  assert.equal(calls, 1, 'a canvas-driven replacement cannot duplicate pending placement');
  complete('Shown.');
  await new Promise(resolve => setTimeout(resolve, 0));
  assert.equal(button().disabled, false, 'the replacement row is usable once the request completes');
});

test('file Cards report placement failures without losing inventory or disabling retry', async () => {
  const { document_, library } = await setup();
  const section = document_.createElement('section');
  const entries = [{boxId: PTO_APP, fileId: PTO_WORK, title: 'Forecast.xlsx', detail: 'Excel · Planning', on: false}];
  let calls = 0;
  library.renderFileCards(section as unknown as HTMLElement, entries, async () => {calls++; throw new Error('unavailable');});
  const button = section.querySelector('[data-file-card-toggle]')!;
  button.click();
  await new Promise(resolve => setTimeout(resolve, 0));
  assert.match(section.textContent, /could not be changed/);
  assert.equal(button.disabled, false);
  assert.equal(section.querySelectorAll('.cv-lib-file-card').length, 1);
  button.click();
  await new Promise(resolve => setTimeout(resolve, 0));
  assert.equal(calls, 2);
});
