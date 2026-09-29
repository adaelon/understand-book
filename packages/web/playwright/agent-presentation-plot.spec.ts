import { expect, test } from "@playwright/test";
import { readFileSync } from "node:fs";

test("a delivered Matplotlib SVG is assembled into the Reader frame", async ({ page }) => {
  const view = JSON.parse(readFileSync(new URL("../../../docs/performance/ed2-windows/real-view.json", import.meta.url), "utf8"));
  await page.setViewportSize({ width: 320, height: 420 });
  await page.goto("/");
  await page.setContent("<main><div id='mount'></div></main>");
  await page.evaluate(async view => {
    const { presentationDocument } = await import("/src/presentation-document.ts");
    window.addEventListener("message", event => {
      if (event.data?.kind === "observe") (event.source as Window).postMessage({ channel: "agent-presentation", kind: "accepted", revision: event.data.revision }, "*");
    });
    const frame = document.createElement("iframe");
    frame.setAttribute("sandbox", "allow-scripts");
    frame.srcdoc = presentationDocument(view);
    document.querySelector("#mount")!.appendChild(frame);
  }, view);
  const image = page.frameLocator("iframe").locator("img");
  await expect(image).toBeVisible();
  expect(await image.getAttribute("src")).toMatch(/^data:image\/svg\+xml/);
  expect(await image.evaluate((node: HTMLImageElement) => node.naturalWidth)).toBeGreaterThan(0);
  expect(await image.evaluate(node => node.getBoundingClientRect().width)).toBeLessThanOrEqual(320);
  await expect(page.frameLocator("iframe").locator("body")).toHaveCSS("visibility", "visible");
});
