// Exercises the packaged native WebView, real local server and persisted settings.
// Also accepts an installed local edition: --edition local --executable <path> --out <fresh directory>.
// Windows closes through the normal OS window message; --manual-close keeps the interactive workflow.
import { chromium, expect } from '@playwright/test';
import { spawn, execFile } from 'node:child_process';
import { createServer } from 'node:http';
import { connect } from 'node:net';
import { cp, mkdir, mkdtemp, readFile, writeFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import path from 'node:path';
import assert from 'node:assert/strict';
import { parseArgs, promisify } from 'node:util';

const repo = fileURLToPath(new URL('../../', import.meta.url));
const { values: options } = parseArgs({ options: {
  edition: { type: 'string', default: 'debug' },
  executable: { type: 'string' },
  out: { type: 'string' },
  'manual-close': { type: 'boolean', default: false },
  'legacy-debug': { type: 'boolean', default: false },
} });
assert(['debug', 'local'].includes(options.edition), '--edition must be debug or local');
assert(!options['legacy-debug'] || options.edition === 'debug', '--legacy-debug is only for an existing pre-split Debug package');
assert(options.edition !== 'local' || options.executable, '--executable must identify the installed local application');
const executable = path.resolve(options.executable ?? path.join(repo, 'dist/UnderstandBookLocalDebug/UnderstandBookLocalDebug.exe'));
const root = options.out ? path.resolve(options.out) : await mkdtemp(path.join(repo, 'target/local-desktop-smoke-'));
await mkdir(root, { recursive: true });
const data = path.join(root, 'data');
const book = path.join(root, 'book');
let expectedBook = book;
// A fresh directory is part of the first-use assertion. Keep earlier evidence intact.
await mkdir(book);
const source = '本地阅读验证\n资料保存在自己的电脑上，阅读记录可以在重新打开后继续使用。\n';
await writeFile(path.join(book, 'source.txt'), source);
await writeFile(path.join(book, 'base.json'), JSON.stringify({ book_id: 'local-debug-smoke',
  lid_nodes: [{ lid: '1', path: [1], kind: 'paragraph', span: { start: 0, end: source.length }, children: [] }], graph_nodes: [], graph_edges: [] }));
const calls = [];
const model = createServer(async (req, res) => {
  let raw = ''; for await (const chunk of req) raw += chunk;
  const body = JSON.parse(raw); calls.push({ url: req.url, model: body.model });
  res.setHeader('Content-Type', 'application/json');
  res.end(JSON.stringify({ choices: [{ message: { role: 'assistant', content: '本地模型接口已接通。' }, finish_reason: 'stop' }], usage: { prompt_tokens: 10, completion_tokens: 10, total_tokens: 20 } }));
});
await new Promise(resolve => model.listen(0, '127.0.0.1', resolve));
const baseUrl = `http://127.0.0.1:${model.address().port}/v1`;
const env = { ...process.env, [options.edition === 'local' ? 'UNDERSTAND_BOOK_LOCAL_DATA_DIR' : 'UNDERSTAND_BOOK_DEBUG_DATA_DIR']: data };
for (const key of ['OPENCODE_API_KEY', 'OPENCODE_BASE_URL', 'FLUID_LLM_MODEL', 'UNDERSTAND_BOOK_PROVIDER', 'UNDERSTAND_BOOK_MODEL_SETTINGS', 'UNDERSTAND_BOOK_MODEL_CONFIG_ID', 'UNDERSTAND_BOOK_BUILD_SIDECAR', 'UNDERSTAND_BOOK_NODE', 'UNDERSTAND_BOOK_STAGE_RUNNER']) delete env[key];
if (process.platform === 'win32' && options.edition === 'local') {
  for (const key of Object.keys(env)) if (key.toUpperCase() === 'PATH') delete env[key];
  env.PATH = `${process.env.SystemRoot}\\System32;${process.env.SystemRoot}`;
}
env.WEBVIEW2_USER_DATA_FOLDER = path.join(root, 'webview');
const remoteRequests = [];
const pageErrors = [];
const draftText = '关闭应用后继续写的本地草稿。';
const execute = promisify(execFile);
let child;
let activePage;
let applicationOutput = '';
try {
  for (let run = 1; run <= 2; run++) {
    const cdp = createServer();
    await new Promise(resolve => cdp.listen(0, '127.0.0.1', resolve));
    const cdpPort = cdp.address().port;
    await new Promise(resolve => cdp.close(resolve));
    env.WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = `--remote-debugging-port=${cdpPort}`;
    let output = '';
    applicationOutput = '';
    child = spawn(executable, options.edition === 'debug' && run === 1 ? [book] : [], {
      env, cwd: root, windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'],
    });
    child.stderr.on('data', chunk => { output += chunk; applicationOutput = output; });
    child.stdout.on('data', chunk => { output += chunk; applicationOutput = output; });
    let spawnError;
    child.once('error', error => { spawnError = error; });
    const exited = new Promise(resolve => child.once('exit', resolve));
    await expect.poll(async () => {
      if (spawnError) throw spawnError;
      if (child.exitCode !== null) throw new Error(output);
      try {
        const targets = await (await fetch(`http://127.0.0.1:${cdpPort}/json/list`)).json();
        return targets.some(target => target.type === 'page' && /^http:\/\/(tauri\.localhost|127\.0\.0\.1:)/.test(target.url));
      } catch { return false; }
    }, { timeout: 45000 }).toBe(true);
    const browser = await chromium.connectOverCDP(`http://127.0.0.1:${cdpPort}`);
    const context = browser.contexts()[0];
    await expect.poll(() => context.pages().some(page => /^http:\/\/(tauri\.localhost|127\.0\.0\.1:)/.test(page.url())), {
      timeout: 45000, message: 'The native WebView must finish its initial navigation before interaction',
    }).toBe(true);
    const page = context.pages().find(page => /^http:\/\/(tauri\.localhost|127\.0\.0\.1:)/.test(page.url()));
    assert(page, 'The native application page must exist');
    activePage = page;
    page.on('pageerror', error => pageErrors.push(error.message));
    page.setDefaultTimeout(15000);
    await context.route('**/*', route => {
      const url = new URL(route.request().url());
      if (['http:', 'https:'].includes(url.protocol) && !['127.0.0.1', 'localhost', 'ipc.localhost', 'tauri.localhost'].includes(url.hostname)) {
        remoteRequests.push(url.origin); return route.abort();
      }
      return route.continue();
    });
    if (options.edition === 'local' && run === 1) {
      await page.getByRole('button', { name: '进入书库', exact: true }).waitFor();
      await page.screenshot({ path: path.join(root, 'first-run.png') });
      await page.getByRole('button', { name: '进入书库', exact: true }).click();
    }
    await expect.poll(async () => {
      const startupError = page.locator('#startup-error');
      if (await startupError.isVisible()) throw new Error(await startupError.innerText());
      return page.url();
    }, { timeout: 45000 }).toMatch(/^http:\/\/127\.0\.0\.1:/);
    if (options.edition === 'debug') await page.reload();
    if (options.edition === 'local') {
      await page.addLocatorHandler(page.getByRole('button', { name: '跳过引导', exact: true }), button => button.click());
      const home = page.getByRole('navigation', { name: '主导航' });
      const destination = name => home.getByRole('button', { name, exact: true });
      await expect(page.getByRole('heading', { name: '读一本书，或走完一套课程' })).toBeVisible();
      for (const name of ['学习', '我的理解', '学习资料', '学习画板']) await expect(destination(name)).toBeVisible();
      const library = await (await fetch(new URL('/api/book/library', page.url()))).json();
      expectedBook = path.join(library.root, '本地阅读验证');
      if (run === 1) {
        await cp(book, expectedBook, { recursive: true });
        await page.getByRole('button', { name: '刷新目录' }).click();
        await expect(page.getByRole('heading', { name: '本地阅读验证' })).toBeVisible();
        await page.screenshot({ path: path.join(root, 'library.png') });
        await destination('学习资料').click();
        await expect(page.getByRole('heading', { name: '学习资料', exact: true })).toBeVisible();
        await page.getByRole('button', { name: '新建论文书', exact: true }).click();
        await expect(page.getByRole('tab', { name: '新建论文书' })).toHaveAttribute('aria-selected', 'true');
        await page.getByRole('button', { name: '取消', exact: true }).click();
        await destination('我的理解').click();
        await page.locator('.collection-card').filter({ hasText: '本地阅读验证' }).getByRole('button').click();
        await expect(page.locator('.understanding-map')).toBeVisible();
        await page.getByRole('button', { name: '返回学习', exact: true }).click();
        await destination('学习').click();
        await page.locator('.collection-card').filter({ hasText: '本地阅读验证' }).getByRole('button', { name: '查看这本书' }).click();
        await page.getByRole('button', { name: '阅读原文', exact: true }).click();
        await expect(page.locator('.reader-pane')).toContainText('资料保存在自己的电脑上');
      }
      await destination('学习画板').click();
      await expect(page.getByRole('heading', { name: '学习画板', exact: true })).toBeVisible();
      if (run === 1) {
        await page.getByRole('button', { name: '新建画板', exact: true }).click();
        await page.getByLabel('想弄懂什么？').fill('本地书库的画板');
        await page.getByLabel('从哪份资料开始？').selectOption({ label: '本地阅读验证' });
        await page.getByRole('button', { name: '创建并打开画板' }).click();
      } else {
        await page.locator('.board-library-card').filter({ hasText: '本地书库的画板' }).getByRole('button', { name: '打开画板' }).click();
      }
      await expect(page.getByLabel('画板名称')).toHaveValue('本地书库的画板');
      await page.getByRole('button', { name: '关闭画板，回到阅读' }).click();
    }
    await expect(page.locator('.reader-pane')).toBeVisible({ timeout: 15000 });
    const url = new URL(page.url()).origin;
    async function api(endpoint, body) {
      const response = await fetch(url + '/api' + endpoint, body === undefined ? {} : { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify(body) });
      const result = await response.json();
      assert.equal(response.ok, true, JSON.stringify(result)); return result;
    }
    const status = await api('/desktop/status');
    assert.equal(status.desktop_host, true); assert.equal(status.reader_only, false);
    assert.equal(status.book_dir, expectedBook);
    assert.equal((await api('/book/manifest')).tree[0].lid, '1');
    // A new loopback origin can show the existing first-use tour again on restart.
    const skipTour = page.getByRole('button', { name: '跳过引导', exact: true });
    if (options.edition === 'debug' && await skipTour.isVisible()) await skipTour.click();
    if (run === 1) {
      const initial = await page.evaluate(() => window.__TAURI_INTERNALS__.invoke('desktop_provider_status'));
      assert.equal(initial.configured, false);
      await page.getByRole('button', { name: '更多阅读工具', exact: true }).click();
      await page.getByRole('button', { name: '打开桌面设置', exact: true }).click();
      const capabilities = await page.evaluate(() => window.__UNDERSTAND_BOOK_LOCAL__);
      if (!options['legacy-debug']) assert.equal(capabilities?.noteDrafts, true, 'Native drafts must be declared by both local hosts');
      if (capabilities?.managedBuildEngine || options['legacy-debug']) {
        await expect(page.getByText('本地构建引擎', { exact: true })).toBeVisible();
        await expect(page.getByText('已就绪', { exact: true })).toBeVisible();
        await expect(page.getByRole('button', { name: '安装或重试', exact: true })).toHaveCount(0);
      }
      await expect(page.getByText('尚未配置', { exact: true })).toBeVisible();
      if (!options['legacy-debug']) await page.getByRole('button', { name: '添加模型服务', exact: true }).click();
      await page.getByLabel('服务地址', { exact: true }).fill(baseUrl);
      const modelIdLabel = options['legacy-debug'] ? '模型名称' : '模型 ID';
      await page.getByLabel(modelIdLabel, { exact: true }).fill('local-smoke-model');
      if (!options['legacy-debug']) await page.getByTestId('model-native-tools').selectOption({ label: '支持' });
      await page.getByLabel('API Key', { exact: true }).fill('local-test-key');
      await page.getByRole('button', { name: '保存并应用', exact: true }).click();
      await expect(page.getByText('已保存并应用', { exact: true })).toBeVisible();
      await expect(page.getByLabel('服务地址', { exact: true })).toHaveValue(baseUrl);
      await expect(page.getByLabel(modelIdLabel, { exact: true })).toHaveValue('local-smoke-model');
      if (!options['legacy-debug']) {
        await page.getByRole('button', { name: '添加模型', exact: true }).click();
        await page.getByLabel('模型 ID', { exact: true }).fill('local-feedback-model');
        await page.getByTestId('model-native-tools').selectOption({ label: '支持' });
        await page.getByText('按用途选择模型', { exact: true }).click();
        await page.getByRole('combobox', { name: '教学与练习反馈', exact: true }).selectOption({ label: 'local-feedback-model' });
        await page.getByRole('button', { name: '保存并应用', exact: true }).click();
        await expect(page.getByText('已保存并应用', { exact: true })).toBeVisible();
        const catalog = await page.evaluate(() => window.__TAURI_INTERNALS__.invoke('desktop_model_settings'));
        assert.equal(catalog.settings.models.length, 2);
        assert.equal(catalog.settings.models.find(model => model.id === catalog.settings.bindings.default_model).model_id, 'local-smoke-model');
        assert.equal(catalog.settings.models.find(model => model.id === catalog.settings.bindings.teaching).model_id, 'local-feedback-model');
        assert.equal(Object.hasOwn(catalog.settings.services[0], 'api_key'), false);
        await writeFile(path.join(root, 'model-settings.json'), JSON.stringify(catalog, null, 2));
      }
      await page.screenshot({ path: path.join(root, 'settings.png') });
      await page.getByRole('button', { name: '关闭设置', exact: true }).click();
      const { source_fingerprint } = await api('/book/source_fingerprint');
      await api('/memory/save', { type: 'note', content: 'Local desktop persistence proof',
        note_placement: { kind: 'lid_block', lid: '1', source_fingerprint } });
      const answer = await api('/agent/chat', { message: '你好，请回复一句问候即可。', response_language: 'zh-CN' });
      await writeFile(path.join(root, 'answer.json'), JSON.stringify(answer, null, 2));
      assert.ok(calls.some(call => call.url === '/v1/chat/completions' && call.model === 'local-smoke-model'));
      assert.equal(answer.answer, '本地模型接口已接通。');
      assert.equal(answer.incomplete, false);
      console.log('MODEL_INTERFACE_OK', JSON.stringify(calls));
      await page.getByRole('button', { name: '记一下', exact: true }).click();
      await page.getByLabel('想法正文', { exact: true }).fill(draftText);
      await expect.poll(async () => {
        try { return (await readFile(path.join(data, 'personal-note-drafts.json'), 'utf8')).includes(draftText); }
        catch { return false; }
      }).toBe(true);
    } else {
      const saved = await page.evaluate(() => window.__TAURI_INTERNALS__.invoke('desktop_provider_status'));
      assert.equal(saved.source, 'settings'); assert.equal(saved.model, 'local-smoke-model');
      if (!options['legacy-debug']) {
        const catalog = await page.evaluate(() => window.__TAURI_INTERNALS__.invoke('desktop_model_settings'));
        assert.equal(catalog.settings.models.length, 2);
        assert.equal(catalog.settings.models.find(model => model.id === catalog.settings.bindings.default_model).model_id, 'local-smoke-model');
        assert.equal(catalog.settings.models.find(model => model.id === catalog.settings.bindings.teaching).model_id, 'local-feedback-model');
        assert.equal(Object.hasOwn(catalog.settings.services[0], 'api_key'), false);
      }
      const notes = await api('/memory/recall', { type: 'note' });
      assert.ok(JSON.stringify(notes).includes('Local desktop persistence proof'));
      await page.getByRole('button', { name: '记一下', exact: true }).click();
      await expect(page.getByLabel('想法正文', { exact: true })).toHaveValue(draftText);
      console.log('RESTART_PERSISTENCE_OK');
    }
    assert.deepEqual(remoteRequests, []);
    await page.screenshot({ path: path.join(root, `reader-${run}.png`) });
    if (run === 2) {
      await page.setViewportSize({ width: 960, height: 720 });
      await page.screenshot({ path: path.join(root, 'draft-narrow.png') });
      await page.goto(`${url}/?lang=en`);
      const backToReading = page.getByRole('button', { name: 'Back to reading', exact: true });
      await expect.poll(async () => await backToReading.isVisible() || await page.locator('.reader-pane').isVisible()).toBe(true);
      if (await backToReading.isVisible()) await backToReading.click();
      await expect(page.locator('.reader-pane')).toBeVisible();
      if (options.edition === 'local') {
        await expect(page.getByRole('button', { name: 'Make a note', exact: true })).toBeVisible();
      }
      await page.screenshot({ path: path.join(root, 'reader-en-narrow.png') });
    }
    console.log(`READY_FOR_CLOSE ${run} ${root}`);
    if (process.platform === 'win32' && !options['manual-close']) {
      const closed = await execute('powershell.exe', ['-NoProfile', '-Command',
        `(Get-Process -Id ${child.pid}).CloseMainWindow()`], { windowsHide: true });
      assert.equal(closed.stdout.trim(), 'True', 'The application must receive a normal window close request');
    }
    await expect.poll(() => child.exitCode, { timeout: 45000 }).not.toBeNull();
    await exited;
    assert.equal(child.exitCode, 0, output);
    // Check a fresh TCP connection. A pooled HTTP request can stall on a socket
    // from the previous view even after the embedded listener has exited.
    const port = Number(new URL(url).port);
    await expect.poll(() => new Promise((resolve, reject) => {
      const socket = connect(port, '127.0.0.1');
      socket.once('connect', () => { socket.destroy(); resolve(true); });
      socket.once('error', error => error.code === 'ECONNREFUSED' ? resolve(false) : reject(error));
    }), { message: `The installed reader listener on port ${port} must close` }).toBe(false);
    await expect.poll(async () => { try { await readFile(path.join(data, 'debug-session.json')); return true; } catch { return false; } }).toBe(false);
    console.log(`CLEAN_SHUTDOWN_OK ${run}`);
    await writeFile(path.join(root, `application-${run}.log`), output);
  }
  assert.deepEqual(pageErrors, []);
  const tested = ['native reader', 'settings form', 'local model HTTP (controlled endpoint)', 'note persistence', 'unsaved native draft', 'restart', 'normal window close', 'service shutdown'];
  if (options.edition === 'local') tested.push('library on launch and restart', 'material import dialog', 'book understanding map', 'saved board reopened from library');
  await writeFile(path.join(root, 'result.json'), JSON.stringify({ result: 'passed', edition: options.edition, executable, remoteRequests, pageErrors, modelCalls: calls, tested }, null, 2));
  console.log('PASS', root);
} catch (error) {
  if (activePage && !activePage.isClosed()) {
    await activePage.screenshot({ path: path.join(root, 'failure.png') }).catch(() => {});
    await writeFile(path.join(root, 'failure-page.txt'), await activePage.locator('body').innerText().catch(() => '')).catch(() => {});
    if (/^http:\/\/127\.0\.0\.1:/.test(activePage.url())) {
      const origin = new URL(activePage.url()).origin;
      for (const endpoint of ['book/manifest', 'desktop/status']) {
        const body = await fetch(`${origin}/api/${endpoint}`, { signal: AbortSignal.timeout(5000) }).then(response => response.text()).catch(error => error.message);
        await writeFile(path.join(root, `failure-${endpoint.replace('/', '-')}.json`), body);
      }
    }
  }
  await writeFile(path.join(root, 'failure-application.log'), applicationOutput);
  await writeFile(path.join(root, 'failure.json'), JSON.stringify({ message: error.message, stack: error.stack, remoteRequests, pageErrors, modelCalls: calls }, null, 2));
  throw error;
} finally { if (child?.exitCode === null) child.kill(); model.close(); }
