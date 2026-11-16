import { createHash } from "node:crypto";
import { createRequire } from "node:module";
import { copyFileSync, mkdtempSync, rmSync } from "node:fs";
import path from "node:path";
import { tmpdir } from "node:os";
import { extractPdfTextGeometry, type PdfTextGeometry } from "./pdf-geometry";
import { sourceFromPdfGeometry } from "./pdf-native-source";
import { pdfOcrSession, type PdfOcrResult, type PdfBox } from "./pdf-ocr";
import { markdownToBlocks } from "./md-adapter";
import { segment } from "./segment";
import { buildSourceManifestV2 } from "./source-manifest";
import { pdfUserSpaceCoordinateSystem, type PdfSelectionMapPageShard, type PdfSourceMapEntry } from "./pdf-source-map";
import type { HybridFoundationArtifacts } from "./hybrid-foundation";

const sha = (value: string | Uint8Array) => createHash("sha256").update(value).digest("hex");
const config = { algorithm: "pdf_text_layer_v1" as const, lookback_words: 0, lookahead_words: 0, merge_gap_utf16: 0, coordinate_system: "pdf_user_space" as const, normalization: ["literal_markdown", "reading_order"] };

export async function extractPdfWithOcr(pdf: Uint8Array, options: { signal?: AbortSignal; onProgress?: (message: string) => void } = {}) {
  // Resolve native bindings, worker scripts and WASM beside the packaged sidecar.
  const require = createRequire(process.env.UNDERSTAND_BOOK_SIDECAR_SELF || import.meta.url);
  const { createCanvas } = require("@napi-rs/canvas") as typeof import("@napi-rs/canvas");
  const { createWorker, PSM } = require("tesseract.js/src/index.js") as typeof import("tesseract.js");
  const ocr = pdfOcrSession({
    ...options,
    async createWorker() {
      const directory = mkdtempSync(path.join(tmpdir(), "understand-book-ocr-"));
      try {
        for (const code of ["eng", "chi_sim"]) copyFileSync(require.resolve(`@tesseract.js-data/${code}/4.0.0_best_int/${code}.traineddata.gz`), path.join(directory, `${code}.traineddata.gz`));
        const worker = await createWorker("eng+chi_sim", 1, { langPath: directory, cacheMethod: "none", errorHandler: () => {} });
        await worker.setParameters({ tessedit_pageseg_mode: PSM.AUTO });
        return worker;
      } finally { rmSync(directory, { recursive: true, force: true }); }
    },
    async render(page, region, scale) {
      const viewport = page.getViewport({ scale });
      const rect = [...viewport.convertToViewportPoint(region[0], region[1]), ...viewport.convertToViewportPoint(region[2], region[3])];
      const left = Math.min(rect[0], rect[2]), top = Math.min(rect[1], rect[3]);
      const canvas = createCanvas(Math.max(1, Math.ceil(Math.abs(rect[2] - rect[0]))), Math.max(1, Math.ceil(Math.abs(rect[3] - rect[1]))));
      const task = page.render({ canvas: canvas as any, canvasContext: canvas.getContext("2d") as any, viewport, transform: [1, 0, 0, 1, -left, -top] });
      const abort = () => task.cancel();
      options.signal?.addEventListener("abort", abort, { once: true });
      try { await task.promise; } finally { options.signal?.removeEventListener("abort", abort); }
      return {
        image: canvas.toBuffer("image/png"),
        toPdf(box: PdfBox): PdfBox {
          const a = viewport.convertToPdfPoint(box[0] + left, box[1] + top), b = viewport.convertToPdfPoint(box[2] + left, box[3] + top);
          return [Math.min(a[0], b[0]), Math.min(a[1], b[1]), Math.max(a[0], b[0]), Math.max(a[1], b[1])];
        },
        dispose() { canvas.width = 1; canvas.height = 1; },
      };
    },
  });
  try {
    const geometry = await extractPdfTextGeometry(pdf, { signal: options.signal, preparePage: ocr.preparePage });
    return { geometry, ocr: ocr.results };
  } finally { await ocr.close(); }
}

