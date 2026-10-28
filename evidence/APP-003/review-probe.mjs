// Independent focused replay of the committed consumer's actual native binary.
// Uses its locked Chromium dependency; writes only separate reviewer evidence.
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { writeFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const root = resolve(here, '../..');
const spike = resolve(root, 'spikes/APP-003');
const { chromium } = await import(pathToFileURL(resolve(spike, 'admin/node_modules/playwright/index.mjs')));
const server = spawn(resolve(spike, 'target/release/genui-canvas-spike'), [], {
  cwd: spike, env: { ...process.env, FORMA_PORT: '0' }, stdio: ['ignore', 'pipe', 'pipe'],
});
const report = { scope: 'Independent focused native/keyboard/filter replay; no durable commands or managed acceptance', passed: false };
let browser;
try {
  const origin = await new Promise((accept, reject) => {
    let output = '';
    const timer = setTimeout(() => reject(new Error('startup timeout')), 20_000);
    server.once('exit', code => { clearTimeout(timer); reject(new Error(`server exited ${code}`)); });
    server.stdout.on('data', data => {
      output += data;
      const match = output.match(/FORMA_LISTENING (http:\/\/\S+)/);
      if (match) { clearTimeout(timer); accept(match[1]); }
    });
    server.stderr.on('data', () => {});
  });
  browser = await chromium.launch();
  const page = await browser.newPage();
  const errors = [];
  page.on('pageerror', error => errors.push(error.message));
  await page.addInitScript(() => {
    globalThis.reviewViolations = [];
    document.addEventListener('securitypolicyviolation', e => globalThis.reviewViolations.push(e.violatedDirective));
  });
  await page.goto(origin);
  await page.evaluate(async () => {
    await Promise.all([...document.querySelectorAll('script[type="module"][src]')].map(s => import(s.src)));
    globalThis.reviewNodes = [document.querySelector('.canvas'), ...document.querySelector('.canvas').querySelectorAll('*')];
  });
  const kinds = await page.locator('.card').evaluateAll(nodes => nodes.map(n => n.dataset.kind));
  assert.deepEqual(kinds, ['summary','table','detail','timeline','chart','filter','form','note','status']);
  const geometry = await page.locator('svg rect').evaluateAll(nodes => nodes.map(n => ({
    namespace: n.namespaceURI, titleNamespace: n.querySelector('title').namespaceURI,
    values: ['x','y','width','height'].map(a => Number(n.getAttribute(a))),
  })));
  assert.equal(geometry.length, 12);
  geometry.forEach((bar, i) => {
    assert.equal(bar.namespace, 'http://www.w3.org/2000/svg');
    assert.equal(bar.titleNamespace, bar.namespace);
    assert.deepEqual(bar.values, [(i % 6) * 35 + 0.5, 0, 12.5, 10 + (i % 6) * 9]);
  });
  assert.equal(await page.evaluate(() => globalThis.pwned), undefined);
  assert.ok((await page.locator('.note').textContent()).includes('<script>globalThis.pwned=true</script>'));

  // Reach and complete the form using real Tab/typing/selection keys.
  let tabs = 0;
  while (await page.evaluate(() => document.activeElement?.id) !== 'observation' && tabs < 30) {
    await page.keyboard.press('Tab'); tabs += 1;
  }
  assert.ok(tabs < 30, 'keyboard cannot reach observation');
  await page.keyboard.type('Independent keyboard observation');
  await page.keyboard.press('Tab');
  assert.equal(await page.evaluate(() => document.activeElement?.id), 'decision');
  await page.keyboard.press('ArrowDown');
  await page.keyboard.press('Tab');
  assert.equal(await page.evaluate(() => document.activeElement?.textContent), 'Save observation');
  const submitted = page.waitForResponse(r => r.url() === `${origin}/commands`);
  await page.keyboard.press('Enter');
  assert.equal((await submitted).status(), 200);
  await page.locator('output').filter({ hasText: 'Synthetic observation accepted' }).waitFor();
  report.keyboard = { tabs_to_observation: tabs, explicit_submit: true, acknowledgement: await page.locator('output').textContent() };

  // Prove an actual changed result before testing old-response withholding.
  await page.getByLabel('Linked destination', { exact: true }).selectOption('unmatched');
  await page.waitForFunction(() => [...document.querySelectorAll('.linked-result')].every(n => n.textContent === 'Unmatched: 900,000 bytes'));
  assert.equal(await page.locator('.linked-result').count(), 6);
  report.changed_filter = await page.locator('.linked-result').allTextContents();
  await page.getByLabel('Linked destination', { exact: true }).selectOption('all');
  await page.waitForFunction(() => [...document.querySelectorAll('.linked-result')].every(n => n.textContent === 'All: 1,140,000 bytes'));
  assert.ok(await page.evaluate(() => globalThis.reviewNodes.every(n => n.isConnected)));

  // Hold the completed old server response until the newer result is visible.
  await page.reload();
  await page.evaluate(async () => {
    await Promise.all([...document.querySelectorAll('script[type="module"][src]')].map(s => import(s.src)));
  });
  let releaseOld;
  const held = new Promise(done => { releaseOld = done; });
  let oldReady;
  const ready = new Promise(done => { oldReady = done; });
  await page.route('**/linked?**', async route => {
    if (new URL(route.request().url()).searchParams.get('epoch') === '1') {
      const response = await route.fetch();
      oldReady(); await held; await route.fulfill({ response });
    } else await route.continue();
  });
  await page.getByLabel('Linked destination', { exact: true }).selectOption('unmatched');
  await ready;
  await page.getByLabel('Linked destination', { exact: true }).selectOption('all');
  await page.waitForFunction(() => document.querySelector('.canvas').dataset.epoch === '2');
  const oldResponse = page.waitForResponse(r => r.url().includes('/linked?') && r.url().includes('epoch=1'));
  releaseOld(); await (await oldResponse).finished();
  await page.evaluate(() => new Promise(done => requestAnimationFrame(() => requestAnimationFrame(done))));
  assert.equal(await page.locator('.canvas').getAttribute('data-epoch'), '2');
  assert.deepEqual(await page.locator('.linked-result').allTextContents(), Array(6).fill('All: 1,140,000 bytes'));
  report.stale_response = { old_response_fully_delivered_after_new: true, resulting_epoch: 2, six_views_still_current: true };
  assert.equal((await fetch(`${origin}/linked?filter=unsupported&epoch=3`)).status, 400);
  assert.equal((await fetch(`${origin}/commands`, { method: 'POST', body: new URLSearchParams({observation:'bad'}) })).status, 422);
  assert.deepEqual(errors, []);
  assert.deepEqual(await page.evaluate(() => globalThis.reviewViolations), []);
  report.kinds = kinds;
  report.numeric_svg = geometry;
  report.invalid_filter_status = 400;
  report.invalid_command_status = 422;
  report.page_errors = errors;
  report.csp_violations = [];
  report.browser = browser.version();
  report.passed = true;
} catch (error) {
  report.error = String(error.stack || error); throw error;
} finally {
  if (browser) await browser.close();
  if (server.exitCode === null && server.signalCode === null) {
    await new Promise(done => {
      const timer = setTimeout(() => server.kill('SIGKILL'), 5_000);
      server.once('exit', () => { clearTimeout(timer); done(); });
      server.kill('SIGTERM');
    });
  }
  writeFileSync(resolve(here, 'review-probe.json'), JSON.stringify(report, null, 2) + '\n');
}
console.log(JSON.stringify({ passed: report.passed, scope: report.scope }));
