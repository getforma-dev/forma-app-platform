// Card DOM for the canvas island: the same structure the server renders in
// `canvas/app.tsx`, built with createElement and textContent only (the page
// runs under Trusted Types; nothing here assigns markup). A card shows a
// value only from a released query result that carries its receipt; every
// other state is built from a message alone (ADR-011 I3).
import { chartFigure, chartOf } from './chart';
import { buildRecordsCard, type RecordsCardJson } from './records';
import {
  amount, columnTypes, decimalValue, integer, label, money, receivedAt, when,
  type ColumnType, type Shown,
} from './display';

export type Pair = { id: string; label: string; value: string };
type Child = Node | string | null | undefined | false;

export function el<K extends keyof HTMLElementTagNameMap>(
  document_: Document,
  tag: K,
  attributes: Record<string, string> = {},
  ...children: Child[]
): HTMLElementTagNameMap[K] {
  const node = document_.createElement(tag);
  for (const [name, value] of Object.entries(attributes)) node.setAttribute(name, value);
  for (const child of children) {
    if (child === null || child === undefined || child === false) continue;
    node.append(typeof child === 'string' ? document_.createTextNode(child) : child);
  }
  return node;
}

/** Exact decimal strings gain digit grouping; anything else is unchanged (`page::display`). */
export function display(value: string): string {
  const negative = value.startsWith('-');
  const digits = negative ? value.slice(1) : value;
  if (!digits || !/^[0-9]+$/.test(digits)) return value;
  let grouped = '';
  for (let index = 0; index < digits.length; index += 1) {
    if (index > 0 && (digits.length - index) % 3 === 0) grouped += ',';
    grouped += digits[index];
  }
  return (negative ? '-' : '') + grouped;
}

/** A cell's text (`page::cell_text`): an unknown cell is "unknown", never zero. */
export function cellText(value: unknown, integer: boolean): string {
  if (typeof value === 'string') return integer ? display(value) : value;
  if (typeof value === 'boolean') return String(value);
  return 'unknown';
}

function text(document_: unknown, key: string): string {
  if (typeof document_ !== 'object' || document_ === null) return '';
  const value = (document_ as Record<string, unknown>)[key];
  if (typeof value === 'string') return value;
  if (value === null || value === undefined) return '';
  return JSON.stringify(value);
}

function list(document_: Record<string, unknown>, key: string): Record<string, unknown>[] {
  const value = document_[key];
  return Array.isArray(value) ? value.filter((v) => typeof v === 'object' && v !== null) : [];
}

/** The receipt inspector's rows, in the server's fixed order (`page::receipt_rows`). */
export function receiptRows(receipt: Record<string, unknown>): Pair[] {
  const pair = (id: string, label: string, value: string): Pair => ({ id, label, value });
  const rows: Pair[] = [
    pair('digest', 'Receipt digest', text(receipt, 'digest')),
    pair('result', 'Result digest', text(receipt, 'result_digest')),
    pair('mode', 'Mode', text(receipt, 'mode')),
    pair('app', 'App', text(receipt, 'app_id')),
    pair('work', 'Saved work', text(receipt, 'work_id')),
    pair('publication', 'Publication', text(receipt, 'publication_id')),
    pair('definition', 'Definition digest', text(receipt, 'definition_digest')),
    pair('view', 'View', text(receipt, 'view_id')),
    pair('operator', 'Operator version', text(receipt, 'operator_version')),
  ];
  list(receipt, 'source_revisions').forEach((source, index) => {
    rows.push(pair(
      `source-${index}`,
      `Source revision (${text(source, 'input_id')})`,
      `source ${text(source, 'source_id')} revision ${text(source, 'source_revision_id')} digest ${text(source, 'source_digest')}`,
    ));
  });
  list(receipt, 'parameters').forEach((parameter, index) => {
    rows.push(pair(`parameter-${index}`, `Parameter ${text(parameter, 'field_id')}`, text(parameter.value, 'value')));
  });
  const provenance = receipt.provenance;
  rows.push(
    pair('snapshot', 'Record snapshot sequence', text(receipt, 'record_snapshot_sequence')),
    pair('principal', 'Principal', text(receipt, 'principal_id')),
    pair('membership', 'Membership epoch', text(receipt, 'membership_epoch')),
    pair('policy', 'Policy epoch', text(receipt, 'policy_epoch')),
    pair('filter', 'Filter epoch', text(receipt, 'filter_epoch')),
    pair('query', 'Query digest', text(provenance, 'query_digest')),
    pair('predicate', 'Effective query digest', text(provenance, 'predicate_digest')),
    pair('scope', 'Total scope', text(receipt, 'total_scope')),
    pair('partial', 'Partial', text(receipt, 'partial')),
    pair('truncated', 'Truncated page', text(receipt, 'truncated_page')),
  );
  return rows;
}

