// APP-102 acceptance 4: the ledger's totals for a date range equal the sum of
// the listed live records in that range. `totals()` is the client arithmetic
// the ledger card shows (exact minor units as bigint, per currency).
import assert from 'node:assert/strict';
import { test } from 'node:test';

const COLLECTION = '00000000-0000-4000-8000-000000001683';
const OTHER = '00000000-0000-4000-8000-000000001699';
const money = (cents: string, currency = 'USD') => ({ type: 'money', minor_units: cents, currency, minor_unit_exponent: 2 });
let serial = 0;
const record = (who: string, day: string, status: string, category: string, amount: unknown, collection = COLLECTION) => ({
  record_id: `00000000-0000-4000-8000-0000000c${String(++serial).padStart(4, '0')}`, collection_id: collection, schema_id: 'expense_claim', version: 1, commit_sequence: serial,
  committed_at: `${day}T12:00:00Z`, creator: { principal_id: who, label: who }, updater: { principal_id: who, label: who },
  values: [
    ...(amount ? [{ field_id: 'amount', value: amount, display: '' }] : []),
    { field_id: 'spent_on', value: { type: 'date', value: day }, display: day },
    { field_id: 'status', value: { type: 'enum', value: status }, display: status },
    { field_id: 'category', value: { type: 'enum', value: category }, display: category },
  ],
});
const LISTING = {
  app_id: '00000000-0000-4000-8000-000000001681', work_id: '00000000-0000-4000-8000-000000001684', publication_id: '00000000-0000-4000-8000-0000000016a1',
  definition_digest: `sha256:${'a'.repeat(64)}`, work_version: 1, can_write: true, collections: [], truncated: false,
  records: [
    record('Diego Ramos', '2026-10-01', 'submitted', 'meals', money('8600')),
    record('Diego Ramos', '2026-10-14', 'approved', 'travel', money('41200')),
    record('Sarah Chen', '2026-10-30', 'approved', 'meals', money('1999')),
    record('Sarah Chen', '2026-10-31', 'approved', 'meals', money('5000')),
    record('Priya Natarajan', '2026-09-30', 'approved', 'software', money('1500')),
    record('Priya Natarajan', '2026-10-20', 'rejected', 'software', null),
    record('Diego Ramos', '2026-10-15', 'approved', 'travel', money('90000'), OTHER),
    record('Sarah Chen', '2026-10-16', 'approved', 'travel', money('700', 'EUR')),
  ],
};
const CARD = { collection: COLLECTION, amount: 'amount', date: 'spent_on', category: 'category', status: 'status' };

test('ledger totals for a range equal the sum of the listed records in that range, per currency (APP-102)', async () => {
  const { totals } = await import('../src/canvas/ledger.ts');
  const october = totals(LISTING as never, CARD, { from: '2026-10-01', to: '2026-10-30', status: '' });
  // In range and in the collection: 8600 + 41200 + 1999 (USD), 700 (EUR), and one record with no amount.
  assert.equal(october.count, 5);
  assert.deepEqual([...october.total], [['USD:2', 51799n], ['EUR:2', 700n]]);
  // The range is inclusive at both ends: 2026-10-01 and 2026-10-30 count; 09-30 and 10-31 do not.
  const bounds = totals(LISTING as never, CARD, { from: '2026-10-30', to: '2026-10-30', status: '' });
  assert.deepEqual([...bounds.total], [['USD:2', 1999n]]);
  // A status filter keeps only that status.
  const approved = totals(LISTING as never, CARD, { from: '2026-10-01', to: '2026-10-30', status: 'approved' });
  assert.equal(approved.count, 3);
  assert.deepEqual([...approved.total], [['USD:2', 43199n], ['EUR:2', 700n]]);
  // People and categories add up to the same total.
  const sum = (rows: [string, number, Map<string, bigint>][]) => rows.reduce((t, [, , amounts]) => t + (amounts.get('USD:2') ?? 0n), 0n);
  assert.equal(sum(october.people), 51799n);
  assert.equal(sum(october.categories), 51799n);
  assert.deepEqual(october.people.map(([name, n]) => [name, n]), [['Diego Ramos', 2], ['Sarah Chen', 2], ['Priya Natarajan', 1]]);
  // No filter: every record of the collection.
  const all = totals(LISTING as never, CARD, { from: '', to: '', status: '' });
  assert.equal(all.count, 7);
  assert.deepEqual([...all.total], [['USD:2', 58299n], ['EUR:2', 700n]]);
});
