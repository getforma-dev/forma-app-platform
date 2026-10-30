import { h, mount, createShow, createList, createSignal, activateIslands } from '@getforma/core';
import { submitIntent } from './compose';
import { startBoard } from './board';

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
type Row = { id: string; cells: Cell[] };
type Card = {
  id: string;
  view: string;
  title: string;
  kind: string;
  state: string;
  badge: string;
  message: string;
  query: string;
  integerColumns: string;
  hasSummary: boolean;
  summary: Pair[];
  hasTable: boolean;
  columns: Cell[];
  rows: Row[];
  hasReceipt: boolean;
  receiptDigest: string;
  receiptKey: Pair[];
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
};
type Group = {
  id: string;
  app: string;
  work: string;
  appCard: string;
  title: string;
  label: string;
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
};
type CanvasProps = {
  csrfSignout: string;
  hasGroups: boolean;
  groups: Group[];
  summary: string;
  liveEvents: string;
};

type IntentProps = { placeholder: string };

// The intent bar: the one Forma island on the page. It posts the intent to
// the composer seam (`compose.ts`) and reports its state; placing the new
// app's cards is the board's job.
function IntentBar(props: IntentProps) {
  const [status, setStatus] = createSignal('');
  const [busy, setBusy] = createSignal(false);
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
        void submitIntent(text, (message: string) => setStatus(message)).finally(() => setBusy(false));
      }}
    >
      <label class="sr-only" for="intent-input">What do you need?</label>
      <span class="cv-prompt" aria-hidden="true">›</span>
      <input
        id="intent-input"
        name="intent"
        type="text"
        maxLength={1000}
        placeholder={props.placeholder}
        aria-describedby="intent-status"
      />
      <button id="intent-submit" type="submit" disabled={() => busy()}>
        {() => (busy() ? 'Placing…' : 'Compose')}
      </button>
      <p id="intent-status" class="cv-intent-status" role="status">{() => status()}</p>
    </form>
  );
}

function Canvas(props: CanvasProps) {
  return (
    <div id="cv-shell" class="cv-shell">
      <header class="cv-top">
        <a class="cv-brand" href="/canvas" aria-label="Forma canvas">
          <span class="cv-brand-mark" aria-hidden="true"></span>
          <span>Forma</span>
        </a>
        <div class="cv-top-status">
          <span id="cv-summary" class="cv-chip">{props.summary}</span>
          <span id="cv-live" class="cv-chip" data-state="off">Live updates off</span>
          <a class="cv-link" href="/workspace">Workspace</a>
          <form method="post" action="/auth/signout" class="cv-signout">
            <input type="hidden" name="csrf" value={props.csrfSignout} />
            <button type="submit">Sign out</button>
          </form>
        </div>
      </header>
      <main id="cv-canvas" class="cv-canvas" aria-label="Canvas" data-live-events={props.liveEvents}>
        {props.hasGroups ? null : (
          <section id="cv-empty" class="cv-empty" aria-labelledby="cv-empty-title">
            <p class="cv-eyebrow">Your workspace</p>
            <h1 id="cv-empty-title">What do you need?</h1>
            <p class="cv-empty-copy">
              Describe the app you need. It is assembled from admitted components and runs on your
              organization's data, with a receipt on every number.
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
                <p class="cv-eyebrow">App</p>
                <h2>{group.title}</h2>
                <p class="cv-provenance">{group.label}</p>
                <form method="post" action={group.href} class="cv-window" data-window>
                  <input type="hidden" name="csrf" value={group.csrf} />
                  <label>
                    <span>Window start (UTC)</span>
                    <input name="window_start" type="datetime-local" step="1" value={group.windowStart} />
                  </label>
                  <label>
                    <span>Window end (UTC, excluded)</span>
                    <input name="window_end" type="datetime-local" step="1" value={group.windowEnd} />
                  </label>
                  <label>
                    <span>Destination (optional)</span>
                    <input name="destination" type="text" value={group.destination} />
                  </label>
                  <button type="submit">Run views</button>
                </form>
                <p class="cv-app-links">
                  <a href={group.href}>Open the work page</a>
                </p>
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
                    <p class="cv-message" role="status">{card.message}</p>
                    {createShow(() => card.hasSummary, () => (
                      <dl class="cv-summary">
                        {createList(() => card.summary, item => item.id, item => (
                          <div>
                            <dt>{item.label}</dt>
                            <dd class="value">{item.value}</dd>
                          </div>
                        ))}
                      </dl>
                    ))}
                    {createShow(() => card.hasTable, () => (
                      <div class="cv-table">
                        <table>
                          <thead>
                            <tr>{createList(() => card.columns, column => column.id, column => <th scope="col">{column.text}</th>)}</tr>
                          </thead>
                          <tbody>
                            {createList(() => card.rows, row => row.id, row => (
                              <tr>{createList(() => row.cells, cell => cell.id, cell => <td class="value">{cell.text}</td>)}</tr>
                            ))}
                          </tbody>
                        </table>
                      </div>
                    ))}
                    {createShow(() => card.hasReceipt, () => (
                      <section class="cv-receipt" aria-label="Receipt">
                        <p class="cv-receipt-head">
                          <span>Receipt</span> <code>{card.receiptDigest}</code>
                        </p>
                        <dl class="cv-receipt-key">
                          {createList(() => card.receiptKey, item => item.id, item => (
                            <div>
                              <dt>{item.label}</dt>
                              <dd>
                                <code>{item.value}</code>
                              </dd>
                            </div>
                          ))}
                        </dl>
                        <details>
                          <summary>All receipt fields</summary>
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
                      </section>
                    ))}
                  </div>
                </article>
              ))}
              {createList(() => group.forms, form => form.id, form => (
                <article class="cv-card cv-form" data-card={form.id} data-kind="form" aria-label={form.title}>
                  <header class="cv-card-head">
                    <h3>{form.title}</h3>
                    <span class="badge">command</span>
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
                    <button type="submit">Submit {form.command}</button>
                  </form>
                  <p class="cv-outcome" role="status" data-outcome></p>
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
        <IntentBar placeholder="What do you need? Describe an app…" />
        <p class="cv-layout-note">View layout only · saved in this browser</p>
      </footer>
    </div>
  );
}

await activateIslands({ IntentBar });
startBoard(document);

mount(
  () =>
    Canvas({
      csrfSignout: '',
      hasGroups: false,
      groups: [],
      summary: '',
      liveEvents: '',
    }),
  '#app',
);
