import { h, createShow, createList } from '@getforma/core';
import type { RecordComponent } from './types';

/** Native HTML works without JavaScript; filters are progressive enhancement. */
export function RecordComponents(props: { components: RecordComponent[] }) {
  return <div class="record-components">
    {createList(() => props.components, p => p.id, p => <RecordComponentView component={p} />)}
  </div>;
}

export function RecordComponentView(props: { component: RecordComponent }) {
  const p = props.component;
  return <section class="record-component view" data-record-component={p.id} data-state={p.state} data-filter-epoch={p.filterEpoch}>
      <h2>{p.title}</h2>
      {createShow(() => p.hasFilters, () => <form method="post" class="record-filter-form" data-record-filters={p.id}>
        {createList(() => p.filters, f => f.id, f => <fieldset class="record-filter" data-record-filter={f.id}>
          <legend>{f.label}</legend>
          {createShow(() => f.isText, () => <label>Contains <input type="search" data-filter-value={f.id} value={f.value} maxLength={200} autocomplete="off" /></label>)}
          {createShow(() => f.isEnum, () => <label>Equals <select data-filter-value={f.id}><option value="">All</option>{createList(() => f.options, o => o.value, o => <option value={o.value} selected={o.selected}>{o.label}</option>)}</select></label>)}
          {createShow(() => f.isDate, () => <div><label>From <input type="date" data-filter-start={f.id} value={f.start} /></label><label>Before <input type="date" data-filter-end={f.id} value={f.end} /></label></div>)}
        </fieldset>)}
        <button type="submit" disabled data-record-filter-enable="">Apply filters</button><button type="button" disabled data-record-filter-enable="" data-record-filter-reset="">Reset filters</button>
        <noscript>Filtering needs JavaScript. The values below use the displayed filters.</noscript>
      </form>)}
      <p class="state" role="status" aria-live="polite">{p.message}</p>
      {createShow(() => p.hasSummary, () => <p class="record-metric" title={p.summaryRaw}>{p.summary}</p>)}
      {createShow(() => p.hasBudget, () => <div class="record-budget"><p>{p.budget}</p><p>{p.ratio}</p><p>{p.remaining}</p></div>)}
      {createShow(() => p.hasGroups, () => <figure class="record-groups"><ol>{createList(() => p.groups, g => g.id, g => <li><span>{g.label}</span><strong title={g.raw}>{g.value}</strong><svg class="record-bar-track" aria-hidden="true"><rect class="record-bar" width={g.width} height="8" /></svg></li>)}</ol><figcaption>Values cover all matching saved records. Bars show relative magnitude.</figcaption></figure>)}
      {createShow(() => p.hasTable, () => <div class="shared-table" role="region" aria-label={p.title} tabIndex={0}><table><thead><tr>{createList(() => p.columns, c => c.id, c => <th scope="col">{c.label}</th>)}</tr></thead><tbody>{createList(() => p.rows, r => r.id, r => <tr data-record-version={r.version}>{createList(() => r.cells, c => c.id, c => <td title={c.raw}>{c.text}</td>)}</tr>)}</tbody></table></div>)}
      {createShow(() => p.hasReceipt, () => <details class="receipt"><summary>Calculation details</summary><code>{p.receiptDigest}</code><pre>{p.receiptJson}</pre></details>)}
    </section>;
}
