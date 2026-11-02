// An app owns one attached companion, with lazy, independently cancelled views.
import { el, isUuid } from './render';
import { appWindowIcon } from './app-window';
import { mountAppSummary } from './explain';
import { mountAppSources } from './app-sources';
import { mountAppChat } from './app-chat';
import { mountAppSavedAnswers, renderAppAnswerActions } from './app-saved-answers';
import type { RecordScope } from '../ask/client';

export type AppAssistantTab = 'ask' | 'summary' | 'sources' | 'saved';
type View = {show(): void; hide(): void; dispose(): void};
type Options = {app: string; work: string; title: string; address?: string; onOpenActions?: () => void; onOpenApp?: () => void; onDataCommitted?: (recordId: string) => void; onDataEnabled?: () => Promise<boolean>};
type Companion = {host: HTMLElement; options: Options; open: boolean; tab: AppAssistantTab; select(tab: AppAssistantTab): void; close(): void; dispose(): void; chat: ReturnType<typeof mountAppChat>};
const companions = new Map<string, Companion>();
const names: Record<AppAssistantTab, string> = {ask: 'Ask', summary: 'Summary', sources: 'Sources', saved: 'Saved'};
const tabs = Object.keys(names) as AppAssistantTab[];
const keyOf = (app: string, work: string) => `${app}/${work}`;

function mount(host: HTMLElement, options: Options): Companion {
  const doc = host.ownerDocument;
  const panels = {} as Record<AppAssistantTab, HTMLElement>;
  const buttons = {} as Record<AppAssistantTab, HTMLElement>;
  const views: Partial<Record<AppAssistantTab, View>> = {};
  const current = () => companions.get(keyOf(options.app, options.work)) === entry && host.isConnected;
  const emit = () => {
    const root = host.closest('.cv-unite');
    for (const button of root?.querySelectorAll<HTMLElement>('[data-app-assistant]') ?? [])
      button.setAttribute('aria-expanded', String(entry.open));
    host.dispatchEvent(new CustomEvent('forma:app-companion', {bubbles: true, detail: {app: options.app, work: options.work, open: entry.open, tab: entry.tab}}));
  };
  const chatHost = el(doc, 'div', {class: 'cv-app-assistant-view cv-app-chat-host'});
  const chat = mountAppChat(chatHost, {...options,
    onOpenSource: (fileId: string) => {
      if (!current() || !entry.open) return;
      entry.select('sources'); (views.sources as ReturnType<typeof mountAppSources>).openFile(fileId);
    },
    renderAnswerActions: snapshot => renderAppAnswerActions(doc, snapshot, {...options}),
  });
  views.ask = chat;
  const close = el(doc, 'button', {type: 'button', class: 'cv-window-tool', 'aria-label': 'Close app panel', title: 'Close app panel'}, appWindowIcon(doc, 'close'));
  close.addEventListener('click', () => entry.close());
  const tablist = el(doc, 'div', {class: 'cv-app-assistant-tabs', role: 'tablist', 'aria-label': 'App assistant views'});
  const body = el(doc, 'div', {class: 'cv-app-assistant-body'});
  for (const tab of tabs) {
    const id = `assistant-${options.work}-${tab}`;
    const button = el(doc, 'button', {type: 'button', id: `${id}-tab`, role: 'tab', 'aria-controls': id, 'aria-selected': 'false', tabindex: '-1'}, names[tab]);
    button.addEventListener('click', () => entry.select(tab));
    const panel = tab === 'ask' ? chatHost : el(doc, 'div', {class: `cv-app-assistant-view cv-app-${tab}-host`});
    panel.id = id; panel.setAttribute('role', 'tabpanel'); panel.setAttribute('aria-labelledby', `${id}-tab`); panel.hidden = true; panel.inert = true;
    buttons[tab] = button; panels[tab] = panel; tablist.append(button); body.append(panel);
  }
  tablist.addEventListener('keydown', event => {
    const key = (event as KeyboardEvent).key, at = tabs.indexOf(entry.tab);
    const next = key === 'ArrowRight' ? (at + 1) % tabs.length : key === 'ArrowLeft' ? (at + tabs.length - 1) % tabs.length : key === 'Home' ? 0 : key === 'End' ? tabs.length - 1 : -1;
    if (next < 0) return;
    event.preventDefault(); event.stopPropagation(); entry.select(tabs[next]!); buttons[tabs[next]!]!.focus({preventScroll: true});
  });
  const onKey = (event: KeyboardEvent) => {
    if (event.key === 'Escape' && !event.defaultPrevented) {event.preventDefault(); event.stopPropagation(); entry.close();}
  };
  host.addEventListener('keydown', onKey);
  host.replaceChildren(el(doc, 'header', {class: 'cv-app-assistant-head'},
    el(doc, 'div', {}, el(doc, 'h3', {}, 'App assistant'), el(doc, 'p', {title: options.title}, options.title)), close), tablist, body);
  const entry: Companion = {
    host, options, open: false, tab: 'ask', chat,
    select(tab) {
      if (!current()) return;
      if (entry.open && entry.tab !== tab) views[entry.tab]?.hide();
      entry.tab = tab; entry.open = true; host.hidden = false; host.inert = false;
      for (const name of tabs) {
        const selected = name === tab;
        panels[name].hidden = !selected; panels[name].inert = !selected;
        buttons[name].setAttribute('aria-selected', String(selected)); buttons[name].setAttribute('tabindex', selected ? '0' : '-1');
      }
      views[tab] ??= tab === 'summary' ? mountAppSummary(panels[tab], options)
        : tab === 'sources' ? mountAppSources(panels[tab], {...options, onAskFile: source => {
          if (!current() || !entry.open) return;
          chat.setSource(source.file.file_id); entry.select('ask'); chat.focus();
        }}) : mountAppSavedAnswers(panels[tab], options);
      views[tab]!.show(); emit();
    },
    close() {
      if (!current() || !entry.open) return;
      views[entry.tab]?.hide(); entry.open = false; host.hidden = true; host.inert = true; emit();
      const root = host.closest('.cv-unite');
      root?.querySelector<HTMLElement>('[data-app-assistant]')?.focus({preventScroll: true});
    },
    dispose() {
      entry.open = false; for (const view of Object.values(views)) view?.dispose();
      host.removeEventListener('keydown', onKey); host.replaceChildren(); host.hidden = true; host.inert = true;
    },
  };
  return entry;
}

export function showAppAssistant(host: HTMLElement, options: Options, tab: AppAssistantTab | undefined, request: {toggle?: boolean; question?: string; scope?: RecordScope | null} = {}): void {
  if (!host.isConnected || !isUuid(options.app) || !isUuid(options.work) || (tab !== undefined && !tabs.includes(tab))) return;
  const key = keyOf(options.app, options.work);
  let entry = companions.get(key);
  if (entry?.host !== host) {entry?.dispose(); companions.delete(key); entry = undefined;}
  if (!entry) {entry = mount(host, options); companions.set(key, entry);}
  if (request.toggle && entry.open) {entry.close(); return;}
  if (tab === 'ask') entry.chat.setRecordScope(request.scope ?? null);
  const selected = tab ?? entry.tab;
  entry.select(selected);
  if (selected === 'ask') {if (request.question?.trim()) entry.chat.ask(request.question); else entry.chat.focus();}
}
export function disposeAppAssistant(app: string, work: string): void {
  const key = keyOf(app, work), entry = companions.get(key);
  entry?.dispose(); companions.delete(key);
}
