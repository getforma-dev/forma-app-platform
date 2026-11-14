// APP-142: page size is this browser's layout, and a share names the saved work.
import assert from 'node:assert/strict';
import { test } from 'node:test';

const APP = 'aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee';
const WORK = '11111111-2222-4333-8444-555555555555';
const MEMBER = '0123456789abcdef';

test('a page stays at least the header and one part, and no larger than the viewport', async () => {
  const { pageSize, PAGE_MIN_WIDTH, PAGE_MIN_HEIGHT } = await import('../src/canvas/page.ts');
  assert.deepEqual(pageSize({ width: 100, height: 100 }, { width: 2000, height: 2000 }), { width: PAGE_MIN_WIDTH, height: PAGE_MIN_HEIGHT });
  assert.deepEqual(pageSize({ width: 900, height: 900 }, { width: 800, height: 600 }), { width: 800, height: 600 });
  assert.deepEqual(pageSize({ width: 500, height: 400 }, { width: 100, height: 100 }), { width: PAGE_MIN_WIDTH, height: PAGE_MIN_HEIGHT });
  assert.deepEqual(pageSize({ width: 500, height: 400 }, { width: 1000, height: 800 }), { width: 500, height: 400 });
});

test('a stored page keeps its size, and a record from before size still loads', async () => {
  const store = new Map<string, string>();
  (globalThis as { localStorage?: unknown }).localStorage = {
    getItem: (key: string) => store.get(key) ?? null,
    setItem: (key: string, value: string) => { store.set(key, value); },
  };
  const { loadUnited, saveUnited } = await import('../src/canvas/layout.ts');
  const previous = { 'card-1': { x: 1, y: 2 } };
  saveUnited(MEMBER, [{ work: WORK, app: APP, x: 10, y: 20, previous, width: 640, height: 480 }]);
  const sized = loadUnited(MEMBER);
  assert.equal(sized.length, 1);
  assert.equal(sized[0]?.width, 640);
  assert.equal(sized[0]?.height, 480);
  assert.equal(sized[0]?.x, 10);
  assert.deepEqual(sized[0]?.previous['card-1'], { x: 1, y: 2 });

  saveUnited(MEMBER, [{ work: WORK, app: APP, x: 3, y: 4, previous: {} }]);
  const plain = loadUnited(MEMBER);
  assert.equal(plain[0]?.width, undefined);
  assert.equal(plain[0]?.height, undefined);
  assert.equal(plain[0]?.x, 3);

  store.set(`forma.canvas.united.v1:${MEMBER}`, JSON.stringify([
    { work: WORK, app: APP, x: 1, y: 2, previous: {}, width: 10, height: 9000 },
    { work: WORK, app: APP, x: 8, y: 9, previous: {}, width: '640', height: Number.POSITIVE_INFINITY },
  ]));
  const dropped = loadUnited(MEMBER);
  assert.equal(dropped.length, 2);
  assert.equal(dropped[0]?.width, undefined);
  assert.equal(dropped[0]?.height, undefined);
  assert.equal(dropped[1]?.width, undefined);
  assert.equal(dropped[1]?.height, undefined);
  assert.equal(dropped[1]?.x, 8);
});

test('sharing a page names the saved work, the member and the role, nobody else', async () => {
  const { appShareRequest } = await import('../src/canvas/page.ts');
  const grantee = '99999999-8888-4777-8666-555555555555';
  const request = appShareRequest(APP, WORK, grantee);
  assert.ok(request);
  assert.equal(request.url, `/v1/work/${APP}/${WORK}/share`);
  const body = JSON.parse(request.body) as Record<string, unknown>;
  // APP-192: every share names its role; a page share is a viewer share.
  assert.deepEqual(Object.keys(body), ['grantee_principal_id', 'role']);
  assert.equal(body.grantee_principal_id, grantee);
  assert.equal(body.role, 'viewer');
  assert.equal(JSON.parse(appShareRequest(APP, WORK, grantee, 'contributor')!.body).role, 'contributor');
  assert.equal(appShareRequest(APP.toUpperCase(), WORK, grantee), null);
  assert.equal(appShareRequest(APP, WORK, 'not-a-member'), null);
  assert.equal(appShareRequest(APP, 'card-1', grantee), null);
});
