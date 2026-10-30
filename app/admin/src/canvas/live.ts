// Live updates (seam for `pivot/realtime`): one server-sent-events stream per
// organization, `GET /v1/events` under the session (the page names the URL;
// empty means no stream yet). Each event is data: a closed kind and two ids.
// Nothing in an event is rendered; it only tells the board which cards to
// re-run (through the governed query route) or which work to place (through
// `GET /v1/canvas/works/{app}/{work}`, which re-authorizes).
import { isUuid } from './render';

export type LiveKind = 'record.committed' | 'app.shared' | 'app.placed' | 'app.adapted' | 'app.kept' | 'stream.lagged';
/** An announcement with ids, or the lag marker (`stream.lagged`, no ids): the
 * server dropped hints for this stream, so the board re-runs every card it shows. */
// APP-101: `app.adapted` names the adapted work and the standard work it was
// tailored from (`from_*`); `app.kept` names the standard work whose
// transform kept the defaults.
export type LiveEvent =
  | { kind: 'record.committed' | 'app.shared' | 'app.placed' | 'app.kept'; app_id: string; work_id: string }
  | { kind: 'app.adapted'; app_id: string; work_id: string; from_app_id: string; from_work_id: string }
  | { kind: 'stream.lagged' };
const KINDS: readonly LiveKind[] = ['record.committed', 'app.shared', 'app.placed', 'app.adapted', 'app.kept', 'stream.lagged'];

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
  if (!KINDS.includes(event.kind as LiveKind)) return null;
  if (event.kind === 'stream.lagged') return { kind: 'stream.lagged' };
  if (!isUuid(event.app_id) || !isUuid(event.work_id)) return null;
  if (event.kind === 'app.adapted') {
    if (!isUuid(event.from_app_id) || !isUuid(event.from_work_id)) return null;
    return { kind: 'app.adapted', app_id: event.app_id, work_id: event.work_id, from_app_id: event.from_app_id, from_work_id: event.from_work_id };
  }
  return { kind: event.kind as 'record.committed' | 'app.shared' | 'app.placed' | 'app.kept', app_id: event.app_id, work_id: event.work_id };
}

/** Reconnect delays after the browser gave up on the stream (an HTTP error closes an EventSource for good). */
const BACKOFF_MS = [1000, 2000, 5000, 10000, 30000];

/**
 * Subscribes to `url` (same origin, `/v1/` only); returns the unsubscribe.
 * APP-113: hints sent while the stream was down are lost, so every re-open
 * after the first is reported as `stream.lagged` (the board re-runs what it
 * shows); a stream the browser closed (an HTTP error) is reopened with backoff.
 */
export function subscribeLive(
  url: string,
  onEvent: (event: LiveEvent) => void,
  onState: (state: 'live' | 'reconnecting' | 'off') => void,
): () => void {
  if (!url.startsWith('/v1/') || typeof EventSource === 'undefined') {
    onState('off');
    return () => undefined;
  }
  let source: EventSource | null = null;
  let opened = false;
  let failures = 0;
  let timer = 0;
  let stopped = false;
  const handle = (message: MessageEvent) => {
    const event = liveEvent(message.data);
    if (event) onEvent(event);
  };
  const connect = () => {
    timer = 0;
    if (stopped) return;
    const next = new EventSource(url);
    source = next;
    next.addEventListener('open', () => {
      onState('live');
      failures = 0;
      if (opened) onEvent({ kind: 'stream.lagged' });
      opened = true;
    });
    next.addEventListener('error', () => {
      if (next.readyState !== EventSource.CLOSED) {
        onState('reconnecting');
        return;
      }
      // The browser will not retry this one: reopen it ourselves, slower each time.
      next.close();
      if (stopped || timer) return;
      onState('reconnecting');
      timer = window.setTimeout(connect, BACKOFF_MS[Math.min(failures, BACKOFF_MS.length - 1)]);
      failures += 1;
    });
    next.addEventListener('message', handle);
    for (const kind of KINDS) next.addEventListener(kind, handle as EventListener);
  };
  connect();
  return () => {
    stopped = true;
    window.clearTimeout(timer);
    source?.close();
    onState('off');
  };
}
