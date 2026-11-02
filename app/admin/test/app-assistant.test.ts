import assert from 'node:assert/strict';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {build} from 'esbuild';

const APP = 'aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee', WORK = '11111111-2222-4333-8444-555555555555';
const FILE = '22222222-2222-4333-8444-555555555555', OTHER = '33333333-2222-4333-8444-555555555555';
let serial = 0;
class Element {
  attrs = new Map<string, string>(); dataset: Record<string, string> = {}; children: Element[] = [];
  parent: Element | null = null; ownerDocument: any; hidden = false; inert = false; focused = false; attached = true; id = ''; text = '';
  events: any[] = []; listeners = new Map<string, ((event: any) => void)[]>();
  constructor(readonly tag: string, doc: unknown) {this.ownerDocument = doc;}
  get isConnected(): boolean {return this.attached && (!this.parent || this.parent.isConnected);}
  get textContent(): string {return this.text + this.children.map(child => child.textContent).join('');}
  setAttribute(name: string, value: string) {this.attrs.set(name, value); if (name.startsWith('data-')) this.dataset[name.slice(5).replace(/-([a-z])/g, (_, c) => c.toUpperCase())] = value;}
  append(...values: (Element | string)[]) {for (const value of values) {if (typeof value === 'string') this.text += value; else {value.parent = this; this.children.push(value);}}}
  replaceChildren(...values: (Element | string)[]) {for (const old of this.children) old.parent = null; this.children = []; this.text = ''; this.append(...values);}
  addEventListener(name: string, listener: (event: any) => void) {this.listeners.set(name, [...(this.listeners.get(name) ?? []), listener]);}
  removeEventListener(name: string, listener: (event: any) => void) {this.listeners.set(name, (this.listeners.get(name) ?? []).filter(value => value !== listener));}
  dispatchEvent(event: unknown) {this.events.push(event); return true;}
  dispatch(name: string, values: Record<string, unknown> = {}) {
    const event = {defaultPrevented: false, stopped: false, key: '', preventDefault() {this.defaultPrevented = true;}, stopPropagation() {this.stopped = true;}, ...values};
    for (let node: Element | null = this; node; node = event.stopped ? null : node.parent) for (const listener of node.listeners.get(name) ?? []) listener(event);
    return event;
  }
  click() {this.dispatch('click');}
  focus() {this.focused = true;}
  matches(selector: string) {
    const attr = /^\[([^=\]]+)(?:="([^"]*)")?\]$/.exec(selector);
    if (attr) return attr[2] === undefined ? this.attrs.has(attr[1]!) : this.attrs.get(attr[1]!) === attr[2];
    return selector.startsWith('.') ? (this.attrs.get('class') ?? '').split(' ').includes(selector.slice(1)) : this.tag === selector;
  }
  closest(selector: string): Element | null {return this.matches(selector) ? this : this.parent?.closest(selector) ?? null;}
  querySelectorAll(selector: string): Element[] {return this.children.flatMap(child => [...(child.matches(selector) ? [child] : []), ...child.querySelectorAll(selector)]);}
  querySelector(selector: string): Element | null {return this.querySelectorAll(selector)[0] ?? null;}
}

async function fixture() {
  const entry = fileURLToPath(new URL('../src/canvas/app-assistant.ts', import.meta.url));
  const doc = {}, el = (_doc: unknown, tag: string, attrs: Record<string, string> = {}, ...children: any[]) => {
    const node = new Element(tag, doc); for (const [key, value] of Object.entries(attrs)) node.setAttribute(key, value); node.append(...children); return node;
  };
  const mounts: {kind: string; host: Element; options: any; calls: [string, unknown?][]; controller: any}[] = [];
  const mount = (kind: string) => (host: Element, options: any) => {
    const calls: [string, unknown?][] = [];
    const controller = Object.fromEntries(['show', 'hide', 'dispose', 'focus', 'ask', 'setSource', 'setRecordScope', 'openFile'].map(method => [method, (value?: unknown) => {calls.push([method, value]);}]));
    mounts.push({kind, host, options, calls, controller}); return controller;
  };
  const mocks: Record<string, Record<string, any>> = {
    './render': {el, isUuid: (value: string) => /^[a-f0-9-]{36}$/.test(value)},
    './app-window': {appWindowIcon: () => el(doc, 'svg')},
    './app-chat': {mountAppChat: mount('ask')}, './explain': {mountAppSummary: mount('summary')},
    './app-sources': {mountAppSources: mount('sources')},
    './app-saved-answers': {mountAppSavedAnswers: mount('saved'), renderAppAnswerActions: () => el(doc, 'button')},
  };
  const key = `__assistantTest${serial++}`; (globalThis as any)[key] = mocks;
  const built = await build({entryPoints: [entry], bundle: true, platform: 'node', format: 'esm', write: false,
    plugins: [{name: 'assistant-boundaries', setup(builder) {
      builder.onResolve({filter: /.*/}, args => args.importer === entry && mocks[args.path] ? {path: args.path, namespace: 'assistant-boundary'} : undefined);
      builder.onLoad({filter: /.*/, namespace: 'assistant-boundary'}, args => ({loader: 'js', contents: Object.keys(mocks[args.path]!).map(name => `export const ${name} = (...args) => globalThis[${JSON.stringify(key)}][${JSON.stringify(args.path)}][${JSON.stringify(name)}](...args);`).join('\n')}));
    }}],
  });
  const api = await import('data:text/javascript;base64,' + Buffer.from(built.outputFiles[0]!.text).toString('base64'));
  const frame = el(doc, 'article', {class: 'cv-unite'}), host = el(doc, 'aside');
  for (const tab of ['ask', 'summary', 'sources']) frame.append(el(doc, 'button', {'data-app-assistant': tab}));
  frame.append(host);
  const options = {app: APP, work: WORK, title: 'Checking', address: '/checking'};
  const open = (tab = 'ask', request: unknown = {}) => api.showAppAssistant(host, options, tab, request);
  return {api, frame, host, el, options, mounts, open, view: (kind: string) => mounts.find(value => value.kind === kind)!,
    tab: (name: string) => host.querySelectorAll('[role="tab"]').find(value => value.textContent === name)!,
    cleanup: () => {api.disposeAppAssistant(APP, WORK); delete (globalThis as any)[key];}};
}

