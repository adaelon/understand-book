// Build the real component through the Web Vite config, then inspect its served output.
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { writeFile } from 'node:fs/promises';
import { fileURLToPath, pathToFileURL } from 'node:url';

const web = new URL('../../../packages/web/', import.meta.url);
const require = createRequire(new URL('package.json', web));
const { chromium } = require('@playwright/test');
const { build, preview } = await import(new URL('dist/node/index.js', pathToFileURL(require.resolve('vite/package.json'))));
const outDir = fileURLToPath(new URL('../../../tmp/yy2-production', import.meta.url));
const config = {
  root: fileURLToPath(web),
  configFile: fileURLToPath(new URL('vite.config.ts', web)),
  base: '/yy2/',
  build: { outDir, emptyOutDir: false, copyPublicDir: false,
    rollupOptions: { input: fileURLToPath(new URL('yeye-preview.html', web)) } },
};
await build(config);
const server = await preview({ ...config, preview: { host: '127.0.0.1', port: 6194, strictPort: true } });
const browser = await chromium.launch();
const report = { base: '/yy2/', viewports: [], errors: [] };
try {
  for (const [width, deviceScaleFactor] of [[1440, 1], [390, 2]]) {
    const context = await browser.newContext({ viewport: { width, height: 1000 }, deviceScaleFactor });
    const page = await context.newPage();
    page.on('pageerror', error => report.errors.push(error.message));
    const responses = [];
    page.on('response', response => {
      if (response.request().resourceType() === 'image') responses.push({ url: response.url(), status: response.status() });
    });
    let releaseImages;
    const imagesHeld = new Promise(resolve => { releaseImages = resolve; });
    await page.route('**/*.webp', async route => { await imagesHeld; await route.continue(); });
    await page.goto('http://127.0.0.1:6194/yy2/yeye-preview.html', { waitUntil: 'domcontentloaded' });
    await page.locator('.yeye-mascot').first().waitFor();
    await page.waitForFunction(() => getComputedStyle(document.querySelector('.yeye-mascot')).display === 'inline-block');
    const measure = () => page.locator('[data-sample], figcaption, footer').evaluateAll(elements => elements.map(el => {
      const r = el.getBoundingClientRect();
      return { sample: el.getAttribute('data-sample') ?? el.textContent, x: r.x, y: r.y, width: r.width, height: r.height };
    }));
    const before = await measure();
    assert(await page.locator('img').evaluateAll(images => images.every(image => !image.complete)), 'images must be held for the footprint check');
    releaseImages();
    await page.locator('img').evaluateAll(images => Promise.all(images.map(image => image.decode())));
    const after = await measure();
    assert.deepEqual(after, before, 'image arrival moved reserved space or neighboring content');
    assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), 'horizontal overflow');
    const assets = await page.locator('[data-sample]').evaluateAll(elements => elements.map(el => {
      const img = el.querySelector('img');
      return { sample: el.getAttribute('data-sample'), url: img.currentSrc,
        naturalWidth: img.naturalWidth, width: el.getBoundingClientRect().width,
        height: el.getBoundingClientRect().height, tabIndex: el.tabIndex };
    }));
    for (const asset of assets) {
      assert(new URL(asset.url).pathname.startsWith('/yy2/assets/'), `unbuilt image URL: ${asset.url}`);
      const isAvatar = asset.sample.includes('avatar');
      assert.equal(asset.naturalWidth, isAvatar ? 96 : 600);
      const expected = asset.sample === 'named-avatar' ? 32
        : Number(asset.sample.match(/-(\d+)$/)?.[1] ?? 300);
      assert.equal(asset.width, expected, `${asset.sample} width`);
      assert.equal(asset.height, expected, `${asset.sample} height`);
      assert.equal(asset.tabIndex, -1);
    }
    assert.equal(new Set(assets.map(asset => asset.url)).size, 3);
    assert.equal(responses.length, 3);
    assert(responses.every(response => response.status === 200));
    assert.equal(await page.getByRole('img', { name: '页页，阅读伙伴' }).count(), 1);
    assert.equal(await page.getByRole('img').count(), 1, 'decorative images must be absent from the accessibility tree');
    await page.screenshot({ path: fileURLToPath(new URL(`static-${width}.png`, import.meta.url)), fullPage: true });
    await page.locator('.identities').screenshot({ path: fileURLToPath(new URL(`avatars-${width}.png`, import.meta.url)) });
    report.viewports.push({ width, deviceScaleFactor, assets, responses, layoutStable: true, noOverflow: true });
    await context.close();
  }
  assert.deepEqual(report.errors, []);
  await writeFile(new URL('verification.json', import.meta.url), JSON.stringify(report, null, 2) + '\n');
  console.log('YY2 production verification passed: 2 viewports, 26 image instances, 3 built assets, stable delayed-image layout.');
} finally {
  await browser.close();
  await new Promise(resolve => server.httpServer.close(resolve));
}
