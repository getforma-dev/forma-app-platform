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
import { focusCard, placeWork, refreshGroupRecords, workCard } from './board';
import { showTiming, type Step } from './timing';
import { routeSentence } from './sentences';
import { postCommand, type CommandOutcome } from './commands';
import { clearIntentBar, submitIntent } from './compose';
import { runSlash } from './slash';
import { label } from './display';
import { isDismissKey, showDockCard } from './dock-card';
import { ledgerCardFor, setLedgerRange } from './ledger';
import { RECEIPT_PROMPT, restoreReceipt, takeReceipt, type PendingReceipt } from './receipt';
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
  /** APP-102: the draft was read from an attached receipt. */
  receipt?: boolean;
  /** APP-107: every approver named (a two-level workflow names two); `approver` is the first. */
  approvers?: { principal_id: string; label: string }[];
  /** APP-107: the declared rule that chose the status, display only. */
  rule?: string | null;
  /** APP-107: server-computed display flags (the PO match). */
  flags?: { kind: string; state: string; label: string }[];
};
export type LedgerRange = { from: string; to: string };
export type ActAnswer =
  | { action: 'draft'; draft: Draft }
  | { action: 'open'; app_id: string; work_id: string; title: string; placed: boolean; view: 'ledger' | 'records'; range: LedgerRange | null }
  | { action: 'place' };

export type Posted = CommandOutcome;

const MESSAGES: Record<string, string> = {
  not_connected: 'The action path is not connected on this server yet.',
  invalid_utterance: 'Say it in a sentence between 3 and 500 characters.',
  could_not_parse: 'That could not be understood as an action. Try "I spent $86 on a client dinner at Nobu".',
  could_not_draft: 'That could not be turned into a record: a value is not one the app accepts, or there is nothing to decide. Nothing was changed.',
  composer_unavailable: 'The model is not configured on this server. Nothing was changed.',
  too_many_requests: 'Too many requests in a short time. Try again in a moment.',
  failed: 'The action could not be prepared right now. Nothing was changed.',
  invalid_receipt: 'That file can’t be read as a receipt. Attach a JPEG, PNG or WebP photo under 5 MB. Nothing was sent to the model.',
  payload_too_large: 'That photo is larger than 5 MB. Nothing was changed.',
};

function answerOf(body: unknown): ActAnswer | null {
  if (typeof body !== 'object' || body === null) return null;
  const value = body as Record<string, unknown>;
  if (value.action === 'place') return { action: 'place' };
  // APP-108: an app built from the selected folder (`kind: build_from_box`),
  // placed like an opened work.
  if (value.via === 'folder' && isUuid(value.app_id) && isUuid(value.work_id) && value.action !== 'create' && value.action !== 'decide') {
    return { action: 'open', app_id: value.app_id, work_id: value.work_id, title: String(value.title ?? ''), placed: true, view: 'records', range: null };
  }
  if (value.action === 'open') {
    const range = value.range as Record<string, unknown> | null | undefined;
    return isUuid(value.app_id) && isUuid(value.work_id)
      ? {
        action: 'open', app_id: value.app_id, work_id: value.work_id, title: String(value.title ?? ''), placed: value.placed === true,
        view: value.view === 'ledger' ? 'ledger' : 'records',
        range: range && typeof range.from === 'string' && typeof range.to === 'string' ? { from: range.from, to: range.to } : null,
      }
      : null;
  }
  if (value.action !== 'create' && value.action !== 'decide') return null;
  const draft = value as unknown as Draft;
  if (!isUuid(draft.app_id) || !isUuid(draft.work_id) || !isUuid(draft.record_id) || !Array.isArray(draft.values)) return null;
  if (typeof draft.command_request !== 'object' || draft.command_request === null) return null;
  if (typeof draft.command_request.idempotency_key !== 'string') return null;
  return { action: 'draft', draft };
}

