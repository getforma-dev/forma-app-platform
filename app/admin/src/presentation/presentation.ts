/** Admitted presentation is data. These closed tokens never become HTML, CSS or URLs. */
export type AppAction = { id: string; label: string; behavior: 'navigate' | 'dialog'; target: string };
export type Presentation = { version?: 1; description?: string; sections?: { id: string; title?: string; components: string[] }[]; actions?: AppAction[]; theme?: { mode?: string; accent?: string; density?: string; font?: string } };
export type PresentationProps = { hasDescription: boolean; description: string; mode: string; accent: string; density: string; font: string; actions: { id: string; label: string; behavior: string; target: string }[]; json: string };
const modes = ['default', 'light', 'dark'];
const accents = ['default', 'green', 'crimson', 'amber', 'cyan', 'violet', 'rose', 'ice', 'slate'];
const densities = ['default', 'compact', 'comfortable', 'spacious'];
const fonts = ['default', 'mono'];
const id = (value: unknown): value is string => typeof value === 'string' && /^[a-z][a-z0-9_]{0,63}$/.test(value);
const text = (value: unknown, max: number): value is string => typeof value === 'string' && Array.from(value).length <= max;
const VISIBILITY_PREFIX = 'forma.app.hidden.v1:';
const uuid = (value: string) => /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(value);
function visibilityKey(app: string, work: string): string | null { return uuid(app) && uuid(work) ? `${VISIBILITY_PREFIX}${app}/${work}` : null; }
function visibilityStorage(): Storage | undefined { try { return globalThis.localStorage; } catch { return undefined; } }
/** Appearance only: exact work in this browser. Never an access or publication rule. */
export function hiddenAppComponents(app: string, work: string, storage: Pick<Storage, 'getItem'> | undefined = visibilityStorage()): Set<string> {
  const key = visibilityKey(app, work); if (!key || !storage) return new Set();
  try {
    const raw = storage.getItem(key); if (!raw || raw.length > 8192) return new Set();
    const ids: unknown = JSON.parse(raw);
    return Array.isArray(ids) && ids.length <= 64 && ids.every(id) ? new Set(ids) : new Set();
  } catch { return new Set(); }
}
export function appComponentVisible(app: string, work: string, card: Pick<HTMLElement, 'dataset'>): boolean {
  const component = card.dataset.component;
  return !component || !hiddenAppComponents(app, work).has(component);
}
/** Stores component IDs only; a storage failure does not pretend the preference was saved. */
export function saveHiddenAppComponents(app: string, work: string, ids: readonly string[], document_?: Document,
  storage: Pick<Storage, 'setItem'> | undefined = visibilityStorage()): boolean {
  const key = visibilityKey(app, work);
  if (!key || !storage || ids.length > 64 || !ids.every(id)) return false;
  try { storage.setItem(key, JSON.stringify([...new Set(ids)].sort())); } catch { return false; }
  document_?.dispatchEvent(new CustomEvent('forma:app-view-preferences', { detail: { app, work } }));
  return true;
}
export function parsePresentation(value: unknown): Presentation {
  try {
    const p = typeof value === 'string' ? JSON.parse(value) : value;
    if (!p || typeof p !== 'object' || Array.isArray(p) || (p.version !== undefined && p.version !== 1)) return {};
    if (Object.keys(p).some(key => !['version', 'description', 'sections', 'actions', 'theme'].includes(key))) return {};
    if (Object.keys(p).length && p.version !== 1) return {};
    if (p.description !== undefined && !text(p.description, 1000)) return {};
    const seen = new Set<string>(); const sectionIds = new Set<string>(); const actionIds = new Set<string>();
    if (p.sections !== undefined && (!Array.isArray(p.sections) || p.sections.length > 8 || p.sections.some((s: Presentation['sections'] extends (infer T)[] | undefined ? T : never) => {
      if (!s || Object.keys(s).some(key => !['id','title','components'].includes(key)) || !id(s.id) || sectionIds.has(s.id) || (s.title !== undefined && !text(s.title, 120)) || !Array.isArray(s.components) || !s.components.length || s.components.length > 24) return true;
      sectionIds.add(s.id);
      return s.components.some(component => { if (!id(component) || seen.has(component)) return true; seen.add(component); return false; });
    }))) return {};
    if (p.actions !== undefined && (!Array.isArray(p.actions) || p.actions.length > 8 || p.actions.some((a: AppAction) => {
      if (!a || Object.keys(a).some(key => !['id','label','behavior','target'].includes(key)) || !id(a.id) || actionIds.has(a.id) || !text(a.label, 80) || !a.label.trim() || !id(a.target) || !['navigate', 'dialog'].includes(a.behavior)) return true;
      actionIds.add(a.id); return false;
    }))) return {};
    const t = p.theme;
    if (t !== undefined && (!t || Array.isArray(t) || typeof t !== 'object' || Object.keys(t).some(key => !['mode', 'accent', 'density', 'font'].includes(key)) ||
      (t.mode !== undefined && !modes.includes(t.mode)) || (t.accent !== undefined && !accents.includes(t.accent)) ||
      (t.density !== undefined && !densities.includes(t.density)) || (t.font !== undefined && !fonts.includes(t.font)))) return {};
    return p as Presentation;
  } catch { return {}; }
}
export function applyPresentationTheme(root: HTMLElement, raw: string): void {
  const definition = parsePresentation(raw);
  // Legacy apps receive default rendering tokens too. Only an admitted v1
  // presentation owns an authored design that browser-local Shape must preserve.
  root.dataset.appPresentation = definition.version === 1 ? 'authored' : 'default';
  if (definition.version === 1) {
    delete root.dataset.theme;
    delete root.dataset.themeDensity;
    delete root.dataset.themePreview;
    root.querySelector(':scope > .cv-unite-head > .cv-shaped-label[data-app-theme], :scope > h2 > .cv-shaped-label[data-app-theme], :scope > .cv-card-head > .cv-shaped-label[data-app-theme]')?.remove();
  }
  root.dataset.appMode = definition.theme?.mode ?? 'default'; root.dataset.appAccent = definition.theme?.accent ?? 'default';
  root.dataset.appDensity = definition.theme?.density ?? 'default'; root.dataset.appFont = definition.theme?.font ?? 'default';
  root.classList.add('composed-app');
}

