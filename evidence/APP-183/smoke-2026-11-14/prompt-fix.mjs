// Visual check of the name prompt's banner position after the CSS fix (dev stand, Diego has no name).
const {chromium} = await import(process.env.PLAYWRIGHT_MODULE || 'playwright');
const browser = await chromium.launch({channel: 'chrome', headless: true});
const page = await (await browser.newContext({viewport: {width: 1366, height: 900}})).newPage();
const errors = []; page.on('pageerror', e => errors.push(e.message));
await page.goto('http://127.0.0.1:8701/canvas');
const prompt = page.locator('.cv-name-prompt');
await prompt.waitFor();
const box = await prompt.boundingBox();
await page.screenshot({path: process.argv[2] + '/name-prompt-banner.png'});
console.log(JSON.stringify({box, errors}));
await browser.close();
