// APP-103 (ADR-011 pitch): canvas utility widgets, the weather card, the
// calculator and the clock, placed with `/weather [place]`, `/calculator
// [expression or words]` and `/clock [zone]`, and restyled in words.
//
// Widgets are client-side canvas cards with no governed data: they read no
// organization record, carry no receipt and are labeled "widget". Their
// look is a closed, typed state (`state.ts`); a sentence becomes a new state
// through `POST /v1/widgets/customize` (`card.ts`). Placement and the
// per-browser layout (DS-06) are the canvas's: the board mounts the card
// (`mountWidget`), and this module keeps each widget's state and position in
// this browser's localStorage only (nothing reaches the server). In demo
// mode, as with the app layout, every load starts empty.
//
// Integration: `WIDGET_COMMANDS` is registered with the canvas's slash-command
// registry (`slash.ts` `registerCommands`, from `app.tsx`), so the widgets show
// in the `/` list and `/apps` and run through `runSlash`.
import { announce } from '../render';
import { findVerb, WIDGET_KINDS, type WidgetName } from '../command-registry';
import { activeCanvasItem, canvasHost, isDemoCanvas, mountWidget, onLayoutSaved, unmountWidget, widgetSpot } from '../board';
import { mountCalculator, looksLikeExpression } from './calculator';
import { buildCard, customize, widgetTitle, type WidgetCard } from './card';
import { mountClock } from './clock';
import { defaultState, KINDS, placeOf, validState, ZONES, zoneOf, type ClockState, type WeatherState, type WidgetKind, type WidgetState } from './state';
import { mountWeather } from './weather';

export type WidgetCommand = { slug: string; title: string; purpose: string; aliasOf?: string; run(text: string): void | Promise<void> };

const STORAGE_KEY = 'forma.widgets.v1';
const MAX_WIDGETS = 24;
const SIZE: Record<WidgetKind, [width: number, minHeight: number]> = {
  weather: [400, 380],
  calculator: [340, 560],
  clock: [400, 360],
};

type Placed = { widget: WidgetCard; item: HTMLElement; stop?: () => void };
const placed = new Map<string, Placed>();
let restored = false;

function newId(kind: WidgetKind): string {
  const random = new Uint32Array(2);
  crypto.getRandomValues(random);
  return `w-${kind}-${Array.from(random, (n) => n.toString(36)).join('')}`;
}

type Stored = { id: string; kind: WidgetKind; state: WidgetState; x: number; y: number };

function save(): void {
  if (isDemoCanvas()) return;
  const rows: Stored[] = [];
  for (const { widget, item } of placed.values()) {
    const spot = widgetSpot(item);
    if (spot) rows.push({ id: widget.id, kind: widget.kind, state: widget.state, x: spot.x, y: spot.y });
  }
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(rows));
  } catch {
    // view layout only: nothing to recover
  }
}

function load(): Stored[] {
  try {
    const rows = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? '[]') as unknown;
    if (!Array.isArray(rows)) return [];
    const out: Stored[] = [];
    for (const row of rows.slice(0, MAX_WIDGETS)) {
      if (typeof row !== 'object' || row === null) continue;
      const { id, kind, state, x, y } = row as Record<string, unknown>;
      if (typeof id !== 'string' || !/^w-[a-z]+-[a-z0-9]{1,32}$/.test(id)) continue;
      if (!KINDS.includes(kind as WidgetKind)) continue;
      const valid = validState(kind as WidgetKind, state);
      if (!valid || typeof x !== 'number' || typeof y !== 'number' || !Number.isFinite(x) || !Number.isFinite(y)) continue;
      out.push({ id, kind: kind as WidgetKind, state: valid, x, y });
    }
    return out;
  } catch {
    return [];
  }
}

/** Places a widget of `kind` with `state` and returns it (null before the canvas starts). */
export function placeWidget(
  kind: WidgetKind,
  state: WidgetState = defaultState(kind),
  options: { id?: string; at?: { x: number; y: number }; initial?: string } = {},
): WidgetCard | null {
  if (!canvasHost() || placed.size >= MAX_WIDGETS) return null;
  const id = options.id ?? newId(kind);
  const widget = buildCard(document, kind, id, state);
  let stop: (() => void) | undefined;
  if (kind === 'weather') mountWeather(widget);
  else if (kind === 'calculator') mountCalculator(widget, options.initial ?? '');
  else stop = mountClock(widget);
  const [width, minHeight] = SIZE[kind];
  const item = mountWidget(widget.card, { id, title: widgetTitle(kind), width, minHeight, at: options.at, focus: !options.at });
  if (!item) {
    stop?.();
    return null;
  }
  placed.set(id, { widget, item, stop });
  widget.onState(() => save());
  widget.onRemove(() => {
    stop?.();
    unmountWidget(item);
    placed.delete(id);
    save();
    announce(`${widgetTitle(kind)} removed.`);
  });
  save();
  return widget;
}

