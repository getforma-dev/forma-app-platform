// APP-122 Ask: the right pane's Ask tab, a read-only conversation about the
// records in scope. `?` as the bar's first character opens it with the text
// (`?` pressed outside the bar puts `? ` in the bar, or only focuses it when
// something is half-typed); ⋯ → Ask about this opens it scoped to that card;
// `/ask [address] <question>` scopes it to the app or card the first word
// names (`askLine`, through Shape's resolver: every form, namespaced ones
// and personal shortcuts included), else the selection, else the canvas.
// The scope is shown as a chip that is not removable. A card scope sends
// the card's collection, and the server reads only that collection.
//
// `POST /v1/ask` answers only `{text, receipts, references}`: the text is
// shown as a text node; each reference is a record the answer used, with its
// receipt (record id, version, commit, publication, definition digest) from
// the server, never from the model. A computed number references every
// record it counted (the first twelve shown, the rest under "n more"). A
// question asked while one is being answered waits its turn. Nothing here
// changes the canvas: an answer never shapes a card (Shape is its own job,
// `shape.ts`).
import { activeCanvasItem, boardWorks, focusCard, workCard } from './board';
import { selectedCanvasItem } from './controls';
import { cardAddress, findCardRow, findCards, findApps, type CardRow } from './library';
import { onPaneTab, openPane } from './pane';
import { el, isUuid } from './render';
import { addressOfItem, addressOfWork, resolveTarget, type Target } from './shape';
import type { Completion } from './slash';
import { firstWord, splitAddress } from './talk-view';

export const ASK_URL = '/v1/ask';
const MAX_WORKS = 12;
/** References shown at once; the rest under "n more". */
const SHOWN_REFERENCES = 12;

type ScopeWork = { app_id: string; work_id: string; address: string; collection?: string };
/** A card (one work, its collection when it has one), an app (one work), or the canvas (every app on it). */
export type Scope = { kind: 'card' | 'app' | 'canvas'; address: string; works: ScopeWork[] };

/** A records, ledger or form card's collection: its element's, else its id's (`l-<work>-<collection>`). */
function collectionOf(card: CardRow, element: HTMLElement | null): string | undefined {
  const own = element?.dataset.collection;
  if (isUuid(own)) return own;
  const id = /^[lrf]-[0-9a-f-]{36}-([0-9a-f-]{36})$/.exec(card.id)?.[1];
  return isUuid(id) ? id : undefined;
}

const workOf = (card: CardRow) => ({ app_id: card.app.app, work_id: card.app.work, address: addressOfWork(card.app.app, card.app.work) || card.app.slug });

/** The scope a resolved address names: a card (and its collection), or an app; null for anything else. */
export function scopeOfTarget(target: Target, element: HTMLElement | null = null): Scope | null {
  if (target.kind === 'app') {
    const { app } = target;
    return { kind: 'app', address: target.address, works: [{ app_id: app.app, work_id: app.work, address: addressOfWork(app.app, app.work) || app.slug }] };
  }
  if (target.kind !== 'card') return null;
  const element_ = element ?? (typeof document?.querySelector === 'function' && /^[A-Za-z0-9_-]{1,160}$/.test(target.card.id)
    ? document.querySelector<HTMLElement>(`.cv-card[data-card="${target.card.id}"]`) : null);
  const collection = collectionOf(target.card, element_);
  return { kind: 'card', address: target.address, works: [{ ...workOf(target.card), ...(collection ? { collection } : {}) }] };
}

/** The scope for a canvas item: a card of an app (its address, work and collection), an app card or frame (the app), else null. */
export function scopeOfItem(item: HTMLElement | null): Scope | null {
  if (!item) return null;
  const address = addressOfItem(item);
  if (!address) return null;
  const target = resolveTarget(address);
  const row = findCardRow(item.dataset.instanceId ?? '');
  if (target.kind === 'card' && row) return scopeOfTarget({ kind: 'card', card: row, address: cardAddress(row) }, item.querySelector<HTMLElement>('.cv-card') ?? item);
  return scopeOfTarget(target);
}

/** The selected card, else the canvas: every app on it (at most twelve). */
export function currentScope(): Scope {
  const card = scopeOfItem(selectedCanvasItem() ?? null);
  if (card) return card;
  const works = boardWorks().slice(0, MAX_WORKS).map((work) => ({ app_id: work.app, work_id: work.work, address: addressOfWork(work.app, work.work) }));
  return { kind: 'canvas', address: '', works };
}

/**
 * `/ask [address] <question>`: the first word scopes Ask when it is one of
 * the member's app or card names (any form: `expenses`, `expenses.ledger`,
 * `3.2`, `app:expenses`, `card:expenses.ledger`, a personal shortcut), found
 * through Shape's resolver in its strict mode (exact names only, verbs and
 * widget kinds never). Otherwise the whole line is the question, scoped to
 * the selection, else the canvas. A namespaced first word that names
 * nothing Ask can read (a widget, a folder, an unknown name) is a message.
 */
export function askLine(text: string): { scope: Scope | null; question: string; message?: string } {
  const [word, rest] = firstWord(text);
  if (!word) return { scope: null, question: '' };
  const target = resolveTarget(word, { strict: true });
  const scope = scopeOfTarget(target);
  if (scope) return { scope, question: rest };
  if (splitAddress(word).space) {
    const message = target.kind === 'widget' || splitAddress(word).space === 'widget'
      ? 'A widget has no records to ask about. Ask about an app or a card: /ask expenses …'
      : target.kind === 'none' ? target.message : 'Ask about an app or a card: /ask expenses …';
    return { scope: null, question: '', message };
  }
  return { scope: null, question: text.trim() };
}

