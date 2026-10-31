// APP-120 (Victor: "make it clear that the app name has an alias to call
// it"): one small component, the address chip (`/expenses`, grey, monospace),
// reused wherever an app appears: the Library's app row (with "also
// /expense" for its kept aliases), the app card's header on the canvas, the
// bar's hint while an address is typed ("/expenses — Expenses · file a claim
// or approve one") and the confirm card's footer ("via /expenses"). A
// clickable chip puts "/expenses " in the bar and focuses it, so people learn
// the address by clicking it. Built with `el()` only: text nodes, no markup.
import { addressList, type Address } from './command-registry';
import { el } from './render';

/** Puts `text` in the intent bar, focused with the caret at the end; the autocomplete then shows its hint. */
export function fillBar(text: string, document_: Document = document): void {
  const input = document_.getElementById('intent-input') as HTMLInputElement | null;
  if (!input) return;
  input.value = text;
  input.focus();
  input.setSelectionRange?.(text.length, text.length);
  input.dispatchEvent?.(new Event('input', { bubbles: true }));
}

/**
 * The chip: `/slug` in grey monospace. `interactive` (the default) makes it a
 * button that fills the bar with "/slug "; the bar's own hint uses the plain one.
 */
export function addressChip(document_: Document, slug: string, options: { interactive?: boolean; title?: string } = {}): HTMLElement {
  const text = `/${slug}`;
  if (options.interactive === false) return el(document_, 'code', { class: 'cv-addr-chip', 'data-address': slug }, text);
  const chip = el(document_, 'button', {
    type: 'button', class: 'cv-addr-chip', 'data-address': slug,
    title: options.title ?? `Type ${text} and what you want · click to put it in the bar`,
  }, el(document_, 'code', {}, text));
  chip.addEventListener('click', (event) => {
    (event as Event).stopPropagation?.();
    fillBar(`${text} `, document_);
  });
  return chip;
}

/** "also /expense": an app's kept aliases, small, on the Library's app row only. */
export function aliasNote(document_: Document, aliases: string[]): HTMLElement | null {
  if (!aliases.length) return null;
  return el(document_, 'span', { class: 'cv-addr-also' }, `also ${aliases.map((alias) => `/${alias}`).join(', ')}`);
}

/** "/expenses — Expenses · file a claim or approve one": the bar's hint for an address. */
export function addressLine(document_: Document, address: Pick<Address, 'slug' | 'title' | 'hint'>): HTMLElement {
  return el(document_, 'span', { class: 'cv-addr-line' },
    addressChip(document_, address.slug, { interactive: false }),
    el(document_, 'span', { class: 'cv-addr-what' }, ` — ${address.title}${address.hint ? ` · ${address.hint}` : ''}`));
}

/** The address an app (or a saved work) answers to: a saved app by its work, a library app by its id. */
export function addressFor(app: string, work = '', list: Address[] = addressList()): Address | undefined {
  return (work ? list.find((address) => address.kind === 'saved' && address.work === work) : undefined)
    ?? list.find((address) => address.kind === 'library' && address.app === app);
}

/**
 * The app card's header on the canvas gets its address chip after the title
 * (the card carries `data-app` and `data-work`, set by the board). Idempotent:
 * an existing chip is replaced when the address changed, removed when none.
 */
export function decorateAppCard(card: HTMLElement, list: Address[] = addressList()): void {
  const document_ = card.ownerDocument;
  const address = addressFor(card.dataset.app ?? '', card.dataset.work ?? '', list);
  const existing = card.querySelector<HTMLElement>('.cv-addr-chip');
  if (existing?.dataset.address === address?.slug) return;
  existing?.remove();
  if (!address) return;
  const title = card.querySelector('h2');
  const chip = addressChip(document_, address.slug);
  chip.classList.add('cv-app-addr');
  if (title) title.after(chip);
  else card.prepend(chip);
}

/** Every app card on the page (addresses arrive after the cards). */
export function decorateAppCards(document_: Document): void {
  const list = addressList();
  for (const card of document_.querySelectorAll<HTMLElement>('.cv-app')) if (card.dataset.app) decorateAppCard(card, list);
}
