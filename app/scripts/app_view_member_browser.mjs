// APP-168 member qualification: real owned loopback routes and ordinary UI.
// Synthetic dev identity only. No hosted provider, model request, or seed write.
import assert from 'node:assert/strict';
import {mkdir,readFile,writeFile} from 'node:fs/promises';
import {resolve} from 'node:path';
import {createHash} from 'node:crypto';
const args=Object.fromEntries(process.argv.slice(2).map(arg=>arg.replace(/^--/,'').split('=')));
const origin=args.origin||'http://127.0.0.1:4703';
assert.equal(origin,'http://127.0.0.1:4703','This harness owns only the requested Priya stand');
const out=resolve(args.output||'.cache/reports/app-view-member');await mkdir(out,{recursive:true});
const fixturePath=resolve(out,'fixture.json');
const fixture=JSON.parse(await readFile(fixturePath,'utf8').catch(()=>'{}'));
const persist=()=>writeFile(fixturePath,JSON.stringify(fixture,null,2));
const hash=bytes=>createHash('sha256').update(bytes).digest('hex');
const manifest=()=>readFile('app/admin/dist/manifest.json').then(hash);
const startedManifest=await manifest();
const report={scope:'Owned 4703 synthetic member; real template admission, authorized reads and UI. No hosted identity claim.',started_at:new Date().toISOString(),manifest_sha256:startedManifest,checks:[],errors:[],requests:[],artifacts:[]};
const {chromium}=await import(process.env.PLAYWRIGHT_MODULE||'/tmp/audit-browser/node_modules/playwright/index.mjs');
const browser=await chromium.launch({channel:'chrome',headless:true});
const context=await browser.newContext({viewport:{width:1440,height:1000},reducedMotion:'reduce'});
const page=await context.newPage();
const wire=tab=>{tab.on('pageerror',e=>report.errors.push(e.message));tab.on('request',r=>{if(r.method()!=='GET')report.requests.push({method:r.method(),path:new URL(r.url()).pathname});});};wire(page);
const check=text=>report.checks.push(text);
const until=async(fn,description,ms=20000)=>{const end=Date.now()+ms;while(Date.now()<end){if(await fn())return;await new Promise(r=>setTimeout(r,100));}throw Error(description);};
const api=async(path)=>page.evaluate(async path=>{const r=await fetch(path,{credentials:'same-origin',cache:'no-store',headers:{accept:'application/json'}});return{status:r.status,body:await r.json().catch(()=>null)};},path);
const shot=async(tab,name)=>{await tab.screenshot({path:resolve(out,name),fullPage:true});report.artifacts.push(name);};
let item,frame,base,work;
const mode=async value=>{await frame.locator(`[data-app-view-mode="${value}"]`).click();await until(async()=>await frame.getAttribute('data-app-view')===value,'View did not change');assert.equal(await item.count(),1);};
const closeEditor=async()=>{await page.locator('[data-presentation-editor]').getByRole('button',{name:'Cancel',exact:true}).click();await page.locator('[data-presentation-editor]').waitFor({state:'detached'});};
const visibility=async hide=>{
 await frame.locator('[data-edit-presentation]').click();const editor=page.locator('[data-presentation-editor]');
 const control=editor.locator('[data-editor-component="add"] input[type="checkbox"]');await control.waitFor();await control.setChecked(hide);
 await editor.getByRole('button',{name:'Save my view preferences',exact:true}).click();await editor.getByText('View preferences saved on this browser. The published app and shared links are unchanged.',{exact:true}).waitFor();await closeEditor();
};
try{
 await until(async()=>{try{return(await fetch(origin+'/canvas')).status===200;}catch{return false;}},'Owned stand did not become healthy',30000);
 await page.goto(origin+'/canvas');await page.locator('#cv-canvas.is-canvas').waitFor();
 if(!fixture.dining){
  if(fixture.pending_create){
   // A prior browser interruption is not proof of a failed write. Require
   // two separated authorized inventories to prove no matching work exists.
   const prior=JSON.parse(await readFile(resolve(out,'reconcile.json'),'utf8'));
   assert.ok(Date.now()-Date.parse(fixture.pending_create.started_at)>=60000);
   assert.equal(prior.rows.filter(row=>/dining/i.test(row.title||'')).length,0);
   const current=await page.locator('.cv-lib-app[data-work]').evaluateAll(nodes=>nodes.map(n=>({app_id:n.dataset.app,work_id:n.dataset.work,title:n.querySelector('.cv-lib-app-title')?.textContent})));
   assert.equal(current.filter(row=>/dining/i.test(row.title||'')).length,0,'Matching work exists: manual outcome reconciliation required');
   fixture.interrupted_attempts??=[];fixture.interrupted_attempts.push({...fixture.pending_create,confirmed_absent_at:new Date().toISOString(),prior_inventory_sha256:hash(await readFile(resolve(out,'reconcile.json')))});
   delete fixture.pending_create;await persist();
  }
  const templates=await api('/v1/templates');assert.equal(templates.status,200);await writeFile(resolve(out,'template-catalog.json'),JSON.stringify(templates.body,null,2));
  await page.locator('[data-create-app]').click();await page.locator('[data-template="dining-tracker"]').click();
  const description=await page.locator('#cv-create-app textarea').inputValue();
  assert.equal(description,templates.body.templates.find(row=>row.id==='dining-tracker')?.description);
  fixture.pending_create={template:'dining-tracker',description,started_at:new Date().toISOString()};await persist();
  const awaited=page.waitForResponse(r=>new URL(r.url()).pathname==='/v1/compose'&&r.request().method()==='POST');
  await page.locator('#cv-create-app').getByRole('button',{name:'Create',exact:true}).click();
  const response=await awaited;assert.equal(response.status(),200,await response.text());
  const request=response.request().postDataJSON();assert.equal(request.template_id,'dining-tracker');
  assert.equal(request.text??request.intent??request.sentence,description,'Template description stayed unchanged');
  fixture.dining=await response.json();fixture.created_request={template_id:request.template_id,intent:request.intent};delete fixture.pending_create;await persist();
  await writeFile(resolve(out,'create-response.json'),JSON.stringify(fixture.dining,null,2));
  check('Created a new Dining tracker via the template picker with its unchanged description; no supplied model result or records.');
 }else{
  await page.locator('#cv-library-toggle').click();
  await page.locator(`#lib-${fixture.dining.work_id} [data-lib-open-view]`).click();
  await page.locator('[data-lib-close]').click();
  check('Reused the exact journaled template work; no duplicate creation.');
 }
 work=fixture.dining.work_id;base=`/v1/work/${fixture.dining.app_id}/${work}`;
 item=page.locator(`.cv-item-unite[data-unite="${work}"]`);frame=item.locator('.cv-unite');await frame.waitFor({timeout:30000});
 await mode('open');
 const initial=await api(base+'/presentation');assert.equal(initial.status,200);assert.equal(initial.body.can_edit,true);
 assert.equal(await frame.locator('[data-edit-presentation]').count(),1);assert.equal(await frame.locator('[data-unite-share]').count(),1);
 await frame.locator('[data-unite-share]').click();await page.locator('.share-dialog').waitFor();await page.getByRole('button',{name:'Close sharing',exact:true}).click();
 await visibility(false); // Idempotent local preference reset after a failed browser attempt.
 check('The new owner work exposes common Edit and Share controls; both open their authorized management UI.');
 await frame.locator('[data-app-action="primary"]').click();const dialog=page.locator('.app-component-dialog');await dialog.waitFor();
 await dialog.locator('[name="value.restaurant"]').fill('Unsent app-view draft');
 await page.evaluate(work=>{const form=document.querySelector(`.cv-item-unite[data-unite="${work}"] .app-component-dialog form`);window.appViewProof={item:document.querySelector(`.cv-item-unite[data-unite="${work}"]`),form,input:form.querySelector('[name="value.restaurant"]'),key:form.querySelector('[name="idempotency_key"]').value};},work);
 await page.keyboard.press('Escape');await dialog.waitFor({state:'detached'});
 for(const next of ['face','stack','open'])await mode(next);
 await frame.locator('[data-app-action="primary"]').click();await dialog.waitFor();
 assert.equal(await dialog.locator('[name="value.restaurant"]').inputValue(),'Unsent app-view draft');
 assert.equal(await page.evaluate(()=>window.appViewProof.form===document.querySelector('.app-component-dialog form')&&window.appViewProof.key===document.querySelector('.app-component-dialog [name="idempotency_key"]').value),true);
 await page.keyboard.press('Escape');
 check('Card → Stack → Open retains one outer item, the same unsent form node, its value and original idempotency key.');
 await visibility(true);
 assert.equal(await frame.locator('[data-app-action="primary"]').count(),0);
 const formCard=frame.locator('.cv-card').filter({has:page.locator('[name="value.restaurant"]')});
 assert.equal(await formCard.count(),1);assert.equal(await formCard.isVisible(),false);
 const total=await frame.locator('.cv-card[data-card]').count();await mode('stack');
 for(let i=0;i<=total;i++){
  assert.equal(await frame.locator('[data-app-stack-holder] [name="value.restaurant"]').count(),0,'Hidden form never enters Stack');
  await frame.locator('[data-app-stack-step="1"]').click();
 }
 const stackBefore=await frame.locator('[data-app-stack-position]').textContent();
 const number=await page.locator(`#lib-${work}`).getAttribute('data-number');assert.ok(number);
 await page.locator('#intent-input').fill(`/focus ${number}`);await page.locator('#intent-input').press('Enter');
 await page.locator('#cv-focus').waitFor({state:'visible'});
 assert.equal(await page.locator(`#cv-focus [data-thumb="f-${work}-add"]`).count(),0);
 assert.equal(await page.locator('#cv-focus [name="value.restaurant"]').count(),0);
 await page.locator('[data-focus-close]').click();assert.equal(await frame.getAttribute('data-app-view'),'stack');assert.equal(await frame.locator('[data-app-stack-position]').textContent(),stackBefore);
 await mode('open');assert.equal(await frame.locator('[data-app-action="primary"]').count(),0);
 check('Personal hide removes the form action and skips the same hidden form in Stack and Focus. /focus returns to the unchanged Stack mode/top.');
 const popupPromise=context.waitForEvent('page');await frame.getByRole('link',{name:'Open page ↗',exact:true}).click();const popup=await popupPromise;wire(popup);await popup.waitForLoadState();
 await popup.locator('[data-document]').waitFor();await until(async()=>await popup.locator('[data-app-action="primary"]').count()===0,'New-tab page did not apply local hidden preference');
 assert.equal(await popup.locator('[data-component="add"]').isVisible(),false);
 await shot(popup,'page-hidden.png');await popup.close();
 assert.deepEqual((await api(base+'/presentation')).body,initial.body);
 assert.equal(report.requests.filter(r=>r.method==='POST'&&r.path.endsWith('/presentation')).length,0);
 check('A new-tab authorized /w page honors this browser preference, while publication/digest/work version and presentation are byte-for-byte unchanged.');
 await visibility(false);await frame.locator('[data-app-action="primary"]').click();await dialog.waitFor();
 assert.equal(await dialog.locator('[name="value.restaurant"]').inputValue(),'Unsent app-view draft');
 assert.equal(await page.evaluate(()=>window.appViewProof.input===document.querySelector('.app-component-dialog [name="value.restaurant"]')&&window.appViewProof.key===document.querySelector('.app-component-dialog [name="idempotency_key"]').value),true);
 await shot(page,'draft-restored.png');await page.keyboard.press('Escape');
 check('Unhide restores the original generic action and exact unsent draft/key, without a command or publication request.');
 await page.setViewportSize({width:375,height:812});await mode('face');
 const nav=page.locator(`.navigator-item[data-instance-id="unite-${work}"]`);if(await nav.isVisible())await nav.click();
 await shot(page,'canvas-375.png');
 const mobilePromise=context.waitForEvent('page');await frame.getByRole('link',{name:'Open page ↗',exact:true}).click();const mobile=await mobilePromise;wire(mobile);await mobile.setViewportSize({width:375,height:812});await mobile.waitForLoadState();await mobile.locator('[data-app-action="primary"]').waitFor();
 assert.equal(await mobile.evaluate(()=>document.documentElement.scrollWidth<=innerWidth),true);
 await mobile.locator('[data-app-action="primary"]').click();const mobileDialog=mobile.locator('.app-component-dialog');await mobileDialog.waitFor();
 await mobileDialog.locator('[name="value.restaurant"]').fill('Mobile draft');
 const bounds=await mobileDialog.boundingBox();report.mobile={viewport:mobile.viewportSize(),dialog:bounds};assert.ok(bounds&&bounds.x>=0&&bounds.x+bounds.width<=375.5&&bounds.y>=0&&bounds.y+bounds.height<=812.5);
 assert.equal(await mobileDialog.evaluate(node=>node.scrollWidth<=node.clientWidth+1),true);
 await shot(mobile,'page-dialog-375.png');await mobile.keyboard.press('Escape');await mobile.close();
 check('At 375×812 the authorized app page and generic dialog fit the viewport and keep form controls reachable.');
 assert.equal(report.requests.filter(r=>r.method==='POST'&&(/\/commands\//.test(r.path)||r.path.endsWith('/presentation')||r.path.endsWith('/shares'))).length,0);
 assert.equal(await manifest(),startedManifest,'Native assets changed during qualification');assert.deepEqual(report.errors,[]);
 report.passed=true;
}catch(error){report.passed=false;report.failure=String(error);await shot(page,'failure.png').catch(()=>{});throw error;}
finally{report.completed_at=new Date().toISOString();await writeFile(resolve(out,'report.json'),JSON.stringify(report,null,2));await browser.close();}