/** Reuse the reader's existing artifact contract with direct source-position bindings. */
export function buildPdfNativeFoundation(input: {
  book_id: string; pdf: Uint8Array; original_pdf_path: string; geometry: PdfTextGeometry; ocr?: PdfOcrResult[];
}): { source: string; artifacts: HybridFoundationArtifacts } {
  const native = sourceFromPdfGeometry(input.geometry);
  const blocks = markdownToBlocks(native.source), nodes = segment(blocks);
  const leaves = nodes.filter(node => !node.children.length);
  const configHash = sha(JSON.stringify(config));
  const entries: PdfSourceMapEntry[] = [];
  const shards = new Map<number, PdfSelectionMapPageShard>(input.geometry.pages.map(page => [page.pageIndex, {
    version: "pdf_selection_map_page.v1", book_id: input.book_id, pageIndex: page.pageIndex,
    ...(page.page_label ? { page_label: page.page_label } : {}), chars: [],
  }]));
  const geometryLines = new Map<string, { pageIndex: number; bbox: PdfBox; key: string }>();
  for (const paragraph of native.paragraphs) for (const line of paragraph.lines) {
    for (let index = line.char_start; index < line.char_end; index++) geometryLines.set(`${line.pageIndex}:${index}`, { pageIndex: line.pageIndex, bbox: line.bbox, key: `${line.pageIndex}:${line.lineIndex}` });
  }
  let bindingIndex = 0;
  const pageRegions: Record<string, string[]> = {};
  for (const node of leaves) {
    const lines = new Map<string, { pageIndex: number; bbox: PdfBox }>();
    while (bindingIndex < native.bindings.length && native.bindings[bindingIndex].source_span.start < node.span.end) {
      const binding = native.bindings[bindingIndex++];
      if (binding.source_span.start < node.span.start) continue;
      const { char } = binding;
      const shard = shards.get(char.pageIndex)!;
      // Runtime selection walks char_index order, including OCR and interleaved PDF columns.
      shard.chars.push({ char_index: shard.chars.length, text: char.text, rect: { pageIndex: char.pageIndex, bbox: char.bbox }, source_span: binding.source_span, lid: node.lid });
      const line = geometryLines.get(`${char.pageIndex}:${char.charIndex}`);
      if (line) lines.set(line.key, line);
    }
    const regions = [...lines.values()].map((line, index) => ({ region_id: `pdf-${node.lid}-${index}`, pageIndex: line.pageIndex, bbox: line.bbox }));
    for (const region of regions) (pageRegions[String(region.pageIndex)] ??= []).push(region.region_id);
    entries.push({ lid: node.lid, source_span: node.span, status: regions.length ? "word_mapped" : "unmapped", regions,
      ...(regions.length ? { primary_region: regions[0] } : {}), alignment: { confidence: 1, reason: "Text and source spans were emitted from the same PDF characters" } });
  }
  const pages = [...shards.values()];
  const manifest = buildSourceManifestV2({ book_id: input.book_id, source_sha256: sha(native.source), canonical_source_kind: "pdf_text",
    original_pdf_path: input.original_pdf_path, original_pdf_sha256: sha(input.pdf), pdf_source_map_path: "pdf_source_map.json",
    pdf_selection_map_manifest_path: "pdf_selection_map/manifest.json", alignment_report_path: "alignment_report.json", config_hash: configHash });
  return { source: native.source, artifacts: {
    base: { book_id: input.book_id, lid_nodes: nodes, graph_nodes: [], graph_edges: [] }, source_manifest: manifest,
    pdf_source_map: {
      version: "pdf_source_map.v1", book_id: input.book_id, coordinate_system: pdfUserSpaceCoordinateSystem(), config_hash: configHash,
      pages: input.geometry.pages.map(({ pageIndex, page_label, width, height, rotate, view }) => ({ pageIndex, ...(page_label ? { page_label } : {}), width, height, rotate, view,
        ...(input.ocr?.find(page => page.pageIndex === pageIndex)?.lines.length ? { ocr_lines: input.ocr.find(page => page.pageIndex === pageIndex)!.lines } : {}) })),
      entries, excluded_regions: [], page_region_index: pageRegions, page_excluded_index: {},
    },
    pdf_selection_map_manifest: { version: "pdf_selection_map.v1", book_id: input.book_id, coordinate_system: pdfUserSpaceCoordinateSystem(), config_hash: configHash,
      page_shards: pages.map(page => ({ pageIndex: page.pageIndex, path: `pages/${page.pageIndex}.json`, sha256: sha(JSON.stringify(page, null, 2)) })) },
    pdf_selection_map_pages: pages,
    alignment_report: { version: "alignment_report.v1", book_id: input.book_id, config, config_hash: configHash,
      hard_gates: { all_entries_reference_leaf_lids: true, minimum_text_mapping_ratio: true, minimum_heading_mapping_ratio: true },
      diagnostics: { native_source: true, page_count: input.geometry.pages.length, ocr_pages: input.ocr?.filter(page => page.lines.length).map(page => page.pageIndex) ?? [], empty_pages: input.geometry.pages.filter(page => !page.lines.some(line => line.text.trim())).map(page => page.pageIndex) }, normalization_provenance: [] },
  } };
}
