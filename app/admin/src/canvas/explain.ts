// APP-108: the Explanation card of an app built from a box. When such a work
// is on the board (its lineage route answers), a "Summary" card labeled "AI
// summary · numbers from receipts" sits beside it. Its findings come from
// `GET /v1/boxes/explain/{app}/{work}` (once per page, a `POST` generates one
// when none exists; "Regenerate" is the same `POST`). The server builds every
// value from a released query result and names the view and receipt it came
// from; this card shows text parts as text nodes and each value as a button
// that brings that view card into focus and opens its receipt. Nothing is
// stored in the browser; a reload fetches again when the work is placed.
import { addCards, boardWorks, canvasApi, focusCard, mountWidget, onBoardChange, unmountWidget } from './board';
import { lineage } from './box-api';
import { registerFocusCard, unregisterFocusCard } from './focus';
import { el, isUuid } from './render';

const LABEL = 'AI summary · numbers from receipts';
type Part = { text: string } | { value: string; exact: string; view_id: string; view_title: string; receipt_digest: string };
type Explanation = { state: 'ready' | 'none'; label: string; generated_at: string | null; findings: Part[][] };

class ExplainError extends Error {
  constructor(readonly status: number, readonly code: string) {
    super(code);
  }
}

const seen = new Set<string>();
const cards = new Map<string, { item: HTMLElement; render(explanation: Explanation): void; say(message: string): void }>();
const attempted = new Set<string>();

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

async function call(method: 'GET' | 'POST', app: string, work: string): Promise<Explanation> {
  let response: Response;
  try {
    response = await fetch(`/v1/boxes/explain/${app}/${work}`, {
      method, credentials: 'same-origin', cache: 'no-store',
      ...(method === 'POST' ? { headers: { 'content-type': 'application/json' }, body: '{}' } : {}),
    });
  } catch {
    throw new ExplainError(0, 'network');
  }
  const json = (await response.json().catch(() => null)) as Record<string, unknown> | null;
  if (!response.ok) throw new ExplainError(response.status, typeof json?.code === 'string' ? json.code : 'unavailable');
  const explanation = parse(json);
  if (!explanation) throw new ExplainError(response.status, 'unavailable');
  return explanation;
}

/** Brings the view card of a value into focus and opens its receipt. */
export async function showValue(app: string, work: string, view: string): Promise<void> {
  const id = `c-${work}-${view}`;
  let card = document.querySelector<HTMLElement>(`.cv-item [data-card="${id}"]`);
  if (!card) {
    await addCards(app, work, [id], { focus: false });
    card = document.querySelector<HTMLElement>(`.cv-item [data-card="${id}"]`);
  }
  if (!card) return;
  focusCard(card);
  const receipt = card.querySelector<HTMLDetailsElement>('details.cv-receipt');
  if (receipt) receipt.open = true;
}

function message(error: unknown): string {
  if (error instanceof ExplainError && error.code === 'composer_unavailable') return 'Summaries are not available on this server right now.';
  if (error instanceof ExplainError && error.code === 'too_many_requests') return 'Too many summaries in a short time. Try again in a moment.';
  return 'The summary could not be made right now.';
}

