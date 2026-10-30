// The inline SVG chart of a released table (mirrors `canvas_display::chart`
// in Rust, which draws the same chart on the server for a page without
// JavaScript). It reads only the released rows: one value column of
// non-negative exact numbers (an integer, money or decimal column, by its
// declared type), and a time or text column for the other axis; anything
// else has no chart and keeps its table. Every reading is the caller's
// `format` (render.ts `formatValue`). No library, no network: elements are
// built with createElementNS (Trusted Types).
import { integer, label, parseStamp, stampClock, stampDate, stampHuman, stampSeconds, type ColumnType, type Shown, type Stamp } from './display';

export type Bar = { x: string; y: string; width: string; height: string; peak: boolean; tip: string };
export type Chart = {
  label: string;
  caption: string;
  bars: Bar[];
  line: string;
  area: string;
  grid: { y: string; labelY: string; text: string }[];
  ticks: { x: string; anchor: string; text: string }[];
};

const WIDTH = 560;
const HEIGHT = 188;
const LEFT = 52;
const RIGHT = 8;
const TOP = 10;
const BOTTOM = 26;
const LINE_FROM = 24;
const MAX_POINTS = 120;

export function coordinate(value: number): string {
  const rounded = Math.round(value * 10) / 10;
  return rounded === 0 ? '0' : String(rounded);
}

function step(max: bigint): bigint {
  let target = (max + 3n) / 4n;
  if (target < 1n) target = 1n;
  let magnitude = 1n;
  while (magnitude * 10n <= target) magnitude *= 10n;
  for (const factor of [10n, 20n, 25n, 50n, 100n]) {
    if (magnitude < 10n && factor === 25n) continue;
    const candidate = (factor * magnitude) / 10n;
    if (candidate >= target) return candidate;
  }
  return 10n * magnitude;
}

const NUMERIC = ['integer', 'money', 'decimal'];

export function chartOf(
  columns: string[],
  rows: string[][],
  types: (ColumnType | undefined)[],
  format: (text: string, index: number) => Shown,
): Chart | null {
  const isInteger = (index: number) => NUMERIC.includes(types[index]?.type ?? '');
  const value = columns.findIndex((_, index) => isInteger(index));
  const n = rows.length;
  if (value < 0 || n < 2 || n > MAX_POINTS || rows.some((row) => row.length !== columns.length)) return null;
  const values: bigint[] = [];
  for (const row of rows) {
    const parsed = integer(row[value] ?? '');
    if (!parsed || parsed[0]) return null;
    values.push(parsed[1]);
  }
  const max = values.reduce((a, b) => (b > a ? b : a), 0n);
  if (max === 0n) return null;
  const timeColumn = columns.findIndex((_, index) => index !== value && (types[index] === undefined || types[index]?.type === 'timestamp')
    && rows.every((row) => parseStamp(row[index] ?? '') !== null));
  const axis = timeColumn >= 0 ? timeColumn : columns.findIndex((_, index) => !isInteger(index));
  if (axis < 0) return null;
  const stamps: Stamp[] = timeColumn >= 0 ? rows.map((row) => parseStamp(row[timeColumn] ?? '') as Stamp) : [];
  const reading = (v: bigint): [string, string] => {
    const shown = format(v.toString(), value);
    return [shown.text, shown.exact];
  };

  const plotWidth = WIDTH - LEFT - RIGHT;
  const plotHeight = HEIGHT - TOP - BOTTOM;
  const base = TOP + plotHeight;
  const unitStep = step(max);
  const top = ((max + unitStep - 1n) / unitStep) * unitStep;
  const yOf = (v: bigint) => base - (Number(v) / Number(top)) * plotHeight;
  const grid: Chart['grid'] = [];
  for (let k = 0n; k <= top / unitStep; k += 1n) {
    const v = k * unitStep;
    grid.push({ y: coordinate(yOf(v)), labelY: coordinate(yOf(v) + 3.5), text: v === 0n ? '0' : reading(v)[0] });
  }

  const seconds = stamps.map(stampSeconds);
  const sorted = [...seconds].sort((a, b) => a - b);
  const timed = stamps.length > 0 && sorted.every((s, i) => i === 0 || (sorted[i - 1] as number) < s);
  const sameDay = stamps.every((s, i) => i === 0 || (stampDate(s) === stampDate(stamps[i - 1] as Stamp) && s.year === (stamps[i - 1] as Stamp).year));
  const axisText = (index: number): [string, string] => {
    const stamp = stamps[index];
    if (stamp) return [sameDay ? stampClock(stamp) : stampDate(stamp), stampHuman(stamp)];
    const text = rows[index]?.[axis] ?? '';
    const chars = Array.from(text);
    return [chars.length > 12 ? `${chars.slice(0, 11).join('')}…` : text, text];
  };
  const lineMode = timed && n >= LINE_FROM;
  let centres: number[];
  let width = 0;
  if (timed) {
    const first = sorted[0] as number;
    const last = sorted[n - 1] as number;
    if (lineMode) {
      const span = Math.max(1, last - first);
      centres = seconds.map((s) => LEFT + ((s - first) / span) * plotWidth);
    } else {
      let gap = Infinity;
      for (let i = 1; i < n; i += 1) gap = Math.min(gap, (sorted[i] as number) - (sorted[i - 1] as number));
      gap = Math.max(1, gap);
      const start = first - gap / 2;
      const scale = plotWidth / (last - first + gap);
      centres = seconds.map((s) => LEFT + (s - start) * scale);
      width = Math.max(3, gap * scale * 0.64);
    }
  } else {
    const slot = plotWidth / n;
    centres = rows.map((_, i) => LEFT + slot * (i + 0.5));
    width = Math.max(2, slot * 0.64);
  }

  let peak = 0;
  values.forEach((v, i) => { if (v > (values[peak] as bigint)) peak = i; });
  const tip = (i: number) => {
    const parts = [axisText(i)[1]];
    (rows[i] ?? []).forEach((text, index) => {
      if (index !== axis && index !== value) parts.push(format(text, index).text);
    });
    parts.push(reading(values[i] as bigint)[1]);
    return parts.join(' · ');
  };
  const bars: Bar[] = lineMode ? [] : values.map((v, i) => {
    const y = yOf(v);
    return {
      x: coordinate((centres[i] as number) - width / 2),
      y: coordinate(y),
      width: coordinate(width),
      height: coordinate(base - y),
      peak: i === peak,
      tip: tip(i),
    };
  });
  const byX = rows.map((_, i) => i).sort((a, b) => (centres[a] as number) - (centres[b] as number));
  let line = '';
  let area = '';
  if (lineMode) {
    const points = byX.map((i) => `${coordinate(centres[i] as number)} ${coordinate(yOf(values[i] as bigint))}`);
    line = `M${points.join(' L')}`;
    const baseText = coordinate(base);
    area = `${line} L${coordinate(centres[byX[n - 1] as number] as number)} ${baseText} L${coordinate(centres[byX[0] as number] as number)} ${baseText} Z`;
  }

  const wanted = n <= 6 ? byX : [...new Set([0, 1, 2, 3, 4].map((k) => byX[Math.floor((k * (n - 1) + 2) / 4)] as number))];
  const placed: number[] = [];
  for (const index of wanted) {
    const previous = placed[placed.length - 1];
    if (previous !== undefined && (centres[index] as number) - (centres[previous] as number) < 44) {
      if (index === byX[n - 1]) placed.pop();
      else continue;
    }
    placed.push(index);
  }
  const ticks = placed.map((index, k) => ({
    x: coordinate(centres[index] as number),
    anchor: lineMode && k === 0 ? 'start' : lineMode && k + 1 === placed.length ? 'end' : 'middle',
    text: axisText(index)[0],
  }));

  const peakReading = reading(values[peak] as bigint)[0];
  let caption: string;
  if (timed) {
    const first = stamps[byX[0] as number] as Stamp;
    const last = stamps[byX[n - 1] as number] as Stamp;
    const range = sameDay
      ? `${stampDate(first)}, ${stampClock(first)}–${stampClock(last)} UTC`
      : `${stampHuman(first)} – ${stampHuman(last)} UTC`;
    const at = sameDay ? stampClock(stamps[peak] as Stamp) : stampHuman(stamps[peak] as Stamp);
    caption = `${range}. Peak ${peakReading} at ${at}.`;
  } else {
    caption = `Peak ${peakReading}: ${axisText(peak)[1]}.`;
  }
  return {
    label: `${label(columns[value] ?? '')} by ${label(columns[axis] ?? '').toLowerCase()}, ${n} ${lineMode ? 'points' : 'bars'}. ${caption}`,
    caption,
    bars,
    line,
    area,
    grid,
    ticks,
  };
}

