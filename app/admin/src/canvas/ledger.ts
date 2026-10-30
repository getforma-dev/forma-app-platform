// The expense ledger card (APP-102): every expense the member may read in a
// saved work, added up live. The card is rendered on the server first
// (`canvas/app.tsx`, `query::canvas_ledger`) with totals over the whole
// listing; this module adds the date range (presets and custom) and the
// status filter, and recomputes the total, the split by person and by
// category over the listed records (`GET /v1/work/{app}/{work}/records`,
// at most 200, newest first: labeled "live records, last 200", DS-58). It
// re-reads the listing on every `record.committed` of its work (`board.ts`),
// so a newly confirmed claim appears without a reload. Nothing here writes;
// every amount is a record's own typed money value, summed exactly in its
// minor units (BigInt), per currency.
import { label, money } from './display';
import { recordsJson, type RecordJson, type RecordsJson } from './records';
import { el, pulseTarget } from './render';

export const LEDGER_SCOPE = 'live records, last 200';

/** The server's ledger card JSON (`GET /v1/canvas/works/{app}/{work}` → `ledger[]`). */
export type LedgerCardJson = {
  id: string; title: string; url: string; collection: string; amount_field: string; date_field: string;
  category_field: string; status_field: string; scope: string; total: string; count: string;
};

type Preset = 'all' | 'this-month' | 'last-month' | 'last-30' | 'custom';
const PRESETS: [Preset, string][] = [
  ['all', 'All dates'], ['this-month', 'This month'], ['last-month', 'Last month'], ['last-30', 'Last 30 days'], ['custom', 'Custom range'],
];

const listings = new WeakMap<HTMLElement, RecordsJson>();

/**
 * Sums of typed money, exact (BigInt minor units), one per currency and
 * minor-unit exponent (`USD:2`): a ledger never adds two currencies.
 */
export type Amounts = Map<string, bigint>;

/** Amounts as a person reads them, exact (`display.money`): `$1,234.50`, or `$10.00 + €5.00`. */
export function formatAmounts(amounts: Amounts): string {
  const parts = [...amounts].map(([key, minor]) => {
    const [currency = 'USD', exponent = '2'] = key.split(':');
    return money(minor < 0n, minor < 0n ? -minor : minor, currency, Number(exponent))[1];
  });
  return parts.length ? parts.join(' + ') : money(false, 0n, 'USD', 2)[1];
}

function add(amounts: Amounts, key: string, minor: bigint): void {
  amounts.set(key, (amounts.get(key) ?? 0n) + minor);
}
/** The first amount (the ledger's main currency), for ordering. */
function lead(amounts: Amounts): bigint {
  return amounts.values().next().value ?? 0n;
}

