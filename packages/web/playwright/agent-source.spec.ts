import { expect, test, type Page } from "@playwright/test";

const contextBefore = "在心肌组织中，选择性剪接受到多层调控。研究团队在严格控制批次效应后比较了多个样本，并以相同分析流程验证候选事件。";
const evidence = "剪接调控显著改变了疾病相关转录本的构成。";
const cohortEvidence = "独立队列复核了这些剪接变化。";
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
          : body.source_ref_id === "source_ref_cohort" ? "正文 · Validation cohort" : "正文 · Results",
        highlighted_quote: evidence,
        context_before: contextBefore,
        context_after: contextAfter,
        excerpt: { text, highlight: { start: text.indexOf(evidence), end: text.indexOf(evidence) + evidence.length } },
        stale: false,
        can_open_in_reader: true,
      }),
    });
  });
  await page.route("**/api/agent/source.read", async route => {
    const { source, direction, include_outline } = route.request().postDataJSON();
    calls.push(direction);
    const segment = (lid: string, text: string, kind = 'paragraph', highlights: { start: number; end: number }[] = []) => ({ lid, text, kind, highlights });
    const quote = source.source_ref_id === 'source_ref_cohort' ? cohortEvidence : evidence;
    const initial = [
      segment('2.1', '## 实验方法', 'section'), segment('2.2', contextBefore.repeat(6)),
      segment('2.3', quote, 'paragraph', [{ start: 0, end: quote.length }]),
      segment('2.4', '| 教学候选 | 选入章节 |\n| --- | --- |\n| 缓存复用已有结果 | 是 |\n| 数据变化后使缓存失效 | 是 |', 'table'),
      segment('2.5', '![候选进入路线的示意图](images/cache.svg)', 'image'),
      segment('2.6', contextAfter.repeat(6)), segment('2.7', '- 独立队列\n- 功能实验'),
      segment('2.8', '$$E=mc^2$$', 'formula'), segment('2.9', '\`\`\`python\ncount = 2\n\`\`\`', 'code'),
    ];
    await route.fulfill({ json: {
      source_ref_id: source.source_ref_id, source_lid: '2.3', focus_lid: '2.3',
      segments: direction === 'before' ? [segment('1.1', '实验开展前的研究背景。')] :
        direction === 'after' ? [segment('3.1', '实验结果的后续验证。')] : initial,
      previous_lid: direction === 'around' ? '2.1' : null,
      next_lid: direction === 'around' ? '2.9' : null,
      ...(include_outline ? { outline: [{ lid: '2', title: '实验方法', depth: 0 }] } : {}),
    } });
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

test("desktop answer opens a continuous reader at the citation and reads in both directions", async ({ page }, testInfo) => {
  await page.setViewportSize({ width: 1440, height: 640 });
  const calls = await installSourceRoutes(page);
  await page.goto("/agent-source-visual.html");

  const sourceButtons = page.locator(".agent-source-button");
  await expect(sourceButtons).toHaveCount(2);
  await expect(sourceButtons.nth(0)).toContainText("Materials and … [1]");
  await expect(sourceButtons.nth(1)).toContainText("2 个来源");
  const paragraphBox = await page.locator(".answer-markdown p").first().boundingBox();
  const buttonBox = await sourceButtons.first().boundingBox();
  expect(buttonBox!.y).toBeLessThan(paragraphBox!.y + paragraphBox!.height);

  await sourceButtons.first().click();
  const dialog = page.getByRole("dialog", { name: "回答来源" });
  const citation = dialog.locator("mark");
  await expect(citation).toHaveText(evidence);
  await expect(citation).toBeInViewport();
  const reader = dialog.getByLabel("原文阅读区");
  expect(await reader.evaluate(node => node.scrollTop)).toBeGreaterThan(0);
  await expect(dialog.getByRole("navigation", { name: "原文目录" })).toBeVisible();
  await expect(dialog.locator('li')).toHaveCount(2);
  await expect(dialog.locator('.katex-mathml')).toHaveCSS('position', 'absolute');
  await expect(dialog.locator('pre code')).toContainText('count = 2');
  await expect(dialog.locator('table tr')).toHaveCount(3);
  await expect.poll(() => dialog.locator('img').evaluate((img: HTMLImageElement) => img.complete && img.naturalWidth)).toBe(800);
  await expect(page.getByTestId("reader-status")).toHaveText("保持当前阅读位置");
  expect(calls).toEqual(["resolve", "around"]);
  await expectInsideViewport(page, ".agent-source-popup", 1440, 640);
  await page.screenshot({ path: testInfo.outputPath("agent-source-desktop.png"), fullPage: true });

  await reader.evaluate(node => { node.scrollTop = 0; });
  await reader.hover();
  await page.mouse.wheel(0, -650);
  await expect(reader).toContainText("实验开展前的研究背景。");
  await reader.evaluate(node => { node.scrollTop = node.scrollHeight; });
  await page.mouse.wheel(0, 650);
  await expect(reader).toContainText("实验结果的后续验证。");
  expect(calls).toEqual(["resolve", "around", "before", "after"]);

  await dialog.getByRole("button", { name: "回到引用处" }).click();
  await expect(citation).toBeInViewport();
  await expect(page.getByTestId("reader-status")).toHaveText("保持当前阅读位置");
  await page.keyboard.press("Escape");
  await expect(dialog).toBeHidden();
  await expect(sourceButtons.first()).toBeFocused();

  await sourceButtons.first().click();
  await dialog.getByRole("button", { name: "在正文中查看" }).click();
  await expect(page.getByTestId("reader-status")).toHaveText("已在正文中打开来源");
  expect(calls.at(-1)).toBe("open");
});

test("mobile answer reader fits the viewport and switches between citations", async ({ page }, testInfo) => {
  await page.setViewportSize({ width: 390, height: 844 });
  const calls = await installSourceRoutes(page);
  await page.goto("/agent-source-visual.html");
  await page.locator(".agent-source-button").nth(1).click();

  const dialog = page.getByRole("dialog", { name: "回答来源" });
  await expect(dialog.locator("mark")).toBeInViewport();
  await expect(dialog.locator('table tr')).toHaveCount(3);
  await expectInsideViewport(page, ".agent-source-popup", 390, 844);
  await expect(dialog.getByRole("navigation", { name: "原文目录" })).toBeHidden();
  await dialog.getByRole("button", { name: "目录", exact: true }).click();
  await expect(dialog.getByRole("navigation", { name: "原文目录" })).toBeVisible();
  await dialog.getByRole("navigation", { name: "原文目录" }).getByRole("button", { name: "收起目录" }).click();
  const tabs = dialog.getByRole('tab');
  await expect(tabs).toHaveCount(2);
  await tabs.nth(1).click();
  await expect(tabs.nth(1)).toHaveAttribute('aria-selected', 'true');
  await expect(dialog.locator('h2').first()).toContainText('Validation cohort');
  await expect(dialog.locator('mark')).toHaveText(cohortEvidence);
  await expect(dialog.locator('mark')).toBeInViewport();
  expect(calls).toEqual(["resolve", "resolve", "around", "around"]);

  const overflow = await dialog.evaluate(node => node.scrollWidth > node.clientWidth + 1);
  expect(overflow).toBe(false);
  await page.screenshot({ path: testInfo.outputPath("agent-source-mobile.png"), fullPage: true });
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
