import type { WidgetVirtualizationPolicy } from "./contracts";

export interface CanvasRuntimeHost<Restart = unknown> extends HTMLElement {
  setViewportActive(active: boolean): void;
  focusFirstInteractive(): boolean;
  virtualizationPolicy(): WidgetVirtualizationPolicy;
  captureRestartDescriptor(): Restart | null;
  clear(): void;
}

export interface CanvasRuntimeAdapter<Restart = unknown> {
  findHost(item: HTMLElement): CanvasRuntimeHost<Restart> | null;
  ownsEventTarget(target: EventTarget): boolean;
  restoreHost(descriptor: Restart, document: Document): CanvasRuntimeHost<Restart>;
}

export const NO_RUNTIME_ADAPTER: CanvasRuntimeAdapter<never> = Object.freeze({
  findHost: () => null,
  ownsEventTarget: () => false,
  restoreHost: () => {
    throw new Error("this canvas has no runtime-host adapter");
  },
});
