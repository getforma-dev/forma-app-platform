// APP-104: the library pane. Every card the member may bring onto the
// canvas, numbered (`3` an app, `3.2` its second card, `3.0` its app card).
// The list is rendered by the server from metadata only
// (`query::canvas::library`: outlines and records listings, no view runs);
// this module wires it: search by name or number, Add/Remove, the
// on-canvas indicators, new works the board places this session under "My
// apps", and the `/card` and `/app` commands with their autocomplete.
// APP-118: `/apps [query]` opens it filtered. APP-119: apps are addresses.
//
// APP-120: one canvas model. The Library has two sections: Apps (categories
// → apps → cards) and Widgets (Weather, Calculator, Clock, Note, Folder; the
// Note and Folder rows expand to the member's notes and folders). Every row
// has an on-canvas dot (filled or hollow), app rows read "n of m cards",
// each row has an Add/Remove toggle and its name in grey: an app's address
// chip (`/expenses`, "also /expense"), a card's `app.card` (`expenses.ledger`),
// a widget's `/weather`. The pane keeps no canvas state: it re-renders from
// the board's one `canvas:changed` event (`board.ts` `emitCanvasChanged`).
// Cards are addressed `app.card` or by number: `/card expenses.ledger` and
// `/card 3.2` place the same card. A bare `/<address>` places the app
// (`setAppPlacer`). The pane itself (open, close, tabs) is `pane.ts`.
// Placing goes through the board (`addCards`), which fetches a work through
// the group route (re-authorized) and runs its views through the governed
// query route; nothing here reads data.
//
// Titles are text nodes; ids are the server's card ids.
//
// APP-121: names people give. App and card rows (and the cards on the
// canvas) show the organization's labels and the member's own card names
// (`applyLabels`, from the addresses); a ✎ on a library app's row and on its
// card rows opens the naming editor (`naming.ts`, through `setLabelEditor`).
// The commands are loaded at start, so labels and the app card's address
// chip show without opening the pane.
import { addressChip, aliasNote, decorateAppCard, decorateAppCards } from './address-chip';
import { activeCanvasItem, addCards, boardWorks, CANVAS_CHANGED, isOnBoard, removeCards } from './board';
import { addressList, addressText, cardLabel, onAddressesChange, WIDGET_KINDS, type Address, type WidgetName } from './command-registry';
import { insertCommand, startCommandSheet } from './command-sheet';
import { startShortcutTab } from './shortcut-tab';
import { closePane, isPaneOpen, onPaneTab, openPane, startPane, togglePane } from './pane';
import { stackCards } from './stacks';
import { ensureCommands, registerCommands, registerCompleter, setAppPlacer, type Completion } from './slash';
import { announce, el, isUuid } from './render';
import { removeWidgets, runWidget, widgetCount } from './widgets/index';
import { newNote, noteEntries, toggleNote } from './notes';
import { closeFolders, folderEntries, newFolder, toggleFolder } from './boxes';
import { focusFromItem, openFocus } from './focus';

type Report = (message: string) => void;
export type CardRow = {
  id: string; number: string; title: string; kind: string; slug: string; row: HTMLElement; button: HTMLButtonElement; app: AppRow;
  /** APP-121: the title the server rendered (`title` is the label shown now). */
  base?: string;
};
export type AppRow = {
  app: string; work: string; number: string; title: string; slug: string;
  row: HTMLElement; cards: CardRow[];
  /** APP-119: the app's address, once the server's list arrived. */
  address?: Address;
};

let documentRef: Document | null = null;
/** APP-121: opens the naming editor for an app row (and one of its cards). */
type LabelEditor = (app: AppRow, card: CardRow | null) => void;
let labelEditor: LabelEditor | null = null;
export function setLabelEditor(editor: LabelEditor): void {
  labelEditor = editor;
}
/** APP-121: the indexed app rows (the naming commands find their targets here). */
export function libraryApps(): readonly AppRow[] {
  return apps;
}
/** APP-121: the library card row of a canvas card id. */
export function findCardRow(id: string): CardRow | undefined {
  for (const app of apps) {
    const card = app.cards.find((candidate) => candidate.id === id);
    if (card) return card;
  }
  return undefined;
}
let pane: HTMLElement | null = null;
let search: HTMLInputElement | null = null;
const apps: AppRow[] = [];

const phone = () => typeof matchMedia === 'function' && matchMedia('(max-width: 760px)').matches;

/** Whether the right pane is open (on any tab: Esc and × close it). */
export function isLibraryOpen(): boolean {
  return isPaneOpen();
}

/** Opens the right pane on its Library tab. */
export function openLibrary(focusSearch = true): void {
  openPane('library', { focus: focusSearch });
}

/** APP-118: opens the Library tab filtered by `query` (every app when empty). */
export function showLibrary(query = ''): void {
  openLibrary();
  if (search) search.value = query;
  filter(query);
}

export function closeLibrary(): void {
  closePane();
}

/** `L`: the Library tab, or closes the pane when it shows the Library already. */
export function toggleLibrary(): void {
  togglePane('library');
}

function expand(row: HTMLElement, open: boolean): void {
  row.classList.toggle('is-expanded', open);
  row.querySelector('[data-lib-expand]')?.setAttribute('aria-expanded', String(open));
}

// --- Card addresses (`expenses.ledger`) ---

/**
 * APP-120: a card's name within its app, from its id and kind: a view card
 * (`c-{work}-{view}`) by its view id (`by_status` → `by-status`), the app
 * card `app`, the record form `new`, the records card `records`, the ledger
 * `ledger`. `taken` makes a repeated name unique (`records-2`).
 */
