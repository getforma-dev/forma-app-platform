// Independent JavaScript implementation checks the cross-language byte contract.
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { createHash } from 'node:crypto';

function ordered(value, depth = 0) {
  assert.ok(depth <= 32, 'depth');
  if (value === null || typeof value === 'boolean') return value;
  if (typeof value === 'number') {
    assert.ok(Number.isSafeInteger(value), 'safe integer required');
    return value;
  }
  if (typeof value === 'string') {
    for (const scalar of value) {
      const code = scalar.codePointAt(0);
      assert.ok(code < 0xd800 || code > 0xdfff, 'unpaired surrogate');
    }
    return value;
  }
  if (Array.isArray(value)) return value.map(item => ordered(item, depth + 1));
  assert.equal(typeof value, 'object');
  return Object.fromEntries(Object.keys(value).sort().map(key => {
    assert.match(key, /^[a-z][a-z0-9_]*$/);
    return [key, ordered(value[key], depth + 1)];
  }));
}

const vectors = JSON.parse(readFileSync(new URL('./fixtures/canonical-vectors.json', import.meta.url), 'utf8'));
for (const vector of vectors) {
  const bytes = Buffer.from(JSON.stringify(ordered(vector.value)), 'utf8');
  assert.equal(bytes.toString('hex'), vector.utf8_hex, vector.name);
  assert.equal('sha256:' + createHash('sha256').update('forma:result:v1\0', 'utf8').update(bytes).digest('hex'), vector.digest, vector.name);
}
console.log(`${vectors.length} canonical byte/digest vectors match independently in JavaScript`);
