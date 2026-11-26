import assert from 'node:assert/strict';
import { mkdirSync, mkdtempSync, readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(here, '../..');
const require = createRequire(path.join(root, 'packages/web/package.json'));
const { chromium, expect } = require('@playwright/test');
const tour = JSON.parse(readFileSync(path.join(here, 'tour.json'), 'utf8'));
const url = pathToFileURL(path.join(here, 'index.html')).href;
const args = process.argv.slice(2);
if (args.length > 1) throw new Error('Usage: node verify.mjs [screenshot-directory]');
const output = args.length ? path.resolve(args[0]) : mkdtempSync(path.join(tmpdir(), 'source-walkthrough-'));
mkdirSync(output, { recursive: true });

const first = tour.steps[0];
const fork = tour.steps.find(step => step.next.length > 1);
assert.ok(fork, 'The example needs a branching path');
assert.ok(new Set(tour.steps.map(step => step.file)).size > 1, 'The example needs multiple files');
const branches = fork.next.map(id => {
  const step = tour.steps.find(item => item.id === id);
  assert.ok(step, `Unknown branch "${id}"`);
  return step;
});
const source = new Map();
for (const file of new Set(tour.steps.map(step => step.file))) {
  const text = readFileSync(path.join(root, file), 'utf8').replace(/^\uFEFF/, '');
  source.set(file, text.split(/\r\n|\r|\n/));
}

function nodeFor(page, step) {
  return page.locator('.node[data-step=' + JSON.stringify(step.id) + ']');
}

async function checkStep(page, step, showCode = true) {
  await expect(page.locator('#step-title')).toHaveText(step.title);
  await expect(page.locator('#explanation')).toHaveText(step.explanation);
  await expect(page.locator('#source-path')).toHaveText(step.file);
  await expect(page.locator('.node[aria-pressed="true"]')).toHaveAttribute('data-step', step.id);
  const selected = page.locator('.code-line.selected');
  const numbers = (await selected.locator('.line-number').allTextContents()).map(Number);
  const expected = source.get(step.file).slice(step.start - 1, step.end);
  assert.deepEqual(numbers, expected.map((_, i) => step.start + i), `Line numbers for ${step.id}`);
  assert.deepEqual(await selected.locator('.line-text').allTextContents(), expected, `Source text for ${step.id}`);
  if (!showCode) return;
  const bounds = await page.locator('#code-viewport').evaluate(view => {
    const area = view.getBoundingClientRect();
    const rows = view.querySelectorAll('.code-line.selected');
    return {
      top: rows[0].getBoundingClientRect().top,
      bottom: rows[rows.length - 1].getBoundingClientRect().bottom,
      viewTop: Math.max(0, area.top),
      viewBottom: Math.min(innerHeight, area.top + view.clientHeight),
    };
  });
  assert.ok(bounds.top >= bounds.viewTop - 1 && bounds.bottom <= bounds.viewBottom + 1,
    `Highlighted lines for ${step.id} are outside the visible code panel: ${JSON.stringify(bounds)}`);
}

async function checkWidth(page) {
  const size = await page.evaluate(() => ({
    actual: document.documentElement.scrollWidth, available: document.documentElement.clientWidth,
  }));
  assert.ok(size.actual <= size.available + 1, `Page overflows horizontally: ${JSON.stringify(size)}`);
}

const errors = [];
const network = [];
const browser = await chromium.launch({ channel: 'msedge', headless: true });
async function open(viewport, mobile = false) {
  const context = await browser.newContext({
    viewport, deviceScaleFactor: 1, isMobile: mobile, hasTouch: mobile, offline: true, reducedMotion: 'reduce',
  });
  context.on('request', request => {
    if (/^https?:/i.test(request.url())) network.push(request.url());
  });
  const page = await context.newPage();
  page.on('pageerror', error => errors.push(error.message));
  await page.goto(url);
  await expect(nodeFor(page, first)).toBeVisible();
  return page;
}

try {
  const desktop = await open({ width: 1440, height: 900 });
  await checkStep(desktop, first);
  await checkWidth(desktop);
  await expect(desktop.locator('#previous')).toBeDisabled();
  for (const branch of branches) {
    await nodeFor(desktop, branch).click();
    await checkStep(desktop, branch);
  }
  const chosen = branches[branches.length - 1];
  await desktop.screenshot({ path: path.join(output, 'desktop.png') });
  await desktop.reload();
  await checkStep(desktop, chosen);
  await nodeFor(desktop, first).click();
  for (const step of tour.steps.slice(1)) {
    await desktop.locator('#next').click();
    await checkStep(desktop, step);
  }
  await expect(desktop.locator('#next')).toBeDisabled();
  await desktop.locator('#previous').click();
  await checkStep(desktop, tour.steps.at(-2));
  await desktop.context().close();

  const mobile = await open({ width: 390, height: 844 }, true);
  const flowTab = mobile.locator('.mobile-switch button[data-view="flow"]');
  const codeTab = mobile.locator('.mobile-switch button[data-view="code"]');
  await expect(flowTab).toHaveAttribute('aria-pressed', 'true');
  await expect(mobile.locator('#code-viewport')).toBeHidden();
  const branch = branches[0];
  await nodeFor(mobile, branch).tap();
  await expect(codeTab).toHaveAttribute('aria-pressed', 'true');
  await expect(mobile.locator('#code-viewport')).toBeVisible();
  await checkStep(mobile, branch);
  await checkWidth(mobile);
  await mobile.screenshot({ path: path.join(output, 'mobile-code.png') });
  await flowTab.tap();
  await expect(flowTab).toHaveAttribute('aria-pressed', 'true');
  await expect(nodeFor(mobile, branch)).toBeVisible();
  await expect(mobile.locator('#code-viewport')).toBeHidden();
  await checkStep(mobile, branch, false);
  await checkWidth(mobile);
  await mobile.screenshot({ path: path.join(output, 'mobile-flow.png') });
  await mobile.context().close();

  assert.deepEqual(errors, [], 'Uncaught browser errors');
  assert.deepEqual(network, [], 'Unexpected network requests');
  console.log(`Verified ${tour.steps.length} steps, ${source.size} source files, desktop and mobile. Screenshots: ${output}`);
} finally {
  await browser.close();
}
