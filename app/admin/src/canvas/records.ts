// The records card (ADR-011 scenarios, APP-093/APP-095): a saved work's
// current records as the server lists them through
// `GET /v1/work/{app}/{work}/records` (record_read on the work, RLS, the work
// fence), newest commit first. The card is rendered on the server first
// (`canvas/app.tsx`, `query::canvas::records_cards`) and refreshed here on
// every `record.committed` event of its work and after every command this
// browser confirms. A pending record (at a declared transition's starting
// state) offers Approve and Reject to a member who holds record_write: each
// asks `POST /v1/act` for a server-minted `decide` draft (the typed command
// and its idempotency key) and posts it through the audited command route.
// Nothing here computes a value: every cell is a display the server attached
// to a typed value, every identity (publication, definition digest, commit
// sequence, version) is the record's own.
import { postDraft, requestDecision, type Draft } from './actions';
import { label } from './display';
import { el } from './render';

export type RecordValueJson = { field_id: string; value: unknown; display: string };
export type RecordJson = {
  record_id: string; collection_id: string; schema_id: string; version: number; commit_sequence: number;
  committed_at: string; creator: { principal_id: string; label: string }; updater: { principal_id: string; label: string };
  values: RecordValueJson[];
};
export type RecordsJson = {
  app_id: string; work_id: string; publication_id: string; definition_digest: string; work_version: number; can_write: boolean;
  collections: {
    collection_id: string; schema_id: string; schema_version: number;
    fields: { id: string; type: string; nullable: boolean; writable: boolean; values?: string[] }[];
    commands: { id: string; operation: string; writable_fields: string[]; transitions: { field_id: string; from: string; to: string }[] }[];
  }[];
  records: RecordJson[];
  truncated: boolean;
};
/** The server's records card JSON (`GET /v1/canvas/works/{app}/{work}` → `records[]`). */
export type RecordsCardJson = {
  id: string; title: string; summary: string; url: string; collection: string; schema: string; can_write: boolean;
  decide: string; status_field: string; count: string; pending: string;
};

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;

export function recordsJson(value: unknown): RecordsJson | null {
  if (typeof value !== 'object' || value === null) return null;
  const listing = value as RecordsJson;
  if (!UUID.test(listing.app_id) || !UUID.test(listing.work_id) || !Array.isArray(listing.records) || !Array.isArray(listing.collections)) return null;
  return listing;
}

/** `sha256:<64 hex>` as its first twelve and last six digits (`query::canvas::short_digest`). */
function shortDigest(digest: string): string {
  return /^sha256:[0-9a-f]{64}$/.test(digest) ? `sha256:${digest.slice(7, 19)}…${digest.slice(-6)}` : digest;
}

/** "expense_claim" → "Expense claims" (`query::canvas::schema_title`). */
export function schemaTitle(schema: string): string {
  const words = label(schema);
  return words.endsWith('s') ? words : `${words}s`;
}

/** The idle shell of a records card the island places (`buildGroup`); `refreshRecords` fills it. */
export function buildRecordsCard(document_: Document, card: RecordsCardJson): HTMLElement {
  const node = el(document_, 'article', {
    class: 'cv-card cv-records', 'data-card': card.id, 'data-kind': 'records', 'data-records': card.url,
    'data-collection': card.collection, 'data-schema': card.schema, 'data-decide': card.decide, 'data-status-field': card.status_field,
    'data-can-write': card.can_write ? 'true' : 'false', 'aria-label': card.title,
  },
  el(document_, 'header', { class: 'cv-card-head' }, el(document_, 'h3', {}, card.title), el(document_, 'span', { class: 'badge' }, 'records')),
  el(document_, 'p', { class: 'cv-records-summary', 'data-summary': '' }, card.summary),
  el(document_, 'div', { class: 'cv-card-body', 'data-body': '' }, el(document_, 'p', { class: 'cv-message', role: 'status' }, 'Loading records…')),
  el(document_, 'p', { class: 'cv-records-provenance', 'data-provenance': '' }),
  el(document_, 'p', { class: 'cv-outcome', role: 'status', 'data-outcome': '' }));
  return node;
}

function valueOf(record: RecordJson, field: string): RecordValueJson | undefined {
  return record.values.find((v) => v.field_id === field);
}
function statusOf(record: RecordJson, field: string): string {
  const value = valueOf(record, field)?.value as { value?: unknown } | undefined;
  return typeof value?.value === 'string' ? value.value : '';
}

