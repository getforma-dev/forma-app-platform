// Library projects authorized work metadata into Apps, Widgets and Cards.
// Apps have one face/stack/open view; their components remain inert metadata
// for naming and Ask. Only an explicit member-local category marks a whole
// work as a standalone Card. No classification is inferred from its content.
// Placement always loads the whole work through the authorized board route.
import { addressChip, aliasNote, decorateAppCard, decorateAppCards } from './address-chip';
import { activeCanvasItem, boardWorks, CANVAS_CHANGED, canvasApi, cardFootprint, isStandaloneWork, removeWork, setStandaloneWork, updateWorkTitle } from './board';
import { bindLibraryDrag } from './library-drag';
import { addressList, addressText, allAddresses, appForm, cardLabel, onAddressesChange, WIDGET_KINDS, type Address, type WidgetName } from './command-registry';
import { insertCommand, startCommandSheet } from './command-sheet';
import { startShortcutTab } from './shortcut-tab';
import { closePane, isPaneOpen, onPaneTab, openPane, startPane, togglePane } from './pane';
import { appViewForWork, setAppView, type AppViewMode } from './unite';
import { ensureCommands, registerCommands, registerCompleter, setAppPlacer, type Completion } from './slash';
import { announce, el, isUuid } from './render';
import { removeWidgets, runWidget, widgetCount } from './widgets/index';
import { newNote, noteEntries, toggleNote } from './notes';
import { closeFolders, fileCardEntries, folderEntries, newFolder, toggleFileCard, toggleFolder } from './boxes';
import { focusFromItem } from './focus';

