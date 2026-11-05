// View layout only (ADR-011 debt, APP-092): card positions and the camera,
// kept in this browser's localStorage per app/work. No data, receipt or
// parameter is stored, and nothing here reaches the server. Storage that is
// unavailable (private mode, blocked site data) just means the default layout.
import { storedPageSize } from './page';
import { isUuid } from './ids';

export type ItemLayout = { x: number; y: number; width?: number; height?: number; z?: number; manualScale?: number };
export type Camera = { panX: number; panY: number; zoom: number };

const PREFIX = 'forma.canvas.v1';
const layoutKey = (app: string, work: string) => `${PREFIX}.layout:${app}/${work}`;
const CAMERA_KEY = `${PREFIX}.camera`;

function read(key: string): unknown {
  try {
    const text = localStorage.getItem(key);
    return text ? JSON.parse(text) : null;
  } catch {
    return null;
  }
}
function write(key: string, value: unknown): void {
  try {
    localStorage.setItem(key, JSON.stringify(value));
  } catch {
    // view layout only: nothing to recover
  }
}
const finite = (value: unknown): value is number => typeof value === 'number' && Number.isFinite(value);

export function loadLayout(app: string, work: string): Map<string, ItemLayout> {
  const stored = read(layoutKey(app, work));
  const layout = new Map<string, ItemLayout>();
  if (typeof stored !== 'object' || stored === null) return layout;
  for (const [id, value] of Object.entries(stored as Record<string, unknown>)) {
    const item = value as Partial<ItemLayout>;
    if (!/^[A-Za-z0-9_-]{1,96}$/.test(id) || !finite(item.x) || !finite(item.y)) continue;
    layout.set(id, {
      x: item.x, y: item.y,
      ...(finite(item.width) ? { width: item.width } : {}),
      ...(finite(item.height) ? { height: item.height } : {}),
      ...(finite(item.z) ? { z: item.z } : {}),
      ...(finite(item.manualScale) ? { manualScale: item.manualScale } : {}),
    });
  }
  return layout;
}
export function saveLayout(app: string, work: string, items: Record<string, ItemLayout>): void {
  write(layoutKey(app, work), items);
}
export function loadCamera(): Camera | null {
  const camera = read(CAMERA_KEY) as Partial<Camera> | null;
  return camera && finite(camera.panX) && finite(camera.panY) && finite(camera.zoom)
    ? { panX: camera.panX, panY: camera.panY, zoom: camera.zoom }
    : null;
}
export function saveCamera(camera: Camera): void {
  write(CAMERA_KEY, camera);
}
export function clearLayouts(): void {
  try {
    for (let index = localStorage.length - 1; index >= 0; index -= 1) {
      const key = localStorage.key(index);
      if (key?.startsWith(PREFIX)) localStorage.removeItem(key);
    }
  } catch {
    // nothing stored
  }
}

// APP-104 (DS-67): the cards this member brought onto the canvas, per member
// (a digest the page names, never the principal) in this browser only, as
// `{work: {app, cards}}` in placement order. Kept apart from the layout so
// "Reset layout" leaves the board's contents alone. Card ids only: no data,
// receipt or parameter.
export type PlacedWork = { app: string; cards: string[] };
const placedKey = (member: string) => `forma.canvas.placed.v1:${member}`;
const CARD = /^[A-Za-z0-9_-]{1,96}$/;
// What a browser keeps per member (APP-113: the most recent, capped on save).
const MAX_WORKS = 64;
const MAX_CARDS = 64;
const MAX_FRAMES = 32;

