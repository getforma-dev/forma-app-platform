// APP-104: the library pane. Every card the member may bring onto the
// canvas, grouped Category → App → Card, numbered (`3` an app, `3.2` its
// second card, `3.0` its app card). The list is rendered by the server from
// metadata only (`query::canvas::library`: outlines and records listings, no
// view runs); this module wires it: open and close (the header button, `L`,
// Esc), search by name or number, Add/Remove per card, "Add all" per app,
// the on-canvas indicators, the "Utilities" (widgets), new works the board
// places this session under "My apps", and the `/card` and `/app` commands
// with their autocomplete. Placing goes through the board (`addCards`),
// which fetches a work through the group route (re-authorized) and runs its
// views through the governed query route; nothing here reads data.
//
// Titles are text nodes; ids are the server's card ids.
import { addCards, boardWorks, isOnBoard, onBoardChange, removeCards } from './board';
import { stackCards } from './stacks';
import { registerCommands, registerCompleter, type Completion } from './slash';
import { el, isUuid } from './render';
import { removeWidgets, runWidget, widgetCount, WIDGET_COMMANDS } from './widgets/index';
import type { WidgetKind } from './widgets/state';
import { newNote, noteEntries, onNotesChange, toggleNote } from './notes';

type Report = (message: string) => void;
type CardRow = { id: string; number: string; title: string; kind: string; row: HTMLElement; button: HTMLButtonElement; app: AppRow };
type AppRow = {
  app: string; work: string; number: string; title: string; slug: string;
  row: HTMLElement; cards: CardRow[]; widget?: WidgetKind;
};

let documentRef: Document | null = null;
let pane: HTMLElement | null = null;
let toggle: HTMLElement | null = null;
let search: HTMLInputElement | null = null;
const apps: AppRow[] = [];
let lastFocus: HTMLElement | null = null;

const phone = () => matchMedia('(max-width: 760px)').matches;

export function isLibraryOpen(): boolean {
  return pane?.dataset.open === 'true';
}

export function openLibrary(focusSearch = true): void {
  if (!pane || !documentRef) return;
  if (!isLibraryOpen()) lastFocus = documentRef.activeElement as HTMLElement | null;
  pane.dataset.open = 'true';
  documentRef.getElementById('cv-shell')?.classList.add('is-library-open');
  toggle?.setAttribute('aria-expanded', 'true');
  sync();
  if (focusSearch && matchMedia('(pointer: fine)').matches) search?.focus({ preventScroll: true });
}

export function closeLibrary(): void {
  if (!pane || !documentRef || !isLibraryOpen()) return;
  pane.dataset.open = 'false';
  documentRef.getElementById('cv-shell')?.classList.remove('is-library-open');
  toggle?.setAttribute('aria-expanded', 'false');
  if (pane.contains(documentRef.activeElement)) (lastFocus ?? toggle)?.focus({ preventScroll: true });
}

export function toggleLibrary(): void {
  if (isLibraryOpen()) closeLibrary();
  else openLibrary();
}

function expand(app: AppRow, open: boolean): void {
  app.row.classList.toggle('is-expanded', open);
  app.row.querySelector('[data-lib-expand]')?.setAttribute('aria-expanded', String(open));
}

