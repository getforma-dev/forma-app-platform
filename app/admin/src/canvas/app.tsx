import { h, mount, createShow, createList, createSignal, activateIslands } from '@getforma/core';
import { submitUtterance } from './actions';
import { startAgents } from './agents';
import { startBoard } from './board';
import { registerCommands, startSlash } from './slash';
// APP-103 utility widgets, registered as canvas slash commands (APP-101):
// `/weather`, `/calculator`, `/clock` in the `/` list and `/apps`; a command
// with words restyles the selected widget of that kind (`runWidget`).
import { restoreWidgets, runWidget, WIDGET_COMMANDS } from './widgets/index';

// ADR-011 pitch canvas (APP-092, APP-094): the signed-in workspace as one
// canvas. Every group, card, value and receipt row below is resolved by the
// Rust handler (`query::canvas`) and rendered natively, so the page reads
// without JavaScript. The canvas island (`startBoard`, after activation)
// then lifts the server-rendered cards onto the pan/zoom surface, runs the
// views through the existing JSON query route and posts the record forms to
// the existing command route. A card in any state but `ready` carries no
// value, and `ready`/`empty` always carry their receipt.
type Pair = { id: string; label: string; value: string };
type Cell = { id: string; text: string };
type Stat = { id: string; label: string; value: string; exact: string };
type ShownCell = { id: string; text: string; exact: string; class: string };
type Row = { id: string; cells: ShownCell[] };
type Bar = { id: string; x: string; y: string; width: string; height: string; class: string; tip: string };
type Grid = { id: string; y: string; labelY: string; text: string };
type Tick = { id: string; x: string; anchor: string; text: string };
type Chart = {
  label: string;
  caption: string;
  viewBox: string;
  hasBars: boolean;
  bars: Bar[];
  hasLine: boolean;
  line: string;
  area: string;
  grid: Grid[];
  ticks: Tick[];
};
type Card = {
  id: string;
  view: string;
  title: string;
  kind: string;
  state: string;
  badge: string;
  hasMessage: boolean;
  message: string;
  query: string;
  integerColumns: string;
  hasInsight: boolean;
  insightLabel: string;
  insight: string;
  insightExact: string;
  hasStats: boolean;
  stats: Stat[];
  hasChart: boolean;
  chart: Chart;
  hasTable: boolean;
  hasOpenTable: boolean;
  hasFoldedTable: boolean;
  rowsLabel: string;
  columns: Cell[];
  rows: Row[];
  hasReceipt: boolean;
  receiptDigest: string;
  receiptShort: string;
  receiptSources: string;
  receiptTime: string;
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
  hasRows: boolean;
  hasNoRows: boolean;
  columns: Cell[];
  rows: RecordRow[];
  publication: string;
  definitionDigest: string;
  workVersion: string;
};
// APP-102: the expense ledger card (`query::canvas_ledger`).
type LedgerRow = { id: string; label: string; count: string; total: string };
type LedgerCard = {
  id: string;
  title: string;
  url: string;
  collection: string;
  amountField: string;
  dateField: string;
  categoryField: string;
  statusField: string;
  scope: string;
  total: string;
  count: string;
  hasRows: boolean;
  hasNoRows: boolean;
  people: LedgerRow[];
  categories: LedgerRow[];
};
type Group = {
  id: string;
  app: string;
  work: string;
  appCard: string;
  title: string;
  label: string;
  origins: Origin[];
  href: string;
  csrf: string;
  windowStart: string;
  windowEnd: string;
  destination: string;
  restored: boolean;
  hasCards: boolean;
  cards: Card[];
  hasForms: boolean;
  forms: FormCard[];
  hasRecords: boolean;
  records: RecordsCard[];
  hasLedger: boolean;
  ledger: LedgerCard[];
};
type CanvasProps = {
  csrfSignout: string;
  hasGroups: boolean;
  groups: Group[];
  summary: string;
  liveEvents: string;
  hasLive: boolean;
  hasOrganization: boolean;
  organization: string;
  memberLabel: string;
  memberInitials: string;
  demo: boolean;
  demoMode: string;
};

type IntentProps = { placeholder: string };

