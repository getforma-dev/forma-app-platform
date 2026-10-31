// APP-109 quick notes (Victor: "add a note so one can do /note real quick
// and save a note as a card").
//
// `/note <text>` (or `/note` for an empty one) puts a note card on the canvas:
// a title line and a plain-text body. A note is a typed record (schema
// `note`: title, body, each at most 2,000 characters, storage's text bound)
// of the library's Notes app, kept in a saved work of the member's own
// (`/v1/notes` finds it, or creates it the first time), so it persists across
// browsers and stays private unless the member shares that work. Every write
// goes through the audited, idempotent command route (`add` creates, `edit`
// rewrites the title and body); the notes are read back through the records
// listing. Text is only ever set as a form value or a text node, never as
// markup.
//
// On the canvas a note is edited in place and saved when it loses focus.
// Delete takes it off this canvas (back to the library pane's Utilities →
// Notes, from where it returns); no data changes. Which notes are off the
// canvas, and where each note sits, is kept per member in this browser
// (DS-67); a browser that has not seen a note shows it. A note written in
// another browser arrives live (`record.committed`).
import { canvasHost, isDemoCanvas, mountWidget, onBoardSettled, onLayoutSaved, onLiveEvent, stackBoard, unmountWidget, widgetSpot } from './board';
import { postCommand } from './commands';
import { announce, el, isUuid } from './render';
import type { LiveEvent } from './live';

type Report = (message: string) => void;
type Spot = { x: number; y: number };
type Note = { id: string; version: number; title: string; body: string; at: string };
/** `saving`: the tail of this card's save queue (one command at a time, each on the version the last one produced). */
type Shown = { note: Note; item: HTMLElement; card: HTMLElement; saving: Promise<void> };
type Listing = { publication: string; workVersion: number; schema: string; schemaVersion: number };

export const NOTE_MAX = 2000;
const TITLE_MAX = 200;
const WIDTH = 320;
const MIN_HEIGHT = 220;

let documentRef: Document | null = null;
let work: { app: string; work: string } | null = null;
let listing: Listing | null = null;
const notes = new Map<string, Note>();
const shown = new Map<string, Shown>();
let hidden = new Set<string>();
let spots = new Map<string, Spot>();
const listeners: Array<() => void> = [];

// --- per-member view state in this browser (DS-67): ids and positions only ---

const storageKey = () => `forma.canvas.notes.v1:${stackBoard.member()}`;
function loadView(): void {
  hidden = new Set();
  spots = new Map();
  if (!/^[0-9a-f]{16}$/.test(stackBoard.member())) return;
  try {
    const stored = JSON.parse(localStorage.getItem(storageKey()) ?? 'null') as { hidden?: unknown; spots?: unknown } | null;
    if (Array.isArray(stored?.hidden)) for (const id of stored.hidden.slice(0, 512)) if (isUuid(id)) hidden.add(id);
    if (stored?.spots && typeof stored.spots === 'object') {
      for (const [id, spot] of Object.entries(stored.spots as Record<string, Partial<Spot>>).slice(0, 512)) {
        if (isUuid(id) && Number.isFinite(spot?.x) && Number.isFinite(spot?.y)) spots.set(id, { x: spot.x!, y: spot.y! });
      }
    }
  } catch {
    // view state only
  }
}
function saveView(): void {
  if (!/^[0-9a-f]{16}$/.test(stackBoard.member())) return;
  for (const [id, entry] of shown) {
    const spot = widgetSpot(entry.item);
    if (spot) spots.set(id, spot);
  }
  try {
    localStorage.setItem(storageKey(), JSON.stringify({ hidden: [...hidden], spots: Object.fromEntries(spots) }));
  } catch {
    // view state only
  }
}

function changed(): void {
  for (const listener of listeners) listener();
}

/** Calls `listener` when notes arrive, change or leave the canvas (the library pane's Notes row). */
export function onNotesChange(listener: () => void): void {
  listeners.push(listener);
}

// --- the server: the member's notes work, its records, the command route ---

