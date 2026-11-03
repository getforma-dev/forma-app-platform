// The dock's card host (`#cv-confirm`, above the intent bar): one owner for
// the action path's confirmation card (`actions.ts`) and the near-tie chooser
// (`slash.ts`). APP-113: a card shown while another is pending first settles
// the pending one (its `onReplaced`, e.g. the confirmation resolving as
// replaced), so the intent bar never waits on a card that left the page.
// Closing a card returns focus to where it was when the card opened.
import { el } from './render';

type Current = { card: HTMLElement; onReplaced: () => void; opener: HTMLElement | null };
let current: Current | null = null;

function host(document_: Document): HTMLElement {
  let node = document_.getElementById('cv-confirm');
  if (!node) {
    node = el(document_, 'div', { id: 'cv-confirm', class: 'cv-confirm-host' });
    (document_.querySelector('.cv-dock') ?? document_.body).prepend(node);
  }
  return node;
}

/**
 * Shows `card` in the dock, replacing (and settling) any pending card.
 * Returns its close: it removes the card and returns focus, once.
 */
export function showDockCard(document_: Document, card: HTMLElement, onReplaced: () => void): () => void {
  const previous = current;
  const active = document_.activeElement;
  // Focus goes back to what opened the first card, not to a card being replaced.
  const opener = previous && previous.card.contains(active) ? previous.opener : (active as HTMLElement | null);
  current = { card, onReplaced, opener };
  host(document_).replaceChildren(card);
  previous?.onReplaced();
  let closed = false;
  return () => {
    if (closed) return;
    closed = true;
    const hadFocus = card.contains(document_.activeElement);
    card.remove();
    if (current?.card === card) current = null;
    if (hadFocus && opener?.isConnected) opener.focus({ preventScroll: true });
  };
}

/** Escape on a dock card, unless something inside it already handled the key. */
export function isDismissKey(event: KeyboardEvent): boolean {
  return event.key === 'Escape' && !event.defaultPrevented;
}
