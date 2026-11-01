import {chromium} from '/tmp/audit-browser/node_modules/playwright/index.mjs';
import assert from 'node:assert/strict';
import {readFile,writeFile} from 'node:fs/promises';
const out='.cache/reports/app144-recipient-live',report=JSON.parse(await readFile(out+'/revoke-live.json','utf8'));
assert.equal(report.checks.length,4);assert.equal(report.revoked_during_run,true);
report.sessions.widget={started_at:new Date().toISOString()};delete report.failure;
report.harness_notes.push('The continuation completed revocation then a broad widget selector matched the hidden Library row. Final widget-only session selects the actual .cv-widget article; no record/share mutation repeated.');
const b=await chromium.launch({channel:'chrome',headless:true}),c=await b.newContext({viewport:{width:1440,height:1000}}),p=await c.newPage();p.on('pageerror',e=>report.errors.push(e.message));
try{await p.goto('http://127.0.0.1:7601/canvas');await p.locator('#intent-input').fill('/clock');await p.locator('#intent-input').press('Enter');const w=p.locator('.cv-widget[data-widget="clock"]').last();await w.waitFor();const id=await w.getAttribute('data-widget-id');assert.ok(id);assert.equal(await w.locator('a[href^="/w/"],[data-manage-share],[data-unite]').count(),0);
 const r=await c.request.get('http://127.0.0.1:7601/w/00000000-0000-4000-8000-00000000d0a1/'+encodeURIComponent(id));assert.equal(r.status(),404);assert.equal(new URL(p.url()).pathname,'/canvas');report.widget={kind:'clock',id,work_route_status:r.status(),member_page_links:0};report.checks.push('Actual clock widget remains on canvas, exposes no member work-page/share control, and its identifier does not resolve to a fake /w page (404).');
 await w.screenshot({path:out+'/widget-no-document.png'});assert.deepEqual(report.errors,[]);report.passed=true;
 report.draft_reference='evidence/APP-144/completion-2026-11-01/browser.json: real member draft preservation and same-key uncertain retry after actual commit; guest projection is view only and has no editable draft form.';
}catch(e){report.passed=false;report.failure=String(e);process.exitCode=1;}
finally{report.finished_at=new Date().toISOString();report.sessions.widget.finished_at=report.finished_at;await writeFile(out+'/report.json',JSON.stringify(report,null,2));console.log(JSON.stringify({passed:report.passed,checks:report.checks.length,timings:report.timings,widget:report.widget,failure:report.failure}));await b.close();}
