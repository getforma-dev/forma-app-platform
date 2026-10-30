// The action path on the canvas (ADR-011 scenarios, APP-093/APP-095). The
// intent bar sends the member's words to `POST /v1/act` first: the server
// parses them (the model, or deterministically for a rehearsed utterance in
// the demo organization), resolves the app, the approver and the target
// record under the member's own authority, validates a typed command draft
// with storage's own rules and answers a confirmation card. Nothing is
// written until the member confirms; Confirm posts the draft, with its
// server-minted idempotency key, through the existing audited command route
// (`commands.ts`), then shares the work with the resolved approver through
// the existing share route. An `open` answer focuses or places the app;
// `place` falls back to the composer (`compose.ts`). The utterance travels
// only in a JSON body, never in a URL; the card shows typed values the
// server composed, never the model's text.
import { placeWork, refreshGroupRecords } from './board';
import { submitIntent } from './compose';
import { label } from './display';
import { el, isUuid } from './render';

export const ACT_URL = '/v1/act';

export type DraftValue = { field_id: string; label: string; display: string; value: unknown };
export type Draft = {
  action: 'create' | 'decide';
  app_id: string;
  work_id: string;
  title: string;
  placed: boolean;
  command_id: string;
  record_id: string;
  collection_id: string;
  schema_id: string;
  values: DraftValue[];
  command_request: Record<string, unknown>;
  approver: { principal_id: string; label: string } | null;
  target: { record_id: string; version: number; summary: string } | null;
  confirm_token: string;
  parse_ms: number;
};
export type ActAnswer =
  | { action: 'draft'; draft: Draft }
  | { action: 'open'; app_id: string; work_id: string; title: string; placed: boolean }
  | { action: 'place' };

export type Posted =
  | { kind: 'committed'; recordVersion: string; workVersion: string; commit: string; audit: string }
  | { kind: 'conflict' | 'stale' | 'refused' | 'unconfirmed' | 'busy' | 'failed'; message: string };

const MESSAGES: Record<string, string> = {
  not_connected: 'The action path is not connected on this server yet.',
  invalid_utterance: 'Say it in a sentence between 3 and 500 characters.',
  could_not_parse: 'That could not be understood as an action. Try "I spent $86 on a client dinner at Nobu".',
  could_not_draft: 'That could not be turned into a record: a value is not one the app accepts, or there is nothing to decide. Nothing was changed.',
  composer_unavailable: 'The model is not configured on this server. Nothing was changed.',
  too_many_requests: 'Too many requests in a short time. Try again in a moment.',
  failed: 'The action could not be prepared right now. Nothing was changed.',
};

function answerOf(body: unknown): ActAnswer | null {
  if (typeof body !== 'object' || body === null) return null;
  const value = body as Record<string, unknown>;
  if (value.action === 'place') return { action: 'place' };
  if (value.action === 'open') {
    return isUuid(value.app_id) && isUuid(value.work_id)
      ? { action: 'open', app_id: value.app_id, work_id: value.work_id, title: String(value.title ?? ''), placed: value.placed === true }
      : null;
  }
  if (value.action !== 'create' && value.action !== 'decide') return null;
  const draft = value as unknown as Draft;
  if (!isUuid(draft.app_id) || !isUuid(draft.work_id) || !isUuid(draft.record_id) || !Array.isArray(draft.values)) return null;
  if (typeof draft.command_request !== 'object' || draft.command_request === null) return null;
  if (typeof draft.command_request.idempotency_key !== 'string') return null;
  return { action: 'draft', draft };
}

async function post(body: Record<string, unknown>): Promise<ActAnswer> {
  let response: Response;
  try {
    response = await fetch(ACT_URL, {
      method: 'POST',
      credentials: 'same-origin',
      cache: 'no-store',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify(body),
    });
  } catch {
    throw new Error(MESSAGES.failed);
  }
  const json: unknown = await response.json().catch(() => null);
  if (response.status === 404) throw new Error(MESSAGES.not_connected);
  if (!response.ok) {
    const code = typeof json === 'object' && json !== null ? String((json as { code?: unknown }).code ?? '') : '';
    throw new Error(MESSAGES[code] ?? MESSAGES.failed);
  }
  const answer = answerOf(json);
  if (!answer) throw new Error(MESSAGES.failed);
  return answer;
}

/** The utterance as an action (the server parses and drafts). */
export function requestAction(utterance: string): Promise<ActAnswer> {
  return post({ utterance });
}

