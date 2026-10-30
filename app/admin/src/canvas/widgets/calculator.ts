// APP-103: the calculator widget. Expressions are evaluated by a small
// recursive-descent parser over a closed grammar, never `eval` or
// `Function`: numbers (decimal), + - * / ^, unary minus and parentheses.
// Any other character refuses the whole expression.
import { el } from '../render';
import { type WidgetCard } from './card';

type Token = { kind: 'num'; value: number } | { kind: 'op'; value: '+' | '-' | '*' | '/' | '^' | '(' | ')' };

const MAX_LENGTH = 200;
const MAX_DEPTH = 32;

/** The expression's tokens, or null when it holds anything outside the grammar. */
export function tokenize(text: string): Token[] | null {
  if (text.length > MAX_LENGTH) return null;
  const source = text.replace(/×/g, '*').replace(/÷/g, '/').replace(/−/g, '-');
  const tokens: Token[] = [];
  let index = 0;
  while (index < source.length) {
    const char = source[index] ?? '';
    if (char === ' ') {
      index += 1;
      continue;
    }
    if ('+-*/^()'.includes(char)) {
      tokens.push({ kind: 'op', value: char as '+' });
      index += 1;
      continue;
    }
    const number = /^(?:\d+(?:\.\d*)?|\.\d+)/.exec(source.slice(index));
    if (!number) return null;
    tokens.push({ kind: 'num', value: Number(number[0]) });
    index += number[0].length;
  }
  return tokens;
}

/** The value of `text`, or null (outside the grammar, malformed, or not finite). */
export function evaluate(text: string): number | null {
  const tokens = tokenize(text);
  if (!tokens || tokens.length === 0) return null;
  let position = 0;
  let depth = 0;
  const peek = () => tokens[position];
  const isOp = (value: string) => {
    const token = peek();
    return token?.kind === 'op' && token.value === value;
  };
  const enter = () => {
    depth += 1;
    if (depth > MAX_DEPTH) throw new Error('depth');
  };
  const expression = (): number => {
    enter();
    let value = term();
    while (isOp('+') || isOp('-')) {
      const op = (tokens[position++] as { value: string }).value;
      const right = term();
      value = op === '+' ? value + right : value - right;
    }
    depth -= 1;
    return value;
  };
  const term = (): number => {
    let value = unary();
    while (isOp('*') || isOp('/')) {
      const op = (tokens[position++] as { value: string }).value;
      const right = unary();
      value = op === '*' ? value * right : value / right;
    }
    return value;
  };
  const unary = (): number => {
    if (isOp('-') || isOp('+')) {
      const op = (tokens[position++] as { value: string }).value;
      enter();
      const value = unary();
      depth -= 1;
      return op === '-' ? -value : value;
    }
    return power();
  };
  const power = (): number => {
    const base = primary();
    if (isOp('^')) {
      position += 1;
      enter();
      const exponent = unary();
      depth -= 1;
      return base ** exponent;
    }
    return base;
  };
  const primary = (): number => {
    const token = peek();
    if (token?.kind === 'num') {
      position += 1;
      return token.value;
    }
    if (isOp('(')) {
      position += 1;
      const value = expression();
      if (!isOp(')')) throw new Error('paren');
      position += 1;
      return value;
    }
    throw new Error('primary');
  };
  try {
    const value = expression();
    if (position !== tokens.length || !Number.isFinite(value)) return null;
    return Object.is(value, -0) ? 0 : value;
  } catch {
    return null;
  }
}

/** A result for the display: at most 12 significant digits, no float noise. */
export function formatResult(value: number): string {
  if (Number.isInteger(value) && Math.abs(value) < 1e15) return String(value);
  const text = Number(value.toPrecision(12)).toString();
  return text.length > 16 ? value.toExponential(8).replace(/\.?0+e/, 'e') : text;
}

/** Whether `text` reads as an expression to compute (rather than words). */
export function looksLikeExpression(text: string): boolean {
  return /^[\d\s.+\-*/^()×÷−]+$/.test(text) && /\d/.test(text);
}

// Keys: [label, what it inserts or does, class]. The TI-84 skin restyles the
// same keys and shows its decorative function rows around them.
type Key = readonly [label: string, action: string, kind: string];
const KEYS: readonly Key[] = [
  ['AC', 'clear', 'fn'], ['(', '(', 'fn'], [')', ')', 'fn'], ['⌫', 'back', 'fn'], ['÷', '/', 'op'],
  ['x²', '^2', 'fn'], ['7', '7', 'num'], ['8', '8', 'num'], ['9', '9', 'num'], ['×', '*', 'op'],
  ['xʸ', '^', 'fn'], ['4', '4', 'num'], ['5', '5', 'num'], ['6', '6', 'num'], ['−', '-', 'op'],
  ['(−)', 'neg', 'fn'], ['1', '1', 'num'], ['2', '2', 'num'], ['3', '3', 'num'], ['+', '+', 'op'],
  ['ANS', 'ans', 'fn'], ['0', '0', 'num'], ['.', '.', 'num'], ['=', 'enter', 'enter'],
];

