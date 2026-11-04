// APP-108: "Make data", the confirm-columns sheet (APP-018 lite). The sealed
// parser's inferred types arrive pre-selected; the member can change any
// column's type or nullability, then:
// - "Keep as card": registers the immutable source revision and places its
//   data card (a typed table with its receipt);
// - "Make an app from this": registers it, then asks the composer for an app
//   from this one file under the capped context (`POST /v1/boxes/{box}/apps`);
// - "Add to app…": registers it, attaches the file to the chosen app's box
//   and places the data card beside that app (the server answers the card);
// - "Cancel".
// Re-confirming a file that already has data ("Adjust types") registers a new
// revision; earlier revisions and the cards over them stay as they were.
import {
  attachFile, COLUMN_TYPES, makeApp, makeData, noun, refusalText, idempotencyKey,
  type BoxFile, type Card, type ConfirmedColumn, type ColumnType, type Preview,
} from './box-api';
import { openModal, previewTable } from './box-viewer';
import { boardWorks, markStandaloneWork, placeWork, ensureAppGroup, emitCanvasChanged } from './board';
import { announce, el, isUuid } from './render';

export type SheetDone = { kind: 'card' | 'app' | 'attached' | 'data'; card: Card | null };
export type DataSheetAppAttempt = { key: string; registered: boolean; request?: { purpose: string; columns: ConfirmedColumn[] }; built?: Awaited<ReturnType<typeof makeApp>> };
export type DataSheetOptions = { appOnly?: boolean; placeApp?: (card: Card, isCurrent: () => boolean) => Promise<boolean | void>; attempt?: DataSheetAppAttempt; onRelease?: () => void };

// Mirrors the server's MAX_CONTEXT_DATA_FILES for guidance and result validation;
// selection remains server-owned and file_ids is sent unchanged.
const APP_TABLE_LIMIT = 3;

/** Report only the server's actual used count, never the requested count as used. */
export function appFileSummary(requested: number, used: number): string {
  if (!Number.isSafeInteger(requested) || requested < 1 || requested > 20
    || !Number.isSafeInteger(used) || used < 1 || used > Math.min(APP_TABLE_LIMIT, requested)) {
    throw new Error('invalid_app_file_count');
  }
  return requested === 1 ? 'using 1 requested file' : `using ${used} of ${requested} requested files`;
}

