import assert from 'node:assert/strict';
import { test } from 'node:test';
import { askReply, requestAsk, requestScope, type AskScope, type RecordScope } from '../src/ask/client.ts';
import { documentScope } from '../src/ask/document.ts';
const app = '00000000-0000-4000-8000-000000001001', work = '00000000-0000-4000-8000-000000001004', collection = '00000000-0000-4000-8000-000000001003', record = '00000000-0000-4000-8000-000000001005';
const scope: RecordScope = { kind: 'card', address: 'books.records', works: [{ app_id: app, work_id: work, address: 'books', collection }] };
const receipt = { kind: 'record', reference: 'r1', app_id: app, work_id: work, collection_id: collection, record_id: record, publication_id: '00000000-0000-4000-8000-000000001006', definition_digest: `sha256:${'a'.repeat(64)}`, version: 2, work_version: 1, commit_sequence: 3, committed_at: '2026-11-01T12:00:00Z' };
const answer = { text: 'One record is waiting.', references: [{ ref: 'r1', app_id: app, work_id: work, record_id: record, address: 'books', label: 'A book' }], receipts: [receipt] };
test('record answers retain exact receipts and reject missing, substituted or wrong-collection provenance', () => {
  assert.deepEqual(askReply(answer, scope), answer);
  for (const bad of [
    { ...answer, receipts: [] }, { ...answer, receipts: [{ ...receipt, work_id: app }] },
    { ...answer, receipts: [{ ...receipt, collection_id: app }] }, { ...answer, receipts: [{ ...receipt, reference: 'r2' }] },
    { ...answer, references: [{ ...answer.references[0], work_id: app }] }, { ...answer, spec: {} },
  ]) assert.equal(askReply(bad, scope), null);
});
test('selected widget payload carries its identity and closed state without work or record selectors', () => {
  const widget: AskScope = { kind: 'widget', address: 'clock', widget: 'clock', widget_id: 'w-clock-abc', state: { skin: 'default', accent: 'default', font: 'default', zone: 'UTC', hour12: false }, works: [] };
  const payload = requestScope(widget);
  assert.equal(payload.widget_id, 'w-clock-abc'); assert.equal('works' in payload, false); assert.equal('work_id' in payload, false);
  assert.ok(askReply({ text: 'The clock uses its selected zone.', receipts: [], references: [] }, widget));
  assert.equal(askReply({ text: 'It is 09:30.', receipts: [], references: [] }, widget), null);
  assert.equal(askReply(answer, widget), null);
});
test('native document app and part scopes contain only that saved work; source parts explicitly refuse', () => {
  const root = { dataset: { app, work } } as unknown as HTMLElement;
  assert.deepEqual(documentScope(root)?.scope, { kind: 'app', address: '', works: [{ app_id: app, work_id: work, address: '' }] });
  const records = documentScope(root, { dataset: { collection } } as unknown as HTMLElement)!;
  assert.equal(records.scope.kind, 'card'); assert.equal(records.scope.works.length, 1); assert.equal(records.scope.works[0]?.collection, collection); assert.equal(records.refusal, '');
  const source = documentScope(root, { dataset: { view: 'total', kind: 'summary' } } as unknown as HTMLElement)!;
  assert.ok(source.refusal.includes('cannot yet read')); assert.equal(documentScope({ dataset: { app: '../other', work } } as unknown as HTMLElement), null);
});
test('refused responses never release text or amounts and transport captures the exact selected work', async () => {
  const previous = globalThis.fetch; const calls: RequestInit[] = [];
  try {
    globalThis.fetch = async (_url, init) => { calls.push(init!); return new Response(JSON.stringify({ code: 'could_not_answer', text: '$9000' }), { status: 422 }); };
    const response = await requestAsk('How much?', scope);
    assert.equal(typeof response, 'string'); assert.ok(!String(response).includes('9000'));
    assert.deepEqual(JSON.parse(calls[0]!.body as string), { question: 'How much?', scope });
    globalThis.fetch = async () => new Response(JSON.stringify(answer), { status: 200 });
    assert.deepEqual(await requestAsk('Which record?', scope), answer);
  } finally { globalThis.fetch = previous; }
});

test('changing Ask context preserves the unsent question and suppresses a delayed old-context reply', async () => {
  const { mountAskPanel } = await import('../src/ask/panel.ts');
  class Node {
    children: Node[]=[]; textContent=''; value=''; disabled=false; className='';
    classList={add:()=>{}}; listeners=new Map<string,(event:Event)=>unknown>();
    constructor(readonly tag:string,readonly ownerDocument:Document) {}
    setAttribute() {} append(...nodes:Node[]) {this.children.push(...nodes);}
    replaceChildren(...nodes:Node[]) {this.children=nodes;this.textContent='';}
    addEventListener(name:string, fn:(event:Event)=>unknown) {this.listeners.set(name,fn);}
    removeEventListener(name:string) {this.listeners.delete(name);} focus() {}
  }
  const doc = {createElement:(tag:string)=>new Node(tag,doc as unknown as Document)};
  const host=new Node('div',doc as unknown as Document), prior=globalThis.fetch;
  let release!:(response:Response)=>void;
  globalThis.fetch=async()=>new Promise<Response>(resolve=>{release=resolve;});
  try {
    const panel=mountAskPanel(host as unknown as HTMLElement,scope);
    const form=host.children[1]!,input=form.children[0]!,button=form.children[1]!,output=host.children[2]!;
    input.value='Which records?';
    const pending=form.listeners.get('submit')!({preventDefault(){}} as Event);
    assert.equal(button.disabled,true);
    input.value='Unsent next question';
    panel.setScope({...scope,address:'second.records',works:[{app_id:app,work_id:record,address:'second'}]});
    assert.equal(input.value,'Unsent next question');assert.equal(button.disabled,false);
    release(new Response(JSON.stringify(answer)));await pending;
    assert.equal(output.children.length,0);assert.equal(output.textContent,'');assert.equal(input.value,'Unsent next question');
    panel.invalidate();assert.equal(input.value,'Unsent next question');panel.dispose();
  } finally {globalThis.fetch=prior;}
});
