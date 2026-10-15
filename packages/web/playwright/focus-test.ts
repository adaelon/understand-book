import { test as base, chromium } from '@playwright/test';
import { spawn } from 'node:child_process';
import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';

// Native runs use the built product host, an isolated profile, and the same dist.
// No user's running app or private library is touched.
export const test = process.env.FP7_ENGINE !== 'webview2' ? base : base.extend({
  browser: [async ({}, use) => {
    const root = path.resolve('../../tmp/focus-reading-fp7/native');
    const appdata = path.join(root, 'appdata');
    await mkdir(path.join(appdata, 'UnderstandBook'), { recursive: true });
    await mkdir(path.join(root, 'library'), { recursive: true });
    await writeFile(path.join(appdata, 'UnderstandBook/settings.json'), JSON.stringify({ schema: 'understand_book.desktop_settings.v1', library_root: path.join(root, 'library') }));
    const child = spawn(path.resolve('../../target/debug/UnderstandBook.exe'), [], { cwd: path.resolve('../..'), windowsHide: true,
      env: { ...process.env, LOCALAPPDATA: appdata, WEBVIEW2_USER_DATA_FOLDER: path.join(root, 'webview'),
        WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: '--remote-debugging-port=19235',
        UNDERSTAND_BOOK_MEMORY_DIR: path.join(root, 'memory'), UNDERSTAND_BOOK_PRIVATE_DIR: path.join(root, 'private') }, stdio: 'ignore' });
    let browser: Awaited<ReturnType<typeof chromium.connectOverCDP>> | undefined;
    try {
      const until = Date.now() + 45000;
      while (Date.now() < until) {
        try { browser = await chromium.connectOverCDP('http://127.0.0.1:19235', { timeout: 1000 }); break; }
        catch { if (child.exitCode !== null) throw new Error(`Desktop exited: ${child.exitCode}`); await new Promise(r => setTimeout(r, 300)); }
      }
      if (!browser) throw new Error('Desktop WebView2 debugging endpoint did not start');
      await use(browser);
    } finally { await browser?.close(); child.kill(); }
  }, { scope: 'worker' }],
  context: async ({ browser }, use) => { await use(browser.contexts()[0]); },
  page: async ({ context, baseURL }, use) => {
    const page = context.pages()[0];
    await page.route(baseURL!, route => route.fulfill({ contentType: 'text/html', body: '<!doctype html><body></body>' }));
    await page.goto(baseURL!);
    await page.evaluate(() => { localStorage.clear(); sessionStorage.clear(); });
    await page.unroute(baseURL!);
    await page.goto('about:blank'); await page.setViewportSize({ width: 1440, height: 900 });
    await use(page);
    await page.goto('about:blank');
    await page.unrouteAll({ behavior: 'wait' });
  },
});
