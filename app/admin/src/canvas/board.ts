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

type Group = {
  app: string;
  work: string;
  section: HTMLElement;
  items: HTMLElement[];
  /** The member arranged this group (now or in an earlier visit): its layout is kept. */
  pinned: boolean;
};

const WIDTH: Record<string, number> = { app: 340, summary: 400, timeline: 620, form: 360 };
const HEIGHT: Record<string, number> = { app: 300, summary: 250, timeline: 340, form: 300 };
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
  }
  return group;
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
export async function placeWork(app: string, work: string, labels: string[] = []): Promise<string> {
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
  document_.getElementById('cv-board')?.append(section);
  const placed = adopt(section, y, true);
  if (!placed) return 'Nothing was placed.';
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
  return `${group.title} is on your canvas.`;
}

function onLive(event: LiveEvent): void {
  if (event.kind === 'record.committed') {
    if (groups.has(event.work_id)) runGroup(event.work_id, true);
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
