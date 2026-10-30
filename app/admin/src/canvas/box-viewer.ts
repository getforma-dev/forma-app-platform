// APP-108: a minimal, self-contained file viewer for box files. APP-106
// (`pivot/focus-unite`) is building the canvas's focus modal; until it
// merges, this module is the stand-in, kept behind a tiny API
// (`openViewer(file, boxId)`, `openModal`) so it can be swapped for the focus
// modal without touching the box card.
//
// The modal: role=dialog, aria-modal, a labelled title, focus kept inside
// (Tab wraps), Esc, the close button and a backdrop click close it, and focus
// returns to where it was. Content is built from nodes only:
// - PDF: an <iframe> of the authorized same-origin content route (served
//   `inline` with `Content-Security-Policy: sandbox`, `nosniff`), plus an
//   "Open in a new tab" link for a browser that refuses a sandboxed PDF;
// - image: an <img> of the same route;
// - text / Markdown: the bytes fetched as text into a <pre> text node;
// - CSV / XLSX: the typed preview (`GET …/preview`) as a table whose header
//   shows each column's type, the data status, and "Make data".
import { fetchPreview, formatBytes, noun, refusalText, type BoxFile, type Preview } from './box-api';
import { el } from './render';

type Modal = { root: HTMLElement; body: HTMLElement; actions: HTMLElement; close(): void };
let open: Modal | null = null;

/** Opens a modal (one at a time); `onClose` runs once when it closes. */
export function openModal(title: string, subtitle: string, onClose?: () => void, wide = false): Modal {
  open?.close();
  const document_ = document;
  const previous = document_.activeElement as HTMLElement | null;
  const titleId = `bx-modal-title-${Math.random().toString(36).slice(2, 8)}`;
  const closeButton = el(document_, 'button', { type: 'button', class: 'bx-modal-close', 'aria-label': 'Close' }, '×');
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
  const root = el(document_, 'div', { class: 'bx-backdrop' }, dialog);
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
  // CSV / XLSX: the typed preview from the sealed parser.
  const status = el(document_, 'p', { class: 'bx-status', role: 'status' }, 'Reading the file…');
  modal.body.append(status);
  void fetchPreview(boxId, file.file_id).then((found) => {
    if (!found || 'refused' in found) {
      status.textContent = `This file could not be read as a table. It is kept in the ${noun} as it is.`;
      return;
    }
    status.textContent = file.data
      ? `Data ✓ · ${file.data.row_count} rows registered · ${found.columns.length} columns`
      : `${found.rows} rows · ${found.columns.length} columns · types inferred, not yet confirmed${found.sheet ? ` · sheet ${found.sheet}` : ''}`;
    modal.body.append(previewTable(document_, found));
    const make = el(document_, 'button', { type: 'button', class: 'bx-primary' }, file.data ? 'Adjust types' : 'Make data');
    make.addEventListener('click', () => {
      modal.close();
      makeDataHandler?.(boxId, file, found);
    });
    modal.actions.append(make);
  }).catch((error: unknown) => {
    status.textContent = refusalText(error);
  });
}
