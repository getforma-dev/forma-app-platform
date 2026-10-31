// APP-118: the one command registry. Every command of the intent bar, with
// its name and aliases, its category, a one-line description, its typed
// arguments and an example or two, lives here, and the surfaces read only
// this list: the `/` autocomplete (`slash.ts`), the Commands tab of the
// right pane (`command-sheet.ts`) and the `?` shortcuts sheet
// (`controls.ts`).
//
// APP-119: commands are verbs, apps are addresses. The verbs are what the
// canvas modules register (`registerCommands`: `/card`, `/unite`,
// `/folder`, the widgets, `/note`, …); `/` alone and `/commands` list only
// them. The member's apps are addresses (`setAddresses`, from the server's
// `GET /v1/commands`): each has its slug and aliases from the catalog (or
// the name a saved app was given), and a hint and examples the server
// derived from the definition. They are listed in the Library (`/apps`) and
// in the `/x…` list's own "Apps" section, and `/slug words` still addresses
// one: `findCommand` falls back to `findAddress`. This module holds data
// only: no DOM, no fetch.

export type Category = 'Library' | 'Canvas' | 'Folders' | 'Widgets' | 'Agents' | 'Help';
/** The order the surfaces list categories in. */
export const CATEGORIES: readonly Category[] = ['Library', 'Canvas', 'Folders', 'Widgets', 'Agents', 'Help'];
/** The heading of the addresses' section in the `/x…` list. */
export const APPS_SECTION = 'Apps — type /name and what you want';
/** The Commands tab's footer: addresses live in the Library. */
export const ADDRESS_FOOTER = 'Your apps answer to /name — see /apps';

/** What an argument names; the kinds with values get live suggestions under the bar. */
export type ArgKind = 'text' | 'app' | 'card' | 'folder' | 'city' | 'zone';
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
  run(text: string, report?: Report): void | Promise<void>;
}

/** What the bar can run, verb or address: every field filled in. */
export type Runnable = Omit<CanvasCommand, 'category'> & { aliases: string[]; args: CommandArg[]; examples: string[] };
/** A verb as the surfaces read it. */
export type Command = Runnable & { category: Category };
export type CommandGroup = { category: Category; commands: Command[] };

/** An app the member can address (`/expense …`): a library app or a named saved one. */
export type Address = Runnable & {
  /** What to type after `/slug` (server-derived; the catalog may override it). */
  hint: string;
  kind: 'library' | 'saved';
  app: string;
  work?: string;
  record: boolean;
};

const arg = (name: string, kind: ArgKind, required = false): CommandArg => ({ name, kind, required });
type Meta = Pick<Command, 'category' | 'args' | 'examples'>;

/** What the registry knows about each verb it may meet. Apps are never here (APP-119). */
const KNOWN: Record<string, Meta> = {
  apps: { category: 'Library', args: [arg('app name', 'app')], examples: ['/apps', '/apps purchase'] },
  app: { category: 'Library', args: [arg('app name or number', 'app', true)], examples: ['/app expense', '/app 3'] },
  card: { category: 'Library', args: [arg('card name or number', 'card', true)], examples: ['/card 3.2', '/card spend by category'] },
  note: { category: 'Canvas', args: [arg('text', 'text')], examples: ['/note Call the auditor Friday'] },
  folder: {
    category: 'Folders',
    args: [arg('folder, new or formation', 'folder'), arg('name or instruction', 'text')],
    examples: ['/folder new Q3 board pack', '/folder 1 formation build a metrics app'],
  },
  unite: { category: 'Canvas', args: [arg('app', 'app', true)], examples: ['/unite expense'] },
  split: { category: 'Canvas', args: [arg('united app', 'app', true)], examples: ['/split expense'] },
  arrange: { category: 'Canvas', args: [], examples: ['/arrange'] },
  weather: { category: 'Widgets', args: [arg('city or style', 'city')], examples: ['/weather Austin', '/weather make it dark'] },
  calculator: { category: 'Widgets', args: [arg('expression or style', 'text')], examples: ['/calculator 12*(3+4)', '/calculator make it a TI-84'] },
  clock: { category: 'Widgets', args: [arg('city or time zone', 'zone')], examples: ['/clock Tokyo'] },
  agent: { category: 'Agents', args: [], examples: ['/agent'] },
  commands: { category: 'Help', args: [arg('search', 'text')], examples: ['/commands', '/commands folder'] },
  shortcuts: { category: 'Help', args: [], examples: ['/shortcuts'] },
};

function complete(command: CanvasCommand): Command {
  const known = KNOWN[command.slug];
  return {
    ...command,
    aliases: command.aliases ?? [],
    category: command.category ?? known?.category ?? 'Canvas',
    args: command.args ?? known?.args ?? [],
    examples: command.examples ?? known?.examples ?? [`/${command.slug}`],
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

/** Every verb, once each, in category order. */
export function commandList(): Command[] {
  const seen = new Set<string>();
  const out: Command[] = [];
  for (const command of registered) {
    if (seen.has(command.slug)) continue;
    seen.add(command.slug);
    out.push(complete(command));
  }
  const rank = (command: Command) => CATEGORIES.indexOf(command.category);
  return out.map((command, at) => ({ command, at }))
    .sort((a, b) => rank(a.command) - rank(b.command) || a.at - b.at)
    .map((entry) => entry.command);
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

/** What `/name` runs: a verb, else an app's address (every `/expense …` path is unchanged). */
export function findCommand(name: string): Runnable | undefined {
  return findVerb(name) ?? findAddress(name);
}

/** Verbs grouped by category, in category order, empty categories left out. */
export function groupCommands(commands: Command[]): CommandGroup[] {
  return CATEGORIES
    .map((category) => ({ category, commands: commands.filter((command) => command.category === category) }))
    .filter((group) => group.commands.length > 0);
}

/** The `/` list's verbs for what follows the slash: slug or alias prefix, or the title for two letters or more. */
export function matchCommands(prefix: string): Command[] {
  const key = prefix.toLowerCase();
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
  return addressList().filter((address) => {
    const exact = address.slug === key || address.aliases.includes(key);
    const hit = exact || address.slug.startsWith(key)
      || address.aliases.some((alias) => alias.startsWith(key))
      || (key.length > 1 && address.title.toLowerCase().includes(key));
    return hit && (exact || listed(address));
  });
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
  if (!words.length) return commandList();
  return commandList().filter((command) => {
    const text = [command.slug, ...command.aliases, command.title, command.purpose, command.category, ...command.examples].join(' ').toLowerCase();
    return words.every((word) => text.includes(word));
  });
}

/** `<required>` and `[optional]`, as in a manual page. */
export function argLabel(argument: CommandArg): string {
  return argument.required ? `<${argument.name}>` : `[${argument.name}]`;
}

/** `/expense [what you spent]`, `/card <card name or number>`. */
export function signature(command: Runnable): string {
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
