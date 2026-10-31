// APP-118: the command registry is the one source. The autocomplete and
// the Commands tab list the same commands, and the argument parsing and hints read each command's typed arguments.
// APP-119: commands are verbs, apps are addresses, from the server with a
// derived hint and examples; `/slug …` still runs them. The right pane has
// three tabs, Library, Commands and Shortcuts: `?` is Shortcuts.
// APP-120: one grammar (APP-121 adds /rename and /alias: fourteen). `/` lists the verbs in four groups (Place,
// Arrange, Make, Help) and no address; the first letter adds the Apps
// section; the old widget verbs and `/help` are hidden aliases; a bare
// `/<address>` places the app and `/<address> words` tells it something.
import assert from 'node:assert/strict';
import { test } from 'node:test';

const noop = () => undefined;
const EXPENSE_APP = '00000000-0000-4000-8000-000000001681';
const REVIEW_APP = '00000000-0000-4000-8000-000000001001';
const PTO_APP = '00000000-0000-4000-8000-000000001701';
const SPEND_APP = '00000000-0000-4000-8000-000000001011';
const SAVED_APP = '00000000-0000-4000-8000-0000000000a1';
const SAVED_WORK = '00000000-0000-4000-8000-0000000000a2';

/** `GET /v1/commands` as the server answers it (APP-120: the address rule; old names are aliases). */
const SERVER = {
  commands: [
    { slug: 'expense-review', aliases: ['review-expenses'], title: 'Expense review (demo data)', purpose: 'Review submitted expense claims', kind: 'library', app_id: REVIEW_APP, record: true,
      hint: 'what to show … or review', examples: ['/expense-review expense review queue', '/expense-review approve expenses'] },
    { slug: 'expenses', aliases: ['expense', 'claim'], title: 'Expenses', purpose: 'File and decide expenses', kind: 'library', app_id: EXPENSE_APP, record: true,
      hint: 'file a claim or approve one', examples: ['/expenses $86 at Nobu, client dinner, Sarah approves', '/expenses'] },
    { slug: 'time-off', aliases: ['pto', 'leave', 'vacation'], title: 'Time off', purpose: 'Book time off', kind: 'library', app_id: PTO_APP, record: true,
      hint: 'dates and reason', examples: ['/time-off Nov 24–26 for Thanksgiving, Sarah approves'] },
    { slug: 'vendor-spend-breakdown', aliases: ['vendor-spend', 'spend', 'vendors'], title: 'Vendor spend breakdown', purpose: 'Spend by vendor', kind: 'library', app_id: SPEND_APP, record: true,
      hint: 'what to show … or review', examples: ['/vendor-spend-breakdown vendor spend breakdown'] },
    // An older server: no hint, no examples. The Notes app is the Note widget's (`/note`, `/notes` are its verb).
    { slug: 'notes', aliases: ['note'], title: 'Notes', purpose: 'Library notes', kind: 'library', app_id: '00000000-0000-4000-8000-000000001f01', record: true },
    { slug: 'big-claims', aliases: [], title: 'Big claims', purpose: 'Saved app: Big claims', kind: 'saved', app_id: SAVED_APP, work_id: SAVED_WORK, record: true,
      hint: 'what to show, or what to file or decide', examples: ['/big-claims'] },
  ],
};

/** What the hidden widget aliases ran with (the spies stand in for the board). */
const ran: string[] = [];
const spy = (slug: string) => ({ slug, aliasOf: 'widget', title: slug, purpose: `A ${slug}`, run: (text: string) => { ran.push(`${slug}:${text}`); } });

