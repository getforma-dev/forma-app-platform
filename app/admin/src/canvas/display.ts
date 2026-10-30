// How the canvas shows a released result to a person (mirrors
// `query::canvas_display` in Rust, which renders the same thing on the
// server): numbers formatted from the column's declared value type when the
// result carries one (`column_types`: money with its currency and minor-unit
// exponent, decimal with its scale, integer, text, date, timestamp,
// boolean), compact with a unit where an integer column's name says bytes,
// readable UTC times, and humane labels. Money is never inferred from a
// name. No figure here is computed from the data: each is one released
// value, rounded for reading, with its exact form kept beside it.

export type ColumnType =
  | { type: 'integer' | 'text' | 'date' | 'timestamp' | 'boolean' }
  | { type: 'money'; currency: string; minor_unit_exponent: number }
  | { type: 'decimal'; scale: number };

const SIMPLE = ['integer', 'text', 'date', 'timestamp', 'boolean'] as const;

/** One declared column type, from a string or an object (`type` or `kind`); undefined otherwise. */
export function columnType(raw: unknown): ColumnType | undefined {
  if (typeof raw === 'string') return (SIMPLE as readonly string[]).includes(raw) ? { type: raw as (typeof SIMPLE)[number] } : undefined;
  if (typeof raw !== 'object' || raw === null) return undefined;
  const record = raw as Record<string, unknown>;
  const kind = record.type ?? record.kind;
  if (kind === 'money') {
    const exponent = Number(record.minor_unit_exponent);
    const currency = typeof record.currency === 'string' ? record.currency : '';
    return Number.isInteger(exponent) && exponent >= 0 && exponent <= 6 && /^[A-Z]{3}$/.test(currency)
      ? { type: 'money', currency, minor_unit_exponent: exponent }
      : undefined;
  }
  if (kind === 'decimal') {
    const scale = Number(record.scale);
    return Number.isInteger(scale) && scale >= 0 && scale <= 18 ? { type: 'decimal', scale } : undefined;
  }
  return typeof kind === 'string' ? columnType(kind) : undefined;
}

/** The declared types of `columns`, from an array aligned with them or a map by name. */
export function columnTypes(columns: string[], raw: unknown): (ColumnType | undefined)[] {
  if (Array.isArray(raw)) return columns.map((_, index) => columnType(raw[index]));
  if (typeof raw === 'object' && raw !== null) return columns.map((column) => columnType((raw as Record<string, unknown>)[column]));
  return columns.map(() => undefined);
}

/** `total_bytes` as `Total bytes`. */
export function label(id: string): string {
  const spaced = id.replace(/_/g, ' ');
  return spaced.charAt(0).toUpperCase() + spaced.slice(1);
}

/** An exact integer's sign and magnitude (grouped or not); null otherwise. */
export function integer(text: string): [boolean, bigint] | null {
  const negative = text.startsWith('-');
  const digits = (negative ? text.slice(1) : text).replace(/,/g, '');
  if (!digits || digits.length > 36 || !/^[0-9]+$/.test(digits)) return null;
  const n = BigInt(digits);
  return [negative && n > 0n, n];
}

export function group(n: bigint): string {
  return n.toString().replace(/\B(?=(\d{3})+(?!\d))/g, ',');
}

const TEN = 10n;
const pow = (base: bigint, exponent: number): bigint => base ** BigInt(exponent);

function decimal(q: bigint, decimals: number): string {
  if (decimals === 0) return q.toString();
  const scale = pow(TEN, decimals);
  const fraction = (q % scale).toString().padStart(decimals, '0').replace(/0+$/, '');
  return fraction ? `${q / scale}.${fraction}` : `${q / scale}`;
}

function three(n: bigint, den: bigint): [bigint, number] {
  const whole = n / den;
  const decimals = whole >= 100n ? 0 : whole >= 10n ? 1 : 2;
  const scaled = n * pow(TEN, decimals);
  return [(scaled * 2n + den) / (2n * den), decimals];
}

/** `n / start` in steps of 1000, three significant digits, and the step. */
function compact(n: bigint, start: bigint, suffixes: readonly string[]): [string, number] {
  let k = 0;
  while (k + 1 < suffixes.length && start * pow(1000n, k + 1) <= n) k += 1;
  let [q, d] = three(n, start * pow(1000n, k));
  if (q >= 1000n * pow(TEN, d) && k + 1 < suffixes.length) {
    k += 1;
    [q, d] = three(n, start * pow(1000n, k));
  }
  return [decimal(q, d), k];
}

