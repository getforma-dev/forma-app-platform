import { h, mount, activateIslands, createSignal, createShow, createList } from '@getforma/core';

type Cell = { id: string; text: string };
type Row = { id: string; cells: Cell[] };
type Bar = { id: string; x: number; y: number; width: number; height: number; label: string };
type Choice = { id: string; label: string };
type Field = { id: string; label: string; isText: boolean; isEnum: boolean; choices: Choice[] };
type Item = { id: string; label: string; value: string };
type Card = {
  id: string; title: string; kind: string; style: string; tabIndex: number;
  isSummary: boolean; isTable: boolean; isDetail: boolean; isTimeline: boolean;
  isLinked: boolean; isChart: boolean; isFilter: boolean; isForm: boolean; isNote: boolean; isStatus: boolean;
  items: Item[]; rows: Row[]; bars: Bar[]; fields: Field[];
  text: string; status: string; filterId: string;
};
type CanvasProps = { cards: Card[]; filter: string; epoch: number; linked: string; outcome: string };

// Presentation values are resolved on the server. These events exercise the
// synthetic endpoint, not authorization, query admission or durable commands.
function Canvas(props: CanvasProps) {
  const [selected, setSelected] = createSignal('');
  const [filter, setFilter] = createSignal(props.filter);
  const [epoch, setEpoch] = createSignal(props.epoch);
  const [linked, setLinked] = createSignal(props.linked);
  const [outcome, setOutcome] = createSignal(props.outcome);
  const [requested, setRequested] = createSignal(props.epoch);
  async function changeFilter(event: Event) {
    const value = (event.currentTarget as HTMLSelectElement).value;
    const current = requested() + 1; setRequested(current);
    setFilter(value);
    const response = await fetch(`/linked?filter=${encodeURIComponent(value)}&epoch=${current}`);
    if (!response.ok) return;
    const data = await response.json();
    if (data.epoch !== requested()) return;
    setEpoch(data.epoch); setLinked(data.label);
  }
  async function submit(event: Event) {
    event.preventDefault();
    const data = new FormData(event.currentTarget as HTMLFormElement);
    const body = new URLSearchParams();
    for (const [key, value] of data) body.set(key, String(value));
    const response = await fetch('/commands', { method: 'POST', body });
    if (!response.ok) { setOutcome('Refused'); return; }
    const result = await response.json(); setOutcome(result.outcome);
  }
  return <div class="canvas" data-selected={() => selected()} data-filter={() => filter()} data-epoch={() => epoch()}>
    <p role="status" aria-live="polite" class="linked-scope">Linked scope: {() => linked()}</p>
    {createList(() => props.cards, card => card.id, card => (
      <section class="card" id={card.id} data-kind={card.kind} style={card.style} aria-label={card.title} tabIndex={card.tabIndex}>
        <h2>{card.title}</h2>
        {createShow(() => card.isSummary, () => <dl>{createList(() => card.items, item => item.id, item => <div><dt>{item.label}</dt><dd>{item.value}</dd></div>)}</dl>)}
        {createShow(() => card.isTable, () => <table><caption>{card.title}</caption><thead><tr><th scope="col">Asset</th><th scope="col">Evidence</th></tr></thead><tbody>{createList(() => card.rows, row => row.id, row => <tr>{createList(() => row.cells, cell => cell.id, cell => <td>{cell.text}</td>)}</tr>)}</tbody></table>)}
        {createShow(() => card.isDetail, () => <dl class="detail">{createList(() => card.items, item => item.id, item => <div><dt>{item.label}</dt><dd>{item.value}</dd></div>)}</dl>)}
        {createShow(() => card.isTimeline, () => <svg viewBox="0 0 600 120" role="img" aria-label={card.title}>{createList(() => card.bars, bar => bar.id, bar => <rect x={bar.x} y={bar.y} width={bar.width} height={bar.height}><title>{bar.label}</title></rect>)}</svg>)}
        {createShow(() => card.isChart, () => <svg viewBox="0 0 600 120" role="img" aria-label={card.title}>{createList(() => card.bars, bar => bar.id, bar => <rect x={bar.x} y={bar.y} width={bar.width} height={bar.height}><title>{bar.label}</title></rect>)}</svg>)}
        {createShow(() => card.isFilter, () => <div><label for={card.filterId}>Linked destination</label><select id={card.filterId} name="destination" onChange={changeFilter}><option value="all">All destinations</option><option value="unmatched">Unmatched destinations</option></select><p>Applies to all six linked views.</p></div>)}
        {createShow(() => card.isForm, () => <form method="post" action="/commands" onSubmit={submit}>
          <input type="hidden" name="Idempotency-Key" value="synthetic-render-key" />
          {createList(() => card.fields, field => field.id, field => <div><label for={field.id}>{field.label}</label>
            {createShow(() => field.isText, () => <input id={field.id} name={field.id} type="text" required />)}
            {createShow(() => field.isEnum, () => <select id={field.id} name={field.id}>{createList(() => field.choices, choice => choice.id, choice => <option value={choice.id}>{choice.label}</option>)}</select>)}
          </div>)}<button type="submit">Save observation</button><output aria-live="polite">{() => outcome()}</output>
        </form>)}
        {createShow(() => card.isNote, () => <p class="note">{card.text}</p>)}
        {createShow(() => card.isStatus, () => <p role="status" class="status">{card.status}</p>)}
        {createShow(() => card.isLinked, () => <p class="linked-result" data-epoch={() => epoch()}>{() => linked()}</p>)}
        <button type="button" onClick={() => { setSelected(card.id); document.getElementById(card.id)?.focus(); }}>Focus view</button>
      </section>
    ))}
  </div>;
}
function Home(props: CanvasProps) {
  return <main><h1>Launch renderer qualification</h1><p>Synthetic presentation fixture; no customer data or durable write.</p><Canvas cards={props.cards} filter={props.filter} epoch={props.epoch} linked={props.linked} outcome={props.outcome} /></main>;
}
await activateIslands({ Canvas });
mount(() => Home({ cards: [], filter: 'all', epoch: 0, linked: 'All: 1,140,000 bytes', outcome: 'Not submitted' }), '#app');
