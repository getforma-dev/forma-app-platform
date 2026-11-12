// APP-179: every row of the Shortcuts tab names an action that exists on main.
// The tab is a hand-written table (`shortcut-tab.ts`), so this test checks
// each row against the sources it describes:
// - every "⋯ → <item>" is an item of the canvas ⋯ menu (`app.tsx`);
// - every "<name> tab" is a tab of the right pane (`PANE_TABS`);
// - every `/word` is a registry verb or widget (`command-registry.ts`), or one
//   of the example addresses the rows use for an app or a folder;
// - no row names an action the whole-app views (ADR-018) removed.
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';

const source = (path: string) => readFileSync(new URL(path, import.meta.url), 'utf8');

/** The labels of the canvas ⋯ menu, read from the page markup. */
function menuItems(): Set<string> {
  const page = source('../src/canvas/app.tsx');
  // Only the ⋯ menu (`#cv-more-menu`), not the zoom menu before it.
  const start = page.indexOf('id="cv-more-menu"');
  assert.ok(start > 0, 'the ⋯ menu is in the page');
  const more = page.slice(start, page.indexOf('</div>', start));
  const labels = [...more.matchAll(/role="menuitem(?:checkbox)?"[^>]*data-tool="[a-z-]+"[^>]*><span>([^<]+)<\/span>/g)].map((match) => match[1]!);
  assert.ok(labels.includes('Take off the canvas') && labels.includes('Shortcuts and commands'), 'the ⋯ menu was read');
  assert.ok(!labels.includes('Zoom in'), 'zoom items are not ⋯ items');
  return new Set(labels);
}

/** Example addresses the rows use: an app (`expenses`), any app (`name`), a folder (`q3`, `folder:q3`). */
const EXAMPLE_ADDRESSES = new Set(['expenses', 'name', 'q3']);

/** Actions ADR-018 removed or made refuse; a row must not teach them. */
const REMOVED: [phrase: RegExp, why: string][] = [
  [/ask tab/i, 'Ask moved to the app assistant and the workspace assistant; the pane has no Ask tab'],
  [/expand an app/i, 'Library app rows have no expander'],
  [/drag the card/i, 'only standalone cards drag onto the canvas'],
  [/\/card [a-z]+\.[a-z]+|\/card \d+\.\d+/, '/card places only a whole standalone card, not an app part'],
  [/\bstacks?\b/i, 'the legacy stack path is retired (APP-176); an app is browsed in Browse view'],
  [/\bframe\b/i, 'united frames became the Open view'],
  [/Break apart|\/split|\/fan\b/i, '/split refuses since ADR-018'],
  [/Shift \+ U/, 'Shift + U no longer breaks an app apart'],
];

async function rows() {
  (globalThis as { navigator?: unknown }).navigator ??= { platform: 'MacIntel', userAgent: '' };
  const { SHORTCUTS } = await import('../src/canvas/shortcut-tab.ts');
  return SHORTCUTS.flatMap(([group, list]) => list.map(([what, click, keys]) => ({ group, what, click, keys })));
}

test('every ⋯ path in the Shortcuts tab is a ⋯ menu item (APP-179)', async () => {
  const menu = menuItems();
  let checked = 0;
  for (const row of await rows()) {
    for (const match of row.click.matchAll(/⋯ → ([^,;]+?)(?= on |,|;|$)/g)) {
      checked += 1;
      assert.ok(menu.has(match[1]!.trim()), `${row.what}: "⋯ → ${match[1]}" is not in the ⋯ menu (${[...menu].join(' · ')})`);
    }
  }
  assert.ok(checked >= 10, `the rows name ⋯ items (${checked})`);
});

test('every tab the Shortcuts tab names is a tab of the right pane (APP-179)', async () => {
  const { PANE_TABS } = await import('../src/canvas/pane.ts');
  const tabs = new Set(PANE_TABS.map((tab) => tab[0]!.toUpperCase() + tab.slice(1)));
  for (const row of await rows()) {
    for (const match of `${row.what} ${row.click}`.matchAll(/\b([A-Z][a-z]+) tab\b/g))
      assert.ok(tabs.has(match[1]!), `${row.what}: "${match[0]}" is not a pane tab (${[...tabs].join(', ')})`);
  }
});

test('every /command in the Shortcuts tab is a registry verb, a widget or an example address (APP-179)', async () => {
  const { VERBS, WIDGET_KINDS } = await import('../src/canvas/command-registry.ts');
  // `/help` is the hidden alias of `/commands` (APP-120).
  const known = new Set<string>([...VERBS, ...WIDGET_KINDS, 'help', ...EXAMPLE_ADDRESSES]);
  let checked = 0;
  for (const row of await rows()) {
    for (const match of row.keys.matchAll(/(?:^|\s)\/([a-z][a-z0-9-]*)/g)) {
      checked += 1;
      assert.ok(known.has(match[1]!), `${row.what}: "/${match[1]}" is neither a verb, a widget nor an example address`);
    }
  }
  assert.ok(checked >= 15, `the rows name commands (${checked})`);
});

test('no Shortcuts row names an action the whole-app views removed (APP-179, ADR-018)', async () => {
  for (const row of await rows()) {
    const text = `${row.what} | ${row.click} | ${row.keys}`;
    for (const [phrase, why] of REMOVED) assert.doesNotMatch(text, phrase, `${row.what}: ${why}`);
  }
});
