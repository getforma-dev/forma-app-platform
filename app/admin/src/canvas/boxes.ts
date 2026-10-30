// APP-108 (ADR-011 pitch): boxes on the canvas. A box is a card holding the
// member's files: icon tiles (PDF, image, CSV, spreadsheet, text), drop or
// pick upload with a progress bar per file, a click opens the viewer
// (`box-viewer.ts`), a CSV/XLSX is parsed at once by the sealed parser and
// its inferred columns open in the "Make data" sheet (`data-sheet.ts`), and
// the box lists the apps built from it. A box can build an app from its
// files ("Create an app…", or `/storage <box> create me an app for …`): the
// server sends the composer a capped context and binds the app to the box's
// source revisions; the app's "Built from box" chip opens the box again, and
// a receipt names the file its source revision came from.
//
// Elsewhere on the canvas: a CSV/XLSX dropped on the board or the intent bar
// goes into the member's implicit "Uploads" box, is registered with the
// parser's inferred types and placed as a data card (a typed table with its
// receipt); a small notice offers "Adjust types" and "Make an app from
// this". Other files dropped on the board land in "Uploads" too.
//
// Members see the noun "Folder" (`box-api.ts` NOUN; code and API keep
// "box"). Commands: `/folder` (aliases in FOLDER_ALIASES: storage, box,
// boxes, folders) lists the folders, `/folder <number|name>` opens one,
// `/folder new <name>` creates one, `/folder uploads` opens the implicit
// one; `/folder [<folder>] formation <instruction>` acts on the named folder
// or the selected / last opened / only one: a brief request mounts the
// Brief card (`brief.ts`), anything else builds an app from the folder
// (`/folder <folder> create me an app for <purpose>` still works). The
// library pane gains a "Folders" section. With a folder card selected, the
// intent bar's free text carries its id to the action path (`actions.ts`).
//
// Nothing from the server is markup: names are text nodes, ids checked UUIDs.
// Open box cards are remembered in this browser only (`forma.boxes.v1`, not
// in demo mode), like the widgets.
import {
  ACCEPT, createBox, FOLDER_ALIASES, FOLDER_COMMAND, FORMATION_PROMPT, NOUN, noun, NOUN_PLURAL, nouns, renameBox, wantsBrief, defaultColumns, fetchPreview, formatBytes, getBox, isDataFile, lineage, listBoxes, makeApp, makeData,
  MAX_BOX_FILES, MAX_FILE_BYTES, refusalText, uploadFile, uploadsBox,
  type Box, type BoxContents, type BoxFile, type Preview,
} from './box-api';
import { openViewer, setMakeDataHandler } from './box-viewer';
import { openBrief } from './brief';
import { activeFolder, folderRenamed, openFolderPicker, setActiveFolder, startFolderBar } from './folder-bar';
import { openDataSheet } from './data-sheet';
import { startExplain } from './explain';
import { registerFocusCard, unregisterFocusCard } from './focus';
import {
  boardWorks, canvasApi, canvasHost, isDemoCanvas, mountWidget, onBoardChange, onLayoutSaved,
  placeWork, unmountWidget, widgetSpot,
} from './board';
import { setFileRouter } from './receipt';
import { el, isUuid } from './render';
import { registerCommands, registerCompleter, type Completion } from './slash';

type Report = (message: string) => void;
type OpenBox = { boxId: string; item: HTMLElement; card: HTMLElement; refresh(): Promise<void>; upload(list: File[], auto?: boolean): Promise<void>; rename(): void; name(): string };

const STORAGE_KEY = 'forma.boxes.v1';
const CMD = `/${FOLDER_COMMAND}`;
const open = new Map<string, OpenBox>();
let boxes: Box[] = [];
let started = false;
/** The folder card opened (or brought forward) last. */
let lastOpened: string | null = null;

function announce(message: string): void {
  const status = document.getElementById('cv-activity');
  if (status) status.textContent = message;
}

// --- The notice (non-blocking status with actions) ---

function notice(message: string, actions: { label: string; run(): void }[] = [], linger = 12000): void {
  announce(message);
  const document_ = document;
  let host = document_.getElementById('bx-notice');
  if (!host) {
    host = el(document_, 'div', { id: 'bx-notice', class: 'bx-notice', role: 'status', 'aria-live': 'polite' });
    (document_.querySelector('.cv-dock') ?? document_.body).prepend(host);
  }
  const close = el(document_, 'button', { type: 'button', class: 'bx-notice-close', 'aria-label': 'Dismiss' }, '×');
  host.replaceChildren(el(document_, 'span', { class: 'bx-notice-text' }, message),
    ...actions.map((action) => {
      const button = el(document_, 'button', { type: 'button', class: 'bx-notice-action' }, action.label);
      button.addEventListener('click', () => {
        host?.setAttribute('hidden', '');
        action.run();
      });
      return button;
    }),
    close);
  host.removeAttribute('hidden');
  close.addEventListener('click', () => host?.setAttribute('hidden', ''));
  const shown = host;
  window.clearTimeout(Number(shown.dataset.timer ?? 0));
  if (linger > 0) shown.dataset.timer = String(window.setTimeout(() => shown.setAttribute('hidden', ''), linger));
}

