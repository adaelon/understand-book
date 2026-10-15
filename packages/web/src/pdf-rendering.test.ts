import { describe, expect, it } from "vitest";
import {
  PDF_MAX_BACKING_PIXELS,
  PDF_MAX_RESIDENT_PAGES,
  pdfRenderIdentity,
  planPdfRenderResidency,
  pdfFocusGeometry, pdfFocusRasterSize,
  pdfFocusWindow,
  pdfSpreadStart,
} from "./pdf-rendering";

describe("PDF render residency", () => {
  it('prepares complete neighbouring spreads and preserves the odd last page', () => {
    expect(pdfFocusWindow(5, 9, 2)).toEqual([4, 6, 2]);
    expect(pdfFocusWindow(8, 9, 2)).toEqual([8, 6]);
    expect(pdfFocusWindow(2, 9, 1, -1)).toEqual([2, 1, 3]);
    expect(pdfSpreadStart(5, 2)).toBe(4);
  });
  it('fits the original pages and bounds all six backing stores independent of display zoom', () => {
    for (const width of [1440, 1024, 390, 320]) for (const zoom of [1, 2.5]) for (const dpr of [1, 2, 3]) {
      const sizes = [{ width: 612, height: 792 }, { width: 792, height: 612 }];
      const g = pdfFocusGeometry(width, 844, zoom);
      expect(g.columns).toBe(width >= 1024 ? 2 : 1);
      for (const p of sizes) {
        const scale = Math.min(g.pageWidth / p.width, g.pageHeight / p.height) * zoom;
        const area = pdfFocusRasterSize(p, dpr).width * pdfFocusRasterSize(p, dpr).height;
        expect(area * g.columns * 3).toBeLessThanOrEqual(PDF_MAX_BACKING_PIXELS);
        expect(scale / zoom * p.height).toBeLessThanOrEqual(g.pageHeight + .001);
      }
    }
    expect(pdfFocusGeometry(1440, 900, 1, true).columns).toBe(1);
  });
  it('renders visible pages and one neighbour while retaining the five-page cache budget', () => {
    const candidates = Array.from({ length: 12 }, (_, pageIndex) => ({
      pageIndex, cssWidth: 600, cssHeight: 800, distance: Math.abs(pageIndex - 5),
      visible: pageIndex === 5 || pageIndex === 6,
    }));
    const forward = planPdfRenderResidency(candidates, 1, { direction: 1 });
    expect(forward.pages).toHaveLength(5);
    expect(forward.visiblePages.map(page => page.pageIndex)).toEqual([5, 6]);
    expect(forward.prefetchPage?.pageIndex).toBe(7);
    const backward = planPdfRenderResidency(candidates, 1, { direction: -1 });
    expect(backward.prefetchPage?.pageIndex).toBe(4);
  });
  it("keeps only the five nearest pages and caps their combined backing pixels", () => {
    const plan = planPdfRenderResidency(
      Array.from({ length: 30 }, (_, pageIndex) => ({
        pageIndex,
        cssWidth: 1_000,
        cssHeight: 1_400,
        distance: Math.abs(pageIndex - 16),
      })),
      2,
    );

    expect(plan.pages.map((page) => page.pageIndex)).toEqual([16, 15, 17, 14, 18]);
    expect(plan.pages).toHaveLength(PDF_MAX_RESIDENT_PAGES);
    expect(plan.rasterScale).toBeLessThan(2);
    expect(plan.backingPixels).toBeLessThanOrEqual(PDF_MAX_BACKING_PIXELS);
  });

  it("ignores zero-sized candidates instead of inventing a one-pixel render", () => {
    const plan = planPdfRenderResidency([
      { pageIndex: 0, cssWidth: 0, cssHeight: 800, distance: 0 },
      { pageIndex: 1, cssWidth: 600, cssHeight: 0, distance: 1 },
    ], 2);

    expect(plan.pages).toEqual([]);
    expect(plan.backingPixels).toBe(0);
  });

  it("keys a render by source, page geometry, DPR and generation", () => {
    const base = {
      source: "paper-a:cfg-a:/paper.pdf",
      pageIndex: 3,
      cssWidth: 600,
      cssHeight: 800,
      rasterScale: 2,
      generation: 7,
    };

    expect(pdfRenderIdentity(base)).not.toBe(pdfRenderIdentity({ ...base, cssWidth: 601 }));
    expect(pdfRenderIdentity(base)).not.toBe(pdfRenderIdentity({ ...base, rasterScale: 1.5 }));
    expect(pdfRenderIdentity(base)).not.toBe(pdfRenderIdentity({ ...base, generation: 8 }));
    expect(pdfRenderIdentity(base)).not.toBe(pdfRenderIdentity({ ...base, source: "paper-b:cfg-b:/paper.pdf" }));
  });
});
