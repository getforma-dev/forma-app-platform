// APP-122 Ask: the right pane's Ask tab, a read-only conversation about the
// records in scope. `?` as the bar's first character opens it with the text
// (`?` pressed outside the bar puts `? ` in the bar); ⋯ → Ask about this
// opens it scoped to the selected card. The scope is the selected card, else
// the canvas (the apps on it), shown as a chip that is not removable.
//
// `POST /v1/ask` answers only `{text, receipts, references}`: the text is
// shown as a text node; each reference is a record the answer used, with its
// receipt (record id, version, commit, publication, definition digest) from
// the server, never from the model. Nothing here changes the canvas: an
// answer never shapes a card (Shape is its own job, `shape.ts`).
import { activeCanvasItem, boardWorks, focusCard, workCard } from './board';
import { selectedCanvasItem } from './controls';
import { findCardRow } from './library';
import { onPaneTab, openPane } from './pane';
import { el } from './render';
import { addressOfItem, addressOfWork } from './shape';

export const ASK_URL = '/v1/ask';
const MAX_WORKS = 12;

export type Scope = { kind: 'card' | 'canvas'; address: string; works: { app_id: string; work_id: string; address: string }[] };

/** The scope for a canvas item: a card of an app (its address and work), else null. */
export function scopeOfItem(item: HTMLElement | null): Scope | null {
  if (!item) return null;
  const row = findCardRow(item.dataset.instanceId ?? '');
  const address = addressOfItem(item);
  if (!row || !address) return null;
  return { kind: 'card', address, works: [{ app_id: row.app.app, work_id: row.app.work, address: addressOfWork(row.app.app, row.app.work) || row.app.slug }] };
}

/** The selected card, else the canvas: every app on it (at most twelve). */
export function currentScope(): Scope {
  const card = scopeOfItem(selectedCanvasItem() ?? null);
  if (card) return card;
  const works = boardWorks().slice(0, MAX_WORKS).map((work) => ({ app_id: work.app, work_id: work.work, address: addressOfWork(work.app, work.work) }));
  return { kind: 'canvas', address: '', works };
}

export function scopeText(scope: Scope): string {
  if (scope.kind === 'card') return `Asking about /${scope.address}`;
  const n = scope.works.length;
  return n ? `Asking about your canvas (${n} ${n === 1 ? 'app' : 'apps'})` : 'Your canvas has no apps yet';
}

let scope: Scope | null = null;
let thread: HTMLElement | null = null;
let chip: HTMLElement | null = null;
let input: HTMLInputElement | null = null;
let busy = false;
/** Set by `openAsk`: the next showing keeps its scope instead of reading the selection. */
let pinned = false;

const REFUSALS: Record<string, string> = {
  composer_unavailable: 'That question needs the AI model, and none is configured here. Try: “where are the red invoices”, “how many”, “what’s the total”.',
  could_not_answer: 'The answer came back in the wrong shape, so it was not shown. Nothing changed.',
  too_many_requests: 'Too many questions in a short time. Try again in a moment.',
  invalid_request: 'Ask in a short sentence (up to 500 characters).',
};

function entry(document_: Document, who: 'you' | 'forma' | 'note', ...children: (Node | string)[]): HTMLElement {
  const node = el(document_, 'li', { class: `cv-ask-entry cv-ask-${who}` }, ...children);
  thread?.append(node);
  node.scrollIntoView?.({ block: 'nearest' });
  return node;
}

type Receipt = Record<string, unknown>;
type Reference = { ref: string; app_id: string; work_id: string; record_id: string; address: string; label: string };

function referenceList(document_: Document, references: Reference[], receipts: Receipt[]): HTMLElement {
  const list = el(document_, 'ul', { class: 'cv-ask-refs', 'aria-label': 'Records this answer used' });
  for (const reference of references) {
    const receipt = receipts.find((r) => r.record_id === reference.record_id) ?? {};
    const text = (key: string) => String(receipt[key] ?? '');
    const open = el(document_, 'button', { type: 'button', class: 'cv-ask-ref', title: 'Show its app on the canvas' }, reference.label || reference.record_id.slice(0, 8));
    open.addEventListener('click', () => {
      const card = workCard(reference.work_id, 'records') ?? workCard(reference.work_id, 'ledger') ?? workCard(reference.work_id);
      if (card) focusCard(card);
    });
    list.append(el(document_, 'li', {}, open,
      el(document_, 'details', { class: 'cv-ask-receipt' },
        el(document_, 'summary', {}, 'Receipt'),
        el(document_, 'dl', {},
          ...([['Record', 'record_id'], ['Version', 'version'], ['Commit', 'commit_sequence'], ['Committed', 'committed_at'], ['Publication', 'publication_id'], ['Definition', 'definition_digest']] as const)
            .map(([label, key]) => el(document_, 'div', {}, el(document_, 'dt', {}, label), el(document_, 'dd', {}, el(document_, 'code', {}, text(key)))))))));
  }
  return list;
}

