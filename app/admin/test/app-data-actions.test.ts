import assert from 'node:assert/strict';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {build} from 'esbuild';

const APP = 'aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee', WORK = '11111111-2222-4333-8444-555555555555';
const COLLECTION = '22222222-2222-4333-8444-555555555555', RECORD = '33333333-2222-4333-8444-555555555555';
const NEW = '44444444-2222-4333-8444-555555555555', PUBLICATION = '55555555-2222-4333-8444-555555555555';
const KEY = '0123456789abcdef0123456789abcdef';
const catalog = () => ({app_id: APP, work_id: WORK, publication_id: PUBLICATION, work_version: 3, can_write: true, truncated: false,
  collections: [{collection_id: COLLECTION, schema_id: 'note', schema_version: 1, fields: [{id: 'title', type: 'text'}, {id: 'body', type: 'text'}],
    commands: [{id: 'add', operation: 'create', writable_fields: ['title', 'body']}, {id: 'edit', operation: 'update', writable_fields: ['title', 'body']}]}],
  records: [{record_id: RECORD, collection_id: COLLECTION, version: 2, values: [{field_id: 'title', display: 'Old title'}, {field_id: 'body', display: 'Old details'}]}]});
const proposed = (update = false) => {
  const values = [{field_id: 'title', value: {type: 'text', value: 'New title'}}];
  return {kind: 'app_action_proposal', app_id: APP, work_id: WORK, collection_id: COLLECTION, record_id: update ? RECORD : NEW,
    operation: update ? 'update' : 'create', command_id: update ? 'edit' : 'add',
    values: [{...values[0], label: 'Title', display: 'New title', ...(update ? {previous: 'Old title'} : {})}],
    command_request: {kind: 'command_request', schema_version: 1, app_id: APP, work_id: WORK, publication_id: PUBLICATION,
      command_id: update ? 'edit' : 'add', schema_id: 'note', schema_version_expected: 1, record_id: update ? RECORD : NEW,
      expected_record_version: update ? 2 : 0, expected_work_version: 3, values, idempotency_key: KEY}};
};
const response = (value: unknown, status = 200) => ({ok: status >= 200 && status < 300, status, json: async () => value});
const flush = async () => {for (let i = 0; i < 24; i++) await Promise.resolve();};
class Element {
  attrs = new Map<string, string>(); children: (Element | string)[] = []; parent: Element | null = null;
  hidden = false; disabled = false; value = ''; focused = false; attached = true; ownerDocument: any;
  listeners = new Map<string, ((event: any) => void)[]>();
  constructor(readonly tag: string, doc: unknown) {this.ownerDocument = doc;}
  get isConnected(): boolean {return this.attached && (!this.parent || this.parent.isConnected);}
  get textContent(): string {return this.children.map(value => typeof value === 'string' ? value : value.textContent).join('');}
  set textContent(value: string) {this.replaceChildren(value);}
  setAttribute(name: string, value: string) {this.attrs.set(name, value); if (name === 'value') this.value = value;}
  append(...values: (Element | string)[]) {for (const value of values) {if (value instanceof Element) {value.remove(); value.parent = this;} this.children.push(value);}}
  remove() {if (this.parent) this.parent.children = this.parent.children.filter(value => value !== this); this.parent = null;}
  replaceChildren(...values: (Element | string)[]) {for (const value of this.children) if (value instanceof Element) value.parent = null; this.children = []; this.append(...values);}
  addEventListener(name: string, handler: (event: any) => void) {this.listeners.set(name, [...(this.listeners.get(name) ?? []), handler]);}
  dispatch(name: string) {for (const handler of this.listeners.get(name) ?? []) handler({preventDefault() {}, stopPropagation() {}});}
  click() {this.dispatch('click');}
  focus() {this.focused = true;}
  matches(selector: string) {return selector.startsWith('.') ? (this.attrs.get('class') ?? '').split(' ').includes(selector.slice(1)) : this.tag === selector;}
  querySelectorAll(selector: string): Element[] {return this.children.flatMap(value => value instanceof Element ? [...(value.matches(selector) ? [value] : []), ...value.querySelectorAll(selector)] : []);}
  querySelector(selector: string) {return this.querySelectorAll(selector)[0] ?? null;}
}
let serial = 0;
async function fixture() {
  const entry = fileURLToPath(new URL('../src/canvas/app-data-actions.ts', import.meta.url)), doc = {};
  const el = (_doc: unknown, tag: string, attrs: Record<string, string> = {}, ...children: any[]) => {
    const node = new Element(tag, doc); for (const [key, value] of Object.entries(attrs)) node.setAttribute(key, value); node.append(...children); return node;
  };
  const commits: any[][] = [], changed: string[] = [], calls: {url: string; init: RequestInit; body?: any}[] = [];
  let enabled = 0;
  let commitResult = async (..._args: any[]): Promise<any> => ({kind: 'committed', recordVersion: '1', workVersion: '4', commit: '4', audit: NEW});
  const key = `__dataActions${serial++}`;
  (globalThis as any)[key] = {el, postCommand: (...args: any[]) => {commits.push(args); return commitResult(...args);}};
  const bundle = await build({entryPoints: [entry], bundle: true, format: 'esm', platform: 'node', write: false,
    plugins: [{name: 'data-action-boundaries', setup(builder) {
      builder.onResolve({filter: /^\.\/(render|commands)$/}, args => args.importer === entry || args.importer.endsWith('/app-data-copy.ts') ? {path: args.path, namespace: 'data-action-boundary'} : undefined);
      builder.onLoad({filter: /.*/, namespace: 'data-action-boundary'}, args => ({loader: 'js', contents: `export const ${args.path === './render' ? 'el' : 'postCommand'}=(...args)=>globalThis[${JSON.stringify(key)}].${args.path === './render' ? 'el' : 'postCommand'}(...args);`}));
    }}]});
  const api = await import('data:text/javascript;base64,' + Buffer.from(bundle.outputFiles[0]!.text).toString('base64'));
  const oldFetch = globalThis.fetch;
  let respond = async (url: string, init: RequestInit): Promise<any> => url.endsWith('/records') ? response(catalog()) : response(proposed(JSON.parse(String(init.body)).command_id === 'edit'));
  globalThis.fetch = (async (url: string, init: RequestInit) => {calls.push({url, init, body: init.body ? JSON.parse(String(init.body)) : undefined}); return respond(url, init);}) as typeof fetch;
  const host = el(doc, 'div'), controller = api.mountAppDataActions(host, {app: APP, work: WORK, title: 'Notes', onCommitted: (record: string) => changed.push(record), onDataEnabled: async () => {enabled++; return true;}});
  const find = (selector: string) => host.querySelector(selector)!;
  const choose = (command = 'add') => {find('.cv-app-action-select').value = `${COLLECTION}/${command}`; find('.cv-app-action-select').dispatch('change');};
  const prepare = async (words = 'Add a note titled New title') => {find('textarea').value = words; find('form').dispatch('submit'); await flush();};
  return {controller, host, calls, commits, changed, find, choose, prepare, enabled: () => enabled,
    respond: (fn: typeof respond) => {respond = fn;}, commitResult: (fn: typeof commitResult) => {commitResult = fn;},
    cleanup: () => {controller.dispose(); globalThis.fetch = oldFetch; delete (globalThis as any)[key];}};
}