const SVG = 'http://www.w3.org/2000/svg';
function svg(document_: Document, tag: string, attributes: Record<string, string>, text?: string): SVGElement {
  const node = document_.createElementNS(SVG, tag) as SVGElement;
  for (const [name, value] of Object.entries(attributes)) node.setAttribute(name, value);
  if (text !== undefined) node.textContent = text;
  return node;
}

/** The chart as the server renders it (`canvas/app.tsx`). */
export function chartFigure(document_: Document, chart: Chart): HTMLElement {
  const root = svg(document_, 'svg', {
    class: 'cv-chart-svg',
    viewBox: `0 0 ${WIDTH} ${HEIGHT}`,
    role: 'img',
    'aria-label': chart.label,
  });
  const gridGroup = svg(document_, 'g', { class: 'cv-grid', 'aria-hidden': 'true' });
  const axisGroup = svg(document_, 'g', { class: 'cv-axis', 'aria-hidden': 'true' });
  for (const line of chart.grid) {
    gridGroup.append(svg(document_, 'line', { x1: '52', x2: '552', y1: line.y, y2: line.y }));
    axisGroup.append(svg(document_, 'text', { x: '44', y: line.labelY, 'text-anchor': 'end' }, line.text));
  }
  for (const tick of chart.ticks) {
    axisGroup.append(svg(document_, 'text', { x: tick.x, y: '180', 'text-anchor': tick.anchor }, tick.text));
  }
  root.append(gridGroup, axisGroup);
  if (chart.bars.length) {
    const bars = svg(document_, 'g', { class: 'cv-bars' });
    for (const bar of chart.bars) {
      const rect = svg(document_, 'rect', {
        class: bar.peak ? 'cv-bar is-peak' : 'cv-bar', x: bar.x, y: bar.y, width: bar.width, height: bar.height, rx: '2',
      });
      rect.append(svg(document_, 'title', {}, bar.tip));
      bars.append(rect);
    }
    root.append(bars);
  }
  if (chart.line) {
    const series = svg(document_, 'g', { class: 'cv-series' });
    root.append(series);
    series.append(svg(document_, 'path', { class: 'cv-area', d: chart.area }), svg(document_, 'path', { class: 'cv-line', d: chart.line, pathLength: '1' }));
  }
  const figure = document_.createElement('figure');
  figure.className = 'cv-chart';
  const caption = document_.createElement('figcaption');
  caption.textContent = chart.caption;
  figure.append(root, caption);
  return figure;
}
