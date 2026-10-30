// APP-106 Unite (Victor: "a unite view like the power rangers when they
// unite. So the whole app assembles?"). An app's cards fly together into one
// bordered frame, the app's own composed layout, and break apart again.
//
// - Start: the app card's "Unite", the library pane's app row, `/unite <app>`,
//   U with a card of the app selected, or the toolbar menu. Cards of the app
//   that are not on the canvas yet are brought in; cards in a stack leave it.
// - Layout: computed here, with no model, from the app's definition order
//   (the board keeps the group's cards as the server lists them: the app card,
//   the views in declared order, then the command forms, records and ledger)
//   into a fixed arrangement: a KPI row (summary views), the primary visual
//   (the first chart, else the first table), the detail tables, and a side
//   panel with the app card, the forms and the records. At phone width the
//   frame is one column.
// - Identity: every card element is moved, never rebuilt, so receipts, form
//   drafts, open details, scroll positions and live updates survive (the
//   board runs and refreshes every card that is not parked, wherever it is).
// - Motion: a staggered fly-in (FLIP, about 600–800 ms) in world coordinates,
//   so the camera can move at the same time; with prefers-reduced-motion, a
//   short cross-fade.
// - Break apart: the frame's "Break apart", `/split <app>` or Shift+U returns
//   every card to the spot it had before the unite.
// - A frame is one canvas item (it moves as one) and is kept per member in
//   this browser (DS-06/DS-67): the work, the frame's spot and each card's
//   previous spot.
import { createCanvasItem } from '../canvas-vendor/index';
import { addCards, GAP, onLayoutSaved, setUniteHooks, SLOT_WIDTH, stackBoard as board } from './board';
import { loadUnited, saveUnited } from './layout';
import { findApps } from './library';
import { el, originKind } from './render';
import { registerCommands, registerCompleter, type Completion } from './slash';
import { collapseAll, releaseCards, stackCardsOf } from './stacks';

type Spot = { x: number; y: number };
type Frame = {
  work: string;
  app: string;
  title: string;
  /** Card ids in the frame's reading order (KPI row, main column, side panel). */
  cards: string[];
  item: HTMLElement;
  root: HTMLElement;
  previous: Map<string, Spot>;
};
type Report = (message: string) => void;

const frames = new Map<string, Frame>();
let documentRef: Document | null = null;
// Nothing is saved before the kept frames are back (a layout save during the
// page's first load would otherwise write an empty list over them).
let restored = false;
const WIDE = 1240;
const NARROW = 760;
const PHONE = 400;
const DURATION = 620;

const reducedMotion = () => matchMedia('(prefers-reduced-motion: reduce)').matches;
const phone = () => matchMedia('(max-width: 760px)').matches;

/** Keeps the scroll offsets of an element and its descendants across a DOM move. */
export function keepScroll(root: HTMLElement): () => void {
  const spots: [HTMLElement, number, number][] = [];
  for (const node of [root, ...root.querySelectorAll<HTMLElement>('*')]) {
    if (node.scrollTop || node.scrollLeft) spots.push([node, node.scrollTop, node.scrollLeft]);
  }
  return () => {
    for (const [node, top, left] of spots) {
      node.scrollTop = top;
      node.scrollLeft = left;
    }
  };
}

export function isUniteItem(item: HTMLElement): boolean {
  return item.classList.contains('cv-item-unite');
}

/** The frame that holds card `id`, as its cards in reading order. */
export function frameCardsOf(id: string): string[] | null {
  for (const frame of frames.values()) if (frame.cards.includes(id)) return [...frame.cards];
  return null;
}

/** The cards of a frame's canvas item. */
export function frameCardsOfItem(item: HTMLElement): string[] {
  return [...(frames.get(item.dataset.unite ?? '')?.cards ?? [])];
}

function save(): void {
  const canvas = board.canvas();
  if (!canvas || !restored) return;
  saveUnited(board.member(), Array.from(frames.values(), (frame) => {
    const state = canvas.getItemState(frame.item);
    return { work: frame.work, app: frame.app, x: state.x, y: state.y, previous: Object.fromEntries(frame.previous) };
  }));
}