async function post(body: Record<string, unknown> | FormData): Promise<ActAnswer> {
  let response: Response;
  try {
    // A receipt goes multipart (the browser sets the boundary); words alone as JSON.
    response = await fetch(ACT_URL, body instanceof FormData
      ? { method: 'POST', credentials: 'same-origin', cache: 'no-store', body }
      : { method: 'POST', credentials: 'same-origin', cache: 'no-store', headers: { 'content-type': 'application/json' }, body: JSON.stringify(body) });
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

/**
 * The utterance as an action (the server parses and drafts); with a receipt,
 * multipart; APP-101: `app` pins a slash command's app (JSON only).
 */
export function requestAction(utterance: string, receipt: PendingReceipt | null = null, app: string | null = null): Promise<ActAnswer> {
  if (!receipt) return post(app ? { utterance, app_id: app } : { utterance });
  const form = new FormData();
  if (utterance && utterance !== RECEIPT_PROMPT) form.append('utterance', utterance);
  form.append('receipt', receipt.blob, receipt.name);
  return post(form);
}

/** A structured decision (no model): the server drafts the `decide` command. */
export async function requestDecision(app: string, work: string, record: string, status: string): Promise<Draft> {
  const answer = await post({ decide: { app_id: app, work_id: work, record_id: record, status } });
  if (answer.action !== 'draft') throw new Error(MESSAGES.could_not_draft);
  return answer.draft;
}

/**
 * Confirm: the draft, exactly as the server minted it, through the audited
 * command route with its idempotency key (`commands.ts` `postCommand`; a
 * retry reuses the key and can only return the stored outcome).
 */
export function postDraft(draft: Draft): Promise<Posted> {
  const key = String(draft.command_request.idempotency_key);
  const path = `/v1/work/${draft.app_id}/${draft.work_id}/commands/${draft.command_id}`;
  return postCommand(path, draft.command_request, key, {
    conflict: 'The record changed before your command. Nothing was changed by this submission.',
    stale: 'The saved work moved to a newer revision. Try again.',
    busy: 'The record was busy. Try again in a moment.',
    unconfirmed: 'The result of your command could not be confirmed. Confirming again is safe: it can only return the stored outcome.',
  });
}

/** The share step: the existing share route, record_write so the approver can decide. */
async function shareWith(draft: Draft): Promise<string> {
  const approvers = draft.approvers?.length ? draft.approvers : draft.approver ? [draft.approver] : [];
  const notes: string[] = [];
  for (const approver of approvers) {
    try {
      const response = await fetch(`/v1/work/${draft.app_id}/${draft.work_id}/share`, {
        method: 'POST',
        credentials: 'same-origin',
        cache: 'no-store',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ grantee_principal_id: approver.principal_id, record_write: true }),
      });
      notes.push(response.ok ? `Shared with ${approver.label} for approval.` : `The share with ${approver.label} was refused; the record is filed.`);
    } catch {
      notes.push(`The share with ${approver.label} could not be sent; the record is filed.`);
    }
  }
  return notes.join(' ');
}

/**
 * Shows the confirmation card; resolves with the member's choice, or
 * `replaced` when another dock card (the chooser) took its place.
 */