function pairs(document_: Document, rows: Pair[]): HTMLDListElement {
  const dl = el(document_, 'dl', {});
  for (const row of rows) {
    dl.append(el(document_, 'div', {}, el(document_, 'dt', {}, row.label), el(document_, 'dd', {}, el(document_, 'code', {}, row.value))));
  }
  return dl;
}

function svgNode(document_: Document, tag: string, attributes: Record<string, string>): SVGElement {
  const node = document_.createElementNS('http://www.w3.org/2000/svg', tag) as SVGElement;
  for (const [name, value] of Object.entries(attributes)) node.setAttribute(name, value);
  return node;
}

/** The verified seal (as `canvas/app.tsx` draws it). */
function seal(document_: Document): SVGElement {
  const icon = svgNode(document_, 'svg', { class: 'cv-seal', viewBox: '0 0 16 16', width: '16', height: '16', 'aria-hidden': 'true' });
  icon.append(
    svgNode(document_, 'path', { d: 'M8 1.2l5.4 2v4.1c0 3.3-2.3 6.2-5.4 7.3C4.9 13.5 2.6 10.6 2.6 7.3V3.2z' }),
    svgNode(document_, 'path', { class: 'cv-seal-check', d: 'M5.4 8.1l1.8 1.8 3.5-3.6' }),
  );
  return icon;
}

/**
 * The receipt, always on the card (ADR-011): one compact line, "Verified",
 * the digest's first eight hex digits, the source count and when this page
 * received it; a click opens every receipt field in full, inline, and the
 * receipt document itself.
 */
export function receiptSection(document_: Document, receipt: Record<string, unknown>): HTMLElement {
  const rows = receiptRows(receipt);
  const digest = text(receipt, 'digest');
  const sources = rows.filter((row) => row.id.startsWith('source-')).length;
  const time = receivedAt();
  const summary = el(document_, 'summary', { title: digest },
    seal(document_),
    el(document_, 'span', { class: 'cv-verified' }, 'Verified'),
    el(document_, 'code', { class: 'cv-receipt-id' }, digest.replace(/^sha256:/, '').slice(0, 8)),
    el(document_, 'span', { class: 'cv-receipt-meta' }, `${sources} ${sources === 1 ? 'source' : 'sources'}`),
    el(document_, 'span', { class: 'cv-receipt-meta' }, time),
    el(document_, 'span', { class: 'cv-receipt-toggle', 'aria-hidden': 'true' }));
  const raw = el(document_, 'details', { class: 'cv-receipt-raw' },
    el(document_, 'summary', {}, 'Receipt document (JSON)'),
    el(document_, 'pre', {}, JSON.stringify(receipt, null, 2)));
  return el(document_, 'details', { class: 'cv-receipt', 'aria-label': 'Receipt' },
    summary,
    el(document_, 'div', { class: 'cv-receipt-body' },
      pairs(document_, rows),
      raw,
      el(document_, 'p', { class: 'cv-receipt-note' }, `Received ${time}, as this page saw it; the time is not part of the receipt.`)));
}

function body(card: HTMLElement): HTMLElement {
  let node = card.querySelector<HTMLElement>('[data-body]');
  if (!node) {
    node = el(card.ownerDocument, 'div', { class: 'cv-card-body', 'data-body': '' });
    card.append(node);
  }
  return node;
}

function setState(card: HTMLElement, state: string, badge: string): void {
  card.dataset.state = state;
  const chip = card.querySelector<HTMLElement>('.badge');
  if (chip) chip.textContent = badge;
}

/** A card with no value: idle, running (`loading`), busy, failed or denied. */
export function renderNoValue(card: HTMLElement, state: string, badge: string, message: string): void {
  const document_ = card.ownerDocument;
  setState(card, state, badge);
  body(card).replaceChildren(el(document_, 'p', { class: 'cv-message', role: state === 'failed' || state === 'denied' ? 'alert' : 'status' }, message));
}

export type QueryResponse = {
  view_id: string;
  columns: string[];
  rows: unknown[][];
  truncated_page: boolean;
  receipt: Record<string, unknown>;
  /** Per-column value types (pivot/fix-093), aligned with `columns` or by name. */
  column_types?: unknown;
};

