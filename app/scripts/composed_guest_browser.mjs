// APP-160 cross-surface acceptance on an owned loopback dev stand. The guest
// identity is synthetic; creation/publication/commands/share grants/queries are
// the ordinary product paths. No provider signup or hosted acceptance claim.
import assert from 'node:assert/strict';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { createHash } from 'node:crypto';
const args=Object.fromEntries(process.argv.slice(2).map(arg=>arg.replace(/^--/,'').split('=')));
const origin=args.owner||'http://127.0.0.1:4801',guestOrigin=args.guest||'http://127.0.0.1:4804';
for(const value of [origin,guestOrigin])assert.match(value,/^http:\/\/127\.0\.0\.1:(?!400[1-9]$|4010$)\d+$/,'Owned disposable loopback stand required');
const output=resolve(args.output||'.cache/reports/composed-guest-browser');await mkdir(output,{recursive:true});
const fixturePath=resolve(output,'fixture.json'),fixture=JSON.parse(await readFile(fixturePath,'utf8').catch(()=>'{}'));
assert.ok(!fixture.pending,`Previous mutation is unresolved: ${fixture.pending}. Reconcile it before rerunning.`);
assert.ok(!fixture.command, 'This fixture already contains the observed command. Use a fresh output directory to qualify a new live update; a reload is not live-update evidence.');
const save=()=>writeFile(fixturePath,JSON.stringify(fixture,null,2));
const mutation=async(name,action)=>{fixture.pending=name;await save();const result=await action();delete fixture.pending;await save();return result;};
const {chromium}=await import(process.env.PLAYWRIGHT_MODULE||'/tmp/audit-browser/node_modules/playwright/index.mjs');
const browser=await chromium.launch({channel:'chrome',headless:true});
const owner=await browser.newContext({viewport:{width:1440,height:1100},reducedMotion:'reduce'}),guest=await browser.newContext({viewport:{width:1440,height:1100},reducedMotion:'reduce'});
const page=await owner.newPage(),recipient=await guest.newPage(),board=await guest.newPage();
const manifestBefore=createHash('sha256').update(await readFile('app/admin/dist/manifest.json')).digest('hex');
const report={manifest_sha256:manifestBefore,resumed_confirmed_fixture:!!fixture.app,scope:'Synthetic local owner and account-only recipient. Real admitted empty template, immutable presentation publication, audited commands, named exact-work sharing, authorized record projections and guest browser leases. No hosted/provider claim.',started_at:new Date().toISOString(),checks:[],timings:{},errors:[],artifacts:[]};
for(const tab of[page,recipient,board])tab.on('pageerror',e=>report.errors.push(e.message));
const until=async(fn,ms=20000)=>{const start=Date.now();while(Date.now()-start<ms){if(await fn())return;await new Promise(r=>setTimeout(r,80));}throw Error('Condition timed out');};
const api=async(tab,path,method='GET',body)=>tab.evaluate(async({path,method,body})=>{const r=await fetch(path,{method,credentials:'same-origin',cache:'no-store',headers:{accept:'application/json',...(body===undefined?{}:{'content-type':'application/json'})},...(body===undefined?{}:{body:JSON.stringify(body)})});return{status:r.status,body:await r.json().catch(()=>null)};},{path,method,body});
const shot=async(tab,name)=>{await tab.screenshot({path:resolve(output,name),fullPage:true});report.artifacts.push(name);};
const recordProps=props=>props.parts.flatMap(part=>part.record_components||[]);
const receiptSummary=p=>{const r=JSON.parse(p.receipt_json);return{id:p.id,state:p.state,population:p.population,digest:r.digest,snapshot_digest:r.snapshot_digest,result_digest:r.result_digest,partial:r.partial,total_scope:r.total_scope};};
const themeOf=locator=>locator.evaluate(root=>{const view=root.querySelector('[data-record-component="total"]'),summary=view?.querySelector('.receipt summary');return{root:getComputedStyle(root).backgroundColor,panel:view&&getComputedStyle(view).backgroundColor,text:view&&getComputedStyle(view).color,receipt:summary&&getComputedStyle(summary).color};});
const expectedTheme={root:'rgb(16, 23, 29)',panel:'rgb(25, 35, 44)',text:'rgb(241, 245, 249)',receipt:'rgb(134, 239, 172)'};
let shared,connected;
try{
 await page.goto(origin+'/canvas');await page.locator('[data-create-app][data-create-wired="true"]').waitFor();
 if(!fixture.app)await mutation('create dining template',async()=>{
  await page.locator('[data-create-app]').click();await page.locator('[data-template="dining-tracker"]').click();
  const answer=page.waitForResponse(r=>new URL(r.url()).pathname==='/v1/compose'&&r.request().method()==='POST');
  await page.getByRole('dialog').getByRole('button',{name:'Create',exact:true}).click();const r=await answer;assert.equal(r.status(),200);fixture.app=await r.json();assert.equal(fixture.app.origin,'adapted');await save();
 });
 const app=fixture.app,workPath=`/w/${app.app_id}/${app.work_id}`,base=`/v1/work/${app.app_id}/${app.work_id}`,sharesPath=base+'/shares';
 await page.goto(origin+workPath);await page.getByText('Up to date.',{exact:true}).waitFor();
 const initial=await api(page,workPath);assert.equal(initial.status,200);
 if(!fixture.published){assert.ok(recordProps(initial.body).length===5);assert.ok(recordProps(initial.body).every(p=>p.empty&&p.population==='0'&&p.has_receipt));
  await mutation('publish shared app presentation',async()=>{
   const state=await api(page,base+'/presentation');assert.equal(state.status,200);assert.equal(state.body.can_edit,true);
   const presentation=structuredClone(state.body.presentation);presentation.description='Private live dining journal';presentation.theme={mode:'dark',accent:'green',density:'comfortable',font:'mono'};
   presentation.actions.push({id:'about',label:'About this journal',behavior:'dialog',target:'guide'},{id:'inspect',label:'Inspect meals',behavior:'dialog',target:'ledger'},{id:'browse',label:'Browse meals',behavior:'navigate',target:'ledger'});
   const result=await api(page,base+'/presentation','POST',{expected_publication_id:state.body.publication_id,expected_definition_digest:state.body.definition_digest,expected_work_version:state.body.work_version,presentation});assert.equal(result.status,200,JSON.stringify(result.body));assert.notEqual(result.body.publication_id,state.body.publication_id);fixture.published=result.body.publication_id;await save();
  });
  report.checks.push('Ordinary template picker created an empty app with five actual empty query receipts; authorized versioned presentation publication persisted generic actions and dark/green/mono theme.');
 }
 await page.reload();await page.getByText('Up to date.',{exact:true}).waitFor();
 if(!fixture.share)await mutation('create exact named share',async()=>{
  await page.locator('[data-manage-share]').click();const dialog=page.getByRole('dialog',{name:'Share this app',exact:true});await dialog.getByLabel('Recipient email',{exact:true}).fill('guest@example.test');await dialog.getByLabel('Allow adding this app to their canvas',{exact:true}).check();
  const answer=page.waitForResponse(r=>new URL(r.url()).pathname===sharesPath&&r.request().method()==='POST');await dialog.locator('form').getByRole('button',{name:'Share app',exact:true}).click();const r=await answer;assert.equal(r.status(),200);fixture.share=(await r.json()).share;await save();await dialog.getByRole('button',{name:'Close sharing',exact:true}).click();
 });
 assert.ok(!fixture.revoked,'This fixture was already revoked; choose a new output directory for a new acceptance run.');
 shared=`/s/${fixture.share.share_id}`;await recipient.goto(guestOrigin+shared);
 if(!fixture.accepted)await mutation('recipient explicitly accepts app invitation',async()=>{
  const button=recipient.getByRole('button',{name:'Accept app invitation',exact:true});await button.waitFor();await recipient.waitForResponse(r=>new URL(r.url()).pathname===shared&&r.request().headers().accept==='application/json'&&r.status()===200);
  const answer=recipient.waitForResponse(r=>new URL(r.url()).pathname===`/v1/shared/${fixture.share.share_id}/accept`&&r.request().method()==='POST');await button.click();const r=await answer;assert.equal(r.status(),200);fixture.accepted=true;await save();
 });
 await recipient.getByText('Live data checked just now.',{exact:true}).waitFor();
 assert.equal(await recipient.locator('[data-app-mode="dark"]').count(),1);assert.equal(await recipient.locator('[data-app-font="mono"]').count(),1);
 report.theme=await recipient.locator('[data-app-action]').first().evaluate(node=>({background:getComputedStyle(node).backgroundColor,color:getComputedStyle(node).color}));assert.deepEqual(report.theme,{background:'rgb(134, 239, 172)',color:'rgb(16, 42, 49)'});
 assert.equal(await recipient.getByRole('button',{name:'Add meal',exact:true}).count(),0);assert.equal(await recipient.locator('form[data-command],[data-edit-presentation],[data-ask-app],a[href^="/w/"]').count(),0);
 const guestProps=(await api(recipient,shared)).body;assert.equal(guestProps.available,true);assert.equal(recordProps(guestProps).length,5);assert.ok(recordProps(guestProps).every(p=>p.has_receipt));
 const native=await guest.request.get(guestOrigin+shared);assert.equal(native.status(),200);const html=await native.text();assert.match(html,/data-record-component="total"/);assert.match(html,/data-app-mode="dark"/);assert.match(html,/record_components_receipt/);assert.doesNotMatch(html,/data-command=/);
 report.native={status:native.status(),cache_control:native.headers()['cache-control'],html_sha256:createHash('sha256').update(html).digest('hex'),native_record_components:true,native_receipts:true,member_commands:false};
 if(!fixture.linked)await mutation('recipient explicitly adds connected app',async()=>{
  const answer=recipient.waitForResponse(r=>new URL(r.url()).pathname===`/v1/shared/${fixture.share.share_id}/canvas`&&r.request().method()==='POST');await recipient.getByRole('button',{name:'Add to my canvas',exact:true}).click();const r=await answer;assert.equal(r.status(),200);fixture.linked=true;await save();
 });
 await recipient.getByText('Live data checked just now.',{exact:true}).waitFor();await board.goto(guestOrigin+'/shared');connected=board.locator(`[data-connected-app="${fixture.share.share_id}"]`);await connected.locator('[data-shared-parts]').waitFor();
 report.computed_themes={member:await themeOf(page.locator('[data-document]')),shared:await themeOf(recipient.locator('[data-shared-page]')),connected:await themeOf(connected)};
 for(const value of Object.values(report.computed_themes))assert.deepEqual(value,expectedTheme);
 report.native_computed_themes={};
 for(const [name,context,url,selector] of [['member',owner,origin+workPath,'[data-document]'],['shared',guest,guestOrigin+shared,'[data-shared-page]'],['connected',guest,guestOrigin+'/shared',`[data-connected-app="${fixture.share.share_id}"]`]]){
  const nativeContext=await browser.newContext({javaScriptEnabled:false,storageState:await context.storageState(),viewport:{width:1440,height:1100}});
  try{const nativePage=await nativeContext.newPage();await nativePage.goto(url,{waitUntil:'networkidle'});const root=nativePage.locator(selector);assert.equal(await root.getAttribute('class').then(v=>v.split(/\s+/).includes('composed-app')),true);if(name==='connected'){assert.equal(await root.locator('[data-record-component],[data-app-action],form[data-command]').count(),0);assert.equal(await root.getByRole('link',{name:'Open app',exact:true}).getAttribute('href'),shared);report.native_connected={inventory_only:true,protected_record_components:false,exact_open_app_fallback:true};}else{report.native_computed_themes[name]=await themeOf(root);assert.deepEqual(report.native_computed_themes[name],expectedTheme);}}finally{await nativeContext.close();}
 }
 report.checks.push('Actual computed root/card/text/receipt colors match the dark theme on all 3 live surfaces and separate JavaScript-disabled member/shared native renders. The no-JS connected canvas deliberately exposes inventory and an exact Open app fallback, with no record components/actions before authorized refresh.');
 const memberParts=await page.locator('[data-app-section]').evaluateAll(nodes=>nodes.map(n=>({id:n.dataset.appSection,parts:[...n.querySelectorAll('[data-component]')].map(p=>p.dataset.component)})));
 const recipientParts=await recipient.locator('[data-app-section]').evaluateAll(nodes=>nodes.map(n=>({id:n.dataset.appSection,parts:[...n.querySelectorAll('[data-component]')].map(p=>p.dataset.component)})));
 assert.deepEqual(recipientParts,memberParts);assert.deepEqual(await connected.locator('[data-app-section]').evaluateAll(nodes=>nodes.map(n=>({id:n.dataset.appSection,parts:[...n.querySelectorAll('[data-component]')].map(p=>p.dataset.component)}))),memberParts);
 report.checks.push('Actual guest acceptance and Add to my canvas preserve the published section order/theme; native HTML contains authoritative record views and real receipts, while command/Ask/editor/member links are absent.');
 // Keep both actual guest app views open before the owner's real command.
 await recipient.getByRole('button',{name:'Inspect meals',exact:true}).click();await connected.getByRole('button',{name:'Inspect meals',exact:true}).click();
 assert.equal(await recipient.locator('.app-component-dialog [data-record-component="ledger"]').count(),1);assert.equal(await connected.locator('.app-component-dialog [data-record-component="ledger"]').count(),1);
 for(const tab of[recipient,board])for(const key of [...Array(14).fill('Tab'),...Array(8).fill('Shift+Tab')]){await tab.keyboard.press(key);assert.equal(await tab.locator('.app-component-dialog').evaluate(node=>node.contains(document.activeElement)),true,'Keyboard stays inside the single live component dialog');}
 report.checks.push('Forward and reverse keyboard cycles stay inside both actual guest record dialogs.');
 let recipientNavigations=0,boardNavigations=0;recipient.on('framenavigated',frame=>{if(frame===recipient.mainFrame())recipientNavigations++;});board.on('framenavigated',frame=>{if(frame===board.mainFrame())boardNavigations++;});
 const marker=fixture.marker||`Live cafe ${Date.now()}`;fixture.marker=marker;await save();
 if(!fixture.command)await mutation('owner commits meal once',async()=>{
  await page.getByRole('button',{name:'Add meal',exact:true}).click();const modal=page.locator('.app-component-dialog');await modal.locator('[name="value.restaurant"]').fill(marker);await modal.locator('[name="value.amount"]').fill('12.50');await modal.locator('[name="value.date"]').fill('2026-11-02');
  const answer=page.waitForResponse(r=>r.request().method()==='POST'&&new URL(r.url()).pathname.startsWith(base+'/commands/'));const start=Date.now();await modal.locator('form[data-command] button[type="submit"]').click();const r=await answer;assert.equal(r.status(),200);fixture.command=await r.json();assert.equal(fixture.command.state,'committed');fixture.committed_at_browser=Date.now();report.timings.command_http_ms=Date.now()-start;await save();
 });
 const committed=fixture.committed_at_browser;
 const [sharedMs,connectedMs]=await Promise.all([(async()=>{await recipient.locator('.app-component-dialog tbody').filter({hasText:marker}).waitFor({timeout:20000});return Date.now()-committed;})(),(async()=>{await connected.locator('.app-component-dialog tbody').filter({hasText:marker}).waitFor({timeout:20000});return Date.now()-committed;})()]);
 report.timings.shared_modal_record_after_http_ms=sharedMs;report.timings.connected_modal_record_after_http_ms=connectedMs;
 const memberBatch=await api(page,base+'/record-components'),guestBatch=await api(recipient,`/v1/shared/${fixture.share.share_id}/record-components`);assert.equal(memberBatch.status,200);assert.equal(guestBatch.status,200);
 const projected=p=>({id:p.id,summary:p.summary,population:p.population,rows:p.rows,groups:p.groups,budget:p.budget,remaining:p.remaining});assert.deepEqual(guestBatch.body.components.map(projected),memberBatch.body.components.map(projected));
 const ledger=guestBatch.body.components.find(p=>p.id==='ledger');assert.equal(ledger.rows.length,1);assert.equal(ledger.rows[0].id,fixture.command.record_id);assert.equal(ledger.rows[0].version,String(fixture.command.record_version));assert.ok(ledger.rows[0].cells.some(c=>c.text===marker));
 for(const tab of[recipient,board]){const modal=tab.locator('.app-component-dialog');const receipt=JSON.parse(await modal.locator('.receipt pre').textContent());assert.equal(receipt.snapshot_digest,JSON.parse(ledger.receipt_json).snapshot_digest);assert.equal(receipt.result_digest,JSON.parse(ledger.receipt_json).result_digest);assert.equal(receipt.partial,false);assert.equal(receipt.total_scope,'full_authorized_filtered_collection');}
 report.receipts=guestBatch.body.components.map(receiptSummary);report.command={record_version:fixture.command.record_version,work_version:fixture.command.work_version,commit_sequence:fixture.command.commit_sequence,has_audit_event:!!fixture.command.audit_event_id};
 report.checks.push('A real owner command updated both already-open recipient record dialogs without navigation; visible values and record version match the command and owner projection, with actual snapshot/result receipt digests.');
 await shot(recipient,'guest-live-dialog.png');await shot(board,'connected-live-dialog.png');
 await recipient.keyboard.press('Escape');await board.keyboard.press('Escape');
 for(const width of [320,390]){await recipient.setViewportSize({width,height:844});assert.equal(await recipient.evaluate(()=>document.documentElement.scrollWidth<=innerWidth+1),true,'Only the table scrolls, not the page');const table=recipient.locator('[data-record-component="ledger"] .shared-table');assert.equal(await table.evaluate(n=>n.scrollWidth>n.clientWidth),true,'Narrow records use the labeled scroll region');assert.equal(await table.locator('td').first().evaluate(n=>getComputedStyle(n).whiteSpace),'nowrap');}
 report.checks.push('At 320 and 390 CSS pixels, record values stay unbroken in the labeled horizontal table region without page overflow.');
 await recipient.getByRole('button',{name:'About this journal',exact:true}).click();assert.equal(await recipient.locator('.app-component-dialog').evaluate(n=>n.scrollWidth<=n.clientWidth+1),true);await shot(recipient,'guest-mobile-dialog.png');await recipient.keyboard.press('Escape');
 const offlineAt=Date.now();await recipient.getByRole('button',{name:'Inspect meals',exact:true}).click();await connected.getByRole('button',{name:'Inspect meals',exact:true}).click();await guest.setOffline(true);
 await until(async()=>await recipient.locator('.app-component-dialog,[data-shared-parts]').count()===0&&await connected.locator('.app-component-dialog,[data-shared-parts]').count()===0,5200);report.timings.offline_both_mask_ms=Date.now()-offlineAt;assert.equal(await recipient.getByText(marker,{exact:true}).count(),0);assert.equal(await board.getByText(marker,{exact:true}).count(),0);
 let reauthorizations=0;const observed=r=>{if(new URL(r.url()).pathname===`/v1/shared/${fixture.share.share_id}/authority`&&r.status()===200)reauthorizations++;};recipient.on('response',observed);board.on('response',observed);await guest.setOffline(false);await recipient.locator('[data-shared-parts]').waitFor();await connected.locator('[data-shared-parts]').waitFor();assert.ok(reauthorizations>=2);assert.equal(await recipient.locator('.app-component-dialog').count(),0);assert.equal(await connected.locator('.app-component-dialog').count(),0);
 report.reconnect_authority_successes=reauthorizations;report.checks.push('Offline masks both open dialogs and protected app roots; reconnect reauthorizes both surfaces before restoring data and does not reopen a stale dialog.');
 await recipient.getByRole('button',{name:'Inspect meals',exact:true}).click();await connected.getByRole('button',{name:'Inspect meals',exact:true}).click();
 await page.keyboard.press('Escape');await page.locator('[data-manage-share]').click();const sharing=page.getByRole('dialog',{name:'Share this app',exact:true});const person=sharing.locator('.share-person').filter({hasText:'guest@example.test'});await person.getByRole('button',{name:'Remove access',exact:true}).waitFor();
 await mutation('owner revokes live shared app',async()=>{const answer=page.waitForResponse(r=>new URL(r.url()).pathname===`${sharesPath}/${fixture.share.share_id}`&&r.request().method()==='DELETE');await person.getByRole('button',{name:'Remove access',exact:true}).click();const r=await answer;assert.equal(r.status(),200);fixture.revoked=true;fixture.revoked_at_browser=Date.now();await save();});
 await until(async()=>await recipient.locator('.app-component-dialog,[data-shared-parts]').count()===0&&await connected.locator('.app-component-dialog,[data-shared-parts]').count()===0,5200);report.timings.revoked_both_mask_after_http_ms=Date.now()-fixture.revoked_at_browser;
 assert.equal(await recipient.locator('[data-app-action]').count(),0);assert.equal(await recipient.getByText(marker,{exact:true}).count(),0);assert.equal(await board.getByText(marker,{exact:true}).count(),0);assert.equal((await api(recipient,shared)).status,404);assert.equal((await api(recipient,`/v1/shared/${fixture.share.share_id}/authority`)).status,404);assert.equal((await api(recipient,workPath)).status,404);assert.equal((await guest.request.get(guestOrigin+'/workspace')).status(),404);
 const inventory=await api(board,'/v1/shared');assert.equal(inventory.status,200);assert.ok(!inventory.body.apps.some(a=>a.id===fixture.share.share_id));assert.equal(recipientNavigations,0);assert.equal(boardNavigations,0);
 report.checks.push('Actual owner revoke while both record dialogs are open closes and masks both within the observed five-second bound; JSON/authority/member routes refuse, connected inventory removes the grant, and stale actions cannot reopen content. Browser denials do not claim exhaustive database membership absence.');
 await shot(recipient,'guest-revoked.png');await shot(board,'connected-revoked.png');assert.deepEqual(report.errors,[]);assert.equal(createHash('sha256').update(await readFile('app/admin/dist/manifest.json')).digest('hex'),manifestBefore,'Assets changed during the acceptance run; repeat qualification on stable source.');report.passed=true;
}catch(error){report.passed=false;report.failure=String(error);process.exitCode=1;await shot(recipient,'guest-failure.png').catch(()=>{});await shot(page,'owner-failure.png').catch(()=>{});}
finally{report.finished_at=new Date().toISOString();await browser.close();await writeFile(resolve(output,'report.json'),JSON.stringify(report,null,2));console.log(JSON.stringify(report));}
