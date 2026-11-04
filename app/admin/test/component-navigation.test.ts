import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {build} from 'esbuild';

const APP = 'aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee';
const WORK = '11111111-2222-4333-8444-555555555555';
let serial = 0;

/** Execute the actual navigation entry points; transport, placement and Focus
 * are spies. No copied decision logic or backend authority is supplied. */
async function fixture(module: 'shape'|'explain', options: {hidden?: boolean; placed?: boolean; missing?: boolean} = {}) {
  const entry = fileURLToPath(new URL(`../src/canvas/${module}.ts`, import.meta.url));
  const source = await readFile(entry, 'utf8');
  const exports = new Map<string, string[]>();
  for (const [, names, path] of source.matchAll(/import\s*\{([^}]+)\}\s*from\s*['"]([^'"]+)['"]/gs))
    exports.set(path!, names!.split(',').map(name => name.trim()).filter(name => name && !name.startsWith('type ')));
  const calls: unknown[][] = [], messages: string[] = [];
  let placed = options.placed ?? false;
  const receipt = {open: false};
  const card = {dataset: {card: `c-${WORK}-summary`, component: 'summary'}, closest: () => placed ? null : {}, querySelector: () => receipt};
  const row = {id: card.dataset.card, slug: 'summary', title: 'Summary', number: '1.2', app: {app: APP, work: WORK}};
  const mocks: Record<string, Record<string, (...args: any[]) => any>> = {
    './board': {
      ensureAppGroup: async (...args) => { calls.push(['load', ...args]); return options.missing ? null : {cards: [card]}; },
      isOnBoard: () => placed,
      addCards: () => { throw new Error('Component navigation must not place loose cards'); },
    },
    './unite': {setAppView: async (...args) => {calls.push(['place', ...args]); placed = true;}},
    './focus': {openFocus: (...args) => {calls.push(['focus', ...args]); return placed;}},
    './render': {announce: (message) => messages.push(message), isUuid: (value) => /^[0-9a-f-]{36}$/.test(value)},
    '../presentation/presentation': {appComponentVisible: () => !options.hidden},
    './talk-view': {splitAddress: (name) => ({space: '', name})},
    './widgets/index': {widgetKind: () => null},
    './command-registry': {findCardShortcut: () => null},
    './library': {findCards: () => [row], cardAddress: () => 'app.summary'},
  };
  const key = `__componentNavigation${serial++}`;
  (globalThis as any)[key] = mocks;
  const result = await build({entryPoints: [entry], bundle: true, format: 'esm', platform: 'node', write: false,
    plugins: [{name: 'navigation-boundaries', setup(builder) {
      builder.onResolve({filter: /.*/}, args => args.importer === entry && exports.has(args.path) ? {path: args.path, namespace: 'navigation-spy'} : undefined);
      builder.onLoad({filter: /.*/, namespace: 'navigation-spy'}, args => ({contents: exports.get(args.path)!.map(name => `export const ${name} = (...args) => globalThis[${JSON.stringify(key)}][${JSON.stringify(args.path)}]?.[${JSON.stringify(name)}]?.(...args);`).join('\n'), loader: 'js'}));
    }}],
  });
  const api = await import('data:text/javascript;base64,' + Buffer.from(result.outputFiles[0]!.text).toString('base64'));
  const previousDocument = globalThis.document;
  (globalThis as any).document = {getElementById: () => null};
  return {api, calls, messages, receipt, cleanup: () => {delete (globalThis as any)[key]; (globalThis as any).document = previousDocument;}};
}

test('Explain opens a cold whole app, then focuses its original component and receipt', async () => {
  const f = await fixture('explain');
  try {
    await f.api.showValue(APP, WORK, 'summary');
    assert.deepEqual(f.calls, [['load', APP, WORK], ['place', APP, WORK, 'open', {focus: false}], ['focus', `c-${WORK}-summary`]]);
    assert.equal(f.receipt.open, true);
    const existing = await fixture('explain', {placed: true});
    try {await existing.api.showValue(APP, WORK, 'summary'); assert.equal(existing.calls.some(call => call[0] === 'place'), false); assert.equal(existing.receipt.open, true);} finally {existing.cleanup();}
  } finally {f.cleanup();}
});

test('Explain refuses a personally hidden component without placement or Focus', async () => {
  const f = await fixture('explain', {hidden: true});
  try {await f.api.showValue(APP, WORK, 'summary'); assert.deepEqual(f.calls, [['load', APP, WORK]]); assert.match(f.messages[0]!, /hidden in your views/); assert.equal(f.receipt.open, false);} finally {f.cleanup();}
});

test('cold Shape opens the whole app before reading its retained component', async () => {
  const f = await fixture('shape');
  try {
    await f.api.runShape('app.summary', 'make a total', (message: string) => f.messages.push(message));
    assert.deepEqual(f.calls, [['load', APP, WORK], ['place', APP, WORK, 'open', {focus: false}]]);
    assert.match(f.messages[0]!, /Shaping/);
    assert.match(f.messages[1]!, /no data to shape/);
  } finally {f.cleanup();}
});

test('Shape refuses hidden or unavailable components without placement or a model request', async () => {
  for (const options of [{hidden: true}, {missing: true}]) {
    const f = await fixture('shape', options);
    try {await f.api.runShape('app.summary', 'make a total', (message: string) => f.messages.push(message)); assert.deepEqual(f.calls, [['load', APP, WORK]]); assert.match(f.messages[0]!, options.hidden ? /hidden in your views/ : /could not be placed/);} finally {f.cleanup();}
  }
});


test('Summary receipt navigation stops if its owning panel closes during lookup', async () => {
  const f = await fixture('explain');
  let current = true;
  try {
    const navigation = f.api.showValue(APP, WORK, 'summary', () => current);
    current = false;
    await navigation;
    assert.deepEqual(f.calls, [['load', APP, WORK]]);
    assert.equal(f.receipt.open, false);
  } finally {f.cleanup();}
});
