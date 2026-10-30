// View layout only (ADR-011 debt, APP-092): card positions and the camera,
// kept in this browser's localStorage per app/work. No data, receipt or
// parameter is stored, and nothing here reaches the server. Storage that is
// unavailable (private mode, blocked site data) just means the default layout.
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
const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;
const CARD = /^[A-Za-z0-9_-]{1,96}$/;

export function loadPlaced(member: string): Map<string, PlacedWork> {
  const placed = new Map<string, PlacedWork>();
  if (!/^[0-9a-f]{16}$/.test(member)) return placed;
  const stored = read(placedKey(member));
  if (typeof stored !== 'object' || stored === null) return placed;
  for (const [work, value] of Object.entries(stored as Record<string, unknown>).slice(0, 64)) {
    const entry = value as Partial<PlacedWork> | null;
    if (!UUID.test(work) || !entry || typeof entry.app !== 'string' || !UUID.test(entry.app) || !Array.isArray(entry.cards)) continue;
    const cards = entry.cards.filter((id): id is string => typeof id === 'string' && CARD.test(id) && id.includes(work)).slice(0, 64);
    if (cards.length) placed.set(work, { app: entry.app, cards });
  }
  return placed;
}
export function savePlaced(member: string, placed: Map<string, PlacedWork>): void {
  if (!/^[0-9a-f]{16}$/.test(member)) return;
  const out: Record<string, PlacedWork> = {};
  for (const [work, entry] of placed) if (entry.cards.length) out[work] = entry;
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
  return stored.slice(0, 32).flatMap((value): StoredStack[] => {
    const entry = value as Partial<StoredStack> | null;
    if (!entry || typeof entry.id !== 'string' || !/^[a-z0-9]{4,24}$/.test(entry.id) || !Array.isArray(entry.cards)) return [];
    if (!finite(entry.x) || !finite(entry.y)) return [];
    const cards = entry.cards.filter((id): id is string => typeof id === 'string' && CARD.test(id)).slice(0, 64);
    return [{
      id: entry.id, title: typeof entry.title === 'string' ? entry.title.slice(0, 80) : '', cards,
      top: finite(entry.top) ? Math.max(0, Math.floor(entry.top)) : 0, x: entry.x, y: entry.y,
    }];
  });
}
export function saveStacks(member: string, stacks: StoredStack[]): void {
  if (/^[0-9a-f]{16}$/.test(member)) write(stacksKey(member), stacks);
}
