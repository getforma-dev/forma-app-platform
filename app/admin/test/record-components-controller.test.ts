import assert from 'node:assert/strict';
import { test } from 'node:test';
import { matchingBatch } from '../src/record-components/controller.ts';

const component = { id: 'total', state: 'ready', filters: [], rows: [], columns: [], groups: [], has_receipt: true, message: 'All records' };
test('a dashboard replacement requires the current filter generation and the whole exact component set', () => {
  const ids = new Set(['total', 'trend']);
  const batch = { filter_epoch: 3, components: [component, { ...component, id: 'trend' }] };
  assert.equal(matchingBatch(batch, 3, ids), true);
  assert.equal(matchingBatch(batch, 4, ids), false, 'late older filter result');
  assert.equal(matchingBatch({ ...batch, components: [component] }, 3, ids), false, 'never replace only some linked metrics');
  assert.equal(matchingBatch({ ...batch, components: [component, component] }, 3, ids), false);
  assert.equal(matchingBatch({ ...batch, components: [component, { ...component, id: 'foreign' }] }, 3, ids), false);
});
