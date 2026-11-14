// The record form on the canvas (APP-013, D7.16): one explicit command
// through the existing audited route, `POST /v1/work/{app}/{work}/commands/
// {command}` as the contract's `command_request` JSON with the form's
// server-minted key in `Idempotency-Key`. A retry of the same submission
// reuses that key, so it can only return the stored outcome; storage judges
// every value against the published schema. Without JavaScript the same form
// posts natively and the record page shows the outcome.

export type CommandOutcome =
  | { kind: 'committed'; recordVersion: string; workVersion: string; commit: string; audit: string }
  | { kind: 'conflict' | 'stale' | 'refused' | 'unconfirmed' | 'busy' | 'failed'; message: string };

/** Exact human decimal -> canonical minor units; no floating point. */
export function moneyMinorUnits(raw: string, exponent: unknown): string | null {
  if (typeof exponent !== 'number' || !Number.isInteger(exponent) || exponent < 0 || exponent > 6 || raw.length > 64) return null;
  const text = raw.trim(); const negative = text.startsWith('-'); const number = negative ? text.slice(1) : text;
  const pieces = number.split('.'); if (pieces.length > 2) return null;
  const [whole, fraction = ''] = pieces;
  if ((!whole && !fraction) || !/^[0-9]*$/.test(whole) || !/^[0-9]*$/.test(fraction) || fraction.length > exponent || (exponent === 0 && pieces.length > 1)) return null;
  const digits = `${whole}${fraction}${'0'.repeat(exponent - fraction.length)}`.replace(/^0+/, '') || '0';
  if (digits.length > 38) return null;
  return `${negative && digits !== '0' ? '-' : ''}${digits}`;
}

/** HTML form text only; mirrors `commands::routes::form_typed_value`, not normalized action values. */
export function typedFormValue(spec: Record<string, unknown>, raw: string): Record<string, unknown> | null {
  if (raw === '') return { type: 'null' };
  const kind = spec.type;
  switch (kind) {
    case 'text':
    case 'enum':
    case 'date':
    case 'timestamp':
      return { type: kind, value: raw };
    case 'boolean':
      return { type: 'boolean', value: raw === 'true' || raw === 'on' || raw === '1' };
    case 'integer':
      return spec.unit === undefined ? null : { type: 'integer', value: raw, unit: spec.unit };
    case 'decimal':
      return spec.scale === undefined || spec.unit === undefined
        ? null
        : { type: 'decimal', coefficient: raw, scale: spec.scale, unit: spec.unit };
    case 'money': {
      const minor = moneyMinorUnits(raw, spec.minor_unit_exponent);
      return typeof spec.currency !== 'string' || minor === null ? null
        : { type: 'money', minor_units: minor, currency: spec.currency, minor_unit_exponent: spec.minor_unit_exponent };
    }
    case 'reference':
      return spec.target_input === undefined ? null : { type: 'reference', input_id: spec.target_input, record_id: raw };
    default:
      return null;
  }
}

function field(form: HTMLFormElement, name: string): string {
  const node = form.elements.namedItem(name);
  return node instanceof HTMLInputElement || node instanceof HTMLSelectElement ? node.value : '';
}

/**
 * ADR-020 §9 (APP-208): a multi-select's typed value from the boxes the member
 * checked. The client sends only the spec's declared values, in the spec's
 * (declaration) order, each once; nothing checked is the null value. The spec
 * is the server's echo, and storage judges the value against the published
 * schema, so this orders and filters, it never authorizes.
 */
export function multiEnumValue(spec: Record<string, unknown>, checked: readonly string[]): Record<string, unknown> | null {
  const declared = spec.values;
  if (!Array.isArray(declared) || !declared.every((value) => typeof value === 'string')) return null;
  const picked = new Set(checked);
  const values = (declared as string[]).filter((value) => picked.has(value));
  return values.length === 0 ? { type: 'null' } : { type: 'multi_enum', values };
}

/** The `command_request` for the form, or null if a value cannot be typed. */
export function commandRequest(form: HTMLFormElement, app: string, work: string): Record<string, unknown> | null {
  const values: Record<string, unknown>[] = [];
  /** ADR-020 §9: the checked boxes of each multi-select, composed once per field. */
  const groups = new Map<string, string[]>();
  for (const element of Array.from(form.elements)) {
    if (!(element instanceof HTMLInputElement || element instanceof HTMLSelectElement)) continue;
    if (!element.name.startsWith('value.')) continue;
    // As a native submission: a disabled control (APP-205's empty reference
    // picker, "Nothing to choose yet") is never posted.
    if (element.disabled) continue;
    const id = element.name.slice('value.'.length);
    let spec: Record<string, unknown>;
    try {
      spec = JSON.parse(field(form, `spec.${id}`)) as Record<string, unknown>;
    } catch {
      return null;
    }
    if (spec.type === 'multi_enum') {
      let checked = groups.get(id);
      if (!checked) {
        checked = [];
        groups.set(id, checked);
        values.push({ field_id: id, value: null });
      }
      if (element instanceof HTMLInputElement && element.type === 'checkbox' && element.checked) checked.push(element.value);
      continue;
    }
    const value = typedFormValue(spec, element.value);
    if (!value) return null;
    values.push({ field_id: id, value });
  }
  for (const entry of values) {
    const checked = groups.get(entry.field_id as string);
    if (!checked) continue;
    let spec: Record<string, unknown>;
    try {
      spec = JSON.parse(field(form, `spec.${entry.field_id as string}`)) as Record<string, unknown>;
    } catch {
      return null;
    }
    const value = multiEnumValue(spec, checked);
    if (!value) return null;
    entry.value = value;
  }
  const integer = (name: string) => Number.parseInt(field(form, name), 10);
  return {
    kind: 'command_request',
    schema_version: 1,
    app_id: app,
    work_id: work,
    publication_id: field(form, 'publication_id'),
    command_id: form.dataset.command ?? '',
    schema_id: field(form, 'schema_id'),
    schema_version_expected: integer('schema_version_expected'),
    record_id: field(form, 'record_id'),
    expected_record_version: integer('expected_record_version'),
    expected_work_version: integer('expected_work_version'),
    values,
    idempotency_key: field(form, 'idempotency_key'),
  };
}

