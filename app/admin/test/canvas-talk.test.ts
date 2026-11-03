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

// --- APP-122 review: the merge with APP-121 naming, and the review's findings ---
const show = (value: unknown) => JSON.stringify(value, (_key, x) => (typeof x === 'bigint' ? String(x) : x));

test('/card:<app>.<card> <words> is exactly the dot’s Shape; /card:<x> alone stays the /card: verb’s (APP-121 × APP-122)', async () => {
  const { barIntent, splitAddress, firstWord } = await import('../src/canvas/talk-view.ts');
  assert.deepEqual(barIntent('/card:expenses.ledger only red invoices'), barIntent('/expenses.ledger only red invoices'));
  assert.deepEqual(barIntent('/card:Expenses.Ledger highlight overdue in red'), { kind: 'dot', target: 'expenses.ledger', words: 'highlight overdue in red' });
  assert.deepEqual(barIntent('/card:3.2 by month'), barIntent('/3.2 by month'));
  assert.equal(barIntent('/card:expenses.ledger'), null, 'no words: APP-121’s /card: path places it');
  assert.equal(barIntent('/app:expenses $86 Nobu'), null, 'an app’s full form is Do');
  assert.deepEqual(splitAddress('/card:expenses.ledger'), { space: 'card', name: 'expenses.ledger' });
  assert.deepEqual(splitAddress('App:Notes'), { space: 'app', name: 'notes' });
  assert.deepEqual(splitAddress('expenses'), { space: null, name: 'expenses' });
  assert.deepEqual(firstWord('  expenses.ledger which are late? '), ['expenses.ledger', 'which are late?']);
});

const decimal = (coefficient: string, scale: number) => ({ type: 'decimal', coefficient, scale, unit: 'hours' });
const hoursListing = (truncated = false) => ({
  ...LISTING,
  collections: [{ ...LISTING.collections[0]!, fields: [{ id: 'hours', type: 'decimal', nullable: false, writable: true }, { id: 'units', type: 'integer', nullable: false, writable: true }] }],
  records: [
    { ...record(1, 'a', '0', '2026-10-01', 'submitted'), values: [{ field_id: 'hours', value: decimal('12345', 2), display: '123.45' }, { field_id: 'units', value: { type: 'integer', value: '200' }, display: '200' }] },
    { ...record(2, 'b', '0', '2026-10-02', 'approved'), values: [{ field_id: 'hours', value: decimal('805', 1), display: '80.5' }, { field_id: 'units', value: { type: 'integer', value: '3' }, display: '3' }] },
  ],
  truncated,
});

test('decimals compare and sum at their scale, exactly; integers compare exactly (review finding 1)', async () => {
  const { tableFromRecords, evaluate, cardView, tableFromQuery } = await import('../src/canvas/talk-view.ts');
  const table = tableFromRecords(hoursListing() as never, COLLECTION);
  assert.equal(table.columns.find((c) => c.id === 'hours')?.scale, 2);
  assert.deepEqual(table.rows.map((row) => row.raw.hours), ['123.45', '80.5']);
  // 123.45 is not 12345: it is over 100, not over 200.
  const over = (value: string) => evaluate(table, cardView({ filters: [{ field: 'hours', op: 'gt', value }] })!);
  const big = over('100');
  assert.ok(big.kind === 'rows' && big.rows.length === 1 && big.rows[0]!.row.raw.hours === '123.45');
  const none = over('200');
  assert.ok(none.kind === 'rows' && none.rows.length === 0, '123.45 does not match gt 200');
  const sum = evaluate(table, cardView({ presentation: 'number', measure: { op: 'sum', field: 'hours' } })!);
  assert.ok(sum.kind === 'number' && sum.measured.text === '203.95', show(sum));
  const avg = evaluate(table, cardView({ presentation: 'number', measure: { op: 'avg', field: 'hours' } })!);
  assert.ok(avg.kind === 'number' && avg.measured.text === '101.9750' && !avg.measured.rounded, show(avg));
  const units = evaluate(table, cardView({ filters: [{ field: 'units', op: 'gte', value: '200.5' }] })!);
  assert.ok(units.kind === 'rows' && units.rows.length === 0, '200 is not ≥ 200.5');
  const average = evaluate(table, cardView({ presentation: 'number', measure: { op: 'avg', field: 'units' } })!);
  assert.ok(average.kind === 'number' && average.measured.text === '101.50' && !average.measured.rounded, show(average));
  // A view's decimal column holds its coefficient: read at the declared scale.
  const query = tableFromQuery({ view_id: 'v', columns: ['hours'], column_types: [{ type: 'decimal', scale: 2 }], rows: [['12345']], truncated_page: false, receipt: {} } as never);
  assert.equal(query.rows[0]!.raw.hours, '123.45');
  const q = evaluate(query, cardView({ filters: [{ field: 'hours', op: 'gt', value: '200' }] })!);
  assert.ok(q.kind === 'rows' && q.rows.length === 0);
});

