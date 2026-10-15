import { test, expect } from '@playwright/test';
import { spawn } from 'node:child_process';
import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { observeProductionEntry } from './focus-production-entry';

test.skip(!process.env.FOCUS_ENTRY_URL, 'Use the FP7 production-entry configuration and built local server');

for (const material of ['understandbook', 'kimik3']) test(`real local host, ${material}, progress and exit`, async ({ page, browser }) => {
  const root = path.resolve('../../tmp/focus-reading-fp7/live', material); await mkdir(root, { recursive: true });
  const child = spawn(path.resolve('../../target/debug/server.exe'), ['--reader-only', path.resolve('../../.understand-book', material)], {
    cwd: path.resolve('../..'), windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'], env: { ...process.env,
      UNDERSTAND_BOOK_ADDR: '127.0.0.1:0', UNDERSTAND_BOOK_WEB_DIST: path.resolve('dist'),
      UNDERSTAND_BOOK_MEMORY_DIR: path.join(root, 'memory'), UNDERSTAND_BOOK_PRIVATE_DIR: path.join(root, 'private') },
  });
  let logs = ''; child.stdout.on('data', data => { logs += data; }); child.stderr.on('data', data => { logs += data; });
  const errors: string[] = []; page.on('pageerror', error => errors.push(error.message));
  try {
    await expect.poll(() => logs.match(/listening at (http:\/\/[^\s]+)/)?.[1], { timeout: 45000 }).toBeTruthy();
    const origin = logs.match(/listening at (http:\/\/[^\s]+)/)![1];
    await observeProductionEntry(page); await page.goto(origin);
    await page.getByRole('button', { name: '专注阅读', exact: true }).filter({ visible: true }).first().click();
    const snapshot = () => page.evaluate(() => (window as any).fp6.snapshot() ?? (window as any).fp4.snapshot());
    await expect.poll(async () => !!(await snapshot())?.current, { timeout: 30000 }).toBe(true);
    const before = await snapshot();
    for (let i = 0; i < 4; i++) {
      const old = await snapshot(); await page.getByRole('button', { name: '下一屏', exact: true }).click();
      await expect.poll(async () => (await snapshot())?.current.id, { timeout: 30000 }).not.toBe(old.current.id);
      await expect.poll(async () => (await snapshot())?.turn.phase).toBe('idle'); await page.waitForTimeout(1000);
    }
    const after = await snapshot();
    await page.locator('.focus-tools-toggle').filter({ visible: true }).click();
    await page.getByRole('navigation', { name: '专注阅读工具' }).getByRole('button', { name: '退出专注', exact: true }).click();
    await expect(page.locator('.focus-reader-body,.pdf-focus-host').filter({ visible: true })).toHaveCount(0);
    const state = await page.request.get(`${origin}/api/reader/state`).then(r => r.json());
    const output = path.resolve(process.env.FOCUS_EVIDENCE_DIR!); await mkdir(output, { recursive: true });
    await writeFile(path.join(output, `${material}.json`), JSON.stringify({ material, browser: browser.version(), before, after, state, errors }, null, 2));
    await page.screenshot({ path: path.join(output, `${material}.png`) }); expect(errors).toEqual([]);
  } finally { await page.goto('about:blank'); child.kill(); }
});
