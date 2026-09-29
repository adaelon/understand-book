import { describe, expect, it } from "vitest";
import { renderFormulaSource, renderInlineMarkdown } from "./md";

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
