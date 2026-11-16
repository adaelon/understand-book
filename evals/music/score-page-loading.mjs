import assert from 'node:assert/strict';
import { chromium } from 'playwright';

// The local Erlkönig publication started by scripts/music/start-erlkoenig.ps1.
const origin = 'http://127.0.0.1:8895';
const minimumRequestMs = Number(process.env.SCORE_LATENCY ?? 1000);
const browser = await chromium.launch();
try {
  const page = await browser.newPage({ viewport: { width: 1419, height: 858 } });
  page.setDefaultTimeout(20000);
  const errors = [];
  page.on('pageerror', error => errors.push(error.message));
  await page.addInitScript(minimumMs => {
    localStorage.setItem('understand-book.product-tour.v1.local.reader', 'skipped');
    const request = window.fetch.bind(window);
    window.scoreRequests = [];
    window.fetch = async (...args) => {
      const url = String(args[0]?.url ?? args[0]);
      if (!url.includes('/music/score/')) return request(...args);
      window.scoreRequests.push(url.split('/music/score/')[1]);
      const [response] = await Promise.all([request(...args), new Promise(resolve => setTimeout(resolve, minimumMs))]);
      return response;
    };
  }, minimumRequestMs);
  await page.goto(origin, { waitUntil: 'domcontentloaded' });
  await page.locator('.music-score-surface').waitFor();
  await page.waitForFunction(() => document.querySelector('.music-reader > audio')?.readyState >= 1);
  const focus = page.getByRole('button', { name: '专注聆听', exact: true });
  if (await focus.isVisible()) await focus.click();

  function drawnPage(id) {
    return page.waitForFunction(id => {
      const viewport = document.querySelector('.music-score-surface').getBoundingClientRect();
      const rect = document.querySelector(`[data-page="${id}"] > svg`)?.getBoundingClientRect();
      return rect && Math.min(rect.bottom, viewport.bottom) - Math.max(rect.top, viewport.top) > 50 && performance.now();
    }, id);
  }
  await drawnPage('page-1');
  const measurements = [];
  for (const [timeMs, id] of [[30000, 'page-2'], [158540, 'page-7'], [30000, 'page-2']]) {
    const start = await page.getByRole('slider', { name: '音乐播放位置', exact: true }).evaluate((input, ms) => {
      const start = performance.now();
      input.value = String(ms); input.dispatchEvent(new Event('input', { bubbles: true }));
      return start;
    }, timeMs);
    const drawn = drawnPage(id).then(handle => handle.jsonValue());
    await page.waitForTimeout(170);
    const sample = await page.evaluate(() => {
      const view = document.querySelector('.music-score-surface').getBoundingClientRect();
      const sheets = [...document.querySelectorAll('.music-score-page > svg')].filter(svg => {
        const box = svg.getBoundingClientRect();
        return Math.min(box.bottom, view.bottom) - Math.max(box.top, view.top) > 50;
      });
      return { visible: sheets.map(svg => svg.parentElement.dataset.page), hint: document.querySelector('.music-score-state')?.textContent };
    });
    const elapsed = Math.round(await drawn - start);
    assert(sample.visible.length, `Blank score while seeking to ${timeMs} ms`);
    if (!sample.visible.includes(id)) assert(sample.hint?.includes(id.split('-')[1]), 'Missing destination in the loading hint');
    measurements.push({ timeMs, id, elapsedMs: elapsed, ...sample });
  }
  const requests = await page.evaluate(() => window.scoreRequests);
  assert.equal(requests.filter(id => id === 'page-2').length, 1);
  assert.deepEqual(errors, []);
  console.log(JSON.stringify({ minimumRequestMs, measurements, requests }));
} finally {
  await browser.close();
}
