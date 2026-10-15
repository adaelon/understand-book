// Run against the local VI2 server + frontend. Uses a real browser and real HTTP range responses.
import { createRequire } from 'node:module';
import { readFileSync, writeFileSync, mkdirSync, openSync, readSync, closeSync } from 'node:fs';
import path from 'node:path';
import assert from 'node:assert/strict';
const require = createRequire(new URL('../../packages/web/package.json', import.meta.url));
const { chromium } = require('@playwright/test');
const root = path.resolve(process.argv[2] || 'evals/video/generated/vi2');
const frontend = process.env.VI2_FRONTEND || 'http://localhost:5175';
const backend = process.env.VI2_BACKEND || 'http://127.0.0.1:8793';
const imported = JSON.parse(readFileSync(path.join(root, 'import-verification.json')));
const video = imported.imported.video;
const route = `/api/videos/${video.media_id}/${video.media_revision}`;
const ranges = [];
const browser = await chromium.launch({ channel: process.env.VI2_BROWSER || 'msedge', headless: true });
try {
  const page = await browser.newPage({ viewport: { width: 1280, height: 960 } });
  page.on('response', response => {
    if (response.url().endsWith('/media')) ranges.push({ status: response.status(), range: response.headers()['content-range'] ?? null });
  });
  await page.goto(frontend);
  await page.waitForFunction(() => { const v = document.querySelector('video'); return v?.readyState >= 2 && v.videoWidth === 1920; });
  assert.equal(await page.locator('textarea').isDisabled(), true);
  assert.match(await page.locator('[role=status]').innerText(), /转写.*尚未就绪/);
  const positions = [];
  for (const time of [555.123, 1200, 4620]) {
    await page.locator('video').evaluate((v, time) => { v.currentTime = time; }, time);
    await page.waitForFunction(time => { const v = document.querySelector('video'); return !v.seeking && v.readyState >= 2 && Math.abs(v.currentTime - time) < .2; }, time);
    positions.push(await page.locator('video').evaluate(v => ({ time: v.currentTime, width: v.videoWidth, height: v.videoHeight })));
  }
  await page.locator('video').evaluate(async v => { v.currentTime = 555.123; v.playbackRate = 1.5; await v.play(); });
  await page.waitForFunction(() => document.querySelector('video').currentTime > 556.2);
  const pausedAt = await page.locator('video').evaluate(v => { v.pause(); return v.currentTime; });
  assert.equal(await page.locator('video').evaluate(v => v.paused), true);
  await page.reload();
  await page.waitForFunction(time => { const v = document.querySelector('video'); return v?.readyState >= 2 && !v.seeking && Math.abs(v.currentTime-time) < .2; }, pausedAt);
  assert.equal(await page.locator('video').evaluate(v => v.playbackRate), 1.5);
  mkdirSync(root, { recursive: true });
  await page.screenshot({ path: path.join(root, 'player.png'), fullPage: true });
  const response = await fetch(backend + route + '/media', { headers: { Range: 'bytes=100-131' } });
  assert.equal(response.status, 206); assert.equal(response.headers.get('content-range'), `bytes 100-131/${imported.imported.total_bytes}`);
  const expected = Buffer.alloc(32), fd = openSync(path.join(imported.workspace, 'media/original.mp4'), 'r');
  readSync(fd, expected, 0, 32, 100); closeSync(fd);
  assert.deepEqual(Buffer.from(await response.arrayBuffer()), expected);
  const head = await fetch(backend + route + '/media', { method: 'HEAD' });
  assert.equal(head.headers.get('content-length'), String(imported.imported.total_bytes));
  assert.equal((await head.arrayBuffer()).byteLength, 0);
  const rejected = await fetch(backend + route + '/ask', { method: 'POST', body: '{}' });
  assert.equal(rejected.status, 409);
  assert(ranges.some(r => r.status === 206));
  const result = { date: new Date().toISOString(), browser: browser.version(), platform: process.platform,
    video, positions, paused_at: pausedAt, resumed_rate: 1.5, range_responses: ranges,
    byte_range_exact: true, head_length: head.headers.get('content-length'), qa_status: rejected.status,
    original_external_path_missing: imported.external_path_missing, workspace_moved: imported.workspace_moved };
  writeFileSync(path.join(root, 'browser-verification.json'), JSON.stringify(result, null, 2));
  console.log(JSON.stringify(result, null, 2));
} finally { await browser.close(); }
