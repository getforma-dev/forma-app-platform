// An app-owned conversation and explicit, confirmed data-change mode. Neither
// enters the builder or global action router; all requests name this saved work.
import { ASK_REFUSALS, requestAsk, type AskReply, type RecordScope } from '../ask/client';
import { mountAppDataActions, type AppDataActions } from './app-data-actions';
import type { RowReference } from './box-api';
import { isUuid } from './ids';
import { el } from './render';

export type AppChatOptions = {
  app: string; work: string; title: string; address?: string;
  onOpenSource?: (fileId: string) => void;
  onOpenActions?: () => void;
  onDataCommitted?: (recordId: string) => void;
  onDataEnabled?: () => Promise<boolean>;
  renderAnswerActions?: (snapshot: AppChatSnapshot) => HTMLElement;
};
export type AppChatSnapshot =
  | {kind: 'records'; question: string; reply: AskReply; scope: RecordScope}
  | {kind: 'file'; question: string; reply: {text: string; references: RowReference[]}; source: {app: string; work: string; boxId: string; fileId: string; fileName: string; revisionId: string}};
export type AppChat = { show(): void; hide(): void; dispose(): void; setSource(fileId: string): void; setRecordScope(scope: RecordScope | null): void; ask(question: string): void; focus(): void };
type Source = { id: string; name: string; revision: string };
type Sources = { box: string; files: Source[] };
type Citation = { label: string; receipt: string; file?: string; href?: string };
type Answer = { text: string; citations: Citation[]; total: number; actions?: HTMLElement; citationPage?: number };
type Exchange = { question: string; context: string; label: string; answer?: Answer; error?: string };
const MAX_EXCHANGES = 12, CITATIONS_PER_PAGE = 50;
const RECORDS = 'records', CHOOSE = '';
const unavailable = 'This app is no longer available to you.';
const couldNotAnswer = 'A verified answer could not be shown. Try asking again.';
const object = (value: unknown): value is Record<string, unknown> => !!value && typeof value === 'object' && !Array.isArray(value);

function readSources(value: unknown): Sources | null {
  if (!object(value) || !isUuid(value.box_id) || !Array.isArray(value.files) || value.files.length > 20) return null;
  const files: Source[] = [], seen = new Set<string>();
  for (const file of value.files) {
    if (!object(file) || !isUuid(file.file_id) || typeof file.name !== 'string' || file.name.length > 8192 || seen.has(file.file_id)) return null;
    seen.add(file.file_id);
    files.push({id: file.file_id, name: file.name, revision: isUuid(file.revision_id) ? file.revision_id : ''});
  }
  return {box: value.box_id, files};
}

/** Reject the whole reply if any citation escapes the selected source. */
function fileAnswer(value: unknown, source: Source): {answer: Answer; reply: {text: string; references: RowReference[]}} | null {
  if (!object(value) || Object.keys(value).sort().join(',') !== 'references,text'
    || typeof value.text !== 'string' || !value.text.trim() || Array.from(value.text).length > 1500
    || !Array.isArray(value.references) || value.references.length > 200) return null;
  const citations: Citation[] = [], references: RowReference[] = [], seen = new Set<string>();
  for (const ref of value.references) {
    if (!object(ref) || ref.file_id !== source.id || typeof ref.ref !== 'string' || ref.ref.length > 100 || seen.has(ref.ref)
      || typeof ref.file !== 'string' || ref.file.length > 8192 || typeof ref.sha256 !== 'string' || !/^sha256:[a-f0-9]{64}$/.test(ref.sha256)
      || typeof ref.label !== 'string' || ref.label.length > 8192 || !Number.isSafeInteger(ref.row) || Number(ref.row) < 1
      || (ref.sheet !== null && (typeof ref.sheet !== 'string' || ref.sheet.length > 8192))) return null;
    seen.add(ref.ref);
    references.push({ref: ref.ref, file_id: source.id, file: ref.file, sha256: ref.sha256,
      sheet: ref.sheet, row: Number(ref.row), label: ref.label});
    citations.push({file: source.id,
      label: `${source.name.slice(0, 200)}${ref.sheet ? ` · ${ref.sheet.slice(0, 120)}` : ''} · row ${ref.row}`,
      receipt: `File: ${source.id}\nSHA-256: ${ref.sha256}\nRow under the header: ${ref.row}`});
  }
  return {answer: {text: value.text, citations, total: references.length}, reply: {text: value.text, references}};
}