type Report = (message: string) => void;
export type CardRow = {
  id: string; number: string; title: string; kind: string; canvasKind: string; slug: string; row: HTMLElement; button: HTMLButtonElement; app: AppRow;
  /** APP-121: the title the server rendered (`title` is the label shown now). */
  base?: string;
};
export type AppRow = {
  app: string; work: string; number: string; title: string; slug: string;
  workNameScope?: boolean;
  category?: string;
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
export type SavedCardEntry = {id: string; title: string; kind: 'card'|'answer'; createdAt: string};
let savedEntries: readonly SavedCardEntry[] = [];
let showSaved: ((id: string) => Promise<string>) | null = null;
let refreshSaved: (() => Promise<string>) | null = null;
let savedMessage = 'Loading saved cards and answers…';
/** Talk owns the authorized inventory and placement; Library keeps metadata only. */
export function setSavedCardInventory(entries: readonly SavedCardEntry[], show: (id: string) => Promise<string>, message = '', refresh: (() => Promise<string>) | null = null): void {
  const unchanged = showSaved === show && refreshSaved === refresh && savedMessage === message && entries.length === savedEntries.length
    && entries.every((entry, index) => {const before = savedEntries[index]; return before?.id === entry.id && before.title === entry.title && before.kind === entry.kind && before.createdAt === entry.createdAt;});
  savedEntries = entries; showSaved = show; savedMessage = message; refreshSaved = refresh;
  if (unchanged) return;
  const section = pane?.querySelector<HTMLElement>('[data-category="saved-cards"]');
  if (section) renderSavedCards(section, entries, show, message, refresh);
  filter(search?.value ?? '');
}
/** True saved artifacts have their own card_id, never their parent work's ID. */
export function renderSavedCards(section: HTMLElement, entries: readonly SavedCardEntry[], show: (id: string) => Promise<string>, message = '', refresh: (() => Promise<string>) | null = null): void {
  const document_ = section.ownerDocument;
  const status = el(document_, 'p', {class: 'cv-lib-hint', role: 'status'}, message || (entries.length ? 'Saved cards read live data. Pinned answers retain what was answered at the time.' : 'No saved cards or pinned answers yet.'));
  const list = el(document_, 'ul', {class: 'cv-lib-apps'}, ...entries.map(entry => {
    const date = new Date(entry.createdAt);
    const kind = entry.kind === 'answer' ? `Pinned answer · historical${Number.isNaN(date.valueOf()) ? '' : ` · ${date.toISOString().slice(0, 10)}`}` : 'Saved card · live data';
    const button = el(document_, 'button', {type: 'button', class: 'cv-lib-all', 'data-saved-show': entry.id, 'aria-label': `Show ${entry.title}`}, 'Show') as HTMLButtonElement;
    button.addEventListener('click', () => {
      if (button.disabled) return;
      button.disabled = true;
      status.textContent = 'Opening saved card…';
      void show(entry.id).then(message => {status.textContent = message; announce(message);}, () => {status.textContent = 'That saved card could not be shown. Try again.';}).finally(() => {button.disabled = false;});
    });
    return el(document_, 'li', {class: 'cv-lib-app cv-lib-saved-card', 'data-saved-card': entry.id, 'data-search': `${entry.title} ${kind}`.toLowerCase()},
      el(document_, 'div', {class: 'cv-lib-app-head'}, el(document_, 'div', {class: 'cv-lib-title-group'}, el(document_, 'span', {class: 'cv-lib-app-title'}, entry.title)), button),
      el(document_, 'p', {class: 'cv-lib-hint'}, kind));
  }));
  const reload = el(document_, 'button', {type: 'button', class: 'cv-secondary', 'data-saved-refresh': ''}, 'Refresh saved cards') as HTMLButtonElement;
  reload.disabled = refresh === null;
  reload.addEventListener('click', () => {
    if (!refresh || reload.disabled) return;
    reload.disabled = true; status.textContent = 'Refreshing saved cards…';
    void refresh().then(message => {status.textContent = message; announce(message);}, () => {status.textContent = 'Saved cards could not be refreshed. Try again.';}).finally(() => {reload.disabled = false;});
  });
  section.replaceChildren(el(document_, 'h3', {class: 'cv-lib-cat-title'}, 'Saved cards and answers'), reload, status, list);
}

export type FileCardEntry = {boxId: string; fileId: string; title: string; detail: string; on: boolean};
const pendingFileCards = new Set<string>();
const fileCardRenders = new WeakMap<HTMLElement, {signature: string; toggle: (boxId: string, fileId: string) => Promise<string>}>();

/** Uploaded file inventory is independent of canvas placement. Closing a file
 * card changes its toggle, never removes the authorized file from this list. */
export function renderFileCards(section: HTMLElement, entries: readonly FileCardEntry[], toggle = toggleFileCard): void {
  const signature = JSON.stringify(entries);
  const previous = fileCardRenders.get(section);
  if (previous?.signature === signature && previous.toggle === toggle) return;
  fileCardRenders.set(section, {signature, toggle});
  const document_ = section.ownerDocument;
  const status = el(document_, 'p', {class: 'cv-lib-hint', role: 'status', 'data-file-card-status': ''}, entries.length
    ? 'Your uploaded files. Removing a card from the canvas keeps its file here.'
    : 'Upload a CSV or Excel file to add its card here.');
  const list = el(document_, 'ul', {class: 'cv-lib-apps'}, ...entries.map(entry => {
    const key = `${entry.boxId}/${entry.fileId}`;
    const button = el(document_, 'button', {
      type: 'button', class: 'cv-lib-all', 'data-file-card-toggle': key,
      'aria-pressed': String(entry.on), 'aria-label': `${entry.on ? 'Remove' : 'Add'} ${entry.title} ${entry.on ? 'from' : 'to'} canvas`,
      title: entry.on ? 'Remove from the canvas. The uploaded file stays in Cards.' : 'Bring this file card onto the canvas',
    }, entry.on ? 'Remove' : 'Add') as HTMLButtonElement;
    button.disabled = pendingFileCards.has(key);
    button.addEventListener('click', () => {
      if (pendingFileCards.has(key)) return;
      pendingFileCards.add(key); button.disabled = true;
      status.textContent = entry.on ? 'Removing file card from canvas…' : 'Opening file card…';
      const report = (message: string) => {
        const current = section.querySelector('[data-file-card-status]');
        if (current) current.textContent = message;
        announce(message);
      };
      void toggle(entry.boxId, entry.fileId).then(message => {
        report(message);
      }, () => {
        report('That file card could not be changed. Try again.');
      }).finally(() => {
        pendingFileCards.delete(key);
        button.disabled = false;
        // A canvas change may have replaced the row while placement was pending.
        const current = section.querySelector<HTMLButtonElement>(`[data-file-card-toggle="${key}"]`);
        if (current) current.disabled = false;
      });
    });
    return el(document_, 'li', {
      class: `cv-lib-app cv-lib-saved-card cv-lib-file-card${entry.on ? ' has-on' : ''}`,
      'data-file-card': key, 'data-search': `${entry.title} ${entry.detail} file card`.toLowerCase(),
    },
    el(document_, 'div', {class: 'cv-lib-app-head'},
      el(document_, 'div', {class: 'cv-lib-title-group'},
        el(document_, 'span', {class: 'cv-lib-dot', 'data-lib-dot': '', 'data-on': String(entry.on), 'aria-hidden': 'true'}),
        el(document_, 'span', {class: 'cv-lib-app-title', title: entry.title}, entry.title),
        el(document_, 'span', {class: 'cv-lib-on', 'data-lib-on': ''}, entry.on ? 'On canvas' : '')), button),
    el(document_, 'p', {class: 'cv-lib-hint'}, entry.detail));
  }));
  section.replaceChildren(el(document_, 'h3', {class: 'cv-lib-cat-title'}, 'Uploaded files'), status, list);
}

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

/** One whole-work view, rather than a count of independently placed parts. */
export function appStatus(mode: AppViewMode | null): { dot: 'filled' | 'hollow'; label: string } {
  return { dot: mode ? 'filled' : 'hollow', label: mode ? ({ face: 'App Card view', stack: 'Stack view', open: 'Open view' })[mode] : '' };
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
    workNameScope: row.dataset.workNameScope === 'true',
    category: row.dataset.libraryCategory || row.closest?.<HTMLElement>('.cv-lib-cat')?.dataset.category || 'mine',
    number: row.dataset.number ?? '',
    title: row.querySelector('.cv-lib-app-title')?.textContent ?? '',
    slug: (row.querySelector('.cv-lib-slug')?.textContent ?? '').replace(/^\/(app:)?/, ''),
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
      canvasKind: item.dataset.canvasKind || kind,
      slug: cardSlug(id, kind, work, taken),
      row: item,
      button,
      app,
    });
  }
  row.dataset.libraryCategory = app.category;
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
  entry: { app: string; work: string; number: string; title: string; slug: string; workNameScope?: boolean; cards: { id: string; number: string; title: string; kind: string; canvasKind?: string }[] },
): HTMLElement {
  const head = el(document_, 'div', { class: 'cv-lib-app-head' },
    el(document_, 'div', { class: 'cv-lib-title-group' },
      el(document_, 'span', { class: 'cv-lib-dot', 'data-lib-dot': '', 'aria-hidden': 'true' }),
      el(document_, 'span', { class: 'cv-lib-num' }, entry.number),
      el(document_, 'span', { class: 'cv-lib-app-title', title: entry.title }, entry.title),
      el(document_, 'span', { class: 'cv-lib-on', 'data-lib-on': '' })),
    ...(entry.slug ? [el(document_, 'code', { class: 'cv-lib-slug cv-addr-chip' }, `/${entry.slug}`)] : []),
    el(document_, 'div', { class: 'cv-lib-view-actions' },
      el(document_, 'button', { type: 'button', class: 'cv-lib-all', 'data-lib-all': '', title: 'Add the whole app as its face card' }, 'Add'),
      el(document_, 'button', { type: 'button', class: 'cv-lib-stack', 'data-lib-stack': '', title: 'Show the whole app in Stack view' }, 'Stack'),
      el(document_, 'button', { type: 'button', class: 'cv-lib-stack cv-lib-open-view', 'data-lib-open-view': '', title: 'Open the whole app on the canvas' }, 'Open'),
      el(document_, 'details', { class: 'cv-lib-menu' },
        el(document_, 'summary', { 'aria-label': `Options for ${entry.title}`, title: 'Library options' }, '⋯'),
        el(document_, 'div', { class: 'cv-lib-menu-items' },
          el(document_, 'button', { type: 'button', 'data-lib-classify': '', title: 'Personal category in this browser; the whole saved work stays intact' }, 'Show in Cards'),
          el(document_, 'button', { type: 'button', 'data-lib-remove': '', hidden: '' }, 'Remove from canvas')))));
  const list = el(document_, 'ul', { class: 'cv-lib-cards cv-lib-metadata', 'data-lib-metadata': '', hidden: '', 'aria-hidden': 'true' },
    ...entry.cards.map((card) => el(document_, 'li', {
      class: 'cv-lib-card', 'data-card': card.id, 'data-number': card.number,
      ...(card.canvasKind ? { 'data-canvas-kind': card.canvasKind } : {}),
      'data-search': `${card.number} ${card.title.toLowerCase()} ${card.kind}`,
    },
    el(document_, 'button', { type: 'button', class: 'cv-lib-toggle', 'aria-pressed': 'false', 'data-lib-toggle': '', tabindex: '-1' },
      el(document_, 'span', { class: 'cv-lib-dot', 'data-lib-dot': '', 'aria-hidden': 'true' }),
      el(document_, 'span', { class: 'cv-lib-num' }, card.number),
      el(document_, 'span', { class: 'cv-lib-card-title', title: card.title }, card.title),
      el(document_, 'span', { class: 'cv-lib-kind', 'data-kind': card.kind }, card.kind),
      el(document_, 'span', { class: 'cv-lib-action', 'data-lib-action': '' }, 'Add')))));
  const row = el(document_, 'li', {
    class: 'cv-lib-app', 'data-app': entry.app, 'data-work': entry.work, 'data-number': entry.number, 'data-work-name-scope': String(entry.workNameScope === true),
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
  if (label) label.textContent = `${count} ${section.dataset.category === 'standalone' ? (count === 1 ? 'card' : 'cards') : (count === 1 ? 'app' : 'apps')}`;
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
      app: work.app, work: work.work, number, title: work.title, slug: '', workNameScope: work.workNameScope,
      cards: work.cards.filter((card) => card.id).map((card, at) => ({ ...card, number: `${number}.${at}` })),
    });
    const section = category(documentRef, 'mine', 'My apps');
    section.querySelector('.cv-lib-apps')?.append(row);
    recount(section);
    const indexed = indexRow(row);
    if (indexed) decorate(indexed, rowAddress(indexed, allAddresses()));
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
  const total = kind === 'note' ? snapshot.notes.length : kind === 'folder' ? snapshot.folders.length : 0;
  const of = total ? ` of ${total}` : '';
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

/** Move only explicit, member-local classifications. Unknown works stay Apps. */
function classifyRows(): void {
  const cards = pane?.querySelector<HTMLElement>('[data-category="standalone"] .cv-lib-apps');
  if (!cards || !documentRef) return;
  for (const app of apps) {
    const standalone = isStandaloneWork(app.app, app.work);
    app.row.dataset.standalone = String(standalone);
    if (standalone) app.row.setAttribute('data-lib-entry-drag', '');
    else app.row.removeAttribute('data-lib-entry-drag');
    const home = category(documentRef, app.category || 'mine', app.category === 'mine' ? 'My apps' : app.category || 'Apps');
    const target = standalone ? cards : home?.querySelector('.cv-lib-apps');
    if (target && app.row.parentElement !== target) target.append(app.row);
    const classify = app.row.querySelector('[data-lib-classify]');
    if (classify) classify.textContent = standalone ? 'Show in Apps' : 'Show in Cards';
    for (const button of app.row.querySelectorAll<HTMLElement>('[data-lib-stack], [data-lib-open-view]')) button.hidden = standalone;
    const add = app.row.querySelector<HTMLElement>('[data-lib-all]');
    if (add) add.title = standalone ? 'Add this whole card work to the canvas' : 'Add the whole app as its face card';
  }
  for (const section of pane?.querySelectorAll<HTMLElement>('.cv-lib-cat') ?? []) {
    if (section.dataset.category !== 'widgets') recount(section);
  }
}

/** Library status comes from the single whole-work view controller. */
export function syncLibrary(view: (work: string) => { mode: AppViewMode } | null = appViewForWork, snapshot: () => WidgetSnapshot = liveSnapshot): void {
  if (view === appViewForWork) adoptNewWorks();
  classifyRows();
  for (const app of apps) {
    const current = view(app.work);
    const status = appStatus(current?.mode ?? null);
    setDot(app.row.querySelector('[data-lib-dot]'), Boolean(current));
    const label = app.row.querySelector('[data-lib-on]');
    if (label) label.textContent = status.label;
    app.row.classList.toggle('has-on', Boolean(current));
    const remove = app.row.querySelector<HTMLElement>('[data-lib-remove]');
    if (remove) remove.hidden = !current;
    app.row.querySelector('[data-lib-all]')?.setAttribute('aria-pressed', String(current?.mode === (isStandaloneWork(app.app, app.work) ? 'open' : 'face')));
  }
  const widgets = pane?.querySelector<HTMLElement>('[data-category="widgets"]');
  if (widgets) renderWidgets(widgets, snapshot());
  const files = pane?.querySelector<HTMLElement>('[data-category="file-cards"]');
  if (files) renderFileCards(files, fileCardEntries());
  filter(search?.value ?? '');
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
        if (!app) card.hidden = !hit;
        if (hit) cardHits += 1;
      }
      row.hidden = !(appHit || cardHits);
      if (!row.hidden) shown += 1;
      if (words && !app) expand(row, Boolean(cardHits) && !appHit);
    }
    section.hidden = shown === 0 && (Boolean(words) || !['standalone', 'saved-cards', 'file-cards'].includes(section.dataset.category ?? ''));
  }
  for (const group of pane?.querySelectorAll<HTMLElement>('[data-lib-group]') ?? []) {
    group.hidden = Boolean(words) && !group.querySelector('.cv-lib-cat:not([hidden])');
  }
  if (!words) for (const row of pane?.querySelectorAll<HTMLElement>('.cv-lib-widget') ?? []) expand(row, row.classList.contains('was-expanded'));
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
      el(documentRef, 'p', {}, 'Try an app, a standalone card or a widget name. Or describe what you need in the bar below.'),
      clear);
    list.before(none);
  }
  if (none) {
    none.hidden = any || !words;
    const title = none.querySelector('.cv-lib-none-title');
    if (title) title.textContent = `Nothing matches “${query.trim().slice(0, 40)}”`;
  }
}

