import assert from 'node:assert/strict';
import { test } from 'node:test';
import { editorState, presentationRequest, recoveredPresentation, type EditorState } from '../src/presentation/editor.ts';

const state: EditorState = {
  publication_id: '11111111-1111-4111-8111-111111111111', definition_digest: `sha256:${'a'.repeat(64)}`, work_version: 7,
  can_edit: true, presentation: null, components: [{ id: 'details', title: 'Customer details', kind: 'note' }, { id: 'add', title: 'Add stock', kind: 'form' }],
};
const presentation = {
  version: 1 as const, description: 'Your inventory', sections: [{ id: 'main', components: ['details'] }],
  actions: [{ id: 'new_stock', label: 'Receive stock', behavior: 'dialog' as const, target: 'add' }],
  theme: { mode: 'dark', accent: 'green', density: 'comfortable', font: 'mono' },
};

test('the editor preserves exact publication/digest/work premises independently of domain labels', () => {
  assert.deepEqual(presentationRequest(state, presentation), {
    expected_publication_id: state.publication_id, expected_definition_digest: state.definition_digest,
    expected_work_version: 7, presentation,
  });
  assert.equal(editorState({ ...state, components: [...state.components, state.components[0]] }), null);
  assert.equal(editorState({ ...state, components: [{ id: '../other_work', title: 'Other', kind: 'form' }] }), null);
  assert.equal(editorState({ ...state, work_version: 1.5 }), null);
  assert.equal(editorState({ ...state, presentation: { version: 2 } }), null);
});

test('lost acknowledgements recover only an exact current presentation with edit authority', () => {
  const saved = { ...state, work_version: 8, presentation };
  assert.equal(recoveredPresentation(saved, presentation), saved);
  const reordered = { ...saved, presentation: { theme: presentation.theme, actions: presentation.actions, sections: presentation.sections, description: presentation.description, version: 1 } };
  assert.equal(recoveredPresentation(reordered, presentation), reordered, 'JSON object key order is not a difference');
  assert.equal(recoveredPresentation({ ...saved, can_edit: false }, presentation), null);
  assert.equal(recoveredPresentation({ ...saved, presentation: { ...presentation, actions: [{ ...presentation.actions[0], target: 'details' }] } }, presentation), null, 'a raced action target is never reported as saved');
  assert.equal(recoveredPresentation({ ...saved, presentation: { ...presentation, theme: { ...presentation.theme, mode: 'light' } } }, presentation), null);
  assert.equal(recoveredPresentation(null, presentation), null);
});
