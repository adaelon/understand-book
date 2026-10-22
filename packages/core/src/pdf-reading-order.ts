import type { PdfGeometryLine, PdfGeometryPage } from "./pdf-geometry";

type OrderedPageLine = PdfGeometryLine & { pageIndex: number; contentIndex: number };

function horizontalLineBands(lines: OrderedPageLine[], page: PdfGeometryPage): OrderedPageLine[][] {
  if (!lines.length) return [];
  const heights = lines
    .map((line) => line.bbox[3] - line.bbox[1])
    .filter((height) => height > 0)
    .sort((left, right) => left - right);
  const medianHeight = heights[Math.floor(heights.length / 2)] ?? 0;
  const gapThreshold = Math.max(12, medianHeight * 1.8, page.height * 0.015);
  const spatial = [...lines].sort((left, right) =>
    right.bbox[3] - left.bbox[3]
      || left.bbox[0] - right.bbox[0]
      || left.contentIndex - right.contentIndex,
  );
  const bands: OrderedPageLine[][] = [];
  let bandBottom = Number.POSITIVE_INFINITY;
  for (const line of spatial) {
    const current = bands.at(-1);
    if (!current || bandBottom - line.bbox[3] > gapThreshold) {
      bands.push([line]);
      bandBottom = line.bbox[1];
      continue;
    }
    current.push(line);
    bandBottom = Math.min(bandBottom, line.bbox[1]);
  }
  return bands;
}

function bandLinesInReadingOrder(
  lines: OrderedPageLine[],
  page: PdfGeometryPage,
  maxNormalLineHeight: number,
): OrderedPageLine[] {
  const midpoint = (page.view[0] + page.view[2]) / 2;
  const pageWidth = page.view[2] - page.view[0];
  const spansColumns = (line: PdfGeometryLine): boolean => {
    const width = line.bbox[2] - line.bbox[0];
    return line.bbox[0] < midpoint
      && line.bbox[2] > midpoint
      && width >= pageWidth * 0.45;
  };
  const spanningByHeight = lines
    .filter(spansColumns)
    .sort((left, right) => right.bbox[3] - left.bbox[3]);
  const clusters: typeof spanningByHeight[] = [];
  for (const line of spanningByHeight) {
    const cluster = clusters.at(-1);
    const previous = cluster?.at(-1);
    const previousCenter = previous ? (previous.bbox[1] + previous.bbox[3]) / 2 : null;
    const center = (line.bbox[1] + line.bbox[3]) / 2;
    if (!cluster || previousCenter === null || previousCenter - center > maxNormalLineHeight) {
      clusters.push([line]);
    } else {
      cluster.push(line);
    }
  }
  const primarySpanningCluster = clusters
    .filter((cluster) => cluster.length >= 3)
    .sort((left, right) => right.length - left.length || right[0].bbox[3] - left[0].bbox[3])[0];
  const spanningBand = primarySpanningCluster
    ? (() => {
        const leftEdges = primarySpanningCluster.map((line) => line.bbox[0]).sort((left, right) => left - right);
        return {
        top: Math.max(...primarySpanningCluster.map((line) => line.bbox[3])),
        bottom: Math.min(...primarySpanningCluster.map((line) => line.bbox[1])),
          left: leftEdges[Math.floor(leftEdges.length / 2)],
        };
      })()
    : null;

  return [...lines].sort((left, right) => {
      const group = (line: PdfGeometryLine): number => {
        const alignsWithBandLeft = spanningBand
          ? Math.abs(line.bbox[0] - spanningBand.left) <= Math.max(12, pageWidth * 0.04)
          : false;
        const inPrimaryBand = spanningBand
          ? spansColumns(line) || alignsWithBandLeft
          : spansColumns(line);
        if (inPrimaryBand) return 0;
        return (line.bbox[0] + line.bbox[2]) / 2 < midpoint ? 1 : 2;
      };
      return group(left) - group(right)
        || right.bbox[3] - left.bbox[3]
        || left.bbox[0] - right.bbox[0]
        || left.contentIndex - right.contentIndex;
    });
}

export function pageLinesInReadingOrder(page: PdfGeometryPage, includeTall = false): Array<PdfGeometryLine & { pageIndex: number }> {
  const maxNormalLineHeight = Math.max(30, page.height * 0.08);
  const lines = page.lines
    .filter((line) => line.bbox[0] >= 0
      && line.bbox[1] >= 0
      && line.bbox[2] <= page.width
      && line.bbox[3] <= page.height
      && (includeTall || line.bbox[3] - line.bbox[1] <= maxNormalLineHeight))
    .map((line, contentIndex) => ({ ...line, pageIndex: page.pageIndex, contentIndex }));
  return horizontalLineBands(lines, page)
    .flatMap((band) => bandLinesInReadingOrder(band, page, maxNormalLineHeight))
    .map(({ contentIndex: _contentIndex, ...line }) => line);
}

