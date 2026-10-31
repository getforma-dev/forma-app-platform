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
} from '../canvas-vendor/index';
import { decorateAppCard } from './address-chip';
import { submitCommand } from './commands';
import { clearLayouts, loadCamera, loadLayout, loadPlaced, saveCamera, saveLayout, savePlaced, type ItemLayout, type PlacedWork } from './layout';
import { subscribeLive, type LiveEvent } from './live';
import { parametersOf, runView } from './queries';
import { announce, buildForm, buildGroup, el, groupJson, isUuid, pulseTarget, renderNoValue, type FormJson } from './render';
import { refreshRecords, wireRecordsCard } from './records';
import { refreshCommands } from './slash';
import { refreshLedger, wireLedgerCard } from './ledger';
import { wireReceiptAttach } from './receipt';

type Group = {
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

/** `GENUI_DEMO_MODE` is on for this page (`data-demo="on"`). */
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

/** Whether this page runs the pitch's demo mode (`GENUI_DEMO_MODE`). */
export function isDemoCanvas(): boolean {
  return demo();
}

/** Calls `listener` whenever the board saves the layout (after a move). */
export function onLayoutSaved(listener: () => void): void {
  layoutListeners.push(listener);
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
    let cards = widgetItems.size + stackHooks.count() + uniteHooks.count();
    let apps = 0;
    for (const group of groups.values()) {
      if (group.items.length || uniteHooks.hasWork(group.work)) apps += 1;
      cards += group.items.length;
    }
    chip.textContent = cards
      ? `${apps} ${apps === 1 ? 'app' : 'apps'} · ${cards} ${cards === 1 ? 'card' : 'cards'} on canvas`
      : 'Empty canvas';
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
  if (['app', 'form', 'records', 'ledger'].includes(kind)) return kind;
  return 'table';
}

/** APP-104: every work the board has in hand, with its cards (the pane lists new ones under "My apps"). */
export function boardWorks(): { app: string; work: string; title: string; cards: { id: string; title: string; kind: string }[] }[] {
  return Array.from(groups.values(), (group) => ({
    app: group.app,
    work: group.work,
    title: group.title,
    cards: group.cards.map((card) => ({
      id: card.dataset.card ?? '',
      title: card.dataset.kind === 'app' ? 'App card' : (card.getAttribute('aria-label') ?? 'Card'),
      kind: paneKind(card.dataset.kind ?? ''),
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
// --- APP-106 unite frames (`unite.ts`): an app's cards assembled in one frame ---
const uniteHooks = {
  has: (_id: string): boolean => false,
  count: (): number => 0,
  hasWork: (_work: string): boolean => false,
  remove: (_ids: string[]): string[] => [],
  settled: (): void => undefined,
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
  return group.cards.filter((card) => card.isConnected && !group.section.contains(card));
}

/**
 * APP-106: each card's focus button (`[ ]`, "Focus · F"), and the app card's
 * "Unite" button; `focus.ts` and `unite.ts` handle their clicks.
 */
function decorateCard(card: HTMLElement): void {
  if (card.querySelector(':scope [data-focus-card]')) return;
  const document_ = card.ownerDocument;
  const svg = document_.createElementNS('http://www.w3.org/2000/svg', 'svg');
  svg.setAttribute('viewBox', '0 0 16 16');
  svg.setAttribute('width', '14');
  svg.setAttribute('height', '14');
  svg.setAttribute('aria-hidden', 'true');
  const path = document_.createElementNS('http://www.w3.org/2000/svg', 'path');
  path.setAttribute('d', 'M2.5 6V2.5H6M10 2.5h3.5V6M13.5 10v3.5H10M6 13.5H2.5V10');
  svg.append(path);
  const title = card.getAttribute('aria-label') ?? 'this card';
  const focus = el(document_, 'button', { type: 'button', class: 'cv-card-focus', 'data-focus-card': '', 'aria-label': `Focus ${title}`, title: 'Focus · F' });
  focus.append(svg);
  const remove = el(document_, 'button', { type: 'button', class: 'cv-card-focus cv-card-remove', 'data-card-remove': '', 'aria-label': `Take ${title} off the canvas`, title: 'Take off the canvas · Delete (back to the library; nothing is deleted)' }, '×');
  const head = card.querySelector<HTMLElement>(':scope > .cv-card-head');
  if (head) {
    head.querySelector('h3')?.setAttribute('title', title);
    head.append(el(document_, 'span', { class: 'cv-card-tools' }, focus, remove));
    return;
  }
  // The app card: its title, then Unite and focus at the top right.
  const tools = el(document_, 'div', { class: 'cv-app-tools' },
    el(document_, 'button', { type: 'button', class: 'cv-card-unite', 'data-unite-app': '', title: 'Unite · U: assemble this app’s cards into one frame' }, 'Unite'),
    focus, remove);
  card.querySelector(':scope > h2')?.after(tools);
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
    runCards(found.group, [found.card]);
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
  group(work: string): { app: string; work: string; title: string; cards: HTMLElement[]; origins: string[]; pinned: boolean } | null {
    const group = groups.get(work);
    if (!group) return null;
    const origins = Array.from(group.section.ownerDocument.querySelectorAll<HTMLElement>('.cv-origin'))
      .filter((chip) => group.cards.some((card) => card.contains(chip)))
      .map((chip) => chip.textContent ?? '')
      .filter(Boolean);
    return { app: group.app, work: group.work, title: group.title, cards: [...group.cards], origins, pinned: group.pinned };
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
/**
 * Runs every view card of a group with its window form's parameters;
 * `live` marks a re-run a live event asked for (the cards pulse on arrival).
 */
export function runGroup(work: string, live = false): void {
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

function runGroupNow(work: string, live: boolean): void {
  const group = groups.get(work);
  if (!group) return;
  const parameters = parametersOf(windowForm(group));
  // ADR-011 scenarios: the group's records cards re-read their listing too.
  // APP-106: every shown card, wherever it is (its item, a frame, the focus view).
  const shown = shownCards(group);
  for (const card of shown) {
    if (card.dataset.kind === 'records') void refreshRecords(card, live);
    if (card.dataset.kind === 'ledger') void refreshLedger(card, live);
  }
  for (const card of shown) {
    if (!card.classList.contains('cv-view')) continue;
    if (parameters === null) {
      renderNoValue(card, 'failed', 'failed', 'That time range isn’t valid: the start must come before the end.');
    } else {
      void runView(card, parameters).then(() => {
        if (live) pulse(card);
      });
    }
  }
}

function wireCommand(form: HTMLFormElement, group: Group, card: HTMLElement): void {
  form.addEventListener('submit', (event) => {
    event.preventDefault();
    const button = form.querySelector<HTMLButtonElement>('button[type="submit"]');
    const outcome = card.querySelector<HTMLElement>('[data-outcome]');
    if (button) button.disabled = true;
    card.dataset.state = 'loading';
    if (outcome) outcome.textContent = 'Committing…';
    void submitCommand(form, group.app, group.work).then((result) => {
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
          if (button) button.disabled = false;
        } else {
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
  // APP-098 demo shortcut (GENUI_DEMO_MODE): every load starts from the
  // default arrangement, so two windows of the pitch show the same canvas.
  const saved = demo() ? new Map<string, Partial<WidgetCanvasItemState>>() : loadLayout(app, work);
  const cards = Array.from(section.querySelectorAll<HTMLElement>(':scope > .cv-card'));
  for (const card of cards) decorateCard(card);
  const title = section.getAttribute('aria-label') ?? 'App';
  const group: Group = { app, work, title, section, cards, items: [], pinned: saved.size > 0, saved };
  groups.set(work, group);
  const window_ = section.querySelector<HTMLFormElement>('form[data-window]');
  window_?.addEventListener('submit', (event) => {
    event.preventDefault();
    runGroup(work);
  });
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
export async function addCards(app: string, work: string, ids: string[] | null, options: { keep?: boolean; focus?: boolean } = {}): Promise<string> {
  if (!canvas || !host || !isUuid(app) || !isUuid(work)) return 'Nothing was placed.';
  let group = groups.get(work);
  if (!group) {
    const json = await fetchGroup(app, work);
    if (!json) return 'That app could not be opened. Nothing was placed.';
    if (!groups.has(work)) {
      const section = buildGroup(host.ownerDocument, json, []);
      host.ownerDocument.getElementById('cv-board')?.append(section);
      adopt(section, nextRow(), false, () => false);
    }
    group = groups.get(work);
    if (!group) return 'Nothing was placed.';
  }
  const wanted = group.cards.filter((card) => ids === null || ids.includes(card.dataset.card ?? ''));
  if (!wanted.length) return 'That card is not available to you right now.';
  const fresh: HTMLElement[] = [];
  // A card in a stack or a unite frame is on the board already (APP-106).
  const rowY = group.items.length ? Math.min(...group.items.map((item) => Number(item.dataset.canvasY ?? 0))) : nextRow();
  let x = group.items.reduce((right, item) => Math.max(right, Number(item.dataset.canvasX ?? 0) + item.offsetWidth + GAP), ORIGIN);
  wanted.forEach((card) => {
    const id = card.dataset.card ?? '';
    if (stackHooks.has(id) || uniteHooks.has(id)) return;
    const item = mountCard(group!, card, { x, y: rowY }, fresh.length);
    if (!item) return;
    x += (WIDTH[card.dataset.kind ?? ''] ?? 440) + GAP;
    fresh.push(card);
  });
  if (options.keep !== false) notePlaced(group, wanted.map((card) => card.dataset.card ?? '').filter(Boolean));
  const first = (fresh[0] ?? wanted[0])?.closest<HTMLElement>('.cv-item');
  if (fresh.length) {
    runCards(group, fresh);
    scheduleReflow();
    updateSummary();
    saveSoon();
  }
  if (first && options.focus !== false) {
    // After the re-flow has placed it.
    requestAnimationFrame(() => requestAnimationFrame(() => {
      if (!canvas || !first.isConnected) return;
      canvas.centerItem(first);
      flash(first);
    }));
  }
  const count = fresh.length;
  const message = count
    ? `${count === 1 ? (fresh[0]?.getAttribute('aria-label') ?? 'The card') : `${count} cards`} from ${group.title} ${count === 1 ? 'is' : 'are'} on your canvas.`
    : `${wanted.length === 1 ? 'That card is' : 'Those cards are'} already on your canvas.`;
  announce(message);
  return message;
}

/** APP-104: takes cards off the board (back to the library pane; never deletes data). */
export function removeCards(ids: string[]): number {
  let removed = 0;
  // Cards in a stack leave the stack (`stacks.ts`).
  for (const id of [...stackHooks.remove(ids), ...uniteHooks.remove(ids)]) {
    removed += 1;
    dismissed.add(id);
    for (const entry of placed.values()) entry.cards = entry.cards.filter((card) => card !== id);
  }
  for (const group of groups.values()) {
    for (const item of [...group.items]) {
      const id = item.dataset.instanceId ?? '';
      if (!ids.includes(id)) continue;
      unmountCard(group, item);
      removed += 1;
      dismissed.add(id);
      const entry = placed.get(group.work);
      if (entry) entry.cards = entry.cards.filter((card) => card !== id);
    }
  }
  if (removed) {
    keepPlaced();
    scheduleReflow();
    updateSummary();
    saveSoon();
    announce(`${removed} ${removed === 1 ? 'card' : 'cards'} back in the library.`);
  }
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
    if (ids.length && !ids.every(isOnBoard)) await addCards(app, work, ids, { keep: false, focus: false });
  } finally {
    checking.delete(work);
  }
}

/** ADR-011 scenarios: re-reads a work's records cards (after a command this browser confirmed). */
export async function refreshGroupRecords(work: string, live = false): Promise<void> {
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
  if (!canvas || !host || !isUuid(app) || !isUuid(work)) return 'Nothing was placed.';
  const existing = groups.get(work);
  if (existing) {
    // APP-104: a work the board has seen keeps its unplaced cards in hand;
    // placing it brings them all.
    if (existing.items.length < existing.cards.length) return addCards(app, work, null);
    notePlaced(existing, existing.cards.map((card) => card.dataset.card ?? '').filter(Boolean));
    const first = existing.items[0];
    if (first) {
      canvas.centerItem(first);
      flash(first);
    }
    return 'That app is already on your canvas.';
  }
  const document_ = host.ownerDocument;
  const y = nextRow();
  const bar = (width: string) => el(document_, 'span', { class: 'cv-skeleton-bar', 'data-width': width });
  const ghost = createCanvasItem({
    instanceId: `placing-${work}`,
    displayName: 'Placing an app',
    preferredWidth: 760,
    minHeight: 260,
    content: el(document_, 'article', { class: 'cv-card cv-ghost', 'aria-busy': 'true' },
      el(document_, 'p', { class: 'cv-ghost-status' }, el(document_, 'span', { class: 'cv-spinner', 'aria-hidden': 'true' }), 'Placing your app'),
      el(document_, 'div', { class: 'cv-skeleton', 'aria-hidden': 'true' },
        el(document_, 'div', { class: 'cv-skeleton-card' }, bar('60'), bar('40'), bar('90'), bar('90'), bar('50')),
        el(document_, 'div', { class: 'cv-skeleton-card' }, bar('40'), el(document_, 'span', { class: 'cv-skeleton-block' }), bar('70')),
        el(document_, 'div', { class: 'cv-skeleton-card' }, bar('50'), el(document_, 'span', { class: 'cv-skeleton-chart' },
          ...['35', '60', '45', '80', '55', '95'].map((height) => el(document_, 'i', { 'data-height': height }))), bar('80'))),
      el(document_, 'p', { class: 'cv-message' }, 'Loading its cards. Every number arrives with its proof.')),
    chrome: false,
  });
  ghost.classList.add('cv-item', 'cv-item-ghost');
  canvas.mountItem(ghost, { x: ORIGIN, y }, { focus: false });
  canvas.centerItem(ghost);
  const group = await fetchGroup(app, work);
  canvas.removeItem(ghost, { selectFallback: false });
  if (!group) return 'The app could not be opened. Nothing was placed.';
  // APP-113: another path (addCards, a swap) placed it while this one waited.
  if (groups.has(work)) return placeWorkNow(app, work, labels);
  const started = performance.now();
  const section = buildGroup(document_, group, labels);
  if (nameable(labels, group.origins)) section.dataset.nameable = 'true';
  document_.getElementById('cv-board')?.append(section);
  const adopted = adopt(section, y, true);
  if (!adopted) return 'Nothing was placed.';
  notePlaced(adopted, adopted.cards.map((card) => card.dataset.card ?? '').filter(Boolean));
  decorate(adopted);
  scheduleReflow();
  const elapsed = Math.round(performance.now() - started);
  host.dataset.lastPlaceMs = String(elapsed);
  runGroup(work);
  // Show the whole canvas with the new app arriving; keep fitting while its
  // results grow the cards. On a phone, fitting a row of cards makes them
  // unreadable: the first card is centered at a readable zoom instead.
  const first = adopted.items[0];
  if (narrow() && first) {
    canvas.resetZoom();
    canvas.zoomBy(0.6);
    canvas.centerItem(first);
    refreshZoomLabel();
  } else {
    canvas.fitAll();
    fitPending = true;
    window.setTimeout(() => { fitPending = false; }, 3000);
  }
  updateSummary();
  saveSoon();
  announce(`${group.title} placed on the canvas.`);
  // An adaptation that arrived before this standard work was placed.
  const pending = early.get(work);
  if (pending) {
    early.delete(work);
    void reshape({ app, work }, pending);
  }
  return `${group.title} is on your canvas.`;
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
  for (const item of group.items) {
    const card = item.querySelector<HTMLElement>('.cv-app');
    if (card) return card;
  }
  return null;
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
  const json = await fetchGroup(toApp, toWork);
  // APP-113: re-check after the wait (another swap or placement may have run).
  const old = groups.get(fromWork);
  if (!json || !old || !canvas || !host || groups.has(toWork)) return false;
  const document_ = host.ownerDocument;
  // The new group takes the old one's row: its first card, else its unite frame or stack.
  const frame = host.querySelector<HTMLElement>(`.cv-item-unite[data-unite="${fromWork}"]`);
  const y = Number(old.items[0]?.dataset.canvasY ?? frame?.dataset.canvasY ?? nextRow());
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
  old.section.remove();
  const entries = Array.from(groups.entries());
  groups.clear();
  let fresh: Group | null = null;
  for (const [key, group] of entries) {
    if (key === fromWork) fresh = adopt(section, y, true);
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
  for (const listener of liveListeners) listener(event);
  if (event.kind === 'stream.lagged') {
    // Hints were dropped for this stream: re-run what is shown (APP-095 review).
    for (const work of groups.keys()) runGroup(work, true);
    return;
  }
  if (event.kind === 'record.committed') {
    const group = groups.get(event.work_id);
    const shown = group ? shownCards(group) : [];
    if (shown.length) runGroup(event.work_id, true);
    // APP-104: a record may now wait for this member in a work not on the board.
    stackHooks.live(event.work_id);
    if (!shown.some((card) => card.dataset.kind === 'records') && !group?.cards.some((card) => card.dataset.kind === 'records' && isOnBoard(card.dataset.card ?? ''))) {
      void showWaiting(event.app_id, event.work_id);
    }
    return;
  }
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
  void placeWork(event.app_id, event.work_id).then(announce);
}

/** "Reset layout": every group back to the default rows (view layout only). */
export function resetLayout(): void {
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
    onActiveDragStateChange: (item, dragging) => {
      if (dragging) return;
      stackHooks.dragEnd(item);
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
  const sections = Array.from(document_.querySelectorAll<HTMLElement>('#cv-board > .cv-group'));
  const flush = canvas.deferDerivedUpdates(false);
  let y = ORIGIN;
  for (const section of sections) {
    const work = section.dataset.work ?? '';
    const restored = section.dataset.auto === 'restored';
    const kept = new Set(placed.get(work)?.cards ?? []);
    const group = adopt(section, y, false, (card) => restored || kept.has(card.dataset.card ?? '') || waiting(card));
    if (!group) continue;
    if (restored) notePlaced(group, group.cards.map((card) => card.dataset.card ?? '').filter(Boolean));
    if (group.items.length) {
      y = Math.max(y, ...group.items.map((item) => Number(item.dataset.canvasY ?? 0) + item.offsetHeight)) + ROW_GAP;
    }
  }
  flush();
  wireTools(document_);
  startLive(document_);
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
    if (!button || !id) return;
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
          // No longer openable for this member: forget it.
          placed.delete(work);
          continue;
        }
        if (groups.has(work)) continue;
        const section = buildGroup(document_, json, []);
        document_.getElementById('cv-board')?.append(section);
        const kept = new Set(entry.cards);
        adopt(section, nextRow(), false, (card) => kept.has(card.dataset.card ?? '') || waiting(card));
      }
      restoreFlush();
      // Forget cards that no longer exist.
      for (const [work, entry] of placed) {
        const group = groups.get(work);
        if (group) entry.cards = entry.cards.filter((id) => group.cards.some((card) => card.dataset.card === id));
      }
      keepPlaced();
      settle(document_);
    });
}

/** After the board's first contents are in place: camera, summary, runs. */
function settle(document_: Document): void {
  if (!canvas || !host) return;
  stackHooks.settled();
  uniteHooks.settled();
  const camera = demo() ? null : loadCamera();
  const target = location.hash.match(/^#g-([0-9a-f-]{36})$/)?.[1];
  const items = Array.from(groups.values()).flatMap((group) => group.items);
  if (target && groups.get(target)?.items[0]) {
    canvas.focusItem(groups.get(target)!.items[0]!);
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
