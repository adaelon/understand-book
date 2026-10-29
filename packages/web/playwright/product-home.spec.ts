import { expect, test, type Locator } from '@playwright/test';

async function expectImageLoaded(image: Locator) {
  await expect.poll(() => image.evaluate(node => {
    const img = node as HTMLImageElement;
    return img.complete && img.naturalWidth > 0;
  })).toBe(true);
}

const scenes = [
  { label: '阅读界面', id: 'reading', file: 'reading-workspace-20261024' },
  { label: '理解视频', id: 'video', file: 'video-understanding-20261024' },
  { label: '理解音乐', id: 'music', file: 'music-workspace-20261023' },
  { label: '动手理解', id: 'explore', file: 'interactive-explanation' },
];

for (const width of [320, 390, 768, 1000, 1024, 1440]) {
  test(`workspace introduction and primary actions fit at ${width}px`, async ({ page }, info) => {
    const height = width === 390 ? 844 : 900;
    const errors: string[] = [];
    page.on('pageerror', error => errors.push(error.message));
    await page.setViewportSize({ width, height });
    await page.goto('/home.html');
    await page.evaluate(() => document.fonts.ready);
    const companion = page.locator('.home-hero-companion');
    await expect(companion).toContainText('页页');
    await expect(companion.locator('.yeye-mascot')).toHaveAttribute('aria-hidden', 'true');
    const pictures = page.locator('.home-nav .brand-logo, .home-hero-workspace img');
    await expect(pictures).toHaveCount(3);
    for (const picture of await pictures.all()) await expectImageLoaded(picture);

    const bounds = await page.locator('.home-hero-copy, .home-hero-workspace')
      .evaluateAll(elements => elements.map(element => {
        const { left, right, top, bottom } = element.getBoundingClientRect();
        return { left, right, top, bottom };
      }));
    for (const box of bounds) {
      expect(box.left).toBeGreaterThanOrEqual(0);
      expect(box.right).toBeLessThanOrEqual(width);
    }
    for (let i = 0; i < bounds.length; i++) {
      for (const b of bounds.slice(i + 1)) {
        const a = bounds[i]!;
        expect(a.right <= b.left || b.right <= a.left || a.bottom <= b.top || b.bottom <= a.top).toBe(true);
      }
    }
    const [copy, workspace] = bounds;
    if (width <= 640) {
      expect(copy!.bottom).toBeLessThanOrEqual(workspace!.top);
    }
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);

    const start = page.getByRole('link', { name: '看看怎样学', exact: true });
    if (width === 390 || width === 1440) {
      await expect(start).toBeInViewport({ ratio: 1 });
      await page.screenshot({ path: info.outputPath(`home-viewport-${width}.png`) });
      await page.locator('.home-hero').screenshot({ path: info.outputPath(`home-workspace-${width}.png`) });
    }
    await start.focus();
    await page.keyboard.press('Enter');
    await expect(page).toHaveURL(/#experience$/);
    await expect(page.getByRole('tablist', { name: '选择学习场景' })).toBeInViewport();
    await page.getByRole('link', { name: '试着想明白一个问题', exact: true }).click();
    await expect(page).toHaveURL(/#learning-story$/);
    await expect(page.getByRole('radio', { name: '把一边拉得越长越好', exact: true })).toBeInViewport();
    await page.locator('.home-hero-actions').getByRole('link', { name: '参与内测', exact: false }).click();
    await expect(page).toHaveURL(/#begin$/);
    await expect(page.getByRole('link', { name: '已有内测码，开始注册', exact: false })).toBeInViewport();
    await expect(page.getByRole('link', { name: '已经有账号？继续阅读', exact: false })).toHaveAttribute('href', './?account=login');
    expect(errors).toEqual([]);
  });
}

for (const width of [390, 1440]) {
  test(`workspace and learning scenes open the matching full-size screenshots at ${width}px`, async ({ page }, info) => {
    const errors: string[] = [];
    page.on('pageerror', error => errors.push(error.message));
    await page.setViewportSize({ width, height: 900 });
    await page.goto('/home.html');
    const panel = page.getByRole('tabpanel');
    const dialog = page.getByRole('dialog');
    const imageViewport = dialog.locator('.home-image-viewport');
    const dialogImage = imageViewport.locator('img');
    const previews = [
      { label: '阅读界面', id: 'hero', file: 'reading-workspace-20261024', container: '.home-hero-workspace' },
      { label: '学习工作区', id: 'workspace', file: 'course-workspace-20261021', container: '#workspace' },
      ...scenes.map(scene => ({ ...scene, container: '#scene-panel' })),
    ];
    for (const [index, scene] of previews.entries()) {
      if (scene.container === '#scene-panel') {
        const tab = page.getByRole('tab', { name: scene.label, exact: true });
        await tab.click();
        await expect(tab).toHaveAttribute('aria-selected', 'true');
        await expect(page.getByRole('tab', { selected: true })).toHaveCount(1);
        await expect(panel).toHaveAttribute('aria-labelledby', `scene-tab-${scene.id}`);
      }
      const preview = page.locator(scene.container).getByRole('button', { name: `放大查看：${scene.label}`, exact: true });
      await preview.scrollIntoViewIfNeeded();
      await expect(preview.locator('img')).toHaveAttribute('src', new RegExp(scene.file));
      await expectImageLoaded(preview.locator('img'));
      await preview.screenshot({ path: info.outputPath(`${scene.id}-${width}.png`) });
      await preview.focus();
      await preview.press('Enter');
      await expect(dialog).toBeVisible();
      await expect(dialog.getByRole('heading')).toHaveText(scene.label);
      await expect(dialogImage).toHaveAttribute('src', (await preview.locator('img').getAttribute('src'))!);
      await expectImageLoaded(dialogImage);
      const originalSize = dialog.getByRole('button', { name: '原始尺寸', exact: true });
      await expect(originalSize).toHaveAttribute('aria-pressed', 'false');
      expect(await dialogImage.evaluate(img => img.getBoundingClientRect().width))
        .toBeLessThanOrEqual(await imageViewport.evaluate(el => el.clientWidth + 1));
      await originalSize.click();
      await expect(dialog.getByRole('button', { name: '适应窗口', exact: true })).toHaveAttribute('aria-pressed', 'true');
      const size = await dialogImage.evaluate(node => {
        const image = node as HTMLImageElement;
        return { rendered: image.getBoundingClientRect().width, original: image.naturalWidth };
      });
      expect(size.rendered).toBeCloseTo(size.original, 0);
      if (width === 390) {
        expect(await imageViewport.evaluate(el => el.scrollWidth > el.clientWidth)).toBe(true);
      }
      if (index % 2 === 0) await page.keyboard.press('Escape');
      else await dialog.getByRole('button', { name: '关闭大图', exact: true }).click();
      await expect(dialog).not.toBeVisible();
      await expect(preview).toBeFocused();
    }

    const reading = page.getByRole('tab', { name: '阅读界面', exact: true });
    await reading.click();
    for (const [key, index] of [['ArrowLeft', 3], ['ArrowRight', 0], ['End', 3], ['Home', 0], ['ArrowRight', 1]] as const) {
      await page.keyboard.press(key);
      const expectedScene = scenes[index]!;
      const selected = page.getByRole('tab', { name: expectedScene.label, exact: true });
      await expect(selected).toBeFocused();
      await expect(selected).toHaveAttribute('aria-selected', 'true');
      await expect(selected).toHaveAttribute('tabindex', '0');
      await expect(page.locator('.home-gallery-tabs [tabindex="0"]')).toHaveCount(1);
      await expect(panel).toHaveAttribute('aria-labelledby', `scene-tab-${expectedScene.id}`);
      await expect(panel.locator('img')).toHaveAttribute('src', new RegExp(expectedScene.file));
    }
    await page.keyboard.press('Tab');
    await expect(panel).toBeFocused();
    await page.keyboard.press('Tab');
    const videoPreview = panel.getByRole('button', { name: '放大查看：理解视频', exact: true });
    await expect(videoPreview).toBeFocused();
    await page.keyboard.press('Enter');
    await expect(dialog).toBeVisible();
    await dialog.getByRole('button', { name: '原始尺寸', exact: true }).click();
    await dialog.getByRole('button', { name: '适应窗口', exact: true }).click();
    await expect(dialog.getByRole('button', { name: '原始尺寸', exact: true })).toHaveAttribute('aria-pressed', 'false');
    expect(await dialogImage.evaluate(img => img.getBoundingClientRect().width))
      .toBeLessThanOrEqual(await imageViewport.evaluate(el => el.clientWidth + 1));
    await page.keyboard.press('Escape');
    await expect(videoPreview).toBeFocused();
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
    expect(errors).toEqual([]);
  });
}

for (const width of [1440, 390]) {
  test(`garden experiment keeps attempts and checks changed conditions at ${width}px`, async ({ page }, info) => {
    await page.setViewportSize({ width, height: 900 });
    await page.goto('/home.html');
    await page.getByRole('link', { name: '试着想明白一个问题', exact: true }).click();
    await page.getByRole('radio', { name: '把一边拉得越长越好', exact: true }).check();
    await page.getByRole('button', { name: '带着猜想，动手试试', exact: true }).click();
    const slider = page.getByRole('slider', { name: '调整一条边的长度' });
    await slider.fill('2');
    await expect(page.getByRole('img', { name: '长 2 米，宽 8 米，面积 16 平方米的长方形花园' })).toBeVisible();
    await page.getByRole('button', { name: '记下这一种围法', exact: true }).click();
    await expect(page.getByRole('button', { name: '这一种已记下', exact: true })).toBeDisabled();
    await slider.focus();
    await slider.press('ArrowRight');
    await slider.press('ArrowRight');
    await slider.press('ArrowRight');
    await expect(slider).toHaveValue('5');
    await expect(page.getByRole('img', { name: '长 5 米，宽 5 米，面积 25 平方米的长方形花园' })).toBeVisible();
    await page.getByRole('button', { name: '记下这一种围法', exact: true }).click();
    await slider.fill('9');
    await expect(page.getByRole('img', { name: '长 9 米，宽 1 米，面积 9 平方米的长方形花园' })).toBeVisible();
    await page.getByRole('button', { name: '说说你发现了什么', exact: true }).click();
    const reflection = '我原来以为越长越好，现在发现长宽一起决定面积。';
    await page.getByLabel('用自己的话说一句').fill(reflection);
    await page.locator('.home-demo-explanation summary').click();
    await expect(page.locator('.home-demo-explanation')).toContainText('25 − d²');
    await page.getByRole('button', { name: '换个条件，再想一次', exact: true }).click();
    await expect(page.getByRole('button', { name: '用计算验证', exact: true })).toBeDisabled();
    await page.getByRole('radio', { name: '50 平方米', exact: true }).check();
    await page.getByRole('button', { name: '用计算验证', exact: true }).click();
    await expect(page.getByRole('status')).toContainText('你刚才选了 50 平方米');
    await page.getByRole('button', { name: '看看下次从哪里继续', exact: true }).click();
    await expect(page.locator('.home-learning-note')).toContainText(reflection);
    await expect(page.locator('.home-learning-note')).toContainText('2 × 8 = 16 m²；5 × 5 = 25 m²');
    await expect(page.locator('.home-learning-note')).toContainText('你选了 50 平方米，计算结果是 100 平方米');
    await expect(page.locator('#learning-panel')).toContainText('先把新边长和新面积一起算一遍');
    await page.getByRole('button', { name: '04 换个条件', exact: false }).click();
    await page.getByRole('radio', { name: '100 平方米', exact: true }).check();
    await expect(page.locator('.home-transfer-result')).toHaveCount(0);
    await page.getByRole('button', { name: '用计算验证', exact: true }).click();
    await expect(page.getByRole('status')).toContainText('这次判断与计算一致');
    await page.getByRole('button', { name: '看看下次从哪里继续', exact: true }).click();
    await expect(page.locator('#learning-panel')).toContainText('你已经对照过围栏翻倍后的计算');
    if (width < 850) {
      const activeStep = await page.locator('.home-story-tabs [aria-current="step"]').boundingBox();
      expect(activeStep!.x).toBeGreaterThanOrEqual(0);
      expect(activeStep!.x + activeStep!.width).toBeLessThanOrEqual(width);
    }
    await page.screenshot({ path: info.outputPath(`recap-${width}.png`) });
  });
}

test('skipped steps do not invent attempts, and a fresh page clears the illustration', async ({ page }) => {
  await page.goto('/home.html');
  await page.getByRole('button', { name: '05 下次接着学', exact: false }).click();
  await expect(page.locator('#learning-panel')).toContainText('还没有留下尝试');
  await expect(page.locator('.home-learning-note')).not.toContainText('最初的猜想');
  await expect(page.locator('.home-learning-note')).not.toContainText('记下的围法');
  await page.getByRole('button', { name: '01 先猜一猜', exact: false }).click();
  await page.getByRole('button', { name: '我想先看解释', exact: true }).click();
  await expect(page.locator('.home-demo-explanation')).toHaveAttribute('open', '');
  await expect(page.locator('.home-demo-explanation')).toContainText('25 − d²');
  await page.getByLabel('用自己的话说一句').fill('还没想通为什么长宽相等最好。');
  await page.getByRole('button', { name: '05 下次接着学', exact: false }).click();
  await expect(page.locator('.home-learning-note')).toContainText('还没想通为什么长宽相等最好');
  await expect(page.locator('.home-learning-note')).toContainText('看过的帮助');
  await expect(page.locator('.home-learning-note')).not.toContainText('记下的围法');
  await page.reload();
  await page.getByRole('button', { name: '05 下次接着学', exact: false }).click();
  await expect(page.locator('#learning-panel')).toContainText('还没有留下尝试');
  await expect(page.locator('.home-learning-note')).not.toContainText('看过的帮助');
});

test('homepage and experiment fit small and large screens', async ({ page }, info) => {
  const errors: string[] = [];
  page.on('pageerror', error => errors.push(error.message));
  for (const width of [320, 390, 768, 1440]) {
    await page.setViewportSize({ width, height: 900 });
    await page.goto('/home.html');
    await page.evaluate(() => document.fonts.ready);
    await expect(page.getByRole('heading', { level: 1 })).toContainText('学成自己的理解');
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
    const title = await page.locator('#home-title').boundingBox();
    expect(title!.x + title!.width).toBeLessThanOrEqual(width);
    if (width < 850) {
      const secondStep = await page.getByRole('button', { name: '02 动手试试', exact: false }).boundingBox();
      expect(secondStep!.x + secondStep!.width).toBeLessThan(width);
    }
    for (const label of ['02 动手试试', '03 说说理由', '04 换个条件', '05 下次接着学']) {
      await page.getByRole('button', { name: label, exact: false }).click();
      expect(await page.locator('#learning-panel').evaluate(el => el.scrollWidth <= el.clientWidth)).toBe(true);
    }
    await page.getByRole('button', { name: '02 动手试试', exact: false }).click();
    await page.getByRole('slider', { name: '调整一条边的长度' }).fill('5');
    await page.locator('#learning-story').screenshot({ path: info.outputPath(`experiment-${width}.png`) });
    await page.evaluate(() => { (document.activeElement as HTMLElement)?.blur(); scrollTo({ top: 0, behavior: 'instant' }); });
    await expect.poll(() => page.evaluate(() => scrollY)).toBe(0);
    await page.screenshot({ path: info.outputPath(`home-${width}.png`) });
  }
  await expect(page.getByRole('link', { name: '已有内测码，开始注册', exact: false })).toHaveAttribute('href', './?account=register');
  expect(errors).toEqual([]);
});
