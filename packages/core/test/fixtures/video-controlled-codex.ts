/** Offline acceptance executor: real native MCP/candidate admission, deterministic fixture semantics.
 * This verifies orchestration and billing observations, not model understanding quality. */
import { spawn } from 'node:child_process';
import { appendFileSync, writeFileSync } from 'node:fs';
import { createInterface } from 'node:readline';

const argv = process.argv.slice(2);
if (argv.includes('--version')) { console.log('controlled-video-executor acceptance-only'); process.exit(0); }
const config = (key: string): any => {
  const value = argv.find(arg => arg.startsWith(key + '='));
  if (!value) throw new Error(`Missing ${key}`);
  return JSON.parse(value.slice(key.length + 1));
};
const command: string = config('mcp_servers.understand_book_build_executor.command');
const args: string[] = config('mcp_servers.understand_book_build_executor.args');
const child = spawn(command, args, { windowsHide: true, stdio: ['pipe', 'pipe', 'pipe'] });
child.stderr.pipe(process.stderr);
let sequence = 0, imageCount = 0, generations = 0;
const requests = new Map<number, { resolve: (value: any) => void; reject: (error: Error) => void }>();
createInterface({ input: child.stdout }).on('line', line => {
  const rpc = JSON.parse(line), waiting = requests.get(rpc.id);
  if (!waiting) return;
  requests.delete(rpc.id);
  if (rpc.error || rpc.result?.isError) waiting.reject(new Error(JSON.stringify(rpc)));
  else { imageCount += rpc.result?.content?.filter((c: any) => c.type === 'image').length ?? 0;
    waiting.resolve(rpc.result?.content ? JSON.parse(rpc.result.content[0].text) : rpc.result); }
});
child.on('error', error => { for (const request of requests.values()) request.reject(error); });
child.on('close', code => { for (const request of requests.values()) request.reject(new Error(`MCP closed ${code}`)); });
const call = (name: string, arguments_: unknown): Promise<any> => new Promise((resolve, reject) => {
  const id = ++sequence; requests.set(id, { resolve, reject });
  child.stdin.write(JSON.stringify({ jsonrpc: '2.0', id, method: 'tools/call', params: { name, arguments: arguments_ } }) + '\n');
});

function candidate(contract: any, input: any) {
  if (contract.version === 'video_translation_candidate.v1') return { transcript: input.transcript,
    translations: input.required_segments.map((segment: any) => ({ segment_id: segment.segment_id, zh_text: `验收字幕：${segment.text}` })) };
  if (contract.version === 'video_visual_candidate.v1') return { input_ref: input.input_ref, task_id: input.task_id, events: [{
    start_ms: input.location.start_ms, end_ms: input.location.end_ms, frame_ids: input.required_frame_ids,
    screen_text: '', description: '受控验收画面', change: '用于验证原生图片交付与候选准入',
  }] };
  const members = [
    ...(input.transcript?.length ? [{ kind: 'text', segment_id: input.transcript[0].segment_id, support_role: '验收转写支持' }] : []),
    ...(input.images ?? []).map((image: any) => ({ kind: 'visual', image_id: image.image_id, support_role: '验收画面支持' })),
  ];
  const support = { mode: members.some(m => m.kind === 'text') ? members.some(m => m.kind === 'visual') ? 'joint' : 'text' : 'visual', members };
  if (contract.version === 'video_graph_candidate.v1') return { input_ref: input.input_ref, nodes: [{ local_id: 'fixture',
    type: 'concept', name: '验收概念', meaning: '仅用于验证引用与发布的受控样本', same_as: null, support }], edges: [] };
  if (contract.version === 'video_discourse_candidate.v1') return { input_ref: input.input_ref, items: [{ local_id: 'fixture',
    mode: 'informative', local_function: 'explanation', local_summary: '原讲解结构的受控验收样本', vu_ids: input.vu_ids, support, relations: [] }] };
  throw new Error(`Unsupported fixture candidate ${contract.version}`);
}

try {
  console.log(JSON.stringify({ type: 'turn.started' }));
  let response = await call('executor.open', { version: 'automatic_build_executor_open_request.v3', opaque_handoff_ref: argv.at(-1) });
  let text = '';
  while (response.action.kind !== 'DONE') {
    const action = response.action;
    if (action.kind === 'DELIVER_INPUT') { text = ''; response = await call('executor.input.next', action.next_request); }
    else if (action.kind === 'INPUT_BATCH') {
      const batch = action.batch;
      text += batch.chunks.filter((chunk: any) => chunk.segment === 'semantic_input').map((chunk: any) => chunk.payload_utf8).join('');
      const request = { opaque_session_ref: batch.opaque_session_ref, generation_input_ref: batch.generation_input_ref };
      response = batch.final_for_generation
        ? await call('executor.generation.start', { version: 'automatic_build_executor_generation_start_request.v3', ...request, confirmed_through_ordinal: batch.last_ordinal })
        : await call('executor.input.next', { version: 'automatic_build_executor_input_next_request.v4', ...request, ack_through_ordinal: batch.last_ordinal });
    } else if (action.kind === 'GENERATE') {
      if (action.retry_feedback) throw new Error(JSON.stringify(action.retry_feedback));
      generations++;
      response = await call('executor.submit_candidate', { version: 'automatic_build_executor_candidate_submit.v3',
        opaque_session_ref: action.opaque_session_ref, candidate_sink_ref: action.candidate_sink_ref,
        candidate: candidate(action.output_contract, JSON.parse(text)) });
      console.log(JSON.stringify({ type: 'item.completed', item: { type: 'mcp_tool_call', tool: 'executor.submit_candidate', status: 'completed' } }));
    } else throw new Error(`Unexpected ${action.kind}`);
  }
  if (response.action.status !== 'committed') throw new Error('Candidate not committed');
  const record = { args, generations, images: imageCount, status: 'committed' };
  if (process.env.VIDEO_CONTROLLED_CODEX_LOG) appendFileSync(process.env.VIDEO_CONTROLLED_CODEX_LOG, JSON.stringify(record) + '\n');
  const lifecycle = argv[argv.indexOf('-o') + 1];
  if (lifecycle) writeFileSync(lifecycle, JSON.stringify({ status: 'committed' }));
  console.log(JSON.stringify({ type: 'turn.completed', usage: { input_tokens: generations * 13, cached_input_tokens: 0, output_tokens: generations * 7 } }));
} catch (error) { console.error(error); console.log(JSON.stringify({ type: 'turn.failed' })); process.exitCode = 1; }
finally { child.stdin.end(); }