function confirmCard(document_: Document, draft: Draft, utterance: string, receipt: PendingReceipt | null = null, understoodMs: number | null = null): Promise<'confirm' | 'edit' | 'cancel' | 'replaced'> {
  return new Promise((resolve) => {
    const list = el(document_, 'dl', { class: 'cv-confirm-values' });
    for (const value of draft.values) {
      const numeric = typeof value.value === 'object' && value.value !== null && (value.value as { type?: string }).type === 'money';
      list.append(el(document_, 'div', {}, el(document_, 'dt', {}, value.label), el(document_, 'dd', { class: numeric ? 'value num' : 'value' }, value.display)));
    }
    const chips = el(document_, 'p', { class: 'cv-confirm-chips' });
    chips.append(el(document_, 'span', { class: 'cv-origin', 'data-origin': 'library' }, draft.title));
    if (draft.placed) chips.append(el(document_, 'span', { class: 'cv-origin', 'data-origin': 'seeded' }, 'placed on your canvas'));
    if (draft.action === 'create') {
      const approvers = draft.approvers?.length ? draft.approvers : draft.approver ? [draft.approver] : [];
      if (!approvers.length) chips.append(el(document_, 'span', { class: 'cv-approver-chip is-none' }, 'No approver named'));
      approvers.forEach((approver, index) => chips.append(el(document_, 'span', { class: 'cv-approver-chip', title: approver.principal_id },
        `${approvers.length > 1 ? `Approver ${index + 1}` : 'Approver'}: ${approver.label}`)));
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
      receipt
        ? el(document_, 'figure', { class: 'cv-confirm-receipt' },
          el(document_, 'img', { src: receipt.thumb, alt: 'The attached receipt' }),
          el(document_, 'figcaption', {}, 'Read from this receipt. The photo is not stored; the record notes “receipt: attached”.'))
        : null,
      list,
      chips,
      draft.rule ? el(document_, 'p', { class: 'cv-confirm-rule' }, draft.rule) : null,
      ...(draft.flags ?? []).map((flag) => el(document_, 'p', { class: 'cv-flag', 'data-state': flag.state }, flag.label)),
      el(document_, 'p', { class: 'cv-confirm-note' }, `${understoodMs !== null ? `Understood in ${understoodMs} ms. ` : ''}Nothing is written until you confirm. The command is committed through the audited path with a server-minted key.`),
      el(document_, 'div', { class: 'cv-confirm-actions' }, confirm, edit, cancel));
    let close = () => undefined as void;
    const done = (choice: 'confirm' | 'edit' | 'cancel') => {
      close();
      resolve(choice);
    };
    confirm.addEventListener('click', () => done('confirm'));
    edit.addEventListener('click', () => done('edit'));
    cancel.addEventListener('click', () => done('cancel'));
    card.addEventListener('keydown', (event) => {
      if (isDismissKey(event)) {
        event.preventDefault();
        done('cancel');
      }
    });
    close = showDockCard(document_, card, () => resolve('replaced'));
    confirm.focus();
  });
}

/**
 * APP-108: the folder lane. With a folder chip active or mentioned in the bar,
 * plain sentences go to that folder (`folder-bar.ts`); the router answers
 * true when it took the sentence.
 */
type UtteranceRouter = (text: string, report: (message: string) => void) => Promise<boolean>;
let utteranceRouter: UtteranceRouter | null = null;
export function setUtteranceRouter(router: UtteranceRouter): void {
  utteranceRouter = router;
}

/**
 * The intent bar's submit: the action path first, the composer when the
 * server says `place`; `report` updates the bar's status line.
 */
export async function submitUtterance(text: string, report: (message: string) => void, app: string | null = null): Promise<void> {
  const receiptAttached = document.getElementById('intent-form')?.getAttribute('data-receipt') === 'attached';
  // APP-101: `/name rest` is the explicit lane (`slash.ts`): no guessing.
  if (!app && text.startsWith('/') && !receiptAttached) {
    await runSlash(text, report);
    return;
  }
  // APP-112: a canvas action said in plain words ("unite the expense app"),
  // before the folder lane (an active folder chip never swallows one).
  if (!app && !receiptAttached && await routeSentence(text, report)) return;
  if (!app && !receiptAttached && utteranceRouter && await utteranceRouter(text, report)) return;
  await submitAction(text, report, app);
}

/**
 * The action path itself (no slash routing): `app` pins a slash command's
 * app; `/expense` alone opens the expense ledger (the server's bare command).
 */
