import assert from 'node:assert/strict';
import { once } from 'node:events';
import { writeFileSync } from 'node:fs';
import { PassThrough } from 'node:stream';
import { fileURLToPath } from 'node:url';
import { createBuildExecutorMcpSession, serveBuildExecutorMcpSession,
  BUILD_EXECUTOR_BOOTSTRAP_CONTRACT_V3 as bootstrap } from '../../../skills/build/build-executor-mcp.ts';

// Exercise the real asynchronous transport; the handler supplies controlled terminal actions.
// No Build Engine workspace, model, or paid provider is invoked.
function transport(execute_request: () => any) {
  const session = createBuildExecutorMcpSession({ bootstrap_version: bootstrap.version,
    protocol_generation: bootstrap.session_protocol, session_private_root: process.cwd(), execute_request });
  const input = new PassThrough(), output = new PassThrough();
  serveBuildExecutorMcpSession(session, input, output);
  let id = 0;
  return { input, output, async send(ref = `abhandoff1_${'a'.repeat(64)}`) {
    const data = once(output, 'data');
    input.write(JSON.stringify({ jsonrpc: '2.0', id: ++id, method: 'tools/call', params: {
      name: 'executor.open', arguments: { version: 'automatic_build_executor_open_request.v3', opaque_handoff_ref: ref },
    } }) + '\n');
    const [bytes] = await data;
    return JSON.parse(JSON.parse(bytes.toString()).result.content[0].text);
  } };
}

const results: Array<Record<string, unknown>> = [];
const deadline = setTimeout(() => { console.error('transport probe timed out'); process.exit(1); }, 5000);
try {
  for (const status of ['committed', 'retryable_failure', 'interrupted'] as const) {
    const t = transport(() => ({ version: 'automatic_build_executor_session.v3', action: { kind: 'DONE', status } }));
    const ended = once(t.output, 'finish');
    const response = await t.send(); await ended;
    assert.deepEqual(response.action, { kind: 'DONE', status });
    assert.equal(t.input.destroyed, true);
    results.push({ case: status, complete_response: true, input_closed: true });
  }
  {
    const t = transport(() => { throw new Error('controlled handler failure'); });
    const ended = once(t.output, 'finish');
    const response = await t.send(); await ended;
    assert.equal(response.diagnostic_code, 'executor_internal');
    assert.equal(t.input.destroyed, true);
    results.push({ case: 'terminal_error', complete_response: true, input_closed: true });
  }
  {
    const t = transport(() => ({ version: 'automatic_build_executor_session.v3', action: { kind: 'WAIT', retry_after_ms: 1 } }));
    assert.equal((await t.send()).action.kind, 'WAIT');
    assert.equal((await t.send()).action.kind, 'WAIT');
    assert.equal(t.input.destroyed, false);
    assert.equal(t.output.writableEnded, false);
    t.input.destroy(); t.output.destroy();
    results.push({ case: 'wait', second_call_accepted: true, kept_open: true });
  }
  {
    const t = transport(() => ({ version: 'automatic_build_executor_session.v3', action: { kind: 'DONE', status: 'committed' } }));
    assert.equal((await t.send('abhandoff1_mistyped')).diagnostic_code, 'invalid_arguments');
    assert.equal(t.input.destroyed, false);
    const ended = once(t.output, 'finish');
    assert.equal((await t.send()).action.status, 'committed'); await ended;
    assert.equal(t.input.destroyed, true);
    results.push({ case: 'unbound_correction', corrected: true, input_closed: true });
  }
  const report = { scope: 'real MCP asynchronous stream transport with controlled handler',
    paid_provider_calls: 0, passed: results.length, results };
  writeFileSync(fileURLToPath(new URL('./mcp-lifecycle-result.json', import.meta.url)), JSON.stringify(report, null, 2) + '\n');
  console.log(JSON.stringify(report));
} finally { clearTimeout(deadline); }
