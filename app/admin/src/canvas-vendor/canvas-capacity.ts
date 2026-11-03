export interface CanvasCapacityPolicy {
  max_active_runtimes: number;
  density_warning_widgets: number;
  max_widgets: number;
  max_import_widgets: number;
  interactive_create_burst: number;
  interactive_create_window_seconds: number;
}

export const FALLBACK_CANVAS_CAPACITY: Readonly<CanvasCapacityPolicy> = Object.freeze({
  max_active_runtimes: 50,
  density_warning_widgets: 100,
  max_widgets: 500,
  max_import_widgets: 50,
  interactive_create_burst: 20,
  interactive_create_window_seconds: 5,
});

export interface CanvasCapacityDecision {
  allowed: boolean;
  current: number;
  requested: number;
  limit: number;
}

export function validateCanvasCapacityPolicy(value: unknown): CanvasCapacityPolicy {
  if (typeof value !== "object" || value === null) {
    throw new Error("canvas capacity policy is missing");
  }
  const candidate = value as Partial<Record<keyof CanvasCapacityPolicy, unknown>>;
  const readPositiveInteger = (key: keyof CanvasCapacityPolicy): number => {
    const requested = candidate[key];
    if (!Number.isSafeInteger(requested) || Number(requested) <= 0) {
      throw new Error(`canvas capacity policy has an invalid ${key}`);
    }
    return Number(requested);
  };
  const policy: CanvasCapacityPolicy = {
    max_active_runtimes: readPositiveInteger("max_active_runtimes"),
    density_warning_widgets: readPositiveInteger("density_warning_widgets"),
    max_widgets: readPositiveInteger("max_widgets"),
    max_import_widgets: readPositiveInteger("max_import_widgets"),
    interactive_create_burst: readPositiveInteger("interactive_create_burst"),
    interactive_create_window_seconds: readPositiveInteger(
      "interactive_create_window_seconds",
    ),
  };
  if (
    policy.max_active_runtimes >= policy.density_warning_widgets ||
    policy.density_warning_widgets >= policy.max_widgets ||
    policy.max_import_widgets > policy.max_widgets
  ) {
    throw new Error("canvas capacity policy thresholds are inconsistent");
  }
  return policy;
}

export function canvasCapacityDecision(
  current: number,
  requested: number,
  policy: CanvasCapacityPolicy,
): CanvasCapacityDecision {
  const normalizedCurrent = Math.max(0, Math.floor(current));
  const normalizedRequested = Math.max(0, Math.floor(requested));
  return {
    allowed: normalizedCurrent + normalizedRequested <= policy.max_widgets,
    current: normalizedCurrent,
    requested: normalizedRequested,
    limit: policy.max_widgets,
  };
}

export interface CreationBurstDecision {
  allowed: boolean;
  retry_after_seconds: number;
}

/**
 * Browser-side accidental-loop protection. A consuming application may apply
 * additional authoritative limits at its own persistence or command boundary.
 */
export class InteractiveCreationBudget {
  readonly #timestamps: number[] = [];

  admit(
    requested: number,
    policy: CanvasCapacityPolicy,
    now = Date.now(),
  ): CreationBurstDecision {
    const windowMs = policy.interactive_create_window_seconds * 1_000;
    while (this.#timestamps[0] !== undefined && this.#timestamps[0] <= now - windowMs) {
      this.#timestamps.shift();
    }
    const count = Math.max(0, Math.floor(requested));
    if (this.#timestamps.length + count > policy.interactive_create_burst) {
      const oldest = this.#timestamps[0] ?? now;
      return {
        allowed: false,
        retry_after_seconds: Math.max(1, Math.ceil((oldest + windowMs - now) / 1_000)),
      };
    }
    for (let index = 0; index < count; index += 1) this.#timestamps.push(now);
    return { allowed: true, retry_after_seconds: 0 };
  }
}