/** The fixed arrangement, from the definition order (no model). */
function arrange(cards: HTMLElement[]): { kpi: HTMLElement[]; main: HTMLElement[]; side: HTMLElement[] } {
  const kpi: HTMLElement[] = [];
  const side: HTMLElement[] = [];
  const views: HTMLElement[] = [];
  let primary: HTMLElement | null = null;
  for (const card of cards) {
    const kind = card.dataset.kind ?? '';
    if (kind === 'app' || kind === 'form' || kind === 'records') side.push(card);
    // A status view is a breakdown (a chart or table), not one number.
    else if (kind === 'summary') kpi.push(card);
    else if (!primary && (kind === 'timeline' || kind === 'chart')) primary = card;
    else views.push(card);
  }
  // No chart: the first table is the primary visual; the ledger stays a detail.
  if (!primary) {
    const first = views.findIndex((card) => card.dataset.kind !== 'ledger');
    if (first >= 0) primary = views.splice(first, 1)[0] ?? null;
  }
  return { kpi, main: primary ? [primary, ...views] : views, side };
}

function buildFrame(document_: Document, work: string, title: string, origins: string[]): Frame['root'] {
  const head = el(document_, 'header', { class: 'cv-unite-head', 'data-canvas-drag-handle': '' },
    el(document_, 'span', { class: 'cv-unite-mark', 'aria-hidden': 'true' }),
    el(document_, 'div', { class: 'cv-unite-heading' },
      el(document_, 'h2', { class: 'cv-unite-title' }, title),
      el(document_, 'p', { class: 'cv-origins cv-unite-origins', 'aria-label': 'How this app was assembled' },
        ...origins.map((text) => el(document_, 'span', { class: 'cv-origin', 'data-origin': originKind(text) }, text)))),
    el(document_, 'span', { class: 'cv-unite-count', 'data-unite-count': '' }),
    el(document_, 'div', { class: 'cv-unite-actions', role: 'group', 'aria-label': 'Frame' },
      el(document_, 'button', { type: 'button', class: 'cv-secondary', 'data-unite-focus': '', title: 'Focus the first card · F' }, 'Focus'),
      el(document_, 'button', { type: 'button', class: 'cv-secondary', 'data-unite-split': '', title: 'Break apart · Shift+U' }, 'Break apart')));
  return el(document_, 'article', { class: 'cv-unite', 'data-unite': work, 'data-kind': 'unite', 'aria-label': `${title}, united` },
    head,
    el(document_, 'div', { class: 'cv-unite-grid' },
      el(document_, 'div', { class: 'cv-unite-kpis', 'data-slot': 'kpi' }),
      el(document_, 'div', { class: 'cv-unite-main', 'data-slot': 'main' }),
      el(document_, 'div', { class: 'cv-unite-side', 'data-slot': 'side' })));
}

/** Puts the cards in their slots; returns them in reading order and the frame's width. */
function fill(root: HTMLElement, cards: HTMLElement[]): { order: HTMLElement[]; width: number } {
  const { kpi, main, side } = arrange(cards);
  const slot = (name: string) => root.querySelector<HTMLElement>(`[data-slot="${name}"]`)!;
  slot('kpi').replaceChildren(...kpi);
  slot('main').replaceChildren(...main);
  slot('side').replaceChildren(...side);
  const single = phone();
  const both = main.length > 0 && side.length > 0;
  const grid = root.querySelector<HTMLElement>('.cv-unite-grid')!;
  const columns = !single && both;
  grid.style.gridTemplateColumns = columns ? 'minmax(0, 1.55fr) minmax(0, 1fr)' : 'minmax(0, 1fr)';
  const rows: string[] = [];
  if (kpi.length) rows.push(columns ? '"kpi kpi"' : '"kpi"');
  if (columns) rows.push('"main side"');
  else {
    if (main.length) rows.push('"main"');
    if (side.length) rows.push('"side"');
  }
  grid.style.gridTemplateAreas = rows.join(' ');
  root.classList.toggle('is-single', single);
  const count = root.querySelector('[data-unite-count]');
  if (count) count.textContent = `${cards.length} ${cards.length === 1 ? 'card' : 'cards'}`;
  return { order: [...kpi, ...main, ...side], width: single ? PHONE : columns ? WIDE : NARROW };
}

/** A card's box in world coordinates, measured through the canvas item that holds it. */
function worldBox(card: HTMLElement, item: HTMLElement): { x: number; y: number; width: number; height: number } | null {
  const canvas = board.canvas();
  if (!canvas || !item.isConnected || !card.isConnected) return null;
  const state = canvas.getItemState(item);
  const outer = item.getBoundingClientRect();
  const inner = card.getBoundingClientRect();
  const k = item.offsetWidth ? outer.width / item.offsetWidth : 1;
  if (!k || !inner.width) return null;
  return { x: state.x + (inner.left - outer.left) / k, y: state.y + (inner.top - outer.top) / k, width: inner.width / k, height: inner.height / k };
}