export function cardSlug(id: string, kind: string, work: string, taken: Set<string> = new Set()): string {
  let base = '';
  if (id.startsWith('a-')) base = 'app';
  else if (id.startsWith('f-')) base = 'new';
  else if (id.startsWith('r-')) base = 'records';
  else if (id.startsWith('l-')) base = 'ledger';
  else if (work && id.startsWith(`c-${work}-`)) base = id.slice(work.length + 3);
  else base = kind;
  base = base.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-+|-+$/g, '') || 'card';
  let name = base;
  for (let at = 2; taken.has(name); at += 1) name = `${base}-${at}`;
  taken.add(name);
  return name;
}

/** `expenses.ledger`: the app's address (else its number) and the card's name. */
export function cardAddress(card: CardRow): string {
  return `${card.app.slug || card.app.number}.${card.slug}`;
}

/** "2 of 8 cards", the dot and the toggle of an app row for `on` of `total` cards on the canvas. */
export function appStatus(on: number, total: number): { dot: 'filled' | 'hollow'; label: string; action: 'Add' | 'Remove' } {
  return {
    dot: on > 0 ? 'filled' : 'hollow',
    label: `${on} of ${total} ${total === 1 ? 'card' : 'cards'}`,
    action: total > 0 && on === total ? 'Remove' : 'Add',
  };
}

function setDot(node: Element | null | undefined, on: boolean): void {
  node?.setAttribute('data-on', on ? 'true' : 'false');
}

// --- App rows ---

/** Reads one server-rendered (or island-built) app row and indexes it (exported for the tests). */
export function indexRow(row: HTMLElement): AppRow | null {
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
  if (!isUuid(app.app) || !isUuid(work)) return null;
  const taken = new Set<string>();
  for (const item of row.querySelectorAll<HTMLElement>('.cv-lib-card')) {
    const button = item.querySelector<HTMLButtonElement>('[data-lib-toggle]');
    if (!button) continue;
    const id = item.dataset.card ?? '';
    const kind = item.querySelector('.cv-lib-kind')?.textContent ?? '';
    app.cards.push({
      id,
      number: item.dataset.number ?? '',
      title: item.querySelector('.cv-lib-card-title')?.textContent ?? '',
      kind,
      slug: cardSlug(id, kind, work, taken),
      row: item,
      button,
      app,
    });
  }
  apps.push(app);
  nameCards(app);
  return app;
}

/** Each card row shows its `app.card` name in grey (and is found by it). */
function nameCards(app: AppRow): void {
  if (!documentRef && !app.row.ownerDocument) return;
  const document_ = app.row.ownerDocument;
  for (const card of app.cards) {
    const name = cardAddress(card);
    let label = card.row.querySelector<HTMLElement>('.cv-lib-cardname');
    if (!label) {
      label = el(document_, 'code', { class: 'cv-lib-cardname' }, '');
      card.button.querySelector('[data-lib-action]')?.before(label);
    }
    label.textContent = name;
    card.row.dataset.search = `${card.number} ${card.title.toLowerCase()} ${card.kind} ${name}`;
    // APP-121: a library app's cards can be named (✎).
    if (app.address?.kind === 'library' && !card.row.querySelector('[data-lib-edit]')) {
      card.row.append(el(document_, 'button', { type: 'button', class: 'cv-lib-edit', 'data-lib-edit': card.id, title: `Name ${card.title}`, 'aria-label': `Name ${card.title}` }, '✎'));
    }
  }
}

/** An app row built here (a work placed this session), as the server renders one. */
export function buildAppRow(
  document_: Document,
  entry: { app: string; work: string; number: string; title: string; slug: string; cards: { id: string; number: string; title: string; kind: string }[] },
): HTMLElement {
  const head = el(document_, 'div', { class: 'cv-lib-app-head' },
    el(document_, 'button', { type: 'button', class: 'cv-lib-expand', 'aria-expanded': 'false', 'data-lib-expand': '' },
      el(document_, 'span', { class: 'cv-lib-chevron', 'aria-hidden': 'true' }),
      el(document_, 'span', { class: 'cv-lib-dot', 'data-lib-dot': '', 'aria-hidden': 'true' }),
      el(document_, 'span', { class: 'cv-lib-num' }, entry.number),
      el(document_, 'span', { class: 'cv-lib-app-title', title: entry.title }, entry.title),
      el(document_, 'span', { class: 'cv-lib-on', 'data-lib-on': '' })),
    ...(entry.slug ? [el(document_, 'code', { class: 'cv-lib-slug cv-addr-chip' }, `/${entry.slug}`)] : []),
    el(document_, 'button', { type: 'button', class: 'cv-lib-stack', 'data-lib-stack': '', title: 'Add the app as one stack' }, 'Stack'),
    // APP-106: `unite.ts` handles the click.
    el(document_, 'button', { type: 'button', class: 'cv-lib-stack cv-lib-unite', 'data-lib-unite': '', title: 'Unite: assemble the app’s cards into one frame' }, 'Unite'),
    el(document_, 'button', { type: 'button', class: 'cv-lib-all', 'data-lib-all': '' }, 'Add'));
  const list = el(document_, 'ul', { class: 'cv-lib-cards' },
    ...entry.cards.map((card) => el(document_, 'li', {
      class: 'cv-lib-card', 'data-card': card.id, 'data-number': card.number,
      'data-search': `${card.number} ${card.title.toLowerCase()} ${card.kind}`,
    },
    el(document_, 'button', { type: 'button', class: 'cv-lib-toggle', 'aria-pressed': 'false', 'data-lib-toggle': '' },
      el(document_, 'span', { class: 'cv-lib-dot', 'data-lib-dot': '', 'aria-hidden': 'true' }),
      el(document_, 'span', { class: 'cv-lib-num' }, card.number),
      el(document_, 'span', { class: 'cv-lib-card-title', title: card.title }, card.title),
      el(document_, 'span', { class: 'cv-lib-kind', 'data-kind': card.kind }, card.kind),
      el(document_, 'span', { class: 'cv-lib-action', 'data-lib-action': '' }, 'Add')))));
  const row = el(document_, 'li', {
    class: 'cv-lib-app', 'data-app': entry.app, 'data-work': entry.work, 'data-number': entry.number,
    'data-search': `${entry.number} ${entry.title.toLowerCase()} ${entry.slug ? `/${entry.slug}` : ''}`,
  }, head, list);
  row.id = `lib-${entry.work}`;
  return row;
}