function freshHex(bytes: number): string {
  const random = new Uint8Array(bytes);
  crypto.getRandomValues(random);
  return Array.from(random, (b) => b.toString(16).padStart(2, '0')).join('');
}
function freshUuid(): string {
  const hex = freshHex(16).split('');
  hex[12] = '4';
  hex[16] = '89ab'[Number.parseInt(hex[16] ?? '0', 16) % 4] ?? '8';
  const s = hex.join('');
  return `${s.slice(0, 8)}-${s.slice(8, 12)}-${s.slice(12, 16)}-${s.slice(16, 20)}-${s.slice(20)}`;
}

async function findWork(create: boolean): Promise<boolean> {
  if (work) return true;
  try {
    const response = await fetch('/v1/notes', { method: create ? 'POST' : 'GET', credentials: 'same-origin', cache: 'no-store' });
    if (!response.ok) return false;
    const body = (await response.json().catch(() => null)) as { app_id?: unknown; work_id?: unknown } | null;
    if (isUuid(body?.app_id) && isUuid(body?.work_id)) {
      work = { app: body.app_id, work: body.work_id };
      return true;
    }
  } catch {
    // no notes yet
  }
  return false;
}

function textOf(values: unknown, field: string): string {
  if (!Array.isArray(values)) return '';
  const item = values.find((v) => (v as { field_id?: unknown })?.field_id === field) as { value?: { type?: unknown; value?: unknown } } | undefined;
  return item?.value?.type === 'text' && typeof item.value.value === 'string' ? item.value.value : '';
}

/** Reads the notes work's records; returns false when they cannot be read. */
async function refresh(): Promise<boolean> {
  if (!work) return false;
  let body: Record<string, unknown> | null = null;
  try {
    const response = await fetch(`/v1/work/${work.app}/${work.work}/records`, { credentials: 'same-origin', cache: 'no-store' });
    if (!response.ok) return false;
    body = (await response.json().catch(() => null)) as Record<string, unknown> | null;
  } catch {
    return false;
  }
  const collection = (Array.isArray(body?.collections) ? body.collections : [])
    .find((c) => (c as { schema_id?: unknown })?.schema_id === 'note') as { schema_id: string; schema_version?: unknown; collection_id?: unknown } | undefined;
  if (!body || !collection || typeof body.publication_id !== 'string') return false;
  listing = {
    publication: body.publication_id,
    workVersion: Number(body.work_version),
    schema: collection.schema_id,
    schemaVersion: Number(collection.schema_version),
  };
  const seen = new Set<string>();
  for (const record of Array.isArray(body.records) ? body.records : []) {
    const row = record as { record_id?: unknown; schema_id?: unknown; version?: unknown; values?: unknown; committed_at?: unknown };
    if (row.schema_id !== 'note' || !isUuid(row.record_id)) continue;
    seen.add(row.record_id);
    const note: Note = {
      id: row.record_id,
      version: Number(row.version) || 0,
      title: textOf(row.values, 'title'),
      body: textOf(row.values, 'body'),
      at: typeof row.committed_at === 'string' ? row.committed_at : '',
    };
    const current = notes.get(note.id);
    if (current && current.version >= note.version) continue;
    notes.set(note.id, note);
    const entry = shown.get(note.id);
    if (entry) {
      // APP-113: text the member has not saved yet (being edited, waiting in
      // the save queue, or refused) stays on the card; the next save writes it
      // on this newer version.
      const unsaved = edited(entry);
      entry.note = note;
      if (!unsaved) paint(entry, false);
    }
  }
  for (const id of [...notes.keys()]) if (!seen.has(id)) notes.delete(id);
  return true;
}

type Outcome = { ok: true; version: number } | { ok: false; conflict: boolean; message: string };

