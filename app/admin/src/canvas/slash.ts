// APP-101 slash commands: the intent bar's explicit lane, instant and never
// a guess. APP-118: every command lives in one registry
// (`command-registry.ts`: name, aliases, category, description, typed
// arguments, examples), fed by the canvas modules (`registerCommands`).
// APP-119: commands are verbs, apps are addresses. The member's apps from
// the server (`GET /v1/commands`: every library app with its slug, aliases,
// derived hint and examples, and the member's named saved apps) are
// addresses (`setAddresses`). This module draws the autocomplete: `/` alone
// lists the verbs by category, each with its argument signature; from the
// first letter the verbs that match come first, then a separate section
// "Apps — type /name and what you want" with the matching apps on the
// canvas and the saved apps (an exact slug or alias of any app is always
// offered). After `/name ` a hint row shows the argument being typed and an
// example, and live suggestions appear for the arguments that have values
// (apps, cards, folders, cities, zones; each module registers its own with
// `registerCompleter`). `/commands` opens Commands. APP-122: `?` as the
// first character is Ask (the hint says so; Enter opens the Ask tab with the
// question, `talk.ts`), and `/<app>.<card>` lists that app's cards; after a
// space the words shape the card (the dot is the rule).
// APP-120: `/` alone lists the twelve verbs in four groups (Place, Arrange,
// Make, Help); from the first letter the hidden aliases (`/weather`, `/note`)
// match too and the Apps section joins. A bare `/<address>` places the app
// (the same as `/app <address>`, through `setAppPlacer`); `/<address> words`
// tells that app something, exactly as before. While an address is typed
// the hint reads "/expenses — Expenses · file a claim or approve one".
//
// `/name rest` runs that command: a library app opens at once through the
// pinned compose (`{"intent", "app_id"}`, no routing, no model call) and
// `rest` is tailored by the live transform, or, for a record app when `rest`
// reads like an entry or a decision, goes to the action path pinned to that
// app; a named saved app opens its work directly. The near-tie chooser
// (`showChooser`) lives here too: the top apps plus "Build something new",
// one click proceeds.
//
// Nothing from the server is rendered as markup: titles and purposes are
// text nodes; ids are checked UUIDs.
import { submitAction } from './actions';
import { addressLine } from './address-chip';
import { boardWorks, placeWork } from './board';
import { openCommandSheet } from './command-sheet';
import {
  APPS_SECTION, argumentHint, findCommand, type CardLabel, groupCommands, matchAddresses, matchCommands, registerCommands, setAddresses, signature,
  type Address, type CanvasCommand, type CommandGroup, type Runnable,
} from './command-registry';
import { submitIntent, type Choice } from './compose';
import { isDismissKey, showDockCard } from './dock-card';
import { el, isUuid } from './render';
import { openShortcuts } from './shortcut-tab';

export { registerCommands } from './command-registry';
export type { CanvasCommand } from './command-registry';

export const COMMANDS_URL = '/v1/commands';
type Report = (message: string) => void;

/**
 * APP-104: an argument suggestion under the bar (`/card 3.2`, `/app expense`):
 * a short badge (the number), a title, a detail line and the text it runs.
 * A `value` ending in a space only fills the bar (`/folder new `).
 */
export type Completion = { badge: string; title: string; detail: string; value: string };
type Completer = { complete: (rest: string) => Completion[]; select: boolean };
const completers = new Map<string, Completer>();
/**
 * Offers suggestions for `/slug <rest>`. With `select: false` none is
 * highlighted until an arrow key, so Enter runs what was typed (`/weather`).
 */
export function registerCompleter(slug: string, complete: (rest: string) => Completion[], options: { select?: boolean } = {}): void {
  completers.set(slug, { complete, select: options.select ?? true });
}

/** The Expenses app's addresses (canonical and the kept alias): its words always go to the action path. */
const EXPENSES = ['expenses', 'expense'];

