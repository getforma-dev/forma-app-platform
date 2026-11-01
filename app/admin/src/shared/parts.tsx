import { h, createList, createShow } from '@getforma/core';
import type { View, RecordsCard, SharedPart } from './types';

/** Fixed, read-only native vocabulary: no forms, source links or record links. */
export function SharedParts(props: { parts: SharedPart[]; views: View[]; recordCards: RecordsCard[]; recordsTruncated: boolean }) {
  return <div class="shared-parts" data-shared-parts="">
    <div class="document-parts" data-document-parts="">
      {createList(() => props.parts, part => part.id, part => <article class={part.class} data-document-part={part.id}>
        {createShow(() => part.isNote, () => <section class="view document-note"><h2>{part.title}</h2><p>{part.text}</p></section>)}
        {createShow(() => part.isForm, () => <section class="view"><h2>{part.title}</h2><p class="document-form-message">{part.message}</p></section>)}
      {createList(() => part.views, view => view.id, view => <section class="view" data-view={view.id} data-state={view.state}>
        <h2>{view.title}</h2>
        <p class="state" role="status"><span class="badge">{view.badge}</span> {view.message}</p>
        {createShow(() => view.hasSummary, () => <dl class="summary">
          {createList(() => view.summary, item => item.id, item => <div><dt>{item.label}</dt><dd class="value" title={item.raw}>{item.value}</dd></div>)}
        </dl>)}
        {createShow(() => view.hasChart, () => <figure class="document-chart">
          <svg class="document-chart-svg" viewBox={view.chart.viewBox} role="img" aria-label={view.chart.label}>
            {createList(() => view.chart.grid, line => line.id, line => <g><line class="document-grid" x1="52" x2="552" y1={line.y} y2={line.y} /><text class="document-axis" x="44" y={line.labelY} text-anchor="end">{line.text}</text></g>)}
            {createList(() => view.chart.ticks, tick => tick.id, tick => <text class="document-axis" x={tick.x} y="180" text-anchor={tick.anchor}>{tick.text}</text>)}
            {createShow(() => view.chart.hasBars, () => <g>{createList(() => view.chart.bars, bar => bar.id, bar => <rect class="cv-bar" x={bar.x} y={bar.y} width={bar.width} height={bar.height} rx="2"><title>{bar.tip}</title></rect>)}</g>)}
            {createShow(() => view.chart.hasLine, () => <g><path class="document-area" d={view.chart.area} /><path class="document-line" d={view.chart.line} /></g>)}
          </svg><figcaption>{view.chart.caption}</figcaption>
        </figure>)}
        {createShow(() => view.hasTable, () => <div class="shared-table"><table><thead><tr>
          {createList(() => view.columns, cell => cell.id, cell => <th scope="col">{cell.text}</th>)}
        </tr></thead><tbody>{createList(() => view.rows, row => row.id, row => <tr>
          {createList(() => row.cells, cell => cell.id, cell => <td title={cell.raw}>{cell.text}</td>)}
        </tr>)}</tbody></table></div>)}
        {createShow(() => view.hasReceipt, () => <details class="receipt"><summary>Receipt <code>{view.receiptDigest}</code></summary>
          <dl>{createList(() => view.receipt, item => item.id, item => <div><dt>{item.label}</dt><dd><code>{item.value}</code></dd></div>)}</dl>
          <pre>{view.receiptJson}</pre>
        </details>)}
      </section>)}
      </article>)}
    </div>
    <div data-document-records="">
      {createList(() => props.recordCards, card => card.id, card => <section class="view document-records" data-collection={card.collection}>
        <h2>{card.title}</h2><p>{card.summary}</p>
        {createShow(() => card.hasNoRows, () => <p>No records yet.</p>)}
        {createShow(() => card.hasRows, () => <div class="shared-table"><table><thead><tr>
          {createList(() => card.columns, cell => cell.id, cell => <th scope="col">{cell.text}</th>)}
          <th scope="col">Saved</th>
        </tr></thead><tbody>{createList(() => card.rows, row => row.id, row => <tr>
          {createList(() => row.cells, cell => cell.id, cell => <td title={cell.raw}>{cell.text}</td>)}
          <td><small>Revision {row.version} · {row.committedAt} · Commit {row.commitSequence}</small></td>
        </tr>)}</tbody></table></div>)}
      </section>)}
      {createShow(() => props.recordsTruncated, () => <p class="notice">Showing the newest records. This listing has reached its limit.</p>)}
    </div>
  </div>;
}