/**
 * The fly-in: each card starts where it was (world coordinates) and eases to
 * its new place, staggered so the whole motion stays under 800 ms. Reduced
 * motion: a cross-fade.
 */
function fly(cards: HTMLElement[], before: Map<HTMLElement, { x: number; y: number; width: number; height: number }>, holderOf: (card: HTMLElement) => HTMLElement | null): void {
  const step = cards.length > 1 ? Math.min(45, 180 / (cards.length - 1)) : 0;
  cards.forEach((card, index) => {
    if (reducedMotion()) {
      card.animate([{ opacity: 0 }, { opacity: 1 }], { duration: 200, easing: 'ease-out' });
      return;
    }
    const holder = holderOf(card);
    const after = holder ? worldBox(card, holder) : null;
    const start = before.get(card);
    const keyframes: Keyframe[] = start && after && after.width && after.height
      ? [
        { transformOrigin: 'top left', transform: `translate(${start.x - after.x}px, ${start.y - after.y}px) scale(${start.width / after.width}, ${start.height / after.height})` },
        { transformOrigin: 'top left', transform: 'none' },
      ]
      : [
        { transformOrigin: 'top left', transform: 'translateY(28px) scale(0.94)', opacity: 0 },
        { transformOrigin: 'top left', transform: 'none', opacity: 1 },
      ];
    card.classList.add('is-flying');
    const motion = card.animate(keyframes, { duration: DURATION, delay: index * step, easing: 'cubic-bezier(0.16, 1, 0.3, 1)', fill: 'backwards' });
    void motion.finished.catch(() => undefined).finally(() => {
      card.classList.remove('is-flying');
      repaintCharts(card);
    });
  });
}

/**
 * APP-109 fix: a chart whose result landed while its card was flying (a
 * unite right after placement) could keep its axis labels rasterized at the
 * flight's scale, drawn oversized. Once the card has settled, each chart is
 * swapped for an identical copy (same nodes, no listeners on them), so it is
 * painted afresh at its real size; a chart that renders later is new anyway.
 */
function repaintCharts(card: HTMLElement): void {
  if (!card.isConnected) return;
  for (const chart of card.querySelectorAll<SVGSVGElement>('svg.cv-chart-svg')) {
    chart.replaceWith(chart.cloneNode(true));
  }
}

function syncAppButton(cards: HTMLElement[], united: boolean): void {
  for (const card of cards) {
    const button = card.querySelector<HTMLElement>('[data-unite-app]');
    if (!button) continue;
    button.textContent = united ? 'Break apart' : 'Unite';
    button.title = united ? 'Break apart · Shift+U: every card back where it was' : 'Unite · U: assemble this app’s cards into one frame';
  }
}

/**
 * Assembles the app's cards into its frame. `restore` rebuilds a kept frame
 * quietly (no fetch, no motion, no camera).
 */
