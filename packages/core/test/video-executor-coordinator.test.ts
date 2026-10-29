import { spawn } from 'node:child_process';
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { createInterface } from 'node:readline';
import { fileURLToPath } from 'node:url';
import { afterEach, expect, it } from 'vitest';
import { createVideoExecutorCoordinator } from '../../../skills/build/video-executor-coordinator';
import { createVideoTranslationSession, prepareVideoTranslation } from '../src/video-translation';

const roots: string[] = [];
afterEach(() => { for (const root of roots.splice(0)) rmSync(root, { recursive: true, force: true }); });

function translationFixture() {
  const workspace = mkdtempSync(path.join(tmpdir(), 'video-coordinator-stdio-')); roots.push(workspace);
  const { manifest } = JSON.parse(readFileSync(new URL('../../../crates/base-schema/fixtures/video-contract.json', import.meta.url), 'utf8')).cases[0];
  const count = 40;
  manifest.source.duration_ms = count * 1000;
  manifest.transcript_map_path = 'transcript_map.json';
  manifest.coverage = [{ capability: 'transcript', accepted: [{ video: manifest.material.video, start_ms: 0, end_ms: count * 1000 }], pending: [], failed: [] }];
  writeFileSync(path.join(workspace, 'video_source_manifest.json'), JSON.stringify(manifest));
  writeFileSync(path.join(workspace, 'transcript_map.json'), JSON.stringify({ transcript: manifest.material.transcript,
    segments: Array.from({ length: count }, (_, i) => ({ segment_id: `s${i}`, text: 'This is not overflow.',
      location: { video: manifest.material.video, start_ms: i * 1000, end_ms: (i + 1) * 1000 - 100 },
      text_range: { lid: String(i), range: { start: 0, end: 21 } }, words: [] })) }));
  return { workspace, plan: prepareVideoTranslation(workspace) };
}

async function bounded<T>(operation: Promise<T>, label: string): Promise<T> {
  let timer: ReturnType<typeof setTimeout>;
  try {
    return await Promise.race([operation, new Promise<never>((_, reject) => {
      timer = setTimeout(() => reject(new Error(`${label} timed out`)), 15_000);
    })]);
  } finally { clearTimeout(timer!); }
}

