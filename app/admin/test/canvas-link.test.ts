// The work page's "Open on the canvas" link is /canvas#g-{work}. A fresh
// browser has no local placement. The link loads that work only when the
// authorized library lists it, and it does not fetch a work the page omitted.
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { canvasLinkPlan, listedWorkApp, requestedWork } from '../src/canvas/board.ts';

const WORK = '00000000-0000-4000-8000-000000001682';
const APP = '00000000-0000-4000-8000-000000001681';
const OTHER = '00000000-0000-4000-8000-000000001702';

test('the canvas link loads an unlisted-placement work and leaves an unknown work alone', () => {
  const hash = `#g-${WORK}`;
  assert.equal(requestedWork(hash), WORK);
  assert.equal(requestedWork('#timing=1'), null);
  assert.equal(requestedWork('#g-not-a-work'), null);
  assert.equal(canvasLinkPlan(hash, false, APP), 'place');
  assert.equal(canvasLinkPlan(hash, true, APP), 'focus');
  assert.equal(canvasLinkPlan(hash, false, null), 'absent');
  assert.equal(canvasLinkPlan('#focus=card', false, APP), 'none');

  const root = {
    querySelectorAll(selector: string) {
      assert.equal(selector, '.cv-lib-app');
      return [
        { dataset: { work: WORK, app: APP } },
        { dataset: { work: OTHER, app: 'not-a-uuid' } },
      ];
    },
  };
  assert.equal(listedWorkApp(root as unknown as ParentNode, WORK), APP);
  assert.equal(listedWorkApp(root as unknown as ParentNode, OTHER), null);
  assert.equal(listedWorkApp(root as unknown as ParentNode, '00000000-0000-4000-8000-000000009999'), null);
});
