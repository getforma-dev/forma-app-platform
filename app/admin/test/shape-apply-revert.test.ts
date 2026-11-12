// APP-122 acceptance 3: `/expenses.ledger highlight overdue in red` previews a
// highlighted ledger; nothing changes until Apply; then the card is labeled
// *shaped* and Revert restores it; no record is written. The real `runShape`
// path runs: the real renderer (`render.ts`), view evaluation (`talk-view.ts`)
// and listing parser (`records.ts`). Placement, the dock host and the Library
// lookup are spies, and `fetch` is the transport.
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import { build } from 'esbuild';

const APP = '00000000-0000-4000-8000-000000001681';
const WORK = '00000000-0000-4000-8000-000000001684';
const COLLECTION = '00000000-0000-4000-8000-000000001683';
const money = (cents: string) => ({ type: 'money', minor_units: cents, currency: 'USD', minor_unit_exponent: 2 });
const record = (n: number, merchant: string, cents: string, status: string) => ({
  record_id: `00000000-0000-4000-8000-00000000c00${n}`, collection_id: COLLECTION, schema_id: 'expense_claim', version: 1, commit_sequence: n,
  committed_at: '2026-10-28T12:00:00Z', creator: { principal_id: 'p', label: 'Diego Ramos' }, updater: { principal_id: 'p', label: 'Diego Ramos' },
  values: [
    { field_id: 'amount', value: money(cents), display: `$${(Number(cents) / 100).toFixed(2)}` },
    { field_id: 'merchant', value: { type: 'text', value: merchant }, display: merchant },
    { field_id: 'status', value: { type: 'enum', value: status }, display: status },
  ],
});
const LISTING = {
  app_id: APP, work_id: WORK, publication_id: '00000000-0000-4000-8000-0000000016a1', definition_digest: `sha256:${'a'.repeat(64)}`, work_version: 4, can_write: true,
  collections: [{
    collection_id: COLLECTION, schema_id: 'expense_claim', schema_version: 1, commands: [],
    fields: [
      { id: 'amount', type: 'money', nullable: false, writable: true },
      { id: 'merchant', type: 'text', nullable: false, writable: true },
      { id: 'status', type: 'enum', nullable: false, writable: true, values: ['submitted', 'approved', 'rejected'] },
    ],
  }],
  records: [record(1, 'Nobu', '8600', 'submitted'), record(2, 'Delta', '41200', 'approved')],
  truncated: false,
};
const HIGHLIGHT = { highlight: [{ field: 'status', op: 'in', value: ['submitted'], color: 'crimson' }] };
const THEME = { mode: 'dark', accent: 'green', density: 'compact' };

