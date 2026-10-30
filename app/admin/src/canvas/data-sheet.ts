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
  attachFile, COLUMN_TYPES, makeApp, makeData, noun, refusalText,
  type BoxFile, type Card, type ConfirmedColumn, type ColumnType, type Preview,
} from './box-api';
import { openModal, previewTable } from './box-viewer';
import { boardWorks, placeWork } from './board';
import { announce, el } from './render';

export type SheetDone = { kind: 'card' | 'app' | 'attached' | 'data'; card: Card | null };

/** Opens the sheet for `file` (in box `boxId`) with its `preview`. */
export function openDataSheet(boxId: string, file: BoxFile, preview: Preview, onDone?: (done: SheetDone) => void, adjust = false): void {
  const document_ = document;
  const modal = openModal(adjust || file.data ? `Adjust types · ${file.name}` : `Make data · ${file.name}`,
    `${preview.rows} rows · ${preview.columns.length} columns${preview.sheet ? ` · sheet ${preview.sheet}` : ''} · types inferred by the parser; change any before you confirm`, undefined, true);
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

  keep.addEventListener('click', async () => {
    busy(true);
    status.textContent = 'Registering the data and placing its card…';
    try {
      const made = await makeData(boxId, file.file_id, columns(), true);
      if (made.card) await placeWork(made.card.app_id, made.card.work_id, []);
      finish({ kind: 'card', card: made.card }, made.card
        ? `${file.name} is data now (${made.data?.row_count ?? preview.rows} rows); its card is on the canvas.`
        : `${file.name} is registered as data. No card could be placed for you.`);
    } catch (error) {
      status.textContent = refusalText(error);
      busy(false);
    }
  });
  app.addEventListener('click', async () => {
    if (mode !== 'app') {
      mode = 'app';
      extra.replaceChildren(el(document_, 'label', { class: 'bx-field' }, 'App for', purpose));
      extra.hidden = false;
      app.textContent = 'Build the app';
      purpose.focus();
      return;
    }
    const words = purpose.value.trim() || 'an overview of this data';
    busy(true);
    status.textContent = 'Registering the data, then generating an app from this file…';
    try {
      await makeData(boxId, file.file_id, columns(), false);
      const built = await makeApp(boxId, words, [file.file_id]);
      await placeWork(built.app_id, built.work_id, []);
      finish({ kind: 'app', card: { app_id: built.app_id, work_id: built.work_id, title: built.title } }, `${built.title} was generated from ${file.name} and placed.`);
    } catch (error) {
      status.textContent = refusalText(error);
      busy(false);
    }
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
      if (attached.card) await placeWork(attached.card.app_id, attached.card.work_id, []);
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
  modal.actions.append(cancel, add, app, keep);
  keep.focus();
}
