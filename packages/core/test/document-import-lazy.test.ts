import { mkdtempSync, mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { describe, expect, it, vi } from "vitest";

describe("document preparation loads native PDF support only when needed", () => {
  it("keeps Markdown preparation and unchanged PDF recovery independent of native PDF loading", async () => {
    vi.resetModules();
    const loadNativePdf = vi.fn(() => { throw new Error("native PDF support must remain unloaded"); });
    vi.doMock("../src/pdf-native-foundation", loadNativePdf);
    const root = mkdtempSync(path.join(tmpdir(), "ub-lazy-document-"));
    try {
      const { prepareDocumentInput } = await import("../src/document-import");
      expect(await prepareDocumentInput("book.md", root)).toBe("book.md");
      const source = path.join(root, "saved.pdf");
      writeFileSync(source, "existing source bytes");
      const workspace = path.join(root, ".understand-book", "saved");
      mkdirSync(workspace, { recursive: true });
      writeFileSync(path.join(workspace, "original.pdf"), readFileSync(source));
      writeFileSync(path.join(workspace, "document_import.json"), JSON.stringify({
        source_path: source, source_snapshot: "original.pdf",
      }));
      expect(await prepareDocumentInput(source, root)).toBe(workspace);
      expect(loadNativePdf).not.toHaveBeenCalled();
    } finally {
      vi.doUnmock("../src/pdf-native-foundation");
      vi.resetModules();
      rmSync(root, { recursive: true, force: true });
    }
  });

  it.skipIf(!process.env.UNDERSTAND_BOOK_STARTUP_TEST_EXE)("does not extract native libraries across repeated compiled control calls", () => {
    const root = mkdtempSync(path.join(tmpdir(), "ub-control-startup-"));
    try {
      for (const command of ["build.step", "build.refill", "build.refill"]) {
        const result = spawnSync(process.env.UNDERSTAND_BOOK_STARTUP_TEST_EXE!, [command], {
          input: "{}", encoding: "utf8", windowsHide: true,
          env: { ...process.env, TEMP: root, TMP: root, TMPDIR: root },
        });
        expect(result.status, result.stderr).toBe(0);
        expect(JSON.parse(result.stdout).version).toBe("automatic_build_step.v1");
        expect(readdirSync(root).filter(name => name.endsWith(".node"))).toEqual([]);
      }
    } finally { rmSync(root, { recursive: true, force: true }); }
  }, 30_000);
});
