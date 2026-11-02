import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {build} from 'esbuild';

const APP = 'aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee';
const WORK = '11111111-2222-4333-8444-555555555555';
let serial = 0;
const empty = {state: 'none', findings: []};
const ready = {state: 'ready', findings: [{parts: [{text: 'A saved finding.'}]}]};
const response = (data: unknown, status = 200) => ({ok: status >= 200 && status < 300, status, json: async () => data});
const flush = async () => { for (let i = 0; i < 12; i++) await Promise.resolve(); };

/** Minimal DOM host; real module code owns requests, cancellation and rendering. */
class Element {
  attrs = new Map<string, string>();
  children: (Element|string)[] = [];
  parent: Element|null = null;
  hidden = false; inert = false; disabled = false; focused = false; attached = true;
  listeners = new Map<string, (() => void)[]>();
  events: unknown[] = [];
  ownerDocument: any;
  constructor(readonly tag: string, document_: unknown) { this.ownerDocument = document_; }
  get isConnected(): boolean { return this.attached && (!this.parent || this.parent.isConnected); }
  set textContent(value: string) { this.replaceChildren(value); }
  get textContent(): string { return this.children.map(child => typeof child === 'string' ? child : child.textContent).join(''); }
  setAttribute(name: string, value: string) {this.attrs.set(name, value);}
  removeAttribute(name: string) {this.attrs.delete(name);}
  append(...children: (Element|string)[]) {for (const child of children) {if (child instanceof Element) child.parent = this; this.children.push(child);}}
  replaceChildren(...children: (Element|string)[]) {for (const child of this.children) if (child instanceof Element) child.parent = null; this.children = [];this.append(...children);}
  addEventListener(name: string, handler: () => void) {this.listeners.set(name, [...(this.listeners.get(name) ?? []), handler]);}
  dispatchEvent(event: unknown) {this.events.push(event);return true;}
  click() {for (const handler of this.listeners.get('click') ?? []) handler();}
  focus() {this.focused = true;}
  matches(selector: string) {
    if (selector.startsWith('.')) return (this.attrs.get('class') ?? '').split(' ').includes(selector.slice(1));
    if (selector.startsWith('[')) return this.attrs.has(selector.slice(1, -1));
    return this.tag === selector;
  }
  closest(selector: string): Element|null {return this.matches(selector) ? this : this.parent?.closest(selector) ?? null;}
  querySelector(selector: string): Element|null {
    for (const child of this.children) if (child instanceof Element) {
      if (child.matches(selector)) return child;
      const nested = child.querySelector(selector); if (nested) return nested;
    }
    return null;
  }
}

