// Visual and interaction checks for the music Reader layout; uses real local media.
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import path from 'node:path';
import { chromium } from 'playwright';

const option = (key, fallback) => process.argv.includes(key) ? process.argv[process.argv.indexOf(key) + 1] : fallback;
const published = process.argv.includes('--published');
const base = option('--url', published ? 'http://127.0.0.1:6677' : 'http://127.0.0.1:4193');
const phase = option('--phase', published ? 'published' : 'before');
const out = path.resolve('evals/music/results/score-space-v2');
await fs.mkdir(out, { recursive: true });
const browser = await chromium.launch({ headless: true });
const results = { phase, surfaces: [], checks: [], errors: [] };
try {
  if (published) {
    const page = await browser.newPage({ viewport: { width: 1500, height: 1000 } });
    page.on('pageerror', error => results.errors.push(error.message));
    await page.goto(base);
    await page.locator('.music-score-page > svg').first().waitFor();
    await page.waitForFunction(() => document.querySelector('audio')?.readyState >= 2);
    await page.screenshot({ path: path.join(out, 'published-desktop.png') });
    results.assets = await page.locator('script[src]').evaluateAll(elements => elements.map(element => element.src));
    results.audio = await page.locator('audio').first().evaluate(element => ({ duration: element.duration, ready_state: element.readyState, source: element.currentSrc }));
    const width = await page.locator('.music-reader').evaluate(element => element.clientWidth);
    await page.getByRole('button', { name: '全屏乐谱', exact: true }).click();
    await page.waitForFunction(() => Boolean(document.fullscreenElement));
    assert(await page.locator('.music-reader').evaluate(element => element.clientWidth) > width * 1.2);
    await page.screenshot({ path: path.join(out, 'published-fullscreen.png') });
    await page.getByRole('button', { name: '播放音乐', exact: true }).click();
    await page.waitForFunction(() => document.querySelector('audio').currentTime > .15);
    await page.locator('.music-ask').click();
    await page.waitForFunction(() => !document.fullscreenElement, undefined, { timeout: 3000 });
    assert(await page.locator('audio').first().evaluate(element => element.paused));
    assert(await page.getByPlaceholder(/说说你听到了什么/).isVisible());
    assert.equal(await page.getByPlaceholder(/说说你听到了什么/).inputValue(), '刚才这一段，应该注意听什么？');
    await page.screenshot({ path: path.join(out, 'published-fullscreen-question.png') });
    await page.getByRole('button', { name: '取消片段提问', exact: true }).click();
    results.checks.push('production bundle, actual recording, fullscreen expands score, question pauses and exits fullscreen to visible composer');
    await page.setViewportSize({ width: 390, height: 844 });
    await page.getByRole('button', { name: '返回聆听', exact: true }).click();
    await page.locator('.music-reader').waitFor({ state: 'visible' });
    assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth));
    const controls = await page.locator('.music-player-bar').boundingBox();
    assert(controls.x >= 0 && controls.x + controls.width <= 390 && controls.y + controls.height <= 844);
    await page.screenshot({ path: path.join(out, 'published-mobile.png') });
    await page.locator('.music-details-toggle').click();
    await page.locator('.music-demonstrations button').first().click();
    await page.locator('.music-demo-player audio').evaluate(async element => { await element.play(); });
    await page.locator('.music-ask').click();
    await page.getByRole('button', { name: '返回聆听', exact: true }).waitFor({ state: 'visible' });
    assert(await page.locator('.music-demo-player audio').evaluate(element => element.paused));
    await page.getByRole('button', { name: '返回聆听', exact: true }).click();
    await page.getByRole('button', { name: '取消片段提问', exact: true }).click();
    await page.getByRole('button', { name: '返回原录音', exact: true }).click();
    assert.equal(await page.getByRole('alert').count(), 0);
    results.checks.push('production mobile: score and controls fit, demo plays, ask reveals chat, return/cancel restores score');
    await page.close();
  } else {
  for (const viewport of [{ width: 1500, height: 1000 }, { width: 1280, height: 720 }, { width: 390, height: 844 }]) {
    const page = await browser.newPage({ viewport });
    page.on('pageerror', error => results.errors.push(error.message));
    await page.goto(`${base}/music-reader-visual.html`);
    await page.locator('.music-score-page > svg').first().waitFor();
    await page.waitForFunction(() => document.querySelector('audio')?.readyState >= 2);
    const layout = await page.evaluate(() => {
      const rect = selector => { const r = document.querySelector(selector).getBoundingClientRect(); return { x: r.x, y: r.y, width: r.width, height: r.height }; };
      return { reader: rect('.music-reader'), score: rect('.music-score-surface'), header: rect('.music-heading'), player: rect('.music-player-bar'), overflow: document.documentElement.scrollWidth > innerWidth };
    });
    results.surfaces.push({ viewport, ...layout, score_share: layout.score.height / layout.reader.height });
    await page.screenshot({ path: path.join(out, `${phase}-${viewport.width}.png`) });
    assert(!layout.overflow, `Page overflows at ${viewport.width}`);
    await page.close();
  }
  if (process.argv.includes('--interactions')) {
    const page = await browser.newPage({ viewport: { width: 1280, height: 800 } });
    page.on('pageerror', error => results.errors.push(error.message));
    const fixturePath = path.resolve('evals/music/results/music-animation-context.json').replaceAll('\\', '/');
    await page.goto(`${base}/music-reader-visual.html?fixture=/@fs/${fixturePath}`);
    await page.locator('.music-score-page svg').first().waitFor();
    await page.waitForFunction(() => document.querySelector('audio')?.readyState >= 2);
    await page.getByRole('button', { name: '播放音乐', exact: true }).click();
    await page.waitForFunction(() => document.querySelector('audio').currentTime > .1);
    await page.getByLabel('播放速度', { exact: true }).selectOption('0.75');
    assert.equal(await page.locator('audio').first().evaluate(el => el.playbackRate), .75);
    await page.getByRole('button', { name: '暂停音乐', exact: true }).click();
    await page.getByLabel('音乐播放位置', { exact: true }).evaluate(el => { el.value = '4700'; el.dispatchEvent(new Event('input', { bubbles: true })); });
    await page.waitForFunction(() => document.querySelectorAll('.music-note-overlay circle[style*="opacity: 0.9"]').length >= 3);
    await page.screenshot({ path: path.join(out, 'animation-1280.png') });
    results.checks.push('actual synthesis playback, rate, seek, simultaneous chord highlights');
    const width = await page.locator('.music-score-page svg').first().evaluate(el => el.getBoundingClientRect().width);
    await page.getByRole('button', { name: '放大乐谱', exact: true }).click();
    const wider = await page.locator('.music-score-page svg').first().evaluate(el => el.getBoundingClientRect().width);
    assert(wider > width * 1.2, 'Zoom must enlarge the actual score');
    await page.getByRole('button', { name: '缩小乐谱', exact: true }).click();
    await page.locator('.music-focus-toggle').click();
    await page.locator('.music-voice-options button').first().click();
    assert.equal(await page.locator('.music-voice-options button').first().getAttribute('aria-pressed'), 'true');
    assert(await page.locator('.music-score-page [style*="opacity: 0.22"]').count() > 0);
    await page.screenshot({ path: path.join(out, 'voices-1280.png') });
    await page.keyboard.press('Escape');
    assert.equal(await page.locator('.music-focus-menu').count(), 0);
    assert(await page.locator('.music-focus-toggle').evaluate(el => el === document.activeElement));
    results.checks.push('actual score zoom; voice focus; Escape closes the menu and restores focus');
    await page.getByRole('button', { name: '全屏乐谱', exact: true }).click();
    await page.waitForFunction(() => Boolean(document.fullscreenElement));
    await page.getByRole('button', { name: '退出全屏乐谱', exact: true }).click();
    await page.waitForFunction(() => !document.fullscreenElement);
    results.checks.push('native score fullscreen and return');
    await page.setViewportSize({ width: 390, height: 844 });
    await page.locator('.music-score-surface').dispatchEvent('wheel', { deltaY: 300 });
    assert(await page.locator('.music-follow-toggle').evaluate(element => element.classList.contains('is-detached')));
    await page.getByRole('button', { name: '回到播放位置', exact: true }).click();
    assert(!await page.locator('.music-follow-toggle').evaluate(element => element.classList.contains('is-detached')));
    const toolbarFits = await page.locator('.music-score-toolbar button').evaluateAll(elements => elements.filter(element => element.getBoundingClientRect().width).every(element => { const box = element.getBoundingClientRect(); return box.left >= 0 && box.right <= innerWidth; }));
    assert(toolbarFits, 'Mobile follow, zoom and fullscreen controls must remain visible');
    await page.screenshot({ path: path.join(out, 'animation-390.png') });
    results.checks.push('mobile manual browsing and return to playback position, all toolbar controls visible');
    await page.close();

    const original = await browser.newPage({ viewport: { width: 390, height: 844 } });
    original.on('pageerror', error => results.errors.push(error.message));
    await original.goto(`${base}/music-reader-visual.html`);
    await original.waitForFunction(() => document.querySelector('audio')?.readyState >= 2);
    await original.locator('.music-details-toggle').click();
    await original.locator('.music-demonstrations button').first().click();
    await original.locator('.music-demo-player audio').evaluate(async el => { await el.play(); });
    await original.waitForFunction(() => !document.querySelector('.music-demo-player audio').paused);
    await original.locator('.music-ask').click();
    assert(await original.locator('.music-demo-player audio').evaluate(el => el.paused));
    const fits = await original.evaluate(() => {
      const demo = document.querySelector('.music-demo-player').getBoundingClientRect();
      const question = document.querySelector('.music-question-anchor').getBoundingClientRect();
      return demo.bottom <= question.top && demo.top >= document.querySelector('.music-score-stage').getBoundingClientRect().top;
    });
    assert(fits, 'Demo must not cover question or transport controls');
    await original.screenshot({ path: path.join(out, 'demo-question-390.png') });
    await original.getByRole('button', { name: '取消片段提问', exact: true }).click();
    await original.getByRole('button', { name: '返回原录音', exact: true }).click();
    await original.getByRole('button', { name: '播放音乐', exact: true }).click();
    assert.equal(await original.locator('.music-demo-player').count(), 0);
    await original.getByRole('button', { name: '暂停音乐', exact: true }).click();
    results.checks.push('mobile guide, actual demo, question pauses audio, cancel, return to original');
    await original.close();

    const orchestra = await browser.newPage({ viewport: { width: 1280, height: 800 } });
    orchestra.on('pageerror', error => results.errors.push(error.message));
    await orchestra.goto(`${base}/music-reader-visual.html?fixture=/@fs/${path.resolve('evals/music/results/music-beethoven-context.json').replaceAll('\\', '/')}`);
    await orchestra.waitForFunction(() => document.querySelectorAll('.music-score-page > svg').length === 17);
    await orchestra.locator('.music-focus-toggle').click();
    await orchestra.getByLabel('谱面着色方式', { exact: true }).selectOption('part');
    assert.equal(await orchestra.locator('.music-part-options button').count(), 7);
    await orchestra.locator('.music-part-options button').last().click();
    assert.equal(await orchestra.locator('.music-part-options button').last().getAttribute('aria-pressed'), 'true');
    await orchestra.screenshot({ path: path.join(out, 'orchestra-1280.png') });
    results.checks.push('real Beethoven 17-page score and all six instrument choices');
    await orchestra.close();
  }
  }
  assert.equal(results.errors.length, 0);
} catch (error) {
  results.failure = error.stack;
  throw error;
} finally {
  await fs.writeFile(path.join(out, `${phase}.json`), JSON.stringify(results, null, 2));
  await browser.close();
}
console.log(JSON.stringify(results));
