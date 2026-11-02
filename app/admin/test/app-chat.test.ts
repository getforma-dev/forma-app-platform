import assert from 'node:assert/strict';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {build} from 'esbuild';

const APP = 'aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee', WORK = '11111111-2222-4333-8444-555555555555';
const BOX = '22222222-2222-4333-8444-555555555555', FILE = '33333333-2222-4333-8444-555555555555';
const OTHER = '44444444-2222-4333-8444-555555555555', REVISION = '55555555-2222-4333-8444-555555555555';
const DIGEST = `sha256:${'a'.repeat(64)}`;
let serial = 0;
const response = (data: unknown, status = 200) => ({ok: status >= 200 && status < 300, status, json: async () => data});
const flush = async () => {for (let i = 0; i < 24; i++) await Promise.resolve();};
const source = (id = FILE) => ({file_id: id, name: id === FILE ? 'Checking.csv' : 'Other.csv', revision_id: REVISION});
const lineage = (files = [source()]) => ({box_id: BOX, files});
const ref = (id = FILE, row = 1) => ({ref: `f1r${row}`, file_id: id, file: 'Checking.csv', sha256: DIGEST, sheet: null, row, label: 'Saved row'});

class Element {
  attrs = new Map<string, string>(); children: (Element | string)[] = []; parent: Element | null = null;
  hidden = false; disabled = false; value = ''; focused = false; attached = true; scrollTop = 0; scrollHeight = 500;
  listeners = new Map<string, ((event: any) => void)[]>(); ownerDocument: any;
  constructor(readonly tag: string, doc: unknown) {this.ownerDocument = doc;}
  get isConnected(): boolean {return this.attached && (!this.parent || this.parent.isConnected);}
  get textContent(): string {return this.children.map(value => typeof value === 'string' ? value : value.textContent).join('');}
  set textContent(text: string) {this.replaceChildren(text);}
  setAttribute(name: string, value: string) {this.attrs.set(name, value); if (name === 'value') this.value = value;}
  append(...children: (Element | string)[]) {for (const child of children) {if (child instanceof Element) {child.remove(); child.parent = this;} this.children.push(child);}}
  remove() {if (this.parent) this.parent.children = this.parent.children.filter(value => value !== this); this.parent = null;}
  replaceChildren(...children: (Element | string)[]) {for (const child of this.children) if (child instanceof Element) child.parent = null; this.children = []; this.append(...children);}
  addEventListener(name: string, listener: (event: any) => void) {this.listeners.set(name, [...(this.listeners.get(name) ?? []), listener]);}
  dispatch(name: string) {for (const listener of this.listeners.get(name) ?? []) listener({preventDefault() {}, stopPropagation() {}});}
  click() {this.dispatch('click');}
  focus() {this.focused = true;}
  matches(selector: string) {return selector.startsWith('.') ? (this.attrs.get('class') ?? '').split(' ').includes(selector.slice(1)) : this.tag === selector;}
  querySelectorAll(selector: string): Element[] {return this.children.flatMap(child => child instanceof Element ? [...(child.matches(selector) ? [child] : []), ...child.querySelectorAll(selector)] : []);}
  querySelector(selector: string): Element | null {return this.querySelectorAll(selector)[0] ?? null;}
}