/** The widget of `kind` the member has selected on the canvas, if any. */
function selected(kind: WidgetKind): WidgetCard | null {
  const item = activeCanvasItem();
  if (!item) return null;
  for (const entry of placed.values()) {
    if (entry.item === item && entry.widget.kind === kind) return entry.widget;
  }
  return null;
}

const STYLE_WORDS = /\b(make|style|skin|theme|look|looks|colou?r|font|celsius|fahrenheit|metric|imperial|glass|glassy|paper|retro|matrix|ti-?\s?84|dark|light|neon|amber|green|cyan|violet|purple|rose|pink|red|blue|ice|lcd|mono|serif|rounded|bigger|smaller|default|reset)\b/i;

/**
 * Runs `/kind text`: with no text, places a fresh widget; with arguments it
 * can read itself (a place, a zone, an expression), places one with them;
 * with words, restyles the selected widget of that kind, or places one and
 * restyles it. Returns a status line.
 */
export async function runWidget(kind: WidgetKind, text: string): Promise<string> {
  const words = text.trim();
  const title = widgetTitle(kind);
  const current = words ? selected(kind) : null;
  if (kind === 'weather' && words && !STYLE_WORDS.test(words)) {
    const place = placeOf(words);
    if (place) {
      if (current) {
        rememberPlace(place);
        current.apply({ ...(current.state as WeatherState), location: place });
        return `Weather now shows ${place}.`;
      }
      rememberPlace(place);
      return placeWidget('weather', { ...(defaultState('weather') as WeatherState), location: place })
        ? `Weather for ${place} is on your canvas.`
        : 'The widget could not be placed.';
    }
  }
  if (kind === 'clock' && words) {
    const zone = zoneOf(words.replace(/^(in|show)\s+/i, ''));
    if (zone) {
      if (current) {
        current.apply({ ...(current.state as ClockState), zone });
        return `The clock now shows ${words} time.`;
      }
      return placeWidget('clock', { ...(defaultState('clock') as ClockState), zone })
        ? `A ${words} clock is on your canvas.`
        : 'The widget could not be placed.';
    }
  }
  if (kind === 'calculator' && words && looksLikeExpression(words)) {
    return placeWidget('calculator', defaultState('calculator'), { initial: words })
      ? 'Calculator placed with your expression.'
      : 'The widget could not be placed.';
  }
  if (!words) {
    return placeWidget(kind) ? `${title} placed on your canvas.` : 'The widget could not be placed.';
  }
  const target = current ?? placeWidget(kind);
  if (!target) return 'The widget could not be placed.';
  const ok = await customize(target, words);
  return ok ? `${title} shaped.` : `${title} kept its look.`;
}

export const WIDGET_COMMANDS: WidgetCommand[] = [
  {
    slug: 'weather',
    aliasOf: 'widget',
    title: 'Weather',
    purpose: 'Current weather for a place; restyle it in words.',
    run: (text) => runWidget('weather', text).then(announce),
  },
  {
    slug: 'calculator',
    aliasOf: 'widget',
    title: 'Calculator',
    purpose: 'A calculator; type an expression, or restyle it in words.',
    run: (text) => runWidget('calculator', text).then(announce),
  },
  {
    slug: 'clock',
    aliasOf: 'widget',
    title: 'Clock',
    purpose: 'A clock for a time zone; 12 or 24 hour.',
    run: (text) => runWidget('clock', text).then(announce),
  },
];

// --- APP-120: `/widget <weather|calculator|clock|note|folder> [args]` ---

const WIDGET_TITLES: Record<WidgetName, [title: string, detail: string]> = {
  weather: ['Weather', 'Current weather for a place'],
  calculator: ['Calculator', 'Type an expression, or restyle it'],
  clock: ['Clock', 'A time zone; 12 or 24 hour'],
  note: ['Note', 'A quick note, saved as your own record'],
  folder: ['Folder', 'Your files; formation builds an app from them'],
};

/** The kind `word` names (`weather`, `notes` → `note`, `folders` → `folder`), if any. */
export function widgetKind(word: string): WidgetName | undefined {
  const key = word.trim().toLowerCase();
  return WIDGET_KINDS.find((kind) => kind === key || `${kind}s` === key);
}

/**
 * `/widget <kind> [args]`: the one Place verb for widgets. It runs the kind's
 * own verb (`/weather`, `/calculator`, `/clock`, `/note`, `/folder`, now its
 * hidden aliases), so `/widget weather Austin` and `/weather Austin` do the
 * same thing, and `/widget folder` opens the folder picker.
 */
export const WIDGET_VERB = {
  slug: 'widget',
  title: 'Place a widget',
  purpose: 'Weather, calculator, clock, note or folder: yours alone, no governed data',
  run: async (text: string, report: (message: string) => void = announce) => {
    const match = text.trim().match(/^(\S+)\s*([\s\S]*)$/);
    const kind = match ? widgetKind(match[1] ?? '') : undefined;
    if (!kind) {
      report(match
        ? `There is no ${match[1]} widget. Try /widget weather, calculator, clock, note or folder.`
        : 'Name the widget: /widget weather, calculator, clock, note or folder.');
      return;
    }
    const verb = findVerb(kind);
    if (!verb) {
      report(`The ${kind} widget is not available here.`);
      return;
    }
    await verb.run(match?.[2] ?? '', report);
  },
};

