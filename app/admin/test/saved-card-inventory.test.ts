import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {build} from 'esbuild';
import type {StoredCard, StoredCardChange} from '../src/canvas/cards';

const APP = 'aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee', WORK = '11111111-2222-4333-8444-555555555555';
const card = (suffix: string, kind: 'card'|'answer' = 'card'): StoredCard => ({card_id: `22222222-2222-4222-8222-${suffix.padStart(12, '0')}`, kind, app_id: APP, work_id: WORK, source: 'summary', address: 'app.summary', title: `Saved ${suffix}`, spec: {}, everyone: false, mine: true, can_share: true, can_manage: false, created_at: '2026-11-02T12:00:00Z'});
let serial = 0;
async function fixture() {
  const entry = fileURLToPath(new URL('../src/canvas/talk.ts', import.meta.url)), source = await readFile(entry, 'utf8');
  const exports = new Map<string,string[]>();
  for (const [, names, path] of source.matchAll(/import\s*\{([^}]+)\}\s*from\s*['"]([^'"]+)['"]/gs)) exports.set(path!, names!.split(',').map(n => n.trim()).filter(n => n && !n.startsWith('type ')));
  const snapshots: {entries: any[]; message: string; show(id: string): Promise<string>; refresh(): Promise<string>}[] = [];
  const queue: any[] = [], nodes = new Map<string, object>(), rendered: string[] = [], centered: object[] = [], removed: object[] = [];
  let reads = 0, change: (change: StoredCardChange) => void = () => undefined, gate: Promise<void> | null = null;
  const render = async (value: StoredCard) => {if (gate) await gate; rendered.push(value.card_id); nodes.set(`utility-${value.kind === 'card' ? 'made' : 'pin'}-${value.card_id}`, {id: value.card_id}); return true;};
  const mocks: Record<string,Record<string,(...args: any[]) => any>> = {
    './cards': {cardInventory: async () => {reads++; return await queue.shift();}, onStoredCardChange: fn => {change = fn;}},
    './library': {setSavedCardInventory: (entries, show, message, refresh) => snapshots.push({entries: [...entries], show, message, refresh})},
    './shape': {placeMadeCard: render}, './ask': {placePinnedAnswer: (value) => {rendered.push(value.card_id); nodes.set(`utility-pin-${value.card_id}`, {id: value.card_id}); return true;}},
    './board': {canvasApi: () => ({centerItem: (node: object) => centered.push(node)}), removeUtilityItem: (node) => {removed.push(node); for (const [id, found] of nodes) if (node === found) nodes.delete(id);}},
  };
  const key = `__savedInventory${serial++}`; (globalThis as any)[key] = mocks;
  const built = await build({entryPoints: [entry], bundle: true, format: 'esm', platform: 'node', write: false, plugins: [{name: 'saved-card-boundaries', setup(b) {
    b.onResolve({filter: /.*/}, a => a.importer === entry && exports.has(a.path) ? {path: a.path, namespace: 'saved-spy'} : undefined);
    b.onLoad({filter: /.*/, namespace: 'saved-spy'}, a => ({contents: exports.get(a.path)!.map(name => `export const ${name} = (...args) => globalThis[${JSON.stringify(key)}][${JSON.stringify(a.path)}]?.[${JSON.stringify(name)}]?.(...args);`).join('\n'), loader: 'js'}));
  }}]});
  const oldDocument = globalThis.document;
  (globalThis as any).document = {querySelector: (selector: string) => nodes.get(selector.match(/data-utility="([^"]+)"/)?.[1] ?? '') ?? null};
  const api = await import('data:text/javascript;base64,' + Buffer.from(built.outputFiles[0]!.text).toString('base64'));
  return {api, queue, snapshots, rendered, nodes, removed, centered, reads: () => reads, change: (value: StoredCardChange) => change(value), gate: (value: Promise<void>) => {gate = value;}, cleanup: () => {delete (globalThis as any)[key]; (globalThis as any).document = oldDocument;}};
}

test('one authorized inventory populates exact saved-card IDs, and Show reauthorizes without duplicate placement', async () => {
  const f = await fixture(), first = card('1'), answer = card('2', 'answer');
  try {
    f.queue.push({ok: true, cards: [first, answer]}); assert.equal(await f.api.placeMemberCards(), 1); assert.equal(await f.api.placeMemberCards(), 1); assert.deepEqual(f.rendered, [first.card_id], "app answers stay filed until explicitly placed"); assert.equal(f.reads(), 1);
    const list = f.snapshots.at(-1)!; assert.deepEqual(list.entries.map(row => row.id), [first.card_id, answer.card_id]); assert.equal(list.entries[1].kind, 'answer'); assert.equal('spec' in list.entries[1], false);
    f.queue.push({ok: true, cards: [{...first}, {...answer}]}); assert.match(await list.show(answer.card_id), /on your canvas/); assert.equal(f.reads(), 2); assert.equal(f.rendered.length, 2);
    f.queue.push({ok: true, cards: [first, answer]}); await list.show(answer.card_id); assert.equal(f.rendered.length, 2); assert.equal(f.centered.length, 2);
  } finally {f.cleanup();}
});