/** A body the JSON query route answered with 200, or null if it is not one (then no number). */
export function released(value: unknown): QueryResponse | null {
  if (typeof value !== 'object' || value === null) return null;
  const candidate = value as Partial<QueryResponse>;
  const receipt = candidate.receipt;
  if (!Array.isArray(candidate.columns) || !Array.isArray(candidate.rows)) return null;
  if (!candidate.columns.every((c) => typeof c === 'string')) return null;
  if (!candidate.rows.every((row) => Array.isArray(row))) return null;
  if (typeof receipt !== 'object' || receipt === null) return null;
  if (typeof receipt.digest !== 'string' || !receipt.digest.startsWith('sha256:')) return null;
  if (typeof receipt.result_digest !== 'string') return null;
  return candidate as QueryResponse;
}

/**
 * The one formatter of a released cell (pivot/fix-093's seam): from the
 * column's declared type when the result carries one; money with its
 * currency at its minor-unit exponent, decimal at its scale, integers
 * grouped (compact with bytes when the column's name says bytes), dates and
 * timestamps readable, text never grouped. Without a type, a cell shows as
 * released (a date or timestamp readable). An unknown cell is "unknown",
 * never zero. `exact` is the lossless value; `raw` the released text.
 */
export function formatValue(value: unknown, type: ColumnType | undefined, column = ''): Shown {
  if (value === null || value === undefined) return { text: 'unknown', exact: 'unknown', raw: 'unknown' };
  if (typeof value === 'boolean') return { text: String(value), exact: String(value), raw: String(value) };
  let raw = typeof value === 'string' ? value : '';
  if (typeof value === 'object') {
    const record = value as Record<string, unknown>;
    const inner = record.minor_units ?? record.coefficient ?? record.value;
    if (typeof inner === 'string') raw = inner;
  }
  if (typeof value === 'number' && Number.isSafeInteger(value)) raw = String(value);
  if (!raw) return { text: 'unknown', exact: 'unknown', raw: 'unknown' };
  const parsed = integer(raw);
  const shown = (pair: [string, string]): Shown => ({ text: pair[0], exact: pair[1], raw });
  switch (type?.type) {
    case 'money':
      return parsed ? { ...shown(money(parsed[0], parsed[1], type.currency, type.minor_unit_exponent)), raw: `${raw} minor units (${type.currency})` } : shown([raw, raw]);
    case 'decimal':
      return parsed ? shown(decimalValue(parsed[0], parsed[1], type.scale)) : shown([raw, raw]);
    case 'integer':
      return parsed ? shown(amount(column.toLowerCase().includes('bytes'), parsed[0], parsed[1])) : shown([raw, raw]);
    case 'text':
    case 'boolean':
      return shown([raw, raw]);
    default: {
      const readable = when(raw);
      return shown([readable ?? raw, raw]);
    }
  }
}

/** A cell's title: the exact value, and the raw text when it differs. */
function titleOf(shown: Shown): string {
  return shown.raw === shown.exact || shown.raw === shown.text ? shown.exact : `${shown.exact} · ${shown.raw}`;
}

const NUMERIC = ['integer', 'money', 'decimal'];

function table(document_: Document, columns: string[], rows: unknown[][], types: (ColumnType | undefined)[]): HTMLElement {
  const head = el(document_, 'tr', {});
  for (const column of columns) head.append(el(document_, 'th', { scope: 'col' }, label(column)));
  const body_ = el(document_, 'tbody', {});
  for (const row of rows) {
    const tr = el(document_, 'tr', {});
    row.forEach((value, index) => {
      const type = types[index];
      const shown = formatValue(value, type, columns[index] ?? '');
      tr.append(el(document_, 'td', { class: NUMERIC.includes(type?.type ?? '') ? 'value num' : 'value', title: titleOf(shown) }, shown.text));
    });
    body_.append(tr);
  }
  return el(document_, 'div', { class: 'cv-table' }, el(document_, 'table', {}, el(document_, 'thead', {}, head), body_));
}

