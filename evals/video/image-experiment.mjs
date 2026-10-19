import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { resolve, join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';
import { createInterface } from 'node:readline';

export const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
export const output = join(root, 'evals/video/generated/vi0c');
const media = join(root, 'evals/video/generated');
export const question = 'Read only the supplied images, in their supplied order. For each image transcribe the visible tensor construction, assertion including operator/comment, displayed numeric result, or highlighted explanation. For matrix code give the input shapes and exact einsum signature and result. Compare changes between images. Do not infer hidden text. Return concise JSON with an images array and a comparison string. No other tools or files.';
export const cases = {
  single_a: ['f555123'],
  single_b: ['f650000'],
  crop_a: ['f555123_crop'],
  crop_b: ['f650000_crop'],
  pair: ['f555123', 'f650000'],
  reverse: ['f650000', 'f555123'],
  progressive: ['f600000', 'f615000', 'f630000', 'f650000'],
  matrix: ['f1200000', 'f1210000'],
  capacity8: ['f555123', 'f650000', 'f600000', 'f615000', 'f630000', 'f650000', 'f1200000', 'f1210000'],
};

export function save(path, value) {
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, JSON.stringify(value, null, 2) + '\n');
}

export function pngSize(bytes) {
  if (!bytes.subarray(0, 8).equals(Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]))) throw new Error('Expected PNG');
  return { width: bytes.readUInt32BE(16), height: bytes.readUInt32BE(20) };
}

export function prepare() {
  const replay = JSON.parse(readFileSync(join(media, 'replay.json'), 'utf8'));
  const source = JSON.parse(readFileSync(join(root, 'evals/video/samples.json'), 'utf8')).source;
  const frames = {};
  for (const id of new Set(Object.values(cases).flat().filter(id => !id.endsWith('_crop')))) {
    const frame = replay.frames.find(f => f.id === id);
    if (!frame) throw new Error(`Missing replay frame ${id}`);
    const path = join(media, frame.path);
    const png = readFileSync(path);
    frames[id] = { ...frame, source_revision: source.revision, path, ...pngSize(png), png_bytes: png.length };
  }
  // Original-pixel code regions, excluding burned subtitles. No resizing/OCR.
  const crops = { f555123: [30, 480, 1120, 150], f650000: [30, 420, 1120, 150] };
  mkdirSync(join(output, 'images'), { recursive: true });
  for (const [id, rect] of Object.entries(crops)) {
    const path = join(output, 'images', `${id}_crop.png`);
    const [x, y, width, height] = rect;
    const result = spawnSync(process.env.FFMPEG || 'ffmpeg', ['-hide_banner', '-loglevel', 'error', '-y', '-i', frames[id].path, '-vf', `crop=${width}:${height}:${x}:${y}`, '-frames:v', '1', path], { windowsHide: true, encoding: 'utf8' });
    if (result.status !== 0) throw new Error(result.stderr || 'Crop failed');
    const png = readFileSync(path);
    frames[`${id}_crop`] = { ...frames[id], id: `${id}_crop`, path, rect_pixels: rect,
      rect_normalized: [x / frames[id].width, y / frames[id].height, width / frames[id].width, height / frames[id].height],
      ...pngSize(png), png_bytes: png.length };
  }
  const manifest = { source, frames, cases, question };
  save(join(output, 'inputs.json'), manifest);
  return manifest;
}

export function mcpResult(manifest, caseId) {
  if (!cases[caseId]) throw new Error(`Unknown case: ${caseId}`);
  return { content: cases[caseId].flatMap((id, index) => [
    { type: 'text', text: `Image ${index + 1}` },
    { type: 'image', mimeType: 'image/png', data: readFileSync(manifest.frames[id].path).toString('base64') },
  ]), isError: false };
}

export function measure(result, id = 1) {
  const blocks = result.content.filter(c => c.type === 'image');
  return { image_count: blocks.length,
    images: blocks.map(c => ({ ...pngSize(Buffer.from(c.data, 'base64')), png_bytes: Buffer.from(c.data, 'base64').length, base64_bytes: Buffer.byteLength(c.data) })),
    mcp_result_bytes: Buffer.byteLength(JSON.stringify(result)),
    jsonrpc_line_bytes: Buffer.byteLength(JSON.stringify({ jsonrpc: '2.0', id, result }) + '\n') };
}

export async function serve(caseId, logDir) {
  const manifest = JSON.parse(readFileSync(join(output, 'inputs.json'), 'utf8'));
  const result = mcpResult(manifest, caseId);
  const tools = [{ name: 'video_frames', description: 'Read the fixed ordered source images. No source files are modified.', annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false }, inputSchema: { type: 'object', properties: {}, additionalProperties: false } }];
  for await (const line of createInterface({ input: process.stdin })) {
    const req = JSON.parse(line);
    if (req.id === undefined) continue;
    let value;
    if (req.method === 'initialize') value = { protocolVersion: req.params.protocolVersion, capabilities: { tools: {} }, serverInfo: { name: 'vi0c-image-probe', version: '1.0.0' } };
    else if (req.method === 'tools/list') value = { tools };
    else if (req.method === 'tools/call' && req.params.name === 'video_frames') {
      value = result;
      save(join(logDir, 'mcp-call.json'), req);
      save(join(logDir, 'mcp-result.json'), { jsonrpc: '2.0', id: req.id, result });
      save(join(logDir, 'transport.json'), measure(result, req.id));
    } else {
      process.stdout.write(JSON.stringify({ jsonrpc: '2.0', id: req.id, error: { code: -32601, message: 'Unknown operation' } }) + '\n');
      continue;
    }
    process.stdout.write(JSON.stringify({ jsonrpc: '2.0', id: req.id, result: value }) + '\n');
  }
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const command = process.argv[2];
  if (command === 'prepare') {
    const inputs = prepare();
    const measurements = Object.fromEntries(Object.keys(cases).map(id => [id, measure(mcpResult(inputs, id))]));
    save(join(output, 'measurements.json'), measurements);
    console.log(JSON.stringify(measurements));
  } else if (command === 'serve') await serve(process.argv[3], resolve(process.argv[4]));
  else throw new Error('usage: image-experiment.mjs prepare | serve CASE LOG_DIR');
}
