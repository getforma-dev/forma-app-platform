import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import { build } from 'esbuild';

const APP = 'aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee';
const WORK = '11111111-2222-4333-8444-555555555555';
const BOX = '00000000-0000-4000-8000-000000000001';
const FILE = '00000000-0000-4000-8000-000000000002';
const OTHER = '00000000-0000-4000-8000-000000000003';
const SOURCE = '00000000-0000-4000-8000-000000000004';
const REVISION = '00000000-0000-4000-8000-000000000005';
const url = (box: string, file: string) => `/v1/boxes/${box}/files/${file}/content`;
const file = (id = FILE, name = 'Original.csv', kind = 'csv') => ({ file_id: id, name, kind, source_id: SOURCE, revision_id: REVISION, content_url: 'https://untrusted.example/file' });
const lineage = (...files: unknown[]) => ({ box_id: BOX, box_name: 'Uploads', files });
const response = (value: unknown, status = 200) => ({ ok: status >= 200 && status < 300, status, json: async () => value });
const flush = async () => { for (let i = 0; i < 15; i++) await Promise.resolve(); };
let serial = 0;

class Element {
  attrs = new Map<string, string>();
  children: (Element | string)[] = [];
  parent: Element | null = null;
  attached = true; hidden = false; disabled = false; selected = false; value = '';
  listeners = new Map<string, ((event: any) => void)[]>();
  constructor(readonly tag: string, readonly ownerDocument: any) {}
  get isConnected(): boolean { return this.attached && (!this.parent || this.parent.isConnected); }
  get textContent(): string { return this.children.map(child => typeof child === 'string' ? child : child.textContent).join(''); }
  set textContent(value: string) { this.replaceChildren(value); }
  setAttribute(name: string, value: string) { this.attrs.set(name, value); }
  append(...children: (Element | string)[]) {
    for (const child of children) { if (child instanceof Element) { child.parent = this; child.attached = true; } this.children.push(child); }
  }
  replaceChildren(...children: (Element | string)[]) {
    for (const child of this.children) if (child instanceof Element) { child.parent = null; child.attached = false; }
    this.children = []; this.append(...children);
  }
  remove() { if (this.parent) this.parent.children = this.parent.children.filter(child => child !== this); this.parent = null; this.attached = false; }
  contains(node: Element | null): boolean { return node === this || this.children.some(child => child instanceof Element && child.contains(node)); }
  addEventListener(name: string, handler: (event: any) => void) { this.listeners.set(name, [...(this.listeners.get(name) ?? []), handler]); }
  click() { for (const handler of this.listeners.get('click') ?? []) handler({ preventDefault() {} }); }
  focus() { this.ownerDocument.activeElement = this; }
  matches(selector: string) {
    if (selector.startsWith('.')) return (this.attrs.get('class') ?? '').split(' ').includes(selector.slice(1));
    return this.tag === selector;
  }
  querySelectorAll(selector: string): Element[] {
    return this.children.flatMap(child => child instanceof Element ? [...(child.matches(selector) ? [child] : []), ...child.querySelectorAll(selector)] : []);
  }
  querySelector(selector: string): Element | null { return this.querySelectorAll(selector)[0] ?? null; }
}

async function moduleFixture(name: string, boundaries: (el: (...args: any[]) => Element) => Record<string, unknown>) {
  const entry = fileURLToPath(new URL(`../src/canvas/${name}.ts`, import.meta.url));
  const source = await readFile(entry, 'utf8');
  const exports = new Map<string, string[]>();
  for (const [, names, path] of source.matchAll(/import\s*\{([^}]+)\}\s*from\s*['"]([^'"]+)['"]/gs))
    exports.set(path!, names!.split(',').map(name => name.trim()).filter(name => name && !name.startsWith('type ')));
  const document_: any = { activeElement: null };
  const el = (_doc: unknown, tag: string, attrs: Record<string, string> = {}, ...children: any[]) => {
    const node = new Element(tag, document_);
    for (const [key, value] of Object.entries(attrs)) node.setAttribute(key, value);
    node.append(...children.filter(child => child !== undefined && child !== null && child !== false)); return node;
  };
  const mocks = { './render': { el }, ...boundaries(el) };
  const key = `__sourcesTest${serial++}`;
  (globalThis as any)[key] = mocks;
  const result = await build({ entryPoints: [entry], bundle: true, format: 'esm', platform: 'node', write: false,
    plugins: [{ name: 'sources-boundaries', setup(builder) {
      builder.onResolve({ filter: /.*/ }, args => args.importer === entry && exports.has(args.path) ? { path: args.path, namespace: 'sources-spy' } : undefined);
      builder.onLoad({ filter: /.*/, namespace: 'sources-spy' }, args => ({ contents: exports.get(args.path)!.map(name =>
        `export const ${name} = (...args) => globalThis[${JSON.stringify(key)}][${JSON.stringify(args.path)}]?.[${JSON.stringify(name)}]?.(...args);`).join('\n'), loader: 'js' }));
    } }],
  });
  const api = await import('data:text/javascript;base64,' + Buffer.from(result.outputFiles[0]!.text).toString('base64'));
  return { api, el, document_, host: el(document_, 'div'), cleanup: () => { delete (globalThis as any)[key]; } };
}

