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
import { submitCommand } from './commands';
import { clearLayouts, loadCamera, loadLayout, saveCamera, saveLayout, type ItemLayout } from './layout';
import { subscribeLive, type LiveEvent } from './live';
import { parametersOf, runView } from './queries';
import { buildForm, buildGroup, el, groupJson, isUuid, renderNoValue, type FormJson } from './render';
import { recordsCardsOf, refreshRecords, wireRecordsCard } from './records';
import { refreshCommands } from './slash';
import { ledgerCardsOf, refreshLedger, wireLedgerCard } from './ledger';
import { wireReceiptAttach } from './receipt';

type Group = {
  app: string;
  work: string;
  section: HTMLElement;
  items: HTMLElement[];
  /** The member arranged this group (now or in an earlier visit): its layout is kept. */
  pinned: boolean;
};

const WIDTH: Record<string, number> = { app: 340, summary: 400, timeline: 620, form: 360, records: 680, ledger: 520 };
const HEIGHT: Record<string, number> = { app: 300, summary: 250, timeline: 340, form: 300, records: 260, ledger: 280 };
const GAP = 28;
const ROW_GAP = 72;
const ORIGIN = 64;

let canvas: WidgetCanvas | null = null;
let host: HTMLElement | null = null;
const groups = new Map<string, Group>();
let saveTimer = 0;
let reflowFrame = 0;
let fitPending = false;
const resized = typeof ResizeObserver === 'undefined' ? null : new ResizeObserver(() => scheduleReflow());
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
  let y = ORIGIN;
  for (const group of groups.values()) {
    let bottom = y;
    if (group.pinned) {
      for (const item of group.items) {
        bottom = Math.max(bottom, Number(item.dataset.canvasY ?? 0) + item.offsetHeight);
      }
    } else {
      let x = ORIGIN;
      for (const item of group.items) {
        canvas.moveItemTo(item, x, y);
        x += item.offsetWidth + GAP;
        bottom = Math.max(bottom, y + item.offsetHeight);
      }
    }
    y = bottom + ROW_GAP;
  }
  if (fitPending) canvas.fitAll();
}
function scheduleReflow(): void {
  if (!reflowFrame) reflowFrame = requestAnimationFrame(reflow);
}

/** `GENUI_DEMO_MODE` is on for this page (`data-demo="on"`). */
function demo(): boolean {
  return host?.dataset.demo === 'on';
}

function announce(message: string): void {
  const status = host?.ownerDocument.getElementById('cv-activity');
  if (status) status.textContent = message;
}