/** `/widget <kind>`: the five kinds while the kind is typed (the kind's own suggestions follow, `slash.ts`). */
export function widgetCompletions(rest: string): Suggestion[] {
  const query = rest.trim().toLowerCase();
  return WIDGET_KINDS
    .filter((kind) => !query || kind.startsWith(query))
    .map((kind) => ({ badge: '◇', title: WIDGET_TITLES[kind][0], detail: `${WIDGET_TITLES[kind][1]} · also /${kind}`, value: `/widget ${kind} ` }));
}

// --- APP-118: live suggestions for `/weather <city>` and `/clock <zone>` ---

const RECENT_KEY = 'forma.weather.recent.v1';
const SUGGESTED = ['Austin', 'New York', 'London', 'Tokyo', 'Singapore'];
type Suggestion = { badge: string; title: string; detail: string; value: string };

function storedPlaces(): string[] {
  try {
    const list = JSON.parse(localStorage.getItem(RECENT_KEY) ?? '[]') as unknown;
    return Array.isArray(list) ? list.flatMap((value) => (typeof value === 'string' && placeOf(value) ? [value] : [])).slice(0, 6) : [];
  } catch {
    return [];
  }
}
function rememberPlace(place: string): void {
  try {
    const list = [place, ...storedPlaces().filter((other) => other.toLowerCase() !== place.toLowerCase())].slice(0, 6);
    localStorage.setItem(RECENT_KEY, JSON.stringify(list));
  } catch {
    // a suggestion list only: nothing to recover
  }
}

/** Cities for `/weather`: on the canvas, then recent ones, then a few suggestions. */
export function citySuggestions(rest: string): Suggestion[] {
  const query = rest.trim().toLowerCase();
  const seen = new Set<string>();
  const out: Suggestion[] = [];
  const add = (place: string, detail: string) => {
    const key = place.toLowerCase();
    if (seen.has(key) || (query && !key.startsWith(query) && !key.split(/[\s,]+/).some((word) => word.startsWith(query)))) return;
    seen.add(key);
    out.push({ badge: '☀', title: place, detail, value: `/weather ${place}` });
  };
  for (const { widget } of placed.values()) if (widget.kind === 'weather') add((widget.state as WeatherState).location, 'On your canvas');
  for (const place of storedPlaces()) add(place, 'Recent');
  for (const place of SUGGESTED) add(place, 'Suggested');
  return out.slice(0, 8);
}

/** Zones for `/clock`: the closed list the clock knows. */
export function zoneSuggestions(rest: string): Suggestion[] {
  const query = rest.trim().toLowerCase().replace(/^(in|show)\s+/, '');
  return ZONES
    .filter(([id, name]) => !query || name.toLowerCase().startsWith(query) || id.toLowerCase().includes(query))
    .slice(0, 8)
    .map(([id, name]) => ({ badge: '◷', title: name, detail: id === 'local' ? 'This device’s time' : id, value: `/clock ${name}` }));
}

/** APP-122: the widget `/shape <kind>` addresses: the selected one of that kind, else the first placed. */
export function widgetFor(kind: WidgetKind): WidgetCard | null {
  const chosen = selected(kind);
  if (chosen) return chosen;
  for (const entry of placed.values()) if (entry.widget.kind === kind) return entry.widget;
  return null;
}

/** APP-122: the widget whose canvas item is `item` (⋯ → Shape… on a widget). */
export function widgetOfItem(item: HTMLElement): WidgetCard | null {
  for (const entry of placed.values()) if (entry.item === item) return entry.widget;
  return null;
}

/** APP-104: how many widgets of `kind` are on the canvas (the library pane's indicator). */
export function widgetCount(kind: WidgetKind): number {
  let count = 0;
  for (const entry of placed.values()) if (entry.widget.kind === kind) count += 1;
  return count;
}

/** APP-104: removes widgets from the canvas: every one of `kind`, or the one whose canvas item is `item`. */
export function removeWidgets(match: WidgetKind | HTMLElement): number {
  let removed = 0;
  for (const [id, entry] of [...placed.entries()]) {
    if (typeof match === 'string' ? entry.widget.kind !== match : entry.item !== match) continue;
    entry.stop?.();
    unmountWidget(entry.item);
    placed.delete(id);
    removed += 1;
  }
  if (removed) save();
  return removed;
}

/** Restores this browser's widgets (not in demo mode) and keeps their layout saved. */
export function restoreWidgets(): void {
  if (restored || !canvasHost()) return;
  restored = true;
  onLayoutSaved(save);
  if (isDemoCanvas()) return;
  for (const row of load()) placeWidget(row.kind, row.state, { id: row.id, at: { x: row.x, y: row.y } });
}
