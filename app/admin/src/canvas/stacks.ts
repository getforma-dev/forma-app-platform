// APP-104 stacks (Victor: "add the ability to stack cards and shuffle
// through them", after macOS Stacks, BumpTop's piles and Fences).
//
// - Make: drag a card onto another card or stack and hold it there ~400 ms
//   (the target lights up), then drop; or select cards and press Cmd/Ctrl+G;
//   or "Stack" on an app in the library pane.
// - A stack is one canvas item: the top card in full, two or three offset
//   edges behind it (plain elements, no content), a title (the first card's
//   app; double-click to rename), "+N" and the position ("2/5").
// - Shuffle: horizontal scroll or Shift + scroll over it, ←/→ when it is
//   selected, a swipe on touch: the top card leafs to the next one.
// - Expand: the "+N" badge, Enter on the selected stack, or a
//   double-click fans its cards out in a grid next to it; Esc or a click
//   outside folds them back; a card dragged out of the fan leaves the stack.
//   Cmd/Ctrl+Shift+G spreads a stack onto the canvas.
// - Live: a record committed in a stacked card's work pulses the stack; a
//   records card with a record waiting for the member comes to the top.
//
// Only the top card's element sits in the stack; the others stay in the
// hidden board (their views run when they reach the top). Stacks are kept
// per member in this browser (DS-67), card ids and a text title only.
import { createCanvasItem } from '../canvas-vendor/index';
import { FAN_ROW_HEIGHT, GAP, SLOT_WIDTH as WIDTH, setStackHooks, stackBoard as board } from './board';
import { loadStacks, saveStacks, type StoredStack } from './layout';
import { el } from './render';
import { refreshRecords } from './records';

type Stack = {
  id: string;
  title: string;
  cards: string[];
  top: number;
  item: HTMLElement;
  root: HTMLElement;
  holder: HTMLElement;
  fanned: HTMLElement[] | null;
};

const stacks = new Map<string, Stack>();
const HOLD_MS = 400;
let documentRef: Document | null = null;

function newId(): string {
  const random = new Uint32Array(2);
  crypto.getRandomValues(random);
  return Array.from(random, (n) => n.toString(36)).join('').slice(0, 16);
}

function stackOf(item: Element | null): Stack | null {
  const id = item?.closest<HTMLElement>('.cv-item-stack')?.dataset.stack;
  return id ? stacks.get(id) ?? null : null;
}

export function isStackItem(item: HTMLElement): boolean {
  return item.classList.contains('cv-item-stack');
}

function save(): void {
  const canvas = board.canvas();
  if (!canvas) return;
  const rows: StoredStack[] = [];
  for (const stack of stacks.values()) {
    const state = canvas.getItemState(stack.item);
    rows.push({ id: stack.id, title: stack.title, cards: stack.cards, top: stack.top, x: state.x, y: state.y });
  }
  saveStacks(board.member(), rows);
}

function render(stack: Stack): void {
  const count = stack.cards.length;
  stack.root.dataset.depth = String(Math.min(3, count - 1));
  const set = (selector: string, text: string) => {
    const node = stack.root.querySelector(selector);
    if (node) node.textContent = text;
  };
  set('[data-stack-title]', stack.title);
  set('[data-stack-count]', `+${count - 1}`);
  set('[data-stack-pos]', `${stack.top + 1}/${count}`);
  stack.root.querySelector('[data-stack-count]')?.setAttribute('aria-label', `Fan out all ${count} cards`);
  const dots = stack.root.querySelector('[data-stack-dots]');
  if (dots) {
    dots.replaceChildren(...stack.cards.slice(0, 12).map((_, index) => el(stack.root.ownerDocument, 'i', { class: index === stack.top ? 'is-top' : '' })));
  }
  stack.item.setAttribute('aria-label', `${stack.title}: stack of ${count} cards, showing ${stack.top + 1}`);
}

/** Shows the card at `stack.top` (`direction` animates a leaf forward or back). */
function showTop(stack: Stack, direction = 0): void {
  const id = stack.cards[stack.top];
  const card = id ? board.show(id) : null;
  const current = stack.holder.firstElementChild as HTMLElement | null;
  if (current && current !== card) board.park(current);
  if (card && card !== current) stack.holder.replaceChildren(card);
  if (direction) {
    stack.root.classList.remove('is-leaf-next', 'is-leaf-prev');
    void stack.root.offsetWidth;
    stack.root.classList.add(direction > 0 ? 'is-leaf-next' : 'is-leaf-prev');
  }
  render(stack);
}