/** A structured decision (no model): the server drafts the `decide` command. */
export async function requestDecision(app: string, work: string, record: string, status: string): Promise<Draft> {
  const answer = await post({ decide: { app_id: app, work_id: work, record_id: record, status } });
  if (answer.action !== 'draft') throw new Error(MESSAGES.could_not_draft);
  return answer.draft;
}

function committed(body: unknown): Posted | null {
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
 * Confirm: the draft, exactly as the server minted it, through the audited
 * command route with its idempotency key (`commands.ts`'s protocol; a retry
 * reuses the key and can only return the stored outcome).
 */
export async function postDraft(draft: Draft): Promise<Posted> {
  const key = String(draft.command_request.idempotency_key);
  const path = `/v1/work/${draft.app_id}/${draft.work_id}/commands/${draft.command_id}`;
  for (let attempt = 0; attempt < 4; attempt += 1) {
    let response: Response;
    try {
      response = await fetch(path, {
        method: 'POST',
        credentials: 'same-origin',
        cache: 'no-store',
        headers: { 'content-type': 'application/json', 'idempotency-key': key },
        body: JSON.stringify(draft.command_request),
      });
    } catch {
      return lookUp(path, key);
    }
    const body: unknown = await response.json().catch(() => null);
    const code = typeof body === 'object' && body !== null ? String((body as { code?: unknown }).code ?? '') : '';
    if (response.ok) return committed(body) ?? { kind: 'unconfirmed', message: 'The result of the command could not be confirmed.' };
    if (response.status === 409 && (code === 'in_flight' || code === 'busy')) {
      await sleep(Math.min(10, Math.max(1, Number(response.headers.get('retry-after')) || 2)));
      continue;
    }
    if (response.status === 409) return { kind: 'conflict', message: 'The record changed before your command. Nothing was changed by this submission.' };
    if (response.status === 412) return { kind: 'stale', message: 'The saved work moved to a newer revision. Try again.' };
    if (response.status === 503 && code === 'unknown_outcome') return lookUp(path, key);
    if (response.status === 400 || response.status === 404) return { kind: 'refused', message: 'The command was refused: a value is not one this command accepts. Nothing was changed.' };
    return { kind: 'failed', message: 'The command could not be committed right now. Nothing was changed.' };
  }
  return { kind: 'busy', message: 'The record was busy. Try again in a moment.' };
}

async function lookUp(path: string, key: string): Promise<Posted> {
  try {
    const response = await fetch(`${path}/outcomes/${key}`, { credentials: 'same-origin', cache: 'no-store' });
    if (response.ok) {
      const outcome = committed(await response.json().catch(() => null));
      if (outcome) return outcome;
    }
  } catch {
    // unconfirmed
  }
  return { kind: 'unconfirmed', message: 'The result of your command could not be confirmed. Confirming again is safe: it can only return the stored outcome.' };
}

/** The share step: the existing share route, record_write so the approver can decide. */
async function shareWith(draft: Draft): Promise<string> {
  if (!draft.approver) return '';
  try {
    const response = await fetch(`/v1/work/${draft.app_id}/${draft.work_id}/share`, {
      method: 'POST',
      credentials: 'same-origin',
      cache: 'no-store',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ grantee_principal_id: draft.approver.principal_id, record_write: true }),
    });
    if (response.ok) return `Shared with ${draft.approver.label} for approval.`;
    return `The share with ${draft.approver.label} was refused; the record is filed.`;
  } catch {
    return `The share with ${draft.approver.label} could not be sent; the record is filed.`;
  }
}

function host(document_: Document): HTMLElement {
  let node = document_.getElementById('cv-confirm');
  if (!node) {
    node = el(document_, 'div', { id: 'cv-confirm', class: 'cv-confirm-host' });
    (document_.querySelector('.cv-dock') ?? document_.body).prepend(node);
  }
  return node;
}