/** Draws the listing into the card: the summary line, the table and the provenance line. */
export function renderRecords(card: HTMLElement, listing: RecordsJson): void {
  const document_ = card.ownerDocument;
  const collection = listing.collections.find((c) => c.collection_id === card.dataset.collection) ?? listing.collections[0];
  const body = card.querySelector<HTMLElement>('[data-body]');
  const summary = card.querySelector<HTMLElement>('[data-summary]');
  const provenance = card.querySelector<HTMLElement>('[data-provenance]');
  if (!collection || !body) return;
  const decide = collection.commands.find((c) => c.transitions.length > 0);
  const statusField = decide?.transitions[0]?.field_id ?? '';
  const froms = new Set((decide?.transitions ?? []).map((t) => t.from));
  const records = listing.records.filter((r) => r.collection_id === collection.collection_id);
  const pending = records.filter((r) => listing.can_write && froms.has(statusOf(r, statusField)));
  card.dataset.decide = decide?.id ?? '';
  card.dataset.statusField = statusField;
  card.dataset.canWrite = listing.can_write ? 'true' : 'false';
  const count = records.length;
  if (summary) {
    summary.textContent = listing.can_write && decide
      ? `${pending.length === 0 ? 'Nothing' : pending.length} waiting for your approval · ${count} ${count === 1 ? 'record' : 'records'}`
      : `${count} ${count === 1 ? 'record' : 'records'}`;
  }
  const previous = new Set(Array.from(card.querySelectorAll<HTMLElement>('tr[data-record]')).map((row) => `${row.dataset.record}@${row.dataset.version}`));
  if (!records.length) {
    body.replaceChildren(el(document_, 'p', { class: 'cv-message', role: 'status' }, 'No records yet.'));
  } else {
    const shown = collection.fields.filter((f) => f.id !== statusField);
    const head = el(document_, 'tr', {});
    for (const field of shown) head.append(el(document_, 'th', { scope: 'col' }, label(field.id)));
    if (statusField) head.append(el(document_, 'th', { scope: 'col' }, label(statusField)));
    head.append(el(document_, 'th', { scope: 'col' }, 'Filed by'));
    if (pending.length) head.append(el(document_, 'th', { scope: 'col', class: 'cv-records-actions-head' }, 'Decision'));
    const rows = el(document_, 'tbody', {});
    for (const record of records) {
      const status = statusOf(record, statusField);
      const isPending = listing.can_write && froms.has(status);
      const tr = el(document_, 'tr', { 'data-record': record.record_id, 'data-version': String(record.version), 'data-status': status });
      if (!previous.has(`${record.record_id}@${record.version}`) && previous.size) tr.classList.add('is-new');
      for (const field of shown) {
        const value = valueOf(record, field.id);
        const numeric = field.type === 'money' || field.type === 'integer' || field.type === 'decimal';
        tr.append(el(document_, 'td', { class: numeric ? 'value num' : 'value', title: `${label(field.id)} · version ${record.version}` }, value?.display ?? '—'));
      }
      if (statusField) {
        tr.append(el(document_, 'td', { class: 'cv-status' }, el(document_, 'span', { class: 'cv-status-chip', 'data-status': status }, status)));
      }
      tr.append(el(document_, 'td', { class: 'cv-records-by', title: record.creator.principal_id }, record.creator.label));
      if (pending.length) {
        const cell = el(document_, 'td', { class: 'cv-records-actions' });
        if (isPending && decide) {
          const targets = decide.transitions.filter((t) => t.field_id === statusField && t.from === status).map((t) => t.to);
          for (const to of targets) {
            const button = el(document_, 'button', { type: 'button', class: to.startsWith('approv') ? 'cv-approve' : 'cv-reject', 'data-decide': to }, label(to.replace(/ed$/, 'e')));
            button.addEventListener('click', () => void decideRecord(card, listing, record, to, button));
            cell.append(button);
          }
        }
        tr.append(cell);
      }
      rows.append(tr);
    }
    body.replaceChildren(el(document_, 'div', { class: 'cv-table cv-records-table' }, el(document_, 'table', {}, el(document_, 'thead', {}, head), rows)));
  }
  if (provenance) {
    const newest = records[0];
    provenance.replaceChildren(
      el(document_, 'span', { class: 'cv-verified' }, 'Records'),
      el(document_, 'code', { title: `Publication ${listing.publication_id}` }, `publication ${listing.publication_id.slice(0, 8)}`),
      el(document_, 'code', { title: `Definition digest ${listing.definition_digest}` }, shortDigest(listing.definition_digest)),
      el(document_, 'code', { title: 'The organization commit sequence of the newest record shown' }, newest ? `commit ${newest.commit_sequence}` : 'no commits'),
      el(document_, 'span', { class: 'cv-receipt-meta' }, `work v${listing.work_version}${listing.truncated ? ' · first page' : ''}`));
  }
  card.dataset.state = 'ready';
  const chip = card.querySelector<HTMLElement>('.badge');
  if (chip) chip.textContent = `${count} ${count === 1 ? 'record' : 'records'}`;
}