/** One command through the audited route (`commands.ts` `postCommand`: same key on retry, `Retry-After` honored). */
async function command(id: 'add' | 'edit', record: string, version: number, title: string, body: string): Promise<Outcome> {
  if (!work || !listing) return { ok: false, conflict: false, message: 'Notes are not available right now.' };
  const key = freshHex(16);
  const request = {
    kind: 'command_request', schema_version: 1, app_id: work.app, work_id: work.work, publication_id: listing.publication,
    command_id: id, schema_id: listing.schema, schema_version_expected: listing.schemaVersion, record_id: record,
    expected_record_version: version, expected_work_version: listing.workVersion,
    values: [
      { field_id: 'title', value: { type: 'text', value: title } },
      { field_id: 'body', value: body ? { type: 'text', value: body } : { type: 'null' } },
    ],
    idempotency_key: key,
  };
  const outcome = await postCommand(`/v1/work/${work.app}/${work.work}/commands/${id}`, request, key);
  switch (outcome.kind) {
    case 'committed': return { ok: true, version: Number(outcome.recordVersion) || version + 1 };
    case 'conflict':
    case 'stale': return { ok: false, conflict: true, message: 'This note changed elsewhere.' };
    case 'unconfirmed': return { ok: false, conflict: false, message: 'Not saved: the connection dropped. Edit again to retry.' };
    case 'busy': return { ok: false, conflict: false, message: 'Not saved: the note was busy. Edit again to retry.' };
    default: return { ok: false, conflict: false, message: 'Not saved: the note was refused. Nothing was changed.' };
  }
}

// --- the card ---

/** The card's text as a save would send it. */
function cardText(entry: Shown): { title: string; body: string } {
  const titleInput = entry.card.querySelector<HTMLInputElement>('[data-note-title]');
  const bodyInput = entry.card.querySelector<HTMLTextAreaElement>('[data-note-body]');
  return {
    title: (titleInput?.value ?? '').trim().slice(0, TITLE_MAX) || 'Untitled note',
    body: (bodyInput?.value ?? '').slice(0, NOTE_MAX),
  };
}

/** Whether the card holds text not yet saved (focused, differing, or a save pending or refused). */
function edited(entry: Shown): boolean {
  if (entry.card.contains(entry.card.ownerDocument.activeElement)) return true;
  if (entry.card.querySelector('[data-note-status].is-alert')) return true;
  const { title, body } = cardText(entry);
  return title !== entry.note.title || body !== entry.note.body;
}

function paint(entry: Shown, force: boolean): void {
  const title = entry.card.querySelector<HTMLInputElement>('[data-note-title]');
  const body = entry.card.querySelector<HTMLTextAreaElement>('[data-note-body]');
  if (title) title.value = entry.note.title;
  if (body) body.value = entry.note.body;
  entry.card.setAttribute('aria-label', `Note: ${entry.note.title || 'untitled'}`);
  if (force || !entry.card.querySelector('[data-note-status].is-alert')) {
    status(entry, entry.note.version ? `Saved · version ${entry.note.version}` : '');
  }
  sizeBody(entry);
}

function status(entry: Shown, text: string, alert = false): void {
  const line = entry.card.querySelector<HTMLElement>('[data-note-status]');
  if (!line) return;
  line.textContent = text;
  line.setAttribute('role', alert ? 'alert' : 'status');
  line.classList.toggle('is-alert', alert);
}

function sizeBody(entry: Shown): void {
  const body = entry.card.querySelector<HTMLTextAreaElement>('[data-note-body]');
  if (!body) return;
  body.style.height = 'auto';
  body.style.height = `${Math.min(420, Math.max(72, body.scrollHeight))}px`;
}

function buildCard(document_: Document, id: string): HTMLElement {
  const title = el(document_, 'input', {
    type: 'text', class: 'cv-note-title', 'data-note-title': '', maxlength: String(TITLE_MAX), 'aria-label': 'Note title',
    placeholder: 'Title', autocomplete: 'off', spellcheck: 'true',
  });
  const body = el(document_, 'textarea', {
    class: 'cv-note-body', 'data-note-body': '', maxlength: String(NOTE_MAX), rows: '3', 'aria-label': 'Note', placeholder: 'Write a note…',
  });
  return el(document_, 'article', { class: 'cv-card cv-note', 'data-card': `note-${id}`, 'data-kind': 'note', 'data-note': id, 'aria-label': 'Note' },
    el(document_, 'header', { class: 'cv-card-head cv-note-head', 'data-canvas-drag-handle': '' },
      el(document_, 'span', { class: 'cv-note-mark', 'aria-hidden': 'true' }),
      el(document_, 'span', { class: 'cv-note-label' }, 'Note'),
      el(document_, 'span', { class: 'badge', title: 'Only you can read this note unless you share its work' }, 'private'),
      // APP-118: every utility card has ×. The note stays in the library (Utilities → Notes).
      el(document_, 'span', { class: 'cv-card-tools' },
        el(document_, 'button', { type: 'button', class: 'cv-card-focus cv-card-remove', 'data-note-close': '', 'aria-label': 'Take the note off the canvas', title: 'Take off the canvas (the note is kept)' }, '×'))),
    title,
    body,
    el(document_, 'p', { class: 'cv-note-status', 'data-note-status': '', role: 'status' }));
}

