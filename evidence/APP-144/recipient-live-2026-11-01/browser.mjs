import {chromium} from '/tmp/audit-browser/node_modules/playwright/index.mjs';
import assert from 'node:assert/strict';
import {readFile,writeFile} from 'node:fs/promises';
import {execFileSync} from 'node:child_process';
const target=JSON.parse(await readFile('.cache/reports/rename-share/report.json','utf8')).target;
const origin='http://127.0.0.1:7601',guestOrigin='http://127.0.0.1:7604',strangerOrigin='http://127.0.0.1:7603';
const out='.cache/reports/app144-recipient-live',path=`/w/${target.app_id}/${target.work_id}`,sharePath=`/s/${target.share_id}`,sharesPath=`/v1/work/${target.app_id}/${target.work_id}/shares`;
const browser=await chromium.launch({channel:'chrome',headless:true});
const owner=await browser.newContext({viewport:{width:1440,height:1000}}),guest=await browser.newContext({viewport:{width:1440,height:1000}}),stranger=await browser.newContext({viewport:{width:1440,height:1000}});
const page=await owner.newPage(),recipient=await guest.newPage(),board=await guest.newPage(),other=await stranger.newPage();
const report={source_head:execFileSync('git',['rev-parse','HEAD'],{encoding:'utf8'}).trim(),scope:'Owned local7601/7603/7604 synthetic member and account-only guest. Actual command, storage, share authority, browser reconnect and revoke; no hosted signup or synthetic provider claim.',started_at:new Date().toISOString(),checks:[],errors:[],timings:{}};
for(const p of[page,recipient,board,other])p.on('pageerror',e=>report.errors.push(e.message));
const until=async(fn,ms=15000)=>{const end=Date.now()+ms;while(Date.now()<end){if(await fn())return;await new Promise(r=>setTimeout(r,50));}throw Error('Condition timed out');};
const api=async(p,pth)=>p.evaluate(async pth=>{const r=await fetch(pth,{headers:{accept:'application/json'},cache:'no-store'});return{status:r.status,body:await r.json().catch(()=>null)};},pth);
const row=(props,id)=>props.record_cards.flatMap(c=>c.rows).find(r=>r.id===`rec-${id}`);
const recordProjection=r=>({id:r.id,version:r.version,committed_at:r.committed_at,commit_sequence:r.commit_sequence,cells:r.cells});
const partProjection=props=>props.parts.map(p=>({id:p.id,title:p.title,class:p.class,text:p.text,is_note:p.is_note,is_form:p.is_form,view_ids:p.views.map(v=>v.id)}));
let revoked=false;
try{
 await page.goto(origin+path);const before=(await api(page,path)).body;
 const shares=(await api(page,sharesPath)).body.shares,active=shares.find(s=>s.share_id===target.share_id);assert.equal(active.status,'active');assert.equal(active.allow_add_to_canvas,true);
 await recipient.goto(guestOrigin+sharePath);await recipient.locator('[data-shared-parts]').waitFor();
 await board.goto(guestOrigin+'/shared');const connected=board.locator(`[data-connected-app="${target.share_id}"]`);await connected.locator('[data-shared-parts]').waitFor();
 await other.goto(strangerOrigin+'/canvas');
 const unrelated=await api(other,path),absent=await api(other,`/w/${target.app_id}/00000000-0000-4000-8000-00000000ffff`);assert.equal(unrelated.status,404);assert.deepEqual(unrelated,absent);
 const unrelatedShare=await api(other,sharePath),absentShare=await api(other,'/s/00000000-0000-4000-8000-000000000001.00000000-0000-4000-8000-00000000ffff');assert.equal(unrelatedShare.status,404);assert.deepEqual(unrelatedShare,absentShare);
 const guestMemberRoute=await api(recipient,path);assert.equal(guestMemberRoute.status,404);assert.equal((await guest.request.get(guestOrigin+'/workspace')).status(),404);
 assert.equal((await owner.request.get(origin+path+'?unknown=1')).status(),404);
 report.refusals={stranger_work:unrelated.status,missing_work:absent.status,stranger_share_json:unrelatedShare.status,missing_share_json:absentShare.status,guest_member_work:guestMemberRoute.status,guest_workspace:404,query_string:404};
 report.checks.push('Unrelated member work and shared JSON reads match missing-resource uniform404; account-only guest gets404 for /w and /workspace; member document rejects query strings. Share HTML intentionally remains a neutral sign-in landing, not protected content.');
 const guestBefore=(await api(recipient,sharePath)).body;assert.equal(guestBefore.available,true);assert.deepEqual(partProjection(guestBefore),partProjection(before));
 let recipientNavigations=0,boardNavigations=0;recipient.on('framenavigated',f=>{if(f===recipient.mainFrame())recipientNavigations++;});board.on('framenavigated',f=>{if(f===board.mainFrame())boardNavigations++;});
 const marker='Live shared request '+Date.now();const form=page.locator('form[data-command]').filter({has:page.locator('[name="value.title"]')});assert.equal(await form.count(),1);
 await form.locator('[name="value.title"]').fill(marker);await form.locator('[name="value.author"]').fill('Synthetic author');
 const status=form.locator('[name="value.status"]');if(await status.count())await status.selectOption('to_read');
 const command=new URL(await form.getAttribute('action'),origin).pathname;
 const resultPromise=page.waitForResponse(r=>new URL(r.url()).pathname===command&&r.request().method()==='POST');const submitted=Date.now();await form.locator('button[type=submit]').click();
 const response=await resultPromise;assert.equal(response.status(),200);const outcome=await response.json();assert.equal(outcome.state,'committed');assert.ok(outcome.audit_event_id);const committed=Date.now();
 await writeFile(out+'/exact-command-receipt.json',JSON.stringify(outcome,null,2));
 const [recipientMs,boardMs]=await Promise.all([
  (async()=>{await recipient.locator('[data-document-records]').filter({hasText:marker}).waitFor({timeout:15000});return Date.now()-committed;})(),
  (async()=>{await connected.locator('[data-document-records]').filter({hasText:marker}).waitFor({timeout:15000});return Date.now()-committed;})()
 ]);
 const memberAfter=(await api(page,path)).body,guestAfter=(await api(recipient,sharePath)).body;
 const memberRow=row(memberAfter,outcome.record_id),guestRow=row(guestAfter,outcome.record_id);assert.ok(memberRow&&guestRow);assert.deepEqual(recordProjection(guestRow),recordProjection(memberRow));
 assert.equal(String(guestRow.version),String(outcome.record_version));assert.equal(String(guestRow.commit_sequence),String(outcome.commit_sequence));assert.equal(Date.parse(guestRow.committed_at),Date.parse(outcome.committed_at));
 assert.deepEqual(partProjection(guestAfter),partProjection(memberAfter));
 const ownerParts=await page.locator('[data-document-part]').evaluateAll(nodes=>nodes.map(n=>n.dataset.documentPart));
 assert.deepEqual(await recipient.locator('[data-document-part]').evaluateAll(nodes=>nodes.map(n=>n.dataset.documentPart)),ownerParts);assert.deepEqual(await connected.locator('[data-document-part]').evaluateAll(nodes=>nodes.map(n=>n.dataset.documentPart)),ownerParts);
 assert.equal(await recipient.locator('form[data-command],[data-ask-app],a[href^="/w/"]').count(),0);assert.equal(await connected.locator('form[data-command],[data-ask-app],a[href^="/w/"]').count(),0);
 report.timings.command_http_ms=committed-submitted;report.timings.recipient_record_after_http_ms=recipientMs;report.timings.connected_record_after_http_ms=boardMs;
 report.record={...recordProjection(guestRow),audit_event_id:outcome.audit_event_id};report.parts=partProjection(guestAfter);
 report.receipt_scope='This records-only app displays real row revision/committed_at/commit_sequence, compared exactly with the member row and audited command outcome. It has no governed query-result receipt; no placeholder receipt was added. Source-query receipt rendering remains covered by APP144 library/native evidence.';
 report.checks.push('A real owner native-form command committed once; existing open guest full document and connected app showed identical saved values and record receipt metadata without reload, preserving admitted part order/notes and view-only controls.');
 await recipient.screenshot({path:out+'/recipient-live.png',fullPage:true});await board.screenshot({path:out+'/connected-live.png',fullPage:true});
 const boundsBefore=await connected.boundingBox();const offlineStart=Date.now();await guest.setOffline(true);
 const [offlineDocumentMs,offlineBoardMs]=await Promise.all([
  (async()=>{await until(async()=>await recipient.locator('[data-shared-parts]').count()===0,5000);return Date.now()-offlineStart;})(),
  (async()=>{await until(async()=>await connected.locator('[data-shared-parts]').count()===0,5000);return Date.now()-offlineStart;})()
 ]);
 assert.equal(await recipient.locator('h1').textContent(),'Private app');assert.equal(await connected.locator('.connected-app-head h2').textContent(),'Private app');assert.equal(await recipient.getByText(marker,{exact:true}).count(),0);assert.equal(await connected.getByText(marker,{exact:true}).count(),0);
 await recipient.screenshot({path:out+'/recipient-offline.png',fullPage:true});await board.screenshot({path:out+'/connected-offline.png',fullPage:true});
 let authoritySuccess=0;const seen=r=>{if(new URL(r.url()).pathname===`/v1/shared/${target.share_id}/authority`&&r.status()===200)authoritySuccess++;};recipient.on('response',seen);board.on('response',seen);
 const reconnectStart=Date.now();await guest.setOffline(false);
 await Promise.all([recipient.locator('[data-document-records]').filter({hasText:marker}).waitFor({timeout:15000}),connected.locator('[data-document-records]').filter({hasText:marker}).waitFor({timeout:15000})]);assert.ok(authoritySuccess>=2);
 const recovered=(await api(recipient,sharePath)).body;assert.deepEqual(recordProjection(row(recovered,outcome.record_id)),recordProjection(guestRow));
 const boundsAfter=await connected.boundingBox();for(const k of['x','y','width'])assert.equal(boundsAfter[k],boundsBefore[k]);
 report.timings.offline_document_mask_ms=offlineDocumentMs;report.timings.offline_connected_mask_ms=offlineBoardMs;report.timings.reconnect_both_visible_ms=Date.now()-reconnectStart;report.reconnect_authority_successes=authoritySuccess;
 report.checks.push('Browser network emulation offline removed protected values/title from both surfaces within5s. Reconnect performed fresh successful authority reads before restoring identical row/receipt content and retained connected placement.');
 await page.locator('[data-manage-share]').click();const dialog=page.getByRole('dialog',{name:'Share this app',exact:true});const person=dialog.locator('.share-person').filter({hasText:'guest@example.test'}).filter({has:page.getByRole('button',{name:'Remove access',exact:true})});await person.waitFor();assert.equal(await person.count(),1);
 const revokePromise=page.waitForResponse(r=>new URL(r.url()).pathname===sharesPath+'/'+target.share_id&&r.request().method()==='DELETE');await person.getByRole('button',{name:'Remove access',exact:true}).click();const revokedResponse=await revokePromise;assert.equal(revokedResponse.status(),200);revoked=true;const revokedAt=Date.now();
 const [revokeDocumentMs,revokeBoardMs]=await Promise.all([
  (async()=>{await until(async()=>await recipient.locator('[data-shared-parts]').count()===0,5000);return Date.now()-revokedAt;})(),
  (async()=>{await until(async()=>await connected.locator('[data-shared-parts]').count()===0,5000);return Date.now()-revokedAt;})()
 ]);
 assert.equal(await recipient.locator('h1').textContent(),'Private app');assert.equal(await recipient.getByText(marker,{exact:true}).count(),0);assert.equal(await board.getByText(marker,{exact:true}).count(),0);assert.equal((await api(recipient,sharePath)).status,404);assert.equal((await api(recipient,`/v1/shared/${target.share_id}/authority`)).status,404);
 const inventory=await api(board,'/v1/shared');assert.equal(inventory.status,200);assert.ok(!inventory.body.apps.some(a=>a.id===target.share_id));
 report.timings.revoked_document_mask_ms=revokeDocumentMs;report.timings.revoked_connected_mask_ms=revokeBoardMs;
 assert.equal(recipientNavigations,0);assert.equal(boardNavigations,0);report.checks.push('Owner removed access through real sharing UI; both open surfaces masked protected values within5s, authority/data endpoints returned404, and authorized connected inventory omitted the revoked app. No navigation or reload occurred.');
 await recipient.screenshot({path:out+'/recipient-revoked.png',fullPage:true});await board.screenshot({path:out+'/connected-revoked.png',fullPage:true});
 await page.goto(origin+'/canvas');await page.locator('#intent-input').fill('/clock');await page.locator('#intent-input').press('Enter');const widget=page.locator('.cv-widget[data-widget="clock"]').last();await widget.waitFor();const widgetId=await widget.getAttribute('data-widget-id');assert.ok(widgetId);assert.equal(await widget.locator('a[href^="/w/"],[data-manage-share],[data-unite]').count(),0);
 const widgetRoute=await owner.request.get(`${origin}/w/${target.app_id}/${encodeURIComponent(widgetId)}`);assert.equal(widgetRoute.status(),404);assert.equal(new URL(page.url()).pathname,'/canvas');
 report.widget={kind:'clock',id:widgetId,work_route_status:widgetRoute.status(),member_page_links:0};report.checks.push('Clock remains a canvas widget, has no member document/share controls, and its identifier does not resolve to a fake /w page.');
 report.draft_reference='evidence/APP-144/completion-2026-11-01/browser.json: real member draft preservation and post-commit same-key uncertain retry; guest projection is view only and has no draft form.';
 assert.deepEqual(report.errors,[]);report.passed=true;
}catch(error){report.passed=false;report.failure=String(error);process.exitCode=1;}
finally{report.revoked_during_run=revoked;report.finished_at=new Date().toISOString();await writeFile(out+'/report.json',JSON.stringify(report,null,2));console.log(JSON.stringify(report));await browser.close();}
