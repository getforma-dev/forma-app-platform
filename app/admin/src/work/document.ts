import { renderRegions, type DocumentProps } from './render';
import { submitCommand } from '../canvas/commands';
import { subscribeLive, type LiveEvent } from '../canvas/live';

/** A hint never carries app data or authority. Fetch the exact document again. */
export function refreshesDocument(event: LiveEvent, app: string, work: string): boolean {
  return event.kind === 'stream.lagged' || (event.app_id === app && event.work_id === work);
}

/** Enhance the native document. Live refresh preserves every unsent form,
 * including its draft values and idempotency premises. Only a successful
 * command replaces its own form with a newly authorized server render. */
export function startDocument(): void {
  const root = document.querySelector<HTMLElement>('[data-document]');
  if (!root) return;
  const { app = '', work = '', path = '' } = root.dataset;
  if (!/^\/w\/[0-9a-f-]{36}\/[0-9a-f-]{36}$/.test(path)) return;
  const status = root.querySelector<HTMLElement>('[data-document-state]');
  let stopped = false;
  let refreshing = false;
  let queued = false;
  let unsubscribe = () => undefined as void;
  const lifecycle = new AbortController();
  const pending = new WeakSet<HTMLFormElement>();
  // BFCache may suspend a request whose answer never reached this controller.
  // Permit a retry with the original frozen values/key, never a new command.
  for (const form of root.querySelectorAll<HTMLFormElement>('form[data-command-pending]')) {
    if (form.dataset.commandInvalidated) continue;
    const button = form.querySelector<HTMLButtonElement>('button[type="submit"]');
    if (button) button.disabled = false;
    const notice = form.querySelector<HTMLElement>('[data-command-status]');
    if (notice) notice.textContent = 'The previous result was not confirmed. Submit again to recover the same command.';
  }
  const completed = new Set<string>();
  const say = (message: string) => { if (status) status.textContent = message; };
  const stop = () => {
    if (stopped) return;
    stopped = true;
    unsubscribe();
    lifecycle.abort();
    window.clearInterval(timer);
    document.removeEventListener('visibilitychange', visible);
    document.removeEventListener('forma:document-refresh', requested);
  };
  const refresh = async () => {
    if (stopped) return;
    if (refreshing) { queued = true; return; }
    refreshing = true;
    // Only forms committed before this read began may consume its new keys.
    const committedBeforeRead = new Set(completed);
    try {
      const response = await fetch(path, { credentials: 'same-origin', cache: 'no-store', signal: lifecycle.signal, headers: { accept: 'application/json' } });
      if (stopped) return;
      if (response.status === 404) {
        stop();
        // A denial removes old records AND forms and reveals no distinction
        // between a missing work and a member who lost access.
        const notice = document.createElement('p');
        notice.setAttribute('role', 'alert');
        notice.textContent = 'This app is not available.';
        const link = document.createElement('a');
        link.href = '/workspace';
        link.textContent = 'Workspace';
        root.replaceChildren(notice, link);
        return;
      }
      if (!response.ok) throw new Error('unavailable');
      const props = await response.json() as DocumentProps;
      if (stopped) return;
      if (props.action !== path || props.app !== app || props.work !== work) throw new Error('document mismatch');
      const regions = renderRegions(document, props);
      for (const [selector, replacement] of [
        ['[data-document-records]', regions.records], ['[data-document-views]', regions.views],
      ] as const) {
        const before = root.querySelector(selector);
        if (!before) throw new Error('missing region');
        before.replaceWith(replacement);
      }
      const incoming = regions.forms;
      for (const collection of committedBeforeRead) {
        const find = (scope: ParentNode) => Array.from(scope.querySelectorAll<HTMLElement>('[data-document-form]')).find(node => node.dataset.documentForm === collection);
        const before = find(root);
        const after = find(incoming);
        if (before && after) before.replaceWith(after.cloneNode(true));
        else before?.remove();
        completed.delete(collection);
      }
      // Read access can survive a write-grant removal or a new publication.
      // Keep the draft visible but do not offer an obsolete command form.
      for (const section of root.querySelectorAll<HTMLElement>('[data-document-form]')) {
        const fresh = Array.from(incoming.querySelectorAll<HTMLElement>('[data-document-form]')).find(node => node.dataset.documentForm === section.dataset.documentForm);
        const value = (node: ParentNode, name: string) => node.querySelector<HTMLInputElement>(`input[name="${name}"]`)?.value;
        if (!fresh || ['publication_id', 'schema_version_expected', 'expected_work_version'].some(name => value(section, name) !== value(fresh, name))) {
          const form = section.querySelector('form');
          if (form) form.dataset.commandInvalidated = 'true';
          for (const control of section.querySelectorAll<HTMLInputElement | HTMLSelectElement | HTMLButtonElement>('input, select, button')) control.disabled = true;
          const notice = section.querySelector<HTMLElement>('[data-command-status]');
          if (notice) notice.textContent = 'This form is no longer available for this app revision or your access. Your draft is kept; reload to continue.';
        }
      }
      say('Up to date.');
    } catch {
      if (!stopped) say('Updates are unavailable. Your unfinished form is kept. Try Refresh.');
    } finally {
      refreshing = false;
      if (queued && !stopped) { queued = false; void refresh(); }
    }
  };
  root.addEventListener('submit', async event => {
    const form = event.target;
    if (!(form instanceof HTMLFormElement) || !form.matches('[data-command]')) return;
    event.preventDefault();
    if (pending.has(form) || stopped || form.dataset.commandInvalidated) return;
    pending.add(form);
    form.dataset.commandPending = 'true';
    const button = form.querySelector<HTMLButtonElement>('button[type="submit"]');
    const notice = form.querySelector<HTMLElement>('[data-command-status]');
    if (button) button.disabled = true;
    // A retry must use the original payload under this key, including after
    // an uncertain answer. Freeze values while that outcome is unresolved.
    const controls = Array.from(form.querySelectorAll<HTMLInputElement | HTMLSelectElement>('input, select'));
    for (const control of controls) control.disabled = true;
    if (notice) notice.textContent = 'Saving…';
    const result = await submitCommand(form, app, work);
    if (stopped) return;
    if (result.kind === 'committed') {
      // Keep the recovery marker until a fresh form replaces this one; a
      // BFCache restore before that read can safely replay the same key.
      if (notice) notice.textContent = `Saved · Commit ${result.commit}. Refreshing the app…`;
      const collection = form.closest<HTMLElement>('[data-document-form]')?.dataset.documentForm;
      if (collection) completed.add(collection);
      void refresh();
    } else {
      if (notice) notice.textContent = result.message;
      // Keep the same key and values for retries, including a lost answer.
      pending.delete(form);
      if (form.dataset.commandInvalidated) return;
      if (result.kind === 'refused' || result.kind === 'conflict' || result.kind === 'stale') {
        delete form.dataset.commandPending;
        for (const control of controls) control.disabled = false;
      }
      if (button) button.disabled = false;
    }
  }, { signal: lifecycle.signal });
  const visible = () => { if (document.visibilityState === 'visible') void refresh(); };
  const requested = () => { void refresh(); };
  // The stream can omit a revocation hint; bounded reads also recheck access.
  const timer = window.setInterval(visible, 30_000);
  document.addEventListener('visibilitychange', visible);
  document.addEventListener('forma:document-refresh', requested);
  window.addEventListener('online', requested, { signal: lifecycle.signal });
  window.addEventListener('pagehide', event => {
    stop();
    if (event.persisted) window.addEventListener('pageshow', () => startDocument(), { once: true });
  }, { once: true, signal: lifecycle.signal });
  unsubscribe = subscribeLive('/v1/events', event => {
    if (refreshesDocument(event, app, work)) void refresh();
  }, state => {
    if (!stopped) say(state === 'live' ? 'Live updates connected.' : 'Reconnecting to updates…');
  });
  // Close the gap between native SSR and opening the live subscription.
  void refresh();
}