export function scopeText(scope: Scope): string {
  if (scope.kind === 'card') return `Asking about /${scope.address}`;
  if (scope.kind === 'app') return `Asking about /${scope.address} (all its records)`;
  const n = scope.works.length;
  return n ? `Asking about your canvas (${n} ${n === 1 ? 'app' : 'apps'})` : 'Your canvas has no apps yet';
}

/** `/ask <first word…>`: the member's cards and apps, by their names (Enter still asks what was typed). */
export function askCompletions(rest: string): Completion[] {
  if (/\s/.test(rest.trimStart())) return [];
  const { space, name: query } = splitAddress(rest);
  if (!query && !space) return [];
  const form = (kind: string, name: string) => (space ? `${kind}:${name}` : name);
  const cards = space && space !== 'card' ? [] : findCards(query).filter((card) => card.slug !== 'app').slice(0, 5).map((card) => ({
    badge: card.number, title: form('card', cardAddress(card)), detail: `${card.title} · ask about this card`, value: `/ask ${form('card', cardAddress(card))} `,
  }));
  const apps = (space && space !== 'app') || query.includes('.') ? [] : findApps(query).slice(0, 3).map((app) => ({
    badge: app.number, title: form('app', app.slug || app.number), detail: `${app.title} · ask about all its records`, value: `/ask ${form('app', app.slug || app.number)} `,
  }));
  return [...cards, ...apps].slice(0, 8);
}

let scope: Scope | null = null;
let thread: HTMLElement | null = null;
let chip: HTMLElement | null = null;
let input: HTMLInputElement | null = null;
let busy = false;
/** Questions sent while one is being answered: asked next, in order, each with the scope it was asked in. */
const queued: { text: string; scope: Scope }[] = [];
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

function referenceItem(document_: Document, reference: Reference, receipts: Receipt[]): HTMLElement {
  const receipt = receipts.find((r) => r.record_id === reference.record_id) ?? {};
  const text = (key: string) => String(receipt[key] ?? '');
  const open = el(document_, 'button', { type: 'button', class: 'cv-ask-ref', title: 'Show its app on the canvas' }, reference.label || reference.record_id.slice(0, 8));
  open.addEventListener('click', () => {
    const card = workCard(reference.work_id, 'records') ?? workCard(reference.work_id, 'ledger') ?? workCard(reference.work_id);
    if (card) focusCard(card);
  });
  return el(document_, 'li', {}, open,
    el(document_, 'details', { class: 'cv-ask-receipt' },
      el(document_, 'summary', {}, 'Receipt'),
      el(document_, 'dl', {},
        ...([['Record', 'record_id'], ['Version', 'version'], ['Commit', 'commit_sequence'], ['Committed', 'committed_at'], ['Publication', 'publication_id'], ['Definition', 'definition_digest']] as const)
          .map(([label, key]) => el(document_, 'div', {}, el(document_, 'dt', {}, label), el(document_, 'dd', {}, el(document_, 'code', {}, text(key))))))));
}

/** Every record the answer used, each with its receipt; past the first twelve under "n more records". */
function referenceList(document_: Document, references: Reference[], receipts: Receipt[]): HTMLElement {
  const list = el(document_, 'ul', { class: 'cv-ask-refs', 'aria-label': 'Records this answer used' });
  for (const reference of references.slice(0, SHOWN_REFERENCES)) list.append(referenceItem(document_, reference, receipts));
  const rest = references.slice(SHOWN_REFERENCES);
  if (rest.length) {
    list.append(el(document_, 'li', { class: 'cv-ask-more' },
      el(document_, 'details', {},
        el(document_, 'summary', {}, `${rest.length} more ${rest.length === 1 ? 'record' : 'records'}`),
        el(document_, 'ul', { class: 'cv-ask-refs' }, ...rest.map((reference) => referenceItem(document_, reference, receipts))))));
  }
  return list;
}

/**
 * Sends `question` with `explicit` scope, else the current one; the answer
 * joins the thread. A question asked while one is being answered is queued
 * (and says so), then asked in turn: none is dropped.
 */
export async function ask(question: string, explicit: Scope | null = null): Promise<void> {
  const document_ = document;
  const text = question.trim().replace(/^\?\s*/, '');
  if (!text) return;
  const current = explicit ?? scope ?? currentScope();
  if (busy) {
    queued.push({ text, scope: current });
    entry(document_, 'note', 'Still answering the previous question; this one is next.');
    return;
  }
  entry(document_, 'you', el(document_, 'span', { class: 'cv-ask-scope-mini' }, current.kind === 'canvas' ? 'canvas' : `/${current.address}`), text);
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
    // Every record a computed answer counted (up to 12 apps of 200 records).
    const references = Array.isArray(body.references) ? (body.references as Reference[]).filter((r) => typeof r?.record_id === 'string').slice(0, MAX_WORKS * 200) : [];
    const receipts = Array.isArray(body.receipts) ? (body.receipts as Receipt[]) : [];
    entry(document_, 'forma', el(document_, 'p', {}, body.text), ...(references.length ? [referenceList(document_, references, receipts)] : []));
  } finally {
    busy = false;
    const next = queued.shift();
    if (next) void ask(next.text, next.scope);
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
