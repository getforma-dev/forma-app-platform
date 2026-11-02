// Whole-work placement: Card, Stack and Open are modes of one retained item.
// Components remain trusted server-owned DOM. This browser stores layout only.
import { appComponentVisible, applyPresentationTheme, disposePresentation, parsePresentation, presentationFor } from '../presentation/presentation';
import { createCanvasItem } from '../canvas-vendor/index';
import { ensureAppGroup, rememberAppPlacement, isStandaloneWork, removeWork, refreshGroupPresentation, runGroup, onLayoutSaved, setUniteHooks, stackBoard as board } from './board';
import { loadLayout, loadPlaced, loadStacks, loadUnited, saveUnited, migrateAppViews, type StoredUnite, type AppViewMode } from './layout';
import { findApps } from './library';
import { pageSize } from './page';
import { dismissSharing, openSharing } from '../sharing/dialog';
import { el } from './render';
import { buildAppWindow, appWindowIcon } from './app-window';
import { toggleAppSummary, disposeAppSummary } from './explain';
import { registerCommands, registerCompleter, type Completion } from './slash';
import { releaseCards } from './stacks';
import { declaredLayout } from './page-layout';
export type { AppViewMode } from './layout';

type Spot = {x: number; y: number};
type Frame = {
  work: string; app: string; title: string; cards: string[];
  item: HTMLElement; root: HTMLElement; mode: AppViewMode; top: number;
  openSize: {width: number; height: number}; baseSize: {width: number; height: number}; previous: Map<string, Spot>;
};
export type AppView = Pick<Frame, 'app'|'work'|'mode'|'top'|'item'> & {cards: string[]};
export type AppViewOptions = {at?: Spot; focus?: boolean; preserveMode?: boolean; restore?: StoredUnite; isCurrent?: () => boolean};
type Report = (message: string) => void;
const frames = new Map<string, Frame>();
const pending = new Map<string, Promise<string>>();
const generations = new Map<string, number>();
const removedWorks = new Set<string>();
const retained = new Map<string, StoredUnite>();
let documentRef: Document | null = null;
let restored = false;
let restoration: Promise<void> | null = null;
const WIDE = 1240, NARROW = 760, PHONE = 400;
const phone = () => matchMedia('(max-width: 760px)').matches;

