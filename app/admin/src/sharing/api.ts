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

// --- APP-192: sharing with people in the organization -----------------------
// The routes of `app/src/sharing/mod.rs`. The server is the authority: it
// says which roles this sharer may grant, and it refuses anything else.

export type TeamRole = 'viewer' | 'contributor' | 'reviewer';
const TEAM_ROLES: readonly TeamRole[] = ['viewer', 'contributor', 'reviewer'];
/**
 * Each role in plain words, weakest first. True for shared and private lists
 * alike: in a shared list a contributor may change anyone's entries, in a
 * private list only their own (the dialog says so when the app has one).
 */
export const ROLE_WORDS: Readonly<Record<TeamRole, string>> = {
  viewer: 'Can view',
  contributor: 'Can add and edit entries',
  reviewer: 'Can review all entries',
};
export type TeamMember = { principal_id: string; label: string; role: TeamRole };
export type TeamSharing = {
  roles: TeamRole[];
  canRevokeMembers: boolean;
  /** The app has a private list (computed in Rust): people see only their own entries there. */
  privateLists: boolean;
  /** The app's owner: no share names them, so the person picker leaves them out. */
  owner: string | null;
  organization: { available: boolean; role: TeamRole | null };
  members: TeamMember[];
};
export type Colleague = { principal_id: string; label: string };

function isRole(value: unknown): value is TeamRole {
  return typeof value === 'string' && (TEAM_ROLES as readonly string[]).includes(value);
}
function isLabel(value: unknown): value is string {
  return typeof value === 'string' && value.trim().length > 0 && value.length <= 120;
}
/** The dialog's read, and the person share's route. */
export function teamPath(app: string, work: string): string {
  if (!isUuid(app) || !isUuid(work)) throw new Error('This app is not available for sharing.');
  return `/v1/work/${app}/${work}/share`;
}
/** The server's answer, or null when it is not exactly the expected shape. */
export function teamSharing(value: unknown, app: string, work: string): TeamSharing | null {
  if (!value || typeof value !== 'object') return null;
  const body = value as Record<string, unknown>;
  const organization = body.organization as Record<string, unknown> | null;
  if (body.app_id !== app || body.work_id !== work || typeof body.can_revoke_members !== 'boolean' || typeof body.private_lists !== 'boolean') return null;
  if (body.owner_principal_id !== null && !isUuid(body.owner_principal_id)) return null;
  if (!Array.isArray(body.roles) || body.roles[0] !== 'viewer' || !body.roles.every(isRole)) return null;
  if (!organization || typeof organization !== 'object' || typeof organization.available !== 'boolean') return null;
  if (organization.role !== null && !isRole(organization.role)) return null;
  if (!Array.isArray(body.members)) return null;
  const members: TeamMember[] = [];
  for (const item of body.members as unknown[]) {
    const member = item as Record<string, unknown> | null;
    if (!member || !isUuid(member.principal_id) || !isLabel(member.label) || !isRole(member.role)) return null;
    members.push({ principal_id: member.principal_id as string, label: member.label, role: member.role });
  }
  return {
    roles: [...body.roles] as TeamRole[],
    canRevokeMembers: body.can_revoke_members,
    privateLists: body.private_lists,
    owner: body.owner_principal_id as string | null,
    organization: { available: organization.available, role: organization.role as TeamRole | null },
    members,
  };
}
/** The roles to offer, in order: only those the server says this sharer may grant. */
export function offeredRoles(sharing: TeamSharing): TeamRole[] {
  return TEAM_ROLES.filter(role => sharing.roles.includes(role));
}
export function memberShareRequest(app: string, work: string, grantee: string, role: TeamRole): { url: string; body: string } | null {
  if (!isUuid(app) || !isUuid(work) || !isUuid(grantee) || !isRole(role)) return null;
  return { url: teamPath(app, work), body: JSON.stringify({ grantee_principal_id: grantee, role }) };
}
/**
 * The automatic share (the canvas approver step): the person share's body on
 * its own route, which only adds what the person lacks and never takes any
 * access away, so a reviewer approver keeps their standing.
 */
export function addShareRequest(app: string, work: string, grantee: string, role: TeamRole): { url: string; body: string } | null {
  const request = memberShareRequest(app, work, grantee, role);
  return request && { url: `${request.url}/add`, body: request.body };
}
export function everyoneRequest(app: string, work: string, role: TeamRole | 'none'): { url: string; body: string } | null {
  if (!isUuid(app) || !isUuid(work) || !(role === 'none' || isRole(role))) return null;
  return { url: `${teamPath(app, work)}/organization`, body: JSON.stringify({ role }) };
}
/**
 * Removing one person's access: the person route's `none`, the owner's
 * revocation (`revoke_work_share`). The dialog shows the control only while
 * the server answers `can_revoke_members: true` (the app's owner).
 */
