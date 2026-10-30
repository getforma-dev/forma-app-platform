// APP-106 focus mode (Victor: "modals are awesome for focus work … a []
// button … brings their work into focus and they can switch too when in
// modal view. Then back to canvas for the whole picture").
//
// - Open: a card's focus button (`[ ]`, "Focus · F"), F with a card, stack or
//   frame selected, the frame's "Focus", the toolbar menu, or the deep link
//   `#focus=<card id>` (a refresh reopens the same card).
// - The card itself moves into a centered modal (about 80% of the viewport; a
//   full-screen sheet at phone width) over a dimmed, blurred canvas, and a
//   placeholder of the same size keeps its spot, so nothing on the canvas
//   re-flows. Nothing is rebuilt: form drafts, receipts, open details, scroll
//   and live updates carry on (the board runs every card that is not parked).
// - ← / → (or a swipe) walk the app's cards on the canvas, the stack the card
//   came from, or its unite frame; the thumbnail strip shows where you are,
//   jumps on click, and badges a records card with a record waiting for you.
// - Esc, the close button or a click on the backdrop return the card to its
//   spot and focus to the card. Focus is trapped in the modal (the rest of the
//   page is inert), and arrow keys never fire while typing.
import { isOnBoard, onBoardSettled, paneKind, stackBoard as board } from './board';
import { el } from './render';
import { stackCardsOf } from './stacks';
import { frameCardsOf, isUniteItem, keepScroll } from './unite';

type Session = {
  ids: string[];
  index: number;
  card: HTMLElement | null;
  placeholder: HTMLElement | null;
  opener: HTMLElement | null;
};

let documentRef: Document | null = null;
let dialog: HTMLElement | null = null;
let stage: HTMLElement | null = null;
let strip: HTMLElement | null = null;
let session: Session | null = null;
let observer: MutationObserver | null = null;
let lastPointed: HTMLElement | null = null;
const INERT = ['.cv-top', '#cv-canvas', '#cv-library', '.cv-dock'];

/**
 * APP-108: cards that are not an app's cards (a folder, an app's Summary, a
 * folder's Brief) can be focused too: each registers itself with its own
 * header for the focus button and an optional callback (a focused folder
 * becomes the bar's active folder). They are focused alone (no strip).
 */
type Standalone = { card: HTMLElement; label: string; onFocus?: () => void };
const standalone = new Map<string, Standalone>();
function svgFocus(document_: Document): SVGElement {
  const svg = document_.createElementNS('http://www.w3.org/2000/svg', 'svg');
  svg.setAttribute('viewBox', '0 0 16 16');
  svg.setAttribute('width', '14');
  svg.setAttribute('height', '14');
  svg.setAttribute('aria-hidden', 'true');
  const path = document_.createElementNS('http://www.w3.org/2000/svg', 'path');
  path.setAttribute('d', 'M2.5 6V2.5H6M10 2.5h3.5V6M13.5 10v3.5H10M6 13.5H2.5V10');
  svg.append(path);
  return svg;
}
/** Registers a standalone card (it needs a `data-card` id); its focus button goes into `head`. */
export function registerFocusCard(card: HTMLElement, head: HTMLElement, label: string, onFocus?: () => void): void {
  const id = card.dataset.card ?? '';
  if (!/^[A-Za-z0-9_-]{1,96}$/.test(id)) return;
  standalone.set(id, { card, label, onFocus });
  if (head.querySelector(':scope > [data-focus-card]')) return;
  const button = el(card.ownerDocument, 'button', { type: 'button', class: 'cv-card-focus', 'data-focus-card': '', 'aria-label': `Focus ${card.getAttribute('aria-label') ?? label}`, title: 'Focus · F' });
  button.append(svgFocus(card.ownerDocument));
  head.append(button);
}
/** The card left the canvas: it can no longer be focused (closed first if it is). */
export function unregisterFocusCard(card: HTMLElement): void {
  const id = card.dataset.card ?? '';
  if (session?.card === card) closeFocus();
  standalone.delete(id);
}
function cardOf(id: string): HTMLElement | undefined {
  return board.findCard(id)?.card ?? standalone.get(id)?.card;
}
/** A dialog stacked above focus (a file viewer opened from a focused card) owns the keys. */
function modalAbove(): boolean {
  return Boolean(documentRef?.querySelector('[data-modal-above]'));
}
const FOCUSABLE = 'a[href], button:not([disabled]), input:not([type="hidden"]):not([disabled]), select:not([disabled]), textarea:not([disabled]), summary, [tabindex]:not([tabindex="-1"])';

