import { applyPresentationTheme } from '../presentation/presentation';
import { recordComponentsFor } from '../record-components/controller';
// The canvas island (ADR-011; design D10: one canvas island, geometry and
// keyboard from genui-runtime, vendored in `../canvas-vendor`). It adopts the
// server-rendered cards onto the pan/zoom surface, runs every view through
// the governed query route, posts record forms through the audited command
// route, places works the composer publishes or a teammate shares, and keeps
// the view layout in this browser only. It never computes a number itself.
import {
  WidgetCanvas,
  createCanvasItem,
  createCanvasSurface,
  type WidgetCanvasItemState,
  type WidgetCanvasDragEnd,
} from '../canvas-vendor/index';
import { decorateAppCard } from './address-chip';
import { submitCommand } from './commands';
import { clearLayouts, loadCamera, loadLayout, loadPlaced, mergeArtifactKinds, saveCamera, saveLayout, savePlaced, loadStandaloneWorks, saveStandaloneWorks, type ItemLayout, type PlacedWork } from './layout';
import { hintSchedule, retryOnce, subscribeLive, type Fetch, type LiveEvent } from './live';
import { parametersOf, runView } from './queries';
import { announce, buildForm, buildGroup, el, groupJson, isUuid, pulseTarget, renderNoValue, type FormJson } from './render';
import { refreshRecords, wireRecordsCard } from './records';
import { refreshCommands } from './slash';
import { refreshLedger, wireLedgerCard } from './ledger';
import { wireReceiptAttach } from './receipt';
import { startConnectedApps } from '../shared/canvas-controller';

const recordControllers = new Map<string, ReturnType<typeof recordComponentsFor>>();

export type Group = {
  app: string;
  work: string;
  title: string;
  section: HTMLElement;
  /** Every card of the work, in canvas order, on the board or not (APP-104). */
  cards: HTMLElement[];
  /** The canvas items of the cards on the board, in canvas order. */
  items: HTMLElement[];
  /** The member arranged this group (now or in an earlier visit): its layout is kept. */
  pinned: boolean;
  /** Saved positions by card id (DS-06). */
  saved: Map<string, Partial<WidgetCanvasItemState>>;
};

const WIDTH: Record<string, number> = { app: 340, summary: 400, timeline: 620, form: 360, records: 680, ledger: 520 };
const HEIGHT: Record<string, number> = { app: 300, summary: 250, timeline: 340, form: 300, records: 260, ledger: 280 };
/** The size a card asks the canvas for, before its result changes the height. */
export function cardFootprint(kind: string): { width: number; height: number } {
  return { width: WIDTH[kind] ?? 440, height: HEIGHT[kind] ?? 300 };
}
/** The gap between cards in a row, in a fan and in a frame (board, `stacks.ts`, `unite.ts`). */
export const GAP = 28;
/** A stack's width, and the column a fanned or broken-apart card takes (`stacks.ts`, `unite.ts`). */
export const SLOT_WIDTH = 460;
/** The row height of a stack's fan-out grid (`stacks.ts`). */
export const FAN_ROW_HEIGHT = 380;
const ROW_GAP = 72;
const ORIGIN = 64;
/** Live re-runs of one work within this window coalesce into one (APP-113). */
const LIVE_RUN_MS = 250;

let canvas: WidgetCanvas | null = null;
let host: HTMLElement | null = null;
const groups = new Map<string, Group>();
let saveTimer = 0;
let reflowFrame = 0;
let fitPending = false;
// A card's last laid-out height, kept as `--cv-card-height` so a parked card
// (content-visibility: hidden) keeps its size for the re-flow.
const resized = typeof ResizeObserver === 'undefined' ? null : new ResizeObserver((entries) => {
  for (const entry of entries) {
    const height = Math.round(entry.borderBoxSize?.[0]?.blockSize ?? entry.contentRect.height);
    const item = entry.target as HTMLElement;
    if (height > 0 && item.dataset.attentionState !== 'parked') item.style.setProperty('--cv-card-height', `${height}px`);
  }
  scheduleReflow();
});
// APP-104: the member's placed cards (DS-67), the records cards shown only
// because a record waits for the member (not kept), and what this session
// took off the board on purpose (an auto card stays off until reload).
let member = '';
let placed = new Map<string, PlacedWork>();
let standaloneWorks = new Set<string>();
/** Cards the server confirmed (the page's `card` rows and this run's saved writes). */
const confirmedCards = new Set<string>();
const loadingGroups = new Map<string, Promise<Group | null>>();
const placementGeneration = new Map<string, number>();
/** Personal Library classification; no authority or server definition is inferred. */
export function isStandaloneWork(app: string, work: string): boolean { return standaloneWorks.has(`${app}/${work}`); }
/**
 * The member's Card or App choice for one whole work (APP-169). It shows at
 * once and is saved on the account. A refused or failed save puts the last
 * saved choice back and says so. Resolves `true` once the newest choice for
 * that work is saved, `false` when a save in its queue failed.
 */
export function setStandaloneWork(app: string, work: string, standalone: boolean): Promise<boolean> {
  if (!isUuid(app) || !isUuid(work)) return Promise.resolve(false);
  const key = `${app}/${work}`;
  const before = standaloneWorks.has(key);
  showStandalone(key, standalone);
  return queueArtifactKind(app, work, standalone, before, 'member');
}
function showStandalone(key: string, standalone: boolean): void {
  if (standalone) standaloneWorks.add(key); else standaloneWorks.delete(key);
  saveStandaloneWorks(member, standaloneWorks, confirmedCards);
  emitCanvasChanged();
}
// One kind write per work at a time, so an earlier click can never land after
// a later one. `saved` is the last kind the server confirmed in this run
// (null before the first), `prior` the choice shown before the first queued
// change, `wanted` the newest choice and `mode` who asked for it: the member
// (undone and announced on failure), creation intent (kept on failure for the
// next load), or the first-load upload (kept on failure, and sent `if_absent`
// so it never replaces a choice saved elsewhere). `asked` counts choices, so a
// failed older write does not stop a newer one: only the newest choice's
// failure ends the queue, and any message names that choice.
type KindMode = 'member' | 'intent' | 'upload';
type KindWrite = { prior: boolean; saved: boolean | null; wanted: boolean; mode: KindMode; asked: number; running: Promise<boolean> | null };
const kindWrites = new Map<string, KindWrite>();
function queueArtifactKind(app: string, work: string, card: boolean, prior: boolean, mode: KindMode): Promise<boolean> {
  const key = `${app}/${work}`;
  const entry = kindWrites.get(key) ?? { prior, saved: null, wanted: card, mode, asked: 0, running: null };
  kindWrites.set(key, entry);
  entry.wanted = card;
  entry.mode = mode;
  entry.asked += 1;
  entry.running ??= drainArtifactKind(app, work, key, entry);
  return entry.running;
}
async function drainArtifactKind(app: string, work: string, key: string, entry: KindWrite): Promise<boolean> {
  try {
    while (entry.wanted !== entry.saved) {
      const card = entry.wanted;
      const upload = entry.mode === 'upload';
      const asked = entry.asked;
      const stored = await postArtifactKind(app, work, card, upload);
      if (stored === null) {
        // A newer choice arrived while this one was in flight: send that one.
        if (entry.asked !== asked) continue;
        if (entry.mode === 'member') refuseArtifactKind(work, key, card, entry.saved ?? entry.prior);
        return false;
      }
      entry.saved = stored;
      if (stored) confirmedCards.add(key); else confirmedCards.delete(key);
      // The upload found a choice already saved (another device, after this
      // page rendered). Unless the member chose since, that choice wins.
      if (stored !== card && upload && entry.mode === 'upload') {
        entry.wanted = stored;
        showStandalone(key, stored);
      }
    }
    return true;
  } finally {
    kindWrites.delete(key);
  }
}
/** The kind the server holds after the write (`true` is a Card), or `null` when it was refused or failed. */
async function postArtifactKind(app: string, work: string, card: boolean, ifAbsent: boolean): Promise<boolean | null> {
  const kind = card ? 'card' : 'app';
  try {
    const response = await fetch(`/v1/library/${app}/${work}/kind`, {
      method: 'POST',
      credentials: 'same-origin',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify(ifAbsent ? { kind, if_absent: true } : { kind }),
    });
    if (!response.ok) return null;
    const body = await response.json() as { kind?: unknown } | null;
    if (body?.kind === kind) return card;
    // Only an `if_absent` write may answer with the choice already saved.
    return ifAbsent && (body?.kind === 'card' || body?.kind === 'app') ? body.kind === 'card' : null;
  } catch {
    return null;
  }
}
function refuseArtifactKind(work: string, key: string, card: boolean, restore: boolean): void {
  if (standaloneWorks.has(key) !== restore) showStandalone(key, restore);
  const document_ = host?.ownerDocument ?? (typeof document === 'undefined' ? null : document);
  if (!document_) return;
  const title = groups.get(work)?.title || document_.getElementById(`lib-${work}`)?.querySelector('.cv-lib-app-title')?.textContent || 'This work';
  announce(`${title} could not be saved as ${card ? 'a Card' : 'an App'}. It stays ${restore ? 'a Card' : 'an App'}. Try again.`, document_);
}
/** First-load uploads per page load; the rest wait for a later load (the member write window is 120 an hour). */
const UPLOADS_PER_LOAD = 16;
/**
 * APP-169: adopt the member's saved Card or App choices from the canvas page.
 * `data-artifact-kinds="true"` on the canvas says the server read them; each
 * library row's `data-artifact-kind` is `card`, `app`, or absent when nothing
 * is saved. Only a saved `app` drops a local Card. A local Card with nothing
 * saved stays and is uploaded, one at a time and at most `UPLOADS_PER_LOAD`
 * per load, stopping at the first failure. Without the page signal (the read
 * was unavailable) the local choices stay as they are and nothing is sent.
 */
