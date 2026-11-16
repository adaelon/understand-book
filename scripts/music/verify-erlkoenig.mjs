import { createRequire } from 'node:module';
import { readFileSync, writeFileSync, existsSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
const require = createRequire(new URL('../../packages/web/package.json', import.meta.url));
const { chromium } = require('@playwright/test');
const root = fileURLToPath(new URL('../../.understand-book/erlkoenig/', import.meta.url));
const origin = 'http://127.0.0.1:8895';
const output = path.join(root, 'verification');
const browser = await chromium.launch({ headless: true });
const nativeIndex = JSON.parse(readFileSync(path.join(root, 'workspace/music/native-index.json'), 'utf8'));
let report = { checks: {}, errors: [], full_playback: [] };
const previousPath = path.join(output, 'browser.json');
const previous = existsSync(previousPath) ? JSON.parse(readFileSync(previousPath, 'utf8')) : null;
const seek = async (page, ms) => page.getByRole('slider', { name: '音乐播放位置', exact: true }).evaluate((input, value) => {
  input.value = String(value); input.dispatchEvent(new Event('input', { bubbles: true }));
}, ms);
async function open(page, mobile = false) {
  page.on('pageerror', error => report.errors.push(error.message));
  // Music verification uses the normal remembered-tour preference. The tour's
  // delayed preparation changes the mobile panel and has its own UI tests.
  await page.addInitScript(() => localStorage.setItem('understand-book.product-tour.v1.local.reader', 'skipped'));
  await page.goto(origin, { waitUntil: 'domcontentloaded' });
  await page.locator('.music-heading').waitFor({ state: 'attached' });
  await page.getByText('正在恢复对话，可以先听音乐或查看乐谱。', { exact: true }).waitFor({ state: 'hidden', timeout: 20000 });
  const skip = page.getByRole('button', { name: '跳过引导', exact: true });
  if (await skip.isVisible()) await skip.click();
  const back = page.getByRole('button', { name: '返回聆听', exact: true });
  if (mobile && await back.isVisible()) await back.click();
  await page.locator('.music-score-page svg').first().waitFor({ timeout: 15000 });
  await page.waitForFunction(() => document.querySelector('audio')?.readyState >= 1);
}
try {
  const context = await (await fetch(`${origin}/api/book/music_context`)).json();
  console.log('music_context_ready');
  if (previous?.publication?.publication_id === context.publication.publication_id && previous.checks?.full_playback_ended) {
    report = previous;
    report.errors = [];
  }
  report.publication = context.publication;
  report.duration = context.manifest.recording.duration_ms;
  report.checks.full_note_index = context.manifest.score_objects.filter(note => !note.rest).length === 5309 && context.manifest.note_alignments.length === 5309 && context.manifest.occurrences.length === 148;
  report.checks.indexed_rests = context.manifest.score_objects.filter(note => note.rest).length === 165;
  report.checks.written_long_duration = ['pdf-head-159', 'pdf-head-160'].every(id => context.manifest.score_objects.find(note => note.source_id === id)?.duration_beats === 3);
  report.checks.andante_position = context.manifest.score_marks.find(mark => mark.mark_id === 'direction-147')?.beat === 3;
  report.checks.no_collapsed_bars = [28,32,108,112,114].every(measure => { const o = context.manifest.occurrences.find(o => o.measure === String(measure)); return o.end_ms - o.start_ms > 600 && o.end_ms - o.start_ms < 4000; });
  const pdfResponse = await fetch(`${origin}/api/books/${context.publication.book_id}/publications/${context.publication.publication_id}/music/source`);
  const pdfBytes = Buffer.from(await pdfResponse.arrayBuffer());
  report.checks.original_pdf_exact = pdfResponse.ok && pdfBytes.equals(readFileSync(path.join(root, 'workspace/music/score/original.pdf')));
  const audioRange = await fetch(new URL(context.audio_url, origin), { headers: { Range: 'bytes=0-31' } });
  report.checks.audio_range = audioRange.status === 206 && (await audioRange.arrayBuffer()).byteLength === 32;
  const page = await browser.newPage({ viewport: { width: 1440, height: 1000 } });
  await open(page);
  console.log('reader_open');
  report.checks.automatic_label = await page.locator('.music-material-tag').textContent().then(text => text.includes('部分跟谱'));
  // Locate every real SVG head against independently recovered PDF coordinates.
  const geometry = previous?.publication?.publication_id === context.publication.publication_id && previous.checks?.native_note_geometry ? previous.geometry : await page.evaluate(async ({ assets, notes, rests }) => {
    let mapped = 0, mappedRests = 0, maximumError = 0;
    for (const asset of assets) {
      const holder = document.createElement('div'); holder.style.cssText = 'position:absolute;left:-10000px;top:0;visibility:hidden';
      holder.innerHTML = await (await fetch(asset.url)).text(); document.body.appendChild(holder);
      const svg = holder.querySelector('svg');
      for (const note of notes.filter(note => `page-${note.page}` === asset.page_id)) {
        const head = svg.querySelector(`#${note.source_id} .notehead`);
        if (!head) throw new Error(`Missing native notehead ${note.source_id}`);
        const box = head.getBBox(), error = Math.hypot(box.x + box.width / 2 - note.center_x, box.y + box.height / 2 - note.y);
        maximumError = Math.max(maximumError, error); mapped++;
      }
      for (const rest of rests.filter(rest => `page-${rest.page}` === asset.page_id)) {
        const glyph = svg.querySelector(`#${rest.source_id} .rest-glyph`), box = glyph?.getBBox();
        if (!box || !box.width || !box.height) throw new Error(`Missing actual rest glyph ${rest.source_id}`);
        mappedRests++;
      }
      holder.remove();
    }
    return { mapped, mappedRests, maximumError };
  }, { assets: context.score_pages, notes: nativeIndex.notes, rests: nativeIndex.rests });
  report.geometry = geometry;
  console.log(JSON.stringify({ geometry }));
  report.checks.native_note_geometry = geometry.mapped === 5309 && geometry.maximumError < .4;
  report.checks.native_rest_geometry = geometry.mappedRests === 165;
  await seek(page, 1800);
  await page.screenshot({ path: path.join(output, 'desktop-final.png') });
  console.log('desktop_screenshot_ready');
  const seen = new Set();
  if (report.checks.full_playback_ended) {
    for (const state of report.full_playback) state.pages.forEach(id => seen.add(id));
  } else {
  await page.getByRole('combobox', { name: '播放速度', exact: true }).selectOption('2');
  await seek(page, 0);
  await page.getByRole('button', { name: '播放音乐', exact: true }).click();
  const started = performance.now();
  while (true) {
    const state = await page.evaluate(() => ({ time: document.querySelector('audio').currentTime,
      ended: document.querySelector('audio').ended, error: document.querySelector('audio').error?.message ?? null,
      pages: [...document.querySelectorAll('.music-score-page > svg')].map(svg => svg.parentElement.dataset.page),
      markers: document.querySelectorAll('.music-note-overlay circle').length,
      visible_markers: [...document.querySelectorAll('.music-note-overlay circle')].filter(circle => Number(circle.style.opacity) > .01).length }));
    report.full_playback.push(state); state.pages.forEach(id => seen.add(id));
    if (report.full_playback.length % 10 === 0) { console.log(JSON.stringify({ playback_seconds: state.time, loaded_pages: seen.size })); writeFileSync(path.join(output, 'browser.json'), JSON.stringify(report, null, 2)); }
    if (state.error) throw new Error(state.error);
    if (!state.markers) throw new Error('The indexed score did not mount note markers');
    if (state.ended) break;
    if (performance.now() - started > 170000) throw new Error('Full playback did not reach its end');
    await page.waitForTimeout(2000);
  }
  report.checks.full_playback_ended = true;
  report.checks.all_nine_pages_loaded = seen.size === 9;
  report.checks.live_note_highlights = report.full_playback.filter(state => state.visible_markers > 0).length > 45;
  }
  await page.getByRole('combobox', { name: '播放速度', exact: true }).selectOption('0.75');
  await page.getByRole('button', { name: '作品导览', exact: true }).click();
  await page.locator('.music-exercises button').filter({ hasText: '把两层线条分开追踪' }).click();
  await page.waitForFunction(() => document.querySelector('audio').currentTime > 136.1);
  await page.getByRole('button', { name: '暂停音乐', exact: true }).click();
  await page.locator('[data-page="page-6"] > svg').waitFor();
  report.checks.exercise_seek_page = await page.locator('audio').first().evaluate(audio => audio.currentTime >= 136 && audio.currentTime < 149);
  report.checks.rate = await page.locator('audio').first().evaluate(audio => audio.playbackRate === .75);
  await page.locator('.music-focus-toggle').click();
  await page.getByRole('combobox', { name: '谱面着色方式', exact: true }).selectOption('part');
  await page.locator('.music-voice-options').getByRole('button', { name: '上谱表', exact: true }).click();
  const upper = context.manifest.score_objects.find(object => object.measure === '86' && object.voice.startsWith('1:'));
  const lower = context.manifest.score_objects.find(object => object.measure === '86' && object.voice.startsWith('2:'));
  report.checks.staff_focus = await page.evaluate(({ upper, lower }) => document.getElementById(upper).style.opacity === '1' && document.getElementById(lower).style.opacity === '0.22', { upper: upper.svg_element_id, lower: lower.svg_element_id });
  await page.locator('.music-part-options').getByRole('button', { name: '全部声部', exact: true }).click();
  await page.getByRole('button', { name: '关闭声部选择', exact: true }).click();
  await page.getByRole('button', { name: '循环当前片段', exact: true }).click();
  report.checks.loop = await page.getByRole('button', { name: '循环当前片段', exact: true }).getAttribute('aria-pressed') === 'true';
  await page.getByRole('button', { name: '循环当前片段', exact: true }).click();
  const clicked = context.manifest.score_objects.find(object => object.measure === '86' && !object.rest);
  const aligned = context.manifest.note_alignments.find(note => note.score_object_id === clicked.source_id);
  await seek(page, aligned.start_ms + 700);
  await page.locator(`#${clicked.svg_element_id}`).click();
  report.checks.note_click_seeks_original = Math.abs(await page.locator('audio').first().evaluate(audio => audio.currentTime * 1000) - aligned.start_ms) < 50;
  const unobserved = context.manifest.note_alignments.find(note => note.unobserved);
  const partialMeasure = context.manifest.score_objects.find(o => o.source_id === unobserved.score_object_id).measure;
  const observed = context.manifest.note_alignments.find(note => !note.unobserved && context.manifest.score_objects.find(o => o.source_id === note.score_object_id)?.measure === partialMeasure);
  async function dotOpacity(note) {
    const object = context.manifest.score_objects.find(object => object.source_id === note.score_object_id);
    await seek(page, note.start_ms + 5);
    await page.locator(`#${object.svg_element_id}`).waitFor();
    await page.locator(`[data-page="${object.page_id}"] .music-note-overlay`).waitFor();
    const index = context.manifest.score_objects.filter(o => o.page_id === object.page_id && !o.rest).findIndex(o => o.source_id === object.source_id);
    return page.locator(`[data-page="${object.page_id}"] .music-note-overlay circle`).nth(index * 2 + 1).evaluate(dot => Number(dot.style.opacity));
  }
  report.checks.unobserved_hidden = await dotOpacity(unobserved) === 0;
  report.checks.observed_in_partial_bar_visible = await dotOpacity(observed) > 0;
  for (const measure of [112,114]) {
    const occurrence = context.manifest.occurrences.find(o => o.measure === String(measure));
    await seek(page, occurrence.start_ms + 10);
    await page.getByRole('button', { name: '回到播放位置', exact: true }).click();
    await page.waitForFunction(measure => document.querySelector('.music-follow-toggle')?.textContent?.includes(`第 ${measure} 小节`), measure);
  }
  report.checks.corrected_bar_follow = true;
  await seek(page, 240000);
  await page.getByRole('button', { name: '回到播放位置', exact: true }).click();
  await page.locator('[data-page="page-9"] > svg').waitFor();
  await page.screenshot({ path: path.join(output, 'ending-final.png') });
  report.checks.ending_page = true;
  const mobile = await browser.newPage({ viewport: { width: 390, height: 844 } });
  await open(mobile, true);
  await mobile.getByRole('button', { name: '播放音乐', exact: true }).click();
  await mobile.waitForFunction(() => document.querySelector('audio').currentTime > 1.6);
  await mobile.getByRole('button', { name: '暂停音乐', exact: true }).click();
  await mobile.locator('.music-score-page svg').first().waitFor();
  report.checks.mobile_playback = true;
  await mobile.screenshot({ path: path.join(output, 'mobile-final.png') });
  const dimensions = await mobile.evaluate(() => ({ page: document.querySelector('.music-score-page').getBoundingClientRect().width,
    surface: document.querySelector('.music-score-surface').clientWidth, document: document.documentElement.scrollWidth, viewport: innerWidth }));
  report.mobile = dimensions;
  report.checks.mobile_readable_width = dimensions.page >= 650 && dimensions.page > dimensions.surface;
  report.checks.mobile_no_page_overflow = dimensions.document <= dimensions.viewport;
  if (report.errors.length || Object.values(report.checks).some(value => !value)) throw new Error(JSON.stringify(report.checks));
  console.log(JSON.stringify({ checks: report.checks, pages: [...seen], errors: report.errors, mobile: report.mobile }));
} finally {
  writeFileSync(path.join(output, 'browser.json'), JSON.stringify(report, null, 2));
  await browser.close();
}
