export interface WidgetChromeRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

export type WidgetCommandEdge = "top" | "right" | "bottom" | "left";

export interface WidgetCommandDockGeometry {
  horizontalLength: number;
  horizontalThickness: number;
  sideLength: number;
  sideThickness: number;
  gap: number;
  safeInset: number;
  hysteresis: number;
  handoff?: number;
  handoffRatio?: number;
  gripRadius: number;
}

export interface WidgetCommandDockPlacement {
  edge: WidgetCommandEdge;
  attachmentX: number;
  attachmentY: number;
  state: "ready" | "rescue";
}

function clamp(value: number, min: number, max: number): number {
  return Math.max(min, Math.min(max, value));
}

export function isWidgetCommandEdge(value: string | undefined): value is WidgetCommandEdge {
  return value === "top" || value === "right" || value === "bottom" || value === "left";
}

interface EdgeCandidate extends WidgetCommandDockPlacement {
  clearance: number;
  visibleLength: number;
}

const EDGE_PREFERENCE: readonly WidgetCommandEdge[] = ["top", "right", "bottom", "left"];

function intersection(
  item: WidgetChromeRect,
  viewport: WidgetChromeRect,
): WidgetChromeRect | null {
  const x = Math.max(item.x, viewport.x);
  const y = Math.max(item.y, viewport.y);
  const right = Math.min(item.x + item.width, viewport.x + viewport.width);
  const bottom = Math.min(item.y + item.height, viewport.y + viewport.height);
  if (right <= x || bottom <= y) return null;
  return { x, y, width: right - x, height: bottom - y };
}

function edgeCandidate(
  edge: WidgetCommandEdge,
  item: WidgetChromeRect,
  viewport: WidgetChromeRect,
  geometry: WidgetCommandDockGeometry,
  extraClearance: number,
): EdgeCandidate | null {
  const itemRight = item.x + item.width;
  const itemBottom = item.y + item.height;
  const viewportRight = viewport.x + viewport.width;
  const viewportBottom = viewport.y + viewport.height;
  const horizontal = edge === "top" || edge === "bottom";
  const length = horizontal ? geometry.horizontalLength : geometry.sideLength;
  const thickness = horizontal ? geometry.horizontalThickness : geometry.sideThickness;
  const halfLength = length / 2;
  const requiredClearance = thickness + geometry.gap + geometry.safeInset + extraClearance;
  const clearance = edge === "top"
    ? item.y - viewport.y
    : edge === "bottom"
      ? viewportBottom - itemBottom
      : edge === "left"
        ? item.x - viewport.x
        : viewportRight - itemRight;
  if (clearance < requiredClearance) return null;

  const itemStart = horizontal ? item.x : item.y;
  const itemEnd = horizontal ? itemRight : itemBottom;
  const viewportStart = horizontal ? viewport.x : viewport.y;
  const viewportEnd = horizontal ? viewportRight : viewportBottom;
  const visibleLength = Math.max(
    0,
    Math.min(itemEnd, viewportEnd) - Math.max(itemStart, viewportStart),
  );
  const safeStart = viewportStart + geometry.safeInset + halfLength;
  const safeEnd = viewportEnd - geometry.safeInset - halfLength;
  const attachmentStart = Math.max(itemStart, viewportStart, safeStart);
  const attachmentEnd = Math.min(itemEnd, viewportEnd, safeEnd);
  if (attachmentStart > attachmentEnd) return null;

  const preferred = clamp((itemStart + itemEnd) / 2, attachmentStart, attachmentEnd);
  return {
    edge,
    attachmentX: horizontal ? preferred : edge === "left" ? item.x : itemRight,
    attachmentY: horizontal ? edge === "top" ? item.y : itemBottom : preferred,
    state: "ready",
    clearance,
    visibleLength,
  };
}