function editable(target: EventTarget | null): boolean {
  return target instanceof Element && Boolean(target.closest('input, textarea, select, [contenteditable=""], [contenteditable="true"]'));
}

export function isFocusOpen(): boolean {
  return session !== null;
}

function svgIcon(document_: Document, d: string): SVGElement {
  const svg = document_.createElementNS('http://www.w3.org/2000/svg', 'svg');
  svg.setAttribute('viewBox', '0 0 16 16');
  svg.setAttribute('width', '16');
  svg.setAttribute('height', '16');
  svg.setAttribute('aria-hidden', 'true');
  const path = document_.createElementNS('http://www.w3.org/2000/svg', 'path');
  path.setAttribute('d', d);
  svg.append(path);
  return svg;
}

function build(document_: Document): HTMLElement {
  const prev = el(document_, 'button', { type: 'button', class: 'cv-focus-step', 'data-focus-step': '-1', 'aria-label': 'Previous card (←)', title: 'Previous (←)' });
  prev.append(svgIcon(document_, 'M10 3.5L5.5 8l4.5 4.5'));
  const next = el(document_, 'button', { type: 'button', class: 'cv-focus-step', 'data-focus-step': '1', 'aria-label': 'Next card (→)', title: 'Next (→)' });
  next.append(svgIcon(document_, 'M6 3.5L10.5 8 6 12.5'));
  const close = el(document_, 'button', { type: 'button', class: 'cv-focus-close', 'data-focus-close': '', 'aria-label': 'Back to the canvas (Esc)', title: 'Back to the canvas (Esc)' });
  close.append(svgIcon(document_, 'M3.5 3.5l9 9M12.5 3.5l-9 9'));
  stage = el(document_, 'div', { class: 'cv-focus-stage', 'data-focus-stage': '' });
  strip = el(document_, 'nav', { class: 'cv-focus-strip', 'aria-label': 'Cards' });
  const root = el(document_, 'div', { id: 'cv-focus', class: 'cv-focus', role: 'dialog', 'aria-modal': 'true', 'aria-labelledby': 'cv-focus-title', hidden: '' },
    el(document_, 'div', { class: 'cv-focus-backdrop', 'data-focus-backdrop': '' }),
    el(document_, 'section', { class: 'cv-focus-panel' },
      el(document_, 'header', { class: 'cv-focus-head' },
        el(document_, 'span', { class: 'cv-focus-grip', 'aria-hidden': 'true' }),
        el(document_, 'div', { class: 'cv-focus-heading' },
          el(document_, 'p', { class: 'cv-focus-app', 'data-focus-app': '' }),
          el(document_, 'h2', { id: 'cv-focus-title', class: 'cv-focus-title', 'data-focus-title': '' })),
        el(document_, 'span', { class: 'cv-focus-pos', 'data-focus-pos': '', 'aria-live': 'polite' }),
        prev, next, close),
      stage,
      strip));
  root.addEventListener('click', (event) => {
    const target = event.target as Element | null;
    if (target?.closest('[data-focus-backdrop]')) {
      closeFocus();
      return;
    }
    if (target?.closest('[data-focus-close]')) {
      closeFocus();
      return;
    }
    const step = target?.closest<HTMLElement>('[data-focus-step]');
    if (step) {
      go(Number(step.dataset.focusStep));
      return;
    }
    const thumb = target?.closest<HTMLElement>('[data-focus-index]');
    if (thumb && session) show(Number(thumb.dataset.focusIndex));
  });
  // A horizontal swipe on the card walks the cards (not inside a table or a field).
  let start: { x: number; y: number; id: number } | null = null;
  stage.addEventListener('pointerdown', (event) => {
    const target = event.target as Element | null;
    if (event.pointerType === 'mouse' || target?.closest('.cv-table, input, select, textarea, button')) return;
    start = { x: event.clientX, y: event.clientY, id: event.pointerId };
  });
  stage.addEventListener('pointerup', (event) => {
    if (!start || event.pointerId !== start.id) return;
    const dx = event.clientX - start.x;
    const dy = event.clientY - start.y;
    start = null;
    if (Math.abs(dx) > 64 && Math.abs(dx) > Math.abs(dy) * 1.5) go(dx < 0 ? 1 : -1);
  });
  stage.addEventListener('pointercancel', () => { start = null; });
  (document_.getElementById('cv-shell') ?? document_.body).append(root);
  return root;
}

