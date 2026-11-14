// W2 smoke on the local dev stand (synthetic personas): the invitation page's
// notice and consent box, an organization admin's /admin, the name prompt.
import assert from 'node:assert/strict';
import {writeFile} from 'node:fs/promises';
const {chromium} = await import(process.env.PLAYWRIGHT_MODULE || 'playwright');
const out = process.argv[2];
const admin = process.env.ADMIN_ORIGIN || 'http://127.0.0.1:8702';
const member = process.env.MEMBER_ORIGIN || 'http://127.0.0.1:8703';
const report = {scope: 'W2 local dev stand smoke, synthetic personas; not hosted acceptance', checks: [], errors: []};
const browser = await chromium.launch({channel: 'chrome', headless: true});
try {
  const context = await browser.newContext({viewport: {width: 1366, height: 900}});
  const page = await context.newPage();
  page.on('pageerror', e => report.errors.push(e.message));
  // 1. The invitation page: the notice and the required consent box.
  await page.goto(member + '/invite');
  const consent = page.locator('input[type=checkbox][name=consent]');
  assert.equal(await consent.count(), 1);
  assert.equal(await consent.getAttribute('required'), '');
  const notice = await page.locator('main').innerText();
  assert.ok(notice.length > 200, 'the notice text is on the page');
  await page.screenshot({path: `${out}/invite.png`, fullPage: true});
  report.checks.push('invite: the notice and one required consent checkbox are shown');
  // 2. The member with no name: the prompt, no Admin link.
  await page.goto(member + '/canvas');
  await page.getByText('What should others call you?').waitFor();
  assert.equal(await page.locator('a[href="/admin"]').count(), 0);
  await page.screenshot({path: `${out}/name-prompt.png`});
  await page.fill('#cv-name-input', 'Priya N.');
  await Promise.all([page.waitForURL(member + '/canvas'), page.click('.cv-name-prompt button[type=submit]')]);
  assert.equal(await page.getByText('What should others call you?').count(), 0);
  report.checks.push('name: the prompt shows for a member with no name, saving "Priya N." returns to the canvas without the prompt');
  // 3. The organization admin: the Admin link and the members table.
  const adminPage = await (await browser.newContext({viewport: {width: 1366, height: 900}})).newPage();
  adminPage.on('pageerror', e => report.errors.push(e.message));
  await adminPage.goto(admin + '/canvas');
  await adminPage.locator('a[href="/admin"]').first().click();
  await adminPage.waitForURL(admin + '/admin');
  const rows = await adminPage.locator('table.org-admin-table tbody tr').count();
  assert.ok(rows >= 3, `members listed: ${rows}`);
  assert.ok((await adminPage.locator('table').innerText()).includes('Priya N.'));
  assert.ok(await adminPage.locator('a[href="mailto:feedback@getforma.dev"]').count() >= 1);
  await adminPage.screenshot({path: `${out}/admin.png`, fullPage: true});
  report.checks.push(`admin: the Admin link opens /admin with ${rows} members, the new name "Priya N." among them, and the feedback link`);
} finally {
  await browser.close();
}
assert.deepEqual(report.errors, []);
await writeFile(`${out}/smoke.json`, JSON.stringify(report, null, 2));
console.log(JSON.stringify(report, null, 2));
