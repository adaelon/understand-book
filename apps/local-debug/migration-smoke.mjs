// Import a real, stopped Local Debug data directory through the installed app.
// The source is compared byte-for-byte after import; no credentials are logged.
import { chromium, expect } from '@playwright/test';
import { spawn, execFile } from 'node:child_process';
import { createServer } from 'node:http';
import { mkdir, readFile, readdir, writeFile } from 'node:fs/promises';
import { isDeepStrictEqual, parseArgs, promisify } from 'node:util';
import path from 'node:path';
import assert from 'node:assert/strict';

const { values } = parseArgs({ options: {
  executable: { type: 'string' }, source: { type: 'string' }, out: { type: 'string' },
  'expected-note': { type: 'string' }, 'expected-answer': { type: 'string' }, 'resume-data': { type: 'string' },
} });
for (const key of ['executable', 'source', 'out', 'expected-note']) assert(values[key], `--${key} is required`);
const executable = path.resolve(values.executable);
const source = path.resolve(values.source);
const root = path.resolve(values.out);
await mkdir(root); // Preserve any earlier attempt.
// An upgrade run opens the data created by the previous installed version.
const resume = Boolean(values['resume-data']);
const data = resume ? path.resolve(values['resume-data']) : path.join(root, 'data');
if (resume) assert.equal(JSON.parse(await readFile(path.join(data, 'data-location.json'), 'utf8')), path.join(data, 'imported-data'));
const originalSettings = JSON.parse(await readFile(path.join(source, 'settings.json'), 'utf8'));
const saved = new Map();
async function remember(relative) {
  const filename = path.join(source, relative);
  let entries;
  try { entries = await readdir(filename, { withFileTypes: true }); }
  catch (error) {
    if (error.code === 'ENOENT') return;
    if (error.code === 'ENOTDIR') { saved.set(relative, await readFile(filename)); return; }
    throw error;
  }
  for (const entry of entries) await remember(path.join(relative, entry.name));
}
for (const entry of ['settings.json', '.env', 'memory', 'private', 'personal-note-drafts.json']) await remember(entry);
const env = { ...process.env, UNDERSTAND_BOOK_LOCAL_DATA_DIR: data, UNDERSTAND_BOOK_DEBUG_DATA_DIR: source };
for (const key of ['OPENCODE_API_KEY', 'OPENCODE_BASE_URL', 'FLUID_LLM_MODEL', 'UNDERSTAND_BOOK_PROVIDER', 'UNDERSTAND_BOOK_MODEL_SETTINGS', 'UNDERSTAND_BOOK_MODEL_CONFIG_ID', 'UNDERSTAND_BOOK_BUILD_SIDECAR', 'UNDERSTAND_BOOK_NODE', 'UNDERSTAND_BOOK_STAGE_RUNNER']) delete env[key];
if (process.platform === 'win32') {
  for (const key of Object.keys(env)) if (key.toUpperCase() === 'PATH') delete env[key];
  env.PATH = `${process.env.SystemRoot}\\System32;${process.env.SystemRoot}`;
}
const execute = promisify(execFile);
let child, page;
try {
  for (let run = 1; run <= 2; run++) {
    const portServer = createServer();
    await new Promise(resolve => portServer.listen(0, '127.0.0.1', resolve));
    const port = portServer.address().port;
    await new Promise(resolve => portServer.close(resolve));
    env.WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = `--remote-debugging-port=${port}`;
    child = spawn(executable, [], { env, cwd: root, windowsHide: true, stdio: 'ignore' });
    let spawnError;
    child.once('error', error => { spawnError = error; });
    await expect.poll(async () => {
      if (spawnError) throw spawnError;
      assert.equal(child.exitCode, null, 'Application exited before its window became available');
      try {
        const targets = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
        return targets.some(target => target.type === 'page' && /^http:\/\/(tauri\.localhost|127\.0\.0\.1:)/.test(target.url));
      } catch { return false; }
    }, { timeout: 45000 }).toBe(true);
    const browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`);
    const context = browser.contexts()[0];
    await expect.poll(() => context.pages().some(page => /^http:\/\/(tauri\.localhost|127\.0\.0\.1:)/.test(page.url())), { timeout: 45000 }).toBe(true);
    page = context.pages().find(page => /^http:\/\/(tauri\.localhost|127\.0\.0\.1:)/.test(page.url()));
    assert(page, 'The native application page must exist');
    if (run === 1 && !resume) {
      await page.getByRole('button', { name: `发现旧数据：${source}`, exact: true }).click();
      await expect(page.getByRole('heading', { name: '确认要接续的资料', exact: true })).toBeVisible();
      await expect(page.locator('#import-details')).toContainText('settings.json');
      await expect(page.locator('#import-details')).toContainText('memory');
      await page.screenshot({ path: path.join(root, 'import-preview.png') });
      await page.getByRole('button', { name: '复制并开始阅读', exact: true }).click();
    }
    await expect.poll(async () => {
      if (await page.locator('#startup-error').isVisible()) throw new Error(await page.locator('#startup-error').innerText());
      return page.url();
    }, { timeout: 45000 }).toMatch(/^http:\/\/127\.0\.0\.1:/);
    await expect(page.locator('.reader-pane')).toBeVisible();
    const origin = new URL(page.url()).origin;
    const response = await fetch(`${origin}/api/memory/recall`, {
      method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ type: 'note' }),
    });
    assert.equal(response.status, 200, 'The Reader must accept the note recall request');
    const notes = await response.json();
    assert.ok(JSON.stringify(notes).includes(values['expected-note']), 'Imported notes must be readable through the running service');
    if (values['expected-answer']) {
      const history = await fetch(`${origin}/api/agent/history`);
      assert.equal(history.status, 200);
      assert.ok(JSON.stringify((await history.json()).current.turns).includes(values['expected-answer']), 'The selected chat must contain the original answer');
    }
    const provider = await page.evaluate(() => window.__TAURI_INTERNALS__.invoke('desktop_provider_status'));
    assert.equal(provider.source, 'settings');
    const catalog = originalSettings.model_settings;
    const expectedModel = catalog ? catalog.models.find(model => model.id === catalog.bindings.default_model)?.model_id : originalSettings.provider.model;
    assert.equal(provider.model, expectedModel);
    const location = JSON.parse(await readFile(path.join(data, 'data-location.json'), 'utf8'));
    assert.equal(location, path.join(data, 'imported-data'));
    const receipt = JSON.parse(await readFile(path.join(location, 'local-import.json'), 'utf8'));
    assert.equal(receipt.state, 'complete');
    assert.equal(receipt.source, source);
    const importedSettings = JSON.parse(await readFile(path.join(location, 'settings.json'), 'utf8'));
    assert.ok(isDeepStrictEqual(importedSettings.provider, originalSettings.provider), 'Legacy provider settings changed during import');
    if (catalog) assert.ok(isDeepStrictEqual(importedSettings.model_settings, catalog), 'The saved model catalog changed during import');
    const relocate = original => {
      const relative = path.relative(source, original);
      return relative === '..' || relative.startsWith(`..${path.sep}`) || path.isAbsolute(relative) ? original : path.join(location, relative);
    };
    assert.equal(importedSettings.library_root, relocate(originalSettings.library_root));
    const session = saved.get(path.join('memory', 'session.json'));
    if (session) {
      const book = JSON.parse(session.toString('utf8')).current_book_dir;
      if (book) {
        const status = await (await fetch(`${origin}/api/desktop/status`)).json();
        assert.equal(status.book_dir, relocate(book), 'Restore the same material, relocating only a book copied with its library');
      }
    }
    if (saved.has('personal-note-drafts.json')) {
      assert.ok(isDeepStrictEqual(JSON.parse(await readFile(path.join(location, 'personal-note-drafts.json'), 'utf8')),
        JSON.parse(saved.get('personal-note-drafts.json').toString('utf8'))), 'Drafts changed during import');
    }
    await page.screenshot({ path: path.join(root, `imported-reader-${run}.png`) });
    const close = await execute('powershell.exe', ['-NoProfile', '-Command', `(Get-Process -Id ${child.pid}).CloseMainWindow()`], { windowsHide: true });
    assert.equal(close.stdout.trim(), 'True');
    await expect.poll(() => child.exitCode, { timeout: 45000 }).toBe(0);
    await expect.poll(async () => { try { await fetch(`${origin}/api/desktop/status`); return true; } catch { return false; } }).toBe(false);
  }
  for (const [relative, bytes] of saved) assert.ok((await readFile(path.join(source, relative))).equals(bytes), `Source changed: ${relative}`);
  await writeFile(path.join(root, 'result.json'), JSON.stringify({ result: 'passed', executable, source, imported: path.join(data, 'imported-data'),
    source_files_unchanged: saved.size, source_had_native_drafts: saved.has('personal-note-drafts.json'),
    tested: [...(resume ? ['previous installation data reopened'] : ['native import preview', 'copy and actual service startup']),
      'settings and notes', ...(values['expected-answer'] ? ['selected chat and answer'] : []),
      'restart', 'normal close', 'source unchanged'] }, null, 2));
  console.log('PASS', root);
} catch (error) {
  await writeFile(path.join(root, 'failure.json'), JSON.stringify({ message: error.message, stack: error.stack }, null, 2));
  if (page && !page.isClosed()) await page.screenshot({ path: path.join(root, 'failure.png') }).catch(() => {});
  throw error;
} finally { if (child?.exitCode === null) child.kill(); }
