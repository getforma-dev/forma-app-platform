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
// - L opens the right pane on Library; APP-122: `?` puts `? ` in the bar,
//   ready to ask (Shortcuts stays on `/shortcuts` and ⋯ → Shortcuts);
// - APP-122: ⋯ → Ask about this (the selected card) and ⋯ → Shape… (a card,
//   an app card or frame, a widget: `/shape <address> ` in the bar), through
//   hooks the Ask and Shape modules set (`setTalkActions`);
// - APP-106: F focuses the selected card (`focus.ts`), U unites its app and
//   Shift + U breaks it apart (`unite.ts`); the toolbar menu has both;
// - a selected stack: ← / → leaf through it, Enter fans it out;
// - touch: one finger on the empty canvas pans (the vendor's), two fingers
//   pinch to zoom; Safari's trackpad gesture events zoom too.
//
// No shortcut fires while the member types in the intent bar, a card's form,
// or any input, textarea, select or contenteditable, nor while a modal (the
// file viewer, the data sheet, the agents panel, focus mode) is open.
import { eventOriginatesInInteractiveControl, eventOriginatesInTextEntry } from '../canvas-vendor/index';
import { canvasApi, itemKind, nudgeItems, refreshZoomLabel, removeCards, removeUtilityItem, resetLayout } from './board';
import { closeLibrary, isLibraryOpen, toggleLibrary } from './library';
import { announce } from './render';
import { registerCommands } from './command-registry';
import { openShortcuts } from './shortcut-tab';
import { removeWidgets } from './widgets/index';
import { collapseAll, dissolve, expand, isStackItem, shuffle, stackOfItem, stackSelection } from './stacks';
import { focusFromItem, isFocusOpen } from './focus';
import { isModalOpen } from './box-viewer';
import { isNoteItem, newNote, removeNoteItem } from './notes';
import { toggleTimings } from './timing';
import { frameCardsOfItem, isUniteItem, splitFromItem, uniteFromItem } from './unite';
import { primeAsk } from './address-chip';

/** APP-122: what ⋯ → Ask about this and ⋯ → Shape… do with the chosen item (set by `talk.ts`). */
type TalkActions = { ask: (item: HTMLElement | null) => boolean; shape: (item: HTMLElement | null) => boolean };
let talk: TalkActions | null = null;
export function setTalkActions(actions: TalkActions): void {
  talk = actions;
}

const selection = new Set<HTMLElement>();
let tool: 'move' | 'hand' = 'move';
let documentRef: Document | null = null;
const mac = typeof navigator !== 'undefined' && /Mac|iPhone|iPad/.test(navigator.platform || navigator.userAgent);

/**
 * APP-113: a modal other than focus mode (the file viewer, the data sheet,
 * the agents panel, any `<dialog open>` or `aria-modal` layer) owns the keyboard: no canvas shortcut fires behind it,
 * and none fires from inside one.
 */
export function modalOwnsKeys(event: Event, document_: Document): boolean {
  if (isModalOpen() || document_.querySelector('dialog[open], [data-modal-above]')) return true;
  for (const node of document_.querySelectorAll<HTMLElement>('[aria-modal="true"]')) {
    if (node.id === 'cv-focus' || node.hidden || node.closest('[hidden]')) continue;
    return true;
  }
  const target = event.target as Element | null;
  return Boolean(target?.closest?.('dialog[open], [aria-modal="true"]:not(#cv-focus)'));
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

/** The toolbar's two menus: zoom (`100% ▾`) and More (`⋯`, APP-112's click paths). */
const MENUS = { 'zoom-menu': 'cv-zoom-menu', 'more-menu': 'cv-more-menu' } as const;
type MenuTool = keyof typeof MENUS;
function toggleMenu(tool: MenuTool, open?: boolean): void {
  const menu = documentRef?.getElementById(MENUS[tool]);
  const button = documentRef?.querySelector<HTMLElement>(`[data-tool="${tool}"]`);
  if (!menu || !button) return;
  const show = open ?? menu.hidden;
  if (show) for (const other of Object.keys(MENUS) as MenuTool[]) if (other !== tool) toggleMenu(other, false);
  menu.hidden = !show;
  button.setAttribute('aria-expanded', String(show));
  if (show) menu.querySelector<HTMLElement>('button')?.focus({ preventScroll: true });
}
function openMenu(): boolean {
  return Object.values(MENUS).some((id) => documentRef?.getElementById(id)?.hidden === false);
}
function closeMenus(): void {
  for (const tool of Object.keys(MENUS) as MenuTool[]) toggleMenu(tool, false);
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
    if (isNoteItem(item)) removeNoteItem(item);
    // APP-113: a folder, brief or summary card is a widget item with its own
    // close (`data-widget-close`); the utility widgets go through `removeWidgets`.
    else if (kind === 'widget') {
      if (!removeWidgets(item)) item.querySelector<HTMLElement>('[data-widget-close]')?.click();
    }
    else if (kind === 'utility') removeUtilityItem(item);
    else if (kind === 'card' && item.dataset.instanceId) cards.push(item.dataset.instanceId);
    else if (isStackItem(item)) cards.push(...(stackOfItem(item)?.cards ?? []));
    else if (isUniteItem(item)) cards.push(...frameCardsOfItem(item));
  }
  if (cards.length) removeCards(cards);
  selection.clear();
  renderSelection();
}

