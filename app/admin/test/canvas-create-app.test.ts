import assert from 'node:assert/strict';
import { test } from 'node:test';
import { templateCatalog, startersIn, toggleFavorite, readFavorites, writeFavorites } from '../src/canvas/create-app.ts';

const catalog = [
  { id: 'equipment-requests', title: 'Equipment requests', industry: 'Operations', category: 'Trackers', description: 'Keep equipment requests with an item, requester, and notes.' },
  { id: 'reading-list', title: 'Reading list', industry: 'Education', category: 'Lists', description: 'Keep a reading list with a title, an author, and notes.' },
];

test('the admitted catalog is grouped by industry/category and favorites are member preferences', () => {
  assert.deepEqual(templateCatalog({ templates: catalog }), catalog);
  assert.deepEqual(startersIn(catalog, 'industries', [], 'Education').map((template) => template.id), ['reading-list']);
  assert.deepEqual(startersIn(catalog, 'categories', [], 'Trackers').map((template) => template.id), ['equipment-requests']);
  assert.deepEqual(startersIn(catalog, 'favorites', []), []);
  const marked = toggleFavorite([], 'reading-list');
  assert.deepEqual(startersIn(catalog, 'favorites', marked).map((template) => template.id), ['reading-list']);
  assert.deepEqual(toggleFavorite(marked, 'reading-list'), []);
  assert.deepEqual(templateCatalog({ templates: [...catalog, catalog[0]] }), [], 'duplicate IDs refuse the catalog');
  assert.deepEqual(templateCatalog({ templates: [{ ...catalog[0], id: '../escape' }] }), []);
});

test('favorites stay with the member in this browser and blocked storage is harmless', () => {
  const stored = new Map<string, string>([['cv-template-favorites', '["reading-list"]']]);
  const original = Object.getOwnPropertyDescriptor(globalThis, 'localStorage');
  Object.defineProperty(globalThis, 'localStorage', { configurable: true, value: {
    getItem: (key: string) => stored.get(key) ?? null,
    setItem: (key: string, value: string) => stored.set(key, value),
  } });
  try {
    const first = '0123456789abcdef';
    const second = 'fedcba9876543210';
    assert.deepEqual(readFavorites(first), [], 'unscoped legacy favorites cannot identify their owner');
    writeFavorites(first, ['reading-list']);
    assert.deepEqual(readFavorites(first), ['reading-list']);
    assert.deepEqual(readFavorites(second), []);
    writeFavorites('', ['expense-claims']);
    assert.deepEqual(readFavorites(''), []);
    Object.defineProperty(globalThis, 'localStorage', { configurable: true, get() { throw new Error('blocked'); } });
    assert.deepEqual(readFavorites(first), []);
    assert.doesNotThrow(() => writeFavorites(first, ['reading-list']));
  } finally {
    if (original) Object.defineProperty(globalThis, 'localStorage', original);
    else delete (globalThis as { localStorage?: unknown }).localStorage;
  }
});

// Minimal DOM for the real modal handlers and the compose HTTP seam. No model
// or board result is fabricated: a refused request must fetch no canvas group.
class Node {
  children: Node[] = [];
  attributes = new Map<string, string>();
  dataset: Record<string, string> = {};
  listeners = new Map<string, (() => void)[]>();
  parent: Node | null = null;
  hidden = false;
  value = '';
  textContent = '';
  constructor(readonly tag: string) {}
  setAttribute(key: string, value: string) { this.attributes.set(key, value); }
  append(...children: Node[]) { for (const child of children) { child.parent = this; this.children.push(child); } }
  replaceChildren() { this.children = []; }
  get childElementCount() { return this.children.length; }
  all(): Node[] { return [this, ...this.children.flatMap((child) => child.all())]; }
  querySelectorAll(selector: string): Node[] {
    return this.all().slice(1).filter((node) => {
      const attr = selector.match(/^\[([^=\]]+)(?:="([^"]*)")?\]$/);
      return attr ? node.attributes.has(attr[1]) && (attr[2] === undefined || node.attributes.get(attr[1]) === attr[2]) : node.tag === selector;
    });
  }
  querySelector(selector: string) { return this.querySelectorAll(selector)[0] ?? null; }
  addEventListener(type: string, listener: () => void) { this.listeners.set(type, [...(this.listeners.get(type) ?? []), listener]); }
  fire(type: string) { for (const listener of this.listeners.get(type) ?? []) (listener as (event: unknown) => void)({ preventDefault() {}, stopPropagation() {} }); }
  remove() { if (this.parent) this.parent.children = this.parent.children.filter((child) => child !== this); }
  close() { this.fire('close'); }
  showModal() { this.setAttribute('open', ''); }
  focus() {}
}
class DocumentStub {
  body = new Node('body');
  createElement(tag: string) { return new Node(tag); }
  createTextNode(text: string) { const node = new Node('#text'); node.textContent = text; return node; }
  getElementById(id: string) { return this.body.all().find((node) => node.attributes.get('id') === id) ?? null; }
}

test('catalog discovery is read-only; confirm sends the exact template and refusal places nothing', async () => {
  const { openCreateApp } = await import('../src/canvas/create-app.ts');
  const document_ = new DocumentStub();
  const activity = document_.createElement('div');
  activity.setAttribute('id', 'cv-activity');
  document_.body.append(activity);
  const calls: { url: string; body: unknown }[] = [];
  const originalFetch = globalThis.fetch;
  globalThis.fetch = (async (url: string, init: RequestInit) => {
    if (url === '/v1/templates') return new Response(JSON.stringify({ templates: catalog }));
    calls.push({ url, body: JSON.parse(String(init.body)) });
    return new Response(JSON.stringify({ code: 'could_not_compose' }), { status: 422 });
  }) as typeof fetch;
  try {
    openCreateApp(document_ as unknown as Document);
    await new Promise((resolve) => setTimeout(resolve, 0));
    let dialog = document_.getElementById('cv-create-app')!;
    assert.equal(dialog.querySelector('textarea')!.parent!.hidden, true);
    dialog.querySelector('[data-template="reading-list"]')!.fire('click');
    assert.equal(calls.length, 0);
    dialog.querySelector('[data-create-cancel]')!.fire('click');
    assert.equal(document_.getElementById('cv-create-app'), null);
    assert.equal(calls.length, 0);
    openCreateApp(document_ as unknown as Document);
    await new Promise((resolve) => setTimeout(resolve, 0));
    dialog = document_.getElementById('cv-create-app')!;
    dialog.querySelector('[data-template="equipment-requests"]')!.fire('click');
    const sentence = 'Keep the claims from this team only';
    dialog.querySelector('textarea')!.value = sentence;
    dialog.querySelector('form')!.fire('submit');
    await new Promise((resolve) => setTimeout(resolve, 10));
    assert.deepEqual(calls, [{ url: '/v1/compose', body: { intent: sentence, template_id: 'equipment-requests' } }]);
    assert.match(activity.textContent, /Nothing was placed/);
  } finally { globalThis.fetch = originalFetch; }
});
