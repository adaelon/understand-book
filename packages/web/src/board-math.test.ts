import { describe, expect, it } from "vitest";
import { parseBoardMath } from "./board-math";

const segments = (source: string) => parseBoardMath(source).lines.flatMap((line) => line.segments);
const math = (source: string) => segments(source).filter((segment) => segment.kind === "math");

describe("board math text", () => {
  it("renders both inline delimiters with original offsets for atomic reveals", () => {
    const source = String.raw`状态 $S_t$，查询 \(q_t^\top\)。`;
    const result = parseBoardMath(source);
    expect(result.hasMath).toBe(true);
    expect(result.lines).toHaveLength(1);
    expect(math(source).map((segment) => segment.tex)).toEqual(["S_t", String.raw`q_t^\top`]);
    for (const segment of segments(source)) {
      expect(source.slice(segment.start, segment.end)).toBe(segment.source);
      if (segment.kind === "math") expect(segment.html).toContain('class="katex"');
    }
  });

  it.each([
    "$$S_t = S_{t-1} + k_t v_t^T$$",
    "$$\nS_t = S_{t-1} + k_t v_t^T\n$$",
    "\\[\nS_t = S_{t-1} + k_t v_t^T\n\\]",
    "$\nS_t = S_{t-1} + k_t v_t^T\n$",
  ])("keeps a display formula intact across source line breaks: %s", (formula) => {
    const result = parseBoardMath(`状态更新\n${formula}\n继续查询`);
    expect(result.lines).toHaveLength(3);
    const segment = result.lines[1].segments[0];
    expect(segment.source).toBe(formula);
    expect(segment.kind).toBe("math");
    expect(segment.displayMode).toBe(true);
    expect(segment.html).toContain('class="katex-display"');
    expect(result.lines[2].segments[0].source).toBe("继续查询");
  });

  it("places display formulas between prose on distinct visual lines", () => {
    const result = parseBoardMath("先计算 $$x^2$$ 再查询");
    expect(result.lines).toHaveLength(3);
    expect(result.lines[1].segments[0].kind).toBe("math");
  });

  it("renders archived state equations and dimension products without changing saved source", () => {
    const source = "S_t = S_(t-1) + k_t v_t^T\no_t = q_t^T S_t\n矩阵为 d_k × d_v，每步更新。";
    const rendered = math(source);
    expect(rendered.map((segment) => segment.tex)).toEqual([
      "S_t = S_{t-1} + k_t v_t^{\\top}",
      "o_t = q_t^{\\top} S_t",
      "d_k \\times  d_v",
    ]);
    expect(rendered).toHaveLength(3);
    for (const segment of rendered) {
      expect(source.slice(segment.start, segment.end)).toBe(segment.source);
      expect(segment.html).toContain("<msub>");
      expect(segment.html).not.toContain("katex-error");
    }
  });

  it("does not absorb adjacent English words into a legacy formula", () => {
    const source = "Update S_t = S_(t-1) + k_t v_t^T then query o_t = q_t^T S_t now.";
    expect(math(source).map((segment) => segment.source)).toEqual([
      "S_t = S_(t-1) + k_t v_t^T", "o_t = q_t^T S_t",
    ]);
  });

  it.each([
    "item_id 和 foo_bar 是字段名",
    "item_id = other_id",
    "C:/cache/a_b × c_d/report.txt",
    "路径 /a_b/c_d 和 a_b.txt",
    String.raw`C:\foo_bar\report.txt`,
    String.raw`代码 \$x^2 与费用 $ 10 $`,
    "`$x^2$` 和 `S_t = S_(t-1)`",
  ])("keeps ordinary identifiers, paths and code literal: %s", (source) => {
    expect(parseBoardMath(source).hasMath).toBe(false);
    expect(segments(source).map((segment) => segment.source).join("")).toBe(source);
  });

  it.each([String.raw`错误 $\frac{1}{$`, String.raw`未闭合 \(S_t = x_t`, "未闭合 $$x_t"])("leaves invalid math readable: %s", (source) => {
    const result = parseBoardMath(source);
    expect(result.hasMath).toBe(false);
    expect(segments(source).map((segment) => segment.source).join("")).toBe(source);
    expect(segments(source).map((segment) => segment.html).join("")).not.toContain("katex-error");
  });

  it("escapes raw HTML and disallows trusted KaTeX extensions", () => {
    const source = String.raw`<img src=x onerror=alert(1)> $x^2$ \(\href{javascript:alert(1)}{click}\)`;
    const html = segments(source).map((segment) => segment.html).join("");
    expect(html).toContain("&lt;img");
    expect(html).not.toContain("<img");
    expect(html).not.toContain("href=");
    expect(html).not.toContain("katex-error");
  });

  it("preserves ordinary empty lines and source offsets with CRLF", () => {
    const source = "标题\r\n\r\n状态 $S_t$";
    const result = parseBoardMath(source);
    expect(result.lines).toHaveLength(3);
    expect(result.lines[1].segments).toEqual([]);
    for (const segment of segments(source)) expect(source.slice(segment.start, segment.end)).toBe(segment.source);
    expect(parseBoardMath("")).toEqual({ lines: [{ segments: [] }], hasMath: false });
  });
});
