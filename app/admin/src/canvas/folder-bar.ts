// APP-108: folders in the intent bar, so nobody needs a command. A folder
// becomes the bar's active folder when its card is clicked or opened (or
// picked from `/folder`): a removable chip "📁 <name> ×" sits in the bar and
// the placeholder reads "Ask about <name>, or build something from it…".
// Typing `@` opens a keyboard-navigable picker of the member's folders;
// picking one adds a one-shot mention chip "@<name> ×" (it wins over the
// active folder for the next sentence). While a folder is active or
// mentioned, a plain sentence goes to that folder:
// - a brief-only sentence ("brief me", "what's in it") → the Brief card
//   (`POST /v1/boxes/{box}/brief`);
// - anything else → `POST /v1/act` with `box_id` (the server's action parser
//   answers `{"via":"folder","app_id","work_id",…}` for an app built from the
//   folder, which is placed); a refusal `not_a_folder_request` falls back to
//   `POST /v1/boxes/{box}/apps {"purpose": sentence}`; any other ordinary act
//   answer goes the usual action path without the folder.
// ×, Esc, or a click on empty canvas clears the active folder. Only the chip
// or a mention ever sends a `box_id`; slash commands never do.
import { setUtteranceRouter, submitAction } from './actions';
import { call, BoxError, makeApp, noun, nouns, refusalText, wantsBrief, type Box } from './box-api';
import { openBrief } from './brief';
import { canvasHost, placeWork } from './board';
import { isModalOpen } from './box-viewer';
import { el, isUuid } from './render';

type Report = (message: string) => void;
type Folder = { box_id: string; name: string };
type Hooks = {
  list(): Box[];
  refresh(): Promise<Box[]>;
  label(box: Box): string;
  /** The folder card's canvas item, if open (the Brief mounts beside it). */
  itemOf(boxId: string): HTMLElement | null;
  /** The active folder changed (the card highlights itself). */
  onActive(boxId: string | null): void;
  /** A folder built an app (its card refreshes "Apps built from this folder"). */
  onBuilt(boxId: string): void;
};

let hooks: Hooks | null = null;
let active: Folder | null = null;
let mention: Folder | null = null;
let input: HTMLInputElement | null = null;
let form: HTMLFormElement | null = null;
let chips: HTMLElement | null = null;
let picker: HTMLUListElement | null = null;
let pickerMode: 'mention' | 'active' = 'mention';
let shown: Box[] = [];
let highlighted = -1;
let placeholder = '';

export function activeFolder(): Folder | null {
  return active;
}

function renderChips(): void {
  if (!chips || !input) return;
  const document_ = input.ownerDocument;
  const chip = (folder: Folder, kind: 'active' | 'mention', clear: () => void) => {
    const remove = el(document_, 'button', { type: 'button', class: 'cv-folder-chip-x', 'aria-label': kind === 'active' ? `Stop using ${noun} ${folder.name}` : `Remove the mention of ${folder.name}` }, '×');
    remove.addEventListener('click', () => {
      clear();
      input?.focus();
    });
    return el(document_, 'span', { class: `cv-folder-chip${kind === 'mention' ? ' is-mention' : ''}`, title: kind === 'active' ? `Sentences go to the ${noun} ${folder.name}` : `This sentence goes to the ${noun} ${folder.name}` },
      el(document_, 'span', { class: 'cv-folder-chip-text' }, kind === 'active' ? `📁 ${folder.name}` : `@${folder.name}`), remove);
  };
  chips.replaceChildren(
    ...(mention ? [chip(mention, 'mention', () => { mention = null; renderChips(); })] : []),
    ...(active && (!mention || mention.box_id !== active.box_id) ? [chip(active, 'active', () => setActiveFolder(null))] : []),
  );
  chips.hidden = chips.childElementCount === 0;
  const target = mention ?? active;
  input.placeholder = target ? `Ask about ${target.name}, or build something from it…` : placeholder;
}

/** Makes `folder` the bar's active folder (null clears it). */
export function setActiveFolder(folder: Folder | null): void {
  if (folder && !isUuid(folder.box_id)) return;
  const changed = (active?.box_id ?? null) !== (folder?.box_id ?? null) || (folder && active?.name !== folder.name);
  active = folder ? { box_id: folder.box_id, name: folder.name.slice(0, 60) } : null;
  if (!changed) return;
  renderChips();
  hooks?.onActive(active?.box_id ?? null);
}

