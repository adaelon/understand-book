/** FP1 engineering limits. Evidence: docs/performance/focus-reading-fp1-20261014.
 * FP2/FP3 enforce these on resident + preparing resources together.
 * String bytes mean UTF-16 payload bytes; they are not JS heap measurements.
 */
export const FOCUS_READING_BUDGET = Object.freeze({
  minPageWidthEm: 20,
  pageGapPx: 32,
  maxResidentScreens: 4,
  maxPreparationTasks: 1,
  maxSourceBytes: 512 * 1024,
  maxParsedBytes: 2 * 1024 * 1024,
  maxDomNodes: 12_000,
  maxDecodedImagePixels: 8_000_000,
  maxPdfBackingPixels: 12_000_000,
});

export function focusPageGeometry(width: number, fontSize: number, mobile = false) {
  const { minPageWidthEm, pageGapPx } = FOCUS_READING_BUDGET;
  const pagesPerScreen = !mobile && width >= 2 * minPageWidthEm * fontSize + pageGapPx ? 2 : 1;
  return { pagesPerScreen, pageWidth: (width - (pagesPerScreen - 1) * pageGapPx) / pagesPerScreen, gap: pageGapPx };
}