/** Fetches the listing and redraws the card; a refusal shows no row. */
export async function refreshRecords(card: HTMLElement, live = false): Promise<void> {
  const url = card.dataset.records ?? '';
  if (!url.startsWith('/v1/work/')) return;
  const epoch = String(Number(card.dataset.epoch ?? '0') + 1);
  card.dataset.epoch = epoch;
  let response: Response;
  try {
    response = await fetch(url, { credentials: 'same-origin', cache: 'no-store' });
  } catch {
    return;
  }
  if (card.dataset.epoch !== epoch) return;
  const body: unknown = await response.json().catch(() => null);
  if (card.dataset.epoch !== epoch) return;
  const listing = response.ok ? recordsJson(body) : null;
  if (!listing) {
    const message = response.status === 404 ? 'You do not have access to these records. Nothing is shown.' : 'The records could not be read right now.';
    card.dataset.state = response.status === 404 ? 'denied' : 'failed';
    card.querySelector<HTMLElement>('[data-body]')?.replaceChildren(el(card.ownerDocument, 'p', { class: 'cv-message', role: 'alert' }, message));
    return;
  }
  renderRecords(card, listing);
  if (live) {
    const item = card.closest<HTMLElement>('.cv-item') ?? card;
    item.classList.remove('is-live-updated');
    void item.offsetWidth;
    item.classList.add('is-live-updated');
    window.setTimeout(() => item.classList.remove('is-live-updated'), 1600);
  }
}

/** Approve or Reject: a server-minted decide draft, then the audited command route. */
async function decideRecord(card: HTMLElement, listing: RecordsJson, record: RecordJson, to: string, button: HTMLButtonElement): Promise<void> {
  const outcome = card.querySelector<HTMLElement>('[data-outcome]');
  const row = button.closest<HTMLElement>('tr');
  row?.querySelectorAll<HTMLButtonElement>('button').forEach((b) => { b.disabled = true; });
  if (outcome) {
    outcome.setAttribute('role', 'status');
    outcome.textContent = 'Preparing the decision…';
  }
  let draft: Draft;
  try {
    draft = await requestDecision(listing.app_id, listing.work_id, record.record_id, to);
  } catch (error) {
    if (outcome) {
      outcome.setAttribute('role', 'alert');
      outcome.textContent = error instanceof Error && error.message ? error.message : 'The decision could not be prepared. Nothing was changed.';
    }
    row?.querySelectorAll<HTMLButtonElement>('button').forEach((b) => { b.disabled = false; });
    return;
  }
  if (outcome) outcome.textContent = `Committing ${label(to)}…`;
  const result = await postDraft(draft);
  if (!outcome) return;
  outcome.replaceChildren();
  if (result.kind === 'committed') {
    outcome.setAttribute('role', 'status');
    outcome.append(el(card.ownerDocument, 'strong', {}, `${label(to)}. `), `Record version ${result.recordVersion}, commit ${result.commit}. Audit event `, el(card.ownerDocument, 'code', {}, result.audit));
    await refreshRecords(card, true);
  } else {
    outcome.setAttribute('role', 'alert');
    outcome.append(result.message);
    row?.querySelectorAll<HTMLButtonElement>('button').forEach((b) => { b.disabled = false; });
  }
}

/** Wires a server-rendered or island-built records card: its buttons, then a first refresh when it is idle. */
export function wireRecordsCard(card: HTMLElement): void {
  if (card.dataset.wired === 'true') return;
  card.dataset.wired = 'true';
  // Server-rendered rows carry plain buttons; the first refresh rebuilds them wired.
  void refreshRecords(card);
}

/** Every records card of a group's items. */
export function recordsCardsOf(items: HTMLElement[]): HTMLElement[] {
  return items
    .map((item) => item.querySelector<HTMLElement>('.cv-records'))
    .filter((card): card is HTMLElement => Boolean(card));
}
