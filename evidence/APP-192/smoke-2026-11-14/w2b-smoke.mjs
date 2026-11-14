// W2b smoke on the local dev stand (synthetic personas): the share dialog's
// organization section. Diego creates a work from a first-party template (no
// model call), opens "Share this app" from its work page, gives Sarah "Can review all entries",
// shares with everyone as "Can view" (Priya then opens the work page), and
// stops sharing with everyone (Priya's work page is the uniform 404 again).
// One-off: run once, read smoke.json and the screenshots.
import assert from 'node:assert/strict';
import {writeFile} from 'node:fs/promises';
const {chromium} = await import(process.env.PLAYWRIGHT_MODULE || 'playwright');
const out = process.argv[2];
const owner = process.env.OWNER_ORIGIN;
const other = process.env.OTHER_ORIGIN;
assert.ok(out && owner && other, 'usage: OWNER_ORIGIN=… OTHER_ORIGIN=… node w2b-smoke.mjs <output dir>');
const report = {scope: 'W2b local dev stand smoke, synthetic personas; not hosted acceptance', checks: [], errors: []};
const browser = await chromium.launch({channel: 'chrome', headless: true});
try {
  const context = await browser.newContext({viewport: {width: 1366, height: 1000}});
  const page = await context.newPage();
  page.on('pageerror', e => report.errors.push(e.message));
  await page.goto(owner + '/canvas');
  const composed = await page.evaluate(async () => {
    const {templates} = await (await fetch('/v1/templates', {headers: {accept: 'application/json'}})).json();
    const template = templates.find(t => t.id === 'equipment-requests');
    const response = await fetch('/v1/compose', {method: 'POST', headers: {'content-type': 'application/json', accept: 'application/json'},
      body: JSON.stringify({intent: template.description, template_id: template.id})});
    return {status: response.status, body: await response.json()};
  });
  assert.equal(composed.status, 200, JSON.stringify(composed.body));
  const {app_id: app, work_id: work} = composed.body;
  report.checks.push('a work of Diego\'s own from the equipment-requests template (no model call)');

  // 1. The dialog opens with the organization section and the three roles.
  // The work page carries the same "Share app" button and dialog module as
  // the canvas (a template work is listed in "My apps", not yet placed).
  await page.goto(`${owner}/w/${app}/${work}`);
  await page.locator(`[data-manage-share][data-work="${work}"]`).first().click();
  const dialog = page.getByRole('dialog', {name: 'Share this app', exact: true});
  const team = dialog.locator('.share-team');
  await team.getByRole('heading', {name: 'People in your organization'}).waitFor();
  // A select inside its label: its accessible name includes the chosen
  // option, so the script names the selects by their form names.
  const access = team.locator('select[name=role]');
  const person = team.locator('select[name=grantee]');
  await access.locator('option').first().waitFor({state: 'attached'});
  await person.locator('option', {hasText: 'Sarah Chen'}).waitFor({state: 'attached'});
  assert.deepEqual(await access.locator('option').allTextContents(), ['Can view', 'Can add and edit their entries', 'Can review all entries']);
  const people = await person.locator('option').allTextContents();
  assert.ok(people.includes('Sarah Chen'), people.join(', '));
  assert.ok(!people.some(label => /[0-9a-f]{8}-[0-9a-f]{4}/.test(label)), 'no principal id shown');
  // Nothing is shared with everyone yet: no stop control (the first run's
  // dialog-before-fix.png showed it; a class rule outranked `hidden`).
  assert.equal(await team.getByRole('button', {name: 'Stop sharing with everyone', exact: true}).isVisible(), false);
  await dialog.screenshot({path: `${out}/dialog.png`});
  report.checks.push('the dialog shows "People in your organization", the three roles in plain words, colleagues by name, and no stop control while nothing is shared with everyone');

  // 2. A person share as reviewer.
  await person.selectOption({label: 'Sarah Chen'});
  await access.selectOption({label: 'Can review all entries'});
  await team.getByRole('button', {name: 'Share', exact: true}).click();
  await team.getByRole('status').filter({hasText: 'Shared with Sarah Chen: Can review all entries.'}).waitFor();
  const row = team.locator('.share-member').filter({hasText: 'Sarah Chen'});
  await row.getByText('Can review all entries').waitFor();
  await dialog.screenshot({path: `${out}/shared-with-a-person.png`});
  report.checks.push('sharing with Sarah Chen as "Can review all entries" lists her by name with that access');

  // 3. Everyone in the organization, as "Can view"; Priya opens the work page.
  const otherPage = await (await browser.newContext({viewport: {width: 1366, height: 900}})).newPage();
  otherPage.on('pageerror', e => report.errors.push(e.message));
  const before = await otherPage.goto(`${other}/w/${app}/${work}`);
  assert.equal(before.status(), 404, 'Priya cannot open the work before the share');
  await team.locator('select[name=everyone_role]').selectOption({label: 'Can view'});
  await team.getByRole('button', {name: 'Share with everyone', exact: true}).click();
  await team.getByText('Shared with everyone: Can view. This includes people who join later.').waitFor();
  await team.getByRole('button', {name: 'Stop sharing with everyone', exact: true}).waitFor();
  await dialog.screenshot({path: `${out}/shared-with-everyone.png`});
  const during = await otherPage.goto(`${other}/w/${app}/${work}`);
  assert.equal(during.status(), 200, 'Priya opens the work page while it is shared with everyone');
  report.checks.push('sharing with everyone as "Can view" shows the role and a stop control; Priya, with no share of her own, opens the work page (404 before)');

  // 4. Stop sharing with everyone; Priya is refused again, Sarah keeps hers.
  await team.getByRole('button', {name: 'Stop sharing with everyone', exact: true}).click();
  await team.getByText('Not shared with everyone. Sharing with everyone includes people who join later.').waitFor();
  await row.getByText('Can review all entries').waitFor();
  assert.equal(await team.getByRole('button', {name: 'Stop sharing with everyone', exact: true}).isVisible(), false);
  await dialog.screenshot({path: `${out}/stopped-sharing-with-everyone.png`});
  const after = await otherPage.goto(`${other}/w/${app}/${work}`);
  assert.equal(after.status(), 404, 'Priya cannot open the work after the stop');
  report.checks.push('stopping sharing with everyone: the stop control goes away, Priya\'s work page is the 404 again, Sarah Chen keeps "Can review all entries"');
} finally {
  await browser.close();
  await writeFile(`${out}/smoke.json`, JSON.stringify(report, null, 2) + '\n');
}
assert.deepEqual(report.errors, [], 'no page errors');
console.log(JSON.stringify(report, null, 2));