/** A minimal DOM: the selectors `shape.ts` uses (`:scope > X`, `.class`, tag, `[hidden]`), events and `after`. */
class Node_ {
  children: Node_[] = [];
  parentElement: Node_ | null = null;
  dataset: Record<string, string> = {};
  attributes = new Map<string, string>();
  classes = new Set<string>();
  hidden = false;
  inert = false;
  text = '';
  listeners = new Map<string, ((event: unknown) => void)[]>();
  classList = { add: (...n: string[]) => n.forEach((c) => this.classes.add(c)), remove: (...n: string[]) => n.forEach((c) => this.classes.delete(c)), contains: (c: string) => this.classes.has(c), toggle: (c: string) => this.classes.has(c) ? (this.classes.delete(c), false) : (this.classes.add(c), true) };
  constructor(readonly ownerDocument: Document_, readonly tagName: string) {}
  setAttribute(name: string, value: string) {
    if (name === 'class') value.split(/\s+/).filter(Boolean).forEach((c) => this.classes.add(c));
    else if (name.startsWith('data-')) this.dataset[name.slice(5).replace(/-([a-z])/g, (_m, l: string) => l.toUpperCase())] = value;
    else if (name === 'hidden') this.hidden = true;
    this.attributes.set(name, value);
  }
  getAttribute(name: string) { return this.attributes.get(name) ?? null; }
  removeAttribute(name: string) { this.attributes.delete(name); }
  get textContent(): string { return this.tagName === '#text' ? this.text : this.children.map((c) => c.textContent).join(''); }
  set textContent(value: string) { this.children = []; this.text = value; }
  get firstElementChild(): Node_ | null { return this.children.find((c) => c.tagName !== '#text') ?? null; }
  get isConnected(): boolean { return this.parentElement ? this.parentElement.isConnected : this === this.ownerDocument.body; }
  append(...nodes: (Node_ | string)[]) { for (const node of nodes) this.insert(typeof node === 'string' ? this.ownerDocument.createTextNode(node) : node, this.children.length); }
  after(node: Node_) { const parent = this.parentElement!; parent.insert(node, parent.children.indexOf(this) + 1); }
  insert(node: Node_, at: number) { node.remove(); node.parentElement = this; this.children.splice(at, 0, node); }
  remove() { if (this.parentElement) this.parentElement.children.splice(this.parentElement.children.indexOf(this), 1); this.parentElement = null; }
  addEventListener(type: string, listener: (event: unknown) => void) { this.listeners.set(type, [...(this.listeners.get(type) ?? []), listener]); }
  removeEventListener() {}
  click() { for (const listener of this.listeners.get('click') ?? []) listener({ stopPropagation() {}, preventDefault() {} }); }
  focus() {}
  matchesOne(selector: string): boolean {
    if (selector === '[hidden]') return this.hidden;
    if (selector === '[inert]') return this.inert;
    const withAttribute = /^(\.[\w-]+)\[data-([\w-]+)\]$/.exec(selector);
    if (withAttribute) return this.classes.has(withAttribute[1]!.slice(1)) && this.dataset[withAttribute[2]!.replace(/-([a-z])/g, (_m, l: string) => l.toUpperCase())] !== undefined;
    if (selector.startsWith('.')) return selector.slice(1).split('.').every((c) => this.classes.has(c));
    return this.tagName === selector;
  }
  matches(selector: string) { return selector.split(',').some((part) => this.matchesOne(part.trim())); }
  descendants(): Node_[] { return this.children.flatMap((c) => [c, ...c.descendants()]); }
  querySelectorAll(selector: string): Node_[] {
    const parts = selector.split(',').map((p) => p.trim());
    const found = new Set<Node_>();
    for (const part of parts) {
      const scoped = /^:scope > (.+)$/.exec(part);
      if (scoped) {
        const chain = scoped[1]!.split(' > ').map((s) => s.trim());
        let level: Node_[] = [this];
        for (const step of chain) level = level.flatMap((n) => n.children.filter((c) => c.matchesOne(step)));
        level.forEach((n) => found.add(n));
      } else this.descendants().filter((n) => n.matchesOne(part)).forEach((n) => found.add(n));
    }
    return this.descendants().filter((n) => found.has(n));
  }
  querySelector(selector: string) { return this.querySelectorAll(selector)[0] ?? null; }
  closest(selector: string): Node_ | null { return this.matches(selector) ? this : this.parentElement?.closest(selector) ?? null; }
}
class Document_ {
  body: Node_ = new Node_(this, 'body');
  createElement(tag: string) { return new Node_(this, tag); }
  createTextNode(text: string) { const node = new Node_(this, '#text'); node.text = text; return node; }
  getElementById() { return null; }
  querySelectorAll(selector: string) { return this.body.querySelectorAll(selector); }
  querySelector(selector: string) { return this.body.querySelector(selector); }
}