test('preparing is read-only; explicit confirm sends the reviewed exact-work command and key once', async () => {
  const f = await fixture();
  try {
    assert.equal(f.calls.length, 0); f.controller.show(); await flush(); f.choose(); await f.prepare();
    assert.equal(f.commits.length, 0); assert.deepEqual(f.calls[1]!.body, {command_id: 'add', collection_id: COLLECTION, utterance: 'Add a note titled New title', publication_id: PUBLICATION, expected_work_version: 3});
    assert.match(f.find('.cv-app-action-review').textContent, /New title/); assert.match(f.host.textContent, /Nothing is saved until you confirm/);
    assert.equal(f.find('form').hidden, true); assert.equal(f.find('.cv-app-action-review').hidden, false);
    assert.equal(f.find('.cv-app-action-review').attrs.get('tabindex'), '-1'); assert.equal(f.find('.cv-app-action-review').focused, true);
    const confirm = f.find('.cv-app-action-confirm'); confirm.click(); confirm.click(); await flush();
    assert.equal(f.commits.length, 1); assert.equal(f.commits[0][0], `/v1/work/${APP}/${WORK}/commands/add`);
    assert.deepEqual(f.commits[0][1], proposed().command_request); assert.equal(f.commits[0][2], KEY); assert.deepEqual(f.changed, [NEW]);
    assert.match(f.host.textContent, /Record added/); assert.equal(f.calls.some(call => /\/v1\/act$|compose|shape/.test(call.url)), false);
  } finally {f.cleanup();}
});

