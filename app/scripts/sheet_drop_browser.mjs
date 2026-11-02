// PR42 follow-up: real local file uploads and mouse drops; only actual HTTP
// response delivery is deferred. No model, provider, hosted or cloud action.
import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { resolve } from 'node:path';

const args = Object.fromEntries(process.argv.slice(2).map(value => value.replace(/^--/, '').split('=')));
const origin = args.origin || 'http://127.0.0.1:4702';
assert.match(origin, /^http:\/\/127\.0\.0\.1:(?!400[1-9]$|4010$)\d+$/);
const out = resolve(args.output || '.cache/reports/sheet-review-fixes');
await mkdir(out, { recursive: true });
const report = {
  scope: 'Owned loopback stand; synthetic member, actual upload/storage/read routes and mouse gestures. Response delays retain real server responses. No model/provider/hosted claim.',
  started_at: new Date().toISOString(), checks: [], errors: [], requests: [], observations: [],
};
const { chromium } = await import(process.env.PLAYWRIGHT_MODULE || '/tmp/audit-browser/node_modules/playwright/index.mjs');
const browser = await chromium.launch({ channel: 'chrome', headless: true });
const context = await browser.newContext({ viewport: { width: 1600, height: 1100 }, reducedMotion: 'reduce' });
const page = await context.newPage();
page.setDefaultTimeout(15000);
page.on('pageerror', error => report.errors.push(error.message));
page.on('request', request => {
  const path = new URL(request.url()).pathname;
  if (path.startsWith('/v1/boxes')) report.requests.push({ method: request.method(), path, at: new Date().toISOString() });
});
let forbiddenCalls = 0;
page.on('request', request => {
  if (request.method() === 'POST' && /\/(apps|data|compose|ask)$/.test(new URL(request.url()).pathname)) forbiddenCalls++;
});
const check = text => report.checks.push(text);
const csv = name => ({ name, mimeType: 'text/csv', buffer: Buffer.from('item,amount\nreview,12\n') });
const textFile = name => ({ name, mimeType: 'text/plain', buffer: Buffer.from('Synthetic companion file.\n') });
const pending = async (kind, details) => writeFile(resolve(out, 'pending-action.json'), JSON.stringify({ kind, details, at: new Date().toISOString() }, null, 2));
const clearPending = () => writeFile(resolve(out, 'pending-action.json'), 'null\n');
const uploadRequest = request => request.method() === 'POST' && /^\/v1\/boxes\/[^/]+\/files$/.test(new URL(request.url()).pathname);
const uploadedName = async (response, name) => uploadRequest(response.request()) && response.status() === 201 && (await response.json()).file?.name === name;
const uploadsTo = box => report.requests.filter(row => row.method === 'POST' && row.path === `/v1/boxes/${box}/files`).length;
const activity = () => page.locator('#cv-activity').textContent();
const settle = () => page.waitForTimeout(180);
const wait = async (predicate, label, milliseconds = 15000) => {
  const end = Date.now() + milliseconds;
  while (!(await predicate())) {
    if (Date.now() >= end) throw Error(`Timed out: ${label}`);
    await page.waitForTimeout(50);
  }
};
const releases = new Set();
const cdp = await context.newCDPSession(page);
let activeHold = null;
await cdp.send('Fetch.enable', { patterns: [{ urlPattern: '*://127.0.0.1:*/v1/boxes/*', requestStage: 'Response' }] });
cdp.on('Fetch.requestPaused', async event => {
  const request = { method: () => event.request.method, url: () => event.request.url };
  const hold = activeHold;
  if (!hold || hold.matched || !hold.matcher(request)) {
    await cdp.send('Fetch.continueResponse', { requestId: event.requestId }).catch(error => report.errors.push(String(error)));
    return;
  }
  hold.matched = true;
  try {
    const raw = await cdp.send('Fetch.getResponseBody', { requestId: event.requestId });
    const body = Buffer.from(raw.body, raw.base64Encoded ? 'base64' : 'utf8');
    const headers = Object.fromEntries((event.responseHeaders || []).map(header => [header.name.toLowerCase(), header.value]));
    const metadata = { path: new URL(event.request.url).pathname, status: event.responseStatusCode, sha256: createHash('sha256').update(body).digest('hex'), delay: 'Chrome response-stage; browser request unchanged' };
    if ((headers['content-type'] || '').includes('application/json')) metadata.json = JSON.parse(body.toString());
    report.observations.push(metadata);
    hold.received(metadata);
    await hold.gate;
    await cdp.send('Fetch.continueResponse', { requestId: event.requestId });
  } catch (error) { hold.reject(error); report.errors.push(String(error)); }
  finally { hold.finished(); }
});
async function holdResponse(matcher) {
  assert.equal(activeHold, null);
  let release, received, finished, reject;
  const gate = new Promise(resolve => { release = resolve; });
  let timeout;
  const got = new Promise((resolve, fail) => {
    received = value => { clearTimeout(timeout); resolve(value); };
    reject = error => { clearTimeout(timeout); fail(error); };
    timeout = setTimeout(() => fail(Error('Expected real response did not reach the delay gate within 30 seconds')), 30000);
  });
  const done = new Promise(resolve => { finished = resolve; });
  activeHold = { matcher, matched: false, gate, received, finished, reject, cancel: () => clearTimeout(timeout) };
  const unblock = async () => { release(); await done; activeHold = null; releases.delete(unblock); };
  releases.add(unblock);
  return { got, release: unblock };
}
async function files(box) {
  const response = await context.request.get(origin + `/v1/boxes/${box}`);
  assert.equal(response.status(), 200);
  return (await response.json()).files;
}
async function pick(specs) {
  await pending('upload', specs.map(file => ({ name: file.name, bytes: file.buffer.length, sha256: createHash('sha256').update(file.buffer).digest('hex') })));
  await page.locator('[data-empty-upload]').setInputFiles(specs);
}
async function uploadSheet(name) {
  const response = page.waitForResponse(response => uploadedName(response, name));
  await pick([csv(name)]);
  const actual = await response;
  assert.equal(actual.status(), 201);
  const result = await actual.json();
  const sheet = page.locator(`.bx-sheet[data-file="${result.file.file_id}"]`);
  await sheet.waitFor();
  await clearPending();
  await dismissNotice();
  return { sheet, file: result.file };
}
async function dismissNotice() {
  const notice = page.locator('#bx-notice');
  if (await notice.isVisible()) await notice.getByRole('button', { name: 'Dismiss', exact: true }).click();
}
async function fit() {
  await dismissNotice();
  await page.locator('[data-tool="zoom-menu"]').click();
  await page.locator('[data-tool="fit"]').click();
  await settle();
}
async function center(locator) {
  const box = await locator.boundingBox();
  assert.ok(box && box.width > 0 && box.height > 0);
  return { x: box.x + box.width / 2, y: box.y + box.height / 2 };
}
async function drag(sheet, folder, { release = true } = {}) {
  await fit();
  const from = await center(sheet);
  const to = await center(folder.locator('.bx-drop'));
  await page.mouse.move(from.x, from.y);
  await page.mouse.down();
  await page.mouse.move(to.x, to.y, { steps: 14 });
  await sheet.locator('..').evaluate(node => { if (!node.classList.contains('is-dragging')) throw Error('Real mouse did not start file drag'); });
  if (release) { await page.mouse.up(); await settle(); }
  assert.equal(await page.locator('.bx-modal').count(), 0, 'Dragging must not open the original viewer');
  return { from, to };
}
async function remove(sheet) {
  await sheet.getByRole('button', { name: /^Remove .+ from the canvas$/ }).focus();
  await page.keyboard.press('Enter');
  await sheet.waitFor({ state: 'detached' });
}
try {
  assert.equal((await fetch(origin + '/health/live')).status, 200, 'Start only after the owned stand is ready');
  await page.goto(origin + '/canvas');
  await page.locator('#cv-canvas.is-canvas').waitFor();
  const listing = await context.request.get(origin + '/v1/boxes');
  report.observations.push({ readiness_listing_status: listing.status(), at: new Date().toISOString() });
  assert.equal(listing.status(), 200);
  const boxes = (await listing.json()).boxes;
  const uploads = boxes.find(box => box.kind === 'uploads');
  const existing = uploads ? (await files(uploads.box_id)).length : 0;
  assert.ok(existing <= 13, 'Need seven spare Uploads slots; never reset an existing member fixture implicitly');
  report.observations.push({ initial_uploads_count: existing });

  // Both operations are real; the companion has reached the server but its
  // response is held before the UI can know it completed.
  const mixed = await holdResponse(uploadRequest);
  const firstSheetResponse = page.waitForResponse(response => uploadedName(response, 'mixed.csv'));
  void firstSheetResponse.catch(() => {});
  await pick([csv('mixed.csv'), textFile('held-companion.txt')]);
  const companion = await mixed.got;
  assert.equal(companion.status, 201);
  const target = companion.path.split('/')[3];
  const folder = page.locator(`.bx-card[data-box="${target}"]`);
  await folder.waitFor();
  await settle();
  assert.doesNotMatch((await activity()) || '', /holds your|other files are in|file added to the folder/);
  assert.equal(report.requests.filter(row => row.method === 'POST' && row.path.includes('/files')).length, 1, 'CSV waits for the companion outcome');
  await page.screenshot({ path: resolve(out, 'companion-pending.png'), fullPage: true });
  await mixed.release();
  const firstUpload = await firstSheetResponse;
  assert.equal(firstUpload.status(), 201);
  const first = (await firstUpload.json()).file;
  await page.locator(`.bx-sheet[data-file="${first.file_id}"]`).waitFor();
  await wait(async () => /1 file added to the folder/.test((await page.locator('#bx-notice').textContent()) || ''), 'mixed completed notice');
  assert.equal((await files(target)).filter(file => file.name === 'held-companion.txt').length, 1);
  await clearPending();
  check('Mixed CSV/text upload makes no completed companion claim while its actual response is held; completion reports the stored file after release.');

  await dismissNotice();
  const oversizedPosts = report.requests.filter(row => row.method === 'POST' && row.path.endsWith('/files')).length;
  const oversizeCsvResponse = page.waitForResponse(response => uploadedName(response, 'with-rejected-companion.csv'));
  await pick([csv('with-rejected-companion.csv'), { name: 'too-large.txt', mimeType: 'text/plain', buffer: Buffer.alloc(20 * 1024 * 1024 + 1, 65) }]);
  const oversizeCsv = await oversizeCsvResponse;
  assert.equal(oversizeCsv.status(), 201);
  await wait(async () => /0 of 1/.test((await page.locator('#bx-notice').textContent()) || ''), 'oversize mixed outcome');
  assert.match(await page.locator('#bx-notice').textContent(), /0 of 1 uploads confirmed/);
  await page.getByText('too-large.txt is over 20 MB and was not uploaded.', { exact: true }).first().waitFor();
  assert.doesNotMatch(await page.locator('#bx-notice').textContent(), /other files are in|holds your/);
  assert.equal(report.requests.filter(row => row.method === 'POST' && row.path.endsWith('/files')).length, oversizedPosts + 1, 'Oversize companion never posts');
  await page.screenshot({ path: resolve(out, 'companion-rejected.png'), fullPage: true });
  await clearPending();
  check('Over-20-MiB companion is rejected locally; the mixed notice reports 0 of 1 stored and keeps the CSV file without a false folder success.');
  await dismissNotice();
  for (const other of await page.locator('.bx-card').all()) if (await other.getAttribute('data-box') !== target) await other.getByRole('button', { name: 'Remove the folder from the canvas', exact: true }).click();

  const normal = await uploadSheet('drop-normal.csv');
  const beforeNormal = (await files(target)).length;
  const copiedResponse = page.waitForResponse(response => response.request().method() === 'POST' && new URL(response.url()).pathname === `/v1/boxes/${target}/files`);
  await pending('normal-drop', { target, file: normal.file.file_id });
  await drag(normal.sheet, folder);
  const copied = await copiedResponse;
  assert.equal(copied.status(), 201);
  const newFile = (await copied.json()).file;
  await page.locator(`.bx-sheet[data-file="${newFile.file_id}"]`).waitFor();
  assert.equal((await files(target)).length, beforeNormal + 1);
  assert.match(await activity(), /was copied into the folder/);
  assert.doesNotMatch(await activity(), /Ask|Make an app|can read/);
  await clearPending();
  check('A completed real mouse drop copies exactly one immutable file; success claims only the copy, never automatic Ask/app context inclusion.');
  const copiedSheet = page.locator(`.bx-sheet[data-file="${newFile.file_id}"]`);
  await copiedSheet.dblclick();
  await page.getByText(/Original file · rows 1–2 of 2/).waitFor();
  assert.match(await page.locator('.bx-modal tbody').textContent(), /review/);
  await page.keyboard.press('Escape');
  check('Double-clicking the successfully copied file opens its original cells after drag; drag itself never opens a viewer.');
  await remove(copiedSheet);

  const cancelled = await uploadSheet('drop-cancelled.csv');
  await fit();
  const prior = uploadsTo(target), beforeCancelled = (await files(target)).length;
  const recent = await center(folder.locator('.bx-drop'));
  await page.mouse.click(recent.x, recent.y);
  await drag(cancelled.sheet, folder, { release: false });
  // Move a real mouse gesture, then simulate the browser's lost-window signal.
  // This is explicitly not a fabricated HTTP result or a successful pointerup.
  await page.evaluate(() => window.dispatchEvent(new Event('blur')));
  await page.mouse.up();
  await settle();
  assert.equal(await page.locator('.bx-modal').count(), 0);
  assert.equal(uploadsTo(target), prior);
  assert.equal((await files(target)).length, beforeCancelled);
  check('A real drag cancelled through the browser blur handler after a recent folder click performs no copy; the old pointer release is not reused.');
  await remove(cancelled.sheet);

  const removedBefore = await uploadSheet('removed-before-copy.csv');
  const holdingContent = await holdResponse(request => request.method() === 'GET' && new URL(request.url()).pathname === removedBefore.file.content_url);
  const beforeContentPosts = uploadsTo(target), beforeContentFiles = (await files(target)).length;
  await pending('remove-during-content-read', { target, file: removedBefore.file.file_id });
  await drag(removedBefore.sheet, folder);
  assert.equal((await holdingContent.got).status, 200);
  await remove(removedBefore.sheet);
  await holdingContent.release();
  await wait(async () => /not copied.*removed/.test((await activity()) || ''), 'removed before upload status');
  assert.equal(uploadsTo(target), beforeContentPosts);
  assert.equal((await files(target)).length, beforeContentFiles);
  await clearPending();
  check('Removing the icon while the actual source-content response is held prevents the copy upload and reports removal honestly.');

  const removedAfter = await uploadSheet('removed-after-copy.csv');
  const holdingUpload = await holdResponse(request => uploadRequest(request) && new URL(request.url()).pathname === `/v1/boxes/${target}/files`);
  const beforeUploadPosts = uploadsTo(target), beforeUploadFiles = (await files(target)).length;
  await pending('remove-after-server-copy', { target, file: removedAfter.file.file_id });
  await drag(removedAfter.sheet, folder);
  assert.equal((await holdingUpload.got).status, 201);
  assert.equal((await files(target)).length, beforeUploadFiles + 1, 'Server copy is already committed before client response');
  await remove(removedAfter.sheet);
  await holdingUpload.release();
  await wait(async () => /copied into the folder.*removed from the canvas/.test((await activity()) || ''), 'removed after upload status');
  assert.equal(uploadsTo(target), beforeUploadPosts + 1);
  assert.equal((await files(target)).length, beforeUploadFiles + 1);
  await clearPending();
  check('Removing the icon after a real server copy but before response delivery preserves exactly one copied file and reports the removed icon.');

  const duplicate = await uploadSheet('duplicate-drop.csv');
  const holdingDuplicate = await holdResponse(request => request.method() === 'GET' && new URL(request.url()).pathname === duplicate.file.content_url);
  const beforeDuplicatePosts = uploadsTo(target), beforeDuplicateFiles = (await files(target)).length;
  await pending('duplicate-drop-in-flight', { target, file: duplicate.file.file_id });
  await drag(duplicate.sheet, folder);
  assert.equal((await holdingDuplicate.got).status, 200);
  const point = await center(duplicate.sheet);
  await page.mouse.move(point.x, point.y); await page.mouse.down();
  await page.mouse.move(point.x + 32, point.y + 20, { steps: 5 });
  await page.mouse.move(point.x, point.y, { steps: 5 }); await page.mouse.up();
  await wait(async () => /already being copied/.test((await activity()) || ''), 'duplicate drop status');
  await holdingDuplicate.release();
  await wait(async () => /was copied into the folder/.test((await activity()) || ''), 'first copy completes');
  assert.equal(uploadsTo(target), beforeDuplicatePosts + 1);
  assert.equal((await files(target)).length, beforeDuplicateFiles + 1);
  await clearPending();
  check('A second real drop during the first content read is refused visibly; the original copy completes once with no duplicate upload.');
  await page.screenshot({ path: resolve(out, 'drop-results.png'), fullPage: true });
  assert.equal(forbiddenCalls, 0, 'No model, app generation, source registration or Ask calls');
  assert.deepEqual(report.errors, []);
  report.passed = true;
} catch (error) {
  report.passed = false;
  report.failure = String(error);
  await page.screenshot({ path: resolve(out, 'failure.png'), fullPage: true }).catch(() => {});
  throw error;
} finally {
  activeHold?.cancel();
  report.completed_at = new Date().toISOString();
  await writeFile(resolve(out, 'report.json'), JSON.stringify(report, null, 2));
  // Closing the isolated browser aborts pending client delivery; do not release
  // a failed gate and accidentally let the UI begin a dependent copy.
  await browser.close();
}
