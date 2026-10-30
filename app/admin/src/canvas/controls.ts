// APP-104: familiar canvas controls (Victor: "copy the controls that other
// canvas apps already have, like Figma"). The camera and drag engine is the
// vendored canvas (`../canvas-vendor`, local changes listed in its README):
// a plain wheel or two-finger scroll pans, Ctrl/Cmd + wheel and a trackpad
// pinch zoom toward the cursor, Space + drag and middle-drag pan, a card
// moves by its header. This module adds the rest, pitch-local:
//
// - selection: click a card to select it, Shift + click to add or remove,
//   Esc to clear; Delete/Backspace takes the selected cards off the canvas
//   (back to the library pane; no data changes); arrows nudge them 1 px,
//   Shift + arrows 10 px (canvas units);
// - tools: V move, H hand (a drag anywhere pans), shown in the bottom
//   toolbar with the zoom menu (zoom in/out, 100%, fit, selection);
// - zoom keys: Cmd/Ctrl + `=`/`+` and `-` (the browser's page zoom is held
//   back), Shift + 0 (100%), Shift + 1 (fit all), Shift + 2 (selection);
// - L opens the library, `?` this sheet of shortcuts;
// - touch: one finger on the empty canvas pans (the vendor's), two fingers
//   pinch to zoom; Safari's trackpad gesture events zoom too.
//
// No shortcut fires while the member types in the intent bar, a card's form,
// or any input, textarea, select or contenteditable.
import { canvasApi, itemKind, nudgeItems, refreshZoomLabel, removeCards, removeUtilityItem, resetLayout } from './board';
import { closeLibrary, isLibraryOpen, toggleLibrary } from './library';
import { el } from './render';
import { removeWidgets } from './widgets/index';

const selection = new Set<HTMLElement>();
let tool: 'move' | 'hand' = 'move';
let documentRef: Document | null = null;
let sheet: HTMLElement | null = null;
const mac = typeof navigator !== 'undefined' && /Mac|iPhone|iPad/.test(navigator.platform || navigator.userAgent);
const MOD = mac ? '⌘' : 'Ctrl';

function editable(target: EventTarget | null): boolean {
  if (!(target instanceof Element)) return false;
  return Boolean(target.closest('input, textarea, select, [contenteditable=""], [contenteditable="true"]'));
}

function announce(message: string): void {
  const status = documentRef?.getElementById('cv-activity');
  if (status) status.textContent = message;
}

function renderSelection(): void {
  for (const item of documentRef?.querySelectorAll<HTMLElement>('.widget-instance.is-selected') ?? []) {
    if (!selection.has(item)) item.classList.remove('is-selected');
  }
  for (const item of [...selection]) {
    if (!item.isConnected) selection.delete(item);
    else item.classList.add('is-selected');
  }
}

function clearSelection(): void {
  selection.clear();
  canvasApi()?.clearActive();
  renderSelection();
}

function setTool(next: 'move' | 'hand'): void {
  tool = next;
  canvasApi()?.setPanMode(next === 'hand');
  for (const button of documentRef?.querySelectorAll<HTMLElement>('[data-tool="move"], [data-tool="hand"]') ?? []) {
    button.setAttribute('aria-pressed', String(button.dataset.tool === next));
  }
  announce(next === 'hand' ? 'Hand tool: drag anywhere to pan.' : 'Move tool.');
}

function zoomMenu(open?: boolean): void {
  const menu = documentRef?.getElementById('cv-zoom-menu');
  const button = documentRef?.querySelector<HTMLElement>('[data-tool="zoom-menu"]');
  if (!menu || !button) return;
  const show = open ?? menu.hidden;
  menu.hidden = !show;
  button.setAttribute('aria-expanded', String(show));
  if (show) menu.querySelector<HTMLElement>('button')?.focus({ preventScroll: true });
}

