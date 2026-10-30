export const WIDGET_CHROME_LAYOUTS = ["adaptive-edge-rail"] as const;

export type WidgetChromeLayout = (typeof WIDGET_CHROME_LAYOUTS)[number];

export const DEFAULT_WIDGET_CHROME_LAYOUT: WidgetChromeLayout = "adaptive-edge-rail";

export interface WidgetChromeOptions {
  displayName: string;
  document: Document;
}

export function createWidgetChrome(options: WidgetChromeOptions): HTMLElement {
  const chrome = options.document.createElement("header");
  chrome.className = "widget-chrome";

  const commandRegion = options.document.createElement("div");
  commandRegion.className = "widget-command-region";
  commandRegion.dataset.widgetCommandRegion = "";

  const dragHandle = options.document.createElement("button");
  dragHandle.type = "button";
  dragHandle.className = "widget-drag-handle";
  dragHandle.setAttribute("aria-label", `Move ${options.displayName}`);
  dragHandle.setAttribute(
    "aria-keyshortcuts",
    "ArrowLeft ArrowRight ArrowUp ArrowDown Meta+ArrowLeft Meta+ArrowRight Meta+ArrowUp Meta+ArrowDown Control+ArrowLeft Control+ArrowRight Control+ArrowUp Control+ArrowDown",
  );
  dragHandle.setAttribute(
    "aria-description",
    "Drag to move. Arrow keys nudge. Control or Command plus an arrow selects a nearby widget. Enter opens the selected widget controls. Escape returns to the widget shell.",
  );
  dragHandle.title =
    "Drag to move · Arrow keys nudge · Ctrl/Cmd + Arrow selects nearby · Enter opens controls";
  const dragRail = options.document.createElement("span");
  dragRail.className = "widget-drag-rail";
  dragRail.dataset.widgetCommandRail = "";
  dragRail.setAttribute("aria-hidden", "true");
  dragHandle.append(dragRail);
  commandRegion.append(dragHandle);
  chrome.append(commandRegion);
  return chrome;
}
