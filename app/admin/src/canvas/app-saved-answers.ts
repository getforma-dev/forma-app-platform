// Saved answers are snapshots filed under their app. Placement is explicit and
// temporary; putting a snapshot away never deletes its durable saved answer.
import { askReply } from '../ask/client';
import type { AppChatSnapshot } from './app-chat';
import { placePinnedAnswer, recordSet, titleOf } from './ask';
import { removeUtilityItem } from './board';
import { answeredLine, cardInventory, newCardId, onStoredCardChange, pinnedAnswer, pinRequest, removeCard, storeCard, type Answer, type StoredCard } from './cards';
import { isUuid } from './ids';
import { el } from './render';
import { addressOfWork, previewNewCard } from './shape';

type Owner = {app: string; work: string; title: string};
export type AppAnswerActionsOptions = Owner & {onSaved?: (card: StoredCard) => void};
export type AppSavedAnswersOptions = Owner & {onOpenApp?: () => void};
export type AppSavedAnswersController = {show(): void; hide(): void; dispose(): void; refresh(): void};
type SavedAnswer = {card: StoredCard; question: string; text: string; source: string; citations: {label: string; receipt: string | Record<string, unknown>}[]};
const SOURCE_PAGE_SIZE = 50;

const object = (value: unknown): value is Record<string, unknown> => !!value && typeof value === 'object' && !Array.isArray(value);
const digest = (value: unknown): value is string => typeof value === 'string' && /^sha256:[a-f0-9]{64}$/.test(value);
const belongs = (card: StoredCard, owner: Owner) => card.kind === 'answer' && card.app_id === owner.app && card.work_id === owner.work;

/** Bind each Save action to the exact app/file that produced the answer. */
function saveRequest(id: string, snapshot: AppChatSnapshot, owner: Owner): {body: Record<string, unknown>; records?: Answer} | null {
  if (!isUuid(owner.app) || !isUuid(owner.work)) return null;
  const scope = {kind: 'app' as const, address: addressOfWork(owner.app, owner.work)};
  if (snapshot.kind === 'records') {
    if (snapshot.scope.works.length !== 1 || snapshot.scope.works[0]?.app_id !== owner.app || snapshot.scope.works[0]?.work_id !== owner.work) return null;
    const reply = askReply(snapshot.reply, snapshot.scope);
    if (!reply) return null;
    const records: Answer = {question: snapshot.question, text: reply.text, scope, references: reply.references, receipts: reply.receipts};
    return {body: {...pinRequest(id, records), app_id: owner.app, work_id: owner.work}, records};
  }
  if (snapshot.source.app !== owner.app || snapshot.source.work !== owner.work
    || !isUuid(snapshot.source.boxId) || !isUuid(snapshot.source.fileId)
    || snapshot.reply.references.some(ref => ref.file_id !== snapshot.source.fileId || !digest(ref.sha256))) return null;
  return {body: {
    card_id: id, kind: 'answer', app_id: owner.app, work_id: owner.work,
    question: snapshot.question, text: snapshot.reply.text, scope, references: [], receipts: [],
    file_source: {box_id: snapshot.source.boxId, file_id: snapshot.source.fileId},
    file_references: snapshot.reply.references,
  }};
}

/** Saving files the answer; making a live record card remains a separate preview. */
export function renderAppAnswerActions(document_: Document, snapshot: AppChatSnapshot, options: AppAnswerActionsOptions): HTMLElement {
  const save = el(document_, 'button', {type: 'button', class: 'cv-secondary cv-app-answer-save'}, 'Save answer');
  const status = el(document_, 'span', {class: 'cv-app-answer-status', role: 'status'});
  const root = el(document_, 'div', {class: 'cv-app-answer-actions'}, save, status);
  // One ID per answer action: an uncertain retry cannot create another copy.
  const request = saveRequest(newCardId(), snapshot, options);
  let pending = false, saved = false;
  if (!request) {save.disabled = true;status.textContent = 'This answer cannot be saved to this app.';return root;}
  save.addEventListener('click', () => {
    if (pending || saved || !root.isConnected) return;
    pending = true;save.disabled = true;status.textContent = 'Saving…';
    void storeCard(request.body).then(card => {
      if (typeof card === 'string') {status.textContent = card;return;}
      if (!belongs(card, options) || card.card_id !== request.body.card_id) {status.textContent = 'The saved answer could not be confirmed. Refresh Saved answers.';return;}
      saved = true;save.textContent = 'Saved';status.textContent = 'Filed in this app’s Saved answers.';
      if (root.isConnected) options.onSaved?.(card);
    }).catch(() => {status.textContent = 'The save could not be confirmed. Try again.';})
      .finally(() => {pending = false;save.disabled = saved;});
  });
  if (request.records) {
    const set = recordSet(request.records);
    if (typeof set !== 'string' && set.app === options.app && set.work === options.work) {
      const make = el(document_, 'button', {type: 'button', class: 'cv-secondary cv-app-answer-make', title: 'Preview a live card from these records'}, 'Make it a card');
      make.addEventListener('click', () => {
        if (!root.isConnected) return;
        void previewNewCard({...set, title: titleOf(snapshot.question), view: {records: set.records}}, message => {status.textContent = message;});
      });
      root.append(make);
    }
  }
  return root;
}

