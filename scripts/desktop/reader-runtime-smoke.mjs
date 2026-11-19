// Exercise the installed format runtime from a directory outside the checkout.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { cpSync, existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { deflateSync } from 'node:zlib';

const resourceDir = path.resolve(process.argv[2] ?? 'apps/local-desktop/src-tauri/resources/reader-runtime');
const evidenceDir = path.resolve(process.argv[3] ?? `docs/evidence/dual-edition-20261117/df3/windows/reader-runtime-${Date.now()}`);
mkdirSync(evidenceDir, { recursive: true });
const tempBase = path.resolve(process.platform === 'win32' && process.env.LOCALAPPDATA
  ? path.join(process.env.LOCALAPPDATA, 'Temp') : os.tmpdir());
mkdirSync(tempBase, { recursive: true });
const isolated = mkdtempSync(path.join(tempBase, 'understand-book-runtime-'));
const packaged = path.join(isolated, 'Installed Reader With Spaces');
cpSync(resourceDir, packaged, { recursive: true });
const manifest = JSON.parse(readFileSync(path.join(packaged, 'runtime.json'), 'utf8'));
const node = path.join(packaged, manifest.node);
const runner = path.join(packaged, manifest.runner);
const env = { ...process.env, PATH: process.platform === 'win32' ? path.join(process.env.SystemRoot ?? 'C:/Windows', 'System32') : '/usr/bin:/bin' };
for (const key of ['NODE_PATH', 'NODE_OPTIONS', 'UNDERSTAND_BOOK_SIDECAR_SELF', 'UNDERSTAND_BOOK_BUILD_SIDECAR']) delete env[key];

