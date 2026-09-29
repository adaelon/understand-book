import assert from 'node:assert/strict';
import { test } from 'node:test';
import { mkdtempSync, writeFileSync, existsSync, readdirSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import childProcess from 'node:child_process';
import { syncBuiltinESMExports } from 'node:module';
import { ToolCallId } from '@deepseek-ai/dsh-llm';
import { harness, ScriptedModel, toolResponse, textResponse } from './harness.ts';

const plugin = await import(pathToFileURL(process.env.UB_RELEASE_PLUGIN!).href);
const executable = process.env.UB_RELEASE_ENGINE!;
const children: childProcess.ChildProcess[] = [];
const spawn = childProcess.spawn;
childProcess.spawn = ((command: any, ...args: any[]) => {
  const child = (spawn as any)(command, ...args);
  if (command === executable && child.pid) children.push(child);
  return child;
}) as typeof spawn;
syncBuiltinESMExports();

class MetadataModel extends ScriptedModel {
  async resolveModel(provider: string, model: string) { return { provider, id: model, name: model, context: { contextWindow: 1000000 } }; }
}
function assertExited() {
  assert.ok(children.length > 0);
  for (const child of children) assert.ok(child.exitCode !== null || child.signalCode !== null, `Engine PID ${child.pid} still active`);
}
function objects(value: unknown): any[] {
  if (typeof value === 'string') { try { return [JSON.parse(value)]; } catch { return []; } }
  return value && typeof value === 'object' ? Object.values(value).flatMap(objects) : [];
}
async function tool(h: Awaited<ReturnType<typeof harness>>, name: string, args: object, signal = new AbortController().signal) {
  const result = await h.parent.ctx.tools.execute({ callId: ToolCallId(`release-${name}`), name, arguments: args, agent: h.parent, signal });
  if (signal.aborted) { assert.match(JSON.stringify(result), /ABORTED/); return { cancelled: true }; }
  assert.equal(result.isError, false, JSON.stringify(result));
  return objects(result).find(v => v.invocation_ref);
}

test('packed entry refuses missing and old Engine without creating invocations or tools', { timeout: 30000 }, async () => {
  const h = await harness();
  const registry = path.resolve('refused registry');
  try {
    for (const engine of [path.resolve('missing.exe'), process.env.UB_OLD_ENGINE!]) {
      await assert.rejects(async () => { await h.ctx.plugin(plugin, { executable: engine, driverRoot: registry }); }, /build_engine_(missing|incompatible)/);
      assert.equal(h.parent.ctx.tools.get('ub_build_run'), undefined);
      // Old Engines may retain their bounded unsupported-command diagnostic.
      assert.equal(existsSync(path.join(registry, 'invocations')), false);
    }
  } finally { await h.dispose(); }
});

for (const action of ['cancel', 'unload'] as const) test(`packed plugin ${action} drains native child and compiled MCP; re-enable preserves invocation`, { timeout: 60000 }, async () => {
  const root = mkdtempSync(path.resolve(`中文 ${action} `));
  const source = path.join(root, 'book transactions.md');
  writeFileSync(source, '# Transactions\n\nTransactions group operations into one atomic change.\n');
  let delivered!: () => void;
  const sawInput = new Promise<void>(resolve => { delivered = resolve; });
  const model = new MetadataModel(async function* (request) {
    assert.deepEqual(request.tools!.map(t => t.name).sort(), ['ub_executor_open', 'ub_executor_input_next', 'ub_executor_generation_start', 'ub_executor_submit_candidate'].sort());
    const response = objects(request.messages).filter(v => v.version === 'automatic_build_executor_session.v4').at(-1);
    if (!response) {
      const ref = JSON.stringify(request.messages).match(/abhandoff1_[a-f0-9]{64}/u)?.[0];
      assert.ok(ref);
      yield* toolResponse('open', 'ub_executor_open', { version: 'automatic_build_executor_open_request.v3', opaque_handoff_ref: ref });
    } else {
      assert.equal(response.action.kind, 'DELIVER_INPUT');
      delivered();
      await new Promise<void>(resolve => request.signal!.addEventListener('abort', () => resolve(), { once: true }));
      yield* textResponse('cancelled');
    }
  });
  const h = await harness(model);
  const config = { executable, driverRoot: path.join(root, 'registry'), maxOutputTokens: 4096, safetyMarginTokens: 4096, handoffsPerCall: 1 };
  let fiber = await h.ctx.plugin(plugin, config);
  const answers = h.ctx.on('user-questions/request', async request => ({ answers: [{ id: request.questions[0].id, selected: ['批准'] }] }));
  try {
    const invocation = await tool(h, 'ub_build_prepare_and_confirm', { target_input: source, root_dir: root, pass2: 'disabled' });
    assert.ok(invocation.invocation_ref);
    const signal = new AbortController();
    const running = tool(h, 'ub_build_run', { invocation_ref: invocation.invocation_ref }, signal.signal);
    await sawInput;
    if (action === 'cancel') signal.abort(); else await fiber.dispose();
    const result = await running;
    if (action === 'cancel') assert.equal(result.cancelled, true);
    else assert.equal(result.host_observation.code, 'build_cancelled');
    assert.deepEqual(h.ctx.agents.list(), [h.parent]);
    assertExited();
    if (action === 'cancel') await fiber.dispose();
    assert.equal(h.parent.ctx.tools.get('ub_build_run'), undefined);
    assert.ok(readdirSync(config.driverRoot).length > 0);
    fiber = await h.ctx.plugin(plugin, config);
    assert.ok(h.parent.ctx.tools.get('ub_build_run'));
    assertExited();
    console.log(JSON.stringify({ action, engine_processes_exited: children.length, invocation_preserved: invocation.invocation_ref, root }));
  } finally { answers(); await fiber.dispose(); await h.dispose(); }
});
