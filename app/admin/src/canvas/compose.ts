// The composer seam (APP-093, built on `pivot/composer`; contract to come).
// `composeIntent` posts the member's words, never anything else, to the
// composer route and expects the published work's identity and its honest
// assembly labels (retrieved, adapted, generated: ADR-011 I5). The intent is
// sent in a JSON body only, never in a URL. Placing the work is the board's
// job: it fetches the group through `GET /v1/canvas/works/{app}/{work}`,
// which re-authorizes, and runs its views through the governed query route.
import { placeWork } from './board';
import { isUuid } from './render';

/** The composer route; the contract lands with APP-093. */
export const COMPOSE_URL = '/v1/compose';

export type ComposeResult = { app_id: string; work_id: string; labels: string[] };
export class ComposeError extends Error {
  constructor(readonly code: 'not_connected' | 'refused' | 'failed') {
    super(code);
  }
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
  if (response.status === 404) throw new ComposeError('not_connected');
  if (response.status === 422 || response.status === 400) throw new ComposeError('refused');
  if (!response.ok) throw new ComposeError('failed');
  const body = (await response.json().catch(() => null)) as Partial<ComposeResult> | null;
  if (!body || !isUuid(body.app_id) || !isUuid(body.work_id)) throw new ComposeError('failed');
  const labels = Array.isArray(body.labels)
    ? body.labels.filter((label): label is string => typeof label === 'string' && label.length <= 40).slice(0, 4)
    : [];
  return { app_id: body.app_id, work_id: body.work_id, labels };
}

const MESSAGES: Record<ComposeError['code'], string> = {
  not_connected: 'The composer is not connected on this server yet. Nothing was placed.',
  refused: 'That request could not be composed into an app. Nothing was placed.',
  failed: 'The app could not be composed right now. Nothing was placed.',
};

/** The intent bar's submit: compose, then place; `report` updates its status line. */
export async function submitIntent(text: string, report: (message: string) => void): Promise<void> {
  report('Composing your app…');
  let result: ComposeResult;
  try {
    result = await composeIntent(text);
  } catch (error) {
    report(error instanceof ComposeError ? MESSAGES[error.code] : MESSAGES.failed);
    return;
  }
  report('Placing it on your canvas…');
  report(await placeWork(result.app_id, result.work_id, result.labels));
}
