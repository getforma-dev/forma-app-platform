import { recordComponentsFor } from '../record-components/controller';
import { presentationFor } from '../presentation/presentation';
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
  const { app = '', work = '', path = '', csrf = '' } = root.dataset;
  if (!/^\/w\/[0-9a-f-]{36}\/[0-9a-f-]{36}$/.test(path)) return;
  const presentation = presentationFor(root);
  const recordComponents = recordComponentsFor(root, { app, work });
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
  const replaceForms = new Set<string>();
  const selections = new Map<string, string>();
  for (const select of root.querySelectorAll<HTMLSelectElement>('[data-document-selection] select')) selections.set(select.name, select.value);
  let selectionGeneration = 0;
  const say = (message: string) => { if (status) status.textContent = message; };
  const stop = () => {
    if (stopped) return;
    stopped = true;
    recordComponents.dispose();
    presentation.dispose();
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
    const replacedBeforeRead = new Set(replaceForms);
    const selectionAtRead = selectionGeneration;
    try {
      const body = new URLSearchParams({ csrf, document_action: 'refresh' });
      for (const [name, value] of selections) body.append(name, value);
      const response = await fetch(path, { credentials: 'same-origin', cache: 'no-store', signal: lifecycle.signal,
        ...(selections.size ? { method: 'POST', body } : {}), headers: { accept: 'application/json' } });
      if (stopped) return;
      if (response.status === 404) {
        stop();
        document.dispatchEvent(new Event('forma:document-unavailable'));
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
      if (selectionAtRead !== selectionGeneration) { queued = true; return; }
      const regions = renderRegions(document, props);
      recordComponents.beforeRefresh();
      presentation.beforeRefresh();
      const incoming = regions.parts;
      const findForm = (scope: ParentNode, id: string) => Array.from(scope.querySelectorAll<HTMLElement>('[data-document-form]')).find(node => node.dataset.documentForm === id);
      const invalidate = (section: HTMLElement) => {
        const form = section.querySelector('form');
        if (form) form.dataset.commandInvalidated = 'true';
        for (const control of section.querySelectorAll<HTMLInputElement | HTMLSelectElement | HTMLButtonElement>('input, select, button')) control.disabled = true;
        const notice = section.querySelector<HTMLElement>('[data-command-status]');
        if (notice) notice.textContent = 'This form or record has changed, or your write access ended. Your draft is kept; reload to continue.';
      };
      for (const before of root.querySelectorAll<HTMLElement>('[data-document-form]')) {
        const id = before.dataset.documentForm || '';
        const after = findForm(incoming, id);
        if (committedBeforeRead.has(id) || replacedBeforeRead.has(id)) {
          completed.delete(id);
          replaceForms.delete(id);
          continue;
        }
        const value = (node: ParentNode, name: string) => node.querySelector<HTMLInputElement>(`input[name="${name}"]`)?.value;
        const changed = !after || ['publication_id', 'schema_version_expected', 'expected_work_version', 'record_id', 'expected_record_version'].some(name => value(before, name) !== value(after, name));
        // A create's fresh record UUID changes on every read. Preserve its
        // draft/key while the publication/schema/write authority is unchanged.
        const creating = value(before, 'expected_record_version') === '0' && value(after || before, 'expected_record_version') === '0';
        const changedCreate = !after || ['publication_id', 'schema_version_expected', 'expected_work_version'].some(name => value(before, name) !== value(after, name));
        if (creating ? changedCreate : changed) invalidate(before);
        if (after) after.replaceWith(before);
        else {
          const part = Array.from(incoming.querySelectorAll<HTMLElement>('[data-document-part]')).find(node => node.dataset.documentPart === id);
          if (part) part.append(before);
          else incoming.append(before);
        }
      }
      const beforeParts = root.querySelector('[data-document-parts]');
      const beforeRecords = root.querySelector('[data-document-records]');
      if (!beforeParts || !beforeRecords) throw new Error('missing region');
      beforeParts.replaceWith(incoming);
      beforeRecords.replaceWith(regions.records);
      selections.clear();
      for (const select of incoming.querySelectorAll<HTMLSelectElement>('[data-document-selection] select')) selections.set(select.name, select.value);
      const title = root.querySelector<HTMLElement>('[data-document-title]');
      if (title && typeof props.title === 'string') title.textContent = props.title;
      presentation.afterRefresh(props.presentation);
      recordComponents.afterRefresh();
      say('Up to date.');
    } catch {
      presentation.afterRefresh();
      if (!stopped) say('Updates are unavailable. Your unfinished form is kept. Try Refresh.');
    } finally {
      refreshing = false;
      if (queued && !stopped) { queued = false; void refresh(); }
    }
  };
  root.addEventListener('submit', async event => {
    const form = event.target;
    if (!(form instanceof HTMLFormElement)) return;
    if (form.matches('[data-document-selection]')) {
      event.preventDefault();
      if (stopped) return;
      const select = form.querySelector<HTMLSelectElement>('select');
      const part = form.closest<HTMLElement>('[data-document-part]');
      if (!select || !part?.dataset.documentPart) return;
      const command = part.querySelector<HTMLFormElement>('form[data-command]');
      if (command?.dataset.commandPending) { say('Recover the pending command before choosing another record.'); return; }
      selections.set(select.name, select.value);
      replaceForms.add(part.dataset.documentPart);
      selectionGeneration++;
      void refresh();
      return;
    }
    if (!form.matches('[data-command]')) return;
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
  root.addEventListener('forma:record-components-unavailable', () => { void refresh(); }, { signal: lifecycle.signal });
  const visible = () => { if (document.visibilityState === 'visible') void refresh(); };
  const requested = () => { void refresh(); };
  // The stream can omit a revocation hint; bounded reads also recheck access.
  const timer = window.setInterval(visible, 30_000);
  document.addEventListener('visibilitychange', visible);
  document.addEventListener('forma:document-refresh', requested);
  document.addEventListener('forma:presentation-saved', event => {
    const detail = (event as CustomEvent<{app: string; work: string}>).detail;
    if (detail?.app === app && detail.work === work) void refresh();
  }, { signal: lifecycle.signal });
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
