// APP-101 slash commands: the intent bar's explicit lane, instant and never
// a guess. Typing `/` opens a keyboard-navigable list of the member's
// commands (`GET /v1/commands`: every library app with its slug and aliases,
// and the member's named saved apps), merged with any canvas commands other
// modules register (`registerCommands`, e.g. utility widgets). `/name rest`
// runs that command: a library app opens at once through the pinned compose
// (`{"intent", "app_id"}`, no routing, no model call) and `rest` is tailored
// by the live transform, or, for a record app when `rest` reads like an
// entry or a decision, goes to the action path pinned to that app; a named
// saved app opens its work directly. `/apps` lists every command as a card
// on the canvas. The near-tie chooser (`showChooser`) lives here too: the
// top apps plus "Build something new", one click proceeds.
//
// Nothing from the server is rendered as markup: titles and purposes are
// text nodes; ids are checked UUIDs.
import { submitAction } from './actions';
import { placeUtilityCard, placeWork } from './board';
import { submitIntent, type Choice } from './compose';
import { isDismissKey, showDockCard } from './dock-card';
import { el, isUuid } from './render';

export const COMMANDS_URL = '/v1/commands';
type Report = (message: string) => void;

/**
 * A canvas command. Other modules (for example the utility widgets of
 * `pivot/widgets`) register their own with `registerCommands`; the
 * autocomplete and `/apps` merge them with the server's app commands.
 */
export interface CanvasCommand {
  slug: string;
  title: string;
  purpose: string;
  aliases?: string[];
  run(text: string, report?: Report): void | Promise<void>;
}

const registered: CanvasCommand[] = [];
let server: CanvasCommand[] = [];
let loaded: Promise<void> | null = null;

/** Adds canvas commands (a later registration of the same slug wins). */
export function registerCommands(commands: CanvasCommand[]): void {
  for (const command of commands) {
    const at = registered.findIndex((existing) => existing.slug === command.slug);
    if (at >= 0) registered.splice(at, 1);
    registered.push(command);
  }
}

/**
 * APP-104: an argument completion under the bar (`/card 3.2`, `/app expense`):
 * a short badge (the number), a title, a detail line and the text it runs.
 */
export type Completion = { badge: string; title: string; detail: string; value: string };
const completers = new Map<string, (rest: string) => Completion[]>();
/** Offers completions for `/slug <rest>`. */
export function registerCompleter(slug: string, complete: (rest: string) => Completion[]): void {
  completers.set(slug, complete);
}

/** Every command: the built-in `/apps`, the registered ones, then the server's. */
export function allCommands(): CanvasCommand[] {
  const seen = new Set<string>();
  const out: CanvasCommand[] = [];
  for (const command of [APPS, ...registered, ...server]) {
    if (seen.has(command.slug)) continue;
    seen.add(command.slug);
    out.push(command);
  }
  return out;
}

/** Text that reads like a record entry or a decision (the action path's lane). */
const ENTRY = /\b(approve|approved|reject|rejected|decline|deny|accept|file|submit|spent|paid|log|record|add|sign off)\b|\$\s?\d/i;

/** The slug whose bare form opens the ledger (`actions::ledger::bare_command`). */
const LEDGER = ['expense'];

type ServerCommand = {
  slug: string; aliases: string[]; title: string; purpose: string;
  kind: 'library' | 'saved'; app_id: string; work_id?: string; record: boolean;
};

function serverCommand(entry: ServerCommand): CanvasCommand {
  return {
    slug: entry.slug,
    aliases: entry.aliases,
    title: entry.title,
    purpose: entry.purpose,
    run: async (text: string, report: Report = () => undefined) => {
      const rest = text.trim();
      if (entry.kind === 'saved' && entry.work_id) {
        report(await placeWork(entry.app_id, entry.work_id));
        if (rest && entry.record && ENTRY.test(rest)) await submitAction(rest, report, entry.app_id);
        return;
      }
      // `/expense` alone opens the expense ledger; `/expense <words>` is
      // always the action path for that app (APP-102 with APP-101).
      if (LEDGER.includes(entry.slug)) {
        await (rest ? submitAction(rest, report, entry.app_id) : submitAction('/expense', report));
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

function parseCommands(body: unknown): ServerCommand[] {
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
      aliases: Array.isArray(entry.aliases) ? entry.aliases.filter((a): a is string => typeof a === 'string').slice(0, 8) : [],
      title: String(entry.title ?? '').slice(0, 120),
      purpose: String(entry.purpose ?? '').slice(0, 200),
      kind: entry.kind,
      app_id: entry.app_id,
      work_id: typeof entry.work_id === 'string' ? entry.work_id : undefined,
      record: entry.record === true,
    }];
  });
}

