// APP-098 DS-84: the "Sample data" origin chip keeps its own style, so the
// library's synthetic numbers never look like the member's own data.
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { originKind } from '../src/canvas/render.ts';

test('origin chips keep their kinds, and sample data has its own (APP-098 DS-84)', () => {
  assert.equal(originKind('Sample data'), 'sample');
  assert.equal(originKind('Retrieved from library'), 'library');
  assert.equal(originKind('Seeded definition'), 'seeded');
  assert.equal(originKind('Generated'), 'generated');
  assert.equal(originKind('Adapted to your request'), 'adapted');
});