// The intent bar: the one Forma island on the page. It posts the intent to
// the composer seam (`compose.ts`) and reports its state; placing the new
// app's cards is the board's job.
function IntentBar(props: IntentProps) {
  const [status, setStatus] = createSignal('');
  const [busy, setBusy] = createSignal(false);
  const [statusClass, setStatusClass] = createSignal('cv-intent-status');
  const report = (message: string) => {
    setStatus(message);
    setStatusClass(message ? 'cv-intent-status is-shown' : 'cv-intent-status');
  };
  return (
    <form
      id="intent-form"
      class="cv-intent"
      method="post"
      action="/canvas"
      autocomplete="off"
      aria-label="Describe an app"
      onSubmit={(event: Event) => {
        event.preventDefault();
        const form = event.currentTarget as HTMLFormElement;
        const input = form.elements.namedItem('intent') as HTMLInputElement | null;
        const text = input ? input.value.trim() : '';
        if (!text || busy()) return;
        setBusy(true);
        void submitUtterance(text, report).finally(() => setBusy(false));
      }}
    >
      <label class="sr-only" for="intent-input">What do you need?</label>
      <span class="cv-prompt" aria-hidden="true">
        <svg viewBox="0 0 20 20" width="18" height="18">
          <path d="M10 2.5l1.6 4.3 4.4 1.6-4.4 1.6L10 14.3 8.4 10 4 8.4l4.4-1.6z" />
          <path d="M15.5 13l.7 1.8 1.8.7-1.8.7-.7 1.8-.7-1.8-1.8-.7 1.8-.7z" />
        </svg>
      </span>
      <input
        id="intent-input"
        name="intent"
        type="text"
        maxLength={1000}
        placeholder={props.placeholder}
        aria-describedby="intent-status"
      />
      <button id="intent-submit" type="submit" disabled={() => busy()}>
        {() => (busy() ? 'Composing…' : 'Compose')}
      </button>
      <p id="intent-status" class={() => statusClass()} role="status">
        {() => status()}
      </p>
    </form>
  );
}

