import { test } from 'node:test';
import assert from 'node:assert/strict';
import { AuthorityLease, watchAuthority, AUTHORITY_LEASE_MS, AUTHORITY_TIMEOUT_MS } from '../src/shared/authority';
const allowed = { available: true as const, version: 1, can_add_to_canvas: false };
const key = '00000000-0000-4000-8000-000000000001.00000000-0000-4000-8000-000000000002';
test('authority expires from request start, not delayed arrival', () => {
  const lease = new AuthorityLease(); const ticket = lease.begin(100);
  assert.equal(lease.accept(ticket, allowed, 100 + AUTHORITY_TIMEOUT_MS - 1), true);
  assert.equal(lease.current(100 + AUTHORITY_LEASE_MS - 1), allowed);
  assert.equal(lease.current(100 + AUTHORITY_LEASE_MS), null);
  assert.equal(lease.accept(lease.begin(6000), allowed, 6000 + AUTHORITY_TIMEOUT_MS + 1), false);
  assert.equal(lease.current(8001), null);
});
test('superseded, repeated and invalidated responses cannot restore authority', () => {
  const lease = new AuthorityLease(); const old = lease.begin(100); const next = lease.begin(200);
  assert.equal(lease.accept(old, allowed, 300), false);
  assert.equal(lease.accept(next, { ...allowed, version: 2 }, 400), true);
  assert.equal(lease.accept(next, { ...allowed, version: 99 }, 500), false);
  assert.equal(lease.current(501)?.version, 2);
  const pending = lease.begin(600); lease.invalidate();
  assert.equal(lease.accept(pending, allowed, 700), false); assert.equal(lease.current(701), null);
});
test('only explicit safe authority verdicts are accepted', () => {
  for (const value of [null, {}, { ...allowed, available: false }, { ...allowed, version: 0 }, { ...allowed, version: 1.5 }, { ...allowed, version: Number.MAX_SAFE_INTEGER + 1 }, { ...allowed, can_add_to_canvas: 'true' }]) {
    const lease = new AuthorityLease(); assert.equal(lease.accept(lease.begin(0), value, 10), false); assert.equal(lease.current(10), null);
  }
});
test('monitor masks offline and hidden views; stale bodies cannot revive after reconnect', async () => {
  const saved = new Map<string, PropertyDescriptor | undefined>();
  const replace = (name: string, value: unknown) => { saved.set(name, Object.getOwnPropertyDescriptor(globalThis, name)); Object.defineProperty(globalThis, name, { value, configurable: true, writable: true }); };
  let now = 0; let next = 0;
  const timers = new Map<number, { at: number; interval: number; fn: () => void }>();
  const window_ = new EventTarget() as EventTarget & Record<string, unknown>;
  window_.setTimeout = (fn: () => void, delay: number) => { const id = ++next; timers.set(id, { at: now + delay, interval: 0, fn }); return id; };
  window_.setInterval = (fn: () => void, delay: number) => { const id = ++next; timers.set(id, { at: now + delay, interval: delay, fn }); return id; };
  window_.clearTimeout = window_.clearInterval = (id: number) => timers.delete(id);
  const document_ = new EventTarget() as EventTarget & { visibilityState: string }; document_.visibilityState = 'visible';
  const navigator_ = { onLine: true };
  const calls: { signal: AbortSignal; resolve(value: Response): void }[] = [];
  const masks: string[] = []; const grants: number[] = [];
  const settle = async () => { for (let i = 0; i < 8; i += 1) await Promise.resolve(); };
  const advance = async (ms: number) => {
    const until = now + ms;
    while (true) {
      const due = [...timers.entries()].filter(([, t]) => t.at <= until).sort((a, b) => a[1].at - b[1].at)[0]; if (!due) break;
      const [id, timer] = due; now = timer.at; if (timer.interval) timer.at += timer.interval; else timers.delete(id); timer.fn(); await settle();
    }
    now = until; await settle();
  };
  replace('window', window_); replace('navigator', navigator_); replace('performance', { now: () => now });
  replace('fetch', (_url: string, options: RequestInit) => new Promise<Response>(resolve => calls.push({ signal: options.signal!, resolve })));
  let monitor: ReturnType<typeof watchAuthority> | undefined;
  try {
    monitor = watchAuthority(key, { document: document_ as unknown as Document, active: () => true, mask: m => masks.push(m), authorized: v => grants.push(v.version) });
    assert.equal(calls.length, 1); assert.equal(monitor.current(), null); assert.equal(masks.length, 1);
    calls[0].resolve({ ok: true, json: async () => allowed } as Response); await settle(); assert.deepEqual(grants, [1]);
    await advance(2000); assert.equal(calls.length, 2);
    let releaseBody!: (value: unknown) => void;
    calls[1].resolve({ ok: true, json: () => new Promise(resolve => { releaseBody = resolve; }) } as Response); await settle();
    navigator_.onLine = false; window_.dispatchEvent(new Event('offline')); assert.equal(monitor.current(), null); assert.equal(calls[1].signal.aborted, true);
    navigator_.onLine = true; window_.dispatchEvent(new Event('online')); assert.equal(calls.length, 3);
    calls[2].resolve({ ok: true, json: async () => ({ ...allowed, version: 3 }) } as Response); await settle();
    releaseBody({ ...allowed, version: 2 }); await settle(); assert.equal(monitor.current()?.version, 3); assert.deepEqual(grants, [1, 3]);
    document_.visibilityState = 'hidden'; document_.dispatchEvent(new Event('visibilitychange')); assert.equal(monitor.current(), null);
    document_.visibilityState = 'visible'; document_.dispatchEvent(new Event('visibilitychange')); assert.equal(calls.length, 4); assert.equal(monitor.current(), null);
    calls[3].resolve({ ok: true, json: async () => ({ ...allowed, version: 4 }) } as Response); await settle(); assert.equal(monitor.current()?.version, 4);
    await advance(AUTHORITY_LEASE_MS + 100); assert.equal(monitor.current(), null); assert.ok(masks.length >= 4);
    window_.dispatchEvent(new Event('pagehide')); assert.equal(monitor.current(), null);
    monitor.dispose(); assert.equal(timers.size, 0);
  } finally { monitor?.dispose(); for (const [name, descriptor] of saved) { if (descriptor) Object.defineProperty(globalThis, name, descriptor); else Reflect.deleteProperty(globalThis, name); } }
});
