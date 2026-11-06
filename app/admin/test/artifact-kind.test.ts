import assert from 'node:assert/strict';
import test from 'node:test';
import { mergeArtifactKinds } from '../src/canvas/layout.ts';

const app = '00000000-0000-4000-8000-0000000000a1';
const card = '00000000-0000-4000-8000-0000000000c1';
const many = '00000000-0000-4000-8000-0000000000c2';
const plain = '00000000-0000-4000-8000-0000000000a2';
const local = '00000000-0000-4000-8000-0000000000c3';
const dropped = '00000000-0000-4000-8000-0000000000c4';
const unlisted = '00000000-0000-4000-8000-0000000000c5';

test('saved kinds are explicit, not a count of components, and never drop a local Card the server lacks (APP-169)', () => {
  const merged = mergeArtifactKinds(new Set([`${app}/${local}`, `${app}/${dropped}`, `${app}/${unlisted}`]), [
    { app, work: card, kind: 'card' },
    { app, work: many, kind: 'card' },
    { app, work: plain, kind: null },
    { app, work: local, kind: null },
    { app, work: local, kind: null },
    { app, work: dropped, kind: 'app' },
    { app: 'not-an-id', work: card, kind: 'card' },
    { app, work: plain, kind: 'stack' },
  ]);
  assert.deepEqual(merged.cards, new Set([`${app}/${local}`, `${app}/${unlisted}`, `${app}/${card}`, `${app}/${many}`]));
  assert.deepEqual(merged.upload, [`${app}/${local}`], 'a local Card with no saved choice is uploaded once');
});

test('with no saved choices on the page nothing local changes and nothing is uploaded (APP-169)', () => {
  const merged = mergeArtifactKinds(new Set([`${app}/${local}`]), []);
  assert.deepEqual(merged.cards, new Set([`${app}/${local}`]));
  assert.deepEqual(merged.upload, []);
});