/** A category section of the Apps group ("My apps" is made here). */
function category(document_: Document, id: string, title: string): HTMLElement {
  const group = pane?.querySelector<HTMLElement>('[data-lib-group="apps"]');
  let section = group?.querySelector<HTMLElement>(`[data-category="${id}"]`) ?? null;
  if (!section && group) {
    section = el(document_, 'section', { class: 'cv-lib-cat', 'data-category': id, 'aria-label': title },
      el(document_, 'h3', { class: 'cv-lib-cat-title' }, el(document_, 'span', {}, title), el(document_, 'span', { class: 'cv-lib-count' }, '')),
      el(document_, 'ul', { class: 'cv-lib-apps' }));
    group.append(section);
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
    const indexed = indexRow(row);
    if (indexed) decorate(indexed, rowAddress(indexed, addressList()));
  }
}

/**
 * The library's Notes app is the Note widget to the member (APP-120): its own
 * rows (the library entry and the member's notes work under "My apps") step aside.
 */
function replaceNotesApp(): void {
  const entry = apps.find((app) => app.slug === 'notes' || app.slug === 'note');
  if (!entry) return;
  for (const app of apps.filter((candidate) => candidate.app === entry.app)) {
    app.row.hidden = true;
    app.row.classList.add('is-replaced');
    apps.splice(apps.indexOf(app), 1);
  }
  for (const section of pane?.querySelectorAll<HTMLElement>('.cv-lib-cat') ?? []) {
    if (section.dataset.category === 'widgets') continue;
    recount(section);
    if (!section.querySelector('.cv-lib-app:not([hidden])')) section.hidden = true;
  }
}

// --- The Widgets section ---

/** What the Widgets section shows: how many of each kind are on the canvas, the notes and the folders. */
export type WidgetSnapshot = {
  counts: Record<'weather' | 'calculator' | 'clock', number>;
  notes: { id: string; title: string; on: boolean }[];
  folders: { id: string; number: number; title: string; detail: string; on: boolean }[];
};

const WIDGET_ROWS: Record<WidgetName, string> = {
  weather: 'Weather', calculator: 'Calculator', clock: 'Clock', note: 'Note', folder: 'Folder',
};

/** One widget row's state: on the canvas, its count label and its toggle. */
export function widgetStatus(kind: WidgetName, snapshot: WidgetSnapshot): { on: number; label: string; action: 'Add' | 'Remove' } {
  const on = kind === 'note' ? snapshot.notes.filter((note) => note.on).length
    : kind === 'folder' ? snapshot.folders.filter((folder) => folder.on).length
      : snapshot.counts[kind];
  const of = kind === 'note' ? ` of ${snapshot.notes.length}` : kind === 'folder' ? ` of ${snapshot.folders.length}` : '';
  return { on, label: on || of ? `${on}${of} on canvas` : '', action: on > 0 ? 'Remove' : 'Add' };
}

function liveSnapshot(): WidgetSnapshot {
  return {
    counts: { weather: widgetCount('weather'), calculator: widgetCount('calculator'), clock: widgetCount('clock') },
    notes: noteEntries(),
    folders: folderEntries(),
  };
}

/** The Widgets section's five rows, built once (exported for the tests). */
export function buildWidgets(document_: Document): HTMLElement {
  const rows = WIDGET_KINDS.map((kind) => {
    const expandable = kind === 'note' || kind === 'folder';
    const row = el(document_, 'li', { class: `cv-lib-app cv-lib-widget${expandable ? '' : ' is-utility'}`, 'data-widget': kind, 'data-search': `${kind} ${WIDGET_ROWS[kind].toLowerCase()} /${kind} widget` },
      el(document_, 'div', { class: 'cv-lib-app-head' },
        el(document_, 'button', { type: 'button', class: 'cv-lib-expand', 'aria-expanded': 'false', 'data-lib-expand': '', ...(expandable ? {} : { tabindex: '-1' }) },
          el(document_, 'span', { class: 'cv-lib-chevron', 'aria-hidden': 'true' }),
          el(document_, 'span', { class: 'cv-lib-dot', 'data-lib-dot': '', 'aria-hidden': 'true' }),
          el(document_, 'span', { class: 'cv-lib-app-title' }, WIDGET_ROWS[kind]),
          el(document_, 'span', { class: 'cv-lib-on', 'data-lib-on': '' })),
        addressChip(document_, kind, { title: `Type /${kind} (or /widget ${kind}) · click to put it in the bar` }),
        el(document_, 'button', { type: 'button', class: 'cv-lib-all', 'data-widget-toggle': kind }, 'Add')),
      ...(expandable ? [el(document_, 'ul', { class: 'cv-lib-cards', 'data-widget-list': kind })] : []));
    return row;
  });
  return el(document_, 'section', { class: 'cv-lib-cat cv-lib-widgets', 'data-category': 'widgets', 'aria-label': 'Widgets' },
    el(document_, 'ul', { class: 'cv-lib-apps' }, ...rows));
}

