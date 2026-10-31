// APP-122: the data side of Ask and Shape, with no DOM (the tests run it as
// is). Shape's specs are closed data from the server (`POST /v1/shape`,
// judged there: `composer::shape`); they are judged again here before any
// use (`cardView`, `appTheme`), as a widget's state is (`widgets/state.ts`).
//
// A card's view is evaluated here over the card's own data, in this browser,
// session-only: a records or ledger card over its records listing (each row
// carries its record's receipt: id, version, commit), a view card over its
// released query result (the query's receipt stays on the card). Filters
// and highlights choose rows, a group and a measure compute numbers, and
// every computed number keeps the receipts of the records it came from: a
// view may filter or hide records, never drop receipts (ADR-011 I3). Money
// is summed exactly in minor units (BigInt), per currency, never across;
// decimals and integers exactly at their scale; an average is rounded
// half-even and says so. A capped listing makes every number partial, and
// the card says so. Applied views re-evaluate when the card's data changes
// (`shape.ts`). Nothing here writes.
import { columnTypes, decimalValue, money } from './display';
import { formatValue, type QueryResponse } from './render';
import type { RecordJson, RecordsJson } from './records';

export const PALETTE = ['default', 'green', 'crimson', 'amber', 'cyan', 'violet', 'rose', 'ice', 'slate'] as const;
const PRESENTATIONS = ['table', 'list', 'bar', 'line', 'number'];
const OPS = ['eq', 'ne', 'lt', 'lte', 'gt', 'gte', 'in'];
const MEASURES = ['count', 'sum', 'avg', 'min', 'max'];
const BUCKETS = ['day', 'week', 'month', 'year'];
const MODES = ['default', 'light', 'dark'];
const DENSITIES = ['default', 'compact', 'comfortable', 'spacious'];

export type Condition = { field: string; op: string; value: string | number | string[] };
export type Highlight = Condition & { color: string };
export type CardView = {
  presentation?: string; palette?: string; columns?: string[]; filters?: Condition[]; highlight?: Highlight[];
  group_by?: string; bucket?: string; measure?: { op: string; field?: string }; sort?: { field: string; direction: string };
  limit?: number; title?: string;
};
export type AppTheme = { mode?: string; accent?: string; density?: string };

const FIELD = /^[a-z][a-z0-9_]{0,63}$/;
const isObject = (value: unknown): value is Record<string, unknown> => typeof value === 'object' && value !== null && !Array.isArray(value);
const onlyKeys = (value: Record<string, unknown>, keys: string[]) => Object.keys(value).every((key) => keys.includes(key));

function condition(value: unknown, color: boolean): Highlight | Condition | null {
  if (!isObject(value) || !onlyKeys(value, color ? ['field', 'op', 'value', 'color'] : ['field', 'op', 'value'])) return null;
  const { field, op } = value;
  const raw = value.value;
  if (typeof field !== 'string' || !FIELD.test(field) || typeof op !== 'string' || !OPS.includes(op)) return null;
  const plain = (text: unknown) => typeof text === 'string' && text.length <= 80 && !/[<>{};]/.test(text);
  const ok = op === 'in'
    ? Array.isArray(raw) && raw.length > 0 && raw.length <= 12 && raw.every(plain)
    : plain(raw) || (typeof raw === 'number' && Number.isSafeInteger(raw));
  if (!ok) return null;
  const base = { field, op, value: raw as Condition['value'] };
  if (!color) return base;
  return typeof value.color === 'string' && (PALETTE as readonly string[]).includes(value.color) && value.color !== 'default'
    ? { ...base, color: value.color } : null;
}

