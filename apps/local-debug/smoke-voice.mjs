// Start the Windows app extracted from Setup with a blank profile and record
// synthetic Chinese speech through the Chromium microphone capture path.
import assert from 'node:assert/strict';
import { execFile, spawn } from 'node:child_process';
import { copyFile, mkdir, mkdtemp, readFile, stat, writeFile } from 'node:fs/promises';
import { createServer } from 'node:net';
import path from 'node:path';
import { parseArgs, promisify } from 'node:util';
import { chromium, expect } from '@playwright/test';

const { values: options } = parseArgs({ options: {
  executable: { type: 'string' },
  out: { type: 'string' },
  audio: { type: 'string' },
  'model-dir': { type: 'string' },
} });
assert.equal(process.platform, 'win32', 'This smoke test exercises the Windows WebView2 bundle.');
assert(options.executable, 'Pass --executable <UnderstandBookLocal.exe extracted from Setup>.');
const executable = path.resolve(options.executable);
await stat(executable);
const root = options.out ? path.resolve(options.out) : await mkdtemp(path.resolve('target/local-voice-smoke-'));
if (options.out) await mkdir(root, { recursive: true });
const data = path.join(root, 'data');
await mkdir(data);
const audio = path.join(root, 'microphone.wav');
const transcriptWords = /[读讀][书書].*[学學][习習]/;
const windows = process.env.SystemRoot;
const powershell = path.join(windows, 'System32/WindowsPowerShell/v1.0/powershell.exe');
const execute = promisify(execFile);

if (options.audio) {
  await copyFile(path.resolve(options.audio), audio);
} else {
  const script = path.join(root, 'make-microphone.ps1');
  // Windows PowerShell 5 needs a UTF-8 BOM for the Chinese fixture.
  await writeFile(script, `\uFEFFparam([string]$Audio)
Add-Type -AssemblyName System.Speech
$speaker = [System.Speech.Synthesis.SpeechSynthesizer]::new()
try {
  $speaker.SelectVoiceByHints([System.Speech.Synthesis.VoiceGender]::NotSet, [System.Speech.Synthesis.VoiceAge]::NotSet, 0, [CultureInfo]::GetCultureInfo('zh-CN'))
  $format = [System.Speech.AudioFormat.SpeechAudioFormatInfo]::new(16000, [System.Speech.AudioFormat.AudioBitsPerSample]::Sixteen, [System.Speech.AudioFormat.AudioChannel]::Mono)
  $speaker.SetOutputToWaveFile($Audio, $format)
  $speaker.Speak('今天我在读书。这个例子说明了什么是有效的学习。')
} finally { $speaker.Dispose() }
`);
  await execute(powershell, ['-NoProfile', '-File', script, audio], { windowsHide: true });
}
// The fixture and any --audio copy are mono 16 kHz, signed 16-bit PCM WAV.
const recordingMs = Math.ceil((await stat(audio)).size / 32) + 300;
const temporary = path.join(root, 'temp');
await mkdir(temporary);
const environment = Object.fromEntries(Object.entries(process.env).filter(([name]) =>
  /^(SystemRoot|SystemDrive|windir|ComSpec|PATHEXT|APPDATA|LOCALAPPDATA|USERPROFILE|USERNAME|PUBLIC|ProgramData|ProgramFiles(?:\(x86\))?)$/i.test(name)));
environment.PATH = `${windows}\\System32;${windows}`;
environment.TEMP = environment.TMP = temporary;
environment.UNDERSTAND_BOOK_LOCAL_DATA_DIR = data;
environment.WEBVIEW2_USER_DATA_FOLDER = path.join(root, 'webview');
environment.HF_HOME = path.join(root, 'huggingface');
environment.HTTP_PROXY = environment.HTTPS_PROXY = 'http://127.0.0.1:1';
environment.NO_PROXY = '127.0.0.1,localhost';
const listener = createServer();
await new Promise(resolve => listener.listen(0, '127.0.0.1', resolve));
const cdp = `http://127.0.0.1:${listener.address().port}`;
await new Promise(resolve => listener.close(resolve));
environment.WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = [
  `--remote-debugging-port=${new URL(cdp).port}`,
  '--use-fake-ui-for-media-stream',
  '--use-fake-device-for-media-stream',
  `--use-file-for-fake-audio-capture="${audio}%noloop"`,
].join(' ');