/** Sends `question` with the current scope; the answer joins the thread. */
export async function ask(question: string): Promise<void> {
  const document_ = document;
  const text = question.trim().replace(/^\?\s*/, '');
  if (!text || busy) return;
  const current = scope ?? currentScope();
  entry(document_, 'you', el(document_, 'span', { class: 'cv-ask-scope-mini' }, current.kind === 'card' ? `/${current.address}` : 'canvas'), text);
  if (!current.works.length) {
    entry(document_, 'note', 'Put an app on the canvas first (the Library, L), then ask about it.');
    return;
  }
  busy = true;
  const pending = entry(document_, 'note', 'Reading…');
  try {
    let response: Response;
    try {
      response = await fetch(ASK_URL, {
        method: 'POST', credentials: 'same-origin', cache: 'no-store',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ question: text, scope: { kind: current.kind, address: current.address, works: current.works } }),
      });
    } catch {
      pending.textContent = 'The question could not be sent. Check your connection.';
      return;
    }
    const body = (await response.json().catch(() => null)) as Record<string, unknown> | null;
    if (response.status === 404) {
      pending.textContent = 'You can’t read one of the apps in scope.';
      return;
    }
    if (!response.ok || !body || typeof body.text !== 'string') {
      pending.textContent = REFUSALS[typeof body?.code === 'string' ? body.code : ''] ?? 'Ask could not answer right now. Nothing changed.';
      return;
    }
    // Exactly the three keys: anything else (a view spec) is not shown.
    if (Object.keys(body).some((key) => !['text', 'receipts', 'references'].includes(key))) {
      pending.textContent = REFUSALS.could_not_answer ?? '';
      return;
    }
    pending.remove();
    const references = Array.isArray(body.references) ? (body.references as Reference[]).filter((r) => typeof r?.record_id === 'string').slice(0, 50) : [];
    const receipts = Array.isArray(body.receipts) ? (body.receipts as Receipt[]) : [];
    entry(document_, 'forma', el(document_, 'p', {}, body.text), ...(references.length ? [referenceList(document_, references, receipts)] : []));
  } finally {
    busy = false;
  }
}

function showScope(): void {
  if (chip) chip.textContent = scopeText(scope ?? currentScope());
}

/** Opens the Ask tab (scoped to `explicit`, else the selected card, else the canvas) and asks `question` when given. */
export function openAsk(question = '', explicit: Scope | null = null): void {
  scope = explicit ?? currentScope();
  pinned = true;
  openPane('ask', { focus: !question });
  pinned = false;
  showScope();
  if (question.trim()) void ask(question);
}

/** ⋯ → Ask about this: the selected card's scope. */
export function askAboutItem(item: HTMLElement | null = activeCanvasItem()): boolean {
  const chosen = scopeOfItem(item);
  if (!chosen) return false;
  openAsk('', chosen);
  return true;
}

/** Builds the Ask tab's panel (once). */
export function startAsk(document_: Document): void {
  const panel = document_.getElementById('cv-panel-ask');
  if (!panel || thread) return;
  chip = el(document_, 'p', { class: 'cv-ask-chip', role: 'status' }, '');
  thread = el(document_, 'ol', { class: 'cv-ask-thread', 'aria-live': 'polite', 'aria-label': 'Questions and answers' });
  input = el(document_, 'input', { type: 'text', class: 'cv-ask-input', maxlength: '500', autocomplete: 'off', placeholder: 'Ask about your data: where are the red invoices?', 'aria-label': 'Your question' });
  const form = el(document_, 'form', { class: 'cv-ask-form', autocomplete: 'off' }, input, el(document_, 'button', { type: 'submit' }, 'Ask'));
  form.addEventListener('submit', (event) => {
    event.preventDefault();
    const value = input?.value ?? '';
    if (input) input.value = '';
    void ask(value);
  });
  form.addEventListener('keydown', (event) => event.stopPropagation());
  panel.replaceChildren(
    el(document_, 'p', { class: 'cv-ask-sub' }, 'Ask reads; it never changes records, cards or the canvas. To change how something looks, use /shape.'),
    chip, thread, form);
  onPaneTab('ask', {
    show: (focus) => {
      if (!pinned) scope = currentScope();
      showScope();
      if (focus) input?.focus({ preventScroll: true });
    },
  });
}
