// Use a separately installed Playwright module; no browser profile is shared.
const {chromium}=await import(process.env.PLAYWRIGHT_MODULE || 'playwright');
import assert from 'node:assert/strict';
import {readFile,writeFile,mkdir} from 'node:fs/promises';
const fixturePath=process.argv[2];
if(!fixturePath)throw new Error('Pass the synthetic admitted-app fixture JSON path.');
const output=process.argv[3] || '.cache/reports/app154-repro';
await mkdir(output,{recursive:true});
const fixture=JSON.parse(await readFile(fixturePath,'utf8'));
const ownerOrigin=process.env.OWNER_ORIGIN || 'http://127.0.0.1:7601',guestOrigin=process.env.GUEST_ORIGIN || 'http://127.0.0.1:7604';
const path=`/w/${fixture.app_id}/${fixture.work_id}`,shares=`/v1/work/${fixture.app_id}/${fixture.work_id}/shares`;
const browser=await chromium.launch({channel:'chrome',headless:true});
const owner=await browser.newContext({viewport:{width:1440,height:1000}}),guest=await browser.newContext({viewport:{width:1440,height:1000}}),member=await browser.newContext({viewport:{width:1440,height:1000}});
const page=await owner.newPage(),recipient=await guest.newPage(),board=await guest.newPage(),colleague=await member.newPage();
const report={scope:'Final native navigation and visual check on local dev stand with synthetic identities',checks:[],errors:[]};
for(const p of [page,recipient,board,colleague])p.on('pageerror',e=>report.errors.push(e.message));
const api=async(p,url,method='GET',body)=>p.evaluate(async({url,method,body})=>{const r=await fetch(url,{method,headers:{accept:'application/json',...(body?{'content-type':'application/json'}:{})},...(body?{body:JSON.stringify(body)}:{})});return{status:r.status,body:await r.json().catch(()=>null)};},{url,method,body});
try{
 await page.goto(ownerOrigin+path);
 const created=(await api(page,shares,'POST',{recipient_email:'guest@example.test',allow_add_to_canvas:true,expires_in_days:7,idempotency_key:crypto.randomUUID()})).body.share;
 assert.ok(created);
 await recipient.goto(guestOrigin+created.href);await recipient.getByRole('button',{name:'Accept app invitation'}).click();await recipient.getByRole('button',{name:'Add to my canvas'}).waitFor();await recipient.getByRole('button',{name:'Add to my canvas'}).click();await recipient.getByRole('link',{name:'Open my connected apps'}).waitFor();
 await board.goto(guestOrigin+'/shared');await board.locator('[data-connected-app] [data-shared-parts]').waitFor({timeout:10000});assert.equal(await board.getByRole('link',{name:'Workspace',exact:true}).count(),0);
 report.checks.push('Account-only guest native shared canvas omits Workspace link.');await board.screenshot({path:output+'/connected-canvas.png',fullPage:true});
 await recipient.screenshot({path:output+'/recipient-desktop.png',fullPage:true});await recipient.setViewportSize({width:390,height:844});await recipient.screenshot({path:output+'/recipient-mobile.png',fullPage:true});assert.equal(await recipient.evaluate(()=>document.documentElement.scrollWidth<=innerWidth),true);
 await colleague.goto((process.env.MEMBER_ORIGIN || 'http://127.0.0.1:7602')+'/shared');await colleague.getByRole('link',{name:'Workspace',exact:true}).waitFor();assert.equal((await colleague.request.get((process.env.MEMBER_ORIGIN || 'http://127.0.0.1:7602')+'/workspace')).status(),200);report.checks.push('Existing workspace member native shared canvas retains working Workspace link.');
 await page.getByRole('button',{name:'Share app',exact:true}).click();await page.getByText('Changing its definition or data-source connections pauses this share.',{exact:false}).waitFor();await page.screenshot({path:output+'/owner-share-created.png',fullPage:false});await page.setViewportSize({width:390,height:844});await page.screenshot({path:output+'/owner-share-mobile.png',fullPage:false});assert.equal(await page.getByRole('dialog').count(),1);assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth),true);report.checks.push('Final owner dialog copy includes data-source changes; one dialog and no mobile horizontal overflow.');
 await recipient.getByRole('button',{name:'Sign out of shared apps'}).click();await recipient.getByText('You are signed out. Open a shared app link to sign in.').waitFor();assert.equal(await recipient.getByRole('link',{name:'Workspace',exact:true}).count(),0);assert.equal(await recipient.getByRole('button',{name:'Sign out',exact:true}).count(),0);assert.equal(await recipient.locator('[data-shared-parts]').count(),0);await recipient.screenshot({path:output+'/signed-out-mobile.png',fullPage:false});report.checks.push('Signed-out guest native canvas has no protected content, Workspace link or sign-out action.');
 const current=(await api(page,shares)).body.shares.find(s=>s.share_id===created.share_id);assert.equal((await api(page,shares+'/'+created.share_id,'DELETE',{expected_version:current.version})).status,200);
 assert.deepEqual(report.errors,[]);await writeFile(output+'/final-navigation.json',JSON.stringify(report,null,2));console.log(JSON.stringify(report));
}finally{await browser.close();}
