// APP-141: create an app before it reaches the canvas. The empty-canvas
// "Create an app" choice opens this modal. The server supplies a catalog of
// admitted starting definitions. Confirm instantiates or customizes one.
// Cancel places nothing.
import { submitIntent } from './compose';
import { el } from './render';

export type TemplateList = 'industries' | 'categories' | 'favorites';

export type AppStarter = {
  id: string;
  title: string;
  industry: string;
  category: string;
  description: string;
};

export function templateCatalog(body: unknown): AppStarter[] {
  const raw = (body as { templates?: unknown } | null)?.templates;
  if (!Array.isArray(raw) || raw.length > 24) return [];
  const ids = new Set<string>();
  const valid = raw.every((item): item is AppStarter => {
    if (!item || typeof item !== 'object') return false;
    const value = item as Record<string, unknown>;
    if (typeof value.id !== 'string' || !/^[a-z][a-z0-9-]{0,47}$/.test(value.id) || ids.has(value.id)) return false;
    ids.add(value.id);
    return ['title', 'industry', 'category', 'description'].every((key) => typeof value[key] === 'string' && (value[key] as string).length > 0 && (value[key] as string).length <= 500);
  });
  return valid ? raw as AppStarter[] : [];
}

const favoritesKey = (member: string): string | null => /^[0-9a-f]{16}$/.test(member)
  ? `forma.canvas.create-favorites.v1:${member}` : null;

export function startersIn(catalog: readonly AppStarter[], list: TemplateList, favoriteIds: readonly string[], group = ''): AppStarter[] {
  if (list === 'favorites') return catalog.filter((template) => favoriteIds.includes(template.id));
  return catalog.filter((template) => !group || template[list === 'industries' ? 'industry' : 'category'] === group);
}

export function toggleFavorite(ids: readonly string[], id: string): string[] {
  return ids.includes(id) ? ids.filter((item) => item !== id) : [...ids, id];
}

export function readFavorites(member: string): string[] {
  const key = favoritesKey(member);
  if (!key) return [];
  try {
    const raw = localStorage.getItem(key);
    const parsed = raw ? JSON.parse(raw) as unknown : [];
    return Array.isArray(parsed) ? parsed.filter((item): item is string => typeof item === 'string' && /^[a-z][a-z0-9-]{0,47}$/.test(item)).slice(0, 24) : [];
  } catch {
    return [];
  }
}

export function writeFavorites(member: string, ids: readonly string[]): void {
  const key = favoritesKey(member);
  if (!key) return;
  try {
    localStorage.setItem(key, JSON.stringify(ids));
  } catch {
    // A browser preference: unavailable storage must not interrupt creation.
  }
}