test('a capped listing is never shown as complete: "From X of the newest Y …", and a total says it is partial (review finding 2)', async () => {
  const { tableFromRecords, evaluate, cardView, metaText } = await import('../src/canvas/talk-view.ts');
  const whole = tableFromRecords(LISTING as never, COLLECTION);
  const view = cardView({ presentation: 'number', measure: { op: 'sum', field: 'amount' } })!;
  const full = evaluate(whole, view);
  assert.equal(metaText(full, whole), 'From 4 of 4 records');
  assert.ok(full.kind === 'number' && full.measure === 'Total amount');
  const capped = tableFromRecords({ ...LISTING, truncated: true } as never, COLLECTION);
  assert.equal(capped.truncated, true);
  const partial = evaluate(capped, view);
  assert.ok(partial.kind === 'number' && partial.partial && partial.measure === 'Total amount (newest 4 only)', show(partial));
  assert.equal(metaText(partial, capped), 'From 4 of the newest 4 records loaded; older records exist and are not included');
  const rows = evaluate(capped, cardView({ filters: [{ field: 'status', op: 'eq', value: 'approved' }] })!);
  assert.equal(metaText(rows, capped), '2 of the newest 4 records loaded; older records exist and are not included shown');
});

test('money: an average rounds half-even to the minor unit and says so; currencies are never added together (review nits)', async () => {
  const { tableFromRecords, evaluate, cardView, divideHalfEven } = await import('../src/canvas/talk-view.ts');
  assert.deepEqual(divideHalfEven(3n, 2n), { quotient: 2n, exact: false });
  assert.deepEqual(divideHalfEven(5n, 2n), { quotient: 2n, exact: false });
  assert.deepEqual(divideHalfEven(-5n, 2n), { quotient: -2n, exact: false });
  assert.deepEqual(divideHalfEven(6n, 3n), { quotient: 2n, exact: true });
  const cents = (a: string, b: string, c: string) => ({ ...LISTING, records: [record(1, 'x', a, '2026-10-01', 'submitted'), record(2, 'y', b, '2026-10-01', 'submitted'), record(3, 'z', c, '2026-10-01', 'submitted')] });
  const avg = cardView({ presentation: 'number', measure: { op: 'avg', field: 'amount' } })!;
  const thirds = evaluate(tableFromRecords(cents('100', '200', '200') as never, COLLECTION), avg);
  assert.ok(thirds.kind === 'number' && thirds.measured.text === '$1.67 (rounded)' && thirds.measured.rounded, show(thirds));
  const even = evaluate(tableFromRecords(cents('1', '2', '0') as never, COLLECTION), avg);
  assert.ok(even.kind === 'number' && even.measured.text === '$0.01', show(even));
  const half = evaluate(tableFromRecords({ ...LISTING, records: [record(1, 'x', '1', '2026-10-01', 'a'), record(2, 'y', '2', '2026-10-01', 'a')] } as never, COLLECTION), avg);
  assert.ok(half.kind === 'number' && half.measured.text === '$0.02 (rounded)', 'half a cent rounds to even');
  // A euro claim beside dollar claims: summed per currency, never across.
  const euro = { ...record(5, 'Bistro', '1250', '2026-10-05', 'submitted') };
  euro.values = euro.values.map((v) => (v.field_id === 'amount' ? { ...v, value: { type: 'money', minor_units: '1250', currency: 'EUR', minor_unit_exponent: 2 } } : v)) as typeof euro.values;
  const mixed = tableFromRecords({ ...LISTING, records: [...LISTING.records, euro] } as never, COLLECTION);
  const total = evaluate(mixed, cardView({ presentation: 'number', measure: { op: 'sum', field: 'amount' } })!);
  assert.ok(total.kind === 'number' && total.measured.mixed === true, show(total));
  if (total.kind === 'number') {
    assert.equal(total.measured.text, '$536.50 · €12.50 (per currency, not added)');
    assert.equal(total.receipts.length, 5, 'every record still a receipt');
  }
  const big = evaluate(mixed, cardView({ filters: [{ field: 'amount', op: 'gt', value: '10' }] })!);
  assert.ok(big.kind === 'rows' && big.rows.some(({ row }) => row.text.merchant === 'Bistro'), 'a euro amount compares at its own exponent');
});