function pdf(objects) {
  const chunks = [Buffer.from('%PDF-1.4\n')], offsets = [0];
  let length = chunks[0].length;
  for (let i = 0; i < objects.length; i++) {
    offsets.push(length);
    const chunk = Buffer.concat([Buffer.from(`${i + 1} 0 obj\n`), Buffer.from(objects[i]), Buffer.from('\nendobj\n')]);
    chunks.push(chunk); length += chunk.length;
  }
  chunks.push(Buffer.from(`xref\n0 ${objects.length + 1}\n0000000000 65535 f \n${offsets.slice(1).map(offset => `${String(offset).padStart(10, '0')} 00000 n \n`).join('')}trailer\n<< /Size ${objects.length + 1} /Root 1 0 R >>\nstartxref\n${length}\n%%EOF\n`));
  return Buffer.concat(chunks);
}
function stream(data, fields = '') {
  const bytes = Buffer.from(data);
  return Buffer.concat([Buffer.from(`<< /Length ${bytes.length} ${fields} >>\nstream\n`), bytes, Buffer.from('\nendstream')]);
}
function textPdf() {
  return pdf([
    '<< /Type /Catalog /Pages 2 0 R >>', '<< /Type /Pages /Kids [3 0 R] /Count 1 >>',
    '<< /Type /Page /Parent 2 0 R /MediaBox [0 0 500 200] /Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>',
    '<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>',
    stream('BT /F1 16 Tf 40 100 Td (Local PDF reading works) Tj ET'),
  ]);
}
function scanPdf() {
  const width = 1200, height = 280;
  // A short-lived process releases the native Canvas DLL before cleanup on Windows.
  const generated = spawnSync(node, ['-e', `
    const { createCanvas } = require('./node_modules/@napi-rs/canvas');
    const canvas = createCanvas(${width}, ${height}), ctx = canvas.getContext('2d');
    ctx.fillStyle = '#fff'; ctx.fillRect(0, 0, ${width}, ${height});
    ctx.fillStyle = '#000'; ctx.font = 'bold 76px Arial'; ctx.fillText('LOCAL READER OCR', 60, 170);
    const rgba = ctx.getImageData(0, 0, ${width}, ${height}).data;
    const rgb = Buffer.alloc(${width} * ${height} * 3);
    for (let i = 0; i < ${width} * ${height}; i++) for (let c = 0; c < 3; c++) rgb[i * 3 + c] = rgba[i * 4 + c];
    process.stdout.write(rgb);
  `], { cwd: packaged, env, windowsHide: true, maxBuffer: 4 * 1024 * 1024 });
  assert.equal(generated.status, 0, generated.stderr?.toString());
  const rgb = generated.stdout;
  assert.equal(rgb.length, width * height * 3);
  return pdf([
    '<< /Type /Catalog /Pages 2 0 R >>', '<< /Type /Pages /Kids [3 0 R] /Count 1 >>',
    '<< /Type /Page /Parent 2 0 R /MediaBox [0 0 600 140] /Resources << /XObject << /Image 4 0 R >> >> /Contents 5 0 R >>',
    stream(deflateSync(rgb), `/Type /XObject /Subtype /Image /Width ${width} /Height ${height} /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /FlateDecode`),
    stream('q 600 0 0 140 0 0 cm /Image Do Q'),
  ]);
}
const checks = [];
try {
  // A missing bundled module must not resolve through checkout node_modules.
  for (let directory = path.dirname(isolated); directory !== path.dirname(directory); directory = path.dirname(directory)) {
    assert(!existsSync(path.join(directory, 'node_modules')), `Choose a clean runtime smoke location: ${directory} contains node_modules`);
  }
  for (const [name, bytes, expected, extraction] of [
    ['text', textPdf(), 'Local PDF reading works', 'pdf_text_layer'],
    ['scan', scanPdf(), 'LOCAL READER OCR', 'pdf_text_layer_with_ocr'],
  ]) {
    const book = path.join(isolated, `book-${name}`);
    mkdirSync(path.join(book, '.build/input'), { recursive: true });
    mkdirSync(path.join(book, '.build/jobs'), { recursive: true });
    writeFileSync(path.join(book, 'paper.pdf'), bytes);
    // Existing workbench input identity, not an extra release checksum.
    const fingerprint = { paper_pdf_sha256: createHash('sha256').update(bytes).digest('hex'), config_hash: 'reader-runtime-smoke-v1' };
    writeFileSync(path.join(book, '.build/input/manifest.json'), JSON.stringify({
      version: 'workbench_input_manifest.v1', book_id: `runtime-${name}`, profile_id: 'paper',
      inputs: { paper_pdf: { path: 'paper.pdf', sha256: fingerprint.paper_pdf_sha256 } }, fingerprint,
    }));
    const jobFile = path.join(book, '.build/jobs/smoke.json');
    writeFileSync(jobFile, JSON.stringify({ version: 'build_job_state.v1', job_id: 'smoke', book_id: `runtime-${name}`,
      input_fingerprint: fingerprint, status: 'running', active_run: { runner_token: 'runtime-smoke' },
      events: [], decision_requests: [], permission_requests: [], created_at: '1', updated_at: '1' }));
    const result = spawnSync(node, [runner, '--book-dir', book, '--job-id', 'smoke', '--stage', 'source_reconciliation', '--runner-token', 'runtime-smoke'],
      { cwd: isolated, env, windowsHide: true, encoding: 'utf8', timeout: 120_000 });
    writeFileSync(path.join(evidenceDir, `${name}.log`), `${result.stdout ?? ''}\n${result.stderr ?? ''}`);
    cpSync(book, path.join(evidenceDir, name), { recursive: true });
    assert.equal(result.error, undefined, String(result.error));
    assert.equal(result.status, 0, `${name}: ${result.stderr}`);
    const job = JSON.parse(readFileSync(jobFile, 'utf8'));
    assert.equal(job.status, 'done', JSON.stringify(job.failure_summary));
    const source = readFileSync(path.join(book, 'source.txt'), 'utf8');
    assert(source.includes(expected), `${name}: ${source}`);
    const imported = JSON.parse(readFileSync(path.join(book, 'document_import.json'), 'utf8'));
    assert.equal(imported.text_extraction, extraction);
    checks.push({ name, status: job.status, extraction, source: source.trim() });
  }
  const semantic = spawnSync(node, [runner, '--stage', 'pass1'], { cwd: isolated, env, windowsHide: true, encoding: 'utf8' });
  assert.equal(semantic.status, 2);
  assert.match(semantic.stderr, /build plugin/);
  checks.push({ name: 'semantic-plugin-handoff', exit: semantic.status });
  writeFileSync(path.join(evidenceDir, 'result.json'), JSON.stringify({ platform: process.platform, arch: process.arch, isolated_runtime: true, developer_path: false, checks }, null, 2) + '\n');
  console.log(JSON.stringify({ evidence: evidenceDir, checks }, null, 2));
} finally {
  const resolved = path.resolve(isolated);
  if (path.dirname(resolved) !== tempBase || !path.basename(resolved).startsWith('understand-book-runtime-')) throw new Error('Refusing to remove an unrelated temporary directory.');
  rmSync(resolved, { recursive: true, force: true });
}