export function shuffle(stack: Stack, step: number): void {
  if (stack.fanned || stack.cards.length < 2) return;
  stack.top = (stack.top + step + stack.cards.length) % stack.cards.length;
  showTop(stack, step);
  save();
}

function build(document_: Document, stack: Omit<Stack, 'item' | 'root' | 'holder' | 'fanned'>): { root: HTMLElement; holder: HTMLElement } {
  const holder = el(document_, 'div', { class: 'cv-stack-top' });
  const title = el(document_, 'span', { class: 'cv-stack-title', 'data-stack-title': '', title: 'Double-click to rename' }, stack.title);
  const root = el(document_, 'article', { class: 'cv-stack', 'data-stack': stack.id, 'data-kind': 'stack' },
    el(document_, 'span', { class: 'cv-stack-edge', 'data-edge': '3', 'aria-hidden': 'true' }),
    el(document_, 'span', { class: 'cv-stack-edge', 'data-edge': '2', 'aria-hidden': 'true' }),
    el(document_, 'span', { class: 'cv-stack-edge', 'data-edge': '1', 'aria-hidden': 'true' }),
    el(document_, 'header', { class: 'cv-stack-head', 'data-canvas-drag-handle': '' },
      title,
      el(document_, 'span', { class: 'cv-stack-pos', 'data-stack-pos': '' }),
      el(document_, 'span', { class: 'cv-stack-dots', 'data-stack-dots': '', 'aria-hidden': 'true' }),
      el(document_, 'button', { type: 'button', class: 'cv-stack-count', 'data-stack-count': '', title: 'Fan out (Enter)' })),
    holder);
  return { root, holder };
}

/**
 * Stacks the cards `ids` (top first) at `at` (or where the first one is).
 * Every id must be a card on the board or in a stack.
 */
export function createStack(ids: string[], options: { title?: string; at?: { x: number; y: number }; id?: string; top?: number; quiet?: boolean } = {}): Stack | null {
  const canvas = board.canvas();
  if (!canvas || !documentRef) return null;
  const cards: string[] = [];
  for (const id of ids) {
    if (cards.includes(id)) continue;
    const inStack = [...stacks.values()].find((stack) => stack.cards.includes(id));
    if (inStack) {
      inStack.cards = inStack.cards.filter((card) => card !== id);
      // APP-113: a stack left with one card spreads it (no "+0" stack).
      if (inStack.cards.length < 2) dissolve(inStack);
      else {
        inStack.top = Math.min(inStack.top, inStack.cards.length - 1);
        showTop(inStack);
      }
    }
    if (board.findCard(id)) cards.push(id);
  }
  if (cards.length < 2) return null;
  const firstItem = board.findCard(cards[0]!)?.item;
  const at = options.at ?? (firstItem ? canvas.getItemState(firstItem) : { x: 64, y: 64 });
  for (const id of cards) board.lift(id);
  const id = options.id ?? newId();
  const title = options.title || board.title(cards[0]!);
  const { root, holder } = build(documentRef, { id, title, cards, top: 0 });
  const item = createCanvasItem({ instanceId: `stack-${id}`, displayName: title, preferredWidth: WIDTH, minHeight: 300, content: root });
  item.classList.add('cv-item', 'cv-item-stack');
  if (!options.quiet) item.classList.add('is-placing');
  item.dataset.stack = id;
  canvas.mountItem(item, { x: at.x, y: at.y }, { focus: false });
  const stack: Stack = { id, title, cards, top: Math.min(options.top ?? 0, cards.length - 1), item, root, holder, fanned: null };
  stacks.set(id, stack);
  wire(stack);
  showTop(stack);
  board.pinAll();
  board.changed();
  save();
  if (!options.quiet) board.announce(`Stacked ${cards.length} cards: ${title}.`);
  return stack;
}

/** Adds cards to a stack (on top unless `bottom`). */
function addTo(stack: Stack, ids: string[], bottom = false): void {
  const fresh = ids.filter((id) => !stack.cards.includes(id) && board.findCard(id));
  for (const id of fresh) {
    const other = [...stacks.values()].find((candidate) => candidate !== stack && candidate.cards.includes(id));
    if (other) {
      other.cards = other.cards.filter((card) => card !== id);
      if (other.cards.length < 2) dissolve(other);
      else showTop(other);
    }
    board.lift(id);
  }
  if (!fresh.length) return;
  stack.cards = bottom ? [...stack.cards, ...fresh] : [...fresh, ...stack.cards];
  stack.top = bottom ? stack.top : 0;
  showTop(stack, bottom ? 0 : -1);
  board.changed();
  save();
  board.announce(`${fresh.length === 1 ? 'A card' : `${fresh.length} cards`} added to ${stack.title}.`);
}