async function addAll(app: AppRow, report?: Report): Promise<string> {
  const message = await setAppView(app.app, app.work, isStandaloneWork(app.app, app.work) ? 'open' : 'face');
  report?.(message);
  if (phone()) closeLibrary();
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

/** Placement candidates only: exact classified whole works, never app parts. */
export function findStandaloneWorks(query: string): AppRow[] {
  const candidates = query.trim() ? [...findApps(query), ...findCards(query).map(card => card.app)] : apps;
  const seen = new Set<string>();
  return candidates.filter(app => {
    const key = `${app.app}/${app.work}`;
    if (!isStandaloneWork(app.app, app.work) || seen.has(key)) return false;
    seen.add(key);
    return true;
  });
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
  purpose: 'Place a whole standalone card from the Cards section',
  run: async (text: string, report: Report = () => undefined) => {
    if (!text.trim()) {
      openLibrary();
      report('Pick a whole card in Library → Cards. App sections are opened with their app.');
      return;
    }
    const [app] = findStandaloneWorks(text);
    if (!app) {
      report(`No card matches “${text.trim()}”. Open the library (L) to browse.`);
      return;
    }
    report(await setAppView(app.app, app.work, 'open'));
    clearBar();
  },
};

/** Places the whole app view (`/app` and a bare `/<address>`). */
async function placeApp(app: AppRow, report: Report): Promise<void> {
  report(await setAppView(app.app, app.work, isStandaloneWork(app.app, app.work) ? 'open' : 'face'));
  clearBar();
}

export const APP_COMMAND = {
  slug: 'app',
  title: 'Place an app',
  purpose: 'Add an app’s face card by its /name or number',
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
  title: 'Focus an app or card',
  purpose: 'Focus a whole app or standalone card by name or number (F)',
  run: async (text: string, report: Report = () => undefined) => {
    const words = text.trim();
    clearBar();
    if (!words) {
      report(focusFromItem(activeCanvasItem()) ? 'In focus. Esc returns to the canvas.' : 'Select an app or card first, or type /focus and its name or number.');
      return;
    }
    const app = findApps(words)[0] ?? findStandaloneWorks(words)[0];
    if (!app) {
      report(`No app or standalone card matches “${words}”. Open the library (L) to browse.`);
      return;
    }
    const existing = appViewForWork(app.work);
    if (!existing || existing.app !== app.app) await setAppView(app.app, app.work, isStandaloneWork(app.app, app.work) ? 'open' : 'face', { focus: false });
    const whole = appViewForWork(app.work);
    if (!whole || !focusFromItem(whole.item)) report(`${app.title} could not be opened in focus.`);
  },
};

/** Commands offer whole works only. Metadata aliases never expose app parts. */
export function cardCompletions(rest: string, verb = 'card'): Completion[] {
  const candidates = verb === 'focus' ? findApps(rest) : findStandaloneWorks(rest);
  return candidates.slice(0, 10).map((app) => ({
    badge: app.number,
    title: app.title,
    detail: `${app.slug ? `/${app.slug} · ` : ''}${isStandaloneWork(app.app, app.work) ? 'standalone card' : 'whole app'}${appViewForWork(app.work) ? ' · on canvas' : ''}`,
    value: `/${verb} ${app.slug || app.number}`,
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
      detail: `${app.slug ? `/${app.slug} · ` : ''}${app.address ? `${app.address.hint} · ` : ''}place it ${appViewForWork(app.work) ? ' · on canvas' : ''}`,
      value: `/app ${app.slug || app.number}`,
    })),
  ];
}
function appCompletions(rest: string): Completion[] {
  return findApps(rest).slice(0, 10).map((app) => ({
    badge: app.number,
    title: app.title,
    detail: `${app.slug ? `/${app.slug} · ` : ''}${isStandaloneWork(app.app, app.work) ? 'standalone card' : 'whole app'}`,
    value: `/app ${app.slug || app.number}`,
  }));
}