const child = spawn(executable, [], { cwd: root, env: environment, windowsHide: true, stdio: 'ignore' });
let spawnError, browser, page;
child.on('error', error => { spawnError = error; });
const blocked = [], speechCalls = [];
const appUrl = /^http:\/\/(tauri\.localhost|127\.0\.0\.1:)/;
try {
  await expect.poll(async () => {
    if (spawnError) throw spawnError;
    if (child.exitCode !== null) throw Error(`Desktop exited before startup (${child.exitCode}).`);
    try {
      const targets = await (await fetch(cdp + '/json/list')).json();
      return targets.some(target => target.type === 'page' && appUrl.test(target.url));
    } catch { return false; }
  }, { timeout: 45000 }).toBe(true);
  browser = await chromium.connectOverCDP(cdp);
  const context = browser.contexts()[0];
  const appPage = () => context.pages().find(candidate => appUrl.test(candidate.url()));
  await expect.poll(() => Boolean(appPage()), { timeout: 45000 }).toBe(true);
  page = appPage();
  await context.route('**/*', route => {
    const url = new URL(route.request().url());
    const remote = ['http:', 'https:'].includes(url.protocol) && !['127.0.0.1', 'tauri.localhost', 'ipc.localhost'].includes(url.hostname);
    const model = ['/api/agent/chat', '/api/agent/runs'].includes(url.pathname) && route.request().method() === 'POST';
    if (remote || model) {
      blocked.push(url.origin + url.pathname);
      return route.abort();
    }
    return route.continue();
  });
  await page.getByRole('button', { name: /^(进入书库|Open library)$/ }).click();
  await expect(page).toHaveURL(/^http:\/\/127\.0\.0\.1:/, { timeout: 45000 });
  await page.addLocatorHandler(page.getByRole('button', { name: '跳过引导', exact: true }), button => button.click());
  await page.locator('.collection-card').getByRole('button', { name: '查看这本书' }).first().click();
  await page.getByRole('button', { name: '阅读原文', exact: true }).click();
  await expect(page.locator('.reader-pane')).toBeVisible();
  const origin = new URL(page.url()).origin;
  let status = await (await fetch(origin + '/api/reader/voice-status')).json();
  if (options['model-dir']) {
    assert.equal(status.available, false, 'Optional voice model should start unconfigured on a fresh profile.');
    assert.equal(status.runtime_available, true, 'Bundled voice runtime must be ready out of the box.');
    assert.equal(status.can_download, true, 'Optional voice model should be downloadable.');
    const selectRes = await fetch(origin + '/api/reader/voice-model/select', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ dir: path.resolve(options['model-dir']) }),
    });
    if (!selectRes.ok) throw Error(await selectRes.text());
    status = await (await fetch(origin + '/api/reader/voice-status')).json();
  }
  assert.equal(status.available, true, 'Packaged voice must be ready before recording.');
  assert.equal(status.max_seconds, 90);
  assert.doesNotMatch(await readFile(path.join(data, '.env'), 'utf8'), /VOICE_(?:PYTHON|MODEL_DIR)/);
  const provider = await page.evaluate(() => window.__TAURI_INTERNALS__.invoke('desktop_provider_status'));
  assert.equal(provider.configured, false);
  const agentTab = page.getByRole('tab', { name: '问答', exact: true });
  if (await agentTab.isVisible()) await agentTab.click();
  const draft = page.locator('[data-workspace-input="agent"]');
  const original = '我已经写好的草稿。';
  await draft.fill(original);
  await page.locator('.composer-voice').getByRole('button', { name: '语音输入', exact: true }).click();
  const stop = page.getByRole('button', { name: '停止并转文字', exact: true });
  await expect(stop).toBeVisible();
  await new Promise(resolve => setTimeout(resolve, recordingMs));
  const observed = page.waitForResponse(response => new URL(response.url()).pathname === '/api/reader/voice-transcribe', { timeout: 120000 });
  await stop.click();
  const response = await observed;
  const submitted = response.request().postDataJSON();
  assert.equal(submitted.locale, 'zh-CN');
  const audioBytes = Buffer.from(submitted.audio, 'base64').length;
  assert(audioBytes > 0, 'The actual MediaRecorder output must reach the transcription route.');
  speechCalls.push({ url: response.url(), httpStatus: response.status(), mimeType: submitted.mime_type, audioBytes });
  if (!response.ok()) throw Error(await response.text());
  const result = await response.json();
  assert.match(result.text, transcriptWords);
  const review = page.getByRole('textbox', { name: '识别结果' });
  await expect(review).toHaveValue(result.text.trim());
  await expect(draft).toHaveValue(original);
  await page.screenshot({ path: path.join(root, 'review.png') });
  await page.getByRole('button', { name: '加入输入框', exact: true }).click();
  await expect(draft).toHaveValue(`${original}\n${result.text.trim()}`);
  assert.deepEqual(blocked, []);
  await page.screenshot({ path: path.join(root, 'draft.png') });
  const closed = await execute(powershell, ['-NoProfile', '-Command', `(Get-Process -Id ${child.pid}).CloseMainWindow()`], { windowsHide: true });
  assert.equal(closed.stdout.trim(), 'True');
  await expect.poll(() => child.exitCode, { timeout: 45000 }).toBe(0);
  const evidence = { result: 'passed', voiceEnvironment: 'bundle default', noModelConfigured: true,
    executable, status, speechCalls, blocked, transcript: result.text, durationMs: result.duration_ms };
  await writeFile(path.join(root, 'result.json'), JSON.stringify(evidence, null, 2));
  console.log('VOICE PASS', JSON.stringify(evidence));
} catch (error) {
  if (page && !page.isClosed()) {
    await page.screenshot({ path: path.join(root, 'failure.png'), timeout: 5000 }).catch(() => {});
    await writeFile(path.join(root, 'failure-page.txt'), await page.locator('body').innerText({ timeout: 5000 }).catch(() => ''));
  }
  await writeFile(path.join(root, 'failure.json'), JSON.stringify({ message: error.message, blocked, speechCalls }, null, 2));
  throw error;
} finally {
  if (child.exitCode === null) child.kill();
  await browser?.close().catch(() => {});
}
