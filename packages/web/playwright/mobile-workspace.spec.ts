import { expect, test } from "@playwright/test";

test('mobile QA gives long answers the screen and preserves reading through menus', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 669 });
  await page.goto('/mobile-workspace-visual.html?long-chat');
  const nav = page.locator('.workspace-mobile-nav');
  await nav.getByRole('button', { name: '问答', exact: true }).click();
  const transcript = page.locator('.transcript');
  await expect.poll(async () => (await transcript.boundingBox())!.height).toBeGreaterThan(669 * .65);
  await page.setViewportSize({ width: 390, height: 560 });
  await expect.poll(async () => (await transcript.boundingBox())!.height).toBeGreaterThan(560 * .6);
  await transcript.evaluate(el => { el.scrollTop = 150; el.dispatchEvent(new Event('scroll')); });
  await nav.getByRole('button', { name: '阅读', exact: true }).click();
  await nav.getByRole('button', { name: '问答', exact: true }).click();
  expect(await transcript.evaluate(el => el.scrollTop)).toBeCloseTo(150, 0);
  for (const [label, id] of [['成果', 'artifacts'], ['画像', 'profile'], ['轨迹', 'trace'], ['公式', 'formula']]) {
    await nav.locator('summary').click();
    await nav.getByRole('button', { name: label, exact: true }).click();
    await expect(page.locator(`#reader-panel-${id}`)).toBeVisible();
  }
  await nav.getByRole('button', { name: '问答', exact: true }).click();
  await page.getByRole('button', { name: '问答操作', exact: true }).click();
  await page.getByRole('button', { name: '本书历史', exact: false }).click();
  await expect(page.locator('.history-dialog')).toBeVisible();
  await page.getByRole('button', { name: '关闭历史', exact: true }).click();
  await nav.getByRole('button', { name: '菜单', exact: true }).click();
  await expect(page.getByRole('button', { name: '管理教学会话', exact: true })).toBeVisible();
  await page.locator('.topbar-mobile-close').click();
  const input = page.locator('.agent-input textarea');
  await expect(input).toHaveValue('未发送草稿');
  await input.fill('第一行\n第二行\n第三行\n第四行\n第五行\n第六行');
  await expect.poll(async () => (await input.boundingBox())!.height).toBeGreaterThan(44);
  expect((await input.boundingBox())!.height).toBeLessThanOrEqual(120);
  await input.fill('');
  await expect.poll(async () => (await input.boundingBox())!.height).toBeLessThanOrEqual(46);
  await page.setViewportSize({ width: 390, height: 669 });
  await page.screenshot({ path: '../../tmp/qa-mobile-fix-20261003/answer-' + test.info().project.name + '.png' });
});

test('mobile QA keeps input visible when only the visual viewport shrinks', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 669 });
  await page.goto('/mobile-workspace-visual.html?long-chat');
  await page.locator('.workspace-mobile-nav').getByRole('button', { name: '问答', exact: true }).click();
  for (const fullscreen of [false, true]) {
    if (fullscreen) {
      await page.getByRole('button', { name: '问答操作', exact: true }).click();
      await page.getByRole('button', { name: '问答全屏', exact: true }).click();
    }
    await page.locator('.agent-input textarea').focus();
    await page.evaluate(() => {
      Object.defineProperty(visualViewport, 'height', { configurable: true, value: 350 });
      Object.defineProperty(visualViewport, 'offsetTop', { configurable: true, value: 80 });
      visualViewport!.dispatchEvent(new Event('resize'));
    });
    const send = page.locator('.agent-input').getByRole('button', { name: '发送', exact: true });
    await expect.poll(async () => { const r = await send.boundingBox(); return r!.y + r!.height; }).toBeLessThanOrEqual(430);
    expect((await page.locator('.agent-head').boundingBox())!.y).toBeGreaterThanOrEqual(80);
    expect((await page.locator('.transcript').boundingBox())!.height).toBeGreaterThan(150);
    await page.evaluate(() => {
      delete (visualViewport as unknown as { height?: number }).height;
      delete (visualViewport as unknown as { offsetTop?: number }).offsetTop;
      visualViewport!.dispatchEvent(new Event('resize'));
      (document.activeElement as HTMLElement).blur();
    });
    await expect.poll(async () => (await page.locator('.agent-input').boundingBox())!.y).toBeGreaterThan(450);
  }
});

test.beforeEach(async ({ page }) => {
  await page.goto("/mobile-workspace-visual.html");
  await expect(page.locator(".workspace-shell")).toBeVisible();
});

