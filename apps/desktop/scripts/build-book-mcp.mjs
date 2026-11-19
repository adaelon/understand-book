import { copyFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { desktopTarget, repoRoot, run } from "../../../scripts/desktop/platform.mjs";

if (process.platform !== "win32") {
  throw new Error("the cloud maintenance sidecars currently support Windows only");
}

const desktopRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const target = desktopTarget();
const targetRoot = process.env.CARGO_TARGET_DIR
  ? path.resolve(repoRoot, process.env.CARGO_TARGET_DIR)
  : path.join(repoRoot, "target");
process.env.UNDERSTAND_BOOK_MCP_OUTPUT_DIR ??= path.join(desktopRoot, "src-tauri", "binaries");
const outputDir = path.resolve(process.env.UNDERSTAND_BOOK_MCP_OUTPUT_DIR);

// The public MCP remains independently buildable; cloud video maintenance also
// needs the audio reviewer and the private publication command.
await import("../../../scripts/plugin/build-book-mcp.mjs");
run(process.env.CARGO ?? "cargo", [
  "build", "--locked", "--release", "-p", "reader-service", "--bin", "video_audio_review",
]);
run(process.env.CARGO ?? "cargo", [
  "build", "--locked", "--release", "-p", "server", "--bin", "publish_book",
]);

for (const [sourceName, targetName] of [["publish_book", "publish-book"], ["video_audio_review", "video-audio-review"]]) {
  copyFileSync(
    path.join(targetRoot, "release", `${sourceName}${target.suffix}`),
    path.join(outputDir, `${targetName}-${target.triple}${target.suffix}`),
  );
}
