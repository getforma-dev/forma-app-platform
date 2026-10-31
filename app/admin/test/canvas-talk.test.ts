// APP-122: two AI jobs, chosen by where you type. `?` first is Ask; the dot
// (`/expenses.ledger words`) and `/shape` are Shape; `/weather Austin` still
// places. A card's view runs over its own data in the browser: filters and
// highlights choose rows, a group and a measure compute numbers, and every
// computed number keeps the receipts of the records it came from. Specs are
// closed data, judged again here. Ask sends only a question and its scope and
// shows only `{text, receipts, references}`.
import assert from 'node:assert/strict';
import { test } from 'node:test';

const APP = '00000000-0000-4000-8000-000000001681';
const WORK = '00000000-0000-4000-8000-000000001684';
const COLLECTION = '00000000-0000-4000-8000-000000001683';

const money = (cents: string) => ({ type: 'money', minor_units: cents, currency: 'USD', minor_unit_exponent: 2 });
const record = (n: number, merchant: string, cents: string, day: string, status: string) => ({
  record_id: `00000000-0000-4000-8000-00000000c00${n}`, collection_id: COLLECTION, schema_id: 'expense_claim', version: 1, commit_sequence: n,
  committed_at: `${day}T12:00:00Z`, creator: { principal_id: 'p', label: 'Diego Ramos' }, updater: { principal_id: 'p', label: 'Diego Ramos' },
  values: [
    { field_id: 'amount', value: money(cents), display: `$${(Number(cents) / 100).toFixed(2)}` },
    { field_id: 'merchant', value: { type: 'text', value: merchant }, display: merchant },
    { field_id: 'spent_on', value: { type: 'date', value: day }, display: day },
    { field_id: 'status', value: { type: 'enum', value: status }, display: status },
  ],
});
const LISTING = {
  app_id: APP, work_id: WORK, publication_id: '00000000-0000-4000-8000-0000000016a1', definition_digest: `sha256:${'a'.repeat(64)}`, work_version: 4, can_write: true,
  collections: [{
    collection_id: COLLECTION, schema_id: 'expense_claim', schema_version: 1, commands: [],
    fields: [
      { id: 'amount', type: 'money', nullable: false, writable: true },
      { id: 'merchant', type: 'text', nullable: false, writable: true },
      { id: 'spent_on', type: 'date', nullable: false, writable: true },
      { id: 'status', type: 'enum', nullable: false, writable: true, values: ['submitted', 'approved', 'rejected'] },
    ],
  }],
  records: [
    record(1, 'Nobu', '8600', '2026-10-28', 'submitted'),
    record(2, 'Delta', '41200', '2026-10-03', 'approved'),
    record(3, 'Figma', '1500', '2026-09-14', 'rejected'),
    record(4, 'Uber', '2350', '2026-09-02', 'approved'),
  ],
  truncated: false,
};

test('the bar: ? is Ask, the dot is a card’s Shape, and /weather Austin still places (APP-122)', async () => {
  const { barIntent } = await import('../src/canvas/talk-view.ts');
  assert.deepEqual(barIntent('? where are the red invoices'), { kind: 'ask', question: 'where are the red invoices' });
  assert.deepEqual(barIntent('?where'), { kind: 'ask', question: 'where' });
  assert.deepEqual(barIntent('/expenses.ledger highlight overdue in red'), { kind: 'dot', target: 'expenses.ledger', words: 'highlight overdue in red' });
  assert.deepEqual(barIntent('/expenses.total'), { kind: 'dot', target: 'expenses.total', words: '' });
  assert.deepEqual(barIntent('/3.2 by month'), { kind: 'dot', target: '3.2', words: 'by month' });
  // Everything else keeps its lane: Place, Do, Forma and the verbs.
  for (const text of ['/weather Austin', '/expenses $86 at Nobu, Sarah approves', '/shape calculator make it a TI-84', 'where are the red invoices', '/card expenses.ledger']) {
    assert.equal(barIntent(text), null, text);
  }
});

