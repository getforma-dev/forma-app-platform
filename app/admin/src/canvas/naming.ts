// APP-121: people name apps and cards. Two layers, one route
// (`POST /v1/apps/{app}/labels`, `composer::labels`):
//
// - the organization's labels, by the app's owner or an admin: the app's
//   name, address (`/spend`), kept aliases and description, and each card's
//   name and description; everyone who can open the app sees them;
// - the member's own: shortcuts (`/exp` → an app, `/q3` → a card) and, for
//   someone who may not rename a card for everyone, their own card name.
//
// Three ways in, one path: `/rename <app|app.card> <new name> [/address]`
// and `/alias <shortcut> <app|app.card>` (verbs of the Make group), the ✎ on
// the Library's app and card rows (a small editor in the dock), and a
// double-click on a card's title on the canvas. Each write re-reads the
// member's commands, so the Library, the canvas, the chips and the `/`
// list show the new names at once. Names are text nodes only.
import { addressFor, fillBar } from './address-chip';
import { addressList, findAddress, findCardShortcut, type Address, type CanvasCommand } from './command-registry';
import { isDismissKey, showDockCard } from './dock-card';
import { cardAddress, findApps, findCardRow, findCards, setLabelEditor, type AppRow, type CardRow } from './library';
import { announce, el } from './render';
import { refreshCommands, registerCommands } from './slash';

type Report = (message: string) => void;
export type LabelBody = {
  scope: 'organization' | 'personal';
  card?: string;
  name?: string | null;
  description?: string | null;
  address?: string | null;
  aliases?: string[];
};

const PROBLEMS: Record<string, string> = {
  name_taken: 'is already taken: another app, or someone’s address, answers to it',
  verb_name: 'is a command’s name (like /card or /app)',
  invalid_name: 'is not a name: use 2–32 lowercase letters, digits or hyphens',
  invalid_label: 'could not be saved: names are at most 60 characters',
};

/** What the server's answer means to the member (exported for the tests). */
export function labelMessage(status: number, code: string, what: string): string {
  if (status === 404) return `You can’t change ${what}. Only its owner or an admin renames it for everyone; /alias makes your own shortcut.`;
  const problem = PROBLEMS[code];
  return problem ? `${what} ${problem}.` : `${what} could not be saved. Try again.`;
}

/** Posts one label change; resolves to null when saved, else the message to show. */
export async function saveLabel(app: string, body: LabelBody, what: string): Promise<string | null> {
  let response: Response;
  try {
    response = await fetch(`/v1/apps/${app}/labels`, {
      method: 'POST', credentials: 'same-origin', cache: 'no-store',
      headers: { 'content-type': 'application/json' }, body: JSON.stringify(body),
    });
  } catch {
    return `${what} could not be saved. Try again.`;
  }
  if (response.ok) {
    await refreshCommands();
    return null;
  }
  const json = (await response.json().catch(() => null)) as { code?: unknown } | null;
  return labelMessage(response.status, typeof json?.code === 'string' ? json.code : '', what);
}

