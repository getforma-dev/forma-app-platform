import { h, mount, createShow, createList, activateIslands } from '@getforma/core';
import { startDocument } from './document';

// APP-144 app document: native SSR with one registered enhancement island. Every value, state and
// receipt row is resolved by the Rust handler; this page only lays them out.
// A failed, denied, loading (shown as busy) or idle view carries no value at
// all, so nothing here can show a number for it. A successful run answers a
// redirect back to this page, which re-runs the saved parameters under a
// fresh authorization: a reload restores the views without a re-submit.
// `raw` is a result value's exact engine string (the hover title); the shown
// text is formatted by the column's value type (APP-093 review M4).
type Pair = { id: string; label: string; value: string; raw: string };
type Cell = { id: string; text: string; raw: string };
type Row = { id: string; cells: Cell[] };
type Card = {
  id: string;
  title: string;
  kind: string;
  state: string;
  badge: string;
  message: string;
  idle: boolean;
  loading: boolean;
  ready: boolean;
  empty: boolean;
  failed: boolean;
  denied: boolean;
  hasSummary: boolean;
  summary: Pair[];
  hasTable: boolean;
  columns: Cell[];
  rows: Row[];
  hasReceipt: boolean;
  receiptDigest: string;
  receipt: Pair[];
  receiptJson: string;
};
type Choice = { id: string; value: string };
type Field = {
  id: string;
  kind: string;
  label: string;
  input: string;
  value: string;
  spec: string;
  specInput: string;
  writable: boolean;
  readOnly: boolean;
  hasText: boolean;
  hasChoices: boolean;
  choices: Choice[];
};
type FormCard = {
  id: string;
  title: string;
  command: string;
  action: string;
  csrf: string;
  idempotencyKey: string;
  collection: string;
  publication: string;
  schema: string;
  schemaVersion: string;
  record: string;
  recordVersion: string;
  workVersion: string;
  fields: Field[];
  submit: string;
};
type Origin = { id: string; kind: string; text: string };
type RecordRow = {
  id: string;
  recordId: string;
  href: string;
  version: string;
  status: string;
  pending: boolean;
  cells: Cell[];
  creator: string;
  committedAt: string;
  commitSequence: string;
};
type RecordsCard = {
  id: string;
  title: string;
  summary: string;
  url: string;
  collection: string;
  schema: string;
  canWrite: boolean;
  canWriteText: string;
  decide: string;
  statusField: string;
  count: string;
  pending: string;
  waiting: string;
  hasRows: boolean;
  hasNoRows: boolean;
  columns: Cell[];
  rows: RecordRow[];
  publication: string;
  definitionDigest: string;
  workVersion: string;
};

type RecordLink = { id: string; label: string; href: string };
type WorkProps = {
  title: string;
  app: string;
  work: string;
  forms: FormCard[];
  recordCards: RecordsCard[];
  recordsTruncated: boolean;
  action: string;
  csrf: string;
  windowStart: string;
  windowEnd: string;
  destination: string;
  formError: boolean;
  views: Card[];
  hasQuery: boolean;
  recordsOnly: boolean;
  hasRecords: boolean;
  records: RecordLink[];
  canvas: string;
};

// This explicitly registered island retains the client entry for live updates
// and delegated audited commands; all app content is still in the native HTML.
function DocumentStatus(props: { path: string }) {
  return <p class="document-status" role="status" data-document-status="">
    <span data-document-state="">Updates connect when this page opens.</span>{' '}
    <button type="button" onClick={() => document.dispatchEvent(new CustomEvent('forma:document-refresh'))}>Refresh</button>
    <a href={props.path}>Reload app</a>
  </p>;
}