/** A card view from the server, judged again; null refuses the whole spec. */
export function cardView(value: unknown): CardView | null {
  if (!isObject(value)) return null;
  const keys = ['presentation', 'palette', 'columns', 'filters', 'highlight', 'group_by', 'bucket', 'measure', 'sort', 'limit', 'title'];
  if (!onlyKeys(value, keys) || Object.keys(value).length === 0) return null;
  const view: CardView = {};
  const { presentation, palette, columns, filters, highlight, group_by: group, bucket, measure, sort, limit, title } = value;
  if (presentation !== undefined) {
    if (typeof presentation !== 'string' || !PRESENTATIONS.includes(presentation)) return null;
    view.presentation = presentation;
  }
  if (palette !== undefined) {
    if (typeof palette !== 'string' || !(PALETTE as readonly string[]).includes(palette)) return null;
    view.palette = palette;
  }
  if (columns !== undefined) {
    if (!Array.isArray(columns) || columns.length > 12 || !columns.every((c) => typeof c === 'string' && FIELD.test(c))) return null;
    view.columns = columns as string[];
  }
  for (const [key, list, color] of [['filters', filters, false], ['highlight', highlight, true]] as const) {
    if (list === undefined) continue;
    if (!Array.isArray(list) || list.length > 4) return null;
    const judged = list.map((item) => condition(item, color));
    if (judged.some((item) => item === null)) return null;
    if (key === 'filters') view.filters = judged as Condition[];
    else view.highlight = judged as Highlight[];
  }
  if (group !== undefined) {
    if (typeof group !== 'string' || !FIELD.test(group)) return null;
    view.group_by = group;
  }
  if (bucket !== undefined) {
    if (typeof bucket !== 'string' || !BUCKETS.includes(bucket) || !view.group_by) return null;
    view.bucket = bucket;
  }
  if (measure !== undefined) {
    if (!isObject(measure) || !onlyKeys(measure, ['op', 'field']) || typeof measure.op !== 'string' || !MEASURES.includes(measure.op)) return null;
    if (measure.field !== undefined && (typeof measure.field !== 'string' || !FIELD.test(measure.field))) return null;
    if (measure.op !== 'count' && measure.field === undefined) return null;
    view.measure = { op: measure.op, ...(measure.field ? { field: measure.field as string } : {}) };
  }
  if (sort !== undefined) {
    if (!isObject(sort) || !onlyKeys(sort, ['field', 'direction']) || typeof sort.field !== 'string' || !FIELD.test(sort.field)
      || (sort.direction !== 'asc' && sort.direction !== 'desc')) return null;
    view.sort = { field: sort.field, direction: sort.direction };
  }
  if (limit !== undefined) {
    if (typeof limit !== 'number' || !Number.isInteger(limit) || limit < 1 || limit > 200) return null;
    view.limit = limit;
  }
  if (title !== undefined) {
    if (typeof title !== 'string' || !title.trim() || title.length > 80 || /[<>{}]/.test(title)) return null;
    view.title = title;
  }
  return view;
}

/** An app theme from the server, judged again. */
export function appTheme(value: unknown): AppTheme | null {
  if (!isObject(value) || !onlyKeys(value, ['mode', 'accent', 'density']) || Object.keys(value).length === 0) return null;
  const theme: AppTheme = {};
  for (const [key, set] of [['mode', MODES], ['accent', PALETTE as readonly string[]], ['density', DENSITIES]] as const) {
    const member = value[key];
    if (member === undefined) continue;
    if (typeof member !== 'string' || !set.includes(member)) return null;
    theme[key] = member;
  }
  return theme;
}

// --- The card's data as one table ---

/**
 * A column. Numbers are kept exact: money as minor units (each row's own
 * currency and exponent, `Row.money`, else the column's), a decimal as text
 * at its scale (`123.45`; `scale` is the column's largest), an integer as
 * its digits. Comparisons and sums run in BigInt at a common scale.
 */
