import { expect, test } from '@playwright/test';
import { readFileSync } from 'node:fs';

for (const width of [320, 390, 1440]) {
  test(`English home remains usable at ${width}px`, async ({ page }, info) => {
    const errors: string[] = [];
    page.on('pageerror', error => errors.push(error.message));
    await page.setViewportSize({ width, height: 900 });
    await page.goto('/home.html?lang=en');
    await expect(page.locator('html')).toHaveAttribute('lang', 'en');
    await expect(page.locator('h1')).not.toContainText('把想学的');
    await expect(page.getByRole('combobox', { name: 'Language / 语言' })).toHaveValue('en');
    await page.evaluate(() => document.fonts.ready);
    const visibleText = await page.locator('main').innerText();
    expect(visibleText).not.toMatch(/[\u3400-\u9fff]/);
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
    const nav = await page.locator('.home-nav-actions').boundingBox();
    expect(nav!.x + nav!.width).toBeLessThanOrEqual(width);
    for (const link of await page.locator('.home-nav-actions > a').all()) await expect(link).toBeInViewport({ ratio: 1 });
    await page.screenshot({ path: info.outputPath('home-en.png') });
    for (const selector of ['.home-hero-copy, .home-hero-workspace', '.home-workspace-cover > div']) {
      const bounds = await page.locator(selector).evaluateAll(nodes => nodes.map(node => {
        const { left, right, top, bottom } = node.getBoundingClientRect(); return { left, right, top, bottom };
      }));
      expect(bounds).toHaveLength(2);
      for (const box of bounds) {
        expect(box.left).toBeGreaterThanOrEqual(0);
        expect(box.right).toBeLessThanOrEqual(width);
      }
      const [a, b] = bounds;
      expect(a!.right <= b!.left || b!.right <= a!.left || a!.bottom <= b!.top || b!.bottom <= a!.top).toBe(true);
    }
    await page.getByRole('combobox', { name: 'Language / 语言' }).selectOption('zh-CN');
    await expect(page.locator('h1')).toContainText('把想学的');
    await page.reload();
    await expect(page.locator('html')).toHaveAttribute('lang', 'zh-CN');
    expect(errors).toEqual([]);
  });
}

test('English account controls fit a small screen and reset links survive a language change', async ({ page }, info) => {
  await page.setViewportSize({ width: 390, height: 740 });
  await page.route('**/api/auth/me', route => route.fulfill({ status: 401, json: {} }));
  await page.goto('/index.html?account=register&lang=en');
  await expect(page.getByRole('heading', { name: 'Create a reading account' })).toBeVisible();
  await page.getByRole('button', { name: 'Send registration code', exact: true }).scrollIntoViewIfNeeded();
  await expect(page.getByRole('button', { name: 'Send registration code', exact: true })).toBeInViewport();
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  await page.screenshot({ path: info.outputPath('account-en.png'), fullPage: true });
  await page.goto('/index.html?account=reset-password&lang=zh-CN#token=language-test-token');
  await page.getByRole('combobox', { name: 'Language / 语言' }).selectOption('en');
  await expect(page.locator('input[type=password]')).toHaveCount(2);
  await expect(page.locator('input[type=password]').first()).toBeEnabled();
  await expect(page).not.toHaveURL(/token=/);
});

for (const width of [320, 390, 1440]) {
  test(`English reader and teaching controls at ${width}px`, async ({ page }, info) => {
    const errors: string[] = [];
    page.on('pageerror', error => errors.push(error.message));
    await page.setViewportSize({ width, height: 844 });
    await page.goto('/mobile-learning-preview.html?embedded=1&lang=en');
    await expect(page.locator('.agent-input')).toBeVisible();
    await page.screenshot({ path: info.outputPath('reader-en.png') });
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
    expect(await page.locator('body').innerText()).not.toMatch(/[\u3400-\u9fff]/);
    expect(errors).toEqual([]);
  });
}

test('English desktop library uses translated controls', async ({ page }, info) => {
  await page.addInitScript(() => { Object.assign(window, { __TAURI_INTERNALS__: {
    metadata: { currentWindow: { label: 'main' }, currentWebview: { label: 'main' } },
    invoke: async (command: string) => command === 'plugin:event|listen' ? 1 : [],
    transformCallback: () => 1, unregisterCallback: () => {},
  } }); });
  await page.setViewportSize({ width: 1200, height: 800 });
  await page.goto('/desktop.html?lang=en');
  await expect(page.locator('.desktop-library h1')).toHaveText('Local library');
  expect((await page.locator('body').innerText()).replace('中文', '')).not.toMatch(/[\u3400-\u9fff]/);
  await expect(page.getByRole('alert')).toHaveCount(0);
  await page.screenshot({ path: info.outputPath('desktop-en.png') });
});

