import { expect, test } from '@playwright/test';

test('one reader header keeps navigation, reading tools and the return to the same book usable', async ({ page, context, request }, info) => {
  const seed = await request.get('http://127.0.0.1:4231/fixture').then(response => response.json());
  const errors: string[] = [];
  page.on('pageerror', error => errors.push(error.message));
  await context.addInitScript(() => {
    for (const scene of ['library', 'reader', 'board', 'boards']) localStorage.setItem('understand-book.product-tour.v1.A.' + scene, 'done');
  });
  await context.route('**/api/**', async route => {
    const incoming = route.request(), url = new URL(incoming.url());
    const response = await route.fetch({ url: 'http://127.0.0.1:4230' + url.pathname + url.search,
      headers: { ...await incoming.allHeaders(), host: 'reader.example', origin: 'https://reader.example', cookie: seed.cookie } });
    await route.fulfill({ response });
  });
  try {
    await page.setViewportSize({ width: 1440, height: 900 });
    await page.goto('/');
    await page.locator('.network-book-card').first().click();
    const header = page.locator('.topbar:visible'), tools = header.locator('.tools-toggle');
    const workspace = page.locator('.workspace-shell');
    await expect(workspace).toBeVisible();
    const chapter = await header.locator('.reading-titles strong').innerText();
    const bookHeading = page.locator('#reader-outline .outline-book-title');
    await expect(bookHeading).toBeVisible();
    const book = await bookHeading.innerText();
    await expect(header.getByText(book, { exact: true })).toHaveCount(0);
    const contents = header.getByRole('button', { name: '目录', exact: true });
    const contentsLabel = contents.getByText('目录', { exact: true });
    await contentsLabel.click();
    await expect(page.locator('#reader-outline')).toBeHidden();
    await contentsLabel.click();
    await expect(page.locator('#reader-outline')).toBeVisible();
    await expect(header.getByRole('button', { name: /带我读懂/, includeHidden: true })).toHaveCount(0);
    await expect(page.locator('.right-rail [data-purpose="understand"]')).toBeVisible();
    await tools.click();
    await header.getByRole('button', { name: '收起工具栏' }).click();
    await expect(header).toHaveCount(0);
    await page.getByRole('button', { name: '展开工具栏' }).click();
    await expect(tools).toBeFocused();
    await header.getByRole('button', { name: '专注阅读' }).click();
    await expect(header).toHaveCount(0);
    await page.locator('.focus-tools-toggle:visible').click();
    await page.getByRole('navigation', { name: '专注阅读工具' }).getByRole('button', { name: '退出专注' }).click();
    await expect(header).toBeVisible();
    for (const [width, height] of [[1440, 900], [1024, 768], [390, 844], [320, 568]]) {
      await page.setViewportSize({ width, height });
      await expect(header.getByRole('navigation')).toBeVisible();
      await expect(page.locator('.network-page-bar:visible, .workspace-mobile-top:visible, .network-current-reader:visible')).toHaveCount(0);
      const bounds = await header.boundingBox();
      expect(bounds!.height).toBeLessThanOrEqual(48);
      expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
      await expect(contentsLabel).toBeInViewport();
      await expect(header.locator('.reading-titles strong')).toBeInViewport();
      await expect(tools).toBeInViewport();
      if (width === 1440 || width === 390) await page.screenshot({ path: info.outputPath('reader-' + width + '.png') });
      await tools.click();
      const panel = header.getByRole('region', { name: '阅读工具' });
      await expect(panel).toBeVisible();
      const firstAction = width <= 720 ? header.locator('.topbar-focus') : panel.getByRole('button', { name: '阅读设置' });
      await expect(firstAction).toBeFocused();
      if (width === 1440) await page.screenshot({ path: info.outputPath('tools.png') });
      await page.keyboard.press('Escape');
      await expect(panel).toBeHidden();
      await expect(tools).toBeFocused();
    }
    await contentsLabel.click();
    await expect(workspace).toHaveClass(/outline-open/);
    await expect(page.locator('#reader-outline')).toBeVisible();
    await expect(bookHeading).toBeInViewport();
    const noteShortcut = page.locator('.thought-entry');
    await expect(noteShortcut).toBeHidden();
    await page.keyboard.press('Escape');
    await expect(contents).toBeFocused();
    await expect(noteShortcut).toBeVisible();
    await contentsLabel.click();
    await page.locator('#reader-outline .outline-item.active').click();
    await expect(page.locator('#reader-outline')).toBeHidden();
    await expect(page.locator('.reader-pane')).toBeVisible();
    await expect(contents).toBeFocused();
    await tools.click();
    await header.getByRole('button', { name: '阅读设置', exact: true }).click();
    await page.getByRole('button', { name: '关闭阅读设置' }).click();
    await expect(tools).toBeFocused();
    await header.getByRole('button', { name: '学习画板' }).click();
    await expect(page.locator('.board-library')).toBeVisible();
    await page.locator('.network-current-reader').click();
    await expect(header.locator('.reading-titles strong')).toHaveText(chapter);
    await contentsLabel.click();
    await expect(bookHeading).toBeVisible();
    await expect(bookHeading).toHaveText(book);
    await page.keyboard.press('Escape');
    await tools.click();
    await header.locator('.network-account summary').click();
    await expect(header.getByRole('button', { name: '个人设置' })).toBeInViewport();
    await page.keyboard.press('Escape');
    await header.getByRole('button', { name: '学习资料' }).click();
    await expect(page.locator('.network-library')).toBeVisible();
    expect(errors).toEqual([]);
  } finally {
    await context.unrouteAll({ behavior: 'ignoreErrors' });
  }
});