test('a shaped ledger previews highlights and filters over its records, keeping every receipt; nothing is written (APP-122)', async () => {
  const { tableFromRecords, evaluate, cardView, chips } = await import('../src/canvas/talk-view.ts');
  const before = JSON.stringify(LISTING);
  const table = tableFromRecords(LISTING as never, COLLECTION);
  // `/expenses.ledger highlight overdue in red`: the server's spec, judged again.
  const highlight = cardView({ highlight: [{ field: 'status', op: 'in', value: ['submitted'], color: 'crimson' }] })!;
  const shaped = evaluate(table, highlight);
  assert.equal(shaped.kind, 'rows');
  if (shaped.kind !== 'rows') return;
  assert.deepEqual(shaped.rows.map(({ row, color }) => [row.text.merchant, color]), [['Nobu', 'crimson'], ['Delta', null], ['Figma', null], ['Uber', null]]);
  assert.ok(shaped.rows.every(({ row }) => row.receipt?.record_id.startsWith('00000000-0000-4000-8000-00000000c00')), 'every row keeps its record receipt');
  assert.deepEqual(chips(highlight), ['crimson where status is submitted']);
  // `only red invoices`: a filter hides records; the shown ones keep their receipts.
  const red = evaluate(table, cardView({ filters: [{ field: 'status', op: 'in', value: ['submitted', 'rejected'] }] })!);
  assert.ok(red.kind === 'rows');
  if (red.kind === 'rows') {
    assert.deepEqual(red.rows.map(({ row }) => row.receipt?.record_id.slice(-1)), ['1', '3']);
    assert.equal(red.total, 4);
  }
  // `only over $200`: money compares exactly in minor units.
  const big = evaluate(table, cardView({ filters: [{ field: 'amount', op: 'gt', value: '200' }] })!);
  assert.ok(big.kind === 'rows' && big.rows.length === 1 && big.rows[0]!.row.text.merchant === 'Delta');
  assert.equal(JSON.stringify(LISTING), before, 'the listing is untouched');
});

test('/expenses.total by month computes a card whose numbers trace to their source records (APP-122)', async () => {
  const { tableFromRecords, evaluate, cardView } = await import('../src/canvas/talk-view.ts');
  const table = tableFromRecords(LISTING as never, COLLECTION);
  const view = cardView({ presentation: 'bar', group_by: 'spent_on', bucket: 'month', measure: { op: 'sum', field: 'amount' } })!;
  const shaped = evaluate(table, view);
  assert.equal(shaped.kind, 'groups');
  if (shaped.kind !== 'groups') return;
  assert.deepEqual(shaped.groups.map((g) => [g.label, g.measured.text, g.receipts.map((r) => r.record_id.slice(-1))]), [
    ['Sep 2026', '$38.50', ['3', '4']],
    ['Oct 2026', '$498.00', ['1', '2']],
  ]);
  assert.equal(shaped.groups.reduce((n, g) => n + g.receipts.length, 0), 4, 'every source record is a receipt of exactly one number');
  // The whole total, as a number card, keeps all four receipts.
  const total = evaluate(table, cardView({ presentation: 'number', measure: { op: 'sum', field: 'amount' } })!);
  assert.ok(total.kind === 'number' && total.measured.text === '$536.50' && total.receipts.length === 4);
});

