// Browser smoke check for the homepage -> setup guide / real WASM proof journey.
// Run after the Astro build. CI installs a pinned Playwright in a temporary prefix.
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { mkdir, writeFile } from 'node:fs/promises';
import { spawn } from 'node:child_process';
import { resolve } from 'node:path';

const require = createRequire(import.meta.url);
const { chromium } = require(process.env.AI2RULES_BROWSER_MODULE || 'playwright');
const out = resolve('browser-qa');
await mkdir(out, { recursive: true });
const origin = 'http://127.0.0.1:4321';
const server = spawn('python3', ['-m', 'http.server', '4321', '--bind', '127.0.0.1',
  '--directory', resolve('dist')], { stdio: 'ignore' });
let browser;
const results = [];
const failures = [];

async function snapshot(page, name) {
  await page.screenshot({ path: resolve(out, name + '.png'), fullPage: true });
  // Optional viewport JPEGs allow remote reviewers to inspect images via job logs.
  if (process.env.AI2RULES_INLINE_SCREENSHOTS === '1') {
    const jpeg = await page.screenshot({ type: 'jpeg', quality: 65 });
    console.log('BROWSER_SCREENSHOT:' + name + ':' + jpeg.toString('base64'));
  }
}
async function noHorizontalOverflow(page, label) {
  const geometry = await page.evaluate(() => ({
    viewport: innerWidth, document: document.documentElement.scrollWidth,
    overflowing: [...document.querySelectorAll('main, main a, .kp-panel, .kp-col')]
      .filter(el => el.getBoundingClientRect().right > innerWidth + 1)
      .map(el => el.className || el.tagName).slice(0, 12)
  }));
  assert(geometry.document <= geometry.viewport + 1,
    label + ' overflows: ' + JSON.stringify(geometry));
}
try {
  let ready = false;
  for (let i = 0; i < 100; i++) {
    try { if ((await fetch(origin)).ok) { ready = true; break; } } catch {}
    await new Promise(r => setTimeout(r, 100));
  }
  assert(ready, 'local static server did not become ready');
  browser = await chromium.launch({ headless: true });
  for (const [name, width, height] of [['mobile', 390, 844], ['desktop', 1440, 1000]]) {
    const page = await browser.newPage({ viewport: { width, height }, reducedMotion: 'reduce' });
    const errors = [];
    page.on('pageerror', e => errors.push(e.message));
    try {
      assert.equal((await page.goto(origin, { waitUntil: 'networkidle' })).status(), 200);
      await page.evaluate(() => document.fonts.ready);
      await snapshot(page, name + '-home');
      await noHorizontalOverflow(page, name + ' homepage');
      assert.equal(await page.locator('main a.button').count(), 1);
      const primary = page.getByRole('link', { name: 'Try the kernel in your browser' });
      const box = await primary.boundingBox();
      assert(box && box.y >= 0 && box.y + box.height <= height,
        name + ': primary CTA is outside the first viewport: ' + JSON.stringify(box));
      assert.match(await page.title(), /Execution governance/);
      assert.equal(await page.locator('link[rel="canonical"]').getAttribute('href'),
        'https://ai2rules.dev/');
      const description = await page.locator('meta[name="description"]').getAttribute('content');
      assert.equal(await page.locator('meta[property="og:description"]').getAttribute('content'), description);
      const jsonLd = JSON.parse(await page.locator('script[type="application/ld+json"]').textContent());
      assert.equal(jsonLd.description, description);

      await page.getByRole('link', { name: 'Install for Claude Code CLI' }).click();
      assert.equal(new URL(page.url()).hash, '#cli-setup');
      assert.match(await page.locator('main pre').first().innerText(), /harness init/);
      await page.getByRole('link', { name: /follow the manual Claude Code CLI guide/ }).click();
      await page.waitForURL('**/blog/govern-your-coding-agent-in-one-command/');
      assert.equal(await page.locator('h1').count(), 1);

      await page.goto(origin, { waitUntil: 'networkidle' });
      await primary.click();
      await page.waitForURL('**/playground/');
      await page.waitForFunction(() =>
        document.querySelector('.kp-ver')?.textContent.startsWith('v') &&
        document.querySelectorAll('.kp-col').length === 4);
      assert.equal(await page.locator('.kp-err').count(), 0);
      await snapshot(page, name + '-playground');
      await noHorizontalOverflow(page, name + ' playground');

      const fetchAllowed = page.locator('.kp-col-Allow .kp-chip b').filter({ hasText: /^fetch_web$/ });
      assert.equal(await fetchAllowed.count(), 1, 'clean fetch_web should be allowed');
      await page.getByRole('button', { name: 'Tainted', exact: true }).click();
      assert.equal(await page.locator('.kp-col-Deny .kp-chip b')
        .filter({ hasText: /^fetch_web$/ }).count(), 1, 'tainted fetch_web should be denied');
      await page.locator('input[data-tool="fetch_web"]').uncheck();
      assert.equal(await page.locator('.kp-col-Absent .kp-chip b')
        .filter({ hasText: /^fetch_web$/ }).count(), 1, 'disabled fetch_web should be absent');
      assert.deepEqual(errors, [], 'browser script errors');
      results.push({ viewport: name, size: [width, height], result: 'PASS',
        checks: ['no horizontal overflow', 'primary CTA in first viewport',
          'setup and guide navigation', 'canonical and metadata', 'WASM loaded',
          'fetch_web ALLOW -> DENY -> ABSENT', 'no script errors'] });
    } catch (error) {
      failures.push(name + ': ' + error.message);
      await snapshot(page, name + '-failure').catch(() => {});
      results.push({ viewport: name, result: 'FAIL', error: error.message, scriptErrors: errors });
    } finally { await page.close(); }
  }
  const robots = await (await fetch(origin + '/robots.txt')).text();
  assert.match(robots, /Sitemap: https:\/\/ai2rules\.dev\/sitemap-index\.xml/);
  assert.equal((await fetch(origin + '/sitemap-index.xml')).status, 200);
  assert.deepEqual(failures, [], 'browser journey failed');
} finally {
  console.log('BROWSER_QA_RESULTS:' + JSON.stringify(results));
  await writeFile(resolve(out, 'results.json'), JSON.stringify(results, null, 2));
  if (browser) await browser.close();
  server.kill('SIGTERM');
}