function adoptServerKinds(document_: Document): void {
  if (host?.dataset.artifactKinds !== 'true') return;
  const rows = Array.from(document_.querySelectorAll<HTMLElement>('.cv-lib-app[data-app]'), (row) => ({
    app: row.dataset.app ?? '',
    work: row.dataset.work ?? '',
    kind: row.dataset.artifactKind ?? null,
  }));
  const merged = mergeArtifactKinds(standaloneWorks, rows);
  standaloneWorks = merged.cards;
  for (const row of rows) {
    if (row.kind === 'card') confirmedCards.add(`${row.app}/${row.work}`);
  }
  saveStandaloneWorks(member, standaloneWorks, confirmedCards);
  void uploadLocalCards(merged.upload.slice(0, UPLOADS_PER_LOAD));
}
async function uploadLocalCards(keys: readonly string[]): Promise<void> {
  for (const key of keys) {
    // The member changed this work since the page loaded: their write is sent instead.
    if (!standaloneWorks.has(key) || kindWrites.has(key)) continue;
    const [app, work] = key.split('/');
    if (!await queueArtifactKind(app!, work!, true, true, 'upload')) return;
  }
}
/**
 * Records explicit creation intent (Make a card, Keep data, a replaced Card
 * work) in this browser (ADR-018) and saves it on the account. A failed save
 * is not undone: the Card stays here and the next load uploads it.
 */
export function markStandaloneWork(app: string, work: string): Promise<boolean> {
  if (!isUuid(app) || !isUuid(work)) return Promise.resolve(false);
  const key = `${app}/${work}`;
  const before = standaloneWorks.has(key);
  showStandalone(key, true);
  return queueArtifactKind(app, work, true, before, 'intent');
}
const dismissed = new Set<string>();
const changeListeners: Array<() => void> = [];
const utilityItems = new Set<HTMLElement>();
// APP-101: placements in flight (an `app.adapted` for a standard work still
// being placed waits for it), works whose transform is pending, adapted ⇄
// standard pairs ("back to standard"), and adaptations that arrived before
// their standard work was placed here.
const placing = new Map<string, Promise<string>>();
const tailoring = new Map<string, number>();
type Ref = { app: string; work: string };
const pairs = new Map<string, { standard: Ref; adapted: Ref }>();
const early = new Map<string, Ref>();
// `app.kept` that arrived before the card's tailoring state was shown.
const keptEarly = new Set<string>();
/** How long the canvas waits for `app.adapted` / `app.kept` before keeping the defaults. */
const TAILOR_WAIT_MS = 15000;

/**
 * The default layout: one row per app, its cards left to right, each row
 * below the tallest card of the one above. Cards grow as results arrive, so
 * rows re-flow until the member arranges a group by hand.
 */
function reflow(): void {
  reflowFrame = 0;
  if (!canvas) return;
  // Read every size first (one layout), then write every position, with the
  // canvas's visibility pass deferred to one at the end.
  const sizes = new Map<HTMLElement, [number, number]>();
  for (const group of groups.values()) {
    for (const item of rowItems(group)) sizes.set(item, [item.offsetWidth, item.offsetHeight]);
  }
  const flush = canvas.deferDerivedUpdates(false);
  let y = ORIGIN;
  for (const group of groups.values()) {
    if (!rowItems(group).length) continue;
    let bottom = y;
    if (group.pinned) {
      for (const item of rowItems(group)) {
        bottom = Math.max(bottom, Number(item.dataset.canvasY ?? 0) + (sizes.get(item)?.[1] ?? 0));
      }
    } else {
      let x = ORIGIN;
      for (const item of rowItems(group)) {
        const [width, height] = sizes.get(item) ?? [0, 0];
        if (Number(item.dataset.canvasX) !== x || Number(item.dataset.canvasY) !== y) canvas.moveItemTo(item, x, y);
        x += width + GAP;
        bottom = Math.max(bottom, y + height);
      }
    }
    y = bottom + ROW_GAP;
  }
  flush();
  if (fitPending) canvas.fitAll();
}
/** A group's items in its row: a stack's fanned-out cards float apart. */
function rowItems(group: Group): HTMLElement[] {
  return group.items.filter((item) => !item.dataset.fanned);
}
function scheduleReflow(): void {
  if (!reflowFrame) reflowFrame = requestAnimationFrame(reflow);
}

/**
 * DS-40: `GENUI_DEMO_MODE` is on for this page (`data-demo="on"`), which the
 * server sets only for a listed demo organization with the switch on.
 */
function demo(): boolean {
  return host?.dataset.demo === 'on';
}

function nextRow(): number {
  let bottom = ORIGIN;
  for (const group of groups.values()) {
    for (const item of group.items) {
      bottom = Math.max(bottom, Number(item.dataset.canvasY ?? 0) + item.offsetHeight + ROW_GAP);
    }
  }
  for (const item of host?.querySelectorAll<HTMLElement>('.cv-item-stack, .cv-item-unite') ?? []) {
    bottom = Math.max(bottom, Number(item.dataset.canvasY ?? 0) + item.offsetHeight + ROW_GAP);
  }
  for (const item of utilityItems) bottom = Math.max(bottom, Number(item.dataset.canvasY ?? 0) + item.offsetHeight + ROW_GAP);
  return bottom;
}

// --- APP-103: utility widgets (`./widgets`) share the canvas surface. ---
// A widget is a client-side card with no governed data; the board only
// mounts it, keeps it out of the app rows and tells the widgets module when
// the member moved something (its layout is the widgets module's, DS-06).
const widgetItems = new Set<HTMLElement>();
const layoutListeners: Array<() => void> = [];
const widgetAnchor = typeof ResizeObserver === 'undefined' ? null : new ResizeObserver((entries) => {
  if (!canvas) return;
  for (const entry of entries) {
    const item = entry.target as HTMLElement;
    if (item.dataset.widgetPinned || !widgetItems.has(item)) {
      widgetAnchor?.unobserve(item);
      continue;
    }
    canvas.moveItemTo(item, Number(item.dataset.canvasX ?? ORIGIN), ORIGIN - ROW_GAP - item.offsetHeight);
  }
});

/** The canvas host, once the board is started (null before). */
export function canvasHost(): HTMLElement | null {
  return canvas ? host : null;
}

/** DS-40: whether this page runs the pitch's demo mode (`GENUI_DEMO_MODE`). */
export function isDemoCanvas(): boolean {
  return demo();
}

/** Calls `listener` whenever the board saves the layout (after a move). */
export function onLayoutSaved(listener: () => void): void {
  layoutListeners.push(listener);
}

export type CanvasDragEnd = WidgetCanvasDragEnd;
const dragEndListeners: ((item: HTMLElement, completion: CanvasDragEnd) => void)[] = [];
/** Finishes every moved gesture; only `released` may trigger a drop effect. */
export function onCanvasDragEnd(listener: (item: HTMLElement, completion: CanvasDragEnd) => void): void {
  dragEndListeners.push(listener);
}

/** The canvas item the member selected last, if any. */
export function activeCanvasItem(): HTMLElement | null {
  return canvas?.activeItem() ?? null;
}

/**
 * The widget row sits above the app rows (which flow down from ORIGIN), so
 * the apps' re-flow never overlaps a widget: left to right, bottoms aligned
 * one row gap above the first app row.
 */
function nextWidgetX(): number {
  let x = ORIGIN;
  for (const item of widgetItems) {
    x = Math.max(x, Number(item.dataset.canvasX ?? ORIGIN) + item.offsetWidth + GAP);
  }
  return x;
}

/**
 * Mounts a widget card at `at` (a restored position) or at the next free
 * spot, and returns its canvas item (null before the board starts).
 */
export function mountWidget(
  content: HTMLElement,
  options: { id: string; title: string; width: number; minHeight: number; at?: { x: number; y: number }; focus?: boolean },
): HTMLElement | null {
  if (!canvas || !host) return null;
  const item = createCanvasItem({
    instanceId: options.id,
    displayName: options.title,
    preferredWidth: options.width,
    minHeight: options.minHeight,
    content,
  });
  item.classList.add('cv-item', 'cv-item-widget');
  const x = nextWidgetX();
  canvas.mountItem(item, options.at ?? { x, y: ORIGIN - ROW_GAP - options.minHeight }, { focus: false });
  if (!options.at) {
    item.classList.add('is-placing');
    canvas.moveItemTo(item, x, ORIGIN - ROW_GAP - Math.max(item.offsetHeight, options.minHeight));
  }
  widgetItems.add(item);
  // A widget the member has not moved keeps its bottom on the widget row as
  // a restyle changes its height (a TI-84 is taller), so it never overlaps the apps.
  if (!options.at) {
    widgetAnchor?.observe(item);
  } else {
    item.dataset.widgetPinned = 'true';
  }
  updateSummary();
  if (options.focus !== false) {
    canvas.setActive(item);
    canvas.centerItem(item);
  }
  return item;
}

/** Removes a widget's canvas item. */
export function unmountWidget(item: HTMLElement): void {
  if (!canvas || !widgetItems.delete(item)) return;
  widgetAnchor?.unobserve(item);
  canvas.removeItem(item, { selectFallback: false });
  updateSummary();
}

/** A widget item's position on the canvas. */
export function widgetSpot(item: HTMLElement): { x: number; y: number } | null {
  if (!canvas || !widgetItems.has(item)) return null;
  const state = canvas.getItemState(item);
  return { x: state.x, y: state.y };
}

function saveSoon(): void {
  if (!canvas) return;
  clearTimeout(saveTimer);
  saveTimer = window.setTimeout(() => {
    if (!canvas) return;
    for (const group of groups.values()) {
      if (!group.pinned) continue;
      const items: Record<string, ItemLayout> = {};
      for (const item of group.items) {
        const id = item.dataset.instanceId;
        if (!id) continue;
        const state = canvas.getItemState(item);
        items[id] = { x: state.x, y: state.y, z: state.z, manualScale: state.manualScale };
      }
      saveLayout(group.app, group.work, items);
    }
    saveCamera(canvas.getCamera());
    for (const listener of layoutListeners) listener();
  }, 400);
}

function updateSummary(): void {
  const chip = host?.ownerDocument.getElementById('cv-summary');
  if (chip) {
    let apps = 0, cards = 0;
    for (const group of groups.values()) if (group.items.length || uniteHooks.hasWork(group.work)) {
      if (isStandaloneWork(group.app, group.work)) cards++; else apps++;
    }
    const tools = widgetItems.size + utilityItems.size;
    const counts = [apps ? `${apps} ${apps === 1 ? 'app' : 'apps'}` : '', cards ? `${cards} ${cards === 1 ? 'card' : 'cards'}` : '', tools ? `${tools} ${tools === 1 ? 'tool' : 'tools'}` : ''].filter(Boolean);
    chip.textContent = counts.length ? `${counts.join(' · ')} on canvas` : 'Empty canvas';
  }
  updateEmpty();
  for (const listener of changeListeners) listener();
  emitCanvasChanged();
}

