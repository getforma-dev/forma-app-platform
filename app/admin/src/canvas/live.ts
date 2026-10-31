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

type LiveState = 'live' | 'reconnecting' | 'off';
type Relay = { type: 'hello' } | { type: 'state'; state: LiveState } | { type: 'event'; data: string };

/**
 * Subscribes to `url` (same origin, `/v1/` only); returns the unsubscribe.
 *
 * The server keeps one stream per session (events, review M1: a newer stream
 * replaces the earlier one), and every tab of a browser shares the session
 * cookie. Two tabs each holding a stream replaced each other forever: "Live",
 * then "Reconnecting", over and over (2026-10-31). So the tabs share one: the
 * tab holding a Web Lock named for the stream opens it and relays its events
 * and state to the others on a BroadcastChannel; when that tab goes, the next
 * one takes the lock and the stream (and reports `stream.lagged`, since hints
 * in between were lost). Without Web Locks or BroadcastChannel each tab opens
 * its own stream, as before.
 */
export function subscribeLive(
  url: string,
  onEvent: (event: LiveEvent) => void,
  onState: (state: LiveState) => void,
): () => void {
  const locks = (globalThis.navigator as Navigator | undefined)?.locks;
  if (!url.startsWith('/v1/') || !locks || typeof BroadcastChannel === 'undefined') return streamLive(url, onEvent, onState);
  const name = `forma-live:${url}`;
  const channel = new BroadcastChannel(name);
  let stopped = false;
  let leading: (() => void) | null = null;
  let leaderState: LiveState = 'reconnecting';
  let heardLeader = false;
  let release: (() => void) | null = null;
  const relay = (message: Relay) => channel.postMessage(message);
  channel.onmessage = (message: MessageEvent<Relay>) => {
    const data = message.data;
    if (leading) {
      if (data?.type === 'hello') relay({ type: 'state', state: leaderState });
      return;
    }
    heardLeader = true;
    if (data?.type === 'state' && (data.state === 'live' || data.state === 'reconnecting')) onState(data.state);
    else if (data?.type === 'event') {
      const event = liveEvent(data.data);
      if (event) onEvent(event);
    }
  };
  onState('reconnecting');
  relay({ type: 'hello' });
  void locks.request(name, () => new Promise<void>((resolve) => {
    if (stopped) {
      resolve();
      return;
    }
    release = resolve;
    // Another tab led until now: what it relayed after it went is lost.
    if (heardLeader) onEvent({ kind: 'stream.lagged' });
    leading = streamLive(url, (event) => {
      onEvent(event);
      relay({ type: 'event', data: JSON.stringify(event) });
    }, (state) => {
      onState(state);
      // A leader that stops hands over: the others see it reconnect, not go offline.
      leaderState = state === 'off' ? 'reconnecting' : state;
      relay({ type: 'state', state: leaderState });
    });
  }));
  return () => {
    stopped = true;
    leading?.();
    leading = null;
    release?.();
    channel.close();
    onState('off');
  };
}

/**
 * One tab's own stream.
 * APP-113: hints sent while the stream was down are lost, so every re-open
 * after the first is reported as `stream.lagged` (the board re-runs what it
 * shows); a stream the browser closed (an HTTP error) is reopened with backoff.
 */
function streamLive(
  url: string,
  onEvent: (event: LiveEvent) => void,
  onState: (state: LiveState) => void,
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
