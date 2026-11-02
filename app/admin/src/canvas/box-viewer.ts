// APP-108: the file viewer for folder files, a separate module behind a tiny
// API (`openViewer(file, boxId, page)`, `openModal`). APP-106's focus mode
// (`focus.ts`) moves an existing canvas card into its modal and has no API
// for arbitrary content, so the viewer stays its own dialog with focus mode's
// look (its close button and panel styling) and stacks above it: a file
// opened from a focused folder card closes first on Esc or a backdrop click
// (`data-modal-above` tells focus mode to leave those keys alone).
//
// The modal: role=dialog, aria-modal, a labelled title, focus kept inside
// (Tab wraps), Esc, the close button and a backdrop click close it, and focus
// returns to where it was. Content is built from nodes only:
// - PDF: an <iframe> of the authorized same-origin content route (served
//   `inline` with `Content-Security-Policy: sandbox`, `nosniff`), plus an
//   "Open in a new tab" link for a browser that refuses a sandboxed PDF;
// - image: an <img> of the same route;
// - text / Markdown: the bytes fetched as text into a <pre> text node;
// - CSV / XLSX: paginated original lexical cells, all selectable sheets,
//   no inference/normalization, with a byte-exact original download.
import { fetchOriginalPage, formatBytes, refusalText, type BoxFile, type Preview } from './box-api';
import { el } from './render';

type Modal = { root: HTMLElement; body: HTMLElement; actions: HTMLElement; close(): void };
let open: Modal | null = null;

/** Opens a modal (one at a time); `onClose` runs once when it closes. */
export function openModal(title: string, subtitle: string, onClose?: () => void, wide = false): Modal {
  open?.close();
  const document_ = document;
  const previous = document_.activeElement as HTMLElement | null;
  const titleId = `bx-modal-title-${Math.random().toString(36).slice(2, 8)}`;
  // APP-106's look (focus mode's close button and panel), stacked above it.
  const closeButton = el(document_, 'button', { type: 'button', class: 'cv-focus-close bx-modal-close', 'aria-label': 'Close (Esc)', title: 'Close (Esc)' });
  const x = document_.createElementNS('http://www.w3.org/2000/svg', 'svg');
  x.setAttribute('viewBox', '0 0 16 16');
  x.setAttribute('width', '16');
  x.setAttribute('height', '16');
  x.setAttribute('aria-hidden', 'true');
  const xPath = document_.createElementNS('http://www.w3.org/2000/svg', 'path');
  xPath.setAttribute('d', 'M3.5 3.5l9 9M12.5 3.5l-9 9');
  x.append(xPath);
  closeButton.append(x);
  const body = el(document_, 'div', { class: 'bx-modal-body' });
  const actions = el(document_, 'div', { class: 'bx-modal-actions' });
  const dialog = el(document_, 'div', {
    class: `bx-modal${wide ? ' is-wide' : ''}`, role: 'dialog', 'aria-modal': 'true', 'aria-labelledby': titleId, tabindex: '-1',
  },
  el(document_, 'header', { class: 'bx-modal-head' },
    el(document_, 'div', { class: 'bx-modal-id' },
      el(document_, 'h2', { id: titleId, class: 'bx-modal-title' }, title),
      subtitle ? el(document_, 'p', { class: 'bx-modal-sub' }, subtitle) : null),
    closeButton),
  body,
  actions);
  // `data-modal-above`: focus mode leaves Esc, Tab and the backdrop to this dialog while it is open.
  const root = el(document_, 'div', { class: 'bx-backdrop', 'data-modal-above': '' }, dialog);
  let closed = false;
  const close = () => {
    if (closed) return;
    closed = true;
    root.remove();
    document_.removeEventListener('keydown', onKey, true);
    if (open?.root === root) open = null;
    onClose?.();
    previous?.focus?.({ preventScroll: true });
  };
  const focusables = () => Array.from(dialog.querySelectorAll<HTMLElement>(
    'button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), textarea, iframe, [tabindex]:not([tabindex="-1"])',
  )).filter((node) => node.offsetParent !== null || node === closeButton);
  const onKey = (event: KeyboardEvent) => {
    if (event.key === 'Escape') {
      event.preventDefault();
      event.stopPropagation();
      close();
      return;
    }
    if (event.key === 'Tab') {
      const list = focusables();
      if (!list.length) return;
      const first = list[0]!;
      const last = list[list.length - 1]!;
      if (event.shiftKey && (document_.activeElement === first || !dialog.contains(document_.activeElement))) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && document_.activeElement === last) {
        event.preventDefault();
        first.focus();
      }
    }
    // Keys typed in the modal belong to it, not to the canvas shortcuts.
    if (dialog.contains(document_.activeElement)) event.stopPropagation();
  };
  closeButton.addEventListener('click', close);
  root.addEventListener('mousedown', (event) => {
    if (event.target === root) close();
  });
  document_.addEventListener('keydown', onKey, true);
  document_.body.append(root);
  dialog.focus({ preventScroll: true });
  open = { root, body, actions, close };
  return open;
}

