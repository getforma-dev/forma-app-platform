import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';

const ROLES = [
  '--role-shell', '--role-canvas', '--role-surface', '--role-sunken', '--role-text', '--role-muted', '--role-faint',
  '--role-edge', '--role-line', '--role-hairline', '--role-rule', '--role-accent', '--role-on-accent', '--role-accent-soft',
  '--role-mark', '--role-on-mark', '--role-mark-edge', '--role-mark-hover', '--role-seal', '--role-seal-soft',
  '--role-chart', '--role-chart-peak', '--role-chart-fill', '--role-busy', '--role-bad', '--role-grid',
  '--role-shadow', '--role-shadow-lift', '--role-describe', '--role-describe-edge', '--role-describe-hover',
];

function lin(channel: number): number {
  const c = channel / 255;
  return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
}
function lum(hex: string): number {
  const n = Number.parseInt(hex.slice(1), 16);
  return 0.2126 * lin((n >> 16) & 255) + 0.7152 * lin((n >> 8) & 255) + 0.0722 * lin(n & 255);
}
function contrast(a: string, b: string): number {
  const [hi, lo] = lum(a) > lum(b) ? [lum(a), lum(b)] : [lum(b), lum(a)];
  return (hi + 0.05) / (lo + 0.05);
}

function block(css: string, start: string): string {
  const at = css.indexOf(start);
  assert.ok(at >= 0, start);
  const open = css.indexOf('{', at);
  const close = css.indexOf('}', open);
  return css.slice(open + 1, close);
}

function decl(body: string, name: string): string {
  const found = body.match(new RegExp(`${name}:\\s*([^;]+);`));
  assert.ok(found, name);
  return found[1].trim();
}

test('a judged app theme maps onto one data-theme name', async () => {
  const { themeName } = await import('../src/canvas/talk-view.ts');
  assert.equal(themeName({ density: 'comfortable' }, 'light'), null);
  assert.equal(themeName({ accent: 'default', mode: 'default' }, 'dark'), null);
  assert.equal(themeName({ mode: 'dark' }, 'light'), 'site-dark');
  assert.equal(themeName({ mode: 'light' }, 'dark'), 'site');
  assert.equal(themeName({ accent: 'crimson' }, 'light'), 'crimson');
  assert.equal(themeName({ accent: 'crimson' }, 'dark'), 'crimson-dark');
  assert.equal(themeName({ accent: 'crimson', mode: 'light' }, 'dark'), 'crimson');
  assert.equal(themeName({ accent: 'slate', mode: 'dark' }, 'light'), 'slate-dark');
  assert.equal(themeName({ accent: 'harbor' }, 'dark'), 'harbor-dark');
  assert.equal(themeName({ accent: 'grove', mode: 'light' }, 'dark'), 'grove');
  assert.equal(themeName({}, 'dark'), null);
});

