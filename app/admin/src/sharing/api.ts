import { isUuid } from '../canvas/ids';

export type ShareSummary = {
  share_id: string; version: number; status: 'pending' | 'active' | 'revoked' | 'expired' | 'paused';
  recipient_email: string; allow_add_to_canvas: boolean; expires_at: string; href: string;
};
export type ShareDraft = {
  recipient_email: string; allow_add_to_canvas: boolean; expires_in_days: number; idempotency_key: string;
};
export function isShareKey(value: unknown): value is string {
  return typeof value === 'string' && value.split('.').length === 2 && value.split('.').every(isUuid);
}
export function shareSummary(value: unknown): ShareSummary | null {
  if (!value || typeof value !== 'object') return null;
  const item = value as ShareSummary;
  return isShareKey(item.share_id) && Number.isSafeInteger(item.version) && item.version > 0
    && ['pending', 'active', 'revoked', 'expired', 'paused'].includes(item.status)
    && typeof item.recipient_email === 'string' && typeof item.allow_add_to_canvas === 'boolean'
    && typeof item.expires_at === 'string' && Number.isFinite(Date.parse(item.expires_at)) && item.href === `/s/${item.share_id}` ? item : null;
}
export function sharesPath(app: string, work: string): string {
  if (!isUuid(app) || !isUuid(work)) throw new Error('This app is not available for sharing.');
  return `/v1/work/${app}/${work}/shares`;
}
export class ShareError extends Error {
  constructor(message: string, readonly uncertain = false) { super(message); }
}
export async function sharingRequest(path: string, method = 'GET', body?: unknown): Promise<unknown> {
  let response: Response;
  try {
    response = await fetch(path, { method, credentials: 'same-origin', cache: 'no-store',
      signal: AbortSignal.timeout(8000),
      headers: { accept: 'application/json', ...(body === undefined ? {} : { 'content-type': 'application/json' }) },
      ...(body === undefined ? {} : { body: JSON.stringify(body) }) });
  } catch { throw new ShareError('The result could not be confirmed. Retry the same request when you are connected.', method !== 'GET'); }
  if (!response.ok) {
    if (response.status === 409) throw new ShareError('Sharing changed in another window. Refresh the list and try again.');
    if (response.status === 404) throw new ShareError('This share is not available, or you cannot change it.');
    if (response.status === 400 || response.status === 422) throw new ShareError('Check the email address and expiry, then try again.');
    throw new ShareError('Sharing is unavailable right now. Retry in a moment.', method !== 'GET');
  }
  if (response.status === 204) return null;
  try { return await response.json(); }
  catch { throw new ShareError('The result could not be confirmed. Refresh before trying again.', method !== 'GET'); }
}
export async function listShares(path: string): Promise<ShareSummary[]> {
  const body = await sharingRequest(path) as { shares?: unknown };
  if (!Array.isArray(body?.shares)) throw new ShareError('The share list could not be read.');
  const shares = body.shares.map(shareSummary);
  if (shares.some(item => !item)) throw new ShareError('The share list could not be read.');
  return shares as ShareSummary[];
}
export async function createShare(path: string, draft: ShareDraft): Promise<ShareSummary> {
  const response = await sharingRequest(path, 'POST', draft) as { share?: unknown };
  const share = shareSummary(response?.share);
  if (!share) throw new ShareError('The result could not be confirmed. Retry the same share.', true);
  return share;
}
export function canvasPath(key: string): string {
  if (!isShareKey(key)) throw new Error('This shared app is not available.');
  return `/v1/shared/${key}/canvas`;
}
export async function setConnected(key: string, add: boolean): Promise<void> {
  await sharingRequest(canvasPath(key), add ? 'POST' : 'DELETE', {});
}
