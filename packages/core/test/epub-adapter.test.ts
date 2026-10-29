import { describe, it, expect } from "vitest";
import { zipSync, strToU8 } from "fflate";
import { xhtmlToBlocks, epubToSource } from "../src/epub-adapter";
import { segment } from "../src/segment";
import { checkPartitionInvariant } from "../src/partition";

describe("SA3 epub xhtml asset block recognition", () => {
  const html = `<?xml version="1.0"?><html><body>
    <h1>Command Pattern</h1>
    <p class="body">Commands are reified calls.</p>
    <p>See <img alt="inline" src="figures/inline.png" /> now.</p>
    <p>Inline math <math><mi>F</mi><mo>=</mo><mi>m</mi><mi>a</mi></math> stays clickable.</p>
    <blockquote><p>Nested blockquote paragraph.</p></blockquote>
    <h2>Examples</h2>
    <pre>function demo() {
  return 1;
}</pre>
    <table><tr><th>A</th><th>B</th></tr><tr><td>1</td><td>2</td></tr></table>
    <img alt="diagram" src="images/diagram.png" />
    <math xmlns="http://www.w3.org/1998/Math/MathML"><mi>E</mi><mo>=</mo><mi>m</mi><msup><mi>c</mi><mn>2</mn></msup></math>
  </body></html>`;

  const blocks = xhtmlToBlocks(html);

  it("keeps headings and ordinary leaves while marking pre/table/img/math assets", () => {
    expect(blocks.map((b) => [b.kind, b.level, b.assetKind, b.text])).toEqual([
      ["heading", 1, undefined, "Command Pattern"],
      ["leaf", undefined, undefined, "Commands are reified calls."],
      ["leaf", undefined, undefined, "See"],
      ["leaf", undefined, "image", "![inline](figures/inline.png)"],
      ["leaf", undefined, undefined, "now."],
      ["leaf", undefined, undefined, "Inline math"],
      ["leaf", undefined, "formula", "<math><mi>F</mi><mo>=</mo><mi>m</mi><mi>a</mi></math>"],
      ["leaf", undefined, undefined, "stays clickable."],
      ["leaf", undefined, undefined, "Nested blockquote paragraph."],
      ["heading", 2, undefined, "Examples"],
      ["leaf", undefined, "code", "function demo() {\n  return 1;\n}"],
      ["leaf", undefined, "table", "| A | B |\n| 1 | 2 |"],
      ["leaf", undefined, "image", "![diagram](images/diagram.png)"],
      [
        "leaf",
        undefined,
        "formula",
        '<math xmlns="http://www.w3.org/1998/Math/MathML"><mi>E</mi><mo>=</mo><mi>m</mi><msup><mi>c</mi><mn>2</mn></msup></math>',
      ],
    ]);
  });
});

describe("SA3 epubToSource full chain with assets", () => {
  function makeEpub(): Uint8Array {
    const container = `<?xml version="1.0"?><container><rootfiles><rootfile full-path="OEBPS/content.opf"/></rootfiles></container>`;
    const opf = `<package><manifest>
      <item id="c1" href="Text/ch1.xhtml" media-type="application/xhtml+xml"/>
    </manifest><spine><itemref idref="c1"/></spine></package>`;
    const ch1 = `<html><body>
      <h1>Intro</h1>
      <p>First paragraph.</p>
      <p>Use <math><mi>F</mi><mo>=</mo><mi>m</mi><mi>a</mi></math> here.</p>
      <pre>line 1
  line 2</pre>
      <table><tr><td>x</td><td>y</td></tr></table>
      <img alt="cover" src="cover.png" />
      <math><mi>a</mi><mo>+</mo><mi>b</mi></math>
    </body></html>`;
    return zipSync({
      "META-INF/container.xml": strToU8(container),
      "OEBPS/content.opf": strToU8(opf),
      "OEBPS/Text/ch1.xhtml": strToU8(ch1),
    });
  }

  const { source, blocks } = epubToSource(makeEpub());
  const nodes = segment(blocks);

  it("passes assetKind through segment", () => {
    expect(blocks.map((b) => b.assetKind).filter(Boolean)).toEqual(["formula", "code", "table", "image", "formula"]);
    expect(source).toContain("line 1\n  line 2");
    expect(source).toContain("| x | y |");
    expect(source).toContain("![cover](cover.png)");
    expect(source).toContain("<math><mi>F</mi><mo>=</mo><mi>m</mi><mi>a</mi></math>");
    expect(source).toContain("<math><mi>a</mi><mo>+</mo><mi>b</mi></math>");

    const assetNodes = blocks
      .filter((block) => block.assetKind)
      .map((b) => nodes.find((n) => n.children.length === 0 && n.span.start === b.span.start && n.span.end === b.span.end)?.kind);
    expect(assetNodes).toEqual(["formula", "code", "table", "image", "formula"]);
  });

  it("preserves the partition invariant", () => {
    const report = checkPartitionInvariant(nodes, source);
    expect(report.violations).toEqual([]);
    expect(report.coverage).toBe(1);
    expect(nodes.find((n) => n.lid === "1")?.kind).toBe("chapter");
    expect(nodes.filter((n) => n.children.length === 0)).toHaveLength(9);
  });
});