/** APP-120: the one event the Library re-renders from. */
export const CANVAS_CHANGED = 'canvas:changed';

/**
 * APP-120: tells the page the canvas changed: after every card, widget, note
 * or folder that arrives or leaves (`updateSummary`), and when the member's
 * notes or folders change. The Library is a pure projection of it: it keeps
 * no canvas state of its own and re-renders from this event.
 */
export function emitCanvasChanged(document_: Document | null = host?.ownerDocument ?? (typeof document === 'undefined' ? null : document)): void {
  if (!document_ || typeof document_.dispatchEvent !== 'function') return;
  document_.dispatchEvent(new CustomEvent(CANVAS_CHANGED));
}

/** The empty state shows exactly when nothing is on the board. */
function updateEmpty(): void {
  if (!host) return;
  let any = widgetItems.size > 0 || utilityItems.size > 0 || stackHooks.count() > 0 || uniteHooks.count() > 0;
  for (const group of groups.values()) any ||= group.items.length > 0;
  host.classList.toggle('has-groups', any);
  const empty = host.ownerDocument.getElementById('cv-empty');
  if (any) empty?.setAttribute('hidden', '');
  else empty?.removeAttribute('hidden');
}

/** Calls `listener` whenever cards arrive on or leave the board (the library pane's indicators). */
export function onBoardChange(listener: () => void): void {
  changeListeners.push(listener);
}

/** The canvas, once the board is started (the controls module's camera and selection). */
export function canvasApi(): WidgetCanvas | null {
  return canvas;
}

/** APP-116: a card of `work` shown on the canvas: of `kind` if there is one, else the app card, else any. */
export function workCard(work: string, kind?: string): HTMLElement | null {
  const group = groups.get(work);
  if (!group) return null;
  const shown = shownCards(group);
  return shown.find((card) => kind && card.dataset.kind === kind) ?? shown.find((card) => card.dataset.kind === 'app') ?? shown[0] ?? null;
}

/** APP-116: when the last live event for `work` arrived on this page (`performance.now()`). */
const liveArrivals = new Map<string, number>();
export function liveArrival(work: string): number | undefined {
  return liveArrivals.get(work);
}

/** APP-116: the live transform's outcome for a placed work: `to` is the adapted work, null when the defaults were kept. */
const tailoredListeners: Array<(from: string, to: string | null) => void> = [];
export function onTailored(listener: (from: string, to: string | null) => void): void {
  tailoredListeners.push(listener);
}

/** A phone-width canvas: placements center the first card at a readable zoom instead of fitting everything. */
const narrow = (): boolean => typeof matchMedia === 'function' && matchMedia('(max-width: 760px)').matches;

/** Whether the card `id` is on the board now. */
export function isOnBoard(id: string): boolean {
  if (stackHooks.has(id) || uniteHooks.has(id)) return true;
  for (const group of groups.values()) {
    if (group.items.some((item) => item.dataset.instanceId === id)) return true;
  }
  return false;
}

/** A card kind as the library pane names it (`query::canvas::card_kind`). */
export function paneKind(kind: string): string {
  if (kind === 'summary' || kind === 'status') return 'number';
  if (kind === 'timeline' || kind === 'chart') return 'chart';
  if (['app', 'form', 'records', 'ledger', 'note'].includes(kind)) return kind;
  return 'table';
}

/** APP-104: every work the board has in hand, with its cards (the pane lists new ones under "My apps"). */
export function updateWorkTitle(app: string, work: string, name: string): void {
  const group = groups.get(work);
  if (!group || group.app !== app) return;
  group.title = name;
  group.section.dataset.title = name;
  for (const card of group.cards) if (card.dataset.kind === 'app') {
    const title = card.querySelector('h2'); if (title) title.textContent = name;
    card.setAttribute('aria-label', name);
  }
}

export function boardWorks(): { app: string; work: string; title: string; workNameScope: boolean; cards: { id: string; title: string; kind: string; canvasKind: string }[] }[] {
  return Array.from(groups.values(), (group) => ({
    app: group.app,
    work: group.work,
    title: group.title,
    workNameScope: group.section.dataset.workNameScope === 'true',
    cards: group.cards.map((card) => ({
      id: card.dataset.card ?? '',
      title: card.dataset.kind === 'app' ? 'App card' : (card.getAttribute('aria-label') ?? 'Card'),
      kind: paneKind(card.dataset.kind ?? ''),
      canvasKind: card.dataset.kind ?? '',
    })),
  }));
}

// --- APP-104 stacks (`stacks.ts`): the board's side of piles and fans ---
const stackHooks = {
  has: (_id: string): boolean => false,
  count: (): number => 0,
  remove: (_ids: string[]): string[] => [],
  live: (_work: string): void => undefined,
  settled: (): void => undefined,
  dragEnd: (_item: HTMLElement): void => undefined,
};
export function setStackHooks(hooks: Partial<typeof stackHooks>): void {
  Object.assign(stackHooks, hooks);
}
// --- APP-106 / APP-142: an app's cards gathered into one page (`unite.ts`) ---
const uniteHooks = {
  has: (_id: string): boolean => false,
  count: (): number => 0,
  hasWork: (_work: string): boolean => false,
  remove: (_ids: string[]): string[] => [],
  settled: (): void => undefined,
  restore: async (): Promise<void> => undefined,
  arrange: (): void => undefined,
  place: async (_app: string, _work: string, _mode: 'face' | 'stack' | 'open', _options?: { at?: { x: number; y: number }; focus?: boolean; preserveMode?: boolean }): Promise<string> => 'The canvas is still starting. Try again.',
  removeWork: (_work: string): boolean => false,
};
export function setUniteHooks(hooks: Partial<typeof uniteHooks>): void {
  Object.assign(uniteHooks, hooks);
}
const settledListeners: Array<() => void> = [];
/** Calls `listener` once the board's first contents, stacks and frames are in place. */
export function onBoardSettled(listener: () => void): void {
  settledListeners.push(listener);
}

/**
 * APP-106: the cards of a group that are shown anywhere (their own canvas
 * item, a stack's top, a unite frame, the focus view): every card that is not
 * parked in the hidden board. Live updates and runs follow these.
 */
function shownCards(group: Group): HTMLElement[] {
  return group.cards.filter((card) => card.isConnected && !group.section.contains(card) && !card.closest('[hidden], [inert]'));
}

/**
 * APP-106: each card's focus button (`[ ]`, "Focus · F"), and the app card's
 * "Unite" button; `focus.ts` and `unite.ts` handle their clicks.
 */
function decorateCard(card: HTMLElement): void {
  // Components are owned by one app object. Its toolbar owns Focus and Remove.
  card.querySelectorAll('.cv-card-tools, .cv-app-tools').forEach(node => node.remove());
}
/** Where a card is: its group, its element, and its canvas item when it is on the board on its own. */
function findCard(id: string): { group: Group; card: HTMLElement; item: HTMLElement | null } | null {
  for (const group of groups.values()) {
    const card = group.cards.find((candidate) => candidate.dataset.card === id);
    if (!card) continue;
    const item = group.items.find((other) => other.dataset.instanceId === id) ?? null;
    return { group, card, item };
  }
  return null;
}
export const stackBoard = {
  canvas: (): WidgetCanvas | null => canvas,
  findCard,
  title: (id: string): string => findCard(id)?.group.title ?? 'Stack',
  /** Takes a card's own item off the board, keeping it in the placed set. */
  lift(id: string): void {
    const found = findCard(id);
    if (found?.item) unmountCard(found.group, found.item);
  },
  /**
   * Puts a card on the board on its own at `at` (its saved spot first, unless
   * `exact`); `fanned` floats it out of the rows.
   */
  drop(id: string, at: { x: number; y: number }, fanned = false, exact = false): HTMLElement | null {
    const found = findCard(id);
    if (!found || found.item) return found?.item ?? null;
    const item = mountCard(found.group, found.card, at, fanned ? 0 : -1, fanned, exact);
    if (item) runCards(found.group, [found.card]);
    return item;
  },
  /** A stack's top card: wired, and run if it never ran. */
  show(id: string): HTMLElement | null {
    const found = findCard(id);
    if (!found) return null;
    wireCard(found.group, found.card);
    if (!found.card.closest('[hidden], [inert]')) runCards(found.group, [found.card]);
    return found.card;
  },
  /** A card element back into the hidden board. */
  park(card: HTMLElement): void {
    for (const group of groups.values()) {
      if (group.cards.includes(card)) {
        group.section.append(card);
        return;
      }
    }
  },
  /** Every group keeps its arrangement (a stack sits among them). */
  pinAll(): void {
    for (const group of groups.values()) group.pinned = true;
  },
  changed(): void {
    scheduleReflow();
    updateSummary();
    saveSoon();
  },
  member: (): string => member,
  announce,
  flash,
  /** APP-106: a work's group as the frames need it (cards in definition order). */
  group(work: string): { app: string; work: string; title: string; cards: HTMLElement[]; origins: string[]; pinned: boolean; canShare: boolean; canEditPresentation: boolean; presentation: string; partLayout: string } | null {
    const group = groups.get(work);
    if (!group) return null;
    const origins = Array.from(group.section.ownerDocument.querySelectorAll<HTMLElement>('.cv-origin'))
      .filter((chip) => group.cards.some((card) => card.contains(chip)))
      .map((chip) => chip.textContent ?? '')
      .filter(Boolean);
    return { app: group.app, work: group.work, title: group.title, cards: [...group.cards], origins, pinned: group.pinned, canShare: group.section.dataset.canShare === 'true', canEditPresentation: group.section.dataset.canEditPresentation === 'true', presentation: group.section.dataset.presentation ?? '{}', partLayout: group.section.dataset.partLayout ?? '' };
  },
  /** APP-106: keeps a group's hand arrangement (a unite or a break apart moved it). */
  pin(work: string): void {
    const group = groups.get(work);
    if (group) group.pinned = true;
  },
  /** APP-106: a card element's work, wherever it is. */
  workOf(card: HTMLElement): string | null {
    for (const group of groups.values()) if (group.cards.includes(card)) return group.work;
    return null;
  },
};