async function seeded() {
  const registry = await import('../src/canvas/command-registry.ts');
  const slash = await import('../src/canvas/slash.ts');
  const library = await import('../src/canvas/library.ts');
  const widgets = await import('../src/canvas/widgets/index.ts');
  const naming = await import('../src/canvas/naming.ts');
  const shape = await import('../src/canvas/shape.ts');
  const talk = await import('../src/canvas/talk.ts');
  // The fourteen verbs as the modules register them (some describe themselves,
  // some rely on KNOWN), and the hidden aliases.
  registry.registerCommands([
    library.APPS_COMMAND, library.CARD_COMMAND, library.APP_COMMAND, library.FOCUS_COMMAND,
    widgets.WIDGET_VERB,
    spy('weather'), spy('calculator'), spy('clock'),
    { ...spy('note'), aliases: ['notes'] },
    { slug: 'folder', aliases: ['storage', 'box'], title: 'Make an app from a folder', purpose: 'Formation builds an app from a folder', run: (text: string) => { ran.push(`folder:${text}`); } },
    { slug: 'arrange', title: 'Arrange by app', purpose: 'Lay the canvas out', run: noop },
    { slug: 'unite', title: 'Unite an app', purpose: 'One frame', run: noop },
    { slug: 'split', aliases: ['break'], title: 'Break an app apart', purpose: 'Back to cards', run: noop },
    { slug: 'shortcuts', aliases: ['keys'], title: 'Shortcuts', purpose: 'Every action', run: noop },
    { slug: 'agent', aliases: ['agents', 'mcp'], title: 'Connect an agent', purpose: 'Mint a token', run: noop },
    slash.COMMANDS,
    naming.RENAME_COMMAND, naming.ALIAS_COMMAND,
    shape.SHAPE_COMMAND, talk.ASK_COMMAND,
  ]);
  registry.setAddresses(slash.parseCommands(SERVER).map(slash.serverAddress));
  return { registry, slash };
}

const slugsOf = (groups: { commands: { slug: string }[] }[]) => groups.flatMap((group) => group.commands.map((command) => command.slug));
const VERBS = ['app', 'card', 'widget', 'arrange', 'unite', 'split', 'focus', 'folder', 'rename', 'alias', 'shape', 'ask', 'apps', 'commands', 'shortcuts', 'agent'];

test('/ lists the 16 verbs in six groups and no addresses; a first letter adds the Apps section (APP-120, APP-122)', async () => {
  const { registry, slash } = await seeded();
  const { sheetGroups } = await import('../src/canvas/command-sheet.ts');
  const groups = slash.slashGroups('');
  assert.deepEqual(slugsOf(groups), VERBS);
  assert.deepEqual(groups.map((group) => [group.category, group.commands.length]), [['Place', 3], ['Arrange', 4], ['Make', 3], ['Shape', 1], ['Ask', 1], ['Help', 4]]);
  assert.deepEqual(slash.slashAddresses('', () => true), [], 'no app under / alone, even on the canvas');
  assert.deepEqual(slash.slashAddressSection('', () => true).addresses, []);
  // The Commands tab: the same four groups; hidden aliases show on their verb's row only.
  assert.deepEqual(slugsOf(sheetGroups('')), VERBS);
  assert.deepEqual(sheetGroups('').map((group) => group.category), ['Place', 'Arrange', 'Make', 'Shape', 'Ask', 'Help']);
  // APP-122: Ask is listed as it is typed, `? <question>`; Shape names its three targets.
  assert.equal(registry.signature(registry.findVerb('ask')!), '? <question>');
  assert.equal(registry.findVerb('ask')?.insert, '? ');
  assert.equal(registry.signature(registry.findVerb('shape')!), '/shape <app.card|app|widget> <words>');
  assert.deepEqual(slugsOf(sheetGroups('weather')), ['widget'], 'a hidden alias finds its verb');
  assert.deepEqual(slugsOf(sheetGroups('pto')), [], 'searching the Commands tab finds no app');
  assert.ok(slugsOf(sheetGroups('expenses')).every((slug) => VERBS.includes(slug)), 'only verbs (their examples)');
  assert.deepEqual(registry.hiddenAliases('widget').map((command) => command.slug), ['weather', 'calculator', 'clock', 'note']);
  // Signatures: the Make verb reads as one sentence.
  assert.equal(registry.signature(registry.findVerb('folder')!), '/folder <name> formation <instruction>');
  assert.equal(registry.signature(registry.findVerb('widget')!), '/widget <weather|calculator|clock|note|folder> [args]');
  assert.equal(registry.signature(registry.findVerb('card')!), '/card <app.card or number>');
  // No /remove.
  assert.equal(registry.findCommand('remove'), undefined);
  // From the first letter: matching verbs (hidden aliases too), then the Apps section.
  assert.deepEqual(slugsOf(slash.slashGroups('w')), ['widget', 'weather']);
  assert.deepEqual(slugsOf(slash.slashGroups('n')), ['note']);
  assert.deepEqual(slugsOf(slash.slashGroups('hel')), ['commands'], '/help autocompletes to /commands');
  const works = [{ app: EXPENSE_APP, work: 'w1', title: '', cards: [] }, { app: REVIEW_APP, work: 'w2', title: '', cards: [] }];
  const onCanvas = (address: Parameters<typeof slash.listedAddress>[0]) => slash.listedAddress(address, works);
  const section = slash.slashAddressSection('e', onCanvas);
  assert.equal(section.title, registry.APPS_SECTION);
  assert.deepEqual(section.addresses.map((address) => address.slug), ['expenses', 'expense-review']);
  assert.deepEqual(section.addresses.map((address) => address.hint), ['file a claim or approve one', 'what to show … or review']);
  // Decision 4: on a clean canvas the section falls back to the library; an exact name or old alias is always offered.
  const clean = (address: Parameters<typeof slash.listedAddress>[0]) => slash.listedAddress(address, []);
  assert.equal(slash.slashAddressSection('ex', clean).title, slash.LIBRARY_SECTION);
  assert.deepEqual(slash.slashAddresses('pto', clean).map((address) => address.slug), ['time-off']);
  assert.deepEqual(slash.slashAddresses('expense', clean).map((address) => address.slug), ['expenses']);
  assert.deepEqual(slash.slashAddresses('b', clean).map((address) => address.slug), ['big-claims']);
  // The grammar's one sentence.
  assert.equal(registry.GRAMMAR, 'Type / and a verb to change the canvas, / and an app’s name to tell that app something, or just a sentence.');
  // The Notes app is the Note widget's: `/note` and `/notes` are the verb.
  assert.equal(registry.findCommand('notes')?.slug, 'note');
  assert.ok(!registry.addressList().some((address) => address.slug === 'notes'));
  for (const command of registry.commandList()) assert.ok(command.purpose && command.examples.length, command.slug);
});

