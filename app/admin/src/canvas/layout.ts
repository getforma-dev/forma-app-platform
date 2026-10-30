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
