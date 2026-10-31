// The Shortcuts tab of the right pane (`pane.ts`), beside Library, Commands
// and Ask. `/shortcuts` and ⋯ → "Shortcuts and commands" open the pane on
// this tab. APP-122 (amending APP-120 §3): `?` has one meaning, Ask: on the
// canvas it puts `? ` in the bar, and `?` first in the bar opens the Ask tab.
// It replaces the separate `#cv-shortcuts` modal sheet. It lists every
// action with both of its paths (APP-112): the click and the keys or the
// command. A line at the top links to the Commands tab, where every command
// has its arguments and examples. Built with `el()` only: text nodes, no
// markup.
import { openCommandSheet } from './command-sheet';
import { onPaneTab, openPane } from './pane';
import { GRAMMAR } from './command-registry';
import { el } from './render';

const mac = typeof navigator !== 'undefined' && /Mac|iPhone|iPad/.test(navigator.platform || navigator.userAgent);
const MOD = mac ? '⌘' : 'Ctrl';

/**
 * APP-112: every action with both of its paths, the click and the keys or
 * command ("Unite app: App card → Unite · U"). Every command has a click
 * path; not every click has a command.
 */
export const SHORTCUTS: [string, [what: string, click: string, keys: string][]][] = [
  ['Place', [
    ['An app for a request', 'Type it in plain words in the bar', 'Enter'],
    ['A whole app', 'Library → Apps → Add', '/app expenses · /expenses'],
    ['A card', 'Library → expand an app → Add', '/card expenses.ledger · /card 3.2'],
    ['A widget', 'Library → Widgets → Add, or say “show the weather in Austin”', '/widget weather Austin · /weather'],
    ['A note', '⋯ → New note, or Library → Widgets → Note', '/widget note text · /note'],
    ['A folder', '+ Folder in the toolbar, Library → Widgets → Folder, drop files on the canvas, or Drop files or upload on an empty canvas', '/folder'],
    ['Take it off the canvas', 'The × on it, Library → Remove, or ⋯ → Take off the canvas', 'Delete'],
  ]],
  ['Tell an app', [
    ['File an expense', 'Say it: “I spent $86 at Nobu, Sarah approves”', '/expenses …'],
    ['Approve or reject', 'Records card → Approve · Reject, or say “approve the Nobu dinner”', '/expenses approve …'],
    ['Any app', 'Click its /name chip (Library or the app card) to put it in the bar', '/name …'],
  ]],
  ['Ask and Shape', [
    ['Ask about your data', 'The Ask tab, or ⋯ → Ask about this on a card', '? where are the red invoices'],
    ['Ask about a folder’s files', 'Folder → Ask (its tables; read-only)', '—'],
    ['Shape a card', '⋯ → Shape… on the card', '/expenses.ledger only red invoices · /shape expenses.ledger …'],
    ['Theme an app', '⋯ → Shape… on its app card or frame', '/shape expenses dark green, compact'],
    ['Shape a widget', 'Its Shape field, or ⋯ → Shape… on it', '/shape calculator make it a TI-84'],
    ['Undo a shape', 'Revert on the shaped card, app card or widget', '—'],
  ]],
  ['Arrange', [
    ['Select', 'Click a header · Shift + click to add', '—'],
    ['Move', 'Drag the header', 'Arrow keys · Shift: 10 px'],
    ['Arrange by app', '⋯ → Arrange by app', '/arrange'],
    ['Unite an app', 'App card → Unite, Library → Unite, or ⋯ → Unite app', 'U · /unite expenses'],
    ['Break it apart', 'Frame → Break apart, or ⋯ → Break app apart', 'Shift + U · /split expenses'],
    ['Focus', 'The [ ] button on a card, or ⋯ → Focus', 'F · /focus'],
    ['Stack', 'Drag onto a card and hold, Library → Stack, or ⋯ → Stack them', `${MOD} + G`],
    ['Leaf through a stack', 'The ‹ › on the stack', '← · →'],
    ['Fan a stack out', 'The +N on the stack', 'Enter · Esc folds'],
    ['Spread a stack', 'Spread on the stack, or ⋯ → Spread a stack', `${MOD} + Shift + G`],
    ['Rename a stack', 'Double-click its name', '—'],
  ]],
  ['Make', [
    ['One card from a folder', 'Folder → Make a card (confirm first)', '—'],
    ['An app from a folder', 'Folder → ✨ Make an app (formation), or click the folder and type', '/q3 build a metrics app · /folder:q3 …'],
    ['A brief of a folder', 'Folder → Brief me', '/folder 1 formation brief me'],
  ]],
  ['Move around', [
    ['Pan', 'Scroll, or drag with the hand tool', 'Space + drag'],
    ['Zoom', 'The zoom menu (100% ▾), or pinch', `${MOD} + = · ${MOD} + −`],
    ['100% · fit · selection', 'The zoom menu', 'Shift + 0 · 1 · 2'],
    ['Move tool · hand tool', 'The arrow · hand in the toolbar', 'V · H'],
  ]],
  ['Help', [
    ['Library', 'Library at the top, or the pane’s Library tab', 'L · /apps'],
    ['Every command', 'The pane’s Commands tab, or the link at the top of this tab', '/commands · /help'],
    ['Show timings', '⋯ → Show timings, or say “show timings”', '#timing=1'],
    ['Shortcuts tab', '⋯ → Shortcuts and commands, or the pane’s Shortcuts tab', '/shortcuts'],
    ['Ask', 'The pane’s Ask tab', '? · ? <question>'],
  ]],
];