export type Column = { id: string; type: string; values?: string[]; currency?: string; exponent?: number; scale?: number };
/** A record's receipt: its identity at the version shown (ADR-011 I3). */
export type RecordReceipt = { record_id: string; collection_id: string; version: number; commit_sequence: number; committed_at: string };
export type Money = { currency: string; exponent: number };
export type Row = { raw: Record<string, unknown>; text: Record<string, string>; receipt?: RecordReceipt; money?: Record<string, Money> };
export type Table = {
  source: 'records' | 'query';
  columns: Column[];
  rows: Row[];
  /** A view card's query receipt (shown whole, unchanged, under any shape). */
  receipt: Record<string, unknown> | null;
  /** A records card's provenance: the work's publication and definition. */
  provenance: { app_id: string; work_id: string; publication_id: string; definition_digest: string; work_version: number } | null;
  /** The listing (or the result page) is capped and more exist: every number over it is partial. */
  truncated: boolean;
  /** A records card's collection (what `POST /v1/shape` checks its fields against). */
  collection?: string;
};

/** A coefficient at `scale` as decimal text: (`12345`, 2) → `123.45`. */
export function decimalText(coefficient: string, scale: number): string | null {
  const match = /^(-?)(\d{1,36})$/.exec(coefficient);
  if (!match || !Number.isInteger(scale) || scale < 0 || scale > 18) return null;
  if (scale === 0) return `${match[1]}${match[2]}`;
  const digits = (match[2] ?? '').padStart(scale + 1, '0');
  return `${match[1]}${digits.slice(0, -scale)}.${digits.slice(-scale)}`;
}

/** A stored value's comparable form: enum, text, date or boolean text; money minor units; a decimal at its scale. */
function rawOf(value: unknown): unknown {
  if (!isObject(value)) return value;
  if (value.type === 'null') return null;
  if (typeof value.minor_units === 'string') return value.minor_units;
  if (typeof value.coefficient === 'string') return decimalText(value.coefficient, Number(value.scale ?? 0)) ?? null;
  return value.value ?? null;
}

const fractionDigits = (text: string) => (/\.(\d+)$/.exec(text)?.[1]?.length ?? 0);

/** A records listing (one collection when named) as a table: its fields, and who filed each record. */
export function tableFromRecords(listing: RecordsJson, collection = '', labels: (record: RecordJson) => string = (r) => r.creator.label): Table {
  const chosen = listing.collections.find((c) => c.collection_id === collection) ?? listing.collections[0];
  const columns: Column[] = (chosen?.fields ?? []).filter((f) => FIELD.test(f.id)).map((f) => ({
    id: f.id, type: typeof f.type === 'string' ? f.type : 'unknown', ...(f.values ? { values: f.values } : {}),
  }));
  columns.push({ id: 'filed_by', type: 'text' });
  const rows: Row[] = listing.records
    .filter((record) => !chosen || record.collection_id === chosen.collection_id)
    .map((record) => {
      const raw: Record<string, unknown> = {};
      const text: Record<string, string> = {};
      const money: Record<string, Money> = {};
      for (const value of record.values) {
        raw[value.field_id] = rawOf(value.value);
        text[value.field_id] = value.display;
        const column = columns.find((c) => c.id === value.field_id);
        if (column?.type === 'money' && isObject(value.value)) {
          const currency = typeof value.value.currency === 'string' && /^[A-Z]{3}$/.test(value.value.currency) ? value.value.currency : 'USD';
          const exponent = Number.isInteger(value.value.minor_unit_exponent) ? Number(value.value.minor_unit_exponent) : 2;
          money[value.field_id] = { currency, exponent };
          column.currency ??= currency;
          column.exponent ??= exponent;
        }
        const shown = raw[value.field_id];
        if (column?.type === 'decimal' && typeof shown === 'string') column.scale = Math.max(column.scale ?? 0, fractionDigits(shown));
      }
      raw.filed_by = labels(record);
      text.filed_by = labels(record);
      return {
        raw, text, money,
        receipt: { record_id: record.record_id, collection_id: record.collection_id, version: record.version, commit_sequence: record.commit_sequence, committed_at: record.committed_at },
      };
    });
  for (const column of columns) {
    if (column.type === 'money') { column.currency ??= 'USD'; column.exponent ??= 2; }
    if (column.type === 'decimal') column.scale ??= 0;
  }
  return {
    source: 'records', columns, rows, receipt: null, truncated: listing.truncated === true,
    ...(chosen ? { collection: chosen.collection_id } : {}),
    provenance: { app_id: listing.app_id, work_id: listing.work_id, publication_id: listing.publication_id, definition_digest: listing.definition_digest, work_version: listing.work_version },
  };
}

