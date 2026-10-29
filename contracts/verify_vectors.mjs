// Independent JavaScript implementation checks the cross-language byte contract.
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { createHash } from 'node:crypto';

// Registered digest domains of edition impl-2026-10-29.2; the first eight are edition impl-2026-10-28.1's.
const DOMAINS = new Set([
  'forma:app-definition:v1', 'forma:input-contract:v1', 'forma:record-schema:v1', 'forma:query:v1',
  'forma:query-receipt:v1', 'forma:command-payload:v1', 'forma:source-binding:v1', 'forma:result:v1',
  'forma:normalized-rows:v1', 'forma:job-payload:v1',
]);
const MAX_DIGITS = 38;

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

function gcd(a, b) {
  while (b !== 0n) [a, b] = [b, a % b];
  return a;
}

// exact_rational: signed numerator and positive denominator, both at most 38 digits, in lowest terms.
function exactRational(numerator, denominator) {
  // Canonical decimal integers, each at most 38 digits before reduction (as validate.py and Rust).
  for (const text of [numerator, denominator]) {
    assert.match(text, /^(0|-?[1-9][0-9]*)$/, 'decimal integer');
    assert.ok(text.replace('-', '').length <= MAX_DIGITS, 'more than 38 digits');
  }
  let [n, d] = [BigInt(numerator), BigInt(denominator)];
  assert.notEqual(d, 0n, 'zero denominator');
  if (d < 0n) [n, d] = [-n, -d];
  const divisor = gcd(n < 0n ? -n : n, d);
  [n, d] = [n / divisor, d / divisor];
  return { type: 'exact_rational', numerator: n.toString(), denominator: d.toString() };
}

function load(path) {
  return JSON.parse(readFileSync(new URL(path, import.meta.url), 'utf8'));
}

function verify(vectors) {
  for (const vector of vectors) {
    const domain = vector.domain ?? 'forma:result:v1';
    assert.ok(DOMAINS.has(domain), `${vector.name}: unregistered domain`);
    const bytes = Buffer.from(JSON.stringify(ordered(vector.value)), 'utf8');
    assert.equal(bytes.toString('hex'), vector.utf8_hex, vector.name);
    assert.equal('sha256:' + createHash('sha256').update(domain + '\0', 'utf8').update(bytes).digest('hex'), vector.digest, vector.name);
    if (vector.rational) {
      assert.deepEqual(exactRational(vector.rational.numerator, vector.rational.denominator), vector.value, vector.name);
    }
  }
}

const editionOne = load('./fixtures/impl-2026-10-28.1/canonical-vectors.json');
const editionTwo = load('./fixtures/canonical-vectors.json');
verify(editionOne);
verify(editionTwo);
// Edition two retains every edition-one vector verbatim and adds vectors in its new domains and the rational value.
for (const vector of editionOne) assert.ok(editionTwo.some(other => JSON.stringify(other) === JSON.stringify(vector)), `${vector.name} not retained`);
const count = predicate => editionTwo.filter(predicate).length;
assert.equal(count(v => v.domain === 'forma:normalized-rows:v1'), 2, 'normalized-rows vectors');
assert.equal(count(v => v.domain === 'forma:job-payload:v1'), 1, 'job-payload vectors');
assert.ok(count(v => v.rational) >= 3, 'exact_rational vectors');
assert.throws(() => exactRational('1', '0'), /zero denominator/);
assert.throws(() => exactRational('1' + '0'.repeat(MAX_DIGITS), '1'), /38 digits/);
assert.throws(() => exactRational('1' + '0'.repeat(MAX_DIGITS), '10'), /38 digits/);
for (const text of ['-0', '01', ' 12', '0x10', '1.5', '']) assert.throws(() => exactRational(text, '1'), /decimal integer/);
console.log(`${editionOne.length} edition-one and ${editionTwo.length} edition-two canonical byte/digest vectors match independently in JavaScript`);
