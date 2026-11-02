// An app-owned, on-demand Summary companion. Opening it reads the existing
// authorized summary; only Generate/Regenerate asks the server to make one.
// The panel stays inside the app's canvas item and never becomes a loose card.
import { ensureAppGroup, isOnBoard, onBoardChange } from './board';
import { lineage } from './box-api';
import { openFocus } from './focus';
import { announce, el, isUuid } from './render';
import { setAppView } from './unite';
import { appComponentVisible } from '../presentation/presentation';

const LABEL = 'AI summary · numbers from receipts';
type Part = { text: string } | { value: string; exact: string; view_id: string; view_title: string; receipt_digest: string };
type Explanation = { state: 'ready' | 'none'; label: string; generated_at: string | null; findings: Part[][] };

class ExplainError extends Error {
  constructor(readonly status: number, readonly code: string) {
    super(code);
  }
}

type Companion = {
  app: string; work: string; host: HTMLElement; panel: HTMLElement;
  list: HTMLOListElement; status: HTMLParagraphElement; when: HTMLParagraphElement;
  action: HTMLButtonElement; label: HTMLElement; open: boolean; request: number;
  controller: AbortController | null; actionKind: 'load' | 'generate';
};
const companions = new Map<string, Companion>();
let watching = false;

function parse(value: unknown): Explanation | null {
  if (typeof value !== 'object' || value === null) return null;
  const v = value as Record<string, unknown>;
  if (v.state !== 'ready' && v.state !== 'none') return null;
  const findings = Array.isArray(v.findings) ? v.findings.slice(0, 12).map((finding) => {
    const parts = (finding as { parts?: unknown } | null)?.parts;
    if (!Array.isArray(parts)) return [];
    return parts.slice(0, 24).flatMap((part): Part[] => {
      if (typeof part !== 'object' || part === null) return [];
      const p = part as Record<string, unknown>;
      if (typeof p.text === 'string') return [{ text: p.text.slice(0, 600) }];
      if (typeof p.value === 'string' && typeof p.view_id === 'string' && /^[a-z][a-z0-9_]{0,63}$/.test(p.view_id)) {
        return [{
          value: p.value.slice(0, 80), exact: typeof p.exact === 'string' ? p.exact.slice(0, 80) : '',
          view_id: p.view_id, view_title: typeof p.view_title === 'string' ? p.view_title.slice(0, 120) : p.view_id,
          receipt_digest: typeof p.receipt_digest === 'string' ? p.receipt_digest.slice(0, 80) : '',
        }];
      }
      return [];
    });
  }).filter((parts) => parts.length > 0) : [];
  return {
    state: v.state, label: typeof v.label === 'string' ? v.label.slice(0, 80) : LABEL,
    generated_at: typeof v.generated_at === 'string' ? v.generated_at : null, findings,
  };
}

async function call(method: 'GET' | 'POST', app: string, work: string, signal: AbortSignal): Promise<Explanation> {
  let response: Response;
  try {
    response = await fetch(`/v1/boxes/explain/${app}/${work}`, {
      method, credentials: 'same-origin', cache: 'no-store', signal,
      ...(method === 'POST' ? { headers: { 'content-type': 'application/json' }, body: '{}' } : {}),
    });
  } catch (error) {
    if (signal.aborted) throw error;
    throw new ExplainError(0, 'network');
  }
  const json = (await response.json().catch(() => null)) as Record<string, unknown> | null;
  if (!response.ok) throw new ExplainError(response.status, typeof json?.code === 'string' ? json.code : 'unavailable');
  const explanation = parse(json);
  if (!explanation) throw new ExplainError(response.status, 'unavailable');
  return explanation;
}

