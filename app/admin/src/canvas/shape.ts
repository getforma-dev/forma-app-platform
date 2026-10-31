// APP-122 Shape: change how a card, an app (its theme) or a widget looks or
// what it shows. Typed `/shape <address> <words>` (the Shape verb), or
// `/<app>.<card> <words>` for a card (the dot is the rule), or ⋯ → Shape…,
// which puts `/shape <address> ` in the bar. The address goes through the
// existing resolvers (the Library's `findCards` / `findApps`, the member's
// card shortcuts, the widget kinds), so whatever they accept `/shape` accepts.
//
// `POST /v1/shape` answers only a closed spec for the target (judged on the
// server, `composer::shape`, and again here, `talk-view.ts`). Nothing changes
// until the member clicks: the spec shows as a *preview* on the item with a
// dock card (Apply, Add as new card for a card, Cancel); Apply labels the
// item *shaped* with Revert. Cards and themes are session-only (APP-123
// makes personal cards persist). A card's view is evaluated over the card's
// own data in this browser (`talk-view.ts`), never records: a view may
// filter or hide records, and every number it computes keeps the receipts
// of the records it came from (ADR-011 I3). Text nodes only.
import { addressFor, fillBar } from './address-chip';
import { addCards, focusCard, isOnBoard, placeUtilityCard } from './board';
import { findCardShortcut, type CanvasCommand } from './command-registry';
import { isDismissKey, showDockCard } from './dock-card';
import { cardAddress, findApps, findCardRow, findCards, type AppRow, type CardRow } from './library';
import { recordsJson } from './records';
import { announce, el, receiptSection, released } from './render';
import { registerCompleter, type Completion } from './slash';
import {
  appTheme, cardView, chips, evaluate, fieldsOf, tableFromQuery, tableFromRecords, themeChips,
  type AppTheme, type CardView, type Group, type RecordReceipt, type Shaped, type Table,
} from './talk-view';
import { frameCardsOfItem, isUniteItem } from './unite';
import type { WidgetCard } from './widgets/card';
import { widgetFor, widgetKind, widgetOfItem } from './widgets/index';
import { validState, type WidgetKind, type WidgetState } from './widgets/state';

export const SHAPE_URL = '/v1/shape';
type Report = (message: string) => void;

// --- Resolving an address (the existing resolvers only) ---

export type Target =
  | { kind: 'card'; card: CardRow; address: string }
  | { kind: 'app'; app: AppRow; address: string }
  | { kind: 'widget'; widget: WidgetCard; address: string }
  | { kind: 'none'; message: string };

const appAddress = (app: AppRow) => app.slug || app.address?.slug || app.number;