function iso(date: Date): string {
  return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, '0')}-${String(date.getDate()).padStart(2, '0')}`;
}

/** A preset's inclusive range in the member's local calendar; `all` is unbounded. */
export function presetRange(preset: Preset, today = new Date()): [string, string] {
  const y = today.getFullYear();
  const m = today.getMonth();
  switch (preset) {
    case 'this-month': return [iso(new Date(y, m, 1)), iso(new Date(y, m + 1, 0))];
    case 'last-month': return [iso(new Date(y, m - 1, 1)), iso(new Date(y, m, 0))];
    case 'last-30': return [iso(new Date(y, m, today.getDate() - 29)), iso(today)];
    default: return ['', ''];
  }
}

function field(record: RecordJson, id: string): Record<string, unknown> | undefined {
  const value = record.values.find((v) => v.field_id === id)?.value;
  return typeof value === 'object' && value !== null ? (value as Record<string, unknown>) : undefined;
}
/** A record's money value: minor units exactly, keyed by currency and exponent; null when absent or malformed. */
function amountOf(record: RecordJson, id: string): { key: string; minor: bigint } | null {
  const value = field(record, id);
  const minor = value?.minor_units;
  const currency = typeof value?.currency === 'string' && /^[A-Z]{3}$/.test(value.currency) ? value.currency : 'USD';
  const exponent = Number.isInteger(value?.minor_unit_exponent) && Number(value?.minor_unit_exponent) >= 0 && Number(value?.minor_unit_exponent) <= 6
    ? Number(value?.minor_unit_exponent) : 2;
  if (typeof minor !== 'string' || !/^-?[0-9]{1,36}$/.test(minor)) return null;
  return { key: `${currency}:${exponent}`, minor: BigInt(minor) };
}
function text(record: RecordJson, id: string): string {
  const value = field(record, id)?.value;
  return typeof value === 'string' ? value : '';
}

export type LedgerFilter = { from: string; to: string; status: string };
export type LedgerTotals = { total: Amounts; count: number; people: [string, number, Amounts][]; categories: [string, number, Amounts][] };

/** The ledger's arithmetic over a listing: records in range and status, summed exactly in minor units. */
export function totals(listing: RecordsJson, card: { collection: string; amount: string; date: string; category: string; status: string }, filter: LedgerFilter): LedgerTotals {
  const people = new Map<string, [number, Amounts]>();
  const categories = new Map<string, [number, Amounts]>();
  const total: Amounts = new Map();
  let count = 0;
  for (const record of listing.records) {
    if (card.collection && record.collection_id !== card.collection) continue;
    const day = text(record, card.date);
    if (filter.from && (!day || day < filter.from)) continue;
    if (filter.to && (!day || day > filter.to)) continue;
    if (filter.status && card.status && text(record, card.status) !== filter.status) continue;
    const amount = amountOf(record, card.amount);
    count += 1;
    const person = people.get(record.creator.label) ?? [0, new Map()];
    person[0] += 1;
    people.set(record.creator.label, person);
    const name = card.category ? text(record, card.category) || '—' : '';
    const entry = card.category ? categories.get(name) ?? [0, new Map()] : null;
    if (entry) {
      entry[0] += 1;
      categories.set(name, entry);
    }
    if (!amount) continue;
    add(total, amount.key, amount.minor);
    add(person[1], amount.key, amount.minor);
    if (entry) add(entry[1], amount.key, amount.minor);
  }
  const sorted = (map: Map<string, [number, Amounts]>) =>
    Array.from(map, ([name, [n, sum]]) => [name, n, sum] as [string, number, Amounts])
      .sort((a, b) => (lead(b[2]) > lead(a[2]) ? 1 : lead(b[2]) < lead(a[2]) ? -1 : a[0].localeCompare(b[0])));
  return { total, count, people: sorted(people), categories: sorted(categories) };
}

/** The idle shell of a ledger card the island places (`buildGroup`); `wireLedgerCard` fills it. */
export function buildLedgerCard(document_: Document, card: LedgerCardJson): HTMLElement {
  return el(document_, 'article', {
    class: 'cv-card cv-ledger', 'data-card': card.id, 'data-kind': 'ledger', 'data-records': card.url,
    'data-collection': card.collection, 'data-amount': card.amount_field, 'data-date': card.date_field,
    'data-category': card.category_field, 'data-status-field': card.status_field, 'aria-label': card.title,
  },
  el(document_, 'header', { class: 'cv-card-head' }, el(document_, 'h3', {}, card.title), el(document_, 'span', { class: 'badge' }, card.scope || LEDGER_SCOPE)),
  el(document_, 'div', { class: 'cv-ledger-controls', 'data-controls': '' }),
  el(document_, 'div', { class: 'cv-card-body', 'data-body': '' }, el(document_, 'p', { class: 'cv-message', role: 'status' }, 'Loading the ledger…')),
  el(document_, 'p', { class: 'cv-records-provenance', 'data-provenance': '' }));
}

function filterOf(card: HTMLElement): LedgerFilter {
  const preset = (card.dataset.preset ?? 'all') as Preset;
  const [from, to] = preset === 'custom' ? [card.dataset.from ?? '', card.dataset.to ?? ''] : presetRange(preset);
  return { from, to, status: card.dataset.status ?? '' };
}

function rangeText(filter: LedgerFilter): string {
  if (!filter.from && !filter.to) return 'all dates';
  if (filter.from === filter.to) return filter.from;
  return `${filter.from || 'start'} to ${filter.to || 'today'}`;
}

function table(document_: Document, caption: string, rows: [string, number, Amounts][]): HTMLElement {
  const body = el(document_, 'tbody', {});
  for (const [name, n, sum] of rows) {
    body.append(el(document_, 'tr', {}, el(document_, 'th', { scope: 'row' }, name), el(document_, 'td', { class: 'num' }, String(n)), el(document_, 'td', { class: 'num' }, formatAmounts(sum))));
  }
  return el(document_, 'table', { class: 'cv-ledger-table' }, el(document_, 'caption', {}, caption), body);
}

/** Redraws the card from its cached listing and its current filter. */
export function renderLedger(card: HTMLElement): void {
  const listing = listings.get(card);
  const body = card.querySelector<HTMLElement>('[data-body]');
  if (!listing || !body) return;
  const document_ = card.ownerDocument;
  const filter = filterOf(card);
  const sums = totals(listing, {
    collection: card.dataset.collection ?? '', amount: card.dataset.amount ?? 'amount', date: card.dataset.date ?? 'spent_on',
    category: card.dataset.category ?? '', status: card.dataset.statusField ?? '',
  }, filter);
  const scope = [rangeText(filter), filter.status ? filter.status : 'every status'].join(' · ');
  const total = el(document_, 'p', { class: 'cv-ledger-total' },
    el(document_, 'span', { class: 'cv-ledger-total-value num', 'data-total-cents': String(lead(sums.total)) }, formatAmounts(sums.total)), ' ',
    el(document_, 'span', { class: 'cv-muted' }, `across ${sums.count} ${sums.count === 1 ? 'record' : 'records'} · ${scope}`));
  if (!sums.count) {
    body.replaceChildren(total, el(document_, 'p', { class: 'cv-message', role: 'status' }, 'No records in this range.'));
  } else {
    body.replaceChildren(total, el(document_, 'div', { class: 'cv-ledger-split' },
      table(document_, 'By person', sums.people),
      table(document_, card.dataset.category ? `By ${label(card.dataset.category).toLowerCase()}` : 'By category', sums.categories)));
  }
  const provenance = card.querySelector<HTMLElement>('[data-provenance]');
  provenance?.replaceChildren(
    el(document_, 'span', { class: 'cv-verified' }, 'Records'),
    el(document_, 'span', { class: 'cv-receipt-meta' }, `${LEDGER_SCOPE}${listing.truncated ? ' (more exist)' : ''}; totals computed in this view · work v${listing.work_version}`));
  card.dataset.state = 'ready';
}

function controls(card: HTMLElement): void {
  const host = card.querySelector<HTMLElement>('[data-controls]');
  if (!host || host.dataset.built === 'true') return;
  host.dataset.built = 'true';
  const document_ = card.ownerDocument;
  const preset = el(document_, 'select', { 'aria-label': 'Date range', 'data-preset': '' });
  for (const [value, text] of PRESETS) preset.append(el(document_, 'option', { value }, text));
  const from = el(document_, 'input', { type: 'date', 'aria-label': 'From', 'data-from': '' });
  const to = el(document_, 'input', { type: 'date', 'aria-label': 'To', 'data-to': '' });
  const custom = el(document_, 'span', { class: 'cv-ledger-custom', hidden: '' }, from, el(document_, 'span', { 'aria-hidden': 'true' }, '–'), to);
  const status = el(document_, 'select', { 'aria-label': 'Status', 'data-status': '' }, el(document_, 'option', { value: '' }, 'Every status'));
  const sync = () => {
    preset.value = card.dataset.preset ?? 'all';
    from.value = card.dataset.from ?? '';
    to.value = card.dataset.to ?? '';
    status.value = card.dataset.status ?? '';
    custom.hidden = preset.value !== 'custom';
  };
  preset.addEventListener('change', () => {
    card.dataset.preset = preset.value;
    if (preset.value === 'custom' && !card.dataset.from) {
      const [a, b] = presetRange('this-month');
      card.dataset.from = a;
      card.dataset.to = b;
    }
    sync();
    renderLedger(card);
  });
  for (const input of [from, to]) {
    input.addEventListener('change', () => {
      card.dataset.from = from.value;
      card.dataset.to = to.value;
      renderLedger(card);
    });
  }
  status.addEventListener('change', () => {
    card.dataset.status = status.value;
    renderLedger(card);
  });
  card.addEventListener('ledger:sync', sync);
  host.replaceChildren(preset, custom, status);
  sync();
}

function statusOptions(card: HTMLElement, listing: RecordsJson): void {
  const select = card.querySelector<HTMLSelectElement>('select[data-status]');
  const statusField = card.dataset.statusField ?? '';
  if (!select || !statusField || select.options.length > 1) return;
  const collection = listing.collections.find((c) => c.collection_id === card.dataset.collection) ?? listing.collections[0];
  const values = collection?.fields.find((f) => f.id === statusField)?.values ?? [];
  for (const value of values) select.append(el(card.ownerDocument, 'option', { value }, label(value)));
  select.value = card.dataset.status ?? '';
}

/** Fetches the listing and redraws the card; a refusal shows no amount. */
export async function refreshLedger(card: HTMLElement, live = false): Promise<void> {
  const url = card.dataset.records ?? '';
  if (!url.startsWith('/v1/work/')) return;
  const epoch = String(Number(card.dataset.epoch ?? '0') + 1);
  card.dataset.epoch = epoch;
  let response: Response | null = null;
  try {
    response = await fetch(url, { credentials: 'same-origin', cache: 'no-store' });
  } catch {
    // APP-113: a network error shows the failed state below, never "Loading…" forever.
  }
  const body: unknown = response ? await response.json().catch(() => null) : null;
  if (card.dataset.epoch !== epoch) return;
  const listing = response?.ok ? recordsJson(body) : null;
  if (!listing) {
    listings.delete(card);
    const denied = response?.status === 404;
    card.dataset.state = denied ? 'denied' : 'failed';
    card.querySelector<HTMLElement>('[data-body]')?.replaceChildren(el(card.ownerDocument, 'p', { class: 'cv-message', role: 'alert' },
      denied ? 'You do not have access to these records. Nothing is shown.' : 'The ledger could not be read right now.'));
    return;
  }
  listings.set(card, listing);
  statusOptions(card, listing);
  renderLedger(card);
  if (live) {
    const item = pulseTarget(card);
    item.classList.remove('is-live-updated');
    void item.offsetWidth;
    item.classList.add('is-live-updated');
    window.setTimeout(() => item.classList.remove('is-live-updated'), 1600);
  }
}

/** Wires a server-rendered or island-built ledger card: its controls, then a first refresh. */
export function wireLedgerCard(card: HTMLElement): void {
  if (card.dataset.wired === 'true') return;
  card.dataset.wired = 'true';
  controls(card);
  void refreshLedger(card);
}

/** Sets a card's range (an `open` answer's "from Oct 1 to Oct 30"); empty means all dates. */
export function setLedgerRange(card: HTMLElement, range: { from: string; to: string } | null): void {
  if (range && /^\d{4}-\d{2}-\d{2}$/.test(range.from) && /^\d{4}-\d{2}-\d{2}$/.test(range.to)) {
    card.dataset.preset = 'custom';
    card.dataset.from = range.from;
    card.dataset.to = range.to;
  } else if (range === null && !card.dataset.preset) {
    card.dataset.preset = 'all';
  }
  card.dispatchEvent(new Event('ledger:sync'));
  renderLedger(card);
}

/** The ledger card of a work, wherever the canvas moved it. */
export function ledgerCardFor(app: string, work: string): HTMLElement | null {
  return document.querySelector<HTMLElement>(`.cv-ledger[data-records="/v1/work/${app}/${work}/records"]`);
}