let serial = 0;
async function fixture(kind: 'card' | 'app' = 'card') {
  const entry = fileURLToPath(new URL('../src/canvas/shape.ts', import.meta.url));
  const source = await readFile(entry, 'utf8');
  const imports = new Map<string, string[]>();
  for (const [, names, path] of source.matchAll(/import\s*\{([^}]+)\}\s*from\s*['"]([^'"]+)['"]/gs))
    imports.set(path!, names!.split(',').map((name) => name.trim()).filter((name) => name && !name.startsWith('type ')));
  const document_ = new Document_();
  const card = document_.createElement('article');
  for (const c of ['cv-card', 'cv-records']) card.classes.add(c);
  Object.assign(card.dataset, { card: `c-${WORK}-ledger`, records: `/v1/work/${APP}/${WORK}/records`, collection: COLLECTION });
  const header = document_.createElement('header');
  header.classes.add('cv-card-head');
  const bodyNode = document_.createElement('div');
  bodyNode.classes.add('cv-card-body');
  bodyNode.append('Nobu · Delta');
  card.append(header, bodyNode);
  document_.body.append(card);
  // APP-122 box 6: an app card and its components, all of one saved work.
  const appCard = document_.createElement('article');
  const appHead = document_.createElement('header');
  const components: Node_[] = [];
  if (kind === 'app') {
    for (const c of ['cv-card', 'cv-app']) appCard.classes.add(c);
    Object.assign(appCard.dataset, { work: WORK, kind: 'app' });
    appHead.classes.add('cv-card-head');
    appCard.append(appHead);
    Object.assign(card.dataset, { work: WORK, kind: 'records' });
    components.push(card);
    const view = document_.createElement('article');
    view.classes.add('cv-card');
    Object.assign(view.dataset, { work: WORK, kind: 'view' });
    const viewBody = document_.createElement('div');
    viewBody.classes.add('cv-card-body');
    viewBody.append('Spend by vendor');
    view.append(viewBody);
    components.push(view);
    document_.body.append(appCard, view);
  }
  const appRow = { app: APP, work: WORK, slug: 'expenses', title: 'Expenses', number: '1' };
  const row = { id: card.dataset.card, slug: 'ledger', title: 'Ledger', number: '1.2', app: { app: APP, work: WORK } };
  const docks: Node_[] = [];
  const messages: string[] = [];
  const requests: { url: string; method: string; body?: string }[] = [];
  const real = {
    './render': await import('../src/canvas/render.ts'),
    './talk-view': await import('../src/canvas/talk-view.ts'),
    './records': await import('../src/canvas/records.ts'),
  };
  const mocks: Record<string, Record<string, (...args: any[]) => any>> = {
    ...real as never,
    './board': { ensureAppGroup: async () => ({ cards: [card] }), isOnBoard: () => true, focusCard: () => undefined, onLiveEvent: () => undefined, isDemoCanvas: () => false, addCards: async () => { throw new Error('the app is already on the board'); } },
    './unite': { setAppView: async () => { throw new Error('the card is already on the board'); } },
    '../presentation/presentation': { appComponentVisible: () => true },
    './dock-card': { showDockCard: (_d: unknown, node: Node_) => { docks.push(node); document_.body.append(node); return () => node.remove(); }, isDismissKey: () => false },
    './library': { findCards: () => [row], cardAddress: () => 'expenses.ledger', libraryApps: () => [appRow], findApps: () => [appRow], findCardRow: () => row },
    './command-registry': { findCardShortcut: () => null, findAppAddress: () => null, findAddress: (name: string) => name === 'expenses' ? { app: APP, work: WORK, slug: 'expenses' } : null, findVerb: () => null, appForm: () => 'expenses' },
    './widgets/index': { widgetKind: () => null, widgetFor: () => null, widgetOfItem: () => null },
    './address-chip': { fillBar: () => undefined, addressFor: () => '' },
  };
  const key = `__shapeApplyRevert${serial}`;
  (globalThis as any)[key] = mocks;
  // A banner per fixture gives each test its own shape.ts instance (no shared applied state).
  const result = await build({ entryPoints: [entry], bundle: true, format: 'esm', platform: 'node', write: false, banner: { js: `// fixture ${serial++}` },
    plugins: [{ name: 'shape-boundaries', setup(builder) {
      builder.onResolve({ filter: /.*/ }, (args) => args.importer === entry && imports.has(args.path) ? { path: args.path, namespace: 'shape-spy' } : undefined);
      builder.onLoad({ filter: /.*/, namespace: 'shape-spy' }, (args) => ({ contents: imports.get(args.path)!.map((name) => `export const ${name} = (...args) => globalThis[${JSON.stringify(key)}][${JSON.stringify(args.path)}]?.[${JSON.stringify(name)}]?.(...args);`).join('\n'), loader: 'js' }));
    } }],
  });
  const api = await import('data:text/javascript;base64,' + Buffer.from(result.outputFiles[0]!.text).toString('base64'));
  const previous = { document: (globalThis as any).document, fetch: globalThis.fetch };
  (globalThis as any).document = document_;
  (globalThis as any).fetch = async (url: string, init: { method?: string; body?: string } = {}) => {
    requests.push({ url, method: init.method ?? 'GET', body: init.body });
    if (url === LISTING_URL) return { ok: true, status: 200, json: async () => structuredClone(LISTING) };
    if (url === '/v1/shape') return { ok: true, status: 200, json: async () => (kind === 'app' ? { target: 'app', spec: THEME } : { target: 'card', spec: HIGHLIGHT }) };
    return { ok: false, status: 404, json: async () => ({}) };
  };
  const report = (message: string) => messages.push(message);
  const button = (label: string) => docks.at(-1)!.querySelectorAll('button').find((node) => node.textContent === label)!;
  return { api, card, header, bodyNode, appCard, appHead, components, docks, messages, requests, report, button,
    cleanup: () => { delete (globalThis as any)[key]; (globalThis as any).document = previous.document; globalThis.fetch = previous.fetch; } };
}
const LISTING_URL = `/v1/work/${APP}/${WORK}/records`;

