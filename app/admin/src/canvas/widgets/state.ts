// APP-103: the widgets' closed, typed state, mirrored from the server's
// `app/src/widgets/state.rs`. A look is three keys from fixed sets (skin,
// accent, font) plus the widget's own arguments. Every state the canvas
// applies, whether the member's own or the customize route's answer, is
// judged here first; the card then only sets data attributes the widgets
// stylesheet keys on (ADR-011 I5: no value becomes CSS or markup).

export type WidgetKind = 'weather' | 'calculator' | 'clock';
export const KINDS: readonly WidgetKind[] = ['weather', 'calculator', 'clock'];

export const SKINS = ['default', 'ti84', 'matrix', 'retro', 'glass', 'paper'] as const;
export const ACCENTS = ['default', 'matrix', 'amber', 'cyan', 'crimson', 'violet', 'rose', 'ice'] as const;
export const FONTS = ['default', 'mono', 'serif', 'rounded', 'lcd'] as const;
export const UNITS = ['imperial', 'metric'] as const;

export type Skin = (typeof SKINS)[number];
export type Accent = (typeof ACCENTS)[number];
export type Font = (typeof FONTS)[number];
export type Units = (typeof UNITS)[number];

/** Clock zones: IANA id and display name (the server's `ZONES`, same order). */
export const ZONES: readonly (readonly [string, string])[] = [
  ['local', 'Local'],
  ['UTC', 'UTC'],
  ['America/New_York', 'New York'],
  ['America/Chicago', 'Chicago'],
  ['America/Denver', 'Denver'],
  ['America/Los_Angeles', 'Los Angeles'],
  ['America/Anchorage', 'Anchorage'],
  ['Pacific/Honolulu', 'Honolulu'],
  ['America/Toronto', 'Toronto'],
  ['America/Mexico_City', 'Mexico City'],
  ['America/Sao_Paulo', 'São Paulo'],
  ['America/Bogota', 'Bogotá'],
  ['America/Guayaquil', 'Guayaquil'],
  ['Europe/London', 'London'],
  ['Europe/Dublin', 'Dublin'],
  ['Europe/Paris', 'Paris'],
  ['Europe/Berlin', 'Berlin'],
  ['Europe/Madrid', 'Madrid'],
  ['Europe/Rome', 'Rome'],
  ['Europe/Amsterdam', 'Amsterdam'],
  ['Europe/Stockholm', 'Stockholm'],
  ['Europe/Athens', 'Athens'],
  ['Europe/Moscow', 'Moscow'],
  ['Africa/Cairo', 'Cairo'],
  ['Africa/Lagos', 'Lagos'],
  ['Africa/Johannesburg', 'Johannesburg'],
  ['Asia/Dubai', 'Dubai'],
  ['Asia/Kolkata', 'Mumbai'],
  ['Asia/Singapore', 'Singapore'],
  ['Asia/Hong_Kong', 'Hong Kong'],
  ['Asia/Shanghai', 'Shanghai'],
  ['Asia/Seoul', 'Seoul'],
  ['Asia/Tokyo', 'Tokyo'],
  ['Australia/Sydney', 'Sydney'],
  ['Pacific/Auckland', 'Auckland'],
];

export type Look = { skin: Skin; accent: Accent; font: Font };
export type WeatherState = Look & { location: string; units: Units };
export type CalculatorState = Look;
export type ClockState = Look & { zone: string; hour12: boolean };
export type WidgetState = WeatherState | CalculatorState | ClockState;

const KEYS: Record<WidgetKind, readonly string[]> = {
  weather: ['skin', 'accent', 'font', 'location', 'units'],
  calculator: ['skin', 'accent', 'font'],
  clock: ['skin', 'accent', 'font', 'zone', 'hour12'],
};

export function defaultState(kind: WidgetKind): WidgetState {
  const look: Look = { skin: 'default', accent: 'default', font: 'default' };
  if (kind === 'weather') return { ...look, location: 'Austin', units: 'imperial' };
  if (kind === 'clock') return { ...look, zone: 'local', hour12: true };
  return look;
}

function member<T extends string>(set: readonly T[], value: unknown): T | null {
  if (typeof value !== 'string') return null;
  const text = value.trim().toLowerCase();
  return set.find((item) => item.toLowerCase() === text) ?? null;
}

/** The canonical zone id `text` names (id or display name), or null. */
export function zoneOf(text: string): string | null {
  const wanted = text.trim().toLowerCase();
  return ZONES.find(([id, name]) => id.toLowerCase() === wanted || name.toLowerCase() === wanted)?.[0] ?? null;
}

export function zoneName(id: string): string {
  return ZONES.find(([zone]) => zone === id)?.[1] ?? id;
}

/** A place name: 2–80 letters, digits, spaces and `,.'-` (the server's `state::location`). */
export function placeOf(text: string): string | null {
  const collapsed = text.split(/\s+/).filter(Boolean).join(' ');
  const count = Array.from(collapsed).length;
  if (count < 2 || count > 80) return null;
  if (!/^[\p{L}\p{N} ,.'-]+$/u.test(collapsed) || !/\p{L}/u.test(collapsed)) return null;
  return collapsed;
}

/** Judges `value` as `kind`'s state: exactly its keys, every value in its closed set. */
export function validState(kind: WidgetKind, value: unknown): WidgetState | null {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) return null;
  const object = value as Record<string, unknown>;
  const keys = KEYS[kind];
  if (Object.keys(object).length !== keys.length || !keys.every((key) => key in object)) return null;
  const skin = member(SKINS, object.skin);
  const accent = member(ACCENTS, object.accent);
  const font = member(FONTS, object.font);
  if (!skin || !accent || !font) return null;
  const look: Look = { skin, accent, font };
  if (kind === 'weather') {
    const location = typeof object.location === 'string' ? placeOf(object.location) : null;
    const units = member(UNITS, object.units);
    return location && units ? { ...look, location, units } : null;
  }
  if (kind === 'clock') {
    const zone = typeof object.zone === 'string' ? zoneOf(object.zone) : null;
    return zone && typeof object.hour12 === 'boolean' ? { ...look, zone, hour12: object.hour12 } : null;
  }
  return look;
}
