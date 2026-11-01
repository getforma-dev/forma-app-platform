// Continue after the previous session's successful command/live/offline/reconnect
// checks. No second command is submitted here; revoke is a separate open session.
import {chromium} from '/tmp/audit-browser/node_modules/playwright/index.mjs';
import assert from 'node:assert/strict';
import {readFile,writeFile} from 'node:fs/promises';
const out='.cache/reports/app144-recipient-live';
const report=JSON.parse(await readFile(out+'/command-live.json','utf8'));
assert.equal(report.checks.length,3);assert.equal(report.revoked_during_run,false);
const target=JSON.parse(await readFile('.cache/reports/rename-share/report.json','utf8')).target;
const origin='http://127.0.0.1:7601',guestOrigin='http://127.0.0.1:7604',path=`/w/${target.app_id}/${target.work_id}`,sharePath=`/s/${target.share_id}`,sharesPath=`/v1/work/${target.app_id}/${target.work_id}/shares`;
report.harness_notes=['An initial preparatory attempt used the bare record UUID instead of the renderer rec-UUID row key, then was corrected.','The retained command-live session passed actual commit/live/receipt/offline/reconnect checks but stopped before revocation on an over-scoped asynchronous share-row selector. Revocation and widget checks continued in fresh already-open browser pages; no additional record command was submitted by the continuation.'];
report.sessions={command_live:{started_at:report.started_at,finished_at:report.finished_at,checks_passed:3},revoke_widget:{started_at:new Date().toISOString()}};delete report.failure;
const browser=await chromium.launch({channel:'chrome',headless:true}),owner=await browser.newContext({viewport:{width:1440,height:1000}}),guest=await browser.newContext({viewport:{width:1440,height:1000}});
const page=await owner.newPage(),recipient=await guest.newPage(),board=await guest.newPage();
for(const p of[page,recipient,board])p.on('pageerror',e=>report.errors.push(e.message));
const marker=report.record.cells.find(c=>c.id==='title').text;
const until=async(fn,ms=5000)=>{const end=Date.now()+ms;while(Date.now()<end){if(await fn())return;await new Promise(r=>setTimeout(r,50));}throw Error('Condition timed out');};
const api=async(p,pth)=>p.evaluate(async pth=>{const r=await fetch(pth,{headers:{accept:'application/json'},cache:'no-store'});return{status:r.status,body:await r.json().catch(()=>null)};},pth);
try{
 await page.goto(origin+path);assert.equal((await api(page,sharesPath)).body.shares.find(s=>s.share_id===target.share_id).status,'active');
 await recipient.goto(guestOrigin+sharePath);await recipient.locator('[data-document-records]').filter({hasText:marker}).waitFor();
 await board.goto(guestOrigin+'/shared');const connected=board.locator(`[data-connected-app="${target.share_id}"]`);await connected.locator('[data-document-records]').filter({hasText:marker}).waitFor();
 let navs=0;for(const p of[recipient,board])p.on('framenavigated',f=>{if(f===p.mainFrame())navs++;});
 await page.locator('[data-manage-share]').click();const dialog=page.getByRole('dialog',{name:'Share this app',exact:true});
 const person=dialog.locator('.share-person').filter({hasText:'guest@example.test'}).filter({has:page.getByRole('button',{name:'Remove access',exact:true})});await person.waitFor();assert.equal(await person.count(),1);
 const revokePromise=page.waitForResponse(r=>new URL(r.url()).pathname===sharesPath+'/'+target.share_id&&r.request().method()==='DELETE');await person.getByRole('button',{name:'Remove access',exact:true}).click();const response=await revokePromise;assert.equal(response.status(),200);report.revoked_during_run=true;const start=Date.now();
 const[docMs,boardMs]=await Promise.all([
  (async()=>{await until(async()=>await recipient.locator('[data-shared-parts]').count()===0);return Date.now()-start;})(),
  (async()=>{await until(async()=>await connected.locator('[data-shared-parts]').count()===0);return Date.now()-start;})()
 ]);
 assert.equal(await recipient.locator('h1').textContent(),'Private app');assert.equal(await recipient.getByText(marker,{exact:true}).count(),0);assert.equal(await board.getByText(marker,{exact:true}).count(),0);
 assert.equal((await api(recipient,sharePath)).status,404);assert.equal((await api(recipient,`/v1/shared/${target.share_id}/authority`)).status,404);const inventory=await api(board,'/v1/shared');assert.equal(inventory.status,200);assert.ok(!inventory.body.apps.some(a=>a.id===target.share_id));
 report.timings.revoked_document_mask_ms=docMs;report.timings.revoked_connected_mask_ms=boardMs;
 await guest.setOffline(true);await guest.setOffline(false);
 for(let i=0;i<12;i++){await new Promise(r=>setTimeout(r,500));assert.equal(await recipient.locator('[data-shared-parts]').count(),0);assert.equal(await connected.locator('[data-shared-parts]').count(),0);assert.equal(await recipient.getByText(marker,{exact:true}).count(),0);}
 assert.equal(navs,0);report.checks.push('Owner revoked the same share through real UI while guest document and connected app were already open. Both masked within5s; data/authority returned404 and inventory omitted it. A subsequent offline/reconnect cycle did not revive content over6s. No reload/navigation.');
 await recipient.screenshot({path:out+'/recipient-revoked.png',fullPage:true});await board.screenshot({path:out+'/connected-revoked.png',fullPage:true});
 await page.goto(origin+'/canvas');await page.locator('#intent-input').fill('/clock');await page.locator('#intent-input').press('Enter');const widget=page.locator('.cv-widget[data-widget="clock"]').last();await widget.waitFor();const id=await widget.getAttribute('data-widget-id');assert.ok(id);assert.equal(await widget.locator('a[href^="/w/"],[data-manage-share],[data-unite]').count(),0);
 const r=await owner.request.get(`${origin}/w/${target.app_id}/${encodeURIComponent(id)}`);assert.equal(r.status(),404);assert.equal(new URL(page.url()).pathname,'/canvas');report.widget={kind:'clock',id,work_route_status:r.status(),member_page_links:0};report.checks.push('Real clock widget remains on the canvas, exposes no work-page/share link, and its identifier has no fake /w route (404).');
 report.draft_reference='evidence/APP-144/completion-2026-11-01/browser.json: independently retained real member draft preservation and same-key retry after actual commit; guest projection is view only and has no editable draft form.';
 assert.deepEqual(report.errors,[]);report.passed=true;
}catch(e){report.passed=false;report.failure=String(e);process.exitCode=1;}
finally{report.finished_at=new Date().toISOString();report.sessions.revoke_widget.finished_at=report.finished_at;await writeFile(out+'/report.json',JSON.stringify(report,null,2));console.log(JSON.stringify(report));await browser.close();}
