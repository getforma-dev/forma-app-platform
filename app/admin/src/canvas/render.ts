// Card DOM for the canvas island: the same structure the server renders in
// `canvas/app.tsx`, built with createElement and textContent only (the page
// runs under Trusted Types; nothing here assigns markup). A card shows a
// value only from a released query result that carries its receipt; every
// other state is built from a message alone (ADR-011 I3).

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

/**
 * A key row's value as the card shows it (`query::canvas::abbreviate`): each
 * `sha256:` digest and UUID shortened to its ends; the inspector keeps every
 * full value, and the receipt's own digest is shown in full.
 */
export function abbreviate(value: string): string {
  return value
    .split(' ')
    .map((token) => {
      if (/^sha256:[0-9a-f]{64}$/.test(token)) return `sha256:${token.slice(7, 19)}…${token.slice(-6)}`;
      if (/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(token)) return `${token.slice(0, 8)}…${token.slice(-4)}`;
      return token;
    })
    .join(' ');
}

/** The identities a card always shows (`query::canvas::key_row`). */
export function isKeyRow(id: string): boolean {
  return ['result', 'mode', 'principal', 'membership', 'policy'].includes(id) ||
    id.startsWith('source-') || id.startsWith('parameter-');
}

function pairs(document_: Document, rows: Pair[], className?: string, short = false): HTMLDListElement {
  const dl = el(document_, 'dl', className ? { class: className } : {});
  for (const row of rows) {
    const shown = short ? abbreviate(row.value) : row.value;
    dl.append(el(document_, 'div', {}, el(document_, 'dt', {}, row.label), el(document_, 'dd', {}, el(document_, 'code', {}, shown))));
  }
  return dl;
}

export function receiptSection(document_: Document, receipt: Record<string, unknown>): HTMLElement {
  const rows = receiptRows(receipt);
  const details = el(document_, 'details', {}, el(document_, 'summary', {}, 'All receipt fields'), pairs(document_, rows),
    el(document_, 'pre', {}, JSON.stringify(receipt, null, 2)));
  return el(document_, 'section', { class: 'cv-receipt', 'aria-label': 'Receipt' },
    el(document_, 'p', { class: 'cv-receipt-head' }, el(document_, 'span', {}, 'Receipt'), ' ', el(document_, 'code', {}, text(receipt, 'digest'))),
    pairs(document_, rows.filter((row) => isKeyRow(row.id)), 'cv-receipt-key', true),
    details);
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

/** A released result with its receipt: `ready` or `empty` (`page::card`). */
export function renderReleased(card: HTMLElement, result: QueryResponse): void {
  const document_ = card.ownerDocument;
  const integers = new Set((card.dataset.integers ?? '').split(',').filter(Boolean));
  const kind = card.dataset.kind ?? '';
  const empty = result.rows.every((row) => row.every((cell) => cell === null));
  const children: Node[] = [];
  if (empty) {
    setState(card, 'empty', 'empty');
    children.push(el(document_, 'p', { class: 'cv-message', role: 'status' }, 'No rows matched this window and filter.'));
  } else {
    setState(card, 'ready', 'ready');
    children.push(el(document_, 'p', { class: 'cv-message', role: 'status' }, result.truncated_page ? 'Showing the first page of rows.' : ''));
    const integer = (index: number) => integers.has(result.columns[index] ?? '');
    if (kind === 'summary' && result.rows.length === 1) {
      const dl = el(document_, 'dl', { class: 'cv-summary' });
      result.columns.forEach((column, index) => {
        dl.append(el(document_, 'div', {}, el(document_, 'dt', {}, column), el(document_, 'dd', { class: 'value' }, cellText(result.rows[0]?.[index], integer(index)))));
      });
      children.push(dl);
    } else {
      // One integer column gets a proportional bar drawn from the released
      // values themselves (no other number is introduced).
      const integerIndexes = result.columns.map((_, index) => index).filter(integer);
      const barColumn = integerIndexes.length === 1 ? integerIndexes[0] : -1;
      const max = barColumn < 0 ? 0 : Math.max(0, ...result.rows.map((row) => Number(row[barColumn]) || 0));
      const head = el(document_, 'tr', {});
      for (const column of result.columns) head.append(el(document_, 'th', { scope: 'col' }, column));
      const rows = el(document_, 'tbody', {});
      for (const row of result.rows) {
        const tr = el(document_, 'tr', {});
        row.forEach((cell, index) => {
          const td = el(document_, 'td', { class: 'value' }, cellText(cell, integer(index)));
          if (index === barColumn && max > 0 && typeof cell === 'string') {
            const share = Math.max(0, Math.min(1, Number(cell) / max));
            td.classList.add('has-bar');
            td.style.setProperty('--share', share.toFixed(4));
          }
          tr.append(td);
        });
        rows.append(tr);
      }
      children.push(el(document_, 'div', { class: 'cv-table' }, el(document_, 'table', {}, el(document_, 'thead', {}, head), rows)));
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
  fields: FieldJson[];
};
export type GroupJson = {
  id: string; app: string; work: string; app_card: string; title: string; label: string;
  href: string; csrf: string; window_start: string; window_end: string; destination: string;
  cards: CardJson[]; forms: FormJson[];
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
  node.append(el(document_, 'button', { type: 'submit' }, `Submit ${form.command}`));
  return node;
}

export function buildFormCard(document_: Document, form: FormJson): HTMLElement {
  return el(document_, 'article', { class: 'cv-card cv-form', 'data-card': form.id, 'data-kind': 'form', 'aria-label': form.title },
    el(document_, 'header', { class: 'cv-card-head' }, el(document_, 'h3', {}, form.title), el(document_, 'span', { class: 'badge' }, 'command')),
    buildForm(document_, form),
    el(document_, 'p', { class: 'cv-outcome', role: 'status', 'data-outcome': '' }));
}

/** The same group the server renders, idle, from its JSON. */
export function buildGroup(document_: Document, group: GroupJson, labels: string[]): HTMLElement {
  const window_ = el(document_, 'form', { method: 'post', action: group.href, class: 'cv-window', 'data-window': '' },
    input(document_, 'csrf', group.csrf),
    el(document_, 'label', {}, el(document_, 'span', {}, 'Window start (UTC)'), el(document_, 'input', { name: 'window_start', type: 'datetime-local', step: '1', value: group.window_start })),
    el(document_, 'label', {}, el(document_, 'span', {}, 'Window end (UTC, excluded)'), el(document_, 'input', { name: 'window_end', type: 'datetime-local', step: '1', value: group.window_end })),
    el(document_, 'label', {}, el(document_, 'span', {}, 'Destination (optional)'), el(document_, 'input', { name: 'destination', type: 'text', value: group.destination })),
    el(document_, 'button', { type: 'submit' }, 'Run views'));
  const chips = labels.length
    ? el(document_, 'p', { class: 'cv-labels', 'aria-label': 'How this app was assembled' }, ...labels.map((label) => el(document_, 'span', { class: 'cv-label-chip' }, label)))
    : null;
  const app = el(document_, 'article', { class: 'cv-card cv-app', 'data-card': group.app_card, 'data-kind': 'app', 'aria-label': group.title },
    el(document_, 'p', { class: 'cv-eyebrow' }, 'App'),
    el(document_, 'h2', {}, group.title),
    chips,
    el(document_, 'p', { class: 'cv-provenance' }, group.label),
    window_,
    el(document_, 'p', { class: 'cv-app-links' }, el(document_, 'a', { href: group.href }, 'Open the work page')));
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
  return section;
}
