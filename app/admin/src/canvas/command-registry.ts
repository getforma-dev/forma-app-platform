// APP-118: the one command registry. Every command of the intent bar, with
// its name and aliases, its group, a one-line description, its typed
// arguments and an example or two, lives here, and the surfaces read only
// this list: the `/` autocomplete (`slash.ts`) and the Commands tab of the
// right pane (`command-sheet.ts`).
//
// APP-119: commands are verbs, apps are addresses. The member's apps are
// addresses (`setAddresses`, from the server's `GET /v1/commands`): each has
// its slug and aliases from the catalog (or the name a saved app was given),
// and a hint and examples the server derived from the definition. They are
// listed in the Library (`/apps`) and in the `/x…` list's own "Apps"
// section, and `/slug words` still addresses one: `findCommand` falls back
// to `findAddress`.
//
// APP-120: one grammar. "Type / and a verb to change the canvas, / and an
// app's name to tell that app something, or just a sentence." There are
// twelve verbs in four groups: Place (`/app`, `/card`, `/widget`), Arrange
// (`/arrange`, `/unite`, `/split`, `/focus`), Make (`/folder <name>
// formation <instruction>`) and Help (`/apps`, `/commands`, `/shortcuts`,
// `/agent`). The old widget verbs (`/weather`, `/calculator`, `/clock`,
// `/note`) stay as hidden aliases of `/widget <kind>` (`aliasOf`): accepted
// and autocompleted from their first letter, never listed under `/` alone or
// in the Commands tab's groups. There is no `/remove`: the card's ×, Delete
// or the Library's Remove take things off. This module holds data only: no
// DOM, no fetch.
//
// APP-121: people name things. `/rename <app|app.card> <new name> [/address]`
// and `/alias <shortcut> <app|app.card>` are verbs of the Make group (14
// verbs in all). An address carries the organization's labels (its name,
// address, kept aliases, description and card names, from the server) and
// the member's own shortcuts: `/exp` → an app joins its aliases (for that
// member only); `/q3` → a card is a card shortcut (`findCardShortcut`), and
// `findCommand` runs it as `/card <app>.<card>`. The server already drops a
// shortcut an organization name took (the organization's address wins).

export type Category = 'Place' | 'Arrange' | 'Make' | 'Help';
/** The order the surfaces list groups in. */
export const CATEGORIES: readonly Category[] = ['Place', 'Arrange', 'Make', 'Help'];
/** APP-120: the one sentence the grammar teaches. */
export const GRAMMAR = 'Type / and a verb to change the canvas, / and an app’s name to tell that app something, or just a sentence.';
/** The heading of the addresses' section in the `/x…` list. */
export const APPS_SECTION = 'Apps — type /name and what you want';
/** The Commands tab's footer: addresses live in the Library. */
export const ADDRESS_FOOTER = 'Your apps answer to /name — see /apps';

/** What an argument names; the kinds with values get live suggestions under the bar. */
export type ArgKind = 'text' | 'app' | 'card' | 'folder' | 'city' | 'zone' | 'widget';
export type CommandArg = { name: string; kind: ArgKind; required: boolean };
export type Report = (message: string) => void;

/** A canvas command (a verb) as a module registers it. */
export interface CanvasCommand {
  slug: string;
  title: string;
  purpose: string;
  aliases?: string[];
  category?: Category;
  args?: CommandArg[];
  examples?: string[];
  /** APP-120: how the surfaces print it when the arguments alone cannot (`/folder <name> formation <instruction>`). */
  usage?: string;
  /** APP-120: a hidden alias of this verb (`/weather` of `/widget`): runs and autocompletes, never listed under `/` alone. */
  aliasOf?: string;
  run(text: string, report?: Report): void | Promise<void>;
}

/** What the bar can run, verb or address: every field filled in. */
export type Runnable = Omit<CanvasCommand, 'category'> & { aliases: string[]; args: CommandArg[]; examples: string[] };
/** A verb as the surfaces read it. */
export type Command = Runnable & { category: Category };
export type CommandGroup = { category: Category; commands: Command[] };

/** APP-121: a card's labels: the organization's name and description, the member's own name and shortcuts. */
export type CardLabel = { card: string; name: string; description: string; myName: string; mine: string[] };

/** An app the member can address (`/expense …`): a library app or a named saved one. */
export type Address = Runnable & {
  /** What to type after `/slug` (server-derived; the catalog may override it). */
  hint: string;
  kind: 'library' | 'saved';
  app: string;
  work?: string;
  record: boolean;
  /** APP-121: the organization's description of the app (empty: none). */
  description?: string;
  /** APP-121: the aliases the organization kept (beyond the catalog's). */
  orgAliases?: string[];
  /** APP-121: the member's own shortcuts to the app (already among `aliases`). */
  mine?: string[];
  /** APP-121: whether the member may change the organization's labels (owner or admin). */
  canLabel?: boolean;
  /** APP-121: labeled cards, by their name within the app (`total`). */
  cards?: CardLabel[];
};

