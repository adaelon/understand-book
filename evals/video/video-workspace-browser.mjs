// UI regression for an isolated Reader serving the VI18 lecture; never sends a model request.
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { mkdirSync, writeFileSync } from 'node:fs';
import { resolve, join } from 'node:path';
const [url, output] = process.argv.slice(2);
if (!url || !output) throw new Error('Usage: node evals/video/video-workspace-browser.mjs READER_URL OUTPUT_DIR');
const require = createRequire(new URL('../../packages/web/package.json', import.meta.url));
const { chromium, expect } = require('@playwright/test');
const out = resolve(output); mkdirSync(out, { recursive: true });
const browser = await chromium.launch({ channel: 'msedge', headless: true });
const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
const errors = [], measurements = [];
page.on('pageerror', error => errors.push(error.message));
try {
  await page.goto(url);
  const video = page.locator('.video-reader video');
  await video.waitFor({ timeout: 60000 });
  await page.waitForFunction(() => document.querySelector('.video-reader video')?.readyState >= 2);
  const original = await video.elementHandle();
  assert.equal(await page.locator('.left-rail').count(), 0);
  assert.equal(await page.locator('.topbar .progress').count(), 0);
  const search = page.getByRole('searchbox', { name: '搜索字幕' });
  await search.fill('计算效率');
  await expect(page.locator('.video-reader [data-segment]')).toHaveCount(2);
  await page.getByRole('button', { name: '回看 1:35', exact: true }).click();
  await page.waitForFunction(() => { const v = document.querySelector('.video-reader video'); return v.readyState >= 2 && !v.seeking && v.currentTime > 95 && v.currentTime < 97; });
  await expect(page.getByLabel('当前片段中英对照')).toContainText('训练的计算效率');
  await page.getByRole('button', { name: '中文', exact: true }).click();
  assert.equal(await page.locator('.segment-original').count(), 0);
  await page.getByRole('button', { name: '双语', exact: true }).click();
  await page.getByRole('button', { name: '回到当前', exact: true }).click();
  await expect(page.locator('.video-reader [data-segment]')).toHaveCount(720);
  // Reading earlier subtitles suspends following until explicitly resumed.
  const transcript = page.locator('.video-reader .transcript');
  await transcript.hover(); await page.mouse.wheel(0, 200);
  await expect(page.getByRole('button', { name: '回到当前', exact: true })).toBeVisible();
  const scroll = await transcript.evaluate(p => p.scrollTop);
  await video.evaluate(v => { v.currentTime = 105; });
  await page.waitForFunction(() => !document.querySelector('.video-reader video').seeking);
  assert.equal(await transcript.evaluate(p => p.scrollTop), scroll);
  await page.getByRole('button', { name: '回到当前', exact: true }).click();
  await expect(page.locator('.transcript-row.current')).toBeInViewport();
  await page.getByRole('button', { name: '选择时间范围', exact: true }).click();
  const range = page.getByRole('form', { name: '时间范围编辑' });
  await range.getByLabel('起点（秒）').fill('95'); await range.getByLabel('终点（秒）').fill('115');
  await range.getByRole('button', { name: '保存时间范围' }).click();
  await expect(page.getByLabel('已选时间范围')).toContainText('1:35');
  await page.getByRole('button', { name: '取消草稿', exact: true }).click();
  await page.getByRole('button', { name: '框选画面', exact: true }).click();
  const region = await page.getByLabel('画面框选区域').boundingBox();
  await page.mouse.move(region.x + region.width * .15, region.y + region.height * .25);
  await page.mouse.down(); await page.mouse.move(region.x + region.width * .7, region.y + region.height * .65); await page.mouse.up();
  await expect(page.locator('.video-region-box')).toBeVisible();
  await expect(page.getByRole('button', { name: '取消草稿', exact: true })).toBeVisible();
  await page.getByRole('button', { name: '取消草稿', exact: true }).click();
  // Match the original reported moment for visual comparison.
  await video.evaluate(v => { v.currentTime = 99; });
  await page.waitForFunction(() => { const v = document.querySelector('.video-reader video'); return !v.seeking && v.readyState >= 2; });
  for (const size of [{ width: 1440, height: 900 }, { width: 1280, height: 720 }, { width: 1024, height: 768 }, { width: 390, height: 844 }]) {
    await page.setViewportSize(size);
    if (size.width < 1024 && !(await video.isVisible())) await page.getByRole('button', { name: '返回阅读', exact: true }).click();
    await expect(video).toBeVisible();
    await expect(page.getByRole('button', { name: '框选画面', exact: true })).toBeInViewport();
    const layout = await page.evaluate(() => {
      const bounds = selector => { const e = document.querySelector(selector), r = e.getBoundingClientRect(); return { x: r.x, y: r.y, width: r.width, height: r.height, scroll: e.scrollWidth, client: e.clientWidth }; };
      return { width: innerWidth, overflow: document.documentElement.scrollWidth > innerWidth, video: bounds('.video-reader'), player: bounds('.video-stage'), captions: bounds('.transcript-panel'), assistant: bounds('.right-rail') };
    });
    assert.equal(layout.overflow, false);
    assert(layout.video.scroll <= layout.video.client + 1);
    if (size.width >= 1024) {
      assert(layout.assistant.x >= layout.video.x + layout.video.width);
      await expect(page.locator('.agent-input textarea')).toBeInViewport();
      if (size.width >= 1280) await expect(search).toBeInViewport();
    }
    measurements.push(layout);
    assert(await original.evaluate(v => v === document.querySelector('.video-reader video')));
    await page.screenshot({ path: join(out, `video-${size.width}.png`) });
  }
  await page.locator('.workspace-mobile-nav').getByRole('button', { name: '伴读', exact: true }).click();
  await expect(video).not.toBeVisible();
  const input = page.locator('.agent-input textarea');
  await input.fill('解释当前画面的计算效率');
  await expect(page.getByRole('button', { name: '取消草稿', exact: true })).toBeVisible();
  await page.screenshot({ path: join(out, 'companion-390.png') });
  await page.getByRole('button', { name: '返回阅读', exact: true }).click();
  await expect(video).toBeVisible();
  await page.locator('.workspace-mobile-nav').getByRole('button', { name: '伴读', exact: true }).click();
  await expect(input).toHaveValue('解释当前画面的计算效率');
  await page.getByRole('button', { name: '取消草稿', exact: true }).click();
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.getByRole('button', { name: '专注观看', exact: true }).click();
  await expect(page.locator('.right-rail')).not.toBeVisible();
  await expect(video).toBeVisible();
  await page.getByRole('button', { name: '退出专注', exact: true }).filter({ visible: true }).click();
  await expect(page.locator('.right-rail')).toBeVisible();
  assert(await original.evaluate(v => v === document.querySelector('.video-reader video')));
  assert.deepEqual(errors, []);
  writeFileSync(join(out, 'verification.json'), JSON.stringify({ status: 'passed', url, browser: browser.version(), same_player: true, bilingual_search: true, transcript_browsing: true, region_and_range: true, mobile_draft_retained: true, focus_restored: true, model_requests: 0, errors, measurements }, null, 2));
  console.log(JSON.stringify({ status: 'passed', output: out, sizes: measurements.map(m => m.width) }));
} catch (error) {
  await page.screenshot({ path: join(out, 'failure.png') }).catch(() => {});
  console.error(JSON.stringify({ errors }));
  throw error;
} finally { await browser.close(); }
