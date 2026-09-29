// Reproduce the delivered C1 page's claimed bar-length ratio in its real browser DOM.
import fs from 'node:fs';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
import { expected } from './ex2-numeric-check.mjs';
const require = createRequire(new URL('../../packages/web/package.json', import.meta.url));
const { chromium } = require('@playwright/test');
const root = new URL('./ex2-windows/v7-C1-learning-rate/', import.meta.url);
const view = JSON.parse(fs.readFileSync(new URL('view.json', root)));
const browser = await chromium.launch({ channel: 'msedge', headless: true });
try {
  const page = await browser.newPage({ viewport: { width: 960, height: 720 } });
  await page.evaluate(initialState => {
    window.presentation = { initialState, registerStateReader() {}, registerStateRestorer(fn) { window.restore = fn; }, commitState() {} };
  }, view.initial_state);
  await page.setContent(view.content_files[view.entrypoint]);
  const samples = [];
  for (const eta of [0, 0.7, 1.2]) {
    await page.evaluate(eta => window.restore({ values: { eta, t: 12 } }), eta);
    const actual = await page.evaluate(() => ({
      bars: [...document.querySelectorAll('#ladderBox svg rect')].map((rect, k) => ({ k, width: Number(rect.getAttribute('width')) })),
      caption: document.querySelector('#ladderBox').parentElement.querySelector('figcaption').innerText,
      verdict: document.querySelector('#verdict').innerText,
      current: document.querySelector('#wOut').innerText,
    }));
    const last = actual.bars.at(-1), previous = actual.bars.at(-2);
    const ratio = last.width / previous.width;
    const correct = Math.abs(1 - 2 * eta);
    samples.push({ eta, step: 12, ...actual, observedRatio: ratio, expectedRatio: correct,
      expectedErrors: [expected(eta, 11, 'abs_e'), expected(eta, 12, 'abs_e')],
      pass: Math.abs(ratio - correct) <= 0.01 });
    if (eta === 1.2) await page.screenshot({ path: fileURLToPath(new URL('geometry-eta1.2.png', root)), fullPage: true });
  }
  const report = { pass: samples.every(sample => sample.pass), samples };
  fs.writeFileSync(new URL('geometry-check.json', root), JSON.stringify(report, null, 2));
  console.log(JSON.stringify(report, null, 2));
  if (!report.pass) process.exitCode = 1;
} finally { await browser.close(); }