function removeStack(stack: Stack, _why: string): void {
  const canvas = board.canvas();
  const current = stack.holder.firstElementChild as HTMLElement | null;
  if (current) board.park(current);
  canvas?.removeItem(stack.item, { selectFallback: false });
  stacks.delete(stack.id);
}

/** Spreads a stack's cards onto the canvas (Cmd/Ctrl+Shift+G). */
export function dissolve(stack: Stack): void {
  const canvas = board.canvas();
  if (!canvas) return;
  collapse(stack);
  const at = canvas.getItemState(stack.item);
  const cards = [...stack.cards];
  removeStack(stack, 'unstack');
  cards.forEach((id, index) => board.drop(id, { x: at.x + index * (WIDTH + GAP), y: at.y }));
  board.changed();
  save();
  board.announce(`${cards.length} ${cards.length === 1 ? 'card' : 'cards'} spread onto the canvas.`);
}

/** Fans a stack's cards out in a grid beside it (macOS Stacks' grid). */
export function expand(stack: Stack): void {
  const canvas = board.canvas();
  if (!canvas || stack.fanned) return;
  for (const other of stacks.values()) if (other !== stack) collapse(other);
  const at = canvas.getItemState(stack.item);
  const current = stack.holder.firstElementChild as HTMLElement | null;
  if (current) board.park(current);
  const columns = Math.min(4, Math.max(2, Math.ceil(Math.sqrt(stack.cards.length))));
  stack.fanned = [];
  stack.cards.forEach((id, index) => {
    const x = at.x + WIDTH + GAP * 2 + (index % columns) * (WIDTH + GAP);
    const y = at.y + Math.floor(index / columns) * FAN_ROW_HEIGHT;
    const item = board.drop(id, { x, y }, true);
    if (item) {
      item.classList.add('is-fanned');
      item.style.setProperty('--fan-delay', `${index * 35}ms`);
      stack.fanned?.push(item);
    }
  });
  stack.root.classList.add('is-expanded');
  board.changed();
  const first = stack.fanned[0];
  if (first) canvas.centerItem(first);
  board.announce(`${stack.title}: ${stack.cards.length} cards fanned out. Esc folds them back; drag one out to take it from the stack.`);
}

/** Folds a fanned stack back. */
export function collapse(stack: Stack): void {
  if (!stack.fanned) return;
  for (const item of stack.fanned) {
    if (item.dataset.fanned && item.isConnected) board.lift(item.dataset.instanceId ?? '');
  }
  stack.fanned = null;
  stack.root.classList.remove('is-expanded');
  if (stack.cards.length < 2) {
    dissolve(stack);
    return;
  }
  stack.top = Math.min(stack.top, stack.cards.length - 1);
  showTop(stack);
  board.changed();
  save();
}

export function collapseAll(): boolean {
  let any = false;
  for (const stack of stacks.values()) {
    if (stack.fanned) {
      collapse(stack);
      any = true;
    }
  }
  return any;
}

function rename(stack: Stack): void {
  const document_ = stack.root.ownerDocument;
  const title = stack.root.querySelector<HTMLElement>('[data-stack-title]');
  if (!title) return;
  const input = el(document_, 'input', { type: 'text', class: 'cv-stack-rename', value: stack.title, maxlength: '80', 'aria-label': 'Stack name' }) as HTMLInputElement;
  title.replaceWith(input);
  input.focus();
  input.select();
  const done = (keep: boolean) => {
    const next = input.value.trim();
    if (keep && next) stack.title = next.slice(0, 80);
    input.replaceWith(title);
    render(stack);
    save();
  };
  input.addEventListener('keydown', (event) => {
    event.stopPropagation();
    if (event.key === 'Enter') done(true);
    if (event.key === 'Escape') done(false);
  });
  input.addEventListener('blur', () => { if (input.isConnected) done(true); }, { once: true });
  input.addEventListener('pointerdown', (event) => event.stopPropagation());
}

function wire(stack: Stack): void {
  stack.root.querySelector('[data-stack-count]')?.addEventListener('click', (event) => {
    event.stopPropagation();
    if (stack.fanned) collapse(stack);
    else expand(stack);
  });
  stack.root.addEventListener('dblclick', (event) => {
    const target = event.target as Element;
    if (target.closest('[data-stack-title]')) {
      rename(stack);
      return;
    }
    if (target.closest('input, select, textarea, button, a, summary')) return;
    expand(stack);
  });
  // Touch: a horizontal swipe on the stack leafs through it.
  let start: { x: number; id: number } | null = null;
  stack.holder.addEventListener('pointerdown', (event) => {
    if (event.pointerType === 'touch') start = { x: event.clientX, id: event.pointerId };
  });
  stack.holder.addEventListener('pointermove', (event) => {
    if (!start || event.pointerId !== start.id) return;
    const dx = event.clientX - start.x;
    if (Math.abs(dx) > 48) {
      shuffle(stack, dx < 0 ? 1 : -1);
      start = { x: event.clientX, id: event.pointerId };
    }
  });
  stack.holder.addEventListener('pointerup', () => { start = null; });
}