test('specs are closed data: receipts cannot be dropped, nothing outside the sets passes (APP-122)', async () => {
  const { cardView, appTheme } = await import('../src/canvas/talk-view.ts');
  assert.equal(cardView({ presentation: 'number', receipts: false }), null, 'a view that would drop receipts');
  assert.equal(cardView({ text: 'The red ones are Nobu and Figma' }), null, 'prose');
  assert.equal(cardView({ palette: '#ff0000' }), null);
  assert.equal(cardView({ highlight: [{ field: 'status', op: 'eq', value: 'x', color: 'red; background:url(x)' }] }), null);
  assert.equal(cardView({ filters: [{ field: 'status', op: 'like', value: '%' }] }), null);
  assert.equal(cardView({ title: '<img src=x>' }), null);
  assert.equal(cardView({}), null);
  assert.deepEqual(appTheme({ mode: 'dark', accent: 'green', density: 'compact' }), { mode: 'dark', accent: 'green', density: 'compact' });
  assert.equal(appTheme({ accent: 'green', css: 'x' }), null);
  assert.equal(appTheme({ mode: 'midnight' }), null);
});

// --- A small fake DOM: enough for the Ask tab's thread ---
class Node_ {
  children: Node_[] = [];
  attributes = new Map<string, string>();
  text = '';
  constructor(public tag: string) {}
  setAttribute(name: string, value: string) { this.attributes.set(name, value); }
  append(...nodes: (Node_ | string)[]) { for (const node of nodes) this.children.push(typeof node === 'string' ? Object.assign(new Node_('#text'), { text: node }) : node); }
  remove() {}
  scrollIntoView() {}
  addEventListener() {}
  get textContent(): string { return this.text + this.children.map((c) => c.textContent).join(''); }
  set textContent(value: string) { this.text = value; this.children = []; }
}
const fakeDocument = {
  createElement: (tag: string) => new Node_(tag),
  createTextNode: (text: string) => Object.assign(new Node_('#text'), { text }),
  getElementById: () => null,
  querySelector: () => null,
  querySelectorAll: () => [],
};

test('Ask sends only the question and its scope, and shows only text with receipts; the canvas is untouched (APP-122)', async () => {
  (globalThis as { document?: unknown }).document = fakeDocument;
  const calls: { url: string; init: { method?: string; body?: string } }[] = [];
  const answer = {
    text: '2 records in expenses.ledger need attention: Nobu, $86.00, submitted; Figma, $15.00, rejected.',
    receipts: [{ kind: 'record', record_id: `00000000-0000-4000-8000-00000000c001`, version: 1, commit_sequence: 1 }],
    references: [{ ref: 'r1', app_id: APP, work_id: WORK, record_id: '00000000-0000-4000-8000-00000000c001', address: 'expenses', label: 'Nobu, $86.00, submitted' }],
  };
  (globalThis as { fetch?: unknown }).fetch = async (url: string, init: { method?: string; body?: string }) => {
    calls.push({ url, init });
    return { ok: true, status: 200, json: async () => answer };
  };
  const ask = await import('../src/canvas/ask.ts');
  const scope = { kind: 'card' as const, address: 'expenses.ledger', works: [{ app_id: APP, work_id: WORK, address: 'expenses' }] };
  assert.equal(ask.scopeText(scope), 'Asking about /expenses.ledger');
  ask.openAsk('', scope);
  await ask.ask('? where are all the red invoices?');
  assert.equal(calls.length, 1, 'one request, and only to Ask');
  assert.equal(calls[0]!.url, ask.ASK_URL);
  assert.equal(calls[0]!.init.method, 'POST');
  assert.deepEqual(JSON.parse(calls[0]!.init.body ?? '{}'), { question: 'where are all the red invoices?', scope: { kind: 'card', address: 'expenses.ledger', works: scope.works } });
  // A reply carrying anything but the three keys is not shown.
  answer.text = 'Made them red';
  (answer as Record<string, unknown>).highlight = [{ field: 'status', op: 'eq', value: 'submitted', color: 'crimson' }];
  await ask.ask('make them red');
  assert.equal(calls.length, 2);
  assert.equal(ask.scopeText({ kind: 'canvas', address: '', works: [scope.works[0]!, { ...scope.works[0]!, work_id: APP }] }), 'Asking about your canvas (2 apps)');
});