test('aliases resolve: /weather and /widget weather, /note, /help, /expense bare and with words (APP-120)', async () => {
  const { registry, slash } = await seeded();
  ran.length = 0;
  const messages: string[] = [];
  const report = (message: string) => { messages.push(message); };
  // `/widget weather` and `/weather` run the same widget verb.
  assert.equal(registry.findCommand('weather')?.aliasOf, 'widget');
  await slash.runSlash('/widget weather Austin', report);
  await slash.runSlash('/weather Austin', report);
  await slash.runSlash('/widget note Call the auditor', report);
  await slash.runSlash('/note Call the auditor', report);
  await slash.runSlash('/notes', report);
  await slash.runSlash('/widget folder', report);
  assert.deepEqual(ran, ['weather:Austin', 'weather:Austin', 'note:Call the auditor', 'note:Call the auditor', 'note:', 'folder:']);
  await slash.runSlash('/widget teapot', report);
  assert.match(messages.at(-1) ?? '', /There is no teapot widget/);
  // `/help` is `/commands`.
  assert.equal(registry.findCommand('help')?.slug, 'commands');
  assert.equal(registry.findCommand('HELP')?.slug, 'commands');
  // A bare address places the app (`/expense` = `/expenses` = `/app expenses`).
  const placed: string[] = [];
  slash.setAppPlacer(async (address) => { placed.push(address.slug); return true; });
  await slash.runSlash('/expense', report);
  await slash.runSlash('/expenses', report);
  await slash.runSlash('/pto', report);
  assert.deepEqual(placed, ['expenses', 'expenses', 'time-off']);
  // `/expense <words>` files through the action path pinned to the app, as before.
  const posted: unknown[] = [];
  (globalThis as { fetch?: unknown }).fetch = async (url: string, init?: { body?: string }) => {
    if (url === '/v1/commands') return { ok: true, json: async () => SERVER };
    posted.push([url, JSON.parse(init?.body ?? 'null')]);
    return { ok: false, status: 422, json: async () => ({ code: 'could_not_parse' }) };
  };
  await slash.runSlash('/expense $86 at Nobu, client dinner, Sarah approves', report);
  await slash.runSlash('/expenses $86 at Nobu, client dinner, Sarah approves', report);
  assert.deepEqual(posted, [
    ['/v1/act', { utterance: '$86 at Nobu, client dinner, Sarah approves', app_id: EXPENSE_APP }],
    ['/v1/act', { utterance: '$86 at Nobu, client dinner, Sarah approves', app_id: EXPENSE_APP }],
  ]);
  assert.equal(placed.length, 3, 'words never place the bare app');
  // The bar's hint for an address reads its canonical name, title and hint.
  const expenses = registry.findCommand('expense')!;
  assert.equal(expenses.slug, 'expenses');
  assert.equal(registry.signature(expenses), '/expenses [file a claim or approve one]');
  assert.match(registry.argumentHint(expenses, '').example, /^\/expenses \$86/);
  // A saved app still opens its work.
  const saved = registry.findAddress('big-claims');
  assert.equal(saved?.kind, 'saved');
  assert.equal(saved?.work, SAVED_WORK);
  // An older server's entry (no hint, no examples) still reads.
  assert.equal(slash.parseCommands(SERVER).find((entry) => entry.slug === 'notes')?.hint, '');
});