/** Reads one server-rendered (or island-built) app row. */
function index(row: HTMLElement): AppRow | null {
  const work = row.dataset.work ?? '';
  const app: AppRow = {
    app: row.dataset.app ?? '',
    work,
    number: row.dataset.number ?? '',
    title: row.querySelector('.cv-lib-app-title')?.textContent ?? '',
    slug: (row.querySelector('.cv-lib-slug')?.textContent ?? '').replace(/^\//, ''),
    row,
    cards: [],
  };
  if (row.dataset.widget) app.widget = row.dataset.widget as WidgetKind;
  else if (!isUuid(app.app) || !isUuid(work)) return null;
  for (const item of row.querySelectorAll<HTMLElement>('.cv-lib-card')) {
    const button = item.querySelector<HTMLButtonElement>('[data-lib-toggle]');
    if (!button) continue;
    app.cards.push({
      id: item.dataset.card ?? '',
      number: item.dataset.number ?? '',
      title: item.querySelector('.cv-lib-card-title')?.textContent ?? '',
      kind: item.querySelector('.cv-lib-kind')?.textContent ?? '',
      row: item,
      button,
      app,
    });
  }
  return app;
}

/** An app row built here (a work placed this session, a utility), as the server renders one. */
function buildAppRow(
  document_: Document,
  entry: { app: string; work: string; number: string; title: string; slug: string; widget?: WidgetKind; cards: { id: string; number: string; title: string; kind: string }[] },
): HTMLElement {
  const head = el(document_, 'div', { class: 'cv-lib-app-head' },
    el(document_, 'button', { type: 'button', class: 'cv-lib-expand', 'aria-expanded': 'false', 'data-lib-expand': '' },
      el(document_, 'span', { class: 'cv-lib-chevron', 'aria-hidden': 'true' }),
      el(document_, 'span', { class: 'cv-lib-num' }, entry.number),
      el(document_, 'span', { class: 'cv-lib-app-title' }, entry.title),
      ...(entry.slug ? [el(document_, 'code', { class: 'cv-lib-slug' }, `/${entry.slug}`)] : []),
      el(document_, 'span', { class: 'cv-lib-on', 'data-lib-on': '' })),
    ...(entry.widget ? [] : [
      el(document_, 'button', { type: 'button', class: 'cv-lib-stack', 'data-lib-stack': '', title: 'Add the app as one stack' }, 'Stack'),
      // APP-106: `unite.ts` handles the click.
      el(document_, 'button', { type: 'button', class: 'cv-lib-stack cv-lib-unite', 'data-lib-unite': '', title: 'Unite: assemble the app’s cards into one frame' }, 'Unite'),
    ]),
    el(document_, 'button', { type: 'button', class: 'cv-lib-all', 'data-lib-all': '' }, 'Add all'));
  const list = el(document_, 'ul', { class: 'cv-lib-cards' },
    ...entry.cards.map((card) => el(document_, 'li', {
      class: 'cv-lib-card', 'data-card': card.id, 'data-number': card.number,
      'data-search': `${card.number} ${card.title.toLowerCase()} ${card.kind}`,
    },
    el(document_, 'button', { type: 'button', class: 'cv-lib-toggle', 'aria-pressed': 'false', 'data-lib-toggle': '' },
      el(document_, 'span', { class: 'cv-lib-num' }, card.number),
      el(document_, 'span', { class: 'cv-lib-card-title' }, card.title),
      el(document_, 'span', { class: 'cv-lib-kind', 'data-kind': card.kind }, card.kind),
      el(document_, 'span', { class: 'cv-lib-action', 'data-lib-action': '' }, 'Add')))));
  const row = el(document_, 'li', {
    class: 'cv-lib-app', 'data-app': entry.app, 'data-work': entry.work, 'data-number': entry.number,
    'data-search': `${entry.number} ${entry.title.toLowerCase()} ${entry.slug ? `/${entry.slug}` : ''}`,
  }, head, list);
  if (entry.widget) row.dataset.widget = entry.widget;
  else row.id = `lib-${entry.work}`;
  return row;
}

function category(document_: Document, id: string, title: string): HTMLElement {
  const list = pane?.querySelector<HTMLElement>('[data-lib-list]');
  let section = list?.querySelector<HTMLElement>(`.cv-lib-cat[data-category="${id}"]`) ?? null;
  if (!section && list) {
    section = el(document_, 'section', { class: 'cv-lib-cat', 'data-category': id, 'aria-label': title },
      el(document_, 'h3', { class: 'cv-lib-cat-title' }, el(document_, 'span', {}, title), el(document_, 'span', { class: 'cv-lib-count' }, '')),
      el(document_, 'ul', { class: 'cv-lib-apps' }));
    // "My apps" sits before "Utilities".
    const utilities = list.querySelector('.cv-lib-cat[data-category="utilities"]');
    if (id !== 'utilities' && utilities) list.insertBefore(section, utilities);
    else list.append(section);
  }
  return section!;
}

function recount(section: HTMLElement): void {
  const count = section.querySelectorAll('.cv-lib-app:not([hidden])').length;
  const label = section.querySelector('.cv-lib-count');
  if (label) label.textContent = `${count} ${count === 1 ? 'app' : 'apps'}`;
}

function nextNumber(): number {
  return apps.reduce((max, app) => Math.max(max, Number(app.number) || 0), 0) + 1;
}

/** Works the board placed this session that the page did not list (a composition, a share). */
function adoptNewWorks(): void {
  if (!documentRef) return;
  for (const work of boardWorks()) {
    if (apps.some((app) => app.work === work.work)) continue;
    const number = String(nextNumber());
    const row = buildAppRow(documentRef, {
      app: work.app, work: work.work, number, title: work.title, slug: '',
      cards: work.cards.filter((card) => card.id).map((card, at) => ({ ...card, number: `${number}.${at}` })),
    });
    const section = category(documentRef, 'mine', 'My apps');
    section.querySelector('.cv-lib-apps')?.append(row);
    recount(section);
    const indexed = index(row);
    if (indexed) apps.push(indexed);
  }
}

function utilities(document_: Document): void {
  const section = category(document_, 'utilities', 'Utilities');
  for (const command of WIDGET_COMMANDS) {
    const row = buildAppRow(document_, {
      app: '', work: '', number: '', title: command.title, slug: command.slug, widget: command.slug as WidgetKind,
      cards: [{ id: `widget-${command.slug}`, number: '', title: command.purpose.replace(/\s*\(.*$/, '').replace(/[;.]\s.*$/, ''), kind: 'widget' }],
    });
    row.classList.add('is-utility');
    section.querySelector('.cv-lib-apps')?.append(row);
    const indexed = index(row);
    if (indexed) apps.push(indexed);
  }
  notesRow(document_, section);
  recount(section);
}

// --- APP-109: Utilities → Notes (the member's notes, `notes.ts`) ---
let notes: HTMLElement | null = null;

function notesRow(document_: Document, section: HTMLElement): void {
  notes = buildAppRow(document_, { app: '', work: '', number: '', title: 'Notes', slug: 'note', cards: [] });
  notes.classList.add('is-utility');
  notes.dataset.notes = 'true';
  notes.querySelector('[data-lib-all]')!.textContent = 'New note';
  for (const button of notes.querySelectorAll('[data-lib-stack], [data-lib-unite]')) button.remove();
  section.querySelector('.cv-lib-apps')?.append(notes);
}

/** Rebuilds the Notes row's cards: every note, Add or On canvas. */
function syncNotes(): void {
  if (!notes || !documentRef) return;
  const document_ = documentRef;
  const entries = noteEntries();
  notes.querySelector('.cv-lib-cards')?.replaceChildren(...entries.map((entry) => el(document_, 'li', {
    class: `cv-lib-card${entry.on ? ' is-on' : ''}`, 'data-note': entry.id, 'data-search': `note ${entry.title.toLowerCase()}`,
  },
  el(document_, 'button', { type: 'button', class: 'cv-lib-toggle', 'aria-pressed': String(entry.on), 'data-lib-toggle': '', title: entry.on ? 'On your canvas. Click to take it off (no data changes).' : 'Bring onto the canvas' },
    el(document_, 'span', { class: 'cv-lib-num' }, ''),
    el(document_, 'span', { class: 'cv-lib-card-title' }, entry.title),
    el(document_, 'span', { class: 'cv-lib-kind', 'data-kind': 'note' }, 'note'),
    el(document_, 'span', { class: 'cv-lib-action', 'data-lib-action': '' }, entry.on ? 'On canvas' : 'Add')))));
  const on = entries.filter((entry) => entry.on).length;
  const label = notes.querySelector('[data-lib-on]');
  if (label) label.textContent = entries.length ? `${on} of ${entries.length} on canvas` : '';
  notes.classList.toggle('has-on', on > 0);
}

/** A click inside the Notes row; true when it was one. */
function notesClick(target: Element | null): boolean {
  const row = target?.closest<HTMLElement>('.cv-lib-app[data-notes]');
  if (!row) return false;
  if (target?.closest('[data-lib-expand]')) {
    const open = !row.classList.contains('is-expanded');
    row.classList.toggle('is-expanded', open);
    row.querySelector('[data-lib-expand]')?.setAttribute('aria-expanded', String(open));
    return true;
  }
  if (target?.closest('[data-lib-all]')) {
    void newNote('').then(announce);
    if (phone()) closeLibrary();
    return true;
  }
  const id = target?.closest<HTMLElement>('[data-note]')?.dataset.note;
  if (id && target?.closest('[data-lib-toggle]')) announce(toggleNote(id));
  return true;
}

function announce(message: string): void {
  const status = documentRef?.getElementById('cv-activity');
  if (status) status.textContent = message;
}

/**
 * The Notes library app is shown as Utilities → Notes: its own rows (the
 * library entry and the member's notes work under "My apps") step aside.
 */
function replaceNotesApp(): void {
  const entry = apps.find((app) => app.slug === 'note' && !app.widget);
  if (!entry) return;
  for (const app of apps.filter((candidate) => candidate.app === entry.app && !candidate.widget)) {
    app.row.hidden = true;
    app.row.classList.add('is-replaced');
    apps.splice(apps.indexOf(app), 1);
  }
  for (const section of pane?.querySelectorAll<HTMLElement>('.cv-lib-cat') ?? []) {
    recount(section);
    if (!section.querySelector('.cv-lib-app:not([hidden])')) section.hidden = true;
  }
}

const onBoard = (card: CardRow): boolean => (card.app.widget ? widgetCount(card.app.widget) > 0 : isOnBoard(card.id));

/** The on-canvas indicators. */
function sync(): void {
  adoptNewWorks();
  for (const app of apps) {
    let on = 0;
    for (const card of app.cards) {
      const placed = onBoard(card);
      if (placed) on += 1;
      card.button.setAttribute('aria-pressed', String(placed));
      card.row.classList.toggle('is-on', placed);
      const action = card.button.querySelector('[data-lib-action]');
      if (action) action.textContent = placed ? (app.widget ? `${widgetCount(app.widget)} on canvas` : 'On canvas') : 'Add';
      card.button.title = placed ? 'On your canvas. Click to take it off (no data changes).' : 'Bring onto the canvas';
    }
    const label = app.row.querySelector('[data-lib-on]');
    if (label) label.textContent = on && !app.widget ? `${on} on canvas` : '';
    app.row.classList.toggle('has-on', on > 0);
    const all = app.row.querySelector<HTMLElement>('[data-lib-all]');
    if (all) all.textContent = app.widget ? 'Add' : on === app.cards.length ? 'Remove all' : 'Add all';
  }
}

/** Filters the list by name or number ("3.2", "3", "ledger", "/expense"). */
function filter(query: string): void {
  const words = query.trim().toLowerCase();
  for (const section of pane?.querySelectorAll<HTMLElement>('.cv-lib-cat') ?? []) {
    let shown = 0;
    for (const row of section.querySelectorAll<HTMLElement>('.cv-lib-app')) {
      const app = apps.find((candidate) => candidate.row === row);
      if (!app) continue;
      const appHit = !words || (row.dataset.search ?? '').includes(words) || app.number === words;
      let cardHits = 0;
      for (const card of app.cards) {
        const hit = !words || appHit || (card.row.dataset.search ?? '').includes(words) || card.number.startsWith(words);
        card.row.hidden = !hit;
        if (hit) cardHits += 1;
      }
      row.hidden = !(appHit || cardHits);
      if (!row.hidden) shown += 1;
      if (words) expand(app, Boolean(cardHits) && !app.widget);
    }
    section.hidden = shown === 0;
  }
  if (!words) for (const app of apps) expand(app, app.row.classList.contains('was-expanded'));
}

async function toggleCard(card: CardRow, report?: Report): Promise<void> {
  const message = card.app.widget
    ? (onBoard(card) ? (removeWidgets(card.app.widget), `${card.app.title} removed.`) : await runWidget(card.app.widget, ''))
    : onBoard(card)
      ? (removeCards([card.id]), `${card.title} is back in the library.`)
      : await addCards(card.app.app, card.app.work, [card.id]);
  report?.(message);
  if (!onBoard(card)) return;
  if (phone()) closeLibrary();
}

async function addAll(app: AppRow, report?: Report): Promise<string> {
  if (app.widget) return runWidget(app.widget, '');
  const message = app.cards.every(onBoard)
    ? (removeCards(app.cards.map((card) => card.id)), `${app.title} is back in the library.`)
    : await addCards(app.app, app.work, null);
  report?.(message);
  if (phone() && app.cards.some(onBoard)) closeLibrary();
  return message;
}

// --- Finding cards and apps by name or number (`/card`, `/app`, autocomplete) ---

function score(text: string, query: string): number {
  const value = text.toLowerCase();
  if (value === query) return 90;
  if (value.startsWith(query)) return 70;
  if (value.split(/\s+/).some((word) => word.startsWith(query))) return 55;
  if (value.includes(query)) return 45;
  return 0;
}

export function findCards(query: string): CardRow[] {
  const q = query.trim().toLowerCase().replace(/^#/, '');
  const cards = apps.filter((app) => !app.widget).flatMap((app) => app.cards);
  if (!q) return cards.filter((card) => card.kind !== 'app').slice(0, 12);
  if (/^\d+(\.\d*)?$/.test(q)) {
    return cards.filter((card) => card.number === q || card.number.startsWith(q.includes('.') ? q : `${q}.`));
  }
  return cards
    .map((card) => ({ card, value: Math.max(score(card.title, q), score(`${card.app.title} ${card.title}`, q) - 10) }))
    .filter((entry) => entry.value > 0)
    .sort((a, b) => b.value - a.value)
    .map((entry) => entry.card);
}

export function findApps(query: string): AppRow[] {
  const q = query.trim().toLowerCase().replace(/^\//, '');
  const list = apps.filter((app) => !app.widget);
  if (!q) return list;
  if (/^\d+$/.test(q)) return list.filter((app) => app.number === q);
  return list
    .map((app) => ({ app, value: app.slug === q ? 100 : app.slug.startsWith(q) ? 60 : score(app.title, q) }))
    .filter((entry) => entry.value > 0)
    .sort((a, b) => b.value - a.value)
    .map((entry) => entry.app);
}

/** A placing command leaves the bar empty for the next one. */
function clearBar(): void {
  const input = documentRef?.getElementById('intent-input') as HTMLInputElement | null;
  if (input) input.value = '';
}

const CARD_COMMAND = {
  slug: 'card',
  title: 'Bring a card',
  purpose: 'Place one card by name or number (“/card 3.2”, “/card spend by category”)',
  run: async (text: string, report: Report = () => undefined) => {
    const [card] = findCards(text);
    if (!text.trim()) {
      openLibrary();
      report('Pick a card in the library, or type /card and its number.');
      return;
    }
    if (!card) {
      report(`No card matches “${text.trim()}”. Open the library (L) to browse.`);
      return;
    }
    report(isOnBoard(card.id) ? `${card.number} ${card.title} is already on your canvas.` : await addCards(card.app.app, card.app.work, [card.id]));
    clearBar();
  },
};

const APP_COMMAND = {
  slug: 'app',
  title: 'Bring an app',
  purpose: 'Place every card of one app (“/app expense”, “/app 3”)',
  run: async (text: string, report: Report = () => undefined) => {
    if (!text.trim()) {
      openLibrary();
      report('Pick an app in the library, or type /app and its name.');
      return;
    }
    const [app] = findApps(text);
    if (!app) {
      report(`No app matches “${text.trim()}”. Open the library (L) to browse.`);
      return;
    }
    report(await addCards(app.app, app.work, null));
    clearBar();
  },
};

function cardCompletions(rest: string): Completion[] {
  return findCards(rest).slice(0, 10).map((card) => ({
    badge: card.number,
    title: card.title,
    detail: `${card.app.title} · ${card.kind}${isOnBoard(card.id) ? ' · on canvas' : ''}`,
    value: `/card ${card.number}`,
  }));
}
function appCompletions(rest: string): Completion[] {
  return findApps(rest).slice(0, 10).map((app) => ({
    badge: app.number,
    title: app.title,
    detail: `${app.slug ? `/${app.slug} · ` : ''}${app.cards.length} cards`,
    value: `/app ${app.slug || app.number}`,
  }));
}

/** Wires the pane once (after the board started). */
export function startLibrary(document_: Document): void {
  if (pane) return;
  documentRef = document_;
  pane = document_.getElementById('cv-library');
  toggle = document_.getElementById('cv-library-toggle');
  search = document_.getElementById('cv-lib-search') as HTMLInputElement | null;
  if (!pane) return;
  for (const row of pane.querySelectorAll<HTMLElement>('.cv-lib-app')) {
    const app = index(row);
    if (app) apps.push(app);
  }
  utilities(document_);
  replaceNotesApp();
  // Small libraries open expanded; a large one shows its apps first.
  if (apps.filter((app) => !app.widget).length <= 3) for (const app of apps) if (!app.widget) expand(app, true);
  toggle?.addEventListener('click', () => toggleLibrary());
  document_.addEventListener('click', (event) => {
    const target = event.target as Element | null;
    if (notesClick(target)) return;
    if (target?.closest('[data-lib-open]')) {
      openLibrary();
      return;
    }
    if (target?.closest('[data-lib-close]')) {
      closeLibrary();
      return;
    }
    const row = target?.closest<HTMLElement>('.cv-lib-app');
    const app = row && apps.find((candidate) => candidate.row === row);
    if (!app) return;
    if (target?.closest('[data-lib-expand]')) {
      const open = !app.row.classList.contains('is-expanded');
      expand(app, open);
      app.row.classList.toggle('was-expanded', open);
      return;
    }
    if (target?.closest('[data-lib-stack]') && !app.widget) {
      void addCards(app.app, app.work, null, { focus: false }).then(() => {
        stackCards(app.cards.map((card) => card.id), app.title);
        if (phone()) closeLibrary();
      });
      return;
    }
    if (target?.closest('[data-lib-all]')) {
      void addAll(app);
      return;
    }
    const cardRow = target?.closest<HTMLElement>('.cv-lib-card');
    const card = cardRow && app.cards.find((candidate) => candidate.row === cardRow);
    if (card && target?.closest('[data-lib-toggle]')) void toggleCard(card);
  });
  search?.addEventListener('input', () => filter(search?.value ?? ''));
  search?.addEventListener('keydown', (event) => {
    if (event.key === 'Enter') {
      // Enter adds the first visible card that is not on the canvas yet.
      event.preventDefault();
      const first = apps.flatMap((app) => app.cards).find((card) => !card.row.hidden && !card.app.row.hidden && card.row.offsetParent !== null && !onBoard(card));
      if (first) void toggleCard(first);
    }
  });
  pane.addEventListener('keydown', (event) => {
    if (event.key === 'Escape') {
      event.preventDefault();
      event.stopPropagation();
      if (search && search.value) {
        search.value = '';
        filter('');
      } else {
        closeLibrary();
      }
    }
  });
  onBoardChange(sync);
  onNotesChange(syncNotes);
  registerCommands([CARD_COMMAND, APP_COMMAND]);
  registerCompleter('card', cardCompletions);
  registerCompleter('app', appCompletions);
  sync();
}