function recordAnswer(value: AskReply): Answer {
  const receipts = new Map(value.receipts.map(receipt => [receipt.reference, receipt]));
  return {text: value.text, total: value.references.length, citations: value.references.map(ref => {
    const original = receipts.get(ref.ref)!;
    const receipt: Record<string, string | number> = {};
    for (const key of ['app_id', 'work_id', 'record_id', 'collection_id', 'publication_id', 'definition_digest', 'version', 'work_version', 'commit_sequence', 'committed_at']) {
      const part = original[key];
      if (typeof part === 'string') receipt[key] = part.slice(0, 200);
      else if (typeof part === 'number') receipt[key] = part;
    }
    return {label: ref.label.slice(0, 200) || ref.record_id.slice(0, 8), receipt: JSON.stringify(receipt, null, 2),
      href: `/work/${ref.app_id}/${ref.work_id}/records/${original.collection_id}/${ref.record_id}`};
  })};
}

export function mountAppChat(host: HTMLElement, options: AppChatOptions): AppChat {
  const doc = host.ownerDocument;
  const selector = el(doc, 'select', {class: 'cv-app-chat-context', 'aria-label': 'Question context'});
  const hint = el(doc, 'p', {class: 'cv-app-chat-scope'});
  const thread = el(doc, 'ol', {class: 'cv-app-chat-thread', 'aria-label': `Conversation about ${options.title}`});
  const status = el(doc, 'p', {class: 'cv-app-chat-status', role: 'status', 'aria-live': 'polite'});
  const input = el(doc, 'textarea', {class: 'cv-app-chat-input', rows: '2', maxlength: '500', placeholder: 'Ask about this app’s data…', 'aria-label': `Ask about ${options.title}`});
  const send = el(doc, 'button', {type: 'submit', class: 'cv-app-chat-send'}, 'Ask');
  const stop = el(doc, 'button', {type: 'button', class: 'cv-secondary cv-app-chat-stop'}, 'Stop');
  const retry = el(doc, 'button', {type: 'button', class: 'cv-secondary cv-app-chat-retry'}, 'Try again');
  const open = el(doc, 'button', {type: 'button', class: 'cv-secondary cv-app-chat-open-source'}, 'View original');
  const actions = el(doc, 'div', {class: 'cv-app-chat-actions'}, open);
  if (options.onOpenActions) {
    const button = el(doc, 'button', {type: 'button', class: 'cv-secondary'}, 'Use app actions');
    button.addEventListener('click', () => {if (authorized && visible) options.onOpenActions?.();});
    actions.append(button);
  }
  const form = el(doc, 'form', {class: 'cv-app-chat-form'}, input,
    el(doc, 'div', {class: 'cv-app-chat-submit'}, stop, send));
  const askView = el(doc, 'div', {class: 'cv-app-chat-ask-view'},
    el(doc, 'div', {class: 'cv-app-chat-context-row'}, el(doc, 'span', {}, 'Ask about'), selector),
    hint, actions, thread, status, retry, form);
  const changeView = el(doc, 'div', {class: 'cv-app-chat-change-view'}); changeView.hidden = true;
  const askMode = el(doc, 'button', {type: 'button', 'aria-pressed': 'true'}, 'Ask');
  const changeMode = el(doc, 'button', {type: 'button', 'aria-pressed': 'false'}, 'Change data');
  const modePicker = el(doc, 'div', {class: 'cv-app-chat-mode', role: 'group', 'aria-label': 'Assistant mode'}, askMode, changeMode);
  const root = el(doc, 'section', {class: 'cv-app-chat'}, modePicker, askView, changeView);
  host.replaceChildren(root);
  let mode: 'ask' | 'change' = 'ask', changes: AppDataActions | null = null;
  let alive = true, visible = false, authorized = false, loading = false, generation = 0, focusRequested = false;
  let pending: AbortController | null = null, sources: Sources | null = null, selected = CHOOSE;
  let appDataAvailable = false, contextPicked = false;
  let requestedSource: string | null = null, failed: Exchange | null = null, active: Exchange | null = null, queued: string | null = null;
  const waiting: string[] = [];
  const exchanges: Exchange[] = [];
  const address = options.address || `/${options.work}`;
  const scope: RecordScope = {kind: 'app', address, works: [{app_id: options.app, work_id: options.work, address}]};
  let recordScope = scope, recordScopeRefused = false, recordScopeRefusal = '';
  const recordsLabel = () => recordScope.kind === 'card' ? `Component data · ${recordScope.address}` : 'App data';
  const currentSource = () => sources?.files.find(file => file.id === selected);
  const current = (at: number) => alive && visible && mode === 'ask' && host.isConnected && generation === at;
  const refreshControls = () => {
    selector.disabled = !authorized || !!pending;
    input.disabled = !authorized;
    send.disabled = !authorized || !!pending || !selected || recordScopeRefused;
    stop.hidden = !pending || loading;
    retry.hidden = !!pending || (!failed && authorized);
    open.hidden = !authorized || !currentSource() || !options.onOpenSource;
    thread.hidden = !authorized;
    const file = currentSource();
    hint.textContent = file ? 'The original upload, unchanged by app edits. Answers may use a sample; view the original to see all rows.'
      : selected === RECORDS ? `This ${recordScope.kind === 'card' ? 'component’s' : 'app’s'} saved data, including confirmed changes. Original files stay unchanged.` : 'Choose which data this question should use.';
  };
  const renderThread = () => {
    thread.replaceChildren();
    if (!exchanges.length) thread.append(el(doc, 'li', {class: 'cv-app-chat-empty'},
      el(doc, 'p', {}, 'Explore your data, ask for an explanation, or find a detail.'),
      el(doc, 'p', {}, 'To add or update records, choose Change data and review the proposed change.')));
    for (const exchange of exchanges) {
      const item = el(doc, 'li', {class: 'cv-app-chat-exchange'},
        el(doc, 'p', {class: 'cv-app-chat-question'}, exchange.question),
        el(doc, 'span', {class: 'cv-app-chat-context-label'}, exchange.label));
      if (exchange.answer) {
        item.append(el(doc, 'p', {class: 'cv-app-chat-answer'}, exchange.answer.text));
        if (exchange.answer.total) {
          const answer = exchange.answer;
          const details = el(doc, 'details', {class: 'cv-app-chat-citations'}, el(doc, 'summary', {}, `Sources (${answer.total})`));
          const list = el(doc, 'ul', {class: 'cv-app-chat-citations-list'});
          const previous = el(doc, 'button', {type: 'button', class: 'cv-secondary cv-app-chat-citations-previous', 'aria-label': 'Previous sources'}, 'Previous');
          const next = el(doc, 'button', {type: 'button', class: 'cv-secondary cv-app-chat-citations-next', 'aria-label': 'Next sources'}, 'Next');
          const position = el(doc, 'span', {'aria-live': 'polite'});
          const paintPage = () => {
            const last = Math.ceil(answer.total / CITATIONS_PER_PAGE) - 1;
            answer.citationPage = Math.max(0, Math.min(answer.citationPage ?? 0, last));
            const offset = answer.citationPage * CITATIONS_PER_PAGE;
            list.replaceChildren();
            for (const citation of answer.citations.slice(offset, offset + CITATIONS_PER_PAGE)) {
              const label = citation.file && options.onOpenSource ? el(doc, 'button', {type: 'button', class: 'bx-lineage-open'}, citation.label)
                : citation.href ? el(doc, 'a', {href: citation.href, target: '_blank', rel: 'noopener', title: 'Open record in a new tab'}, citation.label)
                : el(doc, 'span', {}, citation.label);
              if (citation.file) label.addEventListener('click', () => {if (alive && authorized && visible) options.onOpenSource?.(citation.file!);});
              list.append(el(doc, 'li', {}, label, el(doc, 'details', {}, el(doc, 'summary', {}, 'Receipt'), el(doc, 'pre', {}, citation.receipt))));
            }
            previous.disabled = answer.citationPage === 0; next.disabled = answer.citationPage === last;
            position.textContent = `${offset + 1}–${Math.min(offset + CITATIONS_PER_PAGE, answer.total)} of ${answer.total}`;
          };
          previous.addEventListener('click', () => {if (alive && authorized && visible) {answer.citationPage = (answer.citationPage ?? 0) - 1; paintPage();}});
          next.addEventListener('click', () => {if (alive && authorized && visible) {answer.citationPage = (answer.citationPage ?? 0) + 1; paintPage();}});
          paintPage();
          details.append(list);
          if (answer.total > CITATIONS_PER_PAGE) details.append(el(doc, 'nav', {class: 'cv-app-chat-actions cv-app-chat-citation-pages', 'aria-label': 'Answer source pages'}, previous, position, next));
          item.append(details);
        }
        if (exchange.answer.actions) item.append(exchange.answer.actions);
      } else if (exchange.error) item.append(el(doc, 'p', {class: 'cv-app-chat-error'}, exchange.error));
      thread.append(item);
    }
    thread.scrollTop = thread.scrollHeight;
  };
  const cancel = (message = 'Stopped. You can send this question again.', preserveQueue = false) => {
    generation += 1; pending?.abort(); pending = null; loading = false;
    if (!preserveQueue) {waiting.length = 0; queued = null;}
    if (active) {active.error = message; failed = active; active = null; renderThread();}
    status.textContent = ''; refreshControls();
  };
  const deny = () => {
    authorized = false; exchanges.length = 0; sources = null; selected = CHOOSE; failed = null; active = null;
    input.value = ''; queued = null; waiting.length = 0; status.textContent = unavailable; renderThread();
  };
  const updateContexts = (next: Sources | null) => {
    const old = sources;
    if (old && JSON.stringify(old) !== JSON.stringify(next)) {exchanges.length = 0; failed = null; renderThread();}
    sources = next;
    selector.replaceChildren();
    if (appDataAvailable) selector.append(el(doc, 'option', {value: RECORDS}, recordsLabel()));
    if (next?.files.length) {
      if (next.files.length > 1) selector.append(el(doc, 'option', {value: CHOOSE}, 'Choose a source…'));
      for (const file of next.files) selector.append(el(doc, 'option', {value: file.id}, `Original · ${file.name}`));
    }
    if (!appDataAvailable) selector.append(el(doc, 'option', {value: RECORDS}, recordsLabel()));
    if (requestedSource) {
      selected = next?.files.some(file => file.id === requestedSource) ? requestedSource : CHOOSE;
      if (!selected) status.textContent = 'That source is no longer part of this app.';
      contextPicked = true;
      requestedSource = null;
    } else if (!contextPicked && appDataAvailable) {
      selected = RECORDS;
    } else if (selected !== RECORDS && !next?.files.some(file => file.id === selected)) {
      selected = contextPicked ? CHOOSE : next?.files.length === 1 ? next.files[0]!.id : next?.files.length ? CHOOSE : RECORDS;
    }
    selector.value = selected;
  };
  const getSources = async (signal: AbortSignal): Promise<Sources | null> => {
    const response = await fetch(`/v1/boxes/lineage/${options.app}/${options.work}`, {credentials: 'same-origin', cache: 'no-store', signal});
    if (response.status === 404) return null;
    if (response.status === 401 || response.status === 403) throw new Error('denied');
    if (!response.ok) throw new Error('unavailable');
    const result = readSources(await response.json());
    if (!result) throw new Error('invalid_lineage');
    return result;
  };
  const load = async () => {
    cancel(undefined, true); authorized = false; loading = true; pending = new AbortController();
    const controller = pending, at = generation;
    status.textContent = 'Checking this app’s data…'; refreshControls();
    try {
      if (!isUuid(options.app) || !isUuid(options.work)) {deny(); return;}
      const response = await fetch(`/v1/canvas/works/${options.app}/${options.work}`, {credentials: 'same-origin', cache: 'no-store', signal: controller.signal});
      if (!current(at)) return;
      if (response.status === 404 || response.status === 403 || response.status === 401) {deny(); return;}
      if (!response.ok) throw new Error('unavailable');
      const work: unknown = await response.json();
      if (!current(at)) return;
      // These are authorized collection descriptors, not a count of loaded
      // rows: an empty editable app still asks about its own data by default.
      appDataAvailable = object(work) && work.app === options.app && work.work === options.work
        && Array.isArray(work.records) && work.records.some(record => object(record) && isUuid(record.collection));
      const next = await getSources(controller.signal);
      if (!current(at)) return;
      if (sources && !next) {deny(); return;}
      authorized = true; status.textContent = ''; updateContexts(next);
      if (recordScopeRefused) {selected = CHOOSE; selector.value = CHOOSE; status.textContent = recordScopeRefusal;}
    } catch (error) {
      if (current(at)) {
        if (error instanceof Error && error.message === 'denied') deny();
        else status.textContent = 'This app’s data could not be checked. Try again.';
      }
    } finally {
      if (current(at)) {
        pending = null; loading = false; refreshControls();
        if (focusRequested && authorized) {focusRequested = false; input.focus();}
        if (queued && authorized) {
          const question = queued; queued = null;
          if (selected && !recordScopeRefused) {if (input.value === question) input.value = ''; void ask(question);}
        }
      }
    }
  };
  const ask = async (question: string, previous?: Exchange) => {
    if (!authorized || pending || !selected || !question || !visible || !alive || recordScopeRefused) return;
    const file = currentSource(), box = sources?.box;
    if (selected !== RECORDS && (!file || !box)) return;
    const exchange = previous ?? {question, context: selected, label: file ? `Original · ${file.name}` : recordsLabel()};
    if (!previous) {exchanges.push(exchange); if (exchanges.length > MAX_EXCHANGES) exchanges.shift();}
    exchange.error = undefined; active = exchange; failed = null;
    pending = new AbortController(); const controller = pending, at = ++generation;
    status.textContent = 'Reading the selected data…'; renderThread(); refreshControls();
    try {
      let answer: Answer | string;
      if (file && box) {
        const fresh = await getSources(controller.signal);
        if (!current(at)) return;
        if (fresh?.box !== box || !fresh.files.some(value => value.id === file.id && value.revision === file.revision)) {
          deny(); status.textContent = 'This source changed or is no longer available. Reopen Ask to check the app again.'; return;
        }
        const response = await fetch(`/v1/boxes/${box}/ask`, {method: 'POST', credentials: 'same-origin', cache: 'no-store', signal: controller.signal,
          headers: {'content-type': 'application/json'}, body: JSON.stringify({question, file_id: file.id})});
        const value: unknown = await response.json().catch(() => null);
        if (!current(at)) return;
        if ([401, 403, 404].includes(response.status)) {deny(); return;}
        const result = response.ok ? fileAnswer(value, file) : null;
        answer = result?.answer ?? (response.ok ? couldNotAnswer
          : ASK_REFUSALS[object(value) && typeof value.code === 'string' ? value.code : ''] ?? 'Ask is unavailable right now. Try again.');
        if (result) result.answer.actions = options.renderAnswerActions?.({kind: 'file', question, reply: result.reply,
          source: {app: options.app, work: options.work, boxId: box, fileId: file.id, fileName: file.name, revisionId: file.revision}});
      } else {
        const requestedScope = recordScope;
        const reply = await requestAsk(question, requestedScope, controller.signal);
        if (!current(at)) return;
        if (typeof reply === 'string' && reply.includes('no longer available')) {deny(); return;}
        answer = typeof reply === 'string' ? reply : recordAnswer(reply);
        if (typeof reply !== 'string' && typeof answer !== 'string') answer.actions = options.renderAnswerActions?.({kind: 'records', question, reply, scope: requestedScope});
      }
      if (!current(at)) return;
      if (typeof answer === 'string') {exchange.error = answer; failed = exchange; status.textContent = answer;}
      else {exchange.answer = answer; status.textContent = 'Answer ready.';}
    } catch (error) {
      if (current(at)) {
        if (error instanceof Error && error.message === 'denied') deny();
        else {exchange.error = 'The question could not be sent. Check your connection and try again.'; failed = exchange; status.textContent = exchange.error;}
      }
    } finally {
      if (current(at)) {
        pending = null; active = null; renderThread(); refreshControls();
        const next = waiting.shift();
        if (next && authorized && selected && !recordScopeRefused) void ask(next);
      }
    }
  };
  form.addEventListener('submit', event => {
    event.preventDefault(); const question = input.value.trim().slice(0, 500);
    if (!question || !authorized || pending || !selected || recordScopeRefused) return;
    input.value = ''; void ask(question);
  });
  form.addEventListener('keydown', event => {if (event.key !== 'Escape') event.stopPropagation();});
  stop.addEventListener('click', () => cancel());
  retry.addEventListener('click', () => {
    if (!authorized) {void load(); return;}
    if (failed && failed.context === selected) void ask(failed.question, failed);
  });
  selector.addEventListener('change', () => {cancel(); selected = selector.value; contextPicked = true; failed = null; recordScopeRefused = false; refreshControls();});
  open.addEventListener('click', () => {if (authorized && currentSource()) options.onOpenSource?.(selected);});
  const dataActions = () => changes ??= mountAppDataActions(changeView, {...options, onCommitted: options.onDataCommitted, onDataEnabled: async () => {
    appDataAvailable = true;
    if (!contextPicked) selected = RECORDS;
    return options.onDataEnabled?.() ?? true;
  }});
  const selectMode = (next: 'ask' | 'change') => {
    if (!alive || next === mode) return;
    if (mode === 'ask') cancel(); else changes?.hide();
    mode = next; askView.hidden = next !== 'ask'; changeView.hidden = next !== 'change';
    askMode.setAttribute('aria-pressed', String(next === 'ask')); changeMode.setAttribute('aria-pressed', String(next === 'change'));
    if (visible) {if (next === 'ask') {focusRequested = true; void load();} else dataActions().show();}
  };
  askMode.addEventListener('click', () => selectMode('ask'));
  changeMode.addEventListener('click', () => selectMode('change'));
  renderThread(); refreshControls(); root.hidden = true;
  return {
    show() {if (!alive || visible) return; visible = true; root.hidden = false; if (mode === 'ask') void load(); else dataActions().show();},
    hide() {if (!alive) return; visible = false; root.hidden = true; queued = null; focusRequested = false; cancel(); changes?.hide();},
    setSource(fileId) {
      if (!alive || !isUuid(fileId)) return;
      selectMode('ask');
      recordScope = scope; recordScopeRefused = false;
      requestedSource = fileId;
      if (visible) {cancel(); void load();}
    },
    setRecordScope(next) {
      if (!alive) return;
      if (next) selectMode('ask');
      if (!recordScopeRefused && ((!next && recordScope === scope) || (next && selected === RECORDS
        && next.kind === recordScope.kind && next.address === recordScope.address && next.works.length === 1
        && next.works[0]?.app_id === recordScope.works[0]?.app_id && next.works[0]?.work_id === recordScope.works[0]?.work_id
        && next.works[0]?.collection === recordScope.works[0]?.collection && next.works[0]?.address === recordScope.works[0]?.address))) return;
      cancel(); failed = null;
      const work = next?.works[0];
      if (next && (!['app', 'card'].includes(next.kind) || next.works.length !== 1 || work?.app_id !== options.app || work?.work_id !== options.work
        || (work.collection !== undefined && !isUuid(work.collection)) || (next.kind === 'card' && !work.collection))) {
        recordScope = scope; recordScopeRefused = true; requestedSource = null;
        if (authorized) updateContexts(sources);
        selected = CHOOSE; selector.value = CHOOSE;
        recordScopeRefusal = next.kind === 'card' && !work?.collection ? 'This component has no saved records. Choose a source file or ask about the app.' : 'That question context does not belong to this app.';
        status.textContent = recordScopeRefusal;
        refreshControls(); return;
      }
      const resetAppContext = recordScopeRefused || recordScope.kind === 'card';
      recordScopeRefused = false;
      if (next && work) {
        recordScope = {...next, address: next.address.slice(0, 120), works: [{...work, address: work.address.slice(0, 120)}]};
        selected = RECORDS; requestedSource = null; contextPicked = true;
      } else {
        if (resetAppContext) {selected = CHOOSE; contextPicked = false;}
        recordScope = scope;
      }
      if (authorized) updateContexts(sources);
      refreshControls();
    },
    ask(question) {
      const text = question.trim().slice(0, 500); if (!alive || !text) return;
      selectMode('ask');
      focusRequested = true;
      if (pending && (!loading || queued)) {
        if (waiting.length >= 8) {if (!input.value) input.value = text; status.textContent = 'Several questions are waiting. Send this question when they finish.';}
        else {waiting.push(text); status.textContent = 'Your question is next after the current answer.';}
        return;
      }
      queued = text; if (!input.value) input.value = text;
      if (!visible) {visible = true; root.hidden = false; void load();}
      else if (!loading) {
        if (authorized && selected && !recordScopeRefused) {queued = null; if (input.value === text) input.value = ''; void ask(text);}
        else if (!authorized) void load();
      }
      input.focus();
    },
    focus() {if (alive && visible) {if (mode === 'change') changes?.focus(); else {focusRequested = true; if (authorized) {focusRequested = false; input.focus();}}}},
    dispose() {alive = false; visible = false; cancel(); changes?.dispose(); exchanges.length = 0; input.value = ''; sources = null; root.remove();},
  };
}
