import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { epubToSource } from "./epub-adapter";
import { markdownToBlocks } from "./md-adapter";
import { segment, type SourceBlock } from "./segment";
import { ReadOnlyBaseZ } from "./zod";

/** Resume an imported EPUB from its canonical text and published citation anchors.
 * Re-running the current parser would silently apply import changes to an existing build. */
export function loadBookSource(file: string) {
  const directory = path.dirname(file);
  const manifestPath = path.join(directory, "source_manifest.json");
  if ((path.basename(file) === "source.txt" || /\.epub$/i.test(file)) && existsSync(manifestPath)) {
    const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
    const snapshotPath = path.resolve(directory, manifest.canonical_source?.snapshot_path ?? "source.epub");
    if (manifest.canonical_source?.kind === "epub"
      && (path.basename(file) === "source.txt" || path.resolve(file) === snapshotPath)) {
      const source = readFileSync(path.join(directory, "source.txt"), "utf8");
      const base = ReadOnlyBaseZ.parse(JSON.parse(readFileSync(path.join(directory, "base.json"), "utf8")));
      if (base.book_id !== manifest.book_id) throw new Error("EPUB snapshot base identity mismatch");
      return { source, blocks: [] as SourceBlock[], lidNodes: base.lid_nodes };
    }
  }
  if (/\.epub$/i.test(file)) {
    const loaded = epubToSource(new Uint8Array(readFileSync(file)));
    return { ...loaded, lidNodes: segment(loaded.blocks) };
  }
  const source = readFileSync(file, "utf8");
  const blocks = markdownToBlocks(source);
  return { source, blocks, lidNodes: segment(blocks) };
}