/** Re-reads the member's commands from the server. */
export function refreshCommands(): Promise<void> {
  loaded = fetch(COMMANDS_URL, { credentials: 'same-origin', cache: 'no-store' })
    .then((response) => (response.ok ? response.json() : null))
    .then((body) => {
      server = parseCommands(body).map(serverCommand);
    })
    .catch(() => undefined);
  return loaded;
}
function ensureCommands(): Promise<void> {
  return loaded ?? refreshCommands();
}

function find(name: string): CanvasCommand | undefined {
  const all = allCommands();
  return all.find((command) => command.slug === name) ?? all.find((command) => command.aliases?.includes(name));
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
  const command = find(name);
  if (!command) {
    report(`There is no /${name} command. Type / to see yours, or /apps for the list.`);
    return;
  }
  await command.run(match[2] ?? '', report);
}

/** `/apps`: every command as a card on the canvas; a click runs it. */
const APPS: CanvasCommand = {
  slug: 'apps',
  title: 'All commands',
  purpose: 'Show every app and command you can open',
  run: async (_text: string, report: Report = () => undefined) => {
    await ensureCommands();
    const document_ = document;
    const list = el(document_, 'ul', { class: 'cv-apps-list' });
    for (const command of allCommands()) {
      if (command.slug === 'apps') continue;
      const button = el(document_, 'button', { type: 'button', class: 'cv-apps-item' },
        el(document_, 'span', { class: 'cv-apps-slug' }, `/${command.slug}`),
        el(document_, 'span', { class: 'cv-apps-title' }, command.title),
        el(document_, 'span', { class: 'cv-apps-purpose' }, command.purpose));
      button.addEventListener('click', () => void command.run('', report));
      list.append(el(document_, 'li', {}, button));
    }
    placeUtilityCard('apps', 'Your commands', el(document_, 'div', { class: 'cv-card-body' }, list));
    report('Your commands are on the canvas. Click one, or type /name in the bar.');
  },
};

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

