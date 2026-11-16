// Real MediaRecorder -> local ASR -> editable review -> question draft and tutor.
// The WAV is supplied to Chromium's fake microphone, not substituted at the API.
const { chromium, expect: baseExpect } = require('@playwright/test');
const expect = baseExpect.configure({ timeout: 120_000 });
const { mkdir, writeFile } = require('node:fs/promises');
const { join, resolve } = require('node:path');
const assert = require('node:assert/strict');

async function main() {
  const [url, output, audioPath] = process.argv.slice(2);
  assert(url && output && audioPath, 'Pass isolated reader URL, output directory, and a Mandarin WAV.');
  await mkdir(output, { recursive: true });
  const browser = await chromium.launch({ args: ['--use-fake-ui-for-media-stream', '--use-fake-device-for-media-stream', `--use-file-for-fake-audio-capture=${resolve(audioPath)}`] });
  const report = { audioSource: 'Mandarin speech synthesized by Microsoft Huihui; browser MediaRecorder and local ASR are real.', recognition: [], ask: null };
  let activePage;
  const card = page => page.locator('article[data-delivery-ref$=":boundary"]');
  async function open(context) {
    const page = await context.newPage(); activePage = page; page.setDefaultTimeout(120_000);
    page.on('pageerror', error => console.error('Page error:', error.message));
    page.on('requestfailed', request => console.error('Request failed:', request.url(), request.failure()?.errorText));
    await page.goto(url, { waitUntil: 'domcontentloaded' }); await card(page).waitFor({ state: 'attached' });
    const tour = page.getByRole('button', { name: '跳过引导' }); if (await tour.isVisible()) await tour.click();
    const fullscreen = page.getByRole('button', { name: '问答全屏', exact: true });
    if (await fullscreen.isVisible()) await fullscreen.click();
    return page;
  }
  async function dictate(page, target, label) {
    const widget = target.locator('.voice-input');
    await widget.getByRole('button', { name: '语音输入', exact: true }).click();
    await expect(widget).toContainText('正在录音');
    // Capture the entire 5.5-second spoken sample through the browser microphone.
    await page.waitForTimeout(5900);
    const started = Date.now();
    const response = page.waitForResponse(r => r.url().endsWith('/api/reader/voice-transcribe'));
    await widget.getByRole('button', { name: '停止并转文字' }).click();
    const result = await response, value = await result.json();
    assert(result.ok(), JSON.stringify(value));
    report.recognition.push({ label, elapsed_ms: Date.now() - started, ...value });
    await expect(widget.getByRole('textbox', { name: '识别结果' })).toBeVisible();
    assert(value.text.includes('最小值'), JSON.stringify(value));
    return widget;
  }
  try {
    const desktop = await browser.newContext({ viewport: { width: 1440, height: 960 } });
    const page = await open(desktop), q = card(page), editor = q.locator('textarea[data-workspace-input="tutor"]');
    await editor.fill('我先写下可行条件 x ≥ 3。');
    const attemptBefore = await q.locator('.attempts summary').allTextContents();
    const widget = await dictate(page, q, 'calculation-desktop');
    await expect(editor).toHaveValue('我先写下可行条件 x ≥ 3。');
    await expect(q.getByRole('button', { name: '提交解答', exact: true })).toBeDisabled();
    await editor.fill('我先写下可行条件 x ≥ 3。\n这里是录音时补写的文字。');
    const corrected = '我认为最小值是 4，但是不知道在哪个点取得。';
    await widget.getByRole('textbox', { name: '识别结果' }).fill(corrected);
    await q.screenshot({ path: join(output, 'voice-desktop.png') });
    await widget.getByRole('button', { name: '加入输入框' }).click();
    const draft = '我先写下可行条件 x ≥ 3。\n这里是录音时补写的文字。\n' + corrected;
    await expect(editor).toHaveValue(draft);
    assert.deepEqual(await q.locator('.attempts summary').allTextContents(), attemptBefore);
    await page.reload({ waitUntil: 'domcontentloaded' }); await expect(editor).toHaveValue(draft);
    const fullscreen = page.getByRole('button', { name: '问答全屏', exact: true });
    if (await fullscreen.isVisible()) await fullscreen.click();
    const response = page.waitForResponse(r => r.url().endsWith('/api/tutor/ask-step'), { timeout: 120_000 });
    await q.locator('.ask').click();
    const reply = await response, value = await reply.json(); assert(reply.ok(), JSON.stringify(value));
    report.ask = { text: reply.request().postDataJSON().text, reply: value.text };
    assert.equal(report.ask.text, draft);
    await expect(q.locator('[data-step-help]').last()).toBeVisible({ timeout: 120_000 });
    assert.deepEqual(await q.locator('.attempts summary').allTextContents(), attemptBefore);
    await q.screenshot({ path: join(output, 'voice-guidance.png') });

    const mobile = await browser.newContext({ viewport: { width: 390, height: 844 }, isMobile: true, hasTouch: true });
    const phone = await open(mobile), mq = card(phone);
    const mw = await dictate(phone, mq, 'calculation-mobile');
    await mw.scrollIntoViewIfNeeded();
    await phone.screenshot({ path: join(output, 'voice-mobile.png') });
    assert(await phone.evaluate(() => document.documentElement.scrollWidth <= innerWidth + 1));
    await mw.getByRole('button', { name: '加入输入框' }).click();
    await expect(mq.locator('textarea[data-workspace-input="tutor"]')).not.toHaveValue('');

    const composer = page.locator('.agent-compose-row').filter({ visible: true });
    const chatInput = composer.locator('textarea[data-workspace-input="agent"]');
    await chatInput.fill('这是我的聊天草稿。');
    const cw = await dictate(page, composer, 'chat-desktop');
    await expect(composer.locator('.send-agent')).toBeDisabled();
    await cw.getByRole('button', { name: '加入输入框' }).click();
    assert((await chatInput.inputValue()).startsWith('这是我的聊天草稿。\n'));
    await composer.screenshot({ path: join(output, 'voice-chat.png') });
    await writeFile(join(output, 'voice.json'), JSON.stringify(report, null, 2));
    console.log(JSON.stringify(report, null, 2));
  } catch (error) {
    if (activePage) { await activePage.screenshot({ path: join(output, 'voice-failure.png') }); console.error((await activePage.locator('body').innerText()).slice(-5000)); }
    throw error;
  } finally { await browser.close(); }
}
main().catch(error => { console.error(error); process.exitCode = 1; });