async function fixture() {
  const entry = fileURLToPath(new URL('../src/canvas/app-chat.ts', import.meta.url));
  const document_ = {};
  const el = (_doc: unknown, tag: string, attrs: Record<string, string> = {}, ...children: any[]) => {
    const value = new Element(tag, document_); for (const [key, part] of Object.entries(attrs)) value.setAttribute(key, part); value.append(...children); return value;
  };
  const key = `__appChat${serial++}`; (globalThis as any)[key] = {el};
  const built = await build({entryPoints: [entry], bundle: true, platform: 'node', format: 'esm', write: false,
    plugins: [{name: 'app-chat-dom', setup(builder) {
      builder.onResolve({filter: /^\.\/render$/}, args => args.importer === entry ? {path: args.path, namespace: 'chat-dom'} : undefined);
      builder.onLoad({filter: /.*/, namespace: 'chat-dom'}, () => ({contents: `export const el = (...args) => globalThis[${JSON.stringify(key)}].el(...args);`, loader: 'js'}));
    }}],
  });
  const api = await import('data:text/javascript;base64,' + Buffer.from(built.outputFiles[0]!.text).toString('base64'));
  const host = el(document_, 'aside');
  const calls: {url: string; init: RequestInit; body?: any}[] = [], snapshots: any[] = [], opened: string[] = [];
  const oldFetch = globalThis.fetch;
  let handle = async (url: string, _init: RequestInit): Promise<any> => url.includes('/lineage/') ? response({}, 404) : url === '/v1/ask' ? response({text: 'A verified answer.', references: [], receipts: []}) : response({});
  globalThis.fetch = (async (url: string, init: RequestInit) => {calls.push({url, init, body: init.body ? JSON.parse(String(init.body)) : undefined}); return handle(url, init);}) as typeof fetch;
  const options = {app: APP, work: WORK, title: 'Checking', address: '/checking', onOpenSource: (fileId: string) => opened.push(fileId), renderAnswerActions: (snapshot: unknown) => {snapshots.push(snapshot); return el(document_, 'button', {class: 'test-save'}, 'Save answer');}};
  const chat = api.mountAppChat(host, options);
  const field = () => host.querySelector('textarea')!;
  const submit = async (question: string) => {field().value = question; host.querySelector('form')!.dispatch('submit'); await flush();};
  return {api, chat, host, calls, snapshots, opened, options, el, field, submit,
    handle: (fn: typeof handle) => {handle = fn;}, posts: () => calls.filter(value => value.init.method === 'POST'),
    cleanup: () => {chat.dispose(); globalThis.fetch = oldFetch; delete (globalThis as any)[key];}};
}

test('app Ask keeps transcript/draft through close and routes only its exact saved work', async () => {
  const f = await fixture();
  try {
    assert.equal(f.calls.length, 0); f.chat.show(); await flush();
    assert.equal(f.calls[0]!.url, `/v1/canvas/works/${APP}/${WORK}`);
    await f.submit('What is saved here?');
    assert.equal(f.posts().length, 1);
    assert.deepEqual(f.posts()[0]!.body, {question: 'What is saved here?', scope: {kind: 'app', address: '/checking', works: [{app_id: APP, work_id: WORK, address: '/checking'}]}});
    assert.match(f.host.textContent, /A verified answer/); assert.equal(f.snapshots.length, 1);
    const save = f.host.querySelector('.test-save');
    f.field().value = 'An unfinished question'; f.chat.hide(); f.chat.show(); await flush();
    assert.equal(f.field().value, 'An unfinished question'); assert.match(f.host.textContent, /A verified answer/);
    assert.equal(f.host.querySelector('.test-save'), save); assert.equal(f.posts().length, 1);
    assert.equal(f.calls.some(value => /compose|shape|\/v1\/act/.test(value.url)), false);
  } finally {f.cleanup();}
});

