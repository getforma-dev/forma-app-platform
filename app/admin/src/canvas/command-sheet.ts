// APP-118: the commands list (`/commands`, `/help`, and the link at the top
// of the Shortcuts tab). APP-119: it is the Commands tab of the right pane
// (`pane.ts`), beside the Library and Shortcuts tabs, never a canvas card
// and no longer a sheet of its own. It lists the verbs only, by category,
// each with its argument signature, description, aliases and examples, and
// a search box; its footer says where apps are: "Your apps answer to /name
// — see /apps". Clicking a command (or one of its examples) puts it in the
// bar and closes the pane; Esc clears the search, then closes the pane, and
// focus returns to what opened it (the APP-113 pattern). APP-120: the
// twelve verbs in four groups (Place, Arrange, Make, Help) under the one
// sentence of the grammar; a verb's hidden aliases show on its row ("also
// /weather, /calculator, /clock, /note" under `/widget`). Built with `el()`
// only: text nodes, no markup.
import { ADDRESS_FOOTER, GRAMMAR, argLabel, groupCommands, hiddenAliases, searchCommands, type Command, type CommandGroup } from './command-registry';
import { closePane, isPaneOpen, onPaneTab, openPane, paneTab } from './pane';
import { el } from './render';

let search: HTMLInputElement | null = null;
let list: HTMLElement | null = null;

/** The tab's sections for `query`: the registry's verbs, grouped by category (exported for the tests). */
export function sheetGroups(query: string): CommandGroup[] {
  return groupCommands(searchCommands(query));
}

/** Whether the pane is open on its Commands tab. */
export function isCommandSheetOpen(): boolean {
  return isPaneOpen() && paneTab() === 'commands';
}

/** Puts `text` in the intent bar (the autocomplete then shows its hint) and closes the pane. */
export function insertCommand(text: string): void {
  const input = document.getElementById('intent-input') as HTMLInputElement | null;
  closePane(false);
  if (!input) return;
  input.value = text;
  input.focus();
  input.setSelectionRange(text.length, text.length);
  input.dispatchEvent(new Event('input', { bubbles: true }));
}

function commandRow(document_: Document, command: Command): HTMLElement {
  const name = el(document_, 'button', { type: 'button', class: 'cv-commands-item', title: `Put /${command.slug} in the bar` },
    el(document_, 'code', { class: 'cv-commands-sig' },
      // APP-122: Ask is typed `? <question>`, not `/ask`.
      ...(command.usage?.startsWith('?')
        ? [el(document_, 'strong', {}, '?'), el(document_, 'span', { class: 'cv-commands-arg is-required' }, command.usage.slice(1))]
        : [el(document_, 'strong', {}, `/${command.slug}`)]),
      ...(command.usage?.startsWith('?') ? [] : command.usage
        ? [el(document_, 'span', { class: 'cv-commands-arg is-required' }, command.usage.slice(command.slug.length + 1))]
        : command.args.map((argument) => el(document_, 'span', { class: argument.required ? 'cv-commands-arg is-required' : 'cv-commands-arg' }, ` ${argLabel(argument)}`)))),
    el(document_, 'span', { class: 'cv-commands-title' }, command.title),
    el(document_, 'span', { class: 'cv-commands-purpose' }, command.purpose));
  name.addEventListener('click', () => insertCommand(command.insert ?? (command.args.length ? `/${command.slug} ` : `/${command.slug}`)));
  const examples = command.examples.filter((example) => example !== `/${command.slug}` || command.args.length === 0).slice(0, 2);
  const also = [...hiddenAliases(command.slug).map((alias) => alias.slug), ...command.aliases];
  const meta = el(document_, 'div', { class: 'cv-commands-meta' },
    ...(also.length ? [el(document_, 'span', { class: 'cv-commands-aliases' }, `also ${also.map((alias) => `/${alias}`).join(', ')}`)] : []),
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

/** Fills the Commands tab's panel (once): the intro, search, list and the footer. */
function build(document_: Document): boolean {
  if (search) return true;
  const body = document_.getElementById('cv-panel-commands');
  if (!body) return false;
  search = el(document_, 'input', {
    type: 'search', class: 'cv-commands-search', placeholder: 'Search commands', 'aria-label': 'Search commands',
    'aria-controls': 'cv-commands-list', autocomplete: 'off', spellcheck: 'false',
  });
  list = el(document_, 'div', { id: 'cv-commands-list', class: 'cv-commands-list' });
  const apps = el(document_, 'button', { type: 'button', class: 'cv-commands-footer-link', 'data-pane-goto': 'library' }, 'see /apps');
  apps.addEventListener('click', () => openPane('library'));
  const [lead] = ADDRESS_FOOTER.split(' — ');
  body.replaceChildren(
    el(document_, 'p', { class: 'cv-commands-grammar', 'data-commands-grammar': '' }, GRAMMAR),
    el(document_, 'p', { class: 'cv-commands-sub' }, 'Click one to put it in the bar. <required> and [optional] arguments.'),
    search,
    list,
    el(document_, 'p', { class: 'cv-commands-footer', 'data-commands-footer': '' }, el(document_, 'span', {}, `${lead} — `), apps));
  search.addEventListener('input', renderList);
  onPaneTab('commands', {
    show: (focus) => {
      renderList();
      if (focus) search?.focus({ preventScroll: true });
    },
    escape: () => {
      if (!search?.value) return false;
      search.value = '';
      renderList();
      return true;
    },
  });
  return true;
}

/** Opens the pane on Commands (filtered by `query`); focus returns to `from` (else what had it) on close. */
export function openCommandSheet(query = '', from: HTMLElement | null = null): void {
  if (!build(document)) return;
  if (search) search.value = query;
  openPane('commands', { focus: true, from });
}

/** Closes the pane; `restore` hands focus back to what opened it. */
export function closeCommandSheet(restore = true): void {
  closePane(restore);
}

/** Wires the Commands tab at start, so a click on the tab finds it filled. */
export function startCommandSheet(document_: Document): void {
  build(document_);
}
