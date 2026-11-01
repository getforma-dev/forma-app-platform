// APP-144: trusted presentation props become a closed DOM vocabulary. The
// JSON comes from the same authorized native document route; no HTML is parsed.
type Cell = { text: string; raw: string };
type Pair = { label: string; value: string; raw: string };
type View = {
  id: string; title: string; state: string; badge: string; message: string;
  has_summary: boolean; summary: Pair[]; has_table: boolean;
  columns: Cell[]; rows: { cells: Cell[] }[];
  has_receipt: boolean; receipt_digest: string; receipt: Pair[]; receipt_json: string;
};
type Field = {
  label: string; input: string; value: string; spec: string; spec_input: string;
  writable: boolean; read_only: boolean; has_text: boolean; has_choices: boolean;
  choices: { value: string }[];
};
type Form = {
  title: string; collection: string; command: string; action: string; csrf: string;
  idempotency_key: string; publication: string; schema: string; schema_version: string;
  record: string; record_version: string; work_version: string; fields: Field[]; submit: string;
};
type Records = {
  title: string; summary: string; collection: string; has_no_rows: boolean; has_rows: boolean;
  columns: Cell[];
  rows: { record_id?: string; version: string; committed_at: string; commit_sequence: string; cells: Cell[] }[];
};
export type DocumentProps = {
  app: string; work: string; action: string; forms: Form[]; record_cards: Records[];
  records_truncated: boolean; views: View[];
};

function element<K extends keyof HTMLElementTagNameMap>(
  document: Document, tag: K, attributes: Record<string, string> = {}, ...children: (Node | string)[]
): HTMLElementTagNameMap[K] {
  const node = document.createElement(tag);
  for (const [name, value] of Object.entries(attributes)) node.setAttribute(name, value);
  node.append(...children);
  return node;
}

function input(document: Document, name: string, value: string, type = 'hidden'): HTMLInputElement {
  return element(document, 'input', { type, name, value });
}

function formSection(document: Document, props: DocumentProps, form: Form): HTMLElement {
  const expectedAction = `/v1/work/${props.app}/${props.work}/commands/${form.command}`;
  if (!/^[a-z][a-z0-9_]{0,63}$/.test(form.command) || form.action !== expectedAction) throw new Error('invalid command action');
  const section = element(document, 'section', {
    class: 'view document-form', 'data-document-form': form.collection, 'aria-label': form.title,
  }, element(document, 'h2', {}, form.title));
  const node = element(document, 'form', { method: 'post', action: form.action, 'data-command': form.command });
  const hidden: Record<string, string> = {
    csrf: form.csrf, idempotency_key: form.idempotency_key, collection_id: form.collection,
    publication_id: form.publication, schema_id: form.schema, schema_version_expected: form.schema_version,
    record_id: form.record, expected_record_version: form.record_version, expected_work_version: form.work_version,
  };
  for (const [name, value] of Object.entries(hidden)) node.append(input(document, name, value));
  for (const field of form.fields) {
    const label = element(document, 'label', { class: 'document-field' }, element(document, 'span', {}, field.label));
    if (field.has_text) label.append(input(document, field.input, field.value, 'text'));
    if (field.has_choices) {
      const select = element(document, 'select', { name: field.input });
      for (const choice of field.choices) select.append(element(document, 'option', { value: choice.value }, choice.value));
      // Match the native form: an empty new field uses the first offered
      // choice; a declared existing value selects its exact choice.
      if (field.choices.some(choice => choice.value === field.value)) select.value = field.value;
      label.append(select);
    }
    if (field.read_only) label.append(element(document, 'span', {}, field.value));
    if (field.writable) label.append(input(document, field.spec_input, field.spec));
    node.append(label);
  }
  node.append(element(document, 'button', { type: 'submit' }, form.submit),
    element(document, 'p', { role: 'status', 'data-command-status': '' }));
  section.append(node);
  return section;
}

function heading(document: Document, columns: Cell[], record = false): HTMLTableSectionElement {
  const row = element(document, 'tr');
  for (const column of columns) row.append(element(document, 'th', { scope: 'col' }, column.text));
  if (record) row.append(element(document, 'th', { scope: 'col' }, 'Record'));
  return element(document, 'thead', {}, row);
}

