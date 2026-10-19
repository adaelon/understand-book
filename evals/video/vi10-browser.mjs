// Observe real VI10 publications in an isolated local Reader while the build runs.
import { createRequire } from 'node:module';
import { spawn } from 'node:child_process';
import { mkdirSync, readFileSync, writeFileSync, createWriteStream, existsSync } from 'node:fs';
import { resolve, join, dirname } from 'node:path';
import assert from 'node:assert/strict';
const require = createRequire(new URL('../../packages/web/package.json', import.meta.url));
const { chromium } = require('@playwright/test');
const root = resolve('evals/video/generated/vi10'), workspace = join(root, 'workspace'), out = join(root, 'browser-' + Date.now());
mkdirSync(out, { recursive: true });
const env = { ...process.env, UNDERSTAND_BOOK_ADDR: '127.0.0.1:8794', UNDERSTAND_BOOK_MEMORY_DIR: join(out, 'memory'), UNDERSTAND_BOOK_PRIVATE_DIR: join(out, 'private') };
const children = [];
function start(exe, args, name, cwd = process.cwd()) {
  const child = spawn(exe, args, { env, cwd, windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'] });
  const log = createWriteStream(join(out, name + '.log')); child.stdout.pipe(log); child.stderr.pipe(log); children.push(child);
}
async function wait(predicate, label, timeout = 120000) {
  const end = Date.now() + timeout;
  while (Date.now() < end) { if (await predicate()) return; await new Promise(r => setTimeout(r, 500)); }
  throw new Error('Timed out: ' + label);
}
let browser;
try {
  const first = JSON.parse(readFileSync(join(root, 'publications.jsonl'), 'utf8').trim().split('\n')[0]);
  const firstDirectory = join(root, 'service/library/published', first.reference.book_id, first.reference.publication_id);
  start(resolve('target/debug/server.exe'), [firstDirectory], 'server');
  start(process.execPath, [join(dirname(require.resolve('vite/package.json')), 'bin/vite.js'), '--port', '5176', '--strictPort'], 'web', resolve('packages/web'));
  for (const url of ['http://127.0.0.1:8794/desktop/status', 'http://localhost:5176'])
    await wait(async () => { try { return (await fetch(url)).ok; } catch { return false; } }, url);
  browser = await chromium.launch({ channel: 'msedge', headless: true });
  const page = await browser.newPage({ viewport: { width: 1440, height: 1000 } });
  let allowAdoption = false;
  await page.route('**/reader/adopt_same_source_publication', route => allowAdoption ? route.continue() : route.fulfill({ json: { status: 'current' } }));
  const errors = []; page.on('pageerror', e => errors.push(e.message));
  await page.goto('http://localhost:5176');
  await page.getByLabel('中文翻译进度').waitFor();
  await page.locator('.video-reader video').waitFor();
  const video = page.locator('.video-reader video'), element = await video.elementHandle();
  await page.waitForFunction(() => document.querySelector('.video-reader video')?.readyState >= 2);
  await video.evaluate(v => { v.currentTime = 30; v.playbackRate = 1.5; });
  await page.waitForFunction(() => !document.querySelector('.video-reader video')?.seeking);
  await video.evaluate(v => v.pause());
  const initial = await page.getByLabel('中文翻译进度').textContent();
  await page.screenshot({ path: join(out, 'initial.png'), fullPage: true });
  const anchor = await page.locator('.video-reader .transcript').evaluate(pane => {
    const segment = pane.querySelectorAll('[data-segment]')[40]; pane.scrollTop = segment.offsetTop - pane.offsetTop;
    return { id: segment.dataset.segment, offset: segment.getBoundingClientRect().top - pane.getBoundingClientRect().top };
  });
  allowAdoption = true;
  await wait(async () => Number((await page.getByLabel('中文翻译进度').textContent()).match(/中文 (\d+) /)?.[1]) > 0, 'first real Chinese publication', 3600000);
  const offset = await page.locator('.video-reader .transcript').evaluate((pane, id) => pane.querySelector(`[data-segment="${id}"]`).getBoundingClientRect().top - pane.getBoundingClientRect().top, anchor.id);
  assert(Math.abs(offset - anchor.offset) < 2, `English anchor moved ${anchor.offset} -> ${offset}`);
  assert(await element.evaluate(v => v === document.querySelector('.video-reader video')));
  assert.equal(await video.evaluate(v => v.currentTime), 30); assert.equal(await video.evaluate(v => v.playbackRate), 1.5);
  assert.equal(await video.evaluate(v => v.paused), true);
  assert(!(await page.getByLabel('当前片段中英对照').textContent()).includes('待翻译'));
  await page.screenshot({ path: join(out, 'first-batch.png'), fullPage: true });
  writeFileSync(join(out, 'first-batch.json'), JSON.stringify({ initial, anchor, offset, same_player: true, position: 30, rate: 1.5, paused: true }, null, 2));
  await wait(async () => (await page.getByLabel('中文翻译进度').textContent()).includes('中文 720 / 720'), 'whole lecture Chinese', 7200000);
  assert(await element.evaluate(v => v === document.querySelector('.video-reader video')));
  const link = JSON.parse(readFileSync(join(workspace, 'video_publication.json')));
  await video.evaluate(v => { v.currentTime = 4500; });
  await page.waitForFunction(() => !document.querySelector('.video-reader video')?.seeking);
  await page.screenshot({ path: join(out, 'complete.png'), fullPage: true });
  await page.reload();
  await page.locator('.video-reader video').waitFor();
  await wait(async () => (await page.getByLabel('中文翻译进度').textContent()).includes('中文 720 / 720'), 'reload full coverage');
  const result = { status: 'passed', browser: browser.version(), first_batch_anchor_shift_px: offset - anchor.offset,
    same_player: true, paused_position_and_rate_preserved: true, total_segments: 720, reload_preserved: true, publication: link.reference, errors };
  assert.deepEqual(errors, []);
  writeFileSync(join(out, 'verification.json'), JSON.stringify(result, null, 2)); writeFileSync(join(root, 'browser-result.json'), JSON.stringify({ ...result, output: out }, null, 2)); console.log(JSON.stringify(result));
} finally {
  await browser?.close();
  for (const child of children.reverse()) if (child.exitCode === null) child.kill();
}