/** Brings the view card of a value into focus and opens its receipt. */
export async function showValue(app: string, work: string, view: string, isCurrent?: () => boolean): Promise<void> {
  if (!isUuid(app) || !isUuid(work) || !/^[a-z][a-z0-9_]{0,63}$/.test(view) || (isCurrent && !isCurrent())) return;
  const id = `c-${work}-${view}`;
  const group = await ensureAppGroup(app, work);
  if (isCurrent && !isCurrent()) return;
  const card = group?.cards.find(card => card.dataset.card === id);
  if (!card) { announce('That value’s app component could not be opened.'); return; }
  if (!appComponentVisible(app, work, card)) {
    announce('That component is hidden in your views on this browser. Change your view preferences in Edit app to show it.');
    return;
  }
  if (!isOnBoard(id)) await setAppView(app, work, 'open', { focus: false, ...(isCurrent ? {isCurrent} : {}) });
  if (isCurrent && !isCurrent()) return;
  if (!openFocus(id)) { announce('That value’s app component could not be focused.'); return; }
  const receipt = card.querySelector<HTMLDetailsElement>('details.cv-receipt');
  if (receipt) receipt.open = true;
}

function message(error: unknown): string {
  if (error instanceof ExplainError && error.code === 'composer_unavailable') return 'Summaries are not available on this server right now.';
  if (error instanceof ExplainError && error.code === 'too_many_requests') return 'Too many summaries in a short time. Try again in a moment.';
  return 'The summary could not be made right now.';
}

/** Open/close changes are layout events, not independently persisted canvas objects. */
function setOpen(entry: Companion, open: boolean): void {
  entry.open = open;
  entry.host.hidden = !open;
  entry.host.inert = !open;
  const frame = entry.host.closest<HTMLElement>('.cv-item-unite');
  const button = frame?.querySelector<HTMLElement>('[data-app-summary]');
  button?.setAttribute('aria-expanded', String(open));
  entry.host.dispatchEvent(new CustomEvent('forma:app-companion', {
    bubbles: true, detail: {app: entry.app, work: entry.work, open},
  }));
}

function cancel(entry: Companion): void {
  entry.request++;
  entry.controller?.abort();
  entry.controller = null;
  entry.panel.removeAttribute('aria-busy');
}

function dispose(entry: Companion): void {
  cancel(entry);
  companions.delete(entry.work);
  setOpen(entry, false);
  entry.host.replaceChildren();
}

/** Called when the owning app leaves the canvas. */
export function disposeAppSummary(work: string): void {
  const entry = companions.get(work);
  if (entry) dispose(entry);
}

/** Hides the attached panel, preserving the app, its size history and data. */
export function closeAppSummary(work: string): void {
  const entry = companions.get(work);
  if (!entry || !entry.open) return;
  cancel(entry);
  // Reopening reauthorizes through GET; do not retain findings behind a hidden panel.
  entry.list.replaceChildren();
  entry.when.textContent = '';
  setOpen(entry, false);
  entry.host.closest('.cv-item-unite')?.querySelector<HTMLElement>('[data-app-summary]')?.focus();
}

function render(entry: Companion, explanation: Explanation): void {
  const document_ = entry.host.ownerDocument;
  entry.label.textContent = explanation.label || LABEL;
  entry.list.replaceChildren(...explanation.findings.map(parts => el(document_, 'li', {}, ...parts.map(part => {
    if ('text' in part) return part.text;
    const short = part.receipt_digest.replace(/^sha256:/, '').slice(0, 8);
    const button = el(document_, 'button', {
      type: 'button', class: 'cv-ex-value',
      title: `${part.exact ? `Exact ${part.exact}` : part.value} · ${part.view_title}${short ? ` · receipt ${short}` : ''}`,
    }, part.value);
    button.addEventListener('click', () => void showValue(entry.app, entry.work, part.view_id,
      () => companions.get(entry.work) === entry && entry.open && entry.host.isConnected));
    return button;
  }))));
  const none = explanation.state === 'none' || explanation.findings.length === 0;
  entry.status.textContent = none ? 'No summary is available yet. Generate one from this app’s data.' : '';
  entry.when.textContent = explanation.generated_at
    ? `Generated ${explanation.generated_at.replace('T', ' ').replace(/:\d\dZ$/, ' UTC')}` : 'Numbers link to their source receipts.';
  entry.actionKind = 'generate';
  entry.action.hidden = false;
  entry.action.textContent = none ? 'Generate summary' : 'Regenerate';
}

