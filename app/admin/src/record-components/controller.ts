import { renderRecordComponents, type WireRecordComponent } from './render';

type Identity = { app: string; work: string } | { share: string };
type Filter = { id: string; value: string } | { id: string; start: string; end: string };
type Batch = { components: WireRecordComponent[]; filter_epoch: number };
const ID = /^[a-z][a-z0-9_]{0,63}$/;

/** A response from an older input generation can never replace a newer result. */
export function matchingBatch(value: unknown, epoch: number, ids: ReadonlySet<string>): value is Batch {
  if (!value || typeof value !== 'object') return false;
  const batch = value as Batch;
  if (batch.filter_epoch !== epoch || !Array.isArray(batch.components) || batch.components.length > 32) return false;
  const seen = new Set<string>();
  return batch.components.every(component => {
    if (!component || !ID.test(component.id) || !ids.has(component.id) || seen.has(component.id)) return false;
    seen.add(component.id);
    return typeof component.state === 'string' && Array.isArray(component.filters) && Array.isArray(component.rows)
      && Array.isArray(component.columns) && Array.isArray(component.groups)
      && typeof component.has_receipt === 'boolean' && typeof component.message === 'string';
  }) && [...ids].every(id => seen.has(id));
}

export function recordComponentsFor(root: HTMLElement, identity: Identity) {
  const doc = root.ownerDocument;
  const endpoint = 'share' in identity ? `/v1/shared/${encodeURIComponent(identity.share)}/record-components`
    : `/v1/work/${encodeURIComponent(identity.app)}/${encodeURIComponent(identity.work)}/record-components`;
  const listeners = new AbortController();
  let request: AbortController | null = null;
  let generation = 0;
  let epoch = 0;
  let suspended = false;
  let disposed = false;
  let active: Filter[] = [];
  const drafts = new Map<string, string>();
  let focus: { component: string; attribute: string; id: string; start: number | null; end: number | null } | null = null;
  const belongs = (node: HTMLElement) => {
    if ('share' in identity) return root.contains(node);
    const scope = node.closest<HTMLElement>('[data-app][data-work]');
    return scope?.dataset.app === identity.app && scope.dataset.work === identity.work;
  };
  const components = () => Array.from(root.querySelectorAll<HTMLElement>('[data-record-component]')).filter(belongs);
  const enable = () => {
    for (const component of components()) for (const button of component.querySelectorAll<HTMLButtonElement>('[data-record-filter-enable]')) button.disabled = false;
  };
  const inputs = (scope: ParentNode) => Array.from(scope.querySelectorAll<HTMLInputElement | HTMLSelectElement>('[data-filter-value],[data-filter-start],[data-filter-end]'));
  const fieldKey = (field: HTMLElement) => {
    const attribute = field.hasAttribute('data-filter-value') ? 'data-filter-value' : field.hasAttribute('data-filter-start') ? 'data-filter-start' : 'data-filter-end';
    return { attribute, id: field.getAttribute(attribute) || '' };
  };
  const capture = () => {
    for (const component of components()) {
      for (const field of inputs(component)) {
        const { attribute, id } = fieldKey(field);
        drafts.set(`${component.dataset.recordComponent}:${attribute}:${id}`, field.value);
        if (doc.activeElement === field) focus = { component: component.dataset.recordComponent || '', attribute, id,
          start: field instanceof HTMLInputElement ? field.selectionStart : null, end: field instanceof HTMLInputElement ? field.selectionEnd : null };
      }
    }
  };
  const restore = () => {
    for (const component of components()) {
      for (const field of inputs(component)) {
        const { attribute, id } = fieldKey(field);
        const value = drafts.get(`${component.dataset.recordComponent}:${attribute}:${id}`);
        if (value !== undefined) field.value = value;
        if (focus && focus.component === component.dataset.recordComponent && focus.attribute === attribute && focus.id === id) {
          field.focus({ preventScroll: true });
          if (field instanceof HTMLInputElement && focus.start !== null && focus.end !== null && ['search', 'text'].includes(field.type)) field.setSelectionRange(focus.start, focus.end);
        }
      }
    }
    focus = null;
  };
  const mask = (message: string) => {
    for (const component of components()) {
      component.dataset.state = 'loading';
      component.setAttribute('aria-busy', 'true');
      // Keep the filter controls usable while no old total is presented as a
      // result for the newly submitted filters. Receipts clear with values.
      for (const child of Array.from(component.children)) {
        if (!child.matches('h2,form[data-record-filters]')) child.remove();
      }
      const status = doc.createElement('p'); status.className = 'state'; status.setAttribute('role', 'status'); status.textContent = message; component.append(status);
    }
  };
  const refresh = async () => {
    if (disposed || suspended || !components().length) return;
    const token = ++generation;
    request?.abort(); request = new AbortController();
    const ids = new Set(components().map(component => component.dataset.recordComponent || ''));
    const atEpoch = epoch;
    capture(); mask('Updating record views…');
    try {
      const response = await fetch(endpoint, { method: 'POST', credentials: 'same-origin', cache: 'no-store', signal: AbortSignal.any([request.signal, AbortSignal.timeout(15000)]),
        headers: { 'content-type': 'application/json', accept: 'application/json' }, body: JSON.stringify({ filter_epoch: atEpoch, filters: active }) });
      if (disposed || suspended || token !== generation) return;
      if (!response.ok) throw new Error(response.status === 404 ? 'denied' : 'unavailable');
      const body: unknown = await response.json();
      if (disposed || suspended || token !== generation) return;
      if (!matchingBatch(body, atEpoch, ids)) throw new Error('invalid');
      // Construct all replacements before changing any visible component.
      const rendered = renderRecordComponents(body.components, doc);
      const replacements = new Map(Array.from(rendered.querySelectorAll<HTMLElement>('[data-record-component]'), node => [node.dataset.recordComponent, node]));
      capture();
      for (const current of components()) {
        const replacement = replacements.get(current.dataset.recordComponent);
        if (replacement) current.replaceWith(replacement);
      }
      restore();
      enable();
    } catch (error) {
      if (disposed || suspended || token !== generation) return;
      const denied = error instanceof Error && error.message === 'denied';
      mask(denied ? 'This app is no longer available.' : 'Record views could not be refreshed. Apply filters again to retry.');
      for (const component of components()) {
        component.dataset.state = denied ? 'denied' : 'failed';
        component.removeAttribute('aria-busy');
        if (denied) for (const button of component.querySelectorAll<HTMLButtonElement>('[data-record-filter-enable]')) button.disabled = true;
      }
      if (denied) {
        suspended = true;
        root.dispatchEvent(new CustomEvent('forma:record-components-unavailable', { bubbles: true }));
      }
    }
  };
  root.addEventListener('submit', event => {
    const form = event.target;
    if (!(form instanceof HTMLFormElement) || !form.matches('[data-record-filters]') || !belongs(form)) return;
    event.preventDefault();
    if (disposed || suspended) return;
    const next = new Map(active.map(filter => [filter.id, filter]));
    for (const fieldset of form.querySelectorAll<HTMLElement>('[data-record-filter]')) {
      const id = fieldset.dataset.recordFilter || '';
      if (!ID.test(id)) continue;
      const value = fieldset.querySelector<HTMLInputElement | HTMLSelectElement>('[data-filter-value]');
      const start = fieldset.querySelector<HTMLInputElement>('[data-filter-start]');
      const end = fieldset.querySelector<HTMLInputElement>('[data-filter-end]');
      if (value?.value) next.set(id, { id, value: value.value });
      else if (start?.value || end?.value) next.set(id, { id, start: start?.value || '', end: end?.value || '' });
      else next.delete(id);
    }
    active = [...next.values()]; epoch++;
    for (const component of components()) for (const field of inputs(component)) {
      const { attribute, id } = fieldKey(field);
      const filter = next.get(id);
      field.value = !filter ? '' : attribute === 'data-filter-value' && 'value' in filter ? filter.value
        : attribute === 'data-filter-start' && 'start' in filter ? filter.start : attribute === 'data-filter-end' && 'end' in filter ? filter.end : '';
    }
    capture(); void refresh();
  }, { signal: listeners.signal });
  root.addEventListener('click', event => {
    if (!(event.target instanceof Element)) return;
    const reset = event.target.closest<HTMLElement>('[data-record-filter-reset]');
    if (!reset || !belongs(reset)) return;
    event.preventDefault();
    if (disposed || suspended) return;
    active = []; epoch++; drafts.clear(); focus = null;
    for (const component of components()) for (const field of inputs(component)) field.value = '';
    void refresh();
  }, { signal: listeners.signal });
  enable();
  return {
    refresh,
    beforeRefresh() { capture(); request?.abort(); generation++; },
    afterRefresh() {
      if (disposed) return;
      suspended = false; restore(); enable();
      // A regular live/native refresh has an unfiltered snapshot. Do not let
      // it flash underneath active filters; fetch the current scope again.
      if (epoch > 0) void refresh();
    },
    invalidate() {
      suspended = true; request?.abort(); generation++; drafts.clear(); focus = null;
      // The parent authority controller removes all protected app content,
      // including dialogs. Clearing values here fences its outstanding read.
      mask('This app is not available.');
    },
    dispose() { disposed = true; suspended = true; generation++; request?.abort(); listeners.abort(); drafts.clear(); focus = null; },
  };
}
