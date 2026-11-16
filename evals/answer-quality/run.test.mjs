import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

test('saved output imports without a Reader publication or provider credentials and keeps original metrics', async () => {
  const directory = await fs.mkdtemp(path.join(os.tmpdir(), 'answer-quality-import-'));
  const saved = { answer: '实际交付的回答', materials: [{ content: '原材料' }], status: 'delivered',
    elapsed_ms: 12345, usage: { total_tokens: 234 } };
  await fs.writeFile(path.join(directory, 'answer.json'), JSON.stringify(saved));
  const cases = { version: 'saved-output-test.v1', cases: [{ id: 'T1', title: '导入',
    question: '原问题', assessment: '解释原材料', baseline_mode: 'knowledge', saved_product: 'answer.json' }] };
  await fs.writeFile(path.join(directory, 'cases.json'), JSON.stringify(cases));
  const out = path.join(directory, 'run');
  const result = spawnSync(process.execPath, [fileURLToPath(new URL('./run.mjs', import.meta.url)), 'run',
    '--cases', path.join(directory, 'cases.json'), '--arms', 'product', '--out', out], {
    encoding: 'utf8', windowsHide: true,
    env: { ...process.env, OPENCODE_BASE_URL: '', OPENCODE_API_KEY: '', FLUID_LLM_MODEL: '' },
  });
  assert.equal(result.status, 0, result.stderr);
  const run = JSON.parse(await fs.readFile(path.join(out, 'run.json'), 'utf8'));
  assert.equal(run.status, 'completed');
  assert.equal(run.configuration.product_executable, null);
  assert.equal(run.samples[0].answer, saved.answer);
  assert.equal(run.samples[0].original_elapsed_ms, 12345);
  assert.equal(run.samples[0].original_usage.total_tokens, 234);
  const report = await fs.readFile(path.join(out, 'report.md'), 'utf8');
  assert(report.includes('12.3（原运行）'));
  assert(report.includes('234'));
});