async function sourcesFixture() {
  const viewed: any[] = [], asked: any[] = [], larger: any[] = [];
  let disposedViews = 0;
  const f = await moduleFixture('app-sources', el => ({
    './box-api': { contentUrl: url },
    './box-viewer': {
      mountFileViewer: (...args: any[]) => { viewed.push(args); return { dispose() { disposedViews++; } }; },
      openViewer: (...args: any[]) => { larger.push(args); },
    },
    './file-card': { fileCardGlyph: () => el(null, 'svg') },
    './ids': { isUuid: (value: unknown) => typeof value === 'string' && /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(value) },
  }));
  const oldFetch = globalThis.fetch;
  let respond = async (): Promise<any> => response(lineage(file()));
  const calls: { url: string; options: RequestInit }[] = [];
  globalThis.fetch = (async (url: string, options: RequestInit) => { calls.push({ url, options }); return respond(); }) as typeof fetch;
  const controller = f.api.mountAppSources(f.host, { app: APP, work: WORK, title: 'Example app', onAskFile: (source: any) => asked.push(source) });
  return { ...f, controller, calls, viewed, asked, larger, disposedViews: () => disposedViews,
    respond: (fn: typeof respond) => { respond = fn; },
    cleanup: () => { controller.dispose(); globalThis.fetch = oldFetch; f.cleanup(); } };
}

test('Sources presents the exact original and hands file context to the app assistant', async () => {
  const f = await sourcesFixture();
  try {
    f.controller.show(); await flush();
    assert.equal(f.calls[0]!.url, `/v1/boxes/lineage/${APP}/${WORK}`);
    assert.equal(f.calls[0]!.options.cache, 'no-store');
    assert.match(f.host.textContent, /The original file behind this app/);
    assert.match(f.host.textContent, /Original.csvCSV · Original file/);
    assert.doesNotMatch(f.host.textContent, /0 B|Ask anything|Keep original/);
    f.host.querySelector('.cv-app-source-ask')!.click();
    assert.equal(f.asked[0].boxId, BOX); assert.equal(f.asked[0].file.file_id, FILE);
    assert.equal(f.asked[0].sourceId, SOURCE); assert.equal(f.asked[0].revisionId, REVISION);
    f.host.querySelector('.cv-app-source-open')!.click();
    assert.equal(f.viewed[0][1].content_url, url(BOX, FILE)); assert.equal(f.viewed[0][2], BOX);
    assert.match(f.host.textContent, /Open larger/);
    f.host.querySelector('.cv-app-source-larger')!.click();
    assert.equal(f.larger[0][0].file_id, FILE); assert.equal(f.larger[0][1], BOX);
    f.respond(async () => response(lineage(file(FILE, 'Fresh metadata.csv'))));
    f.host.querySelector('.cv-app-source-back')!.click(); await flush();
    assert.equal(f.disposedViews(), 1); assert.equal(f.calls.length, 2);
    assert.match(f.host.textContent, /Fresh metadata.csv/);
  } finally { f.cleanup(); }
});

test('multiple source files stay separate and filenames are text', async () => {
  const f = await sourcesFixture();
  try {
    f.respond(async () => response(lineage(file(FILE, '<img src=x>.csv'), file(OTHER, 'Workbook.xlsx', 'xlsx'))));
    f.controller.show(); await flush();
    assert.match(f.host.textContent, /2 original files/); assert.equal(f.host.querySelectorAll('.cv-app-source').length, 2);
    assert.equal(f.host.querySelector('img'), null); assert.match(f.host.textContent, /<img src=x>.csv/);
    f.host.querySelectorAll('.cv-app-source-ask')[1]!.click();
    assert.equal(f.asked[0].file.file_id, OTHER); assert.equal(f.asked[0].file.kind, 'xlsx');
  } finally { f.cleanup(); }
});

test('hide, removal and disposal prevent stale Sources responses or actions', async () => {
  const f = await sourcesFixture();
  try {
    let resolve!: (value: unknown) => void;
    f.respond(() => new Promise(done => { resolve = done; }));
    f.controller.show(); await flush(); f.controller.hide();
    assert.equal(f.calls[0]!.options.signal!.aborted, true);
    resolve(response(lineage(file()))); await flush(); assert.equal(f.host.children.length, 0);
    f.respond(async () => response(lineage(file()))); f.controller.show(); await flush();
    const oldAsk = f.host.querySelector('.cv-app-source-ask')!;
    f.controller.hide(); oldAsk.click(); assert.equal(f.asked.length, 0);
    f.respond(() => new Promise(done => { resolve = done; })); f.controller.show(); await flush();
    f.host.attached = false; resolve(response(lineage(file(FILE, 'Late secret.csv')))); await flush();
    assert.doesNotMatch(f.host.textContent, /Late secret/);
    f.controller.dispose(); f.host.attached = true; f.controller.show(); assert.equal(f.calls.length, 3);
  } finally { f.cleanup(); }
});