/** A target as typed: an app (`expenses`, `3`) or a card (`expenses.total`, `3.2`, an app card is its app). */
export type Target = { app: AppRow; address: Address; card: CardRow | null };
export function findTarget(text: string): Target | null {
  const word = text.trim().toLowerCase().replace(/^\//, '');
  if (!word) return null;
  const shortcut = findCardShortcut(word);
  if (shortcut) {
    const card = findCards(`${shortcut.address.slug}.${shortcut.card}`)[0];
    if (card) return { app: card.app, address: shortcut.address, card };
  }
  if (word.includes('.')) {
    const card = findCards(word)[0];
    const address = card && (card.app.address ?? addressFor(card.app.app, card.app.work));
    if (!card || !address) return null;
    return { app: card.app, address, card: card.id.startsWith('a-') ? null : card };
  }
  const named = findAddress(word);
  const app = named
    ? findApps(named.slug)[0]
    : findApps(word).find((candidate) => candidate.number === word || candidate.slug === word);
  const address = app && (app.address ?? addressFor(app.app, app.work));
  return app && address ? { app, address, card: null } : null;
}

/** `/rename` reads "<target> <new name> [/address]" (exported for the tests). */
export function parseRename(text: string): { target: string; name: string; address: string | null } {
  const [target = '', ...rest] = text.trim().split(/\s+/);
  let words = rest;
  let address: string | null = null;
  const last = words[words.length - 1];
  if (last && /^\/[a-z0-9-]+$/i.test(last)) {
    address = last.slice(1).toLowerCase();
    words = words.slice(0, -1);
  }
  return { target, name: words.join(' ').trim(), address };
}

/** Renames an app (owner or admin) or a card (for everyone when permitted, else for the member). */
export async function rename(target: Target, name: string, address: string | null): Promise<string> {
  const { card } = target;
  if (!card) {
    if (!target.address.canLabel) {
      return `Only the owner of ${target.address.title} or an admin can rename it for everyone. Type /alias <shortcut> ${target.address.slug} for your own shortcut.`;
    }
    const failed = await saveLabel(target.app.app, { scope: 'organization', ...(name ? { name } : {}), ...(address ? { address } : {}) }, address ? `/${address}` : `“${name}”`);
    if (failed) return failed;
    const now = addressList().find((entry) => entry.app === target.app.app && entry.kind === 'library');
    return `${target.address.title} is now ${now?.title ?? name}${now ? ` at /${now.slug}` : ''}. Everyone who can open it sees the new name; its old names still work.`;
  }
  const everyone = target.address.canLabel;
  const failed = await saveLabel(target.app.app, everyone
    ? { scope: 'organization', card: card.slug, name: name || null }
    : { scope: 'personal', card: card.slug, name: name || null }, `“${name}”`);
  if (failed) return failed;
  return everyone
    ? `${cardAddress(card)} is now “${name || card.base || card.title}” for everyone. Its data and receipts are unchanged.`
    : `${cardAddress(card)} is now “${name || card.base || card.title}” for you. Only the app’s owner or an admin renames it for everyone.`;
}

/** The member's own shortcuts of a target. */
function shortcuts(target: Target): string[] {
  return target.card
    ? target.address.cards?.find((label) => label.card === target.card?.slug)?.mine ?? []
    : target.address.mine ?? [];
}

/** Gives (or, with no target, takes back) a personal shortcut. */
export async function alias(shortcut: string, targetText: string): Promise<string> {
  const name = shortcut.trim().toLowerCase().replace(/^\//, '');
  if (!targetText.trim()) {
    const owner = addressList().find((address) => address.mine?.includes(name));
    const card = findCardShortcut(name);
    const target = owner ? findTarget(owner.slug) : card ? findTarget(`${card.address.slug}.${card.card}`) : null;
    if (!target) return `You have no /${name} shortcut. Type /alias ${name || '<shortcut>'} <app or app.card> to make one.`;
    const failed = await saveLabel(target.app.app, { scope: 'personal', card: target.card?.slug ?? '', aliases: shortcuts(target).filter((other) => other !== name) }, `/${name}`);
    return failed ?? `/${name} is gone. Nothing else changed.`;
  }
  const target = findTarget(targetText);
  if (!target) return `Nothing matches “${targetText.trim()}”. Name an app (expenses) or a card (expenses.total).`;
  const failed = await saveLabel(target.app.app, { scope: 'personal', card: target.card?.slug ?? '', aliases: [...shortcuts(target), name] }, `/${name}`);
  if (failed) return failed;
  return `/${name} now opens ${target.card ? cardAddress(target.card) : target.address.title}, for you only.`;
}

export const RENAME_COMMAND: CanvasCommand = {
  slug: 'rename',
  title: 'Rename',
  purpose: 'Name an app (and give it a new /address) or a card; your team sees it',
  run: async (text: string, report: Report = () => undefined) => {
    const parsed = parseRename(text);
    const target = findTarget(parsed.target);
    if (!target || (!parsed.name && !parsed.address)) {
      report(target ? 'Type the new name after it: /rename expenses Team spend /spend' : 'Type /rename and an app or card, then its new name: /rename expenses.total Q3 spend');
      return;
    }
    fillBar('');
    report(await rename(target, parsed.name, parsed.address));
  },
};

export const ALIAS_COMMAND: CanvasCommand = {
  slug: 'alias',
  title: 'Your shortcut',
  purpose: 'Your own /shortcut to an app or a card; only you see it',
  run: async (text: string, report: Report = () => undefined) => {
    const [shortcut = '', ...rest] = text.trim().split(/\s+/);
    if (!shortcut) {
      report('Type /alias, your shortcut and what it opens: /alias exp expenses');
      return;
    }
    fillBar('');
    report(await alias(shortcut, rest.join(' ')));
  },
};

// --- The ✎ editor (the Library) ---

const list = (value: string): string[] => value.split(/[\s,]+/).map((name) => name.replace(/^\//, '').toLowerCase()).filter(Boolean);

function field(document_: Document, label: string, value: string, options: { disabled?: boolean; hint?: string; name: string; long?: boolean }): HTMLElement {
  const input = el(document_, options.long ? 'textarea' : 'input', {
    class: 'cv-naming-input', name: options.name, ...(options.long ? { rows: '2' } : { type: 'text' }),
    ...(options.disabled ? { disabled: '' } : {}), maxlength: options.long ? '280' : '120',
  }) as HTMLInputElement;
  input.value = value;
  return el(document_, 'label', { class: 'cv-naming-field' },
    el(document_, 'span', { class: 'cv-naming-label' }, label),
    input,
    ...(options.hint ? [el(document_, 'span', { class: 'cv-naming-hint' }, options.hint)] : []));
}

/** Opens the editor for an app row or one of its cards (exported for the tests). */
export function openEditor(app: AppRow, card: CardRow | null, document_: Document = document): HTMLElement | null {
  const address = app.address;
  if (!address) return null;
  const label = card ? address.cards?.find((entry) => entry.card === card.slug) : undefined;
  const owner = address.canLabel === true;
  const what = card ? cardAddress(card) : `/${address.slug}`;
  const fields = el(document_, 'div', { class: 'cv-naming-fields' },
    field(document_, 'Name', card ? label?.name ?? '' : address.title, { name: 'name', disabled: !owner, hint: card ? card.base ?? card.title : '' }),
    ...(card ? [] : [
      field(document_, 'Address', `/${address.slug}`, { name: 'address', disabled: !owner, hint: 'Renaming keeps the old one working' }),
      field(document_, 'Also answers to', (address.orgAliases ?? []).map((name) => `/${name}`).join(', '), { name: 'aliases', disabled: !owner }),
    ]),
    field(document_, 'Description', card ? label?.description ?? '' : address.description ?? '', { name: 'description', disabled: !owner, long: true }),
    ...(card && !owner ? [field(document_, 'My name for it', label?.myName ?? '', { name: 'my_name' })] : []),
    field(document_, 'My shortcuts', (card ? label?.mine ?? [] : address.mine ?? []).map((name) => `/${name}`).join(', '), { name: 'mine', hint: 'Only you see these' }));
  const status = el(document_, 'p', { class: 'cv-naming-status', role: 'status' }, owner ? '' : 'Only the app’s owner or an admin changes these for everyone.');
  const save = el(document_, 'button', { type: 'submit', class: 'cv-primary' }, 'Save');
  const cancel = el(document_, 'button', { type: 'button', class: 'cv-secondary' }, 'Cancel');
  const form = el(document_, 'form', { class: 'cv-card cv-confirm cv-naming', 'aria-labelledby': 'cv-naming-title' },
    el(document_, 'header', { class: 'cv-card-head' }, el(document_, 'h3', { id: 'cv-naming-title' }, `Name ${card ? card.title : address.title}`), el(document_, 'code', { class: 'cv-addr-chip' }, what)),
    fields, status, el(document_, 'div', { class: 'cv-confirm-actions' }, cancel, save)) as HTMLFormElement;
  let close = () => undefined as void;
  const value = (name: string) => (form.querySelector<HTMLInputElement>(`[name="${name}"]`)?.value ?? '').trim();
  form.addEventListener('submit', (event) => {
    event.preventDefault();
    void (async () => {
      save.setAttribute('disabled', '');
      const target = card?.slug ?? '';
      let failed: string | null = null;
      if (owner) {
        const typed = list(value('address'))[0] ?? '';
        failed = await saveLabel(app.app, card
          ? { scope: 'organization', card: target, name: value('name') || null, description: value('description') || null }
          : { scope: 'organization', name: value('name') || null, description: value('description') || null, ...(typed ? { address: typed } : {}), aliases: list(value('aliases')) }, what);
      }
      if (!failed) {
        failed = await saveLabel(app.app, { scope: 'personal', card: target, ...(card && !owner ? { name: value('my_name') || null } : {}), aliases: list(value('mine')) }, 'Your shortcuts');
      }
      save.removeAttribute('disabled');
      if (failed) {
        status.textContent = failed;
        return;
      }
      close();
      announce(`Saved. ${card ? cardAddress(card) : `/${addressList().find((entry) => entry.app === app.app)?.slug ?? address.slug}`} shows its new names.`);
    })();
  });
  cancel.addEventListener('click', () => close());
  form.addEventListener('keydown', (event) => {
    if (isDismissKey(event as KeyboardEvent)) {
      event.preventDefault();
      close();
    }
  });
  close = showDockCard(document_, form, () => undefined);
  form.querySelector<HTMLInputElement>('input:not([disabled])')?.focus();
  return form;
}

// --- Double-click a card's title on the canvas ---

/** Renames the canvas card whose title was double-clicked, in place (Enter saves, Esc cancels). */
function editTitle(heading: HTMLElement): void {
  const cardNode = heading.closest<HTMLElement>('.cv-card');
  const row = cardNode?.dataset.card ? findCardRow(cardNode.dataset.card) : undefined;
  const address = row && (row.app.address ?? addressFor(row.app.app, row.app.work));
  if (!row || !address || address.kind !== 'library' || heading.querySelector('input')) return;
  const appCard = row.id.startsWith('a-');
  if (appCard && !address.canLabel) {
    announce(`Only the owner of ${address.title} or an admin can rename it. Type /alias <shortcut> ${address.slug} for your own shortcut.`);
    return;
  }
  const before = heading.textContent ?? '';
  const document_ = heading.ownerDocument;
  const input = el(document_, 'input', { type: 'text', class: 'cv-title-input', maxlength: '60', 'aria-label': 'New name' }) as HTMLInputElement;
  input.value = before;
  heading.replaceChildren(input);
  input.focus();
  input.select();
  let done = false;
  const finish = async (keep: boolean) => {
    if (done) return;
    done = true;
    const name = input.value.trim();
    heading.textContent = before;
    if (!keep || name === before || !name) return;
    const message = await rename({ app: row.app, address, card: appCard ? null : row }, name, null);
    announce(message);
  };
  input.addEventListener('keydown', (event) => {
    const key = (event as KeyboardEvent).key;
    event.stopPropagation();
    if (key === 'Enter') {
      event.preventDefault();
      void finish(true);
    } else if (key === 'Escape') {
      event.preventDefault();
      void finish(false);
    }
  });
  input.addEventListener('blur', () => void finish(false));
}

/** Wires the naming verbs, the Library's ✎ and the canvas's double-click (once). */
export function startNaming(document_: Document): void {
  registerCommands([RENAME_COMMAND, ALIAS_COMMAND]);
  setLabelEditor((app, card) => {
    openEditor(app, card, document_);
  });
  document_.addEventListener('dblclick', (event) => {
    const target = event.target as Element | null;
    const heading = target?.closest<HTMLElement>('.cv-card-head h3, .cv-card.cv-app > h2');
    if (!heading || heading.closest('.cv-naming, .cv-confirm')) return;
    event.preventDefault();
    event.stopPropagation();
    editTitle(heading);
  });
}