async function read(entry: Companion, method: 'GET' | 'POST'): Promise<void> {
  cancel(entry);
  const request = entry.request;
  const controller = new AbortController();
  entry.controller = controller;
  const current = () => companions.get(entry.work) === entry && entry.host.isConnected
    && entry.open && entry.request === request && !controller.signal.aborted;
  entry.action.disabled = true;
  entry.panel.setAttribute('aria-busy', 'true');
  entry.status.textContent = method === 'GET' ? 'Loading summary…' : 'Summarizing from the receipts…';
  if (method === 'GET') {
    entry.list.replaceChildren();
    entry.when.textContent = '';
    entry.action.hidden = true;
  }
  try {
    if (method === 'GET') {
      const found = await lineage(entry.app, entry.work);
      if (!current()) return;
      if (!found) {
        entry.status.textContent = 'Summaries are available for apps created from files.';
        entry.action.hidden = true;
        return;
      }
    }
    const explanation = await call(method, entry.app, entry.work, controller.signal);
    if (!current()) return;
    render(entry, explanation);
  } catch (error) {
    if (!current()) return;
    if (error instanceof ExplainError && [401, 403, 404].includes(error.status)) {
      entry.list.replaceChildren();
      entry.when.textContent = '';
      entry.status.textContent = 'This summary is no longer available.';
      entry.action.hidden = true;
    } else {
      entry.status.textContent = message(error);
      entry.actionKind = method === 'GET' ? 'load' : 'generate';
      entry.action.textContent = method === 'GET' ? 'Try again' : 'Regenerate';
      entry.action.hidden = false;
    }
  } finally {
    if (current()) {
      entry.action.disabled = false;
      entry.panel.removeAttribute('aria-busy');
      entry.controller = null;
    }
  }
}

function mountCompanion(app: string, work: string, title: string, host: HTMLElement): Companion {
  const document_ = host.ownerDocument;
  const list = el(document_, 'ol', {class: 'cv-ex-findings'});
  const status = el(document_, 'p', {class: 'cv-ex-status', role: 'status'});
  const when = el(document_, 'p', {class: 'cv-ex-when'});
  const label = el(document_, 'p', {class: 'cv-app-summary-label'}, LABEL);
  const action = el(document_, 'button', {type: 'button', class: 'cv-ex-regen'}, 'Generate summary');
  const close = el(document_, 'button', {
    type: 'button', class: 'cv-app-summary-close', 'aria-label': 'Close summary', title: 'Close summary',
  }, '×');
  const headingId = `app-summary-heading-${work}`;
  const panel = el(document_, 'section', {class: 'cv-app-summary', 'aria-labelledby': headingId},
    el(document_, 'header', {class: 'cv-app-summary-head'},
      el(document_, 'div', {}, el(document_, 'h3', {id: headingId}, 'Summary'),
        el(document_, 'p', {class: 'cv-app-summary-of', title}, title)), close),
    el(document_, 'div', {class: 'cv-app-summary-body'}, label, list, status),
    el(document_, 'footer', {class: 'cv-app-summary-foot'}, when, action));
  const entry: Companion = {app, work, host, panel, list, status, when, action, label,
    open: false, request: 0, controller: null, actionKind: 'load'};
  close.addEventListener('click', () => closeAppSummary(work));
  action.addEventListener('click', () => {
    if (entry.action.disabled || !entry.open || !entry.host.isConnected) return;
    void read(entry, entry.actionKind === 'load' ? 'GET' : 'POST');
  });
  host.replaceChildren(panel);
  companions.set(work, entry);
  return entry;
}

/** Called by the app's Summary control; never starts an automatic model request. */
export function toggleAppSummary(app: string, work: string, title: string, host: HTMLElement): void {
  if (!isUuid(app) || !isUuid(work) || !host.isConnected) return;
  startExplain();
  let entry = companions.get(work);
  if (entry && (entry.app !== app || entry.host !== host)) {
    dispose(entry);
    entry = undefined;
  }
  if (entry?.open) { closeAppSummary(work); return; }
  entry ??= mountCompanion(app, work, title, host);
  setOpen(entry, true);
  void read(entry, 'GET');
}

/** Cleanup only: placing an app does not fetch or generate a summary. */
export function startExplain(): void {
  if (watching) return;
  watching = true;
  onBoardChange(() => {
    for (const entry of companions.values()) if (!entry.host.isConnected) dispose(entry);
  });
}
