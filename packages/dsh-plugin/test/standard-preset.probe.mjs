// Explicit installed-runtime probe; no user profile or book is opened.
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { mkdtempSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { join, resolve } from 'node:path';
import { boot, loadOverlayPatches } from '@deepseek-ai/dsh-app-boot';
import { provideCmdline } from '@deepseek-ai/dsh-cmdline';
import { SessionId } from '@deepseek-ai/dsh-session';
import { probeRuntime, ScriptedModel, textResponse, toolResponse } from './harness.ts';
import { createExecutorHost } from '../src/executor-host.ts';

test('installed standard preset switches to Native executor without retaining compaction or parent tools', { timeout: 60000 }, async () => {
  const dir = mkdtempSync(resolve('../../tmp/dh0/standard-'));
  process.env.DSH_HOME = join(dir, 'home');
  const config = join(dir, 'cordis.yml');
  writeFileSync(config, '[]\n');
  const patches = [
    '@deepseek-ai/dsh-base/cordis.patch.yml',
    '@deepseek-ai/dsh-web-app/cordis.patch.yml',
    '@deepseek-ai/dsh-web-app/presets/standard.patch.yml',
  ].flatMap(specifier => loadOverlayPatches('dh0', fileURLToPath(import.meta.resolve(specifier))));
  const ctx = await boot('dh0', config, patches, ctx => {
    provideCmdline(ctx, { args: ['--no-open', '--port', '0'], exit: code => { throw new Error(`startup exit ${code}`); } });
  });
  let host;
  try {
    await ctx.agentPresets.register({ id: 'understand-book-executor', plugins: [] });
    const names = ['ub_executor_open', 'ub_executor_input_next', 'ub_executor_generation_start', 'ub_executor_submit_candidate'];
    const payload = '中文公式 $x=\\alpha$\r\n'.repeat(500) + 'END-STANDARD-INPUT';
    const containsCompletePayload = value => {
      if (typeof value === 'string') {
        try { return JSON.parse(value)?.payload === payload; } catch { return false; }
      }
      return !!value && typeof value === 'object' && Object.values(value).some(containsCompletePayload);
    };
    const candidate = { text: payload.repeat(3), nested: { escaped: '\"\\\r\n' } };
    let step = 0;
    let submitted = false;
    const model = new ScriptedModel(async function* (request) {
      assert.deepEqual(request.tools.map(t => t.name).sort(), [...names].sort());
      if (step > 0) assert.ok(containsCompletePayload(request.messages));
      if (step < 4) {
        const index = step++;
        yield* toolResponse(`standard-${index}`, names[index], index === 3 ? { candidate } : {});
      }
      else yield* textResponse('PRIVATE-FINAL-STANDARD');
    });
    ctx.llm.registerAdapter(['probe'], model);
    const parent = (await ctx.agents.create({ sessionId: SessionId(`dh0-standard-${Date.now()}`), meta: { cwd: dir },
      agentOptions: { provider: 'probe', model: 'scripted' },
      setup: agentCtx => ctx.agentPresets.mount(agentCtx, 'standard').then(() => {}),
    })).agent;
    const inventory = await ctx.agentPresets.compositionInventory();
    for (const id of ['compaction-basic', 'tool-result-pruner']) {
      assert.ok(inventory.find(p => p.id === 'standard').rows.some(row => row.entryId === id && row.enabled && row.fiberState === 2));
    }
    parent.ctx.tools.presentAs('both');
    host = createExecutorHost(ctx, 'understand-book-executor', async binding => {
      assert.equal(binding.agent.ctx.get('compaction'), undefined);
      assert.equal(binding.agent.ctx.get('toolResultPruner'), undefined);
      for (const name of names) binding.agent.ctx.tools.register({ name, description: 'DH0 synthetic operation',
        parameters: { type: 'object', properties: { candidate: { type: 'object' } }, additionalProperties: false },
        output: { schema: { type: 'object', additionalProperties: true }, render: (_args, value) => [{ type: 'text', text: JSON.stringify(value) }] },
        async execute(args, exec) {
          binding.assertCaller(exec.agent);
          if (name === names[3]) { assert.deepEqual(args, { candidate }); submitted = true; }
          return { payload };
        },
      });
    });
    const run = await host.launch({ parent, signal: AbortSignal.timeout(30000),
      opaque_handoff_ref: `abhandoff1_${'9'.repeat(64)}`, runtime: probeRuntime({ max_output_tokens: 4096 }) });
    const terminal = await run.terminal;
    if (terminal.stop_reason !== 'completed') console.log(JSON.stringify(run.binding.agent.session.snapshotEvents().filter(e => e.type.includes('error') || e.type.includes('end'))));
    assert.equal(terminal.stop_reason, 'completed');
    assert.equal(model.requests.length, 5);
    assert.equal(submitted, true);
    assert.equal(ctx.agentPresets.composedPreset(parent.ctx), 'standard');
    assert.equal(JSON.stringify(parent.session.snapshotEvents()).includes('PRIVATE-FINAL-STANDARD'), false);
    await run.dispose();
    console.log(JSON.stringify({ preset: 'shipped-standard', payload_bytes: Buffer.byteLength(payload),
      candidate_request_bytes: Buffer.byteLength(JSON.stringify({ candidate })), requests: model.requests.length, status: 'passed' }));
  } finally { await host?.dispose(); await ctx.fiber.dispose(); }
});