test('one app companion lazily mounts views and preserves them across tab changes and close', async () => {
  const f = await fixture();
  try {
    f.open(); assert.deepEqual(f.mounts.map(value => value.kind), ['ask']); assert.equal(f.host.hidden, false);
    assert.equal(f.view('ask').options.address, '/checking');
    f.tab('Summary').click(); assert.deepEqual(f.mounts.map(value => value.kind), ['ask', 'summary']);
    assert.equal(f.view('ask').calls.at(-1)?.[0], 'hide'); assert.equal(f.view('summary').calls.at(-1)?.[0], 'show');
    const panel = f.host.querySelector('.cv-app-summary-host')!;
    f.host.querySelector('[aria-label="Close app panel"]')!.click();
    assert.equal(f.host.hidden, true); assert.equal(f.host.inert, true); assert.equal(f.view('summary').calls.at(-1)?.[0], 'hide');
    f.open('summary'); assert.equal(f.host.querySelector('.cv-app-summary-host'), panel); assert.equal(f.mounts.length, 2);
    assert.equal(f.frame.querySelector('[data-app-assistant="summary"]')!.attrs.get('aria-expanded'), 'true');
  } finally {f.cleanup();}
});

test('source selection and citations retain the exact file when crossing app-owned tabs', async () => {
  const f = await fixture();
  try {
    f.open('sources');
    f.view('sources').options.onAskFile({file: {file_id: FILE}});
    assert.ok(f.view('ask').calls.some(value => value[0] === 'setSource' && value[1] === FILE));
    assert.equal(f.tab('Ask').attrs.get('aria-selected'), 'true');
    f.view('ask').options.onOpenSource(FILE);
    assert.equal(f.tab('Sources').attrs.get('aria-selected'), 'true');
    assert.deepEqual(f.view('sources').calls.at(-1), ['openFile', FILE]);
    f.open('sources', {toggle: true}); const before = f.view('ask').calls.length;
    f.view('sources').options.onAskFile({file: {file_id: OTHER}}); assert.equal(f.view('ask').calls.length, before);
  } finally {f.cleanup();}
});

test('incoming addressed questions preserve the exact component scope; ordinary Ask resets that override', async () => {
  const f = await fixture();
  try {
    const scope = {kind: 'card', address: 'checking.transactions', works: [{app_id: APP, work_id: WORK, address: 'checking', collection: OTHER}]};
    f.open('ask', {question: 'Which records?', scope});
    assert.deepEqual(f.view('ask').calls.slice(0, 3), [['setRecordScope', scope], ['show', undefined], ['ask', 'Which records?']]);
    f.open('ask'); assert.ok(f.view('ask').calls.some(value => value[0] === 'setRecordScope' && value[1] === null));
    assert.equal(f.mounts.length, 1);
  } finally {f.cleanup();}
});

test('keyboard tabs do not move canvas items and Escape closes the panel with launcher focus', async () => {
  const f = await fixture();
  try {
    let canvasKeys = 0; f.frame.addEventListener('keydown', () => {canvasKeys += 1;});
    f.open(); const list = f.host.querySelector('[role="tablist"]')!;
    const arrow = list.dispatch('keydown', {key: 'ArrowRight'});
    assert.equal(arrow.defaultPrevented, true); assert.equal(arrow.stopped, true); assert.equal(canvasKeys, 0);
    assert.equal(f.tab('Summary').attrs.get('aria-selected'), 'true'); assert.equal(f.tab('Summary').focused, true);
    f.host.dispatch('keydown', {key: 'Escape'}); assert.equal(f.host.hidden, true);
    assert.equal(f.frame.querySelector('[data-app-assistant="summary"]')!.focused, true); assert.equal(canvasKeys, 0);
  } finally {f.cleanup();}
});

test('remounting an app disposes the old host and stale source callbacks cannot reopen it', async () => {
  const f = await fixture();
  try {
    f.open('sources'); const oldSources = f.view('sources'), oldChat = f.view('ask');
    const replacement = f.el({}, 'aside'); f.frame.append(replacement);
    f.api.showAppAssistant(replacement, f.options, 'summary');
    assert.equal(oldSources.calls.at(-1)?.[0], 'dispose'); assert.equal(oldChat.calls.at(-1)?.[0], 'dispose');
    assert.equal(f.host.children.length, 0); const count = oldChat.calls.length;
    oldSources.options.onAskFile({file: {file_id: FILE}}); assert.equal(oldChat.calls.length, count); assert.equal(f.host.hidden, true);
    f.api.disposeAppAssistant(APP, WORK); assert.equal(replacement.children.length, 0);
  } finally {f.cleanup();}
});
