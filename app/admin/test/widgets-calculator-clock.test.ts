// APP-103: the calculator widget's parser. Acceptance: `/calculator` computes
// 12*(3+4) = 84, and the parser refuses anything but numbers, operators and
// parentheses. The parser is a closed recursive-descent grammar, never eval.
import assert from 'node:assert/strict';
import { test } from 'node:test';

test('the calculator computes the acceptance expression and ordinary arithmetic (APP-103)', async () => {
  const { evaluate, formatResult } = await import('../src/canvas/widgets/calculator.ts');
  assert.equal(evaluate('12*(3+4)'), 84);
  assert.equal(evaluate('12 × (3 + 4)'), 84, 'the keypad symbols are the same operators');
  assert.equal(evaluate('2+3*4'), 14, 'precedence');
  assert.equal(evaluate('(2+3)*4'), 20);
  assert.equal(evaluate('2^3^2'), 512, 'power is right-associative');
  assert.equal(evaluate('-2^2'), -4);
  assert.equal(evaluate('10 ÷ 4'), 2.5);
  assert.equal(evaluate('.5 + 1.25'), 1.75);
  assert.equal(evaluate('−3 + 5'), 2);
  assert.equal(evaluate('-0'), 0);
  assert.equal(formatResult(evaluate('0.1+0.2') ?? NaN), '0.3', 'no float noise on the display');
});

test('the calculator parser refuses anything but numbers, operators and parentheses (APP-103)', async () => {
  const { evaluate, tokenize } = await import('../src/canvas/widgets/calculator.ts');
  for (const hostile of [
    'alert(1)', '1+x', 'Math.PI', '2**3', '1;2', '1,2', '1e3', '0x10', '__proto__', 'constructor',
    '1+1)//', '`1`', '"1"', '[1]', '{1}', '1 % 2', '1 = 1', '<b>1</b>', 'NaN', 'Infinity',
  ]) {
    assert.equal(evaluate(hostile), null, `refused: ${hostile}`);
  }
  assert.equal(tokenize('alert(1)'), null, 'a letter refuses the whole expression');
  assert.equal(tokenize('1'.repeat(201)), null, 'over the length bound');
  for (const malformed of ['', '   ', '(1+2', '1+2)', '1+', '*3', '()', '1/0', '0/0']) {
    assert.equal(evaluate(malformed), null, `malformed or not finite: ${JSON.stringify(malformed)}`);
  }
  assert.equal(evaluate('('.repeat(40) + '1' + ')'.repeat(40)), null, 'nesting past the depth bound');
});

test('/clock Tokyo resolves to Asia/Tokyo and shows Tokyo time (APP-103)', async () => {
  const { zoneOf, zoneName } = await import('../src/canvas/widgets/state.ts');
  const { clockParts } = await import('../src/canvas/widgets/clock.ts');
  assert.equal(zoneOf('Tokyo'), 'Asia/Tokyo');
  assert.equal(zoneOf('tokyo'), 'Asia/Tokyo');
  assert.equal(zoneOf('Asia/Tokyo'), 'Asia/Tokyo');
  assert.equal(zoneOf('Atlantis'), null, 'an unknown place is not a zone');
  assert.equal(zoneName('Asia/Tokyo'), 'Tokyo');
  // 2026-11-12 00:30:05 UTC is 09:30:05 on Thursday 12 November in Tokyo (UTC+9, no DST).
  const parts = clockParts(new Date('2026-11-12T00:30:05Z'), 'Asia/Tokyo', false);
  assert.deepEqual([parts.hours, parts.minutes, parts.seconds, parts.period], ['09', '30', '05', '']);
  assert.equal(parts.date, 'THU, NOV 12');
  const twelve = clockParts(new Date('2026-11-12T05:00:00Z'), 'Asia/Tokyo', true);
  assert.deepEqual([twelve.hours, twelve.period], ['02', 'PM']);
});