// --- Persistence (this browser only) ---

function save(): void {
  if (isDemoCanvas()) return;
  const rows: { id: string; x: number; y: number }[] = [];
  for (const entry of open.values()) {
    const spot = widgetSpot(entry.item);
    if (spot) rows.push({ id: entry.boxId, x: spot.x, y: spot.y });
  }
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(rows));
  } catch {
    // view layout only
  }
}

function load(): { id: string; x: number; y: number }[] {
  try {
    const rows = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? '[]') as unknown;
    if (!Array.isArray(rows)) return [];
    return rows.slice(0, 12).flatMap((row) => {
      if (typeof row !== 'object' || row === null) return [];
      const { id, x, y } = row as Record<string, unknown>;
      return isUuid(id) && typeof x === 'number' && typeof y === 'number' && Number.isFinite(x) && Number.isFinite(y) ? [{ id, x, y }] : [];
    });
  } catch {
    return [];
  }
}

// --- The box list (pane, completions) ---

export async function refreshBoxes(): Promise<Box[]> {
  try {
    boxes = await listBoxes();
  } catch {
    // keep the last list
  }
  renderPane();
  return boxes;
}

function boxLabel(box: Box): string {
  return box.kind === 'uploads' ? 'Uploads' : box.name;
}

function renderPane(): void {
  const document_ = document;
  const list = document_.querySelector<HTMLElement>('#cv-library [data-lib-list]');
  if (!list) return;
  let section = list.querySelector<HTMLElement>('.cv-lib-cat[data-category="boxes"]');
  if (!section) {
    section = el(document_, 'section', { class: 'cv-lib-cat bx-lib', 'data-category': 'boxes', 'data-external': 'true', 'aria-label': NOUN_PLURAL },
      el(document_, 'h3', { class: 'cv-lib-cat-title' }, el(document_, 'span', {}, NOUN_PLURAL), el(document_, 'span', { class: 'cv-lib-count' }, '')),
      el(document_, 'ul', { class: 'cv-lib-apps' }));
    const utilities = list.querySelector('.cv-lib-cat[data-category="utilities"]');
    if (utilities) list.insertBefore(section, utilities);
    else list.append(section);
  }
  const rows = boxes.map((box) => {
    const button = el(document_, 'button', { type: 'button', class: 'cv-lib-expand bx-lib-open', title: `Open the ${noun} on the canvas` },
      el(document_, 'span', { class: 'cv-lib-num' }, String(box.number)),
      el(document_, 'span', { class: 'cv-lib-app-title' }, boxLabel(box)),
      el(document_, 'span', { class: 'cv-lib-on' }, `${box.file_count} ${box.file_count === 1 ? 'file' : 'files'}${box.owner ? '' : ' · shared'}`));
    button.addEventListener('click', () => void openBox(box.box_id).then(announce));
    return el(document_, 'li', { class: 'bx-lib-row', 'data-box': box.box_id }, el(document_, 'div', { class: 'cv-lib-app-head' }, button));
  });
  const create = el(document_, 'button', { type: 'button', class: 'cv-lib-all bx-lib-new' }, `+ New ${noun}`);
  create.addEventListener('click', () => void newUntitled().then(announce));
  section.querySelector('.cv-lib-apps')?.replaceChildren(...rows, el(document_, 'li', { class: 'bx-lib-row' }, create));
  const count = section.querySelector('.cv-lib-count');
  if (count) count.textContent = `${boxes.length} ${boxes.length === 1 ? noun : nouns}`;
}