export function memberRevokeRequest(app: string, work: string, grantee: string): { url: string; body: string } | null {
  if (!isUuid(app) || !isUuid(work) || !isUuid(grantee)) return null;
  return { url: teamPath(app, work), body: JSON.stringify({ grantee_principal_id: grantee, role: 'none' }) };
}
/** Why a work cannot be shared with everyone (the sharer's sources may not be their own). */
export const UNAVAILABLE_FOR_EVERYONE = 'This app uses data sources or folder files, so it can be shared one person at a time only.';
/** Plain copy for a refused request. It never names another member. */
export function teamRefusal(status: number, code: string): string {
  if (code === 'reviewer_not_allowed') return 'Only someone who can review all entries can give that access.';
  if (code === 'contributor_not_allowed') return 'Only someone who can add and edit entries can give that access.';
  if (code === 'organization_share_unavailable') return UNAVAILABLE_FOR_EVERYONE;
  if (status === 404) return 'That share was refused. Refresh the list and try again.';
  if (status === 400) return 'That request was not understood. Refresh the list and try again.';
  if (status === 403) return 'This page could not confirm the request. Reload the page and try again.';
  if (status === 429) return 'Too many changes at once. Wait a moment and try again.';
  return 'Sharing is unavailable right now. Try again in a moment.';
}
async function teamRequest(url: string, method: 'GET' | 'POST', body?: string): Promise<{ status: number; json: unknown }> {
  let response: Response;
  try {
    response = await fetch(url, { method, credentials: 'same-origin', cache: 'no-store', signal: AbortSignal.timeout(8000),
      headers: { accept: 'application/json', ...(body === undefined ? {} : { 'content-type': 'application/json' }) },
      ...(body === undefined ? {} : { body }) });
  } catch { throw new ShareError('The result could not be confirmed. Check the list before trying again.', method !== 'GET'); }
  let json: unknown = null;
  try { json = await response.json(); } catch { json = null; }
  if (!response.ok) {
    const code = typeof (json as { code?: unknown } | null)?.code === 'string' ? (json as { code: string }).code : '';
    throw Object.assign(new ShareError(teamRefusal(response.status, code)), { status: response.status });
  }
  return { status: response.status, json };
}
/** The dialog's read; null when this member cannot share the app (the uniform 404). */
export async function readTeamSharing(app: string, work: string): Promise<TeamSharing | null> {
  try {
    const { json } = await teamRequest(teamPath(app, work), 'GET');
    const sharing = teamSharing(json, app, work);
    if (!sharing) throw new ShareError('The sharing list could not be read.');
    return sharing;
  } catch (error) {
    if ((error as { status?: number }).status === 404) return null;
    throw error;
  }
}
/** The other members of the organization, by the names they gave themselves. */
export async function readColleagues(): Promise<Colleague[]> {
  const { json } = await teamRequest('/v1/members', 'GET');
  const members = (json as { members?: unknown } | null)?.members;
  if (!Array.isArray(members) || !members.every(m => m && isUuid((m as Colleague).principal_id) && isLabel((m as Colleague).label))) {
    throw new ShareError('The list of people could not be read.');
  }
  return (members as Colleague[]).map(m => ({ principal_id: m.principal_id, label: m.label }));
}
export async function shareWithMember(app: string, work: string, grantee: string, role: TeamRole): Promise<void> {
  const request = memberShareRequest(app, work, grantee, role);
  if (!request) throw new ShareError('Choose a person and the access to give.');
  const { json } = await teamRequest(request.url, 'POST', request.body);
  if ((json as { grantee_principal_id?: unknown } | null)?.grantee_principal_id !== grantee) {
    throw new ShareError('The result could not be confirmed. Check the list before trying again.', true);
  }
}
export async function shareWithEveryone(app: string, work: string, role: TeamRole | 'none'): Promise<TeamRole | 'none'> {
  const request = everyoneRequest(app, work, role);
  if (!request) throw new ShareError('Choose the access to give everyone.');
  const { json } = await teamRequest(request.url, 'POST', request.body);
  if ((json as { role?: unknown } | null)?.role !== role) {
    throw new ShareError('The result could not be confirmed. Check the list before trying again.', true);
  }
  return role;
}
export async function revokeMemberShare(app: string, work: string, grantee: string): Promise<void> {
  const request = memberRevokeRequest(app, work, grantee);
  if (!request) throw new ShareError('This person cannot be removed here.');
  const { json } = await teamRequest(request.url, 'POST', request.body);
  const answer = json as { grantee_principal_id?: unknown; role?: unknown } | null;
  if (answer?.grantee_principal_id !== grantee || answer?.role !== 'none') {
    throw new ShareError('The result could not be confirmed. Check the list before trying again.', true);
  }
}