async function fixture(options: {lineage?: boolean} = {}) {
  const entry = fileURLToPath(new URL('../src/canvas/explain.ts', import.meta.url));
  const source = await readFile(entry, 'utf8');
  const exports = new Map<string, string[]>();
  for (const [, names, path] of source.matchAll(/import\s*\{([^}]+)\}\s*from\s*['"]([^'"]+)['"]/gs))
    exports.set(path!, names!.split(',').map(name => name.trim()).filter(name => name && !name.startsWith('type ')));
  const document_ = {};
  const el = (_doc: unknown, tag: string, attrs: Record<string, string> = {}, ...children: any[]) => {
    const node = new Element(tag, document_);for (const [key,value] of Object.entries(attrs)) node.setAttribute(key,value);node.append(...children);return node;
  };
  const frame = el(document_, 'article', {class: 'cv-item-unite'});
  const button = el(document_, 'button', {'data-app-summary': ''});
  const host = el(document_, 'aside', {id: `app-summary-${WORK}`, 'data-app-companion': ''});host.hidden = true;
  frame.append(button, host);
  let listener = () => {};
  let ensureGroup = async (): Promise<any> => null;
  const focused: string[] = [], placements: unknown[][] = [];
  const mocks = {
    './board': {onBoardChange: (fn: () => void) => {listener = fn;}, ensureAppGroup: () => ensureGroup(), isOnBoard: () => false},
    './focus': {openFocus: (id: string) => {focused.push(id); return true;}},
    './unite': {setAppView: async (...args: unknown[]) => {placements.push(args);}},
    '../presentation/presentation': {appComponentVisible: () => true},
    './box-api': {lineage: async () => options.lineage === false ? null : {}},
    './render': {el, isUuid: (value: string) => /^[0-9a-f-]{36}$/.test(value)},
  };
  const key = `__summaryTest${serial++}`;
  (globalThis as any)[key] = mocks;
  const result = await build({entryPoints: [entry],bundle:true,format:'esm',platform:'node',write:false,
    plugins:[{name:'summary-boundaries',setup(builder) {
      builder.onResolve({filter:/.*/},args => args.importer === entry && exports.has(args.path) ? {path:args.path,namespace:'summary-spy'} : undefined);
      builder.onLoad({filter:/.*/,namespace:'summary-spy'},args=>({contents:exports.get(args.path)!.map(name=>`export const ${name} = (...args) => globalThis[${JSON.stringify(key)}][${JSON.stringify(args.path)}]?.[${JSON.stringify(name)}]?.(...args);`).join('\n'),loader:'js'}));
    }}],
  });
  const api = await import('data:text/javascript;base64,'+Buffer.from(result.outputFiles[0]!.text).toString('base64'));
  const oldFetch = globalThis.fetch;
  const calls: {method:string;signal:AbortSignal}[] = [];
  let respond = async (_method: string): Promise<any> => response(empty);
  globalThis.fetch = (async (_url: string, options: RequestInit) => {calls.push({method:String(options.method),signal:options.signal as AbortSignal});return respond(String(options.method));}) as typeof fetch;
  const controller=api.mountAppSummary(host,{app:APP,work:WORK,title:'Example app'});
  return {api,host,frame,button,calls,el,controller,focused,placements,changed:()=>listener(),respond:(fn:typeof respond)=>{respond=fn;},
    ensure:(fn:typeof ensureGroup)=>{ensureGroup=fn;}, open:()=>controller.show(), hide:()=>controller.hide(),
    cleanup:()=>{controller.dispose();globalThis.fetch=oldFetch;delete (globalThis as any)[key];}};
}

test('Summary content leaves shared companion controls alone and generates only from an explicit action', async () => {
  const f=await fixture();
  try {
    f.changed();await flush();assert.equal(f.calls.length,0,'mounting performs no read or generation');
    f.open();await flush();assert.deepEqual(f.calls.map(c=>c.method),['GET']);
    const panel=f.host.children[0];
    assert.equal((panel as Element).attrs.get('aria-label'),'Summary of Example app');
    assert.equal(f.host.querySelector('header'),null);assert.equal(f.host.querySelector('.cv-app-summary-close'),null);
    assert.equal(f.host.hidden,true,'the coordinator owns host visibility');
    assert.equal(f.button.attrs.has('aria-expanded'),false);assert.deepEqual(f.host.events,[]);
    const action=f.host.querySelector('.cv-ex-regen')!;assert.equal(action.textContent,'Generate summary');
    f.respond(async()=>response(ready));action.click();await flush();
    assert.deepEqual(f.calls.map(c=>c.method),['GET','POST']);assert.match(f.host.textContent,/A saved finding/);
    f.hide();assert.doesNotMatch(f.host.textContent,/A saved finding/);
    assert.equal(f.host.hidden,true);assert.equal(f.button.focused,false);assert.deepEqual(f.host.events,[]);
    f.open();await flush();assert.equal(f.host.children[0],panel);assert.deepEqual(f.calls.map(c=>c.method),['GET','POST','GET']);
    f.open();await flush();assert.equal(f.calls.length,3,'showing an already active tab does not repeat a read');
  } finally {f.cleanup();}
});

