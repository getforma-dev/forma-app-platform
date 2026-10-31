// The composer seam (APP-093, `POST /v1/compose`). `composeIntent` posts the
// member's words, never anything else, to the composer route and expects the
// saved work's identity and its honest assembly label (retrieved, adapted or
// generated: ADR-011 I5). The intent is sent in a JSON body only, never in a
// URL. Placing the work is the board's job: it fetches the group through
// `GET /v1/canvas/works/{app}/{work}`, which re-authorizes, and runs its views
// through the governed query route. Every refusal here is shown as neutral
// copy; nothing of the response body is rendered besides the title.
import { markTailoring, noteKept, onTailored, placeWork, setReporter, workCard } from './board';
import { showTiming, type Chip, type Step } from './timing';
import { showChooser } from './slash';
import { isUuid } from './render';

/** The composer route (`app/src/composer/routes.rs` PATH). */
export const COMPOSE_URL = '/v1/compose';

/** The route's answer: `{app_id, work_id, origin, title}`. */
export type ComposeOrigin = 'retrieved' | 'adapted' | 'generated';
const ORIGINS: readonly ComposeOrigin[] = ['retrieved', 'adapted', 'generated'];

/** APP-101: how far the live transform got when the route answered. */
export type Tailoring = 'none' | 'pending' | 'kept' | 'adapted';
const TAILORINGS: readonly Tailoring[] = ['none', 'pending', 'kept', 'adapted'];
export type ComposeResult = {
  kind: 'placed'; app_id: string; work_id: string; origin: ComposeOrigin; title: string;
  /** The assembly chips the card shows: the server's origin label, verbatim and closed. */
  labels: string[]; tailoring: Tailoring;
  /** APP-116: the route's own time, when the server reported it. */
  duration_ms: number | null;
};
/** APP-101: a near tie; the member chooses (nothing was written). */
export type Choice = { app_id: string; title: string; purpose: string };
export type ComposeAnswer = ComposeResult | { kind: 'choose'; choices: Choice[] };
/** APP-101: a slash command or chooser click pins the app; "Build something new" asks for generation. */
export type Pin = { app_id: string } | { generate: true } | null;
export type ComposeCode = 'not_connected' | 'unavailable' | 'refused' | 'invalid' | 'too_many' | 'failed';
export class ComposeError extends Error {
  constructor(
    readonly code: ComposeCode,
    readonly retryAfterSeconds: number | null = null,
  ) {
    super(code);
  }
}

/** The server's `Retry-After` in seconds, bounded; `null` when absent or malformed. */
function retryAfter(response: Response): number | null {
  const raw = response.headers.get('retry-after');
  if (!raw || !/^\d{1,6}$/.test(raw.trim())) return null;
  return Math.min(Number(raw.trim()), 24 * 3600);
}

/** The stable `code` of a design §9 problem body (`http::problem`), or `null`. */
async function refusalCode(response: Response): Promise<string | null> {
  const body = (await response.json().catch(() => null)) as { code?: unknown } | null;
  return body && typeof body.code === 'string' && body.code.length <= 64 ? body.code : null;
}

export async function composeIntent(text: string, pin: Pin = null): Promise<ComposeAnswer> {
  let response: Response;
  try {
    response = await fetch(COMPOSE_URL, {
      method: 'POST',
      credentials: 'same-origin',
      cache: 'no-store',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ intent: text, ...(pin ?? {}) }),
    });
  } catch {
    throw new ComposeError('failed');
  }
  // The uniform 404: no session, nothing composable, or no composer route on this server.
  if (response.status === 404) throw new ComposeError('not_connected');
  if (response.status === 429) throw new ComposeError('too_many', retryAfter(response));
  if (response.status === 422 || response.status === 400 || response.status === 413) {
    const code = await refusalCode(response);
    throw new ComposeError(code === 'could_not_compose' ? 'refused' : 'invalid');
  }
  if (response.status === 503) {
    const code = await refusalCode(response);
    throw new ComposeError(code === 'composer_unavailable' ? 'unavailable' : 'failed');
  }
  if (!response.ok) throw new ComposeError('failed');
  const body = (await response.json().catch(() => null)) as (Partial<Record<keyof ComposeResult, unknown>> & { choices?: unknown }) | null;
  if (body && Array.isArray(body.choices)) {
    const choices = body.choices
      .filter((c): c is Choice => typeof c === 'object' && c !== null && isUuid((c as Choice).app_id))
      .slice(0, 3)
      .map((c) => ({ app_id: c.app_id, title: String(c.title ?? '').slice(0, 120), purpose: String(c.purpose ?? '').slice(0, 200) }));
    if (choices.length) return { kind: 'choose', choices };
  }
  if (!body || !isUuid(body.app_id) || !isUuid(body.work_id)) throw new ComposeError('failed');
  if (!ORIGINS.includes(body.origin as ComposeOrigin)) throw new ComposeError('failed');
  const origin = body.origin as ComposeOrigin;
  const title = typeof body.title === 'string' ? body.title.slice(0, 120) : '';
  const tailoring = TAILORINGS.includes(body.tailoring as Tailoring) ? (body.tailoring as Tailoring) : 'none';
  const duration = (body as { duration_ms?: unknown }).duration_ms;
  return {
    kind: 'placed', app_id: body.app_id, work_id: body.work_id, origin, title, labels: [origin], tailoring,
    duration_ms: typeof duration === 'number' && Number.isFinite(duration) ? duration : null,
  };
}