/** Re-renders the Widgets section from `snapshot`: dots, counts, toggles, the notes and folders lists. */
export function renderWidgets(section: HTMLElement, snapshot: WidgetSnapshot): void {
  const document_ = section.ownerDocument;
  for (const kind of WIDGET_KINDS) {
    const row = section.querySelector<HTMLElement>(`[data-widget="${kind}"]`);
    if (!row) continue;
    const status = widgetStatus(kind, snapshot);
    setDot(row.querySelector('[data-lib-dot]'), status.on > 0);
    row.classList.toggle('has-on', status.on > 0);
    const label = row.querySelector('[data-lib-on]');
    if (label) label.textContent = status.label;
    const toggle = row.querySelector(`[data-widget-toggle="${kind}"]`);
    if (toggle) toggle.textContent = status.action;
    const list = row.querySelector(`[data-widget-list="${kind}"]`);
    if (!list) continue;
    const entries = kind === 'note'
      ? snapshot.notes.map((note) => ({ id: note.id, number: '', title: note.title, detail: 'note', on: note.on }))
      : snapshot.folders.map((folder) => ({ id: folder.id, number: String(folder.number), title: folder.title, detail: folder.detail, on: folder.on }));
    const items = entries.map((entry) => el(document_, 'li', {
      class: `cv-lib-card${entry.on ? ' is-on' : ''}`, [kind === 'note' ? 'data-note' : 'data-folder']: entry.id,
      'data-search': `${kind} ${entry.number} ${entry.title.toLowerCase()}`,
    },
    el(document_, 'button', {
      type: 'button', class: 'cv-lib-toggle', 'aria-pressed': String(entry.on), 'data-lib-toggle': '',
      title: entry.on ? 'On your canvas. Click to take it off (nothing is deleted).' : 'Bring onto the canvas',
    },
    el(document_, 'span', { class: 'cv-lib-dot', 'data-lib-dot': '', 'data-on': String(entry.on), 'aria-hidden': 'true' }),
    el(document_, 'span', { class: 'cv-lib-num' }, entry.number),
    el(document_, 'span', { class: 'cv-lib-card-title', title: entry.title }, entry.title),
    el(document_, 'span', { class: 'cv-lib-kind', 'data-kind': kind }, entry.detail),
    el(document_, 'span', { class: 'cv-lib-action', 'data-lib-action': '' }, entry.on ? 'Remove' : 'Add'))));
    const create = el(document_, 'li', { class: 'cv-lib-card cv-lib-new' },
      el(document_, 'button', { type: 'button', class: 'cv-lib-toggle', 'data-widget-new': kind },
        el(document_, 'span', { class: 'cv-lib-card-title' }, kind === 'note' ? '+ New note' : '+ New folder')));
    list.replaceChildren(...items, create);
  }
}

/** A click inside the Widgets section; true when it was one. */
function widgetClick(target: Element | null): boolean {
  const row = target?.closest<HTMLElement>('[data-widget]');
  const kind = row?.dataset.widget as WidgetName | undefined;
  if (!row || !kind) return false;
  if (target?.closest('.cv-addr-chip')) return true;
  if (target?.closest('[data-lib-expand]')) {
    if (kind === 'note' || kind === 'folder') {
      const open = !row.classList.contains('is-expanded');
      expand(row, open);
      row.classList.toggle('was-expanded', open);
    }
    return true;
  }
  const done = (message: Promise<string> | string) => {
    void Promise.resolve(message).then(announce);
    if (phone()) closeLibrary();
  };
  if (target?.closest('[data-widget-new]')) {
    done(kind === 'note' ? newNote('') : newFolder());
    return true;
  }
  if (target?.closest('[data-widget-toggle]')) {
    const status = widgetStatus(kind, liveSnapshot());
    if (status.action === 'Remove') {
      if (kind === 'note') for (const note of noteEntries().filter((entry) => entry.on)) toggleNote(note.id);
      else if (kind === 'folder') closeFolders();
      else removeWidgets(kind);
      announce(`${WIDGET_ROWS[kind]} removed from the canvas. Nothing was deleted.`);
    } else {
      done(kind === 'note' ? newNote('') : kind === 'folder' ? newFolder() : runWidget(kind, ''));
    }
    return true;
  }
  const note = target?.closest<HTMLElement>('[data-note]')?.dataset.note;
  if (note) {
    announce(toggleNote(note));
    return true;
  }
  const folder = target?.closest<HTMLElement>('[data-folder]')?.dataset.folder;
  if (folder) done(toggleFolder(folder));
  return true;
}

// --- The projection: every row's dot, counts and toggles from the board ---

const onBoard = (card: CardRow): boolean => isOnBoard(card.id);

