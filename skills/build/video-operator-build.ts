/** Operator orchestration uses the same durable candidate stores as the video CLI. */
import { existsSync, mkdirSync, readFileSync, renameSync, statSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { importVideo } from '../../packages/core/src/video-import';
import { buildVideoTranscript } from '../../packages/core/src/video-transcript';
import { prepareVideoTranslation } from '../../packages/core/src/video-translation';
import { prepareVideoVisualUnits, readVideoVisualPlan, type VisualUnit } from '../../packages/core/src/video-visual-units';
import { planVideoLectureSemantics } from '../../packages/core/src/video-lecture';
import type { VideoEnrichments } from '../../packages/core/src/video-enrichment';
import type { VideoOperationControl, VideoOperationProgress } from '../../packages/core/src/video-dependencies';
import type { ExternalBuildCostLedger } from '../../packages/core/src/external-build-cost';
import type { VideoSourceManifest } from '../../packages/core/src/generated/VideoSourceManifest';
import type { TranscriptMap } from '../../packages/core/src/generated/TranscriptMap';
import { runVideoTranslation, publishVideoTranslationSnapshot } from './video-translation';
import { runVideoVisualUnits, publishVideoVisualSnapshot } from './video-visual-units';
import { runVideoSemanticSeries, type VideoSemanticJob } from './video-semantic-series';
import type { VideoExecutorControl } from './video-executor-runtime';

export interface OperatorVideoProgress {
  stage: 'import' | 'transcript' | 'translation' | 'visual_scan' | 'visual_frames' | 'visual' | 'graph' | 'discourse' | 'publishing' | 'ready';
  completed: number;
  total: number;
  unit: VideoOperationProgress['unit'] | 'tasks' | 'batches' | 'publications';
  message: string;
  status: 'running' | 'completed' | 'failed' | 'cancelled';
  failure?: string;
}

export interface OperatorVideoBuildInput {
  /** A local MP4, or the existing workspace directory when resuming an imported video. */
  input_path?: string;
  workspace_dir: string;
  codex_executable: string;
  publisher_executable: string;
  publication_service_root: string;
  cost_ledger: ExternalBuildCostLedger;
  /** Personal input preparation uses only local import/transcription; paid CLI enrichment stays operator-owned. */
  personal_source_only?: boolean;
  processing_confirmation?: string;
  signal?: AbortSignal;
  model?: string;
  dependencies?: { ffmpeg?: string; ffprobe?: string; python?: string; modelDir?: string };
  onProgress?: (progress: OperatorVideoProgress) => void | Promise<void>;
}

const progressFile = (workspace: string) => path.join(workspace, '.build/operator-video-progress.json');
const read = <T>(file: string): T => JSON.parse(readFileSync(file, 'utf8'));
const save = (file: string, value: unknown) => {
  mkdirSync(path.dirname(file), { recursive: true });
  writeFileSync(file + '.tmp', JSON.stringify(value, null, 2) + '\n'); renameSync(file + '.tmp', file);
};
export function readOperatorVideoProgress(workspace: string): OperatorVideoProgress | null {
  return existsSync(progressFile(workspace)) ? read(progressFile(workspace)) : null;
}

/** These are source-understanding artifacts. Video discourse is not Tutor teaching material. */
export const OPERATOR_VIDEO_REQUIRED_CAPABILITIES = ['text', 'video', 'translation', 'visual_index', 'graph', 'discourse'] as const;

export async function runOperatorVideoBuild(input: OperatorVideoBuildInput) {
  const workspace = path.resolve(input.workspace_dir), analysis = 'operator-v1';
  const catalog = (): VideoEnrichments | undefined => existsSync(path.join(workspace, 'video_enrichments.json'))
    ? read(path.join(workspace, 'video_enrichments.json')) : undefined;
  const manifest = () => read<VideoSourceManifest>(path.join(workspace, 'video_source_manifest.json'));
  let progress = readOperatorVideoProgress(workspace), pending: Promise<void> = Promise.resolve(), lastProgressAt = 0, progressFailure: unknown;
  function report(next: OperatorVideoProgress) {
    // Progress is derived from copied bytes, decoded positions, completed chunks or accepted candidates.
    if (JSON.stringify(progress) === JSON.stringify(next)) return;
    if (progress?.stage === next.stage && progress.status === next.status && next.completed < next.total
      && next.total === progress.total && next.completed > progress.completed && Date.now() - lastProgressAt < 250) return;
    lastProgressAt = Date.now();
    progress = next; save(progressFile(workspace), next);
    pending = pending.then(() => { if (!progressFailure) return input.onProgress?.(next); })
      .catch(error => { progressFailure ??= error; });
  }
  async function flushProgress() { await pending; if (progressFailure) throw progressFailure; }
  async function stage(next: Omit<OperatorVideoProgress, 'status'>) {
    input.signal?.throwIfAborted(); report({ ...next, status: 'running' }); await flushProgress();
  }
  const operation: VideoOperationControl = { ...input.dependencies, signal: input.signal, onProgress: p => {
    const phase = p.phase === 'visual_scan' || p.phase === 'visual_frames' ? p.phase : progress!.stage;
    report({ stage: phase, completed: p.completed, total: p.total, unit: p.unit, status: 'running', message: progress!.message });
  } };
  const execution = (onProgress: () => void | Promise<void>): VideoExecutorControl => ({ signal: input.signal,
    cost_ledger: input.cost_ledger, model: input.model, onProgress });
  // Existing complete imported contributions are reusable even if they were built by the original CLI.
  function covered(capability: string) {
    const current = catalog();
    if (!current) return false;
    const batches = current.batches.filter(b => b.coverage.some(c => c.capability === capability));
    if (!batches.length || batches.some(b => b.files.some(file => !existsSync(path.join(workspace, file))))) return false;
    const ranges = batches.flatMap(b => b.coverage.filter(c => c.capability === capability).flatMap(c => c.accepted))
      .sort((a, b) => a.start_ms - b.start_ms);
    let end = 0;
    for (const range of ranges) { if (range.start_ms > end) return false; end = Math.max(end, range.end_ms); }
    return end >= manifest().source.duration_ms;
  }
  async function snapshot(translation = false) {
    await stage({ stage: 'publishing', completed: 0, total: 1, unit: 'publications', message: '保存已完成的视频理解成果' });
    if (translation) await publishVideoTranslationSnapshot(workspace, input.publisher_executable, input.publication_service_root, input.signal);
    else await publishVideoVisualSnapshot(workspace, input.publisher_executable, input.publication_service_root, input.signal);
    report({ ...progress!, completed: 1 }); await flushProgress();
  }
  try {
    input.signal?.throwIfAborted();
    if (input.cost_ledger.binding.cost_owner === 'account' && !input.personal_source_only) throw new Error('external_build_personal_payment_unsupported');
    await input.cost_ledger.prepare();
    const hasTranscript = existsSync(path.join(workspace, 'video_source_manifest.json')) && existsSync(path.join(workspace, 'transcript_map.json'));
    const source = input.input_path && existsSync(input.input_path) && statSync(input.input_path).isDirectory() ? undefined : input.input_path;
    if (hasTranscript && source) {
      // A raw MP4 request must retain the same import identity, even when later stages are already ready.
      if (!existsSync(path.join(workspace, 'video_import.json'))) throw new Error('Existing video workspace has no import binding; select the workspace directory or use a new workspace');
      await stage({ stage: 'import', completed: 0, total: 0, unit: 'bytes', message: '确认已有视频与本次来源一致' });
      await importVideo(source, workspace, operation);
    }
    if (!hasTranscript) {
      await stage({ stage: 'import', completed: 0, total: 0, unit: 'bytes', message: '导入视频并检查媒体' });
      await importVideo(source, workspace, operation);
      await stage({ stage: 'transcript', completed: 0, total: 0, unit: 'chunks', message: '逐段转写视频原文' });
      const transcript = await buildVideoTranscript(workspace, { ...operation, ...input.dependencies,
        serviceMeter:input.cost_ledger.transcriptMeter(workspace,input.processing_confirmation) });
      if (transcript.stage !== 'ready') throw new Error(transcript.failure ?? 'Video transcript incomplete; resume missing chunks');
    }
    if (input.personal_source_only) {
      report({stage:'ready',completed:1,total:1,unit:'publications',status:'completed',message:'视频原文已就绪，可继续原工作区学习'});
      await flushProgress();
      return {workspace_dir:workspace,progress:progress!,required_capabilities:['text','video']};
    }
    await stage({ stage: 'translation', completed: 0, total: 0, unit: 'tasks', message: '生成双语字幕' });
    if (!covered('translation')) {
      const update = () => { const plan = prepareVideoTranslation(workspace);
        report({ ...progress!, completed: plan.tasks.filter(t => t.status === 'accepted').length, total: plan.tasks.length }); return flushProgress(); };
      await update();
      await runVideoTranslation(workspace, input.codex_executable, undefined, execution(update));
      await flushProgress();
    }
    await snapshot(!covered('translation') || existsSync(path.join(workspace, 'video_translation_progress.json')));

    if (!covered('visual_index')) {
      await stage({ stage: 'visual_scan', completed: 0, total: manifest().source.duration_ms, unit: 'milliseconds', message: '分析视频画面并准备关键帧' });
      const ranges = [];
      for (let start_ms = 0; start_ms < manifest().source.duration_ms; start_ms += 30000)
        ranges.push({ video: manifest().material.video, start_ms, end_ms: Math.min(start_ms + 30000, manifest().source.duration_ms) });
      await prepareVideoVisualUnits(workspace, analysis, undefined, ranges, operation);
      await stage({ stage: 'visual', completed: 0, total: 0, unit: 'tasks', message: '理解画面、代码和公式' });
      const update = () => { const plan = readVideoVisualPlan(workspace, analysis);
        report({ ...progress!, completed: plan.tasks.filter(t => t.status === 'accepted').length, total: plan.tasks.length }); return flushProgress(); };
      await update();
      await runVideoVisualUnits(workspace, analysis, input.codex_executable, undefined, execution(update));
      await flushProgress();
    }
    await snapshot();
    const jobsFile = path.join(workspace, '.build/operator-video-semantics.json');
    if (!existsSync(jobsFile)) {
      const units = catalog()!.batches.flatMap(b => b.files.filter(f => f.endsWith('/visual_units.json'))
        .flatMap(f => read<{ units: VisualUnit[] }>(path.join(workspace, f)).units));
      const transcript = read<TranscriptMap>(path.join(workspace, 'transcript_map.json'));
      save(jobsFile, planVideoLectureSemantics(units, transcript.segments));
    }
    const selections = read<VideoSemanticJob['selection'][]>(jobsFile);
    for (const semanticStage of ['graph', 'discourse'] as const) {
      if (covered(semanticStage)) continue;
      const jobs = selections.map((selection, index) => ({ batch_id: `operator-${semanticStage}-${String(index + 1).padStart(4, '0')}`, selection }));
      await stage({ stage: semanticStage, completed: 0, total: jobs.length, unit: 'batches',
        message: semanticStage === 'graph' ? '整理视频中的概念与关系' : '梳理原讲解的组织与推导' });
      const update = () => { report({ ...progress!, completed: jobs.filter(job => existsSync(path.join(workspace, `video-enrichments/${job.batch_id}/${semanticStage}.json`))).length }); return flushProgress(); };
      await update();
      await runVideoSemanticSeries(workspace, semanticStage, jobs, input.codex_executable, execution(update));
      await flushProgress();
      await snapshot();
    }
    for (const capability of OPERATOR_VIDEO_REQUIRED_CAPABILITIES.slice(2)) if (!covered(capability))
      throw new Error(`Video ${capability} coverage incomplete; resume accepted contributions`);
    report({ stage: 'ready', completed: 1, total: 1, unit: 'publications', status: 'completed', message: '视频预构建完成，可发布共享工作区' });
    await flushProgress();
    return { workspace_dir: workspace, progress: progress!, required_capabilities: [...OPERATOR_VIDEO_REQUIRED_CAPABILITIES] };
  } catch (error) {
    report({ ...(progress ?? { stage: 'import', completed: 0, total: 0, unit: 'bytes', message: '准备视频预构建' }),
      status: input.signal?.aborted ? 'cancelled' : 'failed', failure: error instanceof Error ? error.message : String(error) });
    await pending;
    throw error;
  }
}