/** Inventory data is authorized by the server and checked again before rendering. */
function savedAnswer(card: StoredCard, owner: Owner): SavedAnswer | null {
  if (!belongs(card, owner)) return null;
  const answer = pinnedAnswer(card);
  if (!answer || answer.question.length > 1000 || answer.text.length > 6000) return null;
  const spec = object(card.spec) ? card.spec : null;
  const file = spec?.file_source;
  if (file !== undefined) {
    if (!object(file) || !isUuid(String(file.box_id)) || !isUuid(String(file.file_id))
      || typeof file.name !== 'string' || !digest(file.sha256) || !Array.isArray(spec?.file_references) || spec.file_references.length > 200) return null;
    const citations: SavedAnswer['citations'] = [];
    for (const ref of spec.file_references) {
      if (!object(ref) || ref.file_id !== file.file_id || ref.sha256 !== file.sha256 || !Number.isSafeInteger(ref.row) || Number(ref.row) < 1
        || (ref.sheet !== null && typeof ref.sheet !== 'string')) return null;
      citations.push({label: `${file.name}${ref.sheet ? ` · ${String(ref.sheet)}` : ''} · row ${ref.row}`,
        receipt: `File: ${file.file_id}\nSHA-256: ${file.sha256}\nRow under the header: ${ref.row}`});
    }
    return {card, question: answer.question, text: answer.text, source: file.name, citations};
  }
  const reply = askReply({text: answer.text, references: answer.references, receipts: answer.receipts},
    {kind: 'app', address: card.address, works: [{app_id: owner.app, work_id: owner.work, address: card.address}]});
  if (!reply) return null;
  const receipts = new Map(reply.receipts.map(receipt => [receipt.reference, receipt]));
  return {card, question: answer.question, text: answer.text, source: 'App records',
    citations: reply.references.map(ref => ({label: ref.label, receipt: receipts.get(ref.ref)!}))};
}