function zoomToSelection(): void {
  const canvas = canvasApi();
  if (!canvas) return;
  const target = [...selection].find((item) => item.isConnected) ?? canvas.activeItem();
  if (target) canvas.focusItem(target);
  else announce('Select a card first.');
}

function removeSelected(): void {
  const items = [...selection].filter((item) => item.isConnected);
  if (!items.length) return;
  const cards: string[] = [];
  for (const item of items) {
    const kind = itemKind(item);
    if (kind === 'widget') removeWidgets(item);
    else if (kind === 'utility') removeUtilityItem(item);
    else if (kind === 'card' && item.dataset.instanceId) cards.push(item.dataset.instanceId);
  }
  if (cards.length) removeCards(cards);
  selection.clear();
  renderSelection();
}

function action(name: string): void {
  const canvas = canvasApi();
  if (!canvas) return;
  switch (name) {
    case 'zoom-in': canvas.zoomBy(1.25); break;
    case 'zoom-out': canvas.zoomBy(1 / 1.25); break;
    case 'zoom-reset': canvas.resetZoom(); break;
    case 'fit': canvas.fitAll(); break;
    case 'selection': zoomToSelection(); break;
    case 'reset-layout': resetLayout(); break;
    case 'help': showSheet(); break;
    case 'move': setTool('move'); break;
    case 'hand': setTool('hand'); break;
    default: return;
  }
  refreshZoomLabel();
}

const SHORTCUTS: [string, [string, string][]][] = [
  ['Move around', [
    ['Scroll, two-finger swipe', 'Pan'],
    ['Shift + scroll', 'Pan sideways'],
    ['Space + drag, middle-drag', 'Pan'],
    [`${MOD} + scroll, pinch`, 'Zoom at the cursor'],
    [`${MOD} + =  ·  ${MOD} + −`, 'Zoom in · out'],
    ['Shift + 0', 'Zoom to 100%'],
    ['Shift + 1', 'Zoom to fit'],
    ['Shift + 2', 'Zoom to selection'],
  ]],
  ['Tools', [
    ['V', 'Move: select and drag cards'],
    ['H', 'Hand: drag anywhere to pan'],
  ]],
  ['Cards', [
    ['Click a header', 'Select'],
    ['Shift + click', 'Add to the selection'],
    ['Drag a header', 'Move'],
    ['Arrow keys · Shift', 'Nudge 1 px · 10 px'],
    ['Delete', 'Back to the library (no data changes)'],
    ['Esc', 'Clear the selection'],
  ]],
  ['Canvas', [
    ['L', 'Library'],
    ['/', 'Commands in the bar: /card 3.2, /app expense'],
    ['?', 'This sheet'],
  ]],
];

function showSheet(): void {
  if (!documentRef) return;
  if (!sheet) {
    const document_ = documentRef;
    const close = el(document_, 'button', { type: 'button', class: 'cv-lib-close', 'aria-label': 'Close the shortcuts', title: 'Close (Esc)' }, '×');
    sheet = el(document_, 'div', { id: 'cv-shortcuts', class: 'cv-shortcuts', role: 'dialog', 'aria-modal': 'true', 'aria-labelledby': 'cv-shortcuts-title', hidden: '' },
      el(document_, 'div', { class: 'cv-shortcuts-panel' },
        el(document_, 'header', { class: 'cv-shortcuts-head' }, el(document_, 'h2', { id: 'cv-shortcuts-title' }, 'Keyboard shortcuts'), close),
        el(document_, 'div', { class: 'cv-shortcuts-grid' },
          ...SHORTCUTS.map(([title, rows]) => el(document_, 'section', {},
            el(document_, 'h3', {}, title),
            el(document_, 'dl', {}, ...rows.flatMap(([keys, what]) => [
              el(document_, 'dt', {}, what),
              el(document_, 'dd', {}, ...keys.split(/(\s·\s|\s\+\s|,\s)/).map((part) => (/^(\s·\s|\s\+\s|,\s)$/.test(part)
                ? part
                : el(document_, 'kbd', {}, part)))),
            ])))))));
    close.addEventListener('click', hideSheet);
    sheet.addEventListener('click', (event) => {
      if (event.target === sheet) hideSheet();
    });
    (document_.getElementById('cv-shell') ?? document_.body).append(sheet);
  }
  sheet.hidden = false;
  sheet.querySelector<HTMLElement>('button')?.focus({ preventScroll: true });
}
function hideSheet(): void {
  if (sheet) sheet.hidden = true;
}