/** A released result with its receipt: `ready` or `empty` (`page::card`). */
export function renderReleased(card: HTMLElement, result: QueryResponse): void {
  const document_ = card.ownerDocument;
  const integers = new Set((card.dataset.integers ?? '').split(',').filter(Boolean));
  const kind = card.dataset.kind ?? '';
  const empty = result.rows.every((row) => row.every((cell_) => cell_ === null));
  // Declared types when the result carries them; otherwise the view's
  // exact-integer columns are integers and every other column is as released.
  const declared = columnTypes(result.columns, result.column_types);
  const types = result.columns.map((column, index): ColumnType | undefined =>
    declared[index] ?? (integers.has(column) ? { type: 'integer' } : undefined));
  const children: Node[] = [];
  if (empty) {
    setState(card, 'empty', 'empty');
    children.push(el(document_, 'p', { class: 'cv-message', role: 'status' }, 'No rows matched this window and filter.'));
  } else {
    setState(card, 'ready', 'ready');
    children.push(el(document_, 'p', { class: 'cv-message', role: 'status' }, result.truncated_page ? 'Showing the first page of rows.' : ''));
    const format = (value: unknown, index: number) => formatValue(value, types[index], result.columns[index] ?? '');
    if (kind === 'summary' && result.rows.length === 1) {
      const shown = result.columns.map((column, index) => [column, format(result.rows[0]?.[index], index)] as const);
      const [first, ...rest] = shown;
      if (first) {
        const insight = el(document_, 'div', { class: 'cv-insight' },
          el(document_, 'p', { class: 'cv-insight-label' }, label(first[0])),
          el(document_, 'p', { class: 'cv-insight-value value', title: titleOf(first[1]) }, first[1].text),
          el(document_, 'p', { class: 'cv-insight-exact' }, first[1].exact));
        if (rest.length) {
          const stats = el(document_, 'dl', { class: 'cv-stats' });
          for (const [column, value] of rest) {
            stats.append(el(document_, 'div', {}, el(document_, 'dt', {}, label(column)), el(document_, 'dd', { class: 'value', title: titleOf(value) }, value.text)));
          }
          insight.append(stats);
        }
        children.push(insight);
      }
    } else {
      const texts = result.rows.map((row) => row.map((value) => (typeof value === 'string' ? value : value === null || value === undefined ? 'unknown' : String(value))));
      const chart = chartOf(result.columns, texts, types, (text_, index) => format(text_, index));
      const rows = table(document_, result.columns, result.rows, types);
      if (chart) {
        children.push(chartFigure(document_, chart));
        const count = result.rows.length;
        children.push(el(document_, 'details', { class: 'cv-rows' }, el(document_, 'summary', {}, `${count} ${count === 1 ? 'row' : 'rows'}`), rows));
      } else {
        children.push(rows);
      }
    }
  }
  children.push(receiptSection(document_, result.receipt));
  body(card).replaceChildren(...children);
}

// --- Groups placed by the island (GET /v1/canvas/works/{app}/{work}) ---

export type CardJson = {
  id: string; view: string; title: string; kind: string; state: string; badge: string;
  message: string; query: string; integer_columns: string;
};
export type FieldJson = {
  id: string; kind: string; label: string; input: string; value: string; spec: string;
  spec_input: string; writable: boolean; read_only: boolean; has_text: boolean;
  has_choices: boolean; choices: { id: string; value: string }[];
};
export type FormJson = {
  id: string; title: string; command: string; action: string; csrf: string;
  idempotency_key: string; collection: string; publication: string; schema: string;
  schema_version: string; record: string; record_version: string; work_version: string;
  fields: FieldJson[]; submit?: string;
};
export type OriginJson = { id: string; kind: string; text: string };
export type GroupJson = {
  id: string; app: string; work: string; app_card: string; title: string; label: string; origins?: OriginJson[];
  href: string; csrf: string; window_start: string; window_end: string; destination: string;
  cards: CardJson[]; forms: FormJson[]; records?: RecordsCardJson[];
};

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;
export function isUuid(value: unknown): value is string {
  return typeof value === 'string' && UUID.test(value);
}

export function groupJson(value: unknown): GroupJson | null {
  if (typeof value !== 'object' || value === null) return null;
  const group = value as GroupJson;
  if (!isUuid(group.app) || !isUuid(group.work) || !Array.isArray(group.cards) || !Array.isArray(group.forms)) return null;
  return group;
}

function input(document_: Document, name: string, value: string, type = 'hidden'): HTMLInputElement {
  return el(document_, 'input', { type, name, value });
}

export function buildForm(document_: Document, form: FormJson): HTMLFormElement {
  const node = el(document_, 'form', { method: 'post', action: form.action, class: 'cv-command', 'data-command': form.command },
    input(document_, 'csrf', form.csrf),
    input(document_, 'idempotency_key', form.idempotency_key),
    input(document_, 'collection_id', form.collection),
    input(document_, 'publication_id', form.publication),
    input(document_, 'schema_id', form.schema),
    input(document_, 'schema_version_expected', form.schema_version),
    input(document_, 'record_id', form.record),
    input(document_, 'expected_record_version', form.record_version),
    input(document_, 'expected_work_version', form.work_version));
  for (const field of form.fields) {
    const row = el(document_, 'div', { class: 'cv-field', 'data-field': field.id }, el(document_, 'span', { class: 'cv-field-label' }, field.label));
    if (field.has_choices) {
      const select = el(document_, 'select', { name: field.input, 'aria-label': field.label });
      for (const choice of field.choices) select.append(el(document_, 'option', { value: choice.value }, choice.value));
      row.append(select);
    }
    if (field.has_text) {
      const text_ = input(document_, field.input, field.value, 'text');
      text_.setAttribute('aria-label', field.label);
      row.append(text_);
    }
    if (field.writable) row.append(input(document_, field.spec_input, field.spec));
    if (field.read_only) row.append(el(document_, 'span', { class: 'cv-muted' }, 'not changed by this command'));
    node.append(row);
  }
  node.append(el(document_, 'button', { type: 'submit' }, form.submit || label(form.command)));
  return node;
}

