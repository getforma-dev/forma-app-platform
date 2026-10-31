// The island's only data read: `POST /v1/work/{app}/{work}/queries/{view}`
// (APP-012), same-origin, parameters in the body. The server authorizes,
// executes, builds the receipt and re-authorizes the release; this module
// only maps its answer onto the card. A response that is not a released
// result with a receipt shows no number (ADR-011 I3). Two runs are in flight
// at a time (the engine admits two per organization); a 429 waits its
// `Retry-After` and retries, showing busy meanwhile.
import { released, renderNoValue, renderReleased } from './render';

export type Parameters = { window?: { start: string; end: string }; destination?: string };

const MAX_IN_FLIGHT = 2;
const MAX_BUSY_RETRIES = 3;
let inFlight = 0;
const waiting: (() => void)[] = [];

async function slot<T>(run: () => Promise<T>): Promise<T> {
  if (inFlight >= MAX_IN_FLIGHT) await new Promise<void>((resolve) => waiting.push(resolve));
  inFlight += 1;
  try {
    return await run();
  } finally {
    inFlight -= 1;
    waiting.shift()?.();
  }
}

/** `datetime-local` text as the contract's UTC timestamp (`routes::form_timestamp`). */
export function timestamp(value: string): string | null {
  const trimmed = value.trim();
  if (trimmed.length === 16) return `${trimmed}:00Z`;
  if (trimmed.length === 19) return `${trimmed}Z`;
  if (trimmed.length === 20 && trimmed.endsWith('Z')) return trimmed;
  return null;
}

/**
 * The window form's parameters: both bounds or neither (neither: the view
 * computes over the whole bound revision and its receipt records no window);
 * `null` when the form is not acceptable.
 */
export function parametersOf(form: HTMLFormElement | null): Parameters | null {
  if (!form) return {};
  const field = (name: string) => (form.elements.namedItem(name) as HTMLInputElement | null)?.value ?? '';
  const [start, end, destination] = [field('window_start'), field('window_end'), field('destination').trim()];
  const parameters: Parameters = {};
  if (start || end) {
    const [from, to] = [timestamp(start), timestamp(end)];
    if (!from || !to || from >= to) return null;
    parameters.window = { start: from, end: to };
  }
  if (destination) parameters.destination = destination;
  return parameters;
}

const NO_NUMBER = 'No number is shown.';
const WINDOW = 'That time range isn’t valid: the start must come before the end.';
function failure(status: number, code: string): [string, string, string] {
  if (status === 404) return ['denied', 'denied', 'You don’t have access to this card.'];
  if (status === 422 && code === 'narrow_the_query') {
    return ['failed', 'failed', 'Too much data for one card. Narrow the time range.'];
  }
  if (status === 422 && code === 'query_refused') {
    return ['failed', 'failed', 'This card uses a calculation that isn’t supported yet.'];
  }
  if (status === 422) return ['failed', 'failed', WINDOW];
  if (status === 503 && code === 'source_unavailable') {
    return ['failed', 'failed', 'This card’s data isn’t available right now.'];
  }
  return ['failed', 'failed', 'This card couldn’t load.'];
}

const sleep = (seconds: number) => new Promise((resolve) => setTimeout(resolve, seconds * 1000));

/** Runs one view card; a newer run of the same card supersedes this one (its answer is dropped). */
export async function runView(card: HTMLElement, parameters: Parameters): Promise<void> {
  const url = card.dataset.query;
  if (!url || !url.startsWith('/v1/work/')) return;
  const epoch = String(Number(card.dataset.epoch ?? '0') + 1);
  card.dataset.epoch = epoch;
  const current = () => card.dataset.epoch === epoch;
  renderNoValue(card, 'loading', 'loading', 'Loading…');
  for (let attempt = 0; ; attempt += 1) {
    let response: Response | null;
    try {
      // APP-113: a run superseded while it waited for a slot never fetches.
      response = await slot(() => (current()
        ? fetch(url, {
          method: 'POST',
          credentials: 'same-origin',
          cache: 'no-store',
          headers: { 'content-type': 'application/json' },
          body: JSON.stringify({ parameters }),
        })
        : Promise.resolve(null)));
    } catch {
      if (current()) renderNoValue(card, 'failed', 'failed', 'This card couldn’t load. Check your connection.');
      return;
    }
    if (!response || !current()) return;
    if (response.status === 429 && attempt < MAX_BUSY_RETRIES) {
      const wait = Math.min(10, Math.max(1, Number(response.headers.get('retry-after')) || 2));
      renderNoValue(card, 'loading', 'busy', 'Busy. Trying again in a moment…');
      await sleep(wait);
      if (!current()) return;
      continue;
    }
    if (response.status === 429) {
      renderNoValue(card, 'failed', 'busy', 'Still busy. Try again in a few seconds.');
      return;
    }
    const body: unknown = await response.json().catch(() => null);
    if (!current()) return;
    if (response.ok) {
      const result = released(body);
      if (result) renderReleased(card, result);
      else renderNoValue(card, 'failed', 'failed', `This result came without its proof. ${NO_NUMBER}`);
      return;
    }
    const code = typeof body === 'object' && body !== null && typeof (body as { code?: unknown }).code === 'string'
      ? (body as { code: string }).code
      : '';
    const [state, badge, message] = failure(response.status, code);
    renderNoValue(card, state, badge, message);
    return;
  }
}
