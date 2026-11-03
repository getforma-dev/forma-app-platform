// APP-113 regression tests for the canvas review fixes that run without a
// browser (Node's test runner through tsx; the DOM is faked per test). The
// browser-only fixes (swap, stacks, Space-drag, focus return) are covered by
// the browser walk recorded in the ticket's Log.
import assert from 'node:assert/strict';
import { test } from 'node:test';

type Listener = (event: unknown) => void;

test('shortcuts stand down while another modal owns the keyboard (finding 2)', async () => {
  const { modalOwnsKeys } = await import('../src/canvas/controls.ts');
  const page = (open: string[]) => ({
    querySelector: (selector: string) => (selector.split(',').some((part) => open.includes(part.trim())) ? {} : null),
    querySelectorAll: () => [],
  }) as unknown as Document;
  const key = (inside: string | null) => ({ target: null, key: 'Delete', _inside: inside }) as unknown as Event;
  // Nothing open: the canvas shortcuts run.
  assert.equal(modalOwnsKeys(key(null), page([])), false);
  // The file viewer and the data sheet (`openModal` marks `data-modal-above`).
  assert.equal(modalOwnsKeys(key(null), page(['[data-modal-above]'])), true);
  // The agents panel (`<dialog>` shown with `showModal`).
  assert.equal(modalOwnsKeys(key(null), page(['dialog[open]'])), true);
  // An open aria-modal layer that is not the canvas's own sheet or focus mode.
  const layer = { id: 'other', hidden: false, closest: () => null };
  const withLayer = { querySelector: () => null, querySelectorAll: () => [layer] } as unknown as Document;
  assert.equal(modalOwnsKeys(key(null), withLayer), true);
  const hiddenLayer = { querySelector: () => null, querySelectorAll: () => [{ ...layer, hidden: true }] } as unknown as Document;
  assert.equal(modalOwnsKeys(key(null), hiddenLayer), false);
});

test('a dock card shown over a pending one settles it (finding 3)', async () => {
  const { showDockCard } = await import('../src/canvas/dock-card.ts');
  let shown: unknown = null;
  const host = { replaceChildren: (card: unknown) => { shown = card; } };
  const document_ = { getElementById: () => host, activeElement: null } as unknown as Document;
  const card = () => ({ contains: () => false, remove: () => undefined }) as unknown as HTMLElement;
  const confirm = card();
  const chooser = card();
  let settled = 0;
  const closeConfirm = showDockCard(document_, confirm, () => { settled += 1; });
  assert.equal(shown, confirm);
  showDockCard(document_, chooser, () => undefined);
  assert.equal(shown, chooser);
  assert.equal(settled, 1, 'the replaced confirmation resolves, so the intent bar is not left on "Composing…"');
  closeConfirm();
  assert.equal(settled, 1);
});

test('ledger sums are exact per currency (finding 13)', async () => {
  const { totals, formatAmounts } = await import('../src/canvas/ledger.ts');
  const record = (minor: string, currency = 'USD', exponent = 2) => ({
    record_id: '', collection_id: 'c', schema_id: 's', version: 1, commit_sequence: 1, committed_at: '',
    creator: { principal_id: 'p', label: 'Maya' }, updater: { principal_id: 'p', label: 'Maya' },
    values: [{ field_id: 'amount', display: '', value: { type: 'money', minor_units: minor, currency, minor_unit_exponent: exponent } }],
  });
  const card = { collection: 'c', amount: 'amount', date: '', category: '', status: '' };
  const filter = { from: '', to: '', status: '' };
  const listing = (records: ReturnType<typeof record>[]) => ({ records } as never);
  // Beyond 2^53 minor units: Number would round.
  const big = totals(listing([record('9007199254740993'), record('1')]), card, filter);
  assert.equal(formatAmounts(big.total), '$90,071,992,547,409.94');
  // Currency and exponent come from the value, not a fixed "$x.yy".
  assert.equal(formatAmounts(totals(listing([record('1500', 'JPY', 0)]), card, filter).total), '¥1,500');
  assert.equal(formatAmounts(totals(listing([record('1050'), record('500', 'EUR')]), card, filter).total), '$10.50 + €5.00');
  assert.equal(totals(listing([record('1050'), record('500', 'EUR')]), card, filter).count, 2);
});

test('stored placements keep the most recent works when capped (finding 8)', async () => {
  const store = new Map<string, string>();
  (globalThis as { localStorage?: unknown }).localStorage = {
    getItem: (key: string) => store.get(key) ?? null,
    setItem: (key: string, value: string) => { store.set(key, value); },
  };
  const { loadPlaced, savePlaced } = await import('../src/canvas/layout.ts');
  const member = '0123456789abcdef';
  const uuid = (n: number) => `00000000-0000-4000-8000-${String(n).padStart(12, '0')}`;
  const placed = new Map<string, { app: string; cards: string[] }>();
  for (let n = 0; n < 70; n += 1) placed.set(uuid(n), { app: uuid(1000), cards: [`v-${uuid(n)}`] });
  savePlaced(member, placed);
  const back = loadPlaced(member);
  assert.equal(back.size, 64);
  assert.ok(back.has(uuid(69)), 'the newest work is kept');
  assert.ok(!back.has(uuid(0)), 'the oldest work is dropped');
});

test('the live stream reopens after the browser closes it, and asks for a catch-up (finding 5)', async () => {
  const timers: (() => void)[] = [];
  (globalThis as { window?: unknown }).window = {
    setTimeout: (run: () => void) => timers.push(run),
    clearTimeout: () => undefined,
  };
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
  const { subscribeLive } = await import('../src/canvas/live.ts');
  const states: string[] = [];
  const events: string[] = [];
  subscribeLive('/v1/events', (event) => events.push(event.kind), (state) => states.push(state));
  const first = FakeSource.all[0]!;
  first.fire('open');
  assert.deepEqual(events, [], 'the first open is not a catch-up');
  // An HTTP error: the browser closes the stream for good.
  first.readyState = FakeSource.CLOSED;
  first.fire('error');
  assert.equal(states.at(-1), 'reconnecting');
  assert.equal(timers.length, 1);
  timers.shift()!();
  const second = FakeSource.all[1];
  assert.ok(second, 'a new stream is opened after the backoff');
  second.fire('open');
  assert.deepEqual(events, ['stream.lagged'], 'the board re-runs what it shows after a reconnect');
  assert.equal(states.at(-1), 'live');
});
