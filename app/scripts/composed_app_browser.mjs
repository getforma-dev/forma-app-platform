// APP-160: real product controls on an owned disposable dev stand. This is
// synthetic local identity qualification, never a hosted-provider claim.
// PLAYWRIGHT_MODULE must identify an installed Playwright module. No dependency
// or credential is downloaded by this runner. Outputs stay in the given cache.
import assert from 'node:assert/strict';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { createHash } from 'node:crypto';

const options = Object.fromEntries(process.argv.slice(2).map(arg => arg.replace(/^--/, '').split('=')));
const origin = options.owner || 'http://127.0.0.1:4801';
const guestOrigin = options.guest || 'http://127.0.0.1:4804';
if (![origin, guestOrigin].every(value => /^http:\/\/127\.0\.0\.1:\d+$/.test(value))) throw new Error('Disposable loopback stand required');
if (!process.env.PLAYWRIGHT_MODULE) throw new Error('Set PLAYWRIGHT_MODULE to the installed Playwright entry');
const { chromium } = await import(process.env.PLAYWRIGHT_MODULE);
const output = resolve(options.output || '.cache/reports/composed-browser');
const manifest = () => readFile(resolve('app/admin/dist/manifest.json')).then(bytes => createHash('sha256').update(bytes).digest('hex'));
const manifestBefore = await manifest();
await mkdir(output, { recursive: true });
const fixturePath = resolve(output, 'fixture.json');
const fixture = JSON.parse(await readFile(fixturePath, 'utf8').catch(() => '{}'));
const saveFixture = () => writeFile(fixturePath, JSON.stringify(fixture, null, 2));
const report = { scope: 'Synthetic local identities; real admission, immutable publication, PostgreSQL, commands and confined record queries', manifest_sha256: manifestBefore, started: new Date().toISOString(), checks: [], errors: [], artifacts: [] };
const browser = await chromium.launch({ channel: 'chrome', headless: true });
const owner = await browser.newContext({ viewport: { width: 1440, height: 1100 }, reducedMotion: 'reduce' });
const page = await owner.newPage();
page.on('pageerror', error => report.errors.push(error.message));
const until = async (fn, timeout = 20000) => { const start = Date.now(); while (Date.now() - start < timeout) { if (await fn()) return; await new Promise(resolve => setTimeout(resolve, 100)); } throw new Error('Condition timed out'); };
const api = async (tab, path, method = 'GET', body) => tab.evaluate(async ({ path, method, body }) => {
  const response = await fetch(path, { method, credentials: 'same-origin', cache: 'no-store', headers: { accept: 'application/json', ...(body ? { 'content-type': 'application/json' } : {}) }, ...(body ? { body: JSON.stringify(body) } : {}) });
  return { status: response.status, body: await response.json().catch(() => null) };
}, { path, method, body });
const shot = async (tab, name) => { await tab.screenshot({ path: resolve(output, name), fullPage: true }); report.artifacts.push(name); };
const created = async id => {
  await page.goto(origin + '/canvas');
  await page.locator('[data-create-app]').click();
  await page.locator(`[data-template="${id}"]`).click();
  const response = page.waitForResponse(response => new URL(response.url()).pathname === '/v1/compose' && response.request().method() === 'POST');
  await page.getByRole('dialog').getByRole('button', { name: 'Create', exact: true }).click();
  const outcome = await response; assert.equal(outcome.status(), 200, await outcome.text());
  const result = await outcome.json(); assert.equal(result.origin, 'adapted');
  await page.locator(`[data-unite="${result.work_id}"]`).first().waitFor();
  return result;
};
const workPath = item => `/w/${item.app_id}/${item.work_id}`;
const basePath = item => `/v1/work/${item.app_id}/${item.work_id}`;

