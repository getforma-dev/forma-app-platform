// ADR-020 (APP-205): the canvas form's reference picker, rebuilt on the
// client from the form JSON the server rendered: options valued by record
// id, the server-marked record selected, and a disabled "Nothing to choose
// yet" select (posts nothing) for an empty target.
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { Document_, type Node_ } from './fake-canvas-page.ts';
import { buildForm, type FormJson } from '../src/canvas/render.ts';

const field = (id: string, extra: Record<string, unknown>) => ({
  input_mode: 'text', id, kind: 'reference', label: id, input: `value.${id}`, value: '',
  spec: '{"target_input":"items","type":"reference"}', spec_input: `spec.${id}`,
  writable: true, read_only: false, has_text: false, has_choices: false, choices: [], ...extra,
});

test('the reference picker selects the marked record and an empty target is disabled', () => {
  const document_ = new Document_();
  const form: FormJson = {
    id: 'f', title: 'Respond', command: 'respond', action: '/v1/work/a/w/commands/respond', csrf: 'c',
    idempotency_key: 'k', collection: 'c', publication: 'p', schema: 'responses', schema_version: '1',
    record: 'r', record_version: '0', work_version: '1',
    fields: [
      field('item', { has_choices: true, choices: [
        { id: 'o0', value: '00000000-0000-4000-8000-0000000000a3', label: 'Third item', selected: true },
        { id: 'o1', value: '00000000-0000-4000-8000-0000000000a2', label: 'Record', selected: false },
      ] }),
      field('other', { no_choices: true }),
    ],
  };
  const node = buildForm(document_ as unknown as Document, form) as unknown as Node_;
  const selects = node.querySelectorAll('select');
  assert.equal(selects.length, 2);
  const [picker, empty] = selects;
  assert.equal(picker!.getAttribute('name'), 'value.item');
  const options = picker!.children as (Node_ & { selected?: boolean })[];
  assert.deepEqual(options.map((o) => [o.getAttribute('value'), o.children[0]?.textContent, o.selected]), [
    ['00000000-0000-4000-8000-0000000000a3', 'Third item', true],
    ['00000000-0000-4000-8000-0000000000a2', 'Record', false],
  ]);
  assert.equal(empty!.getAttribute('name'), 'value.other');
  assert.equal(empty!.getAttribute('disabled'), '');
  assert.equal(empty!.children.length, 1);
  assert.equal(empty!.children[0]!.getAttribute('value'), '');
  assert.equal(empty!.children[0]!.children[0]?.textContent, 'Nothing to choose yet');
  // No text box for either reference.
  assert.equal(node.querySelectorAll('input').filter((i) => i.getAttribute('type') === 'text').length, 0);
});
