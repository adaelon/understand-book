import { expect, test } from "@playwright/test";

test.beforeEach(async ({ page }) => {
  await page.request.post("http://127.0.0.1:4175/reset-scene");
  await page.route("**/api/**", async route => {
    const response = await route.fetch({ url: route.request().url().replace(/^.*\/api/, "http://127.0.0.1:4175") });
    await route.fulfill({ response });
  });
});

test.afterEach(async ({ page }) => {
  await page.unrouteAll({ behavior: "ignoreErrors" });
});

test("gold sample keeps the three trajectories and true iteration values distinct", async ({ page }) => {
  await page.goto("/agent-presentation-visual.html");
  const frame = page.frameLocator(".agent-presentation iframe");
  await expect(frame.locator("#scene")).toBeVisible();
  await page.getByRole("button", { name: "展开", exact: true }).click();
  await frame.locator("#next").click();
  await expect(frame.locator("#numbers")).toContainText("0.8");
  const rows = await frame.locator("#numbers tr").allTextContents();
  expect(rows[0]).toContain("η=0.2");
  expect(rows[0]).not.toContain("[object");
  expect(rows[0]).toContain("0.8-1.21.44");
  expect(rows[1]).toContain("3.21.21.44");
  expect(rows[2]).toContain("4.42.45.76");
  await frame.locator("#next").click();
  const second = await frame.locator("#numbers tr").allTextContents();
  expect(second[0]).toContain("1.28-0.720.5184");
  expect(second[1]).toContain("1.28-0.720.5184");
  expect(second[2]).toContain("-0.88-2.888.2944");
  await frame.locator("#timeline").evaluate((node: HTMLInputElement) => {
    node.value = "250"; node.dispatchEvent(new Event("input", { bubbles: true })); node.dispatchEvent(new Event("change", { bubbles: true }));
  });
  await expect(frame.locator("#time-label")).toHaveText("真实第 2 步，过渡 50%");
  await expect(frame.locator("#readout")).toContainText("数字仍属于已完成的真实迭代");
  const midway = await frame.locator('[data-lane="cross"] .dot').getAttribute("cx");
  await frame.locator("#timeline").evaluate((node: HTMLInputElement) => { node.value = "275"; node.dispatchEvent(new Event("input", { bubbles: true })); });
  expect(Number(await frame.locator('[data-lane="slow"] .dot').getAttribute("cx"))).toBeLessThan(383.2);
  expect(Number(await frame.locator('[data-lane="cross"] .dot').getAttribute("cx"))).toBeGreaterThan(383.2);
  await frame.locator("#back").click();
  await expect(frame.locator('[data-lane="cross"] .ghost')).toHaveCount(1);
  await expect(frame.locator('[data-lane="cross"] .ghost circle')).toHaveCount(3);
  await frame.locator("#next").click();
  await frame.locator("#timeline").evaluate((node: HTMLInputElement) => { node.value = "250"; node.dispatchEvent(new Event("input", { bubbles: true })); });
  expect(await frame.locator('[data-lane="cross"] .dot').getAttribute("cx")).toBe(midway);
  await frame.locator('.preset[data-eta="1.1"]').click();
  await expect(frame.locator("#time-label")).toHaveText("真实第 0 步，过渡 0%");
  await expect(frame.locator('[data-lane="custom"] .ghost circle')).toHaveCount(1);
  const fixture = await page.request.get("http://127.0.0.1:4175/fixture").then(r => r.json());
  await expect.poll(async () => {
    const view = await page.request.post("http://127.0.0.1:4175/agent/presentation.read", { data: fixture }).then(r => r.json());
    return view.restored_state?.values?.page?.eta;
  }).toBe(1.1);
});

