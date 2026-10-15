import { appendFileSync, existsSync, mkdirSync, readFileSync, realpathSync, writeFileSync } from 'node:fs';
import { spawn, execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { randomBytes } from 'node:crypto';
import { createInterface } from 'node:readline';
import { fileURLToPath } from 'node:url';
import path from 'node:path';
import { videoExecutorArgs, videoExecutorPrompt, videoExecutorStorageArgs } from './video-executor-runtime';
import { createBuildExecutorMcpSession } from './build-executor-mcp';
import { BUILD_EXECUTOR_BOOTSTRAP_CONTRACT_V3 as bootstrap } from '../../packages/core/src/build-executor-connection-capability';
import { createVideoVisualSession, failVideoVisualTask, prepareVideoVisualUnits, readVideoVisualPlan, publishVideoVisualUnits } from '../../packages/core/src/video-visual-units';
import type { VisualScanOptions } from '../../packages/core/src/video-visual-scan';
import type { MediaLocation } from '../../packages/core/src/generated/MediaLocation';
import { sameVideoEnrichments } from '../../packages/core/src/video-enrichment';
const script = fileURLToPath(import.meta.url);
export async function publishVideoVisualSnapshot(workspace: string, publisher: string, service: string) {
  const linkFile = path.join(workspace, 'video_publication.json');
  if (existsSync(linkFile)) {
    const link = JSON.parse(readFileSync(linkFile, 'utf8')), destination = path.resolve(service, 'library/published', link.reference.book_id, link.reference.publication_id);
    const catalog = path.join(destination, 'video_enrichments.json');
    if (existsSync(catalog) && realpathSync.native(path.resolve(workspace, link.directory)) === realpathSync.native(destination)
      && sameVideoEnrichments(JSON.parse(readFileSync(catalog, 'utf8')), JSON.parse(readFileSync(path.join(workspace, 'video_enrichments.json'), 'utf8'))))
      return { reused: true, reference: link.reference, elapsed_ms: 0 };
  }
  const started = Date.now();
  const { stdout } = await promisify(execFile)(publisher, [path.resolve(service), 'import', path.resolve(workspace)], { windowsHide: true, maxBuffer: 8 * 1024 * 1024 });
  const manifest = JSON.parse(stdout);
  return { reused: false, reference: manifest.reference, elapsed_ms: Date.now() - started, files: manifest.files };
}
export async function runVideoVisualUnits(workspace: string, analysis: string, executable: string, publish?: () => Promise<void>) {
  workspace = path.resolve(workspace);
  const plan = readVideoVisualPlan(workspace, analysis);
  for (let group = 0; group < Math.ceil(plan.tasks.length / 6); group++) {
    const current = readVideoVisualPlan(workspace, analysis);
    if (current.tasks.slice(group * 6, (group + 1) * 6).some(t => t.status !== 'accepted')) {
      const started = Date.now(), handoff = 'abhandoff1_' + randomBytes(32).toString('hex');
      const run = path.join(workspace, '.build/video-visual', analysis, 'runs', String(group), String(started)); mkdirSync(run, { recursive: true });
      const role = videoExecutorPrompt();
      const args = ['exec', '--ignore-user-config', '--skip-git-repo-check', '--ephemeral', '-C', run, ...videoExecutorStorageArgs(workspace),
        '-c', 'model_reasoning_effort="low"', '-c', 'features.shell_tool=false', '-c', 'features.apps=false',
        '-c', 'developer_instructions=' + JSON.stringify(role),
        '-c', 'mcp_servers.understand_book_build_executor.command=' + JSON.stringify(process.execPath),
        '-c', 'mcp_servers.understand_book_build_executor.args=' + JSON.stringify(videoExecutorArgs('video-visual-units', script, ['serve', workspace, analysis, handoff, String(group)])),
        '-c', 'mcp_servers.understand_book_build_executor.required=true',
        '-c', 'mcp_servers.understand_book_build_executor.default_tools_approval_mode="approve"',
        '--json', '-o', path.join(run, 'lifecycle.txt'), handoff];
      const child = spawn(executable, args, { windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'] });
      const events: unknown[] = [];
      createInterface({ input: child.stdout }).on('line', line => { try {
        const e = JSON.parse(line); if (['turn.completed', 'turn.failed', 'error'].includes(e.type)) events.push(e);
        else if (e.item) events.push({ type: e.type, item_type: e.item.type, tool: e.item.tool, status: e.item.status });
      } catch { /* CLI non-JSON diagnostic */ } });
      child.stderr.on('data', bytes => appendFileSync(path.join(run, 'stderr.txt'), bytes));
      try {
        const exit = await new Promise<number | null>((resolve, reject) => { child.on('close', resolve); child.on('error', reject); });
        const complete = readVideoVisualPlan(workspace, analysis).tasks.slice(group * 6, (group + 1) * 6).every(t => t.status === 'accepted');
        writeFileSync(path.join(run, 'run.json'), JSON.stringify({ group, elapsed_ms: Date.now() - started, exit_code: exit, complete, events }, null, 2));
        if (!complete) throw new Error('Visual executor incomplete; resume the same command');
      } catch (e) { failVideoVisualTask(workspace, analysis, (e as Error).message); throw e; }
    }
    publishVideoVisualUnits(workspace, analysis); await publish?.();
    console.log(JSON.stringify({ analysis, group, status: 'accepted' }));
  }
}
export async function runVideoVisualCli(argv: string[]) {
  const [mode, workspaceArg, analysis, argument, extra, service] = argv, workspace = path.resolve(workspaceArg);
  if (mode === 'serve') {
    const session = createBuildExecutorMcpSession({ bootstrap_version: bootstrap.version, protocol_generation: bootstrap.session_protocol,
      session_private_root: path.join(workspace, '.build/video-visual', analysis), execute_request: createVideoVisualSession(workspace, analysis, argument, Number(extra)) });
    for await (const line of createInterface({ input: process.stdin })) {
      const request = JSON.parse(line), response = session.handle_message(request) as any;
      if (response !== undefined) {
        appendFileSync(path.join(workspace, '.build/video-visual', analysis, 'mcp-events.jsonl'), JSON.stringify({ at: new Date().toISOString(), method: request.method,
          tool: request.params?.name, isError: response.result?.isError ?? false, bytes: Buffer.byteLength(JSON.stringify(response)),
          images: response.result?.content?.filter((c: any) => c.type === 'image').length ?? 0,
          ...(response.result?.isError ? { diagnostic: response.result.content?.[0]?.text } : {}) }) + '\n');
        process.stdout.write(JSON.stringify(response) + '\n');
      }
    }
  } else if (mode === 'prepare') {
    const settings: { options?: VisualScanOptions; ranges?: MediaLocation[] } = argument ? JSON.parse(readFileSync(argument, 'utf8')) : {};
    const plan = await prepareVideoVisualUnits(workspace, analysis, settings.options, settings.ranges);
    console.log(JSON.stringify({ analysis, scan_frames: plan.scan.samples.length, candidates: plan.scan.candidate_indices.length, tasks: plan.tasks.length }));
  } else if (mode === 'run' && existsSync(argument)) {
    await runVideoVisualUnits(workspace, analysis, path.resolve(argument), extra && service ? async () => {
      const receipt = await publishVideoVisualSnapshot(workspace, path.resolve(extra), service);
      appendFileSync(path.join(workspace, '.build/video-visual', analysis, 'publications.jsonl'), JSON.stringify(receipt) + '\n');
    } : undefined);
  } else throw new Error('Expected prepare WORKSPACE ANALYSIS [SETTINGS_JSON] or run WORKSPACE ANALYSIS CODEX_EXE [PUBLISHER SERVICE_ROOT]');
}
if (process.argv[1] && path.resolve(process.argv[1]) === script) await runVideoVisualCli(process.argv.slice(2));
