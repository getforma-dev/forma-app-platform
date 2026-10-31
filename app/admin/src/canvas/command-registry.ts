// APP-118: the one command registry. Every command of the intent bar, with
// its name and aliases, its category, a one-line description, its typed
// arguments and an example or two, lives here, and the three surfaces read
// only this list: the `/` autocomplete (`slash.ts`), the `/commands` sheet
// (`command-sheet.ts`) and the `?` shortcuts sheet (`controls.ts`).
//
// Commands arrive from two places: the canvas modules register theirs
// (`registerCommands`: `/card`, `/unite`, `/folder`, the widgets, `/note`,
// …), and the server's `GET /v1/commands` lists the member's apps
// (`setServerCommands`, from `slash.ts`). A command may describe itself; the
// `KNOWN` table fills in what it leaves out, so a command keeps one shape
// wherever it came from. This module holds data only: no DOM, no fetch.

export type Category = 'Apps' | 'Records' | 'Folders' | 'Canvas' | 'Widgets' | 'Agents' | 'Help';
/** The order the surfaces list categories in. */
export const CATEGORIES: readonly Category[] = ['Apps', 'Records', 'Folders', 'Canvas', 'Widgets', 'Agents', 'Help'];

/** What an argument names; the kinds with values get live suggestions under the bar. */
export type ArgKind = 'text' | 'app' | 'card' | 'folder' | 'city' | 'zone';
export type CommandArg = { name: string; kind: ArgKind; required: boolean };
export type Report = (message: string) => void;

/** A canvas command as a module registers it. */
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

/** A command as the surfaces read it: every field filled in. */
export type Command = CanvasCommand & { aliases: string[]; category: Category; args: CommandArg[]; examples: string[] };
export type CommandGroup = { category: Category; commands: Command[] };

const arg = (name: string, kind: ArgKind, required = false): CommandArg => ({ name, kind, required });
type Meta = Pick<Command, 'category' | 'args' | 'examples'>;

/** What the registry knows about each command it may meet (canvas and server slugs). */
const KNOWN: Record<string, Meta> = {
  apps: { category: 'Apps', args: [arg('app name', 'app')], examples: ['/apps', '/apps purchase'] },
  app: { category: 'Apps', args: [arg('app name or number', 'app', true)], examples: ['/app expense', '/app 3'] },
  card: { category: 'Apps', args: [arg('card name or number', 'card', true)], examples: ['/card 3.2', '/card spend by category'] },
  expense: { category: 'Records', args: [arg('what you spent', 'text')], examples: ['/expense $86 at Nobu, client dinner, Sarah approves', '/expense'] },
  pto: { category: 'Records', args: [arg('dates and reason', 'text')], examples: ['/pto Nov 24–26 for Thanksgiving, Sarah approves'] },
  purchase: { category: 'Records', args: [arg('what to buy', 'text')], examples: ['/purchase 3 monitors for $1,200 from Dell'] },
  invoice: { category: 'Records', args: [arg('vendor, amount and PO', 'text')], examples: ['/invoice Acme $4,800 against PO-1042'] },
  access: { category: 'Records', args: [arg('system and why', 'text')], examples: ['/access Snowflake read access for the Q3 audit'] },
  discount: { category: 'Records', args: [arg('deal and discount', 'text')], examples: ['/discount 8% on the Globex renewal'] },
  note: { category: 'Records', args: [arg('text', 'text')], examples: ['/note Call the auditor Friday'] },
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

/**
 * A server app command. The workflow apps (`/expense`, `/pto`, …) are in
 * KNOWN under Records; any other library or saved app is listed under Apps
 * (most library apps keep records, so the flag alone would put every app
 * there), and its argument says whether words can also file or decide.
 */
export function serverMeta(slug: string, record: boolean): Meta {
  return KNOWN[slug] ?? {
    category: 'Apps',
    args: [arg(record ? 'what to show or file' : 'what to show', 'text')],
    examples: [`/${slug}`],
  };
}

function complete(command: CanvasCommand): Command {
  const known = KNOWN[command.slug];
  return {
    ...command,
    aliases: command.aliases ?? [],
    category: command.category ?? known?.category ?? 'Apps',
    args: command.args ?? known?.args ?? [],
    examples: command.examples ?? known?.examples ?? [`/${command.slug}`],
  };
}

const registered: CanvasCommand[] = [];
let server: CanvasCommand[] = [];

/** Adds canvas commands (a later registration of the same slug wins). */
export function registerCommands(commands: CanvasCommand[]): void {
  for (const command of commands) {
    const at = registered.findIndex((existing) => existing.slug === command.slug);
    if (at >= 0) registered.splice(at, 1);
    registered.push(command);
  }
}

/** Replaces the server's commands (the member's apps). */
export function setServerCommands(commands: CanvasCommand[]): void {
  server = commands;
}

/** Every command, once each (a canvas command shadows a server one of the same slug), in category order. */
export function commandList(): Command[] {
  const seen = new Set<string>();
  const out: Command[] = [];
  for (const command of [...registered, ...server]) {
    if (seen.has(command.slug)) continue;
    seen.add(command.slug);
    out.push(complete(command));
  }
  const rank = (command: Command) => CATEGORIES.indexOf(command.category);
  return out.map((command, at) => ({ command, at }))
    .sort((a, b) => rank(a.command) - rank(b.command) || a.at - b.at)
    .map((entry) => entry.command);
}

/** The command `name` names (its slug, else an alias). */
export function findCommand(name: string): Command | undefined {
  const key = name.toLowerCase();
  const all = commandList();
  return all.find((command) => command.slug === key) ?? all.find((command) => command.aliases.includes(key));
}

/** Commands grouped by category, in category order, empty categories left out. */
export function groupCommands(commands: Command[]): CommandGroup[] {
  return CATEGORIES
    .map((category) => ({ category, commands: commands.filter((command) => command.category === category) }))
    .filter((group) => group.commands.length > 0);
}

/** The `/` list for what follows the slash: slug or alias prefix, or the title for two letters or more. */
export function matchCommands(prefix: string): Command[] {
  const key = prefix.toLowerCase();
  return commandList().filter((command) => command.slug.startsWith(key)
    || command.aliases.some((alias) => alias.startsWith(key))
    || (key.length > 1 && command.title.toLowerCase().includes(key)));
}

/** The command sheet's search: any word of the name, aliases, title, description or examples. */
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
export function signature(command: Command): string {
  return [`/${command.slug}`, ...command.args.map(argLabel)].join(' ');
}

/**
 * Reads `rest` (what follows `/name `) into the command's arguments: every
 * argument but the last takes one word, the last takes the remainder.
 * `index` is the argument being typed now (the next one after a space).
 */
export function parseArgs(command: Command, rest: string): { values: Record<string, string>; index: number } {
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
export function argumentHint(command: Command, rest: string): Hint {
  const { index } = parseArgs(command, rest);
  const parts = [{ text: `/${command.slug}`, current: false }, ...command.args.map((argument, at) => ({ text: argLabel(argument), current: at === index }))];
  const example = command.examples.find((text) => text.trim().includes(' ')) ?? command.examples[0] ?? '';
  const current = command.args[index];
  const note = !current
    ? 'Nothing more to type · Enter runs it'
    : current.required ? `Next: ${current.name}` : `Optional: ${current.name} · Enter runs it as is`;
  return { parts, example, note };
}
