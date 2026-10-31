// APP-119: the right-hand pane, one pane with three tabs: Library (the apps
// and cards you can bring in, `library.ts`), Commands (the verbs with their
// arguments and examples, `command-sheet.ts`) and Shortcuts (every action
// with its click path and keys, `shortcut-tab.ts`). `L` and `/apps` open it
// on Library; `/commands` and `/help` on Commands; `?` (on the canvas or in
// the empty bar) and `/shortcuts` on Shortcuts. One × and Esc close it;
// focus returns to what opened it. The tabs follow the
// ARIA tabs pattern: ←/→, Home and End move between them (automatic
// activation), the selected tab is the only one in the Tab order. At phone
// width the pane is the bottom sheet it always was, with the tabs on top.
// The markup is the page's (`app.tsx`, `#cv-library`); this module only
// wires it.

export type PaneTab = 'library' | 'commands' | 'shortcuts';
export const PANE_TABS: readonly PaneTab[] = ['library', 'commands', 'shortcuts'];

type TabHooks = {
  /** The tab became visible (`focus`: move focus into it). */
  show?: (focus: boolean) => void;
  /** Esc inside the tab: true when the tab handled it (cleared its search). */
  escape?: () => boolean;
};

let documentRef: Document | null = null;
let pane: HTMLElement | null = null;
let toggle: HTMLElement | null = null;
let lastFocus: HTMLElement | null = null;
const hooks = new Map<PaneTab, TabHooks>();

const tabButton = (tab: PaneTab) => pane?.querySelector<HTMLElement>(`[data-pane-tab="${tab}"]`) ?? null;
const panel = (tab: PaneTab) => documentRef?.getElementById(`cv-panel-${tab}`) ?? null;

export function isPaneOpen(): boolean {
  return pane?.dataset.open === 'true';
}

/** The tab the pane shows (or will show when opened). */
export function paneTab(): PaneTab {
  const tab = pane?.dataset.tab as PaneTab | undefined;
  return tab && PANE_TABS.includes(tab) ? tab : 'library';
}

/** A tab's open, focus and Esc behavior (`library.ts`, `command-sheet.ts`, `shortcut-tab.ts`). */
export function onPaneTab(tab: PaneTab, tabHooks: TabHooks): void {
  hooks.set(tab, tabHooks);
}

/** Marks `tab` selected (ARIA state, roving tab stop, panels); no focus move. */
function mark(tab: PaneTab): void {
  if (!pane) return;
  pane.dataset.tab = tab;
  for (const each of PANE_TABS) {
    const button = tabButton(each);
    const selected = each === tab;
    button?.setAttribute('aria-selected', String(selected));
    button?.setAttribute('tabindex', selected ? '0' : '-1');
    const body = panel(each);
    if (body) body.hidden = !selected;
  }
  toggle?.setAttribute('aria-expanded', String(isPaneOpen()));
}

/** Shows `tab` (the pane stays as it is: open or closed). */
export function selectTab(tab: PaneTab, focus = false): void {
  mark(tab);
  if (isPaneOpen()) hooks.get(tab)?.show?.(focus);
}

/**
 * Opens the pane on `tab`. `from` is where focus returns on close (else what
 * had focus when the pane opened); `focus` moves focus into the tab.
 */
export function openPane(tab: PaneTab, options: { focus?: boolean; from?: HTMLElement | null } = {}): void {
  if (!pane || !documentRef) return;
  if (!isPaneOpen()) lastFocus = options.from ?? (documentRef.activeElement as HTMLElement | null);
  else if (options.from) lastFocus = options.from;
  pane.dataset.open = 'true';
  documentRef.getElementById('cv-shell')?.classList.add('is-library-open');
  mark(tab);
  hooks.get(tab)?.show?.(options.focus ?? true);
}

/** Closes the pane; `restore` hands focus back to what opened it. */
export function closePane(restore = true): void {
  if (!pane || !documentRef || !isPaneOpen()) return;
  pane.dataset.open = 'false';
  documentRef.getElementById('cv-shell')?.classList.remove('is-library-open');
  toggle?.setAttribute('aria-expanded', 'false');
  const inside = pane.contains(documentRef.activeElement);
  if (restore && (inside || documentRef.activeElement === documentRef.body)) {
    const target = lastFocus?.isConnected ? lastFocus : toggle;
    target?.focus({ preventScroll: true });
  }
  lastFocus = null;
}

/** `L`: opens the pane on Library; on Library already, closes it. */
export function togglePane(tab: PaneTab): void {
  if (isPaneOpen() && paneTab() === tab) closePane();
  else openPane(tab);
}

/** Wires the pane once (the page's markup). */
export function startPane(document_: Document): void {
  if (pane) return;
  documentRef = document_;
  pane = document_.getElementById('cv-library');
  toggle = document_.getElementById('cv-library-toggle');
  if (!pane) return;
  mark(paneTab());
  for (const tab of PANE_TABS) {
    tabButton(tab)?.addEventListener('click', () => selectTab(tab, false));
  }
  pane.querySelector('[role="tablist"]')?.addEventListener('keydown', (event) => {
    const key = (event as KeyboardEvent).key;
    const at = PANE_TABS.indexOf(paneTab());
    const next = key === 'ArrowRight' ? (at + 1) % PANE_TABS.length
      : key === 'ArrowLeft' ? (at - 1 + PANE_TABS.length) % PANE_TABS.length
        : key === 'Home' ? 0 : key === 'End' ? PANE_TABS.length - 1 : -1;
    if (next < 0) return;
    event.preventDefault();
    const tab = PANE_TABS[next]!;
    selectTab(tab, false);
    tabButton(tab)?.focus({ preventScroll: true });
  });
  pane.addEventListener('keydown', (event) => {
    if (event.key !== 'Escape' || event.defaultPrevented) return;
    event.preventDefault();
    event.stopPropagation();
    if (!hooks.get(paneTab())?.escape?.()) closePane();
  });
}
