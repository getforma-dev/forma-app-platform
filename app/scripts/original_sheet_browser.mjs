// PR42 original-file qualification on an owned disposable loopback stand.
// Synthetic member sign-in; ordinary upload/storage/parser/read routes, no model.
import assert from 'node:assert/strict';
import {readFile,writeFile,mkdir} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {resolve} from 'node:path';
const args=Object.fromEntries(process.argv.slice(2).map(a=>a.replace(/^--/,'').split('=')));
const origin=args.origin||'http://127.0.0.1:4701';
assert.match(origin,/^http:\/\/127\.0\.0\.1:(?!400[1-9]$|4010$)\d+$/);
const out=resolve(args.output||'.cache/reports/original-sheet-browser');await mkdir(out,{recursive:true});
const report={scope:'Owned loopback stand, synthetic member identity; actual upload/parser/read/UI. No provider or hosted claim.',started_at:new Date().toISOString(),checks:[],errors:[]};
const {chromium}=await import(process.env.PLAYWRIGHT_MODULE||'/tmp/audit-browser/node_modules/playwright/index.mjs');
const browser=await chromium.launch({channel:'chrome',headless:true});
const context=await browser.newContext({viewport:{width:1440,height:1000},reducedMotion:'reduce'});const page=await context.newPage();
page.on('pageerror',e=>report.errors.push(e.message));
let appMutations=0;page.on('request',r=>{if(r.method()==='POST'&&/\/(apps|data|compose)$/.test(new URL(r.url()).pathname))appMutations++;});
const check=text=>report.checks.push(text);
const dialog=()=>page.locator('.bx-modal');
const csvValue=s=>`"${s.replaceAll('"','""')}"`;
const csv=Buffer.from(['  header  ,text,...'+Array.from({length:29},(_,i)=>`,c${i+4}`).join(''),'',...Array.from({length:104},(_,i)=>[String(i+1).padStart(4,'0'),'<img src=x onerror=alert(1)>',i===0?'line one\nline two':`line ${i+1}`,...Array.from({length:29},(_,j)=>j===28?`last-${i+1}`:`${i}-${j}`)].map(csvValue).join(','))].join('\r\n')+'\r\n');
const upload=async(name,buffer)=>{
 await writeFile(resolve(out,'pending-upload.json'),JSON.stringify({name,sha256:createHash('sha256').update(buffer).digest('hex')}));
 const response=page.waitForResponse(r=>r.request().method()==='POST'&&/^\/v1\/boxes\/[^/]+\/files$/.test(new URL(r.url()).pathname));
 await page.locator('[data-empty-upload]').setInputFiles({name,mimeType:'application/octet-stream',buffer});
 const r=await response;assert.equal(r.status(),201);const result=await r.json();
 await writeFile(resolve(out,'last-upload.json'),JSON.stringify(result));await writeFile(resolve(out,'pending-upload.json'),'null');
 const file=page.locator(`.bx-sheet[data-file="${result.file.file_id}"]`);await file.waitFor();return{file,result};
};
try{
 for(let attempt=0;attempt<90;attempt++){try{const r=await fetch(origin+'/canvas');if(r.status===200)break;}catch{}if(attempt===89)throw Error('Owned stand not ready');await new Promise(r=>setTimeout(r,2000));}
 await page.goto(origin+'/canvas');await page.locator('[data-empty-upload][data-empty-upload-wired]').waitFor({timeout:1000}).catch(()=>{});
 await page.locator('#cv-canvas.is-canvas').waitFor();
 const {file,result}=await upload('original <img>.csv',csv);
 assert.equal(appMutations,0);check('Upload directly places one file icon with no app/source-registration request.');
 await page.evaluate(()=>window.getSelection()?.removeAllRanges());await page.screenshot({path:resolve(out,'file-icon.png'),fullPage:true});
 await file.click();assert.equal(await dialog().count(),0);check('A single click selects the file without opening it.');
 await file.dblclick();await page.getByText(/Original file · rows 1–50 of 105/).waitFor({timeout:30000});
 assert.equal(await dialog().locator('tbody tr').count(),50);
 assert.equal(await dialog().locator('tbody tr').first().locator('td').first().textContent(),'  header  ');
 assert.equal(await dialog().locator('tbody tr').nth(1).locator('td').nth(0).textContent(),'0001');
 assert.equal(await dialog().locator('tbody tr').nth(1).locator('td').nth(1).textContent(),'<img src=x onerror=alert(1)>');
 assert.equal(await dialog().locator('tbody img,script').count(),0);assert.equal(await dialog().locator('h2 img').count(),0);
 await page.evaluate(()=>window.getSelection()?.removeAllRanges());await page.screenshot({path:resolve(out,'original-cells.png'),fullPage:true});
 await page.getByRole('button',{name:'Next 50 rows',exact:true}).click();await page.getByText(/rows 51–100 of 105/).waitFor();
 await page.getByRole('button',{name:'Next 50 rows',exact:true}).click();await page.getByText(/rows 101–105 of 105/).waitFor();
 assert.equal(await dialog().locator('tbody tr').last().locator('td').last().textContent(),'last-104');
 assert.equal(await page.getByRole('button',{name:'Next 50 rows',exact:true}).isDisabled(),true);
 const bytes=await context.request.get(origin+result.file.content_url);assert.equal(bytes.status(),200);assert.deepEqual(await bytes.body(),csv);
 await page.getByRole('button',{name:'Raw CSV',exact:true}).click();await page.getByText('Original CSV text · all content, including blank lines and quoting',{exact:true}).waitFor();
 assert.equal(await page.locator('pre.bx-text').textContent(),csv.toString());
 check('All105 logical rows/all32 columns and whitespace/multiline/leading-zero cells are available; markup stays text; raw CSV and download preserve original bytes.');
 await page.evaluate(()=>window.getSelection()?.removeAllRanges());await page.screenshot({path:resolve(out,'original-csv.png'),fullPage:true});await page.keyboard.press('Escape');
 await file.focus();await page.keyboard.press('Enter');await dialog().waitFor();await page.keyboard.press('Escape');
 await file.focus();await page.keyboard.press('Space');await dialog().waitFor();await page.keyboard.press('Escape');check('Enter and Space open the original; Escape closes and returns focus.');
 const bounds=await file.boundingBox();assert.ok(bounds);const x=bounds.x+bounds.width/2,y=bounds.y+bounds.height/2;
 await page.mouse.move(x,y);await page.mouse.down();await page.mouse.move(x+60,y+35,{steps:8});await page.mouse.move(x,y,{steps:8});await page.mouse.up();assert.equal(await dialog().count(),0);
 check('Dragging away and back to the starting point does not open the viewer.');
 await page.reload();await file.waitFor({timeout:30000});
 const storage=await page.evaluate(()=>Object.entries(localStorage).filter(([key])=>key.startsWith('forma.canvas.sheets.v2:')));
 assert.equal(storage.length,1);assert.ok(!storage[0][1].includes('original'));assert.ok(!storage[0][1].includes('onerror'));
 await file.dblclick();await page.getByText(/Original file · rows 1–50 of 105/).waitFor();await page.keyboard.press('Escape');
 check('Reload restores the file from authorized metadata; browser storage contains scoped references/positions, no filenames or cells.');
 // Delay actual authorized responses to exercise supersession and close.
 const delayNext=async()=>{
  let release,received,finished;const gate=new Promise(r=>{release=r;}),got=new Promise(r=>{received=r;}),done=new Promise(r=>{finished=r;});
  await page.route(`**/files/${result.file.file_id}/sheet`,async route=>{const response=await route.fetch();received();await gate;await route.fulfill({response});finished();},{times:1});
  return{release,got,done};
 };
 const delayed=await delayNext();await file.dblclick();await delayed.got;
 await page.getByRole('button',{name:'Raw CSV',exact:true}).click();await page.getByText('Original CSV text · all content, including blank lines and quoting',{exact:true}).waitFor();
 delayed.release();await delayed.done;await page.waitForTimeout(100);
 assert.equal(await dialog().locator('table').count(),0);assert.equal(await page.locator('pre.bx-text').textContent(),csv.toString());
 const closing=await delayNext();await page.getByRole('button',{name:'Table',exact:true}).click();await closing.got;await page.keyboard.press('Escape');closing.release();await closing.done;await page.waitForTimeout(100);assert.equal(await dialog().count(),0);
 check('Late actual table responses cannot overwrite a newer raw view or remount a closed dialog.');
 const workbook=await upload('two-sheets.xlsx',await readFile('parser/fixtures/valid.xlsx'));
 await workbook.file.dblclick();await page.getByRole('combobox',{name:'Workbook sheet'}).waitFor();
 assert.equal(await dialog().locator('select option').count(),2);assert.ok((await dialog().textContent()).includes('9007199254740993'));assert.ok((await dialog().textContent()).includes('-1.25e2'));
 await page.getByRole('combobox',{name:'Workbook sheet'}).selectOption('1');await page.getByRole('combobox',{name:'Workbook sheet'}).waitFor();assert.equal(await page.getByRole('combobox',{name:'Workbook sheet'}).inputValue(),'1');
 await page.getByText(/Original file · rows/).waitFor();assert.ok(!(await dialog().textContent()).includes('-1.25e2'));
 await page.evaluate(()=>window.getSelection()?.removeAllRanges());await page.screenshot({path:resolve(out,'original-workbook.png'),fullPage:true});await page.keyboard.press('Escape');
 check('Workbook selection exposes both original worksheets and exact large/exponent lexical values.');
 const formula=await upload('formulas.xlsx',await readFile('parser/fixtures/formula.xlsx'));
 await formula.file.dblclick();await page.getByText(/This original file cannot be displayed/).waitFor({timeout:30000});
 assert.equal(await dialog().locator('table').count(),0);assert.equal(await page.getByRole('link',{name:'Download original file',exact:true}).count(),1);
 check('A formula-bearing workbook stays a file; refusal explains limits and retains original download, without fabricated/sample cells.');
 await page.keyboard.press('Escape');
 for(const width of [390,320]){
  await page.setViewportSize({width,height:844});await page.reload();await file.waitFor();
  // Fitting completes after all authorized metadata reads settle.
  await page.waitForTimeout(600);
  const icon=await file.boundingBox();assert.ok(icon&&icon.x>=0&&icon.x+icon.width<=width&&icon.y>=0&&icon.y<844);
  await page.evaluate(()=>window.getSelection()?.removeAllRanges());await page.screenshot({path:resolve(out,`file-icon-${width}.png`),fullPage:true});
  await file.focus();await page.keyboard.press('Enter');await page.getByText(/Original file · rows 1–50 of 105/).waitFor();
  const box=await dialog().boundingBox();assert.ok(box&&box.x>=0&&box.x+box.width<=width&&box.y>=0&&box.y+box.height<=844);
  await page.getByRole('button',{name:'Next 50 rows',exact:true}).click();await page.getByText(/rows 51–100 of 105/).waitFor();
  const close=await page.getByRole('button',{name:'Close (Esc)',exact:true}).boundingBox();assert.ok(close&&close.x>=0&&close.x+close.width<=width);
  await page.evaluate(()=>window.getSelection()?.removeAllRanges());await page.screenshot({path:resolve(out,`original-cells-${width}.png`),fullPage:true});
  await page.getByRole('button',{name:'Close (Esc)',exact:true}).click();assert.equal(await dialog().count(),0);
 }
 check('390px and320px: restored file icon and modal remain in viewport; pagination and close are reachable.');
 await file.getByRole('button',{name:'Remove original <img>.csv from the canvas',exact:true}).focus();await page.keyboard.press('Enter');assert.equal(await file.count(),0);assert.equal(await dialog().count(),0);check('Keyboard removal removes only the placement and does not open a viewer or delete the original.');
 assert.deepEqual(await (await context.request.get(origin+result.file.content_url)).body(),csv);
 assert.equal(appMutations,0);assert.deepEqual(report.errors,[]);report.passed=true;
}catch(error){report.passed=false;report.failure=String(error);await page.evaluate(()=>window.getSelection()?.removeAllRanges());await page.screenshot({path:resolve(out,'failure.png'),fullPage:true});throw error;}
finally{report.completed_at=new Date().toISOString();await writeFile(resolve(out,'report.json'),JSON.stringify(report,null,2));await browser.close();}