describe("EPUB images inside inline containers", () => {
  it("preserves text and a nested image in reading order without duplicating text", () => {
    const blocks = xhtmlToBlocks(
      '<p>Before <span>inner <img alt="diagram" src="../Images/diagram.png" /> after</span> end.</p><p>Figure caption.</p>',
      "OEBPS/Text/ch1.xhtml",
    );

    expect(blocks).toEqual([
      { kind: "leaf", text: "Before inner" },
      {
        kind: "leaf",
        assetKind: "image",
        text: "![diagram](../Images/diagram.png)",
        image: { alt: "diagram", src: "../Images/diagram.png", epubPath: "OEBPS/Images/diagram.png" },
      },
      { kind: "leaf", text: "after end." },
      { kind: "leaf", text: "Figure caption." },
    ]);
  });

  it("keeps multiple deeply nested images and their reference paths in order", () => {
    const blocks = xhtmlToBlocks(
      '<p><span><a href="#figure"><img alt="first" src="../Images/figure%201.png?size=2#view" /></a> between <em><img alt="second" src="../Images/second.jpg" /></em></span> tail.</p>',
      "OEBPS/Text/ch1.xhtml",
    );

    expect(blocks.map((block) => [block.assetKind, block.text, block.image?.epubPath])).toEqual([
      ["image", "![first](../Images/figure%201.png?size=2#view)", "OEBPS/Images/figure 1.png"],
      [undefined, "between", undefined],
      ["image", "![second](../Images/second.jpg)", "OEBPS/Images/second.jpg"],
      [undefined, "tail.", undefined],
    ]);
  });

  it("carries a nested image through source spans and segmentation without blank wrapper leaves", () => {
    const zip = zipSync({
      "META-INF/container.xml": strToU8('<container><rootfiles><rootfile full-path="OEBPS/content.opf"/></rootfiles></container>'),
      "OEBPS/content.opf": strToU8('<package><manifest><item id="c1" href="Text/ch1.xhtml"/></manifest><spine><itemref idref="c1"/></spine></package>'),
      "OEBPS/Text/ch1.xhtml": strToU8('<html><body><h1>Chapter</h1><p>Before.</p><p> \n<span> <img alt="图示" src="../Images/figure.png" /> </span> </p><p>Figure caption.</p><p>After.</p></body></html>'),
    });
    const { source, blocks } = epubToSource(zip);
    const nodes = segment(blocks);

    expect(blocks.map((block) => block.text)).toEqual(["Chapter", "Before.", "![图示](../Images/figure.png)", "Figure caption.", "After."]);
    const image = blocks[2];
    expect(image.assetKind).toBe("image");
    expect(image.image).toEqual({ alt: "图示", src: "../Images/figure.png", epubPath: "OEBPS/Images/figure.png" });
    for (const block of blocks) expect(source.slice(block.span.start, block.span.end)).toBe(block.text);
    expect(nodes.find((node) => node.kind === "image")?.span).toEqual(image.span);
    expect(checkPartitionInvariant(nodes, source).violations).toEqual([]);
  });

  it("preserves image-table cells and their labels in reading order while retaining text tables", () => {
    const blocks = xhtmlToBlocks(`<html><body>
      <p>Before.</p>
      <table><caption>Figure comparison.</caption><tbody><tr>
        <td><p>Left label.</p><p><span><img alt="left" src="../Images/left.png" /></span></p><p>Left caption.</p></td>
        <td><p>Right label.</p><p><span><img alt="right" src="../Images/right.png" /></span></p><p>Right caption.</p></td>
      </tr></tbody></table>
      <table><tr><th>Column</th><th>Value</th></tr><tr><td>Text</td><td>2</td></tr></table>
      <p>After.</p>
    </body></html>`, "OEBPS/Text/ch1.xhtml");

    expect(blocks.map((block) => [block.assetKind, block.text])).toEqual([
      [undefined, "Before."],
      [undefined, "Figure comparison."],
      [undefined, "Left label."],
      ["image", "![left](../Images/left.png)"],
      [undefined, "Left caption."],
      [undefined, "Right label."],
      ["image", "![right](../Images/right.png)"],
      [undefined, "Right caption."],
      ["table", "| Column | Value |\n| Text | 2 |"],
      [undefined, "After."],
    ]);
    expect(blocks.filter((block) => block.assetKind === "image").map((block) => block.image?.epubPath)).toEqual([
      "OEBPS/Images/left.png",
      "OEBPS/Images/right.png",
    ]);
  });
});
