// APP-141: create an app before it reaches the canvas. The empty-canvas
// "Create an app" choice opens this modal. A template comes first. The
// description is the second step and is the only thing sent to compose.
// Cancel places nothing.
import { submitIntent } from './compose';
import { el } from './render';

export type TemplateList = 'industries' | 'categories' | 'favorites';

export type AppTemplate = {
  id: string;
  list: Exclude<TemplateList, 'favorites'>;
  label: string;
  blurb: string;
  starter: string;
};

/** A fixed grouping we wrote. The library catalog has no industry field. */
export const TEMPLATES: AppTemplate[] = [
  { id: 'expense-claims', list: 'industries', label: 'Expense claims', blurb: 'Finance', starter: 'Track expense claims with a status, an amount, and who submitted them.' },
  { id: 'reading-list', list: 'industries', label: 'Reading list', blurb: 'Operations', starter: 'Keep a reading list with a title, an author, and whether it has been read.' },
  { id: 'kept-list', list: 'categories', label: 'A list you keep', blurb: 'Records', starter: 'Keep a list of items with a name and a status.' },
  { id: 'a-count', list: 'categories', label: 'A count', blurb: 'Numbers', starter: 'Count the records and show the total with a receipt.' },
];

const FAVORITES = 'cv-template-favorites';

export function templatesIn(list: TemplateList, favoriteIds: readonly string[]): AppTemplate[] {
  if (list === 'favorites') return TEMPLATES.filter((template) => favoriteIds.includes(template.id));
  return TEMPLATES.filter((template) => template.list === list);
}

export function toggleFavorite(ids: readonly string[], id: string): string[] {
  return ids.includes(id) ? ids.filter((item) => item !== id) : [...ids, id];
}

function readFavorites(): string[] {
  try {
    const raw = localStorage.getItem(FAVORITES);
    const parsed = raw ? JSON.parse(raw) as unknown : [];
    return Array.isArray(parsed) ? parsed.filter((item): item is string => typeof item === 'string' && TEMPLATES.some((template) => template.id === item)) : [];
  } catch {
    return [];
  }
}

function writeFavorites(ids: readonly string[]): void {
  localStorage.setItem(FAVORITES, JSON.stringify(ids));
}

/** Opens the create-app modal. Confirm is the only path that calls compose. */
export function openCreateApp(document_: Document): void {
  document_.getElementById('cv-create-app')?.remove();
  let list: TemplateList = 'industries';
  let favorites = readFavorites();
  let chosen: AppTemplate | null = null;

  const lists = el(document_, 'div', { class: 'cv-create-templates', role: 'list' });
  const describe = el(document_, 'label', { class: 'cv-create-describe' }, 'Describe it');
  const text = el(document_, 'textarea', { class: 'cv-create-text', rows: '4', maxlength: '500' }) as HTMLTextAreaElement;
  describe.append(text);
  describe.hidden = true;
  const confirm = el(document_, 'button', { type: 'submit', class: 'cv-create-confirm' }, 'Create') as HTMLButtonElement;
  confirm.hidden = true;

  const paint = (): void => {
    lists.replaceChildren();
    for (const template of templatesIn(list, favorites)) {
      const pick = el(document_, 'button', { type: 'button', class: 'cv-create-template', 'data-template': template.id }, template.label);
      const star = el(document_, 'button', { type: 'button', class: 'cv-create-star', 'aria-pressed': favorites.includes(template.id) ? 'true' : 'false', 'aria-label': `Favorite ${template.label}` }, favorites.includes(template.id) ? '★' : '☆');
      star.addEventListener('click', (event) => {
        event.stopPropagation();
        favorites = toggleFavorite(favorites, template.id);
        writeFavorites(favorites);
        paint();
      });
      pick.addEventListener('click', () => {
        chosen = template;
        text.value = template.starter;
        describe.hidden = false;
        confirm.hidden = false;
        text.focus();
      });
      lists.append(el(document_, 'div', { class: 'cv-create-row', role: 'listitem' }, pick, el(document_, 'span', { class: 'cv-create-blurb' }, template.blurb), star));
    }
    if (!lists.childElementCount) lists.append(el(document_, 'p', { class: 'cv-create-empty' }, 'No favorites yet. Star a template to keep it here.'));
  };

  const tabs = el(document_, 'div', { class: 'cv-create-tabs', role: 'tablist' });
  for (const [id, label] of [['industries', 'Industries'], ['categories', 'Categories'], ['favorites', 'Favorites']] as const) {
    const tab = el(document_, 'button', { type: 'button', role: 'tab', 'data-create-list': id, 'aria-selected': id === list ? 'true' : 'false' }, label);
    tab.addEventListener('click', () => {
      list = id;
      for (const button of tabs.querySelectorAll('button')) button.setAttribute('aria-selected', button === tab ? 'true' : 'false');
      paint();
    });
    tabs.append(tab);
  }

  const dialog = el(document_, 'dialog', { id: 'cv-create-app', class: 'cv-create', 'aria-labelledby': 'cv-create-title' },
    el(document_, 'form', { class: 'cv-create-form', method: 'dialog' },
      el(document_, 'h2', { id: 'cv-create-title' }, 'Create an app'),
      el(document_, 'p', { class: 'cv-create-lead' }, 'Pick a template. Then describe it. Nothing is placed until you create it.'),
      tabs,
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
    if (!chosen || !sentence) return;
    dialog.close();
    const report = (message: string): void => {
      const live = document_.getElementById('cv-activity');
      if (live) live.textContent = message;
    };
    void submitIntent(sentence, report);
  });
  document_.body.append(dialog);
  paint();
  if (typeof dialog.showModal === 'function') dialog.showModal();
  else dialog.setAttribute('open', '');
}

/** The empty canvas's create choice opens the modal instead of the intent bar. */
export function startCreateApp(document_: Document): void {
  const button = document_.querySelector<HTMLElement>('[data-empty-describe]');
  if (!button || button.dataset.createWired) return;
  button.dataset.createWired = 'true';
  button.addEventListener('click', (event) => {
    event.preventDefault();
    openCreateApp(document_);
  });
}