try {
  if (!fixture.dining) { fixture.dining = await created('dining-tracker'); await saveFixture(); report.checks.push('Created Dining tracker through the ordinary template picker; no seed records or model output.'); }
  const dining = fixture.dining;
  await page.goto(origin + workPath(dining));
  await page.getByText('Up to date.', { exact: true }).waitFor();
  const initialDesign = await api(page, basePath(dining) + '/presentation');
  assert.equal(initialDesign.status, 200);
  if (!fixture.designed) {
    await page.getByRole('button', { name: 'Edit app', exact: true }).click();
    const editor = page.locator('[data-presentation-editor]');
    await editor.getByLabel('App introduction', { exact: true }).fill('Every meal, accounted for.');
    await editor.getByLabel('Appearance', { exact: true }).selectOption('dark');
    await editor.getByLabel('Accent', { exact: true }).selectOption('green');
    await editor.getByLabel('Typography', { exact: true }).selectOption('mono');
    await editor.getByLabel('Button 1 label', { exact: true }).fill('Log meal');
    await editor.getByRole('button', { name: 'Add button', exact: true }).click();
    await editor.getByLabel('Button 2 label', { exact: true }).fill('About this tracker');
    await editor.getByLabel('Button 2 component', { exact: true }).selectOption('guide');
    await editor.getByRole('button', { name: 'Add button', exact: true }).click();
    await editor.getByLabel('Button 3 label', { exact: true }).fill('View meals');
    await editor.getByLabel('Button 3 behavior', { exact: true }).selectOption('navigate');
    await editor.getByLabel('Button 3 component', { exact: true }).selectOption('ledger');
    const response = page.waitForResponse(response => response.url().endsWith('/presentation') && response.request().method() === 'POST');
    await editor.getByRole('button', { name: 'Save app design', exact: true }).click();
    const result = await response; assert.equal(result.status(), 200, await result.text());
    await editor.waitFor({ state: 'detached' }); fixture.designed = true; await saveFixture();
    const current = await api(page, basePath(dining) + '/presentation');
    const stale = await api(page, basePath(dining) + '/presentation', 'POST', {
      expected_publication_id: initialDesign.body.publication_id,
      expected_definition_digest: initialDesign.body.definition_digest,
      expected_work_version: initialDesign.body.work_version,
      presentation: current.body.presentation,
    });
    assert.equal(stale.status, 409);
    assert.deepEqual((await api(page, basePath(dining) + '/presentation')).body, current.body);
    report.checks.push('Replaying stale design premises returns409 and leaves the published design and work version unchanged.');
  }
  await page.reload(); await page.getByRole('button', { name: 'Log meal', exact: true }).waitFor();
  assert.equal(await page.locator('[data-document]').getAttribute('data-app-mode'), 'dark');
  assert.equal(await page.locator('[data-document]').getAttribute('data-app-font'), 'mono');
  report.checks.push('Normal Design app controls persisted theme, introduction and independent action labels/behaviors/targets through immutable publication and reload.');
  let commands = 0;
  page.on('request', request => { if (request.method() === 'POST' && new URL(request.url()).pathname.includes('/commands/')) commands++; });
  await page.getByRole('button', { name: 'Log meal', exact: true }).click();
  const modal = page.locator('.app-component-dialog'); await modal.waitFor();
  await modal.locator('[name="value.restaurant"]').fill('Unsent draft');
  assert.equal(await modal.evaluate(node => node.matches(':modal')), true);
  for (let i = 0; i < 12; i++) {
    await page.keyboard.press('Tab');
    assert.equal(await modal.evaluate(node => node.contains(document.activeElement)), true);
  }
  await page.evaluate(() => { window.composedDraft = document.querySelector('[name="value.restaurant"]'); window.composedKey = document.querySelector('[name="idempotency_key"]').value; });
  await page.keyboard.press('Escape'); await modal.waitFor({ state: 'detached' });
  assert.equal(await page.getByRole('button', { name: 'Log meal', exact: true }).evaluate(node => document.activeElement === node), true);
  await page.getByRole('button', { name: 'Log meal', exact: true }).click();
  assert.equal(await modal.locator('[name="value.restaurant"]').inputValue(), 'Unsent draft');
  assert.equal(await page.evaluate(() => window.composedDraft === document.querySelector('[name="value.restaurant"]') && window.composedKey === document.querySelector('[name="idempotency_key"]').value), true);
  assert.equal(commands, 0); await modal.getByRole('button', { name: 'Close', exact: true }).click();
  report.checks.push('Opening/closing/reopening a modal sends no command, retains the same form node/idempotency key and restores focus.');
  await page.getByRole('button', { name: 'About this tracker', exact: true }).click();
  await modal.getByText('Getting started', { exact: true }).waitFor();
  assert.equal(await modal.locator('form[data-command]').count(), 0);
  await page.keyboard.press('Escape');
  await page.getByRole('button', { name: 'View meals', exact: true }).click();
  assert.equal(await page.evaluate(() => document.activeElement?.getAttribute('data-component')), 'ledger');
  report.checks.push('Generic actions also open a read-only note dialog and navigate to the exact admitted list component without submitting a form.');
  const meals = [['Outside', '99.99', '2026-10-31'], ['Cafe A', '12.50', '2026-11-02'], ['Cafe A', '7.50', '2026-11-03'], ['Cafe B', '25.00', '2026-11-04'], ['Outside', '99.99', '2026-12-01']];
  for (let index = fixture.saved || 0; index < meals.length; index++) {
    await page.getByRole('button', { name: 'Log meal', exact: true }).click();
    const [restaurant, amount, date] = meals[index];
    await modal.locator('[name="value.restaurant"]').fill(restaurant);
    await modal.locator('[name="value.amount"]').fill(amount);
    await modal.locator('[name="value.date"]').fill(date);
    if (index === 0) {
      let committedReceipt;
      let firstRequest;
      await page.route('**/commands/*/outcomes/*', route => route.fulfill({ status: 503, contentType: 'application/json', body: '{"code":"injected_lookup_outage"}' }));
      await page.route('**/commands/*', async route => {
        firstRequest = route.request().postData();
        // Playwright's API transport omits browser fetch metadata. Preserve
        // this intercepted same-origin request's context when forwarding it.
        const actual = await route.fetch({ headers: { ...await route.request().allHeaders(), origin, 'sec-fetch-site': 'same-origin' } });
        if (actual.status() !== 200) { await route.fulfill({ response: actual }); return; }
        committedReceipt = await actual.json();
        await route.fulfill({ status: 502, contentType: 'application/json', body: '{"code":"injected_lost_acknowledgement"}' });
      }, { times: 1 });
      await modal.locator('form[data-command] button[type="submit"]').click();
      await modal.locator('[data-command-status]').filter({ hasText: 'could not be confirmed' }).waitFor();
      assert.equal(await modal.locator('[name="value.restaurant"]').isDisabled(), true);
      await page.unroute('**/commands/*/outcomes/*');
      const recovered = page.waitForResponse(response => response.request().method() === 'POST' && new URL(response.url()).pathname.includes('/commands/'));
      await modal.locator('form[data-command] button[type="submit"]').click();
      const replay = await recovered;
      assert.equal(replay.status(), 200, await replay.text());
      assert.equal(replay.request().postData(), firstRequest);
      const stored = await replay.json();
      assert.equal(stored.audit_event_id, committedReceipt.audit_event_id);
      assert.equal(stored.commit_sequence, committedReceipt.commit_sequence);
      fixture.saved = 1; await saveFixture();
      await until(async () => !(await modal.locator('form[data-command] button[type="submit"]').isDisabled()));
      await modal.getByRole('button', { name: 'Close', exact: true }).click();
      report.checks.push('An injected lost acknowledgement plus failed outcome lookup retains frozen form values; retry sends identical bytes/key and returns the same real audit/commit.');
      continue;
    }
    const response = page.waitForResponse(response => response.request().method() === 'POST' && new URL(response.url()).pathname.includes('/commands/'));
    await modal.locator('form[data-command] button[type="submit"]').click();
    const saved = await response; assert.equal(saved.status(), 200, await saved.text());
    const receipt = await saved.json(); assert.equal(receipt.state, 'committed'); assert.ok(receipt.audit_event_id);
    fixture.saved = index + 1; await saveFixture();
    await until(async () => !(await modal.locator('form[data-command] button[type="submit"]').isDisabled()));
    await modal.getByRole('button', { name: 'Close', exact: true }).click();
  }
  const filters = page.locator('[data-record-filters="ledger"]');
  await filters.locator('[data-filter-start="date_filter"]').fill('2026-11-01');
  await filters.locator('[data-filter-end="date_filter"]').fill('2026-12-01');
  await filters.getByRole('button', { name: 'Apply filters', exact: true }).click();
  await until(async () => await page.locator('[data-record-component="total"] .record-metric').textContent().catch(() => '') === '$45.00');
  assert.equal(await page.locator('[data-record-component="ledger"] tbody tr').count(), 3);
  assert.match(await page.locator('[data-record-component="budget"]').innerText(), /45\.00% used/);
  assert.match(await page.locator('[data-record-component="budget"]').innerText(), /\$55\.00 remaining/);
  const receipt = JSON.parse(await page.locator('[data-record-component="total"] .receipt pre').textContent());
  assert.ok(receipt.digest); fixture.receipt = receipt; await saveFixture();
  await shot(page, 'member-desktop.png');
  report.checks.push('Five real audited commands persist the half-open date fixture: November total45.00, count3, budget45percent/55.00remaining and inspectable snapshot receipt.');
  await filters.locator('[data-filter-value="restaurant_filter"]').fill('Cafe B');
  await filters.getByRole('button', { name: 'Apply filters', exact: true }).click();
  await until(async () => await page.locator('[data-record-component="total"] .record-metric').textContent().catch(() => '') === '$25.00');
  assert.equal(await page.locator('[data-record-component="ledger"] tbody tr').count(), 1);
  await page.getByRole('button', { name: 'Refresh', exact: true }).click();
  await until(async () => await page.locator('[data-record-component="total"] .record-metric').textContent().catch(() => '') === '$25.00');
  assert.equal(await filters.locator('[data-filter-value="restaurant_filter"]').inputValue(), 'Cafe B');
  report.checks.push('Shared restaurant/date filters update all declared metrics and survive an ordinary live document refresh.');
  await page.route('**/record-components', async route => {
    if (route.request().method() === 'POST') await route.fulfill({ status: 503, contentType: 'application/json', body: '{"code":"injected_read_failure"}' });
    else await route.continue();
  });
  await filters.locator('[data-filter-value="restaurant_filter"]').fill('Cafe A');
  await filters.getByRole('button', { name: 'Apply filters', exact: true }).click();
  await until(async () => await page.locator('[data-record-component="total"]').getAttribute('data-state') === 'failed');
  assert.equal(await page.locator('[data-record-component="total"] .record-metric').count(), 0);
  assert.equal(await filters.locator('[data-filter-value="restaurant_filter"]').inputValue(), 'Cafe A');
  await page.unroute('**/record-components');
  await filters.getByRole('button', { name: 'Apply filters', exact: true }).click();
  await until(async () => await page.locator('[data-record-component="total"] .record-metric').textContent().catch(() => '') === '$20.00');
  await filters.locator('[data-filter-value="restaurant_filter"]').fill('No matching restaurant');
  await filters.getByRole('button', { name: 'Apply filters', exact: true }).click();
  await until(async () => await page.locator('[data-record-component="total"] .record-metric').textContent().catch(() => '') === '$0.00');
  assert.equal(await page.locator('[data-record-component="ledger"] tbody tr').count(), 0);
  await filters.getByRole('button', { name: 'Reset filters', exact: true }).click();
  await until(async () => await page.locator('[data-record-component="total"] .record-metric').textContent().catch(() => '') === '$244.98');
  report.checks.push('Injected read failure masks metrics while retaining the filter; retry recovers exact values, empty matches are genuine zero, and reset restores all five records.');
  for (const width of [390, 320]) {
    await page.setViewportSize({ width, height: 844 });
    assert.equal(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), true);
    await page.getByRole('button', { name: 'Log meal', exact: true }).click();
    await modal.locator('[name="value.restaurant"]').fill('Phone draft');
    assert.equal(await modal.evaluate(node => node.scrollWidth <= node.clientWidth + 1), true);
    await shot(page, `modal-${width}.png`);
    await page.keyboard.press('Escape');
    await shot(page, `member-${width}.png`);
  }
  report.checks.push('320px and390px pages and modal remain within the viewport; Escape restores the same trigger.');
  await page.setViewportSize({ width: 1440, height: 1100 });
  await page.getByRole('link', { name: 'Open on the canvas', exact: true }).click();
  const united = page.locator(`.cv-unite[data-work="${dining.work_id}"]`);
  await united.waitFor();
  assert.equal(await united.getAttribute('data-app-mode'), 'dark');
  await united.getByRole('button', { name: 'Log meal', exact: true }).click();
  await modal.locator('[name="value.restaurant"]').fill('Canvas draft');
  await page.evaluate(() => { window.canvasDraft = document.querySelector('.app-component-dialog [name="value.restaurant"]'); window.canvasKey = document.querySelector('.app-component-dialog [name="idempotency_key"]').value; });
  await page.keyboard.press('Tab'); await page.keyboard.press('Shift+Tab');
  assert.equal(await modal.evaluate(node => node.contains(document.activeElement)), true);
  await page.keyboard.press('Escape');
  await united.locator('[data-unite-split]').click();
  // Fan out restores each card's own position. Use the real navigator to
  // bring a parked card into view instead of scrolling the canvas DOM.
  await page.locator(`.navigator-item[data-instance-id="f-${dining.work_id}-add"]`).click();
  const formCard = page.locator(`.cv-card[data-work="${dining.work_id}"]`).filter({ has: page.locator('[name="value.restaurant"]') });
  await formCard.waitFor();
  assert.equal(await formCard.locator('[name="value.restaurant"]').inputValue(), 'Canvas draft');
  await formCard.locator('[data-focus-card]').click();
  assert.equal(await page.locator('[data-focus-stage] [name="value.restaurant"]').inputValue(), 'Canvas draft');
  await page.locator('[data-focus-close]').click();
  await page.locator(`.navigator-item[data-instance-id="a-${dining.work_id}"]`).click();
  await page.locator(`.cv-card[data-kind="app"][data-work="${dining.work_id}"] [data-unite-app]`).click();
  await united.getByRole('button', { name: 'Log meal', exact: true }).click();
  assert.equal(await page.evaluate(() => window.canvasDraft === document.querySelector('.app-component-dialog [name="value.restaurant"]') && window.canvasKey === document.querySelector('.app-component-dialog [name="idempotency_key"]').value), true);
  await shot(page, 'canvas-modal.png');
  await page.keyboard.press('Escape');
  await shot(page, 'canvas-united.png');
  report.checks.push('The same app/theme opens on canvas; modal draft and key survive Fan out, Focus, return and Unite without submitting.');
  if (!fixture.equipment) { await page.setViewportSize({ width: 1440, height: 1100 }); fixture.equipment = await created('equipment-requests'); await saveFixture(); }
  await page.goto(origin + workPath(fixture.equipment));
  await page.getByRole('button', { name: 'Request equipment', exact: true }).click();
  await page.locator('.app-component-dialog [name="value.item"]').fill('Independent inventory draft');
  await page.keyboard.press('Escape');
  report.checks.push('An unrelated Equipment requests app uses the same action dispatcher and its own exact form component.');
  assert.deepEqual(report.errors, []);
  assert.equal(await manifest(), manifestBefore, 'Build changed during browser qualification; repeat on a stable build');
  report.completed = new Date().toISOString();
  await writeFile(resolve(output, 'member.json'), JSON.stringify(report, null, 2));
  console.log(JSON.stringify(report));
} catch (error) {
  report.failure = String(error);
  await shot(page, 'failure.png').catch(() => {});
  await writeFile(resolve(output, 'member-failure.json'), JSON.stringify(report, null, 2));
  console.log(JSON.stringify(report));
  throw error;
} finally { await browser.close(); }