/** What `address` names: a widget kind, a card (`app.card`, `3.2`, a card shortcut), else an app. */
export function resolveTarget(address: string): Target {
  const text = address.trim().replace(/^\//, '');
  if (!text) return { kind: 'none', message: 'Name what to shape: /shape expenses.ledger …, /shape expenses …, /shape calculator …' };
  const kind = widgetKind(text);
  if (kind) {
    if (kind === 'note' || kind === 'folder') return { kind: 'none', message: `A ${kind} keeps its contents; shaping a ${kind} is not available yet.` };
    const widget = widgetFor(kind);
    return widget ? { kind: 'widget', widget, address: kind } : { kind: 'none', message: `Place a ${kind} first: /widget ${kind}.` };
  }
  const shortcut = findCardShortcut(text);
  const named = shortcut ? findCards(`${shortcut.address.slug}.${shortcut.card}`) : [];
  if (named[0]) return cardTarget(named[0]);
  if (text.includes('.')) {
    const [card] = findCards(text);
    return card ? cardTarget(card) : { kind: 'none', message: `No card matches ${text}. Open the Library (L) to see each card’s app.card name.` };
  }
  const [app] = findApps(text);
  if (app) return { kind: 'app', app, address: appAddress(app) };
  return { kind: 'none', message: `Nothing answers to ${text}. Shape a card (app.card), an app or a widget.` };
}

function cardTarget(card: CardRow): Target {
  // The app card's look is its app's: shaping it themes the app.
  if (card.slug === 'app') return { kind: 'app', app: card.app, address: appAddress(card.app) };
  return { kind: 'card', card, address: cardAddress(card) };
}

/** The address of a canvas item (⋯ → Shape…, ⋯ → Ask about this); null when it has none. */
export function addressOfItem(item: HTMLElement | null): string | null {
  if (!item) return null;
  const widget = widgetOfItem(item);
  if (widget) return widget.kind;
  if (isUniteItem(item)) {
    const row = frameCardsOfItem(item).map(findCardRow).find(Boolean);
    return row ? appAddress(row.app) : null;
  }
  const row = findCardRow(item.dataset.instanceId ?? '');
  if (!row) return null;
  return row.slug === 'app' ? appAddress(row.app) : cardAddress(row);
}

// --- Loading a card's data ---

function cardElement(id: string): HTMLElement | null {
  if (!/^[A-Za-z0-9_-]{1,160}$/.test(id)) return null;
  return document.querySelector<HTMLElement>(`.cv-item .cv-card[data-card="${id}"]`) ?? document.querySelector<HTMLElement>(`.cv-card[data-card="${id}"]`);
}

/** The card's own data as a table: its records listing, or its released query result. */
async function loadTable(card: HTMLElement): Promise<Table | string> {
  try {
    const records = card.dataset.records;
    if (records && records.startsWith('/v1/work/')) {
      const response = await fetch(records, { credentials: 'same-origin', cache: 'no-store' });
      const listing = response.ok ? recordsJson(await response.json()) : null;
      return listing ? tableFromRecords(listing, card.dataset.collection ?? '') : 'This card’s records could not be read.';
    }
    const query = card.dataset.query;
    if (query && query.startsWith('/v1/work/')) {
      const response = await fetch(query, {
        method: 'POST', credentials: 'same-origin', cache: 'no-store',
        headers: { 'content-type': 'application/json' }, body: JSON.stringify({ parameters: {} }),
      });
      const result = response.ok ? released(await response.json()) : null;
      return result ? tableFromQuery(result) : 'This card’s result came without its receipt, so it cannot be shaped.';
    }
  } catch {
    return 'This card’s data could not be read. Check your connection.';
  }
  return 'This card has no data to shape (a form or a note). Shape its app’s theme with /shape <app>.';
}

// --- The request ---

const REFUSALS: Record<string, string> = {
  could_not_shape: 'That came back as something other than a view, so nothing was shown. Nothing changed.',
  composer_unavailable: 'Shaping that needs the AI model, and none is configured here. Try “only red invoices”, “total by month” or “highlight overdue in red”.',
  too_many_requests: 'Too many shapes in a short time. Try again in a moment.',
  invalid_request: 'Say it in a short sentence (up to 300 characters). Nothing changed.',
};

async function requestShape(body: Record<string, unknown>): Promise<{ target: string; spec: unknown } | string> {
  let response: Response;
  try {
    response = await fetch(SHAPE_URL, {
      method: 'POST', credentials: 'same-origin', cache: 'no-store',
      headers: { 'content-type': 'application/json' }, body: JSON.stringify(body),
    });
  } catch {
    return 'The shape could not be sent. Nothing changed.';
  }
  const answer = (await response.json().catch(() => null)) as Record<string, unknown> | null;
  if (response.status === 404) return 'You can’t open that app, so it can’t be shaped.';
  if (!response.ok) return REFUSALS[typeof answer?.code === 'string' ? answer.code : ''] ?? 'The shape could not be made right now. Nothing changed.';
  if (!answer || typeof answer.target !== 'string') return REFUSALS.could_not_shape ?? '';
  return { target: answer.target, spec: answer.spec };
}

// --- Rendering a shaped card ---

function shortId(id: string): string {
  return id.slice(0, 8);
}

function receiptList(document_: Document, receipts: RecordReceipt[], table: Table): HTMLElement {
  const rows = receipts.slice(0, 200).map((r) => el(document_, 'li', {},
    el(document_, 'code', { title: r.record_id }, shortId(r.record_id)),
    ` version ${r.version} · commit ${r.commit_sequence} · ${r.committed_at}`));
  const provenance = table.provenance;
  return el(document_, 'details', { class: 'cv-shaped-receipts', 'aria-label': 'Receipts' },
    el(document_, 'summary', {}, `Receipts · ${receipts.length} ${receipts.length === 1 ? 'record' : 'records'}`),
    el(document_, 'ul', {}, ...rows),
    provenance ? el(document_, 'p', { class: 'cv-shaped-provenance' },
      'Publication ', el(document_, 'code', {}, shortId(provenance.publication_id)),
      ' · definition ', el(document_, 'code', { title: provenance.definition_digest }, provenance.definition_digest.slice(0, 19)),
      ` · work version ${provenance.work_version}`) : null);
}

/** The receipts a shaped card keeps: the query's whole receipt, or its records' receipts. */
function receiptsFor(document_: Document, table: Table, receipts: RecordReceipt[]): HTMLElement {
  if (table.source === 'query' && table.receipt) return receiptSection(document_, table.receipt);
  return receiptList(document_, receipts, table);
}

const SVG = 'http://www.w3.org/2000/svg';
function chart(document_: Document, groups: Group[], line: boolean): Element | null {
  if (groups.length < 1 || groups.length > 60) return null;
  const max = Math.max(...groups.map((g) => g.measured.value), 0);
  if (max <= 0) return null;
  const [width, height, pad] = [520, 160, 18];
  const step = (width - pad * 2) / groups.length;
  const svg = document_.createElementNS(SVG, 'svg');
  svg.setAttribute('viewBox', `0 0 ${width} ${height + 22}`);
  svg.setAttribute('class', 'cv-shaped-chart');
  svg.setAttribute('role', 'img');
  svg.setAttribute('aria-label', groups.map((g) => `${g.label}: ${g.measured.text}`).join('; '));
  const y = (v: number) => pad / 2 + (height - pad) * (1 - v / max);
  const points: string[] = [];
  groups.forEach((g, i) => {
    const x = pad + i * step;
    if (line) points.push(`${(x + step / 2).toFixed(1)},${y(g.measured.value).toFixed(1)}`);
    else {
      const bar = document_.createElementNS(SVG, 'rect');
      bar.setAttribute('x', (x + step * 0.15).toFixed(1));
      bar.setAttribute('width', (step * 0.7).toFixed(1));
      bar.setAttribute('y', y(g.measured.value).toFixed(1));
      bar.setAttribute('height', (height - pad / 2 - y(g.measured.value)).toFixed(1));
      bar.setAttribute('class', 'cv-shaped-bar');
      const tip = document_.createElementNS(SVG, 'title');
      tip.textContent = `${g.label}: ${g.measured.text}`;
      bar.append(tip);
      svg.append(bar);
    }
    if (groups.length <= 12) {
      const label = document_.createElementNS(SVG, 'text');
      label.setAttribute('x', (x + step / 2).toFixed(1));
      label.setAttribute('y', String(height + 14));
      label.setAttribute('text-anchor', 'middle');
      label.textContent = g.label.length > 12 ? `${g.label.slice(0, 11)}…` : g.label;
      svg.append(label);
    }
  });
  if (line) {
    const path = document_.createElementNS(SVG, 'polyline');
    path.setAttribute('points', points.join(' '));
    path.setAttribute('class', 'cv-shaped-line');
    svg.append(path);
  }
  return svg;
}

function body(document_: Document, shaped: Shaped, table: Table, view: CardView): HTMLElement {
  const box = el(document_, 'div', { class: 'cv-shaped-body' });
  if (shaped.kind === 'number') {
    box.append(
      el(document_, 'p', { class: 'cv-insight-label' }, shaped.measure),
      el(document_, 'p', { class: 'cv-insight-value value' }, shaped.measured.text),
      el(document_, 'p', { class: 'cv-shaped-meta' }, `From ${shaped.count} of ${shaped.total} ${table.source === 'records' ? 'records' : 'rows'}`),
      receiptsFor(document_, table, shaped.receipts));
    return box;
  }
  if (shaped.kind === 'groups') {
    const figure = view.presentation === 'table' ? null : chart(document_, shaped.groups, view.presentation === 'line');
    if (figure) box.append(figure);
    const rows = shaped.groups.map((g) => el(document_, 'tr', {},
      el(document_, 'th', { scope: 'row' }, g.label),
      el(document_, 'td', { class: 'value num' }, g.measured.text),
      el(document_, 'td', {}, table.source === 'records' ? receiptList(document_, g.receipts, table) : `${g.count} rows`)));
    box.append(
      el(document_, 'div', { class: 'cv-table' }, el(document_, 'table', {},
        el(document_, 'thead', {}, el(document_, 'tr', {},
          el(document_, 'th', { scope: 'col' }, shaped.by.replace(/_/g, ' ')),
          el(document_, 'th', { scope: 'col' }, shaped.measure),
          el(document_, 'th', { scope: 'col' }, 'Receipts'))),
        el(document_, 'tbody', {}, ...rows))),
      el(document_, 'p', { class: 'cv-shaped-meta' }, `From ${shaped.matched} of ${shaped.total} ${table.source === 'records' ? 'records' : 'rows'}`));
    if (table.source === 'query' && table.receipt) box.append(receiptSection(document_, table.receipt));
    return box;
  }
  const list = view.presentation === 'list';
  const columns = shaped.columns;
  const receipts = shaped.rows.flatMap(({ row }) => (row.receipt ? [row.receipt] : []));
  if (list) {
    box.append(el(document_, 'ul', { class: 'cv-shaped-list' }, ...shaped.rows.map(({ row, color }) => el(document_, 'li', color ? { 'data-highlight': color } : {},
      columns.slice(0, 3).map((c) => row.text[c.id] ?? '').filter(Boolean).join(' · ')))));
  } else {
    box.append(el(document_, 'div', { class: 'cv-table' }, el(document_, 'table', {},
      el(document_, 'thead', {}, el(document_, 'tr', {}, ...columns.map((c) => el(document_, 'th', { scope: 'col' }, c.id.replace(/_/g, ' '))))),
      el(document_, 'tbody', {}, ...shaped.rows.map(({ row, color }) => el(document_, 'tr', color ? { 'data-highlight': color } : {},
        ...columns.map((c) => el(document_, 'td', { class: ['money', 'integer', 'decimal'].includes(c.type) ? 'value num' : 'value' }, row.text[c.id] ?? '—'))))))));
  }
  box.append(
    el(document_, 'p', { class: 'cv-shaped-meta' }, `${shaped.rows.length} of ${shaped.total} ${table.source === 'records' ? 'records' : 'rows'} shown`),
    receiptsFor(document_, table, receipts));
  return box;
}

/** A shaped card's section: its chips, its label (preview or shaped) and its body with receipts. */
export function shapedSection(document_: Document, state: 'preview' | 'shaped', address: string, view: CardView, table: Table): HTMLElement {
  const shaped = evaluate(table, view);
  return el(document_, 'section', { class: 'cv-shaped', 'data-state': state, 'data-palette': view.palette ?? 'default', 'aria-label': `${state === 'preview' ? 'Preview' : 'Shaped'} view of /${address}` },
    el(document_, 'p', { class: 'cv-shaped-chips' },
      el(document_, 'span', { class: 'badge cv-shaped-badge' }, state),
      ...(view.title ? [el(document_, 'strong', {}, view.title)] : []),
      ...chips(view).map((chip) => el(document_, 'span', { class: 'cv-shaped-chip' }, chip))),
    body(document_, shaped, table, view));
}

// --- Applying: preview, Apply, Add as new card, Revert (session only) ---

type Applied = { address: string; view: CardView; table: Table };
const appliedCards = new Map<string, Applied>();
const appliedThemes = new Map<string, AppTheme>();
let newCards = 0;

function head(card: HTMLElement): HTMLElement | null {
  return card.querySelector<HTMLElement>(':scope > .cv-card-head, :scope > header, :scope > h2');
}

/** Hides the card's own content (it keeps running underneath) and shows `section` instead. */
function showSection(card: HTMLElement, section: HTMLElement | null): void {
  card.querySelector(':scope > .cv-shaped')?.remove();
  const header = head(card);
  for (const child of Array.from(card.children) as HTMLElement[]) {
    if (child === header || child.classList.contains('cv-shaped')) continue;
    if (section) {
      if (!child.hidden) {
        child.hidden = true;
        child.dataset.shapeHidden = 'true';
      }
    } else if (child.dataset.shapeHidden === 'true') {
      child.hidden = false;
      delete child.dataset.shapeHidden;
    }
  }
  if (section) (header ?? card.firstElementChild)?.after(section);
  if (!section) delete card.dataset.shaped;
}

function labelShaped(host: HTMLElement | null, revert: () => void): void {
  if (!host) return;
  host.querySelector(':scope > .cv-shaped-label')?.remove();
  const document_ = host.ownerDocument;
  const button = el(document_, 'button', { type: 'button', class: 'cv-secondary cv-shaped-revert' }, 'Revert');
  button.addEventListener('click', (event) => {
    event.stopPropagation();
    revert();
  });
  host.append(el(document_, 'span', { class: 'cv-shaped-label' }, el(document_, 'span', { class: 'badge cv-shaped-badge' }, 'shaped'), button));
}
function unlabel(host: HTMLElement | null): void {
  host?.querySelector(':scope > .cv-shaped-label')?.remove();
}

function revertCard(card: HTMLElement, id: string, report: Report = announce): void {
  appliedCards.delete(id);
  showSection(card, null);
  unlabel(head(card));
  report('Reverted: the card shows its own view again.');
}

function applyCard(card: HTMLElement, id: string, applied: Applied): void {
  appliedCards.set(id, applied);
  showSection(card, shapedSection(card.ownerDocument, 'shaped', applied.address, applied.view, applied.table));
  card.dataset.shaped = 'true';
  labelShaped(head(card), () => revertCard(card, id));
}

/** The dock card that asks Apply / Add as new card / Cancel. */
function confirmDock(title: string, chipList: string[], buttons: { label: string; primary?: boolean; run: () => void }[], onCancel: () => void): void {
  const document_ = document;
  let close = () => undefined as void;
  let settled = false;
  const settle = (run: () => void) => {
    if (settled) return;
    settled = true;
    close();
    run();
  };
  const actions = el(document_, 'div', { class: 'cv-confirm-actions' });
  for (const button of buttons) {
    const node = el(document_, 'button', { type: 'button', class: button.primary ? 'cv-primary' : 'cv-secondary' }, button.label);
    node.addEventListener('click', () => settle(button.run));
    actions.append(node);
  }
  const cancel = el(document_, 'button', { type: 'button', class: 'cv-secondary' }, 'Cancel');
  cancel.addEventListener('click', () => settle(onCancel));
  actions.append(cancel);
  const card = el(document_, 'section', { class: 'cv-card cv-confirm cv-shape-confirm', role: 'dialog', 'aria-modal': 'false', 'aria-labelledby': 'cv-shape-title' },
    el(document_, 'header', { class: 'cv-card-head' }, el(document_, 'h3', { id: 'cv-shape-title' }, title), el(document_, 'span', { class: 'badge' }, 'preview')),
    el(document_, 'p', { class: 'cv-shaped-chips' }, ...chipList.map((chip) => el(document_, 'span', { class: 'cv-shaped-chip' }, chip))),
    el(document_, 'p', { class: 'cv-confirm-note' }, 'Nothing changes until you apply it. No record is written.'),
    actions);
  card.addEventListener('keydown', (event) => {
    if (isDismissKey(event)) {
      event.preventDefault();
      settle(onCancel);
    }
  });
  close = showDockCard(document_, card, () => settle(onCancel));
  actions.querySelector<HTMLButtonElement>('button')?.focus();
}

async function shapeCard(target: Extract<Target, { kind: 'card' }>, words: string, report: Report): Promise<void> {
  const row = target.card;
  if (!isOnBoard(row.id)) await addCards(row.app.app, row.app.work, [row.id], { focus: false });
  const card = cardElement(row.id);
  if (!card) {
    report(`${target.address} could not be placed to shape it.`);
    return;
  }
  report(`Shaping /${target.address}…`);
  const table = await loadTable(card);
  if (typeof table === 'string') {
    report(table);
    return;
  }
  const answer = await requestShape({
    address: target.address, words,
    target: { kind: 'card', app_id: row.app.app, work_id: row.app.work, card: row.slug, fields: fieldsOf(table) },
  });
  if (typeof answer === 'string') {
    report(answer);
    return;
  }
  const view = answer.target === 'card' ? cardView(answer.spec) : null;
  if (!view) {
    report(REFUSALS.could_not_shape ?? '');
    return;
  }
  const before = appliedCards.get(row.id) ?? null;
  showSection(card, shapedSection(card.ownerDocument, 'preview', target.address, view, table));
  card.dataset.shaped = 'preview';
  focusCard(card);
  const restore = () => {
    if (before) applyCard(card, row.id, before);
    else {
      showSection(card, null);
      unlabel(head(card));
    }
  };
  confirmDock(`Shape /${target.address}`, chips(view), [
    { label: 'Apply', primary: true, run: () => { applyCard(card, row.id, { address: target.address, view, table }); report(`/${target.address} is shaped. Revert is on the card.`); } },
    {
      label: 'Add as new card',
      run: () => {
        restore();
        newCards += 1;
        const title = view.title ?? `${row.title} (shaped)`;
        const section = shapedSection(card.ownerDocument, 'shaped', target.address, view, table);
        section.prepend(el(card.ownerDocument, 'p', { class: 'cv-shaped-from' }, `Made by you from /${target.address} · this session`));
        placeUtilityCard(`shape-${newCards}`, title, section);
        report(`A new card from /${target.address} is on the canvas. Its numbers keep their receipts.`);
      },
    },
  ], () => { restore(); report('Nothing changed.'); });
}

function appCards(app: AppRow): HTMLElement[] {
  return Array.from(document.querySelectorAll<HTMLElement>('.cv-card[data-work]')).filter((card) => card.dataset.work === app.work);
}
function setTheme(cards: HTMLElement[], theme: AppTheme | null): void {
  for (const card of cards) {
    for (const key of ['mode', 'accent', 'density'] as const) {
      const value = theme?.[key];
      const name = `theme${key[0]!.toUpperCase()}${key.slice(1)}` as 'themeMode';
      if (value && value !== 'default') card.dataset[name] = value;
      else delete card.dataset[name];
    }
  }
}
function appHeader(app: AppRow): HTMLElement | null {
  const card = appCards(app).find((c) => c.dataset.kind === 'app');
  return card ? head(card) : null;
}

async function shapeApp(target: Extract<Target, { kind: 'app' }>, words: string, report: Report): Promise<void> {
  const { app } = target;
  if (!appCards(app).length) await addCards(app.app, app.work, null);
  report(`Shaping /${target.address}…`);
  const answer = await requestShape({ address: target.address, words, target: { kind: 'app', app_id: app.app, work_id: app.work } });
  if (typeof answer === 'string') {
    report(answer);
    return;
  }
  const theme = answer.target === 'app' ? appTheme(answer.spec) : null;
  if (!theme) {
    report(REFUSALS.could_not_shape ?? '');
    return;
  }
  const before = appliedThemes.get(app.work) ?? null;
  const cards = appCards(app);
  setTheme(cards, theme);
  for (const card of cards) card.dataset.themePreview = 'true';
  const revert = () => {
    appliedThemes.delete(app.work);
    setTheme(appCards(app), null);
    unlabel(appHeader(app));
    announce(`/${target.address} has its own look again.`);
  };
  const settle = () => { for (const card of appCards(app)) delete card.dataset.themePreview; };
  confirmDock(`Shape /${target.address}: every card`, themeChips(theme), [
    {
      label: 'Apply', primary: true,
      run: () => {
        settle();
        appliedThemes.set(app.work, theme);
        setTheme(appCards(app), theme);
        labelShaped(appHeader(app), revert);
        report(`Every /${target.address} card is shaped; each keeps its own view. Revert is on the app card.`);
      },
    },
  ], () => { settle(); setTheme(appCards(app), before); report('Nothing changed.'); });
}

function shapeWidgetLabel(widget: WidgetCard, previous: WidgetState): void {
  widget.chips.querySelector('.cv-shaped-label')?.remove();
  const button = el(widget.card.ownerDocument, 'button', { type: 'button', class: 'cv-secondary cv-shaped-revert' }, 'Revert');
  const label = el(widget.card.ownerDocument, 'span', { class: 'cv-shaped-label' }, el(widget.card.ownerDocument, 'span', { class: 'badge cv-shaped-badge' }, 'shaped'), button);
  button.addEventListener('click', (event) => {
    event.stopPropagation();
    widget.apply(previous);
    label.remove();
    widget.announce('Reverted.');
  });
  widget.chips.append(label);
}

async function shapeWidget(target: Extract<Target, { kind: 'widget' }>, words: string, report: Report): Promise<void> {
  const { widget } = target;
  report(`Shaping the ${widget.kind}…`);
  const answer = await requestShape({ address: target.address, words, target: { kind: 'widget', widget: widget.kind, state: widget.state } });
  if (typeof answer === 'string') {
    report(answer);
    return;
  }
  const next = answer.target === 'widget' ? validState(widget.kind as WidgetKind, answer.spec) : null;
  if (!next) {
    report(REFUSALS.could_not_shape ?? '');
    return;
  }
  const previous = widget.state;
  widget.apply(next);
  widget.card.dataset.shapePreview = 'true';
  const settle = () => { delete widget.card.dataset.shapePreview; };
  const look = [`${next.skin} skin`, `${next.accent} accent`, `${next.font} font`];
  confirmDock(`Shape /${target.address}`, look, [
    { label: 'Apply', primary: true, run: () => { settle(); shapeWidgetLabel(widget, previous); report(`The ${widget.kind} is shaped. Revert is on it.`); } },
  ], () => { settle(); widget.apply(previous); report('Nothing changed.'); });
}

/** `/shape <address> <words>` and the dot shortcut: resolve, ask the server, preview. */
export async function runShape(address: string, words: string, report: Report = announce): Promise<void> {
  const target = resolveTarget(address);
  if (target.kind === 'none') {
    report(target.message);
    return;
  }
  if (!words.trim()) {
    fillBar(`/shape ${target.address} `);
    report(`Say how /${target.address} should look or what it should show, then Enter.`);
    return;
  }
  const input = document.getElementById('intent-input') as HTMLInputElement | null;
  if (input) input.value = '';
  if (target.kind === 'card') await shapeCard(target, words.trim(), report);
  else if (target.kind === 'app') await shapeApp(target, words.trim(), report);
  else await shapeWidget(target, words.trim(), report);
}

/** `/<app>.<card> words` (the dot is the rule): the card's Shape; a bare `/<app>.<card>` places it. */
export async function runDot(target: string, words: string, report: Report = announce): Promise<void> {
  if (!words) {
    const [card] = findCards(target);
    if (!card) {
      report(`No card matches ${target}. Open the Library (L) to browse.`);
      return;
    }
    report(isOnBoard(card.id) ? `${card.title} is already on your canvas.` : await addCards(card.app.app, card.app.work, [card.id]));
    const input = document.getElementById('intent-input') as HTMLInputElement | null;
    if (input) input.value = '';
    return;
  }
  await runShape(target, words, report);
}

export const SHAPE_COMMAND: CanvasCommand = {
  slug: 'shape',
  title: 'Shape',
  purpose: 'Change how a card, an app (its theme) or a widget looks or what it shows; previewed, then Apply',
  run: (text: string, report: Report = announce) => {
    const match = text.trim().match(/^(\S+)\s*([\s\S]*)$/);
    if (!match) {
      report('Name what to shape: /shape expenses.ledger only red invoices · /shape expenses dark green · /shape calculator make it a TI-84.');
      return;
    }
    return runShape(match[1] ?? '', match[2] ?? '', report);
  },
};

/** `/shape <address>`: cards (app.card), apps and the widgets on the canvas, while the address is typed. */
export function shapeCompletions(rest: string): Completion[] {
  if (/\s/.test(rest.trimStart())) return [];
  const query = rest.trim();
  const cards = findCards(query).filter((card) => card.slug !== 'app').slice(0, 6).map((card) => ({
    badge: card.number, title: cardAddress(card), detail: `${card.title} · its view`, value: `/shape ${cardAddress(card)} `,
  }));
  const apps = query.includes('.') ? [] : findApps(query).slice(0, 3).map((app) => ({
    badge: app.number, title: appAddress(app), detail: `${app.title} · a theme on every card`, value: `/shape ${appAddress(app)} `,
  }));
  const widgets = (['weather', 'calculator', 'clock'] as const)
    .filter((kind) => (!query || kind.startsWith(query)) && widgetFor(kind))
    .map((kind) => ({ badge: '◇', title: kind, detail: 'Its look', value: `/shape ${kind} ` }));
  return [...cards, ...apps, ...widgets].slice(0, 10);
}

/** `/expenses.le…`: the cards of that app, by their dotted names (a space then shapes). */
export function dotCompletions(rest: string): Completion[] {
  return findCards(rest).filter((card) => card.slug !== 'app').slice(0, 8).map((card) => ({
    badge: card.number, title: cardAddress(card), detail: `${card.title} · type what it should show`, value: `/${cardAddress(card)} `,
  }));
}

/** The address an app or card has on the canvas (for Ask's scope and its chip). */
export function addressOfWork(app: string, work: string): string {
  return addressFor(app, work)?.slug ?? '';
}

export function startShape(): void {
  registerCompleter('shape', shapeCompletions);
  registerCompleter('.', dotCompletions, { select: false });
}
