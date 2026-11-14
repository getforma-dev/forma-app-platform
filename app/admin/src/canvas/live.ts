// Live updates (seam for `pivot/realtime`): one server-sent-events stream per
// organization, `GET /v1/events` under the session (the page names the URL;
// empty means no stream yet). Each event is data: a closed kind and two ids.
// Nothing in an event is rendered; it only tells the board which cards to
// re-run (through the governed query route) or which work to place (through
// `GET /v1/canvas/works/{app}/{work}`, which re-authorizes).
import { isUuid } from './ids';

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

// --- APP-195: when a refresh that a live hint triggered runs, and its one retry.
//
// One save on a shared page sends the same hint to every member who has the
// page open. When every page re-read at once, the data pool refused most of
// them (APP-193 finding, G6). So a hinted refresh starts after a uniform random
// delay of 0–3 s, and a read that fails with a 503 or a network error is tried
// once more after a uniform random 1–3 s before the page shows that it failed.
// The page that committed reads at once. Refreshes the member starts (Refresh,
// coming back to the tab, coming back online) do not use this section.
// The random source and the clock are injectable, so tests are exact.

/** Hinted refreshes start uniformly within this window after the hint. */
export const HINT_SPREAD_MS = 3000;
/** The one retry waits uniformly between these. */
export const RETRY_MIN_MS = 1000;
export const RETRY_MAX_MS = 3000;

export type Timing = {
  /** Uniform in [0, 1). */
  random(): number;
  setTimeout(run: () => void, ms: number): unknown;
  clearTimeout(handle: unknown): void;
};
export const browserTiming: Timing = {
  random: () => Math.random(),
  setTimeout: (run, ms) => globalThis.setTimeout(run, ms),
  clearTimeout: (handle) => globalThis.clearTimeout(handle as ReturnType<typeof setTimeout>),
};

export type Fetch = (input: string, init?: RequestInit) => Promise<Response>;

/** A whole number of milliseconds in [from, to), whatever the source returns. */
function uniform(timing: Timing, from: number, to: number): number {
  const value = timing.random();
  const unit = Number.isFinite(value) && value > 0 ? Math.min(value, 1) : 0;
  return Math.min(to - 1, from + Math.floor(unit * (to - from)));
}

/** A hinted refresh's start delay, in [0, 3000) ms. */
export function hintDelay(timing: Timing = browserTiming): number {
  return uniform(timing, 0, HINT_SPREAD_MS);
}

/** The wait before the one retry, in [1000, 3000) ms. */
export function retryDelay(timing: Timing = browserTiming): number {
  return uniform(timing, RETRY_MIN_MS, RETRY_MAX_MS);
}

/**
 * Hinted refreshes, at most one waiting per key (a page, or one work on the
 * canvas). The first hint for a key waits 0–3 s; hints for the key while it
 * waits join it, because the read that starts later covers them all. `now`
 * runs at once in place of a waiting hint (the page that committed), for the
 * same reason. This is the hints' coalescing; each caller keeps its own.
 */
export function hintSchedule(timing: Timing = browserTiming) {
  const waiting = new Map<string, unknown>();
  const cancel = (key: string) => {
    if (!waiting.has(key)) return;
    timing.clearTimeout(waiting.get(key));
    waiting.delete(key);
  };
  return {
    hint(key: string, run: () => void): void {
      if (waiting.has(key)) return;
      waiting.set(key, timing.setTimeout(() => {
        waiting.delete(key);
        run();
      }, hintDelay(timing)));
    },
    now(key: string, run: () => void): void {
      cancel(key);
      run();
    },
    waiting: (key: string) => waiting.has(key),
    clear(): void {
      for (const key of [...waiting.keys()]) cancel(key);
    },
  };
}

function pause(timing: Timing, ms: number, signal?: AbortSignal | null): Promise<void> {
  return new Promise((resolve, reject) => {
    if (signal?.aborted) {
      reject(signal.reason);
      return;
    }
    const stop = () => {
      timing.clearTimeout(handle);
      reject(signal?.reason);
    };
    const handle = timing.setTimeout(() => {
      signal?.removeEventListener('abort', stop);
      resolve();
    }, ms);
    signal?.addEventListener('abort', stop, { once: true });
  });
}

/**
 * `request` that, after a 503 or a network error, waits 1–3 s and asks once
 * more: the second answer, or its error, is final and the page shows it as
 * before. Any other answer is returned at once. A read whose signal was
 * aborted (the page stopped, or its own timeout) is never retried.
 */
export function retryOnce(timing: Timing = browserTiming, request: Fetch = (input, init) => fetch(input, init)): Fetch {
  return async (input, init) => {
    try {
      const response = await request(input, init);
      if (response.status !== 503) return response;
    } catch (error) {
      if (init?.signal?.aborted) throw error;
    }
    await pause(timing, retryDelay(timing), init?.signal);
    return request(input, init);
  };
}
