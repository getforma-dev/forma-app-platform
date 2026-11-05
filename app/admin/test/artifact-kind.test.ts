import assert from 'node:assert/strict';
import test from 'node:test';
import { durableCardKeys } from '../src/canvas/layout.ts';

const app = '00000000-0000-4000-8000-0000000000a1';
const card = '00000000-0000-4000-8000-0000000000c1';
const many = '00000000-0000-4000-8000-0000000000c2';
const plain = '00000000-0000-4000-8000-0000000000a2';

test('durable cards are explicit kinds, not a count of components (APP-169)', () => {
  assert.deepEqual(durableCardKeys([
    { app, work: card, kind: 'card' },
    { app, work: many, kind: 'card' },
    { app, work: plain, kind: '' },
    { app, work: plain, kind: 'app' },
    { app: 'not-an-id', work: card, kind: 'card' },
  ]), [`${app}/${card}`, `${app}/${many}`]);
});