export async function submitAction(text: string, report: (message: string) => void, app: string | null = null): Promise<void> {
  const receipt = app ? null : takeReceipt(document);
  const pin = app ? { app_id: app } : null;
  report(receipt ? 'Reading the receipt…' : 'Understanding…');
  const started = performance.now();
  let answer: ActAnswer;
  try {
    answer = await requestAction(text, receipt, app);
  } catch (error) {
    const message = error instanceof Error ? error.message : MESSAGES.failed;
    if (receipt) {
      // The member may fix the words or the file and send again.
      if (message !== MESSAGES.invalid_receipt && message !== MESSAGES.payload_too_large) restoreReceipt(document, receipt);
      report(message);
      return;
    }
    if (message === MESSAGES.not_connected) {
      await submitIntent(text, report, pin);
      return;
    }
    report(message);
    return;
  }
  const parse = Math.round(performance.now() - started);
  document.getElementById('cv-canvas')?.setAttribute('data-last-act-ms', String(parse));
  const understood: Step = { label: 'understand', ms: parse, server: answer.action === 'draft' && typeof answer.draft.parse_ms === 'number' ? answer.draft.parse_ms : null };
  if (answer.action === 'place') {
    await submitIntent(text, report, pin, [understood]);
    return;
  }
  if (answer.action === 'open') {
    report(answer.placed ? 'Placing it on your canvas…' : 'Bringing it up…');
    const placing = performance.now();
    const placed = await placeWork(answer.app_id, answer.work_id);
    showTiming('Open', [understood, { label: 'on canvas', ms: performance.now() - placing }], workCard(answer.work_id, answer.view));
    clearIntentBar();
    void refreshGroupRecords(answer.work_id, true);
    const ledger = answer.view === 'ledger' ? ledgerCardFor(answer.app_id, answer.work_id) : null;
    if (ledger) {
      setLedgerRange(ledger, answer.range);
      focusCard(ledger);
      report(answer.range ? `Expense ledger, ${answer.range.from} to ${answer.range.to}.` : 'Expense ledger.');
      return;
    }
    report(placed);
    return;
  }
  const draft = answer.draft;
  report(draft.action === 'create' ? `Understood in ${parse} ms. Review the record, then confirm.` : `Understood in ${parse} ms. Review the decision, then confirm.`);
  const asked = confirmCard(document, draft, text, receipt, parse);
  showTiming(draft.action === 'create' ? 'Draft' : 'Decision', [{ ...understood, label: 'draft' }], document.querySelector<HTMLElement>('#cv-confirm .cv-confirm'));
  const choice = await asked;
  if (choice === 'edit') {
    if (receipt) restoreReceipt(document, receipt);
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
  // Another dock card (the chooser) took its place: that card reports now.
  if (choice === 'replaced') return;
  report('Saving through the audited path…');
  const committing = performance.now();
  const result = await postDraft(draft);
  if (result.kind !== 'committed') {
    report(result.message);
    return;
  }
  const saved = performance.now() - committing;
  const sharing = performance.now();
  const shared = await shareWith(draft);
  const shareMs = performance.now() - sharing;
  const placing = performance.now();
  const placed = await placeWork(draft.app_id, draft.work_id);
  const placeMs = performance.now() - placing;
  void refreshGroupRecords(draft.work_id, true);
  showTiming(draft.action === 'create' ? 'Filed' : 'Decided', [
    { ...understood, label: 'draft' },
    { label: 'saved', ms: saved },
    ...(shared ? [{ label: 'shared', ms: shareMs }] : []),
    { label: 'on canvas', ms: placeMs },
  ], workCard(draft.work_id, 'records'));
  clearIntentBar();
  const verb = draft.action === 'create' ? 'Filed' : label(draft.command_id);
  report(`${verb}: record v${result.recordVersion}, commit ${result.commit}, audit ${result.audit.slice(0, 8)}. ${shared} ${placed === 'That app is already on your canvas.' ? '' : placed}`.trim());
}