function onKey(event: KeyboardEvent): void {
  if (event.defaultPrevented || event.isComposing) return;
  const mod = event.metaKey || event.ctrlKey;
  // The browser's page zoom keys zoom the canvas instead (outside text fields).
  if (mod && !event.altKey && ['=', '+', '-', '_'].includes(event.key)) {
    if (editable(event.target)) return;
    event.preventDefault();
    action(event.key === '-' || event.key === '_' ? 'zoom-out' : 'zoom-in');
    return;
  }
  if (event.key === 'Escape') {
    if (sheet && !sheet.hidden) {
      event.preventDefault();
      hideSheet();
      return;
    }
    const menu = documentRef?.getElementById('cv-zoom-menu');
    if (menu && !menu.hidden) {
      event.preventDefault();
      zoomMenu(false);
      return;
    }
    if (editable(event.target)) return;
    if (isLibraryOpen()) {
      event.preventDefault();
      closeLibrary();
      return;
    }
    if (selection.size || canvasApi()?.activeItem()) {
      clearSelection();
      return;
    }
    return;
  }
  if (editable(event.target) || mod || event.altKey) return;
  if (sheet && !sheet.hidden) return;
  // Shift + digit: the browser reports `)`, `!`, `@` on US layouts; read the code.
  if (event.shiftKey && (event.code === 'Digit0' || event.code === 'Digit1' || event.code === 'Digit2')) {
    event.preventDefault();
    action(event.code === 'Digit0' ? 'zoom-reset' : event.code === 'Digit1' ? 'fit' : 'selection');
    return;
  }
  if (event.key === '?') {
    event.preventDefault();
    showSheet();
    return;
  }
  const key = event.key.toLowerCase();
  if (!event.shiftKey && (key === 'v' || key === 'h' || key === 'l')) {
    event.preventDefault();
    if (key === 'l') toggleLibrary();
    else setTool(key === 'h' ? 'hand' : 'move');
    return;
  }
  // Inside the library pane and the toolbar, keys belong to their controls.
  if ((event.target as Element | null)?.closest?.('#cv-library, .cv-tools, .cv-dock')) return;
  const selected = [...selection].filter((item) => item.isConnected);
  if (!selected.length) {
    const active = canvasApi()?.activeItem();
    if (active && (event.key === 'Delete' || event.key === 'Backspace')) selected.push(active);
  }
  if (!selected.length) return;
  if (event.key === 'Delete' || event.key === 'Backspace') {
    event.preventDefault();
    event.stopImmediatePropagation();
    for (const item of selected) selection.add(item);
    removeSelected();
    return;
  }
  const step = event.shiftKey ? 10 : 1;
  const move: Record<string, [number, number]> = {
    ArrowLeft: [-step, 0], ArrowRight: [step, 0], ArrowUp: [0, -step], ArrowDown: [0, step],
  };
  const delta = move[event.key];
  if (delta) {
    event.preventDefault();
    event.stopImmediatePropagation();
    nudgeItems(selected, delta[0], delta[1]);
  }
}

