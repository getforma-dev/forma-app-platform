// APP-118: the command registry is the one source. The autocomplete and
// the Commands tab list the same commands, and the argument parsing and hints read each command's typed arguments.
// APP-119: commands are verbs, apps are addresses: `/` and `/commands` list
// verbs only; apps come from the server with a derived hint and examples,
// show in their own section from the first letter (on the canvas, saved, or
// an exact slug), and `/slug …` still runs them. The right pane has three
// tabs, Library, Commands and Shortcuts, and each entry point opens the
// right one: `?` (canvas and empty bar) is Shortcuts, as on GitHub or Linear.
import assert from 'node:assert/strict';
import { test } from 'node:test';

const noop = () => undefined;
const EXPENSE_APP = '00000000-0000-4000-8000-000000001681';
const REVIEW_APP = '00000000-0000-4000-8000-000000001001';
const PTO_APP = '00000000-0000-4000-8000-000000001701';
const SPEND_APP = '00000000-0000-4000-8000-000000001011';
const SAVED_APP = '00000000-0000-4000-8000-0000000000a1';
const SAVED_WORK = '00000000-0000-4000-8000-0000000000a2';

/** `GET /v1/commands` as the server answers it (APP-119: with `hint` and `examples`). */
const SERVER = {
  commands: [
    { slug: 'expense-review', aliases: ['review-expenses'], title: 'Expense review queue', purpose: 'Review submitted expense claims', kind: 'library', app_id: REVIEW_APP, record: true,
      hint: 'what to show … or review', examples: ['/expense-review expense review queue', '/expense-review approve expenses'] },
    { slug: 'expense', aliases: ['expenses', 'claim'], title: 'Expense claims', purpose: 'File and decide expenses', kind: 'library', app_id: EXPENSE_APP, record: true,
      hint: 'what you spent', examples: ['/expense $86 at Nobu, client dinner, Sarah approves', '/expense'] },
    { slug: 'pto', aliases: ['time-off', 'leave', 'vacation'], title: 'Time off', purpose: 'Book time off', kind: 'library', app_id: PTO_APP, record: true,
      hint: 'dates and reason', examples: ['/pto Nov 24–26 for Thanksgiving, Sarah approves'] },
    { slug: 'vendor-spend', aliases: ['spend', 'vendors'], title: 'Vendor spend', purpose: 'Spend by vendor', kind: 'library', app_id: SPEND_APP, record: true,
      hint: 'what to show … or review', examples: ['/vendor-spend vendor spend breakdown'] },
    // An older server: no hint, no examples.
    { slug: 'note', aliases: ['notes'], title: 'Notes app', purpose: 'Library notes', kind: 'library', app_id: '00000000-0000-4000-8000-000000001f01', record: true },
    { slug: 'big-claims', aliases: [], title: 'Big claims', purpose: 'Saved app: Big claims', kind: 'saved', app_id: SAVED_APP, work_id: SAVED_WORK, record: true,
      hint: 'what to show, or what to file or decide', examples: ['/big-claims'] },
  ],
};

async function seeded() {
  const registry = await import('../src/canvas/command-registry.ts');
  const slash = await import('../src/canvas/slash.ts');
  // Canvas commands (verbs) as the modules register them (some describe themselves, some rely on KNOWN).
  registry.registerCommands([
    { slug: 'apps', title: 'Library', purpose: 'Open the Library', run: noop },
    { slug: 'card', title: 'Bring a card', purpose: 'Place one card', run: noop },
    { slug: 'folder', aliases: ['storage', 'box'], title: 'Folders', purpose: 'Your folders', run: noop },
    { slug: 'weather', title: 'Weather', purpose: 'Weather for a place', run: noop },
    { slug: 'arrange', title: 'Arrange by app', purpose: 'Lay the canvas out', run: noop },
    { slug: 'note', aliases: ['notes'], title: 'Note', purpose: 'A quick note', run: noop },
    { slug: 'custom', title: 'Custom', purpose: 'Described by itself', category: 'Agents', args: [{ name: 'who', kind: 'text', required: true }], examples: ['/custom me'], run: noop },
    slash.COMMANDS,
  ]);
  registry.setAddresses(slash.parseCommands(SERVER).map(slash.serverAddress));
  return { registry, slash };
}

const slugsOf = (groups: { commands: { slug: string }[] }[]) => groups.flatMap((group) => group.commands.map((command) => command.slug));
const VERBS = ['apps', 'card', 'arrange', 'note', 'folder', 'weather', 'custom', 'commands'];