test("keeps one core tree through repeated region and orientation-sized projections", async ({ page }) => {
  const viewport = page.viewportSize()!;
  const shell = page.locator(".workspace-shell");
  const overflow = await page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth);
  expect(overflow).toBeLessThanOrEqual(1);

  await page.locator(".reader-pane").evaluate((node) => { node.setAttribute("data-instance", "reader-stable"); });
  if (viewport.width < 1024) {
    const topbar = page.locator(".topbar");
    const mobileNavigation = page.locator(".workspace-mobile-nav");
    await expect(topbar).toBeHidden();
    await expect(mobileNavigation).toBeVisible();
    expect((await shell.boundingBox())?.y).toBe(0);
    const mobileNavigationBox = await mobileNavigation.boundingBox();
    expect(mobileNavigationBox).not.toBeNull();
    expect(mobileNavigationBox!.y + mobileNavigationBox!.height).toBeLessThanOrEqual(viewport.height + 1);
    await page.getByRole("button", { name: "菜单" }).click();
    await expect(topbar).toBeVisible();
    await topbar.getByRole("button", { name: "目录" }).click();
    await expect(topbar).toBeHidden();
    await expect(page.locator("#reader-outline")).toBeVisible();
    const outlineBackdrop = page.getByRole("button", { name: "关闭目录" });
    const outlineBackdropBox = await outlineBackdrop.boundingBox();
    expect(outlineBackdropBox).not.toBeNull();
    await page.mouse.click(
      outlineBackdropBox!.x + outlineBackdropBox!.width - 4,
      outlineBackdropBox!.y + Math.min(24, outlineBackdropBox!.height / 2),
    );
    await expect(page.locator("#reader-outline")).toBeHidden();
    await page.getByRole("button", { name: "菜单" }).click();
    await topbar.getByRole("button", { name: "新对话" }).click();
    await expect(topbar).toBeHidden();
    await expect(page.locator(".fixture-new-chat-count")).toHaveAttribute("data-count", "1");
    for (let index = 0; index < 10; index += 1) {
      await page.getByRole("button", { name: "问答" }).click();
      await page.getByRole("button", { name: "阅读", exact: true }).click();
    }
    await expect(page.locator('.reader-pane[data-instance="reader-stable"]')).toHaveCount(1);
    await page.getByRole("button", { name: "问答" }).click();
    await expect(page.locator(".agent-input textarea")).toHaveValue("未发送草稿");
    await page.getByRole("button", { name: "阅读", exact: true }).click();
  } else {
    await expect(shell).toHaveAttribute("data-mode", "wide");
    await expect(page.locator(".topbar")).toBeVisible();
    await expect(page.getByRole("button", { name: "菜单" })).toHaveCount(0);
  }

  if (viewport.width >= 732 && viewport.width < 1024) {
    await page.getByRole("button", { name: "对照" }).click();
    await expect(shell).toHaveAttribute("data-mode", "compare");
  }
});

test("freezes a document selection independently of mouseup and preserves source text", async ({ page }) => {
  await page.evaluate(() => {
    const text = document.createTreeWalker(document.querySelector<HTMLElement>('[data-lid="1.1"]')!, NodeFilter.SHOW_TEXT).nextNode()!;
    const range = document.createRange();
    range.setStart(text, 0);
    range.setEnd(text, 4);
    const selection = window.getSelection()!;
    selection.removeAllRanges();
    selection.addRange(range);
    document.dispatchEvent(new Event("selectionchange"));
  });
  await expect(page.locator(".fixture-selection-action")).toContainText("移动阅读");
  await page.evaluate(() => {
    window.getSelection()?.removeAllRanges();
    document.dispatchEvent(new Event("selectionchange"));
  });
  await expect(page.locator(".fixture-selection-action")).toContainText("移动阅读");

  const source = page.locator(".asset-source.asset-code");
  const original = await source.textContent();
  await page.getByRole("button", { name: "换行" }).click();
  await expect(source).toHaveClass(/soft-wrap/);
  expect(await source.textContent()).toBe(original);
  await page.getByRole("button", { name: "展开" }).click();
  await expect(page.locator(".asset-block")).toHaveClass(/asset-expanded/);
});

test("does not submit while an IME composition is active", async ({ page }) => {
  const input = page.locator(".agent-input textarea");
  if (page.viewportSize()!.width < 1024) await page.getByRole("button", { name: "问答" }).click();
  await input.evaluate((element) => {
    element.dispatchEvent(new KeyboardEvent("keydown", {
      key: "Enter", ctrlKey: true, isComposing: true, bubbles: true,
    }));
  });
  await expect(page.locator(".fixture-send-count")).toHaveAttribute("data-count", "0");
  await input.evaluate((element) => {
    element.dispatchEvent(new KeyboardEvent("keydown", {
      key: "Enter", ctrlKey: true, isComposing: false, bubbles: true,
    }));
  });
  await expect(page.locator(".fixture-send-count")).toHaveAttribute("data-count", "1");
});

test('RE5 focus returns to the original projection and preserves live slots and draft', async ({ page }) => {
  const shell = page.locator('.workspace-shell');
  const originalMode = await shell.getAttribute('data-mode');
  await page.locator('.reader-pane').evaluate(el => el.setAttribute('data-instance', 'same-reader'));
  await page.locator('.right-rail').evaluate(el => el.setAttribute('data-instance', 'same-assistant'));
  const focus = page.getByRole('button', { name: '专注阅读', exact: true }).filter({ visible: true });
  await focus.click();
  await expect(shell).toHaveAttribute('data-mode', 'single');
  await expect(shell).toHaveAttribute('data-foreground', 'reader');
  await expect(page.locator('.right-rail')).toBeHidden();
  await page.locator('.workspace-mobile-nav').getByRole('button', { name: '问答', exact: true }).click();
  await expect(page.locator('.agent-input textarea')).toHaveValue('未发送草稿');
  await page.locator('.workspace-mobile-nav').getByRole('button', { name: '阅读', exact: true }).click();
  if (page.viewportSize()!.width >= 1024) await page.getByRole('button', { name: '展开工具栏', exact: true }).click();
  await page.getByRole('button', { name: '退出专注', exact: true }).filter({ visible: true }).click();
  await expect(shell).toHaveAttribute('data-mode', originalMode!);
  await expect(page.locator('[data-instance="same-reader"]')).toHaveCount(1);
  await expect(page.locator('[data-instance="same-assistant"]')).toHaveCount(1);
  expect(await page.evaluate(() => document.documentElement.scrollWidth - innerWidth)).toBeLessThanOrEqual(1);
});
