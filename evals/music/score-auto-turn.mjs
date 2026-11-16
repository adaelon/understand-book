import assert from 'node:assert/strict';
import { writeFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { chromium } from 'playwright';

// Play the recording continuously. Stored progress sets the opening position only.
export async function measureAutoTurns(page, { entry, owner = 'local', manifest, start = 20000, end = 56000, delay = 5000 }) {
  const width = page.viewportSize().width;
  const objects = new Map(manifest.score_objects.map(object => [object.source_id, object]));
  const turns = [];
  for (const bar of manifest.occurrences) {
    const nodes = bar.score_object_ids.map(id => objects.get(id)).filter(Boolean);
    const pageId = nodes[0]?.page_id;
    if (pageId && pageId !== turns.at(-1)?.page) turns.push({ page: pageId, startMs: bar.start_ms, bar: bar.measure,
      headIds: nodes.filter(node => node.page_id === pageId && !node.rest).slice(0, 8).map(node => node.svg_element_id) });
  }
  const first = turns.filter(turn => turn.startMs <= start).at(-1);
  const checked = turns.map((turn, index) => ({ ...turn, previousPageMs: turns[index - 1]?.startMs ?? 0 }))
    .filter(turn => start < turn.startMs && turn.startMs < end);
  assert(first && checked.length, 'The recording interval must contain a page turn');
  page.setDefaultTimeout(30000);
  const errors = [];
  page.on('pageerror', error => errors.push(error.message));
  const recording = manifest.recording.source;
  await page.addInitScript(({ key, tourKey, start, firstPage, delay }) => {
    localStorage.setItem(tourKey, 'skipped');
    localStorage.setItem(key, JSON.stringify({ ms: start, rate: 1 }));
    const request = window.fetch.bind(window);
    window.turnRequests = [];
    window.fetch = async (...args) => {
      const id = String(args[0]?.url ?? args[0]).split('/music/score/')[1];
      if (!id) return request(...args);
      const atMs = () => Math.round((document.querySelector('.music-reader > audio')?.currentTime ?? 0) * 1000);
      const entry = { page: id, startMs: atMs() };
      window.turnRequests.push(entry);
      const [response] = await Promise.all([request(...args), new Promise(resolve => setTimeout(resolve, id === firstPage ? 0 : delay))]);
      entry.responseMs = atMs();
      return response;
    };
  }, { key: `music-position:${owner}:${recording.recording_id}:${recording.revision}`, tourKey: `understand-book.product-tour.v1.${encodeURIComponent(owner)}.reader`, start, firstPage: first.page, delay });
  await page.goto(entry, { waitUntil: 'domcontentloaded' });
  await page.locator('.music-reader').waitFor({ state: 'attached' });
  const back = page.getByRole('button', { name: '返回聆听', exact: true });
  if (width < 600 && await back.isVisible()) await back.click();
  const focus = page.getByRole('button', { name: '专注聆听', exact: true });
  if (await focus.isVisible()) await focus.click();
  await page.locator(`[data-page="${first.page}"] > svg`).waitFor();
  await page.waitForFunction(start => Math.abs(document.querySelector('.music-reader > audio').currentTime * 1000 - start) < 100, start);
  await page.evaluate(({ checked, end }) => {
    const audio = document.querySelector('.music-reader > audio');
    const surface = document.querySelector('.music-score-surface');
    const outcomes = checked.map(turn => ({ ...turn, readyAtMs: null, noteInViewAtMs: null, blankMs: 0 }));
    window.turnOutcomes = outcomes;
    let previous = null;
    function sample() {
      const ms = Math.round(audio.currentTime * 1000), view = surface.getBoundingClientRect();
      for (const result of outcomes) {
        // A repeat may revisit the same SVG. Measure this turn after the preceding page begins.
        if (ms < result.previousPageMs || result.noteInViewAtMs !== null) continue;
        const host = surface.querySelector(`[data-page="${result.page}"]`), box = host.getBoundingClientRect();
        const svg = host.querySelector(':scope > svg');
        if (!svg?.querySelector('.music-note-overlay')) result.readyAtMs = null;
        else if (result.readyAtMs === null) result.readyAtMs = ms;
        if (!audio.paused && !svg && previous !== null && Math.min(box.bottom, view.bottom) - Math.max(box.top, view.top) > view.height / 2) result.blankMs += Math.max(0, ms - previous);
        if (ms >= result.startMs && result.noteInViewAtMs === null && svg && result.headIds.some(id => {
          const r = svg.getElementById(id)?.getBoundingClientRect();
          return r && r.bottom > view.top && r.top < view.bottom && r.right > view.left && r.left < view.right;
        })) result.noteInViewAtMs = ms;
      }
      previous = ms;
      if (ms < end) requestAnimationFrame(sample);
    }
    sample();
  }, { checked, end });
  await page.getByRole('button', { name: '播放音乐', exact: true }).click();
  await page.waitForFunction(end => document.querySelector('.music-reader > audio').currentTime * 1000 >= end, end,
    { timeout: end - start + 40000 });
  await page.getByRole('button', { name: '暂停音乐', exact: true }).click();
  const result = await page.evaluate(() => ({ turns: window.turnOutcomes.map(t => ({
    page: t.page, measure: t.bar, boundaryMs: t.startMs, readyLeadMs: t.readyAtMs === null ? null : t.startMs - t.readyAtMs,
    visibleLagMs: t.noteInViewAtMs === null ? null : t.noteInViewAtMs - t.startMs, blankMs: t.blankMs,
  })), requests: window.turnRequests }));
  return { ...result, entry, width, start, end, delay, errors };
}
export function assertAutoTurns(result) {
  assert.deepEqual(result.errors, []);
  for (const turn of result.turns) {
    assert(turn.readyLeadMs > 0, `${turn.page} was not prepared before the music reached it`);
    assert.equal(turn.blankMs, 0, `An empty ${turn.page} scrolled into view`);
    assert(turn.visibleLagMs !== null && turn.visibleLagMs < 500, `The first notes of ${turn.page} appeared late`);
  }
}
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const origin = process.env.SCORE_ORIGIN ?? 'http://127.0.0.1:8895';
  const width = Number(process.env.SCORE_WIDTH ?? 1419);
  const context = await (await fetch(origin + '/api/book/music_context')).json();
  const browser = await chromium.launch();
  try {
    const page = await browser.newPage({ viewport: { width, height: width < 600 ? 844 : 858 } });
    const result = await measureAutoTurns(page, { entry: origin, manifest: context.manifest,
      start: Number(process.env.SCORE_START_MS ?? 20000), end: Number(process.env.SCORE_END_MS ?? 56000), delay: Number(process.env.SCORE_LATENCY ?? 5000) });
    console.log(JSON.stringify(result));
    if (process.env.SCORE_REPORT) await writeFile(process.env.SCORE_REPORT, JSON.stringify(result, null, 2));
    if (process.env.SCORE_SCREENSHOT) await page.screenshot({ path: process.env.SCORE_SCREENSHOT });
    if (!process.env.SCORE_BASELINE) assertAutoTurns(result);
  } finally { await browser.close(); }
}