function keepPlaced(): void {
  if (member) savePlaced(member, placed);
}
function notePlaced(group: Group, ids: string[]): void {
  const entry = placed.get(group.work) ?? { app: group.app, cards: [] };
  for (const id of ids) {
    dismissed.delete(id);
    if (!entry.cards.includes(id)) entry.cards.push(id);
  }
  // Most recent last: storage keeps the newest when it caps (`layout.ts`).
  placed.delete(group.work);
  placed.set(group.work, entry);
  keepPlaced();
}

/** Keep the complete work ref for reload; components are never independent placements. */
export function rememberAppPlacement(work: string): void {
  const group = groups.get(work);
  if (group) notePlaced(group, group.cards.map(card => card.dataset.card ?? '').filter(Boolean));
}
/** Fetch and wire through the authorized route without briefly mounting loose components. */
export function ensureAppGroup(app: string, work: string): Promise<Group | null> {
  if (!canvas || !host || !isUuid(app) || !isUuid(work)) return Promise.resolve(null);
  const existing = groups.get(work);
  if (existing) return Promise.resolve(existing.app === app ? existing : null);
  const key = `${app}/${work}`;
  const loading = loadingGroups.get(key);
  if (loading) return loading;
  const pending = (async () => {
    const json = await fetchGroup(app, work);
    if (!json || !canvas || !host) return null;
    const raced = groups.get(work);
    if (raced) return raced.app === app ? raced : null;
    const section = buildGroup(host.ownerDocument, json, []);
    if (nameable([], json.origins)) section.dataset.nameable = 'true';
    if (section.dataset.app !== app || section.dataset.work !== work) return null;
    host.ownerDocument.getElementById('cv-board')?.append(section);
    const group = adopt(section, nextRow(), false, () => false);
    if (group) decorate(group);
    return group;
  })().finally(() => loadingGroups.delete(key));
  loadingGroups.set(key, pending);
  return pending;
}
/** Take the whole app off this canvas; its Library entry and data remain. */
export function removeWork(work: string): number {
  placementGeneration.set(work, (placementGeneration.get(work) ?? 0) + 1);
  const removedFrame = uniteHooks.removeWork(work);
  const group = groups.get(work);
  const hadPlacement = placed.delete(work);
  if (group) {
    const ids = group.cards.map(card => card.dataset.card ?? '').filter(Boolean);
    stackHooks.remove(ids);
    for (const id of ids) dismissed.add(id);
    for (const item of [...group.items]) unmountCard(group, item);
  }
  keepPlaced(); scheduleReflow(); updateSummary(); saveSoon();
  return removedFrame || hadPlacement ? 1 : 0;
}

function updateZoomLabel(): void {
  const label = host?.ownerDocument.querySelector<HTMLElement>('[data-zoom-label]');
  if (label && canvas) label.textContent = `${Math.round(canvas.getCamera().zoom * 100)}%`;
}

/** A short glow on a card a live event just updated (CSS honors reduced motion). */
function pulse(card: HTMLElement): void {
  const item = pulseTarget(card);
  item.classList.remove('is-live-updated');
  void item.offsetWidth;
  item.classList.add('is-live-updated');
  window.setTimeout(() => item.classList.remove('is-live-updated'), 1600);
}

/** Pending live re-runs by work (APP-113). */
const liveRuns = new Map<string, number>();
/** APP-195: each live hint's re-reads wait a random 0–3 s, one wait per hint key; a hidden page re-reads once when shown (`live.ts`). */
const hints = hintSchedule(undefined, { resumeWhenShown: true });
/** APP-195: a hinted re-read tries a 503 or a network error once more, 1–3 s later. */
const liveRead = retryOnce();
/**
 * Runs every view card of a group with its window form's parameters;
 * `live` marks a re-run a live event asked for (the cards pulse on arrival).
 */
export function runGroup(work: string, live = false): void {
  void recordControllers.get(work)?.refresh();
  // APP-113: a burst of live events for one work runs its cards once.
  window.clearTimeout(liveRuns.get(work));
  liveRuns.delete(work);
  if (live) {
    liveRuns.set(work, window.setTimeout(() => {
      liveRuns.delete(work);
      runGroupNow(work, true);
    }, LIVE_RUN_MS));
    return;
  }
  runGroupNow(work, false);
}

/**
 * APP-195: a live hint's re-run of `work`, once its wait is over: the record
 * views and every shown card, each read retried once. A 250 ms re-run still
 * waiting is dropped: this one starts later than whatever it was waiting for.
 */
function runGroupHinted(work: string): void {
  void recordControllers.get(work)?.refresh(true);
  window.clearTimeout(liveRuns.get(work));
  liveRuns.delete(work);
  runGroupNow(work, true, liveRead);
}

function runGroupNow(work: string, live: boolean, request?: Fetch): void {
  const group = groups.get(work);
  if (!group) return;
  const parameters = parametersOf(windowForm(group));
  // ADR-011 scenarios: the group's records cards re-read their listing too.
  // APP-106: every shown card, wherever it is (its item, a frame, the focus view).
  const shown = shownCards(group);
  for (const card of shown) {
    if (card.dataset.kind === 'records') void refreshRecords(card, live, request);
    if (card.dataset.kind === 'ledger') void refreshLedger(card, live, request);
  }
  for (const card of shown) {
    if (!card.classList.contains('cv-view')) continue;
    if (parameters === null) {
      renderNoValue(card, 'failed', 'failed', 'That time range isn’t valid: the start must come before the end.');
    } else {
      void runView(card, parameters, request).then(() => {
        if (live) pulse(card);
      });
    }
  }
}

function wireCommand(form: HTMLFormElement, group: Group, card: HTMLElement): void {
  let pending = false;
  form.addEventListener('submit', (event) => {
    event.preventDefault();
    if (pending || form.dataset.commandInvalidated) return;
    pending = true; form.dataset.commandPending = 'true';
    const controls = Array.from(form.querySelectorAll<HTMLInputElement | HTMLSelectElement>('input,select'));
    for (const control of controls) control.disabled = true;
    const button = form.querySelector<HTMLButtonElement>('button[type="submit"]');
    const outcome = card.querySelector<HTMLElement>('[data-outcome]');
    if (button) button.disabled = true;
    card.dataset.state = 'loading';
    if (outcome) outcome.textContent = 'Committing…';
    void submitCommand(form, group.app, group.work).then((result) => {
      pending = false;
      if (!outcome) return;
      outcome.replaceChildren();
      if (result.kind === 'committed') {
        card.dataset.state = 'ready';
        outcome.setAttribute('role', 'status');
        outcome.append(
          el(card.ownerDocument, 'strong', {}, 'Committed. '),
          `Record version ${result.recordVersion}, work version ${result.workVersion}, commit ${result.commit}. Audit event `,
          el(card.ownerDocument, 'code', {}, result.audit),
          ' ',
        );
        const again = el(card.ownerDocument, 'button', { type: 'button', class: 'cv-secondary' }, 'New record');
        again.addEventListener('click', () => void freshForm(group, card));
        outcome.append(again);
        // A committed record can change what the group's views compute.
        runGroup(group.work);
      } else {
        card.dataset.state = result.kind === 'unconfirmed' || result.kind === 'busy' ? 'loading' : 'failed';
        outcome.setAttribute('role', 'alert');
        outcome.append(result.message);
        if (result.kind === 'unconfirmed' || result.kind === 'busy') {
          if (button && !form.dataset.commandInvalidated) button.disabled = false;
        } else {
          delete form.dataset.commandPending;
          const again = el(card.ownerDocument, 'button', { type: 'button', class: 'cv-secondary' }, 'New record');
          again.addEventListener('click', () => void freshForm(group, card));
          outcome.append(' ', again);
        }
      }
    });
  });
}

/** Replaces a form card's form with a fresh record's (new id, new key, current premise). */
async function freshForm(group: Group, card: HTMLElement): Promise<void> {
  const fresh = await fetchGroup(group.app, group.work);
  const id = card.dataset.card;
  const form: FormJson | undefined = fresh?.forms.find((candidate) => candidate.id === id);
  const outcome = card.querySelector<HTMLElement>('[data-outcome]');
  if (!form) {
    if (outcome) outcome.textContent = 'A new record cannot be started right now.';
    return;
  }
  const node = buildForm(card.ownerDocument, form);
  card.querySelector('form.cv-command')?.replaceWith(node);
  card.dataset.state = '';
  if (outcome) outcome.textContent = '';
  wireCommand(node, group, card);
  node.querySelector<HTMLElement>('select, input[type="text"]')?.focus();
}

/** The group's run window (its app card's form), on the board or not. */
function windowForm(group: Group): HTMLFormElement | null {
  for (const card of group.cards) {
    const form = card.querySelector<HTMLFormElement>('form[data-window]');
    if (form) return form;
  }
  return null;
}

/** A records card the server marked as holding a record that waits for this member. */
function waiting(card: HTMLElement): boolean {
  return card.dataset.kind === 'records' && Number(card.dataset.waiting ?? '0') > 0 && !dismissed.has(card.dataset.card ?? '');
}

/**
 * Takes a server-rendered (or island-built) group's cards in hand and puts
 * on the board those `include` admits (APP-104: the member's placed cards,
 * an auto card; everything when omitted), from row `y`. The other cards
 * stay in the hidden board, ready for the library pane.
 */
