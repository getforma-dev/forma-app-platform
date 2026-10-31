// APP-118: the command sheet (`/commands`, `/help`, `?` in the empty bar, and
// the link in the `?` shortcuts sheet). A modal side sheet, never a canvas
// card: the registry's commands by category, each with its argument
// signature, description, aliases and examples, and a search box. Clicking a
// command (or one of its examples) puts it in the bar and closes the sheet;
// Esc or × close it. Focus stays inside while it is open and returns to what
// opened it (the APP-113 pattern). Built with `el()` only: text nodes, no
// markup.
import { argLabel, groupCommands, searchCommands, type Command, type CommandGroup } from './command-registry';
import { trapTab } from './focus';
import { el } from './render';

let sheet: HTMLElement | null = null;
let search: HTMLInputElement | null = null;
let list: HTMLElement | null = null;
let opener: HTMLElement | null = null;

/** The sheet's sections for `query`: the registry's commands, grouped by category (exported for the tests). */
export function sheetGroups(query: string): CommandGroup[] {
  return groupCommands(searchCommands(query));
}

export function isCommandSheetOpen(): boolean {
  return Boolean(sheet && !sheet.hidden);
}

/** Puts `text` in the intent bar (the autocomplete then shows its hint) and closes the sheet. */
export function insertCommand(text: string): void {
  const input = document.getElementById('intent-input') as HTMLInputElement | null;
  closeCommandSheet(false);
  if (!input) return;
  input.value = text;
  input.focus();
  input.setSelectionRange(text.length, text.length);
  input.dispatchEvent(new Event('input', { bubbles: true }));
}

function commandRow(document_: Document, command: Command): HTMLElement {
  const name = el(document_, 'button', { type: 'button', class: 'cv-commands-item', title: `Put /${command.slug} in the bar` },
    el(document_, 'code', { class: 'cv-commands-sig' },
      el(document_, 'strong', {}, `/${command.slug}`),
      ...command.args.map((argument) => el(document_, 'span', { class: argument.required ? 'cv-commands-arg is-required' : 'cv-commands-arg' }, ` ${argLabel(argument)}`))),
    el(document_, 'span', { class: 'cv-commands-title' }, command.title),
    el(document_, 'span', { class: 'cv-commands-purpose' }, command.purpose));
  name.addEventListener('click', () => insertCommand(command.args.length ? `/${command.slug} ` : `/${command.slug}`));
  const examples = command.examples.filter((example) => example !== `/${command.slug}` || command.args.length === 0).slice(0, 2);
  const meta = el(document_, 'div', { class: 'cv-commands-meta' },
    ...(command.aliases.length ? [el(document_, 'span', { class: 'cv-commands-aliases' }, `also ${command.aliases.map((alias) => `/${alias}`).join(', ')}`)] : []),
    ...examples.map((example) => {
      const button = el(document_, 'button', { type: 'button', class: 'cv-commands-example', title: 'Put this example in the bar' }, el(document_, 'code', {}, example));
      button.addEventListener('click', () => insertCommand(example));
      return button;
    }));
  return el(document_, 'li', { class: 'cv-commands-row', 'data-command': command.slug }, name, meta);
}

function renderList(): void {
  if (!list || !search) return;
  const document_ = list.ownerDocument;
  const groups = sheetGroups(search.value);
  list.replaceChildren(...groups.map((group) => el(document_, 'section', { class: 'cv-commands-cat', 'aria-label': group.category },
    el(document_, 'h3', {}, group.category, el(document_, 'span', { class: 'cv-commands-count' }, String(group.commands.length))),
    el(document_, 'ul', {}, ...group.commands.map((command) => commandRow(document_, command))))));
  if (!groups.length) {
    list.append(el(document_, 'p', { class: 'cv-commands-none', role: 'status' }, `No command matches “${search.value.trim().slice(0, 40)}”. Or just say what you need in the bar.`));
  }
}

function build(document_: Document): HTMLElement {
  const close = el(document_, 'button', { type: 'button', class: 'cv-lib-close', 'aria-label': 'Close the commands', title: 'Close (Esc)' }, '×');
  search = el(document_, 'input', {
    type: 'search', class: 'cv-commands-search', placeholder: 'Search commands', 'aria-label': 'Search commands',
    'aria-controls': 'cv-commands-list', autocomplete: 'off', spellcheck: 'false',
  });
  list = el(document_, 'div', { id: 'cv-commands-list', class: 'cv-commands-list' });
  const panel = el(document_, 'div', { class: 'cv-commands-panel' },
    el(document_, 'header', { class: 'cv-commands-head' },
      el(document_, 'div', {},
        el(document_, 'h2', { id: 'cv-commands-title' }, 'Commands'),
        el(document_, 'p', { class: 'cv-commands-sub' }, 'Type / in the bar, or click one to put it there. <required> and [optional] arguments.')),
      close),
    search,
    list);
  const root = el(document_, 'div', { id: 'cv-commands', class: 'cv-commands', role: 'dialog', 'aria-modal': 'true', 'aria-labelledby': 'cv-commands-title', hidden: '' }, panel);
  close.addEventListener('click', () => closeCommandSheet());
  root.addEventListener('click', (event) => {
    if (event.target === root) closeCommandSheet();
  });
  root.addEventListener('keydown', (event) => {
    if (event.key === 'Escape') {
      event.preventDefault();
      event.stopPropagation();
      closeCommandSheet();
    } else if (event.key === 'Tab') {
      trapTab(event, panel);
    }
  });
  search.addEventListener('input', renderList);
  (document_.getElementById('cv-shell') ?? document_.body).append(root);
  return root;
}

/** Opens the sheet (filtered by `query`); focus returns to `from` (else what had it) on close. */
export function openCommandSheet(query = '', from: HTMLElement | null = null): void {
  const document_ = document;
  if (!sheet) sheet = build(document_);
  if (sheet.hidden) opener = from ?? (document_.activeElement as HTMLElement | null);
  if (search) search.value = query;
  renderList();
  sheet.hidden = false;
  search?.focus({ preventScroll: true });
}

/** Closes the sheet; `restore` hands focus back to what opened it. */
export function closeCommandSheet(restore = true): void {
  if (!sheet || sheet.hidden) return;
  sheet.hidden = true;
  if (restore && opener?.isConnected) opener.focus({ preventScroll: true });
  opener = null;
}
