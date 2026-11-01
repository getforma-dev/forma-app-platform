import {chromium} from '/tmp/audit-browser/node_modules/playwright/index.mjs';
import assert from 'node:assert/strict';
import {readFile,writeFile} from 'node:fs/promises';
const target=JSON.parse(await readFile('.cache/reports/shared-ui-fixture.json','utf8'));
const sibling=JSON.parse(await readFile('.cache/reports/app143/browser.json','utf8')).work;
assert.equal(target.app_id,sibling.app);assert.notEqual(target.work_id,sibling.work);
const ownerOrigin='http://127.0.0.1:7601',guestOrigin='http://127.0.0.1:7604',out='.cache/reports/rename-share';
const browser=await chromium.launch({channel:'chrome',headless:true});
const owner=await browser.newContext({viewport:{width:1440,height:1000}}),guest=await browser.newContext({viewport:{width:1440,height:1000}});
const page=await owner.newPage(),recipient=await guest.newPage(),connected=await guest.newPage();
const report={scope:'Local synthetic member and account-only external guest on owned7601/7604; existing accepted share, real HTTP/storage/UI rename and reauthorized live reads; no hosted identity assertion.',started_at:new Date().toISOString(),checks:[],errors:[]};
for(const p of[page,recipient,connected])p.on('pageerror',error=>report.errors.push(error.message));
const api=async(p,path)=>p.evaluate(async path=>{const r=await fetch(path,{headers:{accept:'application/json'},cache:'no-store'});return{status:r.status,body:await r.json().catch(()=>null)};},path);
const pin=p=>p.forms.map(f=>({id:f.id,publication:f.publication,work_version:f.work_version}));
const until=async fn=>{const end=Date.now()+15000;while(Date.now()<end){if(await fn())return;await new Promise(r=>setTimeout(r,75));}throw Error('Expected existing live title did not appear');};
try{
 const path=`/w/${target.app_id}/${target.work_id}`,siblingPath=`/w/${sibling.app}/${sibling.work}`,sharesPath=`/v1/work/${target.app_id}/${target.work_id}/shares`,labelPath=`/v1/work/${target.app_id}/${target.work_id}/label`;
 await page.goto(ownerOrigin+path);
 const shares=await api(page,sharesPath);assert.equal(shares.status,200);
 let share=shares.body.shares.find(s=>s.status==='active'&&s.allow_add_to_canvas&&s.recipient_email==='guest@example.test');
 if(!share){
  await page.locator('[data-manage-share]').click();const dialog=page.getByRole('dialog',{name:'Share this app',exact:true});
  await dialog.getByLabel('Recipient email',{exact:true}).fill('guest@example.test');await dialog.getByLabel('Allow adding this app to their canvas',{exact:true}).check();
  const created=page.waitForResponse(r=>new URL(r.url()).pathname===sharesPath&&r.request().method()==='POST');await dialog.getByRole('button',{name:'Share app',exact:true}).click();
  const response=await created;assert.equal(response.status(),200);share=(await response.json()).share;assert.equal(share.status,'pending');
  await recipient.goto(guestOrigin+share.href);await recipient.getByRole('button',{name:'Accept app invitation',exact:true}).click();await recipient.getByRole('button',{name:'Add to my canvas',exact:true}).waitFor();
  report.setup_grant='A real named guest share was created and explicitly accepted during setup before opening baseline pages; prior guest grants had been revoked by previous tests.';
 }
 else report.setup_grant='Previously active named guest share reused';
 const before=(await api(page,path)).body,siblingBefore=(await api(page,siblingPath)).body,labelBefore=(await api(page,labelPath)).body;
 assert.ok(before.forms.length);assert.ok(siblingBefore.forms.length);
 await recipient.goto(guestOrigin+share.href);await recipient.getByRole('heading',{name:before.title,exact:true}).waitFor();
 const guestBefore=await api(recipient,share.href);assert.equal(guestBefore.status,200);assert.equal(guestBefore.body.available,true);assert.equal(guestBefore.body.share_id,share.share_id);
 assert.equal((await guest.request.get(guestOrigin+'/workspace')).status(),404);
 if(!guestBefore.body.added_to_canvas){await recipient.getByRole('button',{name:'Add to my canvas',exact:true}).click();await recipient.getByRole('link',{name:'Open my connected apps',exact:true}).waitFor();report.setup='Existing active share explicitly added to connected canvas before baseline';}
 else report.setup='Existing active share and connected placement reused';
 await connected.goto(guestOrigin+'/shared');
 const card=connected.locator(`[data-connected-app="${share.share_id}"]`);await card.getByRole('heading',{name:before.title,exact:true}).waitFor();
 const rectBefore=await card.boundingBox();assert.ok(rectBefore);
 const shareBefore=(await api(page,sharesPath)).body.shares.find(s=>s.share_id===share.share_id);
 let recipientNavigations=0,connectedNavigations=0;
 recipient.on('framenavigated',f=>{if(f===recipient.mainFrame())recipientNavigations++;});connected.on('framenavigated',f=>{if(f===connected.mainFrame())connectedNavigations++;});
 await recipient.screenshot({path:out+'/recipient-before.png',fullPage:true});await connected.screenshot({path:out+'/connected-before.png',fullPage:true});
 await page.goto(ownerOrigin+'/canvas');await page.locator('#cv-library-toggle').click();
 await page.getByRole('tab',{name:'Library',exact:true}).click();
 await page.locator(`.cv-lib-app[data-work="${target.work_id}"] [data-lib-edit=""]`).click();
 const name=page.getByLabel('App name',{exact:true});await until(()=>name.isEnabled());
 const renamed='Shared reading review '+new Date().toISOString().slice(11,19);
 await name.fill(renamed);const responsePromise=page.waitForResponse(r=>new URL(r.url()).pathname===labelPath&&r.request().method()==='POST');
 const started=Date.now();await page.locator('.cv-naming button[type="submit"]').click();
 const response=await responsePromise;assert.equal(response.status(),200);const outcome=await response.json();assert.equal(outcome.saved,true);assert.equal(outcome.label.version,labelBefore.version+1);
 const renameCommittedAt=Date.now();
 const [documentMs,connectedMs]=await Promise.all([
  (async()=>{await until(async()=>await recipient.locator('h1').textContent()===renamed);return Date.now()-renameCommittedAt;})(),
  (async()=>{await until(async()=>await card.locator('.connected-app-head h2').textContent()===renamed);return Date.now()-renameCommittedAt;})()
 ]);
 const after=(await api(page,path)).body,siblingAfter=(await api(page,siblingPath)).body,shareAfter=(await api(page,sharesPath)).body.shares.find(s=>s.share_id===share.share_id);
 const guestAfter=await api(recipient,share.href);
 assert.equal(after.title,renamed);assert.equal(guestAfter.body.title,renamed);assert.equal(guestAfter.body.available,true);assert.equal(guestAfter.body.share_id,share.share_id);
 assert.deepEqual(pin(after),pin(before));assert.equal(siblingAfter.title,siblingBefore.title);assert.deepEqual(pin(siblingAfter),pin(siblingBefore));assert.deepEqual(shareAfter,shareBefore);
 assert.equal(recipientNavigations,0);assert.equal(connectedNavigations,0);assert.equal(await recipient.locator('form[data-command],[data-ask-app],a[href^="/w/"]').count(),0);
 const rectAfter=await card.boundingBox();assert.equal(rectAfter.x,rectBefore.x);assert.equal(rectAfter.y,rectBefore.y);assert.equal(rectAfter.width,rectBefore.width);
 report.target={app_id:target.app_id,work_id:target.work_id,old_title:before.title,new_title:renamed,share_id:share.share_id,share_version:shareBefore.version,pin:pin(after)};
 report.sibling={app_id:sibling.app,work_id:sibling.work,title:siblingAfter.title,pin:pin(siblingAfter)};
 report.timing={rename_http_ms:renameCommittedAt-started,recipient_title_observed_after_http_ms:documentMs,connected_title_observed_after_http_ms:connectedMs,assertion_deadline_ms:15000};
 report.checks.push('An already-active named guest share and connected canvas were open before the real UI rename.','Recipient document and connected app heading updated without navigation or reload through their normal reauthorized refresh.','Share ID/status/version/expiry/add permission and work publication/version were unchanged; same-authoring-app sibling title and pin were unchanged.','Account-only guest remains denied workspace and receives no command, Ask or member-work controls; connected placement kept x/y/width.');
 await recipient.screenshot({path:out+'/recipient-after.png',fullPage:true});await connected.screenshot({path:out+'/connected-after.png',fullPage:true});
 assert.deepEqual(report.errors,[]);report.passed=true;
}catch(error){report.passed=false;report.failure=String(error);process.exitCode=1;}
finally{report.finished_at=new Date().toISOString();await writeFile(out+'/report.json',JSON.stringify(report,null,2));console.log(JSON.stringify(report));await browser.close();}