function adopt(section: HTMLElement, y: number, animate: boolean, include: (card: HTMLElement) => boolean = () => true): Group | null {
  if (!canvas) return null;
  const app = section.dataset.app ?? '';
  const work = section.dataset.work ?? '';
  if (!isUuid(app) || !isUuid(work) || groups.has(work)) return null;
  // DS-40 (APP-098, GENUI_DEMO_MODE): every load starts from the
  // default arrangement, so two windows of the pitch show the same canvas.
  const saved = demo() ? new Map<string, Partial<WidgetCanvasItemState>>() : loadLayout(app, work);
  const cards = Array.from(section.querySelectorAll<HTMLElement>(':scope > .cv-card'));
  for (const card of cards) {
    // APP-120 gap / APP-121: every card knows its app and work once it
    // leaves the section, so the app card gets its address chip and every
    // card its label on whichever path placed it.
    applyPresentationTheme(card, section.dataset.presentation ?? '{}');
    card.dataset.app = app;
    card.dataset.work = work;
    decorateCard(card);
  }
  const title = section.getAttribute('aria-label') ?? 'App';
  const group: Group = { app, work, title, section, cards, items: [], pinned: saved.size > 0, saved };
  groups.set(work, group);
  if (host && cards.some(card => card.querySelector('[data-record-component]'))) recordControllers.set(work, recordComponentsFor(host.ownerDocument.body, { app, work }));
  const window_ = section.querySelector<HTMLFormElement>('form[data-window]');
  window_?.addEventListener('submit', (event) => {
    event.preventDefault();
    runGroup(work);
  });
  decorate(group);
  let x = ORIGIN;
  let index = 0;
  for (const card of cards) {
    if (!include(card)) continue;
    const item = mountCard(group, card, { x, y }, animate ? index : -1);
    if (item) x += (WIDTH[card.dataset.kind ?? ''] ?? 440) + GAP;
    index += 1;
  }
  return group;
}

/**
 * Puts one card of `group` on the board at `at` (or its saved position);
 * `delay` ≥ 0 animates it into place. Wires the card the first time.
 */
function mountCard(group: Group, card: HTMLElement, at: { x: number; y: number }, delay: number, fanned = false, exact = false): HTMLElement | null {
  if (!canvas) return null;
  const id = card.dataset.card ?? '';
  if (!id || group.items.some((item) => item.dataset.instanceId === id)) return null;
  const kind = card.dataset.kind ?? '';
  // Figma's convention: a card moves by its header.
  (card.querySelector<HTMLElement>(':scope > .cv-card-head') ?? card.querySelector<HTMLElement>(':scope > h2'))
    ?.setAttribute('data-canvas-drag-handle', '');
  const width = WIDTH[kind] ?? 440;
  const item = createCanvasItem({
    instanceId: id,
    displayName: card.getAttribute('aria-label') ?? 'Card',
    preferredWidth: width,
    minHeight: HEIGHT[kind] ?? 300,
    content: card,
  });
  item.classList.add('cv-item', `cv-item-${kind || 'view'}`);
  item.dataset.group = group.work;
  if (fanned) item.dataset.fanned = 'true';
  if (delay >= 0) {
    item.classList.add('is-placing');
    item.style.setProperty('--place-delay', `${delay * 70}ms`);
  }
  canvas.mountItem(item, (fanned || exact ? undefined : group.saved.get(id)) ?? at, { focus: false });
  resized?.observe(item);
  const order = group.cards.indexOf(card);
  const before = group.items.findIndex((other) => group.cards.indexOf(other.querySelector<HTMLElement>(':scope > .cv-card') ?? other) > order);
  if (before < 0) group.items.push(item);
  else group.items.splice(before, 0, item);
  wireCard(group, card);
  return item;
}

/** Wires a card's form, records or ledger the first time; refreshes them after. */
function wireCard(group: Group, card: HTMLElement): void {
  const kind = card.dataset.kind ?? '';
  if (card.dataset.wired !== 'true') {
    const form = card.querySelector<HTMLFormElement>('form.cv-command');
    if (form) {
      card.dataset.wired = 'true';
      wireCommand(form, group, card);
    }
    if (kind === 'records') wireRecordsCard(card);
    if (kind === 'ledger') wireLedgerCard(card);
  } else if (kind === 'records') {
    void refreshRecords(card);
  } else if (kind === 'ledger') {
    void refreshLedger(card);
  }
}

/** Takes a card off the board (it goes back to the library pane; no data changes). */
function unmountCard(group: Group, item: HTMLElement): void {
  if (!canvas) return;
  const card = item.querySelector<HTMLElement>(':scope > .cv-card');
  resized?.unobserve(item);
  canvas.removeItem(item, { selectFallback: false });
  group.items = group.items.filter((other) => other !== item);
  if (card) group.section.append(card);
}

/** Runs the view cards among `cards` (the group's window), refreshing records and ledgers. */
function runCards(group: Group, cards: HTMLElement[]): void {
  const parameters = parametersOf(windowForm(group));
  for (const card of cards) {
    if (!card.classList.contains('cv-view')) continue;
    if (card.dataset.state === 'ready' || card.dataset.state === 'empty') continue;
    if (parameters === null) {
      renderNoValue(card, 'failed', 'failed', 'That time range isn’t valid: the start must come before the end.');
    } else {
      void runView(card, parameters);
    }
  }
}

/**
 * APP-104: brings cards of a work onto the board (the library pane, `/card`,
 * `/app`): `ids` null means every card. The work is fetched through the
 * group route (which re-authorizes) when the board has not seen it yet. The
 * new cards animate into place, run through the governed query route, and
 * the camera goes to the first. Returns a status line.
 */
export async function addCards(
  app: string,
  work: string,
  ids: string[] | null,
  options: { keep?: boolean; focus?: boolean; at?: { x: number; y: number } } = {},
): Promise<string> {
  if (ids !== null && !isStandaloneWork(app, work)) return 'App components stay together. Add the app from the Library.';
  return uniteHooks.place(app, work, isStandaloneWork(app, work) ? 'open' : 'face', options);
}

/** APP-104: takes cards off the board (back to the library pane; never deletes data). */
export function removeCards(ids: string[]): number {
  const works = new Set(ids.flatMap(id => { const found = findCard(id); return found ? [found.group.work] : []; }));
  let removed = 0;
  for (const work of works) removed += removeWork(work);
  if (removed) announce(`${removed} ${removed === 1 ? 'item' : 'items'} back in the Library. Nothing was deleted.`);
  return removed;
}

/**
 * A record changed in a work whose waiting records card is not on the board:
 * shows that card when a record now waits for this member (not kept).
 */
const checking = new Set<string>();
async function showWaiting(app: string, work: string): Promise<void> {
  if (checking.has(work) || !host?.ownerDocument.getElementById(`lib-${work}`)) return;
  checking.add(work);
  try {
    const json = await fetchGroup(app, work);
    const ids = (json?.records ?? []).filter((card) => Number(card.waiting ?? '0') > 0 && !dismissed.has(card.id)).map((card) => card.id);
    if (ids.length && !uniteHooks.hasWork(work)) await uniteHooks.place(app, work, 'open', { focus: false });
  } finally {
    checking.delete(work);
  }
}

/** ADR-011 scenarios: re-reads a work's records cards (after a command this browser confirmed). */
export async function refreshGroupRecords(work: string, live = false): Promise<void> {
  void recordControllers.get(work)?.refresh();
  const group = groups.get(work);
  if (!group) return;
  const shown = shownCards(group);
  await Promise.all([
    ...shown.filter((card) => card.dataset.kind === 'records').map((card) => refreshRecords(card, live)),
    ...shown.filter((card) => card.dataset.kind === 'ledger').map((card) => refreshLedger(card, live)),
  ]);
}

/** APP-102: centers a card's canvas item and flashes it (the ledger an `open` asked for). */
export function focusCard(card: HTMLElement): void {
  const item = card.closest<HTMLElement>('.cv-item');
  if (!canvas || !item) return;
  canvas.centerItem(item);
  flash(item);
}

/** A presentation publish changes the pin, never a draft's command identity. */
export async function refreshGroupPresentation(app: string, work: string): Promise<boolean> {
  const group = groups.get(work); if (!group || group.app !== app) return false;
  const fresh = await fetchGroup(app, work); if (!fresh || groups.get(work) !== group) return false;
  group.section.dataset.presentation = fresh.presentation?.json ?? '{}';
  group.section.dataset.canEditPresentation = String(fresh.can_edit_presentation === true);
  group.section.dataset.partLayout = fresh.part_layout ?? '';
  for (const card of group.cards) {
    applyPresentationTheme(card, fresh.presentation?.json ?? '{}');
    const form = card.querySelector<HTMLFormElement>('form.cv-command'); if (!form) continue;
    const next = fresh.forms.find(value => value.id === card.dataset.card);
    const publication = form.querySelector<HTMLInputElement>('input[name="publication_id"]')?.value;
    if (!next || publication !== next.publication) {
      form.dataset.commandInvalidated = 'true';
      for (const control of form.querySelectorAll<HTMLInputElement | HTMLButtonElement | HTMLSelectElement>('input,button,select')) control.disabled = true;
      let notice = card.querySelector<HTMLElement>('[data-presentation-stale]');
      if (!notice) { notice = el(card.ownerDocument, 'p', { role: 'status', 'data-presentation-stale': '' }); form.append(notice); }
      notice.textContent = 'This app changed. Your draft is kept; reload the app before saving.';
    }
  }
  return true;
}

/** Enabling an imported app's data adds capabilities without replacing its retained views or drafts. */
export async function refreshGroupCapabilities(app: string, work: string, isCurrent: () => boolean = () => true): Promise<boolean> {
  const group = groups.get(work); if (!group || group.app !== app || !isCurrent()) return false;
  const fresh = await fetchGroup(app, work); if (!fresh || groups.get(work) !== group || !isCurrent()) return false;
  const section = buildGroup(group.section.ownerDocument, fresh, []);
  const incoming = Array.from(section.querySelectorAll<HTMLElement>(':scope > .cv-card'));
  const ids = new Set(incoming.map(card => card.dataset.card));
  // This is an additive data upgrade, never a general builder replacement.
  if (group.cards.some(card => !ids.has(card.dataset.card))) return false;
  group.section.dataset.presentation = fresh.presentation?.json ?? '{}';
  group.section.dataset.canEditPresentation = String(fresh.can_edit_presentation === true);
  group.section.dataset.partLayout = fresh.part_layout ?? '';
  for (const card of incoming) {
    if (group.cards.some(existing => existing.dataset.card === card.dataset.card)) continue;
    card.dataset.app = app; card.dataset.work = work;
    decorateCard(card); group.cards.push(card); group.section.append(card);
  }
  for (const card of group.cards) {
    applyPresentationTheme(card, group.section.dataset.presentation);
    const form = card.querySelector<HTMLFormElement>('form.cv-command');
    const publication = form?.querySelector<HTMLInputElement>('input[name="publication_id"]')?.value;
    if (form && publication !== fresh.forms.find(value => value.id === card.dataset.card)?.publication) {
      form.dataset.commandInvalidated = 'true';
      for (const control of form.querySelectorAll<HTMLInputElement | HTMLButtonElement | HTMLSelectElement>('input,button,select')) control.disabled = true;
      if (!card.querySelector('[data-presentation-stale]')) form.append(el(card.ownerDocument, 'p', {role: 'status', 'data-presentation-stale': ''}, 'This app changed. Your draft is kept; reload the app before saving.'));
    }
  }
  if (!recordControllers.has(work) && group.cards.some(card => card.querySelector('[data-record-component]')))
    recordControllers.set(work, recordComponentsFor(group.section.ownerDocument.body, {app, work}));
  decorate(group); rememberAppPlacement(work); emitCanvasChanged();
  return true;
}

