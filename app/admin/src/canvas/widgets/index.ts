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
import { activeCanvasItem, canvasHost, isDemoCanvas, mountWidget, onLayoutSaved, unmountWidget, widgetSpot } from '../board';
import { mountCalculator, looksLikeExpression } from './calculator';
import { buildCard, customize, widgetTitle, type WidgetCard } from './card';
import { mountClock } from './clock';
import { defaultState, KINDS, placeOf, validState, zoneOf, type ClockState, type WeatherState, type WidgetKind, type WidgetState } from './state';
import { mountWeather } from './weather';

export type WidgetCommand = { slug: string; title: string; purpose: string; run(text: string): void | Promise<void> };

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

function announce(message: string): void {
  const status = document.getElementById('cv-activity');
  if (status) status.textContent = message;
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
        current.apply({ ...(current.state as WeatherState), location: place });
        return `Weather now shows ${place}.`;
      }
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
  return ok ? `${title} restyled.` : `${title} kept its look.`;
}

export const WIDGET_COMMANDS: WidgetCommand[] = [
  {
    slug: 'weather',
    title: 'Weather',
    purpose: 'Current weather for a place (“/weather Austin”); restyle it in words.',
    run: (text) => runWidget('weather', text).then(announce),
  },
  {
    slug: 'calculator',
    title: 'Calculator',
    purpose: 'A calculator (“/calculator 12*(3+4)”, or “make it a TI-84 in Matrix style”).',
    run: (text) => runWidget('calculator', text).then(announce),
  },
  {
    slug: 'clock',
    title: 'Clock',
    purpose: 'A clock for a time zone (“/clock Tokyo”), 12 or 24 hour.',
    run: (text) => runWidget('clock', text).then(announce),
  },
];

/** Restores this browser's widgets (not in demo mode) and keeps their layout saved. */
export function restoreWidgets(): void {
  if (restored || !canvasHost()) return;
  restored = true;
  onLayoutSaved(save);
  if (isDemoCanvas()) return;
  for (const row of load()) placeWidget(row.kind, row.state, { id: row.id, at: { x: row.x, y: row.y } });
}
