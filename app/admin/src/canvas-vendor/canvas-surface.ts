import type { WidgetCanvasElements } from "./widget-canvas";

export interface CanvasSurface {
  root: HTMLElement;
  grid: HTMLElement;
  elements: WidgetCanvasElements;
  setNavigatorVisible(visible: boolean): void;
  dispose(): void;
}

export interface CanvasSurfaceOptions {
  className?: string;
  navigator?: boolean;
  navigatorLabel?: string;
}

export function createCanvasSurface(
  container: HTMLElement,
  options: CanvasSurfaceOptions = {},
): CanvasSurface {
  const document_ = container.ownerDocument;
  const root = document_.createElement("section");
  root.className = ["forma-canvas", options.className].filter(Boolean).join(" ");
  root.dataset.formaCanvas = "";

  const viewport = document_.createElement("div");
  viewport.className = "forma-canvas-viewport";
  viewport.dataset.formaCanvasViewport = "";
  viewport.tabIndex = 0;
  viewport.setAttribute("aria-label", "Widget canvas");

  const grid = document_.createElement("div");
  grid.className = "forma-canvas-grid";
  grid.setAttribute("aria-hidden", "true");

  const stage = document_.createElement("div");
  stage.className = "forma-canvas-stage";
  stage.dataset.formaCanvasStage = "";
  stage.setAttribute("role", "list");

  const zoomStatus = document_.createElement("output");
  zoomStatus.className = "forma-canvas-zoom-status";
  zoomStatus.setAttribute("aria-live", "polite");

  const navigator = document_.createElement("aside");
  navigator.className = "forma-canvas-navigator";
  navigator.dataset.formaCanvasNavigator = "";
  navigator.setAttribute("aria-label", options.navigatorLabel ?? "Canvas navigator");

  const navigatorItems = document_.createElement("div");
  navigatorItems.className = "forma-canvas-navigator-items";
  const navigatorViewport = document_.createElement("div");
  navigatorViewport.className = "forma-canvas-navigator-viewport";
  navigatorViewport.setAttribute("aria-hidden", "true");
  navigator.append(navigatorItems, navigatorViewport);

  viewport.append(grid, stage, zoomStatus, navigator);
  root.append(viewport);
  container.append(root);

  const setNavigatorVisible = (visible: boolean): void => {
    navigator.hidden = !visible;
  };
  setNavigatorVisible(options.navigator !== false);

  return {
    root,
    grid,
    elements: { viewport, stage, zoomStatus, navigator, navigatorItems, navigatorViewport },
    setNavigatorVisible,
    dispose: () => root.remove(),
  };
}