const arg = (name: string, kind: ArgKind, required = false): CommandArg => ({ name, kind, required });
type Meta = Pick<Command, 'category' | 'args' | 'examples'> & { usage?: string };

/** The fourteen verbs in their order; what the registry knows about each (apps are never here, APP-119). */
export const VERBS = ['app', 'card', 'widget', 'arrange', 'unite', 'split', 'focus', 'folder', 'rename', 'alias', 'apps', 'commands', 'shortcuts', 'agent'] as const;
/** APP-120: the widget kinds `/widget` places; the first four are also hidden verbs of their own. */
export const WIDGET_KINDS = ['weather', 'calculator', 'clock', 'note', 'folder'] as const;
export type WidgetName = typeof WIDGET_KINDS[number];

const KNOWN: Record<string, Meta> = {
  app: { category: 'Place', args: [arg('app name or number', 'app', true)], examples: ['/app expenses', '/app 3'] },
  card: { category: 'Place', args: [arg('app.card or number', 'card', true)], examples: ['/card expenses.ledger', '/card 3.2'] },
  widget: {
    category: 'Place',
    args: [arg('weather|calculator|clock|note|folder', 'widget', true), arg('args', 'text')],
    examples: ['/widget weather Austin', '/widget note Call the auditor Friday'],
    usage: '/widget <weather|calculator|clock|note|folder> [args]',
  },
  arrange: { category: 'Arrange', args: [], examples: ['/arrange'] },
  unite: { category: 'Arrange', args: [arg('app', 'app', true)], examples: ['/unite expenses'] },
  split: { category: 'Arrange', args: [arg('united app', 'app', true)], examples: ['/split expenses'] },
  focus: { category: 'Arrange', args: [arg('app.card or number', 'card')], examples: ['/focus', '/focus expenses.ledger'] },
  folder: {
    category: 'Make',
    args: [arg('name', 'folder', true), arg('formation <instruction>', 'text')],
    examples: ['/folder Q3 board pack formation build a metrics app', '/folder 1 formation brief me'],
    usage: '/folder <name> formation <instruction>',
  },
  rename: {
    category: 'Make',
    args: [arg('app or app.card', 'card', true), arg('new name [/address]', 'text', true)],
    examples: ['/rename expenses Team spend /spend', '/rename expenses.total Q3 spend'],
    usage: '/rename <app|app.card> <new name> [/address]',
  },
  alias: {
    category: 'Make',
    args: [arg('shortcut', 'text', true), arg('app or app.card', 'card', true)],
    examples: ['/alias exp expenses', '/alias q3 expenses.total'],
    usage: '/alias <shortcut> <app|app.card>',
  },
  apps: { category: 'Help', args: [arg('query', 'app')], examples: ['/apps', '/apps purchase'] },
  commands: { category: 'Help', args: [arg('search', 'text')], examples: ['/commands', '/commands folder'] },
  shortcuts: { category: 'Help', args: [], examples: ['/shortcuts'] },
  agent: { category: 'Help', args: [], examples: ['/agent'] },
  // Hidden aliases of `/widget <kind>` (APP-120).
  note: { category: 'Place', args: [arg('text', 'text')], examples: ['/note Call the auditor Friday'] },
  weather: { category: 'Place', args: [arg('city or style', 'city')], examples: ['/weather Austin', '/weather make it dark'] },
  calculator: { category: 'Place', args: [arg('expression or style', 'text')], examples: ['/calculator 12*(3+4)', '/calculator make it a TI-84'] },
  clock: { category: 'Place', args: [arg('city or time zone', 'zone')], examples: ['/clock Tokyo'] },
};

function complete(command: CanvasCommand): Command {
  const known = KNOWN[command.slug];
  return {
    ...command,
    aliases: command.aliases ?? [],
    category: command.category ?? known?.category ?? 'Place',
    args: command.args ?? known?.args ?? [],
    examples: command.examples ?? known?.examples ?? [`/${command.slug}`],
    ...(command.usage ?? known?.usage ? { usage: command.usage ?? known?.usage } : {}),
  };
}

const registered: CanvasCommand[] = [];
let addresses: Address[] = [];
const addressListeners = new Set<() => void>();

/** Adds canvas commands (a later registration of the same slug wins). */
export function registerCommands(commands: CanvasCommand[]): void {
  for (const command of commands) {
    const at = registered.findIndex((existing) => existing.slug === command.slug);
    if (at >= 0) registered.splice(at, 1);
    registered.push(command);
  }
}