function Work(props: WorkProps) {
  return (
    <main class="page work" data-document="" data-app={props.app} data-work={props.work} data-path={props.action}>
      <a class="forma-brand" href="/" aria-label="Forma home"><span class="forma-mark-tile" aria-hidden="true"><span class="forma-mark"></span></span><span>forma<span class="forma-dot">.</span></span><span class="forma-product">canvas</span></a>
      <h1>{props.title}</h1>
      <p class="lede">
        One app, with the same saved work as your canvas.
      </p>
      <p><a href={props.canvas}>Open on the canvas</a></p>
      <DocumentStatus path={props.action} />
      {createShow(() => props.recordsOnly, () => (
        <p>This work keeps records. Changes appear here and on your canvas.</p>
      ))}
      {createShow(() => props.hasRecords, () => (
        <section aria-label="Records">
          <h2>Records</h2>
          <ul>
            {createList(() => props.records, record => record.id, record => (
              <li><a href={record.href}>{record.label}</a></li>
            ))}
          </ul>
        </section>
      ))}
      <div class="document-forms" data-document-forms="">
        {createList(() => props.forms, form => form.id, form => (
          <section class="view document-form" data-document-form={form.collection} aria-label={form.title}>
            <h2>{form.title}</h2>
            <form method="post" action={form.action} data-command={form.command}>
              <input type="hidden" name="csrf" value={form.csrf} />
              <input type="hidden" name="idempotency_key" value={form.idempotencyKey} />
              <input type="hidden" name="collection_id" value={form.collection} />
              <input type="hidden" name="publication_id" value={form.publication} />
              <input type="hidden" name="schema_id" value={form.schema} />
              <input type="hidden" name="schema_version_expected" value={form.schemaVersion} />
              <input type="hidden" name="record_id" value={form.record} />
              <input type="hidden" name="expected_record_version" value={form.recordVersion} />
              <input type="hidden" name="expected_work_version" value={form.workVersion} />
              {createList(() => form.fields, field => field.id, field => (
                <div class="document-field">
                  {createShow(() => field.hasText, () => <label>
                    <span>{field.label}</span><input name={field.input} type="text" value={field.value} />
                  </label>)}
                  {createShow(() => field.hasChoices, () => <label>
                    <span>{field.label}</span><select name={field.input} value={field.value}>
                      {createList(() => field.choices, choice => choice.id, choice => <option value={choice.value}>{choice.value}</option>)}
                    </select>
                  </label>)}
                  {createShow(() => field.readOnly, () => <p>{field.label}: {field.value}</p>)}
                  {createShow(() => field.writable, () => <input type="hidden" name={field.specInput} value={field.spec} />)}
                </div>
              ))}
              <button type="submit">{form.submit}</button>
              <p role="status" data-command-status=""></p>
            </form>
          </section>
        ))}
      </div>
      <div data-document-records="">
        {createList(() => props.recordCards, card => card.id, card => (
          <section class="view document-records" data-collection={card.collection} aria-label={card.title}>
            <h2>{card.title}</h2>
            <p>{card.summary}</p>
            {createShow(() => card.hasNoRows, () => <p>No records yet.</p>)}
            {createShow(() => card.hasRows, () => <table>
              <thead><tr>
                {createList(() => card.columns, column => column.id, column => <th scope="col">{column.text}</th>)}
                <th scope="col">Record</th>
              </tr></thead>
              <tbody>{createList(() => card.rows, row => row.id, row => <tr>
                {createList(() => row.cells, cell => cell.id, cell => <td>{cell.text}</td>)}
                <td><a href={row.href}>Open record</a>
                  <small>Revision {row.version} · {row.committedAt} · Commit {row.commitSequence}</small></td>
              </tr>)}</tbody>
            </table>)}
          </section>
        ))}
        {createShow(() => props.recordsTruncated, () => <p class="notice">Showing the newest records. This listing has reached its limit.</p>)}
      </div>
      {createShow(() => props.hasQuery, () => (
        <form method="post" action={props.action} class="run">
          <input type="hidden" name="csrf" value={props.csrf} />
          <div>
            <label for="window_start">Window start (UTC)</label>
            <input id="window_start" name="window_start" type="datetime-local" step="1" required value={props.windowStart} />
          </div>
          <div>
            <label for="window_end">Window end (UTC, excluded)</label>
            <input id="window_end" name="window_end" type="datetime-local" step="1" required value={props.windowEnd} />
          </div>
          <div>
            <label for="destination">Destination (optional)</label>
            <input id="destination" name="destination" type="text" value={props.destination} />
          </div>
          <button type="submit">Run views</button>
        </form>
      ))}
      {createShow(() => props.formError, () => (
        <p class="notice" role="alert">
          The window was not accepted. Give a start before the end, as UTC date and time. Nothing ran.
        </p>
      ))}
      <div data-document-views="">
      {createList(() => props.views, card => card.id, card => (
        <section class="view" data-view={card.id} data-state={card.state} aria-label={card.title}>
          <h2>{card.title}</h2>
          <p class="state" role="status">
            <span class="badge">{card.badge}</span> {card.message}
          </p>
          {createShow(() => card.hasSummary, () => (
            <dl class="summary">
              {createList(() => card.summary, item => item.id, item => (
                <div>
                  <dt>{item.label}</dt>
                  <dd class="value" title={item.raw}>{item.value}</dd>
                </div>
              ))}
            </dl>
          ))}
          {createShow(() => card.hasTable, () => (
            <table>
              <thead>
                <tr>{createList(() => card.columns, column => column.id, column => <th scope="col">{column.text}</th>)}</tr>
              </thead>
              <tbody>
                {createList(() => card.rows, row => row.id, row => (
                  <tr>{createList(() => row.cells, cell => cell.id, cell => <td class="value" title={cell.raw}>{cell.text}</td>)}</tr>
                ))}
              </tbody>
            </table>
          ))}
          {createShow(() => card.hasReceipt, () => (
            <details class="receipt">
              <summary>
                Receipt <code>{card.receiptDigest}</code>
              </summary>
              <dl>
                {createList(() => card.receipt, item => item.id, item => (
                  <div>
                    <dt>{item.label}</dt>
                    <dd>
                      <code>{item.value}</code>
                    </dd>
                  </div>
                ))}
              </dl>
              <pre>{card.receiptJson}</pre>
            </details>
          ))}
        </section>
      ))}
      </div>
      <p>
        <a href="/workspace">Workspace</a>
      </p>
    </main>
  );
}

await activateIslands({ DocumentStatus });
startDocument();

mount(
  () =>
    Work({
      title: '',
      app: '',
      work: '',
      forms: [],
      recordCards: [],
      recordsTruncated: false,
      action: '',
      csrf: '',
      windowStart: '',
      windowEnd: '',
      destination: '',
      formError: false,
      views: [],
      hasQuery: false,
      recordsOnly: true,
      hasRecords: false,
      records: [],
      canvas: '/canvas',
    }),
  '#app',
);