/**
 * The suggestions for `/name rest`: the name's own completer, else its verb's;
 * APP-120: after `/widget <kind> ` the kind's own (`/widget weather Au` lists
 * cities as `/weather Au` does).
 */
function completerFor(name: string, slug: string, rest: string): { completer: Completer | undefined; rest: string } {
  if (slug === 'widget') {
    const kind = rest.match(/^(\S+)\s+([\s\S]*)$/);
    const own = kind ? completers.get((kind[1] ?? '').toLowerCase()) : undefined;
    if (own) return { completer: own, rest: kind?.[2] ?? '' };
  }
  return { completer: completers.get(name) ?? completers.get(slug), rest };
}

/** Text that reads like a record entry or a decision (the action path's lane). */
const ENTRY = /\b(approve|approved|reject|rejected|decline|deny|accept|file|submit|spent|paid|log|record|add|sign off)\b|\$\s?\d/i;

/**
 * APP-120: a bare `/<address>` places the app, as `/app <address>` does. The
 * Library registers the placer (it knows the app's rows and cards); it
 * answers false when it has no row for the app, and the pinned compose
 * places it instead.
 */
type Placer = (address: Address, report: Report) => Promise<boolean>;
let placeApp: Placer | null = null;
export function setAppPlacer(placer: Placer): void {
  placeApp = placer;
}

export type ServerCommand = {
  slug: string; aliases: string[]; title: string; purpose: string;
  kind: 'library' | 'saved'; app_id: string; work_id?: string; record: boolean;
  hint: string; examples: string[];
  /** APP-121: the organization's labels and the member's own shortcuts (optional: an older server sends none). */
  description?: string; org_aliases?: string[]; mine?: string[]; can_label?: boolean; cards?: CardLabel[];
};

/** The hint when the server sent none (an older server): what words can do. */
const fallbackHint = (record: boolean) => (record ? 'what to show or file' : 'what to show');

/** An app from the server as an address: `/slug words` opens it, files or decides (APP-101/102). */
export function serverAddress(entry: ServerCommand): Address {
  const hint = entry.hint || fallbackHint(entry.record);
  return {
    slug: entry.slug,
    aliases: entry.aliases,
    title: entry.title,
    purpose: entry.purpose,
    hint,
    kind: entry.kind,
    app: entry.app_id,
    work: entry.work_id,
    record: entry.record,
    args: [{ name: hint, kind: 'text', required: false }],
    examples: entry.examples.length ? entry.examples : [`/${entry.slug}`],
    description: entry.description ?? '',
    orgAliases: entry.org_aliases ?? [],
    mine: entry.mine ?? [],
    canLabel: entry.can_label === true,
    cards: entry.cards ?? [],
    run: async (text: string, report: Report = () => undefined) => {
      const rest = text.trim();
      if (entry.kind === 'saved' && entry.work_id) {
        report(await placeWork(entry.app_id, entry.work_id));
        if (rest && entry.record && ENTRY.test(rest)) await submitAction(rest, report, entry.app_id);
        return;
      }
      // APP-120: a bare address places the app (`/expenses` = `/app expenses`).
      if (!rest && placeApp && (await placeApp(serverAddress(entry), report))) return;
      // `/expenses <words>` (and `/expense <words>`) is always the action path
      // for that app (APP-102 with APP-101).
      if (rest && [entry.slug, ...entry.aliases].some((name) => EXPENSES.includes(name))) {
        await submitAction(rest, report, entry.app_id);
        return;
      }
      if (rest && entry.record && ENTRY.test(rest)) {
        await submitAction(rest, report, entry.app_id);
        return;
      }
      await submitIntent(rest.length >= 3 ? rest : entry.title, report, { app_id: entry.app_id });
    },
  };
}

const NAME = /^[a-z][a-z0-9-]{1,31}$/;
const names = (value: unknown, limit: number): string[] =>
  (Array.isArray(value) ? value.filter((a): a is string => typeof a === 'string' && NAME.test(a)).slice(0, limit) : []);
