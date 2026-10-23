// Real score + production App/components. Admission is intercepted so this UI check makes no model calls.
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import path from 'node:path';
import { chromium } from 'playwright';
import { createCanvas, loadImage } from '@napi-rs/canvas';

const option = (key, fallback) => process.argv.includes(key) ? process.argv[process.argv.indexOf(key) + 1] : fallback;
const base = option('--url', 'http://127.0.0.1:4195');
const fixturePath = path.resolve(option('--fixture', 'evals/music/results/music-review-context.json'));
const music = JSON.parse(await fs.readFile(fixturePath, 'utf8'));
const book = music.publication.book_id;
const out = path.resolve('evals/music/results/question-images-20261023');
await fs.mkdir(out, { recursive: true });
const browser = await chromium.launch({ headless: true });
const result = { fixture: fixturePath, surfaces: [], checks: [], errors: [] };
try {
  for (const viewport of [{ width: 1500, height: 1000 }, { width: 390, height: 844 }]) {
    const context = await browser.newContext({ viewport, permissions: ['clipboard-read', 'clipboard-write'] });
    const page = await context.newPage();
    const submissions = [];
    page.on('pageerror', error => result.errors.push(error.stack));
    await page.route('**/src/main.ts*', async route => {
      const response = await route.fetch();
      const body = (await response.text()).replaceAll('/src/LocalApp.vue', '/src/App.vue');
      await route.fulfill({ response, body });
    });
    await page.route('**/api/**', async route => {
      const endpoint = new URL(route.request().url()).pathname.slice(4);
      const answers = {
        '/desktop/status': { desktop_host: false, reader_only: true, active_book: true },
        '/book/video_playback': null, '/book/video_context': null, '/book/music_context': music,
        '/book/manifest': { tree: [{ lid: '1', kind: 'paragraph', children: [], span: { start: 0, end: 4 } }], stats_by_lid: {} },
        '/book/asset_manifest': { images: [] }, '/book/source_fingerprint': { book_id: book, source_fingerprint: 'music-question-ui-fixture' },
        '/reader/state': { book_id: book, revision: 0, viewport: { anchor_lid: '1', top_lid: '1', bottom_lid: '1', width: 1, visible_lids: ['1'] }, profile: { profile_id: 'technical_learning', profile_version: '1' }, layout: { rev: 0, open_slots: [] } },
        '/profile/manifest': { profile_id: 'technical_learning', slots: [] }, '/profile/memory': null,
        '/profile/backfill': { jobs: [], sessions: [] }, '/memory/recall': [], '/build_intent/artifacts': { overlay: null },
        '/book/text': { lid: '1', text: '作品导览' }, '/book/source_manifest': null,
        '/book/library': { root: 'fixture', books: [] }, '/build_intent/usage.event': {},
        '/agent/history': { active_session_id: 'image-chat', sessions: [], current: { id: 'image-chat', book_id: book, turns: [], goals: [] } },
        '/tutor/state': { control: { enabled: false, revision: 0, current_tutor_session_id: null }, sessions: {} },
        '/tutor/readiness': { status: 'ready', source_id: book, source_revision: '1', limitations: [], reason: '可用', teaching_assets: { status: 'ready', teaching_map_revision: null, limitations: [], reason: '可用' } },
      };
      if (endpoint === '/agent/runs') {
        submissions.push(route.request().postDataJSON());
        await route.fulfill({ status: 409, json: { error_code: 'MUSIC_QUESTION_INVALID', category: 'conflict', message: '浏览器验收：保留这次图片草稿' } });
      } else if (Object.hasOwn(answers, endpoint)) await route.fulfill({ json: answers[endpoint] });
      else await route.fulfill({ status: 404, json: { error_code: 'NOT_FOUND', category: 'not_found', message: 'No fixture for ' + endpoint } });
    });
    await page.goto(base);
    await page.locator('.music-score-page > svg').first().waitFor();
    const screenshotButton = page.getByRole('button', { name: '框选谱面提问', exact: true });
    await screenshotButton.waitFor();
    await page.screenshot({ path: path.join(out, `ready-${viewport.width}.png`) });
    await screenshotButton.click(); await page.keyboard.press('Escape');
    assert.equal(await page.locator('.music-score-surface.is-selecting').count(), 0);
    await screenshotButton.click();
    const surface = await page.locator('.music-score-surface').boundingBox();
    const staff = await page.locator('.music-score-page svg g.staff').first().boundingBox();
    assert(staff, 'The real score must contain notation');
    const left = Math.max(surface.x + 8, staff.x - 8), top = Math.max(surface.y + 8, staff.y - 20);
    const right = Math.min(surface.x + surface.width - 18, left + 580), bottom = Math.min(surface.y + surface.height - 18, Math.max(top + 70, staff.y + staff.height + 20));
    await page.mouse.move(left, top); await page.mouse.down();
    await page.mouse.move(right, bottom, { steps: 8 });
    await page.screenshot({ path: path.join(out, `selecting-${viewport.width}.png`) });
    await page.mouse.up();
    const thumbnail = page.getByAltText('待发送的问题图片', { exact: true });
    await thumbnail.waitFor();
    const data = await thumbnail.getAttribute('src');
    const png = Buffer.from(data.split(',')[1], 'base64');
    const cropPath = path.join(out, `score-crop-${viewport.width}.png`);
    await fs.writeFile(cropPath, png);
    const image = await loadImage(png), canvas = createCanvas(image.width, image.height), ctx = canvas.getContext('2d');
    ctx.drawImage(image, 0, 0);
    const pixels = ctx.getImageData(0, 0, image.width, image.height).data;
    let dark = 0;
    for (let i = 0; i < pixels.length; i += 4) if (pixels[i + 3] > 200 && pixels[i] < 130 && pixels[i + 1] < 130 && pixels[i + 2] < 130) dark++;
    assert(dark > 100, 'The screenshot must contain visible notation, not a blank SVG');
    assert(image.width <= 2048 && image.height <= 2048);
    assert(await page.locator('audio').first().evaluate(audio => audio.paused));
    const textarea = page.locator('textarea[data-workspace-input="agent"]');
    assert.equal(await textarea.inputValue(), '', 'A screenshot can be sent without knowing what to ask');
    await page.screenshot({ path: path.join(out, `attached-${viewport.width}.png`) });
    await page.getByRole('button', { name: '放大问题图片', exact: true }).click();
    await page.getByRole('dialog', { name: '问题图片预览' }).waitFor({ state: 'visible' });
    await page.screenshot({ path: path.join(out, `preview-${viewport.width}.png`) });
    await page.getByRole('button', { name: '复制图片', exact: true }).click();
    await page.getByRole('button', { name: '已复制', exact: true }).waitFor();
    await page.getByRole('button', { name: '关闭图片预览', exact: true }).click();
    const firstSubmission = page.waitForResponse(response => new URL(response.url()).pathname === '/api/agent/runs');
    await page.getByRole('button', { name: '发送', exact: true }).click();
    await firstSubmission;
    await page.waitForFunction(() => document.querySelector('.send-agent:not(:disabled)'));
    assert.equal(submissions.length, 1);
    assert.equal(submissions[0].music_question.image.origin, 'selection');
    assert.equal(submissions[0].music_question.image.png_base64, png.toString('base64'));
    assert(submissions[0].message.includes('我是初学者'));
    assert(await thumbnail.isVisible(), 'A rejected submission must keep its attachment');
    await page.getByRole('button', { name: '移除图片', exact: true }).click();
    assert.equal(await thumbnail.count(), 0);
    await textarea.focus(); await page.keyboard.press('Control+V');
    await thumbnail.waitFor();
    await page.locator('input[aria-label="选择问题图片"]').setInputFiles(cropPath);
    await page.waitForFunction(() => !document.querySelector('.image-loading'));
    await textarea.fill('我完全不识谱，这五条线和上面的符号分别是什么？');
    const secondSubmission = page.waitForResponse(response => new URL(response.url()).pathname === '/api/agent/runs');
    await textarea.press('Control+Enter');
    await secondSubmission;
    await page.waitForFunction(() => document.querySelector('.send-agent:not(:disabled)'));
    assert.equal(submissions.length, 2);
    assert.equal(submissions[1].music_question.image.origin, 'upload');
    assert.equal(submissions[1].music_question.audio.start_ms, submissions[0].music_question.audio.start_ms);
    assert.equal(await textarea.inputValue(), '我完全不识谱，这五条线和上面的符号分别是什么？');
    assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth));
    const compose = await page.locator('.agent-compose-row').boundingBox();
    assert(compose.x >= 0 && compose.x + compose.width <= viewport.width + 1);
    result.surfaces.push({ viewport, image: { width: image.width, height: image.height, dark_pixels: dark }, compose });
    result.checks.push(`${viewport.width}px: real notation crop; pause; image-only request; PNG bytes in submission; preview; clipboard copy/paste; upload replacement; remove; failed draft retained; Ctrl+Enter; frozen range; no horizontal overflow`);
    await page.screenshot({ path: path.join(out, `retained-${viewport.width}.png`) });
    await context.close();
  }
  assert.deepEqual(result.errors, []);
} catch (error) {
  const page = browser.contexts().at(-1)?.pages().at(-1);
  if (page) await page.screenshot({ path: path.join(out, 'failure.png') });
  result.errors.push(error.stack); throw error;
} finally {
  await fs.writeFile(path.join(out, 'verification.json'), JSON.stringify(result, null, 2));
  await browser.close();
}
console.log(JSON.stringify(result, null, 2));
