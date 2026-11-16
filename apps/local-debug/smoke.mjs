// Exercises the packaged native WebView, real local server and persisted settings.
// Close each native window when READY_FOR_CLOSE appears to verify clean shutdown.
import { chromium, expect } from '@playwright/test';
import { spawn } from 'node:child_process';
import { createServer } from 'node:http';
import { mkdir, mkdtemp, readFile, writeFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import path from 'node:path';
import assert from 'node:assert/strict';

const repo = fileURLToPath(new URL('../../', import.meta.url));
const root = await mkdtemp(path.join(repo, 'target/local-debug-smoke-'));
const data = path.join(root, 'data');
const book = path.join(root, 'book');
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
const env = { ...process.env, UNDERSTAND_BOOK_DEBUG_DATA_DIR: data };
for (const key of ['OPENCODE_API_KEY', 'OPENCODE_BASE_URL', 'FLUID_LLM_MODEL', 'UNDERSTAND_BOOK_PROVIDER', 'UNDERSTAND_BOOK_BUILD_SIDECAR']) delete env[key];
const remoteRequests = [];
let child;
try {
  for (let run = 1; run <= 2; run++) {
    const cdp = createServer();
    await new Promise(resolve => cdp.listen(0, '127.0.0.1', resolve));
    const cdpPort = cdp.address().port;
    await new Promise(resolve => cdp.close(resolve));
    env.WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = `--remote-debugging-port=${cdpPort}`;
    let output = '';
    child = spawn(path.join(repo, 'dist/UnderstandBookLocalDebug/UnderstandBookLocalDebug.exe'), run === 1 ? [book] : [], {
      env, cwd: root, windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'],
    });
    child.stderr.on('data', chunk => { output += chunk; });
    const exited = new Promise(resolve => child.once('exit', resolve));
    await expect.poll(async () => {
      if (child.exitCode !== null) throw new Error(output);
      try { return (await fetch(`http://127.0.0.1:${cdpPort}/json/version`)).ok; } catch { return false; }
    }, { timeout: 45000 }).toBe(true);
    const browser = await chromium.connectOverCDP(`http://127.0.0.1:${cdpPort}`);
    const context = browser.contexts()[0];
    await expect.poll(() => context.pages().length).toBeGreaterThan(0);
    const page = context.pages()[0];
    page.setDefaultTimeout(15000);
    await context.route('**/*', route => {
      const url = new URL(route.request().url());
      if (['http:', 'https:'].includes(url.protocol) && !['127.0.0.1', 'localhost', 'ipc.localhost'].includes(url.hostname)) {
        remoteRequests.push(url.origin); return route.abort();
      }
      return route.continue();
    });
    await expect.poll(() => page.url()).toMatch(/^http:\/\/127\.0\.0\.1:/);
    await page.reload();
    await expect(page.locator('.reader-pane')).toBeVisible();
    const url = new URL(page.url()).origin;
    async function api(endpoint, body) {
      const response = await fetch(url + '/api' + endpoint, body === undefined ? {} : { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify(body) });
      const result = await response.json();
      assert.equal(response.ok, true, JSON.stringify(result)); return result;
    }
    const status = await api('/desktop/status');
    assert.equal(status.desktop_host, true); assert.equal(status.reader_only, false);
    assert.equal(status.book_dir, book);
    assert.equal((await api('/book/manifest')).tree[0].lid, '1');
    if (run === 1) {
      const initial = await page.evaluate(() => window.__TAURI_INTERNALS__.invoke('desktop_provider_status'));
      assert.equal(initial.configured, false);
      await page.getByRole('button', { name: '更多阅读工具', exact: true }).click();
      await page.getByRole('button', { name: '打开桌面设置', exact: true }).click();
      await expect(page.getByText('本地构建引擎', { exact: true })).toBeVisible();
      await expect(page.getByText('已就绪', { exact: true })).toBeVisible();
      await expect(page.getByRole('button', { name: '安装或重试', exact: true })).toHaveCount(0);
      await expect(page.getByText('尚未配置', { exact: true })).toBeVisible();
      await page.getByLabel('服务地址', { exact: true }).fill(baseUrl);
      await page.getByLabel('模型名称', { exact: true }).fill('local-smoke-model');
      await page.getByLabel('API Key', { exact: true }).fill('local-test-key');
      await page.getByRole('button', { name: '保存并应用', exact: true }).click();
      await expect(page.getByText('已保存并应用', { exact: true })).toBeVisible();
      await expect(page.getByLabel('服务地址', { exact: true })).toHaveValue(baseUrl);
      await expect(page.getByLabel('模型名称', { exact: true })).toHaveValue('local-smoke-model');
      await page.screenshot({ path: path.join(root, 'settings.png') });
      await page.getByRole('button', { name: '关闭设置', exact: true }).click();
      const { source_fingerprint } = await api('/book/source_fingerprint');
      await api('/memory/save', { type: 'note', content: 'Local debug persistence proof',
        note_placement: { kind: 'lid_block', lid: '1', source_fingerprint } });
      const answer = await api('/agent/chat', { message: '你好，请回复一句问候即可。', response_language: 'zh-CN' });
      await writeFile(path.join(root, 'answer.json'), JSON.stringify(answer, null, 2));
      assert.ok(calls.some(call => call.url === '/v1/chat/completions' && call.model === 'local-smoke-model'));
      assert.equal(answer.answer, '本地模型接口已接通。');
      assert.equal(answer.incomplete, false);
      console.log('MODEL_INTERFACE_OK', JSON.stringify(calls));
    } else {
      const saved = await page.evaluate(() => window.__TAURI_INTERNALS__.invoke('desktop_provider_status'));
      assert.equal(saved.source, 'settings'); assert.equal(saved.model, 'local-smoke-model');
      const notes = await api('/memory/recall', { type: 'note' });
      assert.ok(JSON.stringify(notes).includes('Local debug persistence proof'));
      console.log('RESTART_PERSISTENCE_OK');
    }
    assert.deepEqual(remoteRequests, []);
    await page.screenshot({ path: path.join(root, `reader-${run}.png`) });
    console.log(`READY_FOR_CLOSE ${run} ${root}`);
    await exited;
    assert.equal(child.exitCode, 0, output);
    await expect.poll(async () => { try { await fetch(url + '/api/desktop/status'); return true; } catch { return false; } }).toBe(false);
    await expect.poll(async () => { try { await readFile(path.join(data, 'debug-session.json')); return true; } catch { return false; } }).toBe(false);
    console.log(`CLEAN_SHUTDOWN_OK ${run}`);
  }
  await writeFile(path.join(root, 'result.json'), JSON.stringify({ result: 'passed', remoteRequests, modelCalls: calls, tested: ['native reader', 'settings form', 'local model HTTP', 'note persistence', 'restart', 'clean shutdown'] }, null, 2));
  console.log('PASS', root);
} finally { if (child?.exitCode === null) child.kill(); model.close(); }