/**
 * Saves the card's text when it differs from the stored note (save on blur).
 * APP-113: saves queue per card, so each one sends the version the previous
 * one produced (two quick blurs no longer race on the same version).
 */
function save(entry: Shown): Promise<void> {
  entry.saving = entry.saving.then(() => saveNow(entry)).catch(() => status(entry, 'Not saved. Edit again to retry.', true));
  return entry.saving;
}

async function saveNow(entry: Shown): Promise<void> {
  const { title, body } = cardText(entry);
  if (title === entry.note.title && body === entry.note.body) return;
  status(entry, 'Saving…');
  let outcome = await command('edit', entry.note.id, entry.note.version, title, body);
  if (!outcome.ok && outcome.conflict && (await refresh())) {
    // Changed in another browser: the member's text wins, on the current version.
    const current = notes.get(entry.note.id);
    if (current) outcome = await command('edit', current.id, current.version, title, body);
  }
  if (outcome.ok) {
    entry.note = { ...entry.note, title, body, version: outcome.version };
    notes.set(entry.note.id, entry.note);
    status(entry, `Saved · version ${outcome.version}`);
    changed();
  } else {
    status(entry, outcome.message, true);
  }
}

function show(note: Note, focus = false): HTMLElement | null {
  const existing = shown.get(note.id);
  if (existing) return existing.item;
  if (!documentRef || !canvasHost()) return null;
  const card = buildCard(documentRef, note.id);
  const item = mountWidget(card, { id: `note-${note.id}`, title: 'Note', width: WIDTH, minHeight: MIN_HEIGHT, at: spots.get(note.id), focus });
  if (!item) return null;
  item.classList.add('cv-item-note');
  item.dataset.note = note.id;
  const entry: Shown = { note, item, card, saving: Promise.resolve() };
  shown.set(note.id, entry);
  paint(entry, true);
  card.addEventListener('input', (event) => {
    if ((event.target as Element | null)?.matches('[data-note-body]')) sizeBody(entry);
    status(entry, 'Editing…');
  });
  card.addEventListener('focusout', (event) => {
    if (event.relatedTarget instanceof Node && card.contains(event.relatedTarget)) return;
    void save(entry);
  });
  card.addEventListener('keydown', (event) => {
    if (event.key === 'Enter' && (event.target as Element | null)?.matches('[data-note-title]')) {
      event.preventDefault();
      card.querySelector<HTMLElement>('[data-note-body]')?.focus();
    }
    if (event.key === 'Escape') (event.target as HTMLElement | null)?.blur?.();
  });
  return item;
}

function hide(id: string): void {
  const entry = shown.get(id);
  if (!entry) return;
  shown.delete(id);
  const spot = widgetSpot(entry.item);
  if (spot) spots.set(id, spot);
  unmountWidget(entry.item);
}

// --- what other modules call ---

export function isNoteItem(item: HTMLElement): boolean {
  return item.classList.contains('cv-item-note');
}

/** Delete on the canvas: the note goes back to the library pane (no data changes). */
export function removeNoteItem(item: HTMLElement): boolean {
  const id = item.dataset.note ?? '';
  const entry = shown.get(id);
  if (!entry) return false;
  void save(entry);
  hide(id);
  hidden.add(id);
  saveView();
  changed();
  return true;
}

