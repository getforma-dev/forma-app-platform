import { isShareKey, canvasPath, sharingRequest } from '../sharing/api';
import { renderReadOnlyRegions, type ReadOnlyProps } from '../work/render';
import { watchAuthority, type AuthorityMonitor } from './authority';

export type SharedJson = ReadOnlyProps & {
  share_id: string; title: string; status: string; owner_label: string; expires_at: string;
  available: boolean; message: string; can_add_to_canvas: boolean; added_to_canvas: boolean;
  needs_sign_in: boolean; can_accept: boolean; sign_in_action: string;
};
export function sharedJson(value: unknown, key: string): SharedJson {
  const data = value as SharedJson | null;
  if (!data || !isShareKey(key) || data.share_id !== key || typeof data.available !== 'boolean'
    || typeof data.can_add_to_canvas !== 'boolean' || typeof data.added_to_canvas !== 'boolean'
    || typeof data.can_accept !== 'boolean' || typeof data.needs_sign_in !== 'boolean'
    || typeof data.title !== 'string' || typeof data.message !== 'string'
    || (data.parts !== undefined && !Array.isArray(data.parts))
    || !Array.isArray(data.views) || !Array.isArray(data.record_cards)) throw new Error('The shared app could not be verified.');
  return data;
}
export async function fetchShared(key: string): Promise<SharedJson> {
  if (!isShareKey(key)) throw new Error('Invalid shared app.');
  return sharedJson(await sharingRequest(`/s/${key}`), key);
}
export function replaceSharedParts(root: HTMLElement, props: ReadOnlyProps): void {
  // Preserve receipt disclosure while refreshing the authoritative values.
  const opened = new Set(Array.from(root.querySelectorAll<HTMLElement>('[data-view]')).filter(view => view.querySelector('details')?.open).map(view => view.dataset.view));
  const replacement = renderReadOnlyRegions(root.ownerDocument, props);
  for (const view of replacement.querySelectorAll<HTMLElement>('[data-view]')) {
    const receipt = view.querySelector('details');
    if (receipt && opened.has(view.dataset.view)) receipt.open = true;
  }
  const current = root.querySelector('[data-shared-parts]');
  if (current) current.replaceWith(replacement); else (root.querySelector('[data-shared-content]') ?? root).append(replacement);
}
export function startSharedPage(document_: Document = document): void {
  const root = document_.querySelector<HTMLElement>('[data-shared-page]');
  const key = root?.dataset.share ?? '';
  if (!root || !isShareKey(key)) return;
  const status = root.querySelector<HTMLElement>('[data-shared-status]');
  let refreshing = false;
  let acting = false;
  let generation = 0;
  const live = !!root.querySelector('[data-shared-parts], [data-share-accept]');
  let wasAvailable = !!root.querySelector('[data-shared-parts]');
  let monitor: AuthorityMonitor | null = null;
  const say = (message: string) => { if (status) status.textContent = message; };
  function unavailable(message: string): void {
    root!.querySelector('[data-shared-parts]')?.remove();
    const title = root!.querySelector('h1'); if (title) title.textContent = 'Private app';
    const owner = root!.querySelector<HTMLElement>('.shared-by'); if (owner) owner.hidden = true;
    for (const button of root!.querySelectorAll<HTMLButtonElement>('[data-share-add], [data-share-accept]')) button.disabled = true;
    say(message);
    if (status) { const retry = document_.createElement('a'); retry.href = `/s/${key}`; retry.textContent = 'Sign in or check access'; status.append(' ', retry); }
  }
  async function refresh(): Promise<void> {
    if (!live || refreshing || acting || document_.visibilityState === 'hidden') return;
    if (navigator.onLine === false) { unavailable('You are offline. Reconnect to verify access and live data.'); return; }
    const authority = monitor?.current();
    if (wasAvailable && !authority) { monitor?.refresh(); return; }
    refreshing = true;
    const observed = generation;
    try {
      const props = await fetchShared(key);
      if (observed !== generation) return;
      const accept = root!.querySelector<HTMLButtonElement>('[data-share-accept]');
      if (!props.available) {
        unavailable(props.message || 'This app is not available.');
        if (accept) { accept.disabled = !props.can_accept; accept.hidden = !props.can_accept; }
        return;
      }
      if (!wasAvailable) { wasAvailable = true; monitor?.refresh(); return; }
      if (!authority || monitor?.current()?.version !== authority.version) return;
      replaceSharedParts(root!, props);
      if (accept) { accept.hidden = true; accept.disabled = true; }
      const title = root!.querySelector('h1'); if (title) title.textContent = props.title;
      const owner = root!.querySelector<HTMLElement>('.shared-by');
      if (owner) { owner.hidden = false; owner.textContent = `Shared by ${props.owner_label} · Access expires ${props.expires_at}`; }
      const message = root!.querySelector('[data-shared-message]'); if (message) message.textContent = props.message;
      let add = root!.querySelector<HTMLButtonElement>('[data-share-add]');
      if (!add && props.can_add_to_canvas && !props.added_to_canvas) {
        add = document_.createElement('button');
        add.type = 'button'; add.className = 'share-primary'; add.dataset.shareAdd = ''; add.textContent = 'Add to my canvas';
        add.addEventListener('click', () => document_.dispatchEvent(new CustomEvent('forma:shared-action', { detail: 'add' })));
        root!.querySelector('.shared-actions')?.prepend(add);
      }
      if (add) { add.hidden = !props.can_add_to_canvas || props.added_to_canvas; add.disabled = !props.can_add_to_canvas; }
      if (props.added_to_canvas && !root!.querySelector('[data-share-open]')) {
        const open = document_.createElement('a'); open.href = '/shared'; open.className = 'share-secondary'; open.dataset.shareOpen = ''; open.textContent = 'Open my connected apps'; root!.querySelector('.shared-actions')?.prepend(open);
      }
      say('Live data checked just now.');
    } catch (error) {
      if (observed === generation) {
        unavailable(error instanceof Error ? error.message : 'Could not verify access. Refresh to try again.');
      }
    } finally { refreshing = false; }
  }
  // Native markup may have arrived in a hidden tab or from a slow navigation.
  // It receives a fresh browser authorization lease before being presented.
  if (wasAvailable) unavailable('Checking current access…');
  monitor = watchAuthority(key, { document: document_, active: () => wasAvailable,
    mask(message) { generation += 1; unavailable(message); },
    authorized(value) {
      const add = root.querySelector<HTMLButtonElement>('[data-share-add]');
      if (add && !value.can_add_to_canvas) { add.disabled = true; add.hidden = true; }
      if (!root.querySelector('[data-shared-parts]')) window.setTimeout(() => { void refresh(); }, 0);
    },
  });
  document_.addEventListener('forma:shared-action', event => {
    const action = (event as CustomEvent<string>).detail;
    if (action === 'refresh') { void refresh(); return; }
    if (acting || (action !== 'add' && action !== 'accept')) return;
    const button = root.querySelector<HTMLButtonElement>(action === 'add' ? '[data-share-add]' : '[data-share-accept]');
    if (!button || button.disabled || button.hidden || (action === 'add' && !monitor?.current()?.can_add_to_canvas)) return;
    acting = true; generation += 1; button.disabled = true;
    say(action === 'add' ? 'Adding the live app…' : 'Accepting your invitation…');
    void sharingRequest(action === 'add' ? canvasPath(key) : `/v1/shared/${key}/accept`, 'POST', {}).then(() => {
      // The server owns acceptance and placement; a full native response also
      // updates the authenticated controls after a formerly pending invite.
      window.location.reload();
    }).catch(error => { say(error instanceof Error ? error.message : 'The change could not be confirmed. Retry to check.'); button.disabled = false; acting = false; });
  });
  window.setInterval(() => { void refresh(); }, 3000);
  document_.addEventListener('visibilitychange', () => { if (document_.visibilityState === 'visible') { unavailable('Checking current access…'); void refresh(); } });
  window.addEventListener('online', () => { void refresh(); });
  window.addEventListener('pagehide', () => { generation += 1; if (wasAvailable) unavailable('Checking current access…'); });
  window.addEventListener('pageshow', event => { if (event.persisted) { generation += 1; unavailable('Checking current access…'); void refresh(); } });
}
