import { copyFileSync, existsSync, mkdirSync } from "node:fs";
import { spawnSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

const desktopRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const repoRoot = path.resolve(desktopRoot, "..", "..");
const outputDir = process.env.UNDERSTAND_BOOK_SIDECAR_OUTPUT_DIR
  ? path.resolve(process.env.UNDERSTAND_BOOK_SIDECAR_OUTPUT_DIR)
  : path.join(desktopRoot, "src-tauri", "binaries");
const output = path.join(outputDir, "understand-book-build-x86_64-pc-windows-msvc.exe");
mkdirSync(outputDir, { recursive: true });

function bunExecutable() {
  if (process.env.BUN_BINARY) return process.env.BUN_BINARY;
  if (process.platform !== "win32") return "bun";
  for (const directory of (process.env.PATH ?? "").split(path.delimiter)) {
    const executable = path.join(directory, "node_modules", "bun", "bin", "bun.exe");
    if (existsSync(executable)) return executable;
  }
  throw new Error("bun.exe was not found; install Bun or set BUN_BINARY");
}

const result = spawnSync(
  bunExecutable(),
  [
    "build",
    path.join(repoRoot, "skills", "build", "sidecar-entry.ts"),
    "--compile",
    "--target=bun-windows-x64",
    "--loader=.md:text",
    "--loader=.toml:text",
    "--loader=.cmd:text",
    "--loader=.py:text",
    "--external=@napi-rs/canvas",
    "--external=tesseract.js",
    `--outfile=${output}`,
  ],
  { cwd: repoRoot, stdio: "inherit" },
);
if (result.error) throw result.error;
if (result.status !== 0) process.exit(result.status ?? 1);
// Keep the LGPL renderer replaceable, with its exact corresponding source and notices.
const musicResources = spawnSync(process.execPath,
  [path.join(desktopRoot, 'scripts/prepare-music-resources.mjs'), path.join(outputDir, 'resources/music/verovio')],
  { cwd:repoRoot, stdio:'inherit', windowsHide:true });
if (musicResources.error) throw musicResources.error;
if (musicResources.status !== 0) process.exit(musicResources.status ?? 1);
const pdfResources = spawnSync(process.execPath,
  [path.join(desktopRoot, 'scripts/prepare-pdf-resources.mjs'), path.join(outputDir, 'node_modules')],
  { cwd: repoRoot, stdio: 'inherit', windowsHide: true });
if (pdfResources.error) throw pdfResources.error;
if (pdfResources.status !== 0) process.exit(pdfResources.status ?? 1);
// OCR uses native Canvas + Node workers; run that stage on the bundled Node runtime.
const stageDir = path.join(outputDir, 'resources/build');
mkdirSync(stageDir, { recursive: true });
copyFileSync(process.execPath, path.join(stageDir, 'node.exe'));
copyFileSync(path.join(path.dirname(process.execPath), 'LICENSE'), path.join(stageDir, 'NODE-LICENSE.txt'));
const stage = spawnSync(bunExecutable(), ['build', path.join(repoRoot, 'skills/build/workbench-stage-runner.ts'),
  '--target=node', '--format=esm', '--external=@napi-rs/canvas', '--external=tesseract.js', '--external=pdfjs-dist',
  `--outfile=${path.join(stageDir, 'workbench-stage.mjs')}`],
  { cwd: repoRoot, stdio: 'inherit', windowsHide: true });
if (stage.error) throw stage.error;
if (stage.status !== 0) process.exit(stage.status ?? 1);
