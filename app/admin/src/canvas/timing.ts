// APP-116 timing chips: the app measures and shows its own step times, so a
// recording can show real numbers. Off by default; "Show timings" in the
// toolbar's More menu, `/canvas#timing=1` (kept in this browser) or the sentence
// "show timings" turn it on, `#timing=0` or the same switch off. While off
// nothing is drawn at all (no hidden chips in the page).
//
// A chip is a list of steps, each measured by this page (`performance.now`
// around the request) and, where the server's answer carries its own
// duration (`parse_ms`, `duration_ms`, a folder build's `timings`), the
// server's number. Two places show it: the dock line above the intent bar
// (the latest action) and the affected card (until the next action on it).
// Agent activity (a record changed through MCP, Slack) shows "via <label>"
// and the time it arrived: the live event carries no tool or duration.
import { announce, el } from './render';

const KEY = 'forma.timings.v1';
let on = false;
let documentRef: Document | null = null;
const listeners: Array<(enabled: boolean) => void> = [];

export type Step = { label: string; ms: number; server?: number | null; text?: string };

/** `12 ms`, `0.8 s`, `12 s`. */
export function formatMs(ms: number): string {
  if (!Number.isFinite(ms) || ms < 0) return '–';
  if (ms < 1000) return `${Math.max(0, Math.round(ms))} ms`;
  if (ms < 10000) return `${(ms / 1000).toFixed(1)} s`;
  return `${Math.round(ms / 1000)} s`;
}

/** The chip's text: `pick 4 ms · reshape 0.8 s` (a server number in brackets when it differs from the page's). */
export function chipText(steps: Step[]): string {
  return steps.map((step) => {
    if (step.text) return `${step.label} ${step.text}`;
    const server = typeof step.server === 'number' && Number.isFinite(step.server) ? step.server : null;
    const shown = server !== null && Math.abs(step.ms - server) >= 5 ? `${formatMs(step.ms)} (server ${formatMs(server)})` : formatMs(server ?? step.ms);
    return `${step.label} ${shown}`;
  }).join(' · ');
}

export function timingsOn(): boolean {
  return on;
}
export function onTimingsChange(listener: (enabled: boolean) => void): void {
  listeners.push(listener);
}

export function setTimings(enabled: boolean): void {
  on = enabled;
  try {
    if (enabled) localStorage.setItem(KEY, '1');
    else localStorage.removeItem(KEY);
  } catch {
    // a per-browser preference only
  }
  documentRef?.getElementById('cv-shell')?.classList.toggle('show-timings', enabled);
  for (const toggle of documentRef?.querySelectorAll<HTMLElement>('[data-tool="timings"]') ?? []) toggle.setAttribute('aria-checked', String(enabled));
  if (!enabled) for (const chip of documentRef?.querySelectorAll('.cv-timing') ?? []) chip.remove();
  for (const listener of listeners) listener(enabled);
}

/** The card element a chip belongs on (a card, a widget, a folder card). */
function holder(target: HTMLElement | null | undefined): HTMLElement | null {
  if (!target || !target.isConnected) return null;
  return target.closest<HTMLElement>('.cv-card, .cv-widget') ?? target;
}

export type Chip = { add(step: Step): void; move(target: HTMLElement | null): void };

/** When this page last showed a chip for its own action: a live echo of it does not replace that chip. */
let lastLocal = -Infinity;

/** A live update's chip (another person's or agent's change), unless it is the echo of this page's own action. */
export function showLiveTiming(card: HTMLElement, ms: number): void {
  if (performance.now() - lastLocal < 5000) return;
  showTiming('Live update', [{ label: 'event → on this screen', ms }], card, true);
}

/**
 * Shows `title: steps` on the dock line and on the card `target`. Returns a
 * handle to add a later step (the reshape after the pick) or to move the chip
 * to a card that exists only later. Nothing is drawn while timings are off.
 */
export function showTiming(title: string, steps: Step[], target?: HTMLElement | null, live = false): Chip {
  if (!live) lastLocal = performance.now();
  const all = [...steps];
  let card = holder(target ?? null);
  const draw = () => {
    if (!on || !documentRef) return;
    const text = `${title ? `${title}: ` : ''}${chipText(all)}`;
    const dock = documentRef.getElementById('cv-timing-line');
    dock?.replaceChildren(el(documentRef, 'span', { class: 'cv-timing cv-timing-dock', title: 'Measured live on this page; server numbers come from the server’s answer' },
      el(documentRef, 'span', { class: 'cv-timing-mark', 'aria-hidden': 'true' }, '⏱'), text));
    if (card && !card.isConnected) card = null;
    if (!card) return;
    card.querySelector(':scope > .cv-timing-card')?.remove();
    card.append(el(documentRef, 'p', { class: 'cv-timing cv-timing-card', title: 'Measured live' },
      el(documentRef, 'span', { class: 'cv-timing-mark', 'aria-hidden': 'true' }, '⏱'), text));
  };
  draw();
  return {
    add(step) {
      all.push(step);
      draw();
    },
    move(next) {
      card = holder(next);
      draw();
    },
  };
}