export function buildFormCard(document_: Document, form: FormJson): HTMLElement {
  return el(document_, 'article', { class: 'cv-card cv-form', 'data-card': form.id, 'data-kind': 'form', 'aria-label': form.title },
    el(document_, 'header', { class: 'cv-card-head' }, el(document_, 'h3', {}, form.title), el(document_, 'span', { class: 'badge' }, 'record')),
    buildForm(document_, form),
    el(document_, 'p', { class: 'cv-outcome', role: 'status', 'data-outcome': '' }));
}

/** An origin label's style: library, adapted, generated or seeded. */
export function originKind(text_: string): string {
  const lower = text_.toLowerCase();
  if (lower.includes('generat')) return 'generated';
  if (lower.includes('adapt')) return 'adapted';
  if (lower.includes('seed')) return 'seeded';
  return 'library';
}

/** The same group the server renders, idle, from its JSON. */
export function buildGroup(document_: Document, group: GroupJson, labels: string[]): HTMLElement {
  const window_ = el(document_, 'form', { method: 'post', action: group.href, class: 'cv-window', 'data-window': '' },
    input(document_, 'csrf', group.csrf),
    el(document_, 'label', {}, el(document_, 'span', {}, 'From (UTC)'), el(document_, 'input', { name: 'window_start', type: 'datetime-local', step: '1', value: group.window_start })),
    el(document_, 'label', {}, el(document_, 'span', {}, 'Until (UTC, excluded)'), el(document_, 'input', { name: 'window_end', type: 'datetime-local', step: '1', value: group.window_end })),
    el(document_, 'label', {}, el(document_, 'span', {}, 'Destination (optional)'), el(document_, 'input', { name: 'destination', type: 'text', value: group.destination })),
    el(document_, 'div', { class: 'cv-window-actions' },
      el(document_, 'button', { type: 'submit' }, 'Run views'),
      el(document_, 'a', { href: group.href }, 'Open work page')));
  // The composer's honest assembly labels when it placed this app; the
  // server's origin chips otherwise.
  const origins = labels.length
    ? labels.map((text_) => ({ kind: originKind(text_), text: text_ }))
    : (group.origins ?? []).filter((origin) => typeof origin?.text === 'string');
  const chips = el(document_, 'p', { class: 'cv-origins', 'aria-label': 'How this app was assembled', title: group.label },
    ...origins.map((origin) => el(document_, 'span', { class: 'cv-origin', 'data-origin': originKind(origin.kind === 'seeded' ? 'seeded' : origin.text) }, origin.text)));
  const app = el(document_, 'article', { class: 'cv-card cv-app', 'data-card': group.app_card, 'data-kind': 'app', 'aria-label': group.title },
    el(document_, 'h2', {}, group.title),
    chips,
    el(document_, 'p', { class: 'sr-only' }, group.label),
    window_);
  const section = el(document_, 'section', { class: 'cv-group', id: group.id, 'data-app': group.app, 'data-work': group.work, 'aria-label': group.title }, app);
  for (const card of group.cards) {
    section.append(el(document_, 'article', {
      class: 'cv-card cv-view', id: card.id, 'data-card': card.id, 'data-view': card.view, 'data-kind': card.kind,
      'data-state': card.state, 'data-query': card.query, 'data-integers': card.integer_columns, 'aria-label': card.title,
    },
    el(document_, 'header', { class: 'cv-card-head' }, el(document_, 'h3', {}, card.title), el(document_, 'span', { class: 'badge' }, card.badge)),
    el(document_, 'div', { class: 'cv-card-body', 'data-body': '' }, el(document_, 'p', { class: 'cv-message', role: 'status' }, card.message))));
  }
  for (const form of group.forms) section.append(buildFormCard(document_, form));
  for (const card of group.records ?? []) section.append(buildRecordsCard(document_, card));
  return section;
}