/** Opens the create-app modal. Confirm is the only path that calls compose. */
export function openCreateApp(document_: Document): void {
  document_.getElementById('cv-create-app')?.remove();
  let list: TemplateList = 'industries';
  const member = document_.getElementById('cv-canvas')?.dataset.member ?? '';
  let favorites = readFavorites(member);
  let chosen: AppStarter | null = null;
  let catalog: AppStarter[] = [];
  let group = '';
  let loading = true;
  const groups = el(document_, 'div', { class: 'cv-create-groups', 'aria-label': 'Template groups' });
  const status = el(document_, 'p', { class: 'cv-create-empty', role: 'status' }, 'Loading templates…');

  const lists = el(document_, 'div', { class: 'cv-create-templates', role: 'list' });
  const describe = el(document_, 'label', { class: 'cv-create-describe' }, 'Describe your app');
  const text = el(document_, 'textarea', { class: 'cv-create-text', rows: '4', minlength: '3', maxlength: '500', required: '' }) as HTMLTextAreaElement;
  describe.append(text);
  describe.hidden = true;
  const confirm = el(document_, 'button', { type: 'submit', class: 'cv-create-confirm' }, 'Create') as HTMLButtonElement;
  confirm.hidden = true;

  const paint = (): void => {
    lists.replaceChildren();
    groups.replaceChildren();
    if (list !== 'favorites') {
      for (const name of ['', ...new Set(catalog.map((template) => template[list === 'industries' ? 'industry' : 'category']))]) {
        const button = el(document_, 'button', { type: 'button', class: 'cv-create-group', 'aria-pressed': name === group ? 'true' : 'false' }, name || 'All');
        button.addEventListener('click', () => { group = name; paint(); });
        groups.append(button);
      }
    }
    for (const template of startersIn(catalog, list, favorites, group)) {
      const pick = el(document_, 'button', { type: 'button', class: 'cv-create-template', 'data-template': template.id, 'aria-pressed': chosen?.id === template.id ? 'true' : 'false' }, template.title);
      const star = el(document_, 'button', { type: 'button', class: 'cv-create-star', 'aria-pressed': favorites.includes(template.id) ? 'true' : 'false', 'aria-label': `Favorite ${template.title}` }, favorites.includes(template.id) ? '★' : '☆');
      star.addEventListener('click', (event) => {
        event.stopPropagation();
        favorites = toggleFavorite(favorites, template.id);
        writeFavorites(member, favorites);
        paint();
      });
      pick.addEventListener('click', () => {
        chosen = template;
        text.value = template.description;
        describe.hidden = false;
        confirm.hidden = false;
        paint();
        text.focus();
      });
      lists.append(el(document_, 'div', { class: 'cv-create-row', role: 'listitem' }, pick, el(document_, 'span', { class: 'cv-create-blurb' }, list === 'categories' ? template.category : template.industry), star));
    }
    if (!lists.childElementCount && !loading && catalog.length) lists.append(el(document_, 'p', { class: 'cv-create-empty' }, 'No templates here yet. Star a template to keep it in Favorites.'));
  };

  const tabs = el(document_, 'div', { class: 'cv-create-tabs', role: 'tablist' });
  for (const [id, label] of [['industries', 'Industries'], ['categories', 'Categories'], ['favorites', 'Favorites']] as const) {
    const tab = el(document_, 'button', { type: 'button', role: 'tab', 'data-create-list': id, 'aria-selected': id === list ? 'true' : 'false' }, label);
    tab.addEventListener('click', () => {
      list = id;
      group = '';
      for (const button of tabs.querySelectorAll('button')) button.setAttribute('aria-selected', button === tab ? 'true' : 'false');
      paint();
    });
    tabs.append(tab);
  }

  const dialog = el(document_, 'dialog', { id: 'cv-create-app', class: 'cv-create', 'aria-labelledby': 'cv-create-title' },
    el(document_, 'form', { class: 'cv-create-form', method: 'dialog' },
      el(document_, 'h2', { id: 'cv-create-title' }, 'Create an app'),
      el(document_, 'p', { class: 'cv-create-lead' }, 'Pick a template, then describe what you need. Keep its description to use it as-is, or edit it to customize your app.'),
      tabs,
      groups,
      status,
      lists,
      describe,
      el(document_, 'div', { class: 'cv-create-actions' },
        el(document_, 'button', { type: 'button', class: 'cv-create-cancel', 'data-create-cancel': '' }, 'Cancel'),
        confirm)));
  dialog.addEventListener('close', () => dialog.remove());
  dialog.addEventListener('cancel', () => dialog.close());
  dialog.querySelector('[data-create-cancel]')?.addEventListener('click', () => dialog.close());
  dialog.querySelector('form')?.addEventListener('submit', (event) => {
    event.preventDefault();
    const sentence = text.value.trim();
    if (!chosen || sentence.length < 3 || sentence.length > 500) return;
    dialog.close();
    const report = (message: string): void => {
      const live = document_.getElementById('cv-activity');
      if (live) live.textContent = message;
    };
    // The selected server template is part of the request, never an example
    // silently replaced by a matching existing work.
    void submitIntent(sentence, report, { template_id: chosen.id });
  });
  document_.body.append(dialog);
  paint();
  if (typeof dialog.showModal === 'function') dialog.showModal();
  else dialog.setAttribute('open', '');
  void fetch('/v1/templates', { credentials: 'same-origin', cache: 'no-store' })
    .then(async (response) => { if (!response.ok) throw new Error('unavailable'); return response.json() as Promise<unknown>; })
    .then((body) => {
      catalog = templateCatalog(body);
      if (!catalog.length) throw new Error('unavailable');
      loading = false;
      status.textContent = '';
      status.hidden = true;
      paint();
    }).catch(() => {
      loading = false;
      status.textContent = 'Templates could not be loaded. Close this dialog and try again. Nothing was created.';
      paint();
    });
}

/** Both first-run and persistent Create controls open the same admitted catalog. */
export function startCreateApp(document_: Document): void {
  for (const button of document_.querySelectorAll<HTMLElement>('[data-empty-describe], [data-create-app]')) {
    if (button.dataset.createWired) continue;
    button.dataset.createWired = 'true';
    button.addEventListener('click', (event) => {
      event.preventDefault();
      openCreateApp(document_);
    });
  }
}
