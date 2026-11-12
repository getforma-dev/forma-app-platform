// APP-098 DS-84: the "Sample data" origin chip keeps its own style, so the
// library's synthetic numbers never look like the member's own data.
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { groupOrigins, originKind } from '../src/canvas/render.ts';

test('origin chips keep their kinds, and sample data has its own (APP-098 DS-84)', () => {
  assert.equal(originKind('Sample data'), 'sample');
  assert.equal(originKind('Retrieved from library'), 'library');
  assert.equal(originKind('Seeded definition'), 'seeded');
  assert.equal(originKind('Generated'), 'generated');
  assert.equal(originKind('Adapted to your request'), 'adapted');
});

test('the Sample data chip survives the composer\'s assembly labels (APP-098 DS-84)', () => {
  const served = [
    { id: 'o0', kind: 'library', text: 'Retrieved from library' },
    { id: 'o1', kind: 'sample', text: 'Sample data' },
  ];
  // "Back to standard" passes the composer's label; the data chip stays.
  assert.deepEqual(groupOrigins(['retrieved'], served).map((origin) => origin.text), ['retrieved', 'Sample data']);
  // With no labels, the server's chips are used as they are.
  assert.deepEqual(groupOrigins([], served).map((origin) => origin.kind), ['library', 'sample']);
  // A work without sample data gains no chip.
  assert.deepEqual(groupOrigins(['retrieved'], [served[0]!]).map((origin) => origin.text), ['retrieved']);
});