test('a newer preview owns its item: the older preview’s Cancel (its dock replaced) does not undo it (review finding 3)', async () => {
  (globalThis as { document?: unknown }).document = fakeDocument;
  const { claimPreview } = await import('../src/canvas/shape.ts');
  const { showDockCard } = await import('../src/canvas/dock-card.ts');
  const item = {};
  const screen: string[] = ['own view'];
  // Preview 1, its dock, whose Cancel restores the item…
  const first = claimPreview(item);
  screen.push('preview 1');
  const host = Object.assign(new Node_('div'), { replaceChildren() {} });
  const doc = { ...fakeDocument, getElementById: () => host, activeElement: null } as unknown as Document;
  const dock = () => Object.assign(new Node_('section'), { contains: () => false }) as unknown as HTMLElement;
  showDockCard(doc, dock(), () => { if (first()) screen.splice(1); });
  // …then preview 2 renders and opens its dock, which replaces (cancels) dock 1.
  const second = claimPreview(item);
  screen.push('preview 2');
  showDockCard(doc, dock(), () => { if (second()) screen.splice(1); });
  assert.deepEqual(screen, ['own view', 'preview 1', 'preview 2'], 'preview 2 is still shown');
  assert.equal(first(), false);
  assert.equal(second(), true);
  // Another item’s preview is independent.
  const other = claimPreview({});
  assert.equal(other(), true);
  assert.equal(second(), true);
});

test('Ask: a second question while one is answered waits its turn, none is dropped; ? never overwrites half-typed text (review finding 6, nit)', async () => {
  (globalThis as { document?: unknown }).document = fakeDocument;
  const bodies: string[] = [];
  let release: () => void = () => undefined;
  (globalThis as { fetch?: unknown }).fetch = async (_url: string, init: { body?: string }) => {
    bodies.push(JSON.parse(init.body ?? '{}').question);
    if (bodies.length === 1) await new Promise<void>((resolve) => { release = resolve; });
    return { ok: true, status: 200, json: async () => ({ text: 'ok', receipts: [], references: [] }) };
  };
  const ask = await import('../src/canvas/ask.ts');
  const scope = { kind: 'app' as const, address: 'expenses', works: [{ app_id: APP, work_id: WORK, address: 'expenses' }] };
  assert.equal(ask.scopeText(scope), 'Asking about /expenses (all its records)');
  const first = ask.ask('how many claims', scope);
  await new Promise((resolve) => setTimeout(resolve, 5));
  await ask.ask('what is the total', scope);
  assert.deepEqual(bodies, ['how many claims'], 'the second waits');
  release();
  await first;
  await new Promise((resolve) => setTimeout(resolve, 20));
  assert.deepEqual(bodies, ['how many claims', 'what is the total'], 'then it is asked');
  // `?` on the canvas: an empty bar gets `? `; half-typed text is only focused.
  const { primeAsk } = await import('../src/canvas/address-chip.ts');
  const input = Object.assign(new Node_('input'), { value: '', focus() { focused += 1; }, setSelectionRange() {}, dispatchEvent() { return true; } });
  let focused = 0;
  const doc = { ...fakeDocument, getElementById: () => input } as unknown as Document;
  primeAsk(doc);
  assert.equal(input.value, '? ');
  input.value = '/expenses $86 at No';
  primeAsk(doc);
  assert.equal(input.value, '/expenses $86 at No', 'never overwritten');
  assert.equal(focused, 2);
});