/** The cards ← / → walk: the stack the card is in, its frame, or its app's cards on the canvas. */
function sequenceFor(id: string): string[] {
  if (standalone.has(id)) return [id];
  const stack = stackCardsOf(id);
  if (stack) return stack.cards;
  const frame = frameCardsOf(id);
  if (frame) return frame;
  const found = board.findCard(id);
  const ids = (found?.group.cards ?? []).map((card) => card.dataset.card ?? '').filter((card) => card && isOnBoard(card));
  return ids.includes(id) ? ids : [id];
}

function titleOf(card: HTMLElement | undefined): string {
  if (!card) return 'Card';
  return card.dataset.kind === 'app' ? 'App card' : card.getAttribute('aria-label') ?? 'Card';
}

function pending(card: HTMLElement | undefined): number {
  if (!card || card.dataset.kind !== 'records') return 0;
  return Number(card.dataset.pendingNow ?? card.dataset.waiting ?? '0') || 0;
}

function renderStrip(): void {
  if (!session || !strip || !documentRef) return;
  const document_ = documentRef;
  const total = session.ids.length;
  strip.replaceChildren(...session.ids.map((id, index) => {
    const card = cardOf(id);
    const title = titleOf(card);
    // A status view is a breakdown: drawn as a chart, not one number.
    const raw = card?.dataset.kind ?? '';
    const kind = raw === 'ledger' ? 'ledger' : raw === 'status' ? 'chart' : paneKind(raw);
    return el(document_, 'button', {
      type: 'button', class: 'cv-focus-thumb', 'data-focus-index': String(index), 'data-kind': kind, 'data-thumb': id,
      'aria-label': `Card ${index + 1} of ${total}: ${title}`, title,
    },
    el(document_, 'span', { class: 'cv-focus-thumb-art', 'data-kind': kind, 'aria-hidden': 'true' }),
    el(document_, 'span', { class: 'cv-focus-thumb-title' }, title),
    el(document_, 'span', { class: 'cv-focus-thumb-badge', 'data-thumb-badge': '', hidden: '' }));
  }));
  strip.hidden = total < 2;
  syncStrip();
}

/** The current card, the waiting badges and the "news" marks on the strip. */
function syncStrip(): void {
  if (!session || !strip) return;
  for (const thumb of strip.querySelectorAll<HTMLElement>('[data-focus-index]')) {
    const index = Number(thumb.dataset.focusIndex);
    const card = cardOf(thumb.dataset.thumb ?? '');
    if (index === session.index) thumb.setAttribute('aria-current', 'true');
    else thumb.removeAttribute('aria-current');
    const count = pending(card);
    const badge = thumb.querySelector<HTMLElement>('[data-thumb-badge]');
    if (badge) {
      badge.hidden = count === 0;
      badge.textContent = String(count);
      badge.title = `${count} waiting for your approval`;
    }
  }
}

function watch(): void {
  observer?.disconnect();
  if (!session || typeof MutationObserver === 'undefined') return;
  observer = new MutationObserver((records) => {
    for (const record of records) {
      const card = record.target as HTMLElement;
      if (record.attributeName === 'class' && card.classList.contains('is-live-updated') && card !== session?.card) {
        const thumb = strip?.querySelector<HTMLElement>(`[data-thumb="${CSS.escape(card.dataset.card ?? '')}"]`);
        thumb?.classList.remove('has-news');
        void thumb?.offsetWidth;
        thumb?.classList.add('has-news');
      }
    }
    syncStrip();
  });
  for (const id of session.ids) {
    const card = cardOf(id);
    if (card) observer.observe(card, { attributes: true, attributeFilter: ['data-pending-now', 'data-waiting', 'data-state', 'class'] });
  }
}