const short = (value: unknown, limit: number): string => (typeof value === 'string' ? value.slice(0, limit) : '');

/** APP-121: the labeled cards of an entry (`[{card, name, description, my_name, mine}]`). */
function parseCards(value: unknown): CardLabel[] {
  if (!Array.isArray(value)) return [];
  return value.flatMap((item): CardLabel[] => {
    const card = (item as Record<string, unknown> | null)?.card;
    if (typeof card !== 'string' || !/^[a-z0-9][a-z0-9-]{0,47}$/.test(card)) return [];
    const entry = item as Record<string, unknown>;
    return [{ card, name: short(entry.name, 60), description: short(entry.description, 280), myName: short(entry.my_name, 60), mine: names(entry.mine, 8) }];
  }).slice(0, 64);
}

/** Parses `GET /v1/commands` (exported for the tests); `hint` and `examples` are optional. */
export function parseCommands(body: unknown): ServerCommand[] {
  const list = (body as { commands?: unknown } | null)?.commands;
  if (!Array.isArray(list)) return [];
  return list.flatMap((value): ServerCommand[] => {
    if (typeof value !== 'object' || value === null) return [];
    const entry = value as Record<string, unknown>;
    if (typeof entry.slug !== 'string' || !/^[a-z][a-z0-9-]{1,31}$/.test(entry.slug) || !isUuid(entry.app_id)) return [];
    if (entry.kind !== 'library' && entry.kind !== 'saved') return [];
    if (entry.kind === 'saved' && !isUuid(entry.work_id)) return [];
    return [{
      slug: entry.slug,
      aliases: names(entry.aliases, 24),
      title: String(entry.title ?? '').slice(0, 120),
      purpose: String(entry.purpose ?? '').slice(0, 200),
      kind: entry.kind,
      app_id: entry.app_id,
      work_id: typeof entry.work_id === 'string' ? entry.work_id : undefined,
      record: entry.record === true,
      hint: typeof entry.hint === 'string' ? entry.hint.slice(0, 160) : '',
      examples: Array.isArray(entry.examples)
        ? entry.examples.filter((e): e is string => typeof e === 'string' && e.startsWith('/')).slice(0, 4).map((e) => e.slice(0, 200))
        : [],
      description: short(entry.description, 280),
      org_aliases: names(entry.org_aliases, 8),
      mine: names(entry.mine, 8),
      can_label: entry.can_label === true,
      cards: parseCards(entry.cards),
    }];
  });
}

let loaded: Promise<void> | null = null;
/** Re-reads the member's commands from the server. */
export function refreshCommands(): Promise<void> {
  loaded = fetch(COMMANDS_URL, { credentials: 'same-origin', cache: 'no-store' })
    .then((response) => (response.ok ? response.json() : null))
    .then((body) => {
      setAddresses(parseCommands(body).map(serverAddress));
    })
    .catch(() => undefined);
  return loaded;
}
/** The member's commands, read once (the Library asks too: its rows show each app's address). */
export function ensureCommands(): Promise<void> {
  return loaded ?? refreshCommands();
}

/** `/name rest`: runs the command, or explains how to see the list. */
export async function runSlash(text: string, report: Report): Promise<void> {
  const match = text.match(/^\/([a-z0-9-]{1,32})(?:\s+([\s\S]*))?$/i);
  if (!match) {
    report('Type / to see your commands.');
    return;
  }
  await ensureCommands();
  const name = (match[1] ?? '').toLowerCase();
  const command = findCommand(name);
  if (!command) {
    report(`There is no /${name} command. Type / to see yours, or /commands for the list.`);
    return;
  }
  await command.run(match[2] ?? '', report);
}

/** The `/` list's verbs for `prefix`, grouped by category (exported for the tests). */
export function slashGroups(prefix: string): CommandGroup[] {
  return groupCommands(matchCommands(prefix));
}