/** Wires the `/` autocomplete onto the intent bar (once). */
export function startSlash(document_: Document): void {
  const input = document_.getElementById('intent-input') as HTMLInputElement | null;
  const form = document_.getElementById('intent-form') as HTMLFormElement | null;
  if (!input || !form || input.dataset.slash === 'on') return;
  input.dataset.slash = 'on';
  const list = el(document_, 'ul', { id: 'cv-slash', class: 'cv-slash', role: 'listbox', 'aria-label': 'Commands', hidden: '' });
  form.append(list);
  input.setAttribute('role', 'combobox');
  input.setAttribute('aria-autocomplete', 'list');
  input.setAttribute('aria-controls', 'cv-slash');
  input.setAttribute('aria-expanded', 'false');
  let shown: CanvasCommand[] = [];
  let completions: Completion[] = [];
  let active = -1;

  const close = () => {
    list.hidden = true;
    input.setAttribute('aria-expanded', 'false');
    input.removeAttribute('aria-activedescendant');
    active = -1;
  };
  const highlight = (index: number) => {
    active = index;
    Array.from(list.children).forEach((child, at) => child.setAttribute('aria-selected', at === index ? 'true' : 'false'));
    if (index >= 0) input.setAttribute('aria-activedescendant', `cv-slash-${index}`);
    list.children[index]?.scrollIntoView({ block: 'nearest' });
  };
  const complete = (command: CanvasCommand) => {
    input.value = `/${command.slug} `;
    close();
    input.focus();
    if (completers.has(command.slug)) render();
  };
  const run = (value: string) => {
    input.value = value;
    close();
    form.requestSubmit();
  };
  const renderCompletions = (list_: Completion[]) => {
    completions = list_;
    shown = [];
    list.replaceChildren(...list_.map((completion, index) => {
      const option = el(document_, 'li', { id: `cv-slash-${index}`, role: 'option', 'aria-selected': 'false', class: 'cv-slash-option cv-slash-completion' },
        el(document_, 'span', { class: 'cv-slash-name cv-slash-badge' }, completion.badge),
        el(document_, 'span', { class: 'cv-slash-title' }, completion.title),
        el(document_, 'span', { class: 'cv-slash-purpose' }, completion.detail));
      option.addEventListener('mousedown', (event) => {
        event.preventDefault();
        run(completion.value);
      });
      return option;
    }));
    list.hidden = list_.length === 0;
    input.setAttribute('aria-expanded', list_.length ? 'true' : 'false');
    highlight(list_.length ? 0 : -1);
  };
  const render = () => {
    const value = input.value;
    const argument = value.match(/^\/([a-z0-9-]+)\s+([\s\S]*)$/i);
    const completer = argument ? completers.get((argument[1] ?? '').toLowerCase()) : undefined;
    if (argument && completer) {
      renderCompletions(completer(argument[2] ?? ''));
      return;
    }
    completions = [];
    const match = value.match(/^\/([a-z0-9-]*)$/i);
    if (!match) {
      close();
      return;
    }
    const prefix = (match[1] ?? '').toLowerCase();
    shown = allCommands()
      .filter((command) => command.slug.startsWith(prefix) || command.aliases?.some((alias) => alias.startsWith(prefix))
        || (prefix.length > 1 && command.title.toLowerCase().includes(prefix)))
      .slice(0, 12);
    list.replaceChildren(...shown.map((command, index) => {
      const option = el(document_, 'li', { id: `cv-slash-${index}`, role: 'option', 'aria-selected': 'false', class: 'cv-slash-option' },
        el(document_, 'span', { class: 'cv-slash-name' }, `/${command.slug}`),
        el(document_, 'span', { class: 'cv-slash-title' }, command.title),
        el(document_, 'span', { class: 'cv-slash-purpose' }, command.purpose));
      option.addEventListener('mousedown', (event) => {
        event.preventDefault();
        complete(command);
      });
      return option;
    }));
    list.hidden = shown.length === 0;
    input.setAttribute('aria-expanded', shown.length ? 'true' : 'false');
    highlight(shown.length ? 0 : -1);
  };
  input.addEventListener('input', () => {
    if (input.value.startsWith('/')) void ensureCommands().then(render);
    else close();
  });
  input.addEventListener('keydown', (event) => {
    if (!list.hidden && completions.length) {
      if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
        event.preventDefault();
        const step = event.key === 'ArrowDown' ? 1 : -1;
        highlight((active + step + completions.length) % completions.length);
      } else if (event.key === 'Tab' || event.key === 'Enter') {
        const completion = completions[active];
        if (!completion) return;
        event.preventDefault();
        if (event.key === 'Tab') {
          input.value = `${completion.value}`;
          render();
        } else {
          run(completion.value);
        }
      } else if (event.key === 'Escape') {
        event.preventDefault();
        close();
      }
      return;
    }
    if (list.hidden || !shown.length) return;
    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      event.preventDefault();
      const step = event.key === 'ArrowDown' ? 1 : -1;
      highlight((active + step + shown.length) % shown.length);
    } else if (event.key === 'Tab' || event.key === 'Enter') {
      const command = shown[active];
      if (!command) return;
      // Enter on a fully typed command runs it; otherwise it completes.
      if (event.key === 'Enter' && input.value.trim().toLowerCase() === `/${command.slug}`) {
        close();
        return;
      }
      event.preventDefault();
      complete(command);
    } else if (event.key === 'Escape') {
      event.preventDefault();
      close();
    }
  });
  input.addEventListener('blur', () => window.setTimeout(close, 120));
}
