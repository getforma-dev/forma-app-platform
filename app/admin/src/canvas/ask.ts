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
//
// APP-123: `@` mentions scope a question (`? @expenses.total why …`,
// `@app:expenses`, `@card:expenses.ledger`, a personal shortcut), through
// the same resolver; the scope chip is removable (×), and removing it scopes
// the conversation to the canvas. Each answer has **Pin to canvas** (a
// read-only answer card, stored per member, that never runs again;
// `placePinnedAnswer`) and, when its records come from one list, **Make it
// a card** (a Shape preview of a new card over the answer's records;
// nothing is added without a click).
//
// APP-124: a folder is a scope too (the folder's **Ask** button, or the
// selected folder card): the question goes to `POST /v1/boxes/{box}/ask`,
// which reads the folder's tables read-only and answers exactly `{text,
// references}`, each reference a row with its file (name, SHA-256), sheet
// and row number, from the server. A folder answer has no Pin or Make it a
// card (those keep record receipts); **Make a card** is on the folder.
import { activeCanvasItem, boardWorks, focusCard, placeUtilityCard, workCard } from './board';
import { answeredLine, newCardId, pinnedAnswer, pinRequest, storeCard, type Answer, type StoredCard } from './cards';
import { selectedCanvasItem } from './controls';
import { cardAddress, findCardRow, findCards, findApps, type CardRow } from './library';
import { onPaneTab, openPane } from './pane';
import { el, isUuid } from './render';
import { addressOfItem, addressOfWork, previewNewCard, removeButton, resolveTarget, type Target } from './shape';
import type { Completion } from './slash';
import { firstWord, MAX_RECORD_SET, mentionsOf, splitAddress } from './talk-view';
// After the board's modules: `box-api` must not be evaluated first (an import cycle through `render`).
import { contentUrl, folderAnswer, type RowReference } from './box-api';
import { openViewer } from './box-viewer';

export const ASK_URL = '/v1/ask';
const MAX_WORKS = 12;
/** References shown at once; the rest under "n more". */
const SHOWN_REFERENCES = 12;

type ScopeWork = { app_id: string; work_id: string; address: string; collection?: string };
/** A card (one work, its collection when it has one), an app (one work), the canvas (every app on it), or a folder (APP-124: its tables). */
export type Scope = { kind: 'card' | 'app' | 'canvas' | 'folder'; address: string; works: ScopeWork[]; box_id?: string };

/** APP-124: a folder's scope: its tables, read by the folder's own route. `name` is what the chip shows. */
export function folderScope(boxId: string, name: string): Scope | null {
  return isUuid(boxId) ? { kind: 'folder', address: name.slice(0, 60) || 'Folder', works: [], box_id: boxId } : null;
}

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
  // APP-124: a folder card asks about the folder's tables.
  const folder = item.matches?.('.bx-card[data-box]') ? item : item.querySelector?.<HTMLElement>('.bx-card[data-box]');
  if (folder?.dataset.box) return folderScope(folder.dataset.box, folder.querySelector('.bx-title')?.textContent ?? '');
  const address = addressOfItem(item);
  if (!address) return null;
  const target = resolveTarget(address);
  const row = findCardRow(item.dataset.instanceId ?? '');
  if (target.kind === 'card' && row) return scopeOfTarget({ kind: 'card', card: row, address: cardAddress(row) }, item.querySelector<HTMLElement>('.cv-card') ?? item);
  return scopeOfTarget(target);
}

/** The canvas: every app on it (at most twelve). */
export function canvasScope(): Scope {
  const works = boardWorks().slice(0, MAX_WORKS).map((work) => ({ app_id: work.app, work_id: work.work, address: addressOfWork(work.app, work.work) }));
  return { kind: 'canvas', address: '', works };
}

/** The selected card, else the canvas. */
export function currentScope(): Scope {
  return scopeOfItem(selectedCanvasItem() ?? null) ?? canvasScope();
}

/**
 * APP-123: the scope an `@` mention names (`expenses`, `expenses.total`,
 * `app:expenses`, `card:expenses.ledger`, `3.2`, a personal shortcut), in
 * the resolver's strict mode; else why not.
 */