/** Preserve form/table scroll positions when their same nodes change hosts. */
export function keepScroll(root: HTMLElement): () => void {
  const spots: [HTMLElement, number, number][] = [];
  for (const node of [root, ...root.querySelectorAll<HTMLElement>('*')])
    if (node.scrollTop || node.scrollLeft) spots.push([node, node.scrollTop, node.scrollLeft]);
  return () => { for (const [node, top, left] of spots) { node.scrollTop = top; node.scrollLeft = left; } };
}
export function isUniteItem(item: HTMLElement): boolean { return item.classList.contains('cv-item-unite'); }
export function appViewForWork(work: string): AppView | null {
  const frame = frames.get(work);
  return frame ? {app: frame.app, work, mode: frame.mode, top: frame.top, cards: [...frame.cards], item: frame.item} : null;
}
export function appViewOf(item: HTMLElement | null): AppView | null {
  return item ? appViewForWork(item.dataset.unite ?? '') : null;
}
export function frameCardsOf(id: string): string[] | null {
  for (const frame of frames.values()) if (frame.cards.includes(id)) return visibleCards(frame).map(card => card.dataset.card ?? '');
  return null;
}
export function frameCardsOfItem(item: HTMLElement): string[] { return appViewOf(item)?.cards ?? []; }
export function selectedAppCard(item: HTMLElement): HTMLElement | null {
  const frame = frames.get(item.dataset.unite ?? '');
  if (!frame) return null;
  const cards = visibleCards(frame);
  return frame.mode === 'stack' ? cards.find(card => card.dataset.card === frame.cards[frame.top]) ?? cards[0] ?? null : cards.find(card => card.dataset.kind === 'app') ?? cards[0] ?? null;
}
function save(): void {
  const canvas = board.canvas(); if (!canvas || !restored) return;
  for (const frame of frames.values()) {
    const state = canvas.getItemState(frame.item);
    retained.set(frame.work, {app: frame.app, work: frame.work, x: state.x, y: state.y,
      previous: Object.fromEntries(frame.previous), mode: frame.mode, top: frame.top, ...frame.openSize});
  }
  saveUnited(board.member(), [...retained.values()]);
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

/** Puts the cards in their slots; returns them in reading order and the frame's width. */
function fill(root: HTMLElement, cards: HTMLElement[], layout = ''): { order: HTMLElement[]; width: number } {
  const slots = declaredLayout(cards.map(card => card.dataset.card ?? ''), layout);
  if (slots) {
    const byId = new Map(cards.map(card => [card.dataset.card ?? '', card]));
    const grid = root.querySelector<HTMLElement>('.cv-unite-grid')!;
    const order = slots.flatMap(slot => byId.get(slot.id) ?? []);
    root.dataset.declaredLayout = 'true';
    grid.replaceChildren(...slots.flatMap(slot => {
      const card = byId.get(slot.id); if (!card) return [];
      return el(root.ownerDocument, 'section', { class: 'cv-unite-part', 'data-component': card.dataset.component ?? '', 'data-part-row': String(slot.row), 'data-part-column': String(slot.column), 'data-part-width': String(slot.width) }, card);
    }));
    const width = phone() ? PHONE : WIDE;
    layoutPage(root, width);
    const count = root.querySelector('[data-unite-count]');
    if (count) count.textContent = `${cards.length} ${cards.length === 1 ? 'part' : 'parts'}`;
    return { order, width };
  }
  delete root.dataset.declaredLayout;
  root.querySelector('.cv-unite-grid')!.replaceChildren(...['kpi','main','side'].map(name => el(root.ownerDocument, 'div', {class: `cv-unite-${name === 'kpi' ? 'kpis' : name}`, 'data-slot': name})));
  const { kpi, main, side } = arrange(cards);
  const slot = (name: string) => root.querySelector<HTMLElement>(`[data-slot="${name}"]`)!;
  slot('kpi').replaceChildren(...kpi);
  slot('main').replaceChildren(...main);
  slot('side').replaceChildren(...side);
  const width = phone() ? PHONE : main.length && side.length ? WIDE : NARROW;
  layoutPage(root, width);
  const count = root.querySelector('[data-unite-count]');
  if (count) count.textContent = `${cards.length} ${cards.length === 1 ? 'part' : 'parts'}`;
  return { order: [...kpi, ...main, ...side], width };
}

/** Reflow the existing parts when this page narrows, independently of the browser. */
function layoutPage(root: HTMLElement, width: number): void {
  if (root.dataset.declaredLayout === 'true') {
    const single = width <= NARROW;
    const grid = root.querySelector<HTMLElement>('.cv-unite-grid')!;
    grid.style.gridTemplateColumns = single ? 'minmax(0, 1fr)' : 'repeat(12, minmax(0, 1fr))';
    grid.style.gridTemplateAreas = 'none';
    for (const part of grid.querySelectorAll<HTMLElement>('.cv-unite-part')) {
      part.style.gridColumn = single ? '1' : `${Number(part.dataset.partColumn) + 1} / span ${Number(part.dataset.partWidth)}`;
      part.style.gridRow = single ? 'auto' : String(Number(part.dataset.partRow));
    }
    root.classList.toggle('is-single', single);
    return;
  }
  const has = (slot: string) => Boolean(root.querySelector(`[data-slot="${slot}"]`)?.childElementCount);
  const single = width <= NARROW;
  const both = has('main') && has('side');
  const grid = root.querySelector<HTMLElement>('.cv-unite-grid')!;
  const columns = !single && both;
  grid.style.gridTemplateColumns = columns ? 'minmax(0, 1.55fr) minmax(0, 1fr)' : 'minmax(0, 1fr)';
  const rows: string[] = [];
  if (has('kpi')) rows.push(columns ? '"kpi kpi"' : '"kpi"');
  if (columns) rows.push('"main side"');
  else {
    if (has('main')) rows.push('"main"');
    if (has('side')) rows.push('"side"');
  }
  grid.style.gridTemplateAreas = rows.join(' ');
  root.classList.toggle('is-single', single);
}

function viewportWorld(): {width: number; height: number} {
  const zoom = board.canvas()?.getCamera().zoom || 1;
  const view = documentRef?.querySelector<HTMLElement>('[data-forma-canvas-viewport]');
  return {width: (view?.clientWidth ?? 1000) / zoom, height: (view?.clientHeight ?? 800) / zoom};
}
function applySize(frame: Frame, width: number, height: number): void {
  const canvas = board.canvas(); if (!canvas) return;
  const viewport = viewportWorld(), size = pageSize({width, height}, viewport);
  frame.baseSize = size;
  const companion = !frame.root.querySelector<HTMLElement>('[data-app-companion]')!.hidden;
  const beside = viewport.width >= 720;
  const total = pageSize({width: size.width + (companion && beside ? 336 : 0), height: companion ? Math.max(size.height, beside ? 420 : 620) : size.height}, viewport);
  frame.root.classList.toggle('has-companion', companion);
  frame.root.classList.toggle('is-companion-below', companion && !beside);
  canvas.resizeItem(frame.item, total.width, total.height);
  frame.item.style.height = `${Math.round(total.height)}px`;
  frame.root.classList.add('is-sized');
  if (frame.mode === 'open') { frame.openSize = size; layoutPage(frame.root, total.width - (companion && beside ? 336 : 0)); }
}
/** Expanding a window should keep its controls reachable without moving its saved position. */
function revealFrame(frame: Frame): void {
  const viewport = documentRef?.querySelector<HTMLElement>('[data-forma-canvas-viewport]');
  if (!viewport || frames.get(frame.work) !== frame || !frame.item.isConnected) return;
  // Focusing an offscreen control can scroll an overflow-hidden viewport.
  // This surface is positioned by its camera; native offsets would double-pan it.
  viewport.scrollLeft = 0; viewport.scrollTop = 0;
  const bounds = viewport.getBoundingClientRect(), rect = frame.item.getBoundingClientRect();
  if (rect.left < bounds.left + 16 || rect.right > bounds.right - 16 || rect.top < bounds.top + 16 || rect.bottom > bounds.bottom - 56) board.canvas()?.centerItem(frame.item);
}
function releaseFocus(frame: Frame): void {
  const card = documentRef?.querySelector<HTMLElement>('#cv-focus .cv-card[data-card]');
  if (card && frame.cards.includes(card.dataset.card ?? '')) documentRef?.querySelector<HTMLElement>('[data-focus-close]')?.click();
}
function cardsOf(frame: Frame): HTMLElement[] { return frame.cards.flatMap(id => board.findCard(id)?.card ?? []); }
function visibleCards(frame: Frame): HTMLElement[] { return cardsOf(frame).filter(card => appComponentVisible(frame.app, frame.work, card)); }
function renderMode(frame: Frame, mode: AppViewMode): void {
  releaseFocus(frame); disposePresentation(frame.root);
  const cards = cardsOf(frame), scrolls = cards.map(keepScroll);
  const parking = frame.root.querySelector<HTMLElement>('[data-app-view-parking]')!;
  parking.append(...cards);
  frame.mode = isStandaloneWork(frame.app, frame.work) ? 'open' : mode;
  const standalone = isStandaloneWork(frame.app, frame.work);
  frame.root.dataset.placementKind = standalone ? 'card' : 'app';
  frame.root.querySelector<HTMLElement>('.cv-app-view-picker')!.hidden = standalone;
  frame.root.querySelector<HTMLElement>('[data-app-summary]')!.hidden = standalone;
  for (const control of frame.root.querySelectorAll<HTMLElement>('.cv-window-control[data-app-view-mode]')) control.hidden = standalone;
  frame.root.querySelector('.cv-unite-actions')?.setAttribute('aria-label', standalone ? 'Card' : 'App');
  const ask = frame.root.querySelector('[data-unite-ask]');
  if (ask) { ask.setAttribute('aria-label', standalone ? 'Ask about this card' : 'Ask about this app'); ask.setAttribute('title', standalone ? 'Ask about this card' : 'Ask about this app'); }
  const edit = frame.root.querySelector('[data-edit-presentation]');
  if (edit) { edit.setAttribute('aria-label', standalone ? 'Card settings' : 'App settings'); edit.setAttribute('title', standalone ? 'Card settings' : 'App settings'); }
  frame.root.dataset.appView = frame.mode; frame.item.dataset.appView = frame.mode;
  for (const name of ['face', 'stack', 'open'] as const) {
    const host = frame.root.querySelector<HTMLElement>(`[data-app-view-${name}]`)!;
    host.hidden = name !== frame.mode; host.inert = host.hidden;
  }
  for (const button of frame.root.querySelectorAll<HTMLElement>('[data-app-view-mode]')) button.setAttribute('aria-pressed', String(button.dataset.appViewMode === frame.mode));
  const resize = frame.root.querySelector<HTMLElement>('[data-unite-resize]')!;
  resize.hidden = frame.mode !== 'open';
  if (frame.mode === 'face') {
    const host = frame.root.querySelector<HTMLElement>('[data-app-view-face]')!;
    const document_ = frame.root.ownerDocument;
    const components = visibleCards(frame).filter(card => card.dataset.kind !== 'app');
    const summary = parsePresentation(board.group(frame.work)?.presentation).description || 'Open your workspace to explore the data and use its tools.';
    const labels: Record<string, string> = {summary: 'Metrics', chart: 'Charts', table: 'Tables', status: 'Breakdown', timeline: 'Timeline', ledger: 'Ledger', records: 'Records', form: 'Forms'};
    const kinds = [...new Set(components.map(card => labels[card.dataset.kind ?? ''] ?? 'Data'))];
    host.replaceChildren(
      el(document_, 'div', {class: 'cv-app-card-heading'}, el(document_, 'span', {class: 'cv-app-card-mark', 'aria-hidden': 'true'}, appWindowIcon(document_, 'app')), el(document_, 'h3', {'data-app-face-title': ''}, frame.title)),
      el(document_, 'p', {class: 'cv-app-face-summary'}, summary),
      el(document_, 'div', {class: 'cv-app-card-bottom'}, el(document_, 'div', {class: 'cv-app-card-kinds'}, ...kinds.slice(0, 3).map(kind => el(document_, 'span', {}, kind))),
        el(document_, 'button', {type: 'button', class: 'cv-app-open-button', 'data-app-view-mode': 'open'}, 'Open app', appWindowIcon(document_, 'external'))));
    applySize(frame, PHONE, 300);
  } else if (frame.mode === 'stack') {
    const visible = visibleCards(frame);
    const card = visible.find(card => card.dataset.card === frame.cards[frame.top]) ?? visible[0];
    frame.top = card ? frame.cards.indexOf(card.dataset.card ?? '') : 0;
    frame.root.querySelector('[data-app-stack-holder]')!.replaceChildren(...(card ? [card] : []));
    const position = frame.root.querySelector('[data-app-stack-position]');
    if (position) position.textContent = card ? `View ${visible.indexOf(card) + 1} of ${visible.length}` : 'No visible components';
    for (const button of frame.root.querySelectorAll<HTMLButtonElement>('[data-app-stack-step]')) button.disabled = visible.length < 2;
    applySize(frame, 460, Math.min(620, viewportWorld().height * 0.8));
  } else {
    fill(frame.root, cards, board.group(frame.work)?.partLayout ?? '');
    presentationFor(frame.root);
    applySize(frame, frame.openSize.width, frame.openSize.height);
  }
  applyPresentationTheme(frame.root, board.group(frame.work)?.presentation ?? '{}');
  const count = frame.root.querySelector('[data-unite-count]');
  const componentCount = visibleCards(frame).filter(card => card.dataset.kind !== 'app').length;
  if (count) count.textContent = `${componentCount} ${componentCount === 1 ? 'component' : 'components'}`;
  for (const restore of scrolls) restore();
  runGroup(frame.work);
}
export function shuffleAppView(item: HTMLElement, step: number): boolean {
  const frame = frames.get(item.dataset.unite ?? '');
  if (!frame || frame.mode !== 'stack' || !Number.isSafeInteger(step)) return false;
  const visible = visibleCards(frame);
  if (!visible.length) return false;
  const current = Math.max(0, visible.findIndex(card => card.dataset.card === frame.cards[frame.top]));
  const next = ((current + step) % visible.length + visible.length) % visible.length;
  frame.top = frame.cards.indexOf(visible[next]!.dataset.card ?? '');
  renderMode(frame, 'stack'); save(); return true;
}
/** One queue per work prevents parallel Add/Stack/Open requests from duplicating it. */
export function setAppView(app: string, work: string, mode: AppViewMode, options: AppViewOptions = {}): Promise<string> {
  const generation = generations.get(work) ?? 0;
  const prior = pending.get(work) ?? Promise.resolve('');
  const request = prior.catch(() => '').then(() => setAppViewNow(app, work, mode, options, generation));
  pending.set(work, request);
  void request.finally(() => { if (pending.get(work) === request) pending.delete(work); }).catch(() => undefined);
  return request;
}
async function setAppViewNow(app: string, work: string, mode: AppViewMode, options: AppViewOptions, generation: number): Promise<string> {
  const canvas = board.canvas();
  if (!canvas || !documentRef || !['face','stack','open'].includes(mode)) return 'Nothing was placed.';
  if ((generations.get(work) ?? 0) !== generation) return 'That placement was cancelled.';
  if (options.restore && removedWorks.has(work)) return 'That placement was removed.';
  if (!await ensureAppGroup(app, work)) return 'That app could not be opened. Nothing was placed.';
  if (options.isCurrent && !options.isCurrent()) return 'That placement was cancelled.';
  if ((generations.get(work) ?? 0) !== generation) return 'That placement was cancelled.';
  const group = board.group(work);
  if (!group || group.app !== app) return 'That app could not be opened.';
  let frame = frames.get(work);
  if (frame && frame.app !== app) return 'That app could not be opened.';
  // Generic live placement resolves the mode here, after earlier explicit requests.
  // A mode captured before this work's queue runs could overwrite a user's Open.
  mode = isStandaloneWork(app, work) ? 'open' : options.preserveMode && frame ? frame.mode : mode;
  if (frame) {
    if (!options.restore && (frame.mode !== mode || frame.root.dataset.placementKind !== (isStandaloneWork(app, work) ? 'card' : 'app'))) renderMode(frame, mode);
  } else {
    const cards = group.cards, first = cards.flatMap(card => board.findCard(card.dataset.card ?? '')?.item ?? [])[0];
    const fallback = first ? canvas.getItemState(first) : {x: 64, y: 64 + frames.size * 420};
    const at = options.at ?? (options.restore ? {x: options.restore.x, y: options.restore.y} : fallback);
    const ids = cards.map(card => card.dataset.card ?? '').filter(Boolean);
    releaseCards(ids);
    for (const id of ids) board.lift(id);
    const root = buildAppWindow(documentRef, app, work, group.title, group.origins, group.canShare, group.canEditPresentation, group.presentation, isStandaloneWork(app, work));
    const item = createCanvasItem({instanceId: `unite-${work}`, displayName: group.title, preferredWidth: PHONE, minHeight: 300, content: root});
    item.classList.add('cv-item', 'cv-item-unite'); item.dataset.unite = work;
    const viewport = viewportWorld();
    frame = {app, work, title: group.title, cards: ids, item, root, mode, top: options.restore?.top ?? 0,
      previous: new Map(Object.entries(options.restore?.previous ?? {})), baseSize: {width: PHONE, height: 300},
      openSize: {width: options.restore?.width ?? Math.min(WIDE, viewport.width * 0.8), height: options.restore?.height ?? Math.min(680, viewport.height * 0.75)}};
    frames.set(work, frame);
    canvas.mountItem(item, at, {focus: false});
    renderMode(frame, mode);
    for (const id of ids) board.show(id);
  }
  if (!options.restore) removedWorks.delete(work);
  rememberAppPlacement(work); board.pin(work); board.changed(); save();
  if (!options.restore && options.focus !== false) { canvas.centerItem(frame.item); board.flash(frame.item); }
  const message = `${frame.title} is on your canvas in ${frame.mode === 'face' ? 'Card' : frame.mode === 'stack' ? 'Browse' : 'Open'} view.`;
  if (!options.restore) board.announce(message);
  return message;
}
/** Arrange whole placements without changing their mode, size or component nodes. */
export function arrangeAppViews(): number {
  const canvas = board.canvas(); if (!canvas) return 0;
  const origin = 64, gap = 48, right = Math.max(origin + PHONE, viewportWorld().width - origin);
  let x = origin, y = origin, rowHeight = 0;
  for (const frame of frames.values()) {
    const state = canvas.getItemState(frame.item);
    const width = state.width, height = frame.item.offsetHeight;
    if (x > origin && x + width > right) { x = origin; y += rowHeight + gap; rowHeight = 0; }
    canvas.moveItemTo(frame.item, x, y);
    x += width + gap; rowHeight = Math.max(rowHeight, height);
  }
  save(); board.changed();
  return frames.size;
}
/** Backward-compatible internal entry: Open is idempotent, never a toggle. */
export function unite(app: string, work: string): Promise<string> { return setAppView(app, work, 'open'); }
export function removeAppView(work: string): boolean {
  generations.set(work, (generations.get(work) ?? 0) + 1);
  removedWorks.add(work);
  retained.delete(work);
  const frame = frames.get(work);
  if (!frame) { save(); return false; }
  releaseFocus(frame); disposePresentation(frame.root); disposeAppSummary(work);
  for (const card of cardsOf(frame)) board.park(card);
  frames.delete(work); board.canvas()?.removeItem(frame.item, {selectFallback: false});
  board.changed(); save(); return true;
}
/** Kept only as a safe compatibility response for old controls/commands. */
export function breakApart(_work: string): string { return 'Apps stay together. Choose Card, Browse or Open view.'; }
export function splitFromItem(_item: HTMLElement | null): string { return breakApart(''); }
export function uniteFromItem(item: HTMLElement | null): Promise<string> | string {
  const view = appViewOf(item);
  if (view) return setAppView(view.app, view.work, 'open');
  const found = item ? board.findCard(item.dataset.instanceId ?? '') : null;
  return found ? setAppView(found.group.app, found.group.work, 'open') : 'Select an app first, then choose Open.';
}
export function dismissPageShare(_document: Document): boolean { return dismissSharing(); }
/** Restore only prior placements. Unavailable work metadata is not used as authority. */
export function restoreAppViews(): Promise<void> {
  if (restoration) return restoration;
  restoration = (async () => {
    const united = loadUnited(board.member()).filter(row => !removedWorks.has(row.work));
    for (const row of united) retained.set(row.work, row);
    const works = [...loadPlaced(board.member())].flatMap(([work, entry], index) => {
      if (removedWorks.has(work)) return [];
      const group = board.group(work);
      const positions = loadLayout(entry.app, work);
      const prior = entry.cards.flatMap(id => positions.get(id) ?? [])[0];
      return group?.app === entry.app ? [{app: entry.app, work, cards: group.cards.map(card => card.dataset.card ?? ''), standalone: isStandaloneWork(entry.app, work), at: prior ? {x: prior.x, y: prior.y} : {x: 64, y: 64 + index * 420}}] : [];
    });
    for (const row of united) if (!works.some(work => work.work === row.work)) {
      const group = board.group(row.work);
      if (group?.app === row.app) works.push({app: row.app, work: row.work, cards: group.cards.map(card => card.dataset.card ?? ''), standalone: isStandaloneWork(row.app, row.work), at: {x: row.x, y: row.y}});
    }
    for (const row of migrateAppViews(united, loadStacks(board.member()), works))
      if (!removedWorks.has(row.work)) await setAppView(row.app, row.work, row.mode ?? 'open', {restore: row, focus: false});
    restored = true; save();
  })();
  return restoration;
}
function bindResize(document_: Document): void {
  document_.addEventListener('pointerdown', event => {
    const handle = (event.target as Element | null)?.closest<HTMLElement>('[data-unite-resize]');
    if (!handle || event.button !== 0 || handle.closest('.is-hand-tool, .is-pan-ready')) return;
    const frame = frames.get(handle.closest<HTMLElement>('.cv-unite')?.dataset.unite ?? '');
    const canvas = board.canvas(); if (!frame || !canvas || frame.mode !== 'open') return;
    event.preventDefault(); event.stopPropagation();
    const shown = canvas.getItemState(frame.item);
    const beside = frame.root.classList.contains('has-companion') && !frame.root.classList.contains('is-companion-below');
    const pointer = event.pointerId, origin = {width: shown.width - (beside ? 336 : 0)}, startX = event.clientX, startY = event.clientY;
    const originHeight = frame.item.offsetHeight;
    const move = (ev: PointerEvent) => {
      if (ev.pointerId !== pointer || frame.mode !== 'open') return;
      const zoom = canvas.getCamera().zoom || 1;
      applySize(frame, origin.width + (ev.clientX - startX) / zoom, originHeight + (ev.clientY - startY) / zoom);
    };
    const end = (ev: PointerEvent) => {
      if (ev.pointerId !== pointer) return;
      document_.removeEventListener('pointermove', move); document_.removeEventListener('pointerup', end); document_.removeEventListener('pointercancel', end); save();
    };
    document_.addEventListener('pointermove', move); document_.addEventListener('pointerup', end); document_.addEventListener('pointercancel', end);
  }, true);
}
const UNITE_COMMAND = {slug: 'unite', title: 'Open an app', purpose: 'Open one whole app on the canvas',
  run: async (text: string, report: Report = () => undefined) => {
    const [app] = text.trim() ? findApps(text) : [];
    report(app ? await setAppView(app.app, app.work, 'open') : 'Name an app to open, or choose one from the Library.');
  }};
// Reserved historical spellings remain a closed refusal, never an extraction.
const SPLIT_COMMAND = {slug: 'split', title: 'Apps stay together', purpose: 'Choose Card, Browse or Open instead', aliasOf: 'unite', aliases: ['break', 'fan'],
  run: (_text: string, report: Report = () => undefined) => { report(breakApart('')); }};
function completions(rest: string): Completion[] {
  return findApps(rest).slice(0, 10).map(app => ({badge: app.number, title: app.title, detail: 'Open app', value: `/unite ${app.slug || app.number}`}));
}
export function startUnite(document_: Document): void {
  if (documentRef) return;
  documentRef = document_;
  setUniteHooks({has: id => [...frames.values()].some(frame => frame.cards.includes(id)),
    count: () => frames.size, hasWork: work => frames.has(work),
    remove: ids => {
      const affected = [...frames.values()].filter(frame => frame.cards.some(id => ids.includes(id)));
      const handled = affected.flatMap(frame => frame.cards);
      for (const frame of affected) removeAppView(frame.work);
      return handled;
    }, place: (app, work, mode, options) => setAppView(app, work, mode, options),
    restore: restoreAppViews, arrange: arrangeAppViews, removeWork: work => removeAppView(work), settled: () => undefined});
  onLayoutSaved(save);
  document_.addEventListener('forma:app-view-preferences', event => {
    const detail = (event as CustomEvent<{app: string; work: string}>).detail;
    const frame = detail && frames.get(detail.work);
    if (frame?.app === detail.app) { renderMode(frame, frame.mode); save(); }
  });
  document_.defaultView?.addEventListener('storage', event => {
    for (const frame of frames.values()) if (event.key === null || event.key === `forma.app.hidden.v1:${frame.app}/${frame.work}`) { renderMode(frame, frame.mode); save(); }
  });
  document_.addEventListener('forma:work-renamed', event => {
    const detail = (event as CustomEvent<{app: string; work: string; name: string}>).detail;
    const frame = detail && frames.get(detail.work); if (!frame || frame.app !== detail.app) return;
    frame.title = detail.name;
    const title = frame.root.querySelector<HTMLElement>('.cv-unite-title');
    if (title) {title.textContent = detail.name; title.title = detail.name;}
    const faceTitle = frame.root.querySelector('[data-app-face-title]');
    if (faceTitle) faceTitle.textContent = detail.name;
    frame.root.setAttribute('aria-label', detail.name); frame.item.dataset.widgetName = detail.name;
    frame.item.setAttribute('aria-label', detail.name); frame.item.querySelector('.widget-drag-handle')?.setAttribute('aria-label', `Move ${detail.name}`);
  });
  document_.addEventListener('forma:app-companion', event => {
    const detail = (event as CustomEvent<{app: string; work: string; open: boolean}>).detail;
    const frame = detail && frames.get(detail.work); if (!frame || frame.app !== detail.app) return;
    frame.root.querySelector('[data-app-summary]')?.setAttribute('aria-expanded', String(detail.open));
    applySize(frame, frame.baseSize.width, frame.baseSize.height);
    if (detail.open) revealFrame(frame);
  });
  document_.addEventListener('forma:presentation-saved', event => {
    const detail = (event as CustomEvent<{app: string; work: string}>).detail; if (!detail) return;
    void refreshGroupPresentation(detail.app, detail.work).then(ok => {
      const frame = frames.get(detail.work);
      if (ok && frame?.app === detail.app) renderMode(frame, frame.mode);
    });
  });
  document_.addEventListener('click', event => {
    const target = event.target as Element | null, root = target?.closest<HTMLElement>('.cv-unite');
    const frame = root && frames.get(root.dataset.unite ?? '');
    if (frame && target?.closest('[data-app-summary]')) {
      toggleAppSummary(frame.app, frame.work, frame.title, frame.root.querySelector<HTMLElement>('[data-app-companion]')!); return;
    }
    if (frame && target?.closest('[data-app-view-remove]')) { removeWork(frame.work); return; }
    if (frame && target?.closest('[data-unite-share]')) {openSharing(document_, frame.app, frame.work, frame.title); return;}
    const choice = target?.closest<HTMLElement>('[data-app-view-mode]');
    if (frame && choice) {void setAppView(frame.app, frame.work, choice.dataset.appViewMode as AppViewMode, {focus: false}).then(() => revealFrame(frame)); return;}
    const step = target?.closest<HTMLElement>('[data-app-stack-step]');
    if (frame && step) {shuffleAppView(frame.item, Number(step.dataset.appStackStep)); return;}
    const appButton = target?.closest<HTMLElement>('[data-unite-app]');
    const card = appButton?.closest<HTMLElement>('.cv-card[data-card]');
    const found = card ? board.findCard(card.dataset.card ?? '') : null;
    if (found) void setAppView(found.group.app, found.group.work, 'open');
  });
  document_.addEventListener('dblclick', event => {
    const target = event.target as Element | null;
    if (target?.closest('button,a,input,select,textarea,summary,[data-app-companion]')) return;
    const frame = frames.get(target?.closest<HTMLElement>('.cv-unite')?.dataset.unite ?? '');
    if (frame?.mode === 'face') void setAppView(frame.app, frame.work, 'open', {focus: false}).then(() => revealFrame(frame));
  });
  bindResize(document_); registerCommands([UNITE_COMMAND, SPLIT_COMMAND]); registerCompleter('unite', completions);
}