// --- APP-119/120: every app row shows its address chip, aliases, hint and an example ---

/** The address a row answers to: a saved app by its work, a library app by its slug. */
export function rowAddress(row: { slug: string; work: string; widget?: unknown; workNameScope?: boolean }, list: Address[]): Address | undefined {
  if (row.widget) return undefined;
  return list.find((address) => address.kind === 'saved' && address.work === row.work)
    ?? (!row.workNameScope && row.slug ? list.find((address) => address.kind === 'library' && (address.slug === row.slug || address.aliases.includes(row.slug))) : undefined);
}

/** The first example that says more than the bare address. */
export function firstExample(address: Address): string {
  return address.examples.find((example) => example.trim().includes(' ')) ?? address.examples[0] ?? `/${address.slug}`;
}

/** Shows `address` on the app's row: the chip next to the title, then "also /expense", the hint and an example (exported for the tests). */
export function decorate(app: AppRow, address: Address | undefined): void {
  const document_ = app.row.ownerDocument;
  if (app.workNameScope && !app.row.querySelector('[data-lib-edit=""]')) {
    app.row.querySelector('.cv-lib-title-group')?.after(el(document_, 'button', { type: 'button', class: 'cv-lib-edit', 'data-lib-edit': '', title: `Name ${app.title}`, 'aria-label': `Name ${app.title}` }, '✎'));
  }
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
  if (!app.workNameScope && address.kind === 'library' && !app.row.querySelector('[data-lib-edit=""]')) {
    app.row.querySelector('.cv-lib-title-group')?.after(el(document_, 'button', { type: 'button', class: 'cv-lib-edit', 'data-lib-edit': '', title: `Name ${address.title}`, 'aria-label': `Name ${address.title}` }, '✎'));
  }
  // The plain `/slug` the server rendered becomes the address chip (one component everywhere).
  const head = app.row.querySelector('.cv-lib-app-head');
  const plain = app.row.querySelector<HTMLElement>('.cv-lib-slug');
  // APP-121: the short form when unambiguous, else `/app:notes`.
  const chip = addressChip(document_, appForm(address));
  chip.classList.add('cv-lib-slug');
  if (plain) plain.replaceWith(chip);
  else app.row.querySelector('.cv-lib-title-group')?.after(chip);
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
  const list = allAddresses();
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


/** A saved app name updates only this work; cards and unsent forms stay mounted. */
export function applyWorkName(appId: string, work: string, name: string): void {
  for (const app of apps) {
    if (app.app !== appId || app.work !== work) continue;
    app.title = name;
    retitle(app.row.querySelector<HTMLElement>('.cv-lib-app-title'), name);
    app.row.dataset.search = `${app.number} ${name.toLowerCase()} ${app.slug}`;
  }
  updateWorkTitle(appId, work, name);
  documentRef?.dispatchEvent(new CustomEvent('forma:work-renamed', { detail: { app: appId, work, name } }));
}

/** APP-121: the app's and cards' labels on the Library rows (exported for the tests). */
export function applyLabels(list: Address[] = allAddresses()): void {
  for (const app of apps) {
    const address = app.address ?? rowAddress(app, list);
    if (!address || (app.workNameScope && (address.kind !== 'saved' || address.work !== app.work))) continue;
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
export function labelCanvasCards(document_: Document, list: Address[] = allAddresses()): void {
  const placed = new Map<string, HTMLElement>();
  for (const node of document_.querySelectorAll<HTMLElement>('[data-card]')) {
    if (node.classList.contains('cv-card') && node.dataset.card) placed.set(node.dataset.card, node);
  }
  if (!placed.size) return;
  for (const app of apps) {
    const address = app.address ?? rowAddress(app, list);
    if (!address || (app.workNameScope && (address.kind !== 'saved' || address.work !== app.work))) continue;
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
  const cardsGroup = el(document_, 'div', { class: 'cv-lib-group', 'data-lib-group': 'cards' },
    el(document_, 'h2', { class: 'cv-lib-group-title' }, 'Cards'),
    el(document_, 'section', {class: 'cv-lib-cat', 'data-category': 'file-cards', 'aria-label': 'Uploaded files'}),
    el(document_, 'section', { class: 'cv-lib-cat', 'data-category': 'standalone', 'aria-label': 'Standalone cards' },
      el(document_, 'p', { class: 'cv-lib-hint' }, 'Whole card works. This personal category is saved in this browser.'),
      el(document_, 'ul', { class: 'cv-lib-apps' })),
    el(document_, 'section', {class: 'cv-lib-cat', 'data-category': 'saved-cards', 'aria-label': 'Saved cards and answers'}));
  list.replaceChildren(appsGroup, widgetsGroup, cardsGroup);
  renderFileCards(cardsGroup.querySelector<HTMLElement>('[data-category="file-cards"]')!, fileCardEntries());
  renderSavedCards(cardsGroup.querySelector<HTMLElement>('[data-category="saved-cards"]')!, savedEntries, showSaved ?? (async () => 'Saved cards are still loading.'), savedMessage, refreshSaved);
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
  // App component metadata stays hidden; apps never expand into loose parts.
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
    if (target?.closest('[data-lib-classify]')) {
      const standalone = !isStandaloneWork(app.app, app.work);
      setStandaloneWork(app.app, app.work, standalone);
      // A placed work follows the same default as its new category, in place.
      if (appViewForWork(app.work)) void setAppView(app.app, app.work, standalone ? 'open' : 'face').then(announce);
      app.row.querySelector('details')?.removeAttribute('open');
      syncLibrary();
      return;
    }
    if (target?.closest('[data-lib-remove]')) {
      removeWork(app.work);
      syncLibrary();
      announce(`${app.title} removed from the canvas. Its saved content is unchanged.`);
      return;
    }
    if (target?.closest('[data-lib-stack], [data-lib-open-view]')) {
      void setAppView(app.app, app.work, target.closest('[data-lib-stack]') ? 'stack' : 'open').then(announce);
      if (phone()) closeLibrary();
      return;
    }
    if (target?.closest('[data-lib-all]')) void addAll(app, announce);
  });
  search?.addEventListener('input', () => filter(search?.value ?? ''));
  search?.addEventListener('keydown', (event) => {
    if (event.key === 'Enter') {
      // Search activates the first matching whole work, never a hidden part.
      event.preventDefault();
      const first = apps.find(app => !app.row.hidden && app.row.offsetParent !== null);
      if (first) void addAll(first, announce);
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
  bindLibraryDrag(document_, {
    cardFrom(row) {
      const app = apps.find(candidate => candidate.row === row);
      if (!app || !isStandaloneWork(app.app, app.work)) return null;
      return { id: `a-${app.work}`, title: app.title, number: app.number, kind: 'app', app: app.app, work: app.work };
    },
    onBoard: (id) => Boolean(appViewForWork(id.replace(/^a-/, ''))),
    place: (card, at) => { void setAppView(card.app, card.work, 'open', { at, focus: false }).then(announce); },
    camera: () => canvasApi()?.getCamera() ?? null,
    viewport: () => document_.querySelector<HTMLElement>('[data-forma-canvas-viewport]'),
    footprint: cardFootprint,
  });
}