test('Shape previews a highlighted ledger, Apply labels it shaped, Revert restores it, and nothing is written (APP-122)', async () => {
  const f = await fixture();
  try {
    await f.api.runShape('expenses.ledger', 'highlight overdue in red', f.report);
    // Preview: the card's own body is hidden under a preview section; nothing is applied yet.
    assert.equal(f.docks.length, 1, 'one Apply / Add as new card / Cancel dock');
    assert.equal(f.card.dataset.shaped, 'preview');
    assert.equal(f.card.querySelector(':scope > .cv-shaped')?.dataset.state, 'preview');
    assert.equal(f.bodyNode.hidden, true);
    assert.equal(f.header.querySelector(':scope > .cv-shaped-label'), null, 'no shaped label before Apply');
    const highlighted = f.card.querySelector(':scope > .cv-shaped')!.descendants().filter((node) => node.dataset.highlight === 'crimson');
    assert.equal(highlighted.length, 1, 'exactly the submitted (Nobu) row is highlighted crimson');
    assert.match(highlighted[0]!.textContent, /Nobu/);
    assert.match(f.docks[0]!.textContent, /Nothing changes until you apply it\. No record is written\./);
    // Apply: the card is labeled shaped, with Revert on it.
    f.button('Apply').click();
    assert.equal(f.card.dataset.shaped, 'true');
    assert.equal(f.card.querySelector(':scope > .cv-shaped')?.dataset.state, 'shaped');
    const label = f.header.querySelector(':scope > .cv-shaped-label');
    assert.ok(label, 'the shaped label is on the card header');
    assert.equal(label!.querySelector('.cv-shaped-badge')?.textContent, 'shaped');
    assert.equal(f.docks[0]!.isConnected, false, 'the dock closed');
    assert.match(f.messages.at(-1)!, /is shaped\. Revert is on the card\./);
    // Revert: the card shows its own view again.
    label!.querySelector('.cv-shaped-revert')!.click();
    assert.equal(f.card.querySelector(':scope > .cv-shaped'), null);
    assert.equal(f.header.querySelector(':scope > .cv-shaped-label'), null);
    assert.equal(f.bodyNode.hidden, false);
    assert.equal(f.card.dataset.shaped, undefined);
    // No record was written: one listing read and one shape request, nothing else.
    assert.deepEqual(f.requests.map((r) => `${r.method} ${r.url}`), [`GET ${LISTING_URL}`, 'POST /v1/shape']);
    assert.equal(JSON.parse(f.requests[1]!.body!).target.collection, COLLECTION);
  } finally { f.cleanup(); }
});

