import { recordComponentsFor } from '../record-components/controller';
import { invalidatePresentation, disposePresentation } from '../presentation/presentation';
import { WidgetCanvas, createCanvasItem, createCanvasSurface } from '../canvas-vendor/index';
import { isShareKey, setConnected, sharingRequest } from '../sharing/api';
import { fetchShared, replaceSharedParts, syncSampleNote } from './document';
import { watchAuthority, type AuthorityMonitor } from './authority';

export type ConnectedEntry = { id: string; title: string; href: string; owner_label: string; can_add_to_canvas: boolean };
export function connectedEntries(value: unknown): ConnectedEntry[] {
  const apps = (value as { apps?: unknown[] } | null)?.apps;
  if (!Array.isArray(apps) || apps.length > 100) throw new Error('Could not verify your connected apps.');
  const ids = new Set<string>();
  return apps.map(value => {
    const entry = value as ConnectedEntry;
    if (!entry || !isShareKey(entry.id) || entry.href !== `/s/${entry.id}` || typeof entry.title !== 'string'
      || typeof entry.owner_label !== 'string' || entry.can_add_to_canvas !== true || ids.has(entry.id)) throw new Error('Could not verify your connected apps.');
    ids.add(entry.id);
    return entry;
  });
}
export type ConnectedHost = {
  document: Document;
  mount(key: string, title: string, card: HTMLElement): HTMLElement;
  remove(item: HTMLElement): void;
  status?(message: string): void;
};
function node(document: Document, tag: string, attributes: Record<string, string> = {}, ...children: (Node | string)[]): HTMLElement {
  const result = document.createElement(tag);
  for (const [key, value] of Object.entries(attributes)) result.setAttribute(key, value);
  result.append(...children);
  return result;
}
function cardFor(document: Document, entry: ConnectedEntry): HTMLElement {
  return node(document, 'article', { class: 'connected-app', 'data-connected-app': entry.id },
    node(document, 'header', { class: 'connected-app-head', 'data-canvas-drag-handle': '' },
      node(document, 'div', {}, node(document, 'p', { class: 'share-policy' }, 'Connected · View only'), node(document, 'h2', {}, entry.title), node(document, 'p', { class: 'connected-owner' }, entry.owner_label)),
      node(document, 'a', { class: 'share-secondary', href: entry.href, target: '_blank', rel: 'noopener' }, 'Open app'),
      node(document, 'button', { type: 'button', 'data-connected-remove': entry.id }, 'Remove from canvas')),
    node(document, 'div', { 'data-app-presentation-header': '' }),
    node(document, 'p', { 'data-connected-loading': '', role: 'status' }, 'Checking live data…'));
}
/** Inventory and data are authorized independently. These are linked refs,
 * never local copies, ordinary work permissions or automatic share placement. */