test('a source-only app enables a reviewed editable copy then refreshes its native action catalog', async () => {
  const f = await fixture();
  try {
    let editable = false;
    f.respond(async (url, init) => {
      if (url.endsWith('/records')) return response(editable ? catalog() : {...catalog(), can_write: false, collections: [], records: []});
      assert.equal(url, `/v1/work/${APP}/${WORK}/data-copy`);
      if (init.method === 'POST') editable = true;
      return response({app_id: APP, work_id: WORK, publication_id: PUBLICATION, work_version: 3, editable, can_enable: !editable,
        total_rows: 2500, files: [{name: 'Accounts.csv', rows: 2500}], limits: {rows: 100000, bytes: 67108864}});
    });
    f.controller.show(); await flush(); assert.equal(f.find('form').hidden, true);
    f.find('.cv-app-data-copy-enable').click(); assert.equal(editable, false); f.find('.cv-app-data-copy-confirm').click(); await flush();
    assert.equal(editable, true); assert.equal(f.enabled(), 1); assert.equal(f.find('form').hidden, false);
    assert.equal(f.find('.cv-app-data-copy-host').hidden, true); assert.match(f.find('.cv-app-action-select').textContent, /Add/);
    assert.equal(f.calls.filter(call => call.url.endsWith('/records')).length, 2); assert.equal(f.commits.length, 0);
  } finally {f.cleanup();}
});

test('updates require an explicit record and review old/new values with pinned versions', async () => {
  const f = await fixture();
  try {
    f.controller.show(); await flush(); f.choose('edit'); await f.prepare('Change title to New title'); assert.equal(f.calls.length, 1);
    f.find('.cv-app-action-record').value = RECORD; f.find('.cv-app-action-record').dispatch('change'); await f.prepare('Change title to New title');
    assert.equal(f.calls[1]!.body.record_id, RECORD); assert.equal(f.calls[1]!.body.expected_record_version, 2);
    assert.match(f.find('.cv-app-action-review').textContent, /Old title → New title/);
    assert.doesNotMatch(f.find('.cv-app-action-review').textContent, new RegExp(RECORD));
    f.find('.cv-app-action-cancel').click(); assert.equal(f.commits.length, 0); assert.match(f.host.textContent, /Draft cancelled/);
    assert.equal(f.find('textarea').value, 'Change title to New title');
    assert.equal(f.find('form').hidden, false); assert.equal(f.find('.cv-app-action-review').hidden, true); assert.equal(f.find('textarea').focused, true);
  } finally {f.cleanup();}
});

test('the initial update picker uses scale-aware decimal and currency displays without depending on search', async () => {
  const f = await fixture();
  try {
    const listed = catalog();
    listed.records[0]!.values = [
      {field_id: 'title', display: '125', assistant_display: '1.25'},
      {field_id: 'body', display: '$123.45', assistant_display: 'KWD 12.345'},
    ] as typeof listed.records[0]['values'];
    f.respond(async () => response(listed));
    f.controller.show(); await flush(); f.choose('edit');
    assert.match(f.find('.cv-app-action-record').textContent, /1\.25 · KWD 12\.345/);
    assert.doesNotMatch(f.find('.cv-app-action-record').textContent, /\$123\.45/);
    assert.equal(f.calls.some(call => call.url.includes('/search')), false);
  } finally {f.cleanup();}
});

