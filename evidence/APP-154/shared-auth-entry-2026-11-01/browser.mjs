// Browser regression over actual native PageResponse fixtures. Every request
// stays on an owned loopback HTTPS server; no real provider/account is used.
import assert from 'node:assert/strict';
import { readFile, readdir, writeFile, mkdtemp, rm } from 'node:fs/promises';
import { createServer } from 'node:https';
import { execFileSync } from 'node:child_process';
import { tmpdir } from 'node:os';
import { basename, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

const [fixturePath, outputPath] = process.argv.slice(2);
assert.ok(fixturePath && outputPath, 'usage: browser.mjs <native.json> <report.json>');
assert.ok(process.env.PLAYWRIGHT_MODULE, 'set PLAYWRIGHT_MODULE to an existing test installation');
const { chromium } = await import(pathToFileURL(resolve(process.env.PLAYWRIGHT_MODULE)).href);
const fixtures = JSON.parse(await readFile(fixturePath, 'utf8'));
const dist = resolve('app/admin/dist');
const assets = new Set(await readdir(dist));
const app = 'https://127.0.0.1:7697';
const provider = 'https://localhost:7697'; // Same trusted test config as the native fixture.
const foreign = 'https://127.0.0.2:7697';
const key = '11111111-1111-4111-8111-111111111111.22222222-2222-4222-8222-222222222222';
const start = `/auth/shared/${key}/start`;
const browser = await chromium.launch({ channel: 'chrome', headless: true });
const results = [];
const certificate = await mkdtemp(resolve(tmpdir(), 'forma-shared-entry-'));
execFileSync('openssl', ['req', '-x509', '-newkey', 'rsa:2048', '-nodes', '-keyout', resolve(certificate, 'key.pem'), '-out', resolve(certificate, 'cert.pem'), '-days', '1', '-subj', '/CN=localhost'], { stdio: 'ignore' });
let handler;
const server = createServer({ key: await readFile(resolve(certificate, 'key.pem')), cert: await readFile(resolve(certificate, 'cert.pem')) }, (request, response) => { void handler(request, response); });
await new Promise((resolve, reject) => { server.once('error', reject); server.listen(7697, '127.0.0.1', resolve); });
try {
  for (const test of [
    { name: 'self_only_reproduces_blocked_redirect', fixture: 'no_provider', intent: 'sign_up', target: provider, allowed: false },
    { name: 'signup_reaches_exact_provider', fixture: 'anonymous', intent: 'sign_up', target: provider, allowed: true },
    { name: 'signin_reaches_exact_provider', fixture: 'anonymous', intent: 'sign_in', target: provider, allowed: true },
    { name: 'pending_verification_signin_reaches_provider', fixture: 'verification_pending', intent: 'sign_in', target: provider, allowed: true },
    { name: 'foreign_redirect_stays_blocked', fixture: 'anonymous', intent: 'sign_up', target: foreign, allowed: false },
  ]) {
    const context = await browser.newContext({ viewport: { width: 1000, height: 800 }, serviceWorkers: 'block', ignoreHTTPSErrors: true });
    const page = await context.newPage();
    const observations = { posts: 0, providerGets: 0, policyErrors: 0, pageErrors: 0, unexpected: [] };
    page.on('console', message => { if (/form-action/i.test(message.text()) && /violat|refus|block/i.test(message.text())) observations.policyErrors++; });
    page.on('pageerror', () => { observations.pageErrors++; });
    handler = async (request, response) => {
      const url = new URL(request.url, `https://${request.headers.host}`);
      if (url.origin === app && url.pathname === `/s/${key}` && request.method === 'GET') {
        const fixture = fixtures[test.fixture];
        response.writeHead(200, fixture.headers); response.end(fixture.body);
      } else if (url.origin === app && url.pathname === start && request.method === 'POST') {
        const chunks = []; for await (const chunk of request) chunks.push(chunk);
        const fields = new URLSearchParams(Buffer.concat(chunks).toString());
        assert.deepEqual([...fields.keys()].sort(), ['csrf', 'intent']);
        assert.equal(fields.get('csrf'), 'synthetic-csrf');
        assert.equal(fields.get('intent'), test.intent);
        observations.posts++;
        response.writeHead(303, { location: `${test.target}/t/synthetic/authorize`, 'cache-control': 'no-store' }); response.end();
      } else if ([provider, foreign].includes(url.origin) && url.pathname === '/t/synthetic/authorize' && request.method === 'GET') {
        observations.providerGets++;
        response.writeHead(200, { 'content-type': 'text/html' }); response.end('<!doctype html><title>Synthetic provider</title><h1>Provider entry reached</h1>');
      } else if (url.origin === app && assets.has(basename(url.pathname)) && request.method === 'GET') {
        const name = basename(url.pathname);
        const type = name.endsWith('.js') ? 'application/javascript' : name.endsWith('.css') ? 'text/css' : 'font/woff2';
        response.writeHead(200, { 'content-type': type }); response.end(await readFile(resolve(dist, name)));
      } else if (url.pathname === '/favicon.ico') { response.writeHead(204); response.end(); }
      else {
        observations.unexpected.push({ method: request.method, origin: url.origin, path: url.pathname });
        response.writeHead(404); response.end();
      }
    };
    await page.goto(`${app}/s/${key}`, { waitUntil: 'networkidle' });
    // Startup has loaded the actual published island bundle. The native form
    // must remain usable after adoption as well as without JavaScript.
    assert.equal(await page.locator('button[value="sign_up"]').count(), test.fixture === 'verification_pending' ? 0 : 1);
    await page.locator(`button[value="${test.intent}"]`).click({ noWaitAfter: true });
    const deadline = Date.now() + 5000;
    while (Date.now() < deadline && (test.allowed ? observations.providerGets === 0 : observations.policyErrors === 0)) {
      await new Promise(resolve => setTimeout(resolve, 25));
    }
    assert.equal(observations.posts, 1, test.name);
    assert.equal(observations.providerGets, test.allowed ? 1 : 0, JSON.stringify({ test: test.name, observations, url: page.url() }));
    assert.equal(observations.pageErrors, 0, test.name);
    assert.deepEqual(observations.unexpected, [], test.name);
    if (test.allowed) {
      await page.getByRole('heading', { name: 'Provider entry reached', exact: true }).waitFor();
      assert.equal(new URL(page.url()).origin, provider);
      assert.equal(observations.policyErrors, 0);
    } else {
      assert.ok(observations.policyErrors > 0, test.name);
      assert.equal(new URL(page.url()).origin, app);
    }
    results.push({ name: test.name, passed: true, ...observations });
    await context.close();
  }
  await writeFile(outputPath, JSON.stringify({ browser: browser.version(), nativeFixture: 'shared_auth_entry_keeps_the_exact_provider_form_policy', syntheticNetworkOnly: true, results }, null, 2) + '\n');
  console.log(`${results.length} native-policy browser cases passed`);
} finally { await browser.close(); await new Promise(resolve => server.close(resolve)); await rm(certificate, { recursive: true, force: true }); }