/** A released query result as a table; its receipt rides along whole. */
export function tableFromQuery(result: QueryResponse): Table {
  const types = columnTypes(result.columns, result.column_types);
  const columns: Column[] = result.columns.map((id, index) => {
    const type = types[index];
    const safe = FIELD.test(id) ? id : `c${index}`;
    if (type?.type === 'money') return { id: safe, type: 'money', currency: type.currency, exponent: type.minor_unit_exponent };
    if (type?.type === 'decimal') return { id: safe, type: 'decimal', scale: type.scale };
    return { id: safe, type: type?.type ?? 'text' };
  });
  const rows = result.rows.map((cells) => {
    const raw: Record<string, unknown> = {};
    const text: Record<string, string> = {};
    cells.forEach((cell, index) => {
      const column = columns[index];
      if (!column) return;
      const value = rawOf(cell);
      // A view's decimal column holds its coefficient: read it at the column's scale.
      raw[column.id] = column.type === 'decimal' && typeof value === 'string' && !value.includes('.')
        ? decimalText(value, column.scale ?? 0) : value;
      text[column.id] = formatValue(cell, types[index], result.columns[index] ?? '').text;
    });
    return { raw, text };
  });
  return { source: 'query', columns, rows, receipt: result.receipt, provenance: null, truncated: result.truncated_page === true };
}

/** What `POST /v1/shape` is told about a card: its fields (ids, types, enum values). */
export function fieldsOf(table: Table): { id: string; type: string; values?: string[] }[] {
  const known = ['text', 'enum', 'money', 'integer', 'decimal', 'date', 'timestamp', 'boolean', 'reference'];
  return table.columns.slice(0, 40).map((c) => ({
    id: c.id, type: known.includes(c.type) ? c.type : 'unknown',
    ...(c.values?.length ? { values: c.values.filter((v) => /^[A-Za-z0-9_ .-]{1,64}$/.test(v)).slice(0, 32) } : {}),
  }));
}

// --- Evaluation ---

/** "200", "200.5" as an integer at `scale` ("200" at 2 → 20000n); null when it is not a plain decimal. Digits past the scale are cut. */
export function minorOf(text: string, scale: number): bigint | null {
  const match = /^(-?)(\d{1,36})(?:\.(\d{1,18}))?$/.exec(text.trim());
  if (!match) return null;
  const fraction = (match[3] ?? '').padEnd(scale, '0').slice(0, scale);
  const value = BigInt(`${match[2]}${fraction}`);
  return match[1] ? -value : value;
}

const NUMERIC = ['money', 'integer', 'decimal'];
const moneyOf = (row: Row, column: Column): Money => row.money?.[column.id] ?? { currency: column.currency ?? 'USD', exponent: column.exponent ?? 2 };
/** The scale a column's stored text is read at: money its exponent (the text is minor units, scale 0), a decimal its scale. */
const scaleOf = (column: Column): number => (column.type === 'decimal' ? column.scale ?? 0 : 0);

/** A numeric cell as an exact integer at `scale` (money: minor units, read at scale 0 and rescaled). */
function exactOf(row: Row, column: Column, scale: number): bigint | null {
  const raw = row.raw[column.id];
  if (raw === null || raw === undefined) return null;
  return minorOf(String(raw), scale);
}

function numberOf(row: Row, column: Column): number | null {
  const raw = row.raw[column.id];
  if (raw === null || raw === undefined) return null;
  const text = String(raw);
  if (!/^-?\d+(\.\d+)?$/.test(text)) return null;
  return column.type === 'money' ? Number(text) / 10 ** moneyOf(row, column).exponent : Number(text);
}

