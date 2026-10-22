import { spawn } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, mkdtempSync, readFileSync, renameSync, rmSync, statSync, writeFileSync } from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { deriveBookId } from "./book-id";
import { writeHybridFoundationArtifacts } from "./hybrid-foundation";
import { buildProfileMetadata, buildReproducibleProfileArtifactHeader } from "./profile-artifact";

export interface DocumentImport {
  version: "document_import.v1";
  source_path: string;
  source_format: "pdf" | "pptx" | "ppt";
  source_snapshot: string;
  page_count: number;
  page_unit: "page" | "slide";
  text_extraction: "pdf_text_layer" | "pdf_text_layer_with_ocr";
  ocr_pages?: number[];
  limitations: string[];
}

export class DocumentPreparationError extends Error {
  readonly code = "document_preparation_unavailable";
}

export function isDocumentInput(file: string): boolean {
  return /\.(pdf|pptx|ppt)$/iu.test(file);
}

/** An explicit executable is shared by the upload service and the build host. */
export function documentConverter(): string | undefined {
  const executable = process.env.UNDERSTAND_BOOK_LIBREOFFICE;
  return executable && path.isAbsolute(executable) && existsSync(executable) && statSync(executable).isFile() ? executable : undefined;
}

async function slidesToPdf(input: string, outputDirectory: string, executable: string, signal?: AbortSignal): Promise<string> {
  const profile = pathToFileURL(path.join(outputDirectory, "office-profile")).href;
  await new Promise<void>((resolve, reject) => {
    const child = spawn(executable, [
      `-env:UserInstallation=${profile}`, "--headless", "--convert-to",
      'pdf:impress_pdf_Export:{"ExportHiddenSlides":{"type":"boolean","value":"true"}}',
      "--outdir", outputDirectory, input,
    ], { windowsHide: true, shell: false, stdio: "ignore" });
    const abort = () => child.kill();
    signal?.addEventListener("abort", abort, { once: true });
    // A malformed deck must not retain the sole preparation worker indefinitely.
    const timer = setTimeout(() => child.kill(), 120_000);
    child.once("error", reject);
    child.once("close", (code) => {
      clearTimeout(timer); signal?.removeEventListener("abort", abort);
      if (signal?.aborted) reject(new Error("资料准备已取消，原文件仍可继续使用。"));
      else if (code !== 0) reject(new DocumentPreparationError("课件转 PDF 失败，请检查文件是否能正常打开，或直接上传导出的 PDF。"));
      else resolve();
    });
    if (signal?.aborted) abort();
  });
  const output = path.join(outputDirectory, `${path.basename(input, path.extname(input))}.pdf`);
  if (!existsSync(output) || statSync(output).size === 0) {
    throw new DocumentPreparationError("课件转换没有生成 PDF，请直接上传从演示软件导出的 PDF。");
  }
  return output;
}

/**
 * Text-layer extraction is deterministic and runs before the model plan.
 * Source PDF pages remain the visual authority for tables, equations and diagrams.
 */
export async function prepareDocumentInput(
  input: string, rootDir: string, options: { book_id?: string; signal?: AbortSignal; libreoffice?: string } = {},
): Promise<string> {
  const sourcePath = path.resolve(rootDir, input);
  if (!isDocumentInput(sourcePath)) return input;
  const format = path.extname(sourcePath).slice(1).toLowerCase() as DocumentImport["source_format"];
  const converter = options.libreoffice ?? documentConverter();
  const bookId = deriveBookId(sourcePath, options.book_id);
  const library = path.join(path.resolve(rootDir), ".understand-book");
  const workspace = path.join(library, bookId);
  const marker = path.join(workspace, "document_import.json");
  if (existsSync(marker)) {
    const saved = JSON.parse(readFileSync(marker, "utf8")) as DocumentImport;
    if (saved.source_path !== sourcePath || !readFileSync(path.join(workspace, saved.source_snapshot)).equals(readFileSync(sourcePath))) {
      throw new DocumentPreparationError("这份准备成果对应另一份原文件，请为新文件创建新的资料准备任务。");
    }
    return workspace;
  }
  if (format !== "pdf" && !converter) {
    throw new DocumentPreparationError("当前服务器尚未启用 PPT/PPTX 转换，请从演示软件导出 PDF 后上传。");
  }
  if (existsSync(workspace)) throw new DocumentPreparationError("已有资料工作区缺少文档导入记录，请使用新的资料准备任务。");
  mkdirSync(library, { recursive: true });
  const staging = mkdtempSync(path.join(library, ".document-"));
  try {
    const original = path.join(staging, "original.pdf");
    if (format === "pdf") copyFileSync(sourcePath, original);
    else {
      const conversionDir = path.join(staging, "conversion"); mkdirSync(conversionDir);
      const pdf = await slidesToPdf(sourcePath, conversionDir, converter!, options.signal);
      copyFileSync(pdf, original);
      copyFileSync(sourcePath, path.join(staging, `source.${format}`));
      rmSync(conversionDir, { recursive: true, force: true });
    }
    if (options.signal?.aborted) throw new Error("资料准备已取消。");
    // Control calls and unchanged document recovery do not need the native PDF runtime.
    const { extractPdfWithOcr, buildPdfNativeFoundation } = await import("./pdf-native-foundation");
    const pdf = new Uint8Array(readFileSync(original));
    const { geometry, ocr } = await extractPdfWithOcr(pdf, { signal: options.signal }).catch(error => {
      if (error?.name === "PasswordException") throw new DocumentPreparationError("这份 PDF 需要密码，请先上传已解锁的副本。");
      if (error?.name === "InvalidPDFException") throw new DocumentPreparationError("无法读取这份 PDF，请确认文件可正常打开后重新导出。");
      throw error;
    });
    const { source, artifacts } = buildPdfNativeFoundation({ book_id: bookId, pdf, original_pdf_path: "original.pdf", geometry, ocr });
    if (!source.trim()) throw new DocumentPreparationError("这份 PDF 在文字提取和图片识别后仍没有可读正文，原文件可继续阅读。");
    writeHybridFoundationArtifacts(staging, source, artifacts);
    writeFileSync(path.join(staging, "profile_metadata.json"), JSON.stringify(buildProfileMetadata(
      buildReproducibleProfileArtifactHeader({ book_id: bookId, content_profile: "technical_learning" })), null, 2));
    const imported: DocumentImport = {
      version: "document_import.v1", source_path: sourcePath, source_format: format,
      source_snapshot: format === "pdf" ? "original.pdf" : `source.${format}`,
      page_count: geometry.pages.length, page_unit: format === "pdf" ? "page" : "slide",
      text_extraction: ocr.some(page => page.lines.length) ? "pdf_text_layer_with_ocr" : "pdf_text_layer",
      ocr_pages: ocr.filter(page => page.lines.length).map(page => page.pageIndex),
      limitations: ["公式、表格、图示及版式请以来源 PDF 页为准。", ...(ocr.some(page => page.lines.length) ? ["图片中的文字由自动识别补充，可对照原页阅读。"] : [])],
    };
    writeFileSync(path.join(staging, "document_import.json"), JSON.stringify(imported, null, 2));
    renameSync(staging, workspace);
    return workspace;
  } finally {
    if (existsSync(staging)) rmSync(staging, { recursive: true, force: true });
  }
}
