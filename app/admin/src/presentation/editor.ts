import { isUuid } from '../canvas/ids';

function el<K extends keyof HTMLElementTagNameMap>(doc: Document, tag: K, attributes: Record<string, string> = {}, ...children: (Node | string)[]): HTMLElementTagNameMap[K] {
  const node = doc.createElement(tag);
  for (const [name, value] of Object.entries(attributes)) node.setAttribute(name, value);
  for (const child of children) node.append(typeof child === 'string' ? doc.createTextNode(child) : child);
  return node;
}

type Component = { id: string; title: string; kind: string };
type Section = { id: string; title?: string; components: string[] };
type Action = { id: string; label: string; behavior: 'dialog' | 'navigate'; target: string };
type Theme = { mode?: string; accent?: string; density?: string; font?: string };
type Presentation = { version: 1; description?: string; sections?: Section[]; actions?: Action[]; theme?: Theme };
export type EditorState = {
  publication_id: string; definition_digest: string; work_version: number;
  can_edit: boolean; presentation: Presentation | null; components: Component[];
};

/** A response is an editing premise, never permission granted by the browser. */
export function editorState(value: unknown): EditorState | null {
  if (!value || typeof value !== 'object') return null;
  const state = value as EditorState;
  if (!isUuid(state.publication_id) || typeof state.definition_digest !== 'string'
    || !Number.isSafeInteger(state.work_version) || state.work_version < 1 || typeof state.can_edit !== 'boolean'
    || !Array.isArray(state.components) || state.components.length > 64) return null;
  const ids = new Set<string>();
  for (const component of state.components) {
    if (!component || !/^[a-z][a-z0-9_]{0,63}$/.test(component.id) || ids.has(component.id)
      || typeof component.title !== 'string' || typeof component.kind !== 'string') return null;
    ids.add(component.id);
  }
  if (state.presentation !== null && (!state.presentation || state.presentation.version !== 1)) return null;
  return state;
}

export function presentationRequest(state: EditorState, presentation: Presentation) {
  return { expected_publication_id: state.publication_id, expected_definition_digest: state.definition_digest,
    expected_work_version: state.work_version, presentation };
}

function stable(value: unknown): string {
  if (Array.isArray(value)) return `[${value.map(stable).join(',')}]`;
  if (value && typeof value === 'object') return `{${Object.entries(value).sort(([a], [b]) => a.localeCompare(b)).map(([key, item]) => `${JSON.stringify(key)}:${stable(item)}`).join(',')}}`;
  return JSON.stringify(value);
}

/** Recover a lost save response only when a fresh authorized read proves the exact result. */
export function recoveredPresentation(value: unknown, requested: Presentation): EditorState | null {
  const state = editorState(value);
  return state && state.can_edit && stable(state.presentation) === stable(requested) ? state : null;
}

const wired = new WeakSet<Document>();
export function startPresentationEditor(document_: Document = document): void {
  if (wired.has(document_)) return;
  wired.add(document_);
  document_.addEventListener('click', event => {
    const target = event.target;
    if (!(target instanceof Element)) return;
    const button = target.closest<HTMLElement>('[data-edit-presentation]');
    if (!button) return;
    const scope = button.closest<HTMLElement>('[data-app][data-work]');
    const app = button.dataset.app || scope?.dataset.app || '';
    const work = button.dataset.work || scope?.dataset.work || '';
    if (!isUuid(app) || !isUuid(work)) return;
    event.preventDefault();
    void openPresentationEditor(document_, app, work, button);
  });
}