/** The stack of a canvas item, if it is one. */
export function stackOfItem(item: HTMLElement): Stack | null {
  return stackOf(item);
}

/** Cmd/Ctrl+G: stacks the selected cards (and the cards of selected stacks). */
export function stackSelection(items: HTMLElement[]): boolean {
  const ids: string[] = [];
  let into: Stack | null = null;
  for (const item of items) {
    const stack = stackOf(item);
    if (stack) {
      into ??= stack;
      if (stack !== into) ids.push(...stack.cards);
    } else if (item.dataset.group && item.dataset.instanceId) {
      ids.push(item.dataset.instanceId);
    }
  }
  if (into) {
    addTo(into, ids, true);
    return true;
  }
  return createStack(ids) !== null;
}

/** "Stack" in the library pane: an app's cards as one stack. */
export function stackCards(ids: string[], title: string): void {
  const existing = [...stacks.values()].find((stack) => ids.every((id) => stack.cards.includes(id)));
  if (existing) {
    board.canvas()?.centerItem(existing.item);
    board.flash(existing.item);
    return;
  }
  const stack = createStack(ids, { title });
  if (stack) {
    requestAnimationFrame(() => board.canvas()?.centerItem(stack.item));
  }
}

// --- Drag one card onto another and hold: a pile (BumpTop) ---

function wireDragToStack(document_: Document): void {
  let target: HTMLElement | null = null;
  let timer = 0;
  let armed = false;
  const clear = () => {
    window.clearTimeout(timer);
    target?.classList.remove('is-stack-target');
    target = null;
    armed = false;
  };
  document_.addEventListener('pointermove', (event) => {
    const dragged = document_.querySelector<HTMLElement>('.widget-instance.is-dragging');
    if (!dragged || dragged.dataset.fanned) {
      if (target) clear();
      return;
    }
    const under = document_.elementsFromPoint(event.clientX, event.clientY)
      .map((node) => node.closest<HTMLElement>('.widget-instance'))
      .find((item): item is HTMLElement => Boolean(item) && item !== dragged && !item!.dataset.fanned) ?? null;
    const stackable = (item: HTMLElement | null) => Boolean(item && (isStackItem(item) || item.dataset.group));
    const next = stackable(under) && stackable(dragged) ? under : null;
    if (next === target) return;
    clear();
    if (!next) return;
    target = next;
    timer = window.setTimeout(() => {
      armed = true;
      target?.classList.add('is-stack-target');
    }, HOLD_MS);
  }, true);
  document_.addEventListener('pointerup', () => {
    const dragged = document_.querySelector<HTMLElement>('.widget-instance.is-dragging');
    const onto = armed ? target : null;
    clear();
    if (!dragged || !onto) return;
    // After the canvas has finished the drag.
    window.setTimeout(() => pile(dragged, onto), 30);
  }, true);
}

function pile(dragged: HTMLElement, onto: HTMLElement): void {
  const canvas = board.canvas();
  if (!canvas) return;
  const at = canvas.getItemState(onto);
  const draggedStack = stackOf(dragged);
  const ontoStack = stackOf(onto);
  const draggedIds = draggedStack ? draggedStack.cards : [dragged.dataset.instanceId ?? ''];
  if (ontoStack) {
    if (draggedStack) {
      collapse(draggedStack);
      removeStack(draggedStack, 'merged');
    }
    addTo(ontoStack, draggedIds);
    board.flash(ontoStack.item);
    return;
  }
  if (draggedStack) {
    addTo(draggedStack, [onto.dataset.instanceId ?? ''], true);
    canvas.moveItemTo(draggedStack.item, at.x, at.y);
    save();
    return;
  }
  const stack = createStack([...draggedIds, onto.dataset.instanceId ?? ''], { at: { x: at.x, y: at.y }, title: board.title(onto.dataset.instanceId ?? '') });
  if (stack) board.flash(stack.item);
}

/**
 * Takes cards out of their stacks (parked in the hidden board; a stack left
 * with one card spreads it onto the canvas). Returns the ids it handled.
 */