export function startConnectedApps(host: ConnectedHost, initial: HTMLElement[] = []): void {
  type Target = { records: ReturnType<typeof recordComponentsFor> | null; item: HTMLElement; card: HTMLElement; monitor: AuthorityMonitor | null; entry: ConnectedEntry; busy: boolean; epoch: number };
  const items = new Map<string, Target>();
  let busy = false;
  let generation = 0;
  let inventoryHealthy = true;
  const removing = new Set<string>();
  function mask(target: Target, message: string): void {
    target.epoch += 1;
    target.records?.invalidate();
    invalidatePresentation(target.card);
    target.card.querySelector('[data-shared-parts]')?.remove();
    const title = target.card.querySelector('h2'); if (title) title.textContent = 'Private app';
    const owner = target.card.querySelector('.connected-owner'); if (owner) owner.textContent = '';
    syncSampleNote(target.card.querySelector<HTMLElement>('.connected-owner')?.parentElement ?? null, null);
    target.item.setAttribute('aria-label', 'Private app'); target.item.dataset.widgetName = 'Private app';
    let status = target.card.querySelector<HTMLElement>('[data-connected-loading]');
    if (!status) { status = node(host.document, 'p', { role: 'status', 'data-connected-loading': '' }); target.card.append(status); }
    status.textContent = message;
  }
  const remove = (key: string) => {
    const target = items.get(key);
    if (target) { target.records?.dispose(); disposePresentation(target.card); target.monitor?.dispose(); host.remove(target.item); items.delete(key); }
  };
  async function refreshData(target: Target): Promise<void> {
    const key = target.entry.id;
    const authority = target.monitor?.current();
    if (!inventoryHealthy || target.busy || !authority || !authority.can_add_to_canvas || removing.has(key)) return;
    const epoch = target.epoch;
    target.busy = true;
    try {
      const props = await fetchShared(key);
      const current = target.monitor?.current();
      if (!items.has(key) || epoch !== target.epoch || !current || current.version !== authority.version || !current.can_add_to_canvas || removing.has(key)) return;
      if (!props.available || !props.can_add_to_canvas || !props.added_to_canvas) { remove(key); return; }
      target.records?.beforeRefresh();
      replaceSharedParts(target.card, props);
      target.records ??= recordComponentsFor(target.card, { share: key });
      target.records.afterRefresh();
      target.card.querySelector('[data-connected-loading]')?.remove();
      const title = target.card.querySelector('h2'); if (title) title.textContent = props.title;
      const owner = target.card.querySelector('.connected-owner'); if (owner) owner.textContent = props.owner_label;
      // APP-098 DS-84: the card's charts read the same guest JSON, so they carry the same note.
      syncSampleNote(target.card.querySelector<HTMLElement>('.connected-owner')?.parentElement ?? null, props);
      target.item.setAttribute('aria-label', props.title); target.item.dataset.widgetName = props.title;
    } catch { if (epoch === target.epoch) mask(target, 'Live data is temporarily unavailable. Retrying…'); }
    finally { target.busy = false; }
  }
  function attach(entry: ConnectedEntry, card = cardFor(host.document, entry)): Target {
    const target: Target = { records: null, entry, card, item: host.mount(entry.id, entry.title, card), monitor: null, busy: false, epoch: 0 };
    items.set(entry.id, target);
    card.addEventListener('forma:record-components-unavailable', () => { mask(target, 'This app is not available.'); target.monitor?.refresh(); });
    mask(target, 'Checking current access…');
    target.monitor = watchAuthority(entry.id, { document: host.document,
      active: () => target.item.dataset.attentionState !== 'parked',
      mask: message => mask(target, message),
      authorized(authority) {
        if (!authority.can_add_to_canvas) { remove(entry.id); return; }
        if (!target.card.querySelector('[data-shared-parts]')) void refreshData(target);
      },
    });
    return target;
  }
  for (const card of initial) {
    const id = card.dataset.connectedApp ?? '';
    if (isShareKey(id)) attach({ id, title: card.querySelector('h2')?.textContent ?? 'Connected app', href: `/s/${id}`, owner_label: '', can_add_to_canvas: true }, card);
  }
  async function refresh(): Promise<void> {
    if (busy || host.document.visibilityState === 'hidden') return;
    if (navigator.onLine === false) { host.status?.('You are offline. Reconnect to verify your connected apps.'); return; }
    busy = true;
    const observed = generation;
    try {
      const entries = connectedEntries(await sharingRequest('/v1/shared'));
      if (observed !== generation) return;
      inventoryHealthy = true;
      const allowed = new Set(entries.map(entry => entry.id));
      for (const key of items.keys()) if (!allowed.has(key)) remove(key);
      for (const entry of entries) {
        if (removing.has(entry.id)) continue;
        try { const target = items.get(entry.id) ?? attach(entry); void refreshData(target); }
        catch { host.status?.('A connected app could not be placed. Refresh to try again.'); }
      }
      host.status?.(items.size ? `${items.size} connected ${items.size === 1 ? 'app' : 'apps'} · Live access checks active` : 'No connected apps yet.');
    } catch { if (observed === generation) {
      inventoryHealthy = false;
      for (const target of items.values()) mask(target, 'Connected apps could not be verified. Retrying…');
      host.status?.('Sign in or refresh to verify your connected apps.');
    } }
    finally { busy = false; }
  }
  host.document.addEventListener('click', event => {
    const button = (event.target as Element | null)?.closest<HTMLButtonElement>('[data-connected-remove]');
    const key = button?.dataset.connectedRemove ?? '';
    if (!button || !items.has(key) || removing.has(key)) return;
    event.preventDefault(); event.stopPropagation();
    removing.add(key); generation += 1; button.disabled = true;
    void setConnected(key, false).then(() => { remove(key); host.status?.('App removed from your canvas. Your invitation is unchanged.'); }).catch(error => {
      host.status?.(error instanceof Error ? error.message : 'Removal could not be confirmed. Try again.'); button.disabled = false;
    }).finally(() => { removing.delete(key); void refresh(); });
  });
  host.document.addEventListener('forma:connected-refresh', () => { void refresh(); for (const item of items.values()) item.monitor?.refresh(); });
  host.document.addEventListener('visibilitychange', () => { generation += 1; if (host.document.visibilityState === 'visible') void refresh(); });
  window.addEventListener('online', () => { void refresh(); });
  window.addEventListener('pagehide', () => { generation += 1; });
  window.addEventListener('pageshow', event => { if (event.persisted) { generation += 1; void refresh(); } });
  window.setInterval(() => { void refresh(); }, 3000);
  void refresh();
}
export function startSharedCanvas(document_: Document = document): void {
  const container = document_.querySelector<HTMLElement>('[data-connected-surface]');
  if (!container) return;
  const initial = Array.from(container.querySelectorAll<HTMLElement>('[data-connected-app]'));
  const surface = createCanvasSurface(container, { className: 'connected-surface', navigatorLabel: 'Connected apps navigator' });
  const canvas = new WidgetCanvas(surface.elements, { maxItems: 100 });
  let next = 0;
  const empty = document_.querySelector<HTMLElement>('[data-connected-empty]');
  const status = document_.querySelector<HTMLElement>('[data-connected-status]');
  const mounted = new Set<HTMLElement>();
  startConnectedApps({ document: document_,
    mount(key, title, card) {
      const item = createCanvasItem({ instanceId: `shared-${key.replace('.', '-')}`, displayName: title, preferredWidth: 700, minHeight: 300, content: card });
      item.classList.add('connected-item');
      const i = next++;
      canvas.mountItem(item, { x: 48 + (i % 2) * 750, y: 64 + Math.floor(i / 2) * 700 }, { focus: false });
      mounted.add(item); if (empty) empty.hidden = true;
      return item;
    },
    remove(item) { canvas.removeItem(item, { selectFallback: false }); mounted.delete(item); if (empty) empty.hidden = mounted.size > 0; },
    status(message) { if (status) status.textContent = message; },
  }, initial);
  container.querySelector('[data-connected-initial]')?.remove();
  document_.addEventListener('forma:connected-fit', () => canvas.fitAll());
}
