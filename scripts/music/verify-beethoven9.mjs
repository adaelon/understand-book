import { createRequire } from 'node:module';
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
const require = createRequire(new URL('../../packages/web/package.json', import.meta.url));
const { chromium } = require('@playwright/test');
const root = path.resolve(process.argv[2] ?? '.understand-book/beethoven9-tennstedt');
const origin = process.env.BEETHOVEN_PREVIEW_ORIGIN ?? 'http://127.0.0.1:5184';
const output = path.join(root, 'verification');
mkdirSync(output, { recursive: true });
const browser = await chromium.launch({ headless: true });
const results = [];
const seek = async (page, ms) => page.getByRole('slider', { name: '音乐播放位置', exact: true }).evaluate((input, value) => {
  input.value = String(value); input.dispatchEvent(new Event('input', { bubbles: true }));
}, ms);
try {
  for (const movement of ['m1', 'm2', 'm3', 'm4']) {
    const page = await browser.newPage({ viewport: { width: 1440, height: 1000 } });
    const errors = [];
    page.on('pageerror', error => errors.push(error.message));
    page.on('console', message => { if (message.type() === 'error') errors.push(message.text()); });
    const fixture = `/@fs/${path.join(root, movement, 'music-context.json').replaceAll('\\', '/')}`;
    const started = performance.now();
    await page.goto(`${origin}/music-reader-visual.html?fixture=${encodeURIComponent(fixture)}`, { waitUntil: 'domcontentloaded', timeout: 60000 });
    await page.locator('.music-score-page svg').first().waitFor({ timeout: 30000 });
    await page.waitForFunction(() => document.querySelector('audio')?.readyState >= 1, undefined, { timeout: 30000 });
    const initialReadyMs = Math.round(performance.now() - started);
    const initial = await page.evaluate(() => ({ duration: document.querySelector('audio').duration,
      position: document.querySelector('audio').currentTime, loaded_pages: document.querySelectorAll('.music-score-page > svg').length,
      total_pages: document.querySelectorAll('.music-score-page').length }));
    const checks = { playback: false };
    if (movement === 'm2') {
      const manifest = JSON.parse(readFileSync(path.join(root, movement, 'music_source_manifest.json'), 'utf8'));
      const firsts = new Map();
      const repeat = manifest.occurrences.find(occurrence => { if (firsts.has(occurrence.measure)) return true; firsts.set(occurrence.measure, occurrence); return false; });
      if (!repeat) throw new Error('The archived scherzo must retain its repeated source measures');
      const original = firsts.get(repeat.measure);
      const expectedPage = manifest.score_objects.find(object => object.source_id === repeat.score_object_ids[0]).page_id;
      for (const occurrence of [original, repeat]) {
        await seek(page, occurrence.start_ms + 10);
        await page.locator(`[data-page="${expectedPage}"] > svg`).waitFor({ timeout: 15000 });
      }
      checks.repeat_source = true;
    }
    if (movement === 'm4') {
      await seek(page, 524000);
      await page.locator('.music-lyrics').waitFor({ timeout: 15000 });
      checks.lyrics = (await page.locator('.music-lyrics').textContent()).includes('温柔的爱妻');
      await page.getByRole('button', { name: '全部声部', exact: true }).click();
      await page.locator('.music-part-options button').filter({ hasText: '女高音独唱' }).click();
      checks.focus = await page.locator('.music-part-options button').filter({ hasText: '女高音独唱' }).getAttribute('aria-pressed') === 'true';
      await page.keyboard.press('Escape');
      await page.getByRole('combobox', { name: '播放速度', exact: true }).selectOption('0.75');
      checks.rate = await page.locator('audio').first().evaluate(audio => audio.playbackRate === 0.75);
      await page.getByRole('button', { name: '循环当前片段', exact: true }).click();
      checks.loop = await page.getByRole('button', { name: '循环当前片段', exact: true }).getAttribute('aria-pressed') === 'true';
      await page.getByRole('button', { name: '循环当前片段', exact: true }).click();
    }
    const before = await page.locator('audio').first().evaluate(audio => audio.currentTime);
    await page.getByRole('button', { name: '播放音乐', exact: true }).click();
    await page.waitForFunction(start => document.querySelector('audio').currentTime > start + 0.15, before, { timeout: 15000 });
    await page.getByRole('button', { name: '暂停音乐', exact: true }).click();
    checks.playback = true;
    await page.screenshot({ path: path.join(output, `${movement}-desktop.png`) });
    if (movement === 'm4') {
      await page.setViewportSize({ width: 390, height: 844 });
      await page.screenshot({ path: path.join(output, 'm4-mobile.png') });
    }
    const final = await page.evaluate(() => ({ loaded_pages: document.querySelectorAll('.music-score-page > svg').length,
      lyric_text: document.querySelector('.music-lyrics')?.textContent ?? null,
      viewport_overflow: document.documentElement.scrollWidth > innerWidth,
      metrics: document.querySelector('#music-qa-metrics')?.textContent }));
    if (errors.length || final.viewport_overflow) throw new Error(JSON.stringify({ movement, errors, final }));
    results.push({ movement, initial_ready_ms: initialReadyMs, initial, checks, final, errors });
    console.log(JSON.stringify({ movement, initial_ready_ms: initialReadyMs, initial, checks, loaded_pages: final.loaded_pages, errors }));
    await page.close();
  }
} finally {
  writeFileSync(path.join(output, 'browser.json'), JSON.stringify(results, null, 2));
  await browser.close();
}
