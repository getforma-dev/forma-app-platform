// The tabs of one browser share one live stream (2026-10-31: two tabs each
// holding a stream replaced each other on the server, so the chip flapped
// between Live and Reconnecting).
import { test } from 'node:test';
import assert from 'node:assert/strict';

type Listener = (event: unknown) => void;

test('two tabs share one live stream, and the next tab takes over when the first goes', async () => {
  (globalThis as { window?: unknown }).window = { setTimeout: () => 0, clearTimeout: () => undefined };
  class FakeSource {
    static CLOSED = 2;
    static all: FakeSource[] = [];
    readyState = 0;
    listeners = new Map<string, Listener[]>();
    constructor(readonly url: string) { FakeSource.all.push(this); }
    addEventListener(kind: string, listener: Listener) { this.listeners.set(kind, [...(this.listeners.get(kind) ?? []), listener]); }
    close() { this.readyState = FakeSource.CLOSED; }
    fire(kind: string, event: unknown = {}) { for (const listener of this.listeners.get(kind) ?? []) listener(event); }
  }
  (globalThis as { EventSource?: unknown }).EventSource = FakeSource;
  // A Web Locks stand-in: one holder per name, the next waiter runs when it lets go.
  const queue: (() => Promise<void>)[] = [];
  let held = false;
  const next = () => {
    const run = queue.shift();
    if (!run) return;
    held = true;
    void run().then(() => { held = false; next(); });
  };
  const locks = { request: (_name: string, run: () => Promise<void>) => { queue.push(run); if (!held) next(); return Promise.resolve(); } };
  const navigator = (globalThis as { navigator?: Record<string, unknown> }).navigator;
  const before = Object.getOwnPropertyDescriptor(globalThis, 'navigator');
  Object.defineProperty(globalThis, 'navigator', { value: { ...navigator, locks }, configurable: true });
  try {
    const { subscribeLive } = await import('../src/canvas/live.ts');
    const settle = () => new Promise((resolve) => setTimeout(resolve, 20));
    const a = { states: [] as string[], events: [] as string[] };
    const b = { states: [] as string[], events: [] as string[] };
    const stopA = subscribeLive('/v1/events', (e) => a.events.push(e.kind), (s) => a.states.push(s));
    const stopB = subscribeLive('/v1/events', (e) => b.events.push(e.kind), (s) => b.states.push(s));
    await settle();
    assert.equal(FakeSource.all.length, 1, 'one stream for both tabs');
    const first = FakeSource.all[0]!;
    first.fire('open');
    first.fire('record.committed', { data: JSON.stringify({ kind: 'record.committed', app_id: '00000000-0000-4000-8000-000000000001', work_id: '00000000-0000-4000-8000-000000000002' }) });
    await settle();
    assert.equal(a.states.at(-1), 'live');
    assert.equal(b.states.at(-1), 'live', 'the other tab shows the leader\'s state');
    assert.deepEqual(a.events, ['record.committed']);
    assert.deepEqual(b.events, ['record.committed'], 'the other tab gets the relayed event');
    stopA();
    await settle();
    assert.equal(FakeSource.all.length, 2, 'the next tab opens the stream when the first goes');
    assert.ok(b.events.includes('stream.lagged'), 'and catches up on what it missed');
    assert.notEqual(b.states.at(-1), 'off', 'a hand-over is not shown as offline');
    stopB();
  } finally {
    if (before) Object.defineProperty(globalThis, 'navigator', before);
  }
});
