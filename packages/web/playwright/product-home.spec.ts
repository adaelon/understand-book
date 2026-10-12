import { expect, test } from '@playwright/test';

for (const width of [1440, 390]) {
  test(`garden experiment keeps attempts and checks changed conditions at ${width}px`, async ({ page }) => {
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
    await page.screenshot({ path: `../../tmp/home-learning-20261012/recap-${width}.png` });
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

test('homepage and experiment fit small and large screens; product images still open', async ({ page }) => {
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
    await page.locator('#learning-story').screenshot({ path: `../../tmp/home-learning-20261012/experiment-${width}.png` });
    await page.evaluate(() => { (document.activeElement as HTMLElement)?.blur(); scrollTo({ top: 0, behavior: 'instant' }); });
    await expect.poll(() => page.evaluate(() => scrollY)).toBe(0);
    await page.screenshot({ path: `../../tmp/home-learning-20261012/home-${width}.png` });
  }
  await page.getByRole('button', { name: '放大查看：阅读现场', exact: true }).click();
  await expect(page.getByRole('dialog')).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(page.getByRole('dialog')).not.toBeVisible();
  await expect(page.getByRole('link', { name: '已有内测码，开始注册', exact: false })).toHaveAttribute('href', './?account=register');
  expect(errors).toEqual([]);
});