/** "Agent activity" on a card: `via Claude · 2:14:03 PM`. */
export function showActivity(target: HTMLElement | null, label: string, what: string): void {
  const card = holder(target);
  if (!on || !documentRef || !card) return;
  const time = new Date().toLocaleTimeString([], { hour: 'numeric', minute: '2-digit', second: '2-digit' });
  card.querySelector(':scope > .cv-activity-chip')?.remove();
  card.append(el(documentRef, 'p', { class: 'cv-timing cv-activity-chip', title: 'A change that arrived live from another channel' },
    el(documentRef, 'span', { class: 'cv-timing-mark', 'aria-hidden': 'true' }, '⚡'),
    `Agent activity: ${what} via ${label} · ${time}`));
}

/** Resolves with the ms until every view card of `work` shows a value (or a final state), capped at `limit`. */
export function viewsSettled(work: string, since: number, limit = 20000): Promise<number | null> {
  return new Promise((resolve) => {
    const check = () => {
      const cards = [...(documentRef?.querySelectorAll<HTMLElement>(`.cv-view[data-query*="/${work}/"]`) ?? [])].filter((card) => card.isConnected && !card.closest('.cv-board'));
      if (cards.length && cards.every((card) => !['loading', 'idle'].includes(card.dataset.state ?? ''))) {
        resolve(performance.now() - since);
        return true;
      }
      if (performance.now() - since > limit) {
        resolve(null);
        return true;
      }
      return false;
    };
    const tick = () => { if (!check()) window.setTimeout(tick, 40); };
    tick();
  });
}

/** Reads `#timing=1|0` and this browser's choice; wires the dock line. */
export function startTimings(document_: Document): void {
  if (documentRef) return;
  documentRef = document_;
  let stored = false;
  try {
    stored = localStorage.getItem(KEY) === '1';
  } catch {
    stored = false;
  }
  // `/canvas#timing=1` (the canvas page answers 404 to any query string, so
  // the switch rides in the fragment, which never reaches the server);
  // `?timing=1` is read too where a page allows it. The fragment is removed
  // once read, so a shared link does not carry it.
  const hash = window.location.hash.match(/(?:^#|&)timing=([01])\b/);
  const query = hash?.[1] ?? new URLSearchParams(window.location.search).get('timing');
  if (hash) {
    const rest = window.location.hash.replace(/(^#|&)timing=[01]\b&?/, '$1').replace(/^#&?$/, '');
    history.replaceState(history.state, '', `${window.location.pathname}${window.location.search}${rest}`);
  }
  const dock = document_.querySelector('.cv-dock');
  if (dock && !document_.getElementById('cv-timing-line')) {
    dock.prepend(el(document_, 'div', { id: 'cv-timing-line', class: 'cv-timing-line', 'aria-live': 'off' }));
  }
  setTimings(query === '1' ? true : query === '0' ? false : stored);
}

/** The "show timings" / "hide timings" toggle (menu, sentence). */
export function toggleTimings(force?: boolean): string {
  setTimings(force ?? !on);
  const message = on ? 'Timings on: each action shows its real step times.' : 'Timings off.';
  announce(message);
  return message;
}

/** A folder build's server steps, in the server's order and words made plain. */
const FOLDER_STEPS: [key: string, label: string][] = [
  ['parse_ms', 'read files'], ['context_ms', 'context'], ['generation_ms', 'model'], ['admission_publish_ms', 'admitted'],
  ['store_ms', 'stored'], ['data_ms', 'data'], ['pdf_ms', 'PDF'], ['views_ms', 'views'], ['summary_ms', 'summary'],
];
export function serverSteps(timings: unknown): Step[] {
  if (typeof timings !== 'object' || timings === null) return [];
  const value = timings as Record<string, unknown>;
  return FOLDER_STEPS.flatMap(([key, label]): Step[] => {
    const ms = value[key];
    if (typeof ms !== 'number' || !Number.isFinite(ms)) return [];
    // A rehearsed demo build calls no model: say so rather than "model 0 ms".
    if (key === 'generation_ms' && ms === 0) return [{ label, ms, text: 'not called (rehearsed)' }];
    return [{ label, ms, server: ms }];
  });
}

/**
 * A folder formation's chip: the server's steps, the page's placement, then
 * the time until the new app's views all show a value.
 */
export async function formationChip(timings: unknown, placeMs: number, work: string, card: HTMLElement | null): Promise<void> {
  const chip = showTiming('Formation', [...serverSteps(timings), { label: 'on canvas', ms: placeMs }], card);
  const since = performance.now();
  const views = await viewsSettled(work, since);
  if (views !== null) chip.add({ label: 'views', ms: views });
}
