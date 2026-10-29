import { strToU8, zipSync } from "fflate";
import { describe, expect, it } from "vitest";
import { buildBookStructureUnitSources } from "../src/book-structure";
import { structureDiscoverySections, structureDiscoverySources, structureSourceOutline } from "../src/book-structure-discovery";
import { epubToSource } from "../src/epub-adapter";
import { markdownToBlocks } from "../src/md-adapter";
import { checkPartitionInvariant } from "../src/partition";
import { segment } from "../src/segment";

function epubWithBody(body: string) {
  return zipSync({
    "META-INF/container.xml": strToU8('<container><rootfiles><rootfile full-path="OEBPS/content.opf"/></rootfiles></container>'),
    "OEBPS/content.opf": strToU8('<package><manifest><item id="body" href="body.xhtml"/></manifest><spine><itemref idref="body"/></spine></package>'),
    "OEBPS/body.xhtml": strToU8(`<html><body>${body}</body></html>`),
  });
}

describe("unsectioned source coverage through BookStructure discovery", () => {
  it("keeps every heading-free EPUB leaf in one navigable unit without making its cover image a title", () => {
    const { source, blocks } = epubToSource(epubWithBody('<img src="cover.jpg"/><p>First argument.</p><p>Second argument.</p><p>Last conclusion.</p>'));
    expect(source).toBe("![](cover.jpg)\n\nFirst argument.\n\nSecond argument.\n\nLast conclusion.\n\n");
    const nodes = segment(blocks);
    const leaves = nodes.filter(node => node.children.length === 0);
    const sources = buildBookStructureUnitSources({ lidNodes: nodes, source });
    expect(sources).toHaveLength(1);
    expect(sources[0]).toMatchObject({ unit_lid: "1", unit_kind: "section", title: "未命名单元" });
    expect(sources.flatMap(unit => unit.leaf_lids)).toEqual(leaves.map(node => node.lid));
    expect(sources[0].leaf_lids.every(lid => lid.startsWith(`${sources[0].unit_lid}.`))).toBe(true);
    expect(sources[0].excerpts.map(excerpt => excerpt.text)).toEqual(blocks.map(block => block.text));
    expect(structureDiscoverySources(sources, nodes, source)[0].excerpts).toEqual(sources[0].excerpts);
    expect(Object.values(structureDiscoverySections(sources[0], nodes, source))).toEqual(
      leaves.map(() => ({ lid: "1", title: "未命名单元" })),
    );
    expect(structureSourceOutline(sources, nodes, source).chapters[0].title).toBe("未命名单元");
    expect(checkPartitionInvariant(nodes, source)).toMatchObject({ ok: true, coverage: 1, violations: [] });
  });

  it("covers all heading-free Markdown body leaves exactly once", () => {
    const source = "First argument.\n\nSecond argument.\n\nLast conclusion.";
    const nodes = segment(markdownToBlocks(source));
    const sources = buildBookStructureUnitSources({ lidNodes: nodes, source });
    const leaves = nodes.filter(node => node.children.length === 0);
    expect(sources).toHaveLength(1);
    expect(sources.flatMap(unit => unit.leaf_lids)).toEqual(leaves.map(node => node.lid));
    expect(new Set(sources.flatMap(unit => unit.leaf_lids)).size).toBe(leaves.length);
  });

  it("keeps an unheaded preface outside real EPUB headings while covering both ranges", () => {
    const { source, blocks } = epubToSource(epubWithBody('<p>Preface.</p><h2>Methods</h2><p>Method body.</p><h3>Detail</h3><p>Detail body.</p>'));
    const nodes = segment(blocks);
    const sources = buildBookStructureUnitSources({ lidNodes: nodes, source });
    expect(sources.map(unit => unit.unit_lid)).toEqual(["1", "2"]);
    expect(sources[1].title).toBe("Methods");
    expect(sources.flatMap(unit => unit.leaf_lids)).toEqual(nodes.filter(node => node.children.length === 0).map(node => node.lid));
    expect(sources[0].excerpts.map(excerpt => excerpt.text)).toEqual(["Preface."]);
    expect(sources[1].excerpts.map(excerpt => excerpt.text)).toEqual(["Methods", "Method body.", "Detail", "Detail body."]);
  });
});