/** Replaces the member's addresses (their apps, from the server). */
export function setAddresses(list: Address[]): void {
  const seen = new Set<string>();
  addresses = list.filter((address) => !seen.has(address.slug) && Boolean(seen.add(address.slug)));
  for (const listener of addressListeners) listener();
}

/** Calls `listener` whenever the addresses change (the Library decorates its rows). */
export function onAddressesChange(listener: () => void): () => void {
  addressListeners.add(listener);
  return () => addressListeners.delete(listener);
}

/** Every verb and hidden alias, once each, in group order (the twelve in their order, then any other). */
export function commandList(): Command[] {
  const seen = new Set<string>();
  const out: Command[] = [];
  for (const command of registered) {
    if (seen.has(command.slug)) continue;
    seen.add(command.slug);
    out.push(complete(command));
  }
  const rank = (command: Command) => CATEGORIES.indexOf(command.category);
  const order = (command: Command) => {
    const at = (VERBS as readonly string[]).indexOf(command.slug);
    return at < 0 ? VERBS.length : at;
  };
  return out.map((command, at) => ({ command, at }))
    .sort((a, b) => rank(a.command) - rank(b.command) || order(a.command) - order(b.command) || a.at - b.at)
    .map((entry) => entry.command);
}

/** APP-120: the verbs the surfaces list (`/` alone, the Commands tab): every verb that is not a hidden alias. */
export function visibleCommands(): Command[] {
  return commandList().filter((command) => !command.aliasOf);
}

/** APP-120: the hidden aliases of `slug` (`/widget` → `/weather`, `/calculator`, `/clock`, `/note`). */
export function hiddenAliases(slug: string): Command[] {
  return commandList().filter((command) => command.aliasOf === slug);
}

/** Every address the member has, except those a verb of the same name shadows. */
export function addressList(): Address[] {
  const verbs = new Set(registered.flatMap((command) => [command.slug, ...(command.aliases ?? [])]));
  return addresses.filter((address) => !verbs.has(address.slug));
}

/** The verb `name` names (its slug, else an alias). */
export function findVerb(name: string): Command | undefined {
  const key = name.toLowerCase();
  const all = commandList();
  return all.find((command) => command.slug === key) ?? all.find((command) => command.aliases.includes(key));
}

/** The app `name` addresses (its slug, else an alias), wherever it is. */
export function findAddress(name: string): Address | undefined {
  const key = name.toLowerCase();
  const all = addressList();
  return all.find((address) => address.slug === key) ?? all.find((address) => address.aliases.includes(key));
}

/** APP-121: the card a member's own shortcut names (`/q3` → `expenses.total`). */
export function findCardShortcut(name: string): { address: Address; card: string } | undefined {
  const key = name.toLowerCase();
  for (const address of addressList()) {
    const card = (address.cards ?? []).find((label) => label.mine.includes(key));
    if (card) return { address, card: card.card };
  }
  return undefined;
}

/** APP-121: a card's display name for this member: their own, else the organization's, else `fallback`. */
export function cardLabel(address: Address | undefined, card: string, fallback: string): string {
  const label = address?.cards?.find((entry) => entry.card === card);
  return label?.myName || label?.name || fallback;
}

/** What `/name` runs: a verb, else an app's address (every `/expense …` path is unchanged), else a card shortcut. */
export function findCommand(name: string): Runnable | undefined {
  const found = findVerb(name) ?? findAddress(name);
  if (found) return found;
  const shortcut = findCardShortcut(name);
  if (!shortcut) return undefined;
  const target = `${shortcut.address.slug}.${shortcut.card}`;
  return {
    slug: name.toLowerCase(),
    title: cardLabel(shortcut.address, shortcut.card, target),
    purpose: `Your shortcut to ${target}`,
    aliases: [],
    args: [],
    examples: [`/${name.toLowerCase()}`],
    run: (text: string, report?: Report) => findVerb('card')?.run(`${target} ${text}`.trim(), report),
  };
}

/** Verbs grouped by category, in category order, empty categories left out. */
export function groupCommands(commands: Command[]): CommandGroup[] {
  return CATEGORIES
    .map((category) => ({ category, commands: commands.filter((command) => command.category === category) }))
    .filter((group) => group.commands.length > 0);
}

/**
 * The `/` list's verbs for what follows the slash: slug or alias prefix, or
 * the title for two letters or more. `/` alone lists the visible verbs only;
 * from the first letter the hidden aliases match too (`/we` → `/weather`).
 */
export function matchCommands(prefix: string): Command[] {
  const key = prefix.toLowerCase();
  if (!key) return visibleCommands();
  return commandList().filter((command) => command.slug.startsWith(key)
    || command.aliases.some((alias) => alias.startsWith(key))
    || (key.length > 1 && command.title.toLowerCase().includes(key)));
}