test('/ alone lists verbs only; the Commands tab lists the same verbs (APP-119)', async () => {
  const { registry, slash } = await seeded();
  const { sheetGroups } = await import('../src/canvas/command-sheet.ts');
  const all = registry.commandList().map((command) => command.slug);
  assert.deepEqual(all, VERBS);
  assert.deepEqual(slugsOf(slash.slashGroups('')), all);
  assert.deepEqual(slash.slashAddresses('', () => true), [], 'no app under / alone, even on the canvas');
  assert.deepEqual(slugsOf(sheetGroups('')), all);
  // No app section anywhere in the verb lists: the Apps and Records categories are gone.
  const categories = slash.slashGroups('').map((group) => group.category);
  assert.deepEqual(categories, ['Library', 'Canvas', 'Folders', 'Widgets', 'Agents', 'Help']);
  assert.ok(!categories.some((category) => (category as string) === 'Apps' || (category as string) === 'Records'));
  assert.deepEqual(sheetGroups('').map((group) => group.category), categories);
  assert.deepEqual(slugsOf(sheetGroups('expense')), [], 'searching the Commands tab finds no app');
  // The footer points to /apps.
  assert.equal(registry.ADDRESS_FOOTER, 'Your apps answer to /name — see /apps');
  // A verb shadows the app of the same name (`/note`); each verb once.
  assert.equal(registry.findCommand('note')?.title, 'Note');
  assert.ok(!registry.addressList().some((address) => address.slug === 'note'));
  for (const command of registry.commandList()) assert.ok(command.purpose && command.examples.length, command.slug);
});

test('/ex shows the matching verbs, then an Apps section with derived hints (APP-119)', async () => {
  const { registry, slash } = await seeded();
  // Both expense apps on the canvas.
  const works = [{ app: EXPENSE_APP, work: 'w1', title: '', cards: [] }, { app: REVIEW_APP, work: 'w2', title: '', cards: [] }];
  const onCanvas = (address: Parameters<typeof slash.listedAddress>[0]) => slash.listedAddress(address, works);
  assert.deepEqual(slugsOf(slash.slashGroups('ex')), [], 'no verb starts with ex');
  const apps = slash.slashAddresses('ex', onCanvas);
  assert.deepEqual(apps.map((address) => address.slug), ['expense', 'expense-review'], 'shorter own-name match first');
  assert.deepEqual(apps.map((address) => address.hint), ['what you spent', 'what to show … or review']);
  assert.equal(registry.APPS_SECTION, 'Apps — type /name and what you want');
  // Verbs first, then apps: `/a` lists `/apps` and `/arrange`, apps only by their own names.
  assert.deepEqual(slugsOf(slash.slashGroups('a')), ['apps', 'arrange']);
  // Decision 4: on a clean canvas the section lists only saved apps …
  const clean = (address: Parameters<typeof slash.listedAddress>[0]) => slash.listedAddress(address, []);
  assert.deepEqual(slash.slashAddresses('ex', clean), []);
  const fallback = slash.slashAddressSection('ex', clean);
  assert.equal(fallback.title, slash.LIBRARY_SECTION, 'a clean canvas falls back to the library');
  assert.equal(fallback.addresses[0]?.slug, 'expense', '/e ranks the app whose own name starts with e, shortest first');
  assert.ok(fallback.addresses.length <= 5);
  assert.deepEqual(slash.slashAddressSection('', clean).addresses, [], 'nothing under / alone');
  assert.deepEqual(slash.slashAddresses('b', clean).map((address) => address.slug), ['big-claims']);
  // … but an exact slug or alias of any app the member can open is offered (`/pto` in the demo).
  assert.deepEqual(slash.slashAddresses('pto', clean).map((address) => address.slug), ['pto']);
  assert.deepEqual(slash.slashAddresses('leave', clean).map((address) => address.slug), ['pto']);
  assert.deepEqual(slash.slashAddresses('expense', clean).map((address) => address.slug), ['expense']);
});

test('/expense, /pto and a saved /big-claims still resolve and run as before (APP-119)', async () => {
  const { registry } = await seeded();
  const expense = registry.findCommand('expense');
  assert.equal(expense?.title, 'Expense claims', 'findCommand falls back to the address');
  assert.equal(registry.findCommand('expenses')?.slug, 'expense');
  assert.equal(registry.findCommand('leave')?.slug, 'pto');
  assert.equal(registry.findCommand('big-claims')?.slug, 'big-claims');
  assert.equal(registry.findCommand('HELP')?.slug, 'commands');
  // Signatures and hints read the server's hint (the catalog override keeps the demo copy).
  assert.equal(registry.signature(expense!), '/expense [what you spent]');
  assert.equal(registry.signature(registry.findCommand('pto')!), '/pto [dates and reason]');
  assert.equal(registry.signature(registry.findCommand('vendor-spend')!), '/vendor-spend [what to show … or review]');
  const hint = registry.argumentHint(expense!, '');
  assert.deepEqual(hint.parts.filter((part) => part.current).map((part) => part.text), ['[what you spent]']);
  assert.match(hint.example, /^\/expense \$86/);
  // An older server's entry (no hint, no examples) still reads.
  const parsed = (await import('../src/canvas/slash.ts')).parseCommands(SERVER);
  assert.equal(parsed.find((entry) => entry.slug === 'note')?.hint, '');
  // The run paths are the APP-101/102 ones: a saved app opens its work, `/expense` alone the ledger.
  const saved = registry.findAddress('big-claims');
  assert.equal(saved?.kind, 'saved');
  assert.equal(saved?.work, SAVED_WORK);
  assert.equal(typeof saved?.run, 'function');
});

