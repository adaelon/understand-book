import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { mcpResult, measure, cases } from './image-experiment.mjs';
import { visualChecks, codexRecord, archive } from './image-report.mjs';

test('native image blocks retain bytes and supplied order; no labels leak answers', () => {
  const dir = mkdtempSync(join(tmpdir(), 'vi0c-images-'));
  const frames = {};
  for (const [i, id] of ['f555123', 'f650000'].entries()) {
    const path = join(dir, `${id}.png`);
    writeFileSync(path, Buffer.from([i, 2, 3]));
    frames[id] = { path };
  }
  const forward = mcpResult({ frames }, 'pair');
  const reverse = mcpResult({ frames }, 'reverse');
  assert.equal(forward.content[1].type, 'image');
  assert.equal(forward.content[1].mimeType, 'image/png');
  assert.deepEqual(Buffer.from(forward.content[1].data, 'base64'), Buffer.from([0, 2, 3]));
  assert.equal(forward.content[1].data, reverse.content[3].data);
  assert.equal(forward.content[3].data, reverse.content[1].data);
  assert.deepEqual(forward.content.filter(c => c.type === 'text'), [{ type: 'text', text: 'Image 1' }, { type: 'text', text: 'Image 2' }]);
  assert.throws(() => mcpResult({ frames }, 'missing'), /Unknown case/);
  assert.equal(cases.capacity8.length, 8);
});

test('capacity counts base64, all content blocks and complete JSON-RPC line', () => {
  const png = Buffer.alloc(24);
  Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]).copy(png);
  png.writeUInt32BE(1920, 16); png.writeUInt32BE(1080, 20);
  const result = { content: [{ type: 'text', text: 'Image 1' }, { type: 'image', mimeType: 'image/png', data: png.toString('base64') }], isError: false };
  const actual = measure(result, 12345);
  assert.equal(actual.images[0].png_bytes, png.length);
  assert.equal(actual.images[0].base64_bytes, 32);
  assert.equal(actual.images[0].width, 1920);
  assert.equal(actual.jsonrpc_line_bytes, Buffer.byteLength(JSON.stringify({ jsonrpc: '2.0', id: 12345, result }) + '\n'));
  assert.ok(actual.jsonrpc_line_bytes > actual.mcp_result_bytes);
});

test('checks reject swapped answers, cropped-out guesses, and success without image evidence', () => {
  const a = { text: 'x = torch.tensor([1e-8], dtype=torch.float16); assert x == 0; 0.000e+0' };
  const b = { text: 'x = torch.tensor([1e-8], dtype=torch.bfloat16); assert x != 0; 1.001e-8' };
  assert.ok(Object.values(visualChecks('pair', { images: [a, b] })).every(Boolean));
  assert.equal(visualChecks('reverse', { images: [a, b] }).image_1_dtype, false);
  assert.equal(visualChecks('crop_a', { images: [a] }).image_1_no_hidden_result, false);
  const dir = mkdtempSync(join(tmpdir(), 'vi0c-missing-evidence-'));
  writeFileSync(join(dir, 'run.json'), JSON.stringify({ exit_code: 0 }));
  const path = join(dir, 'frame.png');
  writeFileSync(path, 'fixture');
  const result = codexRecord({ frames: { f555123: { path } } }, 'single_a', dir);
  assert.equal(result.status, 'failed');
  assert.match(result.error, /ENOENT/);
  assert.throws(() => archive({ codex_all_passed: false }, dir), /requires all selected checks/);
});
