import { describe, it, expect } from "vitest";
import { readFileSync } from "node:fs";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { markdownToBlocks } from "../src/md-adapter";
import { segment } from "../src/segment";
import { checkPartitionInvariant } from "../src/partition";
import { LidNodeZ } from "../src/zod";

const here = dirname(fileURLToPath(import.meta.url));
const src = readFileSync(resolve(here, "fixtures/sample.md"), "utf8");

describe("S1 段级 LID 切分(Model A)", () => {
  const blocks = markdownToBlocks(src);
  const nodes = segment(blocks);
  const byLid = new Map(nodes.map((n) => [n.lid, n]));

  it("分区不变式通过 + 切段覆盖率 100%", () => {
    const report = checkPartitionInvariant(nodes, src);
    expect(report.violations).toEqual([]);
    expect(report.ok).toBe(true);
    expect(report.coverage).toBe(1);
  });

  it("Model A 结构:容器 + 标题首叶", () => {
    // 章容器 "1" 有 5 子(标题段 + 2 正文段 + 2 节容器)
    expect(byLid.get("1")?.kind).toBe("chapter");
    expect(byLid.get("1")?.children).toEqual(["1.1", "1.2", "1.3", "1.4", "1.5"]);
    // 标题 = 容器首叶,kind paragraph(LidNode 不存 text,文本走 span 回切原文)
    expect(byLid.get("1.1")?.kind).toBe("paragraph");
    // 节容器
    expect(byLid.get("1.4")?.kind).toBe("section");
    expect(byLid.get("1.4")?.children).toEqual(["1.4.1", "1.4.2"]);
    expect(byLid.get("1.5")?.children).toEqual(["1.5.1", "1.5.2"]);
  });

  it("叶子标题段 span 回切原文含标题文字", () => {
    const titleLeaf = byLid.get("1.1")!;
    expect(src.slice(titleLeaf.span.start, titleLeaf.span.end)).toBe("# 第1章 引言");
    const para = byLid.get("1.2")!;
    expect(src.slice(para.span.start, para.span.end)).toBe("这是引言的第一段。");
  });

  it("叶子数 = 7,容器数 = 3,总节点 = 10", () => {
    const leaves = nodes.filter((n) => n.children.length === 0);
    expect(leaves).toHaveLength(7);
    expect(nodes.length - leaves.length).toBe(3);
    expect(nodes).toHaveLength(10);
  });

  it("每个节点符合 LidNode schema(zod 镜像 Rust 权威)", () => {
    for (const n of nodes) LidNodeZ.parse(n);
  });
});

describe("unsectioned source ranges", () => {
  it("groups heading-free leaves in one anonymous section without adding source leaves", () => {
    const source = "First paragraph.\n\nSecond paragraph.\n\n![Diagram](diagram.png)\n\nLast paragraph.";
    const blocks = markdownToBlocks(source);
    const before = structuredClone(blocks);
    const nodes = segment(blocks);
    const leaves = nodes.filter(node => node.children.length === 0);
    const containers = nodes.filter(node => node.children.length > 0);

    expect(containers).toHaveLength(1);
    expect(containers[0]).toMatchObject({
      lid: "1", kind: "section", children: ["1.1", "1.2", "1.3", "1.4"],
      span: { start: blocks[0].span.start, end: blocks.at(-1)!.span.end },
    });
    expect(leaves.map(node => node.span)).toEqual(blocks.map(block => block.span));
    expect(leaves.map(node => source.slice(node.span.start, node.span.end))).toEqual(blocks.map(block => block.text));
    expect(leaves.map(node => node.kind)).toEqual(["paragraph", "paragraph", "image", "paragraph"]);
    expect(blocks).toEqual(before);
    expect(checkPartitionInvariant(nodes, source)).toMatchObject({ ok: true, coverage: 1, violations: [] });
    for (const node of nodes) LidNodeZ.parse(node);
  });

  it("ends an anonymous preface before a real heading, including a section-level first heading", () => {
    const source = "Preface one.\n\nPreface two.\n\n## Methods\n\nMethod body.\n\n### Detail\n\nDetail body.\n\n## Results\n\nResult body.";
    const blocks = markdownToBlocks(source);
    const nodes = segment(blocks);
    const byLid = new Map(nodes.map(node => [node.lid, node]));
    expect(nodes.filter(node => node.path.length === 1).map(node => node.lid)).toEqual(["1", "2", "3"]);
    expect(byLid.get("1")).toMatchObject({ kind: "section", children: ["1.1", "1.2"], span: { start: 0, end: blocks[1].span.end } });
    expect(byLid.get("2")).toMatchObject({ kind: "section", children: ["2.1", "2.2", "2.3"] });
    expect(byLid.get("2.3")).toMatchObject({ kind: "section", children: ["2.3.1", "2.3.2"] });
    expect(byLid.get("3")).toMatchObject({ kind: "section", children: ["3.1", "3.2"] });
    expect(nodes.filter(node => node.children.length === 0).map(node => node.span)).toEqual(blocks.map(block => block.span));
    expect(checkPartitionInvariant(nodes, source)).toMatchObject({ ok: true, coverage: 1, violations: [] });
  });

  it("does not create a container for empty input", () => {
    expect(segment([])).toEqual([]);
  });
});