test('Cancel on a Shape preview leaves the card exactly as it was (APP-122)', async () => {
  const f = await fixture();
  try {
    await f.api.runShape('expenses.ledger', 'highlight overdue in red', f.report);
    f.button('Cancel').click();
    assert.equal(f.card.querySelector(':scope > .cv-shaped'), null);
    assert.equal(f.bodyNode.hidden, false);
    assert.equal(f.card.dataset.shaped, undefined);
    assert.equal(f.header.querySelector(':scope > .cv-shaped-label'), null);
    assert.equal(f.messages.at(-1), 'Nothing changed.');
    assert.deepEqual(f.requests.map((r) => r.method), ['GET', 'POST']);
  } finally { f.cleanup(); }
});

test('Shape previews a theme on every card of an app, Apply themes them all keeping each view, Revert restores them (APP-122 box 6)', async () => {
  const f = await fixture('app');
  try {
    await f.api.runShape('expenses', 'dark green, compact', f.report);
    const themed = [f.appCard, ...f.components];
    // Preview: every card of the work carries the theme; nothing is applied yet.
    assert.equal(f.docks.length, 1);
    for (const node of themed) {
      assert.equal(node.dataset.theme, 'green-dark', 'every card previews the theme');
      assert.equal(node.dataset.themeDensity, 'compact');
      assert.equal(node.dataset.themePreview, 'true');
    }
    assert.equal(f.appHead.querySelector(':scope > .cv-shaped-label'), null, 'no shaped label before Apply');
    assert.match(f.docks[0]!.textContent, /Nothing changes until you apply it\./);
    // Apply: every card keeps the theme and its own view; the app card is labeled shaped with Revert.
    f.button('Apply').click();
    for (const node of themed) {
      assert.equal(node.dataset.theme, 'green-dark');
      assert.equal(node.dataset.themePreview, undefined, 'the preview marker is settled');
      assert.equal(node.querySelector(':scope > .cv-shaped'), null, 'each card keeps its own view (no shaped section)');
    }
    assert.equal(f.bodyNode.hidden, false);
    const label = f.appHead.querySelector(':scope > .cv-shaped-label');
    assert.ok(label, 'the app card is labeled shaped');
    assert.match(f.messages.at(-1)!, /Every \/expenses card is shaped; each keeps its own view\./);
    // Revert on the app card restores every card.
    label!.querySelector('.cv-shaped-revert')!.click();
    for (const node of themed) {
      assert.equal(node.dataset.theme, undefined, 'the theme is removed from every card');
      assert.equal(node.dataset.themeDensity, undefined);
    }
    assert.equal(f.appHead.querySelector(':scope > .cv-shaped-label'), null);
    // Theming reads no records and writes nothing: one shape request only.
    assert.deepEqual(f.requests.map((r) => `${r.method} ${r.url}`), ['POST /v1/shape']);
    assert.deepEqual(JSON.parse(f.requests[0]!.body!).target, { kind: 'app', app_id: APP, work_id: WORK });
  } finally { f.cleanup(); }
});

test('Cancel on an app theme preview leaves every card as it was (APP-122 box 6)', async () => {
  const f = await fixture('app');
  try {
    await f.api.runShape('expenses', 'dark green, compact', f.report);
    f.button('Cancel').click();
    for (const node of [f.appCard, ...f.components]) {
      assert.equal(node.dataset.theme, undefined);
      assert.equal(node.dataset.themePreview, undefined);
    }
    assert.equal(f.messages.at(-1), 'Nothing changed.');
  } finally { f.cleanup(); }
});