/** A folder's name changed (rename): the chips follow. */
export function folderRenamed(boxId: string, name: string): void {
  if (active?.box_id === boxId) active = { box_id: boxId, name };
  if (mention?.box_id === boxId) mention = { box_id: boxId, name };
  renderChips();
}

// --- The picker (`@` and `/folder`) ---

function closePicker(): void {
  if (!picker || !input) return;
  picker.hidden = true;
  picker.replaceChildren();
  shown = [];
  highlighted = -1;
  input.setAttribute('aria-expanded', 'false');
  input.removeAttribute('aria-activedescendant');
}

function highlight(index: number): void {
  if (!picker || !input) return;
  highlighted = index;
  picker.querySelectorAll('[role="option"]').forEach((option, at) => option.setAttribute('aria-selected', String(at === index)));
  if (index >= 0) input.setAttribute('aria-activedescendant', `cv-folder-opt-${index}`);
  else input.removeAttribute('aria-activedescendant');
}

function renderPicker(query: string): void {
  if (!picker || !input || !hooks) return;
  const words = query.trim().toLowerCase();
  const list = hooks.list();
  shown = list.filter((box) => !words || hooks!.label(box).toLowerCase().includes(words) || String(box.number) === words).slice(0, 8);
  const document_ = input.ownerDocument;
  if (!shown.length) {
    picker.replaceChildren(el(document_, 'li', { class: 'cv-folder-empty', role: 'presentation' },
      list.length ? `No ${noun} matches “${query.slice(0, 30)}”.` : `No ${nouns} yet. Drop files on the canvas to make one.`));
  } else {
    picker.replaceChildren(...shown.map((box, index) => {
      const option = el(document_, 'li', { id: `cv-folder-opt-${index}`, role: 'option', 'aria-selected': 'false', class: 'cv-folder-option' },
        el(document_, 'span', { class: 'cv-folder-num' }, String(box.number)),
        el(document_, 'span', { class: 'cv-folder-name' }, `📁 ${hooks!.label(box)}`),
        el(document_, 'span', { class: 'cv-folder-meta' }, `${box.file_count} ${box.file_count === 1 ? 'file' : 'files'}`));
      option.addEventListener('mousedown', (event) => {
        event.preventDefault();
        choose(index);
      });
      return option;
    }));
  }
  picker.hidden = false;
  input.setAttribute('aria-expanded', 'true');
  highlight(shown.length ? 0 : -1);
}

/** The `@query` being typed before the caret, if any. */
function mentionQuery(): { start: number; query: string } | null {
  if (!input) return null;
  const caret = input.selectionStart ?? input.value.length;
  const before = input.value.slice(0, caret);
  const match = before.match(/(^|\s)@([^@\n]{0,40})$/);
  if (!match) return null;
  return { start: caret - (match[2] ?? '').length - 1, query: match[2] ?? '' };
}

function choose(index: number): void {
  const box = shown[index];
  if (!box || !input || !hooks) return;
  const folder = { box_id: box.box_id, name: hooks.label(box) };
  if (pickerMode === 'active') {
    setActiveFolder(folder);
  } else {
    const found = mentionQuery();
    if (found) {
      const caret = input.selectionStart ?? input.value.length;
      input.value = (input.value.slice(0, found.start) + input.value.slice(caret)).replace(/\s{2,}/g, ' ');
      input.setSelectionRange(found.start, found.start);
    }
    mention = folder;
    renderChips();
  }
  closePicker();
  input.focus();
}

/** `/folder` alone: the picker; choosing makes the folder active. */
export async function openFolderPicker(): Promise<void> {
  if (!input || !hooks) return;
  await hooks.refresh();
  pickerMode = 'active';
  input.value = '';
  input.focus();
  renderPicker('');
}

// --- The folder lane ---