export function mentionScope(mention: string): Scope | string {
  const target = resolveTarget(mention, { strict: true });
  const scope = scopeOfTarget(target);
  if (scope) return scope;
  if (target.kind === 'widget' || splitAddress(mention).space === 'widget') return `@${mention} is a widget: it has no records to ask about. Mention an app or a card.`;
  return target.kind === 'none' ? `@${mention}: ${target.message}` : 'Use @ followed by an app or card address to mention it.';
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
      ? 'A widget has no records to ask about. Choose an app or a card from the Library to ask about.'
      : target.kind === 'none' ? target.message : 'Choose an app or a card from the Library to ask about.';
    return { scope: null, question: '', message };
  }
  return { scope: null, question: text.trim() };
}

export function scopeText(scope: Scope): string {
  if (scope.kind === 'folder') return `Asking about 📁 ${scope.address} (its tables)`;
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
let chipText: HTMLElement | null = null;
let chipRemove: HTMLElement | null = null;
let input: HTMLInputElement | null = null;
let busy = false;
/** Questions sent while one is being answered: asked next, in order, each with the scope it was asked in. */
const queued: { text: string; scope: Scope }[] = [];
/** Set by `openAsk`: the next showing keeps its scope instead of reading the selection. */
let pinned = false;

const REFUSALS: Record<string, string> = {
  composer_unavailable: 'That question needs the AI model, and none is configured here. Try: “where are the red invoices”, “how many”, “what’s the total”.',
  could_not_answer: 'The answer came back in the wrong shape, so it was not shown. Nothing changed.',
  folder_unavailable: 'That question needs the AI model, and none is configured here. Nothing changed.',
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
  let text = question.trim().replace(/^\?\s*/, '');
  if (!text) return;
  let current = explicit ?? scope ?? currentScope();
  // APP-123: an `@` mention scopes this question and the chip (removable).
  const { mentions, question: rest } = mentionsOf(text);
  if (mentions.length) {
    if (mentions.length > 1) {
      entry(document_, 'note', 'Ask about one app or card at a time: keep one @ mention.');
      return;
    }
    const mentioned = mentionScope(mentions[0] ?? '');
    if (typeof mentioned === 'string') {
      entry(document_, 'note', mentioned);
      return;
    }
    scope = mentioned;
    current = mentioned;
    showScope();
    text = rest;
    if (!text) {
      entry(document_, 'note', `Now ask your question about /${mentioned.address}.`);
      return;
    }
  }
  if (busy) {
    queued.push({ text, scope: current });
    entry(document_, 'note', 'Still answering the previous question; this one is next.');
    return;
  }
  entry(document_, 'you', el(document_, 'span', { class: 'cv-ask-scope-mini' }, current.kind === 'canvas' ? 'canvas' : current.kind === 'folder' ? `📁 ${current.address}` : `/${current.address}`), text);
  if (current.kind === 'folder') {
    busy = true;
    try {
      await askFolder(document_, current, text);
    } finally {
      busy = false;
      const next = queued.shift();
      if (next) void ask(next.text, next.scope);
    }
    return;
  }
  if (!current.works.length) {
    entry(document_, 'note', 'Create an app from the bar or place one from the Library (L), then ask about it.');
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
    const answer: Answer = {
      question: text, text: body.text, scope: { kind: current.kind, address: current.address },
      references: Array.isArray(body.references) ? (body.references as Answer['references']) : [],
      receipts: Array.isArray(body.receipts) ? (body.receipts as Answer['receipts']) : [],
    };
    entry(document_, 'forma', el(document_, 'p', {}, body.text), ...(references.length ? [referenceList(document_, references, receipts)] : []), answerActions(document_, answer));
  } finally {
    busy = false;
    const next = queued.shift();
    if (next) void ask(next.text, next.scope);
  }
}

/** One row a folder answer used: the file and row, its provenance under "Receipt", and "Open file". */
function rowItem(document_: Document, reference: RowReference, boxId: string): HTMLElement {
  const where = `${reference.file}${reference.sheet ? ` · ${reference.sheet}` : ''} · row ${reference.row}`;
  const open = el(document_, 'button', { type: 'button', class: 'cv-ask-ref', title: 'Open the file' }, reference.label ? `${where}: ${reference.label}` : where);
  open.addEventListener('click', () => openViewer({
    file_id: reference.file_id, name: reference.file, kind: /\.xlsx$/i.test(reference.file) ? 'xlsx' : 'csv', media_type: '', bytes: 0,
    sha256: reference.sha256, created_at: '', content_url: contentUrl(boxId, reference.file_id), data: null,
  }, boxId));
  return el(document_, 'li', {}, open,
    el(document_, 'details', { class: 'cv-ask-receipt' },
      el(document_, 'summary', {}, 'Receipt'),
      el(document_, 'dl', {},
        ...([['File', reference.file], ['SHA-256', reference.sha256], ['Sheet', reference.sheet ?? '—'], ['Row', `${reference.row} (under the header)`]] as const)
          .map(([label, value]) => el(document_, 'div', {}, el(document_, 'dt', {}, label), el(document_, 'dd', {}, el(document_, 'code', {}, value)))))));
}

/** APP-124: a question about a folder's tables: `POST /v1/boxes/{box}/ask`; the answer and every row it used. */
async function askFolder(document_: Document, current: Scope, text: string): Promise<void> {
  const boxId = current.box_id ?? '';
  const pending = entry(document_, 'note', 'Reading the folder’s tables…');
  if (!isUuid(boxId)) {
    pending.textContent = 'That folder is not available to you.';
    return;
  }
  let response: Response;
  try {
    response = await fetch(`/v1/boxes/${boxId}/ask`, {
      method: 'POST', credentials: 'same-origin', cache: 'no-store',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ question: text }),
    });
  } catch {
    pending.textContent = 'The question could not be sent. Check your connection.';
    return;
  }
  const body = (await response.json().catch(() => null)) as unknown;
  if (response.status === 404) {
    pending.textContent = 'That folder is not available to you.';
    return;
  }
  const answer = response.ok ? folderAnswer(body) : null;
  if (!answer) {
    const code = typeof (body as { code?: unknown } | null)?.code === 'string' ? String((body as { code: string }).code) : '';
    pending.textContent = (code === 'composer_unavailable' ? REFUSALS.folder_unavailable : REFUSALS[code]) ?? 'Ask could not answer right now. Nothing changed.';
    return;
  }
  pending.remove();
  const list = el(document_, 'ul', { class: 'cv-ask-refs', 'aria-label': 'Rows this answer used' });
  for (const reference of answer.references.slice(0, SHOWN_REFERENCES)) list.append(rowItem(document_, reference, boxId));
  const rest = answer.references.slice(SHOWN_REFERENCES);
  if (rest.length) {
    list.append(el(document_, 'li', { class: 'cv-ask-more' },
      el(document_, 'details', {},
        el(document_, 'summary', {}, `${rest.length} more ${rest.length === 1 ? 'row' : 'rows'}`),
        el(document_, 'ul', { class: 'cv-ask-refs' }, ...rest.map((reference) => rowItem(document_, reference, boxId))))));
  }
  entry(document_, 'forma', el(document_, 'p', {}, answer.text), ...(answer.references.length ? [list] : []));
}

