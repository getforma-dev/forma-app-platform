// The composer seam (APP-093, `POST /v1/compose`). `composeIntent` posts the
// member's words, never anything else, to the composer route and expects the
// saved work's identity and its honest assembly label (retrieved, adapted or
// generated: ADR-011 I5). The intent is sent in a JSON body only, never in a
// URL. Placing the work is the board's job: it fetches the group through
// `GET /v1/canvas/works/{app}/{work}`, which re-authorizes, and runs its views
// through the governed query route. Every refusal here is shown as neutral
// copy; nothing of the response body is rendered besides the title.
import { placeWork } from './board';
import { isUuid } from './render';

/** The composer route (`app/src/composer/routes.rs` PATH). */
export const COMPOSE_URL = '/v1/compose';

/** The route's answer: `{app_id, work_id, origin, title}`. */
export type ComposeOrigin = 'retrieved' | 'adapted' | 'generated';
const ORIGINS: readonly ComposeOrigin[] = ['retrieved', 'adapted', 'generated'];

export type ComposeResult = { app_id: string; work_id: string; origin: ComposeOrigin; title: string; labels: string[] };
export type ComposeCode = 'not_connected' | 'unavailable' | 'refused' | 'invalid' | 'too_many' | 'failed';
export class ComposeError extends Error {
  constructor(
    readonly code: ComposeCode,
    readonly retryAfterSeconds: number | null = null,
  ) {
    super(code);
  }
}

/** The assembly chips the card shows for an origin (the server's label, verbatim and closed). */
export function labelsFor(origin: ComposeOrigin): string[] {
  return [origin];
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

export async function composeIntent(text: string): Promise<ComposeResult> {
  let response: Response;
  try {
    response = await fetch(COMPOSE_URL, {
      method: 'POST',
      credentials: 'same-origin',
      cache: 'no-store',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ intent: text }),
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
  const body = (await response.json().catch(() => null)) as Partial<Record<keyof ComposeResult, unknown>> | null;
  if (!body || !isUuid(body.app_id) || !isUuid(body.work_id)) throw new ComposeError('failed');
  if (!ORIGINS.includes(body.origin as ComposeOrigin)) throw new ComposeError('failed');
  const origin = body.origin as ComposeOrigin;
  const title = typeof body.title === 'string' ? body.title.slice(0, 120) : '';
  return { app_id: body.app_id, work_id: body.work_id, origin, title, labels: labelsFor(origin) };
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

/** The intent bar's submit: compose, then place; `report` updates its status line. */
export async function submitIntent(text: string, report: (message: string) => void): Promise<void> {
  report('Composing your app…');
  let result: ComposeResult;
  try {
    result = await composeIntent(text);
  } catch (error) {
    report(messageFor(error));
    return;
  }
  report(result.title ? `Placing “${result.title}” on your canvas…` : 'Placing it on your canvas…');
  report(await placeWork(result.app_id, result.work_id, result.labels));
}
