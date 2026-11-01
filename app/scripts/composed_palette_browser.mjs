// APP-157/160: computed appearance and same-node movement on an owned dev stand.
// Uses two existing reference apps from the member runner's explicit fixture file.
// No command/design mutation, fake sessions, private browser profile or dependency download.
// Usage and limits: app/scripts/COMPOSED-BROWSER.md; --help prints the invocation.
import assert from 'node:assert/strict';
import {mkdir,readFile,writeFile} from 'node:fs/promises';
import {resolve} from 'node:path';
import {createHash} from 'node:crypto';
const options=Object.fromEntries(process.argv.slice(2).map(arg=>arg.replace(/^--/,'').split('=')));
const usage='PLAYWRIGHT_MODULE=/absolute/path/to/playwright/index.mjs node app/scripts/composed_palette_browser.mjs --fixture=/path/to/member-run/fixture.json --owner=http://127.0.0.1:4801 --output=.cache/reports/palette-new';
if('help' in options){console.log(usage);process.exit(0);}
assert.ok(Object.keys(options).every(key=>['fixture','owner','output'].includes(key)),usage);
assert.ok(options.fixture&&process.env.PLAYWRIGHT_MODULE,usage);
const origin=options.owner||'http://127.0.0.1:4801';
assert.match(origin,/^http:\/\/127\.0\.0\.1:(?!400[1-9]$|4010$)\d+$/,'Owned disposable loopback stand required');
const fixtureBytes=await readFile(resolve(options.fixture));const fixtures=JSON.parse(fixtureBytes);
for(const name of ['dining','equipment'])for(const key of ['app_id','work_id'])assert.match(fixtures[name]?.[key]||'',/^[0-9a-f]{8}-[0-9a-f]{4}-[47][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/,'A completed member reference fixture with exact work IDs is required');
const output=resolve(options.output||'.cache/reports/composed-palette-browser');await mkdir(output,{recursive:true});
assert.equal(await readFile(resolve(output,'report.json')).then(()=>true).catch(()=>false),false,'Preserve previous results and choose a fresh output directory');
const {chromium}=await import(process.env.PLAYWRIGHT_MODULE);
const manifest=async()=>createHash('sha256').update(await readFile('app/admin/dist/manifest.json')).digest('hex');
const report={scope:'Actual local synthetic existing Dining dark and Equipment default-light apps. Read-only product journey; no saved commands or design changes.',started_at:new Date().toISOString(),fixture_sha256:createHash('sha256').update(fixtureBytes).digest('hex'),manifest_sha256:await manifest(),checks:[],themes:{},errors:[],writes:[],authorized_query_posts:0};
const browser=await chromium.launch({channel:'chrome',headless:true});
const context=await browser.newContext({viewport:{width:1440,height:1100},reducedMotion:'reduce',serviceWorkers:'block'});const page=await context.newPage();page.setDefaultTimeout(15000);
const readQueries=new Set(['dining','equipment'].map(name=>`/v1/work/${fixtures[name].app_id}/${fixtures[name].work_id}/record-components`));
const isReadQuery=request=>{const url=new URL(request.url());return request.method()==='POST'&&url.origin===origin&&readQueries.has(url.pathname);};
await page.route('**/*',route=>['GET','HEAD','OPTIONS'].includes(route.request().method())||isReadQuery(route.request())?route.continue():route.abort('blockedbyclient'));
page.on('pageerror',e=>report.errors.push(e.message));page.on('request',r=>{if(isReadQuery(r)){report.authorized_query_posts++;return;}if(!['GET','HEAD','OPTIONS'].includes(r.method()))report.writes.push({method:r.method(),path:new URL(r.url()).pathname});});
function luminance(color){const c=color.slice(0,3).map(v=>{v/=255;return v<=.04045?v/12.92:((v+.055)/1.055)**2.4;});return c[0]*.2126+c[1]*.7152+c[2]*.0722;}
function contrast(a,b){const x=luminance(a),y=luminance(b);return(Math.max(x,y)+.05)/(Math.min(x,y)+.05);}
async function inspect(root,selectors){return root.evaluate((node,selectors)=>{const canvas=document.createElement('canvas');canvas.width=canvas.height=1;const ctx=canvas.getContext('2d');const rgba=color=>{ctx.clearRect(0,0,1,1);ctx.fillStyle=color;ctx.fillRect(0,0,1,1);return Array.from(ctx.getImageData(0,0,1,1).data);};const bg=node=>{if(!node)return[255,255,255];const c=rgba(getComputedStyle(node).backgroundColor),a=c[3]/255;if(a===1)return c.slice(0,3);const p=bg(node.parentElement);return c.slice(0,3).map((v,i)=>Math.round(v*a+p[i]*(1-a)));};const s=getComputedStyle(node);return{mode:node.dataset.appMode,root_background:bg(node),root_text:rgba(s.color),samples:selectors.flatMap(selector=>[...node.querySelectorAll(selector)].filter(n=>n.getClientRects().length&&!n.closest('[hidden]')).map(n=>({selector,text:n.textContent.trim().slice(0,50),foreground:rgba(getComputedStyle(n).color),background:bg(n),css_background:getComputedStyle(n).backgroundColor})))}} ,selectors);}
function checkPalette(value,label){assert.ok(contrast(value.root_text,value.root_background)>=4.5,`${label} root`);assert.ok(value.samples.length>0,`${label} samples`);for(const item of value.samples){item.contrast=contrast(item.foreground,item.background);assert.ok(item.contrast>=4.5,`${label} ${item.selector} ${item.text}: ${item.contrast}`);}}
try{
 for(const name of ['dining','equipment']){
  const d=fixtures[name];assert.ok(d);await page.goto(`${origin}/w/${d.app_id}/${d.work_id}`);await page.getByText('Up to date.',{exact:true}).waitFor();
  const doc=page.locator('[data-document]');const presentation=JSON.parse(await doc.getAttribute('data-presentation'));const action=presentation.actions.find(a=>a.behavior==='dialog');assert.ok(action);
  const results=report.themes[name]={standalone:await inspect(doc,['h1','[data-app-action]','.receipt summary'])};checkPalette(results.standalone,`${name} standalone`);
  await page.getByRole('link',{name:'Open on the canvas',exact:true}).click();const appCard=page.locator(`.cv-card[data-kind="app"][data-work="${d.work_id}"]`);await appCard.locator('[data-unite-app]').click();const united=page.locator(`.cv-unite[data-work="${d.work_id}"]`);await united.waitFor();if(name==='dining')await united.locator('[data-record-component="total"][data-state="ready"]').waitFor();
  results.united=await inspect(united,['.cv-unite-head h2','.cv-unite-head button','.cv-unite-head a','.cv-unite-head .cv-origin','.cv-unite-count','.receipt summary']);checkPalette(results.united,`${name} united`);await page.screenshot({path:`${output}/${name}-united.png`});
  await united.locator(`[data-app-action="${action.id}"]`).click();const modal=page.locator('.app-component-dialog');const input=modal.locator('input[type="text"]').first();await input.fill('Unsent palette probe');
  await input.evaluate(n=>{window.paletteNode=n;window.paletteKey=n.closest('form').querySelector('[name="idempotency_key"]').value;});await page.keyboard.press('Escape');await united.locator('[data-unite-split]').click();
  const formCard=page.locator(`.cv-card[data-kind="form"][data-work="${d.work_id}"]`).first();const card=await formCard.getAttribute('data-card');await page.locator(`.navigator-item[data-instance-id="${card}"]`).click();
  results.fanned=await inspect(formCard,['.cv-card-head h3','.cv-field-label','.badge','input[type="text"]','a','button[type="submit"]']);checkPalette(results.fanned,`${name} fanned`);await formCard.locator('[data-focus-card]').click();const focus=page.locator('[data-focus-stage] .cv-card');await focus.waitFor();
  results.focus=await inspect(focus,['.cv-card-head h3','.cv-field-label','.badge','input[type="text"]','a','button[type="submit"]']);checkPalette(results.focus,`${name} focus`);
  assert.deepEqual(results.focus.root_background,results.fanned.root_background);assert.equal(await focus.locator('input[type="text"]').first().evaluate(n=>n===window.paletteNode&&n.value==='Unsent palette probe'&&n.closest('form').querySelector('[name="idempotency_key"]').value===window.paletteKey),true);
  await page.screenshot({path:`${output}/${name}-focus.png`});await page.locator('[data-focus-close]').click();
  if(name==='dining'){
   const recordCard=page.locator(`.cv-card[data-work="${d.work_id}"][data-component="ledger"]`);const recordId=await recordCard.getAttribute('data-card');await formCard.locator('[data-focus-card]').click();await page.locator(`.cv-focus-thumb[data-thumb="${recordId}"]`).click();const recordFocus=page.locator('[data-focus-stage] .cv-card');await recordFocus.locator('.receipt summary').waitFor();results.record_focus=await inspect(recordFocus,['h2','.receipt summary','th','td']);checkPalette(results.record_focus,'dining focused record receipt');await page.screenshot({path:`${output}/dining-record-focus.png`});await page.locator('[data-focus-close]').click();
  }
  report.checks.push(`${name}: actual standalone, united header controls/link/origin badge, fanned form and focused form colors meet 4.5:1; the same unsent form node/key survives movement.`);
 }
 assert.deepEqual(report.writes,[]);assert.deepEqual(report.errors,[]);assert.equal(await manifest(),report.manifest_sha256);report.passed=true;
}catch(error){report.passed=false;report.failure=String(error);process.exitCode=1;await page.screenshot({path:`${output}/failure.png`}).catch(()=>{});}
finally{await browser.close();report.finished_at=new Date().toISOString();await writeFile(`${output}/report.json`,JSON.stringify(report,null,2));console.log(JSON.stringify(report));}
