import { describe, it, expect } from "vitest";
import { readFileSync } from "node:fs";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { zipSync, strToU8 } from "fflate";
import { epubToSource } from "../src/epub-adapter";
import { markdownToBlocks } from "../src/md-adapter";
import { segment, type SourceBlock } from "../src/segment";
import type { LidNode } from "../src/generated/LidNode";
import { splitWindows, estimateTokens, type WindowBudget, type Window } from "../src/window";
import { routeModelInputSlices } from "../src/model-input-slice";

const here = dirname(fileURLToPath(import.meta.url));
const sampleSrc = readFileSync(resolve(here, "fixtures/sample.md"), "utf8");

/** 窗口层分区不变式:全部叶子被窗口恰好覆盖一次(无遗漏 + 无重叠)。 */
function assertLeafPartition(nodes: LidNode[], windows: Window[]) {
  const allLeaves = nodes.filter((n) => n.children.length === 0).map((n) => n.lid);
  const seen = new Set<string>();
  for (const w of windows) {
    for (const l of w.leafLids) {
      expect(seen.has(l)).toBe(false); // 无重叠
      seen.add(l);
    }
  }
  expect(seen.size).toBe(allLeaves.length); // 全覆盖(计数)
  for (const l of allLeaves) expect(seen.has(l)).toBe(true); // 全覆盖(逐一)
}

function unsectionedEpub(body: string) {
  return epubToSource(zipSync({
    "META-INF/container.xml": strToU8('<container><rootfiles><rootfile full-path="content.opf"/></rootfiles></container>'),
    "content.opf": strToU8('<package><manifest><item id="text" href="text.xhtml" media-type="application/xhtml+xml"/></manifest><spine><itemref idref="text"/></spine></package>'),
    "text.xhtml": strToU8(`<html><body>${body}</body></html>`),
  }));
}

/** 既有裸根叶索引仍是 splitWindows 的合法输入,独立于当前 segment 的匿名容器布局。 */
function rootLeafNodes(blocks: SourceBlock[]): LidNode[] {
  return blocks.map((block, index) => ({
    lid: String(index + 1), path: [index + 1], kind: block.assetKind ?? "paragraph",
    span: { ...block.span }, children: [],
  }));
}

describe("estimateTokens(确定性近似)", () => {
  it("CJK 按 1 / ASCII 按 0.25,向上取整", () => {
    expect(estimateTokens("你好")).toBe(2);
    expect(estimateTokens("abcd")).toBe(1); // ceil(4*0.25)
    expect(estimateTokens("中a")).toBe(2); // ceil(1 + 0.25)
    expect(estimateTokens("")).toBe(0);
    expect(estimateTokens("🙂🚀🧭🙂")).toBe(1);
    expect(estimateTokens("界🙂abc")).toBe(2);
    expect(estimateTokens("㿿一鿿ꀀ")).toBe(3);
  });
});