test('the Library row shows the address, its aliases, hint and example; /apps searches slug, alias and hint (APP-119)', async () => {
  const { registry } = await seeded();
  const { rowAddress, firstExample } = await import('../src/canvas/library.ts');
  const list = registry.addressList();
  // A library row by its slug (or an old one), a saved app's row by its work.
  const expenses = rowAddress({ slug: 'expenses', work: 'seeded' }, list);
  assert.equal(expenses?.hint, 'file a claim or approve one');
  assert.equal(rowAddress({ slug: 'expense', work: 'seeded' }, list)?.slug, 'expenses', 'a page rendered before the rename');
  assert.equal(firstExample(expenses!), '/expenses $86 at Nobu, client dinner, Sarah approves');
  assert.equal(rowAddress({ slug: '', work: SAVED_WORK }, list)?.slug, 'big-claims');
  assert.equal(rowAddress({ slug: '', work: 'other' }, list), undefined);
  assert.equal(rowAddress({ slug: 'weather', work: '', widget: 'weather' }, list), undefined);
  const search = (query: string) => registry.searchAddresses(query).map((address) => address.slug);
  assert.deepEqual(search('expense'), ['expense-review', 'expenses']);
  assert.deepEqual(search('/pto'), ['time-off'], 'by alias');
  assert.deepEqual(search('vacation'), ['time-off']);
  assert.deepEqual(search('approve'), ['expenses'], 'by hint');
  assert.ok(search('').includes('big-claims'));
});

// --- The right pane: one pane, three tabs (a small fake DOM: no browser) ---