/** The item F and U act on: the first selected one, else the active one. */
function chosenItem(): HTMLElement | null {
  return [...selection].find((item) => item.isConnected) ?? canvasApi()?.activeItem() ?? null;
}
/** APP-122: the selected item (Ask's scope): the first selected one, none when the selection is empty. */
export function selectedCanvasItem(): HTMLElement | null {
  return [...selection].find((item) => item.isConnected) ?? null;
}
function focusSelected(): void {
  if (!focusFromItem(chosenItem())) announce('Select a card first, then press F to focus it.');
}
function uniteSelected(split = false): void {
  const outcome = split ? splitFromItem(chosenItem()) : uniteFromItem(chosenItem());
  void Promise.resolve(outcome).then(announce);
}

/** The selected cards, or the active one: what the More menu's card actions act on. */
function chosenItems(): HTMLElement[] {
  const items = [...selection].filter((item) => item.isConnected);
  if (!items.length) {
    const active = canvasApi()?.activeItem();
    if (active) items.push(active);
  }
  return items;
}
function stackChosen(): void {
  if (!stackSelection(chosenItems())) announce('Select two or more cards first (Shift + click their headers), then choose Stack them.');
  selection.clear();
  renderSelection();
}
function spreadChosen(): void {
  const stacks = chosenItems().map((item) => stackOfItem(item)).filter((stack) => stack !== null);
  if (!stacks.length) announce('Select a stack first.');
  for (const stack of stacks) dissolve(stack);
}
function removeChosen(): void {
  const items = chosenItems();
  if (!items.length) {
    announce('Select a card first, or use the × on its header.');
    return;
  }
  for (const item of items) selection.add(item);
  removeSelected();
  announce(`${items.length === 1 ? 'The card is' : `${items.length} cards are`} back in the library. Nothing was deleted.`);
}

function action(name: string): void {
  const canvas = canvasApi();
  if (!canvas) return;
  switch (name) {
    case 'library': toggleLibrary(); break;
    case 'new-note': void newNote('').then(announce); break;
    case 'timings': toggleTimings(); break;
    case 'split': uniteSelected(true); break;
    case 'stack': stackChosen(); break;
    case 'spread': spreadChosen(); break;
    case 'remove': removeChosen(); break;
    case 'zoom-in': canvas.zoomBy(1.25); break;
    case 'zoom-out': canvas.zoomBy(1 / 1.25); break;
    case 'zoom-reset': canvas.resetZoom(); break;
    case 'fit': canvas.fitAll(); break;
    case 'selection': zoomToSelection(); break;
    case 'reset-layout': resetLayout(); break;
    case 'help': openShortcuts(); break;
    case 'ask-about': if (!talk?.ask(chosenItem())) announce('Select a card first, then choose Ask about this.'); break;
    case 'shape': if (!talk?.shape(chosenItem())) announce('Select a card, an app or a widget first, then choose Shape….'); break;
    case 'focus': focusSelected(); break;
    case 'unite': uniteSelected(); break;
    case 'move': setTool('move'); break;
    case 'hand': setTool('hand'); break;
    default: return;
  }
  refreshZoomLabel();
}

