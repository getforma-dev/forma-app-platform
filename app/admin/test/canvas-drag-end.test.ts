import assert from 'node:assert/strict';
import { test } from 'node:test';
import { WidgetCanvas, type WidgetCanvasDragEnd } from '../src/canvas-vendor/widget-canvas';

// Minimal browser primitives, not a substitute drag implementation: all gesture
// listeners, pointer ownership, movement, cancellation and commits run in the
// real vendored WidgetCanvas. Frames are explicit so terminal races are tested.
type Listener = (event: any) => void;
class Target {
  listeners = new Map<string, Set<Listener>>();
  addEventListener(type: string, listener: Listener, options?: {signal?: AbortSignal}) {
    if (options?.signal?.aborted) return;
    const entries = this.listeners.get(type) ?? new Set<Listener>();
    entries.add(listener); this.listeners.set(type, entries);
    options?.signal?.addEventListener('abort', () => this.removeEventListener(type, listener), {once: true});
  }
  removeEventListener(type: string, listener: Listener) { this.listeners.get(type)?.delete(listener); }
  emit(type: string, fields: Record<string, unknown> = {}) {
    const event = {type, target: this, defaultPrevented: false, ...fields,
      preventDefault() { this.defaultPrevented = true; }, stopPropagation() {}, composedPath: () => [this]};
    for (const listener of [...this.listeners.get(type) ?? []]) listener(event);
    return event;
  }
}
class Classes {
  values = new Set<string>();
  contains(name: string) { return this.values.has(name); }
  add(...names: string[]) { names.forEach(name => this.values.add(name)); }
  remove(...names: string[]) { names.forEach(name => this.values.delete(name)); }
  toggle(name: string, force = !this.contains(name)) { if (force) this.add(name); else this.remove(name); return force; }
}
class Element extends Target {
  dataset: Record<string, string> = {};
  attributes: Record<string, string> = {};
  classList = new Classes();
  style: Record<string, any> = {setProperty(name: string, value: string) { this[name] = value; }, removeProperty(name: string) { delete this[name]; }};
  children: Element[] = [];
  parentElement: Element | null = null;
  inert = false; tabIndex = -1; textContent = ''; type = ''; title = '';
  clientWidth = 1000; clientHeight = 800;
  capture = new Set<number>(); captureFails = false;
  constructor(readonly ownerDocument: Document_, readonly tag = 'div') { super(); }
  set className(value: string) { this.classList.values = new Set(value.split(/\s+/).filter(Boolean)); }
  get className() { return [...this.classList.values].join(' '); }
  get isConnected(): boolean { return this === this.ownerDocument.body || (this.parentElement?.isConnected ?? false); }
  append(...children: Element[]) { for (const child of children) { child.remove(); child.parentElement = this; this.children.push(child); } }
  remove() { if (this.parentElement) this.parentElement.children = this.parentElement.children.filter(child => child !== this); this.parentElement = null; }
  contains(child: Element | null): boolean { return child === this || this.children.some(node => node.contains(child)); }
  setAttribute(name: string, value: string) { this.attributes[name] = value; }
  getAttribute(name: string) { return this.attributes[name] ?? null; }
  removeAttribute(name: string) { delete this.attributes[name]; }
  matches(selector: string): boolean {
    if (selector === '[inert]') return this.inert;
    if (selector.startsWith('.')) return this.classList.contains(selector.slice(1));
    if (selector === '[data-canvas-drag-handle]') return 'canvasDragHandle' in this.dataset;
    return false;
  }
  querySelectorAll(selector: string): Element[] {
    const selectors = selector.split(',').map(value => value.trim());
    return this.children.flatMap(child => [...(selectors.some(value => child.matches(value)) ? [child] : []), ...child.querySelectorAll(selector)]);
  }
  closest(selector: string): Element | null { return this.matches(selector) ? this : this.parentElement?.closest(selector) ?? null; }
  getBoundingClientRect() { return {x: 0, y: 0, left: 0, top: 0, right: this.clientWidth, bottom: this.clientHeight, width: this.clientWidth, height: this.clientHeight}; }
  focus() { this.ownerDocument.activeElement = this; }
  setPointerCapture(id: number) { if (this.captureFails) throw new Error('capture unavailable'); this.capture.add(id); }
  hasPointerCapture(id: number) { return this.capture.has(id); }
  releasePointerCapture(id: number) { this.capture.delete(id); this.emit('lostpointercapture', {pointerId: id}); }
}
class Window_ extends Target {
  AbortController = AbortController; HTMLElement = Element; Element = Element; HTMLButtonElement = Element;
  ResizeObserver = class { observe() {} unobserve() {} disconnect() {} };
  performance = {now: () => 1000};
  frames = new Map<number, FrameRequestCallback>(); next = 1;
  requestAnimationFrame(callback: FrameRequestCallback) { const id = this.next++; this.frames.set(id, callback); return id; }
  cancelAnimationFrame(id: number) { this.frames.delete(id); }
  setTimeout() { return this.next++; } clearTimeout() {}
  flush() {
    for (let cycle = 0; this.frames.size; cycle++) {
      assert.ok(cycle < 20, 'frame queue settles');
      const frames = [...this.frames]; this.frames.clear();
      for (const [, callback] of frames) callback(1000);
    }
  }
}
class Document_ extends Target {
  defaultView = new Window_(); body = new Element(this); activeElement: Element | null = null; hidden = false;
  createElement(tag: string) { return new Element(this, tag); }
  querySelector() { return null; }
}
function pointer(target: Target, type: string, id = 7, x = 100, y = 100) {
  return target.emit(type, {pointerId: id, pointerType: 'mouse', isPrimary: true, button: 0, clientX: x, clientY: y});
}
function fixture(fallback = false) {
  const document = new Document_(), window = document.defaultView;
  const viewport = document.createElement('main'), stage = document.createElement('div');
  const zoomStatus = document.createElement('button'), navigator = document.createElement('div');
  const navigatorItems = document.createElement('div'), navigatorViewport = document.createElement('div');
  document.body.append(viewport); viewport.append(stage, zoomStatus, navigator); navigator.append(navigatorItems, navigatorViewport);
  const events: {dragging: boolean; completion: WidgetCanvasDragEnd | null}[] = [];
  let commits = 0;
  const canvas = new WidgetCanvas({viewport, stage, zoomStatus, navigator, navigatorItems, navigatorViewport} as any, {
    onCommit: () => commits++, onActiveDragStateChange: (_item, dragging, completion) => events.push({dragging, completion}),
  });
  const item = document.createElement('article'), handle = document.createElement('button');
  item.dataset.instanceId = 'sheet-one'; item.className = 'widget-instance';
  handle.className = 'widget-drag-handle'; handle.captureFails = fallback; item.append(handle);
  canvas.mountItem(item as any, {x: 10, y: 20, width: 400, height: 300}, {focus: false}); window.flush();
  const target = fallback ? window : handle;
  const begin = () => { pointer(handle, 'pointerdown'); pointer(target, 'pointermove', 7, 130, 140); };
  return {document, window, canvas, viewport, item, handle, target, events, begin, commits: () => commits};
}

