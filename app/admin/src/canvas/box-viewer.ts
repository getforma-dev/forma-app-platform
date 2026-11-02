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
  const status = el(document_, 'p', { class: 'bx-status', role: 'status' }, 'Reading the original file…');
  const selection = el(document_, 'div', { class: 'bx-sheet-selection' });
  const table = el(document_, 'div', {});
  const navigation = el(document_, 'div', { class: 'bx-sheet-navigation' });
  modal.body.append(status, selection, table, navigation);
  modal.body.append(el(document_, 'p', { class: 'bx-sheet-limit' },
    'Original cell values, 50 rows per page. CSV shows logical records; Raw CSV also shows original quoting and blank lines. Excel shows stored values, including raw date serials, without styles, formulas or recalculation. Blank spreadsheet coordinates are retained through the last stored cell.'));
  modal.actions.append(el(document_, 'a', { href: file.content_url, download: file.name, class: 'bx-link' }, 'Download original file'));
  let generation = 0;
  let digest = '';
  if (file.kind === 'csv') {
    const raw = el(document_, 'button', { type: 'button', class: 'bx-link' }, 'Raw CSV');
    raw.addEventListener('click', () => {
      const current = ++generation;
      table.replaceChildren(); navigation.replaceChildren(); selection.replaceChildren();
      status.textContent = 'Reading original CSV bytes…';
      void fetch(file.content_url, { credentials: 'same-origin', cache: 'no-store' }).then(async (response) => {
        if (!response.ok) throw new Error('refused');
        const bytes = await response.arrayBuffer();
        if (bytes.byteLength > 20 * 1024 * 1024) throw new Error('too large');
        const text = new TextDecoder('utf-8', { fatal: true }).decode(bytes);
        if (current !== generation || !modal.root.isConnected) return;
        status.textContent = 'Original CSV text · all content, including blank lines and quoting';
        table.append(el(document_, 'pre', { class: 'bx-text', tabindex: '0', 'aria-label': 'Original CSV text' }, text));
      }).catch(() => {
        if (current === generation && modal.root.isConnected) status.textContent = 'The original CSV is unavailable. No cached content is shown.';
      });
    });
    const cells = el(document_, 'button', { type: 'button', class: 'bx-link' }, 'Table');
    cells.addEventListener('click', () => { void load(0, 0); });
    modal.actions.append(raw, cells);
  }
  const load = async (sheetIndex: number, offset: number): Promise<void> => {
    const current = ++generation;
    // Clear previous contents before each authority check, including failures.
    table.replaceChildren(); navigation.replaceChildren(); selection.replaceChildren();
    status.textContent = 'Reading the original file…';
    try {
      const page = await fetchOriginalPage(boxId, file.file_id, sheetIndex, offset);
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
      status.textContent = `Original file · rows ${start}–${offset + page.rows.length} of ${page.row_count} · ${page.column_count} columns${page.format === 'xlsx' ? ` · ${page.sheets.length} sheets` : ''}`;
      const columnName = (index: number): string => index < 26 ? String.fromCharCode(65 + index) : `A${String.fromCharCode(65 + index - 26)}`;
      const header = el(document_, 'tr', {}, el(document_, 'th', { scope: 'col' }, 'Row'),
        ...Array.from({ length: page.column_count }, (_, index) => el(document_, 'th', { scope: 'col' }, columnName(index))));
      const rows = page.rows.map((row, index) => el(document_, 'tr', {},
        el(document_, 'th', { scope: 'row' }, String(offset + index + 1)),
        ...row.map((cell) => el(document_, 'td', {}, cell))));
      table.append(el(document_, 'div', { class: 'bx-table-wrap bx-original-table', role: 'region', 'aria-label': 'Original spreadsheet cells', tabindex: '0' },
        el(document_, 'table', { class: 'bx-table' }, el(document_, 'thead', {}, header), el(document_, 'tbody', {}, ...rows))));
      const previous = el(document_, 'button', { type: 'button' }, 'Previous 50 rows');
      const next = el(document_, 'button', { type: 'button' }, 'Next 50 rows');
      previous.disabled = offset === 0;
      next.disabled = offset + page.rows.length >= page.row_count;
      previous.addEventListener('click', () => { void load(sheetIndex, Math.max(0, offset - page.page_size)); });
      next.addEventListener('click', () => { void load(sheetIndex, offset + page.page_size); });
      navigation.append(previous, next);
    } catch (error) {
      if (current !== generation || !modal.root.isConnected) return;
      status.textContent = 'This original file cannot be displayed here right now. No cells have been substituted or omitted. You can download it unchanged.';
      modal.body.append(el(document_, 'p', { class: 'bx-sheet-limit' },
        'Table limits: UTF-8 CSV up to 10 MB / 100,000 rows; XLSX up to 5 MB / 10,000 rows per sheet / 16 sheets; 32 columns, 320,000 cells and 2,000 characters / 8 KB per cell. Raw CSV and original downloads allow the full 20 MB upload limit. Formula-bearing, macro-enabled, encrypted, external-link or malformed workbooks are refused. XLS, XLSM, ODS and other formats are not supported.'));
      if (error instanceof Error && error.message === 'not_found') modal.actions.replaceChildren();
    }
  };
  void load(0, 0);
}