/** Whether a viewer or sheet is open now. */
export function isModalOpen(): boolean {
  return open !== null;
}

type MakeData = (boxId: string, file: BoxFile, preview: Preview) => void;
let makeDataHandler: MakeData | null = null;
/** The box card hands the viewer its "Make data" action (keeps this module standalone). */
export function setMakeDataHandler(handler: MakeData): void {
  makeDataHandler = handler;
}

const KIND_LABEL: Record<BoxFile['kind'], string> = { pdf: 'PDF', image: 'Image', csv: 'CSV', xlsx: 'Spreadsheet', text: 'Text' };

export function spreadsheetColumnName(index: number): string {
  let name = '';
  for (let n = index + 1; n > 0; n = Math.floor((n - 1) / 26)) name = String.fromCharCode(65 + (n - 1) % 26) + name;
  return name;
}

export function spreadsheetColumnIndex(label: string): number | null {
  const text = label.trim().toUpperCase();
  let value = 0;
  if (/^[0-9]{1,5}$/.test(text)) value = Number(text);
  else if (/^[A-Z]{1,3}$/.test(text)) for (const c of text) value = value * 26 + c.charCodeAt(0) - 64;
  return value >= 1 && value <= 16384 ? value - 1 : null;
}

/** A preview table: the header shows each column's name and type. */
export function previewTable(document_: Document, preview: Preview): HTMLElement {
  const head = el(document_, 'tr', {}, ...preview.columns.map((column) => el(document_, 'th', { scope: 'col' },
    el(document_, 'span', { class: 'bx-col-name' }, column.header),
    el(document_, 'span', { class: 'bx-type', 'data-type': column.type }, column.type + (column.nullable ? '?' : '')))));
  const rows = preview.sample.map((row) => el(document_, 'tr', {}, ...preview.columns.map((_, index) => el(document_, 'td', {}, row[index] ?? ''))));
  return el(document_, 'div', { class: 'bx-table-wrap' },
    el(document_, 'table', { class: 'bx-table' }, el(document_, 'thead', {}, head), el(document_, 'tbody', {}, ...rows)));
}

