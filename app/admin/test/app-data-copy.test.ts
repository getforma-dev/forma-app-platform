import assert from 'node:assert/strict';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {build} from 'esbuild';

const APP = 'aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee', WORK = '11111111-2222-4333-8444-555555555555';
const PUBLICATION = '55555555-2222-4333-8444-555555555555';
const capability = (editable = false) => ({app_id: APP, work_id: WORK, publication_id: PUBLICATION, work_version: editable ? 4 : 3,
  editable, can_enable: !editable, total_rows: 2500, files: [{name: 'Accounts.csv', rows: 2500}], limits: {rows: 100000, bytes: 67108864},
  ...(editable ? {imported_rows: 2500} : {})});
const response = (value: unknown, status = 200) => ({ok: status >= 200 && status < 300, status, json: async () => value});
const flush = async () => {for (let i = 0; i < 24; i++) await Promise.resolve();};
class Element {
  attrs = new Map<string, string>(); children: (Element | string)[] = []; parent: Element | null = null;
  hidden = false; disabled = false; focused = false; ownerDocument: any;
  listeners = new Map<string, ((event: any) => void)[]>();
  constructor(readonly tag: string, doc: unknown) {this.ownerDocument = doc;}
  get isConnected(): boolean {return !this.parent || this.parent.isConnected;}
  get textContent(): string {return this.children.map(value => typeof value === 'string' ? value : value.textContent).join('');}
  set textContent(value: string) {this.replaceChildren(value);}
  setAttribute(name: string, value: string) {this.attrs.set(name, value);}
  append(...values: (Element | string)[]) {for (const value of values) {if (value instanceof Element) {value.remove(); value.parent = this;} this.children.push(value);}}
  remove() {if (this.parent) this.parent.children = this.parent.children.filter(value => value !== this); this.parent = null;}
  replaceChildren(...values: (Element | string)[]) {for (const value of this.children) if (value instanceof Element) value.parent = null; this.children = []; this.append(...values);}
  addEventListener(name: string, handler: (event: any) => void) {this.listeners.set(name, [...(this.listeners.get(name) ?? []), handler]);}
  click() {for (const handler of this.listeners.get('click') ?? []) handler({preventDefault() {}});}
  focus() {this.focused = true;}
  matches(selector: string) {return selector.startsWith('.') ? (this.attrs.get('class') ?? '').split(' ').includes(selector.slice(1)) : this.tag === selector;}
  querySelectorAll(selector: string): Element[] {return this.children.flatMap(value => value instanceof Element ? [...(value.matches(selector) ? [value] : []), ...value.querySelectorAll(selector)] : []);}
  querySelector(selector: string) {return this.querySelectorAll(selector)[0] ?? null;}
}
let serial = 0;
async function fixture() {
  const entry = fileURLToPath(new URL('../src/canvas/app-data-copy.ts', import.meta.url)), doc = {};
  const el = (_doc: unknown, tag: string, attrs: Record<string, string> = {}, ...children: any[]) => {
    const node = new Element(tag, doc); for (const [key, value] of Object.entries(attrs)) node.setAttribute(key, value); node.append(...children); return node;
  };
  const key = `__dataCopy${serial++}`; (globalThis as any)[key] = {el};
  const bundle = await build({entryPoints: [entry], bundle: true, format: 'esm', platform: 'node', write: false,
    plugins: [{name: 'data-copy-dom', setup(builder) {
      builder.onResolve({filter: /^\.\/render$/}, () => ({path: 'render', namespace: 'data-copy-dom'}));
      builder.onLoad({filter: /.*/, namespace: 'data-copy-dom'}, () => ({loader: 'js', contents: `export const el=(...args)=>globalThis[${JSON.stringify(key)}].el(...args);`}));
    }}]});
  const api = await import('data:text/javascript;base64,' + Buffer.from(bundle.outputFiles[0]!.text).toString('base64'));
  const oldFetch = globalThis.fetch, calls: {url: string; init: RequestInit; body?: any}[] = [];
  let enabled = 0, respond = async (_url: string, init: RequestInit): Promise<any> => response(capability(init.method === 'POST'));
  globalThis.fetch = (async (url: string, init: RequestInit) => {calls.push({url, init, body: init.body ? JSON.parse(String(init.body)) : undefined}); return respond(url, init);}) as typeof fetch;
  const host = el(doc, 'div'), controller = api.mountAppDataCopy(host, {app: APP, work: WORK, onEnabled: async () => {enabled++; return true;}});
  const find = (selector: string) => host.querySelector(selector)!;
  return {controller, host, calls, find, enabled: () => enabled, posts: () => calls.filter(call => call.init.method === 'POST'),
    respond: (fn: typeof respond) => {respond = fn;}, cleanup: () => {controller.dispose(); globalThis.fetch = oldFetch; delete (globalThis as any)[key];}};
}