function matches(row: Row, column: Column | undefined, condition: Condition): boolean {
  if (!column) return false;
  const raw = row.raw[column.id];
  if (raw === null || raw === undefined) return false;
  const list = Array.isArray(condition.value) ? condition.value.map(String) : [String(condition.value)];
  if (condition.op === 'in') return list.includes(String(raw));
  let order: number;
  if (NUMERIC.includes(column.type)) {
    // Exact, in BigInt at a common scale: a money filter is in major units
    // ("200" is $200.00), a decimal's at its scale, an integer's as written.
    const target = list[0] ?? '';
    const base = column.type === 'money' ? moneyOf(row, column).exponent : scaleOf(column);
    const scale = Math.min(36, Math.max(base, fractionDigits(target)));
    const left = column.type === 'money'
      ? (/^-?\d+$/.test(String(raw)) ? BigInt(String(raw)) * 10n ** BigInt(scale - base) : null)
      : exactOf(row, column, scale);
    const right = minorOf(target, scale);
    if (left === null || right === null) return false;
    order = left < right ? -1 : left > right ? 1 : 0;
  } else {
    const [left, right] = [String(raw), list[0] ?? ''];
    order = left < right ? -1 : left > right ? 1 : 0;
  }
  switch (condition.op) {
    case 'eq': return order === 0;
    case 'ne': return order !== 0;
    case 'lt': return order < 0;
    case 'lte': return order <= 0;
    case 'gt': return order > 0;
    case 'gte': return order >= 0;
    default: return false;
  }
}

const MONTHS = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];

/** A date's bucket key and label: `2026-10` / "Oct 2026", a week's Monday, a day, a year. */
export function bucketOf(text: string, bucket: string): [string, string] | null {
  const match = /^(\d{4})-(\d{2})-(\d{2})/.exec(text);
  if (!match) return null;
  const [year, month, day] = [Number(match[1]), Number(match[2]), Number(match[3])];
  switch (bucket) {
    case 'year': return [String(year), String(year)];
    case 'month': return [`${match[1]}-${match[2]}`, `${MONTHS[month - 1]} ${year}`];
    case 'week': {
      const date = new Date(Date.UTC(year, month - 1, day));
      date.setUTCDate(date.getUTCDate() - ((date.getUTCDay() + 6) % 7));
      const key = date.toISOString().slice(0, 10);
      return [key, `Week of ${MONTHS[date.getUTCMonth()]} ${date.getUTCDate()}`];
    }
    default: return [`${match[1]}-${match[2]}-${match[3]}`, `${MONTHS[month - 1]} ${day}, ${year}`];
  }
}

/**
 * A computed number: `text` as shown, `value` for charts and sorting.
 * `rounded`: an average that did not divide exactly (rounded half-even to the
 * shown unit, and the text says so). `mixed`: money in more than one
 * currency, measured per currency and never added across (no chart).
 */
export type Measured = { text: string; value: number; minor?: bigint; rounded?: boolean; mixed?: boolean };
export type Group = { key: string; label: string; measured: Measured; count: number; receipts: RecordReceipt[] };
export type Shaped =
  | { kind: 'rows'; columns: Column[]; rows: { row: Row; color: string | null }[]; total: number; partial: boolean }
  | { kind: 'groups'; by: string; measure: string; groups: Group[]; total: number; matched: number; partial: boolean }
  | { kind: 'number'; measure: string; measured: Measured; count: number; receipts: RecordReceipt[]; total: number; partial: boolean };

/** `n / d` rounded half-even (banker's rounding), and whether it was exact. */
export function divideHalfEven(n: bigint, d: bigint): { quotient: bigint; exact: boolean } {
  const negative = (n < 0n) !== (d < 0n);
  const [a, b] = [n < 0n ? -n : n, d < 0n ? -d : d];
  let quotient = a / b;
  const remainder = a % b;
  if (remainder * 2n > b || (remainder * 2n === b && quotient % 2n === 1n)) quotient += 1n;
  return { quotient: negative ? -quotient : quotient, exact: remainder === 0n };
}