/** Puts the focused card back in its spot (or in the hidden board, if it came from there). */
function putBack(): void {
  if (!session?.card) return;
  const { card, placeholder } = session;
  delete card.dataset.focused;
  const restoreScroll = keepScroll(card);
  if (placeholder?.isConnected) placeholder.replaceWith(card);
  else board.park(card);
  placeholder?.remove();
  restoreScroll();
  session.card = null;
  session.placeholder = null;
}

function show(index: number): void {
  if (!session || !stage || !dialog || !documentRef) return;
  const total = session.ids.length;
  const next = ((index % total) + total) % total;
  const id = session.ids[next] ?? '';
  const card = cardOf(id);
  if (!card) return;
  putBack();
  session.index = next;
  const restoreScroll = keepScroll(card);
  let placeholder: HTMLElement | null = null;
  if (card.closest('#cv-board')) {
    // A stack's hidden card: wired and run, then shown here.
    board.show(id);
  } else {
    placeholder = el(documentRef, 'div', { class: 'cv-focus-placeholder', 'aria-hidden': 'true' }, 'In focus');
    placeholder.style.height = `${card.offsetHeight}px`;
    card.replaceWith(placeholder);
  }
  stage.replaceChildren(card);
  card.dataset.focused = 'true';
  restoreScroll();
  session.card = card;
  session.placeholder = placeholder;
  const work = board.workOf(card) ?? '';
  const app = dialog.querySelector('[data-focus-app]');
  const own = standalone.get(id);
  if (app) app.textContent = own ? own.label : board.group(work)?.title ?? '';
  own?.onFocus?.();
  const title = dialog.querySelector('[data-focus-title]');
  if (title) title.textContent = titleOf(card);
  const pos = dialog.querySelector('[data-focus-pos]');
  if (pos) pos.textContent = total > 1 ? `${next + 1} / ${total}` : '';
  for (const step of dialog.querySelectorAll<HTMLButtonElement>('[data-focus-step]')) step.disabled = total < 2;
  syncStrip();
  strip?.querySelector<HTMLElement>(`[data-focus-index="${next}"]`)?.scrollIntoView({ block: 'nearest', inline: 'nearest' });
  try {
    history.replaceState(history.state, '', `#focus=${id}`);
  } catch {
    // the deep link is a convenience
  }
}

function go(step: number): void {
  if (!session || session.ids.length < 2) return;
  show(session.index + step);
  board.announce(`${titleOf(session.card ?? undefined)}, card ${session.index + 1} of ${session.ids.length}.`);
}

/** Opens card `id` in focus (it must be on the canvas). */
export function openFocus(id: string, opener: HTMLElement | null = null): boolean {
  const document_ = documentRef;
  if (!document_ || !((board.findCard(id) && isOnBoard(id)) || standalone.get(id)?.card.isConnected)) return false;
  dialog ??= build(document_);
  const ids = sequenceFor(id);
  if (session) putBack();
  session = { ids, index: Math.max(0, ids.indexOf(id)), card: null, placeholder: null, opener: opener ?? (document_.activeElement as HTMLElement | null) };
  renderStrip();
  show(session.index);
  dialog.hidden = false;
  for (const selector of INERT) document_.querySelector<HTMLElement>(selector)?.setAttribute('inert', '');
  document_.getElementById('cv-shell')?.classList.add('is-focus-open');
  watch();
  dialog.querySelector<HTMLElement>('[data-focus-close]')?.focus({ preventScroll: true });
  board.announce(`${titleOf(session.card ?? undefined)} in focus. Left and right arrows walk the cards; Esc returns to the canvas.`);
  return true;
}