function committed(body: unknown): CommandOutcome | null {
  if (typeof body !== 'object' || body === null) return null;
  const outcome = body as Record<string, unknown>;
  if (outcome.kind !== 'command_outcome' || outcome.state !== 'committed') return null;
  return {
    kind: 'committed',
    recordVersion: String(outcome.record_version),
    workVersion: String(outcome.work_version),
    commit: String(outcome.commit_sequence),
    audit: String(outcome.audit_event_id),
  };
}

const sleep = (seconds: number) => new Promise((resolve) => setTimeout(resolve, seconds * 1000));

/**
 * ADR-020: storage's typed refusals (422, the code alone), worded as
 * `commands::refusal_copy` words them for MCP and Slack. Nothing was written.
 */
export const REFUSAL_COPY: Readonly<Record<string, string>> = {
  requirement_not_met: 'Not saved: the item this refers to is no longer open.',
  unchanged: 'Already locked in: your entry already holds these values.',
};

/** The outcome copy a caller may tailor (the form, a confirmed draft, a note). */
export type OutcomeCopy = Partial<Record<'conflict' | 'stale' | 'refused' | 'busy' | 'unconfirmed', string>>;
const COPY: Required<OutcomeCopy> = {
  conflict: 'The record changed before your command. Nothing was changed by this submission. Start a new record and submit again.',
  stale: 'The saved work moved to a newer revision. Start a new record under it.',
  refused: 'The command was refused: a value is not one this command accepts. Nothing was changed.',
  busy: 'The record was busy. Submit again in a moment.',
  unconfirmed: 'The result of your command could not be confirmed. Submitting again with the same form is safe: it can only return the stored outcome.',
};

/**
 * One `command_request` through the audited route (`path`, same origin) with
 * its idempotency key: `in_flight`/`busy` wait the server's `Retry-After`
 * (1–10 s) and retry with the same key; a lost answer reads the stored
 * outcome by key (ADR-002). The form, the confirmed draft and the notes all
 * post through here.
 */
export async function postCommand(path: string, body: unknown, key: string, copy: OutcomeCopy = {}): Promise<CommandOutcome> {
  const text = { ...COPY, ...copy };
  const serialized = JSON.stringify(body);
  for (let attempt = 0; attempt < 4; attempt += 1) {
    let response: Response;
    try {
      response = await fetch(path, {
        method: 'POST',
        credentials: 'same-origin',
        cache: 'no-store',
        headers: { 'content-type': 'application/json', 'idempotency-key': key },
        body: serialized,
      });
    } catch {
      return lookUp(path, key, text.unconfirmed);
    }
    const json: unknown = await response.json().catch(() => null);
    const code = typeof json === 'object' && json !== null ? String((json as { code?: unknown }).code ?? '') : '';
    if (response.ok) return committed(json) ?? { kind: 'unconfirmed', message: 'The result of the command could not be confirmed.' };
    if (response.status === 409 && (code === 'in_flight' || code === 'busy')) {
      await sleep(Math.min(10, Math.max(1, Number(response.headers.get('retry-after')) || 2)));
      continue;
    }
    if (response.status === 409) return { kind: 'conflict', message: text.conflict };
    if (response.status === 412) return { kind: 'stale', message: text.stale };
    if (response.status === 422 && code === 'app_data_limit') return {kind: 'refused', message: 'This change would exceed the app data limit of 100,000 rows or 64 MiB. Nothing was saved. Original files are unchanged.'};
    if (response.status === 422 && Object.hasOwn(REFUSAL_COPY, code)) return { kind: 'refused', message: REFUSAL_COPY[code] };
    // A gateway/server failure may follow a committed transaction whose
    // response was lost. Recover by the same key; never claim rollback.
    if (response.status >= 500) return lookUp(path, key, text.unconfirmed);
    if (response.status === 400 || response.status === 404) return { kind: 'refused', message: text.refused };
    return { kind: 'failed', message: 'The command could not be committed right now. Nothing was changed.' };
  }
  return { kind: 'busy', message: text.busy };
}

/** Submits the form once (retrying `in_flight`/`busy` with the same key). */
export async function submitCommand(form: HTMLFormElement, app: string, work: string): Promise<CommandOutcome> {
  const request = commandRequest(form, app, work);
  if (!request || !form.action) {
    return { kind: 'refused', message: COPY.refused };
  }
  const url = new URL(form.action, location.href);
  if (url.origin !== location.origin || !url.pathname.startsWith('/v1/work/')) {
    return { kind: 'failed', message: 'The command was not sent.' };
  }
  return postCommand(url.pathname, request, String(request.idempotency_key));
}

/** ADR-002: after a lost answer, read the outcome by key under current authority. */
async function lookUp(path: string, key: string, unconfirmed: string): Promise<CommandOutcome> {
  try {
    const response = await fetch(`${path}/outcomes/${key}`, { credentials: 'same-origin', cache: 'no-store' });
    if (response.ok) {
      const outcome = committed(await response.json().catch(() => null));
      if (outcome) return outcome;
    }
  } catch {
    // fall through: unconfirmed
  }
  return { kind: 'unconfirmed', message: unconfirmed };
}
