// Opt-in probe: runs only when explicitly passed to run-installed.mjs.
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { homedir } from 'node:os';
import { join } from 'node:path';
import LocalCredentials from '@deepseek-ai/dsh-credentials-local';
import * as DeepSeek from '@deepseek-ai/dsh-llm-deepseek-api-key';
import { harness } from './harness.ts';
import { createExecutorHost } from '../src/executor-host.ts';
import { resolveExecutorRuntime } from '../src/executor-runtime.ts';

test('installed official provider completes a real Native four-tool roundtrip', { timeout: 180000 }, async () => {
  const h = await harness();
  let host;
  try {
    await h.ctx.plugin(LocalCredentials, { path: join(homedir(), '.dsh', '.credentials.yaml'), watch: false });
    h.ctx.loader.builtins['dh0-deepseek'] = DeepSeek;
    const id = await h.ctx.loader.create({ id: 'dh0-deepseek', name: 'cordis:dh0-deepseek', config: { reasoningEffort: 'off', maxTokens: 4096, retryPolicy: { mode: 'normal', maxRetries: 0 } } });
    await h.ctx.loader.resolve(id).fiber.await();
    const names = ['ub_executor_open', 'ub_executor_input_next', 'ub_executor_generation_start', 'ub_executor_submit_candidate'];
    const payload = 'DH0 合成材料；公式 $x=\\alpha$；代码 `const x = 1`。END-DH0';
    const calls = [];
    host = createExecutorHost(h.ctx, 'understand-book-executor', async binding => {
      for (const [index, name] of names.entries()) binding.agent.ctx.tools.register({
        name,
        description: `DH0 diagnostic step ${index + 1}. Call these tools once in order: ${names.join(', ')}. On final submit copy the exact payload from input_next into candidate. After submit stop.`,
        parameters: { type: 'object', additionalProperties: false, properties: index === 3 ? { candidate: { type: 'string' } } : {}, required: index === 3 ? ['candidate'] : [] },
        output: { schema: { type: 'object', additionalProperties: true }, render: (_args, value) => [{ type: 'text', text: JSON.stringify(value) }] },
        async execute(args, exec) {
          binding.assertCaller(exec.agent);
          assert.equal(index, calls.length);
          if (index === 3) assert.equal(args.candidate, payload);
          calls.push(name);
          return index === 1 ? { payload, next: names[2] } : { next: names[index + 1] ?? 'stop' };
        },
      });
    });
    const runtime = await resolveExecutorRuntime(h.ctx, { provider: 'deepseek-official', model: 'deepseek-v4-pro', reasoningEffort: 'off', maxTokens: 4096 }, 4096);
    const run = await host.launch({ parent: h.parent, signal: AbortSignal.timeout(150000), opaque_handoff_ref: `abhandoff1_${'f'.repeat(64)}`, runtime });
    const terminal = await run.terminal;
    assert.deepEqual(calls, names);
    assert.equal(terminal.stop_reason, 'completed');
    await run.dispose();
    assert.deepEqual(h.ctx.agents.list(), [h.parent]);
    console.log(JSON.stringify({ provider: 'deepseek-official', model: 'deepseek-v4-pro', reasoningEffort: 'off', payload_bytes: Buffer.byteLength(payload), tool_calls: calls.length, status: 'passed' }));
  } finally { await host?.dispose(); await h.dispose(); }
});