function Canvas(props: CanvasProps) {
  return (
    <div id="cv-shell" class="cv-shell">
      <header class="cv-top">
        <div class="cv-identity">
          <a class="cv-brand" href="/canvas" aria-label="Forma canvas">
            <span class="cv-brand-mark" aria-hidden="true"></span>
            <span>Forma</span>
          </a>
          {createShow(() => props.hasOrganization, () => (
            <span class="cv-org" title="Your organization">{props.organization}</span>
          ))}
          {createShow(() => props.demo, () => (
            <span class="cv-demo-note" title="The organizations, people and records here are synthetic">Synthetic data</span>
          ))}
        </div>
        <div class="cv-top-status">
          <span id="cv-summary" class="cv-count">{props.summary}</span>
          {createShow(() => props.hasLive, () => (
            <span id="cv-live" class="cv-live" data-state="connecting" role="status" hidden>
              <span class="cv-live-dot" aria-hidden="true"></span>
              <span data-live-text>Live</span>
            </span>
          ))}
          <a class="cv-link" href="/workspace">Workspace</a>
          <span class="cv-avatar" title={props.memberLabel} aria-label={props.memberLabel} role="img">
            {props.memberInitials}
          </span>
          <form method="post" action="/auth/signout" class="cv-signout">
            <input type="hidden" name="csrf" value={props.csrfSignout} />
            <button type="submit">Sign out</button>
          </form>
        </div>
      </header>
      <main id="cv-canvas" class="cv-canvas" aria-label="Canvas" data-live-events={props.liveEvents} data-demo={props.demoMode}>
        {props.hasGroups ? null : (
          <section id="cv-empty" class="cv-empty" aria-labelledby="cv-empty-title">
            <h1 id="cv-empty-title">What do you need?</h1>
            <p class="cv-empty-copy">
              Describe the app you need. Forma assembles it from admitted components, runs it on your
              organization's data and puts a receipt on every number.
            </p>
            <div class="cv-suggestions" aria-label="Suggested intents">
              <button type="button" data-intent="Review expenses over $200">Review expenses over $200</button>
              <button type="button" data-intent="Investigate outbound traffic from build-worker-17">
                Investigate outbound traffic from build-worker-17
              </button>
              <button type="button" data-intent="Vendor spend this quarter">Vendor spend this quarter</button>
            </div>
          </section>
        )}
        <div id="cv-board" class="cv-board">
          {createList(() => props.groups, group => group.id, group => (
            <section
              class="cv-group"
              id={group.id}
              data-app={group.app}
              data-work={group.work}
              aria-label={group.title}
            >
              <article class="cv-card cv-app" data-card={group.appCard} data-kind="app" aria-label={group.title}>
                <h2>{group.title}</h2>
                <p class="cv-origins" aria-label="How this app was assembled" title={group.label}>
                  {createList(() => group.origins, origin => origin.id, origin => (
                    <span class="cv-origin" data-origin={origin.kind}>{origin.text}</span>
                  ))}
                </p>
                <p class="sr-only">{group.label}</p>
                <form method="post" action={group.href} class="cv-window" data-window>
                  <input type="hidden" name="csrf" value={group.csrf} />
                  <label>
                    <span>From (UTC)</span>
                    <input name="window_start" type="datetime-local" step="1" value={group.windowStart} />
                  </label>
                  <label>
                    <span>Until (UTC, excluded)</span>
                    <input name="window_end" type="datetime-local" step="1" value={group.windowEnd} />
                  </label>
                  <label>
                    <span>Destination (optional)</span>
                    <input name="destination" type="text" value={group.destination} />
                  </label>
                  <div class="cv-window-actions">
                    <button type="submit">Run views</button>
                    <a href={group.href}>Open work page</a>
                  </div>
                </form>
              </article>
              {createList(() => group.cards, card => card.id, card => (
                <article
                  class="cv-card cv-view"
                  id={card.id}
                  data-card={card.id}
                  data-view={card.view}
                  data-kind={card.kind}
                  data-state={card.state}
                  data-query={card.query}
                  data-integers={card.integerColumns}
                  aria-label={card.title}
                >
                  <header class="cv-card-head">
                    <h3>{card.title}</h3>
                    <span class="badge">{card.badge}</span>
                  </header>
                  <div class="cv-card-body" data-body>
                    {createShow(() => card.hasMessage, () => (
                      <p class="cv-message" role="status">{card.message}</p>
                    ))}
                    {createShow(() => card.hasInsight, () => (
                      <div class="cv-insight">
                        <p class="cv-insight-label">{card.insightLabel}</p>
                        <p class="cv-insight-value value" title={card.insightExact}>{card.insight}</p>
                        <p class="cv-insight-exact">{card.insightExact}</p>
                        {createShow(() => card.hasStats, () => (
                          <dl class="cv-stats">
                            {createList(() => card.stats, stat => stat.id, stat => (
                              <div>
                                <dt>{stat.label}</dt>
                                <dd class="value" title={stat.exact}>{stat.value}</dd>
                              </div>
                            ))}
                          </dl>
                        ))}
                      </div>
                    ))}
                    {createShow(() => card.hasChart, () => (
                      <figure class="cv-chart">
                        <svg class="cv-chart-svg" viewBox={card.chart.viewBox} role="img" aria-label={card.chart.label}>
                          <g class="cv-grid" aria-hidden="true">
                            {createList(() => card.chart.grid, line => line.id, line => (
                              <line x1="52" x2="552" y1={line.y} y2={line.y} />
                            ))}
                          </g>
                          <g class="cv-axis" aria-hidden="true">
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
                              <path class="cv-area" d={card.chart.area} />
                              <path class="cv-line" d={card.chart.line} pathLength={1} />
                            </g>
                          ))}
                        </svg>
                        <figcaption>{card.chart.caption}</figcaption>
                      </figure>
                    ))}
                    {createShow(() => card.hasOpenTable, () => (
                      <div class="cv-table">
                        <table>
                          <thead>
                            <tr>{createList(() => card.columns, column => column.id, column => <th scope="col">{column.text}</th>)}</tr>
                          </thead>
                          <tbody>
                            {createList(() => card.rows, row => row.id, row => (
                              <tr>{createList(() => row.cells, cell => cell.id, cell => <td class={cell.class} title={cell.exact}>{cell.text}</td>)}</tr>
                            ))}
                          </tbody>
                        </table>
                      </div>
                    ))}
                    {createShow(() => card.hasFoldedTable, () => (
                      <details class="cv-rows">
                        <summary>{card.rowsLabel}</summary>
                        <div class="cv-table">
                          <table>
                            <thead>
                              <tr>{createList(() => card.columns, column => column.id, column => <th scope="col">{column.text}</th>)}</tr>
                            </thead>
                            <tbody>
                              {createList(() => card.rows, row => row.id, row => (
                                <tr>{createList(() => row.cells, cell => cell.id, cell => <td class={cell.class} title={cell.exact}>{cell.text}</td>)}</tr>
                              ))}
                            </tbody>
                          </table>
                        </div>
                      </details>
                    ))}
                    {createShow(() => card.hasReceipt, () => (
                      <details class="cv-receipt" aria-label="Receipt">
                        <summary title={card.receiptDigest}>
                          <svg class="cv-seal" viewBox="0 0 16 16" width="16" height="16" aria-hidden="true">
                            <path d="M8 1.2l5.4 2v4.1c0 3.3-2.3 6.2-5.4 7.3C4.9 13.5 2.6 10.6 2.6 7.3V3.2z" />
                            <path class="cv-seal-check" d="M5.4 8.1l1.8 1.8 3.5-3.6" />
                          </svg>
                          <span class="cv-verified">Verified</span>
                          <code class="cv-receipt-id">{card.receiptShort}</code>
                          <span class="cv-receipt-meta">{card.receiptSources}</span>
                          <span class="cv-receipt-meta">{card.receiptTime}</span>
                          <span class="cv-receipt-toggle" aria-hidden="true"></span>
                        </summary>
                        <div class="cv-receipt-body">
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
                          <details class="cv-receipt-raw">
                            <summary>Receipt document (JSON)</summary>
                            <pre>{card.receiptJson}</pre>
                          </details>
                          <p class="cv-receipt-note">Received {card.receiptTime}, as this page saw it; the time is not part of the receipt.</p>
                        </div>
                      </details>
                    ))}
                  </div>
                </article>
              ))}
              {createList(() => group.forms, form => form.id, form => (
                <article class="cv-card cv-form" data-card={form.id} data-kind="form" aria-label={form.title}>
                  <header class="cv-card-head">
                    <h3>{form.title}</h3>
                    <span class="badge">record</span>
                  </header>
                  <form method="post" action={form.action} class="cv-command" data-command={form.command}>
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
                      <div class="cv-field" data-field={field.id}>
                        <span class="cv-field-label">{field.label}</span>
                        {createShow(() => field.hasChoices, () => (
                          <select name={field.input} aria-label={field.label}>
                            {createList(() => field.choices, choice => choice.id, choice => (
                              <option value={choice.value}>{choice.value}</option>
                            ))}
                          </select>
                        ))}
                        {createShow(() => field.hasText, () => (
                          <input type="text" name={field.input} value={field.value} aria-label={field.label} />
                        ))}
                        {createShow(() => field.writable, () => (
                          <input type="hidden" name={field.specInput} value={field.spec} />
                        ))}
                        {createShow(() => field.readOnly, () => (
                          <span class="cv-muted">not changed by this command</span>
                        ))}
                      </div>
                    ))}
                    <button type="submit">{form.submit}</button>
                  </form>
                  <p class="cv-outcome" role="status" data-outcome></p>
                </article>
              ))}
              {createList(() => group.records, records => records.id, records => (
                <article
                  class="cv-card cv-records"
                  data-card={records.id}
                  data-kind="records"
                  data-records={records.url}
                  data-collection={records.collection}
                  data-schema={records.schema}
                  data-decide={records.decide}
                  data-status-field={records.statusField}
                  data-can-write={records.canWriteText}
                  data-state="ready"
                  aria-label={records.title}
                >
                  <header class="cv-card-head">
                    <h3>{records.title}</h3>
                    <span class="badge">{records.count} records</span>
                  </header>
                  <p class="cv-records-summary" data-summary>{records.summary}</p>
                  <div class="cv-card-body" data-body>
                    {createShow(() => records.hasNoRows, () => (
                      <p class="cv-message" role="status">No records yet.</p>
                    ))}
                    {createShow(() => records.hasRows, () => (
                      <div class="cv-table cv-records-table">
                        <table>
                          <thead>
                            <tr>{createList(() => records.columns, column => column.id, column => <th scope="col">{column.text}</th>)}</tr>
                          </thead>
                          <tbody>
                            {createList(() => records.rows, row => row.id, row => (
                              <tr data-record={row.recordId} data-version={row.version} data-status={row.status}>
                                {createList(() => row.cells, cell => cell.id, cell => <td class="value">{cell.text}</td>)}
                              </tr>
                            ))}
                          </tbody>
                        </table>
                      </div>
                    ))}
                  </div>
                  <p class="cv-records-provenance" data-provenance>
                    <span class="cv-verified">Records</span>
                    <code>publication {records.publication}</code>
                    <code>{records.definitionDigest}</code>
                    <span class="cv-receipt-meta">work v{records.workVersion}</span>
                  </p>
                  <p class="cv-outcome" role="status" data-outcome></p>
                </article>
              ))}
              {createList(() => group.ledger, ledger => ledger.id, ledger => (
                <article
                  class="cv-card cv-ledger"
                  data-card={ledger.id}
                  data-kind="ledger"
                  data-records={ledger.url}
                  data-collection={ledger.collection}
                  data-amount={ledger.amountField}
                  data-date={ledger.dateField}
                  data-category={ledger.categoryField}
                  data-status-field={ledger.statusField}
                  data-state="ready"
                  aria-label={ledger.title}
                >
                  <header class="cv-card-head">
                    <h3>{ledger.title}</h3>
                    <span class="badge">{ledger.scope}</span>
                  </header>
                  <div class="cv-ledger-controls" data-controls></div>
                  <div class="cv-card-body" data-body>
                    <p class="cv-ledger-total"><span class="cv-ledger-total-value num">{ledger.total}</span> <span class="cv-muted">across {ledger.count} records · all dates · every status</span></p>
                    {createShow(() => ledger.hasNoRows, () => (
                      <p class="cv-message" role="status">No records yet.</p>
                    ))}
                    {createShow(() => ledger.hasRows, () => (
                      <div class="cv-ledger-split">
                        <table class="cv-ledger-table">
                          <caption>By person</caption>
                          <tbody>
                            {createList(() => ledger.people, row => row.id, row => (
                              <tr><th scope="row">{row.label}</th><td class="num">{row.count}</td><td class="num">{row.total}</td></tr>
                            ))}
                          </tbody>
                        </table>
                        <table class="cv-ledger-table">
                          <caption>By category</caption>
                          <tbody>
                            {createList(() => ledger.categories, row => row.id, row => (
                              <tr><th scope="row">{row.label}</th><td class="num">{row.count}</td><td class="num">{row.total}</td></tr>
                            ))}
                          </tbody>
                        </table>
                      </div>
                    ))}
                  </div>
                  <p class="cv-records-provenance" data-provenance><span class="cv-verified">Records</span> <span class="cv-receipt-meta">{ledger.scope}; totals computed in this view</span></p>
                </article>
              ))}
            </section>
          ))}
        </div>
        <p id="cv-activity" class="sr-only" role="status" aria-live="polite"></p>
      </main>
      <footer class="cv-dock">
        <div class="cv-tools" role="toolbar" aria-label="Canvas view" data-tools hidden>
          <button type="button" data-tool="zoom-out" aria-label="Zoom out">−</button>
          <button type="button" data-tool="zoom-reset" aria-label="Reset zoom">100%</button>
          <button type="button" data-tool="zoom-in" aria-label="Zoom in">+</button>
          <button type="button" data-tool="fit">Fit all</button>
          <button type="button" data-tool="reset-layout" title="View layout only: saved in this browser">Reset layout</button>
        </div>
        <IntentBar placeholder="Describe the app you need, or type / for your apps" />
        <p class="cv-layout-note">Layout saved in this browser</p>
      </footer>
    </div>
  );
}

await activateIslands({ IntentBar });
startBoard(document);
startSlash(document);
registerCommands(WIDGET_COMMANDS.map((command) => ({
  ...command,
  run: (text: string, report?: (message: string) => void) =>
    runWidget(command.slug as 'weather' | 'calculator' | 'clock', text).then((message) => report?.(message)),
})));
startAgents(document);
restoreWidgets();

mount(
  () =>
    Canvas({
      csrfSignout: '',
      hasGroups: false,
      groups: [],
      summary: '',
      liveEvents: '',
      hasLive: false,
      hasOrganization: false,
      organization: '',
      memberLabel: '',
      memberInitials: '',
      demo: false,
      demoMode: '',
    }),
  '#app',
);