function showScope(): void {
  const shown = scope ?? currentScope();
  if (chipText) chipText.textContent = scopeText(shown);
  // The chip is removable when it names an app or a card: removing it asks about the canvas.
  if (chipRemove) chipRemove.hidden = shown.kind === 'canvas';
}

/** The Ask tab's scope now (null until it is first opened or a mention sets it). */
export function askScope(): Scope | null {
  return scope;
}

/** × on the scope chip: the conversation is about the canvas again. */
export function clearScope(): void {
  scope = canvasScope();
  showScope();
}

// --- APP-123: Pin to canvas, Make it a card ---

const answerOrigin = (answer: Answer) => (answer.scope.kind === 'canvas' ? 'your canvas' : `/${answer.scope.address}`);

/** What **Make it a card** builds on: the answer's records when they come from one list (one work, one collection). */
export function recordSet(answer: Answer): { app: string; work: string; collection: string; records: string[]; address: string } | string {
  const receipts = answer.receipts.filter((r) => typeof r.record_id === 'string');
  if (!receipts.length) return 'This answer used no records, so there is nothing to make a card from.';
  const works = new Set(receipts.map((r) => `${String(r.app_id)}/${String(r.work_id)}/${String(r.collection_id)}`));
  if (works.size > 1) return 'This answer’s records come from more than one list. Ask about one card using its address, then make it a card.';
  if (receipts.length > MAX_RECORD_SET) return `A card holds at most ${MAX_RECORD_SET} records of an answer.`;
  const [first] = receipts;
  const [app, work, collection] = [String(first?.app_id ?? ''), String(first?.work_id ?? ''), String(first?.collection_id ?? '')];
  if (!isUuid(app) || !isUuid(work) || !isUuid(collection)) return 'This answer’s records cannot be shown as a card.';
  const reference = answer.references.find((r) => r.work_id === work);
  const address = answer.scope.kind === 'canvas' || !answer.scope.address ? (reference?.address || addressOfWork(app, work)) : answer.scope.address;
  return { app, work, collection, records: [...new Set(receipts.map((r) => String(r.record_id)))], address };
}

