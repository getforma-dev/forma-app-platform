import { isShareKey } from '../sharing/api';

export const AUTHORITY_POLL_MS = 2000;
export const AUTHORITY_TIMEOUT_MS = 2000;
// Leave the 100 ms clearing tick inside ADR-002's 5,000 ms display bound.
export const AUTHORITY_LEASE_MS = 4900;
export type Authority = { available: true; version: number; can_add_to_canvas: boolean };
export type AuthorityTicket = { sequence: number; started: number };

/** Freshness belongs to request start, never arrival time. A delayed or
 * superseded response cannot revive a revoked/expired authorization lease. */
export class AuthorityLease {
  private sequence = 0;
  private accepted = 0;
  private deadline = 0;
  private value: Authority | null = null;
  begin(now: number): AuthorityTicket { return { sequence: ++this.sequence, started: now }; }
  accept(ticket: AuthorityTicket, value: unknown, now: number): boolean {
    const data = value as Authority | null;
    if (ticket.sequence !== this.sequence || ticket.sequence <= this.accepted
      || now < ticket.started || now - ticket.started > AUTHORITY_TIMEOUT_MS
      || !data || data.available !== true || !Number.isSafeInteger(data.version) || data.version < 1
      || typeof data.can_add_to_canvas !== 'boolean') return false;
    this.accepted = ticket.sequence;
    this.deadline = ticket.started + AUTHORITY_LEASE_MS;
    this.value = data;
    return true;
  }
  current(now: number): Authority | null { return now < this.deadline ? this.value : null; }
  invalidate(): void { this.sequence += 1; this.deadline = 0; this.value = null; }
}

export type AuthorityMonitor = { current(): Authority | null; refresh(): void; dispose(): void };
export function watchAuthority(key: string, options: {
  document: Document; active(): boolean; mask(message: string): void; authorized(value: Authority): void;
}): AuthorityMonitor {
  if (!isShareKey(key)) throw new Error('Invalid shared app.');
  const lease = new AuthorityLease();
  let request: AbortController | null = null;
  let disposed = false;
  let wasActive = false;
  let masked = false;
  let knownVersion: number | null = null;
  const now = () => performance.now();
  const active = () => !disposed && options.active() && options.document.visibilityState !== 'hidden' && navigator.onLine !== false;
  function mask(message = 'Checking current access…'): void {
    if (!masked) { masked = true; options.mask(message); }
  }
  function suspend(message = 'Checking current access…'): void {
    lease.invalidate(); request?.abort(); request = null;
    if (wasActive) mask(message);
    wasActive = false;
  }
  async function refresh(): Promise<void> {
    if (!active()) { suspend(navigator.onLine === false ? 'You are offline. Reconnect to verify access.' : 'Checking current access…'); return; }
    wasActive = true;
    if (!lease.current(now())) mask();
    if (request) return;
    const ticket = lease.begin(now());
    const controller = new AbortController(); request = controller;
    const timeout = window.setTimeout(() => controller.abort(), AUTHORITY_TIMEOUT_MS);
    try {
      const response = await fetch(`/v1/shared/${key}/authority`, { credentials: 'same-origin', cache: 'no-store', headers: { accept: 'application/json' }, signal: controller.signal });
      if (!response.ok) throw new Error('This app is unavailable. Sign in or check access.');
      const value: unknown = await response.json();
      if (request !== controller || !active()) return;
      if (!lease.accept(ticket, value, now())) throw new Error('Access could not be verified in time. Retrying…');
      const permitted = lease.current(now())!;
      if (knownVersion !== null && knownVersion !== permitted.version) { masked = false; mask('Access changed. Refreshing the app…'); }
      knownVersion = permitted.version; masked = false;
      options.authorized(permitted);
    } catch (error) {
      if (request === controller) { lease.invalidate(); mask(error instanceof Error ? error.message : 'Access could not be verified. Retrying…'); }
    } finally { window.clearTimeout(timeout); if (request === controller) request = null; }
  }
  const tick = window.setInterval(() => { void refresh(); }, AUTHORITY_POLL_MS);
  const expiry = window.setInterval(() => {
    if (!active()) { suspend(); return; }
    if (!wasActive) { void refresh(); return; }
    if (!lease.current(now())) mask('Access is being rechecked. Live data is hidden until it is confirmed.');
  }, 100);
  const visibility = () => { suspend(); if (active()) void refresh(); };
  const pagehide = () => suspend();
  const pageshow = () => { if (active()) void refresh(); };
  options.document.addEventListener('visibilitychange', visibility);
  window.addEventListener('online', visibility); window.addEventListener('offline', visibility);
  window.addEventListener('pagehide', pagehide); window.addEventListener('pageshow', pageshow);
  void refresh();
  return {
    current: () => active() ? lease.current(now()) : null,
    refresh: () => { void refresh(); },
    dispose() { disposed = true; suspend(); window.clearInterval(tick); window.clearInterval(expiry);
      options.document.removeEventListener('visibilitychange', visibility);
      window.removeEventListener('online', visibility); window.removeEventListener('offline', visibility);
      window.removeEventListener('pagehide', pagehide); window.removeEventListener('pageshow', pageshow);
    },
  };
}