/**
 * Decision 4 (APP-119): the canvas layout lives in this browser, so the
 * `/x…` list's Apps section shows the apps on the canvas and the saved
 * (named) apps; `matchAddresses` still offers an exact slug of any app.
 */
export function listedAddress(address: Address, works = boardWorks()): boolean {
  return address.kind === 'saved' || works.some((work) => work.app === address.app);
}

/** The `/x…` list's Apps section for `prefix` (empty for `/` alone; exported for the tests). */
export function slashAddresses(prefix: string, listed: (address: Address) => boolean = (address) => listedAddress(address)): Address[] {
  return matchAddresses(prefix, listed);
}

/** Shown when none of the member's placed or saved apps match: library apps
 *  they could address, so `/e` on a clean canvas still finds `/expense`. */
export const LIBRARY_SECTION = 'From your library — type /name and what you want';
const LIBRARY_SUGGESTIONS = 5;

/** The Apps section of the dropdown: placed and saved apps first; when none
 *  match, up to five matching library apps under their own heading. */
export function slashAddressSection(
  prefix: string,
  listed: (address: Address) => boolean = (address) => listedAddress(address),
): { title: string; addresses: Address[] } {
  const own = slashAddresses(prefix, listed);
  if (own.length || !prefix) return { title: APPS_SECTION, addresses: own };
  return { title: LIBRARY_SECTION, addresses: matchAddresses(prefix).slice(0, LIBRARY_SUGGESTIONS) };
}

/** `/commands [search]` (aliases `/help`): the right pane's Commands tab, never a canvas card. */
export const COMMANDS: CanvasCommand = {
  slug: 'commands',
  aliases: ['help'],
  title: 'All commands',
  purpose: 'Every command with its arguments and examples; click one to put it in the bar',
  run: (text: string, report: Report = () => undefined) => {
    const input = document.getElementById('intent-input') as HTMLInputElement | null;
    if (input) input.value = '';
    openCommandSheet(text.trim(), input);
    report('Every command is in the Commands tab. Click one to put it in the bar.');
  },
};

/** The right pane on its Shortcuts tab (`/shortcuts`, ⋯ → Shortcuts; APP-122: `?` is Ask now). */
export function openHelp(from: HTMLElement | null): void {
  openShortcuts(from);
}

// --- The near-tie chooser ---

/** Shows the top apps for a near tie plus "Build something new"; one click proceeds. */
export function showChooser(text: string, choices: Choice[], report: Report): void {
  const document_ = document;
  let close = () => undefined as void;
  const options = el(document_, 'div', { class: 'cv-chooser-options' });
  const pick = (run: () => Promise<void>) => {
    close();
    void run();
  };
  for (const choice of choices) {
    const button = el(document_, 'button', { type: 'button', class: 'cv-chooser-option' },
      el(document_, 'strong', {}, choice.title),
      el(document_, 'span', {}, choice.purpose));
    button.addEventListener('click', () => pick(() => submitIntent(text, report, { app_id: choice.app_id })));
    options.append(button);
  }
  const build = el(document_, 'button', { type: 'button', class: 'cv-chooser-option cv-chooser-new' },
    el(document_, 'strong', {}, 'Build something new'),
    el(document_, 'span', {}, 'Compose a new app for this request (generated, labeled as such)'));
  build.addEventListener('click', () => pick(() => submitIntent(text, report, { generate: true })));
  options.append(build);
  const cancel = el(document_, 'button', { type: 'button', class: 'cv-secondary' }, 'Cancel');
  cancel.addEventListener('click', () => {
    close();
    report('Nothing was placed.');
  });
  const card = el(document_, 'section', { class: 'cv-card cv-confirm cv-chooser', role: 'dialog', 'aria-modal': 'false', 'aria-labelledby': 'cv-chooser-title' },
    el(document_, 'header', { class: 'cv-card-head' }, el(document_, 'h3', { id: 'cv-chooser-title' }, 'Which app did you mean?')),
    el(document_, 'p', { class: 'cv-confirm-quote' }, `“${text}”`),
    options,
    el(document_, 'div', { class: 'cv-confirm-actions' }, cancel));
  card.addEventListener('keydown', (event) => {
    if (isDismissKey(event)) {
      event.preventDefault();
      close();
      report('Nothing was placed.');
    }
  });
  // A card that replaces this one reports for itself; nothing waits on the chooser.
  close = showDockCard(document_, card, () => undefined);
  options.querySelector<HTMLButtonElement>('button')?.focus();
}

