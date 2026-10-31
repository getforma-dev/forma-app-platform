// APP-118: the command registry is the one source. The autocomplete, the
// command sheet and the `?` shortcuts sheet list the same commands, and the
// argument parsing and hints read each command's typed arguments.
import assert from 'node:assert/strict';
import { test } from 'node:test';

const noop = () => undefined;

async function seeded() {
  const registry = await import('../src/canvas/command-registry.ts');
  // Canvas commands as the modules register them (some describe themselves, some rely on KNOWN).
  registry.registerCommands([
    { slug: 'card', title: 'Bring a card', purpose: 'Place one card', run: noop },
    { slug: 'folder', aliases: ['storage', 'box'], title: 'Folders', purpose: 'Your folders', run: noop },
    { slug: 'weather', title: 'Weather', purpose: 'Weather for a place', run: noop },
    { slug: 'arrange', title: 'Arrange by app', purpose: 'Lay the canvas out', run: noop },
    { slug: 'note', aliases: ['notes'], title: 'Note', purpose: 'A quick note', run: noop },
    { slug: 'commands', aliases: ['help'], title: 'All commands', purpose: 'The sheet', run: noop },
    { slug: 'custom', title: 'Custom', purpose: 'Described by itself', category: 'Agents', args: [{ name: 'who', kind: 'text', required: true }], examples: ['/custom me'], run: noop },
  ]);
  // The member's apps from the server; `note` is shadowed by the canvas command.
  registry.setServerCommands([
    { slug: 'expense', aliases: ['expenses'], title: 'Expense claims', purpose: 'File and decide expenses', ...registry.serverMeta('expense', true), run: noop },
    { slug: 'pto', aliases: ['leave'], title: 'Time off', purpose: 'Time off', ...registry.serverMeta('pto', true), run: noop },
    { slug: 'vendor-spend', aliases: [], title: 'Vendor spend', purpose: 'Spend by vendor', ...registry.serverMeta('vendor-spend', false), run: noop },
    { slug: 'note', aliases: [], title: 'Notes app', purpose: 'Library notes', ...registry.serverMeta('note', true), run: noop },
  ]);
  return registry;
}

const slugsOf = (groups: { commands: { slug: string }[] }[]) => groups.flatMap((group) => group.commands.map((command) => command.slug));

test('the autocomplete, the command sheet and the ? sheet list the same commands (APP-118)', async () => {
  const registry = await seeded();
  const { slashGroups } = await import('../src/canvas/slash.ts');
  const { sheetGroups } = await import('../src/canvas/command-sheet.ts');
  const { shortcutCommandGroups } = await import('../src/canvas/controls.ts');
  const all = registry.commandList().map((command) => command.slug);
  assert.deepEqual(slugsOf(slashGroups('')), all);
  assert.deepEqual(slugsOf(sheetGroups('')), all);
  assert.deepEqual(slugsOf(shortcutCommandGroups()), all);
  // Each command once; a canvas command shadows the server's of the same slug.
  assert.equal(new Set(all).size, all.length);
  assert.equal(registry.findCommand('note')?.title, 'Note');
  // Grouped in the registry's category order, with the same groups everywhere.
  const categories = slashGroups('').map((group) => group.category);
  assert.deepEqual(categories, ['Apps', 'Records', 'Folders', 'Canvas', 'Widgets', 'Agents', 'Help']);
  assert.deepEqual(sheetGroups('').map((group) => group.category), categories);
  assert.deepEqual(shortcutCommandGroups().map((group) => group.category), categories);
  // Every listed command carries a category, a description and at least one example.
  for (const command of registry.commandList()) {
    assert.ok(command.purpose && command.examples.length, command.slug);
  }
});

test('the / list filters by slug, alias and title; the sheet searches descriptions too', async () => {
  const registry = await seeded();
  const { slashGroups } = await import('../src/canvas/slash.ts');
  const { sheetGroups } = await import('../src/canvas/command-sheet.ts');
  assert.deepEqual(slugsOf(slashGroups('exp')), ['expense']);
  assert.deepEqual(slugsOf(slashGroups('stor')), ['folder']);
  assert.deepEqual(slugsOf(slashGroups('help')), ['commands']);
  assert.ok(slugsOf(sheetGroups('spend')).includes('vendor-spend'));
  assert.deepEqual(slugsOf(sheetGroups('/weather')), ['weather']);
  assert.deepEqual(slugsOf(sheetGroups('zzqq')), []);
  // An alias finds the command.
  assert.equal(registry.findCommand('leave')?.slug, 'pto');
  assert.equal(registry.findCommand('HELP')?.slug, 'commands');
});

test('signatures and argument hints come from the typed arguments', async () => {
  const registry = await seeded();
  const find = (slug: string) => {
    const command = registry.findCommand(slug);
    assert.ok(command, slug);
    return command;
  };
  assert.equal(registry.signature(find('expense')), '/expense [what you spent]');
  assert.equal(registry.signature(find('card')), '/card <card name or number>');
  assert.equal(registry.signature(find('folder')), '/folder [folder, new or formation] [name or instruction]');
  assert.equal(registry.signature(find('arrange')), '/arrange');
  assert.equal(registry.signature(find('custom')), '/custom <who>');
  // A server app the registry does not know gets its kind's argument.
  assert.equal(registry.signature(find('vendor-spend')), '/vendor-spend [what to show]');
  assert.equal(find('vendor-spend').category, 'Apps');
  assert.equal(find('pto').category, 'Records');

  // `/expense ` → the hint marks its one argument and shows an example.
  const expense = registry.argumentHint(find('expense'), '');
  assert.deepEqual(expense.parts.filter((part) => part.current).map((part) => part.text), ['[what you spent]']);
  assert.match(expense.example, /^\/expense \$86/);
  // `/folder ` marks the folder; after `1 ` the instruction.
  const folder = find('folder');
  assert.deepEqual(registry.argumentHint(folder, '').parts.filter((p) => p.current).map((p) => p.text), ['[folder, new or formation]']);
  assert.deepEqual(registry.argumentHint(folder, '1').parts.filter((p) => p.current).map((p) => p.text), ['[folder, new or formation]']);
  assert.deepEqual(registry.argumentHint(folder, '1 ').parts.filter((p) => p.current).map((p) => p.text), ['[name or instruction]']);
  assert.deepEqual(registry.parseArgs(folder, '1 formation build a metrics app'), {
    values: { 'folder, new or formation': '1', 'name or instruction': 'formation build a metrics app' }, index: 1,
  });
  assert.deepEqual(registry.parseArgs(folder, 'new Q3 board pack').values['name or instruction'], 'Q3 board pack');
  // The last argument takes the rest; a command with none says Enter runs it.
  assert.deepEqual(registry.parseArgs(find('card'), '  spend by category ').values, { 'card name or number': 'spend by category' });
  assert.equal(registry.argumentHint(find('arrange'), '').note, 'Nothing more to type · Enter runs it');
  assert.equal(registry.argumentHint(find('card'), '').note, 'Next: card name or number');
});