/** A card's title from a question: plain, at most 80 characters. */
export function titleOf(question: string): string {
  const plain = question.replace(/[<>{}]/g, '').replace(/\s+/g, ' ').trim();
  return plain.length > 80 ? `${plain.slice(0, 79)}…` : plain || 'Answer';
}

function answerActions(document_: Document, answer: Answer): HTMLElement {
  const pin = el(document_, 'button', { type: 'button', class: 'cv-secondary cv-ask-pin', title: 'Keep this answer on the canvas; it does not run again' }, 'Pin to canvas');
  pin.addEventListener('click', () => {
    pin.setAttribute('disabled', '');
    void storeCard(pinRequest(newCardId(), answer)).then((stored) => {
      if (typeof stored === 'string') {
        pin.removeAttribute('disabled');
        entry(document_, 'note', stored);
        return;
      }
      pin.textContent = 'Pinned';
      placePinnedAnswer(stored);
    });
  });
  const actions = el(document_, 'p', { class: 'cv-ask-actions' }, pin);
  const set = recordSet(answer);
  if (typeof set !== 'string') {
    const make = el(document_, 'button', { type: 'button', class: 'cv-secondary cv-ask-make', title: 'Preview a new card of these records; nothing is added until you click Add as new card' }, 'Make it a card');
    make.addEventListener('click', () => {
      void previewNewCard({ app: set.app, work: set.work, collection: set.collection, address: set.address, title: titleOf(answer.question), view: { records: set.records } });
    });
    actions.append(make);
  }
  return actions;
}

/**
 * APP-123: a pinned answer on the canvas: "As answered on <date>", the
 * question, the text and every record it used with its receipt. Read-only:
 * it never runs again, and its text is what Ask said then (never derived
 * again). Its maker's **Remove** deletes it (audited).
 */
export function placePinnedAnswer(card: StoredCard): boolean {
  const answer = pinnedAnswer(card);
  if (!answer) return false;
  const document_ = document;
  const placed = `pin-${card.card_id}`;
  const references = answer.references.filter((r) => typeof r.record_id === 'string').slice(0, MAX_WORKS * 200) as Reference[];
  const body = el(document_, 'section', { class: 'cv-pinned-answer', 'aria-label': 'Pinned answer' },
    el(document_, 'p', { class: 'cv-shaped-from' }, answeredLine(card, answerOrigin(answer))),
    el(document_, 'p', { class: 'cv-pinned-question' }, `“${answer.question}”`),
    el(document_, 'p', { class: 'cv-pinned-text' }, answer.text),
    ...(references.length ? [referenceList(document_, references, answer.receipts)] : []));
  if (card.mine) body.append(el(document_, 'p', { class: 'cv-made-actions' }, removeButton(document_, card, placed, (message) => entry(document_, 'note', message))));
  placeUtilityCard(placed, card.title, body);
  return true;
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
  chipText = el(document_, 'span', { class: 'cv-ask-chip-text' }, '');
  chipRemove = el(document_, 'button', { type: 'button', class: 'cv-ask-chip-x', 'aria-label': 'Remove the scope: ask about the canvas', title: 'Ask about the canvas instead' }, '×');
  chipRemove.addEventListener('click', () => {
    clearScope();
    input?.focus({ preventScroll: true });
  });
  chip = el(document_, 'p', { class: 'cv-ask-chip', role: 'status' }, chipText, chipRemove);
  thread = el(document_, 'ol', { class: 'cv-ask-thread', 'aria-live': 'polite', 'aria-label': 'Questions and answers' });
  input = el(document_, 'input', { type: 'text', class: 'cv-ask-input', maxlength: '500', autocomplete: 'off', placeholder: 'Ask about your data (use @ followed by an app or card address)', 'aria-label': 'Your question' });
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