function keys(document_: Document, text: string): Node[] {
  if (text === '—') return [document_.createTextNode('—')];
  return text.split(/(\s·\s|\s\+\s)/).map((part) => (/^(\s·\s|\s\+\s)$/.test(part)
    ? document_.createTextNode(part)
    : el(document_, part.startsWith('/') || part.startsWith('#timing') || part.startsWith('? ') ? 'code' : 'kbd', {}, part)));
}

let built = false;

/** Fills the Shortcuts tab's panel (once): the Commands link, then the tables. */
function build(document_: Document): boolean {
  if (built) return true;
  const body = document_.getElementById('cv-panel-shortcuts');
  if (!body) return false;
  const commands = el(document_, 'button', { type: 'button', class: 'cv-shortcuts-all', 'data-pane-goto': 'commands' }, 'Every command with its arguments → Commands');
  commands.addEventListener('click', () => openCommandSheet(''));
  body.replaceChildren(
    el(document_, 'p', { class: 'cv-shortcuts-sub' }, 'Everything works by clicking. Keys and / commands are shortcuts. ', commands),
    el(document_, 'p', { class: 'cv-commands-grammar' }, GRAMMAR),
    el(document_, 'div', { class: 'cv-shortcuts-grid' },
      ...SHORTCUTS.map(([title, rows]) => el(document_, 'section', { 'aria-label': title },
        el(document_, 'h3', {}, title),
        el(document_, 'table', { class: 'cv-shortcuts-table' },
          el(document_, 'thead', { class: 'sr-only' }, el(document_, 'tr', {},
            el(document_, 'th', { scope: 'col' }, 'Action'), el(document_, 'th', { scope: 'col' }, 'Click'), el(document_, 'th', { scope: 'col' }, 'Keys or command'))),
          el(document_, 'tbody', {}, ...rows.map(([what, click, shortcut]) => el(document_, 'tr', {},
            el(document_, 'th', { scope: 'row' }, what),
            el(document_, 'td', { class: 'cv-shortcuts-click' }, click),
            el(document_, 'td', { class: 'cv-shortcuts-keys' }, ...keys(document_, shortcut))))))))));
  onPaneTab('shortcuts', {
    // Focus goes to the tab itself: ←/→ reach the others, Tab the link and tables.
    show: (focus) => {
      if (focus) document_.getElementById('cv-tab-shortcuts')?.focus({ preventScroll: true });
    },
  });
  built = true;
  return true;
}

/** Opens the pane on Shortcuts; focus returns to `from` (else what had it) on close. */
export function openShortcuts(from: HTMLElement | null = null): void {
  if (!build(document)) return;
  openPane('shortcuts', { focus: true, from });
}

/** Wires the Shortcuts tab at start, so a click on the tab finds it filled. */
export function startShortcutTab(document_: Document): void {
  build(document_);
}