test('confirmed store/remove events win over a stale initial list and need no polling', async () => {
  const f = await fixture(), old = card('3'), added = card('4');
  try {
    let resolve!: (value: unknown) => void; f.queue.push(new Promise(r => {resolve = r;})); const initial = f.api.placeMemberCards();
    f.change({card: added}); f.change({removed: old.card_id}); resolve({ok: true, cards: [old]});
    assert.equal(await initial, 1); assert.deepEqual(f.snapshots.at(-1)!.entries.map(row => row.id), [added.card_id]); assert.deepEqual(f.rendered, [added.card_id]); assert.equal(f.reads(), 1);
  } finally {f.cleanup();}
});

test('failed refresh retains metadata and refuses new content, with manual recovery even from empty initial inventory', async () => {
  const f = await fixture(), value = card('5');
  try {
    f.queue.push({ok: false, message: 'Unavailable'}); assert.equal(await f.api.placeMemberCards(), 0); const empty = f.snapshots.at(-1)!; assert.equal(empty.entries.length, 0);
    f.queue.push({ok: true, cards: [value]}); assert.match(await empty.refresh(), /refreshed/); assert.equal(f.rendered.length, 0);
    f.queue.push({ok: false, message: 'Unavailable'}); assert.match(await f.snapshots.at(-1)!.show(value.card_id), /Nothing new was shown/); assert.equal(f.snapshots.at(-1)!.entries[0].id, value.card_id); assert.equal(f.rendered.length, 0);
  } finally {f.cleanup();}
});

test('removal while saved-card data loads cannot resurrect its canvas utility', async () => {
  const f = await fixture(), value = card('6');
  try {
    f.queue.push({ok: true, cards: []}); await f.api.placeMemberCards(); f.change({card: value});
    let release!: () => void; f.gate(new Promise(r => {release = r;})); f.queue.push({ok: true, cards: [value]});
    const show = f.snapshots.at(-1)!.show(value.card_id); await new Promise(r => setTimeout(r, 0)); f.change({removed: value.card_id}); release();
    assert.match(await show, /changed while it was opening/); assert.equal(f.nodes.size, 0); assert.equal(f.removed.length, 1); assert.equal(f.snapshots.at(-1)!.entries.length, 0);
  } finally {f.cleanup();}
});

test('a broken view listener cannot change confirmed store/share/remove outcomes', async () => {
  const api = await import('../src/canvas/cards'); const oldFetch = globalThis.fetch, value = card('7'), observed: StoredCardChange[] = [];
  const offBroken = api.onStoredCardChange(() => {throw new Error('broken view');}), off = api.onStoredCardChange(event => observed.push(event));
  globalThis.fetch = async () => new Response(JSON.stringify({card: value}), {status: 200, headers: {'content-type': 'application/json'}});
  try {
    assert.equal((await api.storeCard({}) as StoredCard).card_id, value.card_id);
    assert.equal((await api.shareCard(value.card_id, true) as StoredCard).card_id, value.card_id);
    assert.equal(await api.removeCard(value.card_id), true); assert.equal(observed.length, 3);
    globalThis.fetch = async () => new Response('{}', {status: 503}); assert.equal((await api.cardInventory()).ok, false); assert.equal(typeof await api.storeCard({}), 'string'); assert.equal(observed.length, 3);
  } finally {globalThis.fetch = oldFetch; offBroken(); off();}
});

test('successful inventory absence withdraws a displayed answer, while an outage does not', async () => {
  const f = await fixture(), value = card('8', 'answer');
  try {
    f.queue.push({ok: true, cards: [value]}); await f.api.placeMemberCards(); assert.equal(f.nodes.size, 0);
    f.queue.push({ok: true, cards: [value]}); await f.snapshots.at(-1)!.show(value.card_id); const node = [...f.nodes.values()][0];
    f.queue.push({ok: false, message: 'Unavailable'}); await f.snapshots.at(-1)!.refresh(); assert.equal(f.nodes.size, 1); assert.equal(f.removed.length, 0);
    f.queue.push({ok: true, cards: []}); await f.snapshots.at(-1)!.refresh(); assert.equal(f.nodes.size, 0); assert.deepEqual(f.removed, [node]); assert.equal(f.snapshots.at(-1)!.entries.length, 0);
  } finally {f.cleanup();}
});

test('a slow initial restore cannot remove the newer valid Show placement of the same saved ID', async () => {
  const f = await fixture(), value = card('9');
  try {
    let release!: () => void; f.gate(new Promise(r => {release = r;})); f.queue.push({ok: true, cards: [value]});
    const initial = f.api.placeMemberCards(); await new Promise(r => setTimeout(r, 0));
    f.queue.push({ok: true, cards: [{...value}]}); const show = f.snapshots.at(-1)!.show(value.card_id); await new Promise(r => setTimeout(r, 0));
    assert.equal(f.rendered.length, 0); release(); await initial;
    assert.match(await show, /on your canvas/); assert.equal(f.nodes.size, 1); assert.equal(f.rendered.length, 2); assert.equal(f.removed.length, 1);
    const current = [...f.nodes.values()][0]; assert.notEqual(current, f.removed[0]); assert.equal(f.centered[0], current);
  } finally {f.cleanup();}
});