/** Shows the confirmation card; resolves with the member's choice. */
function confirmCard(document_: Document, draft: Draft, utterance: string): Promise<'confirm' | 'edit' | 'cancel'> {
  return new Promise((resolve) => {
    const container = host(document_);
    const list = el(document_, 'dl', { class: 'cv-confirm-values' });
    for (const value of draft.values) {
      const numeric = typeof value.value === 'object' && value.value !== null && (value.value as { type?: string }).type === 'money';
      list.append(el(document_, 'div', {}, el(document_, 'dt', {}, value.label), el(document_, 'dd', { class: numeric ? 'value num' : 'value' }, value.display)));
    }
    const chips = el(document_, 'p', { class: 'cv-confirm-chips' });
    chips.append(el(document_, 'span', { class: 'cv-origin', 'data-origin': 'library' }, draft.title));
    if (draft.placed) chips.append(el(document_, 'span', { class: 'cv-origin', 'data-origin': 'seeded' }, 'placed on your canvas'));
    if (draft.action === 'create') {
      chips.append(draft.approver
        ? el(document_, 'span', { class: 'cv-approver-chip', title: draft.approver.principal_id }, `Approver: ${draft.approver.label}`)
        : el(document_, 'span', { class: 'cv-approver-chip is-none' }, 'No approver named'));
    } else if (draft.target) {
      chips.append(el(document_, 'span', { class: 'cv-approver-chip', title: draft.target.record_id }, `Record: ${draft.target.summary} (v${draft.target.version})`));
    }
    const heading = draft.action === 'create' ? `File this ${label(draft.schema_id).toLowerCase()}?` : `${label(draft.command_id)} this record?`;
    const confirm = el(document_, 'button', { type: 'button', class: 'cv-confirm-yes' }, draft.action === 'create' ? 'Confirm and file' : 'Confirm');
    const edit = el(document_, 'button', { type: 'button', class: 'cv-secondary' }, 'Edit');
    const cancel = el(document_, 'button', { type: 'button', class: 'cv-secondary' }, 'Cancel');
    const card = el(document_, 'section', { class: 'cv-card cv-confirm', role: 'dialog', 'aria-modal': 'false', 'aria-labelledby': 'cv-confirm-title' },
      el(document_, 'header', { class: 'cv-card-head' },
        el(document_, 'h3', { id: 'cv-confirm-title' }, heading),
        el(document_, 'span', { class: 'badge' }, draft.action === 'create' ? 'draft' : 'decision')),
      el(document_, 'p', { class: 'cv-confirm-quote' }, `“${utterance}”`),
      list,
      chips,
      el(document_, 'p', { class: 'cv-confirm-note' }, 'Nothing is written until you confirm. The command is committed through the audited path with a server-minted key.'),
      el(document_, 'div', { class: 'cv-confirm-actions' }, confirm, edit, cancel));
    const done = (choice: 'confirm' | 'edit' | 'cancel') => {
      card.remove();
      resolve(choice);
    };
    confirm.addEventListener('click', () => done('confirm'));
    edit.addEventListener('click', () => done('edit'));
    cancel.addEventListener('click', () => done('cancel'));
    card.addEventListener('keydown', (event) => {
      if ((event as KeyboardEvent).key === 'Escape') done('cancel');
    });
    container.replaceChildren(card);
    confirm.focus();
  });
}

/**
 * The intent bar's submit: the action path first, the composer when the
 * server says `place`; `report` updates the bar's status line.
 */
export async function submitUtterance(text: string, report: (message: string) => void): Promise<void> {
  report('Understanding…');
  const started = performance.now();
  let answer: ActAnswer;
  try {
    answer = await requestAction(text);
  } catch (error) {
    const message = error instanceof Error ? error.message : MESSAGES.failed;
    if (message === MESSAGES.not_connected) {
      await submitIntent(text, report);
      return;
    }
    report(message);
    return;
  }
  const parse = Math.round(performance.now() - started);
  document.getElementById('cv-canvas')?.setAttribute('data-last-act-ms', String(parse));
  if (answer.action === 'place') {
    await submitIntent(text, report);
    return;
  }
  if (answer.action === 'open') {
    report(answer.placed ? 'Placing it on your canvas…' : 'Bringing it up…');
    report(await placeWork(answer.app_id, answer.work_id));
    void refreshGroupRecords(answer.work_id, true);
    return;
  }
  const draft = answer.draft;
  report(draft.action === 'create' ? `Understood in ${parse} ms. Review the record, then confirm.` : `Understood in ${parse} ms. Review the decision, then confirm.`);
  const choice = await confirmCard(document, draft, text);
  if (choice === 'edit') {
    const input = document.getElementById('intent-input') as HTMLInputElement | null;
    if (input) {
      input.value = text;
      input.focus();
      input.select();
    }
    report('Edit your sentence and send it again.');
    return;
  }
  if (choice === 'cancel') {
    report('Cancelled. Nothing was changed.');
    return;
  }
  report('Committing through the audited command path…');
  const result = await postDraft(draft);
  if (result.kind !== 'committed') {
    report(result.message);
    return;
  }
  const shared = await shareWith(draft);
  const placed = await placeWork(draft.app_id, draft.work_id);
  void refreshGroupRecords(draft.work_id, true);
  const verb = draft.action === 'create' ? 'Filed' : label(draft.command_id);
  report(`${verb}: record v${result.recordVersion}, commit ${result.commit}, audit ${result.audit.slice(0, 8)}. ${shared} ${placed === 'That app is already on your canvas.' ? '' : placed}`.trim());
}