/** Two-finger pinch on touch screens (the vendor pans with one finger). */
function wirePinch(viewport: HTMLElement): void {
  const touches = new Map<number, { x: number; y: number }>();
  let last: { distance: number; x: number; y: number } | null = null;
  const measure = () => {
    const [a, b] = [...touches.values()];
    if (!a || !b) return null;
    return { distance: Math.hypot(a.x - b.x, a.y - b.y), x: (a.x + b.x) / 2, y: (a.y + b.y) / 2 };
  };
  viewport.addEventListener('pointerdown', (event) => {
    if (event.pointerType !== 'touch') return;
    touches.set(event.pointerId, { x: event.clientX, y: event.clientY });
    if (touches.size === 2) {
      canvasApi()?.cancelGestures();
      last = measure();
    }
  }, true);
  viewport.addEventListener('pointermove', (event) => {
    if (!touches.has(event.pointerId)) return;
    touches.set(event.pointerId, { x: event.clientX, y: event.clientY });
    const now = touches.size === 2 ? measure() : null;
    const canvas = canvasApi();
    if (!now || !last || !canvas || last.distance <= 0) return;
    event.preventDefault();
    canvas.panBy(now.x - last.x, now.y - last.y);
    canvas.zoomAt(now.distance / last.distance, now.x, now.y);
    refreshZoomLabel();
    last = now;
  }, true);
  const end = (event: PointerEvent) => {
    touches.delete(event.pointerId);
    if (touches.size < 2) last = null;
  };
  viewport.addEventListener('pointerup', end, true);
  viewport.addEventListener('pointercancel', end, true);
  // Safari's trackpad pinch arrives as gesture events, not wheel + ctrlKey.
  let scale = 1;
  type Gesture = Event & { scale: number; clientX: number; clientY: number };
  viewport.addEventListener('gesturestart', (event) => {
    event.preventDefault();
    scale = (event as Gesture).scale || 1;
  });
  viewport.addEventListener('gesturechange', (event) => {
    event.preventDefault();
    const gesture = event as Gesture;
    canvasApi()?.zoomAt(gesture.scale / scale, gesture.clientX, gesture.clientY);
    scale = gesture.scale;
    refreshZoomLabel();
  });
}

/** Wires the controls once (after the board started). */
export function startControls(document_: Document): void {
  if (documentRef) return;
  documentRef = document_;
  const host = document_.getElementById('cv-canvas');
  if (!host || !canvasApi()) return;
  if (!mac) {
    for (const node of document_.querySelectorAll<HTMLElement>('.cv-kbd-group')) node.textContent = (node.textContent ?? '').replace('⌘', 'Ctrl');
  }
  document_.addEventListener('keydown', onKey, true);
  // A page-zoom pinch over the dock or the pane zooms nothing.
  document_.addEventListener('wheel', (event) => {
    if (event.ctrlKey && !host.contains(event.target as Node)) event.preventDefault();
  }, { passive: false });
  document_.addEventListener('click', (event) => {
    const target = event.target as Element | null;
    const button = target?.closest<HTMLElement>('[data-tool]');
    if (button) {
      if (button.dataset.tool === 'zoom-menu') {
        zoomMenu();
        return;
      }
      action(button.dataset.tool ?? '');
      if (button.closest('#cv-zoom-menu')) zoomMenu(false);
      return;
    }
    const menu = document_.getElementById('cv-zoom-menu');
    if (menu && !menu.hidden && !target?.closest('.cv-zoom')) zoomMenu(false);
  });
  // Selection: a press on a card selects it (Shift adds or removes); a
  // press on the empty canvas clears it.
  host.addEventListener('pointerdown', (event) => {
    if (event.button !== 0 || tool === 'hand') return;
    const target = event.target as Element | null;
    const item = target?.closest<HTMLElement>('.widget-instance');
    if (!item) {
      if (!event.shiftKey && target?.closest('.forma-canvas-viewport') && !target.closest('.forma-canvas-navigator')) {
        selection.clear();
        renderSelection();
      }
      return;
    }
    if (event.shiftKey) {
      if (selection.has(item)) selection.delete(item);
      else selection.add(item);
    } else if (!selection.has(item)) {
      selection.clear();
      selection.add(item);
    }
    renderSelection();
  }, true);
  const viewport = host.querySelector<HTMLElement>('.forma-canvas-viewport') ?? host;
  wirePinch(viewport);
}