test('every theme block assigns the full role set, and text clears 4.5:1', () => {
  const css = readFileSync(new URL('../src/styles/themes.css', import.meta.url), 'utf8');
  const canvas = readFileSync(new URL('../src/styles/canvas.css', import.meta.url), 'utf8');
  const site = block(css, ':root,\n[data-theme="site"]');
  const siteDark = block(css, '[data-theme="site-dark"]');
  const osDark = block(css, ':root:not([data-theme])');
  const ink = block(css, '[data-theme="ink"],');
  const inkDark = block(css, '[data-theme="ink-dark"],');
  for (const body of [site, siteDark, osDark, ink, inkDark]) {
    for (const role of ROLES) assert.match(body, new RegExp(`${role}:`), role);
  }
  for (const role of ROLES) assert.equal(decl(siteDark, role), decl(osDark, role), role);
  assert.equal(decl(site, '--role-shell'), '#f4f4ed');
  assert.equal(decl(site, '--role-mark'), '#c5f953');
  assert.equal(decl(siteDark, '--role-accent'), '#c5f953');
  assert.ok(contrast(decl(site, '--role-text'), decl(site, '--role-surface')) >= 4.5);
  assert.ok(contrast(decl(site, '--role-muted'), decl(site, '--role-surface')) >= 4.5);
  assert.ok(contrast(decl(site, '--role-on-mark'), decl(site, '--role-mark')) >= 4.5);
  assert.ok(contrast(decl(ink, '--role-text'), decl(ink, '--role-surface')) >= 4.5);
  assert.ok(contrast(decl(ink, '--role-muted'), decl(ink, '--role-surface')) >= 4.5);
  assert.ok(contrast(decl(ink, '--role-faint'), decl(ink, '--role-surface')) >= 4.5);
  assert.ok(contrast(decl(ink, '--role-on-accent'), decl(ink, '--role-accent')) >= 4.5);
  assert.ok(contrast(decl(inkDark, '--role-text'), decl(inkDark, '--role-canvas')) >= 4.5);
  assert.ok(contrast(decl(inkDark, '--role-muted'), decl(inkDark, '--role-canvas')) >= 4.5);
  assert.ok(contrast(decl(inkDark, '--role-edge'), decl(inkDark, '--role-canvas')) >= 3);
  assert.ok(contrast(decl(inkDark, '--role-on-accent'), decl(inkDark, '--role-accent')) >= 4.5);
  for (const name of ['green', 'crimson', 'amber', 'cyan', 'violet', 'rose', 'ice', 'slate']) {
    assert.match(css, new RegExp(`\\[data-theme="${name}"\\]`));
    assert.match(css, new RegExp(`\\[data-theme="${name}-dark"\\]`));
    const light = block(css, `[data-theme="${name}"] {`);
    const dark = block(css, `[data-theme="${name}-dark"] {`);
    assert.ok(contrast(decl(light, '--role-on-accent'), decl(light, '--role-accent')) >= 4.5, name);
    assert.ok(contrast(decl(dark, '--role-on-accent'), decl(dark, '--role-accent')) >= 4.5, `${name}-dark`);
  }
  assert.match(canvas, /data-theme-density="comfortable"/);
  assert.match(css, /--space-1:\s*0\.25rem/);
  assert.match(css, /--radius-1:\s*4px/);
  assert.match(css, /--control:\s*2\.5rem/);
  assert.match(canvas, /\.cv-create-form[\s\S]*background:\s*var\(--role-shell\)|\.cv-create \{[\s\S]*background:\s*var\(--role-shell\)/);
  assert.doesNotMatch(canvas, /--cv-paper|--cv-ink|#f7f4ee|#1c1915/);
  assert.doesNotMatch(canvas, /#18a974|#12805c|#d0343b/);
  assert.doesNotMatch(css, /data-theme="cupcake"|data-theme="cyberpunk"/);
  for (const name of ['harbor', 'stone', 'grove']) {
    const light = block(css, `[data-theme="${name}"] {`);
    const dark = block(css, `[data-theme="${name}-dark"] {`);
    for (const role of ROLES) {
      assert.match(light, new RegExp(`${role}:`), `${name} ${role}`);
      assert.match(dark, new RegExp(`${role}:`), `${name}-dark ${role}`);
    }
    assert.ok(contrast(decl(light, '--role-text'), decl(light, '--role-surface')) >= 4.5, name);
    assert.ok(contrast(decl(light, '--role-muted'), decl(light, '--role-surface')) >= 4.5, name);
    assert.ok(contrast(decl(light, '--role-on-accent'), decl(light, '--role-accent')) >= 4.5, name);
    assert.ok(contrast(decl(light, '--role-edge'), decl(light, '--role-shell')) >= 3, name);
    assert.ok(contrast(decl(dark, '--role-text'), decl(dark, '--role-canvas')) >= 4.5, `${name}-dark`);
    assert.ok(contrast(decl(dark, '--role-on-accent'), decl(dark, '--role-accent')) >= 4.5, `${name}-dark`);
    assert.ok(contrast(decl(dark, '--role-edge'), decl(dark, '--role-canvas')) >= 3, `${name}-dark`);
    for (const role of ['--role-busy', '--role-bad']) {
      assert.ok(contrast(decl(dark, role), decl(dark, '--role-sunken')) >= 4.5, `${name}-dark ${role}`);
      assert.ok(contrast(decl(light, role), decl(light, '--role-sunken')) >= 4.5, `${name} ${role}`);
    }
  }
});

test('a canvas palette is a closed name and follows the color scheme', async () => {
  const { paletteName, themeForPalette } = await import('../src/canvas/palette.ts');
  assert.equal(paletteName('harbor'), 'harbor');
  assert.equal(paletteName('cupcake'), 'site');
  assert.equal(paletteName(null), 'site');
  assert.equal(themeForPalette('site', 'light'), 'site');
  assert.equal(themeForPalette('site', 'dark'), 'site-dark');
  assert.equal(themeForPalette('stone', 'light'), 'stone');
  assert.equal(themeForPalette('grove', 'dark'), 'grove-dark');
  assert.equal(themeForPalette('nope', 'dark'), 'site-dark');
  const { schemeChoice, nextPalette } = await import('../src/canvas/palette.ts');
  assert.equal(nextPalette('site', 'ink'), 'ink');
  assert.equal(nextPalette('ink', 'ink'), 'site');
  assert.equal(nextPalette('harbor', 'grove'), 'harbor');
  assert.equal(schemeChoice('dark'), 'dark');
  assert.equal(schemeChoice('light'), 'light');
  assert.equal(schemeChoice('system'), null);
});

test('a stored app theme keeps only closed names', async () => {
  const { appThemesFromStorage } = await import('../src/canvas/shape.ts');
  const work = '00000000-0000-4000-8000-000000000001';
  assert.deepEqual(appThemesFromStorage({ [work]: { accent: 'harbor', mode: 'dark' } }), [[work, { accent: 'harbor', mode: 'dark' }]]);
  assert.deepEqual(appThemesFromStorage({ [work]: { accent: 'cupcake' }, bad: { accent: 'ink' } }), []);
});

test('Shape distinguishes default renderer tokens from an admitted presentation', async () => {
  const { acceptsAppTheme } = await import('../src/canvas/shape.ts');
  assert.equal(acceptsAppTheme({ dataset: { appMode: 'default', appPresentation: 'default' } }), true);
  assert.equal(acceptsAppTheme({ dataset: { appMode: 'dark', appPresentation: 'authored' } }), false);
  assert.equal(acceptsAppTheme({ dataset: { appMode: 'default', appPresentation: 'authored' } }), false);
  assert.equal(acceptsAppTheme({ dataset: { appMode: 'dark' } }), false);
  assert.equal(acceptsAppTheme({ dataset: {} }), true);
});

test('canvas palette persists ordinary choices but demo mode neither reads nor writes them', async () => {
  const { startPalette } = await import('../src/canvas/palette.ts');
  const prior = ['localStorage', 'HTMLButtonElement', 'matchMedia'].map(name => [name, Object.getOwnPropertyDescriptor(globalThis, name)] as const);
  class Button {
    attrs: Record<string, string> = {};
    constructor(public dataset: Record<string, string>) {}
    closest() { return this; }
    setAttribute(key: string, value: string) { this.attrs[key] = value; }
  }
  try {
    Object.defineProperty(globalThis, 'HTMLButtonElement', { configurable: true, value: Button });
    Object.defineProperty(globalThis, 'matchMedia', { configurable: true, value: () => ({matches: false, addEventListener() {}}) });
    for (const demo of [false, true]) {
      const storage = new Map([['forma.canvas.palette.v1', 'harbor'], ['forma.canvas.scheme.v1', 'dark']]);
      let reads = 0, writes = 0, click: (event: { target: Button }) => void = () => {};
      Object.defineProperty(globalThis, 'localStorage', { configurable: true, value: {
        getItem(key: string) { reads++; return storage.get(key) ?? null; },
        setItem(key: string, value: string) { writes++; storage.set(key, value); },
      }});
      const day = new Button({lookScheme:'light'}), dark = new Button({lookScheme:'dark'}), ink = new Button({lookPalette:'ink'});
      const document_ = {
        documentElement: {dataset: {} as Record<string, string>},
        getElementById() { return {dataset:{demo: demo ? 'on' : 'off'}}; },
        querySelector() { return {addEventListener(_name: string, handler: typeof click) {click=handler;}}; },
        querySelectorAll(selector: string) {return selector==='[data-look-scheme]' ? [day,dark] : [ink];},
      };
      startPalette(document_ as unknown as Document);
      assert.equal(document_.documentElement.dataset.theme, demo ? 'site' : 'harbor-dark');
      click({target:ink});click({target:day});
      assert.equal(document_.documentElement.dataset.theme,'ink');
      assert.equal(ink.attrs['aria-pressed'],'true');assert.equal(day.attrs['aria-pressed'],'true');
      assert.equal(reads,demo?0:2);assert.equal(writes,demo?0:2);
      if (demo) assert.equal(storage.get('forma.canvas.palette.v1'),'harbor');
    }
  } finally {
    for (const [name, descriptor] of prior) if (descriptor) Object.defineProperty(globalThis,name,descriptor); else Reflect.deleteProperty(globalThis,name);
  }
});