export async function unite(app: string, work: string, restore?: { at: Spot; previous: Map<string, Spot> }): Promise<string> {
  const canvas = board.canvas();
  if (!canvas || !documentRef) return 'Nothing was united.';
  const existing = frames.get(work);
  if (existing) {
    canvas.centerItem(existing.item);
    board.flash(existing.item);
    return `${existing.title} is already united.`;
  }
  if (!restore) {
    collapseAll();
    await addCards(app, work, null, { focus: false });
    if (frames.has(work)) return 'That app is already united.';
  }
  const group = board.group(work);
  if (!group) return 'That app could not be opened. Nothing was united.';
  // Where each card is now: its own item, or the stack that holds it.
  const before = new Map<HTMLElement, { x: number; y: number; width: number; height: number }>();
  const previous = new Map<string, Spot>(restore?.previous ?? []);
  const members: HTMLElement[] = [];
  const stacked: string[] = [];
  const scrolls: (() => void)[] = [];
  let fromStack = 0;
  for (const card of group.cards) {
    const id = card.dataset.card ?? '';
    const found = id ? board.findCard(id) : null;
    if (!found) continue;
    if (found.item) {
      const box = worldBox(card, found.item);
      if (box) before.set(card, box);
      if (!restore) previous.set(id, { x: box?.x ?? 0, y: box?.y ?? 0 });
    } else {
      const stack = stackCardsOf(id);
      if (!stack) continue;
      const state = canvas.getItemState(stack.item);
      before.set(card, { x: state.x, y: state.y + 34, width: state.width, height: Math.max(260, state.height - 34) });
      if (!restore) previous.set(id, { x: state.x + fromStack * (state.width + GAP), y: state.y });
      fromStack += 1;
      stacked.push(id);
    }
    scrolls.push(keepScroll(card));
    members.push(card);
  }
  if (!members.length) return 'None of that app’s cards are on your canvas.';
  releaseCards(stacked);
  const at = restore?.at ?? {
    x: Math.min(...[...previous.values()].map((spot) => spot.x)),
    y: Math.min(...[...previous.values()].map((spot) => spot.y)),
  };
  for (const card of members) board.lift(card.dataset.card ?? '');
  const root = buildFrame(documentRef, work, group.title, group.origins);
  const { order, width } = fill(root, members);
  for (const restoreScroll of scrolls) restoreScroll();
  const item = createCanvasItem({ instanceId: `unite-${work}`, displayName: `${group.title}, united`, preferredWidth: width, minHeight: 360, content: root, chrome: false });
  item.classList.add('cv-item', 'cv-item-unite');
  item.dataset.unite = work;
  canvas.mountItem(item, at, { focus: false });
  for (const restoreScroll of scrolls) restoreScroll();
  const frame: Frame = { work, app, title: group.title, cards: order.map((card) => card.dataset.card ?? ''), item, root, previous };
  frames.set(work, frame);
  syncAppButton(members, true);
  board.pin(work);
  board.changed();
  save();
  if (restore) return `${group.title} is united.`;
  fly(order, before, () => item);
  canvas.focusItem(item);
  const message = `${group.title} united: ${order.length} ${order.length === 1 ? 'card' : 'cards'} in one frame.`;
  board.announce(message);
  return message;
}

/** Returns every card of a frame to the spot it had before the unite. */
export function breakApart(work: string): string {
  const canvas = board.canvas();
  const frame = frames.get(work);
  if (!canvas || !frame) return 'That app is not united.';
  const frameState = canvas.getItemState(frame.item);
  const before = new Map<HTMLElement, { x: number; y: number; width: number; height: number }>();
  const cards: HTMLElement[] = [];
  for (const id of frame.cards) {
    const card = board.findCard(id)?.card;
    if (!card) continue;
    const box = worldBox(card, frame.item);
    if (box) before.set(card, box);
    cards.push(card);
  }
  frames.delete(work);
  const items = new Map<HTMLElement, HTMLElement>();
  cards.forEach((card, index) => {
    const id = card.dataset.card ?? '';
    const spot = frame.previous.get(id) ?? { x: frameState.x + index * (SLOT_WIDTH + GAP), y: frameState.y };
    const restoreScroll = keepScroll(card);
    const item = board.drop(id, spot, false, true);
    restoreScroll();
    if (item) items.set(card, item);
  });
  canvas.removeItem(frame.item, { selectFallback: false });
  syncAppButton(cards, false);
  board.pin(work);
  board.changed();
  save();
  fly(cards, before, (card) => items.get(card) ?? null);
  const message = `${frame.title} broken apart: every card is back where it was.`;
  board.announce(message);
  return message;
}

/** U on a selected item: the app of that card or stack (a frame is already united). */
export function uniteFromItem(item: HTMLElement | null): Promise<string> | string {
  if (!item) return 'Select a card of an app first, then press U.';
  if (isUniteItem(item)) {
    const frame = frames.get(item.dataset.unite ?? '');
    return frame ? `${frame.title} is already united. Shift+U breaks it apart.` : 'Nothing was united.';
  }
  const id = item.classList.contains('cv-item-stack')
    ? item.querySelector<HTMLElement>('.cv-stack-top > .cv-card')?.dataset.card ?? ''
    : item.dataset.instanceId ?? '';
  const found = id ? board.findCard(id) : null;
  if (!found) return 'Unite works on an app’s cards. Select one first.';
  return unite(found.group.app, found.group.work);
}

/** Shift+U on a selected item: breaks apart its frame, or the frame of its app. */
export function splitFromItem(item: HTMLElement | null): string {
  if (!item) return 'Select a united app first, then press Shift+U.';
  if (isUniteItem(item)) return breakApart(item.dataset.unite ?? '');
  const id = item.dataset.instanceId ?? '';
  const work = board.findCard(id)?.group.work ?? '';
  return frames.has(work) ? breakApart(work) : 'That app is not united.';
}

function clearBar(): void {
  const input = documentRef?.getElementById('intent-input') as HTMLInputElement | null;
  if (input) input.value = '';
}

