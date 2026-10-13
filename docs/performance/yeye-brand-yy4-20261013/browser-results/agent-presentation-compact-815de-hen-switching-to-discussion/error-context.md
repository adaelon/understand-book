# Instructions

- Following Playwright test failed.
- Explain why, be concise, respect Playwright best practices.
- Provide a snippet of code with the fix, if possible.

# Test info

- Name: agent-presentation.spec.ts >> compact mobile toolbar remains reachable when switching to discussion
- Location: playwright\agent-presentation.spec.ts:117:1

# Error details

```
Error: apiRequestContext.post: connect ECONNREFUSED 127.0.0.1:4175
Call log:
  - → POST http://127.0.0.1:4175/reset-scene
    - user-agent: Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) HeadlessChrome/149.0.7827.55 Safari/537.36
    - accept: */*
    - accept-encoding: gzip,deflate,br

```

# Test source

```ts
  1   | import { expect, test } from "@playwright/test";
  2   | 
  3   | test.beforeEach(async ({ page }) => {
> 4   |   await page.request.post("http://127.0.0.1:4175/reset-scene");
      |                      ^ Error: apiRequestContext.post: connect ECONNREFUSED 127.0.0.1:4175
  5   |   // Real Rust acceptance host owns persistence, semantics and source navigation.
  6   |   await page.route("**/api/**", async route => {
  7   |     const url = route.request().url().replace(/^.*\/api/, "http://127.0.0.1:4175");
  8   |     const response = await route.fetch({ url });
  9   |     await route.fulfill({ response });
  10  |   });
  11  | });
  12  | 
  13  | test.afterEach(async ({ page }) => {
  14  |   await page.unrouteAll({ behavior: "ignoreErrors" });
  15  | });
  16  | 
  17  | test("MU7 frame messages require the current window, channel and bound source", async ({ page }) => {
  18  |   await page.goto("/agent-presentation-visual.html");
  19  |   const frame = page.frameLocator(".agent-presentation iframe");
  20  |   await expect(frame.locator("#result")).toBeVisible();
  21  |   const channel = await frame.locator("body").evaluate(() => {
  22  |     const script = document.head.querySelector("script")!.textContent!;
  23  |     return /"channel":"([^"]+)"/.exec(script)![1];
  24  |   });
  25  |   await page.evaluate(channel => window.postMessage({ channel, kind: "source", source_ref_id: "source-1" }, "*"), channel);
  26  |   await frame.locator("body").evaluate((_, channel) => {
  27  |     parent.postMessage({ channel: "agent-presentation", kind: "error" }, "*");
  28  |     parent.postMessage({ channel: channel + "-old", kind: "error" }, "*");
  29  |     parent.postMessage({ channel, kind: "source", source_ref_id: "another-user-source" }, "*");
  30  |     parent.postMessage({ channel, kind: "navigate", url: "/api/me/chats" }, "*");
  31  |   }, channel);
  32  |   await page.waitForTimeout(100);
  33  |   await expect(page.getByRole("dialog", { name: "回答来源" })).toHaveCount(0);
  34  |   await expect(frame.locator("#result")).toBeVisible();
  35  |   await frame.locator("[data-source-ref]").first().click();
  36  |   await expect(page.getByRole("dialog", { name: "回答来源" })).toBeVisible();
  37  | });
  38  | 
  39  | test("rich answer uses bound static/dynamic sources, one frame, responsive layout and theme", async ({ page }, info) => {
  40  |   await page.setViewportSize({ width: 1100, height: 850 });
  41  |   await page.goto("/agent-presentation-visual.html");
  42  |   const frame = page.frameLocator(".agent-presentation iframe");
  43  |   await expect(frame.locator("#result")).toBeVisible();
  44  |   await expect(frame.locator("#result")).toHaveText("2/3");
  45  |   await expect(frame.locator("body")).toHaveAttribute("data-isolated", "yes");
  46  |   expect(await page.locator("body").getAttribute("data-presentation-escaped")).toBeNull();
  47  |   const source = frame.locator("[data-source-ref]").first();
  48  |   await expect(source).not.toHaveText("页面自定标签");
  49  |   await source.click();
  50  |   await expect(page.getByRole("dialog", { name: "回答来源" })).toBeVisible();
  51  |   await page.getByRole("button", { name: "在正文中查看" }).click();
  52  |   await expect(page.getByTestId("reader-status")).toHaveText("已在正文中打开来源");
  53  |   await frame.locator("#irrelevant").click();
  54  |   await expect(frame.locator("#result")).toBeVisible();
  55  |   await expect(frame.locator("#result")).toHaveText("2/3");
  56  |   await frame.locator("#complete").click();
  57  |   await expect(frame.locator("#result")).toBeVisible();
  58  |   await expect(frame.locator("#result")).toHaveText("1");
  59  |   await frame.locator("#dynamic [data-source-ref]").click();
  60  |   await expect(page.getByRole("dialog", { name: "回答来源" })).toBeVisible();
  61  |   await page.getByRole("button", { name: "在正文中查看" }).click();
  62  |   await page.getByRole("button", { name: "展开", exact: true }).click();
  63  |   await expect(frame.locator("#result")).toHaveText("1");
  64  |   await expect(page.locator(".agent-presentation iframe")).toHaveCount(1);
  65  |   const expanded = await page.locator(".agent-presentation").boundingBox();
  66  |   expect(expanded?.x).toBe(12);
  67  |   expect(expanded?.width).toBeCloseTo(1100 * .63 - 12, 0);
  68  |   expect(expanded!.height).toBeGreaterThan(780);
  69  |   expect((await page.locator('.agent-presentation iframe').boundingBox())!.height).toBeGreaterThan(720);
  70  |   await expect(page.locator('.presentation-workspace .agent-input')).toBeVisible();
  71  |   await frame.locator("#dynamic [data-source-ref]").click();
  72  |   await expect(page.getByRole("dialog", { name: "回答来源" })).toBeVisible();
  73  |   await page.getByRole("button", { name: "在正文中查看" }).click();
  74  |   await page.screenshot({ path: info.outputPath("expanded.png") });
  75  |   await page.getByRole("button", { name: "收起", exact: true }).click();
  76  |   await expect(frame.locator("#result")).toHaveText("1");
  77  |   await page.setViewportSize({ width: 390, height: 844 });
  78  |   const overflow = await frame.locator("body").evaluate(node => node.scrollWidth > document.documentElement.clientWidth + 1);
  79  |   expect(overflow).toBe(false);
  80  |   await page.getByRole("button", { name: "展开", exact: true }).click();
  81  |   const narrow = await page.locator(".agent-presentation").boundingBox();
  82  |   expect(narrow?.width).toBe(374);
  83  |   await page.getByRole("button", { name: "收起", exact: true }).click();
  84  |   await expect(frame.locator("#result")).toHaveText("1");
  85  |   await page.evaluate(() => {
  86  |     for (const [name, value] of Object.entries({ "--canvas": "#202020", "--ink": "#f2ece3", "--surface": "#303030", "--line": "#555555", "--accent": "#f0a080" })) {
  87  |       document.documentElement.style.setProperty(name, value);
  88  |     }
  89  |   });
  90  |   await expect.poll(() => frame.locator("body").evaluate(node => getComputedStyle(node).backgroundColor)).toBe("rgb(32, 32, 32)");
  91  |   await page.screenshot({ path: info.outputPath("narrow-theme.png") });
  92  |   await page.getByRole('button', { name: '文字说明与来源', exact: true }).click();
  93  |   await expect(page.locator(".readable-text")).toContainText("需要三处证据");
  94  | });
  95  | 
  96  | test('content zoom preserves the live controls, source navigation and iframe', async ({ page }) => {
  97  |   await page.setViewportSize({ width: 1280, height: 800 });
  98  |   await page.goto('/agent-presentation-visual.html');
  99  |   const frame = page.frameLocator('.agent-presentation iframe');
  100 |   await expect(frame.locator('#result')).toHaveText('2/3');
  101 |   await frame.locator('#complete').click();
  102 |   await expect(frame.locator('#result')).toHaveText('1');
  103 |   await page.getByRole('button', { name: '展开', exact: true }).click();
  104 |   await page.getByRole('button', { name: '缩小演示' }).click();
```