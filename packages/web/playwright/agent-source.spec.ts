import { expect, test, type Page } from "@playwright/test";

const contextBefore = "在心肌组织中，选择性剪接受到多层调控。研究团队在严格控制批次效应后比较了多个样本，并以相同分析流程验证候选事件。";
const evidence = "剪接调控显著改变了疾病相关转录本的构成。";
const contextAfter = "这些变化随后在独立队列中得到复核，同时结合功能实验评估其与疾病通路的关系。连续上下文保留了实验条件、比较对象和结论边界。";

async function installSourceRoutes(page: Page) {
  const calls: string[] = [];
  await page.route('**/api/book/asset_manifest', route => route.fulfill({ json: { version: 'asset_manifest.v1', book_id: 'source-book', images: [
    { original_src: 'images/cache.svg', url_path: '/api/book/assets/cache.svg', status: 'available' },
  ] } }));
  await page.route('**/api/book/assets/cache.svg', route => route.fulfill({ contentType: 'image/svg+xml', body: '<svg xmlns="http://www.w3.org/2000/svg" width="800" height="220" viewBox="0 0 800 220"><rect width="800" height="220" rx="20" fill="#f0ede5"/><rect x="50" y="65" width="260" height="90" rx="16" fill="#e0dccf"/><path d="M340 110H450m-18-14 18 14-18 14" stroke="#9a4e22" stroke-width="5" fill="none"/><rect x="490" y="65" width="260" height="90" rx="16" fill="#d8e3d6"/><g font-family="sans-serif" font-size="26" text-anchor="middle" fill="#343a32"><text x="180" y="120">Candidate</text><text x="620" y="120">Selected</text></g></svg>' }));
  await page.route("**/api/agent/source.resolve", async (route) => {
    calls.push("resolve");
    const body = route.request().postDataJSON() as { source_ref_id: string };
    const text = `## 实验方法\n\n${contextBefore}\n\n${evidence}\n\n| 教学候选 | 所属单元与来源 | 选入章节 | 进入宏观路线 |\n| 缓存复用已有结果 | 缓存，1.2 | 是 | 是 |\n| 数据变化后使缓存失效 | 缓存，1.3.2 | 是 | 否 |\n\n![候选进入路线的示意图](images/cache.svg)\n\n${contextAfter}\n\n- 独立队列\n- 功能实验\n\n$$E=mc^2$$\n\n\`\`\`python\ncount = 2\n\`\`\``;
    await route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({
        source_ref_id: body.source_ref_id,
        label: body.source_ref_id === "source_ref_methods"
          ? "正文 · Materials and Methods"
          : "正文 · Results",
        highlighted_quote: evidence,
        context_before: contextBefore,
        context_after: contextAfter,
        excerpt: { text, highlight: { start: text.indexOf(evidence), end: text.indexOf(evidence) + evidence.length } },
        stale: false,
        can_open_in_reader: true,
      }),
    });
  });
  await page.route("**/api/agent/source.open", async (route) => {
    calls.push("open");
    const body = route.request().postDataJSON() as { source_ref_id: string };
    await route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({ source_ref_id: body.source_ref_id, opened: true }),
    });
  });
  return calls;
}

async function expectInsideViewport(page: Page, selector: string, width: number, height: number) {
  const box = await page.locator(selector).boundingBox();
  expect(box).not.toBeNull();
  expect(box!.x).toBeGreaterThanOrEqual(0);
  expect(box!.y).toBeGreaterThanOrEqual(0);
  expect(box!.x + box!.width).toBeLessThanOrEqual(width);
  expect(box!.y + box!.height).toBeLessThanOrEqual(height);
}