export function releaseCards(ids: string[]): string[] {
  const handled: string[] = [];
  for (const stack of [...stacks.values()]) {
    const out = stack.cards.filter((id) => ids.includes(id));
    if (!out.length) continue;
    handled.push(...out);
    collapse(stack);
    const current = stack.holder.firstElementChild as HTMLElement | null;
    if (current && out.includes(current.dataset.card ?? '')) board.park(current);
    stack.cards = stack.cards.filter((id) => !out.includes(id));
    if (stack.cards.length === 0) removeStack(stack, 'empty');
    else if (stack.cards.length === 1) dissolve(stack);
    else {
      stack.top = Math.min(stack.top, stack.cards.length - 1);
      showTop(stack);
    }
  }
  if (handled.length) save();
  return handled;
}

/** APP-106: the cards of the stack that holds `id` (top first as stacked), and which is on top. */
export function stackCardsOf(id: string): { cards: string[]; top: string; item: HTMLElement } | null {
  const stack = [...stacks.values()].find((candidate) => candidate.cards.includes(id) && !candidate.fanned);
  return stack ? { cards: [...stack.cards], top: stack.cards[stack.top] ?? id, item: stack.item } : null;
}

/** Starts stacks once (after the board): hooks, drag-to-pile, shuffle on scroll. */
export function startStacks(document_: Document): void {
  if (documentRef) return;
  documentRef = document_;
  setStackHooks({
    has: (id) => [...stacks.values()].some((stack) => stack.cards.includes(id)),
    count: () => [...stacks.values()].reduce((sum, stack) => sum + (stack.fanned ? 0 : stack.cards.length), 0),
    remove: releaseCards,
    live: (work) => {
      for (const stack of stacks.values()) {
        const members = stack.cards.filter((id) => id.includes(work));
        if (!members.length) continue;
        stack.item.classList.remove('is-live-updated');
        void stack.item.offsetWidth;
        stack.item.classList.add('is-live-updated');
        stack.root.classList.add('has-news');
        window.setTimeout(() => stack.root.classList.remove('has-news'), 6000);
        // A records card with a record waiting comes to the top.
        for (const id of members) {
          const card = board.findCard(id)?.card;
          if (!card || card.dataset.kind !== 'records') continue;
          void refreshRecords(card, true).then(() => {
            if (Number(card.dataset.pendingNow ?? '0') > 0 && stack.cards[stack.top] !== id && !stack.fanned) {
              stack.top = stack.cards.indexOf(id);
              showTop(stack, -1);
              save();
            }
          });
        }
      }
    },
    settled: () => {
      for (const row of loadStacks(board.member())) {
        const ids = row.cards.filter((id) => board.findCard(id)?.item);
        if (ids.length >= 2) createStack(ids, { id: row.id, title: row.title, at: { x: row.x, y: row.y }, top: row.top, quiet: true });
      }
    },
    dragEnd: (item) => {
      if (item.dataset.fanned) {
        // Pulled out of a fan: the card leaves its stack and stays where it was dropped.
        delete item.dataset.fanned;
        item.classList.remove('is-fanned');
        const id = item.dataset.instanceId ?? '';
        for (const stack of stacks.values()) {
          if (!stack.fanned?.includes(item)) continue;
          stack.fanned = stack.fanned.filter((other) => other !== item);
          stack.cards = stack.cards.filter((card) => card !== id);
          board.announce(`Taken out of ${stack.title}.`);
        }
      }
      if (isStackItem(item)) save();
    },
  });
  wireDragToStack(document_);
  const host = document_.getElementById('cv-canvas');
  // Horizontal scroll (or Shift + scroll) over a stack leafs through it.
  let travel = 0;
  host?.addEventListener('wheel', (event) => {
    const stack = stackOf(event.target as Element);
    if (!stack || stack.fanned || event.ctrlKey || event.metaKey) return;
    const horizontal = Math.abs(event.deltaX) > Math.abs(event.deltaY);
    if (!horizontal && !event.shiftKey) return;
    event.preventDefault();
    event.stopPropagation();
    travel += horizontal ? event.deltaX : event.deltaY;
    if (Math.abs(travel) >= 60) {
      shuffle(stack, travel > 0 ? 1 : -1);
      travel = 0;
    }
  }, { capture: true, passive: false });
  // A click outside a fan folds it back.
  document_.addEventListener('pointerdown', (event) => {
    const target = event.target as Element | null;
    if (!target?.closest('.forma-canvas-viewport')) return;
    if (target.closest('.widget-instance.is-fanned, .cv-item-stack')) return;
    collapseAll();
  }, true);
}