test('reopening reauthorizes metadata and clears revoked or malformed source lists', async () => {
  const f = await sourcesFixture();
  try {
    f.controller.show(); await flush(); f.host.querySelector('.cv-app-source-open')!.click();
    f.controller.hide(); assert.equal(f.disposedViews(), 1); assert.equal(f.host.children.length, 0);
    f.respond(async () => response({}, 404)); f.controller.show(); await flush();
    assert.match(f.host.textContent, /No source files are available/); assert.doesNotMatch(f.host.textContent, /Original.csv/);
    f.controller.hide(); f.respond(async () => response(lineage(file(), file()))); f.controller.show(); await flush();
    assert.match(f.host.textContent, /could not be loaded/); assert.equal(f.host.querySelector('.cv-app-source-open'), null);
    f.respond(async () => response(lineage(file()))); f.host.querySelector('button')!.click(); await flush();
    assert.match(f.host.textContent, /Original.csv/); assert.equal(f.calls.length, 4);
  } finally { f.cleanup(); }
});

async function viewerFixture() {
  const pages: number[][] = [];
  const page = (offset: number, columnOffset: number) => ({ format: 'csv', sheets: [], rows: [['001.2300', '<source>']], offset,
    column_offset: columnOffset, row_count: 100, column_count: 64, page_size: 50, column_page_size: 32,
    sha256: 'stable-digest', formula_count: 0, formula_cells: [], missing_formula_results: 0 });
  let respond = async (_sheet: number, offset: number, columnOffset: number): Promise<any> => page(offset, columnOffset);
  const f = await moduleFixture('box-viewer', () => ({ './box-api': {
    fetchOriginalPage: (_box: string, _file: string, ...position: number[]) => { pages.push(position); return respond(position[0]!, position[1]!, position[2]!); },
    formatBytes: String, refusalText: () => 'Original file is unavailable.',
  } }));
  const original = { ...file(), bytes: 0, content_url: url(BOX, FILE) };
  return { ...f, pages, page, original, respond: (fn: typeof respond) => { respond = fn; } };
}

test('the inline original viewer retains page navigation and discards superseded reads', async () => {
  const f = await viewerFixture();
  try {
    const controller = f.api.mountFileViewer(f.host, f.original, BOX); await flush();
    assert.match(f.host.textContent, /001.2300<source>/); assert.equal(f.host.querySelector('source'), null);
    assert.deepEqual(f.pages, [[0, 0, 0]]);
    f.host.querySelectorAll('button').find(button => button.textContent === 'Next 50 rows')!.click(); await flush();
    assert.deepEqual(f.pages[1], [0, 50, 0]);
    f.host.querySelectorAll('button').find(button => button.textContent === 'Next 32 columns')!.click(); await flush();
    assert.deepEqual(f.pages[2], [0, 50, 32]);
    const pending: ((value: unknown) => void)[] = [];
    f.respond(() => new Promise(done => { pending.push(done); }));
    f.host.querySelectorAll('button').find(button => button.textContent === 'Previous 50 rows')!.click();
    f.host.querySelectorAll('button').find(button => button.textContent === 'Table')!.click();
    pending[1]!({ ...f.page(0, 0), rows: [['Newest page']] }); await flush();
    pending[0]!({ ...f.page(0, 32), rows: [['Superseded page']] }); await flush();
    assert.match(f.host.textContent, /Newest page/); assert.doesNotMatch(f.host.textContent, /Superseded page/);
    let resolve!: (value: unknown) => void;
    f.respond(() => new Promise(done => { resolve = done; }));
    f.host.querySelectorAll('button').find(button => button.textContent === 'Next 50 rows')!.click();
    controller.dispose(); resolve({ ...f.page(0, 32), rows: [['Late original']] }); await flush();
    assert.equal(f.host.children.length, 0);
  } finally { f.cleanup(); }
});

test('inline text requests abort on disposal and cannot populate a removed view', async () => {
  const f = await viewerFixture(); const oldFetch = globalThis.fetch;
  try {
    let resolve!: (value: unknown) => void; let signal!: AbortSignal;
    globalThis.fetch = (async (_url: string, options: RequestInit) => { signal = options.signal!; return new Promise(done => { resolve = done; }); }) as typeof fetch;
    const controller = f.api.mountFileViewer(f.host, { ...f.original, kind: 'text' }, BOX);
    const pre = f.host.querySelector('pre')!; controller.dispose(); assert.equal(signal.aborted, true);
    resolve({ ok: true, text: async () => 'Late confidential text' }); await flush();
    assert.equal(f.host.children.length, 0); assert.equal(pre.textContent, 'Loading…');
  } finally { globalThis.fetch = oldFetch; f.cleanup(); }
});