async function fetchGroup(app: string, work: string) {
  try {
    const response = await fetch(`/v1/canvas/works/${app}/${work}`, { credentials: 'same-origin', cache: 'no-store' });
    return response.ok ? groupJson(await response.json()) : null;
  } catch {
    return null;
  }
}

function flash(item: HTMLElement): void {
  item.classList.remove('is-arrived');
  void item.offsetWidth;
  item.classList.add('is-arrived');
}

/**
 * Places a work on the canvas (the composer's newly published work, a shared
 * work, or a live `app.placed` event) and returns a status line. A work that
 * is already here is brought into focus. Placement happens after the server
 * answers, in one frame; its time is kept on the canvas element
 * (`data-last-place-ms`) for the pitch's measurements.
 */
export function placeWork(app: string, work: string, labels: string[] = []): Promise<string> {
  const inFlight = placing.get(work);
  if (inFlight) return inFlight;
  const placed = placeWorkNow(app, work, labels).finally(() => placing.delete(work));
  placing.set(work, placed);
  return placed;
}

async function placeWorkNow(app: string, work: string, labels: string[]): Promise<string> {
  const generation = placementGeneration.get(work) ?? 0;
  const group = await ensureAppGroup(app, work);
  if ((placementGeneration.get(work) ?? 0) !== generation) return 'That placement was cancelled.';
  if (!group) return 'The app could not be opened. Nothing was placed.';
  if (nameable(labels, [])) { group.section.dataset.nameable = 'true'; decorate(group); }
  const existingMode = host?.querySelector<HTMLElement>(`.cv-item-unite[data-unite="${work}"]`)?.dataset.appView;
  const mode = existingMode === 'open' || existingMode === 'stack' ? existingMode : 'face';
  const result = await uniteHooks.place(app, work, isStandaloneWork(app, work) ? 'open' : mode, { preserveMode: true });
  const pending = early.get(work);
  if (pending) { early.delete(work); void reshape({ app, work }, pending); }
  return result;
}

// --- APP-101: live transform, back to standard, save as /name ---

/**
 * The intent bar's reporter (its island owns the status line; writing the
 * DOM directly would detach its binding): the latest one a submit handed us.
 */
let reporter: ((message: string) => void) | null = null;
let lastReport = '';
export function setReporter(report: (message: string) => void): (message: string) => void {
  reporter = report;
  return (message: string) => {
    lastReport = message;
    report(message);
  };
}
/** Reports on the intent bar; with `when`, only while its last message matches. */
function reportStatus(message: string, when?: RegExp): void {
  if (!reporter || (when && !when.test(lastReport))) return;
  lastReport = message;
  reporter(message);
}

/** Generated and adapted apps can be saved as a slash command. */
function nameable(labels: string[], origins: { kind: string; text: string }[] | undefined): boolean {
  const words = [...labels, ...(origins ?? []).map((origin) => origin.kind)];
  return words.some((word) => /generat|adapt/i.test(word));
}

function appCard(group: Group): HTMLElement | null {
  return group.cards.find(card => card.dataset.kind === 'app') ?? null;
}

/** The app card's APP-101 controls: back to standard / adapted view, and save as /name. */
function decorate(group: Group): void {
  const card = appCard(group);
  if (!card) return;
  const document_ = card.ownerDocument;
  // APP-120: the app card's header shows the app's address chip (`/expenses`).
  card.dataset.app = group.app;
  card.dataset.work = group.work;
  decorateAppCard(card);
  card.querySelector('.cv-tailor-controls')?.remove();
  const controls = el(document_, 'div', { class: 'cv-tailor-controls' });
  const pair = pairs.get(group.work);
  if (pair) {
    const showingAdapted = pair.adapted.work === group.work;
    const toggle = el(document_, 'button', { type: 'button', class: 'cv-secondary', 'data-toggle-standard': '' },
      showingAdapted ? 'Back to standard' : 'Show adapted view');
    toggle.addEventListener('click', () => {
      const target = showingAdapted ? pair.standard : pair.adapted;
      void swapGroup(group.work, target.app, target.work, showingAdapted ? ['retrieved'] : []).then((swapped) => {
        announce(swapped ? (showingAdapted ? 'Back to the standard view.' : 'Showing the adapted view.') : 'That view could not be opened.');
      });
    });
    controls.append(toggle);
  }
  if (group.section.dataset.nameable === 'true') controls.append(nameForm(group));
  if (controls.childElementCount) card.append(controls);
}

function nameForm(group: Group): HTMLElement {
  const document_ = host?.ownerDocument ?? document;
  const input = el(document_, 'input', {
    type: 'text', name: 'slug', 'aria-label': 'Command name', placeholder: 'my-app', maxlength: '32',
    pattern: '[a-z][a-z0-9-]{1,31}', autocomplete: 'off', spellcheck: 'false',
  });
  const status = el(document_, 'span', { class: 'cv-name-status', role: 'status' });
  const form = el(document_, 'form', { class: 'cv-name', 'aria-label': 'Save as a slash command' },
    el(document_, 'span', { class: 'cv-name-prefix', 'aria-hidden': 'true' }, 'Save as /'),
    input,
    el(document_, 'button', { type: 'submit', class: 'cv-secondary' }, 'Save'),
    status);
  form.addEventListener('submit', (event) => {
    event.preventDefault();
    const slug = (input as HTMLInputElement).value.trim().replace(/^\//, '').toLowerCase();
    if (!/^[a-z][a-z0-9-]{1,31}$/.test(slug) || slug.includes('--') || slug.endsWith('-')) {
      status.textContent = 'Use 2 to 32 lowercase letters, digits or dashes.';
      return;
    }
    status.textContent = 'Saving…';
    void nameWork(group.app, group.work, slug).then(async (outcome) => {
      if (typeof outcome === 'string') {
        status.textContent = outcome;
        return;
      }
      await swapGroup(group.work, outcome.app_id, outcome.work_id, [], false);
      await refreshCommands();
      announce(`Saved as /${outcome.slug}. Type /${outcome.slug} to open it.`);
      status.textContent = `Saved as /${outcome.slug}.`;
      reportStatus(`Saved as /${outcome.slug}. Type /${outcome.slug} to open it any time.`);
    });
  });
  return form;
}

async function nameWork(app: string, work: string, slug: string): Promise<{ app_id: string; work_id: string; slug: string } | string> {
  try {
    const response = await fetch(`/v1/work/${app}/${work}/name`, {
      method: 'POST',
      credentials: 'same-origin',
      cache: 'no-store',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ slug }),
    });
    const body = (await response.json().catch(() => null)) as Record<string, unknown> | null;
    if (response.ok && body && isUuid(body.app_id) && isUuid(body.work_id) && typeof body.slug === 'string') {
      return { app_id: body.app_id, work_id: body.work_id, slug: body.slug };
    }
    if (body?.code === 'name_taken') return 'That name is taken. Try another.';
    if (body?.code === 'invalid_name') return 'Use 2 to 32 lowercase letters, digits or dashes.';
    return 'It could not be saved right now.';
  } catch {
    return 'It could not be saved right now.';
  }
}

/**
 * Replaces the group of `fromWork` in place (same row) by the work
 * `(toApp, toWork)`, fetched through the group route (which re-authorizes).
 * The pair record follows the swap so the toggle keeps working.
 */
async function swapGroup(fromWork: string, toApp: string, toWork: string, labels: string[] = [], keepPair = true): Promise<boolean> {
  if (!groups.has(fromWork) || !canvas || !host || groups.has(toWork)) return false;
  const generation = placementGeneration.get(fromWork) ?? 0;
  const json = await fetchGroup(toApp, toWork);
  if ((placementGeneration.get(fromWork) ?? 0) !== generation) return false;
  // APP-113: re-check after the wait (another swap or placement may have run).
  const old = groups.get(fromWork);
  if (!json || !old || !canvas || !host || groups.has(toWork)) return false;
  const document_ = host.ownerDocument;
  // The new group takes the old one's row: its first card, else its unite frame or stack.
  const frame = host.querySelector<HTMLElement>(`.cv-item-unite[data-unite="${fromWork}"]`);
  const y = Number(old.items[0]?.dataset.canvasY ?? frame?.dataset.canvasY ?? nextRow());
  const previousMode = frame?.dataset.appView;
  const mode = previousMode === 'open' || previousMode === 'stack' ? previousMode : 'face';
  // APP-113: the old cards leave their stacks and unite frames too (a stack
  // left with one card spreads it; an emptied frame is removed and forgotten),
  // so no stale card, frame or count survives the swap.
  const ids = old.cards.map((card) => card.dataset.card ?? '').filter(Boolean);
  stackHooks.remove(ids);
  uniteHooks.remove(ids);
  const section = buildGroup(document_, json, labels);
  if (nameable(labels, json.origins) || old.section.dataset.nameable === 'true') section.dataset.nameable = 'true';
  document_.getElementById('cv-board')?.append(section);
  for (const item of old.items) {
    resized?.unobserve(item);
    canvas.removeItem(item, { selectFallback: false });
  }
  recordControllers.get(fromWork)?.dispose(); recordControllers.delete(fromWork);
  old.section.remove();
  const entries = Array.from(groups.entries());
  groups.clear();
  let fresh: Group | null = null;
  for (const [key, group] of entries) {
    if (key === fromWork) fresh = adopt(section, y, true, () => false);
    else groups.set(key, group);
  }
  if (!fresh) return false;
  placed.delete(fromWork);
  notePlaced(fresh, fresh.cards.map((card) => card.dataset.card ?? '').filter(Boolean));
  const pair = pairs.get(fromWork);
  if (pair && keepPair) pairs.set(toWork, pair);
  if (!keepPair) pairs.delete(fromWork);
  window.clearTimeout(tailoring.get(fromWork));
  tailoring.delete(fromWork);
  window.clearTimeout(liveRuns.get(fromWork));
  liveRuns.delete(fromWork);
  decorate(fresh);
  if (isStandaloneWork(old.app, fromWork)) markStandaloneWork(toApp, toWork);
  await uniteHooks.place(toApp, toWork, isStandaloneWork(toApp, toWork) ? 'open' : mode, { at: { x: Number(frame?.dataset.canvasX ?? ORIGIN), y } });
  scheduleReflow();
  runGroup(toWork);
  updateSummary();
  saveSoon();
  const first = fresh.items[0];
  if (first) {
    canvas.centerItem(first);
    flash(first);
  }
  return true;
}