class FakeNode {
  attributes = new Map<string, string>();
  dataset: Record<string, string> = {};
  children: FakeNode[] = [];
  listeners = new Map<string, ((event: unknown) => void)[]>();
  hidden = false;
  value = '';
  parent: FakeNode | null = null;
  text = '';
  classes = new Set<string>();
  classList = {
    add: (name: string) => this.classes.add(name),
    remove: (name: string) => this.classes.delete(name),
    contains: (name: string) => this.classes.has(name),
    toggle: (name: string, on?: boolean) => ((on ?? !this.classes.has(name)) ? this.classes.add(name) : this.classes.delete(name)),
  };
  constructor(public owner: FakeDocument, public tagName = 'div') {}
  get ownerDocument() { return this.owner; }
  get isConnected() { return true; }
  get id() { return this.attributes.get('id') ?? ''; }
  get textContent(): string { return this.text + this.children.map((child) => child.textContent).join(''); }
  setAttribute(name: string, value: string) {
    if (name === 'hidden') this.hidden = true;
    else if (name.startsWith('data-')) this.dataset[name.slice(5).replace(/-([a-z])/g, (_m, c: string) => c.toUpperCase())] = value;
    else if (name === 'class') value.split(/\s+/).filter(Boolean).forEach((c) => this.classes.add(c));
    this.attributes.set(name, value);
    if (name === 'id') this.owner.ids.set(value, this);
  }
  getAttribute(name: string) { return this.attributes.get(name) ?? null; }
  removeAttribute(name: string) { this.attributes.delete(name); }
  append(...nodes: (FakeNode | string)[]) {
    for (const node of nodes) {
      const child = typeof node === 'string' ? this.owner.createTextNode(node) : node;
      child.parent = this;
      this.children.push(child);
    }
  }
  replaceChildren(...nodes: FakeNode[]) { this.children = []; this.append(...nodes); }
  addEventListener(type: string, listener: (event: unknown) => void) { this.listeners.set(type, [...(this.listeners.get(type) ?? []), listener]); }
  dispatch(type: string, extra: Record<string, unknown> = {}) {
    const event = { type, target: this, defaultPrevented: false, preventDefault() { this.defaultPrevented = true; }, stopPropagation: noop, ...extra };
    for (let node: FakeNode | null = this; node; node = node.parent) for (const listener of node.listeners.get(type) ?? []) listener(event);
    return event;
  }
  focus() { this.owner.activeElement = this; }
  contains(node: unknown): boolean { return node === this || this.children.some((child) => child.contains(node)); }
  all(): FakeNode[] { return [this, ...this.children.flatMap((child) => child.all())]; }
  matches(selector: string): boolean {
    const attr = selector.match(/^\[([a-z-]+)(?:="([^"]*)")?\]$/);
    if (attr) return attr[2] === undefined ? this.attributes.has(attr[1]!) : this.attributes.get(attr[1]!) === attr[2];
    if (selector.startsWith('.')) return this.classes.has(selector.slice(1));
    return false;
  }
  querySelector(selector: string) { return this.all().slice(1).find((node) => node.matches(selector)) ?? null; }
  querySelectorAll(selector: string) { return this.all().slice(1).filter((node) => node.matches(selector)); }
}
class FakeDocument {
  ids = new Map<string, FakeNode>();
  activeElement: FakeNode | null = null;
  body = new FakeNode(this, 'body');
  createElement(tag: string) { return new FakeNode(this, tag); }
  createTextNode(text: string) { const node = new FakeNode(this, '#text'); node.text = text; return node; }
  getElementById(id: string) { return this.ids.get(id) ?? null; }
  // No `<dialog open>`, `[data-modal-above]` or `aria-modal` layer is open.
  querySelector(_selector: string) { return null; }
  querySelectorAll(_selector: string) { return []; }
}

/** The page's pane markup (`app.tsx` `#cv-library`), as the fake DOM builds it. */
function page() {
  const document_ = new FakeDocument();
  const node = (tag: string, attributes: Record<string, string>, ...children: FakeNode[]) => {
    const made = document_.createElement(tag);
    for (const [name, value] of Object.entries(attributes)) made.setAttribute(name, value);
    made.append(...children);
    return made;
  };
  const libraryTab = node('button', { role: 'tab', id: 'cv-tab-library', 'aria-selected': 'true', 'aria-controls': 'cv-panel-library', 'data-pane-tab': 'library' });
  const commandsTab = node('button', { role: 'tab', id: 'cv-tab-commands', 'aria-selected': 'false', 'aria-controls': 'cv-panel-commands', 'data-pane-tab': 'commands' });
  const askTab = node('button', { role: 'tab', id: 'cv-tab-ask', 'aria-selected': 'false', 'aria-controls': 'cv-panel-ask', 'data-pane-tab': 'ask' });
  const shortcutsTab = node('button', { role: 'tab', id: 'cv-tab-shortcuts', 'aria-selected': 'false', 'aria-controls': 'cv-panel-shortcuts', 'data-pane-tab': 'shortcuts' });
  const tablist = node('div', { role: 'tablist' }, libraryTab, commandsTab, askTab, shortcutsTab);
  const close = node('button', { 'data-lib-close': '' });
  const search = node('input', { id: 'cv-lib-search' });
  const libraryPanel = node('div', { id: 'cv-panel-library', role: 'tabpanel' }, search);
  const commandsPanel = node('div', { id: 'cv-panel-commands', role: 'tabpanel', hidden: '' });
  const askPanel = node('div', { id: 'cv-panel-ask', role: 'tabpanel', hidden: '' });
  const shortcutsPanel = node('div', { id: 'cv-panel-shortcuts', role: 'tabpanel', hidden: '' });
  const pane = node('aside', { id: 'cv-library', 'data-open': 'false', 'data-tab': 'library' }, node('header', {}, tablist, close), libraryPanel, commandsPanel, askPanel, shortcutsPanel);
  const toggle = node('button', { id: 'cv-library-toggle', 'aria-expanded': 'false' });
  const input = node('input', { id: 'intent-input' });
  const board = node('div', { class: 'forma-canvas-viewport' });
  node('div', { id: 'cv-shell' }, toggle, pane, board, input);
  return { document_, pane, libraryTab, commandsTab, askTab, shortcutsTab, tablist, libraryPanel, commandsPanel, askPanel, shortcutsPanel, toggle, input, board };
}

test('the right pane opens on the right tab from each entry point (APP-119)', async () => {
  const { document_, pane, libraryTab, commandsTab, shortcutsTab, tablist, libraryPanel, commandsPanel, askPanel, shortcutsPanel, toggle, input, board } = page();
  (globalThis as { document?: unknown }).document = document_;
  (globalThis as { matchMedia?: unknown }).matchMedia = () => ({ matches: false });
  (globalThis as { fetch?: unknown }).fetch = async () => ({ ok: true, json: async () => SERVER });
  const panes = await import('../src/canvas/pane.ts');
  const sheet = await import('../src/canvas/command-sheet.ts');
  const shortcuts = await import('../src/canvas/shortcut-tab.ts');
  const library = await import('../src/canvas/library.ts');
  const controls = await import('../src/canvas/controls.ts');
  const { slash } = await seeded();
  panes.startPane(document_ as unknown as Document);
  sheet.startCommandSheet(document_ as unknown as Document);
  shortcuts.startShortcutTab(document_ as unknown as Document);
  // The canvas keys (no canvas in the fake page: only the key handler is wired).
  controls.startControls(document_ as unknown as Document);
  const state = () => ({ open: pane.dataset.open, tab: pane.dataset.tab, library: !libraryPanel.hidden, commands: !commandsPanel.hidden, ask: !askPanel.hidden, shortcuts: !shortcutsPanel.hidden });
  const on = (tab: string) => ({ open: 'true', tab, library: tab === 'library', commands: tab === 'commands', ask: tab === 'ask', shortcuts: tab === 'shortcuts' });
  const closed = (tab: string) => ({ ...on(tab), open: 'false' });
  const onLibrary = on('library');
  const onCommands = on('commands');
  const onShortcuts = on('shortcuts');
  const key = (target: FakeNode, name: string) => {
    const event = { key: name, code: '', target, defaultPrevented: false, isComposing: false, shiftKey: name === '?', metaKey: false, ctrlKey: false, altKey: false,
      composedPath: () => [target], preventDefault() { this.defaultPrevented = true; }, stopPropagation: noop, stopImmediatePropagation: noop };
    controls.onKey(event as unknown as KeyboardEvent);
    return event;
  };

  // `L` opens it on Library; `L` again closes it.
  key(board, 'l');
  assert.deepEqual(state(), onLibrary);
  assert.equal(toggle.getAttribute('aria-expanded'), 'true');
  library.toggleLibrary();
  assert.deepEqual(state(), closed('library'));

  // `/commands` and `/help` open it on Commands, filtered; focus goes to its search.
  input.focus();
  await slash.COMMANDS.run('folder', noop);
  assert.deepEqual(state(), onCommands);
  const search = commandsPanel.querySelector('.cv-commands-search')!;
  assert.equal(document_.activeElement, search);
  assert.equal(search.value, 'folder');
  assert.ok(commandsPanel.textContent.includes('Your apps answer to /name — see /apps'), 'the footer');
  assert.ok(!commandsPanel.textContent.includes('/expense'), 'no app in the Commands tab');
  // While on Commands, `L` switches to Library (it does not close the pane).
  library.toggleLibrary();
  assert.deepEqual(state(), onLibrary);
  // `/apps expense` opens it on Library with the search filled.
  await library.APPS_COMMAND.run('expense', noop);
  assert.deepEqual(state(), onLibrary);
  // `/help` is the same verb.
  assert.equal((await import('../src/canvas/command-registry.ts')).findCommand('help')?.slug, 'commands');
  await slash.COMMANDS.run('', noop);
  assert.deepEqual(state(), onCommands);
  // Esc in the pane clears the search first, then closes; focus returns to the bar.
  search.value = 'x';
  pane.dispatch('keydown', { key: 'Escape' });
  assert.deepEqual(state(), onCommands);
  assert.equal(search.value, '');
  pane.dispatch('keydown', { key: 'Escape' });
  assert.deepEqual(state(), closed('commands'));
  assert.equal(document_.activeElement, input);

  // `/shortcuts` (and ⋯ → Shortcuts) open it on Shortcuts (APP-122: `?` is Ask now).
  slash.openHelp(input as unknown as HTMLElement);
  assert.deepEqual(state(), onShortcuts);
  assert.equal(document_.activeElement, shortcutsTab);
  // The tab lists every action with its click path and keys; rows keep their /commands.
  const text = shortcutsPanel.textContent;
  assert.ok(text.includes('Unite an app') && text.includes('/unite expense'), 'a row with its /command');
  assert.ok(text.includes('Shortcuts tab') && !text.includes('This sheet') && !text.includes('this sheet'), 'the Shortcuts row describes the tab');
  assert.ok(text.includes('Ask about your data') && text.includes('? where are the red invoices'), 'APP-122: the Ask and Shape rows');
  assert.ok(text.includes('/shape calculator make it a TI-84') && !text.includes('Restyle'), 'APP-122: widgets shape, the Restyle row is gone');
  // Its one line at the top links to the Commands tab.
  const link = shortcutsPanel.querySelector('[data-pane-goto="commands"]')!;
  assert.equal(link.textContent, 'Every command with its arguments → Commands');
  link.dispatch('click');
  assert.deepEqual(state(), onCommands);
  // Esc (search empty) closes it; focus returns to the bar, which opened it.
  pane.dispatch('keydown', { key: 'Escape' });
  assert.deepEqual(state(), closed('commands'));
  assert.equal(document_.activeElement, input);

  // APP-122: `?` on the canvas puts `? ` in the bar, ready to ask; the pane stays as it is.
  board.focus();
  assert.equal(key(board, '?').defaultPrevented, true);
  assert.deepEqual(state(), closed('commands'));
  assert.equal(input.value, '? ');
  assert.equal(document_.activeElement, input);
  input.value = '';
  assert.equal(document_.getElementById('cv-shortcuts'), null, 'the old #cv-shortcuts modal is gone');
  assert.ok(!('showSheet' in controls), 'no showSheet export');
  // `?` typed in a text field is just text.
  panes.closePane();
  const typed = key({ ...input, matches: (selector: string) => selector.includes('input') } as unknown as FakeNode, '?');
  assert.equal(typed.defaultPrevented, false);
  assert.equal(pane.dataset.open, 'false');
  // `L` from Shortcuts switches to Library.
  key(board, '?');
  key(board, 'l');
  assert.deepEqual(state(), onLibrary);

  // The tabs are ARIA tabs: one selected, one tab stop; ←/→/Home/End move and select.
  commandsTab.dispatch('click');
  assert.deepEqual(state(), onCommands);
  assert.equal(commandsTab.getAttribute('aria-selected'), 'true');
  assert.equal(libraryTab.getAttribute('aria-selected'), 'false');
  assert.equal(shortcutsTab.getAttribute('aria-selected'), 'false');
  assert.equal(commandsTab.getAttribute('tabindex'), '0');
  assert.equal(libraryTab.getAttribute('tabindex'), '-1');
  assert.equal(shortcutsTab.getAttribute('tabindex'), '-1');
  tablist.dispatch('keydown', { key: 'ArrowLeft' });
  assert.deepEqual(state(), onLibrary);
  assert.equal(document_.activeElement, libraryTab);
  tablist.dispatch('keydown', { key: 'ArrowLeft' });
  assert.deepEqual(state(), onShortcuts, '← wraps to the last tab');
  assert.equal(document_.activeElement, shortcutsTab);
  tablist.dispatch('keydown', { key: 'ArrowRight' });
  assert.deepEqual(state(), onLibrary, '→ wraps to the first tab');
  tablist.dispatch('keydown', { key: 'End' });
  assert.deepEqual(state(), onShortcuts);
  tablist.dispatch('keydown', { key: 'Home' });
  assert.deepEqual(state(), onLibrary);
  shortcutsTab.dispatch('click');
  assert.deepEqual(state(), onShortcuts);
  // The one × closes it, whatever the tab.
  panes.closePane();
  assert.equal(pane.dataset.open, 'false');
  assert.equal(toggle.getAttribute('aria-expanded'), 'false');
});