export function closeFocus(): void {
  if (!session || !dialog || !documentRef) return;
  const card = session.card;
  const opener = session.opener;
  putBack();
  observer?.disconnect();
  session = null;
  dialog.hidden = true;
  for (const selector of INERT) documentRef.querySelector<HTMLElement>(selector)?.removeAttribute('inert');
  documentRef.getElementById('cv-shell')?.classList.remove('is-focus-open');
  try {
    if (location.hash.startsWith('#focus=')) history.replaceState(history.state, '', `${location.pathname}${location.search}`);
  } catch {
    // the deep link is a convenience
  }
  // Focus returns to the card (its focus button), else to what opened focus.
  const back = card && card.isConnected && !card.closest('#cv-board')
    ? card.querySelector<HTMLElement>('[data-focus-card]')
    : opener?.isConnected ? opener : card ? stackCardsOf(card.dataset.card ?? '')?.item ?? null : null;
  back?.focus({ preventScroll: true });
  board.announce('Back to the canvas.');
}

/** F on a selected canvas item: a card, a stack's top card, or the frame's last touched card. */
export function focusFromItem(item: HTMLElement | null): boolean {
  if (!item) return false;
  if (isUniteItem(item)) {
    const pointed = lastPointed && item.contains(lastPointed) ? lastPointed : item.querySelector<HTMLElement>('.cv-card[data-card]');
    return openFocus(pointed?.dataset.card ?? '');
  }
  if (item.classList.contains('cv-item-stack')) {
    return openFocus(item.querySelector<HTMLElement>('.cv-stack-top > .cv-card')?.dataset.card ?? '');
  }
  if (item.dataset.group) return openFocus(item.dataset.instanceId ?? '');
  const own = item.querySelector<HTMLElement>('.cv-card[data-card]');
  return own && standalone.has(own.dataset.card ?? '') ? openFocus(own.dataset.card ?? '') : false;
}

function onKey(event: KeyboardEvent): void {
  if (!session || !dialog || event.isComposing || modalAbove()) return;
  if (event.key === 'Escape') {
    event.preventDefault();
    event.stopImmediatePropagation();
    closeFocus();
    return;
  }
  if (event.key === 'Tab') {
    const focusables = Array.from(dialog.querySelectorAll<HTMLElement>(FOCUSABLE)).filter((node) => node.getClientRects().length > 0);
    const first = focusables[0];
    const last = focusables[focusables.length - 1];
    const active = documentRef?.activeElement ?? null;
    if (!first || !last) return;
    if (!dialog.contains(active)) {
      event.preventDefault();
      first.focus();
    } else if (event.shiftKey && active === first) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && active === last) {
      event.preventDefault();
      first.focus();
    }
    return;
  }
  if ((event.key === 'ArrowLeft' || event.key === 'ArrowRight') && !editable(event.target) && !event.metaKey && !event.ctrlKey && !event.altKey && !event.shiftKey) {
    event.preventDefault();
    event.stopImmediatePropagation();
    go(event.key === 'ArrowRight' ? 1 : -1);
  }
}

/** Wires focus mode once (after the board, stacks and unite). */
export function startFocus(document_: Document): void {
  if (documentRef) return;
  documentRef = document_;
  document_.addEventListener('keydown', onKey, true);
  document_.addEventListener('pointerdown', (event) => {
    const card = (event.target as Element | null)?.closest<HTMLElement>('.cv-card[data-card]');
    if (card) lastPointed = card;
  }, true);
  document_.addEventListener('click', (event) => {
    const target = event.target as Element | null;
    const button = target?.closest<HTMLElement>('[data-focus-card]');
    if (button) {
      const card = button.closest<HTMLElement>('.cv-card[data-card]');
      if (card && card !== session?.card) openFocus(card.dataset.card ?? '', button);
      return;
    }
    const frameFocus = target?.closest<HTMLElement>('[data-unite-focus]');
    if (frameFocus) {
      const first = frameFocus.closest('.cv-unite')?.querySelector<HTMLElement>('.cv-card[data-card]');
      if (first) openFocus(first.dataset.card ?? '', frameFocus);
    }
  });
  onBoardSettled(() => {
    const match = location.hash.match(/^#focus=([A-Za-z0-9_-]{1,96})$/);
    if (match?.[1]) openFocus(match[1]);
  });
}