const UNITE_COMMAND = {
  slug: 'unite',
  title: 'Unite an app',
  purpose: 'Assemble every card of one app into its own frame (“/unite expense”)',
  run: async (text: string, report: Report = () => undefined) => {
    const [app] = text.trim() ? findApps(text) : [];
    if (!app) {
      report(text.trim() ? `No app matches “${text.trim()}”. Open the library (L) to browse.` : 'Name the app: /unite expense, or select one of its cards and press U.');
      return;
    }
    report(await unite(app.app, app.work));
    clearBar();
  },
};

const SPLIT_COMMAND = {
  slug: 'split',
  title: 'Break an app apart',
  purpose: 'Return a united app’s cards to where they were (“/split expense”)',
  aliases: ['break'],
  run: (text: string, report: Report = () => undefined) => {
    const united = findApps(text).filter((app) => frames.has(app.work));
    const work = united[0]?.work ?? (text.trim() ? '' : frames.size === 1 ? [...frames.keys()][0] ?? '' : '');
    report(work ? breakApart(work) : text.trim() ? `${text.trim()} is not united.` : 'Name the united app: /split expense.');
    clearBar();
  },
};

function completions(slug: string, onlyUnited: boolean): (rest: string) => Completion[] {
  return (rest) => findApps(rest)
    .filter((app) => !onlyUnited || frames.has(app.work))
    .slice(0, 10)
    .map((app) => ({
      badge: app.number,
      title: app.title,
      detail: `${app.cards.length} cards${frames.has(app.work) ? ' · united' : ''}`,
      value: `/${slug} ${app.slug || app.number}`,
    }));
}

/** Starts unite once (after the board and the library): hooks, buttons, commands, kept frames. */
export function startUnite(document_: Document): void {
  if (documentRef) return;
  documentRef = document_;
  setUniteHooks({
    has: (id) => [...frames.values()].some((frame) => frame.cards.includes(id)),
    count: () => [...frames.values()].reduce((sum, frame) => sum + frame.cards.length, 0),
    hasWork: (work) => frames.has(work),
    remove: (ids) => {
      const handled: string[] = [];
      for (const frame of [...frames.values()]) {
        const out = frame.cards.filter((id) => ids.includes(id));
        if (!out.length) continue;
        handled.push(...out);
        for (const id of out) {
          const card = board.findCard(id)?.card;
          if (card) {
            syncAppButton([card], false);
            board.park(card);
          }
        }
        frame.cards = frame.cards.filter((id) => !out.includes(id));
        if (!frame.cards.length) {
          board.canvas()?.removeItem(frame.item, { selectFallback: false });
          frames.delete(frame.work);
        } else {
          const count = frame.root.querySelector('[data-unite-count]');
          if (count) count.textContent = `${frame.cards.length} ${frame.cards.length === 1 ? 'card' : 'cards'}`;
        }
      }
      if (handled.length) save();
      return handled;
    },
    settled: () => {
      restored = true;
      for (const row of loadUnited(board.member())) {
        if (!board.group(row.work)) continue;
        void unite(row.app, row.work, { at: { x: row.x, y: row.y }, previous: new Map(Object.entries(row.previous)) });
      }
      save();
    },
  });
  onLayoutSaved(save);
  document_.addEventListener('click', (event) => {
    const target = event.target as Element | null;
    const split = target?.closest<HTMLElement>('[data-unite-split]');
    if (split) {
      breakApart(split.closest<HTMLElement>('.cv-unite')?.dataset.unite ?? '');
      return;
    }
    const appButton = target?.closest<HTMLElement>('[data-unite-app]');
    if (appButton) {
      const card = appButton.closest<HTMLElement>('.cv-card[data-card]');
      const found = card ? board.findCard(card.dataset.card ?? '') : null;
      if (!found) return;
      if (frames.has(found.group.work)) breakApart(found.group.work);
      else void unite(found.group.app, found.group.work);
      return;
    }
    const row = target?.closest<HTMLElement>('[data-lib-unite]')?.closest<HTMLElement>('.cv-lib-app');
    if (row?.dataset.app && row.dataset.work) {
      if (frames.has(row.dataset.work)) breakApart(row.dataset.work);
      else void unite(row.dataset.app, row.dataset.work);
      if (phone()) documentRef?.querySelector<HTMLElement>('[data-lib-close]')?.click();
    }
  });
  registerCommands([UNITE_COMMAND, SPLIT_COMMAND]);
  registerCompleter('unite', completions('unite', false));
  registerCompleter('split', completions('split', true));
}