test('an app data limit refuses the proposal without confirmation and keeps the editable draft', async () => {
  const f = await fixture();
  try {
    f.respond(async url => url.endsWith('/records') ? response(catalog()) : response({code: 'app_data_limit'}, 422));
    f.controller.show(); await flush(); f.choose(); await f.prepare('Add a note titled New title');
    assert.match(f.host.textContent, /100,000 rows or 64 MiB/); assert.match(f.host.textContent, /Nothing was saved. Original files are unchanged/);
    assert.equal(f.host.querySelector('.cv-app-action-confirm'), null); assert.equal(f.commits.length, 0);
    assert.equal(f.find('textarea').value, 'Add a note titled New title'); assert.equal(f.find('form').hidden, false);
  } finally {f.cleanup();}
});

test('record search reaches older rows but requires explicit selection and preserves the target through review reopen', async () => {
  const f = await fixture();
  try {
    const old = {...catalog().records[0]!, record_id: NEW, version: 7};
    const update = proposed(true); update.record_id = NEW; update.command_request.record_id = NEW; update.command_request.expected_record_version = 7;
    f.respond(async url => url.includes('/records/search?') ? response({...catalog(), records: [old]})
      : url.endsWith('/records') ? response({...catalog(), truncated: true}) : response(update));
    f.controller.show(); await flush(); f.choose('edit');
    assert.match(f.host.textContent, /Find records to select older data/);
    f.find('.cv-app-action-search-input').value = 'Old title'; f.find('.cv-app-action-search-button').click(); await flush();
    const search = f.calls.find(call => call.url.includes('/records/search?'))!;
    const parameters = new URL(search.url, 'http://localhost').searchParams;
    assert.equal(parameters.get('collection'), COLLECTION); assert.equal(parameters.get('q'), 'Old title');
    assert.equal(f.find('.cv-app-action-record').value, ''); await f.prepare('Change title to New title');
    assert.equal(f.calls.filter(call => call.init.method === 'POST').length, 0, 'a single match is never selected automatically');
    f.find('.cv-app-action-record').value = NEW; f.find('.cv-app-action-record').dispatch('change'); await f.prepare('Change title to New title');
    const draft = f.calls.find(call => call.url.endsWith('/proposals'))!;
    assert.equal(draft.body.record_id, NEW); assert.equal(draft.body.expected_record_version, 7);
    f.controller.hide(); f.controller.show(); await flush();
    assert.equal(f.find('.cv-app-action-review').hidden, false);
    assert.equal(f.calls.filter(call => call.url.includes('/records/search?')).length, 2, 'reopen reauthorizes the older selected record');
    f.find('.cv-app-action-confirm').click(); await flush(); assert.equal(f.commits[0][1].record_id, NEW); assert.equal(f.commits[0][1].expected_record_version, 7);
  } finally {f.cleanup();}
});

test('record search rejects changed premises and ignores late results after the selected action changes', async () => {
  const f = await fixture();
  try {
    let resolve!: (value: unknown) => void;
    f.respond(async url => url.includes('/records/search?') ? new Promise(done => {resolve = done;}) : response(catalog()));
    f.controller.show(); await flush(); f.choose('edit'); f.find('.cv-app-action-search-input').value = 'Old title';
    f.find('.cv-app-action-search-button').click(); const request = f.calls.at(-1)!; f.choose('add');
    assert.equal(request.init.signal!.aborted, true);
    resolve(response({...catalog(), records: [{...catalog().records[0]!, record_id: NEW}]})); await flush();
    assert.equal(f.find('.cv-app-action-record').querySelectorAll('option').some(option => option.value === NEW), false);
    f.respond(async url => url.includes('/records/search?') ? response({...catalog(), work_version: 4}) : response(catalog()));
    f.choose('edit'); f.find('.cv-app-action-search-button').click(); await flush();
    assert.equal(f.find('form').hidden, true); assert.match(f.host.textContent, /app changed/); assert.equal(f.commits.length, 0);
  } finally {f.cleanup();}
});

