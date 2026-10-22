// Isolated real-browser verification. Requires a copied unpublished workspace,
// its live server, and Vite with UNDERSTAND_BOOK_ADDR pointing to that server.
// All saved revisions remain pending; browser playback is not human listening.
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import path from 'node:path';
import { chromium } from 'playwright';
const option = (key, fallback) => process.argv.includes(key) ? process.argv[process.argv.indexOf(key) + 1] : fallback;
const url = option('--url', 'http://127.0.0.1:4193');
const name = option('--name', 'schubert');
const out = path.resolve('evals/music/results');
await fs.mkdir(out, { recursive: true });
const browser = await chromium.launch({ headless: true });
const page = await browser.newPage({ viewport: { width: 1440, height: 1000 } });
const errors = []; page.on('pageerror', error => errors.push(error.message));
try {
  if (process.argv.includes('--published')) {
    await page.goto(option('--published-url', 'http://127.0.0.1:6677/'));
    await page.locator('.music-score-page > svg').first().waitFor();
    await page.getByText('这首作品', { exact: false }).first().waitFor();
    assert.equal(await page.locator('[role="alert"]').count(), 0);
    await page.screenshot({ path: path.join(out, 'music-reader-published-app.png') });
    await fs.writeFile(path.join(out, 'music-reader-published-app-check.json'), JSON.stringify({ url: page.url(), errors, body: (await page.locator('body').innerText()).slice(0, 5000), audio: await page.locator('audio').first().evaluate(element => ({ ready_state: element.readyState, duration: element.duration, source: element.currentSrc })) }, null, 2));
  } else if (process.argv.includes('--switch')) {
    const base = path.resolve('.understand-book/music-calibration-qa');
    assert((await page.request.post(`${url}/api/book/open`, { data: { dir: path.join(base, 'schubert') } })).ok());
    const initialContext = await (await page.request.get(`${url}/api/build_workbench/music/context`)).json();
    await page.goto(`${url}/`); await page.locator('.calibration-workbench').waitFor();
    await page.getByRole('button', { name: '返回工作区' }).click();
    async function openWorkspace(directory) {
      await page.getByRole('button', { name: /^(切换材料|打开书)$/ }).click();
      const dialog = page.getByRole('dialog', { name: '选择阅读材料' });
      await dialog.getByText('通过目录打开 / 查看材料位置', { exact: true }).click();
      await dialog.getByPlaceholder('.understand-book/book-id').fill(path.join(base, directory));
      await dialog.getByRole('button', { name: '打开', exact: true }).click();
      await dialog.waitFor({ state: 'hidden' });
    }
    await openWorkspace('beethoven');
    assert.equal((await page.locator('body').innerText()).includes('Publication materials or declared dependencies are invalid'), false);
    assert.equal((await page.request.get(new URL(initialContext.audio_url, url).href)).status(), 404);
    assert.equal((await page.request.get(new URL(initialContext.score_pages[0].url, url).href)).status(), 404);
    await page.getByRole('navigation', { name: '本地音乐校准' }).getByRole('button', { name: /Symphony/ }).waitFor();
    assert.equal(await page.locator('.calibration-workbench').count(), 0);
    const fetched = page.waitForResponse(response => response.url().endsWith('/build_workbench/music/context'));
    await page.getByRole('navigation', { name: '本地音乐校准' }).getByRole('button').click();
    const beethoven = await (await fetched).json();
    assert(new URL(beethoven.audio_url, url).searchParams.has('recording_id'));
    await page.locator('.calibration-svg > svg').waitFor();
    assert.equal(await page.locator('.calibration-header h1').innerText(), beethoven.manifest.work.title);
    await page.waitForFunction(() => document.querySelector('audio')?.readyState >= 1);
    const beethovenDuration = await page.locator('audio').evaluate(audio => audio.duration);
    assert(Math.abs(beethovenDuration - beethoven.manifest.recording.duration_ms / 1000) < 1, `Audio duration ${beethovenDuration}; expected ${beethoven.manifest.recording.duration_ms / 1000}`);
    await page.getByLabel('选择校准音符').selectOption({ index: 1 });
    assert.match(await page.getByLabel('选择校准音符').innerText(), /（实音）/);
    await page.screenshot({ path: path.join(out, 'music-calibration-switched-beethoven.png') });
    await page.getByRole('button', { name: '返回工作区' }).click();
    await openWorkspace('nonmusic');
    assert.equal(await page.getByRole('navigation', { name: '本地音乐校准' }).count(), 0);
    await openWorkspace('schubert');
    await page.getByRole('navigation', { name: '本地音乐校准' }).getByRole('button', { name: /Moments/ }).waitFor();
    assert.equal(await page.locator('.calibration-workbench').count(), 0);
    await page.getByRole('navigation', { name: '本地音乐校准' }).getByRole('button').click();
    await page.locator('.calibration-svg > svg').waitFor();
    await page.waitForFunction(() => document.querySelector('audio')?.readyState >= 1);
    const schubertDuration = await page.locator('audio').evaluate(audio => audio.duration);
    assert(Math.abs(schubertDuration - 101) < 1);
    await page.getByLabel('选择校准音符').selectOption({ index: 1 });
    await page.screenshot({ path: path.join(out, 'music-calibration-switched-schubert.png') });
    assert.equal(errors.length, 0, errors.join('\n'));
    await fs.writeFile(path.join(out, 'music-calibration-workspace-switch.json'), JSON.stringify({ beethoven: { work: beethoven.manifest.work.title, recording: beethoven.manifest.recording.source, actual_duration: beethovenDuration, audio_url: beethoven.audio_url }, schubert_actual_duration: schubertDuration, old_audio_and_score_status: 404, unpublished_reader_projection_error: false, nonmusic_entry_hidden: true, nonmusic_to_music_entry_appeared: true, opened_context_refetched: true, errors }, null, 2));
  } else if (process.argv.includes('--screens-only')) {
    assert((await page.request.post(`${url}/api/book/open`, { data: { dir: path.resolve('.understand-book/music-calibration-qa', name) } })).ok());
    await page.goto(`${url}/`); await page.locator('.calibration-svg > svg').waitFor();
    const boundary = Number(await page.getByLabel('小节终点秒').inputValue());
    // Instrument the native setter without changing it: record the real media
    // clock just before the UI seeks back at the selected loop boundary.
    await page.locator('audio').evaluate(audio => {
      const descriptor = Object.getOwnPropertyDescriptor(HTMLMediaElement.prototype, 'currentTime');
      window.__calibrationLoopCapture = [];
      Object.defineProperty(audio, 'currentTime', { configurable: true, get() { return descriptor.get.call(this); }, set(value) {
        const before = descriptor.get.call(this); descriptor.set.call(this, value);
        if (before > value + .3) window.__calibrationLoopCapture.push({ before, after: value });
      } });
    });
    await page.getByLabel('循环本小节').check();
    await page.getByRole('button', { name: '复听原时间', exact: true }).click();
    await page.waitForFunction(() => window.__calibrationLoopCapture.length > 0);
    const loop = await page.evaluate(() => window.__calibrationLoopCapture[0]);
    loop.boundary = boundary; loop.overshoot_ms = (loop.before - boundary) * 1000;
    assert(loop.overshoot_ms >= 0 && loop.overshoot_ms < 60, `Loop overshoot ${loop.overshoot_ms}ms`);
    await page.getByRole('button', { name: '暂停', exact: true }).click();
    await page.setViewportSize({ width: 390, height: 844 });
    await page.screenshot({ path: path.join(out, `music-calibration-${name}-mobile.png`) });
    const button = await page.getByRole('button', { name: '返回工作区' }).boundingBox();
    assert(button.x >= 0 && button.x + button.width <= 390);
    await page.locator('.calibration-save-panel').scrollIntoViewIfNeeded();
    await page.screenshot({ path: path.join(out, `music-calibration-${name}-mobile-save.png`) });
    await fs.writeFile(path.join(out, `music-calibration-${name}-final-ui.json`), JSON.stringify({ loop, return_button: button, errors }, null, 2));
  } else {
    const before = await (await page.request.get(`${url}/api/build_workbench/music/context`)).json();
    assert(before && !before.publication && before.manifest.occurrences.length > 0);
    await page.goto(`${url}/music-calibration-visual.html`);
    await page.locator('.calibration-svg > svg').waitFor();
    const occurrence = before.manifest.occurrences[0];
    const object = before.manifest.score_objects.find(item => occurrence.score_object_ids.includes(item.source_id) && !item.rest);
    await page.locator(`[id="${object.svg_element_id}"] .notehead`).first().click();
    assert.equal(await page.getByLabel('选择校准音符').inputValue(), object.source_id);
    await page.getByLabel('复听速度').selectOption('0.75');
    await page.getByRole('button', { name: '复听原时间', exact: true }).click();
    await page.waitForFunction(start => { const audio = document.querySelector('audio'); return audio && !audio.paused && audio.currentTime * 1000 > start + 150; }, occurrence.start_ms);
    const playback = await page.locator('audio').evaluate(element => ({ current_ms: element.currentTime * 1000, paused: element.paused, rate: element.playbackRate, ready_state: element.readyState }));
    await page.getByRole('button', { name: '暂停', exact: true }).click();
    await page.getByLabel('窗口结束小节').selectOption('1');
    const changed = (occurrence.end_ms + 10) / 1000;
    await page.getByLabel('小节终点秒').fill(String(changed));
    await page.getByLabel('当前小节出现').selectOption(before.manifest.occurrences[1].occurrence_id);
    assert.equal(Number(await page.getByLabel('小节起点秒').inputValue()), changed);
    await page.getByRole('button', { name: '放弃草稿修改' }).click();
    await page.getByLabel('选择校准音符').selectOption(object.source_id);
    await page.screenshot({ path: path.join(out, `music-calibration-${name}-desktop.png`) });
    const reason = '界面流程验证，候选保留待检查';
    await page.getByLabel('修改原因', { exact: true }).fill(reason);
    await page.getByLabel('时间判断的依据').fill('浏览器流程验证，沿用原候选时间；未进行人工复听核准。');
    const responsePromise = page.waitForResponse(response => response.url().endsWith('/build_workbench/music/calibrate') && response.request().method() === 'POST');
    await page.getByRole('button', { name: '保存本地修订' }).click();
    const response = await responsePromise;
    assert(response.ok(), await response.text());
    const after = await response.json();
    await page.getByRole('status').filter({ hasText: '已保存到本地工作区' }).waitFor();
    assert.equal(after.history.length, before.history.length + 1);
    const revision = after.history.at(-1).revision;
    assert(revision.coverage.every(item => item.status === 'pending'));
    assert.equal(revision.provenance[0].kind, 'interpretation');
    await page.reload(); await page.locator('.calibration-svg > svg').waitFor();
    await page.locator('.calibration-history').getByText(reason, { exact: false }).first().waitFor();
    await page.locator('.calibration-history').scrollIntoViewIfNeeded();
    await page.screenshot({ path: path.join(out, `music-calibration-${name}-history.png`) });
    await page.setViewportSize({ width: 390, height: 844 });
    await page.reload(); await page.locator('.calibration-svg > svg').waitFor();
    assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth));
    await page.screenshot({ path: path.join(out, `music-calibration-${name}-mobile.png`) });
    await page.locator('.calibration-save-panel').scrollIntoViewIfNeeded();
    await page.screenshot({ path: path.join(out, `music-calibration-${name}-mobile-save.png`) });
    assert.equal(errors.length, 0, errors.join('\n'));
    await page.setViewportSize({ width: 1440, height: 1000 });
    await page.goto(`${url}/`); await page.locator('.calibration-workbench').waitFor();
    await page.getByRole('button', { name: '返回工作区', exact: true }).click();
    await page.getByRole('navigation', { name: '本地音乐校准' }).getByRole('button').click();
    await page.locator('.calibration-svg > svg').waitFor();
    await page.screenshot({ path: path.join(out, `music-calibration-${name}-local-app.png`) });
    await fs.writeFile(path.join(out, `music-calibration-${name}-ui.json`), JSON.stringify({ url, work: before.manifest.work.title, occurrences: before.manifest.occurrences.length, pages: before.score_pages.length, publication: null, playback, shared_boundary_changed_and_discarded: true, saved_pending_revision: revision, history_before: before.history.length, history_after: after.history.length, reopened_history: true, local_app_open_close_reopen: true, mobile_overflow: false, errors }, null, 2));
  }
} catch (error) {
  await page.screenshot({ path: path.join(out, `music-calibration-${name}-failure.png`) });
  await fs.writeFile(path.join(out, `music-calibration-${name}-failure.json`), JSON.stringify({ error: error.stack, pageErrors: errors, body: await page.locator('body').innerText(), audio: await page.locator('audio').evaluateAll(elements => elements.map(audio => ({ duration: audio.duration, source: audio.currentSrc, readyState: audio.readyState }))) }, null, 2));
  throw error;
} finally { await browser.close(); }