test("desktop source stays inline and opens an anchored popup before reader navigation", async ({ page }, testInfo) => {
  await page.setViewportSize({ width: 1440, height: 640 });
  const calls = await installSourceRoutes(page);
  await page.goto("/agent-source-visual.html");

  const sourceButtons = page.locator(".agent-source-button");
  await expect(sourceButtons).toHaveCount(2);
  await expect(sourceButtons.nth(0)).toContainText("Materials and … [1]");
  await expect(sourceButtons.nth(1)).toContainText("2 个来源");
  const paragraphBox = await page.locator(".answer-markdown p").first().boundingBox();
  const buttonBox = await sourceButtons.first().boundingBox();
  expect(paragraphBox).not.toBeNull();
  expect(buttonBox).not.toBeNull();
  expect(buttonBox!.y).toBeLessThan(paragraphBox!.y + paragraphBox!.height);

  await sourceButtons.first().click();
  const popup = page.getByRole("dialog", { name: "回答来源" });
  await expect(popup).toBeVisible();
  await expect(popup.locator("mark")).toHaveText(evidence);
  await expect(popup.locator('h2')).toHaveText('实验方法');
  await expect(popup.locator('li')).toHaveCount(2);
  await expect(popup.locator('.katex')).toHaveCount(1);
  await expect(popup.locator('.katex-mathml')).toHaveCSS('position', 'absolute');
  await expect(popup.locator('pre code')).toContainText('count = 2');
  await expect(popup.locator('table tr')).toHaveCount(3);
  await expect.poll(() => popup.locator('img').evaluate((img: HTMLImageElement) => img.complete && img.naturalWidth)).toBe(800);
  await expect(page.getByTestId("reader-status")).toHaveText("保持当前阅读位置");
  expect(calls).toEqual(["resolve"]);
  await expectInsideViewport(page, ".agent-source-popup", 1440, 640);

  const popupBox = await popup.boundingBox();
  expect(popupBox).not.toBeNull();
  const popupIsBesideButton = (
    popupBox!.x + popupBox!.width <= buttonBox!.x - 8
    || popupBox!.x >= buttonBox!.x + buttonBox!.width + 8
  );
  expect(popupIsBesideButton).toBe(true);

  const overflow = await popup.evaluate((node) => ({
    horizontal: node.scrollWidth > node.clientWidth + 1,
    actions: Array.from(node.querySelectorAll("button")).some((button) => {
      const rect = button.getBoundingClientRect();
      return rect.left < 0 || rect.right > window.innerWidth;
    }),
  }));
  expect(overflow).toEqual({ horizontal: false, actions: false });
  await page.screenshot({ path: testInfo.outputPath("agent-source-desktop.png"), fullPage: true });

  await page.getByRole("button", { name: "在正文中查看" }).click();
  await expect(page.getByTestId("reader-status")).toHaveText("已在正文中打开来源");
  expect(calls).toEqual(["resolve", "open"]);
});

test("mobile source popup is a viewport-bound bottom sheet", async ({ page }, testInfo) => {
  await page.setViewportSize({ width: 390, height: 844 });
  const calls = await installSourceRoutes(page);
  await page.goto("/agent-source-visual.html");
  await page.locator(".agent-source-button").first().click();

  const popup = page.getByRole("dialog", { name: "回答来源" });
  await expect(popup.locator("mark")).toHaveText(evidence);
  await expect(popup.locator('table tr')).toHaveCount(3);
  await expect.poll(() => popup.locator('img').evaluate((img: HTMLImageElement) => img.complete && img.naturalWidth)).toBe(800);
  await popup.evaluate(el => Promise.all(el.getAnimations().map(animation => animation.finished)));
  const box = await popup.boundingBox();
  expect(box).not.toBeNull();
  expect(Math.round(box!.x)).toBe(0);
  expect(Math.round(box!.width)).toBe(390);
  expect(Math.round(box!.y + box!.height)).toBe(844);
  await expectInsideViewport(page, ".agent-source-popup", 390, 844);
  expect(calls).toEqual(["resolve"]);

  const textOverflow = await popup.locator(".source-popup-head strong, .source-excerpt, .source-open-reader").evaluateAll(
    (nodes) => nodes.some((node) => node.scrollWidth > node.clientWidth + 1),
  );
  expect(textOverflow).toBe(false);
  const imageBox = (await popup.locator('img').boundingBox())!;
  expect(imageBox.width).toBeLessThanOrEqual(354);
  await page.screenshot({ path: testInfo.outputPath("agent-source-mobile.png"), fullPage: true });
  await popup.locator('img').scrollIntoViewIfNeeded();
  await page.screenshot({ path: testInfo.outputPath('agent-source-mobile-image.png'), fullPage: true });
});

test("desktop fullscreen and mobile conversation fill the viewport without losing the draft or answer", async ({ page }) => {
  for (const size of [{ width: 1440, height: 800 }, { width: 390, height: 844 }]) {
    await page.setViewportSize(size);
    await page.goto("/agent-source-visual.html");
    await page.locator(".agent-input textarea").fill("未发送的问题");
    const mobile = size.width < 1024;
    if (!mobile) await page.getByRole("button", { name: "问答全屏" }).click();
    const selector = mobile ? '.right-rail.conversation-mobile' : '.right-rail.fullscreen';
    const rail = page.locator(selector);
    await expect(rail).toBeVisible();
    await expectInsideViewport(page, selector, size.width, size.height);
    const box = await rail.boundingBox();
    expect(box).toMatchObject({ x: 0, y: 0, width: size.width });
    expect(Math.abs(box!.height - size.height)).toBeLessThan(1);
    await expect(page.locator(".agent-input textarea")).toHaveValue("未发送的问题");
    await expect(page.locator(".transcript")).toContainText("剪接调控会改变心肌细胞");
    if (!mobile) {
      await page.keyboard.press("Escape");
      await expect(rail).toHaveCount(0);
    }
    await expect(page.locator(".agent-input textarea")).toHaveValue("未发送的问题");
  }
});