/** The library pane's Notes row: every note, newest first, and whether it is on the canvas. */
export function noteEntries(): { id: string; title: string; on: boolean }[] {
  return [...notes.values()]
    .sort((a, b) => b.at.localeCompare(a.at))
    .map((note) => ({ id: note.id, title: note.title || 'Untitled note', on: shown.has(note.id) }));
}

/** The library pane: brings a note back, or takes it off the canvas. */
export function toggleNote(id: string): string {
  const note = notes.get(id);
  if (!note) return 'That note is not available.';
  if (shown.has(id)) {
    const entry = shown.get(id);
    if (entry) removeNoteItem(entry.item);
    return `${note.title || 'The note'} is back in the library.`;
  }
  hidden.delete(id);
  show(note, true);
  saveView();
  changed();
  return `${note.title || 'The note'} is on your canvas.`;
}

/** `/note <text>`: a new note through the `add` command; the first line is its title. */
export async function newNote(text: string): Promise<string> {
  if (!(await findWork(true)) || !(listing || (await refresh()))) {
    return 'Notes are not available to you right now. Nothing was saved.';
  }
  const words = text.trim();
  const [first = '', ...rest] = words.split(/\r?\n/);
  let title = first.trim();
  let body = rest.join('\n').trim();
  if (title.length > 120) {
    body = words;
    title = `${title.slice(0, 117).trimEnd()}…`;
  }
  if (!title) title = 'Untitled note';
  if (body.length > NOTE_MAX) return `A note holds at most ${NOTE_MAX.toLocaleString()} characters. Nothing was saved.`;
  const id = freshUuid();
  const outcome = await command('add', id, 0, title.slice(0, TITLE_MAX), body);
  if (!outcome.ok) return outcome.message;
  const note: Note = { id, version: outcome.version, title, body, at: new Date().toISOString() };
  notes.set(id, note);
  const item = show(note, true);
  saveView();
  changed();
  if (item) {
    const field = shown.get(id)?.card.querySelector<HTMLElement>(words ? '[data-note-body]' : '[data-note-title]');
    requestAnimationFrame(() => field?.focus({ preventScroll: true }));
  }
  return words ? 'Note saved on your canvas.' : 'A new note is on your canvas. Give it a title.';
}

export const NOTE_COMMAND = {
  slug: 'note',
  title: 'Note',
  purpose: 'Keep a quick note as a card; saved as your own record.',
  aliases: ['notes'],
  run: async (text: string, report: Report = () => undefined) => {
    report(await newNote(text));
    const input = documentRef?.getElementById('intent-input') as HTMLInputElement | null;
    if (input) input.value = '';
  },
};

function onLive(event: LiveEvent): void {
  if (!work) return;
  // A reconnect (`stream.lagged`) may have missed a note's commit: read them again.
  if (event.kind !== 'stream.lagged' && (event.kind !== 'record.committed' || event.work_id !== work.work)) return;
  void refresh().then((ok) => {
    if (!ok) return;
    // A note written in another browser arrives here too.
    for (const note of notes.values()) if (!hidden.has(note.id) && !shown.has(note.id)) show(note);
    changed();
  });
}

/** Starts notes once (after the board): reads the member's notes and shows those this browser has not put away. */
export function startNotes(document_: Document): void {
  if (documentRef) return;
  documentRef = document_;
  onLiveEvent(onLive);
  onLayoutSaved(saveView);
  document_.addEventListener('click', (event) => {
    const button = (event.target as Element | null)?.closest<HTMLElement>('[data-note-close]');
    const id = button?.closest<HTMLElement>('[data-note]')?.dataset.note ?? '';
    const entry = shown.get(id);
    if (!button || !entry) return;
    event.stopPropagation();
    removeNoteItem(entry.item);
    announce(`${entry.note.title || 'The note'} is back in the library (Utilities → Notes). Nothing was deleted.`);
  });
  onBoardSettled(() => {
    loadView();
    void findWork(false).then(async (found) => {
      if (found && (await refresh()) && !isDemoCanvas()) {
        for (const note of [...notes.values()].sort((a, b) => a.at.localeCompare(b.at))) if (!hidden.has(note.id)) show(note);
      }
      changed();
    });
  });
}