test('a sole source is explicit, fresh and file-scoped; save hook receives all validated references', async () => {
  const f = await fixture();
  try {
    f.handle(async url => url.includes('/lineage/') ? response(lineage()) : url.endsWith('/ask') ? response({text: 'These rows were read.', references: Array.from({length: 60}, (_, i) => ref(FILE, i + 1))}) : response({}));
    f.chat.ask('What rows were read?'); await flush();
    assert.deepEqual(f.posts().map(value => value.url), [`/v1/boxes/${BOX}/ask`]);
    assert.deepEqual(f.posts()[0]!.body, {question: 'What rows were read?', file_id: FILE});
    assert.equal(f.calls.filter(value => value.url.includes('/lineage/')).length, 2);
    assert.equal(f.snapshots[0].kind, 'file'); assert.equal(f.snapshots[0].reply.references.length, 60);
    assert.equal(f.snapshots[0].source.fileId, FILE);
    const list = f.host.querySelector('.cv-app-chat-citations-list')!;
    assert.equal(list.children.length, 50); assert.match(f.host.textContent, /1–50 of 60/);
    f.host.querySelector('.cv-app-chat-citations-next')!.click();
    assert.equal(list.children.length, 10); assert.match(list.textContent, /row 60/); assert.match(f.host.textContent, /51–60 of 60/);
    const lastPageSource = list.querySelector('.bx-lineage-open')!; lastPageSource.click(); assert.deepEqual(f.opened, [FILE]);
    assert.equal(f.host.querySelector('.cv-app-chat-citations-next')!.disabled, true);
    f.host.querySelector('.cv-app-chat-citations-previous')!.click(); assert.equal(list.children.length, 50);
    f.chat.hide(); lastPageSource.click(); assert.deepEqual(f.opened, [FILE], 'hidden chat cannot reopen a source from an old page');
    assert.equal(f.field().value, '');
  } finally {f.cleanup();}
});

test('saving a file answer preserves canonical long filenames and sheet names beyond display limits', async () => {
  const f = await fixture();
  try {
    const name = `${'Monthly transactions '.repeat(12)}.csv`, sheet = 'Detailed receipts '.repeat(10);
    const reference = {...ref(), file: name, sheet, label: 'Exact original row label'};
    f.handle(async url => url.includes('/lineage/') ? response(lineage([{...source(), name}]))
      : url.endsWith('/ask') ? response({text: 'One source row.', references: [reference]}) : response({}));
    f.chat.ask('Find a row'); await flush();
    assert.deepEqual(f.snapshots[0].reply.references, [reference]); assert.equal(f.snapshots[0].source.fileName, name);
  } finally {f.cleanup();}
});

test('multiple sources require selection and a foreign or malformed citation rejects the entire answer', async () => {
  const f = await fixture();
  try {
    f.handle(async url => url.includes('/lineage/') ? response(lineage([source(), source(OTHER)])) : url.endsWith('/ask') ? response({text: 'Must never display.', references: [ref(FILE)]}) : response({}));
    f.chat.ask('Show the total'); await flush(); assert.equal(f.posts().length, 0); assert.equal(f.field().value, 'Show the total');
    const selector = f.host.querySelector('select')!; assert.equal(selector.value, ''); selector.value = OTHER; selector.dispatch('change');
    await f.submit('Show the total');
    assert.equal(f.posts()[0]!.body.file_id, OTHER); assert.doesNotMatch(f.host.textContent, /Must never display/);
    assert.equal(f.snapshots.length, 0); assert.match(f.host.textContent, /verified answer could not be shown/);
  } finally {f.cleanup();}
});

test('closing aborts a pending answer; a late completion cannot display or create saved-answer actions', async () => {
  const f = await fixture();
  try {
    let resolve!: (value: unknown) => void;
    f.handle(async url => url.includes('/lineage/') ? response({}, 404) : url === '/v1/ask' ? new Promise(done => {resolve = done;}) : response({}));
    f.chat.show(); await flush(); await f.submit('Pending question');
    f.field().value = 'Next draft'; f.chat.hide(); assert.equal(f.posts()[0]!.init.signal!.aborted, true);
    resolve(response({text: 'Late content.', references: [], receipts: []})); await flush();
    assert.doesNotMatch(f.host.textContent, /Late content/); assert.equal(f.snapshots.length, 0);
    f.chat.show(); await flush(); assert.equal(f.field().value, 'Next draft'); assert.match(f.host.textContent, /Stopped/);
  } finally {f.cleanup();}
});