function recordSection(document: Document, props: DocumentProps | null, card: Records): HTMLElement {
  const section = element(document, 'section', {
    class: 'view document-records', 'data-collection': card.collection, 'aria-label': card.title,
  }, element(document, 'h2', {}, card.title), element(document, 'p', {}, card.summary));
  if (card.has_no_rows) section.append(element(document, 'p', {}, 'No records yet.'));
  if (card.has_rows) {
    const body = element(document, 'tbody');
    for (const row of card.rows) {
      const tr = element(document, 'tr');
      for (const cell of row.cells) tr.append(element(document, 'td', {}, cell.text));
      const provenance = element(document, 'td');
      if (props && row.record_id) {
        const path = `/work/${encodeURIComponent(props.app)}/${encodeURIComponent(props.work)}/records/${encodeURIComponent(card.collection)}/${encodeURIComponent(row.record_id)}`;
        provenance.append(element(document, 'a', { href: path }, 'Open record'));
      }
      provenance.append(element(document, 'small', {}, `Revision ${row.version} · ${row.committed_at} · Commit ${row.commit_sequence}`));
      tr.append(provenance);
      body.append(tr);
    }
    section.append(element(document, 'table', {}, heading(document, card.columns, true), body));
  }
  return section;
}

export type ReadOnlyProps = Pick<DocumentProps, 'record_cards' | 'records_truncated' | 'views'>;
/** Shared recipients receive no ordinary record links, form controls or write actions. */
export function renderReadOnlyRegions(document: Document, props: ReadOnlyProps): HTMLElement {
  const root = element(document, 'div', { class: 'shared-parts', 'data-shared-parts': '' });
  const views = element(document, 'div', { 'data-document-views': '' });
  for (const card of props.views) views.append(viewSection(document, card));
  const records = element(document, 'div', { 'data-document-records': '' });
  for (const card of props.record_cards) records.append(recordSection(document, null, card));
  if (props.records_truncated) records.append(element(document, 'p', { class: 'notice' }, 'Showing the newest records. This listing has reached its limit.'));
  root.append(views, records);
  return root;
}

function viewSection(document: Document, card: View): HTMLElement {
  const section = element(document, 'section', {
    class: 'view', 'data-view': card.id, 'data-state': card.state, 'aria-label': card.title,
  }, element(document, 'h2', {}, card.title), element(document, 'p', { class: 'state', role: 'status' },
    element(document, 'span', { class: 'badge' }, card.badge), ' ', card.message));
  // Only a released ready result may carry values. Empty results can carry
  // their real receipt, and no failed/denied state can manufacture a number.
  if (card.state === 'ready' && card.has_summary) {
    const list = element(document, 'dl', { class: 'summary' });
    for (const item of card.summary) list.append(element(document, 'div', {},
      element(document, 'dt', {}, item.label), element(document, 'dd', { class: 'value', title: item.raw }, item.value)));
    section.append(list);
  }
  if (card.state === 'ready' && card.has_table) {
    const body = element(document, 'tbody');
    for (const row of card.rows) {
      const tr = element(document, 'tr');
      for (const cell of row.cells) tr.append(element(document, 'td', { class: 'value', title: cell.raw }, cell.text));
      body.append(tr);
    }
    section.append(element(document, 'table', {}, heading(document, card.columns), body));
  }
  if ((card.state === 'ready' || card.state === 'empty') && card.has_receipt) {
    const list = element(document, 'dl');
    for (const item of card.receipt) list.append(element(document, 'div', {},
      element(document, 'dt', {}, item.label), element(document, 'dd', {}, element(document, 'code', {}, item.value))));
    section.append(element(document, 'details', { class: 'receipt' },
      element(document, 'summary', {}, 'Receipt ', element(document, 'code', {}, card.receipt_digest)),
      list, element(document, 'pre', {}, card.receipt_json)));
  }
  return section;
}

/** The caller validates exact work identity before replacing these regions. */
export function renderRegions(document: Document, props: DocumentProps): {
  records: HTMLElement; views: HTMLElement; forms: HTMLElement;
} {
  const records = element(document, 'div', { 'data-document-records': '' });
  for (const card of props.record_cards) records.append(recordSection(document, props, card));
  if (props.records_truncated) records.append(element(document, 'p', { class: 'notice' },
    'Showing the newest records. This listing has reached its limit.'));
  const views = element(document, 'div', { 'data-document-views': '' });
  for (const card of props.views) views.append(viewSection(document, card));
  const forms = element(document, 'div', { class: 'document-forms', 'data-document-forms': '' });
  for (const form of props.forms) forms.append(formSection(document, props, form));
  return { records, views, forms };
}
