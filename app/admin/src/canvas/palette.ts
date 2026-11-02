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

export function paletteName(value: unknown): string {
  return PALETTES.some(([id]) => id === value) ? String(value) : 'site';
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

function scheme(): 'light' | 'dark' {
  return typeof matchMedia === 'function' && matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
}

export function startPalette(document_: Document): void {
  let palette = 'site';
  try {
    palette = paletteName(localStorage.getItem(KEY));
  } catch {
    palette = 'site';
  }
  applyPalette(document_, palette, scheme());
  const select = document_.getElementById('cv-palette');
  if (select instanceof HTMLSelectElement) {
    select.value = palette;
    select.addEventListener('change', () => {
      palette = paletteName(select.value);
      try {
        localStorage.setItem(KEY, palette);
      } catch {
        // The choice still applies for this page when storage is blocked.
      }
      applyPalette(document_, palette, scheme());
    });
  }
  if (typeof matchMedia === 'function') {
    matchMedia('(prefers-color-scheme: dark)').addEventListener?.('change', () => {
      applyPalette(document_, palette, scheme());
    });
  }
}