test('same-context addressed questions queue without cancelling the answer or draft; hiding clears the queue', async () => {
  const f = await fixture();
  try {
    let resolve!: (value: unknown) => void, defer = true;
    f.handle(async url => url.includes('/lineage/') ? response({}, 404) : url === '/v1/ask'
      ? defer ? new Promise(done => {resolve = done;}) : response({text: 'Ready.', references: [], receipts: []}) : response({}));
    f.chat.show(); await flush(); f.chat.ask('First question'); await flush();
    f.field().value = 'My unfinished draft'; f.chat.setRecordScope(null); f.chat.ask('Second question'); await flush();
    assert.equal(f.posts().length, 1); assert.equal(f.posts()[0]!.init.signal!.aborted, false); assert.equal(f.field().value, 'My unfinished draft');
    defer = false; resolve(response({text: 'First answer.', references: [], receipts: []})); await flush();
    assert.deepEqual(f.posts().map(value => value.body.question), ['First question', 'Second question']); assert.equal(f.snapshots.length, 2);
    defer = true; f.chat.ask('Third question'); await flush(); f.chat.ask('Fourth question'); f.chat.hide();
    resolve(response({text: 'Late third answer.', references: [], receipts: []})); await flush();
    assert.equal(f.posts().length, 3); f.chat.show(); await flush(); assert.equal(f.posts().length, 3);
    assert.equal(f.field().value, 'My unfinished draft'); assert.doesNotMatch(f.host.textContent, /Late third answer/);
  } finally {f.cleanup();}
});

test('revoked access clears old answers and drafts, and source detachment never falls back to folder or records Ask', async () => {
  const f = await fixture();
  try {
    let valid = true;
    f.handle(async url => url.includes('/lineage/') ? response(lineage()) : url.endsWith('/ask') ? response({text: 'Private answer.', references: [ref()]}) : response({}, valid ? 200 : 404));
    f.chat.show(); await flush(); await f.submit('Read it'); assert.match(f.host.textContent, /Private answer/);
    f.field().value = 'Private draft'; valid = false; f.chat.hide(); f.chat.show(); await flush();
    assert.doesNotMatch(f.host.textContent, /Private answer/); assert.equal(f.field().value, ''); assert.match(f.host.textContent, /no longer available/);
    valid = true; f.chat.hide(); f.chat.show(); await flush();
    f.handle(async url => url.includes('/lineage/') ? response({}, 404) : response({}));
    await f.submit('Detached source'); assert.equal(f.posts().length, 1); assert.match(f.host.textContent, /source changed or is no longer available/);
  } finally {f.cleanup();}
});

test('component questions preserve their collection, and another app or work cannot override the scope', async () => {
  const f = await fixture();
  try {
    const component = {kind: 'card', address: '/checking/transactions', works: [{app_id: APP, work_id: WORK, address: '/checking/transactions', collection: OTHER}]};
    f.chat.setRecordScope(component); f.chat.ask('Show these transactions'); await flush();
    assert.deepEqual(f.posts()[0]!.body.scope, component); assert.deepEqual(f.snapshots[0].scope, component);
    f.chat.setRecordScope({...component, works: [{...component.works[0], work_id: OTHER}]}); f.chat.ask('Wrong scope'); await flush();
    assert.equal(f.posts().length, 1); assert.match(f.host.textContent, /does not belong to this app/);
    f.chat.setRecordScope(null); await f.submit('Back to this app'); assert.equal(f.posts()[1]!.body.scope.kind, 'app');
  } finally {f.cleanup();}
});

test('a source-query component cannot silently widen to app records before or after loading', async () => {
  const f = await fixture();
  try {
    const scope = {kind: 'card', address: 'checking.summary', works: [{app_id: APP, work_id: WORK, address: 'checking'}]};
    f.chat.setRecordScope(scope); f.chat.ask('What is the total?'); await flush();
    assert.equal(f.posts().length, 0); assert.equal(f.field().value, 'What is the total?'); assert.match(f.host.textContent, /component has no saved records/);
    const picker = f.host.querySelector('select')!; picker.value = 'records'; picker.dispatch('change');
    await f.submit('Read this app’s records instead'); assert.equal(f.posts().length, 1); assert.equal(f.posts()[0]!.body.scope.kind, 'app');
  } finally {f.cleanup();}
});

