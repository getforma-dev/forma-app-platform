// One read-only Ask transport for the board, focus and standalone document.
import type { WidgetKind, WidgetState } from '../canvas/widgets/state';
import { DAILY_LIMIT_CODE, DAILY_LIMIT_MESSAGE } from '../canvas/daily-limit';
export type AskWork = { app_id: string; work_id: string; address: string; collection?: string };
export type RecordScope = { kind: 'card' | 'app' | 'canvas'; address: string; works: AskWork[] };
export type WidgetScope = { kind: 'widget'; address: string; works: []; widget_id: string; widget: WidgetKind; state: WidgetState };
export type AskScope = RecordScope | WidgetScope;
export type AskReference = { ref: string; app_id: string; work_id: string; record_id: string; address: string; label: string };
export type AskReply = { text: string; references: AskReference[]; receipts: Record<string, unknown>[] };
export const ASK_URL = '/v1/ask';
const object = (value: unknown): value is Record<string, unknown> => typeof value === 'object' && value !== null && !Array.isArray(value);
const uuid = (value: unknown): value is string => typeof value === 'string' && /^[0-9a-f]{8}-[0-9a-f]{4}-[47][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/.test(value);
export const ASK_REFUSALS: Record<string, string> = {
  composer_unavailable: 'Ask needs the AI model, and it is not available here. Nothing changed.',
  could_not_answer: 'Ask could not produce a verified answer. No answer was shown and nothing changed.',
  too_many_requests: 'Too many questions in a short time. Try again in a moment.',
  invalid_request: 'This question or context is not supported. Nothing changed.',
  [DAILY_LIMIT_CODE]: DAILY_LIMIT_MESSAGE,
};
export function requestScope(scope: AskScope): Record<string, unknown> {
  if (scope.kind === 'widget') return { kind: scope.kind, address: scope.address, widget_id: scope.widget_id, widget: scope.widget, state: scope.state };
  return { kind: scope.kind, address: scope.address, works: scope.works };
}
/** References and receipts are inseparable and must belong to the requested work/collection. */
export function askReply(value: unknown, scope: AskScope): AskReply | null {
  if (!object(value) || Object.keys(value).sort().join(',') !== 'receipts,references,text'
    || typeof value.text !== 'string' || !value.text.trim() || Array.from(value.text).length > 1500
    || !Array.isArray(value.references) || !Array.isArray(value.receipts)
    || value.references.length > 2400 || value.references.length !== value.receipts.length) return null;
  if (scope.kind === 'widget') return value.references.length || /\p{N}/u.test(value.text) ? null : { text: value.text, receipts: [], references: [] };
  const refs: AskReference[] = [], receipts: Record<string, unknown>[] = [], seen = new Set<string>();
  for (const ref of value.references) {
    if (!object(ref) || typeof ref.ref !== 'string' || seen.has(ref.ref) || !uuid(ref.app_id) || !uuid(ref.work_id) || !uuid(ref.record_id)
      || typeof ref.address !== 'string' || typeof ref.label !== 'string') return null;
    const work = scope.works.find(w => w.app_id === ref.app_id && w.work_id === ref.work_id);
    const receipt = value.receipts.find(r => object(r) && r.reference === ref.ref);
    if (!work || !object(receipt) || receipt.kind !== 'record' || receipt.app_id !== ref.app_id || receipt.work_id !== ref.work_id || receipt.record_id !== ref.record_id
      || !uuid(receipt.collection_id) || (work.collection && receipt.collection_id !== work.collection) || !uuid(receipt.publication_id)
      || typeof receipt.definition_digest !== 'string' || !/^sha256:[a-f0-9]{64}$/.test(receipt.definition_digest)
      || !Number.isSafeInteger(receipt.version) || Number(receipt.version) < 1 || !Number.isSafeInteger(receipt.work_version) || Number(receipt.work_version) < 1
      || !Number.isSafeInteger(receipt.commit_sequence) || Number(receipt.commit_sequence) < 1 || typeof receipt.committed_at !== 'string') return null;
    seen.add(ref.ref); refs.push(ref as AskReference); receipts.push(receipt);
  }
  return { text: value.text, references: refs, receipts };
}
export async function requestAsk(question: string, scope: AskScope, signal?: AbortSignal): Promise<AskReply | string> {
  let response: Response;
  try {
    response = await fetch(ASK_URL, { method: 'POST', credentials: 'same-origin', cache: 'no-store', signal,
      headers: { 'content-type': 'application/json' }, body: JSON.stringify({ question, scope: requestScope(scope) }) });
  } catch { return 'The question could not be sent. Check your connection.'; }
  const body: unknown = await response.json().catch(() => null);
  if (response.status === 404) return 'This context is no longer available to you. No answer was shown.';
  if (!response.ok) return ASK_REFUSALS[object(body) && typeof body.code === 'string' ? body.code : ''] ?? 'Ask is unavailable. Nothing changed.';
  return askReply(body, scope) ?? ASK_REFUSALS.could_not_answer!;
}
