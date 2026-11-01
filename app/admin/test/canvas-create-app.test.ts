import assert from 'node:assert/strict';
import { test } from 'node:test';
import { TEMPLATES, templatesIn, toggleFavorite } from '../src/canvas/create-app.ts';

test('a template is chosen before a description, and favorites are a separate list (APP-141)', () => {
  assert.deepEqual(templatesIn('industries', []).map((template) => template.id), ['expense-claims', 'reading-list']);
  assert.deepEqual(templatesIn('categories', []).map((template) => template.id), ['kept-list', 'a-count']);
  assert.deepEqual(templatesIn('favorites', []), []);
  const marked = toggleFavorite([], 'reading-list');
  assert.deepEqual(templatesIn('favorites', marked).map((template) => template.id), ['reading-list']);
  assert.deepEqual(toggleFavorite(marked, 'reading-list'), []);
  for (const template of TEMPLATES) assert.ok(template.starter.length > 10);
});