for (const end of ['explicit cancellation', 'pointercancel', 'lostpointercapture', 'window blur', 'hidden document'] as const) {
  test(`${end} commits moved geometry but never reports a released drop`, () => {
    const f = fixture();
    try {
      // An unrelated old document release must not become this gesture's drop.
      pointer(f.document, 'pointerup', 2, 800, 600);
      f.begin();
      if (end === 'explicit cancellation') f.canvas.cancelGestures();
      else if (end === 'window blur') f.window.emit('blur');
      else if (end === 'hidden document') { f.document.hidden = true; f.document.emit('visibilitychange'); }
      else pointer(f.target, end, 7, 900, 700);
      assert.equal(f.commits(), 1, 'movement is durable before the cleanup frame');
      assert.equal(f.canvas.getItemState(f.item as any).x, 40);
      assert.equal(f.canvas.getItemState(f.item as any).y, 60);
      f.window.flush();
      assert.deepEqual(f.events, [{dragging: true, completion: null}, {dragging: false, completion: {kind: 'cancelled'}}]);
      assert.equal(f.item.classList.contains('is-dragging'), false);
      assert.equal(f.viewport.classList.contains('is-dragging-widget'), false);
      pointer(f.target, 'pointerup', 7, 900, 700); f.window.flush();
      assert.equal(f.commits(), 1); assert.equal(f.events.length, 2, 'late release cannot revive a canceled gesture');
    } finally { f.canvas.dispose(); }
  });
}

for (const fallback of [false, true]) {
  test(`matching pointerup carries only that gesture's release coordinates (${fallback ? 'window fallback' : 'capture'})`, () => {
    const f = fixture(fallback);
    try {
      f.begin();
      pointer(f.target, 'pointermove', 8, 900, 700);
      pointer(f.target, 'pointerup', 8, 900, 700);
      pointer(f.target, 'pointercancel', 8, 900, 700);
      pointer(f.target, 'lostpointercapture', 8, 900, 700);
      assert.equal(f.commits(), 0, 'foreign pointer cannot end the active gesture');
      pointer(f.target, 'pointerup', 7, 160, 180);
      pointer(f.document, 'pointerup', 9, 950, 750);
      assert.equal(f.commits(), 1);
      f.window.flush();
      assert.deepEqual(f.events, [{dragging: true, completion: null}, {dragging: false, completion: {kind: 'released', pointerId: 7, x: 160, y: 180}}]);
      assert.equal(f.canvas.getItemState(f.item as any).x, 70);
      assert.equal(f.canvas.getItemState(f.item as any).y, 100);
      pointer(f.target, 'lostpointercapture', 7); pointer(f.target, 'pointerup', 7); f.window.flush();
      assert.equal(f.commits(), 1); assert.equal(f.events.length, 2, 'release/capture cleanup fires only once');
    } finally { f.canvas.dispose(); }
  });
}

test('a click below the drag threshold cannot drop or commit movement', () => {
  const f = fixture();
  try {
    pointer(f.handle, 'pointerdown'); pointer(f.target, 'pointermove', 7, 103, 104); pointer(f.target, 'pointerup'); f.window.flush();
    assert.deepEqual(f.events, []); assert.equal(f.commits(), 0);
  } finally { f.canvas.dispose(); }
});

test('removing an item during a moved gesture cancels without a detached drop callback', () => {
  const f = fixture();
  try {
    f.begin(); f.canvas.removeItem(f.item as any); f.window.flush();
    assert.equal(f.commits(), 1); assert.equal(f.item.isConnected, false);
    assert.deepEqual(f.events, [{dragging: true, completion: null}]);
    pointer(f.target, 'pointerup'); f.window.flush(); assert.equal(f.events.length, 1);
  } finally { f.canvas.dispose(); }
});