test('closing or removing an app prevents late Summary responses from reviving it', async () => {
  const f=await fixture();
  try {
    let resolve!: (value: unknown)=>void;
    f.respond(()=>new Promise(done=>{resolve=done;}));f.open();await flush();
    f.hide();assert.equal(f.calls[0]!.signal.aborted,true);
    resolve(response(ready));await flush();assert.equal(f.host.hidden,true);assert.doesNotMatch(f.host.textContent,/A saved finding/);
    f.open();await flush();f.host.attached=false;f.changed();
    assert.equal(f.calls[1]!.signal.aborted,true);resolve(response(ready));await flush();assert.equal(f.host.children.length,0);
  } finally {f.cleanup();}
});

test('failed authorization clears findings and disables further generation', async () => {
  const f=await fixture();
  try {
    f.respond(async()=>response(ready));f.open();await flush();assert.match(f.host.textContent,/A saved finding/);
    f.respond(async()=>response({code:'not_found'},404));f.host.querySelector('.cv-ex-regen')!.click();await flush();
    assert.doesNotMatch(f.host.textContent,/A saved finding/);assert.match(f.host.textContent,/no longer available/);
    assert.equal(f.host.querySelector('.cv-ex-regen')!.hidden,true);
  } finally {f.cleanup();}
});

test('non-derived apps show availability without a summary or model request', async () => {
  const f=await fixture({lineage:false});
  try {f.open();await flush();assert.equal(f.calls.length,0);assert.match(f.host.textContent,/apps created from files/);assert.equal(f.host.querySelector('.cv-ex-regen')!.hidden,true);} finally {f.cleanup();}
});

test('retrying a failed summary read remains a GET', async () => {
  const f=await fixture();
  try {
    f.respond(async()=>response({code:'unavailable'},503));f.open();await flush();
    const action=f.host.querySelector('.cv-ex-regen')!;assert.equal(action.textContent,'Try again');
    f.respond(async()=>response(empty));action.click();await flush();assert.deepEqual(f.calls.map(c=>c.method),['GET','GET']);
  } finally {f.cleanup();}
});


test('disposing or replacing a mounted Summary cancels requests without clearing the replacement host', async () => {
  const f=await fixture();
  let replacement: {show(): void; hide(): void; dispose(): void}|undefined;
  try {
    let resolve!: (value: unknown)=>void;
    f.respond(()=>new Promise(done=>{resolve=done;}));f.open();await flush();
    replacement=f.api.mountAppSummary(f.host,{app:APP,work:WORK,title:'Replacement app'});
    assert.equal(f.calls[0]!.signal.aborted,true);
    const panel=f.host.children[0];
    f.controller.dispose();f.open();
    assert.equal(f.host.children[0],panel);assert.equal(f.calls.length,1);
    resolve(response(ready));await flush();assert.doesNotMatch(f.host.textContent,/A saved finding/);
    f.respond(async()=>response(empty));replacement!.show();await flush();
    assert.equal(f.calls.length,2);assert.equal((panel as Element).attrs.get('aria-label'),'Summary of Replacement app');
  } finally {replacement?.dispose();f.cleanup();}
});

test('a pending receipt click stays cancelled after its Summary tab is hidden and reopened', async () => {
  const f=await fixture();
  try {
    f.respond(async()=>response({state:'ready',findings:[{parts:[{value:'42',view_id:'total'}]}]}));
    f.open();await flush();
    let resolve!: (value: unknown)=>void;
    f.ensure(()=>new Promise(done=>{resolve=done;}));
    f.host.querySelector('.cv-ex-value')!.click();f.hide();f.open();await flush();
    const card=f.el({},'article');(card as any).dataset={card:`c-${WORK}-total`};
    resolve({cards:[card]});await flush();
    assert.deepEqual(f.focused,[]);assert.deepEqual(f.placements,[]);
  } finally {f.cleanup();}
});