function rescuePlacement(
  item: WidgetChromeRect,
  viewport: WidgetChromeRect,
  visible: WidgetChromeRect,
  geometry: WidgetCommandDockGeometry,
): WidgetCommandDockPlacement {
  const viewportRight = viewport.x + viewport.width;
  const viewportBottom = viewport.y + viewport.height;
  const safeX = clamp(
    visible.x + visible.width / 2,
    viewport.x + geometry.gripRadius,
    viewportRight - geometry.gripRadius,
  );
  const safeY = clamp(
    visible.y + visible.height / 2,
    viewport.y + geometry.gripRadius,
    viewportBottom - geometry.gripRadius,
  );
  const itemRight = item.x + item.width;
  const itemBottom = item.y + item.height;
  const visibleEdges: WidgetCommandEdge[] = [];
  if (item.y >= viewport.y && item.y <= viewportBottom) visibleEdges.push("top");
  if (itemRight >= viewport.x && itemRight <= viewportRight) visibleEdges.push("right");
  if (itemBottom >= viewport.y && itemBottom <= viewportBottom) visibleEdges.push("bottom");
  if (item.x >= viewport.x && item.x <= viewportRight) visibleEdges.push("left");
  const edge = visibleEdges[0] ?? "top";
  return {
    edge,
    attachmentX: edge === "left"
      ? clamp(item.x, viewport.x + geometry.gripRadius, viewportRight - geometry.gripRadius)
      : edge === "right"
        ? clamp(itemRight, viewport.x + geometry.gripRadius, viewportRight - geometry.gripRadius)
        : safeX,
    attachmentY: edge === "top"
      ? clamp(item.y, viewport.y + geometry.gripRadius, viewportBottom - geometry.gripRadius)
      : edge === "bottom"
        ? clamp(itemBottom, viewport.y + geometry.gripRadius, viewportBottom - geometry.gripRadius)
        : safeY,
    state: "rescue",
  };
}

/**
 * Chooses a physical widget edge for a fixed-screen-size rail target without reading layout.
 * The longest visible physical edge wins, with top/right/bottom/left as the stable
 * tie order. A previous edge is retained until the winner gains enough visible
 * length to clear the handoff threshold. The attachment is the rail center on the
 * widget edge; visual offsets and the transparent hit target remain CSS-owned.
 */
export function resolveWidgetCommandDockPlacement(
  item: WidgetChromeRect,
  viewport: WidgetChromeRect,
  geometry: WidgetCommandDockGeometry,
  previousEdge?: WidgetCommandEdge,
): WidgetCommandDockPlacement | null {
  const visible = intersection(item, viewport);
  if (!visible) return null;

  const hardCandidates = new Map<WidgetCommandEdge, EdgeCandidate>();
  for (const edge of EDGE_PREFERENCE) {
    const candidate = edgeCandidate(edge, item, viewport, geometry, 0);
    if (candidate) hardCandidates.set(edge, candidate);
  }

  let best: EdgeCandidate | undefined;
  for (const edge of EDGE_PREFERENCE) {
    const candidate = hardCandidates.get(edge);
    if (candidate && (!best || candidate.visibleLength > best.visibleLength)) best = candidate;
  }

  const itemRight = item.x + item.width;
  const itemBottom = item.y + item.height;
  const viewportRight = viewport.x + viewport.width;
  const viewportBottom = viewport.y + viewport.height;
  const canonicalTop = hardCandidates.get("top");
  const isFullyVisible = item.x >= viewport.x && item.y >= viewport.y &&
    itemRight <= viewportRight && itemBottom <= viewportBottom;
  const isTopCentered = canonicalTop !== undefined &&
    Math.abs(canonicalTop.attachmentX - (item.x + itemRight) / 2) < 0.001;
  if (isFullyVisible && isTopCentered) return canonicalTop;

  if (best && previousEdge && best.edge !== previousEdge) {
    const previous = hardCandidates.get(previousEdge);
    if (previous) {
      const handoff = Math.max(
        geometry.handoff ?? geometry.hysteresis * 4,
        previous.visibleLength * (geometry.handoffRatio ?? 0.15),
      );
      if (best.visibleLength - previous.visibleLength < handoff) return previous;
    }
  }

  if (best) return best;

  return rescuePlacement(item, viewport, visible, geometry);
}