export function loadPlaced(member: string): Map<string, PlacedWork> {
  const placed = new Map<string, PlacedWork>();
  if (!/^[0-9a-f]{16}$/.test(member)) return placed;
  const stored = read(placedKey(member));
  if (typeof stored !== 'object' || stored === null) return placed;
  for (const [work, value] of Object.entries(stored as Record<string, unknown>).slice(-MAX_WORKS)) {
    const entry = value as Partial<PlacedWork> | null;
    if (!isUuid(work) || !entry || typeof entry.app !== 'string' || !isUuid(entry.app) || !Array.isArray(entry.cards)) continue;
    const cards = entry.cards.filter((id): id is string => typeof id === 'string' && CARD.test(id) && id.includes(work)).slice(-MAX_CARDS);
    if (cards.length) placed.set(work, { app: entry.app, cards });
  }
  return placed;
}
export function savePlaced(member: string, placed: Map<string, PlacedWork>): void {
  if (!/^[0-9a-f]{16}$/.test(member)) return;
  // Capped on save, keeping the most recent (a work placed again moves to the end).
  const out: Record<string, PlacedWork> = {};
  for (const [work, entry] of [...placed].filter(([, entry]) => entry.cards.length).slice(-MAX_WORKS)) {
    out[work] = { app: entry.app, cards: entry.cards.slice(-MAX_CARDS) };
  }
  write(placedKey(member), out);
}

// APP-104 stacks (DS-67): per member in this browser, card ids, a title the
// member may edit (text only), the top card and the stack's position.
export type StoredStack = { id: string; title: string; cards: string[]; top: number; x: number; y: number };
const stacksKey = (member: string) => `forma.canvas.stacks.v1:${member}`;
export function loadStacks(member: string): StoredStack[] {
  if (!/^[0-9a-f]{16}$/.test(member)) return [];
  const stored = read(stacksKey(member));
  if (!Array.isArray(stored)) return [];
  return stored.slice(-MAX_FRAMES).flatMap((value): StoredStack[] => {
    const entry = value as Partial<StoredStack> | null;
    if (!entry || typeof entry.id !== 'string' || !/^[a-z0-9]{4,24}$/.test(entry.id) || !Array.isArray(entry.cards)) return [];
    if (!finite(entry.x) || !finite(entry.y)) return [];
    // A stack lists its cards top first (newest on top).
    const cards = entry.cards.filter((id): id is string => typeof id === 'string' && CARD.test(id)).slice(0, MAX_CARDS);
    return [{
      id: entry.id, title: typeof entry.title === 'string' ? entry.title.slice(0, 80) : '', cards,
      top: finite(entry.top) ? Math.max(0, Math.floor(entry.top)) : 0, x: entry.x, y: entry.y,
    }];
  });
}
export function saveStacks(member: string, stacks: StoredStack[]): void {
  if (/^[0-9a-f]{16}$/.test(member)) write(stacksKey(member), stacks.slice(-MAX_FRAMES));
}

// APP-106 unite frames, and APP-142 the page (DS-67): per member in this
// browser, the works open as a page, the page's position and size, and each
// part's position before the page opened (for fan-out). Ids and numbers only.
// A record written before size was stored still loads.
export type StoredUnite = {
  work: string;
  app: string;
  x: number;
  y: number;
  previous: Record<string, { x: number; y: number }>;
  width?: number;
  height?: number;
  /** Missing on earlier records: an old united page is Open. */
  mode?: AppViewMode;
  top?: number;
};
export type AppViewMode = 'face' | 'stack' | 'open';
const unitedKey = (member: string) => `forma.canvas.united.v1:${member}`;
export function loadUnited(member: string): StoredUnite[] {
  if (!/^[0-9a-f]{16}$/.test(member)) return [];
  const stored = read(unitedKey(member));
  if (!Array.isArray(stored)) return [];
  return stored.slice(-MAX_FRAMES).flatMap((value): StoredUnite[] => {
    const entry = value as Partial<StoredUnite> | null;
    if (!entry || typeof entry.work !== 'string' || !isUuid(entry.work) || typeof entry.app !== 'string' || !isUuid(entry.app)) return [];
    if (!finite(entry.x) || !finite(entry.y) || typeof entry.previous !== 'object' || entry.previous === null) return [];
    const previous: Record<string, { x: number; y: number }> = {};
    for (const [id, spot] of Object.entries(entry.previous).slice(-MAX_CARDS)) {
      if (CARD.test(id) && spot && finite(spot.x) && finite(spot.y)) previous[id] = { x: spot.x, y: spot.y };
    }
    const size = storedPageSize(entry.width, entry.height);
    const mode = entry.mode === 'face' || entry.mode === 'stack' ? entry.mode : 'open';
    const top = finite(entry.top) ? Math.min(MAX_CARDS - 1, Math.max(0, Math.floor(entry.top))) : 0;
    return [{ work: entry.work, app: entry.app, x: entry.x, y: entry.y, previous, mode, top, ...size }];
  });
}
export function saveUnited(member: string, united: StoredUnite[]): void {
  if (/^[0-9a-f]{16}$/.test(member)) write(unitedKey(member), united.slice(-MAX_FRAMES));
}