/** One op over exact integers; avg rounds half-even (to `extra` more digits). */
function reduce(values: bigint[], op: string, extra = 0): { result: bigint; rounded: boolean } | null {
  if (!values.length) return op === 'sum' ? { result: 0n, rounded: false } : null;
  if (op === 'sum') return { result: values.reduce((a, b) => a + b, 0n), rounded: false };
  if (op === 'min') return { result: values.reduce((a, b) => (b < a ? b : a)), rounded: false };
  if (op === 'max') return { result: values.reduce((a, b) => (b > a ? b : a)), rounded: false };
  const total = values.reduce((a, b) => a + b, 0n) * 10n ** BigInt(extra);
  const { quotient, exact } = divideHalfEven(total, BigInt(values.length));
  return { result: quotient, rounded: !exact };
}

const roundedText = (text: string, rounded: boolean) => (rounded ? `${text} (rounded)` : text);

function measure(rows: Row[], op: string, column: Column | undefined): Measured {
  if (op === 'count' || !column) return { text: String(rows.length), value: rows.length };
  if (column.type === 'money') {
    // Per currency (and exponent): never added across currencies.
    const byCurrency = new Map<string, { money: Money; values: bigint[] }>();
    for (const row of rows) {
      const raw = row.raw[column.id];
      if (typeof raw !== 'string' || !/^-?\d+$/.test(raw)) continue;
      const money = moneyOf(row, column);
      const key = `${money.currency}:${money.exponent}`;
      const entry = byCurrency.get(key) ?? { money, values: [] };
      entry.values.push(BigInt(raw));
      byCurrency.set(key, entry);
    }
    if (!byCurrency.size) byCurrency.set('none', { money: { currency: column.currency ?? 'USD', exponent: column.exponent ?? 2 }, values: [] });
    const parts = [...byCurrency.values()].map(({ money: unit, values }) => {
      const reduced = reduce(values, op);
      if (!reduced) return { text: '—', value: 0, minor: 0n, rounded: false };
      const { result, rounded } = reduced;
      return {
        text: roundedText(money(result < 0n, result < 0n ? -result : result, unit.currency, unit.exponent)[1], rounded),
        value: Number(result) / 10 ** unit.exponent, minor: result, rounded,
      };
    });
    if (parts.length === 1) return parts[0]!;
    return { text: `${parts.map((p) => p.text).join(' · ')} (per currency, not added)`, value: 0, mixed: true, rounded: parts.some((p) => p.rounded) };
  }
  if (column.type === 'integer' || column.type === 'decimal') {
    const scale = scaleOf(column);
    const values = rows.map((row) => exactOf(row, column, scale)).filter((v): v is bigint => v !== null);
    // An average keeps two more digits than its column, rounded half-even.
    const extra = op === 'avg' ? Math.min(2, 18 - scale) : 0;
    const reduced = reduce(values, op, extra);
    if (!reduced) return { text: '—', value: 0 };
    const at = scale + extra;
    const { result, rounded } = reduced;
    const text = decimalValue(result < 0n, result < 0n ? -result : result, at)[1];
    return { text: roundedText(text, rounded), value: Number(decimalText(result.toString(), at) ?? 0), rounded };
  }
  const values = rows.map((row) => numberOf(row, column)).filter((v): v is number => v !== null);
  let value = 0;
  if (op === 'sum') value = values.reduce((a, b) => a + b, 0);
  else if (op === 'avg') value = values.length ? values.reduce((a, b) => a + b, 0) / values.length : 0;
  else if (op === 'min') value = values.length ? Math.min(...values) : 0;
  else value = values.length ? Math.max(...values) : 0;
  return { text: Number.isInteger(value) ? String(value) : value.toFixed(2), value };
}

const receiptsOf = (rows: Row[]) => rows.flatMap((row) => (row.receipt ? [row.receipt] : []));

