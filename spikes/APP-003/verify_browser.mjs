import { chromium, firefox, webkit } from './admin/node_modules/playwright/index.mjs';
import { spawn } from 'node:child_process';
import { readFileSync, writeFileSync } from 'node:fs';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import assert from 'node:assert/strict';
const root = dirname(fileURLToPath(import.meta.url));
const evidence = resolve(root, '../../evidence/APP-003');
const processLog = [];
const server = spawn(resolve(root, 'target/release/genui-canvas-spike'), [], { cwd: root, env: { ...process.env, FORMA_PORT: '0' }, stdio: ['ignore', 'pipe', 'pipe'] });
let output = '';
let origin;
const report = { scope: 'Published 3.2 native consumer adoption and synthetic interactions; not cold-client parity, persistence, auth or client SLO proof', results: [], passed: false };
try {
origin = await new Promise((accept, reject) => {
  const timer = setTimeout(() => reject(new Error('owned server startup deadline')), 20000);
  server.on('exit', code => { clearTimeout(timer); reject(new Error(`server exited ${code}`)); });
  server.stdout.on('data', chunk => { output += chunk.toString(); processLog.push(chunk.toString()); const match = output.match(/FORMA_LISTENING (http:\/\/\S+)/); if (match) { clearTimeout(timer); accept(match[1]); } });
  server.stderr.on('data', chunk => processLog.push(chunk.toString()));
});
  const response = await fetch(origin);
  const html = await response.text();
  assert.equal(response.status, 200);
  assert.equal(response.headers.get('cache-control'), 'no-store');
  assert.equal(response.headers.get('x-content-type-options'), 'nosniff');
  const nonce = response.headers.get('content-security-policy').match(/'nonce-([^']+)'/)[1];
  assert.ok(html.includes(`nonce="${nonce}"`));
  const second = await fetch(origin);
  assert.notEqual(second.headers.get('content-security-policy').match(/'nonce-([^']+)'/)[1], nonce);
  const stateMatch = html.match(/<script[^>]*data-island-state[^>]*>([\s\S]*?)<\/script>/);
  assert.ok(stateMatch);
  report.response = { native_html_bytes: Buffer.byteLength(html), state_envelope_utf8_bytes: Buffer.byteLength(stateMatch[1]), no_store: true, fresh_nonce: true, full_csp: response.headers.get('content-security-policy') };
  assert.ok(report.response.state_envelope_utf8_bytes < 1048576);
  const expectedKinds = ['summary','table','detail','timeline','chart','filter','form','note','status'];
  for (const [engineName, engine] of Object.entries({ chromium, firefox, webkit })) {
    const browser = await engine.launch();
    try {
      const states = [];
      for (const state of ['normal','empty','nested-empty','long','rows500']) {
        const page = await browser.newPage();
        const errors = [];
        page.on('pageerror', error => errors.push(error.message));
        await page.addInitScript(() => {
          globalThis.__policyViolations = [];
          document.addEventListener('securitypolicyviolation', e => globalThis.__policyViolations.push(e.violatedDirective));
        });
        let release;
        const gate = new Promise(done => { release = done; });
        await page.route('**/home.*.js', async route => { await gate; await route.continue(); });
        await page.goto(`${origin}/?state=${state}`, { waitUntil: 'commit' });
        await page.locator('.canvas').waitFor();
        const initial = await page.evaluate(() => {
          const root = document.querySelector('.canvas'); const walk = document.createTreeWalker(root, NodeFilter.SHOW_ALL); const nodes = [root]; while (walk.nextNode()) nodes.push(walk.currentNode);
          globalThis.__originalNodes = nodes; globalThis.__root = root; globalThis.__writes = [];
          globalThis.__observer = new MutationObserver(records => globalThis.__writes.push(...records.map(r => ({ type:r.type, attribute:r.attributeName }))));
          globalThis.__observer.observe(root, { childList:true, attributes:true, characterData:true, subtree:true });
          return { nodes:nodes.length, kinds:[...root.querySelectorAll('.card')].map(n => n.dataset.kind), state_utf8_bytes:new TextEncoder().encode(document.querySelector('script[data-island-state]').textContent).length };
        });
        release(); await page.waitForLoadState('load');
        await page.evaluate(async () => { const scripts=[...document.querySelectorAll('script[type="module"][src]')]; if (!scripts.length) throw new Error('No activation module'); await Promise.all(scripts.map(script => import(script.src))); });
        const adoption = await page.evaluate(async () => {
          const root = document.querySelector('.canvas'); const walk = document.createTreeWalker(root, NodeFilter.SHOW_ALL); const nodes=[root]; while(walk.nextNode()) nodes.push(walk.currentNode);
          const writes=[...globalThis.__writes,...globalThis.__observer.takeRecords().map(r=>({type:r.type,attribute:r.attributeName}))]; globalThis.__observer.disconnect();
          const control=[]; const observer=new MutationObserver(records=>control.push(...records)); observer.observe(root,{attributes:true}); root.setAttribute('data-observer-control','detected'); await Promise.resolve(); observer.disconnect(); root.removeAttribute('data-observer-control');
          return { same_root:root===globalThis.__root, same_nodes:nodes.length===globalThis.__originalNodes.length&&nodes.every((n,i)=>n===globalThis.__originalNodes[i]), initial_writes:writes, observer_positive_control:control.length===1 };
        });
        assert.deepEqual(initial.kinds, state==='empty'?[]:expectedKinds);
        assert.ok(initial.state_utf8_bytes < 1048576);
        assert.equal(adoption.same_root,true); assert.equal(adoption.same_nodes,true); assert.deepEqual(adoption.initial_writes,[]); assert.equal(adoption.observer_positive_control,true);
        let interactions = null;
        if (state === 'normal') {
          const geometry = await page.locator('svg rect').evaluateAll(nodes=>nodes.map(n=>({namespace:n.namespaceURI,x:n.getAttribute('x'),width:n.getAttribute('width'),height:n.getAttribute('height'),title:n.querySelector('title')?.textContent,titleNamespace:n.querySelector('title')?.namespaceURI})));
          assert.equal(geometry.length,12); assert.ok(geometry.every(n=>n.namespace==='http://www.w3.org/2000/svg'&&n.titleNamespace===n.namespace&&Number.isFinite(Number(n.height))&&n.width==='12.5'));
          assert.equal(geometry[0].x,'0.5'); assert.equal(geometry[0].title,'bucket 0: server value');
          assert.equal(await page.evaluate(()=>globalThis.pwned), undefined);
          assert.ok((await page.locator('.note').textContent()).includes('<script>globalThis.pwned=true</script>'));
          await page.getByLabel('Observation', { exact:true }).fill('Keyboard observation 東京');
          await page.getByLabel('Decision', { exact:true }).selectOption('accepted');
          await page.getByRole('button',{name:'Save observation',exact:true}).focus();
          const command = page.waitForResponse(r=>r.url()===`${origin}/commands`);
          await page.keyboard.press('Enter'); assert.equal((await command).status(),200);
          await page.locator('output').filter({hasText:'Synthetic observation accepted'}).waitFor();
          await page.getByLabel('Linked destination',{exact:true}).focus();
          const slow = page.waitForResponse(r=>r.url().includes('/linked?')&&r.url().includes('epoch=1'));
          const fast = page.waitForResponse(r=>r.url().includes('/linked?')&&r.url().includes('epoch=2'));
          await page.getByLabel('Linked destination',{exact:true}).selectOption('unmatched');
          await page.getByLabel('Linked destination',{exact:true}).selectOption('all');
          await fast; await slow;
          await page.waitForFunction(()=>document.querySelector('.canvas').dataset.epoch==='2');
          const linked = await page.locator('.linked-result').allTextContents();
          assert.equal(linked.length,6); assert.ok(linked.every(text=>text==='All: 1,140,000 bytes'));
          assert.equal(await page.locator('.canvas').getAttribute('data-filter'),'all');
          await page.getByRole('button',{name:'Focus view',exact:true}).nth(6).focus();
          await page.keyboard.press('Enter');
          assert.equal(await page.evaluate(()=>document.activeElement?.id),'view-form');
          assert.equal(await page.locator('.canvas').getAttribute('data-selected'),'view-form');
          const accessibility = await page.locator('.canvas').ariaSnapshot();
          assert.ok(accessibility.includes('Observation')); assert.ok(accessibility.includes('Decision')); assert.ok(accessibility.includes('Linked destination')); assert.ok(accessibility.includes('Synthetic observation accepted'));
          if (engineName === 'chromium') { await page.screenshot({path:resolve(evidence,'canvas-chromium.png'),fullPage:true}); }
          const preserved = await page.evaluate(()=>document.querySelector('.canvas')===globalThis.__root && globalThis.__originalNodes.every(n=>n.isConnected));
          assert.equal(preserved,true);
          interactions = { form_keyboard_submit:true, linked_filter_stale_epoch_refused:true, keyboard_focus:'view-form', original_nodes_remain_connected:true, numeric_svg:geometry, accessible_state:accessibility, hostile_literal:true };
        }
        const policies=await page.evaluate(()=>globalThis.__policyViolations); assert.deepEqual(errors,[]); assert.deepEqual(policies,[]);
        states.push({state,initial,adoption,interactions,page_errors:errors,csp_violations:policies,passed:true}); await page.close();
      }
      const nojs = await browser.newPage({javaScriptEnabled:false});
      await nojs.goto(origin); await nojs.getByLabel('Observation',{exact:true}).fill('Before activation'); await nojs.getByLabel('Decision',{exact:true}).selectOption('review');
      await Promise.all([nojs.waitForURL('**/commands'),nojs.getByRole('button',{name:'Save observation',exact:true}).click()]);
      assert.ok((await nojs.locator('body').textContent()).includes('Synthetic observation accepted')); await nojs.close();
      report.results.push({engine:engineName,version:browser.version(),states,no_javascript_form_post:true,passed:true});
    } finally { await browser.close(); }
  }
  report.passed=true;
} catch (error) { report.error=String(error.stack || error); throw error; }
finally {
  if (server.exitCode === null && server.signalCode === null) { await new Promise(done => { const timer=setTimeout(()=>server.kill('SIGKILL'),5000); server.once('exit',()=>{clearTimeout(timer);done();}); server.kill('SIGTERM'); }); }
  writeFileSync(resolve(evidence,'browser.json'),JSON.stringify(report,null,2)+'\n');
  writeFileSync(resolve(evidence,'server-output.txt'),processLog.join(''));
}
console.log(JSON.stringify({passed:report.passed,engines:report.results.map(x=>x.engine)}));
