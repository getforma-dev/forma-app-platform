import assert from 'node:assert/strict';
import { test } from 'node:test';
import { WidgetCanvas, type WidgetCanvasDragEnd } from '../src/canvas-vendor/widget-canvas';
import { preserveCanvasControlPointer } from '../src/canvas/controls.ts';

// Minimal browser primitives, not a substitute drag implementation: all gesture
// listeners, pointer ownership, movement, cancellation and commits run in the
// real vendored WidgetCanvas. Frames are explicit so terminal races are tested.
type Listener = (event: any) => void;
class Target {
  listeners = new Map<string, Set<Listener>>();
  captures = new Set<Listener>();
  addEventListener(type: string, listener: Listener, options?: boolean | {signal?: AbortSignal; capture?: boolean}) {
    const settings = typeof options === 'boolean' ? {capture: options} : options;
    if (settings?.signal?.aborted) return;
    const entries = this.listeners.get(type) ?? new Set<Listener>();
    entries.add(listener); this.listeners.set(type, entries);
    if (settings?.capture) this.captures.add(listener);
    settings?.signal?.addEventListener('abort', () => this.removeEventListener(type, listener), {once: true});
  }
  removeEventListener(type: string, listener: Listener) { this.listeners.get(type)?.delete(listener); }
  emit(type: string, fields: Record<string, unknown> = {}) {
    const event = {type, target: this, defaultPrevented: false, ...fields,
      preventDefault() { this.defaultPrevented = true; }, stopPropagation() {}, stopImmediatePropagation() {}, composedPath: () => [this]};
    for (const listener of [...this.listeners.get(type) ?? []]) listener(event);
    return event;
  }
  dispatch(type: string, fields: Record<string, unknown> = {}) {
    const path: Target[] = [this];
    if (this instanceof Element) {
      for (let parent = this.parentElement; parent; parent = parent.parentElement) path.push(parent);
      path.push(this.ownerDocument, this.ownerDocument.defaultView);
    } else if (this instanceof Document_) path.push(this.defaultView);
    let stopped = false, immediate = false;
    const event = {type, target: this, currentTarget: this as Target, defaultPrevented: false, ...fields,
      preventDefault() { this.defaultPrevented = true; }, stopPropagation() { stopped = true; },
      stopImmediatePropagation() { stopped = true; immediate = true; }, composedPath: () => path};
    const invoke = (target: Target, capture: boolean) => {
      event.currentTarget = target;
      for (const listener of [...target.listeners.get(type) ?? []]) {
        if (target.captures.has(listener) === capture) listener(event);
        if (immediate) break;
      }
    };
    for (const target of [...path].reverse()) { invoke(target, true); if (stopped) return event; }
    for (const target of path) { invoke(target, false); if (stopped) break; }
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
  scrollWidth = 1000; scrollHeight = 800; scrollTop = 0; scrollLeft = 0;
  overflowX = 'visible'; overflowY = 'visible';
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
    if (selector.includes(',')) return selector.split(',').some(part => this.matches(part.trim()));
    if (selector === '[inert]') return this.inert;
    if (selector.startsWith('.')) return this.classList.contains(selector.slice(1));
    if (selector === '[data-canvas-drag-handle]') return 'canvasDragHandle' in this.dataset;
    if (selector === '[contenteditable]:not([contenteditable=\'false\'])') return this.attributes.contenteditable !== undefined && this.attributes.contenteditable !== 'false';
    const attribute = selector.match(/^(\w+)?\[([\w-]+)(?:=['"]?([^'"\]]+)['"]?)?\]$/);
    if (attribute) {
      const [, tag, name, wanted] = attribute;
      const key = name!.replace(/^data-/, '').replace(/-([a-z])/g, (_, letter: string) => letter.toUpperCase());
      const value = name!.startsWith('data-') ? this.dataset[key] : this.attributes[name!];
      return (!tag || this.tag === tag) && value !== undefined && (wanted === undefined || value === wanted);
    }
    return this.tag === selector;
  }
  querySelectorAll(selector: string): Element[] {
    const selectors = selector.split(',').map(value => value.trim());
    return this.children.flatMap(child => [...(selectors.some(value => child.matches(value)) ? [child] : []), ...child.querySelectorAll(selector)]);
  }
  querySelector(selector: string): Element | null { return this.querySelectorAll(selector)[0] ?? null; }
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
  getComputedStyle(element: Element) { return {overflowX: element.overflowX, overflowY: element.overflowY}; }
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
  return target.dispatch(type, {pointerId: id, pointerType: 'mouse', isPrimary: true, button: 0, clientX: x, clientY: y});
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
  return {document, window, canvas, viewport, stage, item, handle, target, events, begin, commits: () => commits};
}

