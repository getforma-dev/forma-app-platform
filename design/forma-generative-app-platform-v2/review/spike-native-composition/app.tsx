import { h, Fragment, mount, activateIslands, createSignal, createShow, createList } from '@getforma/core';

// A typed app definition, resolved by the server into presentation props.
// Every value is data; the component library is the trusted renderer.
type Cell = { id: string; text: string };
type Row = { id: string; cells: Cell[] };
type Bar = { id: string; x: string; height: string; label: string };
type Choice = { id: string; label: string };
type Field = { id: string; label: string; isText: boolean; isEnum: boolean; choices: Choice[] };
type Item = { id: string; label: string; value: string };
type Card = {
  id: string;
  title: string;
  kind: 'summary' | 'table' | 'timeline' | 'form';
  isSummary: boolean;
  isTable: boolean;
  isTimeline: boolean;
  isForm: boolean;
  style: string;
  items: Item[];
  rows: Row[];
  bars: Bar[];
  fields: Field[];
};
type CanvasProps = { cards: Card[] };

function Canvas(props: CanvasProps) {
  const [selected, setSelected] = createSignal('');
  return (
    <div class="canvas" data-selected={() => selected()}>
      {createList(() => props.cards, card => card.id, card => (
        <section class="card" data-kind={card.kind} style={card.style} aria-label={card.title}>
          <h2>{card.title}</h2>
          {createShow(() => card.isSummary, () => (
            <dl class="summary">
              {createList(() => card.items, i => i.id, i => (
                <div class="kv"><dt>{i.label}</dt><dd>{i.value}</dd></div>
              ))}
            </dl>
          ))}
          {createShow(() => card.isTable, () => (
            <table class="grid"><tbody>
              {createList(() => card.rows, r => r.id, r => (
                <tr>{createList(() => r.cells, c => c.id, c => <td>{c.text}</td>)}</tr>
              ))}
            </tbody></table>
          ))}
          {createShow(() => card.isTimeline, () => (
            <svg viewBox="0 0 600 120" role="img" aria-label={card.title}>
              {createList(() => card.bars, b => b.id, b => (
                <rect x={b.x} y="0" width="8" height={b.height}><title>{b.label}</title></rect>
              ))}
            </svg>
          ))}
          {createShow(() => card.isForm, () => (
            <form method="post" action="/commands">
              {createList(() => card.fields, f => f.id, f => (
                <div class="field">
                  <label for={f.id}>{f.label}</label>
                  {createShow(() => f.isText, () => <input id={f.id} name={f.id} type="text" />)}
                  {createShow(() => f.isEnum, () => (
                    <select id={f.id} name={f.id}>
                      {createList(() => f.choices, o => o.id, o => <option value={o.id}>{o.label}</option>)}
                    </select>
                  ))}
                </div>
              ))}
              <button type="submit">Save</button>
            </form>
          ))}
          <button type="button" onClick={() => setSelected(card.id)}>Focus</button>
        </section>
      ))}
    </div>
  );
}

function Home(props: CanvasProps) {
  return (
    <main class="page">
      <h1>composition-spike</h1>
      <Canvas cards={props.cards} />
      <p><a href="/about">About</a></p>
    </main>
  );
}

await activateIslands({ Canvas });
mount(() => Home({ cards: [] }), '#app');