it.each(['video-translation', 'video-semantic-series'] as const)(
  '%s serve relays real MCP sessions through a subprocess and exits after stdin closes', async command => {
    const { workspace, plan } = translationFixture();
    expect(plan.tasks).toHaveLength(2);
    const coordinator = await createVideoExecutorCoordinator(path.join(workspace, '.build/coordinator'));
    const handoff = 'abhandoff1_' + (command === 'video-translation' ? 'a' : 'b').repeat(64);
    const handler = createVideoTranslationSession(workspace, 'zh-001', handoff);
    const exchanges: Array<{ request: any; response: any }> = [], handlerPids: number[] = [];
    const endpoint = coordinator.register(handoff, request => {
      handlerPids.push(process.pid);
      return handler(request);
    }, (request, response) => exchanges.push({ request, response }));
    // Deliberately absent child-side files prove that serve uses the parent session.
    const proxyWorkspace = path.join(workspace, 'proxy-workspace'), missingJobs = path.join(workspace, 'missing-jobs.json');
    const serveArgs = command === 'video-translation'
      ? ['serve', proxyWorkspace, 'parent-owned-group', handoff, endpoint]
      : ['serve', proxyWorkspace, 'discourse', missingJobs, handoff, endpoint];
    const repoRoot = fileURLToPath(new URL('../../../', import.meta.url));
    const binary = process.env.UNDERSTAND_BOOK_EXECUTOR_TEST_EXE;
    const child = spawn(binary ?? process.execPath,
      [...(binary ? [] : ['--import', 'tsx', 'skills/build/automatic-build.ts']), command, ...serveArgs], {
        cwd: repoRoot, windowsHide: true, stdio: ['pipe', 'pipe', 'pipe'],
        env: { ...process.env, UNDERSTAND_BOOK_AUTOMATIC_BUILD_DRIVER_ROOT: path.join(workspace, 'registry') },
      });
    let stderr = '', spawnError: Error | undefined;
    child.stderr.on('data', bytes => { stderr += bytes; });
    child.on('error', error => { spawnError = error; });
    const closed = new Promise<{ code: number | null; signal: NodeJS.Signals | null }>(resolve => {
      child.once('close', (code, signal) => resolve({ code, signal }));
    });
    const lines = createInterface({ input: child.stdout })[Symbol.asyncIterator]();
    let id = 0;
    const rpc = async (method: string, params: unknown = {}): Promise<any> => {
      const request = { jsonrpc: '2.0', id: ++id, method, params };
      child.stdin.write(JSON.stringify(request) + '\n');
      const line = await bounded(lines.next(), `${command} ${method}`);
      if (line.done) throw new Error(`Video proxy ended before response: ${spawnError?.message ?? stderr}`);
      const response = JSON.parse(line.value);
      expect(response.id).toBe(request.id);
      expect(response.error).toBeUndefined();
      return response.result;
    };
    const tool = async (name: string, args: unknown) => {
      const response = await rpc('tools/call', { name, arguments: args });
      expect(response.isError, JSON.stringify(response)).toBe(false);
      return JSON.parse(response.content[0].text).action;
    };
    try {
      expect(await rpc('initialize', { protocolVersion: '2025-06-18', capabilities: {}, clientInfo: { name: 'video-stdio-test', version: '1' } }))
        .toMatchObject({ protocolVersion: '2025-06-18', capabilities: { tools: { listChanged: false } } });
      child.stdin.write(JSON.stringify({ jsonrpc: '2.0', method: 'notifications/initialized' }) + '\n');
      const listed = await rpc('tools/list');
      expect(listed.tools.map((t: any) => t.name)).toEqual(['executor.open', 'executor.input.next', 'executor.generation.start', 'executor.submit_candidate']);
      let action = await tool('executor.open', { version: 'automatic_build_executor_open_request.v3', opaque_handoff_ref: handoff });
      let submitted = 0;
      while (action.kind !== 'DONE') {
        expect(action.kind).toBe('DELIVER_INPUT');
        let request = action.next_request, batch: any, text = '';
        do {
          action = await tool('executor.input.next', request); batch = action.batch;
          text += batch.chunks.filter((c: any) => c.segment === 'semantic_input').map((c: any) => c.payload_utf8).join('');
          request = { ...request, ack_through_ordinal: batch.last_ordinal };
        } while (!batch.final_for_generation);
        const source = JSON.parse(text);
        expect(source.required_segments.map((s: any) => s.segment_id)).toEqual(plan.tasks[submitted].segment_ids);
        action = await tool('executor.generation.start', { version: 'automatic_build_executor_generation_start_request.v3',
          opaque_session_ref: batch.opaque_session_ref, generation_input_ref: batch.generation_input_ref, confirmed_through_ordinal: batch.last_ordinal });
        expect(action.kind).toBe('GENERATE');
        action = await tool('executor.submit_candidate', { version: 'automatic_build_executor_candidate_submit.v3',
          opaque_session_ref: action.opaque_session_ref, candidate_sink_ref: action.candidate_sink_ref,
          candidate: { transcript: source.transcript, translations: source.required_segments.map((s: any) => ({ segment_id: s.segment_id, zh_text: '这不是上溢。' })) } });
        submitted++;
      }
      expect(action).toEqual({ kind: 'DONE', status: 'committed' });
      expect(submitted).toBe(2);
      expect(prepareVideoTranslation(workspace).tasks.map(t => ({ status: t.status, attempts: t.attempts })))
        .toEqual([{ status: 'accepted', attempts: 1 }, { status: 'accepted', attempts: 1 }]);
      for (const task of plan.tasks) {
        const accepted = JSON.parse(readFileSync(path.join(workspace, '.build/video-translation/accepted', task.id + '.json'), 'utf8'));
        expect(accepted.map((t: any) => t.segment_id)).toEqual(task.segment_ids);
        expect(accepted.every((t: any) => t.zh_text === '这不是上溢。')).toBe(true);
      }
      expect(new Set(handlerPids)).toEqual(new Set([process.pid]));
      expect(child.pid).not.toBe(process.pid);
      expect(exchanges.filter(e => e.request.method === 'notifications/initialized')).toEqual([
        { request: { jsonrpc: '2.0', method: 'notifications/initialized' }, response: undefined },
      ]);
      expect(exchanges.filter(e => e.response !== undefined)).toHaveLength(id);
      expect(existsSync(proxyWorkspace)).toBe(false); expect(existsSync(missingJobs)).toBe(false);
      child.stdin.end();
      expect(await bounded(closed, `${command} exit`), stderr).toEqual({ code: 0, signal: null });
      expect(await lines.next()).toEqual({ value: undefined, done: true });
      expect(spawnError).toBeUndefined(); expect(stderr).toBe('');
    } finally {
      if (child.exitCode === null && child.signalCode === null) child.kill();
      await bounded(closed, `${command} cleanup`);
      coordinator.unregister(handoff);
      await coordinator.close();
    }
  }, 45_000,
);
