import assert from 'node:assert/strict';
import { test } from 'node:test';
import { loadUnited, saveUnited, migrateAppViews, loadStandaloneWorks, saveStandaloneWorks, type StoredUnite } from '../src/canvas/layout';

const APP = 'aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee';
const WORK = '11111111-2222-4333-8444-555555555555';
const SECOND = '11111111-2222-4333-8444-555555555556';
const MEMBER = '0123456789abcdef';
const old: StoredUnite = {app: APP, work: WORK, x: 12, y: 34, previous: {}, width: 800, height: 600};

test('app modes and last Open size survive browser layout persistence; old united rows mean Open', () => {
  const store = new Map<string, string>();
  Object.assign(globalThis, {localStorage: {getItem: (key: string) => store.get(key) ?? null, setItem: (key: string, value: string) => store.set(key, value)}});
  saveUnited(MEMBER, [old]);
  assert.deepEqual(loadUnited(MEMBER), [{...old, mode: 'open', top: 0}]);
  for (const mode of ['face', 'stack', 'open'] as const) {
    saveUnited(MEMBER, [{...old, mode, top: 3}]);
    assert.deepEqual(loadUnited(MEMBER), [{...old, mode, top: 3}]);
  }
  store.set(`forma.canvas.united.v1:${MEMBER}`, JSON.stringify([{...old, mode: 'javascript:bad', top: -8}]));
  assert.deepEqual(loadUnited(MEMBER), [{...old, mode: 'open', top: 0}]);
  assert.deepEqual(loadUnited('another-member'), []);
});

test('new or old united placement wins over a duplicate legacy stack without duplicating work', () => {
  const works = [{app: APP, work: WORK, cards: ['face', 'form']}];
  const stack = {id: 'pile1', title: 'Old pile', cards: ['face','form'], top: 1, x: 90, y: 100};
  assert.deepEqual(migrateAppViews([old], [stack], works), [{...old, mode: 'open'}]);
  assert.deepEqual(migrateAppViews([{...old, mode: 'face'}], [stack], works), [{...old, mode: 'face'}]);
});

test('legacy stacks become whole apps with their selected part and position; mixed piles separate deterministically', () => {
  const works = [{app: APP, work: WORK, cards: ['face', 'form']}, {app: APP, work: SECOND, cards: ['other']}];
  const stack = {id: 'pile1', title: 'Old pile', cards: ['other', 'form', 'face'], top: 1, x: 90, y: 100};
  const rows = migrateAppViews([], [stack], works);
  assert.deepEqual(rows.map(row => ({work: row.work, mode: row.mode, top: row.top, x: row.x, y: row.y})), [
    {work: WORK, mode: 'stack', top: 1, x: 90, y: 100},
    {work: SECOND, mode: 'stack', top: 0, x: 578, y: 100},
  ]);
});

test('loose app subsets migrate to Face while an explicitly independent card stays atomic Open', () => {
  const works = [{app: APP, work: WORK, cards: ['form'], at: {x: 9, y: 17}}, {app: APP, work: SECOND, cards: ['summary','form'], standalone: true}];
  const rows = migrateAppViews([], [], works);
  assert.equal(rows[0]?.mode, 'face'); assert.equal(rows[0]?.x, 9);
  assert.equal(rows[1]?.mode, 'open');
  assert.equal(migrateAppViews([{...old, mode: 'stack'}], [], [{...works[0]!, standalone: true}])[0]?.mode, 'open');
  assert.deepEqual(migrateAppViews([old], [], []), [], 'unloaded work is never inferred from cached metadata alone');
});

test('personal Card classification stores exact refs per member, caps malformed input and survives blocked storage', () => {
  const store = new Map<string,string>();
  Object.assign(globalThis, {localStorage: {getItem:(key:string)=>store.get(key)??null, setItem:(key:string,value:string)=>store.set(key,value)}});
  saveStandaloneWorks(MEMBER, new Set([`${APP}/${WORK}`, `${APP}/${SECOND}`, 'guess-from-title', `${APP}/${WORK}/extra`]));
  assert.deepEqual([...loadStandaloneWorks(MEMBER)], [`${APP}/${WORK}`, `${APP}/${SECOND}`]);
  assert.deepEqual([...loadStandaloneWorks('fedcba9876543210')], []);
  assert.ok(![...store.values()].join('').includes('guess-from-title'));
  saveStandaloneWorks(MEMBER, new Set()); assert.equal(loadStandaloneWorks(MEMBER).size, 0);
  Object.assign(globalThis,{localStorage:{getItem:()=>{throw new Error('blocked');},setItem:()=>{throw new Error('blocked');}}});
  assert.doesNotThrow(()=>saveStandaloneWorks(MEMBER,new Set([`${APP}/${WORK}`]))); assert.equal(loadStandaloneWorks(MEMBER).size,0);
});