test('legacy create commands with transitions require an existing target and review an update', async () => {
  const f = await fixture();
  try {
    const listing = catalog();
    listing.collections[0]!.commands = [{id: 'decide', operation: 'create', writable_fields: ['title'], transitions: [{field_id: 'title', from: 'Old title', to: 'New title'}]} as any];
    const proposal = proposed(true); proposal.command_id = 'decide'; proposal.command_request.command_id = 'decide';
    f.respond(async url => url.endsWith('/records') ? response(listing) : response(proposal));
    f.controller.show(); await flush();
    assert.match(f.find('.cv-app-action-select').textContent, /Decide · Note \(update\)/);
    await f.prepare('Choose New title'); assert.equal(f.calls.length, 1, 'no proposal without an explicit existing record');
    f.find('.cv-app-action-record').value = RECORD; f.find('.cv-app-action-record').dispatch('change'); await f.prepare('Choose New title');
    assert.equal(f.calls[1]!.body.record_id, RECORD); assert.equal(f.calls[1]!.body.expected_record_version, 2);
    assert.match(f.find('.cv-app-action-review').textContent, /Update this record/);
    f.find('.cv-app-action-confirm').click(); await flush();
    assert.equal(f.commits[0][0], `/v1/work/${APP}/${WORK}/commands/decide`);
    assert.equal(f.commits[0][1].record_id, RECORD); assert.equal(f.commits[0][1].expected_record_version, 2);
  } finally {f.cleanup();}
});

test('editing the request invalidates a proposal and aborts a late preparing response', async () => {
  const f = await fixture();
  try {
    f.controller.show(); await flush(); f.choose(); await f.prepare(); const oldConfirm = f.find('.cv-app-action-confirm');
    f.find('textarea').value = 'A different title'; f.find('textarea').dispatch('input'); oldConfirm.click(); assert.equal(f.commits.length, 0);
    let resolve!: (value: unknown) => void;
    f.respond(async () => new Promise(done => {resolve = done;})); await f.prepare();
    f.find('textarea').value = 'Edited while waiting'; f.find('textarea').dispatch('input'); assert.equal(f.calls.at(-1)!.init.signal!.aborted, true);
    resolve(response(proposed())); await flush(); assert.equal(f.host.querySelector('.cv-app-action-confirm'), null);
  } finally {f.cleanup();}
});

test('cross-work proposals, substituted fields and mismatched typed values never reach confirmation', async () => {
  const f = await fixture();
  try {
    f.controller.show(); await flush(); f.choose();
    for (const mutate of [
      (value: any) => {value.command_request.work_id = NEW;},
      (value: any) => {value.command_request.values[0].value.value = 'Different undisplayed text';},
      (value: any) => {value.values[0].field_id = 'admin'; value.command_request.values[0].field_id = 'admin';},
      (value: any) => {value.command_request.expected_work_version = 99;},
    ]) {
      const value = JSON.parse(JSON.stringify(proposed())); mutate(value); f.respond(async () => response(value)); await f.prepare();
      assert.equal(f.host.querySelector('.cv-app-action-confirm'), null); assert.equal(f.commits.length, 0);
    }
  } finally {f.cleanup();}
});