/**
 * The `/x…` list's addresses (never for `/` alone): those whose slug, alias
 * or title match, narrowed to `listed` (apps on the canvas and saved apps,
 * decided by the caller), except that an exact slug or alias of any app the
 * member can open is always offered (`/pto` on a clean canvas).
 */
export function matchAddresses(prefix: string, listed: (address: Address) => boolean = () => true): Address[] {
  const key = prefix.toLowerCase();
  if (!key) return [];
  // Rank: exact name, then the app's own name starting with the prefix, then
  // an alias, then the title; shorter names first within a rank.
  const rank = (address: Address): number => {
    if (address.slug === key || address.aliases.includes(key)) return 0;
    if (address.slug.startsWith(key)) return 1;
    if (address.aliases.some((alias) => alias.startsWith(key))) return 2;
    if (key.length > 1 && address.title.toLowerCase().includes(key)) return 3;
    return 9;
  };
  return addressList()
    .map((address) => ({ address, rank: rank(address) }))
    .filter(({ address, rank: r }) => r < 9 && (r === 0 || listed(address)))
    .sort((a, b) => a.rank - b.rank || a.address.slug.length - b.address.slug.length || a.address.slug.localeCompare(b.address.slug))
    .map(({ address }) => address);
}

/** The Library's search over addresses: any word of the slug, aliases, title, hint or examples. */
export function searchAddresses(query: string): Address[] {
  const words = query.toLowerCase().replace(/^\//, '').split(/\s+/).filter(Boolean);
  if (!words.length) return addressList();
  return addressList().filter((address) => {
    const text = addressText(address);
    return words.every((word) => text.includes(word));
  });
}

/** The words an address is found by (slug, aliases, title, hint). */
export function addressText(address: Address): string {
  return [`/${address.slug}`, ...address.aliases.map((alias) => `/${alias}`), address.title, address.hint].join(' ').toLowerCase();
}

/** The Commands tab's search: any word of the name, aliases, title, description or examples. */
export function searchCommands(query: string): Command[] {
  const words = query.toLowerCase().replace(/^\//, '').split(/\s+/).filter(Boolean);
  if (!words.length) return visibleCommands();
  // A verb is found by its hidden aliases too (`weather` finds `/widget`).
  return visibleCommands().filter((command) => {
    const hidden = hiddenAliases(command.slug).flatMap((alias) => [alias.slug, ...alias.aliases, alias.title]);
    const text = [command.slug, ...command.aliases, ...hidden, command.title, command.purpose, command.category, ...command.examples].join(' ').toLowerCase();
    return words.every((word) => text.includes(word));
  });
}

/** `<required>` and `[optional]`, as in a manual page. */
export function argLabel(argument: CommandArg): string {
  return argument.required ? `<${argument.name}>` : `[${argument.name}]`;
}

/** `/expense [what you spent]`, `/card <card name or number>`. */
export function signature(command: Runnable): string {
  if (command.usage) return command.usage;
  return [`/${command.slug}`, ...command.args.map(argLabel)].join(' ');
}
/**
 * Reads `rest` (what follows `/name `) into the command's arguments: every
 * argument but the last takes one word, the last takes the remainder.
 * `index` is the argument being typed now (the next one after a space).
 */
export function parseArgs(command: Runnable, rest: string): { values: Record<string, string>; index: number } {
  const values: Record<string, string> = {};
  let remaining = rest.replace(/^\s+/, '');
  let index = 0;
  for (let at = 0; at < command.args.length; at += 1) {
    const argument = command.args[at]!;
    index = at;
    if (at === command.args.length - 1) {
      values[argument.name] = remaining.trim();
      break;
    }
    const word = remaining.match(/^(\S+)(\s+)?([\s\S]*)$/);
    if (!word) break;
    values[argument.name] = word[1] ?? '';
    if (!word[2]) break;
    remaining = word[3] ?? '';
    index = at + 1;
  }
  return { values, index };
}

/** The hint under the bar: the signature with the argument being typed marked, and an example. */
export type Hint = { parts: { text: string; current: boolean }[]; example: string; note: string };
export function argumentHint(command: Runnable, rest: string): Hint {
  const { index } = parseArgs(command, rest);
  const parts = [{ text: `/${command.slug}`, current: false }, ...command.args.map((argument, at) => ({ text: argLabel(argument), current: at === index }))];
  const example = command.examples.find((text) => text.trim().includes(' ')) ?? command.examples[0] ?? '';
  const current = command.args[index];
  const note = !current
    ? 'Nothing more to type · Enter runs it'
    : current.required ? `Next: ${current.name}` : `Optional: ${current.name} · Enter runs it as is`;
  return { parts, example, note };
}