/** Opens `file` of box `boxId` in the viewer. */
export function openViewer(file: BoxFile, boxId: string, page: number | null = null): void {
  const document_ = document;
  const modal = openModal(file.name, `${KIND_LABEL[file.kind]}${file.bytes ? ` · ${formatBytes(file.bytes)}` : ''}${page ? ` · page ${page}` : ''}`, undefined, file.kind !== 'image');
  const newTab = el(document_, 'a', { href: file.content_url, target: '_blank', rel: 'noopener noreferrer', class: 'bx-link' }, 'Open in a new tab');
  if (file.kind === 'pdf') {
    // A cited page opens there (the browser viewer's `#page=N`).
    const src = page && Number.isInteger(page) && page > 0 ? `${file.content_url}#page=${page}` : file.content_url;
    modal.body.append(el(document_, 'iframe', { class: 'bx-pdf', src, title: page ? `${file.name}, page ${page}` : file.name }));
    modal.actions.append(newTab);
    return;
  }
  if (file.kind === 'image') {
    modal.body.append(el(document_, 'img', { class: 'bx-image', src: file.content_url, alt: file.name }));
    modal.actions.append(newTab);
    return;
  }
  if (file.kind === 'text') {
    const pre = el(document_, 'pre', { class: 'bx-text' }, 'Loading…');
    modal.body.append(pre);
    void fetch(file.content_url, { credentials: 'same-origin', cache: 'no-store' })
      .then((response) => (response.ok ? response.text() : Promise.reject(new Error('refused'))))
      .then((text) => { pre.textContent = text.slice(0, 200000); })
      .catch(() => { pre.textContent = 'This file could not be read now.'; });
    return;
  }
  // Read immutable original cells, independently from the Make data preview.
  const status = el(document_, 'p', { class: 'bx-status', role: 'status', tabindex: '-1' }, 'Reading the original file…');
  const selection = el(document_, 'div', { class: 'bx-sheet-selection' });
  const table = el(document_, 'div', {});
  const navigation = el(document_, 'div', { class: 'bx-sheet-navigation' });
  const formulaNote = el(document_, 'p', { class: 'bx-sheet-limit', hidden: '' });
  modal.body.append(status, selection, navigation, formulaNote, table);
  modal.body.append(el(document_, 'p', { class: 'bx-sheet-limit' },
    'Original values, 50 rows × 32 columns per page. Jump to any row or column; no data is dropped between pages. Excel shows stored values and saved formula results without recalculation or styles. Date serials remain as stored. CSV shows logical records; Raw CSV includes original quoting and blank lines.'));
  modal.actions.append(el(document_, 'a', { href: file.content_url, download: file.name, class: 'bx-link' }, 'Download original file'));
  let generation = 0;
  let digest = '';
  if (file.kind === 'csv') {
    const raw = el(document_, 'button', { type: 'button', class: 'bx-link' }, 'Raw CSV');
    raw.addEventListener('click', () => {
      const current = ++generation;
      table.replaceChildren(); navigation.replaceChildren(); selection.replaceChildren(); formulaNote.hidden = true;
      status.textContent = 'Reading original CSV bytes…';
      void fetch(file.content_url, { credentials: 'same-origin', cache: 'no-store' }).then(async (response) => {
        if (!response.ok) throw new Error('refused');
        const bytes = await response.arrayBuffer();
        if (bytes.byteLength > 20 * 1024 * 1024) throw new Error('too large');
        const text = new TextDecoder('utf-8', { fatal: true }).decode(bytes);
        if (current !== generation || !modal.root.isConnected) return;
        const starts = [0];
        while (starts[starts.length - 1]! < text.length) {
          let end = Math.min(text.length, starts[starts.length - 1]! + 65536);
          if (end < text.length && text.charCodeAt(end - 1) >= 0xd800 && text.charCodeAt(end - 1) <= 0xdbff) end--;
          starts.push(end);
        }
        const renderRaw = (pageIndex: number) => {
          if (current !== generation || !modal.root.isConnected) return;
          table.replaceChildren(); navigation.replaceChildren();
          status.textContent = `Original CSV text · page ${pageIndex + 1} of ${Math.max(1, starts.length - 1)} · quoting and blank lines preserved`;
          table.append(el(document_, 'pre', { class: 'bx-text', tabindex: '0', 'aria-label': 'Original CSV text' }, text.slice(starts[pageIndex], starts[pageIndex + 1])));
          const before = el(document_, 'button', { type: 'button' }, 'Previous text page');
          const after = el(document_, 'button', { type: 'button' }, 'Next text page');
          before.disabled = pageIndex === 0; after.disabled = pageIndex + 1 >= starts.length - 1;
          before.addEventListener('click', () => { status.focus(); renderRaw(pageIndex - 1); });
          after.addEventListener('click', () => { status.focus(); renderRaw(pageIndex + 1); });
          navigation.append(before, after);
        };
        renderRaw(0);
      }).catch(() => {
        if (current === generation && modal.root.isConnected) status.textContent = 'The original CSV is unavailable. No cached content is shown.';
      });
    });
    const cells = el(document_, 'button', { type: 'button', class: 'bx-link' }, 'Table');
    cells.addEventListener('click', () => { void load(0, 0); });
    modal.actions.append(raw, cells);
  }
  const load = async (sheetIndex: number, offset: number, columnOffset = 0): Promise<void> => {
    const current = ++generation;
    if (modal.root.contains(document_.activeElement)) status.focus({ preventScroll: true });
    // Clear previous contents before each authority check, including failures.
    table.replaceChildren(); navigation.replaceChildren(); selection.replaceChildren(); formulaNote.hidden = true;
    status.textContent = 'Reading the original file…';
    try {
      const page = await fetchOriginalPage(boxId, file.file_id, sheetIndex, offset, columnOffset);
      if (current !== generation || !modal.root.isConnected) return;
      if (digest && digest !== page.sha256) throw new Error('original changed');
      digest = page.sha256;
      if (page.format === 'xlsx') {
        const select = el(document_, 'select', { 'aria-label': 'Workbook sheet' });
        page.sheets.forEach((name, index) => {
          const option = el(document_, 'option', { value: String(index) }, name);
          if (index === sheetIndex) option.selected = true;
          select.append(option);
        });
        select.addEventListener('change', () => { void load(Number(select.value), 0); });
        selection.append(select);
      }
      const start = page.rows.length ? offset + 1 : 0;
      const width = Math.min(page.column_page_size, page.column_count - columnOffset);
      const columnRange = width ? `${spreadsheetColumnName(columnOffset)}–${spreadsheetColumnName(columnOffset + width - 1)}` : '0';
      status.textContent = `Original file · rows ${start.toLocaleString()}–${(offset + page.rows.length).toLocaleString()} of ${page.row_count.toLocaleString()} · columns ${columnRange} of ${page.column_count.toLocaleString()}${page.format === 'xlsx' ? ` · ${page.sheets.length} sheets` : ''}`;
      if (page.formula_count) {
        formulaNote.hidden = false;
        formulaNote.textContent = `${page.formula_count.toLocaleString()} formula cells in this sheet. Saved results may be out of date; Forma does not recalculate them.${page.missing_formula_results ? ` ${page.missing_formula_results.toLocaleString()} have no saved result and are marked “No saved result”.` : ''}`;
      }
      const header = el(document_, 'tr', {}, el(document_, 'th', { scope: 'col' }, 'Row'),
        ...Array.from({ length: width }, (_, index) => el(document_, 'th', { scope: 'col' }, spreadsheetColumnName(columnOffset + index))));
      const formulas = new Map(page.formula_cells.map(cell => [`${cell.row}/${cell.column}`, cell.cached]));
      const rows = page.rows.map((row, index) => el(document_, 'tr', {},
        el(document_, 'th', { scope: 'row' }, String(offset + index + 1)),
        ...row.map((cell, column) => {
          const cached = formulas.get(`${offset + index + 1}/${columnOffset + column + 1}`);
          const td = el(document_, 'td', cached === undefined ? {} : { 'data-formula-result': cached ? 'saved' : 'missing', title: cached ? 'Saved formula result; not recalculated' : 'This formula has no saved result in the original file' },
            cached === false ? el(document_, 'span', { class: 'bx-formula-missing' }, 'No saved result') : cell);
          return td;
        })));
      table.append(el(document_, 'div', { class: 'bx-table-wrap bx-original-table', role: 'region', 'aria-label': 'Original spreadsheet cells', tabindex: '0' },
        el(document_, 'table', { class: 'bx-table' }, el(document_, 'thead', {}, header), el(document_, 'tbody', {}, ...rows))));
      const previous = el(document_, 'button', { type: 'button' }, 'Previous 50 rows');
      const next = el(document_, 'button', { type: 'button' }, 'Next 50 rows');
      previous.disabled = offset === 0;
      next.disabled = offset + page.rows.length >= page.row_count;
      previous.addEventListener('click', () => { void load(sheetIndex, Math.max(0, offset - page.page_size), columnOffset); });
      next.addEventListener('click', () => { void load(sheetIndex, offset + page.page_size, columnOffset); });
      const left = el(document_, 'button', { type: 'button' }, 'Previous 32 columns');
      const right = el(document_, 'button', { type: 'button' }, 'Next 32 columns');
      left.disabled = columnOffset === 0;
      right.disabled = columnOffset + width >= page.column_count;
      left.addEventListener('click', () => { void load(sheetIndex, offset, Math.max(0, columnOffset - page.column_page_size)); });
      right.addEventListener('click', () => { void load(sheetIndex, offset, columnOffset + page.column_page_size); });
      const rowInput = el(document_, 'input', { type: 'number', min: '1', max: String(Math.max(1, page.row_count)), step: '1', 'aria-label': 'Go to row' });
      rowInput.value = String(start || 1);
      const columnInput = el(document_, 'input', { type: 'text', maxlength: '5', 'aria-label': 'Go to column', title: 'Column letters or number, for example AA or 27' });
      columnInput.value = spreadsheetColumnName(columnOffset);
      const go = el(document_, 'button', { type: 'submit' }, 'Go');
      const jump = el(document_, 'form', { class: 'bx-sheet-jump', 'aria-label': 'Jump to spreadsheet position' },
        el(document_, 'label', {}, 'Row', rowInput), el(document_, 'label', {}, 'Column', columnInput), go);
      jump.addEventListener('submit', event => {
        event.preventDefault();
        const row = Number(rowInput.value), column = spreadsheetColumnIndex(columnInput.value);
        if (!Number.isInteger(row) || row < 1 || row > page.row_count || column === null || column >= page.column_count) {
          status.textContent = `Choose a row from 1 to ${page.row_count.toLocaleString()} and a column from A to ${spreadsheetColumnName(Math.max(0, page.column_count - 1))}.`;
          return;
        }
        void load(sheetIndex, Math.floor((row - 1) / page.page_size) * page.page_size, Math.floor(column / page.column_page_size) * page.column_page_size);
      });
      navigation.append(el(document_, 'div', { class: 'bx-sheet-page-buttons' }, previous, next, left, right), jump);
    } catch (error) {
      if (current !== generation || !modal.root.isConnected) return;
      status.textContent = refusalText(error);
      if (error instanceof Error && error.message === 'not_found') modal.actions.replaceChildren();
    }
  };
  void load(0, 0);
}
