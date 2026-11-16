import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import path from 'node:path';
import { chromium } from 'playwright';

const base = process.env.SCORE_NAV_ORIGIN ?? 'http://127.0.0.1:5176';
const fixture = path.resolve('.understand-book/beethoven9-tennstedt/m1/music-context.json').replaceAll('\\', '/');
const context = JSON.parse(await fs.readFile(fixture, 'utf8'));
const { manifest } = context;
const out = path.resolve('docs/performance/instrument-navigation-20261105');
await fs.mkdir(out, { recursive: true });
const result = { checks: [], centers: [], errors: [] };
const browser = await chromium.launch({ args: ['--mute-audio'] });
const page = await browser.newPage({ viewport: { width: 1219, height: 837 } });
page.on('pageerror', error => result.errors.push(error.message));
const occurrence = measure => manifest.occurrences.find(item => item.measure === String(measure));
const objectFor = (part, measure) => manifest.score_objects.find(object => object.part_id === part && object.measure === String(measure));
async function seek(measure, offset = 100) {
  const ms = occurrence(measure).start_ms + offset;
  await page.getByLabel('音乐播放位置', { exact: true }).evaluate((input, ms) => {
    input.value = String(ms); input.dispatchEvent(new Event('input', { bubbles: true }));
  }, ms);
  await page.waitForFunction(ms => Math.abs(document.querySelector('audio').currentTime * 1000 - ms) < 30, ms);
  return ms;
}
async function centered(part, measure) {
  const object = objectFor(part, measure);
  const selector = `[data-page="${object.page_id}"] [id="${object.svg_element_id}"]`;
  await page.waitForFunction(selector => {
    const element = document.querySelector(selector);
    if (!element) return false;
    const staff = element.closest('.staff');
    const box = (staff?.querySelector(':scope > path:nth-of-type(3)') ?? staff ?? element).getBoundingClientRect();
    const view = document.querySelector('.music-score-surface').getBoundingClientRect();
    return Math.abs((box.top + box.bottom - view.top - view.bottom) / 2) < 2;
  }, selector);
  result.centers.push({ part, measure, errorPx: await page.locator(selector).evaluate(element => {
    const staff = element.closest('.staff');
    const box = (staff?.querySelector(':scope > path:nth-of-type(3)') ?? staff ?? element).getBoundingClientRect();
    const view = document.querySelector('.music-score-surface').getBoundingClientRect();
    return Math.abs((box.top + box.bottom - view.top - view.bottom) / 2);
  }) });
}
const instrument = part => page.locator(`.music-instrument-list [data-part="${part}"]`);
try {
  await page.addInitScript(({ recording, ms }) => localStorage.setItem(`music-position:visual-review:${recording.recording_id}:${recording.revision}`, JSON.stringify({ ms, rate: 1 })),
    { recording: manifest.recording.source, ms: occurrence(69).start_ms + 100 });
  await page.goto(`${base}/music-reader-visual.html?fixture=/@fs/${fixture}`);
  await page.locator('[data-page="page-9"] > svg .music-note-overlay').waitFor({ state: 'attached' });
  await page.waitForFunction(() => document.querySelector('audio')?.readyState >= 2);
  assert.equal(await page.locator('.music-instrument-list button').count(), manifest.parts.length);
  const before = await page.locator('audio').evaluate(audio => ({ time: audio.currentTime, paused: audio.paused }));
  const focusBefore = await page.locator('.music-focus-toggle').innerText();
  for (const part of ['P2', 'P32', 'P48', 'P52']) {
    await instrument(part).click();
    await centered(part, 69);
    assert.equal(await instrument(part).getAttribute('aria-pressed'), 'true');
  }
  assert.deepEqual(await page.locator('audio').evaluate(audio => ({ time: audio.currentTime, paused: audio.paused })), before);
  assert.equal(await page.locator('.music-focus-toggle').innerText(), focusBefore);
  await page.screenshot({ path: path.join(out, 'desktop-bass.png') });
  result.checks.push('all 20 instruments; top, middle, strings and bass centered at bar 69; audio and highlight focus unchanged');

  const expandedWidth = await page.locator('.music-score-surface').evaluate(surface => surface.clientWidth);
  await page.getByRole('button', { name: '收起乐器导航', exact: true }).click();
  assert.equal(await page.locator('.music-instrument-list').isVisible(), false);
  assert(await page.locator('.music-score-surface').evaluate(surface => surface.clientWidth) > expandedWidth + 70);
  await centered('P52', 69);
  await page.screenshot({ path: path.join(out, 'desktop-collapsed.png') });
  await page.getByRole('button', { name: '展开乐器导航', exact: true }).press('Enter');
  await centered('P52', 69);
  assert.equal(await instrument('P52').getAttribute('aria-pressed'), 'true');
  assert.deepEqual(await page.locator('audio').evaluate(audio => ({ time: audio.currentTime, paused: audio.paused })), before);
  result.checks.push('collapse widens the score; keyboard expansion restores the list and selection without seeking');

  await page.getByRole('button', { name: '放大乐谱', exact: true }).click();
  await centered('P52', 69);
  await page.getByRole('button', { name: '缩小乐谱', exact: true }).click();
  await centered('P52', 69);
  result.checks.push('zoom retains the selected instrument at the center');

  await seek(1);
  await instrument('P2').click();
  await centered('P2', 1);
  await seek(547);
  await instrument('P52').click();
  await centered('P52', 547);
  result.checks.push('first staff on first page and last staff on last page can be centered');

  await seek(69);
  await centered('P52', 69);
  await page.locator('.music-score-surface').dispatchEvent('wheel', { deltaY: -100 });
  await instrument('P48').click();
  assert(await page.locator('.music-follow-toggle').evaluate(button => button.classList.contains('is-detached')));
  assert(Math.abs(await page.locator('audio').evaluate(audio => audio.currentTime * 1000) - (occurrence(69).start_ms + 100)) < 30);
  await page.getByRole('button', { name: '回到播放位置', exact: true }).click();
  await centered('P48', 69);
  result.checks.push('manual browsing remains detached; return to playback keeps the selected instrument');

  await page.locator('.music-score-surface').dispatchEvent('wheel', { deltaY: -100 });
  await page.locator('.music-score-surface').evaluate(surface => {
    const sheet = surface.querySelector('[data-page="page-8"]').getBoundingClientRect();
    const view = surface.getBoundingClientRect();
    surface.scrollTop += sheet.top + sheet.height / 2 - view.top - view.height / 2;
  });
  await page.locator('[data-page="page-8"] .music-note-overlay').waitFor({ state: 'attached' });
  await instrument('P2').click();
  await centered('P2', 57);
  await page.getByRole('button', { name: '收起乐器导航', exact: true }).click();
  await centered('P2', 57);
  await page.getByRole('button', { name: '展开乐器导航', exact: true }).click();
  await centered('P2', 57);
  assert(Math.abs(await page.locator('audio').evaluate(audio => audio.currentTime * 1000) - (occurrence(69).start_ms + 100)) < 30);
  await page.getByRole('button', { name: '回到播放位置', exact: true }).click();
  await centered('P2', 69);
  result.checks.push('instrument navigation uses the manually viewed page, without seeking the recording');

  await seek(72, Math.max(0, occurrence(72).end_ms - occurrence(72).start_ms - 1200));
  await instrument('P52').click();
  await page.getByRole('button', { name: '播放音乐', exact: true }).click();
  await page.waitForFunction(ms => document.querySelector('audio').currentTime * 1000 > ms, occurrence(73).start_ms + 250);
  await page.getByRole('button', { name: '暂停音乐', exact: true }).click();
  await centered('P52', 73);
  result.checks.push('continuous playback turns from page 9 to 10 while following bass');

  await page.getByRole('button', { name: '收起乐器导航', exact: true }).click();
  await page.setViewportSize({ width: 390, height: 844 });
  assert.equal(await page.locator('.music-instrument-list').isVisible(), false);
  await page.locator('.music-instrument-toggle').click();
  await page.keyboard.press('Escape');
  assert.equal(await page.locator('.music-instrument-list').isVisible(), false);
  assert(await page.locator('.music-instrument-toggle').evaluate(button => button === document.activeElement));
  await page.locator('.music-instrument-toggle').click();
  await page.screenshot({ path: path.join(out, 'mobile-list.png') });
  await instrument('P48').click();
  await centered('P48', 73);
  assert.equal(await page.locator('.music-instrument-list').isVisible(), false);
  assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth));
  await page.screenshot({ path: path.join(out, 'mobile-violin.png') });
  result.checks.push('390px menu opens, Escape restores focus, selection closes it, violin centered, no page overflow');
  assert.deepEqual(result.errors, []);
} catch (error) {
  result.failure = error.stack;
  await page.screenshot({ path: path.join(out, 'failure.png') });
  throw error;
} finally {
  await fs.writeFile(path.join(out, 'results.json'), JSON.stringify(result, null, 2));
  await browser.close();
}
console.log(JSON.stringify(result, null, 2));
