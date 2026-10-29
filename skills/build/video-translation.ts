/** Source-mode video translation driver. Usage:
 * node --import tsx skills/build/video-translation.ts run WORKSPACE CODEX_EXE
 * Resume the same command to retry only missing work. */
import { appendFileSync, existsSync, mkdirSync, readFileSync, writeFileSync, realpathSync } from 'node:fs';
import { execFile } from 'node:child_process';
import { promisify, isDeepStrictEqual } from 'node:util';
import { randomBytes } from 'node:crypto';
import { createInterface } from 'node:readline';
import { fileURLToPath } from 'node:url';
import path from 'node:path';
import { videoExecutorArgs, videoExecutorPrompt, videoExecutorStorageArgs, runVideoCodex, type VideoExecutorControl } from './video-executor-runtime';
import { createBuildExecutorMcpSession } from './build-executor-mcp';
import { BUILD_EXECUTOR_BOOTSTRAP_CONTRACT_V3 as bootstrap } from '../../packages/core/src/build-executor-connection-capability';
import { createVideoTranslationSession, failVideoTranslation, prepareVideoTranslation, publishVideoTranslationGroup } from '../../packages/core/src/video-translation';
import { createVideoExecutorCoordinator, proxyVideoExecutor, runVideoExecutorPool } from './video-executor-coordinator';
import { sameVideoEnrichments } from '../../packages/core/src/video-enrichment';

const script = fileURLToPath(import.meta.url);
export async function publishVideoTranslationSnapshot(workspace: string, publisher: string, serviceRoot: string, signal?: AbortSignal) {
  const linkFile = path.join(workspace, 'video_publication.json');
  if (existsSync(linkFile)) {
    const link = JSON.parse(readFileSync(linkFile, 'utf8')), directory = path.resolve(workspace, link.directory);
    // A lost publisher reply is recovered from its durable link, without copying
    // the same complete video again on every resumed invocation.
    const destination = path.resolve(serviceRoot, 'library/published', link.reference.book_id, link.reference.publication_id);
    if (existsSync(destination) && realpathSync.native(directory) === realpathSync.native(destination) && ['video_enrichments.json', 'video_translation_progress.json'].every(file =>
      existsSync(path.join(directory, file)) && existsSync(path.join(workspace, file))
      && (file === 'video_enrichments.json' ? sameVideoEnrichments : isDeepStrictEqual)(JSON.parse(readFileSync(path.join(directory, file), 'utf8')), JSON.parse(readFileSync(path.join(workspace, file), 'utf8')))))
      return { reused: true, reference: link.reference, elapsed_ms: 0 };
  }
  const started = Date.now();
  const { stdout } = await promisify(execFile)(publisher, [path.resolve(serviceRoot), 'import', path.resolve(workspace)], { windowsHide: true, maxBuffer: 8 * 1024 * 1024, signal });
  const manifest = JSON.parse(stdout);
  return { reused: false, reference: manifest.reference, elapsed_ms: Date.now() - started, files: manifest.files };
}
export async function runVideoTranslation(workspace: string, executable: string, publish?: (group: string) => Promise<void>, control: VideoExecutorControl = {}) {
  workspace = path.resolve(workspace);
  const plan = prepareVideoTranslation(workspace);
  const groups = [...new Set(plan.tasks.map(t => t.group))];
  const coordinator = plan.tasks.some(t => t.status !== 'accepted')
    ? await createVideoExecutorCoordinator(path.join(workspace, '.build/video-translation')) : undefined;
  try {
    // A fully accepted resume only republishes snapshots; keep those serial even
    // when no model coordinator is needed.
    await runVideoExecutorPool(groups, coordinator ? control.max_parallel ?? 2 : 1, control, async (group, workerControl) => {
      const current = prepareVideoTranslation(workspace);
      if (current.tasks.some(t => t.group === group && t.status !== 'accepted')) {
        const started = Date.now(), handoff = 'abhandoff1_' + randomBytes(32).toString('hex');
        const run = path.join(workspace, '.build/video-translation/runs', group, String(started)); mkdirSync(run, { recursive: true });
        const endpoint = coordinator!.register(handoff, createVideoTranslationSession(workspace, group, handoff), (request, response) =>
          logTranslationExchange(workspace, group, request, response));
        const args = ['exec', '--ignore-user-config', '--skip-git-repo-check', '--ephemeral', '-C', run, ...videoExecutorStorageArgs(workspace),
          '-c', 'model_reasoning_effort="low"',
          '-c', 'features.shell_tool=false', '-c', 'features.apps=false',
          '-c', 'developer_instructions=' + JSON.stringify(videoExecutorPrompt()),
          '-c', 'mcp_servers.understand_book_build_executor.command=' + JSON.stringify(process.execPath),
          '-c', 'mcp_servers.understand_book_build_executor.args=' + JSON.stringify(videoExecutorArgs('video-translation', script, ['serve', workspace, group, handoff, endpoint])),
          '-c', 'mcp_servers.understand_book_build_executor.required=true',
          '-c', 'mcp_servers.understand_book_build_executor.default_tools_approval_mode="approve"',
          '--json', '-o', path.join(run, 'lifecycle.txt'), handoff];
        try {
          await runVideoCodex({ executable, args, run_dir: run,
            logical_call_id: `video:${workspace}:translation:${group}`,
            complete: () => prepareVideoTranslation(workspace).tasks.filter(t => t.group === group).every(t => t.status === 'accepted') }, workerControl);
        } catch (error) {
          await coordinator!.runExclusive(() => failVideoTranslation(workspace, group, (error as Error).message));
          throw error;
        } finally { coordinator!.unregister(handoff); }
      }
      const publishGroup = async () => {
        workerControl.signal?.throwIfAborted();
        publishVideoTranslationGroup(workspace, group);
        await workerControl.onProgress?.();
        await publish?.(group);
        console.log(JSON.stringify({ group, status: 'accepted', segments: current.tasks.filter(t => t.group === group).flatMap(t => t.segment_ids).length }));
      };
      // An external snapshot may read several files asynchronously. Hold the same
      // writer queue until it finishes so another group's accept cannot change them.
      if (coordinator) await coordinator.runExclusive(publishGroup); else await publishGroup();
    });
  } finally { await coordinator?.close(); }
}