const MESSAGES: Record<ComposeCode, string> = {
  not_connected: 'Nothing could be composed for this request on this server. Nothing was placed.',
  unavailable: 'The composer is not available on this server right now. Nothing was placed.',
  refused: 'That request could not be composed into an app. Nothing was placed.',
  invalid: 'Describe what you need in a few words (3 to 500 characters). Nothing was placed.',
  too_many: 'You have composed a few apps recently. Please try again later. Nothing was placed.',
  failed: 'The app could not be composed right now. Nothing was placed.',
};

/** The neutral message for a refusal; a 429 names its wait when the server gave one. */
export function messageFor(error: unknown): string {
  if (!(error instanceof ComposeError)) return MESSAGES.failed;
  if (error.code === 'too_many' && error.retryAfterSeconds !== null) {
    const minutes = Math.max(1, Math.ceil(error.retryAfterSeconds / 60));
    return `You have composed a few apps recently. Please try again in about ${minutes} ${minutes === 1 ? 'minute' : 'minutes'}. Nothing was placed.`;
  }
  return MESSAGES[error.code];
}

/**
 * The intent bar's submit: compose, then place; `report` updates its status
 * line. APP-101: the card lands at once with the app's defaults; when the
 * request said more, a "tailoring" state follows until `app.adapted` (the
 * card reshapes in place) or `app.kept` arrives on the live stream. A near
 * tie shows the chooser instead of guessing.
 */
// APP-116: a compose's chip waits for the live transform's outcome (reshape or kept).
// (Registered on first use: board.ts and this module import each other.)
const tailoringChips = new Map<string, { chip: Chip; since: number }>();
let tailoredWired = false;
function wireTailored(): void {
  if (tailoredWired) return;
  tailoredWired = true;
  onTailored((from, to) => {
    const pending = tailoringChips.get(from);
    if (!pending) return;
    tailoringChips.delete(from);
    pending.chip.add({ label: to ? 'reshape' : 'kept defaults', ms: performance.now() - pending.since });
    if (to) window.setTimeout(() => pending.chip.move(workCard(to, 'app')), 60);
  });
}

/** Empties the bar after a request that did what it said (the next one starts clean). */
export function clearIntentBar(): void {
  const input = document.getElementById('intent-input') as HTMLInputElement | null;
  if (input && input.value) {
    input.value = '';
    input.dispatchEvent(new Event('input', { bubbles: true }));
  }
}

export async function submitIntent(text: string, bar: (message: string) => void, pin: Pin = null, before: Step[] = []): Promise<void> {
  const report = setReporter(bar);
  report(pin && 'app_id' in pin ? 'Opening it…' : 'Composing your app…');
  const started = performance.now();
  let answer: ComposeAnswer;
  try {
    answer = await composeIntent(text, pin);
  } catch (error) {
    report(messageFor(error));
    return;
  }
  if (answer.kind === 'choose') {
    report('A few apps fit. Pick one, or build something new.');
    showChooser(text, answer.choices, report);
    return;
  }
  const result = answer;
  const composeMs = performance.now() - started;
  document.getElementById('cv-canvas')?.setAttribute('data-last-compose-ms', String(Math.round(composeMs)));
  report(result.title ? `Placing “${result.title}” on your canvas…` : 'Placing it on your canvas…');
  const placing = performance.now();
  const placed = await placeWork(result.app_id, result.work_id, result.labels);
  const chip = showTiming(pin && 'app_id' in pin ? 'Open' : 'Compose', [
    ...before,
    { label: pin && 'generate' in pin ? 'build' : 'pick', ms: composeMs, server: result.duration_ms },
    { label: 'on canvas', ms: performance.now() - placing },
  ], workCard(result.work_id, 'app'));
  if (/on your canvas/.test(placed)) clearIntentBar();
  wireTailored();
  if (result.tailoring === 'pending') tailoringChips.set(result.work_id, { chip, since: started + composeMs });
  if (result.tailoring === 'pending') {
    const state = markTailoring(result.work_id);
    report(state === 'tailoring' ? `${placed} Tailoring it to your request…`
      : state === 'adapted' ? 'Adapted to your request. “Back to standard” shows the defaults.'
        : 'Kept the standard view.');
    return;
  }
  if (result.tailoring === 'kept') noteKept(result.work_id);
  report(placed);
}