// --- The autocomplete under the intent bar ---

type Entry = { kind: 'command'; command: Runnable } | { kind: 'completion'; completion: Completion };

/** Wires the `/` autocomplete onto the intent bar (once). */
export function startSlash(document_: Document): void {
  const input = document_.getElementById('intent-input') as HTMLInputElement | null;
  const form = document_.getElementById('intent-form') as HTMLFormElement | null;
  if (!input || !form || input.dataset.slash === 'on') return;
  input.dataset.slash = 'on';
  registerCommands([COMMANDS]);
  const hint = el(document_, 'div', { id: 'cv-slash-hint', class: 'cv-slash-hint', hidden: '' });
  const list = el(document_, 'ul', { id: 'cv-slash', class: 'cv-slash', role: 'listbox', 'aria-label': 'Commands', hidden: '' });
  const pop = el(document_, 'div', { class: 'cv-slash-pop', hidden: '' }, hint, list);
  form.append(pop);
  input.setAttribute('role', 'combobox');
  input.setAttribute('aria-autocomplete', 'list');
  input.setAttribute('aria-controls', 'cv-slash');
  input.setAttribute('aria-expanded', 'false');
  let entries: Entry[] = [];
  let active = -1;
  let hintKey = '';

  const options = () => Array.from(list.querySelectorAll<HTMLElement>('[role="option"]'));
  const sync = () => {
    pop.hidden = list.hidden && hint.hidden;
    input.setAttribute('aria-expanded', list.hidden ? 'false' : 'true');
  };
  // The bar keeps its own description (the status line); the hint joins it while shown.
  const describe = (on: boolean) => {
    const ids = (input.getAttribute('aria-describedby') ?? '').split(/\s+/).filter((id) => id && id !== 'cv-slash-hint');
    if (on) ids.push('cv-slash-hint');
    if (ids.length) input.setAttribute('aria-describedby', ids.join(' '));
    else input.removeAttribute('aria-describedby');
  };
  const hideHint = () => {
    hint.hidden = true;
    hintKey = '';
    describe(false);
  };
  const close = () => {
    list.hidden = true;
    entries = [];
    hideHint();
    input.removeAttribute('aria-activedescendant');
    active = -1;
    sync();
  };
  const highlight = (index: number) => {
    active = index;
    options().forEach((option, at) => option.setAttribute('aria-selected', at === index ? 'true' : 'false'));
    if (index >= 0) input.setAttribute('aria-activedescendant', `cv-slash-${index}`);
    else input.removeAttribute('aria-activedescendant');
    options()[index]?.scrollIntoView({ block: 'nearest' });
  };
  const fill = (value: string) => {
    input.value = value;
    input.focus();
    render();
  };
  const run = (value: string) => {
    input.value = value;
    close();
    form.requestSubmit();
  };
  const accept = (entry: Entry, key: 'Tab' | 'Enter' | 'click') => {
    if (entry.kind === 'command') {
      // A command with no arguments runs on Enter; otherwise its name fills the bar.
      if (key !== 'Tab' && entry.command.args.length === 0) run(`/${entry.command.slug}`);
      else fill(`/${entry.command.slug} `);
      return;
    }
    const value = entry.completion.value;
    if (key === 'Tab' || value.endsWith(' ')) fill(value);
    else run(value);
  };
  const option = (entry: Entry, index: number, ...children: Node[]) => {
    const node = el(document_, 'li', { id: `cv-slash-${index}`, role: 'option', 'aria-selected': 'false', class: `cv-slash-option${entry.kind === 'completion' ? ' cv-slash-completion' : ''}` }, ...children);
    node.addEventListener('mousedown', (event) => {
      event.preventDefault();
      accept(entry, 'click');
    });
    return node;
  };
  const showHint = (command: Runnable, rest: string) => {
    const shown = argumentHint(command, rest);
    const address = 'hint' in command && 'kind' in command ? command as Address : null;
    const key = JSON.stringify({ shown, address: address?.slug });
    if (key !== hintKey && address) {
      // APP-120: an address reads "/expenses — Expenses · file a claim or approve one".
      hintKey = key;
      hint.replaceChildren(
        addressLine(document_, address),
        ...(shown.example ? [el(document_, 'span', { class: 'cv-slash-example' }, 'e.g. ', el(document_, 'code', {}, shown.example))] : []));
    } else if (key !== hintKey) {
      hintKey = key;
      hint.replaceChildren(
        el(document_, 'code', { class: 'cv-slash-sig' }, ...shown.parts.flatMap((part, at) => [
          ...(at ? [document_.createTextNode(' ')] : []),
          part.current ? el(document_, 'mark', {}, part.text) : document_.createTextNode(part.text),
        ])),
        el(document_, 'span', { class: 'cv-slash-note' }, shown.note),
        ...(shown.example ? [el(document_, 'span', { class: 'cv-slash-example' }, 'e.g. ', el(document_, 'code', {}, shown.example))] : []));
    }
    hint.hidden = false;
    describe(true);
  };
  const renderCompletions = (found: Completion[], select: boolean) => {
    entries = found.map((completion) => ({ kind: 'completion', completion }));
    list.replaceChildren(...found.map((completion, index) => option(entries[index]!, index,
      el(document_, 'span', { class: 'cv-slash-name cv-slash-badge' }, completion.badge),
      el(document_, 'span', { class: 'cv-slash-title' }, completion.title),
      el(document_, 'span', { class: 'cv-slash-purpose' }, completion.detail))));
    list.hidden = found.length === 0;
    highlight(found.length && select ? 0 : -1);
  };
  const render = () => {
    const value = input.value;
    // APP-122: `?` first is Ask; the hint names the job (Enter asks).
    if (value.startsWith('?')) {
      entries = [];
      list.hidden = true;
      hintKey = '?';
      hint.replaceChildren(
        el(document_, 'code', { class: 'cv-slash-sig' }, el(document_, 'mark', {}, '? <question>')),
        el(document_, 'span', { class: 'cv-slash-note' }, 'Ask: a read-only question about your data · Enter asks'),
        el(document_, 'span', { class: 'cv-slash-example' }, 'e.g. ', el(document_, 'code', {}, '? where are the red invoices')));
      hint.hidden = false;
      describe(true);
      sync();
      return;
    }
    // APP-122: `/<app>.<card>` lists the app's cards; `/<app>.<card> words` shapes it.
    const dotted = value.match(/^\/([a-z0-9][a-z0-9-]*\.[a-z0-9-]*)(\s+[\s\S]*)?$/i);
    if (dotted) {
      const words = dotted[2] !== undefined;
      hintKey = `.${words}`;
      hint.replaceChildren(
        el(document_, 'code', { class: 'cv-slash-sig' }, `/${dotted[1]} `, el(document_, 'mark', {}, '<what it should show>')),
        el(document_, 'span', { class: 'cv-slash-note' }, words ? 'Shape: a preview first, then Apply' : 'A card: Enter places it, a space then words shape it'),
        el(document_, 'span', { class: 'cv-slash-example' }, 'e.g. ', el(document_, 'code', {}, '/expenses.ledger only red invoices')));
      hint.hidden = false;
      describe(true);
      const dot = completers.get('.');
      if (!words && dot) renderCompletions(dot.complete(dotted[1] ?? ''), dot.select);
      else {
        entries = [];
        list.hidden = true;
        highlight(-1);
      }
      sync();
      return;
    }
    const argument = value.match(/^\/([a-z0-9-]+)\s+([\s\S]*)$/i);
    if (argument) {
      const command = findCommand(argument[1] ?? '');
      const rest = argument[2] ?? '';
      if (!command) {
        close();
        return;
      }
      showHint(command, rest);
      const { completer, rest: tail } = completerFor((argument[1] ?? '').toLowerCase(), command.slug, rest);
      if (completer) renderCompletions(completer.complete(tail), completer.select);
      else {
        entries = [];
        list.hidden = true;
        highlight(-1);
      }
      sync();
      return;
    }
    hideHint();
    const match = value.match(/^\/([a-z0-9-]*)$/i);
    if (!match) {
      close();
      return;
    }
    const prefix = match[1] ?? '';
    entries = [];
    const section = (id: string, title: string, items: Node[]) => el(document_, 'li', { role: 'group', 'aria-labelledby': id, class: 'cv-slash-group' },
      el(document_, 'span', { id, class: 'cv-slash-cat', role: 'presentation' }, title),
      el(document_, 'ul', { role: 'none', class: 'cv-slash-items' }, ...items));
    const nodes = slashGroups(prefix).map((group) => section(`cv-slash-cat-${group.category.toLowerCase()}`, group.category,
      group.commands.map((command) => {
        const entry: Entry = { kind: 'command', command };
        entries.push(entry);
        return option(entry, entries.length - 1,
          el(document_, 'span', { class: 'cv-slash-name' }, signature(command)),
          el(document_, 'span', { class: 'cv-slash-title' }, command.title),
          el(document_, 'span', { class: 'cv-slash-purpose' }, command.purpose));
      })));
    // APP-119: apps are addresses, in their own section after the verbs.
    const { title: appsTitle, addresses } = slashAddressSection(prefix);
    if (addresses.length) {
      nodes.push(section('cv-slash-cat-apps', appsTitle, addresses.map((address) => {
        const entry: Entry = { kind: 'command', command: address };
        entries.push(entry);
        return option(entry, entries.length - 1,
          el(document_, 'span', { class: 'cv-slash-name' }, `/${address.slug}`),
          el(document_, 'span', { class: 'cv-slash-title' }, address.title),
          el(document_, 'span', { class: 'cv-slash-purpose' }, address.hint));
      })));
    }
    list.replaceChildren(...nodes);
    list.hidden = entries.length === 0;
    highlight(entries.length ? 0 : -1);
    sync();
  };
  input.addEventListener('input', () => {
    if (input.value.startsWith('/')) void ensureCommands().then(render);
    else if (input.value.startsWith('?')) render();
    else close();
  });
  input.addEventListener('keydown', (event) => {
    if (list.hidden || !entries.length) {
      if (event.key === 'Escape' && !pop.hidden) {
        event.preventDefault();
        close();
      }
      return;
    }
    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      event.preventDefault();
      const step = event.key === 'ArrowDown' ? 1 : -1;
      highlight(active < 0 && step < 0 ? entries.length - 1 : (active + step + entries.length) % entries.length);
    } else if (event.key === 'Tab' || event.key === 'Enter') {
      const entry = entries[active];
      if (!entry || event.shiftKey) return;
      // Enter on a fully typed command name runs it as typed.
      if (event.key === 'Enter' && entry.kind === 'command' && /^\/[a-z0-9-]+$/i.test(input.value.trim()) && findCommand(input.value.trim().slice(1))?.slug === entry.command.slug) {
        event.preventDefault();
        run(input.value.trim());
        return;
      }
      event.preventDefault();
      accept(entry, event.key);
    } else if (event.key === 'Escape') {
      event.preventDefault();
      close();
    }
  });
  input.addEventListener('blur', () => window.setTimeout(() => {
    if (document_.activeElement !== input) close();
  }, 120));
}