/** The card reshapes in place: the standard work becomes the adapted one. */
async function reshape(standard: Ref, adapted: Ref): Promise<void> {
  pairs.set(standard.work, { standard, adapted });
  pairs.set(adapted.work, { standard, adapted });
  // The adapted work's own chips ("Adapted to your request", "Amount > $200")
  // come from the group route (`query::canvas::adapted_origins`).
  const swapped = await swapGroup(standard.work, adapted.app, adapted.work, []);
  if (swapped) {
    for (const listener of tailoredListeners) listener(standard.work, adapted.work);
    host?.setAttribute('data-last-reshape-at', String(Math.round(performance.now())));
    announce('Adapted to your request.');
    reportStatus('Adapted to your request. “Back to standard” shows the defaults.', /Tailoring/);
  }
}

/**
 * The "tailoring to your request…" state on a just-placed card (APP-101).
 * Answers what the bar should say: the outcome may already have arrived
 * (a fast transform reshapes or keeps before the placement finishes).
 */
export function markTailoring(work: string): 'tailoring' | 'adapted' | 'kept' {
  if (keptEarly.delete(work)) {
    noteKept(work);
    return 'kept';
  }
  const group = groups.get(work);
  const card = group && appCard(group);
  if (!group || !card) return pairs.has(work) || early.has(work) ? 'adapted' : 'kept';
  const document_ = card.ownerDocument;
  card.querySelector('.cv-tailoring')?.remove();
  card.append(el(document_, 'p', { class: 'cv-tailoring', role: 'status' },
    el(document_, 'span', { class: 'cv-spinner', 'aria-hidden': 'true' }), 'Tailoring to your request…'));
  group.items.forEach((item) => item.classList.add('is-tailoring'));
  window.clearTimeout(tailoring.get(work));
  tailoring.set(work, window.setTimeout(() => noteKept(work), TAILOR_WAIT_MS));
  return 'tailoring';
}

/** The transform kept the defaults: a neutral note, never an error. */
export function noteKept(work: string): void {
  window.clearTimeout(tailoring.get(work));
  tailoring.delete(work);
  const group = groups.get(work);
  const card = group && appCard(group);
  if (!group || !card) return;
  group.items.forEach((item) => item.classList.remove('is-tailoring'));
  card.querySelector('.cv-tailoring')?.remove();
  card.querySelector('.cv-kept')?.remove();
  card.append(el(card.ownerDocument, 'p', { class: 'cv-kept', role: 'status' }, 'Kept the standard view.'));
  for (const listener of tailoredListeners) listener(work, null);
  reportStatus('Kept the standard view.', /Tailoring/);
}

async function onAdapted(event: { app_id: string; work_id: string; from_app_id: string; from_work_id: string }): Promise<void> {
  if (groups.has(event.work_id)) return;
  const standard = { app: event.from_app_id, work: event.from_work_id };
  const adapted = { app: event.app_id, work: event.work_id };
  await placing.get(standard.work)?.catch(() => undefined);
  if (groups.has(standard.work)) {
    await reshape(standard, adapted);
    return;
  }
  // The standard work may still be on its way here (the compose answer or an
  // `app.shared`): wait briefly, then place the adapted work on its own.
  early.set(standard.work, adapted);
  window.setTimeout(() => {
    if (early.get(standard.work) === adapted) {
      early.delete(standard.work);
      if (!groups.has(adapted.work)) void placeWork(adapted.app, adapted.work, []).then(announce);
    }
  }, 3000);
}

/**
 * APP-101 utility cards (no data; APP-118 moved the `/apps` list into the
 * Library pane, so none places one today): mounted on the canvas at the next
 * free row and brought into view, with a × that removes it; an existing card
 * of the same id is replaced. None is kept in a saved layout.
 */
export function placeUtilityCard(id: string, title: string, body: HTMLElement): void {
  if (!canvas || !host) return;
  const document_ = host.ownerDocument;
  const instance = `utility-${id}`;
  const existing = document_.querySelector<HTMLElement>(`[data-utility="${instance}"]`);
  if (existing) {
    canvas.removeItem(existing, { selectFallback: false });
    utilityItems.delete(existing);
  }
  // APP-118: every utility card has × (it holds no data).
  const close = el(document_, 'button', { type: 'button', class: 'cv-card-focus cv-card-remove', 'data-utility-close': '', 'aria-label': `Remove ${title} from the canvas`, title: 'Remove' }, '×');
  const card = el(document_, 'article', { class: 'cv-card cv-utility', 'data-card': instance, 'aria-label': title },
    el(document_, 'header', { class: 'cv-card-head' }, el(document_, 'h3', {}, title), el(document_, 'span', { class: 'cv-card-tools' }, close)),
    body);
  const item = createCanvasItem({ instanceId: instance, displayName: title, preferredWidth: 520, minHeight: 200, content: card });
  item.classList.add('cv-item', 'cv-item-utility', 'is-placing');
  item.dataset.utility = instance;
  canvas.mountItem(item, { x: ORIGIN, y: nextRow() }, { focus: false });
  canvas.centerItem(item);
  utilityItems.add(item);
  close.addEventListener('click', (event) => {
    event.stopPropagation();
    if (removeUtilityItem(item)) announce(`${title} removed.`);
  });
  updateSummary();
}

/** Removes a utility card (`/apps`), a widget-like card with no data. */
export function removeUtilityItem(item: HTMLElement): boolean {
  const linked = item.querySelector<HTMLButtonElement>('[data-connected-remove]');
  if (linked) { linked.click(); return false; }
  if (!canvas || !utilityItems.delete(item)) return false;
  canvas.removeItem(item, { selectFallback: false });
  updateSummary();
  return true;
}

const liveListeners: Array<(event: LiveEvent) => void> = [];
/** APP-109: calls `listener` with every live event this page receives (the notes module's refresh). */
export function onLiveEvent(listener: (event: LiveEvent) => void): void {
  liveListeners.push(listener);
}

function onLive(event: LiveEvent): void {
  if (event.kind !== 'stream.lagged') liveArrivals.set(event.work_id, performance.now());
  // APP-195: every page that has the work open gets the same hint at once, so
  // what a hint re-reads (the cards here and the listeners' own reads) starts
  // after a random 0–3 s. Hints for the same key while one waits join it.
  if (event.kind === 'stream.lagged') {
    hints.hint('stream.lagged', () => {
      for (const listener of liveListeners) listener(event);
      // Hints were dropped for this stream: re-run what is shown (APP-095 review).
      for (const work of groups.keys()) runGroupHinted(work);
    });
    return;
  }
  if (event.kind === 'record.committed') {
    const { app_id: app, work_id: work } = event;
    hints.hint(`record.committed:${work}`, () => {
      for (const listener of liveListeners) listener(event);
      const group = groups.get(work);
      const shown = group ? shownCards(group) : [];
      if (shown.length) runGroupHinted(work);
      // APP-104: a record may now wait for this member in a work not on the board.
      stackHooks.live(work);
      if (!shown.some((card) => card.dataset.kind === 'records') && !group?.cards.some((card) => card.dataset.kind === 'records' && isOnBoard(card.dataset.card ?? ''))) {
        void showWaiting(app, work);
      }
    });
    return;
  }
  for (const listener of liveListeners) listener(event);
  if (event.kind === 'app.adapted') {
    void onAdapted(event);
    return;
  }
  if (event.kind === 'app.kept') {
    if (tailoring.has(event.work_id)) noteKept(event.work_id);
    else {
      keptEarly.add(event.work_id);
      window.setTimeout(() => keptEarly.delete(event.work_id), 30000);
    }
    return;
  }
  if (event.kind === 'app.shared') {
    // Permission is not placement. The recipient explicitly adds a live ref.
    host?.ownerDocument.dispatchEvent(new CustomEvent('forma:connected-refresh'));
    announce('An app was shared. Open its private link to review access.');
    return;
  }
  void placeWork(event.app_id, event.work_id).then(announce);
}

/** "Reset layout": every group back to the default rows (view layout only). */
export function resetLayout(): void {
  uniteHooks.arrange();
  if (!canvas) return;
  clearLayouts();
  for (const group of groups.values()) {
    group.pinned = false;
    group.saved.clear();
  }
  reflow();
  canvas.fitAll();
  saveSoon();
}

/** Nudges canvas items by world pixels (arrow keys); their groups keep the new layout. */
export function nudgeItems(items: HTMLElement[], deltaX: number, deltaY: number): void {
  if (!canvas || !items.length) return;
  const flush = canvas.deferDerivedUpdates(false);
  for (const item of items) {
    const state = canvas.getItemState(item);
    canvas.moveItemTo(item, state.x + deltaX, state.y + deltaY);
    if (widgetItems.has(item)) {
      item.dataset.widgetPinned = 'true';
      widgetAnchor?.unobserve(item);
    }
    const group = groups.get(item.dataset.group ?? '');
    if (group) group.pinned = true;
  }
  flush();
  fitPending = false;
  saveSoon();
}