/** Opens the sheet for `file` (in box `boxId`) with its `preview`. `also` names other spreadsheets requested for the server's capped app context. */
export function openDataSheet(boxId: string, file: BoxFile, preview: Preview, onDone?: (done: SheetDone) => void, adjust = false, also?: readonly string[], options: DataSheetOptions = {}): void {
  const document_ = document;
  const fileIds = [...new Set([file.file_id, ...(also ?? []).filter(isUuid)])];
  const many = fileIds.length > 1;
  const attempt = options.attempt ?? { key: idempotencyKey(), registered: false };
  let appBusy = false, closed = false;
  const release = () => { if (closed && !appBusy) options.onRelease?.(); };
  const modal = openModal(options.appOnly ? `Make an app · ${file.name}` : adjust || file.data ? `Adjust types · ${file.name}` : `Make data · ${file.name}`,
    `${preview.rows} rows · ${preview.columns.length} columns${preview.sheet ? ` · sheet ${preview.sheet}` : ''} · types inferred by the parser; change any before you confirm${many ? `. ${fileIds.length} files requested; apps use up to ${APP_TABLE_LIMIT} readable tables, with confirmed tables first, then the newest.` : ''}`, () => { closed = true; release(); }, true);
  const choices = preview.columns.map((column) => {
    const select = el(document_, 'select', { 'aria-label': `Type of ${column.header}` },
      ...COLUMN_TYPES.map((type) => {
        const option = el(document_, 'option', { value: type }, type);
        if (type === column.type) option.setAttribute('selected', '');
        return option;
      }));
    const nullable = el(document_, 'input', { type: 'checkbox', 'aria-label': `${column.header} may be empty` });
    nullable.checked = column.nullable;
    const row = el(document_, 'li', { class: 'bx-col' },
      el(document_, 'span', { class: 'bx-col-name', title: column.id }, column.header),
      select,
      el(document_, 'label', { class: 'bx-col-null' }, nullable, 'may be empty'));
    return { column, select, nullable, row };
  });
  const columns = (): ConfirmedColumn[] => choices.map(({ column, select, nullable }) => {
    const type = select.value as ColumnType;
    const confirmed: ConfirmedColumn = { id: column.id, type, nullable: nullable.checked };
    if (type === 'decimal') confirmed.scale = column.scale ?? 2;
    return confirmed;
  });
  const status = el(document_, 'p', { class: 'bx-status', role: 'status' }, '');
  modal.body.append(
    ...(many ? [el(document_, 'p', { class: 'bx-hint' }, `These columns belong to ${file.name} only. Other requested files use their saved types or inferred defaults. Unreadable files may be skipped; the result reports how many were used.`)] : []),
    el(document_, 'ul', { class: 'bx-cols', 'aria-label': 'Columns and their types' }, ...choices.map((choice) => choice.row)),
    el(document_, 'p', { class: 'bx-hint' }, `First ${Math.min(preview.sample.length, 20)} rows as the parser read them:`),
    previewTable(document_, preview),
    status);

  // The purpose of an app, and the app to add to, open in place.
  const purpose = el(document_, 'input', { type: 'text', maxlength: '200', placeholder: 'What should the app show? e.g. metrics by month', 'aria-label': 'Purpose of the app' });
  const works = boardWorks();
  const target = el(document_, 'select', { 'aria-label': 'App to add this data to' },
    ...works.map((work) => el(document_, 'option', { value: `${work.app}/${work.work}` }, work.title)));
  const extra = el(document_, 'div', { class: 'bx-extra', hidden: '' });

  const busy = (on: boolean) => {
    for (const button of modal.actions.querySelectorAll('button')) button.disabled = on;
    modal.root.querySelector('.bx-modal')?.setAttribute('aria-busy', String(on));
  };
  const finish = (done: SheetDone, message: string) => {
    announce(message);
    onDone?.(done);
    modal.close();
  };

  const keep = el(document_, 'button', { type: 'button', class: 'bx-primary' }, 'Keep as card');
  const app = el(document_, 'button', { type: 'button' }, 'Make an app from this');
  const add = el(document_, 'button', { type: 'button' }, 'Add to app…');
  const cancel = el(document_, 'button', { type: 'button', class: 'bx-quiet' }, 'Cancel');
  let mode: 'none' | 'app' | 'add' = 'none';
  if (attempt.request) {
    purpose.value = attempt.request.purpose;
    purpose.disabled = true;
    for (const choice of choices) {
      const prior = attempt.request.columns.find(column => column.id === choice.column.id);
      if (prior) { choice.select.value = prior.type; choice.nullable.checked = prior.nullable; }
      choice.select.disabled = true; choice.nullable.disabled = true;
    }
  }

  keep.addEventListener('click', async () => {
    busy(true);
    status.textContent = 'Registering the data and placing its card…';
    try {
      const made = await makeData(boxId, file.file_id, columns(), true);
      if (made.card) { markStandaloneWork(made.card.app_id, made.card.work_id); await placeWork(made.card.app_id, made.card.work_id, []); }
      finish({ kind: 'card', card: made.card }, made.card
        ? `${file.name} is data now (${made.data?.row_count ?? preview.rows} rows); its card is on the canvas.`
        : `${file.name} is registered as data. No card could be placed for you.`);
    } catch (error) {
      status.textContent = refusalText(error);
      busy(false);
    }
  });
  app.addEventListener('click', async () => {
    if (appBusy) return;
    if (mode !== 'app') {
      mode = 'app';
      extra.replaceChildren(el(document_, 'label', { class: 'bx-field' }, 'App for', purpose));
      extra.hidden = false;
      app.textContent = 'Build the app';
      purpose.focus();
      return;
    }
    attempt.request ??= { purpose: purpose.value.trim() || 'an overview of this data', columns: columns() };
    // A retry repeats this exact creation intent, even after a lost response.
    purpose.disabled = true;
    for (const choice of choices) { choice.select.disabled = true; choice.nullable.disabled = true; }
    appBusy = true;
    busy(true);
    status.textContent = many
      ? `Registering this file's columns, then requesting an app from up to ${Math.min(APP_TABLE_LIMIT, fileIds.length)} of ${fileIds.length} files…`
      : 'Registering the data, then generating an app from this file…';
    try {
      if (!attempt.registered) { await makeData(boxId, file.file_id, attempt.request.columns, false); attempt.registered = true; }
      attempt.built ??= await makeApp(boxId, attempt.request.purpose, fileIds, attempt.key);
      const built = attempt.built;
      const used = appFileSummary(fileIds.length, built.files);
      if (!modal.root.isConnected) {
        const group = await ensureAppGroup(built.app_id, built.work_id);
        emitCanvasChanged();
        announce(`${built.title} was saved. ${group ? 'Find it in Apps.' : 'Refresh to find it in Apps.'}`);
        return;
      }
      const placed = options.placeApp ? await options.placeApp({ app_id: built.app_id, work_id: built.work_id, title: built.title }, () => !closed && modal.root.isConnected)
        : await placeWork(built.app_id, built.work_id, []);
      finish({ kind: 'app', card: { app_id: built.app_id, work_id: built.work_id, title: built.title } },
        placed === false ? `${built.title} was saved ${used}. The file card was left as it was. Refresh to find the app in Apps.` : `${built.title} was generated ${used} and placed.`);
    } catch (error) {
      status.textContent = refusalText(error);
      if (!attempt.registered) {
        // Registration failed before any app request: allow type corrections.
        delete attempt.request;
        purpose.disabled = false;
        for (const choice of choices) { choice.select.disabled = false; choice.nullable.disabled = false; }
      }
      busy(false);
    } finally { appBusy = false; release(); }
  });
  add.addEventListener('click', async () => {
    if (mode !== 'add') {
      mode = 'add';
      extra.replaceChildren(works.length
        ? el(document_, 'label', { class: 'bx-field' }, 'Add to', target)
        : el(document_, 'p', { class: 'bx-hint' }, 'Bring the app onto the canvas first (the library pane, L), then add this data to it.'));
      extra.hidden = false;
      add.textContent = 'Add to this app';
      return;
    }
    const [appId, workId] = target.value.split('/');
    if (!appId || !workId) return;
    busy(true);
    status.textContent = 'Registering the data and adding it to the app…';
    try {
      await makeData(boxId, file.file_id, columns(), false);
      const attached = await attachFile(boxId, file.file_id, appId, workId);
      if (attached.card) { markStandaloneWork(attached.card.app_id, attached.card.work_id); await placeWork(attached.card.app_id, attached.card.work_id, []); }
      const title = works.find((work) => work.app === appId && work.work === workId)?.title ?? 'the app';
      finish({ kind: 'attached', card: attached.card }, `${file.name} was added to ${title}’s ${noun}${attached.card ? '; its data card is beside the app' : ''}.`);
    } catch (error) {
      status.textContent = refusalText(error);
      busy(false);
    }
  });
  cancel.addEventListener('click', () => modal.close());
  purpose.addEventListener('keydown', (event) => {
    if (event.key === 'Enter') {
      event.preventDefault();
      app.click();
    }
  });
  modal.body.append(extra);
  if (options.appOnly) {
    mode = 'app';
    extra.replaceChildren(el(document_, 'label', { class: 'bx-field' }, 'App for', purpose));
    extra.hidden = false;
    app.textContent = 'Build the app';
    app.classList.add('bx-primary');
    modal.actions.append(cancel, app);
    purpose.focus();
  } else {
    modal.actions.append(cancel, add, app, keep);
    keep.focus();
  }
}
