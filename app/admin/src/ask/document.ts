import { mountAskPanel } from './panel';
import type { RecordScope } from './client';
const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[47][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;
/** The document only names its current saved work; part scopes never add another work. */
export function documentScope(root: HTMLElement, part: HTMLElement | null = null): { scope: RecordScope; refusal: string } | null {
  const app = root.dataset.app ?? '', work = root.dataset.work ?? '';
  if (!UUID.test(app) || !UUID.test(work)) return null;
  const collection = part?.dataset.collection;
  const scope: RecordScope = { kind: part ? 'card' : 'app', address: '', works: [{ app_id: app, work_id: work, address: '', ...(collection && UUID.test(collection) ? { collection } : {}) }] };
  return { scope, refusal: part && (!collection || !UUID.test(collection)) ? 'Ask cannot yet read source-query results or static content. Ask about the app’s records or a records part instead.' : '' };
}
export function initDocumentAsk(doc: Document): void {
  const root = doc.querySelector<HTMLElement>('[data-document]'), host = doc.querySelector<HTMLElement>('[data-document-ask]');
  if (!root || !host || host.dataset.askWired === 'true') return;
  const initial = documentScope(root); if (!initial) return;
  host.dataset.askWired = 'true'; const panel = mountAskPanel(host, initial.scope);
  root.addEventListener('click', event => {
    const button = (event.target as Element | null)?.closest<HTMLElement>('[data-ask-app], [data-ask-card]');
    if (!button) return;
    const part = button.hasAttribute('data-ask-card') ? button.closest<HTMLElement>('[data-document-part], [data-collection]') : null;
    const next = documentScope(root, part); if (!next) return;
    host.hidden = false; panel.setScope(next.scope, next.refusal); panel.focus(); host.scrollIntoView({ block: 'nearest', behavior: 'smooth' });
  });
  doc.defaultView?.addEventListener('pagehide', () => panel.invalidate());
  doc.defaultView?.addEventListener('offline', () => panel.invalidate());
  doc.addEventListener('visibilitychange', () => { if (doc.hidden) panel.invalidate(); });
  doc.addEventListener('forma:document-unavailable', () => panel.invalidate());
}
