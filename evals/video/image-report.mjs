import { readFileSync, existsSync, copyFileSync, mkdirSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { cases, output, root, mcpResult, measure, save } from './image-experiment.mjs';

function read(path) { return JSON.parse(readFileSync(path, 'utf8')); }
function answerJson(text) { return JSON.parse(text.trim().replace(/^```(?:json)?\s*/, '').replace(/\s*```$/, '')); }

// Exact observable facts from the independently reviewed VI0a sample contract.
// These checks are not a general semantic quality score.
export function visualChecks(id, answer) {
  const images = answer.images || [];
  const checks = { ordered_image_count: images.length === cases[id].length };
  for (const [index, frame] of cases[id].entries()) {
    const text = JSON.stringify(images[index] ?? null);
    const prefix = `image_${index + 1}`;
    if (frame.startsWith('f555123') || frame.startsWith('f650000')) {
      const a = frame.startsWith('f555123');
      checks[`${prefix}_dtype`] = text.includes(a ? 'dtype=torch.float16' : 'dtype=torch.bfloat16');
      checks[`${prefix}_operator`] = (a ? /assert x\s*==\s*0/ : /assert x\s*!=\s*0/).test(text);
      if (!frame.endsWith('_crop')) checks[`${prefix}_result`] = text.includes(a ? '0.000e+0' : '1.001e-8');
      else checks[`${prefix}_no_hidden_result`] = !/0\.000e\+0|1\.001e-8/.test(text);
    }
    if (['f600000', 'f615000', 'f630000'].includes(frame)) {
      const keyword = { f600000: /Google Brain/i, f615000: /dynamic range/i, f630000: /resolution.*worse/i }[frame];
      checks[`${prefix}_highlight`] = keyword.test(text);
    }
    if (frame === 'f1200000' || frame === 'f1210000') {
      checks[`${prefix}_signature`] = /seq1 hidden, hidden seq2\s*->\s*seq1 seq2/.test(text);
      checks[`${prefix}_input_shapes`] = /3\s*[,×x]\s*4/.test(text) && /4\s*[,×x]\s*3/.test(text);
    }
  }
  return checks;
}

export function codexRecord(manifest, id, dir) {
  const runFile = join(dir, 'run.json');
  if (!existsSync(runFile)) return { status: 'not_run' };
  const run = read(runFile);
  const expected = mcpResult(manifest, id);
  const checks = { process_success: run.exit_code === 0 && !run.timed_out };
  let answer = null;
  let transport = null;
  let error = null;
  let usage = null;
  try {
    const wire = read(join(dir, 'mcp-result.json'));
    checks.native_image_bytes_and_order = JSON.stringify(wire.result) === JSON.stringify(expected);
    transport = measure(wire.result, wire.id);
    const events = readFileSync(join(dir, 'events.jsonl'), 'utf8').trim().split('\n').map(JSON.parse);
    const calls = events.filter(e => e.item?.type === 'mcp_tool_call' && e.type === 'item.completed');
    checks.one_successful_image_call = calls.length === 1 && calls[0].item.server === 'vi0c' && calls[0].item.status === 'completed';
    checks.no_other_tool_access = events.filter(e => e.type === 'item.completed').every(e =>
      ['mcp_tool_call', 'agent_message', 'reasoning', 'error'].includes(e.item?.type));
    checks.turn_completed = events.some(e => e.type === 'turn.completed') && !events.some(e => e.type === 'turn.failed');
    usage = events.findLast(e => e.type === 'turn.completed')?.usage ?? null;
    answer = answerJson(readFileSync(join(dir, 'answer.txt'), 'utf8'));
    Object.assign(checks, visualChecks(id, answer));
  } catch (e) { error = e.message; }
  return { status: !error && Object.values(checks).every(Boolean) ? 'passed' : 'failed', ...run, checks, transport, usage, answer, error };
}

export function runtimeRecord(manifest, id, dir) {
  if (!existsSync(join(dir, 'run.json'))) return { status: 'not_run' };
  const run = read(join(dir, 'run.json'));
  const host = read(join(dir, 'host.json'));
  const request = read(join(dir, 'request.json'));
  const content = request.messages.at(-1).content;
  const expected = mcpResult(manifest, id).content;
  const projected = expected.map(c => c.type === 'text' ? c : { type: 'image_url', image_url: { url: `data:image/png;base64,${c.data}`, detail: 'high' } });
  const checks = { process_success: host.exit_code === 0 && run.error === null,
    full_body_measured: run.request_json_bytes === Buffer.byteLength(readFileSync(join(dir, 'request.json'))) };
  // Compare objects: Rust's sorted object keys need not match JS insertion order.
  checks.images_and_order_preserved = content.length === projected.length && content.every((c, index) =>
    c.type === projected[index].type && (c.type === 'text' ? c.text === projected[index].text :
      c.image_url?.url === projected[index].image_url.url && c.image_url?.detail === 'high'));
  if (host.mode === 'record') checks.actual_http_body_matches = readFileSync(join(dir, 'http-body.json'), 'utf8') === readFileSync(join(dir, 'request.json'), 'utf8');
  return { status: Object.values(checks).every(Boolean) ? 'projection_passed' : 'failed', ...host, ...run, checks };
}

export function report() {
  const manifest = read(join(output, 'inputs.json'));
  const codex = Object.fromEntries(Object.keys(cases).map(id => [id, codexRecord(manifest, id, join(output, 'codex', id))]));
  const runtime = Object.fromEntries(Object.keys(cases).map(id => [id, runtimeRecord(manifest, id, join(output, 'runtime-record', id))]));
  const result = { codex, runtime_record: runtime,
    runtime_live: { status: 'pending_configuration', authorization: '2026-10-10 user deferred real Reader Provider and DSH validation' },
    dsh: { status: 'pending_configuration', production_tool_render: 'text JSON.stringify(value)', semantic_inference: false },
    limits: 'Largest successful case is a tested lower bound; no provider hard maximum inferred.',
    price: { currency: null, amount: null, reason: 'No per-request monetary charge returned by Codex subscription host' },
    codex_all_passed: Object.values(codex).every(r => r.status === 'passed'),
    runtime_projection_all_passed: Object.values(runtime).every(r => r.status === 'projection_passed') };
  save(join(output, 'verification.json'), result);
  console.log(JSON.stringify({ codex: Object.fromEntries(Object.entries(codex).map(([id,r]) => [id, {status:r.status, failures:Object.entries(r.checks || {}).filter(([,v]) => !v).map(([k])=>k), error:r.error}])), runtime_projection_all_passed:result.runtime_projection_all_passed }));
  return result;
}

export function archive(result, destination) {
  if (!result.codex_all_passed || !result.runtime_projection_all_passed) throw new Error('Archive requires all selected checks to pass');
  mkdirSync(join(destination, 'answers'), { recursive: true });
  save(join(destination, 'verification.json'), result);
  copyFileSync(join(output, 'capacity.json'), join(destination, 'capacity.json'));
  const inputs = read(join(output, 'inputs.json'));
  inputs.source = { ...inputs.source, path: 'See samples.json source.path' };
  for (const frame of Object.values(inputs.frames)) {
    frame.path = frame.rect_pixels ? `vi0c/images/${frame.id}.png` : `frames/${frame.id}.png`;
  }
  save(join(destination, 'inputs.json'), { ...inputs, media_root: '../../generated' });
  for (const id of Object.keys(cases)) copyFileSync(join(output, 'codex', id, 'answer.txt'), join(destination, 'answers', `${id}.json`));
  const usage = Object.values(result.codex).reduce((totals, r) => {
    for (const [key, value] of Object.entries(r.usage || {})) totals[key] = (totals[key] || 0) + value;
    return totals;
  }, {});
  const attempts = [
    ['old-cli-single-a', 'service rejected model on CLI 0.147.0'],
    ['missing-annotation-single-a', 'read-only tool annotations missing; tool approval refused'],
    ['timeout-reverse', 'image tool completed; final model response absent at evaluation timeout'],
  ].filter(([id]) => existsSync(join(output, 'codex', id, 'run.json'))).map(([id, reason]) => {
    const dir = join(output, 'codex', id);
    const events = readFileSync(join(dir, 'events.jsonl'), 'utf8').trim().split('\n').map(JSON.parse);
    return { id, reason, ...read(join(dir, 'run.json')),
      usage: events.findLast(e => e.type === 'turn.completed')?.usage ?? null,
      interruption: existsSync(join(dir, 'interruption.json')) ? read(join(dir, 'interruption.json')) : null,
      errors: events.filter(e => e.type === 'error' || e.type === 'turn.failed').map(e => e.error || e.message) };
  });
  save(join(destination, 'attempts.json'), attempts);
  save(join(destination, 'summary.json'), {
    date: new Date().toLocaleDateString('en-CA', { timeZone: 'Asia/Hong_Kong' }),
    models: [...new Set(Object.values(result.codex).map(r => r.model))], reasoning_effort: 'low',
    host_versions: [...new Set(Object.values(result.codex).map(r => r.host_version).filter(Boolean))],
    passed_cases: Object.keys(cases).length, usage,
    successful_elapsed_ms: Object.values(result.codex).reduce((sum, r) => sum + r.elapsed_ms, 0),
    maximum_tested: result.codex.capacity8.transport,
    raw_wire_root: '../../generated/vi0c',
    other_attempts: 'attempts.json; their usage and elapsed time are separate from successful cases',
    runtime_live: result.runtime_live, dsh: result.dsh, price: result.price,
  });
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const result = report();
  if (!result.codex_all_passed || !result.runtime_projection_all_passed) process.exitCode = 1;
  else if (process.argv[2] === '--archive') archive(result, join(root, 'evals/video/results/vi0c-20261010'));
}