async function route(text: string, report: Report): Promise<boolean> {
  const target = mention ?? active;
  if (!target || !hooks) return false;
  mention = null;
  renderChips();
  if (wantsBrief(text)) {
    report(`Writing a brief of ${target.name} from its files…`);
    report(await openBrief(target.box_id, target.name, true, hooks.itemOf(target.box_id)));
    return true;
  }
  report(`Working from the ${noun} ${target.name}…`);
  let answer: Record<string, unknown>;
  try {
    answer = await call('POST', '/v1/act', { utterance: text, box_id: target.box_id });
  } catch (error) {
    if (error instanceof BoxError && error.code === 'not_a_folder_request') {
      try {
        report(`Generating an app from ${target.name}…`);
        const built = await makeApp(target.box_id, text);
        hooks.onBuilt(target.box_id);
        report(`${built.title}: built from ${target.name}. ${await placeWork(built.app_id, built.work_id, [])}`);
      } catch (inner) {
        report(refusalText(inner));
      }
      return true;
    }
    report(error instanceof BoxError && error.code === 'composer_unavailable'
      ? 'The model is not configured on this server. Nothing was changed.'
      : refusalText(error));
    return true;
  }
  if (answer.via === 'folder' && isUuid(answer.app_id) && isUuid(answer.work_id)) {
    hooks.onBuilt(target.box_id);
    const title = typeof answer.title === 'string' ? answer.title.slice(0, 120) : 'The app';
    report(`${title}: built from ${target.name}. ${await placeWork(answer.app_id, answer.work_id, [])}`);
    return true;
  }
  // An ordinary action (a record, an open): the usual path, without the folder.
  await submitAction(text, report);
  return true;
}

/** Wires the bar: chips, the `@` picker, the folder lane, clearing. */
export function startFolderBar(document_: Document, given: Hooks): void {
  if (hooks) return;
  hooks = given;
  form = document_.getElementById('intent-form') as HTMLFormElement | null;
  input = document_.getElementById('intent-input') as HTMLInputElement | null;
  if (!form || !input) return;
  placeholder = input.placeholder;
  chips = el(document_, 'span', { class: 'cv-folder-chips', hidden: '' });
  input.before(chips);
  picker = el(document_, 'ul', { id: 'cv-folder-picker', class: 'cv-folder-picker', role: 'listbox', 'aria-label': `Your ${nouns}`, hidden: '' });
  form.append(picker);
  input.setAttribute('aria-autocomplete', 'list');
  setUtteranceRouter(route);
  input.addEventListener('input', () => {
    const found = mentionQuery();
    if (found) {
      pickerMode = 'mention';
      if (!hooks?.list().length) void hooks?.refresh().then(() => renderPicker(found.query));
      else renderPicker(found.query);
      input?.setAttribute('aria-controls', 'cv-folder-picker');
      return;
    }
    if (pickerMode === 'active' && picker && !picker.hidden && input && !input.value.startsWith('/')) {
      renderPicker(input.value);
      return;
    }
    closePicker();
  });
  // Capture: the picker's keys come before the bar's submit and the slash list.
  form.addEventListener('keydown', (event) => {
    if (!picker || picker.hidden) {
      if (event.key === 'Backspace' && input && event.target === input && input.selectionStart === 0 && input.selectionEnd === 0) {
        if (mention) { mention = null; renderChips(); event.preventDefault(); }
      }
      if (event.key === 'Escape' && event.target === input && !input?.value && (active || mention)) {
        mention = null;
        setActiveFolder(null);
        renderChips();
        event.preventDefault();
        event.stopPropagation();
      }
      return;
    }
    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      event.preventDefault();
      event.stopPropagation();
      if (shown.length) highlight((highlighted + (event.key === 'ArrowDown' ? 1 : shown.length - 1)) % shown.length);
    } else if ((event.key === 'Enter' || event.key === 'Tab') && highlighted >= 0) {
      event.preventDefault();
      event.stopPropagation();
      choose(highlighted);
    } else if (event.key === 'Escape') {
      event.preventDefault();
      event.stopPropagation();
      closePicker();
    }
  }, true);
  input.addEventListener('blur', () => window.setTimeout(closePicker, 120));
  // Esc anywhere (outside a field and a dialog) clears the active folder.
  document_.addEventListener('keydown', (event) => {
    if (event.key !== 'Escape' || !active || isModalOpen()) return;
    const target = event.target as HTMLElement | null;
    if (target && (target.isContentEditable || ['INPUT', 'TEXTAREA', 'SELECT'].includes(target.tagName))) return;
    setActiveFolder(null);
  });
  // A click on empty canvas (not a pan) clears it too.
  const host = canvasHost();
  if (host) {
    let down: { x: number; y: number } | null = null;
    host.addEventListener('pointerdown', (event) => { down = { x: event.clientX, y: event.clientY }; }, true);
    host.addEventListener('click', (event) => {
      const target = event.target as Element | null;
      const moved = down ? Math.hypot(event.clientX - down.x, event.clientY - down.y) : 0;
      down = null;
      if (!active || moved > 4 || target?.closest('.cv-item, button, a, input, [role="toolbar"], [role="menu"]')) return;
      setActiveFolder(null);
    });
  }
}
