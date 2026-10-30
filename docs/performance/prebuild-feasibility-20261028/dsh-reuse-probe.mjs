import assert from 'node:assert/strict';
import { harness } from '../../../packages/dsh-plugin/test/harness.ts';
import { SessionId } from '@deepseek-ai/dsh-session';
import { createUserMessage } from '@deepseek-ai/dsh-llm';

// Run from the repository root:
// node --import tsx docs/performance/prebuild-feasibility-20261028/dsh-reuse-probe.mjs
// This uses the local ScriptedModel harness. It makes no real provider calls.
const h = await harness();
try {
  const handle = await h.ctx.agents.create({
    sessionId: SessionId('resident-feasibility-probe'),
    parentAgent: h.parent,
    meta: { parentSession: h.parent.id, origin: 'subagent', delegationDepth: 1 },
    agentOptions: { provider: 'probe', model: 'scripted', maxTokens: 1000 },
    setup: async (ctx) => {
      await h.ctx.agentPresets.mount(ctx, 'understand-book-executor');
      ctx.tools.presentAs('native');
      ctx.tools.register({
        name: 'resident_probe',
        description: 'Local test only',
        parameters: { type: 'object', properties: {} },
        output: { schema: {}, render: () => [] },
        async execute() { return { ok: true }; },
      });
    },
  });
  const child = handle.agent;
  for (const text of ['first-unit-local-probe', 'second-unit-local-probe']) {
    child.followup(createUserMessage({
      content: [{ type: 'text', text }],
      source: { kind: 'user' },
    }));
    await child.whenIdle();
    assert.equal(h.ctx.agents.get(child.id), child);
    assert.ok(h.ctx.tools.get('resident_probe', child));
  }
  assert.equal(h.model.requests.length, 2);
  assert.ok(JSON.stringify(h.model.requests[1].messages).includes('first-unit-local-probe'));
  assert.ok(JSON.stringify(h.model.requests[1].messages).includes('second-unit-local-probe'));
  const result = {
    same_child_for_two_turns: true,
    calls: h.model.requests.length,
    scoped_tool_persists: true,
    context_accumulates: true,
  };
  await handle.dispose();
  assert.equal(h.ctx.agents.get(child.id), undefined);
  console.log(JSON.stringify({ ...result, disposed_cleanly: true }));
} finally {
  await h.dispose();
}