/** `n` scaled by `10^scale`, exact and grouped: `262,620.04`. */
function scaled(n: bigint, scale: number): string {
  if (scale === 0) return group(n);
  const unit = pow(TEN, scale);
  return `${group(n / unit)}.${(n % unit).toString().padStart(scale, '0')}`;
}

const BYTES = ['B', 'KB', 'MB', 'GB', 'TB', 'PB', 'EB'] as const;
const SUFFIX = ['', 'K', 'M', 'B', 'T'] as const;
const SYMBOL: Record<string, string> = { USD: '$', EUR: '€', GBP: '£', JPY: '¥' };

/** What a person reads (`text`), the exact value (`exact`) and the raw released text (`raw`). */
export type Shown = { text: string; exact: string; raw: string };

/** An integer column's value, compact with bytes when its name says so (`canvas_display::amount`). */
export function amount(bytes: boolean, negative: boolean, n: bigint): [string, string] {
  const sign = negative ? '-' : '';
  if (bytes) {
    let short: string;
    if (n < 1000n) short = `${n} B`;
    else {
      const [text, k] = compact(n, 1n, BYTES);
      short = `${text} ${BYTES[k]}`;
    }
    return [`${sign}${short}`, `${sign}${group(n)} ${n === 1n ? 'byte' : 'bytes'}`];
  }
  let short: string;
  if (n < 10_000n) short = group(n);
  else {
    const [text, k] = compact(n, 1n, SUFFIX);
    short = `${text}${SUFFIX[k]}`;
  }
  return [`${sign}${short}`, `${sign}${group(n)}`];
}

/** Money in minor units of `currency` at `exponent`: `$263K` and `$262,620.04`. */
export function money(negative: boolean, minor: bigint, currency: string, exponent: number): [string, string] {
  const sign = negative ? '-' : '';
  const symbol = SYMBOL[currency];
  const unit = pow(TEN, exponent);
  const exactNumber = scaled(minor, exponent);
  const withCurrency = (number: string) => (symbol ? `${sign}${symbol}${number}` : `${sign}${number} ${currency}`);
  const exact = withCurrency(exactNumber);
  if (minor / unit < 10_000n) return [exact, exact];
  const [text, k] = compact(minor, unit, SUFFIX);
  return [withCurrency(`${text}${SUFFIX[k]}`), exact];
}

/** A decimal coefficient at `scale`: compact when large, exact beside it. */
export function decimalValue(negative: boolean, coefficient: bigint, scale: number): [string, string] {
  const sign = negative ? '-' : '';
  const exact = `${sign}${scaled(coefficient, scale)}`;
  if (coefficient / pow(TEN, scale) < 10_000n) return [exact, exact];
  const [text, k] = compact(coefficient, pow(TEN, scale), SUFFIX);
  return [`${sign}${text}${SUFFIX[k]}`, exact];
}

const MONTHS = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];
const STAMP = /^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2})(?::(\d{2})(?:\.\d+)?)?Z$/;
const two = (n: number) => String(n).padStart(2, '0');

export type Stamp = { year: number; month: number; day: number; hour: number; minute: number; second: number };

export function parseStamp(text: string): Stamp | null {
  const match = STAMP.exec(text);
  if (!match) return null;
  const [year, month, day, hour, minute] = match.slice(1, 6).map(Number) as [number, number, number, number, number];
  const second = match[6] === undefined ? 0 : Number(match[6]);
  if (month < 1 || month > 12 || day < 1 || day > 31 || hour > 23 || minute > 59 || second > 59) return null;
  return { year, month, day, hour, minute, second };
}
export const stampSeconds = (s: Stamp): number => Date.UTC(s.year, s.month - 1, s.day, s.hour, s.minute, s.second) / 1000;
export const stampDate = (s: Stamp): string => `${MONTHS[s.month - 1]} ${s.day}`;
export const stampClock = (s: Stamp): string =>
  s.second === 0 ? `${two(s.hour)}:${two(s.minute)}` : `${two(s.hour)}:${two(s.minute)}:${two(s.second)}`;
/** `Oct 22, 10:03`. */
export const stampHuman = (s: Stamp): string => `${stampDate(s)}, ${stampClock(s)}`;

/** A date or timestamp for reading; null when the text is neither. */
export function when(text: string): string | null {
  const stamp = parseStamp(text);
  if (stamp) return stampHuman(stamp);
  if (text.length === 10) {
    const date = parseStamp(`${text}T00:00Z`);
    if (date) return `${stampDate(date)}, ${date.year}`;
  }
  return null;
}

/** `15:04 UTC`, now: when this page received a result. */
export function receivedAt(now = new Date()): string {
  return `${two(now.getUTCHours())}:${two(now.getUTCMinutes())} UTC`;
}