function mountCard(app: string, work: string, title: string) {
  const document_ = document;
  const list = el(document_, 'ol', { class: 'cv-ex-findings' });
  const status = el(document_, 'p', { class: 'cv-ex-status', role: 'status' });
  const when = el(document_, 'p', { class: 'cv-ex-when' });
  const regenerate = el(document_, 'button', { type: 'button', class: 'cv-ex-regen' }, 'Regenerate');
  const close = el(document_, 'button', { type: 'button', class: 'bx-close', 'data-widget-close': '', 'aria-label': 'Remove the summary from the canvas', title: 'Remove' }, '×');
  const head = el(document_, 'header', { class: 'cv-ex-head' },
    el(document_, 'div', { class: 'cv-ex-id' },
      el(document_, 'h2', {}, 'Summary'),
      el(document_, 'p', { class: 'cv-ex-of' }, title)),
    el(document_, 'span', { class: 'cv-origin cv-ex-label', 'data-origin': 'generated' }, LABEL));
  const card = el(document_, 'article', { class: 'cv-card cv-ex', 'data-kind': 'explain', 'data-card': `ex-${work}`, 'aria-label': `Summary of ${title}` },
    head,
    list, status,
    el(document_, 'footer', { class: 'cv-ex-foot' }, when, regenerate));
  // Beside the app card: just left of its group, top-aligned.
  const appItem = document_.querySelector<HTMLElement>(`.cv-item [data-card="a-${work}"]`)?.closest<HTMLElement>('.cv-item');
  const state = appItem ? canvasApi()?.getItemState(appItem) : null;
  const at = state ? { x: state.x - 440, y: state.y } : undefined;
  const item = mountWidget(card, { id: `explain-${work}`, title: 'Summary', width: 420, minHeight: 220, at, focus: false });
  if (!item) return null;
  // APP-106 focus mode (F or the focus button), as for every card.
  registerFocusCard(card, head, title);
  head.append(close);
  const render = (explanation: Explanation) => {
    const label = card.querySelector('.cv-ex-label');
    if (label) label.textContent = explanation.label || LABEL;
    list.replaceChildren(...explanation.findings.map((parts) => el(document_, 'li', {}, ...parts.map((part) => {
      if ('text' in part) return part.text;
      const short = part.receipt_digest.replace(/^sha256:/, '').slice(0, 8);
      const button = el(document_, 'button', {
        type: 'button', class: 'cv-ex-value',
        title: `${part.exact ? `Exact ${part.exact}` : part.value} · ${part.view_title}${short ? ` · receipt ${short}` : ''}`,
      }, part.value);
      button.addEventListener('click', () => void showValue(app, work, part.view_id));
      return button;
    }))));
    const none = explanation.state === 'none' || explanation.findings.length === 0;
    status.textContent = none ? 'No summary. The model’s answer was not used, or none was available; the numbers stay on their cards.' : '';
    when.textContent = explanation.generated_at ? `Generated ${explanation.generated_at.replace('T', ' ').replace(/:\d\dZ$/, ' UTC')}` : '';
  };
  const say = (text: string) => { status.textContent = text; };
  regenerate.addEventListener('click', () => {
    if (regenerate.disabled) return;
    regenerate.disabled = true;
    say('Summarizing from the receipts…');
    void call('POST', app, work).then(render).catch((error: unknown) => {
      if (error instanceof ExplainError && error.status === 404) remove(work);
      else say(message(error));
    }).finally(() => { regenerate.disabled = false; });
  });
  close.addEventListener('click', () => remove(work));
  const entry = { item, render, say };
  cards.set(work, entry);
  return entry;
}

function remove(work: string): void {
  const entry = cards.get(work);
  if (!entry) return;
  const card = entry.item.querySelector<HTMLElement>('.cv-ex');
  if (card) unregisterFocusCard(card);
  unmountWidget(entry.item);
  cards.delete(work);
}

async function consider(app: string, work: string, title: string): Promise<void> {
  const found = await lineage(app, work);
  if (!found) return;
  let explanation: Explanation;
  try {
    explanation = await call('GET', app, work);
  } catch {
    // Not a derived app (a data card, no route), or not now: no card.
    return;
  }
  if (cards.has(work)) return;
  const entry = mountCard(app, work, title);
  if (!entry) return;
  entry.render(explanation);
  if (explanation.state === 'none' && !attempted.has(work)) {
    attempted.add(work);
    entry.say('Summarizing from the receipts…');
    try {
      entry.render(await call('POST', app, work));
    } catch (error) {
      if (error instanceof ExplainError && error.status === 404) remove(work);
      else entry.say(message(error));
    }
  }
}

function scan(): void {
  const placed = new Set<string>();
  for (const work of boardWorks()) {
    placed.add(work.work);
    if (seen.has(work.work) || !isUuid(work.app) || !isUuid(work.work)) continue;
    // Only once the app card is on the board.
    if (!document.querySelector(`.cv-item [data-card="a-${work.work}"]`)) continue;
    seen.add(work.work);
    void consider(work.app, work.work, work.title);
  }
  // A work taken off the board takes its summary with it; placed again, it fetches again.
  for (const work of [...seen]) {
    if (placed.has(work) && document.querySelector(`.cv-item [data-card="a-${work}"]`)) continue;
    seen.delete(work);
    remove(work);
  }
}

/** Watches the board for works built from a box and keeps their summary cards. */
export function startExplain(): void {
  onBoardChange(scan);
  scan();
}
