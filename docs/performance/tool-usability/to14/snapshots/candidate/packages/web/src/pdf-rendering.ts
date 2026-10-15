export const PDF_MAX_RESIDENT_PAGES = 5;
export const PDF_MAX_BACKING_PIXELS = 12_000_000;

export interface PdfRenderCandidate {
  pageIndex: number;
  cssWidth: number;
  cssHeight: number;
  distance: number;
  visible?: boolean;
}

export interface PdfRenderPlanPage extends PdfRenderCandidate {
  backingWidth: number;
  backingHeight: number;
  backingPixels: number;
}

export interface PdfRenderPlan {
  pages: PdfRenderPlanPage[];
  visiblePages: PdfRenderPlanPage[];
  prefetchPage?: PdfRenderPlanPage;
  rasterScale: number;
  backingPixels: number;
}

export interface PdfRenderIdentityInput {
  source: string;
  pageIndex: number;
  cssWidth: number;
  cssHeight: number;
  rasterScale: number;
  generation: number;
}

/** Select the nearest useful pages and lower raster density only when the backing-store budget requires it. */
export function planPdfRenderResidency(
  candidates: PdfRenderCandidate[],
  requestedRasterScale: number,
  limits: { maxPages?: number; maxBackingPixels?: number; direction?: 1 | -1 } = {},
): PdfRenderPlan {
  const maxPages = limits.maxPages ?? PDF_MAX_RESIDENT_PAGES;
  const maxBackingPixels = limits.maxBackingPixels ?? PDF_MAX_BACKING_PIXELS;
  const scale = Number.isFinite(requestedRasterScale) && requestedRasterScale > 0
    ? requestedRasterScale
    : 1;
  const selected = candidates
    .filter((page) => page.cssWidth > 0 && page.cssHeight > 0)
    .sort((left, right) => Number(!!right.visible) - Number(!!left.visible)
      || left.distance - right.distance || left.pageIndex - right.pageIndex)
    .slice(0, Math.max(0, maxPages));
  const cssPixels = selected.reduce((total, page) => total + page.cssWidth * page.cssHeight, 0);
  if (!cssPixels || maxBackingPixels <= 0) return { pages: [], visiblePages: [], rasterScale: scale, backingPixels: 0 };
  const rasterScale = Math.min(scale, Math.sqrt(maxBackingPixels / cssPixels));
  const pages = selected.map((page) => {
    const backingWidth = Math.max(1, Math.floor(page.cssWidth * rasterScale));
    const backingHeight = Math.max(1, Math.floor(page.cssHeight * rasterScale));
    return { ...page, backingWidth, backingHeight, backingPixels: backingWidth * backingHeight };
  });
  const visiblePages = pages.filter(page => page.visible);
  // At a gap between pages, prepare the nearest page rather than leaving the viewport blank.
  if (!visiblePages.length && pages.length) visiblePages.push(pages[0]);
  const direction = limits.direction ?? 1;
  const edge = direction > 0
    ? Math.max(...visiblePages.map(page => page.pageIndex))
    : Math.min(...visiblePages.map(page => page.pageIndex));
  const prefetchPage = pages.filter(page => direction * (page.pageIndex - edge) > 0)
    .sort((a, b) => direction * (a.pageIndex - b.pageIndex))[0];
  return {
    pages,
    visiblePages,
    prefetchPage,
    rasterScale,
    backingPixels: pages.reduce((total, page) => total + page.backingPixels, 0),
  };
}

export function pdfRenderIdentity(input: PdfRenderIdentityInput): string {
  return [
    input.source,
    input.pageIndex,
    input.cssWidth.toFixed(3),
    input.cssHeight.toFixed(3),
    input.rasterScale.toFixed(4),
    input.generation,
  ].join(":");
}