export function mountAppSavedAnswers(host: HTMLElement, options: AppSavedAnswersOptions): AppSavedAnswersController {
  const document_ = host.ownerDocument;
  const status = el(document_, 'p', {class: 'cv-app-saved-status', role: 'status'});
  const list = el(document_, 'ol', {class: 'cv-app-saved-list', 'aria-label': `Saved answers for ${options.title}`});
  const refresh = el(document_, 'button', {type: 'button', class: 'cv-secondary'}, 'Refresh');
  const putAway = el(document_, 'button', {type: 'button', class: 'cv-secondary'}, 'Put all away');
  const toolbar = el(document_, 'div', {class: 'cv-app-saved-toolbar'}, refresh, putAway);
  if (options.onOpenApp) {
    const open = el(document_, 'button', {type: 'button', class: 'cv-secondary'}, 'Open app');
    open.addEventListener('click', () => {if (current()) options.onOpenApp?.();});toolbar.append(open);
  }
  host.replaceChildren(el(document_, 'section', {class: 'cv-app-saved'},
    el(document_, 'p', {class: 'cv-app-saved-hint'}, 'Saved answers are snapshots. Place one on the canvas when you need it; closing it files it here.'), toolbar, status, list));
  let alive = true, visible = false, generation = 0, loading = false;
  let entries: SavedAnswer[] = [];
  const busy = new Set<string>();
  const current = (ticket = generation) => alive && visible && host.isConnected && ticket === generation;
  const utility = (id: string) => isUuid(id) ? document_.querySelector<HTMLElement>(`[data-utility="utility-pin-${id}"]`) : null;
  const putOneAway = (id: string) => {const item = utility(id);return item ? removeUtilityItem(item) : false;};
  const controls = () => {refresh.disabled = loading;putAway.disabled = loading || !entries.length;};
  const render = () => {
    list.replaceChildren(...entries.map(({card, question, text, source, citations}) => {
      const place = el(document_, 'button', {type: 'button', class: 'cv-secondary'}, 'Place on canvas');
      const away = el(document_, 'button', {type: 'button', class: 'cv-secondary'}, 'Put away');
      place.disabled = away.disabled = busy.has(card.card_id);
      place.addEventListener('click', () => {void placeAnswer(card.card_id);});
      away.addEventListener('click', () => {if (current()) {putOneAway(card.card_id);status.textContent = 'Filed here. The saved answer is kept.';}});
      const actions = el(document_, 'div', {class: 'cv-app-saved-actions'}, place, away);
      if (card.mine) {
        const remove = el(document_, 'button', {type: 'button', class: 'cv-secondary cv-app-saved-delete'}, 'Delete saved answer');
        remove.disabled = busy.has(card.card_id);
        remove.addEventListener('click', () => {void deleteAnswer(card.card_id);});actions.append(remove);
      }
      const details = el(document_, 'details', {class: 'cv-app-saved-answer'}, el(document_, 'summary', {}, 'Read saved answer'), el(document_, 'p', {}, text));
      if (citations.length) {
        const pageBody = el(document_, 'div', {class: 'cv-app-saved-source-page'});
        const range = el(document_, 'span', {role: 'status'});
        const previous = el(document_, 'button', {type: 'button', class: 'cv-secondary'}, 'Previous sources');
        const next = el(document_, 'button', {type: 'button', class: 'cv-secondary'}, 'Next sources');
        let page = 0;
        const renderPage = () => {
          const start = page * SOURCE_PAGE_SIZE, end = Math.min(start + SOURCE_PAGE_SIZE, citations.length);
          pageBody.replaceChildren(...citations.slice(start, end).map(citation => el(document_, 'div', {}, el(document_, 'p', {}, citation.label),
            el(document_, 'pre', {}, typeof citation.receipt === 'string' ? citation.receipt : JSON.stringify(citation.receipt, null, 2)))));
          previous.disabled = page === 0; next.disabled = end === citations.length;
          range.textContent = `${start + 1}–${end} of ${citations.length}`;
        };
        previous.addEventListener('click', () => {if (current() && page > 0) {page--;renderPage();}});
        next.addEventListener('click', () => {if (current() && (page + 1) * SOURCE_PAGE_SIZE < citations.length) {page++;renderPage();}});
        renderPage();
        details.append(el(document_, 'details', {class: 'cv-app-saved-receipts'}, el(document_, 'summary', {}, `Sources (${citations.length})`), pageBody,
          ...(citations.length > SOURCE_PAGE_SIZE ? [el(document_, 'div', {class: 'cv-app-saved-actions'}, previous, range, next)] : [])));
      }
      return el(document_, 'li', {class: 'cv-app-saved-entry', 'data-saved-answer': card.card_id},
        el(document_, 'h4', {}, question), el(document_, 'p', {class: 'cv-app-saved-meta'}, answeredLine(card, source)), details, actions);
    }));
    controls();
  };
  const load = async () => {
    if (!current()) return;
    const ticket = ++generation;loading = true;entries = [];list.replaceChildren();controls();status.textContent = 'Loading saved answers…';
    try {
      const result = await cardInventory();if (!current(ticket)) return;
      if (!result.ok) {status.textContent = 'Saved answers could not be refreshed. Try Refresh.';return;}
      entries = result.cards.flatMap(card => {const answer = savedAnswer(card, options);return answer ? [answer] : [];});
      render();status.textContent = entries.length ? `${entries.length} saved ${entries.length === 1 ? 'answer' : 'answers'}.` : 'No saved answers yet. Save an answer from this app’s Ask tab.';
    } catch {if (current(ticket)) status.textContent = 'Saved answers could not be refreshed. Try Refresh.';}
    finally {if (current(ticket)) {loading = false;controls();}}
  };
  const placeAnswer = async (id: string) => {
    if (!current() || busy.has(id) || !entries.some(entry => entry.card.card_id === id)) return;
    const ticket = generation;busy.add(id);render();status.textContent = 'Checking access…';
    try {
      // Never reveal a snapshot using metadata retained from an earlier listing.
      const result = await cardInventory();if (!current(ticket)) return;
      const card = result.ok ? result.cards.find(card => card.card_id === id && savedAnswer(card, options)) : null;
      if (!card) {
        entries = result.ok ? entries.filter(entry => entry.card.card_id !== id) : [];
        if (result.ok) putOneAway(id);
        render();status.textContent = 'That saved answer is no longer available. Refresh the list.';return;
      }
      status.textContent = placePinnedAnswer(card) ? 'Answer placed on the canvas. Closing it keeps it saved here.' : 'That answer could not be placed.';
    } catch {if (current(ticket)) status.textContent = 'That answer could not be placed. Try again.';}
    finally {busy.delete(id);if (current()) render();}
  };
  const deleteAnswer = async (id: string) => {
    if (!current() || busy.has(id) || !entries.some(entry => entry.card.card_id === id && entry.card.mine)) return;
    busy.add(id);render();status.textContent = 'Deleting saved answer…';
    try {
      const result = await removeCard(id);
      if (result !== true) {if (current()) status.textContent = result;return;}
      putOneAway(id);
      if (current()) {await load();if (current()) status.textContent = 'Saved answer deleted.';}
    } catch {if (current()) status.textContent = 'The deletion could not be confirmed. Refresh the list.';}
    finally {busy.delete(id);if (current()) render();}
  };
  refresh.addEventListener('click', () => {void load();});
  putAway.addEventListener('click', () => {
    if (!current() || loading) return;
    for (const entry of entries) putOneAway(entry.card.card_id);
    status.textContent = 'This app’s answers are off the canvas and still saved here.';
  });
  const unlisten = onStoredCardChange(change => {
    const relevant = 'card' in change ? belongs(change.card, options) : entries.some(entry => entry.card.card_id === change.removed);
    if (relevant && current()) void load();
  });
  controls();
  return {
    show() {if (!alive || visible) return;visible = true;void load();},
    hide() {visible = false;generation++;loading = false;entries = [];list.replaceChildren();status.textContent = '';controls();},
    dispose() {if (!alive) return;alive = false;visible = false;generation++;entries = [];unlisten();host.replaceChildren();},
    refresh() {if (current()) void load();},
  };
}
