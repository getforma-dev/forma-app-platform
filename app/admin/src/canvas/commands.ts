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

/** A form text under the published field's value type (`commands::routes::typed_value`). */
export function typedValue(spec: Record<string, unknown>, raw: string): Record<string, unknown> | null {
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
    case 'money':
      return spec.currency === undefined || spec.minor_unit_exponent === undefined
        ? null
        : { type: 'money', minor_units: raw, currency: spec.currency, minor_unit_exponent: spec.minor_unit_exponent };
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

/** The `command_request` for the form, or null if a value cannot be typed. */
export function commandRequest(form: HTMLFormElement, app: string, work: string): Record<string, unknown> | null {
  const values: Record<string, unknown>[] = [];
  for (const element of Array.from(form.elements)) {
    if (!(element instanceof HTMLInputElement || element instanceof HTMLSelectElement)) continue;
    if (!element.name.startsWith('value.')) continue;
    const id = element.name.slice('value.'.length);
    let spec: Record<string, unknown>;
    try {
      spec = JSON.parse(field(form, `spec.${id}`)) as Record<string, unknown>;
    } catch {
      return null;
    }
    const value = typedValue(spec, element.value);
    if (!value) return null;
    values.push({ field_id: id, value });
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

/** Submits the form once (retrying `in_flight`/`busy` with the same key). */
export async function submitCommand(form: HTMLFormElement, app: string, work: string): Promise<CommandOutcome> {
  const request = commandRequest(form, app, work);
  if (!request || !form.action) {
    return { kind: 'refused', message: 'The command was refused: a value is not one this command accepts. Nothing was changed.' };
  }
  const key = String(request.idempotency_key);
  const url = new URL(form.action, location.href);
  if (url.origin !== location.origin || !url.pathname.startsWith('/v1/work/')) {
    return { kind: 'failed', message: 'The command was not sent.' };
  }
  for (let attempt = 0; attempt < 4; attempt += 1) {
    let response: Response;
    try {
      response = await fetch(url.pathname, {
        method: 'POST',
        credentials: 'same-origin',
        cache: 'no-store',
        headers: { 'content-type': 'application/json', 'idempotency-key': key },
        body: JSON.stringify(request),
      });
    } catch {
      return lookUp(url.pathname, key);
    }
    const body: unknown = await response.json().catch(() => null);
    const code = typeof body === 'object' && body !== null ? String((body as { code?: unknown }).code ?? '') : '';
    if (response.ok) {
      return committed(body) ?? { kind: 'unconfirmed', message: 'The result of the command could not be confirmed.' };
    }
    if (response.status === 409 && (code === 'in_flight' || code === 'busy')) {
      await sleep(Math.min(10, Math.max(1, Number(response.headers.get('retry-after')) || 2)));
      continue;
    }
    if (response.status === 409) {
      return { kind: 'conflict', message: 'The record changed before your command. Nothing was changed by this submission. Start a new record and submit again.' };
    }
    if (response.status === 412) {
      return { kind: 'stale', message: 'The saved work moved to a newer revision. Start a new record under it.' };
    }
    if (response.status === 503 && code === 'unknown_outcome') return lookUp(url.pathname, key);
    if (response.status === 400 || response.status === 404) {
      return { kind: 'refused', message: 'The command was refused: a value is not one this command accepts. Nothing was changed.' };
    }
    return { kind: 'failed', message: 'The command could not be committed right now. Nothing was changed.' };
  }
  return { kind: 'busy', message: 'The record was busy. Submit again in a moment.' };
}

/** ADR-002: after a lost answer, read the outcome by key under current authority. */
async function lookUp(path: string, key: string): Promise<CommandOutcome> {
  try {
    const response = await fetch(`${path}/outcomes/${key}`, { credentials: 'same-origin', cache: 'no-store' });
    if (response.ok) {
      const outcome = committed(await response.json().catch(() => null));
      if (outcome) return outcome;
    }
  } catch {
    // fall through: unconfirmed
  }
  return { kind: 'unconfirmed', message: 'The result of your command could not be confirmed. Submitting again with the same form is safe: it can only return the stored outcome.' };
}