describe("S2 窗口切分 [ADR-0009]", () => {
  const blocks = markdownToBlocks(sampleSrc);
  const nodes = segment(blocks);
  const BIG: WindowBudget = { maxInputTokens: 1_000_000, maxLeavesSoft: 10_000 };

  it("无标题 EPUB 的短段、正文和图片按预算合窗,不把每段当作一卷", () => {
    const { source, blocks } = unsectionedEpub('<p>版权</p><p>作者</p><p>判断复杂系统时需要结合上下文。</p><img src="figure.png"/><p>图一</p>');
    const n = segment(blocks);
    const leaves = n.filter((node) => node.children.length === 0);
    expect(n.map((node) => node.kind)).toEqual(["section", "paragraph", "paragraph", "paragraph", "image", "paragraph"]);
    expect(leaves.map((node) => node.lid)).toEqual(["1.1", "1.2", "1.3", "1.4", "1.5"]);
    const originalNodes = structuredClone(n);
    const windows = splitWindows(n, source, BIG);
    expect(windows).toHaveLength(1);
    expect(windows[0].leafLids).toEqual(leaves.map((node) => node.lid));
    expect(windows[0].spans).toEqual(leaves.map((node) => node.span));
    expect(n).toEqual(originalNodes);
    assertLeafPartition(n, windows);
  });

  it("无容器根叶仍遵守输入硬闸,只在完整叶子之间切分", () => {
    const { source, blocks } = unsectionedEpub("<p>甲乙</p><p>丙丁</p><p>戊己</p><p>庚辛</p><p>壬癸</p>");
    const n = rootLeafNodes(blocks);
    const windows = splitWindows(n, source, { maxInputTokens: 4, maxLeavesSoft: 100 });
    expect(windows.map((window) => window.leafLids)).toEqual([["1", "2"], ["3", "4"], ["5"]]);
    expect(windows.map((window) => window.tokens)).toEqual([4, 4, 2]);
    expect(windows.every((window) => !window.overBudget)).toBe(true);
    assertLeafPartition(n, windows);
  });

  it("无容器根叶仍遵守叶子数软闸", () => {
    const { source, blocks } = unsectionedEpub("<p>甲</p><p>乙</p><p>丙</p><p>丁</p><p>戊</p>");
    const n = rootLeafNodes(blocks);
    const windows = splitWindows(n, source, { maxInputTokens: 100, maxLeavesSoft: 2 });
    expect(windows.map((window) => window.leafLids)).toEqual([["1", "2"], ["3", "4"], ["5"]]);
    assertLeafPartition(n, windows);
  });

  it("超预算根叶独立成窗并标记,不遗漏该叶或吞并前后短段", () => {
    const { source, blocks } = unsectionedEpub(`<p>甲</p><p>乙</p><p>${"长".repeat(20)}</p><p>丙</p><p>丁</p>`);
    const n = rootLeafNodes(blocks);
    const windows = splitWindows(n, source, { maxInputTokens: 4, maxLeavesSoft: 100 });
    expect(windows.map((window) => window.leafLids)).toEqual([["1", "2"], ["3"], ["4", "5"]]);
    expect(windows.map((window) => [window.tokens, window.overBudget])).toEqual([[2, false], [20, true], [2, false]]);
    assertLeafPartition(n, windows);
  });

  it("容器前、中、后的游离根叶各自合窗,保留章节和有子节点容器的边界", () => {
    const source = "甲乙丙丁戊己庚辛壬癸子丑";
    const node = (lid: string, kind: LidNode["kind"], start: number, end: number, children: string[] = []): LidNode => ({
      lid, path: lid.split(".").map(Number), kind, span: { start, end }, children,
    });
    const n = [
      node("1", "paragraph", 0, 1), node("2", "image", 1, 2),
      node("3", "chapter", 2, 4, ["3.1", "3.2"]),
      node("3.1", "paragraph", 2, 3), node("3.2", "paragraph", 3, 4),
      node("4", "paragraph", 4, 5), node("5", "paragraph", 5, 6),
      node("6", "chapter", 6, 7), node("7", "section", 7, 8),
      node("8", "paragraph", 8, 10, ["8.1", "8.2"]),
      node("8.1", "paragraph", 8, 9), node("8.2", "paragraph", 9, 10),
      node("9", "paragraph", 10, 11), node("10", "paragraph", 11, 12),
    ];
    const windows = splitWindows(n, source, BIG);
    expect(windows.map((window) => window.leafLids)).toEqual([
      ["1", "2"], ["3.1", "3.2"], ["4", "5"], ["6"], ["7"], ["8.1", "8.2"], ["9", "10"],
    ]);
    assertLeafPartition(n, windows);
  });

  it("大预算下叶子被窗口划分(无遗漏无重叠)", () => {
    const windows = splitWindows(nodes, sampleSrc, BIG);
    assertLeafPartition(nodes, windows);
  });

  it("窗口 token = 其覆盖叶子的估算和", () => {
    const byLid = new Map(nodes.map((n) => [n.lid, n]));
    const windows = splitWindows(nodes, sampleSrc, BIG);
    for (const w of windows) {
      const expected = w.leafLids.reduce(
        (s, l) => s + estimateTokens(sampleSrc.slice(byLid.get(l)!.span.start, byLid.get(l)!.span.end)),
        0,
      );
      expect(w.tokens).toBe(expected);
    }
  });

  it("超大子树按叶子细分:每窗 token ≤ 硬闸 + 仍是划分", () => {
    // 单章 + 很多段,设小硬闸逼细分
    const md = "# 章一\n\n" + Array.from({ length: 20 }, (_, i) => `这是第${i}段的正文内容填充。`).join("\n\n");
    const n = segment(markdownToBlocks(md));
    const budget: WindowBudget = { maxInputTokens: 20, maxLeavesSoft: 1000 };
    const windows = splitWindows(n, md, budget);
    expect(windows.length).toBeGreaterThan(1); // 确实被细分
    for (const w of windows) {
      if (!w.overBudget) expect(w.tokens).toBeLessThanOrEqual(budget.maxInputTokens);
    }
    assertLeafPartition(n, windows);
  });

  it("过小单元贪心合并:窗口数 < 叶子数(且每窗 ≤ 硬闸)", () => {
    const md = "# 章一\n\n" + Array.from({ length: 12 }, (_, i) => `短句${i}。`).join("\n\n");
    const n = segment(markdownToBlocks(md));
    const leaves = n.filter((x) => x.children.length === 0).length;
    const budget: WindowBudget = { maxInputTokens: 6, maxLeavesSoft: 1000 };
    const windows = splitWindows(n, md, budget);
    expect(windows.length).toBeLessThan(leaves); // 小单元被合并
    for (const w of windows) expect(w.tokens).toBeLessThanOrEqual(budget.maxInputTokens);
    assertLeafPartition(n, windows);
  });

  it("软闸:单窗口叶子数 ≤ maxLeavesSoft", () => {
    const md = "# 章一\n\n" + Array.from({ length: 20 }, (_, i) => `第${i}段。`).join("\n\n");
    const n = segment(markdownToBlocks(md));
    const budget: WindowBudget = { maxInputTokens: 1_000_000, maxLeavesSoft: 3 };
    const windows = splitWindows(n, md, budget);
    for (const w of windows) {
      if (!w.overBudget) expect(w.leafLids.length).toBeLessThanOrEqual(3);
    }
    assertLeafPartition(n, windows);
  });

  it("不跨卷:无窗口同时覆盖两个顶层(卷)的叶子", () => {
    const md = "# 卷一\n\n卷一正文。\n\n# 卷二\n\n卷二正文。";
    const n = segment(markdownToBlocks(md));
    const byLid = new Map(n.map((x) => [x.lid, x]));
    const BIG2: WindowBudget = { maxInputTokens: 1_000_000, maxLeavesSoft: 10_000 };
    const windows = splitWindows(n, md, BIG2);
    for (const w of windows) {
      const tops = new Set(w.leafLids.map((l) => byLid.get(l)!.path[0]));
      expect(tops.size).toBe(1); // 每窗口只属一个顶层
    }
    assertLeafPartition(n, windows);
  });

  it("单叶超硬闸:不腰斩,自成一窗并标 overBudget", () => {
    const md = "# 章一\n\n" + "这是一个超长的段落".repeat(20); // 单段远超硬闸
    const n = segment(markdownToBlocks(md));
    const budget: WindowBudget = { maxInputTokens: 5, maxLeavesSoft: 1000 };
    const windows = splitWindows(n, md, budget);
    const over = windows.filter((w) => w.overBudget);
    expect(over.length).toBeGreaterThan(0);
    for (const w of over) expect(w.leafLids.length).toBe(1); // 单叶,未腰斩
    assertLeafPartition(n, windows);
  });

  it("Window.overBudget 只作结构诊断,不能替代模型输入片的预算证明", () => {
    const md = "# 章一\n\n" + "这是一个超长的段落".repeat(20);
    const n = segment(markdownToBlocks(md));
    const byLid = new Map(n.map((node) => [node.lid, node]));
    const [window] = splitWindows(n, md, { maxInputTokens: 5, maxLeavesSoft: 1 }).filter((item) => item.overBudget);
    const leaf = byLid.get(window.leafLids[0])!;
    const routed = routeModelInputSlices({
      source: md,
      source_fingerprint: "window-over-budget-v1",
      parent: leaf,
      budget: {
        router_version: "window_execution_test.v1",
        prompt_sha256: "0".repeat(64),
        stage_body_limit_tokens: 5,
        executor_context_floor_tokens: 100,
        prompt_reserve_tokens: 0,
        protocol_reserve_tokens: 0,
        output_reserve_tokens: 0,
        safety_margin_tokens: 0,
      },
      render: ({ core }) => core,
    });
    expect(routed.status).toBe("routed");
    if (routed.status === "routed") {
      expect(routed.slices.length).toBeGreaterThan(1);
      expect(routed.slices.every((item) => item.proof.status === "within_limit")).toBe(true);
    }
  });
});