/** A measure as a person reads it: "Sum of amount", "Count". */
export function measureLabel(view: CardView): string {
  const op = view.measure?.op ?? 'count';
  const words: Record<string, string> = { count: 'Count', sum: 'Total', avg: 'Average', min: 'Smallest', max: 'Largest' };
  return op === 'count' || !view.measure?.field ? words[op] ?? 'Count' : `${words[op]} ${view.measure.field.replace(/_/g, ' ')}`;
}

/** The view over the table (module notes). */
export function evaluate(table: Table, view: CardView): Shaped {
  const column = (id: string | undefined) => table.columns.find((c) => c.id === id);
  const kept = table.rows.filter((row) => (view.filters ?? []).every((f) => matches(row, column(f.field), f)));
  const partial = table.truncated;
  // A number over a capped listing is never presented as the whole: its label says so.
  const label = partial ? `${measureLabel(view)} (newest ${table.rows.length} only)` : measureLabel(view);
  if (view.group_by) {
    const by = column(view.group_by);
    const groups = new Map<string, { label: string; rows: Row[] }>();
    for (const row of kept) {
      const raw = row.raw[view.group_by];
      const text = raw === null || raw === undefined ? '' : String(raw);
      const bucket = view.bucket ? bucketOf(text, view.bucket) : null;
      const [key, label] = bucket ?? [text || '—', row.text[view.group_by] || text || '—'];
      const entry = groups.get(key) ?? { label, rows: [] };
      entry.rows.push(row);
      groups.set(key, entry);
    }
    const op = view.measure?.op ?? 'count';
    let list: Group[] = [...groups.entries()].map(([key, entry]) => ({
      key, label: entry.label, measured: measure(entry.rows, op, column(view.measure?.field)), count: entry.rows.length, receipts: receiptsOf(entry.rows),
    }));
    const dated = by && (by.type === 'date' || by.type === 'timestamp' || view.bucket);
    if (view.sort?.field === 'value') list.sort((a, b) => (view.sort?.direction === 'asc' ? a.measured.value - b.measured.value : b.measured.value - a.measured.value));
    else list.sort((a, b) => (dated ? a.key.localeCompare(b.key) : b.measured.value - a.measured.value));
    if (view.limit) list = list.slice(0, view.limit);
    return { kind: 'groups', by: view.group_by, measure: label, groups: list, total: table.rows.length, matched: kept.length, partial };
  }
  if (view.presentation === 'number' || (view.measure && !view.group_by)) {
    const op = view.measure?.op ?? 'count';
    return { kind: 'number', measure: label, measured: measure(kept, op, column(view.measure?.field)), count: kept.length, receipts: receiptsOf(kept), total: table.rows.length, partial };
  }
  let rows = [...kept];
  if (view.sort) {
    const by = column(view.sort.field);
    const sign = view.sort.direction === 'asc' ? 1 : -1;
    if (by) {
      rows.sort((a, b) => {
        const [x, y] = [numberOf(a, by), numberOf(b, by)];
        if (x !== null && y !== null) return sign * (x - y);
        return sign * String(a.raw[by.id] ?? '').localeCompare(String(b.raw[by.id] ?? ''));
      });
    }
  }
  if (view.limit) rows = rows.slice(0, view.limit);
  const columns = view.columns?.length
    ? view.columns.map((id) => column(id)).filter((c): c is Column => Boolean(c))
    : table.columns;
  return {
    kind: 'rows',
    columns,
    rows: rows.map((row) => ({ row, color: (view.highlight ?? []).find((h) => matches(row, column(h.field), h))?.color ?? null })),
    total: table.rows.length,
    partial,
  };
}

/**
 * The line under a shaped card: "From 3 of 4 records", "2 of 4 records
 * shown". Over a capped listing it names it: "of the newest 200 records
 * loaded; older records exist and are not included".
 */
export function metaText(shaped: Shaped, table: Table): string {
  const noun = table.source === 'records' ? 'records' : 'rows';
  const of = shaped.partial
    ? `the newest ${shaped.total} ${noun} loaded; older ${noun} exist and are not included`
    : `${shaped.total} ${noun}`;
  if (shaped.kind === 'rows') return `${shaped.rows.length} of ${of} shown`;
  return `From ${shaped.kind === 'number' ? shaped.count : shaped.matched} of ${of}`;
}