test('record citations open their validated record route and keep original receipts for saving', async () => {
  const f = await fixture();
  try {
    const receipt = {reference: 'r1', kind: 'record', app_id: APP, work_id: WORK, record_id: FILE, collection_id: OTHER, publication_id: REVISION,
      definition_digest: DIGEST, version: 1, work_version: 2, commit_sequence: 3, committed_at: '2026-11-02T12:00:00Z'};
    f.handle(async url => url.includes('/lineage/') ? response({}, 404) : url === '/v1/ask' ? response({text: 'One saved record.',
      references: [{ref: 'r1', app_id: APP, work_id: WORK, record_id: FILE, address: 'checking', label: 'Transaction'}], receipts: [receipt]}) : response({}));
    f.chat.show(); f.chat.focus(); await flush(); await f.submit('Which record?');
    const link = f.host.querySelector('a')!; assert.equal(link.attrs.get('href'), `/work/${APP}/${WORK}/records/${OTHER}/${FILE}`);
    assert.equal(link.attrs.get('target'), '_blank'); assert.deepEqual(f.snapshots[0].reply.receipts, [receipt]); assert.equal(f.field().focused, true);
  } finally {f.cleanup();}
});

test('record source pages expose every validated reference without mounting more than 50 at once', async () => {
  const f = await fixture();
  try {
    const references = Array.from({length: 60}, (_, i) => ({ref: `r${i}`, app_id: APP, work_id: WORK,
      record_id: `66666666-2222-4333-8444-${i.toString(16).padStart(12, '0')}`, address: 'checking', label: `Transaction ${i + 1}`}));
    const receipts = references.map(reference => ({reference: reference.ref, kind: 'record', app_id: APP, work_id: WORK, record_id: reference.record_id,
      collection_id: OTHER, publication_id: REVISION, definition_digest: DIGEST, version: 1, work_version: 2, commit_sequence: 3, committed_at: '2026-11-02T12:00:00Z'}));
    f.handle(async url => url.includes('/lineage/') ? response({}, 404) : url === '/v1/ask' ? response({text: 'These records were read.', references, receipts}) : response({}));
    f.chat.ask('Which records?'); await flush();
    assert.equal(f.host.querySelectorAll('a').length, 50); f.host.querySelector('.cv-app-chat-citations-next')!.click();
    const links = f.host.querySelectorAll('a'); assert.equal(links.length, 10); assert.equal(links.at(-1)!.textContent, 'Transaction 60');
    assert.equal(links.at(-1)!.attrs.get('href'), `/work/${APP}/${WORK}/records/${OTHER}/${references[59]!.record_id}`);
    assert.equal(f.snapshots[0].reply.references.length, 60);
  } finally {f.cleanup();}
});

test('retry reuses the failed question and each mounted app bounds its own session transcript', async () => {
  const f = await fixture();
  try {
    let fail = true;
    f.handle(async url => url.includes('/lineage/') ? response({}, 404) : url === '/v1/ask' ? fail ? response({code: 'could_not_answer'}, 422) : response({text: 'Ready.', references: [], receipts: []}) : response({}));
    f.chat.show(); await flush(); await f.submit('First question'); fail = false;
    f.host.querySelector('.cv-app-chat-retry')!.click(); await flush();
    assert.deepEqual(f.posts().map(value => value.body.question), ['First question', 'First question']);
    assert.equal(f.host.querySelectorAll('.cv-app-chat-exchange').length, 1);
    for (let i = 0; i < 14; i++) await f.submit(`Question ${i}`);
    assert.equal(f.host.querySelectorAll('.cv-app-chat-exchange').length, 12); assert.doesNotMatch(f.host.textContent, /First question/);
    const second = f.el({}, 'aside'); const chat = f.api.mountAppChat(second, {...f.options, work: OTHER});
    chat.show(); await flush(); assert.doesNotMatch(second.textContent, /Question 13/); chat.dispose();
    f.chat.dispose(); assert.equal(f.host.children.length, 0);
  } finally {f.cleanup();}
});
