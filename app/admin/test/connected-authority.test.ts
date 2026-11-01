import { test } from 'node:test';
import assert from 'node:assert/strict';
import { startConnectedApps } from '../src/shared/canvas-controller';

class Element extends EventTarget {
  dataset: Record<string, string> = {}; attributes: Record<string, string> = {}; children: Element[] = [];
  textContent = ''; parent: Element | null = null;
  constructor(readonly tag: string) { super(); }
  setAttribute(key: string, value: string) { this.attributes[key] = value; }
  append(...values: (Element | string)[]) { for (const value of values) if (typeof value === 'string') this.textContent += value; else { value.parent = this; this.children.push(value); } }
  remove() { if (this.parent) this.parent.children = this.parent.children.filter(node => node !== this); }
  querySelector(selector: string): Element | null {
    const matches = (node: Element) => selector.startsWith('[') ? Object.hasOwn(node.attributes, selector.slice(1, -1)) : selector.startsWith('.') ? node.attributes.class === selector.slice(1) : node.tag === selector;
    for (const node of this.children) { if (matches(node)) return node; const child = node.querySelector(selector); if (child) return child; } return null;
  }
}

test('inventory outage masks the original connected item and recovery keeps its geometry', async () => {
  const saved = new Map<string, PropertyDescriptor | undefined>();
  const replace = (name: string, value: unknown) => { saved.set(name, Object.getOwnPropertyDescriptor(globalThis, name)); Object.defineProperty(globalThis, name, { configurable: true, writable: true, value }); };
  const key = '00000000-0000-4000-8000-000000000001.00000000-0000-4000-8000-000000000002';
  const document_ = Object.assign(new EventTarget(), { visibilityState: 'visible', createElement: (tag: string) => new Element(tag) });
  const window_ = Object.assign(new EventTarget(), { setTimeout: () => 1, clearTimeout: () => {}, setInterval: () => 1, clearInterval: () => {} });
  const card = new Element('article'); card.dataset.connectedApp = key;
  const title = new Element('h2'); title.textContent = 'Original app'; card.append(title);
  const owner = new Element('p'); owner.attributes.class = 'connected-owner'; owner.textContent = 'Owner'; card.append(owner);
  const parts = new Element('div'); parts.attributes['data-shared-parts'] = ''; card.append(parts);
  const item = new Element('section'); item.dataset.x = '173'; item.dataset.y = '289'; item.append(card);
  let mounts = 0; let removes = 0; let healthy = false; const statuses: string[] = [];
  const settle = async () => { for (let n = 0; n < 30; n += 1) await Promise.resolve(); };
  replace('window', window_); replace('navigator', { onLine: true });
  replace('fetch', (url: string) => {
    if (url.endsWith('/authority')) return new Promise(() => {});
    assert.equal(url, '/v1/shared');
    return Promise.resolve(new Response(JSON.stringify(healthy ? { apps: [{ id: key, href: `/s/${key}`, title: 'Original app', owner_label: 'Owner', can_add_to_canvas: true }] } : {}), { status: healthy ? 200 : 503, headers: { 'content-type': 'application/json' } }));
  });
  try {
    startConnectedApps({ document: document_ as unknown as Document, mount() { mounts += 1; return item as unknown as HTMLElement; }, remove() { removes += 1; }, status(message) { statuses.push(message); } }, [card as unknown as HTMLElement]);
    await settle();
    assert.equal(mounts, 1); assert.equal(removes, 0); assert.equal(card.querySelector('[data-shared-parts]'), null);
    assert.equal(title.textContent, 'Private app');
    assert.match(card.querySelector('[data-connected-loading]')?.textContent ?? '', /could not be verified/);
    healthy = true; document_.dispatchEvent(new Event('forma:connected-refresh')); await settle();
    assert.equal(mounts, 1); assert.equal(removes, 0);
    assert.deepEqual([item.dataset.x, item.dataset.y], ['173', '289']);
    assert.ok(statuses.some(message => message.startsWith('1 connected app')));
  } finally { for (const [name, descriptor] of saved) { if (descriptor) Object.defineProperty(globalThis, name, descriptor); else Reflect.deleteProperty(globalThis, name); } }
});