test('the Library shows each app\'s address, hint and example; /apps searches slug, alias and hint (APP-119)', async () => {
  const { registry } = await seeded();
  const { rowAddress, firstExample } = await import('../src/canvas/library.ts');
  const list = registry.addressList();
  // A library row by its slug, a saved app's row by its work (it had no slug).
  const expense = rowAddress({ slug: 'expense', work: 'seeded' }, list);
  assert.equal(expense?.hint, 'what you spent');
  assert.equal(firstExample(expense!), '/expense $86 at Nobu, client dinner, Sarah approves');
  assert.equal(rowAddress({ slug: '', work: SAVED_WORK }, list)?.slug, 'big-claims');
  assert.equal(rowAddress({ slug: '', work: 'other' }, list), undefined);
  assert.equal(rowAddress({ slug: 'weather', work: '', widget: 'weather' }, list), undefined);
  // `/apps expense` filters by slug, alias and hint.
  const search = (query: string) => registry.searchAddresses(query).map((address) => address.slug);
  assert.deepEqual(search('expense'), ['expense-review', 'expense']);
  assert.deepEqual(search('/time-off'), ['pto'], 'by alias');
  assert.deepEqual(search('vacation'), ['pto']);
  assert.deepEqual(search('spent'), ['expense']);
  assert.deepEqual(search('review'), ['expense-review', 'vendor-spend']);
  // A saved app appears once it is named (the server lists it after "Save as /name").
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
  const shortcutsTab = node('button', { role: 'tab', id: 'cv-tab-shortcuts', 'aria-selected': 'false', 'aria-controls': 'cv-panel-shortcuts', 'data-pane-tab': 'shortcuts' });
  const tablist = node('div', { role: 'tablist' }, libraryTab, commandsTab, shortcutsTab);
  const close = node('button', { 'data-lib-close': '' });
  const search = node('input', { id: 'cv-lib-search' });
  const libraryPanel = node('div', { id: 'cv-panel-library', role: 'tabpanel' }, search);
  const commandsPanel = node('div', { id: 'cv-panel-commands', role: 'tabpanel', hidden: '' });
  const shortcutsPanel = node('div', { id: 'cv-panel-shortcuts', role: 'tabpanel', hidden: '' });
  const pane = node('aside', { id: 'cv-library', 'data-open': 'false', 'data-tab': 'library' }, node('header', {}, tablist, close), libraryPanel, commandsPanel, shortcutsPanel);
  const toggle = node('button', { id: 'cv-library-toggle', 'aria-expanded': 'false' });
  const input = node('input', { id: 'intent-input' });
  const board = node('div', { class: 'forma-canvas-viewport' });
  node('div', { id: 'cv-shell' }, toggle, pane, board, input);
  return { document_, pane, libraryTab, commandsTab, shortcutsTab, tablist, libraryPanel, commandsPanel, shortcutsPanel, toggle, input, board };
}

test('the right pane opens on the right tab from each entry point (APP-119)', async () => {
  const { document_, pane, libraryTab, commandsTab, shortcutsTab, tablist, libraryPanel, commandsPanel, shortcutsPanel, toggle, input, board } = page();
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
  const state = () => ({ open: pane.dataset.open, tab: pane.dataset.tab, library: !libraryPanel.hidden, commands: !commandsPanel.hidden, shortcuts: !shortcutsPanel.hidden });
  const on = (tab: string) => ({ open: 'true', tab, library: tab === 'library', commands: tab === 'commands', shortcuts: tab === 'shortcuts' });
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

  // `?` in the empty bar opens it on Shortcuts (not Commands): `?` means the keyboard shortcuts.
  slash.openHelp(input as unknown as HTMLElement);
  assert.deepEqual(state(), onShortcuts);
  assert.equal(document_.activeElement, shortcutsTab);
  // The tab lists every action with its click path and keys; rows keep their /commands.
  const text = shortcutsPanel.textContent;
  assert.ok(text.includes('Unite an app') && text.includes('/unite expense'), 'a row with its /command');
  assert.ok(text.includes('Shortcuts tab') && !text.includes('This sheet') && !text.includes('this sheet'), 'the ? row describes the tab');
  // Its one line at the top links to the Commands tab.
  const link = shortcutsPanel.querySelector('[data-pane-goto="commands"]')!;
  assert.equal(link.textContent, 'Every command with its arguments → Commands');
  link.dispatch('click');
  assert.deepEqual(state(), onCommands);
  // Esc (search empty) closes it; focus returns to the bar, which opened it.
  pane.dispatch('keydown', { key: 'Escape' });
  assert.deepEqual(state(), closed('commands'));
  assert.equal(document_.activeElement, input);

  // `?` on the canvas opens it on Shortcuts too; no modal sheet is made.
  board.focus();
  assert.equal(key(board, '?').defaultPrevented, true);
  assert.deepEqual(state(), onShortcuts);
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
