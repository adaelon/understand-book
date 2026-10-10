// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { renderMarkdown, renderTableSource } from "./md";
import { createMarkdownDomSourceMap, markMarkdownDomSourceRanges, sourceTextForRanges } from "./markdown-source-map";

// The EPUB importer serializes this published table without a Markdown separator.
const epubTableSource = [
  "| 状态 | 主要归属 | 为什么需要这样区分 |",
  "| 已发布材料与语义成果 | 书籍及其发布版本 | 多个阅读任务能够使用同一份内容，旧引用仍指向原材料 |",
  "| 阅读位置与窗口现场 | 一个读者的阅读现场 | 两个窗口能够各自阅读和导航 |",
  "| 对话与聊天目标 | 对应聊天 | 追问和任务结果需要回到原会话 |",
  "| 笔记与稳定画像 | 读者的私人数据 | 信息可以持续使用，并保留用户归属 |",
  "| 教学会话与学习事实 | 读者的学习状态 | 学习过程可以跨多次交互延续 |",
  "| 当前消息 证据和执行进展 | 一次 Run | 运行中的信息需要与当时的输入和动作保持关联 |",
].join("\n");

function rendered(source: string): HTMLDivElement {
  const root = document.createElement("div");
  root.innerHTML = renderTableSource(source);
  return root;
}

describe("reader table rendering", () => {
  it("renders the imported EPUB table as seven rows with three cells each", () => {
    expect(renderMarkdown(epubTableSource)).not.toContain("<table>");
    const root = rendered(epubTableSource);
    expect(root.querySelectorAll("table")).toHaveLength(1);
    expect(root.querySelectorAll("tr")).toHaveLength(7);
    for (const row of root.querySelectorAll("tr")) expect(row.children).toHaveLength(3);
    expect(root.querySelectorAll("th")).toHaveLength(0);
    expect(root.textContent).toContain("为什么需要这样区分");
  });

  it("preserves every EPUB cell's canonical selection offsets and focus highlight", () => {
    const root = rendered(epubTableSource);
    const map = createMarkdownDomSourceMap(epubTableSource, root);
    let cursor = 0;
    for (const cell of root.querySelectorAll("td")) {
      const text = cell.textContent!;
      const start = epubTableSource.indexOf(text, cursor);
      const selection = document.createRange();
      selection.selectNodeContents(cell);
      const ranges = map.sourceRangesForRange(selection);
      expect(ranges).toEqual([{ start, end: start + text.length }]);
      expect(sourceTextForRanges(epubTableSource, ranges)).toBe(text);
      cursor = start + text.length;
    }
    const quote = "读者的私人数据";
    const start = epubTableSource.indexOf(quote);
    markMarkdownDomSourceRanges(epubTableSource, root, [{ start, end: start + quote.length, className: "source-focus-mark" }]);
    expect(root.querySelector("td mark")?.textContent).toBe(quote);
  });

  it("preserves Markdown headers, alignment, escaped pipes, emphasis, links and math", () => {
    const source = "| Label | Value |\n| :--- | ---: |\n| **A** \\| B | [source](https://example.com) $x^2$ |";
    const root = rendered(source);
    expect(root.querySelectorAll("th")).toHaveLength(2);
    expect(root.querySelector("th")?.getAttribute("style")).toContain("text-align:left");
    expect(root.querySelector("td strong")?.textContent).toBe("A");
    expect(root.querySelector("td")?.textContent).toBe("A | B");
    expect(root.querySelector("a")?.getAttribute("href")).toBe("https://example.com");
    expect(root.querySelector(".katex")).not.toBeNull();
  });

  it("renders supported HTML table structure, spans, inline markup and formula text", () => {
    const root = rendered('<table><thead><tr><th colspan="2">Results</th></tr></thead><tbody><tr><td rowspan="2"><strong>A</strong></td><td>$ x^2 $</td></tr><tr><td><a href="https://example.com">source</a><br>next</td></tr></tbody></table>');
    expect(root.querySelector("th")?.getAttribute("colspan")).toBe("2");
    expect(root.querySelector("td")?.getAttribute("rowspan")).toBe("2");
    expect(root.querySelector("strong")?.textContent).toBe("A");
    expect(root.querySelector(".katex")).not.toBeNull();
    expect(root.querySelector("a")?.getAttribute("href")).toBe("https://example.com");
    expect(root.querySelector("br")).not.toBeNull();
  });

  it.each([
    "| Label | Value |\n| --- | --- |\n| same | **same** |",
    "<table><tr><th>Label</th><th>Value</th></tr><tr><td>same</td><td><strong>same</strong></td></tr></table>",
    '<table><tr><th>Label</th><th>Value</th></tr><tr><td><script>same</script>same</td><td><strong title="same > same">same</strong></td></tr></table>',
  ])("maps a repeated cell value to its selected occurrence in the original source", (source) => {
    const root = rendered(source);
    const range = document.createRange();
    range.selectNodeContents(root.querySelectorAll("td")[1]);
    const map = createMarkdownDomSourceMap(source, root);
    const start = source.lastIndexOf("same");
    expect(map.sourceRangesForRange(range)).toEqual([{ start, end: start + 4 }]);
    markMarkdownDomSourceRanges(source, root, [{ start, end: start + 4, className: "hl-mark" }]);
    expect(root.querySelector("td strong mark")?.textContent).toBe("same");
    expect(root.querySelector("td")?.querySelector("mark")).toBeNull();
  });

  it("keeps source HTML inert while constructing only table presentation markup", () => {
    const source = '<table onclick="alert(1)"><tr><td><img src="x" onerror="alert(1)"><a href="javascript:alert(1)">value</a><script>alert(1)</script></td></tr></table>';
    const root = rendered(source);
    expect(root.querySelectorAll("table")).toHaveLength(1);
    expect(root.querySelector("[onclick], [onerror], img, script, a")).toBeNull();
    expect(root.textContent).toBe("value");
    expect(renderMarkdown(source)).not.toContain("<table>");
  });

  it("keeps a headerless EPUB row and literal cell text intact", () => {
    const root = rendered("| *literal* | <tag> & text |\n| second |  |");
    expect(root.querySelectorAll("th")).toHaveLength(0);
    expect(root.querySelectorAll("td")).toHaveLength(4);
    expect(root.querySelector("td")?.textContent).toBe("*literal*");
    expect(root.querySelectorAll("td")[1].textContent).toBe("<tag> & text");
    expect(root.querySelector("em, tag")).toBeNull();
  });

  it("shows unrecognized table text rather than losing it", () => {
    const root = rendered("table text without row delimiters <unknown>");
    expect(root.querySelector("pre")?.textContent).toBe("table text without row delimiters <unknown>");
  });
});