/** A box by number ("1") or name (case-insensitive; the longest name the text starts with). */
function resolve(text: string): { box: Box; rest: string } | null {
  const trimmed = text.trim();
  const number = trimmed.match(/^#?(\d{1,4})\b\s*([\s\S]*)$/);
  if (number) {
    const box = boxes.find((candidate) => candidate.number === Number(number[1]));
    return box ? { box, rest: number[2] ?? '' } : null;
  }
  const quoted = trimmed.match(/^["“']([^"”']{1,120})["”']\s*([\s\S]*)$/);
  if (quoted) {
    const box = boxes.find((candidate) => boxLabel(candidate).toLowerCase() === (quoted[1] ?? '').toLowerCase());
    return box ? { box, rest: quoted[2] ?? '' } : null;
  }
  const lower = trimmed.toLowerCase();
  let best: Box | null = null;
  for (const candidate of boxes) {
    const name = boxLabel(candidate).toLowerCase();
    if ((lower === name || lower.startsWith(`${name} `)) && (!best || name.length > boxLabel(best).length)) best = candidate;
  }
  return best ? { box: best, rest: trimmed.slice(boxLabel(best).length) } : null;
}

/** "create me an app for metrics" → "metrics"; any other words are the purpose themselves. */
export function purposeOf(rest: string): string {
  return rest.trim()
    .replace(/^(please\s+)?(create|make|build|generate|give)\s+(me\s+)?(an?\s+)?(app|dashboard)\s+(for|of|about|showing|that shows)\s+/i, '')
    .replace(/^(an?\s+)?app\s+(for|of)\s+/i, '')
    .trim();
}

function completions(rest: string): Completion[] {
  const query = rest.trim().toLowerCase();
  const fixed: Completion[] = [
    { badge: '+', title: `New ${noun}`, detail: `${CMD} new <name>`, value: `${CMD} new ` },
    { badge: '↥', title: 'Uploads', detail: 'Files dropped on the canvas', value: `${CMD} uploads` },
  ];
  const listed = boxes
    .filter((box) => !query || String(box.number).startsWith(query) || boxLabel(box).toLowerCase().includes(query))
    .slice(0, 8)
    .map((box) => ({
      badge: String(box.number), title: boxLabel(box), detail: `${box.file_count} ${box.file_count === 1 ? 'file' : 'files'}`,
      value: `${CMD} ${box.number}`,
    }));
  return [...listed, ...fixed.filter((item) => !query || item.title.toLowerCase().includes(query) || 'new'.startsWith(query))];
}

// --- Box cards ---

const ICONS: Record<BoxFile['kind'], string> = { pdf: 'PDF', image: 'IMG', csv: 'CSV', xlsx: 'XLS', text: 'TXT' };

/** Opens (or brings into view) the card of box `boxId`; returns a status line. */
export async function openBox(boxId: string, at?: { x: number; y: number }): Promise<string> {
  if (!isUuid(boxId) || !canvasHost()) return `That ${noun} is not available.`;
  const existing = open.get(boxId);
  if (existing) {
    canvasApi()?.centerItem(existing.item);
    canvasApi()?.setActive(existing.item);
    lastOpened = boxId;
    if (!at) setActiveFolder({ box_id: boxId, name: existing.name() });
    await existing.refresh();
    return `That ${noun} is already on your canvas.`;
  }
  let contents: BoxContents;
  try {
    contents = await getBox(boxId);
  } catch (error) {
    return refusalText(error);
  }
  if (open.has(boxId)) return `That ${noun} is already on your canvas.`;
  const card = buildBoxCard(contents);
  const item = mountWidget(card.card, { id: `box-${boxId}`, title: boxLabel(contents.box), width: 520, minHeight: 360, at, focus: !at });
  if (!item) return `The ${noun} could not be placed.`;
  item.classList.add('cv-item-box');
  const entry: OpenBox = { boxId, item, card: card.card, refresh: card.refresh, upload: card.upload, rename: card.rename, name: card.name };
  open.set(boxId, entry);
  lastOpened = boxId;
  // Opening a folder makes it the bar's active folder (a restored card does not).
  if (!at) setActiveFolder({ box_id: boxId, name: boxLabel(contents.box) });
  else if (activeFolder()?.box_id === boxId) card.card.classList.add('is-folder-active');
  card.onClose(() => {
    unregisterFocusCard(card.card);
    unmountWidget(item);
    open.delete(boxId);
    if (activeFolder()?.box_id === boxId) setActiveFolder(null);
    save();
  });
  save();
  return `${boxLabel(contents.box)} is on your canvas.`;
}

function buildBoxCard(initial: BoxContents) {
  const document_ = document;
  let contents = initial;
  const boxId = initial.box.box_id;
  const closeListeners: (() => void)[] = [];
  const title = el(document_, 'h2', { class: 'bx-title' }, boxLabel(initial.box));
  const eyebrow = el(document_, 'p', { class: 'bx-eyebrow' }, '');
  const status = el(document_, 'p', { class: 'bx-card-status', role: 'status' });
  const briefButton = el(document_, 'button', { type: 'button', class: 'bx-brief', title: `An AI brief of this ${noun}, cited from its files` }, 'Brief me');
  const formationButton = el(document_, 'button', { type: 'button', class: 'bx-brief bx-formation', title: `Build an app from this ${noun} in words` }, '✨ Formation');
  briefButton.addEventListener('click', () => {
    const item = open.get(boxId)?.item ?? null;
    void openBrief(boxId, boxLabel(contents.box), false, item).then(announce);
  });
  const close = el(document_, 'button', { type: 'button', class: 'bx-close', 'aria-label': `Remove the ${noun} from the canvas`, title: 'Remove from canvas (the files stay)' }, '×');
  const picker = el(document_, 'input', { type: 'file', multiple: '', accept: ACCEPT, hidden: '' });
  const pick = el(document_, 'button', { type: 'button', class: 'bx-pick' }, 'Add files');
  const drop = el(document_, 'div', { class: 'bx-drop', 'data-dragging': 'false' },
    el(document_, 'span', {}, 'Drop PDFs, images, CSV, XLSX or text here, or '), pick, picker,
    el(document_, 'span', { class: 'bx-limits' }, 'Up to 20 files, 20 MB each'));
  const progress = el(document_, 'ul', { class: 'bx-progress', 'aria-label': 'Uploads in progress' });
  const tiles = el(document_, 'ul', { class: 'bx-tiles', 'aria-label': `Files in this ${noun}` });
  const appsList = el(document_, 'ul', { class: 'bx-apps' });
  const appsSection = el(document_, 'section', { class: 'bx-apps-section', 'aria-label': `Apps built from this ${noun}` },
    el(document_, 'h3', { class: 'bx-h' }, `Apps built from this ${noun}`), appsList);
  const purpose = el(document_, 'input', { type: 'text', maxlength: '200', value: FORMATION_PROMPT, 'aria-label': `What app should be built from this ${noun}` });
  const build = el(document_, 'button', { type: 'submit' }, 'Build');
  const cancelFormation = el(document_, 'button', { type: 'button', class: 'bx-quiet' }, 'Cancel');
  const form = el(document_, 'form', { class: 'bx-create', autocomplete: 'off', hidden: '' },
    el(document_, 'span', { class: 'bx-formation-mark', 'aria-hidden': 'true' }, '✨'), purpose, build, cancelFormation);
  const head = el(document_, 'header', { class: 'bx-head' },
    el(document_, 'span', { class: 'bx-mark', 'aria-hidden': 'true' }),
    el(document_, 'div', { class: 'bx-id' }, eyebrow, title),
    close);
  const card = el(document_, 'article', { class: 'cv-card bx-card', 'data-kind': 'box', 'data-box': boxId, 'data-card': `bx-${boxId}`, 'aria-label': `${NOUN} ${boxLabel(initial.box)}` },
    head,
    el(document_, 'div', { class: 'bx-actions' }, formationButton, briefButton),
    el(document_, 'div', { class: 'bx-body' }, drop, progress, tiles, appsSection),
    form,
    status);

  const say = (message: string) => { status.textContent = message; };

  const render = () => {
    const { box, files, apps } = contents;
    title.textContent = boxLabel(box);
    eyebrow.textContent = `${box.kind === 'uploads' ? 'Uploads' : box.kind === 'app' ? `App ${noun}` : NOUN} ${box.number} · ${files.length} ${files.length === 1 ? 'file' : 'files'}${box.owner ? '' : ' · shared with you'}`;
    drop.hidden = !box.owner;
    formationButton.hidden = !box.owner;
    if (!box.owner) form.hidden = true;
    title.title = box.owner && box.kind !== 'uploads' ? 'Click to rename' : '';
    tiles.replaceChildren(...files.map((file) => {
      const main = el(document_, 'button', { type: 'button', class: 'bx-tile-open', title: `Open ${file.name}` },
        el(document_, 'span', { class: 'bx-icon', 'data-kind': file.kind, 'aria-hidden': 'true' }, ICONS[file.kind]),
        el(document_, 'span', { class: 'bx-name' }, file.name),
        el(document_, 'span', { class: 'bx-meta' }, `${formatBytes(file.bytes)}${file.data ? ` · Data ✓ (${file.data.row_count} rows)` : ''}`));
      main.addEventListener('click', () => openViewer(file, boxId));
      const tile = el(document_, 'li', { class: 'bx-tile', 'data-kind': file.kind, 'data-file': file.file_id }, main);
      if ((file.kind === 'csv' || file.kind === 'xlsx') && box.owner) {
        const data = el(document_, 'button', { type: 'button', class: `bx-data${file.data ? ' is-data' : ''}` }, file.data ? 'Data ✓' : 'Make data');
        data.title = file.data ? 'Registered as data. Click to adjust the types (a new revision).' : 'Confirm the columns and register this file as data';
        data.addEventListener('click', () => void sheetFor(boxId, file, () => void refresh()));
        tile.append(data);
      }
      return tile;
    }));
    if (!files.length) tiles.append(el(document_, 'li', { class: 'bx-empty' }, box.owner ? 'No files yet.' : 'No files shared with you here.'));
    appsSection.hidden = apps.length === 0;
    appsList.replaceChildren(...apps.map((app) => {
      const button = el(document_, 'button', { type: 'button', class: 'bx-app' },
        el(document_, 'span', { class: 'bx-app-kind', 'data-kind': app.kind }, app.kind === 'data' ? 'data card' : 'generated'),
        el(document_, 'span', { class: 'bx-app-title' }, app.title),
        el(document_, 'span', { class: 'bx-meta' }, `${app.file_ids.length} ${app.file_ids.length === 1 ? 'file' : 'files'}`));
      button.addEventListener('click', () => void placeWork(app.app_id, app.work_id, []).then(say));
      return el(document_, 'li', {}, button);
    }));
  };

  const refresh = async () => {
    try {
      contents = await getBox(boxId);
      render();
    } catch (error) {
      say(refusalText(error));
    }
  };

  const upload = async (list: File[], auto = false) => {
    let opened = false;
    for (const file of list) {
      if (file.size > MAX_FILE_BYTES) {
        say(`${file.name} is over 20 MB. Nothing was stored.`);
        continue;
      }
      if (contents.files.length >= MAX_BOX_FILES) {
        say(`This ${noun} already holds 20 files. Nothing more was stored.`);
        break;
      }
      const bar = el(document_, 'progress', { max: '1', value: '0' });
      const row = el(document_, 'li', { class: 'bx-upload' }, el(document_, 'span', { class: 'bx-name' }, file.name), bar);
      progress.append(row);
      try {
        const uploaded = await uploadFile(boxId, file, (fraction) => { bar.value = fraction; });
        row.remove();
        contents = { ...contents, files: [...contents.files, uploaded.file] };
        render();
        say(`${file.name} added.`);
        const found = uploaded.preview;
        if (found && !('refused' in found) && auto) {
          // Dropped on empty canvas: registered with the inferred types, its data card placed.
          void autoData(boxId, uploaded.file, found, () => void refresh());
        } else if (found && !('refused' in found) && !opened) {
          opened = true;
          openDataSheet(boxId, uploaded.file, found, () => void refresh());
        } else if (found && 'refused' in found) {
          say(`${file.name} was stored, but it could not be read as a table.`);
        }
      } catch (error) {
        row.remove();
        say(`${file.name}: ${refusalText(error)}`);
      }
    }
    await refresh();
    void refreshBoxes();
  };

  pick.addEventListener('click', () => picker.click());
  picker.addEventListener('change', () => {
    const list = Array.from(picker.files ?? []);
    picker.value = '';
    if (list.length) void upload(list);
  });
  for (const name of ['dragenter', 'dragover']) {
    card.addEventListener(name, (event) => {
      if (!(event as DragEvent).dataTransfer?.types.includes('Files') || !contents.box.owner) return;
      event.preventDefault();
      event.stopPropagation();
      drop.dataset.dragging = 'true';
    });
  }
  card.addEventListener('dragleave', () => { drop.dataset.dragging = 'false'; });
  card.addEventListener('drop', (event) => {
    if (!(event as DragEvent).dataTransfer?.types.includes('Files')) return;
    event.preventDefault();
    event.stopPropagation();
    drop.dataset.dragging = 'false';
    if (!contents.box.owner) return;
    const list = Array.from((event as DragEvent).dataTransfer?.files ?? []);
    if (list.length) void upload(list);
  });
  formationButton.addEventListener('click', () => {
    form.hidden = !form.hidden;
    if (!form.hidden) {
      if (!purpose.value.trim()) purpose.value = FORMATION_PROMPT;
      purpose.focus();
      purpose.select();
    }
  });
  cancelFormation.addEventListener('click', () => { form.hidden = true; });
  form.addEventListener('submit', (event) => {
    event.preventDefault();
    const words = purposeOf(purpose.value) || purpose.value.trim();
    if (!words || build.disabled) return;
    build.disabled = true;
    say(`Generating an app from this ${noun} (file names, confirmed columns and a few sample rows only)…`);
    void buildApp(boxId, words).then((message) => {
      say(message);
      form.hidden = true;
      purpose.value = FORMATION_PROMPT;
      return refresh();
    }).finally(() => { build.disabled = false; });
  });
  // Keys typed into the card's fields belong to the field, not the canvas.
  form.addEventListener('keydown', (event) => {
    event.stopPropagation();
    if (event.key === 'Escape') form.hidden = true;
  });
  // APP-106 focus mode: F or the focus button; a focused folder is the active folder.
  registerFocusCard(card, head, NOUN, () => setActiveFolder({ box_id: boxId, name: boxLabel(contents.box) }));
  head.append(close);
  // Clicking the card makes it the bar's active folder.
  card.addEventListener('pointerdown', () => setActiveFolder({ box_id: boxId, name: boxLabel(contents.box) }));
  // Rename: click the title, Enter or blur saves, Esc cancels.
  const rename = () => {
    if (!contents.box.owner || contents.box.kind === 'uploads' || title.hidden) return;
    const field = el(document_, 'input', { type: 'text', class: 'bx-title-edit', maxlength: '60', value: boxLabel(contents.box), 'aria-label': `Name of the ${noun}` });
    title.hidden = true;
    title.after(field);
    field.focus();
    field.select();
    let done = false;
    const finish = async (keep: boolean) => {
      if (done) return;
      done = true;
      const name = field.value.trim().slice(0, 60);
      field.remove();
      title.hidden = false;
      if (!keep || !name || name === boxLabel(contents.box)) return;
      try {
        const renamed = await renameBox(boxId, name);
        contents = { ...contents, box: renamed };
        render();
        folderRenamed(boxId, boxLabel(renamed));
        say(`Renamed to ${boxLabel(renamed)}.`);
        void refreshBoxes();
      } catch (error) {
        say(refusalText(error));
      }
    };
    field.addEventListener('keydown', (event) => {
      event.stopPropagation();
      if (event.key === 'Enter') { event.preventDefault(); void finish(true); }
      if (event.key === 'Escape') { event.preventDefault(); void finish(false); }
    });
    field.addEventListener('blur', () => void finish(true));
  };
  title.addEventListener('click', rename);
  close.addEventListener('click', () => { for (const listener of closeListeners) listener(); });
  render();
  return { card, refresh, upload, rename, name: () => boxLabel(contents.box), onClose: (listener: () => void) => closeListeners.push(listener) };
}

async function sheetFor(boxId: string, file: BoxFile, done: () => void): Promise<void> {
  try {
    const found = await fetchPreview(boxId, file.file_id);
    if (!found || 'refused' in found) {
      announce(`${file.name} could not be read as a table.`);
      return;
    }
    openDataSheet(boxId, file, found, done, Boolean(file.data));
  } catch (error) {
    announce(refusalText(error));
  }
}

/** Builds an app from a box (capped context on the server), places it, and says so. */
async function buildApp(boxId: string, purpose: string, report?: Report): Promise<string> {
  report?.(`Generating an app from the ${noun}…`);
  try {
    const built = await makeApp(boxId, purpose);
    const placed = await placeWork(built.app_id, built.work_id, []);
    void open.get(boxId)?.refresh();
    return `${built.title}: generated from ${built.files} ${built.files === 1 ? 'file' : 'files'} of the ${noun}. ${placed}`;
  } catch (error) {
    return refusalText(error);
  }
}

async function newBox(name: string): Promise<string> {
  try {
    const made = await createBox(name.slice(0, 60));
    await refreshBoxes();
    return openBox(made.box_id);
  } catch (error) {
    return refusalText(error);
  }
}

// --- Ad hoc data: a CSV/XLSX dropped outside a box ---

/** Uploads `file` into "Uploads"; a CSV/XLSX becomes a data card with the inferred types. */
export async function dropAnywhere(file: File): Promise<void> {
  const data = isDataFile(file);
  if (file.size > MAX_FILE_BYTES) {
    notice(`${file.name} is over 20 MB. Nothing was stored.`);
    return;
  }
  notice(`Uploading ${file.name}…`, [], 0);
  let uploads: Box;
  try {
    uploads = await uploadsBox();
  } catch (error) {
    notice(refusalText(error));
    return;
  }
  let uploaded;
  try {
    uploaded = await uploadFile(uploads.box_id, file, (fraction) => {
      const text = document.querySelector('#bx-notice .bx-notice-text');
      if (text) text.textContent = `Uploading ${file.name}… ${Math.round(fraction * 100)}%`;
    });
  } catch (error) {
    notice(`${file.name}: ${refusalText(error)}`);
    return;
  }
  void refreshBoxes();
  void open.get(uploads.box_id)?.refresh();
  const found = uploaded.preview;
  if (!data || !found || 'refused' in found) {
    notice(data ? `${file.name} was kept in Uploads, but it could not be read as a table.` : `${file.name} was kept in Uploads.`,
      [{ label: 'Open Uploads', run: () => void openBox(uploads.box_id).then(announce) }]);
    return;
  }
  await autoData(uploads.box_id, uploaded.file, found, () => void open.get(uploads.box_id)?.refresh());
}

/** Registers a dropped table with the parser's inferred types, places its data card, offers "Adjust types". */
async function autoData(boxId: string, file: BoxFile, found: Preview, done: () => void): Promise<void> {
  notice(`Read ${file.name}: ${found.rows} rows, ${found.columns.length} columns. Registering with the inferred types…`, [], 0);
  try {
    const made = await makeData(boxId, file.file_id, defaultColumns(found), true);
    if (made.card) await placeWork(made.card.app_id, made.card.work_id, []);
    done();
    const file_ = { ...file, data: made.data };
    notice(made.card ? `${file.name} is a data card now (${made.data?.row_count ?? found.rows} rows, types inferred).` : `${file.name} is registered as data.`, [
      { label: 'Adjust types', run: () => openDataSheet(boxId, file_, found, done, true) },
      { label: 'Make an app from this', run: () => openDataSheet(boxId, file_, found, done, true) },
      { label: 'Keep as card', run: () => undefined },
    ], 20000);
  } catch (error) {
    notice(`${file.name}: ${refusalText(error)}`, [
      { label: 'Choose types', run: () => openDataSheet(boxId, file, found, done) },
    ], 20000);
  }
}

/** "+ New folder" and a drop on empty canvas: an "Untitled folder", open, its name ready to type. */
export async function newUntitled(files: File[] = []): Promise<string> {
  let made: Box;
  try {
    made = await createBox(`Untitled ${noun}`);
  } catch (error) {
    return refusalText(error);
  }
  await refreshBoxes();
  const message = await openBox(made.box_id);
  const entry = open.get(made.box_id);
  if (!entry) return message;
  entry.rename();
  if (files.length) void entry.upload(files, true);
  return files.length ? `A new ${noun} holds your ${files.length === 1 ? 'file' : 'files'}; name it.` : `A new ${noun} is on your canvas; name it.`;
}

// --- Provenance on app cards and receipts ---

function workOf(node: Element): { app: string; work: string } | null {
  const work = node.closest<HTMLElement>('.cv-item')?.dataset.group ?? node.closest<HTMLElement>('.cv-group')?.dataset.work ?? '';
  const found = boardWorks().find((entry) => entry.work === work);
  const app = found?.app ?? node.closest<HTMLElement>('.cv-group')?.dataset.app ?? '';
  return isUuid(app) && isUuid(work) ? { app, work } : null;
}

async function openLineage(chip: Element): Promise<void> {
  const owner = workOf(chip);
  if (!owner) return;
  const found = await lineage(owner.app, owner.work);
  announce(found ? await openBox(found.box_id) : 'The box this app was built from is not available to you.');
}

function markChips(): void {
  for (const chip of document.querySelectorAll<HTMLElement>('.cv-origin[data-origin="box"]:not([data-box-link])')) {
    chip.dataset.boxLink = 'true';
    chip.setAttribute('role', 'button');
    chip.setAttribute('tabindex', '0');
    chip.title = `Open the ${noun} this app was built from`;
  }
}

async function receiptLineage(details: HTMLDetailsElement): Promise<void> {
  if (details.dataset.lineage) return;
  details.dataset.lineage = 'pending';
  const owner = workOf(details);
  if (!owner) return;
  const found = await lineage(owner.app, owner.work);
  if (!found || !found.files.length) return;
  const body = details.querySelector('.cv-receipt-body');
  if (!body) return;
  const text = body.textContent ?? '';
  const matched = found.files.filter((file) => file.revision_id && text.includes(file.revision_id));
  const document_ = document;
  const rows = (matched.length ? matched : found.files).map((file) => {
    const button = el(document_, 'button', { type: 'button', class: 'bx-lineage-open' }, 'Open file');
    button.addEventListener('click', () => openViewer({
      file_id: file.file_id, name: file.name, kind: file.kind, media_type: '', bytes: 0, sha256: '', created_at: '',
      content_url: file.content_url, data: null,
    }, found.box_id));
    return el(document_, 'li', {}, el(document_, 'span', {}, `Source file: ${file.name} (${noun} ${found.box_name})`), button);
  });
  details.dataset.lineage = 'shown';
  body.prepend(el(document_, 'ul', { class: 'bx-lineage', 'aria-label': 'Files these numbers come from' }, ...rows));
}

// --- Commands ---

/** `formation`'s target when none is named: the bar's active folder, else the last opened on the board, else the only one. */
function implicitFolder(): Box | null {
  const id = activeFolder()?.box_id ?? (lastOpened && open.has(lastOpened) ? lastOpened : null);
  if (id) return boxes.find((box) => box.box_id === id) ?? null;
  const own = boxes.filter((box) => box.kind !== 'uploads');
  if (own.length === 1) return own[0] ?? null;
  return boxes.length === 1 ? boxes[0] ?? null : null;
}

/** `formation <instruction>` on a folder: a brief, or an app built from it. */
async function formation(box: Box, instruction: string, report: Report): Promise<void> {
  const words = instruction.trim();
  if (!words) {
    report(`Say what to do after formation, e.g. ${CMD} formation brief me, or ${CMD} formation build a metrics app.`);
    return;
  }
  report(await openBox(box.box_id));
  if (wantsBrief(words)) {
    report(`Writing a brief of ${boxLabel(box)} from its files…`);
    report(await openBrief(box.box_id, boxLabel(box), true, open.get(box.box_id)?.item ?? null));
    return;
  }
  report(await buildApp(box.box_id, purposeOf(words) || words, report));
}

const FOLDER = {
  slug: FOLDER_COMMAND,
  aliases: FOLDER_ALIASES,
  title: NOUN_PLURAL,
  purpose: `Your ${nouns} of files (“${CMD} 1”, “${CMD} new Q3 pack”, “${CMD} 1 formation build a metrics app”)`,
  run: async (text: string, report: Report = announce) => {
    const words = text.trim();
    await refreshBoxes();
    if (!words) {
      // No command knowledge needed: pick a folder, it becomes the bar's active folder.
      await openFolderPicker();
      report(`Pick a ${noun}; then just say what you need.`);
      return;
    }
    const created = words.match(/^new\s+([\s\S]{1,60})$/i);
    if (created) {
      report(await newBox((created[1] ?? '').trim()));
      return;
    }
    if (/^new$/i.test(words)) {
      report(`Name the ${noun}: ${CMD} new <name>.`);
      return;
    }
    // `formation <instruction>` on the selected / last opened / only folder.
    const bare = words.match(/^formation\b\s*([\s\S]*)$/i);
    if (bare) {
      const target = implicitFolder();
      if (!target) {
        await openFolderPicker();
        report(`Which ${noun}? Type ${CMD} <number> formation <instruction>, or select a ${noun} card first.`);
        return;
      }
      await formation(target, bare[1] ?? '', report);
      return;
    }
    let target: { box: Box; rest: string } | null = null;
    const uploads = words.match(/^uploads\b\s*([\s\S]*)$/i);
    if (uploads) {
      try {
        target = { box: await uploadsBox(), rest: uploads[1] ?? '' };
      } catch (error) {
        report(refusalText(error));
        return;
      }
    } else {
      target = resolve(words);
    }
    if (!target) {
      report(`There is no ${noun} “${words.slice(0, 40)}”. Type ${CMD} to see your ${nouns}.`);
      return;
    }
    const named = target.rest.trim().match(/^formation\b\s*([\s\S]*)$/i);
    if (named) {
      await formation(target.box, named[1] ?? '', report);
      return;
    }
    // The earlier phrasing still works: "<folder> create me an app for <purpose>".
    const purpose = purposeOf(target.rest);
    report(await openBox(target.box.box_id));
    if (purpose) report(await buildApp(target.box.box_id, purpose, report));
  },
};

/** Wires boxes onto the started canvas: commands, pane, drops, provenance links, restored cards. */
export function startBoxes(document_: Document): void {
  if (started) return;
  started = true;
  setMakeDataHandler((boxId, file, preview: Preview) => openDataSheet(boxId, file, preview, () => void open.get(boxId)?.refresh(), Boolean(file.data)));
  registerCommands([FOLDER]);
  // The intent bar: the active folder chip, `@` mentions, the folder lane.
  startFolderBar(document_, {
    list: () => boxes,
    refresh: refreshBoxes,
    label: boxLabel,
    itemOf: (boxId) => open.get(boxId)?.item ?? null,
    onActive: (boxId) => {
      for (const entry of open.values()) entry.card.classList.toggle('is-folder-active', entry.boxId === boxId);
    },
    onBuilt: (boxId) => void open.get(boxId)?.refresh(),
  });
  // "+ New folder" in the canvas toolbar.
  const tools = document_.querySelector('.cv-tools');
  if (tools && !tools.querySelector('[data-tool="new-folder"]')) {
    const button = el(document_, 'button', { type: 'button', class: 'cv-tool bx-tool-new', 'data-tool': 'new-folder', title: `New ${noun} (drop files on the canvas to make one too)`, 'aria-label': `New ${noun}` }, `+ ${NOUN}`);
    button.addEventListener('click', () => void newUntitled().then(announce));
    tools.append(el(document_, 'span', { class: 'cv-tools-sep', 'aria-hidden': 'true' }), button);
  }
  for (const slug of [FOLDER_COMMAND, ...FOLDER_ALIASES]) registerCompleter(slug, completions);
  // The intent bar routes spreadsheets here; images stay receipts.
  setFileRouter((file) => {
    if (!isDataFile(file)) return false;
    void dropAnywhere(file);
    return true;
  });
  const host = canvasHost();
  if (host) {
    for (const name of ['dragenter', 'dragover']) {
      host.addEventListener(name, (event) => {
        if (!(event as DragEvent).dataTransfer?.types.includes('Files')) return;
        event.preventDefault();
        host.dataset.fileDrag = 'true';
      });
    }
    host.addEventListener('dragleave', (event) => {
      if (event.target === host) host.dataset.fileDrag = 'false';
    });
    host.addEventListener('drop', (event) => {
      if (!(event as DragEvent).dataTransfer?.types.includes('Files')) return;
      event.preventDefault();
      host.dataset.fileDrag = 'false';
      const list = Array.from((event as DragEvent).dataTransfer?.files ?? []).slice(0, 20);
      if (!list.length) return;
      // Empty canvas: a new folder holds them. On another card: Uploads, as before.
      if ((event.target as Element | null)?.closest('.cv-item')) for (const file of list.slice(0, 5)) void dropAnywhere(file);
      else void newUntitled(list).then(announce);
    });
  }
  document_.addEventListener('click', (event) => {
    const chip = (event.target as Element | null)?.closest('.cv-origin[data-origin="box"]');
    if (chip) void openLineage(chip);
  });
  document_.addEventListener('keydown', (event) => {
    const chip = (event.target as Element | null)?.closest?.('.cv-origin[data-origin="box"]');
    if (chip && (event.key === 'Enter' || event.key === ' ')) {
      event.preventDefault();
      void openLineage(chip);
    }
  });
  // `toggle` does not bubble; the capture phase still reaches the document.
  document_.addEventListener('toggle', (event) => {
    const details = event.target as HTMLElement | null;
    if (details instanceof HTMLDetailsElement && details.classList.contains('cv-receipt') && details.open) void receiptLineage(details);
  }, true);
  onBoardChange(markChips);
  // Apps built from a box get their "Summary" card (`explain.ts`).
  startExplain();
  markChips();
  onLayoutSaved(save);
  void refreshBoxes().then(() => {
    if (isDemoCanvas()) return;
    for (const row of load()) void openBox(row.id, { x: row.x, y: row.y });
  });
}