function dispatchPointer(target: Target, type: string, fields: Record<string, unknown> = {}) {
  return target.dispatch(type, {pointerId: 7, pointerType: 'mouse', isPrimary: true, button: 0,
    clientX: 100, clientY: 100, detail: 1, ...fields});
}

function space(target: Target, type = 'keydown', repeat = false) {
  return target.dispatch(type, {code: 'Space', key: ' ', repeat, isComposing: false,
    metaKey: false, ctrlKey: false, altKey: false, shiftKey: false});
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
      assert.equal(f.handle.hasPointerCapture(7), !fallback, 'capture begins only after movement crosses the threshold');
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
    pointer(f.handle, 'pointerdown');
    assert.equal(f.handle.hasPointerCapture(7), false, 'stationary pointer targets must not be retargeted to the handle');
    pointer(f.target, 'pointermove', 7, 103, 104);
    assert.equal(f.handle.hasPointerCapture(7), false, 'the threshold still preserves the original target');
    pointer(f.target, 'pointerup'); f.window.flush();
    assert.deepEqual(f.events, []); assert.equal(f.commits(), 0);
  } finally { f.canvas.dispose(); }
});

test('stationary app-body clicks focus the item and preserve the original double-click target', () => {
  const f = fixture();
  try {
    const composer = f.document.createElement('input'); f.document.body.append(composer); composer.focus();
    const app = f.document.createElement('article'), title = f.document.createElement('h2');
    app.className = 'cv-unite'; app.append(title); f.item.append(app);
    let opened = 0;
    app.addEventListener('dblclick', (event) => {
      assert.equal(event.target, title, 'the app receives its original title target');
      opened++;
    });
    for (const detail of [1, 2]) {
      dispatchPointer(title, 'pointerdown', {detail});
      assert.equal(f.item.hasPointerCapture(7), false);
      assert.equal(f.document.activeElement, f.item, 'selecting the body leaves composer focus');
      dispatchPointer(title, 'pointerup', {detail});
      assert.equal(dispatchPointer(title, 'click', {detail}).defaultPrevented, false);
    }
    assert.equal(dispatchPointer(title, 'dblclick', {detail: 2}).defaultPrevented, false);
    assert.equal(opened, 1);
    assert.deepEqual(f.events, []);
    assert.equal(f.canvas.getItemState(f.item as any).x, 10);
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

test('Move leaves the camera still on a background drag and a background click clears the active item', () => {
  const f = fixture();
  try {
    f.canvas.setActive(f.item as any);
    const camera = f.canvas.getCamera();
    dispatchPointer(f.viewport, 'pointerdown');
    dispatchPointer(f.viewport, 'pointermove', {clientX: 160, clientY: 145});
    dispatchPointer(f.viewport, 'pointerup', {clientX: 160, clientY: 145});
    f.window.flush();
    assert.deepEqual(f.canvas.getCamera(), camera);
    assert.equal(f.viewport.classList.contains('is-panning'), false);
    assert.equal(f.canvas.getItemState(f.item as any).x, 10);
    f.canvas.setActive(f.item as any);
    dispatchPointer(f.stage, 'pointerdown');
    dispatchPointer(f.stage, 'pointerup');
    dispatchPointer(f.stage, 'click');
    assert.equal(f.canvas.activeItem(), null);
  } finally { f.canvas.dispose(); }
});

test('Move drags the whole mounted item from a body inserted after mount and suppresses the resulting click', () => {
  const f = fixture();
  try {
    const body = f.document.createElement('section'), component = f.document.createElement('article');
    body.append(component); f.item.append(body);
    const camera = f.canvas.getCamera();
    dispatchPointer(component, 'pointerdown');
    dispatchPointer(f.item, 'pointermove', {clientX: 140, clientY: 135});
    dispatchPointer(f.item, 'pointerup', {clientX: 150, clientY: 145});
    f.window.flush();
    assert.equal(f.canvas.getItemState(f.item as any).x, 60);
    assert.equal(f.canvas.getItemState(f.item as any).y, 65);
    assert.equal(component.parentElement, body);
    assert.equal(body.parentElement, f.item);
    assert.deepEqual(f.canvas.getCamera(), camera);
    assert.equal(f.events.length, 2);
    assert.equal(f.events[1]?.completion?.kind, 'released');
    let clicks = 0;
    component.addEventListener('click', () => clicks++);
    const click = dispatchPointer(component, 'click');
    assert.equal(click.defaultPrevented, true);
    assert.equal(clicks, 0, 'the completed move cannot activate content under its release');
    dispatchPointer(component, 'pointerdown'); dispatchPointer(f.item, 'pointerup');
    dispatchPointer(component, 'click');
    assert.equal(clicks, 1, 'a subsequent stationary press is not suppressed');
  } finally { f.canvas.dispose(); }
});

test('a new control press clears drag-click suppression before the outer host preserves its native pointer event', () => {
  const f = fixture();
  try {
    const host = f.document.createElement('div'); f.document.body.append(host); host.append(f.viewport);
    const app = f.document.createElement('article'), button = f.document.createElement('button');
    app.className = 'cv-unite'; app.append(button); f.item.append(app);
    let preserved = 0, viewportPresses = 0, clicks = 0;
    host.addEventListener('pointerdown', (event) => {
      if (preserveCanvasControlPointer(event as PointerEvent, host as unknown as HTMLElement)) preserved++;
    }, {capture: true});
    f.viewport.addEventListener('pointerdown', () => viewportPresses++, {capture: true});
    button.addEventListener('click', () => clicks++);
    dispatchPointer(app, 'pointerdown');
    dispatchPointer(f.item, 'pointermove', {clientX: 140, clientY: 130});
    dispatchPointer(f.item, 'pointerup', {clientX: 140, clientY: 130}); f.window.flush();
    assert.equal(dispatchPointer(app, 'click').defaultPrevented, true, 'the old drag click is suppressed');
    const priorViewportPresses = viewportPresses;
    const press = dispatchPointer(button, 'pointerdown');
    assert.equal(preserved, 1, 'the actual control guard intercepted the new press at the host');
    assert.equal(viewportPresses, priorViewportPresses, 'the new press never reaches the canvas viewport');
    assert.equal(press.defaultPrevented, false, 'native focus and activation remain available');
    dispatchPointer(button, 'pointerup');
    assert.equal(dispatchPointer(button, 'click').defaultPrevented, false);
    assert.equal(clicks, 1, 'stale drag suppression must not swallow the new control action');
  } finally { f.canvas.dispose(); }
});

test('Move leaves nested native controls available without moving their app', () => {
  for (const tag of ['button', 'input', 'select', 'textarea', 'label', 'summary', 'a']) {
    const f = fixture();
    try {
      const control = f.document.createElement(tag), child = f.document.createElement('span');
      if (tag === 'a') control.setAttribute('href', '/details');
      control.append(child); f.item.append(control);
      const camera = f.canvas.getCamera();
      const press = dispatchPointer(child, 'pointerdown');
      dispatchPointer(f.item, 'pointermove', {clientX: 150, clientY: 130});
      dispatchPointer(f.item, 'pointerup', {clientX: 150, clientY: 130});
      assert.equal(press.defaultPrevented, false, `${tag}: native focus is preserved`);
      assert.equal(f.canvas.getItemState(f.item as any).x, 10, tag);
      assert.deepEqual(f.canvas.getCamera(), camera, tag);
      assert.deepEqual(f.events, [], tag);
      let clicks = 0;
      control.addEventListener('click', () => clicks++);
      dispatchPointer(child, 'click');
      assert.equal(clicks, 1, `${tag}: native activation is available`);
    } finally { f.canvas.dispose(); }
  }
});

test('Move leaves an overflowing region and its scrollbar available for native scrolling', () => {
  const f = fixture();
  try {
    const scroll = f.document.createElement('section'), row = f.document.createElement('div');
    scroll.overflowY = 'auto'; scroll.clientHeight = 100; scroll.scrollHeight = 500;
    scroll.append(row); f.item.append(scroll);
    for (const target of [scroll, row]) {
      const press = dispatchPointer(target, 'pointerdown');
      dispatchPointer(f.item, 'pointermove', {clientX: 140, clientY: 150});
      dispatchPointer(f.item, 'pointerup', {clientX: 140, clientY: 150});
      assert.equal(press.defaultPrevented, false);
      assert.equal(f.canvas.getItemState(f.item as any).x, 10);
      assert.deepEqual(f.events, []);
    }
  } finally { f.canvas.dispose(); }
});

test('native wheel scrolling stays inside app content at either edge in Move and Hand', () => {
  const f = fixture();
  try {
    const scroll = f.document.createElement('section'), row = f.document.createElement('div');
    scroll.overflowY = 'auto'; scroll.clientHeight = 100; scroll.scrollHeight = 500;
    scroll.append(row); f.item.append(scroll);
    const camera = f.canvas.getCamera();
    for (const hand of [false, true]) {
      f.canvas.setPanMode(hand);
      for (const [scrollTop, deltaY] of [[0, -40], [400, 40]]) {
        scroll.scrollTop = scrollTop!;
        const wheel = row.dispatch('wheel', {deltaMode: 0, deltaX: 0, deltaY, ctrlKey: false, metaKey: false, shiftKey: false});
        assert.equal(wheel.defaultPrevented, false, 'native scroll remains available at the boundary');
        assert.deepEqual(f.canvas.getCamera(), camera, 'the camera must not take over at a scroll boundary');
      }
    }
  } finally { f.canvas.dispose(); }
});

for (const mode of ['hand', 'middle'] as const) {
  test(`${mode} pans over item content without moving/selecting it or activating its release target`, () => {
    const f = fixture();
    try {
      const button = f.document.createElement('button'); f.item.append(button);
      const original = f.canvas.getCamera();
      if (mode === 'hand') f.canvas.setPanMode(true);
      const mouseButton = mode === 'middle' ? 1 : 0;
      dispatchPointer(button, 'pointerdown', {button: mouseButton});
      dispatchPointer(f.viewport, 'pointermove', {button: mouseButton, clientX: 140, clientY: 130});
      dispatchPointer(f.viewport, 'pointerup', {button: mouseButton, clientX: 140, clientY: 130});
      f.window.flush();
      assert.deepEqual(f.canvas.getCamera(), {...original, panX: original.panX + 40, panY: original.panY + 30});
      assert.equal(f.canvas.getItemState(f.item as any).x, 10);
      assert.equal(f.canvas.activeItem(), null);
      assert.deepEqual(f.events, []);
      let clicks = 0; button.addEventListener('click', () => clicks++);
      const click = dispatchPointer(button, 'click', {button: mouseButton});
      assert.equal(click.defaultPrevented, true);
      assert.equal(clicks, 0);
    } finally { f.canvas.dispose(); }
  });
}

test('Space from the Move tool temporarily pans, keeps an active pan on keyup, and returns to item movement', () => {
  const f = fixture();
  try {
    const tool = f.document.createElement('button'); tool.dataset.tool = 'move';
    f.document.body.append(tool); tool.focus();
    const key = space(tool);
    assert.equal(key.defaultPrevented, true);
    assert.equal(f.viewport.classList.contains('is-pan-ready'), true);
    assert.equal(space(tool, 'keydown', true).defaultPrevented, true, 'held Space cannot repeat the focused toolbar action');
    const camera = f.canvas.getCamera();
    dispatchPointer(f.item, 'pointerdown');
    dispatchPointer(f.viewport, 'pointermove', {clientX: 130, clientY: 125});
    assert.equal(f.viewport.classList.contains('is-panning'), true);
    space(tool, 'keyup');
    assert.equal(f.viewport.classList.contains('is-pan-ready'), false);
    assert.equal(f.viewport.classList.contains('is-panning'), true, 'pointer release owns the running gesture');
    dispatchPointer(f.viewport, 'pointerup', {clientX: 130, clientY: 125});
    f.window.flush();
    assert.equal(f.viewport.classList.contains('is-panning'), false);
    assert.equal(f.viewport.classList.contains('is-hand-tool'), false);
    assert.deepEqual(f.canvas.getCamera(), {...camera, panX: camera.panX + 30, panY: camera.panY + 25});
    dispatchPointer(f.item, 'pointerdown');
    dispatchPointer(f.item, 'pointermove', {clientX: 120, clientY: 115});
    dispatchPointer(f.item, 'pointerup', {clientX: 120, clientY: 115});
    f.window.flush();
    assert.equal(f.canvas.getItemState(f.item as any).x, 30);
    assert.equal(f.canvas.getItemState(f.item as any).y, 35);
  } finally { f.canvas.dispose(); }
});

test('Space on native controls retains keyboard activation and text entry', () => {
  for (const tag of ['button', 'input', 'textarea', 'select', 'summary']) {
    const f = fixture();
    try {
      const control = f.document.createElement(tag); f.item.append(control); control.focus();
      const key = space(control);
      assert.equal(key.defaultPrevented, false, tag);
      assert.equal(f.viewport.classList.contains('is-pan-ready'), false, tag);
    } finally { f.canvas.dispose(); }
  }
});

test('releasing Space from the Hand tool keeps the persistent Hand mode', () => {
  const f = fixture();
  try {
    const tool = f.document.createElement('button'); tool.dataset.tool = 'hand';
    f.document.body.append(tool); tool.focus(); f.canvas.setPanMode(true);
    assert.equal(space(tool).defaultPrevented, true);
    space(tool, 'keyup');
    assert.equal(f.viewport.classList.contains('is-pan-ready'), false);
    assert.equal(f.viewport.classList.contains('is-hand-tool'), true);
    const camera = f.canvas.getCamera();
    dispatchPointer(f.item, 'pointerdown');
    dispatchPointer(f.viewport, 'pointermove', {clientX: 130, clientY: 120});
    dispatchPointer(f.viewport, 'pointerup', {clientX: 130, clientY: 120});
    assert.deepEqual(f.canvas.getCamera(), {...camera, panX: camera.panX + 30, panY: camera.panY + 20});
    assert.equal(f.canvas.getItemState(f.item as any).x, 10);
  } finally { f.canvas.dispose(); }
});

test('blur cancels temporary pan and a late release cannot resume it', () => {
  const f = fixture();
  try {
    f.viewport.focus(); space(f.viewport);
    dispatchPointer(f.item, 'pointerdown');
    dispatchPointer(f.viewport, 'pointermove', {clientX: 130, clientY: 120});
    f.window.emit('blur'); f.window.flush();
    const camera = f.canvas.getCamera(), commits = f.commits();
    assert.equal(f.viewport.classList.contains('is-pan-ready'), false);
    assert.equal(f.viewport.classList.contains('is-panning'), false);
    dispatchPointer(f.viewport, 'pointermove', {clientX: 900, clientY: 900});
    dispatchPointer(f.viewport, 'pointerup', {clientX: 900, clientY: 900});
    assert.deepEqual(f.canvas.getCamera(), camera);
    assert.equal(f.commits(), commits);
  } finally { f.canvas.dispose(); }
});

test('changing tools cancels a running item drag without reporting a released drop', () => {
  const f = fixture();
  try {
    f.begin(); f.canvas.setPanMode(true); f.window.flush();
    assert.equal(f.viewport.classList.contains('is-hand-tool'), true);
    assert.equal(f.item.classList.contains('is-dragging'), false);
    assert.deepEqual(f.events, [{dragging: true, completion: null}, {dragging: false, completion: {kind: 'cancelled'}}]);
    pointer(f.target, 'pointerup', 7, 900, 900); f.window.flush();
    assert.equal(f.canvas.getItemState(f.item as any).x, 40);
    assert.equal(f.events.length, 2);
  } finally { f.canvas.dispose(); }
});

test('changing from Hand to Move ends the active pan before the next pointer movement', () => {
  const f = fixture();
  try {
    f.canvas.setPanMode(true);
    dispatchPointer(f.item, 'pointerdown');
    dispatchPointer(f.viewport, 'pointermove', {clientX: 130, clientY: 120});
    f.canvas.setPanMode(false);
    const camera = f.canvas.getCamera(), commits = f.commits();
    assert.equal(f.viewport.classList.contains('is-hand-tool'), false);
    assert.equal(f.viewport.classList.contains('is-panning'), false);
    dispatchPointer(f.viewport, 'pointermove', {clientX: 900, clientY: 900});
    dispatchPointer(f.viewport, 'pointerup', {clientX: 900, clientY: 900});
    assert.deepEqual(f.canvas.getCamera(), camera);
    assert.equal(f.commits(), commits);
  } finally { f.canvas.dispose(); }
});
