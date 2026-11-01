// APP-142: the united app is a page in this browser. The size is view layout.
// Sharing names the app's saved work. It never names one card.

export const PAGE_MIN_WIDTH = 320;
export const PAGE_MIN_HEIGHT = 280;
const PAGE_MAX_STORED = 4000;
const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;

/** Clamps a dragged page to the header-plus-one-part minimum and the viewport. */
export function pageSize(
  asked: { width: number; height: number },
  viewport: { width: number; height: number },
): { width: number; height: number } {
  const maxWidth = Math.max(PAGE_MIN_WIDTH, viewport.width);
  const maxHeight = Math.max(PAGE_MIN_HEIGHT, viewport.height);
  return {
    width: Math.min(maxWidth, Math.max(PAGE_MIN_WIDTH, asked.width)),
    height: Math.min(maxHeight, Math.max(PAGE_MIN_HEIGHT, asked.height)),
  };
}

/** A stored size this browser may restore. Anything else is omitted. */
export function storedPageSize(width: unknown, height: unknown): { width?: number; height?: number } {
  const size: { width?: number; height?: number } = {};
  if (typeof width === 'number' && Number.isFinite(width) && width >= PAGE_MIN_WIDTH && width <= PAGE_MAX_STORED) size.width = width;
  if (typeof height === 'number' && Number.isFinite(height) && height >= PAGE_MIN_HEIGHT && height <= PAGE_MAX_STORED) size.height = height;
  return size;
}

/**
 * The existing share route for the app's saved work. The body names a member.
 * It does not name a card.
 */
export function appShareRequest(app: string, work: string, grantee: string): { url: string; body: string } | null {
  if (!UUID.test(app) || !UUID.test(work) || !UUID.test(grantee)) return null;
  return {
    url: `/v1/work/${app}/${work}/share`,
    body: JSON.stringify({ grantee_principal_id: grantee }),
  };
}