export async function openPresentationEditor(document_: Document, app: string, work: string, trigger?: HTMLElement): Promise<void> {
  if (!isUuid(app) || !isUuid(work) || document_.querySelector('[data-presentation-editor]')) return;
  const endpoint = `/v1/work/${app}/${work}/presentation`;
  const lifecycle = new AbortController();
  let disposed = false;
  let saving = false;
  let state: EditorState | null = null;
  let sections: Section[] = [];
  let actions: Action[] = [];
  let nextId = 0;
  const uniqueId = (prefix: string) => {
    let id: string;
    do { id = `${prefix}_${++nextId}`; } while (sections.some(item => item.id === id) || actions.some(item => item.id === id));
    return id;
  };
  const status = el(document_, 'p', { role: 'status', 'aria-live': 'polite', class: 'presentation-editor-status' }, 'Loading app settings…');
  const body = el(document_, 'div', { class: 'presentation-editor-body' });
  const save = el(document_, 'button', { type: 'submit', class: 'presentation-editor-save' }, 'Save app design');
  save.disabled = true;
  const cancel = el(document_, 'button', { type: 'button' }, 'Cancel');
  const form = el(document_, 'form', { class: 'presentation-editor-form' },
    el(document_, 'h2', { id: 'presentation-editor-title' }, 'Design your app'),
    el(document_, 'p', { class: 'presentation-editor-lead' }, 'Arrange components, choose a theme, and connect buttons to what they open.'),
    status, body,
    el(document_, 'p', { class: 'presentation-editor-note' }, 'Saving updates this app’s design. Existing shared links pause until you share the updated version. Unfinished forms keep their values and need a reload before saving.'),
    el(document_, 'footer', {}, cancel, save));
  const dialog = el(document_, 'dialog', { 'data-presentation-editor': '', class: 'presentation-editor', 'aria-labelledby': 'presentation-editor-title' }, form);
  const close = () => { if (!saving) dialog.close(); };
  cancel.addEventListener('click', close);
  dialog.addEventListener('cancel', event => { if (saving) event.preventDefault(); });
  dialog.addEventListener('close', () => {
    disposed = true; lifecycle.abort(); dialog.remove();
    if (trigger?.isConnected) trigger.focus({ preventScroll: true });
  });
  document_.body.append(dialog);
  dialog.showModal();

  const input = (name: string, value: string, maximum: number): HTMLInputElement => {
    const field = el(document_, 'input', { type: 'text', maxlength: String(maximum), 'aria-label': name });
    field.value = value;
    return field;
  };
  const select = (name: string, choices: [string, string][], value: string): HTMLSelectElement => {
    const field = el(document_, 'select', { 'aria-label': name });
    for (const [id, title] of choices) field.append(el(document_, 'option', { value: id }, title));
    field.value = value;
    return field;
  };
  const group = (label: string, field: HTMLElement) => el(document_, 'label', { class: 'presentation-editor-field' }, el(document_, 'span', {}, label), field);
  const description = el(document_, 'textarea', { rows: '3', maxlength: '1000', 'aria-label': 'App introduction' });
  const mode = select('Appearance', [['default', 'Forma'], ['light', 'Light'], ['dark', 'Dark']], 'default');
  const accent = select('Accent', ['default', 'green', 'crimson', 'amber', 'cyan', 'violet', 'rose', 'ice', 'slate'].map(value => [value, value === 'default' ? 'Forma' : value[0].toUpperCase() + value.slice(1)]), 'default');
  const density = select('Spacing', [['default', 'Default'], ['compact', 'Compact'], ['comfortable', 'Comfortable'], ['spacious', 'Spacious']], 'default');
  const font = select('Typography', [['default', 'Sans serif'], ['mono', 'Monospace']], 'default');
  const sectionRows = el(document_, 'div', { class: 'presentation-editor-sections' });
  const componentRows = el(document_, 'div', { class: 'presentation-editor-components' });
  const actionRows = el(document_, 'div', { class: 'presentation-editor-actions' });
  const addSection = el(document_, 'button', { type: 'button' }, 'Add section');
  const addAction = el(document_, 'button', { type: 'button' }, 'Add button');

  const renderComponents = () => {
    componentRows.replaceChildren();
    if (!state) return;
    const placed = sections.flatMap(section => section.components);
    const ordered = [...placed, ...state.components.filter(component => !placed.includes(component.id)).map(component => component.id)];
    for (const id of ordered) {
      const component = state.components.find(item => item.id === id);
      if (!component) continue;
      const section = sections.find(item => item.components.includes(id));
      const where = select(`Section for ${component.title}`, [...sections.map(item => [item.id, item.title || 'Untitled section'] as [string, string]), ['', 'Open from a button only']], section?.id || '');
      where.addEventListener('change', () => {
        for (const item of sections) item.components = item.components.filter(value => value !== id);
        sections.find(item => item.id === where.value)?.components.push(id);
        renderComponents();
      });
      const up = el(document_, 'button', { type: 'button', 'aria-label': `Move ${component.title} up` }, '↑');
      const down = el(document_, 'button', { type: 'button', 'aria-label': `Move ${component.title} down` }, '↓');
      const index = section?.components.indexOf(id) ?? -1;
      up.disabled = !section || index === 0; down.disabled = !section || index === section.components.length - 1;
      const move = (delta: number) => {
        if (!section || index < 0) return;
        [section.components[index], section.components[index + delta]] = [section.components[index + delta], section.components[index]];
        renderComponents();
        componentRows.querySelector<HTMLElement>(`[data-editor-component="${id}"] select`)?.focus();
      };
      up.addEventListener('click', () => move(-1)); down.addEventListener('click', () => move(1));
      componentRows.append(el(document_, 'div', { class: 'presentation-editor-component', 'data-editor-component': id }, el(document_, 'span', {}, component.title), where, up, down));
    }
  };
  const renderSections = () => {
    sectionRows.replaceChildren();
    sections.forEach((section, index) => {
      const title = input(`Section ${index + 1} heading`, section.title || '', 120);
      title.addEventListener('input', () => { section.title = title.value; });
      title.addEventListener('change', renderComponents);
      const remove = el(document_, 'button', { type: 'button', 'aria-label': `Remove section ${index + 1}` }, 'Remove');
      remove.disabled = sections.length === 1;
      remove.addEventListener('click', () => {
        sections = sections.filter(item => item !== section);
        sections[0].components.push(...section.components);
        renderSections(); renderComponents();
      });
      const up = el(document_, 'button', { type: 'button', 'aria-label': `Move section ${index + 1} up` }, '↑');
      up.disabled = index === 0;
      up.addEventListener('click', () => {
        [sections[index - 1], sections[index]] = [sections[index], sections[index - 1]];
        renderSections(); renderComponents();
      });
      sectionRows.append(el(document_, 'div', { class: 'presentation-editor-section' }, title, up, remove));
    });
    addSection.disabled = sections.length >= 8;
  };
  const renderActions = () => {
    actionRows.replaceChildren();
    actions.forEach((action, index) => {
      const label = input(`Button ${index + 1} label`, action.label, 80); label.required = true;
      label.addEventListener('input', () => { action.label = label.value; });
      const behavior = select(`Button ${index + 1} behavior`, [['dialog', 'Open a modal'], ['navigate', 'Go to component']], action.behavior);
      behavior.addEventListener('change', () => { action.behavior = behavior.value as Action['behavior']; });
      const target = select(`Button ${index + 1} component`, state?.components.map(component => [component.id, component.title]) || [], action.target);
      target.addEventListener('change', () => { action.target = target.value; });
      const remove = el(document_, 'button', { type: 'button', 'aria-label': `Remove button ${index + 1}` }, 'Remove');
      remove.addEventListener('click', () => { actions = actions.filter(item => item !== action); renderActions(); });
      actionRows.append(el(document_, 'fieldset', { class: 'presentation-editor-action' }, el(document_, 'legend', {}, `Button ${index + 1}`), group('Label', label), group('Behavior', behavior), group('Component', target), remove));
    });
    if (!actions.length) actionRows.append(el(document_, 'p', {}, 'No app buttons yet. Add one to open a component in a modal or move to it on the page.'));
    addAction.disabled = actions.length >= 8 || !state?.components.length;
  };
  addSection.addEventListener('click', () => {
    if (sections.length >= 8) return;
    sections.push({ id: uniqueId('section'), title: 'New section', components: [] });
    renderSections(); renderComponents();
  });
  addAction.addEventListener('click', () => {
    if (actions.length >= 8 || !state?.components.length) return;
    actions.push({ id: uniqueId('action'), label: 'Open details', behavior: 'dialog', target: state.components[0].id });
    renderActions();
  });
  const finish = () => {
    saving = false;
    document_.dispatchEvent(new CustomEvent('forma:presentation-saved', { detail: { app, work } }));
    dialog.close();
  };
  form.addEventListener('submit', async event => {
    event.preventDefault();
    if (!state?.can_edit || saving) return;
    const placed = new Set(sections.flatMap(section => section.components));
    const unreachable = state.components.find(component => !placed.has(component.id) && !actions.some(action => action.target === component.id && action.behavior === 'dialog'));
    if (unreachable) { status.textContent = `Place ${unreachable.title} in a section or connect a modal button to it.`; return; }
    if (actions.some(action => action.behavior === 'navigate' && !placed.has(action.target))) { status.textContent = 'A navigation button needs a component that is placed in a section.'; return; }
    const presentation: Presentation = { version: 1, description: description.value.trim(),
      sections: sections.filter(section => section.components.length).map(section => ({ ...section, title: section.title?.trim() || '' })),
      actions: actions.map(action => ({ ...action, label: action.label.trim() })),
      theme: { mode: mode.value, accent: accent.value, density: density.value, font: font.value } };
    saving = true;
    const disabledBefore = new Map(Array.from(form.querySelectorAll<HTMLInputElement | HTMLButtonElement | HTMLSelectElement | HTMLTextAreaElement>('input,button,select,textarea'), control => [control, control.disabled]));
    for (const control of disabledBefore.keys()) control.disabled = true;
    status.textContent = 'Saving app design…';
    try {
      const response = await fetch(endpoint, { method: 'POST', credentials: 'same-origin', cache: 'no-store', signal: AbortSignal.any([lifecycle.signal, AbortSignal.timeout(15000)]),
        headers: { 'content-type': 'application/json', accept: 'application/json' }, body: JSON.stringify(presentationRequest(state, presentation)) });
      if (disposed) return;
      const value: unknown = await response.json();
      if (response.ok && recoveredPresentation(value, presentation)) { finish(); return; }
      if (response.status === 404) { state = null; body.replaceChildren(); status.textContent = 'This app is no longer available to edit.'; return; }
      if (response.status === 409) { state = null; status.textContent = 'This app changed while you were editing. Close and reopen Design app to load the current version.'; return; }
      if (response.status === 400 || response.status === 422) throw new Error('invalid');
      throw new Error('uncertain');
    } catch (error) {
      if (disposed) return;
      if (error instanceof Error && error.message === 'invalid') {
        status.textContent = 'The design was not saved. Check section headings, button labels and component targets.';
      } else {
        // A failed network acknowledgement cannot become a second publication.
        try {
          const response = await fetch(endpoint, { credentials: 'same-origin', cache: 'no-store', signal: AbortSignal.any([lifecycle.signal, AbortSignal.timeout(15000)]), headers: { accept: 'application/json' } });
          if (disposed) return;
          if (response.ok && recoveredPresentation(await response.json(), presentation)) { finish(); return; }
        } catch { /* The original version premises still fence a retry. */ }
        status.textContent = 'The save could not be confirmed. Retry keeps the same version; if the app changed, close and reopen the editor.';
      }
    } finally {
      if (!disposed) {
        saving = false;
        for (const [control, disabled] of disabledBefore) control.disabled = disabled;
        save.disabled = !state?.can_edit;
        addSection.disabled = sections.length >= 8; addAction.disabled = actions.length >= 8 || !state?.components.length;
      }
    }
  });
  try {
    const response = await fetch(endpoint, { credentials: 'same-origin', cache: 'no-store', signal: AbortSignal.any([lifecycle.signal, AbortSignal.timeout(15000)]), headers: { accept: 'application/json' } });
    if (disposed) return;
    state = response.ok ? editorState(await response.json()) : null;
    if (!state || !state.can_edit) { status.textContent = 'The app owner can change this app’s design.'; return; }
    const existing = state.presentation;
    sections = existing?.sections?.length ? structuredClone(existing.sections) : [{ id: 'main', title: '', components: state.components.map(component => component.id) }];
    actions = structuredClone(existing?.actions || []);
    description.value = existing?.description || '';
    mode.value = existing?.theme?.mode || 'default'; accent.value = existing?.theme?.accent || 'default';
    density.value = existing?.theme?.density || 'default'; font.value = existing?.theme?.font || 'default';
    body.append(group('App introduction', description),
      el(document_, 'fieldset', { class: 'presentation-editor-theme' }, el(document_, 'legend', {}, 'Appearance'), group('Appearance', mode), group('Accent', accent), group('Spacing', density), group('Typography', font)),
      el(document_, 'h3', {}, 'Sections'), sectionRows, addSection,
      el(document_, 'h3', {}, 'Components'), componentRows,
      el(document_, 'h3', {}, 'App buttons'), actionRows, addAction);
    renderSections(); renderComponents(); renderActions();
    status.textContent = ''; save.disabled = false; description.focus();
  } catch {
    if (!disposed) status.textContent = 'App settings could not be loaded. Close and try again.';
  }
}
