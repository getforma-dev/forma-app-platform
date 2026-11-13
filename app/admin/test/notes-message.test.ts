// APP-135: a note that cannot be saved says why. A missing Notes app (the
// uniform 404) differs from a temporary failure, which says to try again.
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { noteFailureMessage } from '../src/canvas/notes.ts';
import { shapeRefusal } from '../src/canvas/shape.ts';

test('the note failure message separates a missing Notes app from a temporary failure (APP-135)', () => {
  const missing = noteFailureMessage('missing');
  const temporary = noteFailureMessage('temporary');
  assert.match(missing, /no Notes app/);
  assert.doesNotMatch(missing, /Try again/);
  assert.match(temporary, /Try again/);
  assert.doesNotMatch(temporary, /no Notes app/);
  for (const message of [missing, temporary]) assert.match(message, /Nothing was saved\.$/);
});

test('the no-model Shape refusal suggests rehearsed sentences only in the demo (APP-135)', () => {
  const plain = shapeRefusal('composer_unavailable', false) ?? '';
  const demo = shapeRefusal('composer_unavailable', true) ?? '';
  assert.match(plain, /needs the AI model/);
  assert.doesNotMatch(plain, /red invoices/);
  assert.match(demo, /red invoices/);
  assert.equal(shapeRefusal('too_many_requests', false), shapeRefusal('too_many_requests', true));
});
