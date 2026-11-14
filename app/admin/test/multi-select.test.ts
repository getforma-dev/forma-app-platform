// ADR-020 §9 (APP-208): the multi-select on the client. The canvas form card
// and the native document form rebuild a checkbox group from the form JSON:
// one box per declared value in declaration order, the held ones checked,
// all under the field's one input name after an empty hidden sentinel. The
// command request composes one typed `multi_enum` value from the checked
// boxes: only the spec's declared values, in the spec's order, each once;
// nothing checked is the null value.
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { Document_, type Node_ } from './fake-canvas-page.ts';
import { buildForm, type FormJson } from '../src/canvas/render.ts';
import { multiEnumValue, typedFormValue } from '../src/canvas/commands.ts';
import { renderRegions, type DocumentProps } from '../src/work/render.ts';

const SPEC = { type: 'multi_enum', values: ['red', 'green', 'blue'] };
const CHOICES = [
  { id: 'o0', value: 'red', label: 'red', selected: true },
  { id: 'o1', value: 'green', label: 'green', selected: false },
  { id: 'o2', value: 'blue', label: 'blue', selected: true },
];

test('the canvas form renders a multi-select as a checkbox group under one input name', () => {
  const document_ = new Document_();
  const form: FormJson = {
    id: 'f', title: 'Pick', command: 'pick', action: '/v1/work/a/w/commands/pick', csrf: 'c',
    idempotency_key: 'k', collection: 'c', publication: 'p', schema: 'choices', schema_version: '1',
    record: 'r', record_version: '0', work_version: '1',
    fields: [{
      input_mode: 'text', id: 'tags', kind: 'multi_enum', label: 'tags', input: 'value.tags', value: 'red, blue',
      spec: JSON.stringify(SPEC), spec_input: 'spec.tags', writable: true, read_only: false,
      has_text: false, has_choices: false, has_checkboxes: true, choices: CHOICES,
    }],
  };
  const node = buildForm(document_ as unknown as Document, form) as unknown as Node_;
  assert.equal(node.querySelectorAll('select').length, 0, 'no select');
  const inputs = node.querySelectorAll('input').filter((i) => i.getAttribute('name') === 'value.tags') as (Node_ & { checked?: boolean })[];
  assert.deepEqual(inputs.map((i) => [i.getAttribute('type'), i.getAttribute('value'), i.checked ?? null]), [
    ['hidden', '', null],
    ['checkbox', 'red', true],
    ['checkbox', 'green', false],
    ['checkbox', 'blue', true],
  ]);
  assert.equal(node.querySelectorAll('input').filter((i) => i.getAttribute('type') === 'text').length, 0, 'no text box');
  const labels = node.querySelectorAll('label').filter((l) => l.getAttribute('class') === 'cv-choice');
  assert.deepEqual(labels.map((l) => l.children.map((c) => (typeof c === 'string' ? c : (c as Node_).textContent)).join('').trim()), ['red', 'green', 'blue']);
  assert.equal(node.querySelectorAll('input').find((i) => i.getAttribute('name') === 'spec.tags')?.getAttribute('value'), JSON.stringify(SPEC));
});

test('the command request composes the declared values in declaration order, nothing checked as null', () => {
  assert.deepEqual(multiEnumValue(SPEC, ['blue', 'red']), { type: 'multi_enum', values: ['red', 'blue'] });
  assert.deepEqual(multiEnumValue(SPEC, ['green']), { type: 'multi_enum', values: ['green'] });
  // Only declared values, each once: a forged or repeated box never reaches the request.
  assert.deepEqual(multiEnumValue(SPEC, ['purple', 'blue', 'blue', '']), { type: 'multi_enum', values: ['blue'] });
  assert.deepEqual(multiEnumValue(SPEC, []), { type: 'null' });
  assert.deepEqual(multiEnumValue(SPEC, ['purple']), { type: 'null' });
  assert.equal(multiEnumValue({ type: 'multi_enum' }, ['red']), null);
  assert.equal(multiEnumValue({ type: 'multi_enum', values: [1] }, ['red']), null);
  // A single text never types a multi-select (the group composes it).
  assert.equal(typedFormValue(SPEC, 'red'), null);
  assert.deepEqual(typedFormValue(SPEC, ''), { type: 'null' });
});

// The native document renderer's own tiny DOM, as test/work-render.test.ts uses.
class Element {
  attributes: Record<string, string> = {};
  children: (Element | string)[] = [];
  value = '';
  checked = false;
  constructor(readonly tag: string) {}
  setAttribute(name: string, value: string) { this.attributes[name] = value; }
  append(...children: (Element | string)[]) { this.children.push(...children); }
}
const elements = (node: Element): Element[] => [node, ...node.children.flatMap((child) => (typeof child === 'string' ? [] : elements(child)))];

test('the native document form renders the same checkbox group', () => {
  const document = { createElement: (tag: string) => new Element(tag) } as unknown as Document;
  const APP = 'aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee';
  const WORK = '11111111-2222-4333-8444-555555555555';
  const props: DocumentProps = {
    app: APP, work: WORK, action: `/w/${APP}/${WORK}`, records_truncated: false, record_cards: [], views: [],
    forms: [{ title: 'Pick', collection: 'c', command: 'pick', action: `/v1/work/${APP}/${WORK}/commands/pick`, csrf: 'csrf',
      idempotency_key: 'k', publication: 'p', schema: 'choices', schema_version: '1', record: 'r', record_version: '0', work_version: '1',
      fields: [{ input_mode: 'text', label: 'tags', input: 'value.tags', value: 'red, blue', spec: JSON.stringify(SPEC), spec_input: 'spec.tags',
        writable: true, read_only: false, has_text: false, has_choices: false, has_checkboxes: true, choices: CHOICES }], submit: 'Pick' }],
  };
  const forms = renderRegions(document, props).forms as unknown as Element;
  const tags = elements(forms).filter((node) => node.attributes.name === 'value.tags');
  assert.deepEqual(tags.map((node) => [node.attributes.type, node.attributes.value, node.checked]), [
    ['hidden', '', false],
    ['checkbox', 'red', true],
    ['checkbox', 'green', false],
    ['checkbox', 'blue', true],
  ]);
  assert.equal(elements(forms).filter((node) => node.tag === 'select').length, 0);
  assert.equal(elements(forms).find((node) => node.attributes.name === 'spec.tags')?.attributes.value, JSON.stringify(SPEC));
});
