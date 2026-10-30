// Live updates (seam for `pivot/realtime`): one server-sent-events stream per
// organization, `GET /v1/events` under the session (the page names the URL;
// empty means no stream yet). Each event is data: a closed kind and two ids.
// Nothing in an event is rendered; it only tells the board which cards to
// re-run (through the governed query route) or which work to place (through
// `GET /v1/canvas/works/{app}/{work}`, which re-authorizes).
import { isUuid } from './render';

export type LiveKind = 'record.committed' | 'app.shared' | 'app.placed';
export type LiveEvent = { kind: LiveKind; app_id: string; work_id: string };
const KINDS: readonly LiveKind[] = ['record.committed', 'app.shared', 'app.placed'];

export function liveEvent(data: unknown): LiveEvent | null {
  if (typeof data !== 'string' || data.length > 4096) return null;
  let value: unknown;
  try {
    value = JSON.parse(data);
  } catch {
    return null;
  }
  if (typeof value !== 'object' || value === null) return null;
  const event = value as Record<string, unknown>;
  if (!KINDS.includes(event.kind as LiveKind) || !isUuid(event.app_id) || !isUuid(event.work_id)) return null;
  return { kind: event.kind as LiveKind, app_id: event.app_id, work_id: event.work_id };
}

/** Subscribes to `url` (same origin, `/v1/` only); returns the unsubscribe. */
export function subscribeLive(
  url: string,
  onEvent: (event: LiveEvent) => void,
  onState: (state: 'live' | 'reconnecting' | 'off') => void,
): () => void {
  if (!url.startsWith('/v1/') || typeof EventSource === 'undefined') {
    onState('off');
    return () => undefined;
  }
  const source = new EventSource(url);
  const handle = (message: MessageEvent) => {
    const event = liveEvent(message.data);
    if (event) onEvent(event);
  };
  source.addEventListener('open', () => onState('live'));
  source.addEventListener('error', () => onState(source.readyState === EventSource.CLOSED ? 'off' : 'reconnecting'));
  source.addEventListener('message', handle);
  for (const kind of KINDS) source.addEventListener(kind, handle as EventListener);
  return () => {
    source.close();
    onState('off');
  };
}
