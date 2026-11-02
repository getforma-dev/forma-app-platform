// The canvas palette is this browser's choice. It is not a grant and it is
// not written to the server. Forma (site) stays the default. A dark operating
// system picks the dark block of the chosen family.

export const PALETTES = [
  ['site', 'Forma'],
  ['ink', 'Ink'],
  ['harbor', 'Harbor'],
  ['stone', 'Stone'],
  ['grove', 'Grove'],
] as const;

const KEY = 'forma.canvas.palette.v1';
const SCHEME_KEY = 'forma.canvas.scheme.v1';
/** The three palettes on the dock. Forma stays the default when none is pressed. */
export const DOCK_PALETTES = ['ink', 'harbor', 'stone'] as const;

export function paletteName(value: unknown): string {
  return PALETTES.some(([id]) => id === value) ? String(value) : 'site';
}

/** An explicit Day or Dark choice. Null follows the operating system. */
export function schemeChoice(value: unknown): 'light' | 'dark' | null {
  return value === 'light' || value === 'dark' ? value : null;
}

/** Pressing the active dock palette returns to Forma. */
export function nextPalette(current: string, pressed: string): string {
  if (!(DOCK_PALETTES as readonly string[]).includes(pressed)) return paletteName(current);
  return current === pressed ? 'site' : pressed;
}

/** The data-theme name for a palette and a light or dark scheme. */
export function themeForPalette(palette: string, scheme: 'light' | 'dark'): string {
  const name = paletteName(palette);
  if (name === 'site') return scheme === 'dark' ? 'site-dark' : 'site';
  return scheme === 'dark' ? `${name}-dark` : name;
}

export function applyPalette(document_: Document, palette: string, scheme: 'light' | 'dark'): void {
  document_.documentElement.dataset.theme = themeForPalette(palette, scheme);
}

function systemScheme(): 'light' | 'dark' {
  return typeof matchMedia === 'function' && matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
}

function paint(document_: Document, palette: string, scheme: 'light' | 'dark'): void {
  for (const button of document_.querySelectorAll<HTMLButtonElement>('[data-look-scheme]')) {
    button.setAttribute('aria-pressed', button.dataset.lookScheme === scheme ? 'true' : 'false');
  }
  for (const button of document_.querySelectorAll<HTMLButtonElement>('[data-look-palette]')) {
    button.setAttribute('aria-pressed', button.dataset.lookPalette === palette ? 'true' : 'false');
  }
}

function remember(key: string, value: string): void {
  try {
    localStorage.setItem(key, value);
  } catch {
    // The choice still applies for this page when storage is blocked.
  }
}

export function startPalette(document_: Document): void {
  let palette = 'site';
  let locked: 'light' | 'dark' | null = null;
  try {
    palette = paletteName(localStorage.getItem(KEY));
    locked = schemeChoice(localStorage.getItem(SCHEME_KEY));
  } catch {
    palette = 'site';
    locked = null;
  }
  const scheme = () => locked ?? systemScheme();
  const apply = () => {
    const current = scheme();
    applyPalette(document_, palette, current);
    paint(document_, palette, current);
  };
  apply();
  document_.querySelector('.cv-look')?.addEventListener('click', (event) => {
    const button = (event.target as Element | null)?.closest('button');
    if (!(button instanceof HTMLButtonElement)) return;
    if (button.dataset.lookScheme === 'light' || button.dataset.lookScheme === 'dark') {
      locked = button.dataset.lookScheme;
      remember(SCHEME_KEY, locked);
    }
    const next = button.dataset.lookPalette;
    if (next) {
      palette = nextPalette(palette, next);
      remember(KEY, palette);
    }
    apply();
  });
  if (typeof matchMedia === 'function') {
    matchMedia('(prefers-color-scheme: dark)').addEventListener?.('change', () => {
      if (!locked) apply();
    });
  }
}
