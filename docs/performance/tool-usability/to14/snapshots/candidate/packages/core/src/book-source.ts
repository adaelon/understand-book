import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { epubToSource } from "./epub-adapter";
import { markdownToBlocks } from "./md-adapter";
import { segment, type SourceBlock } from "./segment";
import { ReadOnlyBaseZ } from "./zod";

/** EPUB canonical text lacks heading markup. Existing bundles retain its structure in base.json. */
export function loadBookSource(file: string) {
  if (/\.epub$/i.test(file)) {
    const loaded = epubToSource(new Uint8Array(readFileSync(file)));
    return { ...loaded, lidNodes: segment(loaded.blocks) };
  }
  const source = readFileSync(file, "utf8");
  const manifestPath = path.join(path.dirname(file), "source_manifest.json");
  if (path.basename(file) === "source.txt" && existsSync(manifestPath)) {
    const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
    if (manifest.canonical_source?.kind === "epub") {
      const base = ReadOnlyBaseZ.parse(JSON.parse(readFileSync(path.join(path.dirname(file), "base.json"), "utf8")));
      if (base.book_id !== manifest.book_id) throw new Error("EPUB snapshot base identity mismatch");
      return { source, blocks: [] as SourceBlock[], lidNodes: base.lid_nodes };
    }
  }
  const blocks = markdownToBlocks(source);
  return { source, blocks, lidNodes: segment(blocks) };
}