for (const width of [390, 1440]) {
  test(`English cloud library preserves original material titles at ${width}px`, async ({ page }, info) => {
    const errors: string[] = [];
    page.on('pageerror', error => errors.push(error.message));
    await page.setViewportSize({ width, height: 900 });
    await page.route('**/api/**', route => {
      const path = new URL(route.request().url()).pathname;
      const data: Record<string, unknown> = {
        '/api/auth/me': { user_id: 'language-test', email: 'reader@example.com', csrf_token: 'test-token' },
        '/api/videos': { videos: [] }, '/api/me/reading-resumption': null,
        '/api/learning-workspaces': { workspaces: [] },
        '/api/library': { books: [{ title: '中文原文示例', published_book_ref: { book_id: 'original-chinese', publication_id: 'one' } },
          { title: 'Understanding Computer Systems', published_book_ref: { book_id: 'systems', publication_id: 'two' } }] },
      };
      return path in data ? route.fulfill({ json: data[path] }) : route.fulfill({ status: 404, json: { error_code: 'OBJECT_NOT_FOUND', message: 'Not found' } });
    });
    await page.goto('/index.html?lang=en');
    await expect(page.locator('.network-library')).toBeVisible();
    await expect(page.getByText('中文原文示例', { exact: true }).first()).toBeVisible();
    await expect(page.getByRole('combobox', { name: 'Language / 语言' })).toHaveValue('en');
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
    const account = page.locator('.library-head-actions .network-account summary');
    const bounds = await account.boundingBox();
    expect(bounds!.x + bounds!.width).toBeLessThanOrEqual(width);
    await account.click();
    await expect(page.locator('.library-head-actions .network-account-menu')).toBeVisible();
    await account.click();
    await page.screenshot({ path: info.outputPath('cloud-library-en.png') });
    expect(errors).toEqual([]);
  });
}

test('built-in server login supports English errors and switching back to Chinese', async ({ page }, info) => {
  const source = new URL('../../../crates/server/src/', import.meta.url);
  await page.setViewportSize({ width: 390, height: 844 });
  await page.route('https://reader.example/**', route => {
    const path = new URL(route.request().url()).pathname;
    const assets: Record<string, [string, string]> = { '/': ['multi_user_login.html', 'text/html'], '/auth.js': ['multi_user_login.js', 'text/javascript'], '/auth.css': ['multi_user_login.css', 'text/css'] };
    const asset = assets[path];
    return asset ? route.fulfill({ contentType: asset[1], body: readFileSync(new URL(asset[0], source), 'utf8') }) : route.fulfill({ status: 401, json: {} });
  });
  await page.goto('https://reader.example/?lang=en');
  await expect(page.getByRole('heading', { name: 'Back to your reading' })).toBeVisible();
  await page.getByLabel('Email or username').fill('reader@example.com');
  await page.getByLabel('Password', { exact: true }).fill('test-password');
  await page.getByRole('button', { name: 'Sign in', exact: true }).click();
  await expect(page.getByRole('status')).toContainText('The account or password is incorrect');
  await page.screenshot({ path: info.outputPath('server-login-en.png') });
  await page.getByRole('combobox', { name: 'Language / 语言' }).selectOption('zh-CN');
  await expect(page.getByRole('heading', { name: '回到你的阅读' })).toBeVisible();
});

test('English reading toolbar keeps every supported action within the desktop window', async ({ page }, info) => {
  await page.setViewportSize({ width: 1280, height: 720 });
  await page.route('**/locale-toolbar.html*', route => route.fulfill({ contentType: 'text/html', body: `<!doctype html><html><body><div id="app"></div><script type="module">
    import { createApp } from '/node_modules/.vite/deps/vue.js';
    import TopBar from '/src/components/TopBar.vue';
    import '/src/style.css';
    createApp(TopBar, { chapterTitle: 'Understanding computer architecture', guideChapterTitle: 'Requirements and architecture', guideChapterEnabled: true, progressPct: 35, anchorLid: '1.1', debugOpen: false, leftRailOpen: true, buildIntentOpen: false, buildIntentAvailable: true, workbenchAvailable: true, desktopHost: true, mobileCollapsible: false, mobileOpen: false, typographyAvailable: true, focusAvailable: true }).mount('#app');
    </script></body></html>` }));
  await page.goto('/locale-toolbar.html?lang=en');
  await expect(page.locator('.topbar')).toBeVisible();
  const actions = await page.locator('.topbar-actions > *:visible').evaluateAll(nodes => nodes.map(node => {
    const { left, right, top, bottom } = node.getBoundingClientRect(); return { left, right, top, bottom };
  }));
  for (const box of actions) { expect(box.left).toBeGreaterThanOrEqual(0); expect(box.right).toBeLessThanOrEqual(1280); }
  await page.screenshot({ path: info.outputPath('toolbar-en.png') });
});
