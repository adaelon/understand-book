import assert from 'node:assert/strict';
import { test } from 'node:test';
import * as DeepSeek from '@deepseek-ai/dsh-llm-deepseek-api-key';
import { harness } from './harness.ts';
import { resolveExecutorRuntime } from '../src/executor-runtime.ts';

test('installed provider reports the exact model metadata without a model call', async () => {
  const h = await harness();
  try {
    h.ctx.loader.builtins['dh0-model-info'] = DeepSeek;
    const id = await h.ctx.loader.create({ id: 'dh0-model-info', name: 'cordis:dh0-model-info', config: {} });
    await h.ctx.loader.resolve(id).fiber.await();
    const info = await h.ctx.llm.resolveModelInfo('deepseek-official', 'deepseek-v4-pro');
    assert.equal(info.provider, 'deepseek-official');
    assert.equal(info.id, 'deepseek-v4-pro');
    const runtime = await resolveExecutorRuntime(h.ctx, { provider: 'deepseek-official', model: 'deepseek-v4-pro', reasoningEffort: 'off', maxTokens: 4096 }, 4096);
    assert.equal(runtime.context_window_tokens, info.context.contextWindow);
    assert.equal(runtime.max_output_tokens, 4096);
    assert.equal(runtime.reasoning_effort, 'off');
    assert.equal(Object.isFrozen(runtime), true);
    console.log(JSON.stringify(runtime));
  } finally { await h.dispose(); }
});