function logTranslationExchange(workspace: string, group: string, request: unknown, response: unknown) {
  const r = request as { method?: string; params?: { name?: string } };
  const rpc = response as { result?: { isError?: boolean; content?: Array<{ text?: string }> } } | undefined;
  appendFileSync(path.join(workspace, '.build/video-translation/mcp-events.jsonl'), JSON.stringify({ at: new Date().toISOString(), group,
    method: r.method, tool: r.params?.name, isError: rpc?.result?.isError ?? false,
    ...(rpc?.result?.isError ? { diagnostic: rpc.result.content?.[0]?.text } : {}) }) + '\n');

}

export async function runVideoTranslationCli(argv: string[]) {
  const [mode, workspace, argument, handoff, serviceRoot] = argv;
  if (mode === 'serve') {
    if (serviceRoot) { await proxyVideoExecutor(serviceRoot); return; }
    const execute = createVideoTranslationSession(path.resolve(workspace), argument, handoff);
    const session = createBuildExecutorMcpSession({ bootstrap_version: bootstrap.version, protocol_generation: bootstrap.session_protocol,
      session_private_root: path.resolve(workspace, '.build/video-translation'), execute_request: execute });
    for await (const line of createInterface({ input: process.stdin })) {
      const request = JSON.parse(line);
      const response = await session.handle_message(request);
      logTranslationExchange(workspace, argument, request, response);
      if (response !== undefined) process.stdout.write(JSON.stringify(response) + '\n');
    }
  } else if (mode === 'run' && workspace && argument) {
    if (!existsSync(argument)) throw new Error('Provide the Codex executable path');
    await runVideoTranslation(workspace, path.resolve(argument), handoff && serviceRoot
      ? async group => { const result = await publishVideoTranslationSnapshot(path.resolve(workspace), path.resolve(handoff), serviceRoot);
        appendFileSync(path.join(workspace, '.build/video-translation/publications.jsonl'), JSON.stringify({ group, ...result }) + '\n'); } : undefined);
  } else throw new Error('Expected run WORKSPACE CODEX_EXE');
}
if (process.argv[1] && path.resolve(process.argv[1]) === script) await runVideoTranslationCli(process.argv.slice(2));