test("gold sample survives three viewports, saves a middle scene, reopens and follows up", async ({ page }, info) => {
  await page.setViewportSize({ width: 960, height: 720 });
  await page.goto("/agent-presentation-visual.html");
  const frame = page.frameLocator(".agent-presentation iframe");
  await expect(frame.locator("#scene")).toBeVisible();
  await page.getByRole("button", { name: "展开", exact: true }).click();
  await frame.locator("#play").click();
  await page.waitForTimeout(400);
  await frame.locator("#play").click();
  const paused = await frame.locator("body").evaluate(() => (window as any).ex1Scene.snapshot());
  expect(paused.playing).toBe(false);
  expect(paused.transition_progress).toBeGreaterThan(0);
  expect(paused.transition_progress).toBeLessThan(1);
  const fixture = await page.request.get("http://127.0.0.1:4175/fixture").then(r => r.json());
  await expect.poll(async () => {
    const view = await page.request.post("http://127.0.0.1:4175/agent/presentation.read", { data: fixture }).then(r => r.json());
    return view.restored_state?.values?.page?.transition_progress;
  }).toBeGreaterThan(0);
  await frame.locator("#timeline").evaluate((node: HTMLInputElement) => {
    node.value = "250"; node.dispatchEvent(new Event("input", { bubbles: true })); node.dispatchEvent(new Event("change", { bubbles: true }));
  });
  await frame.locator("#predict-cross").click();
  await frame.locator("#reveal").click();
  await expect(frame.locator("#prediction-result")).toContainText("下一步 w=2.432");
  await expect.poll(async () => {
    const view = await page.request.post("http://127.0.0.1:4175/agent/presentation.read", { data: fixture }).then(r => r.json());
    return view.restored_state?.values?.page?.transition_progress;
  }).toBe(0.5);
  await frame.locator("#scene").scrollIntoViewIfNeeded();
  await page.screenshot({ path: info.outputPath("desktop.png") });
  for (const viewport of [{ width: 320, height: 420 }, { width: 640, height: 240 }]) {
    await page.setViewportSize(viewport);
    await expect(frame.locator("#scene")).toBeVisible();
    expect(await frame.locator("body").evaluate(node => node.scrollWidth <= document.documentElement.clientWidth + 1)).toBe(true);
    const tooSmall = await frame.locator("button").evaluateAll(buttons => buttons.filter(button => {
      const box = button.getBoundingClientRect(); return box.width < 44 || box.height < 44;
    }).map(button => button.id || button.textContent));
    expect(tooSmall).toEqual([]);
    await frame.locator("#scene").scrollIntoViewIfNeeded();
    await page.screenshot({ path: info.outputPath(`${viewport.width}x${viewport.height}.png`) });
  }
  await page.request.post("http://127.0.0.1:4175/reopen");
  await page.reload();
  await expect(frame.locator("#time-label")).toHaveText("真实第 2 步，过渡 50%");
  await expect(frame.locator("#prediction-result")).toContainText("下一步 w=2.432");
  await expect(page.getByText("已恢复上次保存的现场")).toBeVisible();
  await page.getByRole("textbox", { name: "针对当前现场追问" }).fill("请解释现在的跨越过程");
  await page.getByRole("button", { name: "发送追问", exact: true }).click();
  await expect(page.getByTestId("follow-up-status")).toHaveText("追问已完成");
  const requests = await page.request.get("http://127.0.0.1:4175/requests").then(r => r.json());
  const user = requests.at(-1).filter((message: any) => message.role === "User").at(-1).content;
  expect(user).toContain('"semantic_state":2');
  expect(user).toContain('"transition_progress":0.5');
  expect(user).toContain('"reveal_state":true');
  expect(user).toContain("请解释现在的跨越过程");
});

test("gold sample measures five seconds of playback without frame-by-frame host saves", async ({ page }) => {
  await page.addInitScript(() => {
    const NativeObserver = window.MutationObserver;
    (window as any).__ex1ObserverMs = [];
    window.MutationObserver = class extends NativeObserver {
      constructor(callback: MutationCallback) {
        super((records, observer) => {
          const started = performance.now();
          callback(records, observer);
          (window as any).__ex1ObserverMs.push(performance.now() - started);
        });
      }
    };
  });
  let saves = 0;
  page.on("request", request => { if (request.url().endsWith("/presentation.state.save")) saves++; });
  await page.goto("/agent-presentation-visual.html");
  const frame = page.frameLocator(".agent-presentation iframe");
  await expect(frame.locator("#play")).toBeVisible();
  await frame.locator("body").evaluate(() => { (window as any).__ex1ObserverMs = []; });
  saves = 0;
  const sample = frame.locator("body").evaluate(async () => {
    const gaps: number[] = [];
    return await new Promise<{ frameP95: number; observerP95: number; frames: number; observations: number }>(resolve => {
      let previous: number | null = null;
      const started = performance.now();
      const percentile = (values: number[]) => values.length ? values.sort((a,b) => a-b)[Math.ceil(values.length * .95) - 1] : 0;
      const frame = (now: number) => {
        if (previous !== null) gaps.push(now - previous);
        previous = now;
        if (now - started >= 5100) {
          const observations = (window as any).__ex1ObserverMs as number[];
          resolve({ frameP95: percentile(gaps), observerP95: percentile(observations), frames:gaps.length, observations:observations.length });
        } else requestAnimationFrame(frame);
      };
      requestAnimationFrame(frame);
    });
  });
  await frame.locator("#play").click();
  const metrics = await sample;
  console.log(`EX5a ${JSON.stringify({ ...metrics, saves })}`);
  expect(metrics.frames).toBeGreaterThan(100);
  expect(metrics.frameP95).toBeLessThanOrEqual(50);
  expect(metrics.observerP95).toBeLessThanOrEqual(8);
  expect(saves).toBeLessThanOrEqual(8);
});