function nextRow(): number {
  let bottom = ORIGIN;
  for (const group of groups.values()) {
    for (const item of group.items) {
      bottom = Math.max(bottom, Number(item.dataset.canvasY ?? 0) + item.offsetHeight + ROW_GAP);
    }
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
  host.classList.add('has-groups');
  host.ownerDocument.getElementById('cv-empty')?.setAttribute('hidden', '');
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
  if (!chip) return;
  let cards = 0;
  for (const group of groups.values()) cards += group.items.filter((item) => !item.querySelector('.cv-app')).length;
  const apps = groups.size;
  chip.textContent = `${apps} ${apps === 1 ? 'app' : 'apps'} · ${cards} ${cards === 1 ? 'card' : 'cards'}`;
}

function updateZoomLabel(): void {
  const button = host?.ownerDocument.querySelector<HTMLElement>('[data-tool="zoom-reset"]');
  if (button && canvas) button.textContent = `${Math.round(canvas.getCamera().zoom * 100)}%`;
}

/** A short glow on a card a live event just updated (CSS honors reduced motion). */
function pulse(card: HTMLElement): void {
  const item = card.closest<HTMLElement>('.cv-item') ?? card;
  item.classList.remove('is-live-updated');
  void item.offsetWidth;
  item.classList.add('is-live-updated');
  window.setTimeout(() => item.classList.remove('is-live-updated'), 1600);
}

/**
 * Runs every view card of a group with its window form's parameters;
 * `live` marks a re-run a live event asked for (the cards pulse on arrival).
 */
export function runGroup(work: string, live = false): void {
  const group = groups.get(work);
  if (!group) return;
  const form = group.items
    .map((item) => item.querySelector<HTMLFormElement>('form[data-window]'))
    .find((found): found is HTMLFormElement => Boolean(found)) ?? null;
  const parameters = parametersOf(form);
  // ADR-011 scenarios: the group's records cards re-read their listing too.
  for (const card of recordsCardsOf(group.items)) void refreshRecords(card, live);
  for (const card of ledgerCardsOf(group.items)) void refreshLedger(card, live);
  for (const item of group.items) {
    const card = item.querySelector<HTMLElement>('.cv-view');
    if (!card) continue;
    if (parameters === null) {
      renderNoValue(card, 'failed', 'failed', 'The window was not accepted. Give a start before the end, as UTC date and time. Nothing ran.');
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

/** Lifts a server-rendered (or island-built) group onto the canvas at row `y`. */
function adopt(section: HTMLElement, y: number, animate: boolean): Group | null {
  if (!canvas) return null;
  const app = section.dataset.app ?? '';
  const work = section.dataset.work ?? '';
  if (!isUuid(app) || !isUuid(work) || groups.has(work)) return null;
  // APP-098 demo shortcut (GENUI_DEMO_MODE): every load starts from the
  // default arrangement, so two windows of the pitch show the same canvas.
  const saved = demo() ? new Map<string, Partial<WidgetCanvasItemState>>() : loadLayout(app, work);
  const group: Group = { app, work, section, items: [], pinned: saved.size > 0 };
  groups.set(work, group);
  let x = ORIGIN;
  const cards = Array.from(section.querySelectorAll<HTMLElement>(':scope > .cv-card'));
  cards.forEach((card, index) => {
    const id = card.dataset.card ?? '';
    const kind = card.dataset.kind ?? '';
    const width = WIDTH[kind] ?? 440;
    const item = createCanvasItem({
      instanceId: id,
      displayName: card.getAttribute('aria-label') ?? 'Card',
      preferredWidth: width,
      minHeight: HEIGHT[kind] ?? 300,
      content: card,
    });
    item.classList.add('cv-item', `cv-item-${kind || 'view'}`);
    item.dataset.group = work;
    if (animate) {
      item.classList.add('is-placing');
      item.style.setProperty('--place-delay', `${index * 70}ms`);
    }
    const restored: Partial<WidgetCanvasItemState> = saved.get(id) ?? { x, y };
    canvas?.mountItem(item, restored, { focus: false });
    resized?.observe(item);
    group.items.push(item);
    x += width + GAP;
  });
  const window_ = section.querySelector<HTMLFormElement>('form[data-window]');
  window_?.addEventListener('submit', (event) => {
    event.preventDefault();
    runGroup(work);
  });
  for (const card of cards) {
    const form = card.querySelector<HTMLFormElement>('form.cv-command');
    if (form) wireCommand(form, group, card);
    if (card.dataset.kind === 'records') wireRecordsCard(card);
    if (card.dataset.kind === 'ledger') wireLedgerCard(card);
  }
  return group;
}

/** ADR-011 scenarios: re-reads a work's records cards (after a command this browser confirmed). */
export async function refreshGroupRecords(work: string, live = false): Promise<void> {
  const group = groups.get(work);
  if (!group) return;
  await Promise.all([
    ...recordsCardsOf(group.items).map((card) => refreshRecords(card, live)),
    ...ledgerCardsOf(group.items).map((card) => refreshLedger(card, live)),
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
      el(document_, 'p', { class: 'cv-message' }, 'Its views run on your organization’s data. Nothing is shown until each result is released with its receipt.')),
    chrome: false,
  });
  ghost.classList.add('cv-item', 'cv-item-ghost');
  canvas.mountItem(ghost, { x: ORIGIN, y }, { focus: false });
  canvas.centerItem(ghost);
  const group = await fetchGroup(app, work);
  canvas.removeItem(ghost, { selectFallback: false });
  if (!group) return 'The app could not be opened. Nothing was placed.';
  const started = performance.now();
  const section = buildGroup(document_, group, labels);
  if (nameable(labels, group.origins)) section.dataset.nameable = 'true';
  document_.getElementById('cv-board')?.append(section);
  const placed = adopt(section, y, true);
  if (!placed) return 'Nothing was placed.';
  decorate(placed);
  scheduleReflow();
  host.classList.add('has-groups');
  document_.getElementById('cv-empty')?.setAttribute('hidden', '');
  const elapsed = Math.round(performance.now() - started);
  host.dataset.lastPlaceMs = String(elapsed);
  runGroup(work);
  // Show the whole canvas with the new app arriving; keep fitting while its
  // results grow the cards.
  canvas.fitAll();
  fitPending = true;
  window.setTimeout(() => { fitPending = false; }, 3000);
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
  const old = groups.get(fromWork);
  if (!old || !canvas || !host || groups.has(toWork)) return false;
  const json = await fetchGroup(toApp, toWork);
  if (!json || !groups.has(fromWork)) return false;
  const document_ = host.ownerDocument;
  const y = Number(old.items[0]?.dataset.canvasY ?? nextRow());
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
  const pair = pairs.get(fromWork);
  if (pair && keepPair) pairs.set(toWork, pair);
  if (!keepPair) pairs.delete(fromWork);
  window.clearTimeout(tailoring.get(fromWork));
  tailoring.delete(fromWork);
  decorate(fresh);
  scheduleReflow();
  runGroup(toWork);
  updateSummary();
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
 * APP-101 utility cards (the `/apps` list, and the widgets other branches
 * register through `commands.ts`): mounted on the canvas at the next free
 * row and brought into view; an existing card of the same id is replaced.
 */
export function placeUtilityCard(id: string, title: string, body: HTMLElement): void {
  if (!canvas || !host) return;
  const document_ = host.ownerDocument;
  const instance = `utility-${id}`;
  const existing = document_.querySelector<HTMLElement>(`[data-utility="${instance}"]`);
  if (existing) canvas.removeItem(existing, { selectFallback: false });
  const card = el(document_, 'article', { class: 'cv-card cv-utility', 'data-card': instance, 'aria-label': title },
    el(document_, 'header', { class: 'cv-card-head' }, el(document_, 'h3', {}, title)),
    body);
  const item = createCanvasItem({ instanceId: instance, displayName: title, preferredWidth: 520, minHeight: 200, content: card });
  item.classList.add('cv-item', 'cv-item-utility', 'is-placing');
  item.dataset.utility = instance;
  canvas.mountItem(item, { x: ORIGIN, y: nextRow() }, { focus: false });
  canvas.centerItem(item);
  host.classList.add('has-groups');
  document_.getElementById('cv-empty')?.setAttribute('hidden', '');
}

function onLive(event: LiveEvent): void {
  if (event.kind === 'stream.lagged') {
    // Hints were dropped for this stream: re-run what is shown (APP-095 review).
    for (const work of groups.keys()) runGroup(work, true);
    return;
  }
  if (event.kind === 'record.committed') {
    if (groups.has(event.work_id)) runGroup(event.work_id, true);
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

function wireTools(document_: Document): void {
  const tools = document_.querySelector<HTMLElement>('[data-tools]');
  if (tools) tools.hidden = false;
  document_.querySelectorAll<HTMLButtonElement>('[data-tool]').forEach((button) => {
    button.addEventListener('click', () => {
      if (!canvas) return;
      switch (button.dataset.tool) {
        case 'zoom-in': canvas.zoomBy(1.2); break;
        case 'zoom-out': canvas.zoomBy(1 / 1.2); break;
        case 'zoom-reset': canvas.resetZoom(); break;
        case 'fit': canvas.fitAll(); break;
        case 'reset-layout':
          clearLayouts();
          for (const group of groups.values()) group.pinned = false;
          reflow();
          canvas.fitAll();
          break;
      }
      updateZoomLabel();
      saveSoon();
    });
  });
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
  const sections = Array.from(document_.querySelectorAll<HTMLElement>('#cv-board > .cv-group'));
  const flush = canvas.deferDerivedUpdates(false);
  let y = ORIGIN;
  for (const section of sections) {
    const group = adopt(section, y, false);
    if (!group) continue;
    host.classList.add('has-groups');
    y = Math.max(y, ...group.items.map((item) => Number(item.dataset.canvasY ?? 0) + item.offsetHeight)) + ROW_GAP;
  }
  flush();
  const camera = demo() ? null : loadCamera();
  const target = location.hash.match(/^#g-([0-9a-f-]{36})$/)?.[1];
  if (target && groups.get(target)?.items[0]) {
    canvas.focusItem(groups.get(target)!.items[0]!);
  } else if (camera) {
    canvas.restoreCamera(camera);
  } else if (groups.size) {
    canvas.fitAll();
    // Keep fitting while the first results grow the cards, until the member moves the camera.
    fitPending = true;
    window.setTimeout(() => { fitPending = false; }, 6000);
  }
  scheduleReflow();
  updateZoomLabel();
  wireTools(document_);
  // Prefetch: every view runs now, in parallel (two in flight), except the
  // ones the server already ran from this session's run cookie.
  for (const [work, group] of groups) {
    const cards = group.items
      .map((item) => item.querySelector<HTMLElement>('.cv-view'))
      .filter((card): card is HTMLElement => Boolean(card));
    const ran = cards.some((card) => card.dataset.state === 'ready' || card.dataset.state === 'empty');
    if (cards.length && !ran) runGroup(work);
  }
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
  announce(`${groups.size} ${groups.size === 1 ? 'app' : 'apps'} on the canvas.`);
}
