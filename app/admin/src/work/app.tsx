import { RecordComponents } from '../record-components/parts';
import type { RecordComponent } from '../record-components/types';
import { h, mount, createShow, createList, activateIslands } from '@getforma/core';
import { startDocument } from './document';
import { initDocumentAsk } from '../ask/document';
import { startSharing } from '../sharing/dialog';
import { PresentationHeader } from '../presentation/header';
import type { PresentationProps } from '../presentation/presentation';
import { startPresentationEditor } from '../presentation/editor';

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
  hasChart: boolean;
  chart: {
    label: string; caption: string; viewBox: string; hasBars: boolean; hasLine: boolean; line: string; area: string;
    bars: { id: string; x: string; y: string; width: string; height: string; class: string; tip: string }[];
    grid: { id: string; y: string; labelY: string; text: string }[];
    ticks: { id: string; x: string; anchor: string; text: string }[];
  };
  hasReceipt: boolean;
  receiptDigest: string;
  receipt: Pair[];
  receiptJson: string;
};
type Choice = { id: string; value: string; selected: boolean };
type Field = {
  inputMode: string;
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

type Part = {
  id: string; anchor: string; title: string; kind: string; class: string; text: string; collection: string;
  isNote: boolean; isForm: boolean; hasSelector: boolean; selectionName: string; selected: string;
  choices: { id: string; label: string; selected: boolean }[];
  message: string; forms: FormCard[]; views: Card[]; recordComponents: RecordComponent[]; hasRecordComponents: boolean;
};

type RecordLink = { id: string; label: string; href: string };
type WorkProps = {
  presentation: PresentationProps; canEditPresentation: boolean;
  title: string;
  app: string;
  work: string;
  parts: Part[];
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
  canWindow: boolean;
  canDestination: boolean;
  recordsOnly: boolean;
  hasRecords: boolean;
  records: RecordLink[];
  canvas: string;
  canShare: boolean;
  sampleData: boolean;
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
    <main class="page work composed-app" data-document="" data-presentation={props.presentation.json} data-app-mode={props.presentation.mode} data-app-accent={props.presentation.accent} data-app-density={props.presentation.density} data-app-font={props.presentation.font} data-app={props.app} data-work={props.work} data-path={props.action} data-csrf={props.csrf}>
      <a class="forma-brand" href="/" aria-label="Forma home"><span class="forma-mark-tile" aria-hidden="true"><span class="forma-mark"></span></span><span>forma<span class="forma-dot">.</span></span><span class="forma-product">canvas</span></a>
      <h1 data-document-title="">{props.title}</h1>
      <PresentationHeader presentation={props.presentation} />
      <p class="lede">
        Your app, connected to your canvas.
      </p>
      {/* APP-098 DS-84: the library's synthetic sample data, never the member's own. */}
      {createShow(() => props.sampleData, () => (
        <p class="sample-note" data-sample-data=""><span class="badge">Sample data</span> The numbers here come from Forma's synthetic sample data, not your own.</p>
      ))}
      <p><a href={props.canvas}>Open on the canvas</a></p>
      <div class="document-actions">
      {createShow(() => props.canShare, () => <button type="button" class="share-primary" data-manage-share="" data-app={props.app} data-work={props.work} data-title={props.title}>Share app</button>)}
      {createShow(() => props.canEditPresentation, () => <button type="button" data-edit-presentation="" data-app={props.app} data-work={props.work}>Edit app</button>)}
      <button type="button" data-ask-app="">Ask about this app</button>
      </div>
      <div data-document-ask="" hidden></div>
      <DocumentStatus path={props.action} />
      {createShow(() => props.recordsOnly, () => (
        <p>Changes appear here and on your canvas.</p>
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
      {createShow(() => props.hasQuery, () => (
        <form method="post" action={props.action} class="run">
          <input type="hidden" name="csrf" value={props.csrf} />
          {createShow(() => props.canWindow, () => <div>
            <label for="window_start">Window start (UTC, optional)</label>
            <input id="window_start" name="window_start" type="datetime-local" step="1" value={props.windowStart} />
          </div>)}
          {createShow(() => props.canWindow, () => <div>
            <label for="window_end">Window end (UTC, excluded)</label>
            <input id="window_end" name="window_end" type="datetime-local" step="1" value={props.windowEnd} />
          </div>)}
          {createShow(() => props.canDestination, () => <div>
            <label for="destination">Destination (optional)</label>
            <input id="destination" name="destination" type="text" value={props.destination} />
          </div>)}
          <button type="submit">Run views</button>
        </form>
      ))}
      {createShow(() => props.formError, () => (
        <p class="notice" role="alert">
          The window was not accepted. Give a start before the end, as UTC date and time. Nothing ran.
        </p>
      ))}
      <div class="document-parts" data-document-parts="">
        {createList(() => props.parts, part => part.id, part => (
          <article id={part.anchor} class={part.class} data-document-part={part.id} data-component={part.id} data-view={part.id} data-kind={part.kind} data-collection={part.collection}>
            {createShow(() => part.hasRecordComponents, () => <RecordComponents components={part.recordComponents} />)}
            {createShow(() => part.isNote, () => <section class="view document-note"><h2>{part.title}</h2><p>{part.text}</p></section>)}
            {createShow(() => part.hasSelector, () => <form method="post" action={props.action} class="document-selection" data-document-selection="">
              <input type="hidden" name="csrf" value={props.csrf} />
              <input type="hidden" name="document_action" value="refresh" />
              <label><span>{part.title}: choose a record</span><select name={part.selectionName} value={part.selected}>
                {createList(() => part.choices, choice => choice.id, choice => <option value={choice.id} selected={choice.selected}>{choice.label}</option>)}
              </select></label>
              <button type="submit">Open record in this form</button>
            </form>)}
            {createShow(() => part.isForm, () => <p class="document-form-message">{part.message}</p>)}
        {createList(() => part.forms, form => form.id, form => (
          <section class="view document-form" data-document-form={form.id} aria-label={form.title}>
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
                    <span>{field.label}</span>{h('input', { name: field.input, type: 'text', inputmode: field.inputMode, value: field.value })}
                  </label>)}
                  {createShow(() => field.hasChoices, () => <label>
                    <span>{field.label}</span><select name={field.input} value={field.value}>
                      {createList(() => field.choices, choice => choice.id, choice => <option value={choice.value} selected={choice.selected}>{choice.value}</option>)}
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
      {createList(() => part.views, card => card.id, card => (
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
                    {createShow(() => card.hasChart, () => (
                      <figure class="document-chart">
                        <svg class="document-chart-svg" viewBox={card.chart.viewBox} role="img" aria-label={card.chart.label}>
                          <g class="document-grid" aria-hidden="true">
                            {createList(() => card.chart.grid, line => line.id, line => (
                              <line x1="52" x2="552" y1={line.y} y2={line.y} />
                            ))}
                          </g>
                          <g class="document-axis" aria-hidden="true">
                            {createList(() => card.chart.grid, line => line.id, line => (
                              <text x="44" y={line.labelY} text-anchor="end">{line.text}</text>
                            ))}
                            {createList(() => card.chart.ticks, tick => tick.id, tick => (
                              <text x={tick.x} y="180" text-anchor={tick.anchor}>{tick.text}</text>
                            ))}
                          </g>
                          {createShow(() => card.chart.hasBars, () => (
                            <g class="cv-bars">
                              {createList(() => card.chart.bars, bar => bar.id, bar => (
                                <rect class={bar.class} x={bar.x} y={bar.y} width={bar.width} height={bar.height} rx="2">
                                  <title>{bar.tip}</title>
                                </rect>
                              ))}
                            </g>
                          ))}
                          {createShow(() => card.chart.hasLine, () => (
                            <g class="cv-series">
                              <path class="document-area" d={card.chart.area} />
                              <path class="document-line" d={card.chart.line} pathLength={1} />
                            </g>
                          ))}
                        </svg>
                        <figcaption>{card.chart.caption}</figcaption>
                      </figure>
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
            <button type="button" class="document-ask-part" data-ask-card="">Ask about this part</button>
          </article>
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
      <p>
        <a href="/workspace">Workspace</a>
      </p>
    </main>
  );
}

await activateIslands({ DocumentStatus });
startDocument();
initDocumentAsk(document);
startSharing(document);
startPresentationEditor(document);

mount(
  () =>
    Work({
      presentation: { hasDescription: false, description: '', mode: 'default', accent: 'default', density: 'default', font: 'default', actions: [], json: '{}' }, canEditPresentation: false,
      title: '',
      app: '',
      work: '',
      parts: [],
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
      canWindow: false,
      canDestination: false,
      recordsOnly: true,
      hasRecords: false,
      records: [],
      canvas: '/canvas',
      canShare: false,
      sampleData: false,
    }),
  '#app',
);