/** Re-renders every row's on-canvas state (`placed` answers whether a card is on the canvas). */
export function syncLibrary(placed: (card: CardRow) => boolean = onBoard, snapshot: () => WidgetSnapshot = liveSnapshot): void {
  if (placed === onBoard) adoptNewWorks();
  for (const app of apps) {
    let on = 0;
    for (const card of app.cards) {
      const here = placed(card);
      if (here) on += 1;
      card.button.setAttribute('aria-pressed', String(here));
      card.row.classList.toggle('is-on', here);
      setDot(card.row.querySelector('[data-lib-dot]'), here);
      const action = card.button.querySelector('[data-lib-action]');
      if (action) action.textContent = here ? 'Remove' : 'Add';
      card.button.title = here ? 'On your canvas. Click to take it off (no data changes).' : 'Bring onto the canvas';
    }
    const status = appStatus(on, app.cards.length);
    setDot(app.row.querySelector('[data-lib-dot]'), on > 0);
    const label = app.row.querySelector('[data-lib-on]');
    if (label) label.textContent = status.label;
    app.row.classList.toggle('has-on', on > 0);
    const all = app.row.querySelector<HTMLElement>('[data-lib-all]');
    if (all) all.textContent = status.action;
  }
  const widgets = pane?.querySelector<HTMLElement>('[data-category="widgets"]');
  if (widgets) renderWidgets(widgets, snapshot());
}

/**
 * APP-120: the pane re-renders from the board's `canvas:changed` (after the
 * module that placed or removed something recorded it: a microtask later).
 */
export function watchCanvas(document_: Document, render: () => void = () => syncLibrary()): void {
  let queued = false;
  document_.addEventListener(CANVAS_CHANGED, () => {
    if (queued) return;
    queued = true;
    queueMicrotask(() => {
      queued = false;
      render();
      decorateAppCards(document_);
      labelCanvasCards(document_);
    });
  });
}

/** Filters the list by name, address or number ("3.2", "3", "ledger", "/expenses", "expenses.ledger"). */
function filter(query: string): void {
  const words = query.trim().toLowerCase();
  for (const section of pane?.querySelectorAll<HTMLElement>('.cv-lib-cat') ?? []) {
    let shown = 0;
    for (const row of section.querySelectorAll<HTMLElement>('.cv-lib-app')) {
      if (row.classList.contains('is-replaced')) continue;
      const app = apps.find((candidate) => candidate.row === row);
      const appHit = !words || (row.dataset.search ?? '').includes(words) || app?.number === words;
      let cardHits = 0;
      for (const card of row.querySelectorAll<HTMLElement>('.cv-lib-card')) {
        const number = card.dataset.number ?? '';
        const hit = !words || appHit || (card.dataset.search ?? '').includes(words) || (number !== '' && number.startsWith(words));
        card.hidden = !hit;
        if (hit) cardHits += 1;
      }
      row.hidden = !(appHit || cardHits);
      if (!row.hidden) shown += 1;
      if (words) expand(row, Boolean(cardHits) && !appHit);
    }
    section.hidden = shown === 0;
  }
  for (const group of pane?.querySelectorAll<HTMLElement>('[data-lib-group]') ?? []) {
    group.hidden = !group.querySelector('.cv-lib-cat:not([hidden])');
  }
  if (!words) for (const row of pane?.querySelectorAll<HTMLElement>('.cv-lib-app') ?? []) expand(row, row.classList.contains('was-expanded'));
  // An empty search says so, with a way out (APP-116 polish).
  const list = pane?.querySelector<HTMLElement>('[data-lib-list]');
  const any = Array.from(pane?.querySelectorAll<HTMLElement>('.cv-lib-cat') ?? []).some((section) => !section.hidden);
  let none = pane?.querySelector<HTMLElement>('.cv-lib-none') ?? null;
  if (!none && list && documentRef) {
    const clear = el(documentRef, 'button', { type: 'button', class: 'cv-lib-none-clear' }, 'Clear the search');
    clear.addEventListener('click', () => {
      if (search) search.value = '';
      filter('');
      search?.focus({ preventScroll: true });
    });
    none = el(documentRef, 'div', { class: 'cv-lib-none', role: 'status', hidden: '' },
      el(documentRef, 'p', { class: 'cv-lib-none-title' }, ''),
      el(documentRef, 'p', {}, 'Try an app’s /name, a card like expenses.ledger, or its number, like 3.2. Or describe what you need in the bar below.'),
      clear);
    list.before(none);
  }
  if (none) {
    none.hidden = any || !words;
    const title = none.querySelector('.cv-lib-none-title');
    if (title) title.textContent = `Nothing matches “${query.trim().slice(0, 40)}”`;
  }
}

async function toggleCard(card: CardRow, report?: Report): Promise<void> {
  const message = onBoard(card)
    ? (removeCards([card.id]), `${card.title} is back in the library.`)
    : await addCards(card.app.app, card.app.work, [card.id]);
  report?.(message);
  if (onBoard(card) && phone()) closeLibrary();
}

async function addAll(app: AppRow, report?: Report): Promise<string> {
  const message = app.cards.length && app.cards.every(onBoard)
    ? (removeCards(app.cards.map((card) => card.id)), `${app.title} is back in the library.`)
    : await addCards(app.app, app.work, null);
  report?.(message);
  if (phone() && app.cards.some(onBoard)) closeLibrary();
  return message;
}

// --- Finding cards and apps by name, address or number (`/card`, `/app`, autocomplete) ---

function score(text: string, query: string): number {
  const value = text.toLowerCase();
  if (value === query) return 90;
  if (value.startsWith(query)) return 70;
  if (value.split(/\s+/).some((word) => word.startsWith(query))) return 55;
  if (value.includes(query)) return 45;
  return 0;
}

/** The app `name` names exactly: its number, address or one of its aliases. */
function exactApp(name: string): AppRow | undefined {
  return apps.find((app) => app.number === name || app.slug === name || (app.address?.aliases ?? []).includes(name));
}