/** Frontend-only migration: one whole-work placement, never persisted data. */
export function migrateAppViews(
  united: StoredUnite[], stacks: StoredStack[],
  works: {app: string; work: string; cards: string[]; standalone?: boolean; at?: {x: number; y: number}}[],
): StoredUnite[] {
  const result: StoredUnite[] = [];
  for (const work of works) {
    const prior = united.find(row => row.app === work.app && row.work === work.work);
    if (prior) {
      result.push({...prior, mode: work.standalone ? 'open' : prior.mode ?? 'open'});
      continue;
    }
    const pile = stacks.find(row => row.cards.some(id => work.cards.includes(id)));
    const siblings = pile ? works.filter(candidate => pile.cards.some(id => candidate.cards.includes(id))) : [];
    const offset = Math.max(0, siblings.findIndex(candidate => candidate.work === work.work)) * 488;
    const at = pile ? {x: pile.x + offset, y: pile.y} : work.at ?? {x: 64, y: 64};
    const topId = pile?.cards[Math.min(pile.top, pile.cards.length - 1)];
    result.push({app: work.app, work: work.work, ...at, previous: {},
      mode: work.standalone ? 'open' : pile ? 'stack' : 'face',
      top: Math.max(0, work.cards.indexOf(topId ?? ''))});
  }
  return result;
}

// Personal Library classification recorded only from explicit creation/user intent.
const standaloneKey = (member: string) => `forma.canvas.standalone.v1:${member}`;
export function loadStandaloneWorks(member: string): Set<string> {
  if (!/^[0-9a-f]{16}$/.test(member)) return new Set();
  const values = read(standaloneKey(member));
  return new Set(Array.isArray(values) ? values.filter((value): value is string => {
    if (typeof value !== 'string') return false;
    const parts = value.split('/');
    return parts.length === 2 && isUuid(parts[0]) && isUuid(parts[1]);
  }).slice(-MAX_WORKS) : []);
}
/** One listed work's saved choice on the canvas page: `card`, `app`, or `null` when the member saved none. */
export type ServerKind = { app: string; work: string; kind: string | null };
/**
 * APP-169: this browser's Cards merged with the member's saved choices.
 * A saved `card` is added and a saved `app` is removed. A listed work with no
 * saved choice keeps its local Card, and that Card is returned in `upload` to
 * be saved once. Works the page did not list, unknown kinds and bad ids leave
 * the local choice as it is. Component count is not an input.
 */
export function mergeArtifactKinds(local: ReadonlySet<string>, rows: readonly ServerKind[]): { cards: Set<string>; upload: string[] } {
  const cards = new Set(local);
  const upload: string[] = [];
  for (const row of rows) {
    if (!isUuid(row.app) || !isUuid(row.work)) continue;
    const key = `${row.app}/${row.work}`;
    if (row.kind === 'card') cards.add(key);
    else if (row.kind === 'app') cards.delete(key);
    else if (row.kind === null && local.has(key) && !upload.includes(key)) upload.push(key);
  }
  return { cards, upload };
}

export function saveStandaloneWorks(member: string, works: Set<string>): void {
  if (!/^[0-9a-f]{16}$/.test(member)) return;
  write(standaloneKey(member), [...works].filter(value => {
    const parts = value.split('/');
    return parts.length === 2 && isUuid(parts[0]) && isUuid(parts[1]);
  }).slice(-MAX_WORKS));
}
