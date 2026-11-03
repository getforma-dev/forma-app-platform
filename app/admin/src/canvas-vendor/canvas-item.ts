import { createWidgetChrome } from "./widget-chrome";
import type { CanvasRuntimeHost } from "./runtime-adapter";

export interface CanvasItemOptions {
  instanceId: string;
  displayName: string;
  preferredWidth: number;
  minHeight: number;
  resizable?: boolean;
  runtimeHost?: CanvasRuntimeHost;
  content?: Node;
  chrome?: boolean;
  document?: Document;
}

export function createCanvasItem(options: CanvasItemOptions): HTMLElement {
  if (!/^[A-Za-z0-9_-]{1,96}$/.test(options.instanceId)) {
    throw new Error("canvas instance id must be a safe non-empty identifier");
  }
  if (!Number.isFinite(options.preferredWidth) || options.preferredWidth <= 0) {
    throw new Error("preferredWidth must be positive");
  }
  if (!Number.isFinite(options.minHeight) || options.minHeight <= 0) {
    throw new Error("minHeight must be positive");
  }

  const document_ = options.runtimeHost?.ownerDocument ??
    options.content?.ownerDocument ??
    options.document;
  if (!document_) {
    throw new Error("document is required when no runtime host or content node is supplied");
  }
  const item = document_.createElement("article");
  item.className = "widget-instance";
  item.dataset.instanceId = options.instanceId;
  item.dataset.widgetName = options.displayName;
  item.dataset.canvasPreferredWidth = String(options.preferredWidth);
  item.dataset.canvasMinHeight = String(options.minHeight);
  item.dataset.canvasResizable = String(options.resizable === true);
  item.style.setProperty("--widget-preferred-width", `${options.preferredWidth}px`);
  item.style.setProperty("--widget-min-height", `${options.minHeight}px`);
  item.setAttribute("role", "listitem");
  item.setAttribute("aria-label", options.displayName);

  if (options.chrome !== false) {
    item.append(createWidgetChrome({ displayName: options.displayName, document: document_ }));
  }
  if (options.runtimeHost) item.append(options.runtimeHost);
  else if (options.content) item.append(options.content);
  return item;
}