test('unknown save outcomes lock edits and retry exactly the same reviewed submission', async () => {
  const f = await fixture();
  try {
    f.controller.show(); await flush(); f.choose(); await f.prepare();
    f.commitResult(async () => ({kind: 'unconfirmed', message: 'The save could not be confirmed.'})); f.find('.cv-app-action-confirm').click(); await flush();
    assert.equal(f.find('textarea').disabled, true); assert.equal(f.find('.cv-app-action-select').disabled, true);
    assert.match(f.find('.cv-app-action-confirm').textContent, /Retry this save/);
    f.find('textarea').dispatch('input'); assert.ok(f.host.querySelector('.cv-app-action-confirm'));
    f.commitResult(async () => ({kind: 'committed', recordVersion: '1', workVersion: '4', commit: '4', audit: NEW})); f.find('.cv-app-action-confirm').click(); await flush();
    assert.deepEqual(f.commits[1], f.commits[0]); assert.deepEqual(f.changed, [NEW]);
  } finally {f.cleanup();}
});

test('closing a confirmed save does not cancel it or misreport rollback; draft authorization hides cached details', async () => {
  const f = await fixture();
  try {
    f.controller.show(); await flush(); f.choose(); await f.prepare();
    let finish!: (value: unknown) => void; f.commitResult(() => new Promise(done => {finish = done;}));
    f.find('.cv-app-action-confirm').click(); f.controller.hide(); finish({kind: 'committed', recordVersion: '1', workVersion: '4', commit: '4', audit: NEW}); await flush();
    assert.deepEqual(f.changed, [NEW]); assert.equal(f.commits.length, 1);
    f.controller.show(); await flush(); f.choose(); await f.prepare(); f.controller.hide();
    let authorize!: (value: unknown) => void; f.respond(() => new Promise(done => {authorize = done;})); f.controller.show();
    assert.equal(f.find('form').hidden, true); assert.equal(f.find('.cv-app-action-review').hidden, true);
    authorize(response({}, 404)); await flush(); assert.equal(f.find('textarea').value, ''); assert.equal(f.find('.cv-app-action-review').children.length, 0);
    assert.match(f.host.textContent, /no longer available/);
  } finally {f.cleanup();}
});

test('stale or forbidden actions require fresh review and unsupported apps never offer a write', async () => {
  const f = await fixture();
  try {
    f.controller.show(); await flush(); f.choose(); await f.prepare();
    f.commitResult(async () => ({kind: 'conflict', message: 'The record changed. Review a new draft.'})); f.find('.cv-app-action-confirm').click(); await flush();
    assert.equal(f.host.querySelector('.cv-app-action-confirm'), null); assert.equal(f.find('textarea').value, 'Add a note titled New title');
    const empty = catalog(); empty.collections[0]!.commands = []; f.respond(async () => response(empty)); f.find('.cv-app-action-refresh').click(); await flush();
    assert.match(f.host.textContent, /no supported actions/); assert.equal(f.find('form').hidden, true);
    const readonly = catalog(); readonly.can_write = false; f.respond(async () => response(readonly)); f.find('.cv-app-action-refresh').click(); await flush();
    assert.match(f.host.textContent, /do not have permission/); assert.equal(f.find('form').hidden, true);
  } finally {f.cleanup();}
});

test('a failed authorization refresh hides cached review and blocks confirmation while privately retaining the draft', async () => {
  const f = await fixture();
  try {
    f.controller.show(); await flush(); f.choose(); await f.prepare(); const oldConfirm = f.find('.cv-app-action-confirm');
    f.controller.hide(); f.respond(async () => {throw new Error('offline');}); f.controller.show(); await flush();
    assert.equal(f.find('form').hidden, true); assert.equal(f.find('.cv-app-action-review').hidden, true); oldConfirm.click(); assert.equal(f.commits.length, 0);
    assert.equal(f.find('textarea').value, 'Add a note titled New title'); assert.equal(f.find('.cv-app-action-refresh').hidden, false);
    f.respond(async () => response(catalog())); f.find('.cv-app-action-refresh').click(); await flush();
    assert.equal(f.find('form').hidden, true); assert.equal(f.find('.cv-app-action-review').hidden, false); assert.equal(f.find('.cv-app-action-confirm').disabled, false);
  } finally {f.cleanup();}
});