const opWords: Record<string, string> = { eq: 'is', ne: 'is not', lt: '<', lte: '≤', gt: '>', gte: '≥', in: 'is' };
const valueWords = (condition: Condition) => (Array.isArray(condition.value) ? condition.value.join(' or ') : String(condition.value)).replace(/_/g, ' ');

/** The spec in words, as chips: what the preview and the *shaped* label show. Built from the judged spec, never model text. */
export function chips(view: CardView): string[] {
  const out: string[] = [];
  for (const f of view.filters ?? []) out.push(`Only ${f.field.replace(/_/g, ' ')} ${opWords[f.op]} ${valueWords(f)}`);
  for (const h of view.highlight ?? []) out.push(`${h.color} where ${h.field.replace(/_/g, ' ')} ${opWords[h.op]} ${valueWords(h)}`);
  if (view.group_by) out.push(`${measureLabel(view)} by ${view.bucket ?? view.group_by.replace(/_/g, ' ')}`);
  else if (view.measure) out.push(measureLabel(view));
  if (view.presentation) out.push(`${view.presentation}${['bar', 'line'].includes(view.presentation) ? ' chart' : ''}`);
  if (view.columns?.length) out.push(`Columns: ${view.columns.join(', ')}`);
  if (view.sort) out.push(`Sorted by ${view.sort.field} ${view.sort.direction}`);
  if (view.limit) out.push(`First ${view.limit}`);
  if (view.palette && view.palette !== 'default') out.push(`${view.palette} card`);
  if (view.title) out.push(`Titled “${view.title}”`);
  return out;
}

export function themeChips(theme: AppTheme): string[] {
  return [theme.mode && `${theme.mode} mode`, theme.accent && `${theme.accent} accent`, theme.density && `${theme.density} density`].filter((c): c is string => Boolean(c));
}

// --- The bar ---

export type BarIntent = { kind: 'ask'; question: string } | { kind: 'dot'; target: string; words: string } | null;

/**
 * What the bar's text asks for, before the slash lane: `?` first is Ask;
 * `/<app>.<card> words` is the card's Shape (the dot is the rule; a bare
 * `/<app>.<card>` places the card, as `/card` does). APP-121's full form
 * `/card:<app>.<card> words` is exactly the same Shape (the target is the
 * same short form); `/card:<x>` with no words stays the `/card:` verb's.
 */
export function barIntent(text: string): BarIntent {
  const trimmed = text.trim();
  if (trimmed.startsWith('?')) return { kind: 'ask', question: trimmed.slice(1).trim() };
  const dot = /^\/(card:)?([a-z0-9][a-z0-9-]{0,47}\.[a-z0-9][a-z0-9-]{0,47})(?:\s+([\s\S]*))?$/i.exec(trimmed);
  if (!dot) return null;
  const words = (dot[3] ?? '').trim();
  if (dot[1] && !words) return null;
  return { kind: 'dot', target: (dot[2] ?? '').toLowerCase(), words };
}

export type Space = 'app' | 'card' | 'widget' | 'folder';
/** An address split into its namespace (APP-121's full form) and name: `card:expenses.ledger` → card, `expenses.ledger`. */
export function splitAddress(address: string): { space: Space | null; name: string } {
  const text = address.trim().replace(/^\//, '').toLowerCase();
  const match = /^(app|card|widget|folder):(\S+)$/.exec(text);
  return match ? { space: match[1] as Space, name: match[2] ?? '' } : { space: null, name: text };
}

/** `/ask` and `/shape`'s first word and the rest: `expenses.ledger which are late?` → [`expenses.ledger`, `which are late?`]. */
export function firstWord(text: string): [string, string] {
  const match = /^\s*(\S+)\s*([\s\S]*)$/.exec(text);
  return match ? [match[1] ?? '', (match[2] ?? '').trim()] : ['', ''];
}