/** Cards by number (`3.2`, `3`), by address (`expenses.ledger`, `expense.ledger`, `3.ledger`) or by name. */
export function findCards(query: string): CardRow[] {
  const q = query.trim().toLowerCase().replace(/^[#/]/, '');
  const cards = apps.flatMap((app) => app.cards);
  if (!q) return cards.filter((card) => card.kind !== 'app').slice(0, 12);
  if (/^\d+(\.\d*)?$/.test(q)) {
    return cards.filter((card) => card.number === q || card.number.startsWith(q.includes('.') ? q : `${q}.`));
  }
  const dotted = q.match(/^([a-z0-9-]+)\.([a-z0-9-]*)$/);
  if (dotted) {
    const app = exactApp(dotted[1] ?? '');
    if (app) {
      const name = dotted[2] ?? '';
      const exact = app.cards.filter((card) => card.slug === name);
      return exact.length ? exact : app.cards.filter((card) => card.slug.startsWith(name));
    }
  }
  return cards
    .map((card) => ({ card, value: Math.max(score(card.title, q), score(`${card.app.title} ${card.title}`, q) - 10, score(cardAddress(card), q) - 5) }))
    .filter((entry) => entry.value > 0)
    .sort((a, b) => b.value - a.value)
    .map((entry) => entry.card);
}

export function findApps(query: string): AppRow[] {
  const q = query.trim().toLowerCase().replace(/^\//, '');
  if (!q) return [...apps];
  if (/^\d+$/.test(q)) return apps.filter((app) => app.number === q);
  const aliases = (app: AppRow) => app.address?.aliases ?? [];
  return apps
    .map((app) => ({
      app,
      value: app.slug === q || aliases(app).includes(q) ? 100
        : app.slug.startsWith(q) || aliases(app).some((alias) => alias.startsWith(q)) ? 60
          : Math.max(score(app.title, q), app.address ? score(app.address.hint, q) - 15 : 0),
    }))
    .filter((entry) => entry.value > 0)
    .sort((a, b) => b.value - a.value)
    .map((entry) => entry.app);
}

/** A placing command leaves the bar empty for the next one. */
function clearBar(): void {
  const input = documentRef?.getElementById('intent-input') as HTMLInputElement | null;
  if (input) input.value = '';
}

/** APP-118: `/apps [query]` opens the Library (filtered); it never places a canvas card. */
export const APPS_COMMAND = {
  slug: 'apps',
  title: 'Library',
  purpose: 'Open the Library, filtered by a name or number: every app and widget you can bring in',
  run: (text: string, report: Report = () => undefined) => {
    const query = text.trim();
    showLibrary(query);
    clearBar();
    report(query ? `The Library shows what matches “${query.slice(0, 40)}”. Click Add to place it.` : 'Every app you can open is in the Library (L). Click Add to place one.');
  },
};

export const CARD_COMMAND = {
  slug: 'card',
  title: 'Place a card',
  purpose: 'One card of an app, by app.card (expenses.ledger) or number (3.2)',
  run: async (text: string, report: Report = () => undefined) => {
    if (!text.trim()) {
      openLibrary();
      report('Pick a card in the library, or type /card and its app.card or number.');
      return;
    }
    const [card] = findCards(text);
    if (!card) {
      report(`No card matches “${text.trim()}”. Open the library (L) to browse.`);
      return;
    }
    report(isOnBoard(card.id) ? `${card.number} ${card.title} is already on your canvas.` : await addCards(card.app.app, card.app.work, [card.id]));
    clearBar();
  },
};

/** Places every card of `app` (the `/app` path, and a bare `/<address>`). */
async function placeApp(app: AppRow, report: Report): Promise<void> {
  report(await addCards(app.app, app.work, null));
  clearBar();
}

export const APP_COMMAND = {
  slug: 'app',
  title: 'Place an app',
  purpose: 'Every card of one app, by its /name or number (/expenses is the same)',
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
    await placeApp(app, report);
  },
};

/** APP-120: `/focus [card]`: the selected card in focus, or the card named (placed first when it is not on the canvas). */
export const FOCUS_COMMAND = {
  slug: 'focus',
  title: 'Focus a card',
  purpose: 'One card full size: the selected one, or one by app.card or number (F)',
  run: async (text: string, report: Report = () => undefined) => {
    const words = text.trim();
    clearBar();
    if (!words) {
      report(focusFromItem(activeCanvasItem()) ? 'In focus. Esc returns to the canvas.' : 'Select a card first, or type /focus and its app.card or number.');
      return;
    }
    const [card] = findCards(words);
    if (!card) {
      report(`No card matches “${words}”. Open the library (L) to browse.`);
      return;
    }
    if (!isOnBoard(card.id)) await addCards(card.app.app, card.app.work, [card.id], { focus: false });
    if (!openFocus(card.id)) report(`${card.title} could not be opened in focus.`);
  },
};

/** `/card <app.card>` (and `/focus`): the matching cards, each by its `app.card` name and number. */
export function cardCompletions(rest: string, verb = 'card'): Completion[] {
  return findCards(rest).slice(0, 10).map((card) => ({
    badge: card.number,
    title: cardAddress(card),
    detail: `${card.title} · ${card.app.title}${isOnBoard(card.id) ? ' · on canvas' : ''}`,
    value: `/${verb} ${cardAddress(card)}`,
  }));
}
/** `/apps <query>`: open the Library filtered (first), then the matching apps; Enter on one places it. */
function appsCompletions(rest: string): Completion[] {
  const query = rest.trim();
  return [
    {
      badge: 'L', title: query ? `Open the Library filtered by “${query.slice(0, 40)}”` : 'Open the Library',
      detail: 'Every app, card and widget you can bring in', value: query ? `/apps ${query}` : '/apps',
    },
    ...findApps(query).slice(0, 9).map((app) => ({
      badge: app.number,
      title: app.title,
      detail: `${app.slug ? `/${app.slug} · ` : ''}${app.address ? `${app.address.hint} · ` : ''}place it · ${app.cards.length} cards${app.cards.length && app.cards.every(onBoard) ? ' · on canvas' : ''}`,
      value: `/app ${app.slug || app.number}`,
    })),
  ];
}
function appCompletions(rest: string): Completion[] {
  return findApps(rest).slice(0, 10).map((app) => ({
    badge: app.number,
    title: app.title,
    detail: `${app.slug ? `/${app.slug} · ` : ''}${app.cards.length} cards`,
    value: `/app ${app.slug || app.number}`,
  }));
}

// --- APP-119/120: every app row shows its address chip, aliases, hint and an example ---

/** The address a row answers to: a saved app by its work, a library app by its slug. */
export function rowAddress(row: { slug: string; work: string; widget?: unknown }, list: Address[]): Address | undefined {
  if (row.widget) return undefined;
  return list.find((address) => address.kind === 'saved' && address.work === row.work)
    ?? (row.slug ? list.find((address) => address.kind === 'library' && (address.slug === row.slug || address.aliases.includes(row.slug))) : undefined);
}

/** The first example that says more than the bare address. */
export function firstExample(address: Address): string {
  return address.examples.find((example) => example.trim().includes(' ')) ?? address.examples[0] ?? `/${address.slug}`;
}

/** Shows `address` on the app's row: the chip next to the title, then "also /expense", the hint and an example (exported for the tests). */
export function decorate(app: AppRow, address: Address | undefined): void {
  const document_ = app.row.ownerDocument;
  if (app.address === address) return;
  app.address = address;
  app.row.querySelector('.cv-lib-address')?.remove();
  const base = app.row.dataset.searchBase ?? app.row.dataset.search ?? '';
  app.row.dataset.searchBase = base;
  if (!address) {
    app.row.dataset.search = base;
    return;
  }
  app.slug = address.slug;
  if (address.kind === 'library' && !app.row.querySelector('[data-lib-edit=""]')) {
    app.row.querySelector('[data-lib-expand]')?.after(el(document_, 'button', { type: 'button', class: 'cv-lib-edit', 'data-lib-edit': '', title: `Name ${address.title}`, 'aria-label': `Name ${address.title}` }, '✎'));
  }
  // The plain `/slug` the server rendered becomes the address chip (one component everywhere).
  const head = app.row.querySelector('.cv-lib-app-head');
  const plain = app.row.querySelector<HTMLElement>('.cv-lib-slug');
  const chip = addressChip(document_, address.slug);
  chip.classList.add('cv-lib-slug');
  if (plain) plain.replaceWith(chip);
  else app.row.querySelector('[data-lib-expand]')?.after(chip);
  nameCards(app);
  app.row.dataset.search = `${base} ${addressText(address)}`;
  const example = firstExample(address);
  const tryIt = el(document_, 'button', { type: 'button', class: 'cv-lib-example', 'data-lib-example': '', title: 'Put this example in the bar' }, el(document_, 'code', {}, example));
  tryIt.addEventListener('click', (event) => {
    (event as Event).stopPropagation?.();
    insertCommand(example);
  });
  const line = el(document_, 'p', { class: 'cv-lib-address' },
    aliasNote(document_, address.aliases),
    el(document_, 'span', { class: 'cv-lib-addr-hint' }, address.hint),
    tryIt);
  head?.after(line);
}

/** Decorates every row from the member's addresses (they load after the page). */
function applyAddresses(): void {
  const list = addressList();
  for (const app of apps) {
    const address = rowAddress(app, list);
    // A new labels list is a new address object: decorate again.
    if (app.address && address && app.address !== address) app.address = undefined;
    decorate(app, address);
  }
  applyLabels(list);
  if (documentRef) {
    decorateAppCards(documentRef);
    labelCanvasCards(documentRef);
  }
  if (search?.value) filter(search.value);
}

/** APP-121: one title element shows `name`, remembering the server's own the first time. */
function retitle(node: HTMLElement | null, name: string, description = ''): void {
  if (!node) return;
  node.dataset.baseTitle ??= node.textContent ?? '';
  const shown = name || node.dataset.baseTitle || '';
  if (node.textContent !== shown) node.textContent = shown;
  node.setAttribute('title', description || shown);
}

/** APP-121: the app's and cards' labels on the Library rows (exported for the tests). */
export function applyLabels(list: Address[] = addressList()): void {
  for (const app of apps) {
    const address = app.address ?? rowAddress(app, list);
    if (!address) continue;
    retitle(app.row.querySelector<HTMLElement>('.cv-lib-app-title'), address.title, address.description);
    app.title = address.title;
    for (const card of app.cards) {
      card.base ??= card.title;
      const label = (address.cards ?? []).find((entry) => entry.card === card.slug);
      card.title = cardLabel(address, card.slug, card.base);
      retitle(card.row.querySelector<HTMLElement>('.cv-lib-card-title'), card.title, label?.description);
      card.row.dataset.search = `${card.number} ${card.title.toLowerCase()} ${card.kind} ${cardAddress(card)}`;
    }
  }
}

/** APP-121: the cards on the canvas show their labels: the app card the app's display name, a card its label. */
export function labelCanvasCards(document_: Document, list: Address[] = addressList()): void {
  const placed = new Map<string, HTMLElement>();
  for (const node of document_.querySelectorAll<HTMLElement>('[data-card]')) {
    if (node.classList.contains('cv-card') && node.dataset.card) placed.set(node.dataset.card, node);
  }
  if (!placed.size) return;
  for (const app of apps) {
    const address = app.address ?? rowAddress(app, list);
    if (!address) continue;
    for (const card of app.cards) {
      const node = placed.get(card.id);
      if (!node) continue;
      const appCard = card.id.startsWith('a-');
      // The APP-120 gap: whichever path placed it, the app card knows its
      // app and shows its address chip.
      if (!node.dataset.app) {
        node.dataset.app = app.app;
        node.dataset.work = app.work;
      }
      if (appCard) decorateAppCard(node, list);
      const heading = appCard ? node.querySelector<HTMLElement>('h2') : node.querySelector<HTMLElement>('h3');
      const label = (address.cards ?? []).find((entry) => entry.card === card.slug);
      const name = appCard ? address.title : cardLabel(address, card.slug, '');
      retitle(heading, name, appCard ? address.description : label?.description);
      if (name) node.setAttribute('aria-label', name);
    }
  }
}

/** Groups the server's categories under "Apps" and adds the "Widgets" section. */
function sections(document_: Document): void {
  const list = pane?.querySelector<HTMLElement>('[data-lib-list]');
  if (!list) return;
  const categories = Array.from(list.querySelectorAll<HTMLElement>('.cv-lib-cat'));
  const appsGroup = el(document_, 'div', { class: 'cv-lib-group', 'data-lib-group': 'apps' },
    el(document_, 'h2', { class: 'cv-lib-group-title' }, 'Apps'), ...categories);
  const widgetsGroup = el(document_, 'div', { class: 'cv-lib-group', 'data-lib-group': 'widgets' },
    el(document_, 'h2', { class: 'cv-lib-group-title' }, 'Widgets'), buildWidgets(document_));
  list.replaceChildren(appsGroup, widgetsGroup);
}

/** Wires the pane once (after the board started). */
export function startLibrary(document_: Document): void {
  if (pane) return;
  documentRef = document_;
  startPane(document_);
  startCommandSheet(document_);
  startShortcutTab(document_);
  pane = document_.getElementById('cv-library');
  search = document_.getElementById('cv-lib-search') as HTMLInputElement | null;
  if (!pane) return;
  onPaneTab('library', {
    show: (focus) => {
      syncLibrary();
      void ensureCommands();
      if (focus && matchMedia('(pointer: fine)').matches) search?.focus({ preventScroll: true });
    },
    // Esc clears the search first, then closes the pane.
    escape: () => {
      if (!search?.value) return false;
      search.value = '';
      filter('');
      return true;
    },
  });
  sections(document_);
  for (const row of pane.querySelectorAll<HTMLElement>('.cv-lib-app')) {
    if (!row.dataset.widget) indexRow(row);
  }
  // Long names are cut with an ellipsis: the full name shows on hover.
  for (const title of pane.querySelectorAll<HTMLElement>('.cv-lib-app-title, .cv-lib-card-title')) title.title ||= title.textContent ?? '';
  replaceNotesApp();
  // Small libraries open expanded; a large one shows its apps first.
  if (apps.length <= 3) for (const app of apps) expand(app.row, true);
  document_.getElementById('cv-library-toggle')?.addEventListener('click', () => toggleLibrary());
  document_.addEventListener('click', (event) => {
    const target = event.target as Element | null;
    if (!pane?.contains(target)) {
      if (target?.closest('[data-lib-open]')) openLibrary();
      return;
    }
    if (target?.closest('[data-lib-close]')) {
      closeLibrary();
      return;
    }
    if (widgetClick(target)) return;
    const row = target?.closest<HTMLElement>('.cv-lib-app');
    const app = row && apps.find((candidate) => candidate.row === row);
    if (!app || target?.closest('.cv-addr-chip')) return;
    const edit = target?.closest<HTMLElement>('[data-lib-edit]');
    if (edit) {
      labelEditor?.(app, app.cards.find((card) => card.id === edit.dataset.libEdit) ?? null);
      return;
    }
    if (target?.closest('[data-lib-expand]')) {
      const open = !app.row.classList.contains('is-expanded');
      expand(app.row, open);
      app.row.classList.toggle('was-expanded', open);
      return;
    }
    if (target?.closest('[data-lib-stack]')) {
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
  watchCanvas(document_);
  onAddressesChange(applyAddresses);
  applyAddresses();
  // APP-121 (and the APP-120 gap): labels and the app card's chip without opening the pane.
  void ensureCommands();
  registerCommands([APPS_COMMAND, CARD_COMMAND, APP_COMMAND, FOCUS_COMMAND]);
  registerCompleter('apps', appsCompletions);
  registerCompleter('card', (rest) => cardCompletions(rest));
  registerCompleter('focus', (rest) => cardCompletions(rest, 'focus'));
  registerCompleter('app', appCompletions);
  // APP-120: a bare `/<address>` places the app, as `/app <address>` does.
  setAppPlacer(async (address, report) => {
    const app = apps.find((candidate) => (address.kind === 'saved' ? candidate.work === address.work : candidate.slug === address.slug || candidate.app === address.app));
    if (!app) return false;
    await placeApp(app, report);
    return true;
  });
  syncLibrary();
}
