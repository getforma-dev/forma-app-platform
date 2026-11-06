// A small fake canvas page for driving the real `startBoard` (APP-169 review):
// enough DOM, window and storage for the canvas island to start. It is not a
// browser; it only lets a test start the board over chosen server rows.
export const MEMBER = '0123456789abcdef';
export const KEY = `forma.canvas.standalone.v1:${MEMBER}`;

type Listener = (event: unknown) => void;
export class Node_ {
  dataset: Record<string, string> = {};
  attributes = new Map<string, string>();
  children: Node_[] = [];
  parentElement: Node_ | null = null;
  listeners = new Map<string, Listener[]>();
  classes = new Set<string>();
  style: Record<string, unknown> = { setProperty() {}, removeProperty() {} };
  hidden = false; inert = false; tabIndex = -1; textContent = ''; title = '';
  clientWidth = 1000; clientHeight = 800; offsetHeight = 0; scrollTop = 0; scrollLeft = 0;
  classList = {
    add: (...names: string[]) => names.forEach((name) => this.classes.add(name)),
    remove: (...names: string[]) => names.forEach((name) => this.classes.delete(name)),
    contains: (name: string) => this.classes.has(name),
    toggle: (name: string, on = !this.classes.has(name)) => { if (on) this.classes.add(name); else this.classes.delete(name); return on; },
  };
  constructor(readonly ownerDocument: Document_, readonly tagName = 'div') {}
  set className(value: string) { this.classes = new Set(value.split(/\s+/).filter(Boolean)); }
  get className() { return [...this.classes].join(' '); }
  get isConnected() { return true; }
  setAttribute(name: string, value: string) {
    if (name.startsWith('data-')) this.dataset[camel(name)] = value;
    else if (name === 'class') this.className = value;
    else if (name === 'id') this.ownerDocument.ids.set(value, this);
    this.attributes.set(name, value);
  }
  getAttribute(name: string) { return name.startsWith('data-') ? this.dataset[camel(name)] ?? null : this.attributes.get(name) ?? null; }
  hasAttribute(name: string) { return this.getAttribute(name) !== null; }
  removeAttribute(name: string) { if (name.startsWith('data-')) delete this.dataset[camel(name)]; this.attributes.delete(name); }
  append(...nodes: (Node_ | string)[]) {
    for (const node of nodes) {
      if (typeof node === 'string') continue;
      node.parentElement?.children.splice(node.parentElement.children.indexOf(node), 1);
      node.parentElement = this; this.children.push(node);
    }
  }
  prepend(...nodes: Node_[]) { this.append(...nodes); }
  remove() { this.parentElement?.children.splice(this.parentElement.children.indexOf(this), 1); this.parentElement = null; }
  contains(node: Node_ | null): boolean { return node === this || this.children.some((child) => child.contains(node)); }
  addEventListener(type: string, listener: Listener) { this.listeners.set(type, [...(this.listeners.get(type) ?? []), listener]); }
  removeEventListener() {}
  all(): Node_[] { return [this, ...this.children.flatMap((child) => child.all())]; }
  matches(selector: string): boolean {
    if (selector.includes(',')) return selector.split(',').some((part) => this.matches(part.trim()));
    const attribute = selector.match(/^(\.[\w-]+)?\[([\w-]+)(?:="([^"]*)")?\]$/);
    if (attribute) {
      const [, klass, name, wanted] = attribute;
      if (klass && !this.classes.has(klass.slice(1))) return false;
      const value = this.getAttribute(name!);
      return value !== null && (wanted === undefined || value === wanted);
    }
    if (selector.startsWith('.')) return this.classes.has(selector.slice(1));
    return this.tagName === selector;
  }
  querySelectorAll(selector: string): Node_[] { return this.all().slice(1).filter((node) => node.matches(selector)); }
  querySelector(selector: string): Node_ | null { return this.querySelectorAll(selector)[0] ?? null; }
  closest(selector: string): Node_ | null { return this.matches(selector) ? this : this.parentElement?.closest(selector) ?? null; }
  getBoundingClientRect() { return { x: 0, y: 0, left: 0, top: 0, right: 1000, bottom: 800, width: 1000, height: 800 }; }
  focus() {}
}
function camel(name: string): string { return name.slice(5).replace(/-([a-z])/g, (_m, letter: string) => letter.toUpperCase()); }
class Window_ {
  AbortController = AbortController; HTMLElement = Node_; Element = Node_; HTMLButtonElement = Node_;
  ResizeObserver = class { observe() {} unobserve() {} disconnect() {} };
  performance = { now: () => 1000 };
  getComputedStyle() { return { overflowX: 'visible', overflowY: 'visible' }; }
  requestAnimationFrame() { return 1; }
  cancelAnimationFrame() {}
  setTimeout() { return 1; }
  clearTimeout() {}
  setInterval() { return 1; }
  clearInterval() {}
  addEventListener() {}
  removeEventListener() {}
  matchMedia() { return { matches: false, addEventListener() {} }; }
}
export class Document_ {
  ids = new Map<string, Node_>();
  defaultView = new Window_();
  body = new Node_(this, 'body');
  activeElement: Node_ | null = null;
  visibilityState = 'visible';
  hidden = false;
  createElement(tag: string) { return new Node_(this, tag); }
  createElementNS(_ns: string, tag: string) { return new Node_(this, tag); }
  createTextNode(text: string) { const node = new Node_(this, '#text'); node.textContent = text; return node; }
  getElementById(id: string) { return this.ids.get(id) ?? null; }
  querySelectorAll(selector: string) { return selector.includes('>') ? [] : this.body.querySelectorAll(selector); }
  querySelector(selector: string) { return this.querySelectorAll(selector)[0] ?? null; }
  addEventListener() {}
  removeEventListener() {}
  dispatchEvent() { return true; }
}

function element(document_: Document_, tag: string, attributes: Record<string, string>, parent: Node_): Node_ {
  const node = document_.createElement(tag);
  for (const [name, value] of Object.entries(attributes)) node.setAttribute(name, value);
  parent.append(node);
  return node;
}

/** The server page: `kinds` names each row's `data-artifact-kind` (absent when there is no row). */
export function page(app: string, loaded: boolean, kinds: Record<string, 'card' | 'app' | null>): Document_ {
  const document_ = new Document_();
  element(document_, 'main', { id: 'cv-canvas', 'data-member': MEMBER, ...(loaded ? { 'data-artifact-kinds': 'true' } : {}) }, document_.body);
  element(document_, 'p', { id: 'cv-activity' }, document_.body);
  const list = element(document_, 'ul', { class: 'cv-lib-apps' }, document_.body);
  for (const [work, kind] of Object.entries(kinds)) {
    element(document_, 'li', { class: 'cv-lib-app', id: `lib-${work}`, 'data-app': app, 'data-work': work, ...(kind ? { 'data-artifact-kind': kind } : {}) }, list);
  }
  return document_;
}

/** Installs the page, its window, `localStorage` over `store` and `fetch` as the globals the board reads. */
export function install(document_: Document_, store: Map<string, string>, fetch_: (url: string, init?: { method?: string; body?: string }) => Promise<unknown>): void {
  Object.assign(globalThis, {
    document: document_, window: document_.defaultView, location: { hash: '', search: '' },
    HTMLElement: Node_, ResizeObserver: document_.defaultView.ResizeObserver, requestAnimationFrame: () => 1, cancelAnimationFrame: () => undefined,
    matchMedia: () => ({ matches: false, addEventListener() {} }),
    localStorage: { getItem: (key: string) => store.get(key) ?? null, setItem: (key: string, value: string) => { store.set(key, value); }, removeItem: (key: string) => { store.delete(key); }, key: () => null, length: 0 },
    fetch: fetch_,
  });
}

/** One `/kind` POST the test has not answered yet. */
export type HeldPost = { work: string; kind: string; ifAbsent: boolean; answer: (status: number, kind?: string) => void };
/**
 * A `fetch` that holds every `/kind` POST until the test answers it, and
 * records how many were in flight at once. `answer` resolves the oldest one.
 */
export function heldKindFetch() {
  const posts: HeldPost[] = [];
  const pending: HeldPost[] = [];
  let inFlight = 0;
  let maxInFlight = 0;
  const fetch_ = async (url: string, init?: { method?: string; body?: string }) => {
    if (!String(url).endsWith('/kind') || init?.method !== 'POST') return { ok: false, status: 404, json: async () => ({}) };
    const body = JSON.parse(init.body ?? '{}') as { kind: string; if_absent?: boolean };
    const work = String(url).split('/')[4]!;
    inFlight += 1; maxInFlight = Math.max(maxInFlight, inFlight);
    const status = await new Promise<{ status: number; kind: string }>((resolve) => {
      const post: HeldPost = { work, kind: body.kind, ifAbsent: body.if_absent === true, answer: (status, kind = body.kind) => resolve({ status, kind }) };
      posts.push(post); pending.push(post);
    });
    inFlight -= 1;
    return { ok: status.status < 300, status: status.status, json: async () => (status.status < 300 ? { kind: status.kind } : { code: 'unavailable' }) };
  };
  /** Answers the oldest held POST, then lets the board run until it is idle. */
  const answer = async (status: number, kind?: string) => { pending.shift()!.answer(status, kind); await settle(); };
  return { fetch_, posts, pending, answer, maxInFlight: () => maxInFlight };
}

/** Lets queued promise chains run to idle. */
export async function settle(): Promise<void> {
  for (let i = 0; i < 20; i += 1) await new Promise((resolve) => setImmediate(resolve));
}
