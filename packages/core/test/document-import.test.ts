import { afterEach, describe, expect, it } from "vitest";
import { mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { prepareDocumentInput } from "../src/document-import";
import { resolveAutomaticBuildTarget } from "../src/build-orchestrator";
import { loadBookSource } from "../src/book-source";
import { runAutomaticBuildDriverCommandWithPreparation } from "../../../skills/build/automatic-build-driver";

const directories: string[] = [];
afterEach(() => { for (const dir of directories.splice(0)) rmSync(dir, { recursive: true, force: true }); });
const temporary = () => { const dir = mkdtempSync(path.join(tmpdir(), "ub-document-")); directories.push(dir); return dir; };

/** Original exam fixture: text, formula notation and table rows on two pages. */
function pdf(pages: string[][]): Uint8Array {
  const objects = ["<< /Type /Catalog /Pages 2 0 R >>", "", "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>"];
  const pageIds: number[] = [];
  for (const lines of pages) {
    const pageId = objects.length + 1, streamId = pageId + 1;
    pageIds.push(pageId);
    const stream = `BT /F1 12 Tf 30 180 Td ${lines.map((line, index) =>
      `${index ? "0 -20 Td " : ""}(${line.replace(/[\\()]/gu, "\\$&")}) Tj`).join("\n")} ET\n`;
    objects.push(`<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 220] /Resources << /Font << /F1 3 0 R >> >> /Contents ${streamId} 0 R >>`,
      `<< /Length ${Buffer.byteLength(stream)} >>\nstream\n${stream}endstream`);
  }
  objects[1] = `<< /Type /Pages /Kids [${pageIds.map(id => `${id} 0 R`).join(" ")}] /Count ${pages.length} >>`;
  let output = "%PDF-1.4\n"; const offsets = [0];
  for (const [index, object] of objects.entries()) { offsets.push(Buffer.byteLength(output)); output += `${index + 1} 0 obj\n${object}\nendobj\n`; }
  const xref = Buffer.byteLength(output);
  output += `xref\n0 ${objects.length + 1}\n0000000000 65535 f \n`;
  output += offsets.slice(1).map(offset => `${String(offset).padStart(10, "0")} 00000 n \n`).join("");
  output += `trailer\n<< /Size ${objects.length + 1} /Root 1 0 R >>\nstartxref\n${xref}\n%%EOF\n`;
  return new Uint8Array(Buffer.from(output));
}

describe("cloud exam document preparation", () => {
  it("builds a readable PDF source with actual page references and preserves visual originals on resume", async () => {
    const root = temporary(), file = path.join(root, "outline.pdf");
    const original = pdf([["Exam outline", "f(x) = x^2 + 1"], ["Topic | Marks", "Limits | 20", "Derivatives | 30"]]);
    writeFileSync(file, original);
    const workspace = await prepareDocumentInput(file, root);
    const target = resolveAutomaticBuildTarget(file, root);
    expect(target.workspace_dir).toBe(workspace);
    expect(target.profile_id).toBe("technical_learning");
    const source = loadBookSource(target.source_path);
    expect(source.source).toContain("f(x) = x^2 + 1");
    expect(source.source).toContain("Limits");
    const map = JSON.parse(readFileSync(path.join(workspace, "pdf_source_map.json"), "utf8"));
    expect(new Set(map.entries.flatMap((entry: any) => entry.regions.map((region: any) => region.pageIndex)))).toEqual(new Set([0, 1]));
    expect(map.entries.every((entry: any) => source.lidNodes.some(node => node.lid === entry.lid))).toBe(true);
    expect(readFileSync(path.join(workspace, "original.pdf"))).toEqual(Buffer.from(original));
    writeFileSync(path.join(workspace, "accepted-progress.txt"), "keep");
    expect(await prepareDocumentInput(file, root)).toBe(workspace);
    expect(readFileSync(path.join(workspace, "accepted-progress.txt"), "utf8")).toBe("keep");
    const manifest = JSON.parse(readFileSync(path.join(workspace, "source_manifest.json"), "utf8"));
    expect(manifest.capabilities.view_pdf.status).toBe("available");
    expect(manifest.original_pdf.path).toBe("original.pdf");
  });

  it("reaches the actual asynchronous cloud preparation command and creates a model plan", async () => {
    const root = temporary(), file = path.join(root, "source.pdf");
    writeFileSync(file, pdf([["Original exam study outline. Learn limits and solve practice questions."]]));
    const result: any = await runAutomaticBuildDriverCommandWithPreparation({
      version: "dsh_build_prepare.v2", target_input: file, root_dir: root, max_parallel: 1, pass2: "disabled", build_tutor: false,
    });
    expect(result.version).toBe("dsh_build_prepared.v2");
    expect(result.plan.content_profile.id).toBe("technical_learning");
    expect(result.plan.public_stage_closure).toContain("pass1");
    expect(result.workspace_dir).toBe(path.join(root, ".understand-book", "source"));
    expect(result.plan.create.length).toBeGreaterThan(0);
  });

  it("preserves blank pages without rejecting a readable PDF", async () => {
    const root = temporary(), file = path.join(root, "scan.pdf");
    writeFileSync(file, pdf([["Extractable first page"], []]));
    const workspace = await prepareDocumentInput(file, root);
    expect(readFileSync(path.join(workspace, "source.txt"), "utf8")).toContain("Extractable first page");
    expect(JSON.parse(readFileSync(path.join(workspace, "pdf_source_map.json"), "utf8")).pages).toHaveLength(2);
    expect(readFileSync(file).length).toBeGreaterThan(0);
  }, 60_000);

  it("preserves PDF sources when the existing Pass1 writer updates the learning base", async () => {
    const root = temporary(), file = path.join(root, "outline.pdf");
    writeFileSync(file, pdf([["Original source for the learning writer."]]));
    const workspace = await prepareDocumentInput(file, root);
    const manifest = readFileSync(path.join(workspace, "source_manifest.json"), "utf8");
    const map = readFileSync(path.join(workspace, "pdf_source_map.json"), "utf8");
    const repo = path.resolve(__dirname, "../../..");
    const result = spawnSync(process.execPath, [
      path.join(repo, "node_modules/tsx/dist/cli.mjs"), path.join(repo, "skills/build/pass1-batch.ts"),
      path.join(workspace, "source.txt"), "--book-id", "outline", "--allow-partial",
    ], { cwd: root, encoding: "utf8" });
    expect(result.status, result.stderr).toBe(0);
    expect(readFileSync(path.join(workspace, "source_manifest.json"), "utf8")).toBe(manifest);
    expect(readFileSync(path.join(workspace, "pdf_source_map.json"), "utf8")).toBe(map);
  }, 60_000);

  it("requires a real slides converter instead of treating a PPTX as text", async () => {
    const root = temporary(), file = path.join(root, "lecture.pptx");
    writeFileSync(file, "placeholder for rejected input");
    const previous = process.env.UNDERSTAND_BOOK_LIBREOFFICE;
    delete process.env.UNDERSTAND_BOOK_LIBREOFFICE;
    try { await expect(prepareDocumentInput(file, root)).rejects.toThrow("尚未启用 PPT/PPTX"); }
    finally { if (previous !== undefined) process.env.UNDERSTAND_BOOK_LIBREOFFICE = previous; }
  });
});
