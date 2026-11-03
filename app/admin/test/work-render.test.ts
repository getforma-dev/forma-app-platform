import assert from 'node:assert/strict';
import { test } from 'node:test';
import { renderReadOnlyRegions, renderRegions, type DocumentProps } from '../src/work/render.ts';

// A closed DOM spy: there is deliberately no HTML parsing or innerHTML sink.
// Strings remain distinct from created elements so a hostile label cannot
// accidentally become executable markup in the renderer.
class Element {
  attributes: Record<string, string> = {};
  children: (Element | string)[] = [];
  value = '';
  constructor(readonly tag: string) {}
  setAttribute(name: string, value: string) { this.attributes[name] = value; }
  append(...children: (Element | string)[]) { this.children.push(...children); }
}
const document = { createElement: (tag: string) => new Element(tag) } as unknown as Document;
const elements = (node: Element): Element[] => [node, ...node.children.flatMap(child => typeof child === 'string' ? [] : elements(child))];
const text = (node: Element): string => node.children.map(child => typeof child === 'string' ? child : text(child)).join('');
const APP = 'aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee';
const WORK = '11111111-2222-4333-8444-555555555555';
const COLLECTION = '22222222-3333-4444-8555-666666666666';
const RECORD = '33333333-4444-4555-8666-777777777777';
const HOSTILE = '<script>alert("sample")</script>';

function props(): DocumentProps {
  return {
    app: APP, work: WORK, action: `/w/${APP}/${WORK}`, records_truncated: false,
    forms: [{ title: 'New record', collection: COLLECTION, command: 'create',
      action: `/v1/work/${APP}/${WORK}/commands/create`, csrf: 'csrf', idempotency_key: 'same-key',
      publication: 'publication', schema: 'items', schema_version: '1', record: RECORD, record_version: '0', work_version: '3',
      fields: [{ label: HOSTILE, input: 'value.name', value: HOSTILE, spec: '{"type":"text"}', spec_input: 'spec.name',
        writable: true, read_only: false, has_text: true, has_choices: false, choices: [] }], submit: 'Save' }],
    record_cards: [{ title: 'Records', summary: '1 record', collection: COLLECTION, has_no_rows: false, has_rows: true,
      columns: [{ text: 'Name', raw: '' }], rows: [{ record_id: RECORD, version: '2', committed_at: '2026-11-01T00:00:00Z',
        commit_sequence: '17', cells: [{ text: HOSTILE, raw: HOSTILE }] }] }],
    views: [{ id: 'total', title: 'Total', state: 'ready', badge: 'ready', message: '',
      has_summary: true, summary: [{ label: 'Amount', value: '$12.34', raw: '1234' }],
      has_table: false, columns: [], rows: [], has_receipt: true, receipt_digest: 'sha256:receipt',
      receipt: [{ label: 'Digest', value: 'sha256:receipt', raw: '' }],
      receipt_json: '{\n  "result_digest": "sha256:actual",\n  "label": "<script>sample</script>"\n}' }],
  };
}

test('document renderer keeps hostile text inert and preserves command identity and receipt bytes', () => {
  const source = props();
  const rendered = renderRegions(document, source);
  const forms = rendered.forms as unknown as Element;
  const records = rendered.records as unknown as Element;
  const views = rendered.views as unknown as Element;
  assert.equal(elements(forms).find(node => node.attributes.name === 'idempotency_key')?.attributes.value, 'same-key');
  assert.equal(elements(forms).find(node => node.attributes.name === 'expected_work_version')?.attributes.value, '3');
  assert.equal(elements(forms).find(node => node.attributes.name === 'record_id')?.attributes.value, RECORD);
  assert.equal(elements(forms).find(node => node.attributes.name === 'value.name')?.attributes.value, HOSTILE);
  assert.ok(text(forms).includes(HOSTILE));
  assert.ok(text(records).includes(HOSTILE));
  assert.equal(elements(records).find(node => node.tag === 'a')?.attributes.href, `/work/${APP}/${WORK}/records/${COLLECTION}/${RECORD}`);
  assert.equal(elements(views).find(node => node.tag === 'pre') && text(elements(views).find(node => node.tag === 'pre')!), source.views[0]?.receipt_json);
  assert.equal(elements(views).find(node => node.tag === 'dd' && node.attributes.class === 'value')?.attributes.title, '1234');
  assert.ok([...elements(forms), ...elements(records), ...elements(views)].every(node => node.tag !== 'script'));
});

test('failed and denied views cannot display old values or receipts; empty results retain their real receipt', () => {
  for (const state of ['idle', 'loading', 'failed', 'denied']) {
    const source = props();
    source.views[0]!.state = state;
    const views = renderRegions(document, source).views as unknown as Element;
    assert.ok(!text(views).includes('$12.34'), state);
    assert.ok(!elements(views).some(node => node.tag === 'details' || node.tag === 'table'), state);
  }
  const source = props();
  source.views[0]!.state = 'empty';
  const views = renderRegions(document, source).views as unknown as Element;
  assert.ok(!text(views).includes('$12.34'));
  assert.equal(text(elements(views).find(node => node.tag === 'pre')!), source.views[0]!.receipt_json);
});