/** The canvas keys (capture phase on the document; exported for the tests). */
export function onKey(event: KeyboardEvent): void {
  if (event.defaultPrevented || event.isComposing || !documentRef) return;
  // Focus mode owns the keyboard while it is open (`focus.ts`), and so does
  // any other modal (APP-113: Delete behind the file viewer removed cards).
  if (isFocusOpen() || modalOwnsKeys(event, documentRef)) return;
  const typing = eventOriginatesInTextEntry(event);
  const mod = event.metaKey || event.ctrlKey;
  // The browser's page zoom keys zoom the canvas instead (outside text fields).
  // Cmd/Ctrl+G stacks the selection; with Shift, spreads the selected stacks.
  if (mod && !event.altKey && event.code === 'KeyG' && !typing) {
    event.preventDefault();
    const chosen = [...selection].filter((item) => item.isConnected);
    if (event.shiftKey) {
      for (const item of chosen) {
        const stack = stackOfItem(item);
        if (stack) dissolve(stack);
      }
    } else if (!stackSelection(chosen)) {
      announce('Select two or more cards (Shift + click), then press G with Cmd or Ctrl to stack them.');
    }
    selection.clear();
    renderSelection();
    return;
  }
  if (mod && !event.altKey && ['=', '+', '-', '_'].includes(event.key)) {
    if (typing) return;
    event.preventDefault();
    action(event.key === '-' || event.key === '_' ? 'zoom-out' : 'zoom-in');
    return;
  }
  if (event.key === 'Escape') {
    if (!typing && collapseAll()) {
      event.preventDefault();
      return;
    }
    if (openMenu()) {
      event.preventDefault();
      closeMenus();
      return;
    }
    if (typing) return;
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
  if (typing || mod || event.altKey) return;
  // Shift + digit: the browser reports `)`, `!`, `@` on US layouts; read the code.
  if (event.shiftKey && (event.code === 'Digit0' || event.code === 'Digit1' || event.code === 'Digit2')) {
    event.preventDefault();
    action(event.code === 'Digit0' ? 'zoom-reset' : event.code === 'Digit1' ? 'fit' : 'selection');
    return;
  }
  // APP-122: `?` puts `? ` in an empty bar, ready to ask (`ask.ts`); half-typed text is only focused.
  if (event.key === '?') {
    event.preventDefault();
    primeAsk(documentRef ?? document);
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
  // APP-106: F focus, U unite, Shift + U break apart.
  if (key === 'f' && !event.shiftKey) {
    event.preventDefault();
    focusSelected();
    return;
  }
  if (key === 'u') {
    event.preventDefault();
    uniteSelected(event.shiftKey);
    return;
  }
  const selected = [...selection].filter((item) => item.isConnected);
  if (!selected.length) {
    const active = canvasApi()?.activeItem();
    if (active && (event.key === 'Delete' || event.key === 'Backspace')) selected.push(active);
  }
  if (!selected.length) return;
  // A selected stack: ←/→ leaf through it, Enter or Space fans it out.
  const stack = selected.length === 1 ? stackOfItem(selected[0]!) : null;
  if (stack && (event.key === 'ArrowLeft' || event.key === 'ArrowRight') && !event.shiftKey) {
    event.preventDefault();
    event.stopImmediatePropagation();
    shuffle(stack, event.key === 'ArrowRight' ? 1 : -1);
    return;
  }
  // Enter fans a selected stack out, unless a control has the key (a button
  // or link keeps its own activation). Space stays Space + drag panning.
  if (stack && event.key === 'Enter' && !eventOriginatesInInteractiveControl(event)) {
    event.preventDefault();
    event.stopImmediatePropagation();
    expand(stack);
    return;
  }
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
  registerCommands([{
    slug: 'arrange',
    title: 'Arrange by app',
    purpose: 'Lay the canvas out again, one row per app (view layout only)',
    run: (_text: string, report?: (message: string) => void) => {
      resetLayout();
      const input = document_.getElementById('intent-input') as HTMLInputElement | null;
      if (input) input.value = '';
      report?.('Arranged by app: one row per app.');
    },
  }, {
    slug: 'shortcuts',
    aliases: ['keys'],
    title: 'Shortcuts',
    purpose: 'Every action with its click path and its keys (the Shortcuts tab, or ?)',
    run: (_text: string, report?: (message: string) => void) => {
      const input = document_.getElementById('intent-input') as HTMLInputElement | null;
      if (input) input.value = '';
      openShortcuts(input);
      report?.('Shortcuts are in the Shortcuts tab. Esc closes it.');
    },
  }]);
  // A page-zoom pinch over the dock or the pane zooms nothing.
  document_.addEventListener('wheel', (event) => {
    if (event.ctrlKey && !host.contains(event.target as Node)) event.preventDefault();
  }, { passive: false });
  document_.addEventListener('click', (event) => {
    const target = event.target as Element | null;
    const button = target?.closest<HTMLElement>('[data-tool]');
    if (button) {
      const tool = button.dataset.tool ?? '';
      if (tool in MENUS) {
        toggleMenu(tool as MenuTool);
        return;
      }
      action(tool);
      if (button.closest('.cv-zoom-menu')) closeMenus();
      return;
    }
    if (openMenu() && !target?.closest('.cv-zoom')) closeMenus();
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