const TI_TOP = ['Y=', 'WINDOW', 'ZOOM', 'TRACE', 'GRAPH'];
const TI_MID = [['2ND', 'second'], ['MODE', ''], ['DEL', ''], ['ALPHA', 'alpha'], ['X,T,θ,n', ''], ['STAT', '']];
const TI_FN = ['MATH', 'APPS', 'PRGM', 'VARS', 'CLEAR'];
const RAIN = ['ｱ7ﾀ1ﾂ0ﾃ', '3ﾅﾆ8ﾇﾈ2', 'ﾊ5ﾋﾌ9ﾍﾎ', '0ﾏ4ﾐﾑ6ﾒ', 'ﾓ1ﾔ7ﾕﾖ3', 'ﾗ8ﾘﾙ2ﾚﾛ', '6ﾜ0ｦ5ﾝｱ', 'ｲ9ｳ3ｴｵ1'];

/** Builds the calculator body into `widget`. */
export function mountCalculator(widget: WidgetCard, initial = ''): void {
  const document_ = widget.card.ownerDocument;
  let expression = '';
  let answer = '0';
  let justEvaluated = false;
  const exprLine = el(document_, 'div', { class: 'calc-expr', 'aria-label': 'Expression' });
  const result = el(document_, 'output', { class: 'calc-result', 'aria-live': 'polite' }, '0');
  const rain = el(document_, 'div', { class: 'wg-rain', 'aria-hidden': 'true' },
    ...RAIN.map((column) => el(document_, 'span', {}, column.repeat(3))));
  const screen = el(document_, 'div', { class: 'calc-screen' }, rain, exprLine, result);
  const show = () => {
    exprLine.textContent = expression || ' ';
  };
  const compute = () => {
    if (!expression.trim()) return;
    const value = evaluate(expression);
    if (value === null) {
      result.textContent = 'ERR:SYNTAX';
      result.dataset.error = 'true';
      return;
    }
    answer = formatResult(value);
    result.textContent = answer;
    delete result.dataset.error;
    justEvaluated = true;
    widget.announce(`${expression} equals ${answer}`);
  };
  const press = (action: string) => {
    delete result.dataset.error;
    switch (action) {
      case 'clear':
        expression = '';
        result.textContent = '0';
        justEvaluated = false;
        break;
      case 'back':
        expression = expression.slice(0, -1);
        break;
      case 'enter':
        compute();
        break;
      case 'neg':
        expression += '-';
        break;
      case 'ans':
        expression += answer;
        break;
      default:
        if (justEvaluated && /^[\d.(]/.test(action)) expression = '';
        if (justEvaluated && /^[+\-*/^]/.test(action)) expression = answer;
        if (expression.length + action.length <= MAX_LENGTH) expression += action;
    }
    if (action !== 'enter') justEvaluated = false;
    show();
  };
  const keys = el(document_, 'div', { class: 'calc-keys', role: 'group', 'aria-label': 'Keypad' });
  for (const [label, action, kind] of KEYS) {
    const button = el(document_, 'button', { type: 'button', class: `calc-key calc-${kind}`, 'data-key': action, 'aria-label': label === '=' ? 'equals' : label }, label);
    if (label === '=') button.dataset.ti = 'ENTER';
    if (label === 'AC') button.dataset.ti = 'CLEAR';
    if (label === '⌫') button.dataset.ti = 'DEL';
    button.addEventListener('click', () => press(action));
    keys.append(button);
  }
  // The TI-84 skin's decorative rows (no function; hidden in other skins).
  const deco = (labels: readonly string[], kind: string) =>
    el(document_, 'div', { class: `ti-row ti-${kind}`, 'aria-hidden': 'true' },
      ...labels.map((label) => el(document_, 'span', { class: 'ti-key' }, label)));
  const tiTop = deco(TI_TOP, 'top');
  const tiMid = el(document_, 'div', { class: 'ti-row ti-mid', 'aria-hidden': 'true' },
    el(document_, 'div', { class: 'ti-mid-keys' },
      ...TI_MID.map(([label, kind]) => el(document_, 'span', { class: `ti-key${kind ? ` ti-${kind}` : ''}` }, label ?? ''))),
    el(document_, 'div', { class: 'ti-dpad' }, el(document_, 'i', {}), el(document_, 'i', {}), el(document_, 'i', {}), el(document_, 'i', {})));
  const tiFn = deco(TI_FN, 'fn');
  const brand = el(document_, 'div', { class: 'ti-brand', 'aria-hidden': 'true' },
    el(document_, 'span', { class: 'ti-maker' }, 'FORMA INSTRUMENTS'),
    el(document_, 'span', { class: 'ti-model' }, 'TI-84 Plus'));
  widget.body.append(el(document_, 'div', { class: 'calc' }, brand, screen, tiTop, tiMid, tiFn, keys));
  // Typing on the focused calculator: digits, operators, Enter, Backspace, Escape.
  widget.card.addEventListener('keydown', (event) => {
    const target = event.target as HTMLElement | null;
    if (target?.closest('input, textarea, select')) return;
    const key = event.key;
    let action: string | null = null;
    if (/^[\d.+\-*/^()]$/.test(key)) action = key;
    else if (key === 'Enter' || key === '=') action = 'enter';
    else if (key === 'Backspace') action = 'back';
    else if (key === 'Escape' || key === 'Delete') action = 'clear';
    if (action === null) return;
    event.preventDefault();
    event.stopPropagation();
    press(action);
  });
  show();
  if (initial && looksLikeExpression(initial)) {
    expression = initial.replace(/\s+/g, '');
    show();
    compute();
  }
}