test('forms cannot redirect a command to another work or execute a supplied URL', () => {
  for (const action of ['javascript:alert(1)', `https://example.test/v1/work/${APP}/${WORK}/commands/create`, `/v1/work/${APP}/${RECORD}/commands/create`]) {
    const source = props();
    source.forms[0]!.action = action;
    assert.throws(() => renderRegions(document, source), /invalid command action/);
  }
});

test('shared rendering retains actual values and receipt bytes without exposing record links or write controls', () => {
  const source = props();
  const result = renderReadOnlyRegions(document, source) as unknown as Element;
  assert.ok(text(result).includes(HOSTILE));
  assert.ok(text(result).includes('$12.34'));
  assert.ok(text(result).includes('Revision 2'));
  assert.equal(text(elements(result).find(node => node.tag === 'pre')!), source.views[0]!.receipt_json);
  assert.ok(elements(result).every(node => !['a', 'form', 'input', 'button', 'select', 'script'].includes(node.tag)));
  assert.ok(!text(result).includes(APP));
  assert.ok(!elements(result).some(node => Object.hasOwn(node.attributes, 'href') || Object.hasOwn(node.attributes, 'action')));
});

test('admitted document parts retain interleaving, fixed layout classes and independent form identities', () => {
  const source = props();
  const part = (id: string, kind: string, row: number) => ({ id, title: id, kind,
    class: `document-part document-column-0 document-width-12 document-row-${row}`,
    text: '', collection: '', is_note: kind === 'note', is_form: kind === 'form', has_selector: false,
    selection_name: `selection.${id}`, selected: '', choices: [], message: '', forms: [], views: [],
  });
  source.parts = [
    { ...part('instructions', 'note', 1), text: HOSTILE },
    { ...part('add_one', 'form', 2), collection: COLLECTION, forms: [{ ...source.forms[0]!, id: 'add_one' }] },
    { ...part('total', 'status', 3), views: [source.views[0]!] },
    { ...part('add_two', 'form', 4), collection: COLLECTION, forms: [{ ...source.forms[0]!, id: 'add_two' }] },
  ];
  const root = renderRegions(document, source).parts as unknown as Element;
  assert.deepEqual(root.children.map(node => (node as Element).attributes['data-document-part']), ['instructions','add_one','total','add_two']);
  assert.ok(text(root).includes(HOSTILE));
  assert.ok(!elements(root).some(node => node.tag === 'script' || 'style' in node.attributes));
  assert.deepEqual(elements(root).filter(node => 'data-document-form' in node.attributes).map(node => node.attributes['data-document-form']), ['add_one','add_two']);
  assert.equal(text(elements(root).find(node => node.tag === 'pre')!), source.views[0]!.receipt_json);
  source.parts[0]!.class += ' arbitrary-css';
  assert.throws(() => renderRegions(document, source), /invalid layout/);
});

test('existing-record selection stays on the same work and retains native CSRF fallback', () => {
  const source = props(); source.csrf = 'read-token';
  source.parts = [{ id: 'edit', title: 'Edit item', kind: 'form', class: 'document-part document-column-6 document-width-6 document-row-1',
    text: '', collection: COLLECTION, is_note: false, is_form: true, has_selector: true,
    selection_name: 'selection.edit', selected: RECORD, choices: [{ id: RECORD, label: HOSTILE, selected: true }],
    message: '', forms: [{ ...source.forms[0]!, id: 'edit' }], views: [] }];
  const root = renderRegions(document, source).parts as unknown as Element;
  const selector = elements(root).find(node => 'data-document-selection' in node.attributes)!;
  assert.equal(selector.attributes.action, source.action);
  assert.equal(elements(selector).find(node => node.attributes.name === 'csrf')?.attributes.value, 'read-token');
  assert.equal(elements(selector).find(node => node.tag === 'select')?.value, RECORD);
  assert.ok(text(selector).includes(HOSTILE));
  source.parts[0]!.selection_name = 'organization_id';
  assert.throws(() => renderRegions(document, source), /invalid selection/);
});


test('shared parts retain declared note/view/form-label order without member metadata or controls', () => {
  const source = props();
  const part = (id: string, row: number) => ({ id, title: id, class: `document-part document-column-0 document-width-12 document-row-${row}`, text: '', is_note: false, is_form: false, message: '', views: [] });
  const read = { ...source, parts: [
    { ...part('guide', 1), is_note: true, text: HOSTILE },
    { ...part('form', 2), is_form: true, message: 'This form is view only in a shared app.', forms: source.forms, action: source.action },
    { ...part('total', 3), views: source.views },
  ] };
  const root = renderReadOnlyRegions(document, read) as unknown as Element;
  assert.deepEqual(elements(root).filter(node => 'data-document-part' in node.attributes).map(node => node.attributes['data-document-part']), ['guide', 'form', 'total']);
  assert.ok(text(root).includes(HOSTILE) && text(root).includes('view only'));
  assert.ok(elements(root).every(node => !['form', 'input', 'select', 'button', 'a', 'script'].includes(node.tag)));
  assert.ok(!text(root).includes(APP));
  assert.equal(text(elements(root).find(node => node.tag === 'pre')!), source.views[0]!.receipt_json);
});