/** What a canvas item is: a card of a work, a widget, or a utility card. */
export function itemKind(item: HTMLElement): 'card' | 'widget' | 'utility' | null {
  if (widgetItems.has(item)) return 'widget';
  if (utilityItems.has(item)) return 'utility';
  return groups.has(item.dataset.group ?? '') ? 'card' : null;
}

/** Keeps the zoom label of the toolbar current. */
export function refreshZoomLabel(): void {
  updateZoomLabel();
}

function wireTools(document_: Document): void {
  const tools = document_.querySelector<HTMLElement>('[data-tools]');
  if (tools) tools.hidden = false;
  // Suggested intents fill the intent bar and submit it.
  document_.addEventListener('click', (event) => {
    const target = (event.target as Element | null)?.closest<HTMLElement>('[data-intent]');
    if (!target) return;
    const input = document_.getElementById('intent-input') as HTMLInputElement | null;
    const form = document_.getElementById('intent-form') as HTMLFormElement | null;
    if (!input || !form) return;
    input.value = target.dataset.intent ?? '';
    form.requestSubmit();
  });
}

/** Activates the canvas once, after the page's islands are active. */
export function startBoard(document_: Document): void {
  if (canvas) return;
  wireReceiptAttach(document_);
  host = document_.getElementById('cv-canvas');
  if (!host) return;
  const surface = createCanvasSurface(host, { className: 'cv-surface', navigatorLabel: 'Canvas navigator' });
  canvas = new WidgetCanvas(surface.elements, {
    onCommit: saveSoon,
    onChange: () => {
      updateZoomLabel();
      saveSoon();
    },
    onKeyboardNavigation: announce,
    // Moving a card by hand (drag or keyboard) pins its group's layout.
    onActiveDragStateChange: (item, dragging, completion) => {
      if (dragging) return;
      if (completion) {
        for (const listener of dragEndListeners) listener(item, completion);
        if (completion.kind === 'released') stackHooks.dragEnd(item);
      }
      if (widgetItems.has(item)) item.dataset.widgetPinned = 'true';
      const group = groups.get(item.dataset.group ?? '');
      if (group) group.pinned = true;
      fitPending = false;
      saveSoon();
    },
  });
  host.addEventListener('wheel', () => { fitPending = false; }, { passive: true });
  host.addEventListener('pointerdown', () => { fitPending = false; });
  host.classList.add('is-canvas');
  // APP-104: the board holds what the member brought (DS-67), the run
  // cookie's restored work, and records waiting for the member's decision.
  member = host.dataset.member ?? '';
  placed = loadPlaced(member);
  standaloneWorks = loadStandaloneWorks(member);
  adoptServerKinds(document_);
  const sections = Array.from(document_.querySelectorAll<HTMLElement>('#cv-board > .cv-group'));
  const flush = canvas.deferDerivedUpdates(false);
  let y = ORIGIN;
  for (const section of sections) {
    const work = section.dataset.work ?? '';
    const restored = section.dataset.auto === 'restored';
    const group = adopt(section, y, false, () => false);
    if (!group) continue;
    if (restored || group.cards.some(waiting)) notePlaced(group, group.cards.map((card) => card.dataset.card ?? '').filter(Boolean));
    if (group.items.length) {
      y = Math.max(y, ...group.items.map((item) => Number(item.dataset.canvasY ?? 0) + item.offsetHeight)) + ROW_GAP;
    }
  }
  flush();
  wireTools(document_);
  startLive(document_);
  startConnectedApps({ document: document_,
    mount(key, title, card) {
      const item = createCanvasItem({ instanceId: `shared-${key.replace('.', '-')}`, displayName: title, preferredWidth: 700, minHeight: 300, content: card });
      item.classList.add('cv-item', 'connected-item');
      canvas!.mountItem(item, { x: ORIGIN, y: nextRow() }, { focus: false });
      utilityItems.add(item); updateSummary();
      return item;
    },
    remove(item) { canvas!.removeItem(item, { selectFallback: false }); utilityItems.delete(item); updateSummary(); },
    status(message) { const status = document_.querySelector('[data-connected-status]'); if (status) status.textContent = message; },
  });
  // "Try again" on a failed card re-runs that card's app (views, records, ledger).
  document_.addEventListener('click', (event) => {
    const button = (event.target as Element | null)?.closest<HTMLElement>('[data-card-retry]');
    const card = button?.closest<HTMLElement>('.cv-card');
    if (!button || !card) return;
    const found = findCard(card.dataset.card ?? '');
    if (card.dataset.kind === 'records') void refreshRecords(card);
    else if (card.dataset.kind === 'ledger') void refreshLedger(card);
    else if (found) runGroupNow(found.group.work, false);
  });
  // APP-112: the card header's "×" takes the card off the canvas (back to the library; no data changes).
  document_.addEventListener('click', (event) => {
    const button = (event.target as Element | null)?.closest<HTMLElement>('[data-card-remove]');
    const card = button?.closest<HTMLElement>('.cv-card');
    const id = card?.dataset.card;
    if (!button || !id || card.closest('.cv-unite')) return;
    event.stopPropagation();
    removeCards([id]);
    announce(`${card.getAttribute('aria-label') ?? 'The card'} is back in the library. Nothing was deleted.`);
  });
  const missing = Array.from(placed.entries()).filter(([work]) => !groups.has(work));
  if (missing.length) host.classList.add('has-groups');
  void Promise.all(missing.map(([work, entry]) => fetchGroup(entry.app, work).then((json) => [work, entry, json] as const)))
    .then((fetched) => {
      if (!canvas || !host) return;
      const restoreFlush = canvas.deferDerivedUpdates(false);
      for (const [work, entry, json] of fetched) {
        if (!json) {
          // A failed fetch may be transient. Keep references without rendering cached data.
          continue;
        }
        if (groups.has(work)) continue;
        const section = buildGroup(document_, json, []);
        document_.getElementById('cv-board')?.append(section);
        adopt(section, nextRow(), false, () => false);
      }
      restoreFlush();
      // Forget cards that no longer exist.
      for (const [work, entry] of placed) {
        const group = groups.get(work);
        if (group) entry.cards = entry.cards.filter((id) => group.cards.some((card) => card.dataset.card === id));
      }
      keepPlaced();
      void settle(document_);
    });
}

/** The work a `/canvas#g-{work}` link names. Anything else, including a timing hash, is not a request. */
export function requestedWork(hash: string): string | null {
  const work = hash.match(/^#g-([0-9a-f-]{36})$/)?.[1] ?? '';
  return isUuid(work) ? work : null;
}

/**
 * The app id the authorized library lists for `work`. A hash for a work the
 * page did not list has no app id, so the board does not ask the group route
 * about it and does not show its title.
 */
export function listedWorkApp(root: ParentNode, work: string): string | null {
  for (const row of root.querySelectorAll<HTMLElement>('.cv-lib-app')) {
    if (row.dataset.work !== work) continue;
    const app = row.dataset.app ?? '';
    return isUuid(app) ? app : null;
  }
  return null;
}

/** What `/canvas#g-{work}` should do once the board's first contents are in place. */
export function canvasLinkPlan(hash: string, mounted: boolean, app: string | null): 'focus' | 'place' | 'absent' | 'none' {
  const work = requestedWork(hash);
  if (!work) return 'none';
  if (mounted) return 'focus';
  return app ? 'place' : 'absent';
}

/** After the board's first contents are in place: camera, summary, runs. */
async function settle(document_: Document): Promise<void> {
  if (!canvas || !host) return;
  await uniteHooks.restore();
  stackHooks.settled();
  // DS-40: no saved camera in demo mode.
  const camera = demo() ? null : loadCamera();
  const target = requestedWork(location.hash);
  const mounted = target ? host.querySelector<HTMLElement>(`.cv-item-unite[data-unite="${target}"]`) ?? groups.get(target)?.items[0] : undefined;
  const linkedApp = target ? listedWorkApp(document_, target) : null;
  const items = [...Array.from(groups.values()).flatMap((group) => group.items), ...host.querySelectorAll<HTMLElement>('.cv-item-unite')];
  const plan = canvasLinkPlan(location.hash, Boolean(mounted), linkedApp);
  if (plan === 'focus' && mounted) {
    canvas.focusItem(mounted);
  } else if (plan === 'place' && target && linkedApp) {
    // The link asked for this work. Load it through the same authorized
    // placement the library uses, then that path focuses it. An unlisted
    // work (plan 'absent') is not fetched and shows no title.
    void placeWork(linkedApp, target);
  } else if (camera && items.length) {
    canvas.restoreCamera(camera);
  } else if (items.length) {
    canvas.fitAll();
    // Keep fitting while the first results grow the cards, until the member moves the camera.
    fitPending = true;
    window.setTimeout(() => { fitPending = false; }, 6000);
  }
  scheduleReflow();
  updateZoomLabel();
  updateSummary();
  // Prefetch: every view on the board runs now, two in flight, top rows
  // first, except the ones the server already ran from the run cookie.
  for (const [work, group] of groups) {
    const cards = shownCards(group).filter((card) => card.classList.contains('cv-view'));
    const ran = cards.some((card) => card.dataset.state === 'ready' || card.dataset.state === 'empty');
    if (cards.length && !ran) runGroup(work);
  }
  if (!items.length && !widgetItems.size && matchMedia('(pointer: fine)').matches) {
    (document_.getElementById('intent-input') as HTMLInputElement | null)?.focus({ preventScroll: true });
  }
  for (const listener of settledListeners) listener();
  announce(items.length ? `${items.length} ${items.length === 1 ? 'card' : 'cards'} on the canvas.` : 'Your canvas is empty. Open the library or describe what you need.');
}

function startLive(document_: Document): void {
  if (!host) return;
  const live = host.dataset.liveEvents ?? '';
  const chip = document_.getElementById('cv-live');
  const chipText = chip?.querySelector<HTMLElement>('[data-live-text]');
  if (live && chip) {
    chip.hidden = false;
    subscribeLive(live, onLive, (state) => {
      chip.dataset.state = state;
      const text = state === 'live' ? 'Live' : state === 'reconnecting' ? 'Reconnecting' : 'Offline';
      if (chipText) chipText.textContent = text;
      chip.title = state === 'live' ? 'Cards update as your team works' : text;
    });
  }
}
