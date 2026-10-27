import { describe, expect, it } from "vitest";
import { renderFormulaSource, renderInlineMarkdown, renderMarkdown } from "./md";

describe("backslash-delimited answer math", () => {
  it("renders the failed board reply's state update as display math", () => {
    const html = renderMarkdown(String.raw`状态更新：
\[
S_t = S_{t-1} + k_t v_t^\top, \qquad o_t = q_t^\top S_t.
\]`);
    expect(html).toContain('class="katex-display"');
    expect(html).toContain('encoding="application/x-tex"');
    expect(html).not.toContain("katex-error");
    expect(html).not.toContain("<br>\n[<br>");
  });

  it("renders inline state variables beside existing dollar math", () => {
    const html = renderInlineMarkdown(String.raw`状态 \(S_t\)，查询 $q_t$。`);
    expect(html.match(/class="katex"/g)).toHaveLength(2);
    expect(html).not.toContain("katex-display");
  });

  it("preserves code examples and unmatched delimiters", () => {
    const html = renderMarkdown("`\\(S_t\\)`\n\n```tex\n\\[S_t\\]\n```\n\n未闭合 \\(S_t");
    expect(html).not.toContain('class="katex');
    expect(html).toContain("<code>\\(S_t\\)</code>");
    expect(html).toContain("\\[S_t\\]");
    expect(html).toContain("未闭合 (S_t");
  });
});

describe("renderInlineMarkdown", () => {
  it("renders spaced inline superscript math from paper author lines", () => {
    const html = renderInlineMarkdown("Michael Gotthardt $ ^{1,2,3} $");

    expect(html).toContain('<sup class="inline-citation-sup">1,2,3</sup>');
    expect(html).not.toContain("$ ^{1,2,3} $");
  });

  it("still renders ordinary spaced inline math with KaTeX", () => {
    const html = renderInlineMarkdown("Use $ x^2 $ here.");

    expect(html).toContain("katex");
    expect(html).not.toContain("$ x^2 $");
  });

  it("leaves ordinary spaced dollar text alone", () => {
    const html = renderInlineMarkdown("The fee is $ 10 $ today.");

    expect(html).toContain("$ 10 $");
    expect(html).not.toContain("katex");
  });
});

describe("renderFormulaSource", () => {
  it("renders a source display formula as display math without losing its LaTeX", () => {
    const source = "$$\n\\mathrm{Speedup}=\\frac{1}{(1-f)+f/s}.\n$$";
    const html = renderFormulaSource(source);

    expect(html).toContain("katex-display");
    expect(html).toContain("\\mathrm{Speedup}=\\frac{1}{(1-f)+f/s}.");
    expect(html).not.toContain("$$");
  });

  it("keeps a paragraph formula inline", () => {
    const html = renderFormulaSource("$f$");
    expect(html).toContain("class=\"katex\"");
    expect(html).not.toContain("katex-display");
  });

  it("renders a multiline single-dollar source formula as display math", () => {
    const html = renderFormulaSource("$\nI(X;Y) \\le H(Y)\n$");
    expect(html).toContain("katex-display");
    expect(html).toContain('data-formula-delimiter="single"');
  });
});
