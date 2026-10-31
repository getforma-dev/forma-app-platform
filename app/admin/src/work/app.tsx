import { h, mount, createShow, createList } from '@getforma/core';

// APP-012 work page (native SSR, no JavaScript). Every value, state and
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
type WorkProps = {
  title: string;
  action: string;
  csrf: string;
  windowStart: string;
  windowEnd: string;
  destination: string;
  formError: boolean;
  views: Card[];
};

function Work(props: WorkProps) {
  return (
    <main class="page work">
      <a class="forma-brand" href="/" aria-label="Forma home"><span class="forma-mark-tile" aria-hidden="true"><span class="forma-mark"></span></span><span>forma<span class="forma-dot">.</span></span><span class="forma-product">canvas</span></a>
      <h1>{props.title}</h1>
      <p class="lede">
        Saved work.
      </p>
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
      {props.formError ? (
        <p class="notice" role="alert">
          The window was not accepted. Give a start before the end, as UTC date and time. Nothing ran.
        </p>
      ) : null}
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
      <p>
        <a href="/workspace">Workspace</a>
      </p>
    </main>
  );
}

mount(
  () =>
    Work({
      title: '',
      action: '',
      csrf: '',
      windowStart: '',
      windowEnd: '',
      destination: '',
      formError: false,
      views: [],
    }),
  '#app',
);
