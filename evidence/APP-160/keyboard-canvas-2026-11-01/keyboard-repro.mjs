import assert from 'node:assert/strict';
import { readFile,writeFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { chromium } from '/tmp/audit-browser/node_modules/playwright/index.mjs';
const fixture=JSON.parse(await readFile('.cache/reports/composed-browser-final/fixture.json','utf8'));
assert.equal(fixture.designed,true);
const origin='http://127.0.0.1:4801';
const output='.cache/reports/composed-accessibility';
const report={scope:'Read-only synthetic local app keyboard and Chromium accessibility-tree checks. No VoiceOver or real screen-reader announcement claim.',started:new Date().toISOString(),checks:[],requests:{command_posts:0,presentation_posts:0},page_errors:[]};
const browser=await chromium.launch({channel:'chrome',headless:true});
const context=await browser.newContext({viewport:{width:1440,height:1100},reducedMotion:'reduce'});
const page=await context.newPage();
const cdp=await context.newCDPSession(page);
page.on('pageerror',error=>report.page_errors.push(error.message));
page.on('request',request=>{if(request.method()==='POST'){const p=new URL(request.url()).pathname;if(p.includes('/commands/'))report.requests.command_posts++;if(p.endsWith('/presentation'))report.requests.presentation_posts++;}});
const manifest=async()=>createHash('sha256').update(await readFile('app/admin/dist/manifest.json')).digest('hex');
report.manifest_sha256=await manifest();
const tree=async()=>{const result=await cdp.send('Accessibility.getFullAXTree');return result.nodes.filter(n=>!n.ignored).map(n=>({role:n.role?.value,name:n.name?.value,...(n.properties?.some(p=>p.name==='modal'&&p.value?.value===true)?{modal:true}:{})}));};
try{
 await page.goto(`${origin}/w/${fixture.dining.app_id}/${fixture.dining.work_id}`);
 await page.getByText('Up to date.',{exact:true}).waitFor();
 await page.getByRole('button',{name:'Log meal',exact:true}).waitFor();
 const initial=await tree();
 for(const name of ['Log meal','About this tracker','View meals'])assert.ok(initial.some(n=>n.role==='button'&&n.name===name),`AX button ${name}`);
 report.checks.push('App actions appear as named buttons in the Chromium accessibility tree.');
 let reached=false;const sequence=[];
 for(let n=0;n<80;n++){await page.keyboard.press('Tab');const active=await page.evaluate(()=>({tag:document.activeElement?.tagName,name:(document.activeElement?.getAttribute('aria-label')||document.activeElement?.textContent||'').trim().slice(0,80)}));sequence.push(active);if(await page.getByRole('button',{name:'Log meal',exact:true}).evaluate(n=>document.activeElement===n)){reached=true;break;}}
 assert.ok(reached,'primary action keyboard reachable');
 report.keyboard_to_primary=sequence;
 await page.keyboard.press('Enter');
 const modal=page.getByRole('dialog',{name:'Log meal',exact:true});await modal.waitFor();
 assert.equal(await modal.evaluate(n=>n.matches(':modal')),true);
 assert.equal(await modal.locator('[name="value.restaurant"]').evaluate(n=>document.activeElement===n),true);
 const open=await tree();
 assert.ok(open.some(n=>n.role==='dialog'&&n.name==='Log meal'&&n.modal===true));
 assert.ok(open.some(n=>n.role==='textbox'&&n.name==='amount (USD)'));
 assert.equal(await modal.locator('[name="value.amount"]').getAttribute('inputmode'),'decimal');
 assert.ok(open.some(n=>n.role==='button'&&n.name==='Close'));
 assert.ok(open.some(n=>n.role==='button'&&n.name==='Add'));
 report.modal_tab_sequence=[]; for(let i=0;i<24;i++){await page.keyboard.press('Tab');report.modal_tab_sequence.push(await page.evaluate(()=>({tag:document.activeElement?.tagName,name:(document.activeElement?.getAttribute('aria-label')||document.activeElement?.getAttribute('name')||document.activeElement?.textContent||'').trim().slice(0,80)})));assert.equal(await modal.evaluate(n=>n.contains(document.activeElement)),true,'forward focus remains in modal');}
 for(let i=0;i<12;i++){await page.keyboard.press('Shift+Tab');assert.equal(await modal.evaluate(n=>n.contains(document.activeElement)),true,'reverse focus remains in modal');}
 report.checks.push('Enter opens a named native modal; initial focus is the first field, currency is in the accessible input name, and 24 forward/12 reverse tabs stay inside.');
 await writeFile(`${output}/modal-ax.json`,JSON.stringify({scope:report.scope,nodes:open},null,2));
 await page.screenshot({path:`${output}/keyboard-modal-desktop.png`,fullPage:false});
 await page.keyboard.press('Escape');await modal.waitFor({state:'detached'});
 assert.equal(await page.getByRole('button',{name:'Log meal',exact:true}).evaluate(n=>document.activeElement===n),true);
 await page.keyboard.press('Enter');await modal.waitFor();
 await modal.getByRole('button',{name:'Close',exact:true}).focus();await page.keyboard.press('Space');await modal.waitFor({state:'detached'});
 assert.equal(await page.getByRole('button',{name:'Log meal',exact:true}).evaluate(n=>document.activeElement===n),true);
 report.checks.push('Escape and the keyboard-operated Close button dismiss the modal and restore focus to its exact opener.');
 await page.setViewportSize({width:320,height:844});
 assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth),true);
 await page.keyboard.press('Enter');await modal.waitFor();
 assert.equal(await modal.evaluate(n=>n.scrollWidth<=n.clientWidth+1),true);
 await page.screenshot({path:`${output}/keyboard-modal-320.png`,fullPage:false});
 await page.keyboard.press('Escape');
 report.checks.push('At 320 CSS pixels both app and modal fit the viewport; keyboard modal dismissal still succeeds.');
 assert.equal(report.requests.command_posts,0);assert.equal(report.requests.presentation_posts,0);assert.deepEqual(report.page_errors,[]);
 assert.equal(await manifest(),report.manifest_sha256,'stable build required');
 report.passed=true;report.completed=new Date().toISOString();
}catch(error){report.passed=false;report.failure=String(error);await page.screenshot({path:`${output}/failure.png`,fullPage:true}).catch(()=>{});throw error;}
finally{await writeFile(`${output}/report.json`,JSON.stringify(report,null,2));await browser.close();console.log(JSON.stringify({passed:report.passed,checks:report.checks.length,failure:report.failure}));}
