import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import path from 'node:path';
import { chromium } from 'playwright';

const base = process.env.SCORE_NAV_ORIGIN ?? 'http://127.0.0.1:4185';
const fixture = path.resolve('.understand-book/beethoven9-tennstedt/m1/music-context.json').replaceAll('\\', '/');
const { manifest } = JSON.parse(await fs.readFile(fixture, 'utf8'));
const out = path.resolve('docs/evidence/reader-feedback-20261111');
await fs.mkdir(out, { recursive: true });
const result = { checks: [], geometry: [], errors: [] };
const browser = await chromium.launch({ args: ['--mute-audio'] });
const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
page.on('pageerror', error => result.errors.push(error.message));
const occurrence = measure => manifest.occurrences.find(item => item.measure === String(measure));
const pageId = measure => manifest.score_objects.find(object => object.source_id === occurrence(measure).score_object_ids[0]).page_id;
const sheet = measure => page.locator(`.music-score-page[data-page="${pageId(measure)}"]`);
async function seek(measure, offset = 100) {
  const ms = occurrence(measure).start_ms + offset;
  await page.getByLabel('音乐播放位置', { exact: true }).evaluate((input, ms) => {
    input.value = String(ms); input.dispatchEvent(new Event('input', { bubbles: true }));
  }, ms);
  await sheet(measure).locator('svg').waitFor({ state: 'attached' });
}
async function wholePage(measure) {
  await page.waitForFunction(id => {
    const surface = document.querySelector('.music-score-surface').getBoundingClientRect();
    const paper = document.querySelector(`.music-score-page[data-page="${id}"]`).getBoundingClientRect();
    return paper.height > 20 && paper.top >= surface.top - 2 && paper.bottom <= surface.bottom + 2
      && paper.left >= surface.left - 2 && paper.right <= surface.right + 2;
  }, pageId(measure));
  assert.equal(await page.locator('.music-follow-toggle.is-detached').count(), 0);
  result.geometry.push(await sheet(measure).evaluate(element => {
    const paper = element.getBoundingClientRect(), view = element.closest('.music-score-surface').getBoundingClientRect();
    return { viewport: [innerWidth, innerHeight], page: element.dataset.page, paper: { width: paper.width, height: paper.height, top: paper.top, bottom: paper.bottom }, surface: { width: view.width, height: view.height, top: view.top, bottom: view.bottom } };
  }));
}
try {
  await page.addInitScript(({ recording, ms }) => localStorage.setItem(`music-position:visual-review:${recording.recording_id}:${recording.revision}`,
    JSON.stringify({ ms, rate: 1 })), { recording: manifest.recording.source, ms: occurrence(69).start_ms + 100 });
  await page.goto(`${base}/music-reader-visual.html?fixture=/@fs/${fixture}`);
  await sheet(69).locator('svg').waitFor({ state: 'attached' });
  await page.waitForFunction(() => document.querySelector('audio')?.readyState >= 2);
  const width = (await sheet(69).boundingBox()).width;
  await page.getByRole('button', { name: '缩小乐谱', exact: true }).click();
  await page.waitForFunction(width => document.querySelector('.music-score-page').getBoundingClientRect().width < width * .8, width);
  assert.equal(await page.getByLabel('乐谱缩放', { exact: true }).inputValue(), '75');
  assert.equal(await page.locator('.music-follow-toggle.is-detached').count(), 0);
  await page.getByLabel('乐谱缩放', { exact: true }).selectOption('25');
  assert((await sheet(69).boundingBox()).width < width * .3);
  result.checks.push('75% and 25% actually shrink the total score and keep follow mode');

  await page.getByLabel('乐谱缩放', { exact: true }).selectOption('page');
  await wholePage(69);
  await page.locator('.music-instrument-list [data-part="P52"]').click();
  await wholePage(69);
  await page.screenshot({ path: path.join(out, 'score-fit-page-desktop.png') });
  result.checks.push('fit page includes all orchestral staves, even after selecting double bass');

  await seek(72, Math.max(0, occurrence(72).end_ms - occurrence(72).start_ms - 1200));
  await page.getByRole('button', { name: '播放音乐', exact: true }).click();
  await page.waitForFunction(ms => document.querySelector('audio').currentTime * 1000 > ms, occurrence(73).start_ms + 250);
  await page.getByRole('button', { name: '暂停音乐', exact: true }).click();
  await wholePage(73);
  result.checks.push('real recording crosses page 9 to page 10 with the complete next page visible');

  for (const viewport of [{ width: 1280, height: 600 }, { width: 390, height: 844 }, { width: 320, height: 568 }]) {
    await page.setViewportSize(viewport);
    await wholePage(73);
    assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth));
    const control = await page.getByLabel('乐谱缩放', { exact: true }).boundingBox();
    assert(control.x >= 0 && control.x + control.width <= viewport.width);
    if (viewport.width <= 600) {
      const navigator = await page.locator('.music-instrument-nav').boundingBox();
      const paper = await sheet(73).boundingBox();
      assert(navigator.y >= paper.y + paper.height, 'collapsed instrument control must not cover a stave');
    } else {
      const fitWidth = (await sheet(73).boundingBox()).width;
      await page.getByRole('button', { name: '缩小乐谱', exact: true }).click();
      assert((await sheet(73).boundingBox()).width < fitWidth, 'zooming out from a small fitted page must shrink it');
      await page.getByLabel('乐谱缩放', { exact: true }).selectOption('page');
      await wholePage(73);
    }
    await page.screenshot({ path: path.join(out, `score-fit-page-${viewport.width}.png`) });
  }
  result.checks.push('whole page responds to a short desktop and 390px/320px mobile viewports');

  await page.setViewportSize({ width: 1440, height: 900 });
  await page.getByRole('button', { name: '放大乐谱', exact: true }).click();
  assert.notEqual(await page.getByLabel('乐谱缩放', { exact: true }).inputValue(), 'page');
  assert.equal(await page.locator('.music-follow-toggle.is-detached').count(), 0);
  await page.getByLabel('乐谱缩放', { exact: true }).selectOption('page');
  await wholePage(73);
  await page.getByRole('button', { name: '框选谱面提问', exact: true }).click();
  const box = await sheet(73).boundingBox();
  await page.mouse.move(box.x + box.width * .2, box.y + box.height * .2);
  await page.mouse.down();
  await page.mouse.move(box.x + box.width * .8, box.y + box.height * .45, { steps: 12 });
  await page.mouse.up();
  await page.getByText('已附谱面图片，可以直接问「这是什么」', { exact: true }).waitFor();
  assert.equal(await page.locator('.music-selection-hint[role="alert"]').count(), 0);
  result.checks.push('region capture still produces a question image at fit-page scale');
  assert.deepEqual(result.errors, []);
  console.log(JSON.stringify(result, null, 2));
} finally {
  await fs.writeFile(path.join(out, 'music-browser.json'), JSON.stringify(result, null, 2));
  await browser.close();
}
