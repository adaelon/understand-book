// Runs the saved page's own JavaScript and checks rendered numeric text against
// the independent closed form. This is not a Reader save/restore or layout test.
import fs from 'node:fs';
import path from 'node:path';
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { expected } from './ex2-numeric-check.mjs';

const require = createRequire(new URL('../../packages/web/package.json', import.meta.url));
const { chromium } = require('@playwright/test');
const manifest = path.resolve(process.argv[2]);
const plan = JSON.parse(fs.readFileSync(manifest, 'utf8'));
const browser = await chromium.launch({ channel: 'msedge', headless: true });
const results = [];
try {
  for (const item of plan) {
    const root = path.resolve(path.dirname(manifest), item.directory);
    const view = JSON.parse(fs.readFileSync(path.join(root, 'view.json'), 'utf8'));
    const page = await browser.newPage({ viewport: { width: 960, height: 720 } });
    const errors = [];
    page.on('pageerror', error => errors.push(error.message));
    await page.evaluate(initialState => {
      window.presentation = {
        initialState,
        registerStateRestorer(fn) { window.restoreNumericCase = fn; },
        registerStateReader(fn) { window.readNumericCase = fn; },
        commitState() {},
      };
    }, view.initial_state);
    await page.setContent(view.content_files[view.entrypoint]);
    for (const sample of item.samples) {
      await page.evaluate(state => window.restoreNumericCase({ values: state }), sample.state);
      await page.evaluate(() => new Promise(resolve => requestAnimationFrame(resolve)));
      const reads = [];
      for (const probe of sample.probes) {
        const text = (await page.locator(probe.selector).innerText()).trim();
        const normalized = text.replaceAll('−', '-').replaceAll('–', '-');
        const raw = probe.pattern ? normalized.match(new RegExp(probe.pattern))?.[1] : normalized;
        assert.ok(raw != null && raw !== '' && Number.isFinite(Number(raw)), `missing numeric text: ${probe.selector}: ${text}`);
        const correct = expected(sample.eta, probe.step, probe.quantity);
        const tolerance = 0.5 * 10 ** -probe.decimals + 1e-9;
        reads.push({ ...probe, text, actual: Number(raw), expected: correct, tolerance, pass: Math.abs(Number(raw) - correct) <= tolerance });
      }
      const textChecks = [];
      for (const check of sample.textChecks ?? []) {
        const text = await page.locator(check.selector).innerText();
        textChecks.push({ ...check, text, pass: (!check.required || new RegExp(check.required).test(text)) && (!check.forbidden || !new RegExp(check.forbidden).test(text)) });
      }
      results.push({ directory: item.directory, state: sample.state, eta: sample.eta, reads, textChecks, errors: [...errors], pass: !errors.length && reads.every(read => read.pass) && textChecks.every(check => check.pass) });
    }
    await page.close();
  }
} finally {
  await browser.close();
}
const report = { pass: results.every(result => result.pass), results };
fs.writeFileSync(manifest.replace(/\.json$/, '-result.json'), JSON.stringify(report, null, 2));
console.log(JSON.stringify({ pass: report.pass, cases: results.length, readings: results.reduce((n, result) => n + result.reads.length, 0), failures: results.filter(result => !result.pass) }, null, 2));
if (!report.pass) process.exitCode = 1;