test('enabling reviews the complete copy and only confirmation mutates the exact work', async () => {
  const f = await fixture();
  try {
    f.controller.show(); await flush(); assert.equal(f.posts().length, 0);
    f.find('.cv-app-data-copy-enable').click();
    assert.match(f.find('.cv-app-data-copy-review').textContent, /2,500 rows/);
    assert.match(f.host.textContent, /original files stay unchanged/);
    assert.equal(f.find('.cv-app-data-copy-review').focused, true); assert.equal(f.posts().length, 0);
    f.find('.cv-app-data-copy-cancel').click(); assert.equal(f.posts().length, 0); assert.match(f.host.textContent, /Cancelled/);
    f.find('.cv-app-data-copy-enable').click(); const confirm = f.find('.cv-app-data-copy-confirm'); confirm.click(); confirm.click(); await flush();
    assert.equal(f.posts().length, 1); assert.equal(f.posts()[0]!.url, `/v1/work/${APP}/${WORK}/data-copy`);
    assert.equal(f.posts()[0]!.body.publication_id, PUBLICATION); assert.equal(f.posts()[0]!.body.expected_work_version, 3);
    assert.match(f.posts()[0]!.body.idempotency_key, /^[a-f0-9]{32}$/); assert.equal(f.enabled(), 1);
  } finally {f.cleanup();}
});

test('an uncertain result preserves one immutable import request through close and retry', async () => {
  const f = await fixture();
  try {
    let fail = true;
    f.respond(async (_url, init) => {if (init.method === 'POST' && fail) throw new Error('lost response'); return response(capability(init.method === 'POST'));});
    f.controller.show(); await flush(); f.find('.cv-app-data-copy-enable').click(); f.find('.cv-app-data-copy-confirm').click(); await flush();
    assert.match(f.host.textContent, /same request/); const body = f.posts()[0]!.init.body;
    f.controller.hide(); f.controller.show(); await flush(); fail = false;
    assert.equal(f.find('.cv-app-data-copy-confirm').textContent, 'Retry enabling app data'); f.find('.cv-app-data-copy-confirm').click(); await flush();
    assert.equal(f.posts().length, 2); assert.equal(f.posts()[1]!.init.body, body); assert.equal(f.enabled(), 1);
  } finally {f.cleanup();}
});

test('reopening hides prepared source details until fresh authorization and clears revoked access', async () => {
  const f = await fixture();
  try {
    f.controller.show(); await flush(); f.find('.cv-app-data-copy-enable').click(); const stale = f.find('.cv-app-data-copy-confirm');
    f.controller.hide(); let resolve!: (value: unknown) => void;
    f.respond(async () => new Promise(done => {resolve = done;})); f.controller.show();
    assert.equal(f.find('.cv-app-data-copy-content').hidden, true); assert.doesNotMatch(f.host.textContent, /Accounts.csv/);
    stale.click(); assert.equal(f.posts().length, 0);
    resolve(response({}, 404)); await flush(); stale.click(); assert.equal(f.posts().length, 0); assert.doesNotMatch(f.host.textContent, /Accounts.csv/);
  } finally {f.cleanup();}
});

test('limits, foreign identity and changed publication never expose an actionable stale review', async () => {
  const f = await fixture();
  try {
    f.respond(async () => response({...capability(), can_enable: false, reason: 'data_copy_limit', total_rows: 100001, files: [{name: 'Large.csv', rows: 100001}]}));
    f.controller.show(); await flush(); assert.equal(f.host.querySelector('.cv-app-data-copy-enable'), null); assert.match(f.host.textContent, /100,000 rows or 64 MiB/);
    f.controller.hide(); f.respond(async () => response({...capability(), work_id: APP})); f.controller.show(); await flush();
    assert.equal(f.host.querySelector('.cv-app-data-copy-enable'), null); assert.match(f.host.textContent, /could not be checked/);
    f.controller.hide(); f.respond(async () => response(capability())); f.controller.show(); await flush(); f.find('.cv-app-data-copy-enable').click();
    f.controller.hide(); f.respond(async () => response({...capability(), work_version: 4})); f.controller.show(); await flush();
    assert.equal(f.host.querySelector('.cv-app-data-copy-confirm'), null); assert.match(f.host.textContent, /app changed/); assert.equal(f.posts().length, 0);
  } finally {f.cleanup();}
});

test('a confirmed import may finish while closed, without reopening or writing a second time', async () => {
  const f = await fixture();
  try {
    let resolve!: (value: unknown) => void;
    f.respond(async (_url, init) => init.method === 'POST' ? new Promise(done => {resolve = done;}) : response(capability()));
    f.controller.show(); await flush(); f.find('.cv-app-data-copy-enable').click(); f.find('.cv-app-data-copy-confirm').click();
    f.controller.hide(); resolve(response(capability(true))); await flush();
    assert.equal(f.find('.cv-app-data-copy').hidden, true); assert.equal(f.enabled(), 1); assert.equal(f.posts().length, 1);
  } finally {f.cleanup();}
});