function node(document: Document, tag: string, className = '', value = ''): HTMLElement {
  const result = document.createElement(tag); result.className = className; result.textContent = value; return result;
}
export type PresentationController = { beforeRefresh(): void; afterRefresh(value?: PresentationProps | string): void; invalidate(): void; dispose(): void };
const controllers = new WeakMap<HTMLElement, PresentationController>();
export function invalidatePresentation(root: HTMLElement): void {
  controllers.get(root)?.invalidate();
  root.dataset.presentation = '{}';
  root.querySelector('[data-app-presentation-header]')?.replaceChildren();
}
export function disposePresentation(root: HTMLElement): void { controllers.get(root)?.dispose(); }
export function presentationFor(root: HTMLElement): PresentationController {
  const existing = controllers.get(root); if (existing) return existing;
  const document = root.ownerDocument;
  let definition: Presentation = {};
  let components = new Map<string, HTMLElement>();
  let current: { action: string; id: string; node: HTMLElement; anchor: Comment; opener: HTMLElement | null } | null = null;
  let refreshing = false;
  let restoreFocus: HTMLElement | null = null;
  let dialog: HTMLDialogElement | null = null;
  let disposed = false;
  const memberView = () => !root.hasAttribute('data-shared-page') && !root.hasAttribute('data-connected-app')
    && (root.hasAttribute('data-document') || root.hasAttribute('data-unite'));
  const hidden = () => memberView() ? hiddenAppComponents(root.dataset.app ?? '', root.dataset.work ?? '') : new Set<string>();
  const localHidden = new Map<HTMLElement, boolean>();
  function clearLocalHidden(): void { for (const [component, wasHidden] of localHidden) { component.hidden = wasHidden; delete component.dataset.appLocalHidden; } localHidden.clear(); }
  function visibility(): void {
    clearLocalHidden();
    const excluded = hidden();
    if (current && excluded.has(current.id)) close();
    for (const [key, component] of components) if (excluded.has(key)) {
      localHidden.set(component, component.hidden); component.hidden = true; component.dataset.appLocalHidden = 'true';
    }
    for (const section of root.querySelectorAll<HTMLElement>('[data-app-section]')) {
      const members = [...section.querySelectorAll<HTMLElement>('[data-component]')];
      if (members.length && members.every(component => excluded.has(component.dataset.component ?? ''))) {
        localHidden.set(section, section.hidden); section.hidden = true; section.dataset.appLocalHidden = 'true';
      }
    }
  }
  function close(restore = true): void {
    const previous = current; current = null;
    if (previous) { if (previous.anchor.isConnected) previous.anchor.replaceWith(previous.node); else previous.node.remove(); }
    if (dialog?.open) dialog.close();
    dialog?.remove(); dialog = null;
    if (restore && previous?.opener?.isConnected) previous.opener.focus({ preventScroll: true });
  }
  function firstField(target: HTMLElement): HTMLElement | null {
    return target.querySelector<HTMLElement>('input:not([type="hidden"]):not(:disabled),select:not(:disabled),textarea:not(:disabled)') ?? target.querySelector<HTMLElement>('button:not(:disabled):not([data-focus-card]):not([data-card-remove]),a[href]');
  }
  function body(): HTMLElement | null { return dialog?.querySelector<HTMLElement>('[data-app-dialog-body]') ?? null; }
  function actions(): void {
    const host = root.querySelector<HTMLElement>('[data-app-presentation-header]');
    if (!host) return;
    const description = node(document, 'p', 'app-description', definition.description ?? '');
    description.hidden = !definition.description;
    const buttons = node(document, 'div', 'app-actions'); buttons.setAttribute('data-app-actions', '');
    for (const action of definition.actions ?? []) {
      // Missing/permission-filtered targets are never rebound to another part.
      if (!components.has(action.target) || hidden().has(action.target)) continue;
      const button = node(document, 'button', 'app-action', action.label) as HTMLButtonElement;
      button.type = 'button'; button.dataset.appAction = action.id;
      if (action.behavior === 'dialog') button.setAttribute('aria-haspopup', 'dialog');
      buttons.append(button);
    }
    host.replaceChildren(description, buttons);
  }
  function layout(): void {
    const container = root.querySelector<HTMLElement>('[data-document-parts], [data-presentation-layout]');
    if (!container) return;
    components = new Map();
    for (const component of container.querySelectorAll<HTMLElement>('[data-component]')) {
      if (component.parentElement?.closest('[data-component]')) continue;
      const key = component.dataset.component ?? '';
      if (id(key) && !components.has(key)) components.set(key, component);
    }
    // Flatten only our wrappers. This also permits a later publication to remove sections.
    for (const wrapper of Array.from(container.querySelectorAll<HTMLElement>(':scope > [data-app-section], :scope > [data-app-dialog-pool]'))) {
      for (const component of Array.from(wrapper.querySelectorAll<HTMLElement>('[data-component]')).filter(node => !node.parentElement?.closest('[data-component]'))) container.append(component);
      wrapper.remove();
    }
    if (definition.sections?.length) {
      const loose = Array.from(container.children);
      const assigned = new Set<string>();
      for (const section of definition.sections) {
        const wrapper = node(document, 'section', 'app-section'); wrapper.dataset.appSection = section.id;
        if (section.title) wrapper.append(node(document, 'h2', 'app-section-title', section.title));
        const grid = node(document, 'div', 'app-section-grid'); wrapper.append(grid);
        for (const key of section.components) { const component = components.get(key); if (component) { grid.append(component); assigned.add(key); } }
        if (grid.childElementCount) container.append(wrapper);
      }
      const pool = node(document, 'div'); pool.dataset.appDialogPool = ''; pool.hidden = true;
      for (const action of definition.actions ?? []) if (action.behavior === 'dialog' && !assigned.has(action.target)) {
        const component = components.get(action.target); if (component) pool.append(component);
      }
      if (pool.childElementCount) container.append(pool);
      for (const extra of loose) if (extra.parentElement === container) container.append(extra);
    }
  }
  function afterRefresh(value?: PresentationProps | string): void {
    if (disposed) return;
    const raw = typeof value === 'object' ? value.json : value ?? root.dataset.presentation ?? '{}';
    definition = parsePresentation(raw); root.dataset.presentation = typeof raw === 'string' ? raw : '{}';
    applyPresentationTheme(root, root.dataset.presentation);
    layout(); visibility(); actions();
    if (current) current.opener = Array.from(root.querySelectorAll<HTMLElement>('[data-app-action]')).find(button => button.dataset.appAction === current!.action) ?? null;
    if (current && refreshing) {
      const next = components.get(current.id);
      if (!next || !(definition.actions ?? []).some(action => action.id === current!.action && action.behavior === 'dialog' && action.target === current!.id)) close();
      else {
        const anchor = document.createComment('app component position'); next.replaceWith(anchor);
        current.node = next; current.anchor = anchor; body()?.append(next);
        if (restoreFocus?.isConnected && next.contains(restoreFocus)) restoreFocus.focus({ preventScroll: true });
        else (firstField(next) ?? dialog?.querySelector<HTMLElement>('.app-dialog-close'))?.focus({ preventScroll: true });
      }
    }
    refreshing = false; restoreFocus = null;
  }
  function beforeRefresh(): void {
    if (!current || refreshing) return;
    refreshing = true; restoreFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    if (current.anchor.isConnected) current.anchor.replaceWith(current.node);
  }
  const click = (event: Event) => {
    const button = (event.target as Element | null)?.closest<HTMLElement>('[data-app-action]');
    if (!button || !root.contains(button)) return;
    const action = definition.actions?.find(action => action.id === button.dataset.appAction);
    const target = action && components.get(action.target);
    if (!action || !target?.isConnected) return;
    event.preventDefault(); event.stopPropagation();
    if (action.behavior === 'navigate') {
      if (current) close(false);
      if (target.closest('[hidden]')) return;
      target.scrollIntoView({ block: 'nearest', behavior: 'instant' });
      if (!target.hasAttribute('tabindex')) target.tabIndex = -1; target.focus({ preventScroll: true }); return;
    }
    if (current || document.querySelector('dialog[open]')) return;
    dialog = document.createElement('dialog'); dialog.className = 'app-component-dialog'; dialog.dataset.modalAbove = '';
    dialog.setAttribute('aria-label', action.label);
    const heading = node(document, 'header', 'app-dialog-head'); heading.append(node(document, 'h2', '', action.label));
    const dismiss = node(document, 'button', 'app-dialog-close', 'Close') as HTMLButtonElement; dismiss.type = 'button'; dismiss.addEventListener('click', () => close()); heading.append(dismiss);
    const content = node(document, 'div', 'app-dialog-body'); content.dataset.appDialogBody = '';
    const anchor = document.createComment('app component position'); target.replaceWith(anchor); content.append(target);
    current = { action: action.id, id: action.target, node: target, anchor, opener: button };
    dialog.append(heading, content); root.append(dialog);
    dialog.addEventListener('keydown', event => {
      // Native dialogs can advance Tab into browser chrome. Keep this app's
      // keyboard cycle inside its one live component, preserving field order.
      if (event.key === 'Tab' && dialog) {
        const stops = Array.from(dialog.querySelectorAll<HTMLElement>('button:not([disabled]),a[href],input:not([type="hidden"]):not([disabled]),select:not([disabled]),textarea:not([disabled]),summary,[tabindex]:not([tabindex="-1"])'))
          .filter(node => node.getClientRects().length > 0 && !node.closest('[hidden],[inert]'));
        const active = document.activeElement as HTMLElement | null;
        const first = stops[0]; const last = stops[stops.length - 1];
        if (!first || !last) { event.preventDefault(); dialog.focus(); }
        else if (!active || !stops.includes(active) || (event.shiftKey ? active === first : active === last)) {
          event.preventDefault(); (event.shiftKey ? last : first).focus({ preventScroll: true });
        }
      }
      event.stopPropagation();
    });
    dialog.addEventListener('cancel', event => { event.preventDefault(); close(); });
    dialog.addEventListener('close', () => { if (current) close(); });
    if (typeof dialog.showModal !== 'function') { close(); return; }
    dialog.showModal();
    (firstField(target) ?? dismiss).focus();
  };
  root.addEventListener('click', click);
  const preferences = (event: Event) => {
    const detail = (event as CustomEvent<{app?: string; work?: string}>).detail;
    if (detail && (detail.app !== root.dataset.app || detail.work !== root.dataset.work)) return;
    if (memberView() && !disposed) { visibility(); actions(); }
  };
  const storageChanged = (event: StorageEvent) => {
    if (event.key === null || event.key === visibilityKey(root.dataset.app ?? '', root.dataset.work ?? '')) preferences(event);
  };
  document.addEventListener?.('forma:app-view-preferences', preferences);
  document.defaultView?.addEventListener('storage', storageChanged);
  const controller: PresentationController = {
    beforeRefresh, afterRefresh,
    invalidate() { close(false); clearLocalHidden(); definition = {}; components.clear(); root.dataset.presentation = '{}'; root.querySelector('[data-app-presentation-header]')?.replaceChildren(); },
    dispose() { close(false); clearLocalHidden(); disposed = true; root.removeEventListener('click', click); document.removeEventListener?.('forma:app-view-preferences', preferences); document.defaultView?.removeEventListener('storage', storageChanged); controllers.delete(root); },
  };
  controllers.set(root, controller); afterRefresh(); return controller;
}
